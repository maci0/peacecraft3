use super::{COMMON, budget, context, object, slots_rule};
use crate::file_sounds::{Assets, Budget as SoundBudget, SoundInput};
use crate::playercolor_host::{
    DestructableBudget, DestructableCatalog, DestructableCreation, DestructableOrigin,
    DestructableState, EnumHost, Failure, Limits, ObjectHost, PlacedDestructable, PlacedSource,
};
use crate::scripts::{AssetIdentity, ScriptAsset};
use crate::unit_placement::{
    Budget as PlacementBudget, Footprint, FootprintLoader, Grid, Placement, StationaryKey,
    StationaryStamp,
};
use pcx_script::{Host, Prelude, Value};
use pcx_sim::world::Source;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

const DECLARATIONS: &str = "type agent extends handle
type widget extends agent
type destructable extends widget
type timer extends agent
type playercolor extends handle
constant native ConvertPlayerColor takes integer value returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer which returns nothing
native CreateDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable
globals
constant playercolor COLOR = ConvertPlayerColor(19)
endglobals
";

fn host_from(declarations: &str, script: &str, max_handles: usize) -> ObjectHost {
    let common = pcx_script::parse_script(declarations).unwrap();
    let map = pcx_script::parse_script(script).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:destructables", &common)])
            .unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "synthetic:selected-map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(inner).unwrap()
}

fn admitted(max_records: usize, max_handles: usize) -> ObjectHost {
    host_from(DECLARATIONS, "", max_handles)
        .with_destructables(DestructableBudget { max_records })
        .unwrap()
}

fn payload(objectid: i32, variation: i32) -> [Value; 6] {
    [
        Value::Int(objectid),
        Value::Real(1.5),
        Value::Real(-2.25),
        Value::Real(0.0),
        Value::Real(3.5),
        Value::Int(variation),
    ]
}

fn payload_at(objectid: i32, x: f32, y: f32) -> [Value; 6] {
    [
        Value::Int(objectid),
        Value::Real(x),
        Value::Real(y),
        Value::Real(0.0),
        Value::Real(1.0),
        Value::Int(0),
    ]
}

fn cell(x: usize, y: usize) -> usize {
    y * 32 + x
}

fn script_placement(
    stationary_cells: usize,
    stationary_work: usize,
    placed: Vec<StationaryStamp>,
) -> Placement {
    let base = pcx_map::Pathing {
        version: 0,
        width: 32,
        height: 32,
        cells: vec![0; 1024],
    };
    Placement::new(
        Grid {
            source: Source {
                install: "synthetic".into(),
                edition: "test".into(),
                map: "late script destructables".into(),
            },
            origin: [0.0; 2],
            pathing: pcx_map::Pathing {
                version: 0,
                width: 32,
                height: 32,
                cells: vec![0; 1024],
            },
            domains: vec![5; 1024],
        },
        crate::unit_placement::tests::rules(2, -8192.25),
        BTreeMap::new(),
        PlacementBudget {
            cells: 1024,
            footprint_cells: 64,
            stationary_cells,
            stationary_work,
            queries: 100,
        },
    )
    .unwrap()
    .with_stationary(base, placed)
    .unwrap()
    .with_footprint_loader(|path| match path {
        "live.tga" => Ok(Footprint {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "test".into(),
                version: "test".into(),
            },
            cells: vec![[0, 0]],
        }),
        "dead.tga" => Ok(Footprint {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "test".into(),
                version: "test".into(),
            },
            cells: vec![[1, 0]],
        }),
        _ => Err(format!("{path}: missing synthetic pathing texture")),
    })
    .with_script_paths(|id, dead| match (id, dead) {
        (id, false) if id == i32::from_be_bytes(*b"LTlt") => Ok("live.tga".into()),
        (id, true) if id == i32::from_be_bytes(*b"LTlt") => Ok("dead.tga".into()),
        (id, false) if id == i32::from_be_bytes(*b"BADD") => Ok("broken-live.tga".into()),
        (id, true) if id == i32::from_be_bytes(*b"BADD") => Ok("broken-dead.tga".into()),
        (id, false) if id == i32::from_be_bytes(*b"ONE1") => Ok("live.tga".into()),
        (id, true) if id == i32::from_be_bytes(*b"ONE1") => Ok("_".into()),
        _ => Err(format!("{id}: unknown synthetic stationary identity")),
    })
}

#[derive(Default)]
struct SyntheticSource {
    remaining: usize,
    committed: BTreeMap<String, Footprint>,
    staged: BTreeMap<String, Footprint>,
}

struct BudgetedLoader(Arc<Mutex<SyntheticSource>>);

fn synthetic_footprint(cells: Vec<[i32; 2]>) -> Footprint {
    Footprint {
        origin: AssetIdentity {
            source: "synthetic".into(),
            edition: "test".into(),
            version: "test".into(),
        },
        cells,
    }
}

impl FootprintLoader for BudgetedLoader {
    fn load(&mut self, path: &str) -> Result<Footprint, String> {
        let mut source = self.0.lock().unwrap();
        if let Some(footprint) = source
            .committed
            .get(path)
            .or_else(|| source.staged.get(path))
        {
            return Ok(footprint.clone());
        }
        let cells = match path {
            "missing-live.tga" | "malformed-live.tga" => vec![[0, 0], [1, 0]],
            "malformed-dead.tga" => {
                return Err(format!("{path}: malformed synthetic pathing texture"));
            }
            "work-live.tga" | "work-dead.tga" => vec![[0, 0], [0, 1]],
            "cell-live.tga" => vec![[0, 0], [1, 0]],
            "cell-dead.tga" => vec![[0, 1]],
            "good-live.tga" => vec![[0, 0]],
            _ => return Err(format!("{path}: missing synthetic pathing texture")),
        };
        if cells.len() > source.remaining {
            return Err(format!("{path}: synthetic source budget"));
        }
        source.remaining -= cells.len();
        let footprint = synthetic_footprint(cells);
        source.staged.insert(path.into(), footprint.clone());
        Ok(footprint)
    }

    fn commit(&mut self, paths: &[String]) {
        let mut source = self.0.lock().unwrap();
        for path in paths {
            if let Some(footprint) = source.staged.remove(path) {
                source.committed.insert(path.clone(), footprint);
            }
        }
    }

    fn rollback(&mut self, paths: &[String]) {
        let mut source = self.0.lock().unwrap();
        for path in paths {
            if let Some(footprint) = source.staged.remove(path) {
                source.remaining += footprint.cells.len();
            }
        }
    }
}

fn budgeted_script_placement(
    source: Arc<Mutex<SyntheticSource>>,
    stationary_cells: usize,
    stationary_work: usize,
) -> Placement {
    let base = pcx_map::Pathing {
        version: 0,
        width: 32,
        height: 32,
        cells: vec![0; 1024],
    };
    Placement::new(
        Grid {
            source: Source {
                install: "synthetic".into(),
                edition: "test".into(),
                map: "budgeted late script destructables".into(),
            },
            origin: [0.0; 2],
            pathing: pcx_map::Pathing {
                version: 0,
                width: 32,
                height: 32,
                cells: vec![0; 1024],
            },
            domains: vec![5; 1024],
        },
        crate::unit_placement::tests::rules(2, -8192.25),
        BTreeMap::new(),
        PlacementBudget {
            cells: 1024,
            footprint_cells: 64,
            stationary_cells,
            stationary_work,
            queries: 100,
        },
    )
    .unwrap()
    .with_stationary(base, Vec::new())
    .unwrap()
    .with_staged_footprint_loader(BudgetedLoader(source))
    .with_script_paths(|id, dead| {
        let path = match (id, dead) {
            (id, false) if id == i32::from_be_bytes(*b"MISS") => "missing-live.tga",
            (id, true) if id == i32::from_be_bytes(*b"MISS") => "missing-dead.tga",
            (id, false) if id == i32::from_be_bytes(*b"MALF") => "malformed-live.tga",
            (id, true) if id == i32::from_be_bytes(*b"MALF") => "malformed-dead.tga",
            (id, false) if id == i32::from_be_bytes(*b"WORK") => "work-live.tga",
            (id, true) if id == i32::from_be_bytes(*b"WORK") => "work-dead.tga",
            (id, false) if id == i32::from_be_bytes(*b"CELL") => "cell-live.tga",
            (id, true) if id == i32::from_be_bytes(*b"CELL") => "cell-dead.tga",
            (id, false) if id == i32::from_be_bytes(*b"GOOD") => "good-live.tga",
            (id, true) if id == i32::from_be_bytes(*b"GOOD") => "_",
            _ => return Err(format!("{id}: unknown synthetic stationary identity")),
        };
        Ok(path.into())
    })
}

fn create(host: &mut ObjectHost, objectid: i32, variation: i32) -> Result<Value, String> {
    host.native("CreateDestructable", &payload(objectid, variation))
}

fn life_declarations() -> String {
    let mut text = DECLARATIONS.to_string();
    text.push_str(
        "native GetDestructableLife takes destructable whichDestructable returns real\nnative GetDestructableMaxLife takes destructable whichDestructable returns real\nnative SetDestructableLife takes destructable whichDestructable, real life returns nothing\nnative KillDestructable takes destructable whichDestructable returns nothing\nnative SetDestructableInvulnerable takes destructable whichDestructable, boolean flag returns nothing\nnative IsDestructableInvulnerable takes destructable whichDestructable returns boolean\nnative CreateDeadDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable\n",
    );
    text
}

fn slk_table(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
    let mut text = String::from("ID;PWXL;N;E\r\n");
    for (y, row) in std::iter::once(headers)
        .chain(rows.iter().copied())
        .enumerate()
    {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

fn catalog_from(
    rows: &[&[&str]],
    metadata_rows: &[&[&str]],
    decoration_rows: Option<&[&[&str]]>,
    destructable_edits: pcx_objects::editor::Overlay,
    decoration_edits: pcx_objects::editor::Overlay,
) -> DestructableCatalog {
    let data = slk_table(&["DestructableID", "HP", "targType"], rows);
    let metadata = slk_table(
        &["ID", "field", "slk", "type", "index", "category"],
        metadata_rows,
    );
    let mut input = vec![
        (pcx_objects::editor::SlkId::DestructableData.path(), data),
        (
            pcx_objects::editor::SlkId::DestructableMetaData.path(),
            metadata,
        ),
    ];
    if let Some(rows) = decoration_rows {
        input.push((
            pcx_objects::editor::SlkId::Doodads.path(),
            slk_table(&["doodID", "pathTex"], rows),
        ));
    }
    let bytes: Vec<(&str, &[u8])> = input
        .iter()
        .map(|(path, data)| (*path, data.as_slice()))
        .collect();
    let tables = pcx_objects::editor::defaults::DefaultTables::from_bytes(
        pcx_objects::editor::defaults::Identity {
            install: "synthetic".into(),
            edition_version: "synthetic".into(),
        },
        &bytes,
        &[],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 8,
            max_asset_bytes: 4096,
            max_aggregate_input_bytes: 16384,
        },
    )
    .unwrap();
    DestructableCatalog::new(Arc::new(tables), destructable_edits, decoration_edits)
}

fn tree_metadata() -> Vec<&'static [&'static str]> {
    vec![
        &["Ahpv", "HP", "DestructableData", "int", "0", "stats"],
        &[
            "Atgt",
            "targType",
            "DestructableData",
            "string",
            "0",
            "combat",
        ],
    ]
}

pub(super) fn life_catalog() -> DestructableCatalog {
    catalog_from(
        &[&["LTlt", "50", "tree,ground"]],
        &tree_metadata(),
        None,
        pcx_objects::editor::Overlay::default(),
        pcx_objects::editor::Overlay::default(),
    )
}

fn life_host(max_records: usize) -> ObjectHost {
    let mut host = host_from(&life_declarations(), "", 8)
        .with_destructables(DestructableBudget { max_records })
        .unwrap();
    host.admit_destructable_catalog(life_catalog()).unwrap();
    host
}

fn placed_row(creation_number: u32, dead: bool) -> PlacedDestructable {
    PlacedDestructable {
        creation: DestructableCreation {
            origin: DestructableOrigin::Placed { creation_number },
            objectid: i32::from_be_bytes(*b"LTlt"),
            x: 1.0,
            y: 2.0,
            z: Some(3.0),
            face: 0.0,
            scale: 1.0,
            variation: 0,
            dead,
        },
        state: DestructableState {
            dead,
            life: if dead { Some(0.0) } else { Some(50.0) },
            max_life: Some(50.0),
            ..DestructableState::default()
        },
        source: PlacedSource {
            creation_number,
            type_id: *b"LTlt",
            variation: 0,
            position: [1.0, 2.0, 3.0],
            angle: 0.0,
            scale: [1.0, 1.0, 1.0],
            flags: 0,
            life: if dead { 0 } else { 100 },
        },
    }
}

#[test]
fn catalog_exposes_the_authored_tree_classification_and_refuses_unknown_ids() {
    let catalog = life_catalog();
    let stats = catalog.stats(i32::from_be_bytes(*b"LTlt")).unwrap();
    assert_eq!(stats.max_life, 50.0);
    assert!(stats.tree);
    let missing = catalog.stats(i32::from_be_bytes(*b"hfoo")).unwrap_err();
    assert!(
        missing.contains("missing destructable catalog record"),
        "{missing}"
    );
}

fn hp_edit(value_type: pcx_objects::ValueType, value: pcx_objects::Value) -> pcx_objects::Mod {
    pcx_objects::Mod {
        field: *b"Ahpv",
        value_type,
        selectors: Some([0, 0]),
        value,
        end_word: 0,
    }
}

#[test]
fn catalog_enforces_typed_metadata_exact_case_and_present_malformed_records() {
    let ltlt = i32::from_be_bytes(*b"LTlt");
    let rows: Vec<&[&str]> = vec![&["LTlt", "50", "tree,ground"]];
    let wrong_hp_type: Vec<&[&str]> = vec![
        &["Ahpv", "HP", "DestructableData", "string", "0", "stats"],
        &[
            "Atgt",
            "targType",
            "DestructableData",
            "string",
            "0",
            "combat",
        ],
    ];
    let error = catalog_from(
        &rows,
        &wrong_hp_type,
        None,
        pcx_objects::editor::Overlay::default(),
        pcx_objects::editor::Overlay::default(),
    )
    .stats(ltlt)
    .unwrap_err();
    assert!(error.contains("unsupported HP metadata type"), "{error}");

    let missing_hp: Vec<&[&str]> = vec![&[
        "Atgt",
        "targType",
        "DestructableData",
        "string",
        "0",
        "combat",
    ]];
    let error = catalog_from(
        &rows,
        &missing_hp,
        None,
        pcx_objects::editor::Overlay::default(),
        pcx_objects::editor::Overlay::default(),
    )
    .stats(ltlt)
    .unwrap_err();
    assert!(error.contains("missing HP metadata"), "{error}");

    let duplicate_hp: Vec<&[&str]> = vec![
        &["Ahpv", "HP", "DestructableData", "int", "0", "stats"],
        &["Ahp2", "HP", "DestructableData", "int", "1", "stats"],
        &[
            "Atgt",
            "targType",
            "DestructableData",
            "string",
            "0",
            "combat",
        ],
    ];
    let error = catalog_from(
        &rows,
        &duplicate_hp,
        None,
        pcx_objects::editor::Overlay::default(),
        pcx_objects::editor::Overlay::default(),
    )
    .stats(ltlt)
    .unwrap_err();
    assert!(error.contains("unsupported HP metadata"), "{error}");

    let mut edits = pcx_objects::editor::Overlay::default();
    edits.mods.insert(
        "LTlt".into(),
        vec![hp_edit(
            pcx_objects::ValueType::Str,
            pcx_objects::Value::Str(b"75".to_vec()),
        )],
    );
    let error = catalog_from(
        &rows,
        &tree_metadata(),
        None,
        edits,
        pcx_objects::editor::Overlay::default(),
    )
    .stats(ltlt)
    .unwrap_err();
    assert!(error.contains("HP edit has Str wire type"), "{error}");

    let mut edits = pcx_objects::editor::Overlay::default();
    edits.mods.insert(
        "LTlt".into(),
        vec![hp_edit(
            pcx_objects::ValueType::Int,
            pcx_objects::Value::Int(75),
        )],
    );
    assert_eq!(
        catalog_from(
            &rows,
            &tree_metadata(),
            None,
            edits,
            pcx_objects::editor::Overlay::default(),
        )
        .stats(ltlt)
        .unwrap()
        .max_life,
        75.0
    );

    let mut edits = pcx_objects::editor::Overlay::default();
    edits.custom.insert(
        "L001".into(),
        (
            "LTlt".into(),
            vec![hp_edit(
                pcx_objects::ValueType::Int,
                pcx_objects::Value::Int(30),
            )],
        ),
    );
    let custom = catalog_from(
        &rows,
        &tree_metadata(),
        None,
        edits,
        pcx_objects::editor::Overlay::default(),
    );
    assert_eq!(
        custom.stats(i32::from_be_bytes(*b"L001")).unwrap().max_life,
        30.0
    );
    let case = custom.stats(i32::from_be_bytes(*b"ltlt")).unwrap_err();
    assert!(case.contains("case-insensitively"), "{case}");

    let mut edits = pcx_objects::editor::Overlay::default();
    edits.mods.insert("LTlt".into(), Vec::new());
    let malformed = catalog_from(
        &[],
        &tree_metadata(),
        None,
        edits,
        pcx_objects::editor::Overlay::default(),
    )
    .stats(ltlt)
    .unwrap_err();
    assert!(
        malformed.contains("malformed catalog record"),
        "{malformed}"
    );
}

#[test]
fn nonpositive_signed_hp_overrides_refuse_before_creation() {
    for value in [-1_i32, i32::MIN, 0] {
        let mut edits = pcx_objects::editor::Overlay::default();
        edits.mods.insert(
            "LTlt".into(),
            vec![hp_edit(
                pcx_objects::ValueType::Int,
                pcx_objects::Value::Int(value as u32),
            )],
        );
        let catalog = catalog_from(
            &[&["LTlt", "50", "tree"]],
            &tree_metadata(),
            None,
            edits,
            pcx_objects::editor::Overlay::default(),
        );
        let mut host = host_from(&life_declarations(), "", 8)
            .with_destructables(DestructableBudget { max_records: 1 })
            .unwrap();
        host.admit_destructable_catalog(catalog).unwrap();
        let error = host
            .native(
                "CreateDestructable",
                &payload(i32::from_be_bytes(*b"LTlt"), 0),
            )
            .unwrap_err();
        assert!(error.contains("invalid authored HP"), "{value}: {error}");
        assert_eq!(host.executed_destructables().count(), 0);
    }
}

#[test]
fn absent_destructable_is_none_but_a_present_decoration_is_not_an_error() {
    let decoration = catalog_from(
        &[],
        &tree_metadata(),
        Some(&[&["LTlt", "tree.blp"]]),
        pcx_objects::editor::Overlay::default(),
        pcx_objects::editor::Overlay::default(),
    );
    assert_eq!(
        decoration
            .stats_if_destructable(i32::from_be_bytes(*b"LTlt"))
            .unwrap(),
        None
    );
    let absent = catalog_from(
        &[],
        &tree_metadata(),
        None,
        pcx_objects::editor::Overlay::default(),
        pcx_objects::editor::Overlay::default(),
    );
    let missing = absent
        .stats_if_destructable(i32::from_be_bytes(*b"LTlt"))
        .unwrap_err();
    assert!(
        missing.contains("missing destructable catalog record"),
        "{missing}"
    );
}

#[test]
fn missing_catalog_refuses_life_operations_without_inventing_hp() {
    let mut host = host_from(&life_declarations(), "", 8)
        .with_destructables(DestructableBudget { max_records: 1 })
        .unwrap();
    let objectid = i32::from_be_bytes(*b"LTlt");
    let created = host
        .native("CreateDestructable", &payload(objectid, 0))
        .unwrap();
    let Value::Handle(id) = created else {
        panic!("destructable handle");
    };
    let state = host.destructable_state(id).unwrap();
    assert_eq!(state.life, None);
    assert_eq!(state.max_life, None);
    assert!(!state.dead);
    for name in [
        "GetDestructableLife",
        "GetDestructableMaxLife",
        "KillDestructable",
    ] {
        let error = host.native(name, &[Value::Handle(id)]).unwrap_err();
        assert!(error.contains("catalog HP not resolved"), "{name}: {error}");
    }
    let error = host
        .native(
            "SetDestructableLife",
            &[Value::Handle(id), Value::Real(1.0)],
        )
        .unwrap_err();
    assert!(error.contains("catalog HP not resolved"), "{error}");
    let error = host.damage_destructable(id, 1.0).unwrap_err();
    assert_eq!(
        error.failure,
        Failure::DestructableState("catalog HP not resolved")
    );
    let state = host.destructable_state(id).unwrap();
    assert_eq!(state.life, None);
    assert!(!state.dead);
}

#[test]
fn runtime_validates_the_complete_signature_before_a_null_target() {
    let mut host = life_host(1);
    let objectid = i32::from_be_bytes(*b"LTlt");
    let created = host
        .native("CreateDestructable", &payload(objectid, 0))
        .unwrap();
    let Value::Handle(id) = created else {
        panic!("destructable handle");
    };
    for (name, args) in [
        ("SetDestructableLife", vec![Value::Null]),
        ("KillDestructable", vec![Value::Null, Value::Bool(true)]),
        ("GetDestructableLife", vec![Value::Null, Value::Int(1)]),
        ("SetDestructableInvulnerable", vec![Value::Null]),
    ] {
        let error = host.native(name, &args).unwrap_err();
        assert!(
            error.contains("incompatible native/type declaration"),
            "{name}: {error}"
        );
    }
    assert_eq!(
        host.native("GetDestructableLife", &[Value::Null]).unwrap(),
        Value::Real(0.0)
    );
    let state = host.destructable_state(id).unwrap();
    assert_eq!(state.life, Some(50.0));
    for (name, args) in [
        ("SetDestructableLife", vec![Value::Handle(id)]),
        ("SetDestructableInvulnerable", vec![Value::Handle(id)]),
        (
            "KillDestructable",
            vec![Value::Handle(id), Value::Bool(true)],
        ),
    ] {
        let error = host.native(name, &args).unwrap_err();
        assert!(
            error.contains("incompatible native/type declaration"),
            "{name}: {error}"
        );
    }
    assert_eq!(host.destructable_state(id).unwrap(), state);
    let wrong_family = host
        .native("GetDestructableLife", &[Value::Int(0)])
        .unwrap_err();
    assert!(wrong_family.contains("another family"), "{wrong_family}");
}

#[test]
fn script_creation_uses_the_catalog_life_and_shared_death_mutation() {
    let mut host = life_host(4);
    let objectid = i32::from_be_bytes(*b"LTlt");
    let created = host
        .native("CreateDestructable", &payload(objectid, 0))
        .unwrap();
    let Value::Handle(id) = created else {
        panic!("destructable handle");
    };
    assert_eq!(
        host.destructable_state(id).unwrap(),
        DestructableState {
            dead: false,
            life: Some(50.0),
            max_life: Some(50.0),
            ..DestructableState::default()
        }
    );
    assert_eq!(
        host.native("GetDestructableLife", &[Value::Handle(id)])
            .unwrap(),
        Value::Real(50.0)
    );
    assert_eq!(
        host.native("GetDestructableMaxLife", &[Value::Handle(id)])
            .unwrap(),
        Value::Real(50.0)
    );
    host.native(
        "SetDestructableLife",
        &[Value::Handle(id), Value::Real(12.5)],
    )
    .unwrap();
    assert_eq!(host.destructable_state(id).unwrap().life, Some(12.5));
    host.native(
        "SetDestructableLife",
        &[Value::Handle(id), Value::Real(500.0)],
    )
    .unwrap();
    assert_eq!(host.destructable_state(id).unwrap().life, Some(50.0));
    host.native(
        "SetDestructableLife",
        &[Value::Handle(id), Value::Real(0.0)],
    )
    .unwrap();
    let dead = host.destructable_state(id).unwrap();
    assert!(dead.dead);
    assert_eq!(dead.life, Some(0.0));
    host.native(
        "SetDestructableLife",
        &[Value::Handle(id), Value::Real(10.0)],
    )
    .unwrap();
    assert!(!host.destructable_state(id).unwrap().dead);
    host.native("KillDestructable", &[Value::Handle(id)])
        .unwrap();
    let killed = host.destructable_state(id).unwrap();
    assert!(killed.dead);
    assert_eq!(killed.life, Some(0.0));
    let dead_created = host
        .native("CreateDestructable", &payload(objectid, 0))
        .unwrap();
    let Value::Handle(dead_id) = dead_created else {
        panic!("dead handle");
    };
    let _ = host
        .native(
            "SetDestructableLife",
            &[Value::Handle(dead_id), Value::Real(0.0)],
        )
        .unwrap();
    assert!(host.destructable_state(dead_id).unwrap().dead);
    let malformed = host
        .native(
            "SetDestructableLife",
            &[Value::Handle(id), Value::Real(f32::NAN)],
        )
        .unwrap_err();
    assert!(
        malformed.contains("incompatible native/type declaration"),
        "{malformed}"
    );
}

#[test]
fn invulnerability_and_damage_share_the_same_state_path() {
    let mut host = life_host(1);
    let objectid = i32::from_be_bytes(*b"LTlt");
    let created = host
        .native("CreateDestructable", &payload(objectid, 0))
        .unwrap();
    let Value::Handle(id) = created else {
        panic!("destructable handle");
    };
    host.native(
        "SetDestructableInvulnerable",
        &[Value::Handle(id), Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(host.damage_destructable(id, 10.0).unwrap(), 50.0);
    assert_eq!(
        host.damage_destructable(id, -1.0).unwrap_err().failure,
        Failure::DestructableState("finite nonnegative damage")
    );
    assert_eq!(host.damage_destructable(id, 20.0).unwrap(), 50.0);
    host.native(
        "SetDestructableInvulnerable",
        &[Value::Handle(id), Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(host.damage_destructable(id, 20.0).unwrap(), 30.0);
    assert!(!host.destructable_state(id).unwrap().dead);
    assert_eq!(host.damage_destructable(id, 30.0).unwrap(), 0.0);
    assert!(host.destructable_state(id).unwrap().dead);
}

#[test]
fn placed_admission_is_atomic_and_preserves_the_authored_life() {
    let mut host = life_host(1);
    let before = host.inner().handles();
    let refused = host
        .admit_placed_destructables(&[placed_row(1, false), placed_row(2, true)])
        .unwrap_err();
    assert_eq!(refused.failure, Failure::Budget("destructable creation"));
    assert_eq!(host.inner().handles(), before);
    let ids = host
        .admit_placed_destructables(&[placed_row(3, false)])
        .unwrap();
    assert_eq!(ids.len(), 1);
    let state = host.destructable_state(ids[0]).unwrap();
    assert_eq!(state.life, Some(50.0));
    assert_eq!(state.max_life, Some(50.0));
    assert!(!state.dead);
    assert_eq!(
        host.destructable_creation(ids[0]).unwrap().origin,
        DestructableOrigin::Placed { creation_number: 3 }
    );
    let source = host.placed_source(ids[0]).expect("placed source");
    assert_eq!(source.creation_number, 3);
    assert_eq!(source.type_id, *b"LTlt");
    assert_eq!(source.position, [1.0, 2.0, 3.0]);
    assert_eq!(source.scale, [1.0, 1.0, 1.0]);
    assert_eq!(source.life, 100);
}

#[test]
fn placed_handles_retain_the_complete_authored_doo_record() {
    let mut host = life_host(1);
    let mut row = placed_row(77, false);
    row.source.flags = 2;
    row.source.variation = 9;
    row.source.angle = 1.25;
    row.source.scale = [1.5, 2.0, 3.0];
    row.source.life = 37;
    let ids = host.admit_placed_destructables(&[row]).unwrap();
    assert_eq!(host.placed_source(ids[0]), Some(row.source));
}

#[test]
fn death_toggles_only_the_script_objects_stationary_layer() {
    let mut host = life_host(1);
    let objectid = i32::from_be_bytes(*b"LTlt");
    let created = host
        .native("CreateDestructable", &payload(objectid, 0))
        .unwrap();
    let Value::Handle(id) = created else {
        panic!("destructable handle");
    };
    let mut base = pcx_map::Pathing {
        version: 0,
        width: 32,
        height: 32,
        cells: vec![0; 1024],
    };
    base.cells[100] = 0x01;
    let placement = crate::unit_placement::tests::placement(
        pcx_sim::world::Source {
            install: "synthetic".into(),
            edition: "test".into(),
            map: "death".into(),
        },
        1000,
    )
    .with_stationary(
        base,
        vec![crate::unit_placement::StationaryStamp {
            key: crate::unit_placement::StationaryKey::Destructable(id),
            live: vec![100],
            dead: vec![200],
            mask: 0x02,
            active: true,
        }],
    )
    .unwrap();
    host.test_attach_placement(placement);
    assert_eq!(host.test_placement_cell(100), Some(0x03));
    assert_eq!(host.test_placement_cell(200), Some(0x00));
    host.damage_destructable(id, 50.0).unwrap();
    assert_eq!(host.test_placement_cell(100), Some(0x01));
    assert_eq!(host.test_placement_cell(200), Some(0x02));
    host.native(
        "SetDestructableLife",
        &[Value::Handle(id), Value::Real(5.0)],
    )
    .unwrap();
    assert_eq!(host.test_placement_cell(100), Some(0x03));
    assert_eq!(host.test_placement_cell(200), Some(0x00));
    host.native("KillDestructable", &[Value::Handle(id)])
        .unwrap();
    assert_eq!(host.test_placement_cell(100), Some(0x01));
    assert_eq!(host.test_placement_cell(200), Some(0x02));
}

#[test]
fn late_script_creation_composes_live_and_dead_layers_over_existing_collision() {
    let tree = i32::from_be_bytes(*b"LTlt");
    let mut host = life_host(2);
    host.test_attach_placement(script_placement(
        16,
        1 << 20,
        vec![StationaryStamp {
            key: StationaryKey::Placed(1),
            live: vec![cell(8, 8)],
            dead: Vec::new(),
            mask: 0x02,
            active: true,
        }],
    ));
    assert_eq!(host.test_placement_cell(cell(8, 8)), Some(0x02));
    let created = host
        .native("CreateDestructable", &payload_at(tree, 32.0, 32.0))
        .unwrap();
    let Value::Handle(id) = created else {
        panic!("late destructable handle");
    };
    assert_eq!(host.test_placement_cell(cell(8, 8)), Some(0x0a));
    assert_eq!(host.test_placement_cell(cell(9, 8)), Some(0x00));
    host.native("KillDestructable", &[Value::Handle(id)])
        .unwrap();
    assert_eq!(host.test_placement_cell(cell(8, 8)), Some(0x02));
    assert_eq!(host.test_placement_cell(cell(9, 8)), Some(0x08));
    host.native(
        "SetDestructableLife",
        &[Value::Handle(id), Value::Real(5.0)],
    )
    .unwrap();
    assert_eq!(host.test_placement_cell(cell(8, 8)), Some(0x0a));
    assert_eq!(host.test_placement_cell(cell(9, 8)), Some(0x00));
    let dead = host
        .native("CreateDeadDestructable", &payload_at(tree, 64.0, 64.0))
        .unwrap();
    let Value::Handle(dead_id) = dead else {
        panic!("late dead destructable handle");
    };
    assert_eq!(host.destructable_state(dead_id).unwrap().life, Some(0.0));
    assert_eq!(host.test_placement_cell(cell(16, 16)), Some(0x00));
    assert_eq!(host.test_placement_cell(cell(17, 16)), Some(0x08));
}

#[test]
fn late_script_creation_refuses_sources_and_work_without_publishing() {
    for objectid in [i32::from_be_bytes(*b"BADD"), 0x1234_5678] {
        let mut host = admitted(2, 8);
        host.test_attach_placement(script_placement(16, 1 << 20, Vec::new()));
        let before = host.inner().handles();
        let error = host
            .native("CreateDestructable", &payload_at(objectid, 32.0, 32.0))
            .unwrap_err();
        assert!(
            error.contains("missing synthetic pathing texture")
                || error.contains("unknown synthetic stationary identity"),
            "{objectid:#x}: {error}"
        );
        assert_eq!(host.inner().handles(), before);
        assert_eq!(host.test_placement_cell(cell(8, 8)), Some(0x00));
        assert_eq!(host.test_placement_cell(cell(9, 8)), Some(0x00));
        host.native(
            "CreateDestructable",
            &payload_at(i32::from_be_bytes(*b"LTlt"), 32.0, 32.0),
        )
        .unwrap();
        assert_eq!(host.inner().handles(), before + 1);
    }

    let mut host = admitted(1, 8);
    host.test_attach_placement(script_placement(16, 1, Vec::new()));
    let before = host.inner().handles();
    let error = host
        .native(
            "CreateDestructable",
            &payload_at(i32::from_be_bytes(*b"LTlt"), 32.0, 32.0),
        )
        .unwrap_err();
    assert!(error.contains("work budget"), "{error}");
    assert_eq!(host.inner().handles(), before);
    assert_eq!(host.test_placement_cell(cell(8, 8)), Some(0x00));
    assert_eq!(host.test_placement_cell(cell(9, 8)), Some(0x00));

    let mut host = admitted(2, 8);
    host.test_attach_placement(script_placement(1, 2, Vec::new()));
    let before = host.inner().handles();
    let error = host
        .native(
            "CreateDestructable",
            &payload_at(i32::from_be_bytes(*b"LTlt"), 32.0, 32.0),
        )
        .unwrap_err();
    assert!(error.contains("stationary cell budget"), "{error}");
    assert_eq!(host.inner().handles(), before);
    host.native(
        "CreateDestructable",
        &payload_at(i32::from_be_bytes(*b"ONE1"), 32.0, 32.0),
    )
    .unwrap();
    assert_eq!(host.test_placement_cell(cell(8, 8)), Some(0x08));
    assert_eq!(host.inner().handles(), before + 1);
}

#[test]
fn late_script_creation_refuses_cell_and_layer_budgets_atomically() {
    let mut host = admitted(1, 8);
    host.test_attach_placement(script_placement(1, 1 << 20, Vec::new()));
    let before = host.inner().handles();
    let error = host
        .native(
            "CreateDestructable",
            &payload_at(i32::from_be_bytes(*b"LTlt"), 32.0, 32.0),
        )
        .unwrap_err();
    assert!(error.contains("stationary cell budget"), "{error}");
    assert_eq!(host.inner().handles(), before);
    assert_eq!(host.test_placement_cell(cell(8, 8)), Some(0x00));
    assert_eq!(host.test_placement_cell(cell(9, 8)), Some(0x00));

    let layers = (0..64)
        .map(|n| StationaryStamp {
            key: StationaryKey::Placed(n),
            live: Vec::new(),
            dead: Vec::new(),
            mask: 0x08,
            active: true,
        })
        .collect();
    let mut host = admitted(1, 8);
    host.test_attach_placement(script_placement(1024, 1 << 20, layers));
    let before = host.inner().handles();
    let error = host
        .native(
            "CreateDestructable",
            &payload_at(i32::from_be_bytes(*b"LTlt"), 32.0, 32.0),
        )
        .unwrap_err();
    assert!(error.contains("stationary layer budget"), "{error}");
    assert_eq!(host.inner().handles(), before);
    assert_eq!(host.test_placement_cell(cell(8, 8)), Some(0x00));
}

#[test]
fn refused_late_creation_rolls_back_source_work_before_a_distinct_retry() {
    let good = i32::from_be_bytes(*b"GOOD");
    for (objectid, source_budget, stationary_cells, stationary_work, refusal) in [
        (
            i32::from_be_bytes(*b"MISS"),
            3,
            16,
            3,
            "missing synthetic pathing texture",
        ),
        (
            i32::from_be_bytes(*b"MALF"),
            3,
            16,
            3,
            "malformed synthetic pathing texture",
        ),
        (i32::from_be_bytes(*b"WORK"), 4, 16, 3, "work budget"),
        (
            i32::from_be_bytes(*b"CELL"),
            4,
            1,
            1 << 10,
            "stationary cell budget",
        ),
    ] {
        let source = Arc::new(Mutex::new(SyntheticSource {
            remaining: source_budget,
            ..SyntheticSource::default()
        }));
        let mut host = admitted(2, 8);
        host.test_attach_placement(budgeted_script_placement(
            Arc::clone(&source),
            stationary_cells,
            stationary_work,
        ));
        let before = host.inner().handles();
        let error = host
            .native("CreateDestructable", &payload_at(objectid, 32.0, 32.0))
            .unwrap_err();
        assert!(error.contains(refusal), "{objectid:#x}: {error}");
        assert_eq!(host.inner().handles(), before);
        assert_eq!(host.test_placement_cell(cell(8, 8)), Some(0x00));
        {
            let source = source.lock().unwrap();
            assert_eq!(source.remaining, source_budget, "{objectid:#x}");
            assert!(source.staged.is_empty(), "{objectid:#x}");
            assert!(source.committed.is_empty(), "{objectid:#x}");
        }
        host.native("CreateDestructable", &payload_at(good, 32.0, 32.0))
            .unwrap();
        assert_eq!(host.inner().handles(), before + 1, "{objectid:#x}");
        assert_eq!(
            host.test_placement_cell(cell(8, 8)),
            Some(0x08),
            "{objectid:#x}"
        );
        let source = source.lock().unwrap();
        assert!(source.staged.is_empty(), "{objectid:#x}");
        assert!(
            source.committed.contains_key("good-live.tga"),
            "{objectid:#x}"
        );
    }
}

#[test]
fn executed_payload_and_declaration_source_stay_on_the_destructable_identity() {
    let mut host = admitted(2, 8);
    let (source, span) = host.destructable_declaration().unwrap();
    assert_eq!(source, "synthetic:destructables");
    let parsed = pcx_script::parse_script(DECLARATIONS).unwrap();
    let declared = parsed
        .natives()
        .find(|native| native.name.name == "CreateDestructable")
        .unwrap();
    assert_eq!(span, declared.span);
    assert!(DECLARATIONS[span.start as usize..span.end as usize].contains("objectid"));
    let before = host.inner().handles();
    let created = create(&mut host, -7, i32::MIN).unwrap();
    let Value::Handle(id) = created else {
        panic!("destructable handle");
    };
    assert_eq!(host.inner().kind(id).unwrap(), "destructable");
    assert_eq!(
        host.destructable_creation(id).unwrap(),
        DestructableCreation {
            origin: DestructableOrigin::Script,
            objectid: -7,
            x: 1.5,
            y: -2.25,
            face: 0.0,
            scale: 3.5,
            variation: i32::MIN,
            z: None,
            dead: false,
        }
    );
    let timer = host.native("CreateTimer", &[]).unwrap();
    let Value::Handle(timer_id) = timer else {
        panic!("timer");
    };
    assert_eq!(host.inner().kind(timer_id).unwrap(), "timer");
    assert_ne!(id, timer_id);
    let family = host
        .native("DestroyTimer", &[Value::Handle(id)])
        .unwrap_err();
    assert!(family.contains("another family"), "{family}");
    assert_eq!(
        host.destructable_creation(timer_id).unwrap_err().failure,
        Failure::WrongFamily
    );
    assert_eq!(host.inner().handles(), before + 2);
    let other = host
        .native("CreateDestructableZ", &payload(-7, 1))
        .unwrap_err();
    assert!(other.contains("CreateDestructableZ"), "{other}");
    assert!(other.contains("unsupported"), "{other}");
    assert_eq!(host.inner().handles(), before + 2);
}

#[test]
fn zero_budget_malformed_calls_and_arena_capacity_keep_the_prior_record() {
    let mut empty = admitted(0, 4);
    let handles = empty.inner().handles();
    let refused = create(&mut empty, 1, 0).unwrap_err();
    assert!(refused.contains("destructable creation"), "{refused}");
    assert_eq!(empty.inner().handles(), handles);
    let mut host = admitted(2, 1);
    let created = create(&mut host, 4, 0).unwrap();
    let Value::Handle(id) = created else {
        panic!("destructable handle");
    };
    let kept = host.destructable_creation(id).unwrap();
    let handles = host.inner().handles();
    let capacity = create(&mut host, 5, 1).unwrap_err();
    assert!(capacity.contains("capacity"), "{capacity}");
    assert_eq!(host.inner().handles(), handles);
    assert_eq!(host.destructable_creation(id).unwrap(), kept);
    let malformed = host
        .native(
            "CreateDestructable",
            &[
                Value::Int(9),
                Value::Int(1),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap_err();
    assert!(
        malformed.contains("arguments do not match the destructable native declaration"),
        "{malformed}"
    );
    assert!(!malformed.contains("exception"), "{malformed}");
    let infinite = host
        .native(
            "CreateDestructable",
            &[
                Value::Int(9),
                Value::Real(f32::INFINITY),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap_err();
    assert!(
        infinite.contains("recreation policy: finite real required"),
        "{infinite}"
    );
    assert!(!infinite.contains("exception"), "{infinite}");
    assert_eq!(host.inner().handles(), handles);
    assert_eq!(host.destructable_creation(id).unwrap(), kept);
}

#[test]
fn admission_requires_widget_ancestry_one_source_and_the_declared_signature() {
    let again = match admitted(1, 4).with_destructables(DestructableBudget { max_records: 1 }) {
        Err(error) => error,
        Ok(_) => panic!("destructables were admitted twice"),
    };
    assert_eq!(
        again.failure,
        Failure::DestructableState("already admitted")
    );
    let direct = DECLARATIONS.replace(
        "type destructable extends widget\n",
        "type destructable extends agent\n",
    );
    let ancestry =
        match host_from(&direct, "", 4).with_destructables(DestructableBudget { max_records: 1 }) {
            Err(error) => error,
            Ok(_) => panic!("destructable ancestry was admitted"),
        };
    assert_eq!(ancestry.failure, Failure::Signature);
    let wrong = DECLARATIONS.replace(
        "integer variation returns destructable",
        "integer variation returns widget",
    );
    let signature =
        match host_from(&wrong, "", 4).with_destructables(DestructableBudget { max_records: 1 }) {
            Err(error) => error,
            Ok(_) => panic!("wrong return type was admitted"),
        };
    assert_eq!(signature.failure, Failure::Signature);
    let duplicate = "native CreateDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable\n";
    let split = match host_from(DECLARATIONS, duplicate, 4)
        .with_destructables(DestructableBudget { max_records: 1 })
    {
        Err(error) => error,
        Ok(_) => panic!("duplicate declaration was admitted"),
    };
    assert_eq!(split.failure, Failure::Source);
    let mut bare = host_from(DECLARATIONS, "", 4);
    let unsupported = bare
        .native("CreateDestructable", &payload(1, 0))
        .unwrap_err();
    assert!(unsupported.contains("unsupported"), "{unsupported}");
}

fn variant_declarations() -> String {
    let globals = DECLARATIONS.find("\nglobals\n").expect("globals");
    let mut text = DECLARATIONS.to_string();
    text.insert_str(
        globals + 1,
        "native CreateDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable\n\
native CreateDeadDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable\n\
native CreateDeadDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable\n\
native RemoveDestructable takes destructable d returns nothing\n\
native KillDestructable takes destructable d returns nothing\n",
    );
    text
}

#[test]
fn variants_share_one_budget_and_absent_or_bad_declarations_stay_isolated() {
    let declarations = variant_declarations();
    let mut host = host_from(&declarations, "", 4)
        .with_destructables(DestructableBudget { max_records: 1 })
        .unwrap();
    let parsed = pcx_script::parse_script(&declarations).unwrap();
    for name in [
        "CreateDestructable",
        "CreateDestructableZ",
        "CreateDeadDestructable",
        "CreateDeadDestructableZ",
    ] {
        let (source, span) = host
            .destructable_variant_declaration(name)
            .unwrap()
            .expect(name);
        assert_eq!(source, "synthetic:destructables");
        let declared = parsed
            .natives()
            .find(|native| native.name.name == name)
            .unwrap();
        assert_eq!(span, declared.span);
    }
    assert!(
        admitted(1, 4)
            .destructable_variant_declaration("CreateDeadDestructableZ")
            .unwrap()
            .is_none()
    );
    let handles = host.inner().handles();
    let infinite = host
        .native(
            "CreateDestructableZ",
            &[
                Value::Int(1),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Real(f32::NAN),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap_err();
    assert!(
        infinite.contains("recreation policy: finite real required"),
        "{infinite}"
    );
    let wrong_kind = host
        .native(
            "CreateDeadDestructableZ",
            &[
                Value::Int(1),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Int(4),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap_err();
    assert!(
        wrong_kind.contains("arguments do not match the destructable native declaration"),
        "{wrong_kind}"
    );
    assert!(!wrong_kind.contains("exception"), "{wrong_kind}");
    assert_eq!(host.inner().handles(), handles);
    let created = host
        .native(
            "CreateDeadDestructable",
            &[
                Value::Int(-2),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Real(-8.0),
                Value::Real(0.5),
                Value::Int(3),
            ],
        )
        .unwrap();
    let Value::Handle(id) = created else {
        panic!("dead destructable");
    };
    assert_eq!(
        host.destructable_creation(id).unwrap(),
        DestructableCreation {
            origin: DestructableOrigin::Script,
            objectid: -2,
            x: 0.0,
            y: 1.0,
            z: None,
            face: -8.0,
            scale: 0.5,
            variation: 3,
            dead: true,
        }
    );
    let budget = host
        .native("CreateDestructableZ", &{
            let mut args = payload(1, 0).to_vec();
            args.insert(3, Value::Real(9.0));
            args
        })
        .unwrap_err();
    assert!(budget.contains("destructable creation"), "{budget}");
    assert!(host.destructable_creation(id).unwrap().dead);
    assert_eq!(host.inner().handles(), handles + 1);
    let refused = host
        .native("RemoveDestructable", &[Value::Handle(id)])
        .unwrap_err();
    assert!(refused.contains("unsupported"), "{refused}");
    assert_eq!(host.destructable_creation(id).unwrap().variation, 3);
    // Without a catalog there is no effective life to kill.
    let unresolved = host
        .native("KillDestructable", &[Value::Handle(id)])
        .unwrap_err();
    assert!(
        unresolved.contains("catalog HP not resolved"),
        "{unresolved}"
    );
    assert!(host.destructable_state(id).unwrap().dead);
    let broken = declarations.replace(
        "native CreateDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable",
        "native CreateDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns widget",
    );
    let signature =
        match host_from(&broken, "", 4).with_destructables(DestructableBudget { max_records: 1 }) {
            Err(error) => error,
            Ok(_) => panic!("wrong Z declaration was admitted"),
        };
    assert_eq!(signature.failure, Failure::Signature);
    let only_in_map = "native CreateDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable\n";
    let foreign = match host_from(DECLARATIONS, only_in_map, 4)
        .with_destructables(DestructableBudget { max_records: 1 })
    {
        Err(error) => error,
        Ok(_) => panic!("cross-source Z declaration was admitted"),
    };
    assert_eq!(foreign.failure, Failure::Source);
}

fn wave() -> Vec<u8> {
    let mut bytes = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
    bytes.extend(16u32.to_le_bytes());
    for value in [1u16, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(8000u32.to_le_bytes());
    bytes.extend(16000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(4u32.to_le_bytes());
    bytes.extend([0u8, 0, 0, 0]);
    let size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    bytes
}

#[test]
fn owned_vm_keeps_destructable_records_beside_sound_timer_and_player_identities() {
    let common = COMMON.replace(
        "type agent extends handle\n",
        "type agent extends handle\ntype widget extends agent\ntype destructable extends widget\ntype sound extends agent\n",
    );
    let globals = common.find("\nglobals\n").expect("globals block");
    let mut common = common;
    common.insert_str(
        globals + 1,
        "native CreateSound takes string file, boolean repeating, boolean spatial, boolean stop, integer fadeIn, integer fadeOut, string eax returns sound\nnative CreateDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable\nnative CreateDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable\nnative CreateDeadDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable\nnative CreateDeadDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable\n",
    );
    let map = "globals\nplayer P = null\ntimer T = null\nsound S = null\ndestructable A = null\ndestructable B = null\ndestructable C = null\ndestructable D = null\nendglobals\nfunction main takes nothing returns nothing\nset P = Player(0)\nset T = CreateTimer()\nset S = CreateSound(\"Media\\\\a.wav\", false, false, false, 0, 0, \"\")\nset A = CreateDestructable(-7, 1.5, -2.25, 0.0, 3.5, -1)\nset B = CreateDestructableZ(-3, 0.5, 2.0, -4.25, 90.0, 1.0, 2)\nset C = CreateDeadDestructable(8, -1.0, 0.0, 15.0, 0.25, 0)\nset D = CreateDeadDestructableZ(1, 3.0, -3.0, 12.5, 180.0, 2.0, -5)\nendfunction\n";
    let ctx = context(301, &common, map, "destructable-creation");
    let pcm = wave();
    let assets = Assets::read(
        &[SoundInput {
            member: r"Media\a.wav",
            asset: ScriptAsset {
                label: "synthetic:archive::Media\\a.wav",
                bytes: &pcm,
            },
            origin: AssetIdentity {
                edition: "synthetic".into(),
                version: "test".into(),
                source: "synthetic:PCM".into(),
            },
        }],
        SoundBudget {
            resources: 1,
            resource_bytes: pcm.len(),
            total_bytes: pcm.len(),
            metadata_bytes: 1024,
        },
    )
    .unwrap();
    let host = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_file_sounds(assets, 8)
        .unwrap()
        .with_destructables(DestructableBudget { max_records: 4 })
        .unwrap();
    let (source, span) = host.destructable_declaration().unwrap();
    assert_eq!(source, "synthetic/common.j");
    assert!(common[span.start as usize..span.end as usize].contains("CreateDestructable"));
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let Value::Handle(player) = vm.global("P") else {
        panic!("player");
    };
    let Value::Handle(timer) = vm.global("T") else {
        panic!("timer");
    };
    let Value::Handle(sound) = vm.global("S") else {
        panic!("sound");
    };
    let Value::Handle(base) = vm.global("A") else {
        panic!("base destructable");
    };
    let Value::Handle(elevated) = vm.global("B") else {
        panic!("z destructable");
    };
    let Value::Handle(dead) = vm.global("C") else {
        panic!("dead destructable");
    };
    let Value::Handle(dead_z) = vm.global("D") else {
        panic!("dead z destructable");
    };
    let host = vm.host();
    assert_eq!(host.inner.inner().kind(player).unwrap(), "player");
    assert_eq!(host.inner.inner().kind(timer).unwrap(), "timer");
    assert_eq!(host.inner.inner().kind(sound).unwrap(), "sound");
    for id in [base, elevated, dead, dead_z] {
        assert_eq!(host.inner.inner().kind(id).unwrap(), "destructable");
    }
    assert_eq!(
        host.inner
            .native("GetPlayerId", &[Value::Handle(player)])
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        host.inner
            .file_sound_state(sound)
            .unwrap()
            .resource
            .member(),
        r"Media\a.wav"
    );
    assert_eq!(
        host.inner.destructable_creation(base).unwrap(),
        DestructableCreation {
            origin: DestructableOrigin::Script,
            objectid: -7,
            x: 1.5,
            y: -2.25,
            z: None,
            face: 0.0,
            scale: 3.5,
            variation: -1,
            dead: false,
        }
    );
    assert_eq!(
        host.inner.destructable_creation(elevated).unwrap().z,
        Some(-4.25)
    );
    assert!(!host.inner.destructable_creation(elevated).unwrap().dead);
    assert_eq!(host.inner.destructable_creation(dead).unwrap().z, None);
    assert!(host.inner.destructable_creation(dead).unwrap().dead);
    assert_eq!(
        host.inner.destructable_creation(dead_z).unwrap().z,
        Some(12.5)
    );
    assert!(host.inner.destructable_creation(dead_z).unwrap().dead);
    assert_eq!(
        host.inner.destructable_creation(sound).unwrap_err().failure,
        Failure::WrongFamily
    );
    let cross = host
        .inner
        .native("GetPlayerId", &[Value::Handle(base)])
        .unwrap_err();
    assert!(cross.contains("another family"), "{cross}");
}
