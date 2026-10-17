use crate::{binding, fixtures};
use bevy::prelude::*;
use pcx_assets::{ArchiveLayer, NamedMember, Resolver, SourceError};
use pcx_assets::{ContentSource, SourceContent, SourceInfo};
use pcx_mpq::{ExtractionBudget, MpqArchive};
use pcx_sim::world::{Definition, ObjectCatalog, ResolutionProblem};
use peacecraft3::browser_archive_source::{ArchiveSelection, CommittedArchiveSource};
use peacecraft3::browser_configuration::*;
use peacecraft3::map_requests::{MapRequests, RequestFailure};
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, AtomicUsize, Ordering},
};
pub(crate) fn fixture(value: u32) -> binding::Fixture {
    let mut f = binding::Fixture::new(value);
    for info in [
        &mut f.map_info,
        &mut f.common_info,
        &mut f.bj_info,
        &mut f.catalog_infos[0],
    ] {
        info.version = None;
    }
    for identity in [
        &mut f.map_id,
        &mut f.common_id,
        &mut f.bj_id,
        &mut f.catalog_id,
    ] {
        identity.edition = "explicit unknown fixture edition".into();
        identity.version = "explicit unknown fixture version".into();
    }
    f
}
pub(crate) fn request(f: &binding::Fixture, resolver: Resolver) -> Configuration {
    let input = f.input(73);
    let member = |m: peacecraft3::map_binding::IdentifiedMember<'_>| Member {
        path: m.path.into(),
        label: m.label.into(),
        identity: m.identity.clone(),
        source: m.source.clone(),
    };
    Configuration {
        resolver,
        map: member(input.map),
        common: member(input.common),
        blizzard: member(input.blizzard),
        catalog_identity: input.catalog_identity.clone(),
        catalog_label: input.catalog_label.into(),
        catalog_sources: input.catalog_sources.to_vec(),
        required_catalog_tables: input.required_catalog_tables.to_vec(),
        required_fields: input.required_fields.to_vec(),
        revision: input.revision,
        limits: f.limits(),
    }
}
pub(crate) fn app(inbox: &ConfigurationInbox) -> bevy::prelude::App {
    let mut app = bevy::prelude::App::new();
    peacecraft3::map_requests::install(&mut app);
    install(&mut app, inbox.clone());
    app
}
#[cfg_attr(test, test)]
pub fn pending_publication_and_acknowledgement() {
    let f = fixture(235);
    let inbox = ConfigurationInbox::default();
    let mut app = app(&inbox);
    let id = inbox.enqueue(request(&f, f.resolver())).unwrap();
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Pending));
    assert!(matches!(
        inbox.enqueue(request(&f, f.resolver())),
        Err(InboxError::Busy)
    ));
    app.update();
    let active = app.world().resource::<MapRequests>();
    assert_eq!(
        active.admission.context().unwrap().map_digest.hash,
        pcx_mpq::fnv1a64(&f.raw)
    );
    assert_eq!(
        active
            .published_receipts
            .as_ref()
            .unwrap()
            .map
            .provenance
            .source
            .version,
        None
    );
    assert!(active.last_outcome.as_ref().unwrap().result.is_ok());
    assert!(matches!(
        inbox.enqueue(request(&f, f.resolver())),
        Err(InboxError::Busy)
    ));
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("not completed")
    };
    assert!(outcome.result.is_ok());
    assert_eq!(outcome.binding_revision, Some(73));
    assert!(matches!(inbox.poll(id), Err(InboxError::Unknown { .. })));
    let next = inbox.enqueue(request(&f, f.resolver())).unwrap();
    assert_ne!(next, id);
}
#[cfg_attr(test, test)]
pub fn failed_configuration_preserves_entire_active_state() {
    let f = fixture(235);
    let inbox = ConfigurationInbox::default();
    let mut app = app(&inbox);
    let id = inbox.enqueue(request(&f, f.resolver())).unwrap();
    app.update();
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(_)));
    let active = app.world().resource::<MapRequests>();
    let context = active.admission.shared_context().unwrap();
    let receipts = active.published_receipts.as_ref().unwrap().clone();
    let bindings_revision = active.bindings.as_ref().unwrap().revision;
    let outcome_label = active.last_outcome.as_ref().unwrap().label.clone();
    let mut failed = fixture(235);
    failed.override_reply = Some(("chosen common.j".into(), binding::Reply::Invalid));
    let id = inbox.enqueue(request(&failed, failed.resolver())).unwrap();
    app.update();
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("not completed")
    };
    assert!(matches!(&outcome.result, Err(RequestFailure::Binding(_))));
    let active = app.world().resource::<MapRequests>();
    assert!(Arc::ptr_eq(
        &context,
        &active.admission.shared_context().unwrap()
    ));
    assert!(Arc::ptr_eq(
        &receipts,
        active.published_receipts.as_ref().unwrap()
    ));
    assert_eq!(
        active.bindings.as_ref().unwrap().revision,
        bindings_revision
    );
    assert_eq!(active.last_outcome.as_ref().unwrap().label, outcome_label);
    assert!(active.last_outcome.as_ref().unwrap().result.is_ok());
}
#[cfg_attr(test, test)]
pub fn runtime_sources_follow_successful_publication_and_receipt_identity() {
    let first = fixture(235);
    let inbox = ConfigurationInbox::default();
    let mut app = app(&inbox);
    let id = inbox.enqueue(request(&first, first.resolver())).unwrap();
    app.update();
    let receipts = app
        .world()
        .resource::<MapRequests>()
        .published_receipts
        .clone()
        .unwrap();
    let sources = app.world().resource::<SelectedMapSources>();
    let reader = sources.for_receipts(&receipts).unwrap();
    assert!(
        sources
            .for_receipts(&Arc::new((*receipts).clone()))
            .is_none()
    );
    assert_eq!(
        reader
            .resolve_bounded("selected map.w3m", first.raw.len() as u64)
            .unwrap()
            .bytes,
        first.raw
    );
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(_)));

    let reload = app
        .world()
        .resource::<MapRequests>()
        .selected_map_request()
        .unwrap();
    app.world_mut().write_message(reload);
    app.update();
    assert!(Arc::ptr_eq(
        &receipts,
        app.world()
            .resource::<MapRequests>()
            .published_receipts
            .as_ref()
            .unwrap()
    ));
    assert!(Arc::ptr_eq(
        &reader,
        &app.world()
            .resource::<SelectedMapSources>()
            .for_receipts(&receipts)
            .unwrap()
    ));

    let mut failed = fixture(235);
    failed.override_reply = Some(("chosen common.j".into(), binding::Reply::Invalid));
    let id = inbox.enqueue(request(&failed, failed.resolver())).unwrap();
    app.update();
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("missing refusal")
    };
    assert!(outcome.result.is_err());
    assert!(Arc::ptr_eq(
        &reader,
        &app.world()
            .resource::<SelectedMapSources>()
            .for_receipts(&receipts)
            .unwrap()
    ));

    let next = fixture(236);
    assert_ne!(first.raw, next.raw);
    let id = inbox.enqueue(request(&next, next.resolver())).unwrap();
    app.update();
    let current = app
        .world()
        .resource::<MapRequests>()
        .published_receipts
        .as_ref()
        .unwrap();
    let sources = app.world().resource::<SelectedMapSources>();
    assert!(sources.for_receipts(&receipts).is_none());
    let replacement = sources.for_receipts(current).unwrap();
    assert!(!Arc::ptr_eq(&reader, &replacement));
    assert_eq!(
        replacement
            .resolve_bounded("selected map.w3m", next.raw.len() as u64)
            .unwrap()
            .bytes,
        next.raw
    );
    // A running consumer may retain its old reader; selection never rewrites it.
    assert_eq!(
        reader
            .resolve_bounded("selected map.w3m", first.raw.len() as u64)
            .unwrap()
            .bytes,
        first.raw
    );
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(_)));
}

#[cfg_attr(test, test)]
pub fn actual_archive_generation_changes_before_consume_refuse() {
    archive_generation_case(true);
    archive_generation_case(false);
}
fn archive_generation_case(change_before: bool) {
    let mut f = fixture(235);
    for (info, identity) in [
        &mut f.map_info,
        &mut f.common_info,
        &mut f.bj_info,
        &mut f.catalog_infos[0],
    ]
    .into_iter()
    .zip([
        &mut f.map_id,
        &mut f.common_id,
        &mut f.bj_id,
        &mut f.catalog_id,
    ]) {
        info.origin = "config-source".into();
        identity.source = "config-source".into();
    }
    let files = [
        ("selected map.w3m", f.raw.as_slice()),
        ("chosen common.j", f.common.as_slice()),
        ("chosen Blizzard.j", f.bj.as_slice()),
        (
            pcx_objects::editor::path::SlkId::UnitData.path(),
            f.unit_data.as_slice(),
        ),
        (
            pcx_objects::editor::path::SlkId::UnitMetaData.path(),
            f.metadata.as_slice(),
        ),
    ];
    let subsets = [&files[..1], &files[1..2], &files[2..3], &files[3..]];
    let mut raw = fixtures::build_synthetic_map("config archive", &files);
    let archives = Box::leak(Box::new(Mutex::new(vec![
        MpqArchive::from_bytes(raw.split_off(0x200), "config-source").unwrap(),
    ])));
    let revision: &'static Mutex<u32> = Box::leak(Box::new(Mutex::new(7u32)));
    let infos = [
        f.map_info.clone(),
        f.common_info.clone(),
        f.bj_info.clone(),
        f.catalog_infos[0].clone(),
    ];
    let layers = infos
        .into_iter()
        .zip(subsets)
        .enumerate()
        .map(|(i, (info, files))| ArchiveLayer {
            priority: 10 - i as u32,
            source: Box::new(
                CommittedArchiveSource::mount_no_huffman(
                    archives,
                    revision,
                    ArchiveSelection::inspect(archives, revision, 0, 7).unwrap(),
                    info,
                    files
                        .iter()
                        .map(|(n, _)| NamedMember {
                            asset: (*n).into(),
                            member: (*n).into(),
                        })
                        .collect(),
                    ExtractionBudget {
                        max_raw_packed_bytes: 1 << 20,
                        max_decoded_bytes: 1 << 20,
                    },
                )
                .unwrap(),
            ),
        })
        .collect();
    let resolver =
        Resolver::new("1.29b-tft-vanilla-vanilla".parse().unwrap(), None, layers).unwrap();
    let inbox = ConfigurationInbox::default();
    let mut app = app(&inbox);
    let id = inbox.enqueue(request(&f, resolver)).unwrap();
    if change_before {
        *revision.lock().unwrap() = 8;
    }
    app.update();
    if !change_before {
        let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
            panic!("not completed")
        };
        assert!(outcome.result.is_ok());
        let active = app.world().resource::<MapRequests>();
        let receipts = active.published_receipts.as_ref().unwrap();
        let reader = app
            .world()
            .resource::<SelectedMapSources>()
            .for_receipts(receipts)
            .unwrap();
        assert_eq!(
            reader
                .resolve_bounded("selected map.w3m", f.raw.len() as u64)
                .unwrap()
                .bytes,
            f.raw
        );
        *revision.lock().unwrap() = 8;
        assert!(matches!(
            reader.resolve_bounded("selected map.w3m", f.raw.len() as u64),
            Err(pcx_assets::Error::Source {
                source: SourceError::Unavailable { .. },
                ..
            })
        ));
        return;
    }
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("not completed")
    };
    assert!(
        matches!(&outcome.result,Err(RequestFailure::Binding(error)) if matches!(error.as_ref(),peacecraft3::map_binding::BindingError::Resolve{source:pcx_assets::Error::Source{source:SourceError::Unavailable{..},..},..}))
    );
    assert!(
        app.world()
            .resource::<MapRequests>()
            .admission
            .context()
            .is_none()
    );
}

struct ArchiveStore {
    archives: &'static Mutex<Vec<MpqArchive>>,
    revision: &'static Mutex<u32>,
}
fn archive_resolver(f: &binding::Fixture, store: &ArchiveStore, generation: u32) -> Resolver {
    let files = [
        ("selected map.w3m", f.raw.as_slice()),
        ("chosen common.j", f.common.as_slice()),
        ("chosen Blizzard.j", f.bj.as_slice()),
        (
            pcx_objects::editor::path::SlkId::UnitData.path(),
            f.unit_data.as_slice(),
        ),
        (
            pcx_objects::editor::path::SlkId::UnitMetaData.path(),
            f.metadata.as_slice(),
        ),
    ];
    let subsets = [&files[..1], &files[1..2], &files[2..3], &files[3..]];
    let infos = [
        f.map_info.clone(),
        f.common_info.clone(),
        f.bj_info.clone(),
        f.catalog_infos[0].clone(),
    ];
    let layers = infos
        .into_iter()
        .zip(subsets)
        .enumerate()
        .map(|(i, (info, files))| {
            let source = CommittedArchiveSource::mount_no_huffman(
                store.archives,
                store.revision,
                ArchiveSelection::inspect(store.archives, store.revision, 0, generation).unwrap(),
                info,
                files
                    .iter()
                    .map(|(name, _)| NamedMember {
                        asset: (*name).into(),
                        member: (*name).into(),
                    })
                    .collect(),
                ExtractionBudget {
                    max_raw_packed_bytes: 1 << 20,
                    max_decoded_bytes: 1 << 20,
                },
            )
            .unwrap();
            ArchiveLayer {
                priority: 10 - i as u32,
                source: Box::new(source),
            }
        })
        .collect();
    Resolver::new("1.29b-tft-vanilla-vanilla".parse().unwrap(), None, layers).unwrap()
}
fn archive_store(f: &binding::Fixture, generation: u32) -> ArchiveStore {
    let files = [
        ("selected map.w3m", f.raw.as_slice()),
        ("chosen common.j", f.common.as_slice()),
        ("chosen Blizzard.j", f.bj.as_slice()),
        (
            pcx_objects::editor::path::SlkId::UnitData.path(),
            f.unit_data.as_slice(),
        ),
        (
            pcx_objects::editor::path::SlkId::UnitMetaData.path(),
            f.metadata.as_slice(),
        ),
    ];
    let mut raw = fixtures::build_synthetic_map("config archive", &files);
    ArchiveStore {
        archives: Box::leak(Box::new(Mutex::new(vec![
            MpqArchive::from_bytes(raw.split_off(0x200), "config-source").unwrap(),
        ]))),
        revision: Box::leak(Box::new(Mutex::new(generation))),
    }
}
fn archive_files(f: &binding::Fixture) -> [(&str, &[u8]); 5] {
    [
        ("selected map.w3m", f.raw.as_slice()),
        ("chosen common.j", f.common.as_slice()),
        ("chosen Blizzard.j", f.bj.as_slice()),
        (
            pcx_objects::editor::path::SlkId::UnitData.path(),
            f.unit_data.as_slice(),
        ),
        (
            pcx_objects::editor::path::SlkId::UnitMetaData.path(),
            f.metadata.as_slice(),
        ),
    ]
}
fn same_metadata_archive(previous: &binding::Fixture, distinguish: bool) -> MpqArchive {
    let files = archive_files(previous);
    let mut raw = fixtures::build_synthetic_map("replacement archive", &files);
    if distinguish {
        fixtures::mark_unused_hash_slot(&mut raw);
    }
    MpqArchive::from_bytes(raw.split_off(0x200), "config-source").unwrap()
}
struct StagedImport {
    archives: Option<Vec<MpqArchive>>,
    revision: u32,
    image: Option<Vec<u8>>,
}
impl StagedImport {
    fn begin(&mut self) {
        self.archives = Some(Vec::new());
        self.image = None;
    }
    fn cancel(&mut self) {
        self.archives = None;
        self.image = None;
    }
    fn stage(&mut self, archive: MpqArchive) {
        self.archives
            .as_mut()
            .expect("an open import")
            .push(archive);
    }
    fn commit(&mut self, published: &Mutex<Vec<MpqArchive>>, revision: &Mutex<u32>) -> u32 {
        let next = self.revision.checked_add(1).expect("revision");
        *published.lock().unwrap() = self.archives.take().expect("an open import");
        self.image = None;
        *revision.lock().unwrap() = next;
        self.revision = next;
        next
    }
}
fn published_pair(
    app: &App,
) -> (
    Arc<peacecraft3::map_binding::BindingReceipts>,
    Arc<Resolver>,
) {
    let active = app.world().resource::<MapRequests>();
    let receipts = active.published_receipts.as_ref().unwrap().clone();
    let reader = app
        .world()
        .resource::<SelectedMapSources>()
        .for_receipts(&receipts)
        .unwrap();
    (receipts, reader)
}
#[cfg_attr(test, test)]
pub fn committed_revision_replacement_invalidates_the_published_reader() {
    let mut first = fixture(235);
    for (info, identity) in [
        &mut first.map_info,
        &mut first.common_info,
        &mut first.bj_info,
        &mut first.catalog_infos[0],
    ]
    .into_iter()
    .zip([
        &mut first.map_id,
        &mut first.common_id,
        &mut first.bj_id,
        &mut first.catalog_id,
    ]) {
        info.origin = "config-source".into();
        identity.source = "config-source".into();
    }
    let store = archive_store(&first, 7);
    let inbox = ConfigurationInbox::default();
    let mut app = app(&inbox);
    let id = inbox
        .enqueue(request(&first, archive_resolver(&first, &store, 7)))
        .unwrap();
    app.update();
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(_)));
    let (receipts, reader) = published_pair(&app);
    let before_len = store.archives.lock().unwrap()[0].byte_len();
    let before_header = store.archives.lock().unwrap()[0].header;
    let before = store.archives.lock().unwrap()[0].hash_entries().to_vec();
    assert_eq!(
        reader
            .resolve_bounded("selected map.w3m", first.raw.len() as u64)
            .unwrap()
            .bytes,
        first.raw
    );

    let mut staged = StagedImport {
        archives: None,
        revision: 7,
        image: Some(b"pending client image".to_vec()),
    };
    staged.begin();
    staged.stage(same_metadata_archive(&first, false));
    staged.cancel();
    assert!(staged.archives.is_none() && staged.image.is_none());
    assert_eq!(*store.revision.lock().unwrap(), 7);
    assert_eq!(store.archives.lock().unwrap()[0].hash_entries(), before);
    assert!(Arc::ptr_eq(
        &reader,
        &app.world()
            .resource::<SelectedMapSources>()
            .for_receipts(&receipts)
            .unwrap()
    ));

    let mut failed = fixture(235);
    failed.override_reply = Some(("chosen common.j".into(), binding::Reply::Invalid));
    let id = inbox.enqueue(request(&failed, failed.resolver())).unwrap();
    app.update();
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("missing refusal")
    };
    assert!(outcome.result.is_err());
    staged.cancel();
    let (kept_receipts, kept_reader) = published_pair(&app);
    assert!(Arc::ptr_eq(&receipts, &kept_receipts));
    assert!(Arc::ptr_eq(&reader, &kept_reader));
    assert_eq!(
        kept_reader
            .resolve_bounded("selected map.w3m", first.raw.len() as u64)
            .unwrap()
            .bytes,
        first.raw
    );
    drop(kept_reader);

    staged.begin();
    staged.stage(same_metadata_archive(&first, true));
    let generation = staged.commit(store.archives, store.revision);
    assert_eq!(generation, 8);
    let committed = &store.archives.lock().unwrap()[0];
    assert_eq!(committed.path().to_str(), Some("config-source"));
    assert_eq!(committed.byte_len(), before_len);
    assert_eq!(committed.header, before_header);
    assert_ne!(committed.hash_entries(), before);
    assert!(matches!(
        reader.resolve_bounded("selected map.w3m", first.raw.len() as u64),
        Err(pcx_assets::Error::Source {
            source: SourceError::Unavailable { .. },
            ..
        })
    ));
    drop(reader);
    let id = inbox
        .enqueue(request(
            &first,
            archive_resolver(&first, &store, generation),
        ))
        .unwrap();
    app.update();
    let current = app
        .world()
        .resource::<MapRequests>()
        .published_receipts
        .as_ref()
        .unwrap()
        .clone();
    assert!(
        app.world()
            .resource::<SelectedMapSources>()
            .for_receipts(&receipts)
            .is_none()
    );
    let replacement = app
        .world()
        .resource::<SelectedMapSources>()
        .for_receipts(&current)
        .unwrap();
    assert_eq!(
        replacement
            .resolve_bounded("selected map.w3m", first.raw.len() as u64)
            .unwrap()
            .bytes,
        first.raw
    );
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(_)));
}

struct Reentrant {
    base: binding::SyntheticSource,
    callback: Arc<dyn Fn() + Send + Sync>,
}
impl ContentSource for Reentrant {
    fn info(&self) -> &SourceInfo {
        self.base.info()
    }
    fn paths(&self) -> Result<Vec<String>, SourceError> {
        self.base.paths()
    }
    fn read(&self, path: &str) -> Result<Option<SourceContent>, SourceError> {
        (self.callback)();
        self.base.read(path)
    }
    fn read_bounded(&self, path: &str, limit: u64) -> Result<Option<SourceContent>, SourceError> {
        (self.callback)();
        self.base.read_bounded(path, limit)
    }
}
fn reentrant_resolver(f: &binding::Fixture, callback: Arc<dyn Fn() + Send + Sync>) -> Resolver {
    let mk =
        |info: SourceInfo, assets: BTreeMap<String, binding::Reply>| binding::SyntheticSource {
            info,
            assets,
            reads: Arc::new(AtomicUsize::new(0)),
        };
    Resolver::new(
        "1.29b-tft-vanilla-vanilla".parse().unwrap(),
        None,
        vec![
            ArchiveLayer {
                priority: 8,
                source: Box::new(Reentrant {
                    base: mk(
                        f.map_info.clone(),
                        BTreeMap::from([(
                            "selected map.w3m".into(),
                            binding::Reply::Bytes(f.raw.clone()),
                        )]),
                    ),
                    callback,
                }),
            },
            ArchiveLayer {
                priority: 5,
                source: Box::new(mk(
                    f.common_info.clone(),
                    BTreeMap::from([(
                        "chosen common.j".into(),
                        binding::Reply::Bytes(f.common.clone()),
                    )]),
                )),
            },
            ArchiveLayer {
                priority: 3,
                source: Box::new(mk(
                    f.bj_info.clone(),
                    BTreeMap::from([(
                        "chosen Blizzard.j".into(),
                        binding::Reply::Bytes(f.bj.clone()),
                    )]),
                )),
            },
            ArchiveLayer {
                priority: 1,
                source: Box::new(mk(
                    f.catalog_infos[0].clone(),
                    BTreeMap::from([
                        (
                            pcx_objects::editor::path::SlkId::UnitData.path().into(),
                            binding::Reply::Bytes(f.unit_data.clone()),
                        ),
                        (
                            pcx_objects::editor::path::SlkId::UnitMetaData.path().into(),
                            binding::Reply::Bytes(f.metadata.clone()),
                        ),
                    ]),
                )),
            },
        ],
    )
    .unwrap()
}
#[cfg_attr(test, test)]
pub fn source_reenters_poll_enqueue_and_second_consumer() {
    let f = fixture(235);
    let inbox = ConfigurationInbox::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let job = Arc::new(AtomicU32::new(0));
    let spare = Arc::new(Mutex::new(Some(request(&f, f.resolver()))));
    let callback = {
        let inbox = inbox.clone();
        let calls = calls.clone();
        let job = job.clone();
        let spare = spare.clone();
        Arc::new(move || {
            // Deterministic private-lock precheck lives in the cfg(test) child module.

            let id = job.load(Ordering::SeqCst);
            assert!(matches!(inbox.poll(id).unwrap(), Poll::Pending));
            assert!(matches!(
                inbox.enqueue(spare.lock().unwrap().take().unwrap()),
                Err(InboxError::Busy)
            ));
            let mut second = app(&inbox);
            second.update();
            let active = second.world().resource::<MapRequests>();
            assert!(active.admission.context().is_none());
            assert!(active.last_outcome.is_none());
            assert!(matches!(inbox.poll(id).unwrap(), Poll::Pending));
            calls.fetch_add(1, Ordering::SeqCst);
        })
    };
    let id = inbox
        .enqueue(request(&f, reentrant_resolver(&f, callback)))
        .unwrap();
    job.store(id, Ordering::SeqCst);
    let mut primary = app(&inbox);
    primary.update();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        primary
            .world()
            .resource::<MapRequests>()
            .admission
            .context()
            .is_some()
    );
    // A consumer arriving after completion must neither consume nor overwrite it.
    let mut second = app(&inbox);
    second.update();
    assert!(
        second
            .world()
            .resource::<MapRequests>()
            .admission
            .context()
            .is_none()
    );
    assert!(matches!(
        inbox.enqueue(request(&f, f.resolver())),
        Err(InboxError::Busy)
    ));
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("missing completion")
    };
    assert!(outcome.result.is_ok());
    assert_eq!(outcome.binding_revision, Some(73));
    assert!(matches!(inbox.poll(id), Err(InboxError::Unknown { .. })));
    let next = inbox.enqueue(request(&f, f.resolver())).unwrap();
    assert_eq!(next, id + 1);
}
struct CatalogDrop {
    inner: Arc<dyn ObjectCatalog + Send + Sync>,
    inbox: ConfigurationInbox,
    drops: Arc<AtomicUsize>,
}
impl ObjectCatalog for CatalogDrop {
    fn resolve(&self, id: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        self.inner.resolve(id)
    }
}
impl Drop for CatalogDrop {
    fn drop(&mut self) {
        assert!(matches!(
            self.inbox.poll(0),
            Err(InboxError::Unknown { id: 0 })
        ));
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
#[cfg_attr(test, test)]
pub fn retired_active_catalog_drops_outside_mutex() {
    let f = fixture(235);
    let inbox = ConfigurationInbox::default();
    let mut app = app(&inbox);
    let id = inbox.enqueue(request(&f, f.resolver())).unwrap();
    app.update();
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(_)));
    let drops = Arc::new(AtomicUsize::new(0));
    {
        let mut active = app.world_mut().resource_mut::<MapRequests>();
        let catalog = &mut active.bindings.as_mut().unwrap().catalog.0;
        *catalog = Arc::new(CatalogDrop {
            inner: catalog.clone(),
            inbox: inbox.clone(),
            drops: drops.clone(),
        });
    }
    let id = inbox.enqueue(request(&f, f.resolver())).unwrap();
    app.update();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("no completion")
    };
    assert!(outcome.result.is_ok());
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_configuration_consumer_gate() -> u32 {
    pending_publication_and_acknowledgement();
    failed_configuration_preserves_entire_active_state();
    actual_archive_generation_changes_before_consume_refuse();
    source_reenters_poll_enqueue_and_second_consumer();
    retired_active_catalog_drops_outside_mutex();
    runtime_sources_follow_successful_publication_and_receipt_identity();
    committed_revision_replacement_invalidates_the_published_reader();
    7
}
