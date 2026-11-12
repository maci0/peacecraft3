//! Entangle (`entangleinstant`), cargo `load` and Town Bell (`townbelloff`) orders on synthetic
//! ability tables. The numbers are fixtures, not the install's values: the gold seed, the
//! facing quarter and the created type must come from the tables and rules the test supplies.
use super::*;
use crate::playercolor_host::{VisionOrigin, VisionShape};
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_objects::editor::{Overlay, SlkId};
use pcx_script::{HandleId, Value};
use pcx_sim::world::{ObjectCatalog, ResolutionProblem};

const BUILDING_QUARTER: u8 = 3;
const GOLD_SEED: &str = "4242";
const MINE_GOLD: i32 = 1234;
const ENTANGLE_RANGE: &str = "600";
const SENTINEL_RANGE: &str = "800";
const SENTINEL_SIGHT: &str = "900";
const SENTINEL_OWLS: &str = "2";

const MAP: &str = r#"
globals
unit C=null
unit M=null
unit B=null
unit L=null
unit P=null
unit Base=null
unit Active=null
unit S=null
endglobals
function setup takes nothing returns nothing
set C=CreateUnit(Player(4),'u001',16.0,16.0,0.0)
set M=CreateUnit(Player(5),'ngol',48.0,16.0,0.0)
set B=CreateUnit(Player(4),'u002',80.0,16.0,0.0)
set L=CreateUnit(Player(4),'u003',112.0,16.0,0.0)
set P=CreateUnit(Player(4),'u004',144.0,16.0,0.0)
set S=CreateUnit(Player(4),'u007',240.0,16.0,0.0)
endfunction
function make_base takes player p returns unit
return CreateUnit(p,'u005',176.0,16.0,0.0)
endfunction
function make_active takes player p returns unit
return CreateUnit(p,'u006',208.0,16.0,0.0)
endfunction
"#;

fn sylk(headers: &[&str], rows: &[Vec<String>]) -> Vec<u8> {
    let mut text = String::from("ID;PWXL;N;E\r\n");
    for (y, row) in std::iter::once(headers.iter().map(|cell| (*cell).to_owned()).collect())
        .chain(rows.iter().cloned())
        .enumerate()
    {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

fn tables() -> DefaultTables {
    let headers = [
        "alias",
        "code",
        "levels",
        "reqLevel",
        "levelSkip",
        "hero",
        "item",
        "checkDep",
        "Cool1",
        "Cost1",
        "Rng1",
        "DataA1",
        "DataB1",
        "UnitID1",
        "DataD1",
        "targs1",
    ];
    let row = |alias: &str, edits: &[(&str, &str)]| -> Vec<String> {
        headers
            .iter()
            .map(|header| match *header {
                "alias" | "code" => alias.to_owned(),
                "levels" | "checkDep" => "1".to_owned(),
                _ => edits
                    .iter()
                    .find(|(name, _)| name == header)
                    .map_or("0", |(_, value)| *value)
                    .to_owned(),
            })
            .collect()
    };
    let rows = [
        row("Aent", &[("Rng1", ENTANGLE_RANGE), ("UnitID1", "egol")]),
        row("Agld", &[("DataA1", GOLD_SEED)]),
        row("Aloa", &[]),
        row("Amil", &[("DataA1", "u005"), ("DataB1", "u006")]),
        row("Amic", &[]),
        row(
            "Aesn",
            &[
                ("Rng1", SENTINEL_RANGE),
                ("DataB1", SENTINEL_SIGHT),
                ("DataD1", SENTINEL_OWLS),
                ("targs1", "tree,vuln,invu"),
            ],
        ),
    ];
    let meta_headers = ["ID", "field", "slk", "index", "type", "category", "data"];
    let metadata = sylk(
        &meta_headers,
        &[vec![
            "Ent2".into(),
            "UnitID".into(),
            "AbilityData".into(),
            "-1".into(),
            "unitCode".into(),
            "data".into(),
            "0".into(),
        ]],
    );
    let profile = "[Aent]\r\nOrder=entangle\r\nHotkey=E\r\n[Aloa]\r\nOrder=load\r\nHotkey=L\r\n[Amil]\r\nOrder=militia\r\nUnorder=militiaoff\r\nHotkey=M\r\n[Amic]\r\nUnorder=townbelloff\r\nHotkey=T\r\n";
    DefaultTables::from_bytes(
        Identity {
            install: source().install,
            edition_version: "fixture".into(),
        },
        &[
            (SlkId::AbilityData.path(), &sylk(&headers, &rows)),
            (SlkId::AbilityMetaData.path(), &metadata),
        ],
        &[("Units\\HumanAbilityFunc.txt", profile.as_bytes())],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 256 << 10,
        },
    )
    .expect("synthetic ability order tables")
}

const TREE_CODE: &[u8; 4] = b"LTlt";
const TREE_LIFE: f32 = 50.0;
/// Positions of the placed trees: the first within `SENTINEL_RANGE` of the caster at x=240, the
/// second beyond it.
const NEAR_TREE: [f32; 2] = [400.0, 16.0];
const FAR_TREE: [f32; 2] = [5000.0, 16.0];

fn tree_catalog() -> crate::playercolor_host::DestructableCatalog {
    use pcx_objects::editor::EditorReadLimits;
    let data = sylk(
        &["DestructableID", "HP", "targType"],
        &[vec!["LTlt".into(), "50".into(), "tree,ground".into()]],
    );
    let metadata = sylk(
        &["ID", "field", "slk", "type", "index", "category"],
        &[
            ["Ahpv", "HP", "DestructableData", "int", "0", "stats"]
                .map(String::from)
                .to_vec(),
            [
                "Atgt",
                "targType",
                "DestructableData",
                "string",
                "0",
                "combat",
            ]
            .map(String::from)
            .to_vec(),
        ],
    );
    let tables = DefaultTables::from_bytes(
        Identity {
            install: source().install,
            edition_version: "fixture".into(),
        },
        &[
            (SlkId::DestructableData.path(), &data),
            (SlkId::DestructableMetaData.path(), &metadata),
        ],
        &[],
        EditorReadLimits {
            max_listed_paths: 8,
            max_asset_bytes: 4096,
            max_aggregate_input_bytes: 16384,
        },
    )
    .expect("synthetic destructable tables");
    crate::playercolor_host::DestructableCatalog::new(
        Arc::new(tables),
        Overlay::default(),
        Overlay::default(),
    )
}

fn placed_trees() -> Vec<crate::playercolor_host::PlacedDestructable> {
    use crate::playercolor_host::{
        DestructableCreation, DestructableOrigin, DestructableState, PlacedDestructable,
        PlacedSource,
    };
    [NEAR_TREE, FAR_TREE]
        .into_iter()
        .zip(1u32..)
        .map(|([x, y], creation_number)| PlacedDestructable {
            creation: DestructableCreation {
                origin: DestructableOrigin::Placed { creation_number },
                objectid: i32::from_be_bytes(*TREE_CODE),
                x,
                y,
                z: Some(0.0),
                face: 0.0,
                scale: 1.0,
                variation: 0,
                dead: false,
            },
            state: DestructableState {
                life: Some(TREE_LIFE),
                max_life: Some(TREE_LIFE),
                ..DestructableState::default()
            },
            source: PlacedSource {
                creation_number,
                type_id: *TREE_CODE,
                variation: 0,
                position: [x, y, 0.0],
                angle: 0.0,
                scale: [1.0, 1.0, 1.0],
                flags: 0,
                life: 100,
            },
        })
        .collect()
}
/// Every unit type lists its own abilities and `buffType`, apart from the shared fixture fields.
struct Types(Arc<dyn ObjectCatalog + Send + Sync>);

impl ObjectCatalog for Types {
    fn resolve(&self, code: [u8; 4]) -> Result<pcx_sim::world::Definition, ResolutionProblem> {
        let mut definition = self.0.resolve(code)?;
        let (abilities, buff_type) = match &code {
            b"u001" => ("Aent", "_"),
            b"ngol" => ("Agld", "resource"),
            b"egol" => ("_", "resource"),
            b"u002" => ("Amic", "_"),
            b"u003" => ("Aloa", "_"),
            b"u005" | b"u006" => ("Amil", "_"),
            b"u007" => ("Aesn", "_"),
            _ => ("_", "_"),
        };
        for field in definition.fields.values_mut() {
            let Some(metadata) = &field.metadata else {
                continue;
            };
            let FieldSource::Column { slk, column } = &metadata.source else {
                continue;
            };
            match (slk, column.as_str()) {
                (SlkId::UnitAbilities, "abilList") => {
                    field.value = Ok(FieldValue::Cell(abilities.into()));
                }
                (SlkId::UnitData, "buffType") => {
                    field.value = Ok(FieldValue::Cell(buff_type.into()));
                }
                _ => {}
            }
        }
        Ok(definition)
    }
}

fn open() -> (Vm, [HandleId; 5]) {
    open_with_clock(false)
}

fn open_with_clock(clock: bool) -> (Vm, [HandleId; 5]) {
    let (registry, activity) = super::activity::rules_with_load();
    let (hero_rules, combat_rules) = hero_rules();
    let mut fields = hero_fields("STR");
    fields.push(column(
        b"V001",
        SlkId::UnitBalance,
        "sight",
        FieldKind::Real,
        "10",
    ));
    fields.push(column(
        b"V002",
        SlkId::UnitBalance,
        "nsight",
        FieldKind::Real,
        "10",
    ));
    fields.push(column(
        b"uabt",
        SlkId::UnitData,
        "buffType",
        FieldKind::Token,
        "_",
    ));
    let extra = format!(
        "{}\n{}{DECLARATIONS}{HOLY_DECLARATIONS}\n{}\n{}",
        enumeration::TRIGGERS,
        super::activity::ORDERS,
        super::resource::RESOURCE_FAMILY_NATIVES,
        if clock { CLOCK_DECLARATIONS } else { "" },
    );
    let ability_overlay = Overlay::default();
    let hero_budget = crate::playercolor_host::HeroBudget {
        abilities_per_unit: 8,
        overlay_objects: 0,
        modifications_per_ability: 0,
    };
    let origin = crate::scripts::AssetIdentity {
        source: source().install,
        edition: "fixture".into(),
        version: "fixture".into(),
    };
    // The footprint geometry carries `Misc/BuildingAngle`, which the conversion reads for the
    // facing of the created unit.
    let placement_rules = crate::unit_placement::tests::rules(5, -8192.25).with_footprint_geometry(
        crate::footprint_geometry::tests::with_building_quarter(BUILDING_QUARTER),
    );
    let mut h = position_host_with_placement_rules(
        1000,
        16,
        MAP,
        &extra,
        &fields,
        &placed_trees(),
        placement_rules,
        |h| h,
    )
    .with_resource_amounts(Arc::new(tables()))
    .unwrap()
    .with_militia_ability(&tables(), &ability_overlay)
    .unwrap()
    .with_player_abilities(16)
    .unwrap()
    .with_locations()
    .unwrap()
    .with_player_unit_events(8)
    .unwrap()
    .with_unit_states(crate::playercolor_host::UnitStateBudget { max_records: 8 })
    .unwrap()
    .with_player_alliances(crate::playercolor_host::PlayerAllianceBudget { max_records: 8 })
    .unwrap()
    .with_unit_explosions(8)
    .unwrap()
    .with_order_registry(registry)
    .unwrap()
    .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
    .unwrap()
    .with_unit_effects()
    .unwrap()
    .with_hero_skills(tables(), origin, ability_overlay, hero_rules, hero_budget)
    .unwrap();
    h.admit_unit_activity(activity, 65536).unwrap();
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.0,
        mana_per_intelligence: 100.0,
        life_regen_per_strength: 0.0,
        mana_regen_per_intelligence: 0.0,
    })
    .unwrap();
    h.admit_combat(combat_rules, "beta").unwrap();
    h.admit_destructable_catalog(tree_catalog()).unwrap();
    h.set_neutral_passive(15);
    let types = Arc::new(Types(h.unit_catalog().unwrap()));
    h.construction_test_catalog(types);
    if clock {
        h = h
            .with_startup_clock(crate::startup_clock::tests::input(9), "CLOCK", 64, 16, 1000)
            .unwrap();
        h.admit_time_of_day(&pcx_objects::editor::misc::DayCycle {
            day_length: 360.,
            day_hours: 24.,
            sources: vec!["synthetic".into()],
        })
        .unwrap();
    }
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let id = |vm: &Vm, name| match vm.global(name) {
        Value::Handle(id) => id,
        _ => panic!("{name}"),
    };
    let ids = ["C", "M", "B", "L", "P"].map(|name| id(&vm, name));
    (vm, ids)
}

fn issue_target(
    vm: &mut Vm,
    caster: HandleId,
    order: &str,
    target: HandleId,
) -> Result<Value, String> {
    vm.host().inner.native(
        "IssueTargetOrder",
        &[
            Value::Handle(caster),
            Value::Str(order.into()),
            Value::Handle(target),
        ],
    )
}

fn issue_immediate(vm: &mut Vm, unit: HandleId, order: &str) -> Result<Value, String> {
    vm.host().inner.native(
        "IssueImmediateOrder",
        &[Value::Handle(unit), Value::Str(order.into())],
    )
}

fn make(vm: &mut Vm, function: &str, owner: HandleId) -> HandleId {
    match vm.call(function, &[Value::Handle(owner)]).unwrap() {
        Value::Handle(id) => id,
        other => panic!("{function} returned {other:?}"),
    }
}

fn player(vm: &mut Vm, number: i32) -> HandleId {
    match vm
        .host()
        .inner
        .native("Player", &[Value::Int(number)])
        .unwrap()
    {
        Value::Handle(id) => id,
        other => panic!("player returned {other:?}"),
    }
}

fn hidden(vm: &mut Vm, unit: HandleId) -> bool {
    vm.host()
        .inner
        .native("IsUnitHidden", &[Value::Handle(unit)])
        .unwrap()
        == Value::Bool(true)
}

fn set_gold(vm: &mut Vm, mine: HandleId, amount: i32) {
    vm.host()
        .inner
        .native(
            "SetResourceAmount",
            &[Value::Handle(mine), Value::Int(amount)],
        )
        .unwrap();
}

/// Units of type `rawcode` that exist, with their handles.
fn units_of(vm: &mut Vm, rawcode: &[u8; 4]) -> Vec<HandleId> {
    vm.host()
        .inner
        .executed_units()
        .filter(|(_, record)| record.definition.rawcode == *rawcode)
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn entangleinstant_hides_the_mine_and_creates_the_ability_type_for_the_casters_owner() {
    let (mut vm, [caster, mine, ..]) = open();
    set_gold(&mut vm, mine, MINE_GOLD);
    let owner = vm.host().inner.executed_unit(caster).unwrap().owner;
    let mine_position = vm.host().inner.executed_unit(mine).unwrap().position;
    let neutral = player(&mut vm, 5);
    assert_eq!(vm.host().inner.executed_unit(mine).unwrap().owner, neutral);
    assert!(units_of(&mut vm, b"egol").is_empty());

    assert_eq!(
        issue_target(&mut vm, caster, "entangleinstant", mine).unwrap(),
        Value::Bool(true)
    );

    assert!(hidden(&mut vm, mine));
    let created = units_of(&mut vm, b"egol");
    let [entangled] = created.as_slice() else {
        panic!("one entangled mine expected, got {created:?}");
    };
    let record = vm.host().inner.executed_unit(*entangled).unwrap();
    assert_eq!(record.owner, owner);
    assert_eq!(record.position, mine_position);
    assert_eq!(record.face, f32::from(BUILDING_QUARTER) * 90.0);
    assert_eq!(record.resource_amount, Some(MINE_GOLD));
    assert_eq!(record.linked_mine, Some(mine));
    assert!(!hidden(&mut vm, *entangled));
}

#[test]
fn entangleinstant_returns_false_for_a_caster_without_entangle_and_a_target_without_gold() {
    let (mut vm, [caster, mine, bell, _, plain]) = open();
    assert_eq!(
        issue_target(&mut vm, plain, "entangleinstant", mine).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        issue_target(&mut vm, caster, "entangleinstant", bell).unwrap(),
        Value::Bool(false)
    );
    assert!(!hidden(&mut vm, mine));
    assert!(units_of(&mut vm, b"egol").is_empty());
}

#[test]
fn entangleinstant_refuses_a_mine_that_is_already_entangled() {
    let (mut vm, [caster, mine, ..]) = open();
    issue_target(&mut vm, caster, "entangleinstant", mine).unwrap();
    let entangled = units_of(&mut vm, b"egol")[0];
    let error = issue_target(&mut vm, caster, "entangleinstant", entangled).unwrap_err();
    assert!(error.contains("without Agld"), "{error}");
    assert_eq!(units_of(&mut vm, b"egol"), vec![entangled]);
}

#[test]
fn cargo_load_returns_false_when_no_ability_answers_the_load_order() {
    let (mut vm, [caster, mine, bell, _, plain]) = open();
    for unit in [caster, bell, plain] {
        assert_eq!(
            issue_target(&mut vm, unit, "load", mine).unwrap(),
            Value::Bool(false),
            "{unit:?}"
        );
    }
    assert!(!hidden(&mut vm, mine));
}

#[test]
fn cargo_load_by_a_unit_with_a_load_ability_stays_refused() {
    let (mut vm, [_, _, bell, carrier, _]) = open();
    let error = issue_target(&mut vm, carrier, "load", bell).unwrap_err();
    assert!(
        error.contains("cargo load by a cargo-load ability is not decoded"),
        "{error}"
    );
}

#[test]
fn townbelloff_without_militia_in_the_active_form_issues_nothing() {
    let (mut vm, [_, _, bell, ..]) = open();
    assert_eq!(
        issue_immediate(&mut vm, bell, "townbelloff").unwrap(),
        Value::Bool(false)
    );
    let owner = player(&mut vm, 4);
    let other = player(&mut vm, 5);
    // A militia carrier still in its base form, and an active one owned by another player, do
    // not match the owner scan.
    make(&mut vm, "make_base", owner);
    make(&mut vm, "make_active", other);
    assert_eq!(
        issue_immediate(&mut vm, bell, "townbelloff").unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn townbelloff_with_an_active_militia_of_the_owner_stays_refused() {
    let (mut vm, [_, _, bell, ..]) = open();
    let owner = player(&mut vm, 4);
    make(&mut vm, "make_active", owner);
    let error = issue_immediate(&mut vm, bell, "townbelloff").unwrap_err();
    assert!(
        error.contains("conversion callback is not decoded"),
        "{error}"
    );
}

#[test]
fn townbelloff_on_a_unit_without_the_town_bell_is_not_claimed() {
    let (mut vm, [.., plain]) = open();
    assert!(issue_immediate(&mut vm, plain, "townbelloff").is_err());
}

fn global_unit(vm: &Vm, name: &str) -> HandleId {
    match vm.global(name) {
        Value::Handle(id) => id,
        other => panic!("{name} is {other:?}"),
    }
}

/// Handle of the placed tree at `position`.
fn tree_at(vm: &mut Vm, position: [f32; 2]) -> HandleId {
    vm.host()
        .inner
        .destructables_with_handles()
        .find(|(_, creation, _)| [creation.x, creation.y] == position)
        .map(|(handle, ..)| handle)
        .expect("placed tree")
}

#[test]
fn sentinel_spends_one_owl_per_cast_and_attaches_a_sight_area_to_the_tree() {
    let (mut vm, _) = open();
    let caster = global_unit(&vm, "S");
    let tree = tree_at(&mut vm, NEAR_TREE);
    let owner = vm.host().inner.executed_unit(caster).unwrap().owner;
    assert_eq!(vm.host().inner.sentinel_owls().count(), 0);
    // The row lists two owls: the third order has none left.
    for casts in 1..=2 {
        assert_eq!(
            issue_target(&mut vm, caster, "sentinel", tree).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(vm.host().inner.sentinel_owls().count(), casts);
    }
    let owls: Vec<_> = vm.host().inner.sentinel_owls().collect();
    assert_eq!(owls[0], (tree, owner, NEAR_TREE, 900.0));
    let error = issue_target(&mut vm, caster, "sentinel", tree).unwrap_err();
    assert!(error.contains("no owls left"), "{error}");
    assert_eq!(vm.host().inner.sentinel_owls().count(), 2);
}

#[test]
fn a_sentinel_owl_gives_its_owner_a_circle_of_sight_at_the_tree() {
    let (mut vm, _) = open();
    let caster = global_unit(&vm, "S");
    let tree = tree_at(&mut vm, NEAR_TREE);
    let owner = player(&mut vm, 4);
    let other = player(&mut vm, 5);
    let circles = |vm: &mut Vm, viewer| {
        vm.host()
            .inner
            .vision_sources(viewer, false, 64)
            .unwrap()
            .into_iter()
            .filter(|source| matches!(source.origin, VisionOrigin::Sentinel { .. }))
            .map(|source| source.shape)
            .collect::<Vec<_>>()
    };
    assert!(circles(&mut vm, owner).is_empty());
    issue_target(&mut vm, caster, "sentinel", tree).unwrap();
    assert_eq!(
        circles(&mut vm, owner),
        vec![VisionShape::Circle {
            center: NEAR_TREE,
            radius: 900.0
        }]
    );
    assert!(circles(&mut vm, other).is_empty());
}

#[test]
fn sentinel_sources_sample_for_every_viewer_and_retain_exploration_after_removal() {
    use crate::playercolor_host::VisionState;
    let (mut vm, _) = open_with_clock(true);
    let caster = global_unit(&vm, "S");
    let tree = tree_at(&mut vm, NEAR_TREE);
    let owner = player(&mut vm, 4);
    let other = player(&mut vm, 5);
    let point = [16., 80.];
    let sample = |vm: &mut Vm, viewer| {
        let h = &mut vm.host().inner;
        let budget = h.shared_vision_work_limit().unwrap();
        h.sample_shared_vision(budget).unwrap();
        h.vision_cell(viewer, point).unwrap().unwrap()
    };
    assert!(vm.host().inner.admit_shared_vision().unwrap());
    assert!(!sample(&mut vm, owner).explored);
    for _ in 0..2 {
        assert_eq!(
            issue_target(&mut vm, caster, "sentinel", tree).unwrap(),
            Value::Bool(true)
        );
        let h = &mut vm.host().inner;
        let previous = h.vision_cell(owner, point).unwrap();
        let revision = h.vision_revision();
        assert!(h.sample_shared_vision(0).is_err());
        assert_eq!(h.vision_cell(owner, point).unwrap(), previous);
        assert_eq!(h.vision_revision(), revision);
        let cell = sample(&mut vm, owner);
        assert!(cell.explored);
        assert_eq!(cell.raw, VisionState::Visible);
        assert!(!sample(&mut vm, other).explored);
    }
    let h = &mut vm.host().inner;
    let candidates = h.executed_units().count() + h.sentinel_owls().count();
    assert!(h.vision_sources(other, false, candidates - 1).is_err());
    let revision = h.vision_revision();
    assert!(
        h.sample_shared_vision(0).is_ok(),
        "unchanged sample stamps no cells"
    );
    assert_eq!(h.vision_revision(), revision);
    h.damage_destructable(tree, 1.).unwrap();
    let cell = sample(&mut vm, owner);
    assert_eq!(cell.raw, VisionState::Fogged);
    assert!(cell.explored);
    assert!(!sample(&mut vm, other).explored);
}

#[test]
fn a_sentinel_owl_ends_when_its_tree_loses_life() {
    let (mut vm, _) = open();
    let caster = global_unit(&vm, "S");
    let tree = tree_at(&mut vm, NEAR_TREE);
    let other_tree = tree_at(&mut vm, FAR_TREE);
    issue_target(&mut vm, caster, "sentinel", tree).unwrap();
    vm.host()
        .inner
        .damage_destructable(other_tree, 1.0)
        .unwrap();
    assert_eq!(vm.host().inner.sentinel_owls().count(), 1);
    vm.host().inner.damage_destructable(tree, 1.0).unwrap();
    assert_eq!(vm.host().inner.sentinel_owls().count(), 0);
}

#[test]
fn sentinel_returns_false_without_the_ability_and_refuses_what_is_not_decoded() {
    let (mut vm, [plain, ..]) = open();
    let caster = global_unit(&vm, "S");
    let tree = tree_at(&mut vm, NEAR_TREE);
    assert_eq!(
        issue_target(&mut vm, plain, "sentinel", tree).unwrap(),
        Value::Bool(false)
    );
    let far = tree_at(&mut vm, FAR_TREE);
    let error = issue_target(&mut vm, caster, "sentinel", far).unwrap_err();
    assert!(error.contains("outside Rng 800"), "{error}");
    // A unit is not a tree.
    let error = issue_target(&mut vm, caster, "sentinel", plain).unwrap_err();
    assert!(error.contains("non-destructable"), "{error}");
    // A dead tree is a validation the shipped data does not settle.
    vm.host()
        .inner
        .damage_destructable(tree, TREE_LIFE)
        .unwrap();
    let error = issue_target(&mut vm, caster, "sentinel", tree).unwrap_err();
    assert!(error.contains("dead or hidden tree"), "{error}");
    assert_eq!(vm.host().inner.sentinel_owls().count(), 0);
}
