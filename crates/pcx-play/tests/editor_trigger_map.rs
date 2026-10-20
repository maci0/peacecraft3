//! The explicit editor-source wrapper reads a map's trigger graph from its archive
//! and never its compiled script. A map with an invalid `war3map.j` therefore
//! compiles through it, and a map without the graph is refused by member name.

use pcx_map::Map;
use pcx_mpq::checked::ReadBudget;
use pcx_play::editor_triggers::{EditorMapError, compile_editor_map_scripts};
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
/// path does not derive. A graph that names one is refused by name, not bound silently.
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
