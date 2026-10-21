//! The explicit editor-source wrapper reads a map's trigger graph from its archive
//! and never its compiled script. A map with an invalid `war3map.j` therefore
//! compiles through it, and a map without the graph is refused by member name.

use pcx_map::Map;
use pcx_mpq::checked::ReadBudget;
use pcx_play::editor_triggers::{
    EditorMapError, LOWERED_LABEL, compile_editor_map_scripts, compile_editor_triggers_with_objects,
};
use pcx_play::scripts::{AssetIdentity, ScriptAsset};
use pcx_trig::catalog::Catalog;

#[path = "support/editor_fixture.rs"]
mod editor_fixture;
use editor_fixture::{BLIZZARD, PRELUDE, TRIGGER_DATA, build_synthetic_map, wct, wtg};

const BUDGET: ReadBudget = ReadBudget {
    max_archive_bytes: 1 << 20,
    max_archive_table_bytes: 1 << 16,
    max_raw_packed_bytes: 1 << 20,
    max_decoded_bytes: 1 << 20,
};

fn identity() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "fixture".into(),
        source: "memory".into(),
    }
}

fn catalog() -> Catalog {
    Catalog::parse(
        "synthetic-TriggerData.txt",
        "synthetic",
        "fixture",
        TRIGGER_DATA,
    )
    .unwrap()
}

#[test]
fn editor_graph_compiles_even_when_the_compiled_script_is_invalid() {
    let wtg = wtg();
    let wct = wct();
    let bytes = build_synthetic_map(
        "EditorOnly",
        &[
            ("war3map.wtg", &wtg),
            ("war3map.wct", &wct),
            ("war3map.j", b"this is not jass (("),
        ],
    );
    let map = Map::parse(&bytes).unwrap();
    let scripts = compile_editor_map_scripts(
        &map,
        "synthetic:EditorOnly.w3x",
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: PRELUDE.as_bytes(),
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BLIZZARD,
        },
        &catalog(),
        BUDGET,
    )
    .unwrap();
    assert!(scripts.source("lowered war3map.wtg").is_some());
    assert!(scripts.source("war3map.j").is_none());
}

#[test]
fn a_map_without_its_trigger_graph_is_refused_by_member_name() {
    let bytes = build_synthetic_map(
        "JassOnly",
        &[(
            "war3map.j",
            b"function main takes nothing returns nothing\nendfunction\n",
        )],
    );
    let map = Map::parse(&bytes).unwrap();
    let error = compile_editor_map_scripts(
        &map,
        "synthetic:JassOnly.w3x",
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: PRELUDE.as_bytes(),
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BLIZZARD,
        },
        &catalog(),
        BUDGET,
    )
    .err()
    .expect("a map without war3map.wtg must be refused");
    assert!(
        matches!(error, EditorMapError::MissingMember("war3map.wtg")),
        "{error}"
    );
}

/// Object globals (`gg_unit_*`) come from the map's own object creation, which this
/// path does not bind when no records are supplied. A graph that names one is
/// refused by name, not bound silently.
#[test]
fn an_object_global_the_path_does_not_bind_is_refused_by_name() {
    let graph = editor_fixture::wtg_with_variables(
        &[],
        &[editor_fixture::trigger(
            "Object",
            true,
            &[editor_fixture::eca(
                2,
                "DisplayTextToPlayer",
                vec![
                    editor_fixture::param(0, "PlayerOne", None),
                    editor_fixture::param(1, "gg_unit_h001_0001", None),
                ],
            )],
        )],
    );
    let catalog = Catalog::parse(
        "synthetic-TriggerData.txt",
        "synthetic",
        "fixture",
        TRIGGER_DATA,
    )
    .unwrap();
    let error = pcx_play::editor_triggers::compile_editor_triggers(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: PRELUDE.as_bytes(),
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BLIZZARD,
        },
        &graph,
        &editor_fixture::empty_wct(1),
        &catalog,
    )
    .err()
    .expect("an unbound object global must not compile");
    assert!(error.to_string().contains("gg_unit_h001_0001"), "{error}");
}

/// A minimal `war3mapUnits.doo` holding one `h001` unit with creation number 1.
fn unit_byte(type_id: &[u8; 4], creation: u32) -> Vec<u8> {
    let mut data = b"W3do".to_vec();
    for w in [8u32, 11, 1] {
        data.extend(w.to_le_bytes());
    }
    data.extend(type_id);
    data.extend(0u32.to_le_bytes());
    for v in [1f32, 2., 3.] {
        data.extend(v.to_bits().to_le_bytes());
    }
    data.extend(0f32.to_bits().to_le_bytes());
    for v in [1f32, 1., 1.] {
        data.extend(v.to_bits().to_le_bytes());
    }
    data.push(2);
    data.extend(0u16.to_le_bytes());
    for w in [0u32, 0, 0] {
        data.extend(w.to_le_bytes());
    }
    data.extend(u32::MAX.to_le_bytes());
    data.extend(0u32.to_le_bytes());
    data.extend(0u32.to_le_bytes());
    data.extend(0f32.to_bits().to_le_bytes());
    data.extend(0u32.to_le_bytes());
    for _ in 0..3 {
        data.extend(0u32.to_le_bytes());
    }
    data.extend(0u32.to_le_bytes());
    data.extend(0u32.to_le_bytes());
    data.extend(u32::MAX.to_le_bytes());
    for _ in 0..2 {
        data.extend(0u32.to_le_bytes());
    }
    data.extend(creation.to_le_bytes());
    data
}

/// A minimal `war3map.w3r` holding one region with the given sanitized name and
/// bounds `[min_x, min_y, max_x, max_y]` (version 4, no colour).
fn region_byte(name: &str, bounds: [f32; 4]) -> Vec<u8> {
    let mut data = Vec::new();
    for w in [4u32, 1] {
        data.extend(w.to_le_bytes());
    }
    for v in bounds {
        data.extend(v.to_bits().to_le_bytes());
    }
    data.extend(name.as_bytes());
    data.push(0);
    data.extend(0u32.to_le_bytes());
    data.extend(b"RAIN");
    data.extend(b"ambience\0");
    data
}

/// A graph whose trigger assigns `gg_rct_Area` into the rect variable `X`,
/// so the lowered script references the region global as a bare identifier.
fn region_graph() -> Vec<u8> {
    editor_fixture::wtg_with_variables(
        &[editor_fixture::variable("X", "rect", None)],
        &[editor_fixture::trigger(
            "UseRegion",
            true,
            &[editor_fixture::eca(
                2,
                "SetVariable",
                vec![
                    editor_fixture::param(1, "X", None),
                    editor_fixture::param(1, "gg_rct_Area", None),
                ],
            )],
        )],
    )
}

/// A host that records native calls and materializes `Rect` handles, so a run
/// observes whether the region initializer really executed.
struct RectRecorder {
    trace: Vec<String>,
    next_handle: u64,
}

impl pcx_script::Host for RectRecorder {
    fn native(
        &mut self,
        name: &str,
        args: &[pcx_script::Value],
    ) -> Result<pcx_script::Value, String> {
        let rendered: Vec<String> = args
            .iter()
            .map(|a| match a {
                pcx_script::Value::Real(r) => r.to_string(),
                pcx_script::Value::Int(i) => i.to_string(),
                pcx_script::Value::Bool(b) => b.to_string(),
                pcx_script::Value::Str(s) => format!("{s:?}"),
                pcx_script::Value::Null => "null".into(),
                pcx_script::Value::Handle(_) => "handle".into(),
                pcx_script::Value::Code(id) => format!("code {id:?}"),
            })
            .collect();
        if name == "Rect" {
            self.next_handle += 1;
            self.trace.push(format!("Rect({})", rendered.join(", ")));
            return Ok(pcx_script::Value::Handle(pcx_script::HandleId::new(
                std::num::NonZeroU64::new(self.next_handle).unwrap(),
            )));
        }
        Ok(pcx_script::Value::Null)
    }
}

/// A region object global is bound and the compiled script initializes it to a
/// real `Rect` built from the authored `war3map.w3r` bounds. Running
/// `InitGlobals` executes that `Rect(...)` native call; a null-only declaration
/// would not, so this observation discriminates the two.
#[test]
fn a_region_object_global_binds_and_initializes_a_real_rect() {
    let regions = pcx_map::Regions::parse(&region_byte("Area", [10.0, 20.0, 30.0, 40.0])).unwrap();
    let scripts = compile_editor_triggers_with_objects(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: PRELUDE.as_bytes(),
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BLIZZARD,
        },
        &region_graph(),
        &editor_fixture::empty_wct(1),
        &catalog(),
        pcx_play::editor_object_globals::MapRecords {
            regions: Some(&regions),
            ..Default::default()
        },
    )
    .expect("a record-derived region global must bind and compile");
    let lowered = scripts.source(LOWERED_LABEL).expect("lowered source");
    assert!(lowered.contains("rect gg_rct_Area"), "{lowered}");
    assert!(
        lowered.contains("set gg_rct_Area = Rect(10.0, 20.0, 30.0, 40.0)"),
        "{lowered}"
    );
    let host = RectRecorder {
        trace: Vec::new(),
        next_handle: 0,
    };
    let mut vm = scripts.vm(host);
    vm.init_globals().unwrap();
    vm.call("InitGlobals", &[]).unwrap();
    vm.call("Trig_UseRegion_Actions", &[]).unwrap();
    let region = vm.global("gg_rct_Area");
    assert!(matches!(region, pcx_script::Value::Handle(_)));
    assert_eq!(vm.global("udg_X"), region);
    let trace = &vm.host().inner.trace;
    assert!(
        trace.iter().any(|c| c.starts_with("Rect(")),
        "InitGlobals must build the region rect: {trace:?}"
    );
}

#[test]
fn archive_editor_path_initializes_regions_without_reading_compiled_jass() {
    let graph = region_graph();
    let text = editor_fixture::empty_wct(1);
    let region = region_byte("Area", [10.0, 20.0, 30.0, 40.0]);
    let bytes = build_synthetic_map(
        "RegionGraph",
        &[
            ("war3map.wtg", &graph),
            ("war3map.wct", &text),
            ("war3map.w3r", &region),
            ("war3map.j", b"invalid compiled script"),
        ],
    );
    let map = Map::parse(&bytes).unwrap();
    let scripts = compile_editor_map_scripts(
        &map,
        "synthetic:RegionGraph",
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: PRELUDE.as_bytes(),
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BLIZZARD,
        },
        &catalog(),
        BUDGET,
    )
    .unwrap();
    let mut vm = scripts.vm(RectRecorder {
        trace: Vec::new(),
        next_handle: 0,
    });
    vm.init_globals().unwrap();
    vm.call("InitGlobals", &[]).unwrap();
    vm.call("Trig_UseRegion_Actions", &[]).unwrap();
    assert!(matches!(vm.global("udg_X"), pcx_script::Value::Handle(_)));
    assert_eq!(vm.host().inner.trace, ["Rect(10, 20, 30, 40)"]);
}

/// A graph that references a unit object global is refused until its creation
/// initializer is generated, rather than being left as a null handle.
#[test]
fn a_unit_object_global_is_refused_until_its_initializer_is_generated() {
    let graph = editor_fixture::wtg_with_variables(
        &[],
        &[editor_fixture::trigger(
            "Object",
            true,
            &[editor_fixture::eca(
                2,
                "DisplayTextToPlayer",
                vec![
                    editor_fixture::param(0, "PlayerOne", None),
                    editor_fixture::param(1, "gg_unit_h001_0001", None),
                ],
            )],
        )],
    );
    let units = pcx_map::Units::parse(&unit_byte(b"h001", 1)).unwrap();
    let error = compile_editor_triggers_with_objects(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: PRELUDE.as_bytes(),
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BLIZZARD,
        },
        &graph,
        &editor_fixture::empty_wct(1),
        &catalog(),
        pcx_play::editor_object_globals::MapRecords {
            units: Some(&units),
            ..Default::default()
        },
    )
    .err()
    .expect("a unit object global must be refused until its initializer is generated");
    assert!(error.to_string().contains("gg_unit_h001_0001"), "{error}");
}

/// The binding derives the global from the map's records: a reference without a
/// matching record is still refused by name.
#[test]
fn an_object_global_without_a_matching_record_is_refused_by_name() {
    // Only the region "Other" exists; gg_rct_Area has no record.
    let regions = pcx_map::Regions::parse(&region_byte("Other", [10.0, 20.0, 30.0, 40.0])).unwrap();
    let error = compile_editor_triggers_with_objects(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: PRELUDE.as_bytes(),
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BLIZZARD,
        },
        &region_graph(),
        &editor_fixture::empty_wct(1),
        &catalog(),
        pcx_play::editor_object_globals::MapRecords {
            regions: Some(&regions),
            ..Default::default()
        },
    )
    .err()
    .expect("a reference without a matching record must be refused");
    assert!(error.to_string().contains("gg_rct_Area"), "{error}");
}
