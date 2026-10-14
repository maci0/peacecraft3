//! Synthetic gates against actual browser globals and existing App consumer.
use super::*;
use crate::{binding, fixtures};
use peacecraft3::map_requests::MapRequests;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::sync::Arc;
fn fixture() -> binding::Fixture {
    let mut f = binding::Fixture::new(236);
    for (info, id) in [
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
        info.origin = "json-source".into();
        info.version = None;
        id.source = "json-source".into();
        id.edition = "explicit unknown fixture edition".into();
        id.version = "explicit unknown fixture version".into();
    }
    f
}
fn identity(id: &pcx_play::scripts::AssetIdentity) -> Value {
    json!({"source":id.source,"edition":id.edition,"version":id.version})
}
fn metadata(f: &binding::Fixture) -> Value {
    let files = [
        "selected map.w3m",
        "chosen common.j",
        "chosen Blizzard.j",
        pcx_objects::editor::path::SlkId::UnitData.path(),
        pcx_objects::editor::path::SlkId::UnitMetaData.path(),
    ];
    let subsets = [&files[..1], &files[1..2], &files[2..3], &files[3..]];
    let infos = [&f.map_info, &f.common_info, &f.bj_info, &f.catalog_infos[0]];
    let sources:Vec<Value>=infos.iter().zip(subsets).enumerate().map(|(i,(info,files))|json!({"archive_index":0,"committed_revision":7,"priority":10-i,"info":{"id":info.id,"kind":"mpq","origin":info.origin,"version":{"state":"unknown"}},"members":files.iter().map(|name|json!({"asset":name,"member":name})).collect::<Vec<_>>(),"extraction":{"max_raw_packed_bytes":1<<20,"max_decoded_bytes":1<<20}})).collect();
    let input = f.input(73);
    let role = |m: peacecraft3::map_binding::IdentifiedMember<'_>| json!({"source_layer":m.source.id,"path":m.path,"label":m.label,"identity":identity(m.identity)});
    let l = f.limits();
    let a = l.admission;
    let s = a.scripts;
    json!({"schema":"pcx-map-config-v1","codec":"no-huffman","mode":"1.29b-tft-vanilla-vanilla","binding_revision":"73","sources":sources,"map":role(input.map),"common":role(input.common),"blizzard":role(input.blizzard),"catalog":{"identity":identity(input.catalog_identity),"label":input.catalog_label,"source_layers":input.catalog_sources.iter().map(|i|i.id.clone()).collect::<Vec<_>>(),"required_catalog_tables":input.required_catalog_tables.iter().map(|t|t.path()).collect::<Vec<_>>(),"required_fields":input.required_fields},"limits":{"reader":{"max_listed_paths":l.reader.max_listed_paths,"max_asset_bytes":l.reader.max_asset_bytes,"max_aggregate_input_bytes":l.reader.max_aggregate_input_bytes},"overlay":{"unit_bytes":l.overlay.unit_bytes,"item_bytes":l.overlay.item_bytes,"aggregate_bytes":l.overlay.aggregate_bytes},"max_map_file_bytes":l.max_map_file_bytes,"max_resolver_input_bytes":l.max_resolver_input_bytes,"admission":{"terrain_bytes":a.terrain_bytes,"info_bytes":a.info_bytes,"pathing_bytes":a.pathing_bytes,"units_bytes":a.units_bytes,"aggregate_input_bytes":a.aggregate_input_bytes,"required_fields":a.required_fields,"scripts":{"max_script_bytes":s.max_script_bytes,"max_wts_bytes":s.max_wts_bytes,"max_archive_bytes":s.max_archive_bytes,"max_archive_table_bytes":s.max_archive_table_bytes,"max_raw_packed_bytes":s.max_raw_packed_bytes,"max_decoded_bytes":s.max_decoded_bytes,"max_prelude_bytes":s.max_prelude_bytes,"max_aggregate_input_bytes":s.max_aggregate_input_bytes}}}})
}

thread_local! { static APP: RefCell<Option<bevy::prelude::App>> = const {RefCell::new(None)}; }
#[wasm_bindgen]
pub fn ffi_test_metadata() -> String {
    super::super::drop_assets();
    let f = fixture();
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
    let mut bytes = fixtures::build_synthetic_map("FFI configuration fixture", &files);
    assert!(
        !super::super::add_archive("json-source".into(), bytes.split_off(0x200))
            .starts_with("rejected")
    );
    let mut value = metadata(&f);
    for source in value["sources"].as_array_mut().unwrap() {
        source["committed_revision"] = json!(super::super::asset_revision());
    }
    let mut app = bevy::prelude::App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::browser_configuration::install(
        &mut app,
        super::super::map_configuration_inbox().clone(),
    );
    APP.with_borrow_mut(|slot| *slot = Some(app));
    value.to_string()
}
#[wasm_bindgen]
pub fn ffi_test_frame() {
    APP.with_borrow_mut(|slot| slot.as_mut().unwrap().update());
}
#[wasm_bindgen]
pub fn ffi_test_active_revision() -> String {
    APP.with_borrow(|slot| {
        slot.as_ref()
            .unwrap()
            .world()
            .resource::<MapRequests>()
            .last_outcome
            .as_ref()
            .unwrap()
            .binding_revision
            .unwrap()
            .to_string()
    })
}
#[wasm_bindgen]
pub fn ffi_test_stale_before_frame() {
    APP.with_borrow_mut(|slot| {
        let app = slot.as_mut().unwrap();
        let active = app.world().resource::<MapRequests>();
        let old = active.admission.shared_context().unwrap();
        let receipts = active.published_receipts.as_ref().unwrap().clone();
        super::super::drop_assets();
        app.update();
        let active = app.world().resource::<MapRequests>();
        assert!(Arc::ptr_eq(
            &old,
            &active.admission.shared_context().unwrap()
        ));
        assert!(Arc::ptr_eq(
            &receipts,
            active.published_receipts.as_ref().unwrap()
        ));
        assert_eq!(
            active.last_outcome.as_ref().unwrap().binding_revision,
            Some(73)
        );
    });
}
#[wasm_bindgen]
pub fn ffi_test_assert_owned(result: &MapConfigurationResult, expected: &str) {
    match expected {
        "schema" => assert!(
            matches!(&result.state,Reply::Failed(None,Cause::Decoder(Error::Schema(error))) if error.is_data())
        ),
        "syntax" => assert!(
            matches!(&result.state,Reply::Failed(None,Cause::Decoder(Error::Syntax(error))) if error.is_eof()||error.is_syntax())
        ),
        "budget" => assert!(
            matches!(&result.state,Reply::Failed(None,Cause::Decoder(Error::Budget{actual,limit})) if actual>limit)
        ),
        "source" => assert!(
            matches!(&result.state,Reply::Failed(None,Cause::Decoder(Error::Source{layer,source:SourceError::Unavailable{..}})) if layer=="map")
        ),
        "busy" => assert!(matches!(
            &result.state,
            Reply::Failed(None, Cause::Decoder(Error::Queue(InboxError::Busy)))
        )),
        "unknown" => assert!(matches!(
            &result.state,
            Reply::Failed(Some(_), Cause::Queue(InboxError::Unknown { .. }))
        )),
        "rollback" => assert!(
            matches!(&result.state,Reply::Completed(_,outcome) if matches!(&outcome.result,Err(RequestFailure::Binding(error)) if matches!(error.as_ref(),peacecraft3::map_binding::BindingError::Resolve{source:pcx_assets::Error::Source{source:SourceError::Unavailable{..},..},..})))
        ),
        "success" => assert!(
            matches!(&result.state,Reply::Completed(_,outcome) if outcome.result.is_ok()&&outcome.binding_revision==Some(73))
        ),
        _ => panic!("unexpected typed proof"),
    }
}

#[wasm_bindgen]
pub fn ffi_test_assert_inspection_owned(result: &MapArchiveInspection) {
    assert!(
        matches!(&result.state, Inspection::Source(SourceError::Unavailable { detail }) if !detail.is_empty())
    );
}
