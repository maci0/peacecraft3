//! Test-only acceptance against the application's real committed browser store.
use crate::{binding, browser, fixtures};
use pcx_assets::{ArchiveLayer, ContentSource, NamedMember, Resolver, SourceError, SourceInfo};
use pcx_mpq::{ExtractionBudget, ExtractionCause, checked::ReadError};
use peacecraft3::{
    map_binding::BindingError,
    map_requests::{MapRequests, RequestFailure},
};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

fn append(name: &str, files: &[(&str, &[u8])]) {
    let mut bytes = fixtures::build_synthetic_map("application source fixture", files);
    browser::add_archive(name.into(), bytes.split_off(0x200));
}
fn mounted(
    index: usize,
    revision: u32,
    info: SourceInfo,
    names: Vec<NamedMember>,
    budget: ExtractionBudget,
) -> peacecraft3::browser_archive_source::CommittedArchiveSource {
    let selection = browser::committed_archive_selection(index, revision).unwrap();
    assert_eq!(selection.archive_name, info.origin);
    browser::committed_archive_source(selection, info, names, budget).unwrap()
}

#[wasm_bindgen]
pub fn browser_application_source_gate() -> u32 {
    let before = browser::asset_revision();
    browser::drop_assets();
    let revision = browser::asset_revision();
    assert_eq!(Some(revision), before.checked_add(1));
    assert!(browser::committed_archive_selection(0, revision).is_err());
    let mut f = binding::Fixture::new(233);
    let origins = [
        "gate-map.mpq",
        "gate-common.mpq",
        "gate-bj.mpq",
        "gate-catalog.mpq",
    ];
    for ((info, id), origin) in [
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
    ])
    .zip(origins)
    {
        info.origin = origin.into();
        info.version = None;
        id.source = origin.into();
        id.edition = "explicit unknown fixture edition".into();
        id.version = "explicit unknown fixture version".into();
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
    for (name, files) in origins.into_iter().zip(subsets) {
        append(name, files);
    }
    // Legacy append preserves selected indices but does not attest inventory revision.
    assert_eq!(browser::asset_revision(), revision);
    assert!(browser::committed_archive_selection(usize::MAX, revision).is_err());
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
        .map(|(index, (info, files))| ArchiveLayer {
            priority: 10 - index as u32,
            source: Box::new(mounted(
                index,
                revision,
                info,
                files
                    .iter()
                    .map(|(member, _)| NamedMember {
                        asset: (*member).into(),
                        member: (*member).into(),
                    })
                    .collect(),
                ExtractionBudget {
                    max_raw_packed_bytes: 1 << 20,
                    max_decoded_bytes: 1 << 20,
                },
            )),
        })
        .collect();
    let resolver =
        Resolver::new("1.29b-tft-vanilla-vanilla".parse().unwrap(), None, layers).unwrap();
    let mut app = bevy::prelude::App::new();
    peacecraft3::map_requests::install(&mut app);
    assert!(
        app.world_mut()
            .resource_mut::<MapRequests>()
            .replace_selected(&resolver, f.input(31), f.limits())
            .result
            .is_ok()
    );
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
    assert_eq!(old.map_digest.hash, pcx_mpq::fnv1a64(&f.raw));
    assert_eq!(receipts.map.provenance.source, f.map_info);
    assert_eq!(receipts.map.provenance.locator, "selected map.w3m");

    browser::drop_assets();
    assert_ne!(browser::asset_revision(), revision);
    assert!(browser::committed_archive_selection(0, revision).is_err());
    {
        let mut state = app.world_mut().resource_mut::<MapRequests>();
        let refused = state.replace_selected(&resolver, f.input(32), f.limits());
        assert!(
            matches!(&refused.result, Err(RequestFailure::Binding(error)) if matches!(error.as_ref(), BindingError::Resolve { source: pcx_assets::Error::Source { source: SourceError::Unavailable { .. }, .. }, .. }))
        );
    }
    let state = app.world().resource::<MapRequests>();
    assert!(Arc::ptr_eq(
        &old,
        &state.admission.shared_context().unwrap()
    ));
    assert!(Arc::ptr_eq(
        &receipts,
        state.published_receipts.as_ref().unwrap()
    ));
    explicit_selection_without_fallback();
    browser::drop_assets();
    3
}

fn explicit_selection_without_fallback() {
    let revision = browser::asset_revision();
    let mut raw =
        fixtures::build_synthetic_map("oversized selected member", &[("fixture", &[2, 3])]);
    fixtures::alter_block(&mut raw, "fixture", |block| {
        block[8..12].copy_from_slice(&3u32.to_le_bytes())
    });
    browser::add_archive("gate-refused.mpq".into(), raw.split_off(0x200));
    append("gate-valid.mpq", &[("fixture", &[7, 8])]);
    let source = |index, name: &str, id: &str| {
        mounted(
            index,
            revision,
            SourceInfo {
                id: id.into(),
                kind: pcx_assets::LayerKind::Mpq,
                origin: name.into(),
                version: None,
            },
            vec![NamedMember {
                asset: "logical/input".into(),
                member: "fixture".into(),
            }],
            ExtractionBudget {
                max_raw_packed_bytes: 2,
                max_decoded_bytes: 2,
            },
        )
    };
    let refused = source(0, "gate-refused.mpq", "selected refusal");
    let valid = source(1, "gate-valid.mpq", "selected valid");
    assert_eq!(
        valid
            .read_bounded("logical/input", 2)
            .unwrap()
            .unwrap()
            .bytes,
        [7, 8]
    );
    let resolver = Resolver::new(
        "1.29b-tft-vanilla-vanilla".parse().unwrap(),
        None,
        vec![
            ArchiveLayer {
                priority: 2,
                source: Box::new(refused),
            },
            ArchiveLayer {
                priority: 1,
                source: Box::new(valid),
            },
        ],
    )
    .unwrap();
    assert!(
        matches!(resolver.resolve_bounded("logical/input", 2), Err(pcx_assets::Error::Source { layer, source: SourceError::MpqExtraction(error), .. }) if layer == "selected refusal" && matches!(error.cause.as_ref(), ExtractionCause::Preflight(ReadError::BudgetExceeded { .. })))
    );
}
