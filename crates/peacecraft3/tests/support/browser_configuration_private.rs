//! Private bookkeeping/lock checks; no asset parser fixture is duplicated here.
use super::*;
use pcx_assets::{ArchiveLayer, ContentSource, LayerKind, SourceContent, SourceError};
use pcx_objects::editor::EditorReadLimits;
use pcx_play::{map_admission::MapAdmissionBudget, map_scripts_import::MapScriptBudget};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

struct ProbeSource {
    info: SourceInfo,
    read: Arc<dyn Fn() + Send + Sync>,
    retired: Arc<dyn Fn() + Send + Sync>,
}
impl ContentSource for ProbeSource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }
    fn paths(&self) -> Result<Vec<String>, SourceError> {
        Ok(vec!["selected.w3m".into()])
    }
    fn read(&self, _: &str) -> Result<Option<SourceContent>, SourceError> {
        (self.read)();
        Err(SourceError::Unavailable {
            detail: "explicit synthetic callback refusal".into(),
        })
    }
    fn read_bounded(&self, path: &str, _: u64) -> Result<Option<SourceContent>, SourceError> {
        self.read(path)
    }
}
impl Drop for ProbeSource {
    fn drop(&mut self) {
        (self.retired)();
    }
}

// This request always refuses at the first source read. Ones below are explicit
// synthetic read ceilings, not admission defaults or a fabricated asset fixture.
fn request(
    read: Arc<dyn Fn() + Send + Sync>,
    retired: Arc<dyn Fn() + Send + Sync>,
) -> Configuration {
    let source = SourceInfo {
        id: "probe".into(),
        kind: LayerKind::Mpq,
        origin: "synthetic callback source".into(),
        version: None,
    };
    let identity = AssetIdentity {
        source: "synthetic callback source".into(),
        edition: "explicit synthetic edition".into(),
        version: "explicit synthetic version".into(),
    };
    let member = || Member {
        path: "selected.w3m".into(),
        label: "synthetic refused member".into(),
        identity: identity.clone(),
        source: source.clone(),
    };
    let resolver = Resolver::new(
        "1.29b-tft-vanilla-vanilla".parse().unwrap(),
        None,
        vec![ArchiveLayer {
            priority: 1,
            source: Box::new(ProbeSource {
                info: source.clone(),
                read,
                retired,
            }),
        }],
    )
    .unwrap();
    Configuration {
        resolver,
        campaign: None,
        map: member(),
        common: member(),
        blizzard: member(),
        catalog_identity: identity.clone(),
        catalog_label: "explicit synthetic catalog".into(),
        catalog_sources: vec![source],
        required_catalog_tables: vec![],
        required_fields: vec![],
        revision: 1,
        limits: BindingLimits {
            reader: EditorReadLimits {
                max_listed_paths: 1,
                max_asset_bytes: 1,
                max_aggregate_input_bytes: 1,
            },
            overlay: crate::map_binding::OverlayLimits {
                unit_bytes: 1,
                item_bytes: 1,
                aggregate_bytes: 1,
            },
            admission: MapAdmissionBudget {
                scripts: MapScriptBudget {
                    max_script_bytes: 1,
                    max_wts_bytes: 1,
                    max_archive_bytes: 1,
                    max_archive_table_bytes: 1,
                    max_raw_packed_bytes: 1,
                    max_decoded_bytes: 1,
                    max_prelude_bytes: 1,
                    max_aggregate_input_bytes: 1,
                },
                terrain_bytes: 1,
                info_bytes: 1,
                pathing_bytes: 1,
                units_bytes: 1,
                aggregate_input_bytes: 1,
                required_fields: 0,
            },
            max_map_file_bytes: 1,
            max_resolver_input_bytes: 1,
        },
    }
}
fn app(inbox: &ConfigurationInbox) -> App {
    let mut app = App::new();
    crate::map_requests::install(&mut app);
    install(&mut app, inbox.clone());
    app
}
fn drop_probe(inbox: &ConfigurationInbox, count: &Arc<AtomicUsize>) -> Arc<dyn Fn() + Send + Sync> {
    let inbox = inbox.clone();
    let count = count.clone();
    Arc::new(move || {
        assert!(
            inbox.0.try_lock().is_ok(),
            "request source Drop ran under inbox guard"
        );
        assert!(matches!(inbox.poll(0), Err(InboxError::Unknown { id: 0 })));
        count.fetch_add(1, Ordering::SeqCst);
    })
}
#[cfg_attr(test, test)]
pub fn returned_source_error_and_request_drop_reenter_without_guard() {
    let inbox = ConfigurationInbox::default();
    let job = Arc::new(AtomicU32::new(0));
    let reads = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let read = {
        let inbox = inbox.clone();
        let job = job.clone();
        let reads = reads.clone();
        Arc::new(move || {
            // The deterministic precheck prevents a regressed blocking call hanging.
            assert!(
                inbox.0.try_lock().is_ok(),
                "source read ran under inbox guard"
            );
            let id = job.load(Ordering::SeqCst);
            assert!(matches!(inbox.poll(id).unwrap(), Poll::Pending));
            assert!(matches!(
                inbox.enqueue(request(Arc::new(|| {}), Arc::new(|| {}))),
                Err(InboxError::Busy)
            ));
            let mut second = app(&inbox);
            second.update();
            assert!(
                second
                    .world()
                    .resource::<MapRequests>()
                    .last_outcome
                    .is_none()
            );
            assert!(matches!(inbox.poll(id).unwrap(), Poll::Pending));
            reads.fetch_add(1, Ordering::SeqCst);
        })
    };
    let id = inbox
        .enqueue(request(read, drop_probe(&inbox, &drops)))
        .unwrap();
    job.store(id, Ordering::SeqCst);
    let mut app = app(&inbox);
    app.update();
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("missing typed failure")
    };
    assert!(
        matches!(outcome.result,Err(crate::map_requests::RequestFailure::Binding(error)) if matches!(error.as_ref(),crate::map_binding::BindingError::Resolve { source: pcx_assets::Error::Source { source: SourceError::Unavailable { .. }, .. }, .. }))
    );
    assert!(
        app.world()
            .resource::<MapRequests>()
            .admission
            .context()
            .is_none()
    );
}
#[cfg_attr(test, test)]
pub fn exhaustion_busy_and_rejected_request_drop_preserve_ownership() {
    let inbox = ConfigurationInbox::default();
    let drops = Arc::new(AtomicUsize::new(0));
    inbox.0.lock().unwrap().next_id = u32::MAX - 1;
    let id = inbox
        .enqueue(request(Arc::new(|| {}), Arc::new(|| {})))
        .unwrap();
    assert_eq!(id, u32::MAX);
    assert!(matches!(
        inbox.enqueue(request(Arc::new(|| {}), drop_probe(&inbox, &drops))),
        Err(InboxError::Busy)
    ));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Pending));
    let mut app = app(&inbox);
    app.update();
    assert!(matches!(
        inbox.enqueue(request(Arc::new(|| {}), drop_probe(&inbox, &drops))),
        Err(InboxError::Busy)
    ));
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(_)));
    assert!(matches!(
        inbox.enqueue(request(Arc::new(|| {}), drop_probe(&inbox, &drops))),
        Err(InboxError::Exhausted)
    ));
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    assert!(matches!(inbox.poll(id), Err(InboxError::Unknown { .. })));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_configuration_private_gate() -> u32 {
    returned_source_error_and_request_drop_reenter_without_guard();
    exhaustion_busy_and_rejected_request_drop_preserve_ownership();
    2
}
