use crate::browser_bounded_cases::unreachable_fixture as unreachable;
use crate::{binding, fixtures};
use pcx_assets::{ArchiveLayer, ContentSource, NamedMember, Resolver, SourceError, SourceInfo};
use pcx_mpq::{ExtractionBudget, MpqArchive};
use peacecraft3::{
    browser_archive_source::{ArchiveSelection, CommittedArchiveSource},
    map_requests::MapRequests,
};
use std::sync::{Arc, Mutex};
struct Store {
    archives: &'static Mutex<Vec<MpqArchive>>,
    revision: &'static Mutex<u32>,
}
fn store(files: &[(&str, &[u8])], name: &str) -> Store {
    let mut raw = fixtures::build_synthetic_map("source container", files);
    let archive = MpqArchive::from_bytes(raw.split_off(0x200), name).unwrap();
    Store {
        archives: Box::leak(Box::new(Mutex::new(vec![archive]))),
        revision: Box::leak(Box::new(Mutex::new(17))),
    }
}
fn mount(
    store: &Store,
    info: SourceInfo,
    names: Vec<NamedMember>,
    packed: usize,
    decoded: usize,
) -> CommittedArchiveSource {
    let selected = ArchiveSelection::inspect(store.archives, store.revision, 0, 17).unwrap();
    CommittedArchiveSource::mount_no_huffman(
        store.archives,
        store.revision,
        selected,
        info,
        names,
        ExtractionBudget {
            max_raw_packed_bytes: packed,
            max_decoded_bytes: decoded,
        },
    )
    .unwrap()
}
fn unknown(name: &str, id: &str) -> SourceInfo {
    SourceInfo {
        id: id.into(),
        kind: pcx_assets::LayerKind::Mpq,
        origin: name.into(),
        version: None,
    }
}
#[cfg_attr(test, test)]
pub fn exact_selected_member_and_identity() {
    let store = store(
        &[("Physical\\ExactName", &[2, 3])],
        "explicit committed archive",
    );
    let source = mount(
        &store,
        unknown("explicit committed archive", "selected"),
        vec![NamedMember {
            asset: "logical/input".into(),
            member: "Physical\\ExactName".into(),
        }],
        2,
        2,
    );
    assert_eq!(source.paths().unwrap(), ["logical/input"]);
    assert_eq!(source.info().version, None);
    let read = source.read_bounded("logical/input", 2).unwrap().unwrap();
    assert_eq!(read.bytes, [2, 3]);
    assert_eq!(read.locator, "Physical\\ExactName");
    assert_eq!(read.declared_length, 2);
    assert!(source.read_bounded("not advertised", 0).unwrap().is_none());
    assert!(matches!(
        source.read_bounded("logical/input", 1),
        Err(SourceError::MpqExtraction(_))
    ));
    // Legacy append does not advance revision: selected instance remains valid,
    // but this token cannot attest the complete archive inventory.
    let mut raw = fixtures::build_synthetic_map("append", &[("fixture", &[9])]);
    store
        .archives
        .lock()
        .unwrap()
        .push(MpqArchive::from_bytes(raw.split_off(0x200), "appended archive").unwrap());
    assert_eq!(*store.revision.lock().unwrap(), 17);
    assert_eq!(
        source
            .read_bounded("logical/input", 2)
            .unwrap()
            .unwrap()
            .bytes,
        [2, 3]
    );
    *store.revision.lock().unwrap() = 18;
    assert!(matches!(
        source.read_bounded("logical/input", 2),
        Err(SourceError::Unavailable { .. })
    ));
    assert!(source.paths().is_err());
}
#[cfg_attr(test, test)]
pub fn identity_mismatch_and_explicit_inventory() {
    let store = store(&[("fixture", &[2, 3])], "selected archive");
    let mut selection = ArchiveSelection::inspect(store.archives, store.revision, 0, 17).unwrap();
    selection.archive_bytes += 1;
    assert!(matches!(
        CommittedArchiveSource::mount_no_huffman(
            store.archives,
            store.revision,
            selection,
            unknown("selected archive", "selected"),
            vec![],
            ExtractionBudget {
                max_raw_packed_bytes: 2,
                max_decoded_bytes: 2
            }
        ),
        Err(SourceError::Unavailable { .. })
    ));
    let selected = ArchiveSelection::inspect(store.archives, store.revision, 0, 17).unwrap();
    assert!(matches!(
        CommittedArchiveSource::mount_no_huffman(
            store.archives,
            store.revision,
            selected,
            unknown("selected archive", "selected"),
            vec![
                NamedMember {
                    asset: "name".into(),
                    member: "fixture".into()
                },
                NamedMember {
                    asset: "NAME".into(),
                    member: "fixture".into()
                }
            ],
            ExtractionBudget {
                max_raw_packed_bytes: 2,
                max_decoded_bytes: 2
            }
        ),
        Err(SourceError::Ambiguous { .. })
    ));
    let source = mount(
        &store,
        unknown("selected archive", "selected"),
        vec![NamedMember {
            asset: "declared".into(),
            member: "absent raw member".into(),
        }],
        2,
        2,
    );
    assert!(source.read_bounded("declared", 2).unwrap().is_none());
}
#[cfg_attr(test, test)]
pub fn malformed_is_not_missing_or_cache() {
    let store = store(&[("fixture", &[2, 3])], "selected archive");
    let source = mount(
        &store,
        unknown("selected archive", "selected"),
        vec![NamedMember {
            asset: "logical".into(),
            member: "fixture".into(),
        }],
        1,
        2,
    );
    assert!(matches!(
        source.read_bounded("logical", 2),
        Err(SourceError::MpqExtraction(_))
    ));
    // Replacing the instance without changing generation still fails metadata identity.
    let mut raw = fixtures::build_synthetic_map("replacement", &[("fixture", &[4, 5, 6])]);
    store.archives.lock().unwrap()[0] =
        MpqArchive::from_bytes(raw.split_off(0x200), "replacement archive").unwrap();
    assert!(matches!(
        source.read_bounded("logical", 2),
        Err(SourceError::Unavailable { .. })
    ));
}
#[cfg_attr(test, test)]
pub fn resolver_to_actual_app_and_stale_rollback() {
    let mut f = binding::Fixture::new(231);
    for info in [
        &mut f.map_info,
        &mut f.common_info,
        &mut f.bj_info,
        &mut f.catalog_infos[0],
    ] {
        info.version = None;
    }
    for id in [
        &mut f.map_id,
        &mut f.common_id,
        &mut f.bj_id,
        &mut f.catalog_id,
    ] {
        id.version = "explicit unknown release label".into();
        id.edition = "explicit unknown edition label".into();
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
    let stores = [
        store(&files[..1], &f.map_info.origin),
        store(&files[1..2], &f.common_info.origin),
        store(&files[2..3], &f.bj_info.origin),
        store(&files[3..], &f.catalog_infos[0].origin),
    ];
    let infos = [
        f.map_info.clone(),
        f.common_info.clone(),
        f.bj_info.clone(),
        f.catalog_infos[0].clone(),
    ];
    let subsets = [&files[..1], &files[1..2], &files[2..3], &files[3..]];
    let layers = stores
        .iter()
        .zip(infos)
        .zip(subsets)
        .enumerate()
        .map(|(i, ((store, info), subset))| ArchiveLayer {
            priority: 10 - i as u32,
            source: Box::new(mount(
                store,
                info,
                subset
                    .iter()
                    .map(|(name, _)| NamedMember {
                        asset: (*name).into(),
                        member: (*name).into(),
                    })
                    .collect(),
                1 << 20,
                1 << 20,
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
            .replace_selected(&resolver, f.input(7), f.limits())
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
    assert_eq!(receipts.map.provenance.source.version, None);
    assert_eq!(receipts.map.provenance.locator, "selected map.w3m");
    assert_eq!(old.map_digest.hash, pcx_mpq::fnv1a64(&f.raw));
    *stores[0].revision.lock().unwrap() = 18;
    assert!(
        app.world_mut()
            .resource_mut::<MapRequests>()
            .replace_selected(&resolver, f.input(8), f.limits())
            .result
            .is_err()
    );
    let current = app.world().resource::<MapRequests>();
    assert!(Arc::ptr_eq(
        &old,
        &current.admission.shared_context().unwrap()
    ));
    assert!(Arc::ptr_eq(
        &receipts,
        current.published_receipts.as_ref().unwrap()
    ));
}

#[cfg_attr(test, test)]
pub fn unreachable_hash_selection_is_error() {
    let archives = Box::leak(Box::new(Mutex::new(vec![unreachable::unreachable_probe()])));
    let revision = Box::leak(Box::new(Mutex::new(17)));
    let store = Store { archives, revision };
    let source = mount(
        &store,
        unknown("unreachable matching row", "selected"),
        vec![NamedMember {
            asset: "logical".into(),
            member: "fixture".into(),
        }],
        2,
        2,
    );
    let mut raw = fixtures::build_synthetic_map("valid later candidate", &[("fixture", &[2, 3])]);
    store
        .archives
        .lock()
        .unwrap()
        .push(MpqArchive::from_bytes(raw.split_off(0x200), "unselected valid archive").unwrap());
    let error = source.read_bounded("logical", 2).unwrap_err();
    match error {
        SourceError::MpqExtraction(error) => {
            assert_eq!(error.archive.to_str(), Some("unreachable matching row"));
            assert_eq!(error.member, "fixture");
            assert!(
                matches!(error.cause.as_ref(),pcx_mpq::ExtractionCause::Mpq(pcx_mpq::Error::NotFound(name)) if name=="fixture")
            );
        }
        other => panic!("expected contextual present-selection error, got {other:?}"),
    }
}
