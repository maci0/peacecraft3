use crate::binding::{Fixture, Reply, SyntheticSource, info};
use pcx_assets::{ArchiveLayer, ContentSource, Resolver, SourceContent, SourceError, SourceInfo};
use peacecraft3::browser_configuration::SelectedMapSources;
use peacecraft3::map_binding::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn mode() -> pcx_assets::GameMode {
    "1.29b-tft-vanilla-vanilla".parse().unwrap()
}
fn source_limit(error: &BindingError) -> bool {
    match error {
        BindingError::Resolve {
            source:
                pcx_assets::Error::Source {
                    source: SourceError::Limit { .. },
                    ..
                },
            ..
        } => true,
        BindingError::Editor { source, .. } => {
            use std::error::Error;
            let mut current: Option<&(dyn Error + 'static)> = Some(source);
            while let Some(error) = current {
                if error
                    .downcast_ref::<SourceError>()
                    .is_some_and(|e| matches!(e, SourceError::Limit { .. }))
                {
                    return true;
                }
                current = error.source();
            }
            false
        }
        _ => false,
    }
}
#[cfg_attr(test, test)]
pub fn bounded_binder_retains_selected_receipts_and_contents() {
    let f = Fixture::new(231);
    let resolver = f.resolver();
    let legacy = bind_map(&resolver, f.input(7), f.limits()).unwrap();
    let bounded = bind_map_bounded(Arc::new(resolver), f.input(7), f.limits()).unwrap();
    assert_eq!(legacy.receipts.as_ref(), bounded.receipts.as_ref());
    assert_eq!(legacy.catalog_map_bytes, bounded.catalog_map_bytes);
    assert_eq!(bounded.receipts.map.provenance.source, f.map_info);
    assert_eq!(bounded.receipts.map.provenance.archive_priority, Some(8));
    assert_eq!(bounded.receipts.common.provenance.archive_priority, Some(5));
    assert_eq!(
        bounded.receipts.blizzard.provenance.archive_priority,
        Some(3)
    );
    assert!(
        bounded
            .receipts
            .catalog
            .iter()
            .all(|r| r.provenance.archive_priority == Some(1))
    );
    assert_eq!(bounded.receipts.map.hash, pcx_mpq::fnv1a64(&f.raw));
}
#[cfg_attr(test, test)]
pub fn map_prelude_catalog_and_remaining_caps_precede_payload_clones() {
    let f = Fixture::new(1);
    let resolver = f.resolver();
    let mut limits = f.limits();
    limits.max_map_file_bytes = f.raw.len() - 1;
    let error = bind_map_bounded(Arc::new(resolver), f.input(7), limits)
        .err()
        .expect("expected refusal");
    assert!(source_limit(&error), "{error:?}");
    assert_eq!(f.map_reads.load(Ordering::SeqCst), 0);
    assert_eq!(f.common_reads.load(Ordering::SeqCst), 0);
    let f = Fixture::new(1);
    let resolver = f.resolver();
    let mut limits = f.limits();
    limits.admission.scripts.max_prelude_bytes = f.common.len() - 1;
    let error = bind_map_bounded(Arc::new(resolver), f.input(7), limits)
        .err()
        .expect("expected refusal");
    assert!(source_limit(&error), "{error:?}");
    assert_eq!(f.common_reads.load(Ordering::SeqCst), 0);
    assert_eq!(f.bj_reads.load(Ordering::SeqCst), 0);
    let f = Fixture::new(1);
    let resolver = f.resolver();
    let mut limits = f.limits();
    limits.admission.scripts.max_prelude_bytes = f.common.len() + f.bj.len() - 1;
    let error = bind_map_bounded(Arc::new(resolver), f.input(7), limits)
        .err()
        .expect("expected refusal");
    assert!(source_limit(&error), "{error:?}");
    assert_eq!(f.common_reads.load(Ordering::SeqCst), 1);
    assert_eq!(f.bj_reads.load(Ordering::SeqCst), 0);
    let f = Fixture::new(1);
    let resolver = f.resolver();
    let mut limits = f.limits();
    limits.reader.max_asset_bytes = f.unit_data.len() - 1;
    let error = bind_map_bounded(Arc::new(resolver), f.input(7), limits)
        .err()
        .expect("expected refusal");
    assert!(source_limit(&error), "{error:?}");
    assert_eq!(f.catalog_reads.load(Ordering::SeqCst), 0);
    let f = Fixture::new(1);
    let resolver = f.resolver();
    let mut limits = f.limits();
    limits.reader.max_aggregate_input_bytes = f.unit_data.len() + f.metadata.len() - 1;
    let error = bind_map_bounded(Arc::new(resolver), f.input(7), limits)
        .err()
        .expect("expected refusal");
    assert!(source_limit(&error), "{error:?}");
    assert_eq!(f.catalog_reads.load(Ordering::SeqCst), 1);
    let f = Fixture::new(1);
    let resolver = f.resolver();
    let mut limits = f.limits();
    limits.max_resolver_input_bytes = f.raw.len()
        + f.member("war3map.w3u").len()
        + f.common.len()
        + f.bj.len()
        + f.unit_data.len().min(f.metadata.len())
        - 1;
    let error = bind_map_bounded(Arc::new(resolver), f.input(7), limits)
        .err()
        .expect("expected refusal");
    assert!(source_limit(&error), "{error:?}");
    assert_eq!(f.catalog_reads.load(Ordering::SeqCst), 0);
}
struct Unsupported {
    info: SourceInfo,
    reads: Arc<AtomicUsize>,
}
impl ContentSource for Unsupported {
    fn info(&self) -> &SourceInfo {
        &self.info
    }
    fn paths(&self) -> Result<Vec<String>, SourceError> {
        Ok(vec!["selected".into()])
    }
    fn read(&self, _: &str) -> Result<Option<SourceContent>, SourceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(SourceContent {
            bytes: vec![3],
            declared_length: 1,
            locator: "legacy exact member".into(),
        }))
    }
}
#[cfg_attr(test, test)]
pub fn unsupported_winner_never_allocates_falls_back_or_changes_selection() {
    let reads = Arc::new(AtomicUsize::new(0));
    let lower = Arc::new(AtomicUsize::new(0));
    let resolver = Resolver::new(
        mode(),
        None,
        vec![
            ArchiveLayer {
                priority: 9,
                source: Box::new(Unsupported {
                    info: info("unsupported winner"),
                    reads: Arc::clone(&reads),
                }),
            },
            ArchiveLayer {
                priority: 1,
                source: Box::new(SyntheticSource {
                    info: info("lower"),
                    assets: BTreeMap::from([("selected".into(), Reply::Bytes(vec![4]))]),
                    reads: Arc::clone(&lower),
                }),
            },
        ],
    )
    .unwrap();
    let error = resolver
        .resolve_bounded("selected", 1)
        .expect_err("expected refusal");
    assert!(
        matches!(error,pcx_assets::Error::Source{layer,source:SourceError::BudgetUnsupported{limit:1},..} if layer=="unsupported winner")
    );
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    assert_eq!(lower.load(Ordering::SeqCst), 0);
    let legacy = resolver.resolve("selected").unwrap();
    assert_eq!(legacy.bytes, [3]);
    assert_eq!(legacy.provenance.archive_priority, Some(9));
    assert_eq!(legacy.provenance.locator, "legacy exact member");
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_eq!(lower.load(Ordering::SeqCst), 0);
    assert!(matches!(
        resolver.resolve_bounded("missing", 0),
        Err(pcx_assets::Error::MissingAsset { .. })
    ));
}
#[cfg_attr(test, test)]
pub fn actual_mpq_adapter_has_explicit_budget_gap_and_legacy_read_stays_valid() {
    let f = Fixture::new(1);
    let map = pcx_map::Map::parse(&f.raw).unwrap();
    let archive =
        pcx_mpq::MpqArchive::from_bytes(map.archive().to_vec(), "caller synthetic memory archive")
            .unwrap();
    let src = pcx_assets::MpqSource::new(
        archive,
        "actual-mpq".into(),
        Some("synthetic".into()),
        vec![pcx_assets::NamedMember {
            asset: "chosen".into(),
            member: "war3map.j".into(),
        }],
        f.member("war3map.j").len() as u64,
    )
    .unwrap();
    let resolver = Resolver::new(
        mode(),
        None,
        vec![ArchiveLayer {
            priority: 4,
            source: Box::new(src),
        }],
    )
    .unwrap();
    assert!(matches!(
        resolver.resolve_bounded("chosen", f.member("war3map.j").len() as u64),
        Err(pcx_assets::Error::Source {
            source: SourceError::BudgetUnsupported { .. },
            ..
        })
    ));
    let old = resolver.resolve("chosen").unwrap();
    assert_eq!(old.bytes, f.member("war3map.j"));
    assert_eq!(old.provenance.locator, "war3map.j");
    assert_eq!(old.provenance.archive_priority, Some(4));
}
#[cfg_attr(test, test)]
pub fn bounded_missing_malformed_and_source_mismatch_remain_distinct() {
    let mut f = Fixture::new(1);
    f.override_reply = Some(("chosen common.j".into(), Reply::Missing));
    assert!(matches!(
        bind_map_bounded(Arc::new(f.resolver()), f.input(7), f.limits()),
        Err(BindingError::Resolve {
            source: pcx_assets::Error::MissingContent { .. },
            ..
        })
    ));
    let mut f = Fixture::new(1);
    f.override_reply = Some(("chosen common.j".into(), Reply::Invalid));
    assert!(matches!(
        bind_map_bounded(Arc::new(f.resolver()), f.input(7), f.limits()),
        Err(BindingError::Resolve {
            source: pcx_assets::Error::Source {
                source: SourceError::InvalidInput { .. },
                ..
            },
            ..
        })
    ));
    let f = Fixture::new(1);
    let wrong = info("wrong declared source");
    let mut input = f.input(7);
    input.map.source = &wrong;
    assert!(matches!(
        bind_map_bounded(Arc::new(f.resolver()), input, f.limits()),
        Err(BindingError::SourceMismatch { .. })
    ));
}

#[cfg_attr(test, test)]
pub fn mounted_mpq_sources_bind_into_the_actual_app_and_preserve_failed_publication() {
    use pcx_assets::{MpqSource, NamedMember};
    use pcx_mpq::{ExtractionBudget, MpqArchive};
    use peacecraft3::map_requests::{MapRequests, RequestFailure};
    fn mounted(info: &SourceInfo, files: &[(&str, &[u8])], packed_limit: usize) -> MpqSource {
        let raw = crate::fixtures::build_synthetic_map("source container", files);
        let map = pcx_map::Map::parse(&raw).unwrap();
        let archive = MpqArchive::from_bytes(map.archive().to_vec(), &info.origin).unwrap();
        MpqSource::new_bounded(
            archive,
            info.id.clone(),
            info.version.clone(),
            files
                .iter()
                .map(|(name, _)| NamedMember {
                    asset: (*name).into(),
                    member: (*name).into(),
                })
                .collect(),
            ExtractionBudget {
                max_raw_packed_bytes: packed_limit,
                max_decoded_bytes: files.iter().map(|(_, bytes)| bytes.len()).max().unwrap(),
            },
            None,
        )
        .unwrap()
    }
    fn sources(f: &Fixture, map_packed_limit: usize) -> Resolver {
        use pcx_objects::editor::path::SlkId;
        Resolver::new(
            mode(),
            None,
            vec![
                ArchiveLayer {
                    priority: 8,
                    source: Box::new(mounted(
                        &f.map_info,
                        &[("selected map.w3m", &f.raw)],
                        map_packed_limit,
                    )),
                },
                ArchiveLayer {
                    priority: 5,
                    source: Box::new(mounted(
                        &f.common_info,
                        &[("chosen common.j", &f.common)],
                        f.common.len(),
                    )),
                },
                ArchiveLayer {
                    priority: 3,
                    source: Box::new(mounted(
                        &f.bj_info,
                        &[("chosen Blizzard.j", &f.bj)],
                        f.bj.len(),
                    )),
                },
                ArchiveLayer {
                    priority: 1,
                    source: Box::new(mounted(
                        &f.catalog_infos[0],
                        &[
                            (SlkId::UnitData.path(), &f.unit_data),
                            (SlkId::UnitMetaData.path(), &f.metadata),
                        ],
                        f.unit_data.len().max(f.metadata.len()),
                    )),
                },
            ],
        )
        .unwrap()
    }
    let f = Fixture::new(231);
    let mut app = bevy::prelude::App::new();
    peacecraft3::map_requests::install(&mut app);
    let resolver = Arc::new(sources(&f, f.raw.len()));
    {
        let mut state = app.world_mut().remove_resource::<MapRequests>().unwrap();
        let mut selected = app
            .world_mut()
            .remove_resource::<SelectedMapSources>()
            .unwrap();
        assert!(
            state
                .replace_selected(&mut selected, Arc::clone(&resolver), f.input(7), f.limits(),)
                .result
                .is_ok()
        );
        app.world_mut().insert_resource(state);
        app.world_mut().insert_resource(selected);
    }
    let old = app
        .world()
        .resource::<MapRequests>()
        .admission
        .shared_context()
        .unwrap();
    let receipts = app
        .world()
        .resource::<MapRequests>()
        .published_receipts
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(old.sources.map, f.map_id);
    assert_eq!(receipts.map.provenance.source, f.map_info);
    assert_eq!(receipts.map.provenance.locator, "selected map.w3m");
    assert_eq!(old.map_digest.hash, pcx_mpq::fnv1a64(&f.raw));
    let request = app
        .world()
        .resource::<MapRequests>()
        .selected_map_request()
        .unwrap();
    app.world_mut().write_message(request);
    app.update();
    assert!(
        app.world()
            .resource::<MapRequests>()
            .last_outcome
            .as_ref()
            .unwrap()
            .result
            .is_ok()
    );
    let kept = app
        .world()
        .resource::<MapRequests>()
        .admission
        .shared_context()
        .unwrap();
    {
        let mut state = app.world_mut().remove_resource::<MapRequests>().unwrap();
        let mut selected = app
            .world_mut()
            .remove_resource::<SelectedMapSources>()
            .unwrap();
        assert!(
            state
                .replace_selected(
                    &mut selected,
                    Arc::new(sources(&f, f.raw.len() - 1)),
                    f.input(8),
                    f.limits(),
                )
                .result
                .is_err()
        );
        app.world_mut().insert_resource(state);
        app.world_mut().insert_resource(selected);
    }
    let state = app.world().resource::<MapRequests>();
    let error = state.last_outcome.as_ref().unwrap();
    assert!(
        matches!(&error.result, Err(RequestFailure::Binding(error)) if matches!(error.as_ref(), BindingError::Resolve { source: pcx_assets::Error::Source { source: SourceError::MpqExtraction(error), .. }, .. } if matches!(error.cause.as_ref(), pcx_mpq::ExtractionCause::Preflight(pcx_mpq::checked::ReadError::BudgetExceeded { member: "packed member", .. }))))
    );
    assert!(Arc::ptr_eq(
        &kept,
        &state.admission.shared_context().unwrap()
    ));
    assert_eq!(
        receipts.as_ref(),
        state.published_receipts.as_ref().unwrap().as_ref()
    );
}
