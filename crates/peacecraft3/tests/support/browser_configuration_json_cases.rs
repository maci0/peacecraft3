use crate::{binding, fixtures};
use pcx_assets::{NamedMember, SourceError, SourceInfo};
use pcx_mpq::{ExtractionBudget, MpqArchive};
use peacecraft3::browser_configuration::{ConfigurationInbox, InboxError, Poll};
use peacecraft3::browser_configuration_json::{Error, decode_and_enqueue};
use peacecraft3::{
    browser_archive_source::{ArchiveSelection, CommittedArchiveSource},
    map_requests::{MapRequests, RequestFailure},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
struct Store {
    archives: &'static Mutex<Vec<MpqArchive>>,
    revision: &'static Mutex<u32>,
}
impl Store {
    fn factory(
        &self,
        index: usize,
        revision: u32,
        info: SourceInfo,
        names: Vec<NamedMember>,
        budget: ExtractionBudget,
    ) -> Result<CommittedArchiveSource, SourceError> {
        let selected = ArchiveSelection::inspect(self.archives, self.revision, index, revision)?;
        CommittedArchiveSource::mount_no_huffman(
            self.archives,
            self.revision,
            selected,
            info,
            names,
            budget,
        )
    }
}
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
fn store(f: &binding::Fixture) -> Store {
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
    let mut raw = fixtures::build_synthetic_map("json configuration", &files);
    Store {
        archives: Box::leak(Box::new(Mutex::new(vec![
            MpqArchive::from_bytes(raw.split_off(0x200), "json-source").unwrap(),
        ]))),
        revision: Box::leak(Box::new(Mutex::new(7))),
    }
}
fn invalid_text(text: &str) -> Error {
    let inbox = ConfigurationInbox::default();
    let mut calls = 0;
    let result = decode_and_enqueue(text, 1 << 20, &inbox, |_, _, _, _, _| {
        calls += 1;
        panic!("invalid metadata reached source factory")
    });
    assert_eq!(calls, 0);
    result.unwrap_err()
}
fn object_paths(value: &Value, path: String, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                out.push((path.clone(), key.clone()));
                object_paths(value, format!("{path}/{key}"), out);
            }
        }
        Value::Array(values) => {
            for (i, value) in values.iter().enumerate() {
                object_paths(value, format!("{path}/{i}"), out);
            }
        }
        _ => {}
    }
}
#[cfg_attr(test, test)]
pub fn all_nested_fields_required_and_unknowns_rejected() {
    let sample = metadata(&fixture());
    let mut paths = Vec::new();
    object_paths(&sample, String::new(), &mut paths);
    assert!(paths.len() > 100);
    let mut objects = std::collections::BTreeSet::new();
    for (parent, key) in &paths {
        let mut v = sample.clone();
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        let error = invalid_text(&v.to_string());
        assert!(
            matches!(error, Error::Schema(_)),
            "missing {parent}/{key}: {error:?}"
        );
        objects.insert(parent);
        let object = sample.pointer(parent).unwrap();
        let encoded = object.to_string();
        let key_json = serde_json::to_string(key).unwrap();
        let duplicate = format!("{{{key_json}:{},{}", object[key], &encoded[1..]);
        let text = sample.to_string().replacen(&encoded, &duplicate, 1);
        assert!(
            matches!(invalid_text(&text), Error::Schema(_)),
            "duplicate {parent}/{key}"
        );
    }
    for parent in objects {
        let mut v = sample.clone();
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected_policy".into(), json!(1));
        let error = invalid_text(&v.to_string());
        assert!(
            matches!(error, Error::Schema(_)),
            "unknown {parent}: {error:?}"
        );
    }
}
#[cfg_attr(test, test)]
pub fn duplicate_keys_syntax_and_preparse_budget() {
    let raw = metadata(&fixture()).to_string();
    for key in [
        "schema",
        "max_asset_bytes",
        "source_layer",
        "edition",
        "state",
        "max_decoded_bytes",
    ] {
        let needle = format!("\"{key}\":");
        let value = match key {
            "schema" => "\"pcx-map-config-v1\"",
            "source_layer" => "\"map\"",
            "edition" => "\"explicit duplicate\"",
            "state" => "\"unknown\"",
            _ => "1",
        };
        let text = raw.replacen(&needle, &format!("{needle}{value},{needle}"), 1);
        assert!(
            matches!(invalid_text(&text), Error::Schema(_)),
            "duplicate {key}"
        );
    }
    assert!(matches!(invalid_text("{"), Error::Syntax(_)));
    let mut calls = 0;
    let error = decode_and_enqueue("é", 1, &ConfigurationInbox::default(), |_, _, _, _, _| {
        calls += 1;
        panic!("over-budget factory")
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Error::Budget {
            actual: 2,
            limit: 1
        }
    ));
    assert_eq!(calls, 0);
}
#[cfg_attr(test, test)]
pub fn numeric_and_revision_boundaries() {
    let sample = metadata(&fixture());
    for value in [json!(-1), json!(1.5), json!("1"), json!(true), Value::Null] {
        let mut v = sample.clone();
        v["limits"]["reader"]["max_asset_bytes"] = value;
        assert!(matches!(invalid_text(&v.to_string()), Error::Schema(_)));
    }
    for (pointer, value) in [
        ("/sources/0/committed_revision", json!(4294967296u64)),
        ("/sources/0/priority", json!(4294967296u64)),
        ("/catalog/required_fields/0/0", json!(256)),
    ] {
        let mut v = sample.clone();
        *v.pointer_mut(pointer).unwrap() = value;
        assert!(matches!(invalid_text(&v.to_string()), Error::Schema(_)));
    }
    let raw = sample.to_string();
    let old = format!(
        "\"max_asset_bytes\":{}",
        sample["limits"]["reader"]["max_asset_bytes"]
    );
    for bad in ["1e0", "-0", "18446744073709551616"] {
        let text = raw.replacen(&old, &format!("\"max_asset_bytes\":{bad}"), 1);
        assert!(
            matches!(invalid_text(&text), Error::Schema(_)),
            "bad numeric {bad}"
        );
    }
    #[cfg(target_pointer_width = "32")]
    {
        let text = raw.replacen(&old, "\"max_asset_bytes\":4294967296", 1);
        assert!(matches!(invalid_text(&text), Error::Schema(_)));
        let mut v = sample.clone();
        v["sources"][0]["archive_index"] = json!(4294967296u64);
        assert!(matches!(invalid_text(&v.to_string()), Error::Schema(_)));
    }
    for bad in ["", "+1", "-1", " 1", "1 ", "01", "1.0", "1e2", "١"] {
        let mut v = sample.clone();
        v["binding_revision"] = json!(bad);
        assert!(
            matches!(invalid_text(&v.to_string()), Error::Policy { .. }),
            "bad revision {bad}"
        );
    }
    let mut v = sample;
    v["binding_revision"] = json!("18446744073709551616");
    assert!(matches!(invalid_text(&v.to_string()), Error::Revision(_)));
}
#[cfg_attr(test, test)]
pub fn versions_codec_tables_and_references_validate_before_factory() {
    let sample = metadata(&fixture());
    for value in [
        json!({"state":"guessed"}),
        json!({"state":0}),
        json!({"state":false}),
        json!({"state":{"unknown":null}}),
        json!({"state":"known"}),
        json!({"state":"known","value":""}),
        json!({"state":"unknown","value":"extra"}),
        json!(null),
    ] {
        let mut v = sample.clone();
        v["sources"][0]["info"]["version"] = value;
        let e = invalid_text(&v.to_string());
        assert!(matches!(e, Error::Schema(_) | Error::Policy { .. }));
    }
    for (path, value) in [
        ("/schema", json!({"pcx-map-config-v1":null})),
        ("/codec", json!({"no-huffman":null})),
        ("/sources/0/info/kind", json!({"mpq":null})),
    ] {
        let mut v = sample.clone();
        *v.pointer_mut(path).unwrap() = value;
        assert!(matches!(invalid_text(&v.to_string()), Error::Schema(_)));
    }
    let mut v = sample.clone();
    v["codec"] = json!("installed-huffman");
    assert!(matches!(invalid_text(&v.to_string()), Error::Schema(_)));
    let mut v = sample.clone();
    v["catalog"]["required_catalog_tables"][0] = json!("unknown-table.slk");
    assert!(matches!(invalid_text(&v.to_string()), Error::Table { .. }));
    for (pointer, value) in [
        ("/map/source_layer", json!("missing")),
        ("/map/identity/source", json!("")),
        ("/catalog/source_layers/0", json!("missing")),
        ("/sources/3/members/1/member", json!("bad\nlocator")),
    ] {
        let mut v = sample.clone();
        *v.pointer_mut(pointer).unwrap() = value;
        assert!(matches!(invalid_text(&v.to_string()), Error::Policy { .. }));
    }
    for duplicate_priority in [false, true] {
        let mut v = sample.clone();
        if duplicate_priority {
            v["sources"][1]["priority"] = v["sources"][0]["priority"].clone();
        } else {
            v["sources"][1]["info"]["id"] = v["sources"][0]["info"]["id"].clone();
        }
        assert!(matches!(invalid_text(&v.to_string()), Error::Resolver(_)));
    }
}
#[cfg_attr(test, test)]
pub fn accepted_actual_app_and_typed_rollback() {
    let f = fixture();
    let store = store(&f);
    let mut raw = metadata(&f);
    raw["sources"][1]["info"]["version"] =
        json!({"state":"known","value":"explicit synthetic prelude version"});
    let text = raw.to_string();
    let inbox = ConfigurationInbox::default();
    let id = decode_and_enqueue(&text, text.len(), &inbox, |i, r, s, n, b| {
        store.factory(i, r, s, n, b)
    })
    .unwrap();
    let mut app = bevy::prelude::App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::browser_configuration::install(&mut app, inbox.clone());
    app.update();
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("not complete")
    };
    assert!(outcome.result.is_ok());
    let active = app.world().resource::<MapRequests>();
    let old = active.admission.shared_context().unwrap();
    let receipts = active.published_receipts.as_ref().unwrap().clone();
    assert_eq!(old.map_digest.hash, pcx_mpq::fnv1a64(&f.raw));
    assert_eq!(receipts.map.provenance.source.version, None);
    assert_eq!(
        receipts.common.provenance.source.version.as_deref(),
        Some("explicit synthetic prelude version")
    );
    assert_eq!(old.required_fields, f.input(73).required_fields);
    raw["binding_revision"] = json!("18446744073709551615");
    let text = raw.to_string();
    let id = decode_and_enqueue(&text, text.len(), &inbox, |i, r, s, n, b| {
        store.factory(i, r, s, n, b)
    })
    .unwrap();
    *store.revision.lock().unwrap() = 8;
    app.update();
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("not complete")
    };
    assert_eq!(outcome.binding_revision, Some(u64::MAX));
    assert!(
        matches!(&outcome.result,Err(RequestFailure::Binding(error)) if matches!(error.as_ref(),peacecraft3::map_binding::BindingError::Resolve{source:pcx_assets::Error::Source{source:SourceError::Unavailable{..},..},..}))
    );
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
}
#[cfg_attr(test, test)]
pub fn typed_source_resolver_and_queue_causes() {
    let f = fixture();
    let store = store(&f);
    let text = metadata(&f).to_string();
    let inbox = ConfigurationInbox::default();
    *store.revision.lock().unwrap() = 8;
    let result = decode_and_enqueue(&text, text.len(), &inbox, |i, r, s, n, b| {
        store.factory(i, r, s, n, b)
    });
    assert!(
        matches!(result,Err(Error::Source{layer,source:SourceError::Unavailable{..}}) if layer=="map")
    );
    *store.revision.lock().unwrap() = 7;
    let mut calls = 0;
    let result = decode_and_enqueue(&text, text.len(), &inbox, |i, r, s, n, b| {
        let source = store.factory(i, r, s, n, b)?;
        calls += 1;
        if calls == 4 {
            *store.revision.lock().unwrap() = 8;
        }
        Ok(source)
    });
    assert!(matches!(
        result,
        Err(Error::Resolver(pcx_assets::Error::Source {
            source: SourceError::Unavailable { .. },
            ..
        }))
    ));
    *store.revision.lock().unwrap() = 7;
    decode_and_enqueue(&text, text.len(), &inbox, |i, r, s, n, b| {
        store.factory(i, r, s, n, b)
    })
    .unwrap();
    let result = decode_and_enqueue(&text, text.len(), &inbox, |i, r, s, n, b| {
        store.factory(i, r, s, n, b)
    });
    assert!(matches!(result, Err(Error::Queue(InboxError::Busy))));
}

#[cfg_attr(test, test)]
pub fn wrong_declared_role_source_preserves_active_identity() {
    let f = fixture();
    let store = store(&f);
    let inbox = ConfigurationInbox::default();
    let mut app = bevy::prelude::App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::browser_configuration::install(&mut app, inbox.clone());
    let mut data = metadata(&f);
    let text = data.to_string();
    let id = decode_and_enqueue(&text, text.len(), &inbox, |i, r, s, n, b| {
        store.factory(i, r, s, n, b)
    })
    .unwrap();
    app.update();
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("missing first completion")
    };
    assert!(outcome.result.is_ok());
    let active = app.world().resource::<MapRequests>();
    let context = active.admission.shared_context().unwrap();
    let receipts = active.published_receipts.as_ref().unwrap().clone();
    let old_identity = active.bindings.as_ref().unwrap().map_identity.clone();
    data["map"]["source_layer"] = data["common"]["source_layer"].clone();
    data["binding_revision"] = json!("74");
    let text = data.to_string();
    let id = decode_and_enqueue(&text, text.len(), &inbox, |i, r, s, n, b| {
        store.factory(i, r, s, n, b)
    })
    .unwrap();
    app.update();
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("missing mismatch completion")
    };
    assert!(
        matches!(&outcome.result,Err(RequestFailure::Binding(error))if matches!(error.as_ref(),peacecraft3::map_binding::BindingError::SourceMismatch{expected,actual,..} if expected==&vec![f.common_info.clone()] && actual.as_ref()==&f.map_info))
    );
    let active = app.world().resource::<MapRequests>();
    assert!(Arc::ptr_eq(
        &context,
        &active.admission.shared_context().unwrap()
    ));
    assert!(Arc::ptr_eq(
        &receipts,
        active.published_receipts.as_ref().unwrap()
    ));
    assert_eq!(active.bindings.as_ref().unwrap().map_identity, old_identity);
    assert_eq!(
        active.last_outcome.as_ref().unwrap().binding_revision,
        Some(73)
    );
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_configuration_json_gate() -> u32 {
    all_nested_fields_required_and_unknowns_rejected();
    duplicate_keys_syntax_and_preparse_budget();
    numeric_and_revision_boundaries();
    versions_codec_tables_and_references_validate_before_factory();
    accepted_actual_app_and_typed_rollback();
    typed_source_resolver_and_queue_causes();
    wrong_declared_role_source_preserves_active_identity();
    7
}
