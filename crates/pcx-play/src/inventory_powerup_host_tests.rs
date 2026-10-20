//! Automatic powerup integration: grant, queued pickup delivery, consumption, levels and refusals.
//! Built on the combat hero fixture with the selected inventory registry and synthetic tome rows.
use super::*;
use crate::inventory::{InventoryBudget, Rules as InventoryRules};
use crate::inventory_powerup::Rules as TomeRules;
use crate::playercolor_host::{HeroBudget, ItemBudget, UnitStateBudget};
use crate::scripts::AssetIdentity;
use pcx_objects::editor::defaults::Identity;
use pcx_objects::editor::{EditorReadLimits, Overlay};
use pcx_sim::world::ResolutionProblem;

/// Source-shaped rows: the tome grants 7 STR through `DataC1`, one slot count through `L000`.
const POWERUP_DECLARATIONS: &str = r#"
type item extends widget
native CreateItem takes integer i, real x, real y returns item
native UnitAddItem takes unit u, item i returns boolean
native UnitRemoveItem takes unit u, item i returns nothing
native UnitHasItem takes unit u, item i returns boolean
native GetItemX takes item i returns real
native GetItemTypeId takes item i returns integer
native SetItemDropID takes item whichItem, integer unitId returns nothing
native GetHeroStr takes unit whichHero, boolean includeBonuses returns integer
native GetHeroAgi takes unit whichHero, boolean includeBonuses returns integer
native GetHeroInt takes unit whichHero, boolean includeBonuses returns integer
native RemoveUnit takes unit u returns nothing
constant native GetManipulatedItem takes nothing returns item
constant native GetManipulatingUnit takes nothing returns unit
globals
constant playerunitevent EVENT_PLAYER_UNIT_PICKUP_ITEM=ConvertPlayerUnitEvent(-71)
constant playerunitevent EVENT_PLAYER_UNIT_DROP_ITEM=ConvertPlayerUnitEvent(99)
endglobals
"#;
const TOME_STRENGTH: f32 = 7.0;
const LIFE_PER_STRENGTH: f32 = 25.0;
const PRIMARY_DAMAGE: f32 = 1.75;
const HERO_STR: f32 = 10.0;
const HERO_LIFE: f32 = 500.0 + HERO_STR * LIFE_PER_STRENGTH;
const LIFE_STATE: i32 = 0;

fn sylk(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
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

fn powerup_tables() -> Arc<DefaultTables> {
    let items = sylk(
        &[
            "itemID",
            "abilList",
            "powerup",
            "usable",
            "perishable",
            "uses",
        ],
        &[
            &["tstr", "AIsm", "1", "1", "1", "1"],
            &["tmix", "AIsm,L000", "1", "1", "1", "1"],
            &["tsrd", "", "0", "1", "1", "1"],
            &["pstr", "AIs4,AIa4", "0", "0", "0", "-"],
            &["pint", "AIi4", "0", "0", "0", "-"],
            &["pbad", "AIsm", "0", "0", "0", "-"],
            &["uact", "AIsm", "0", "1", "1", "1"],
            &["parm", "AId1", "0", "0", "0", "-"],
            &["parx", "AId8", "0", "0", "0", "-"],
        ],
    );
    let abilities = sylk(
        &[
            "alias", "code", "levels", "DataA1", "DataB1", "DataC1", "DataD1", "DataE1",
        ],
        &[
            &["L000", "Xinv", "1", "2", "1", "1", "1", "1"],
            &["AIsm", "AIsm", "1", "0", "0", "7", "0", "0"],
            &["AIs4", "AIab", "1", "0", "0", "5", "0", "0"],
            &["AIa4", "AIab", "1", "2", "0", "0", "0", "0"],
            &["AIi4", "AIab", "1", "0", "3", "0", "0", "0"],
            &["AId1", "AIde", "1", "1", "0", "0", "0", "0"],
            &["AId8", "AIde", "1", "8", "0", "0", "0", "0"],
        ],
    );
    let metadata = sylk(
        &[
            "ID",
            "field",
            "slk",
            "index",
            "type",
            "data",
            "useSpecific",
            "maxVal",
        ],
        &[
            &["inv1", "Data", "AbilityData", "-1", "int", "1", "L000", "9"],
            &[
                "inv2",
                "Data",
                "AbilityData",
                "-1",
                "bool",
                "2",
                "L000",
                "1",
            ],
            &[
                "inv3",
                "Data",
                "AbilityData",
                "-1",
                "bool",
                "3",
                "L000",
                "1",
            ],
            &[
                "inv4",
                "Data",
                "AbilityData",
                "-1",
                "bool",
                "4",
                "L000",
                "1",
            ],
            &[
                "inv5",
                "Data",
                "AbilityData",
                "-1",
                "bool",
                "5",
                "L000",
                "1",
            ],
            &[
                "Iagi",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "1",
                "AIsm,AIxm,AIs4,AIa4,AIi4",
                "",
            ],
            &[
                "Iint",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "2",
                "AIsm,AIxm,AIs4,AIa4,AIi4",
                "",
            ],
            &[
                "Istr",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "3",
                "AIsm,AIxm,AIs4,AIa4,AIi4",
                "",
            ],
            &[
                "Idef",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "1",
                "AId1",
                "",
            ],
        ],
    );
    Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic powerup".into(),
                edition_version: "fixture".into(),
            },
            &[
                (SlkId::ItemData.path(), &items),
                (SlkId::AbilityData.path(), &abilities),
                (SlkId::AbilityMetaData.path(), &metadata),
            ],
            &[],
            EditorReadLimits {
                max_listed_paths: 4,
                max_asset_bytes: 65536,
                max_aggregate_input_bytes: 131072,
            },
        )
        .unwrap(),
    )
}

/// Class-registration window for the source key `source` with base `base`, in reviewed layout.
fn registration(source: &str, base: &str) -> Vec<u8> {
    let mut w = vec![0x68, 1, 2, 3, 4, 0x68];
    w.extend(base.bytes().rev());
    w.push(0x68);
    w.extend(source.bytes().rev());
    w.extend([0xe8, 0, 0, 0, 0, 0x83, 0xc4, 0x0c, 0xc3]);
    w
}

struct PowerupItems;

impl ObjectCatalog for PowerupItems {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let powerup = if matches!(&rawcode, b"tstr" | b"tmix") {
            "1"
        } else {
            "0"
        };
        let fields = [(*b"ipow", "powerup", powerup)]
            .into_iter()
            .map(|(id, column, value)| {
                let mut field = geometry_field(Ok(FieldValue::Cell(value.into())));
                let m = field.metadata.as_mut().unwrap();
                m.fourcc = id;
                m.kind = FieldKind::Bool;
                m.source = FieldSource::Column {
                    slk: SlkId::ItemData,
                    column: column.into(),
                };
                (id, field)
            })
            .collect();
        Ok(definition(
            rawcode,
            pcx_objects::fourcc_text(rawcode),
            ObjectKind::Item,
            fields,
        ))
    }
}

struct Fixture {
    h: ObjectHost,
    hero: HandleId,
    ordinary: HandleId,
}

fn open() -> Fixture {
    let (hero_rules, combat) = hero_rules();
    // `uabi` supplies the one ability list; the hero fixture's own `abilList` would be ambiguous.
    let mut fields: Vec<_> = hero_fields("STR")
        .into_iter()
        .filter(|(id, _)| *id != *b"A008")
        .collect();
    fields.push((*b"uabi", ability_field("abilList", "L000")));
    let declarations = format!(
        "{}\n{}{DECLARATIONS}{POWERUP_DECLARATIONS}",
        enumeration::TRIGGERS,
        super::activity::ORDERS
    );
    let h = position_host_admitting(65536, 16, "// powerup", &declarations, &fields, |h| h);
    let tables = powerup_tables();
    let origin = AssetIdentity {
        source: "synthetic powerup".into(),
        edition: "fixture".into(),
        version: "fixture".into(),
    };
    let (registry, activity) = super::inventory::order_inputs();
    let inventory = InventoryRules::read(
        Arc::clone(&tables),
        Overlay::default(),
        Arc::clone(&registry),
        &[(
            "synthetic MiscGame",
            b"[Misc]\nPickupItemRange=5\nDropItemRange=7\nGiveItemRange=9".as_slice(),
        )],
        1024,
    )
    .unwrap();
    let tome = TomeRules::decode(
        Arc::clone(&tables),
        Overlay::default(),
        origin.clone(),
        [0; 32],
        &registration("AIsm", "AIxm"),
        &registration("AIab", "APbo"),
        &registration("AIde", "APbo"),
    )
    .unwrap();
    let mut h = h
        .with_order_registry(registry)
        .unwrap()
        .with_player_unit_events(8)
        .unwrap()
        .with_unit_states(UnitStateBudget { max_records: 8 })
        .unwrap()
        .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
        .unwrap()
        .with_hero_skills(
            tables,
            origin,
            Overlay::default(),
            hero_rules,
            HeroBudget {
                abilities_per_unit: 8,
                overlay_objects: 4,
                modifications_per_ability: 8,
            },
        )
        .unwrap()
        .with_item_catalog(
            Arc::new(PowerupItems),
            source(),
            ItemBudget {
                max_records: 16,
                max_retained_fields: 4,
                max_modifications: 0,
            },
        )
        .unwrap()
        .with_item_drops(16)
        .unwrap()
        .with_inventory(
            inventory,
            InventoryBudget {
                max_slots_per_unit: 9,
                max_pending_orders: 4,
            },
        )
        .unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: LIFE_PER_STRENGTH,
        mana_per_intelligence: 15.0,
    })
    .unwrap();
    let mut h = h.with_inventory_powerup(tome).unwrap();
    h.admit_unit_activity(activity, 65536).unwrap();
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_combat(combat, "alpha").unwrap();
    h.set_neutral_passive(15);
    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    let mut create = |raw: &[u8; 4], x: f32| {
        let Value::Handle(id) = h
            .native(
                "CreateUnit",
                &[
                    player.clone(),
                    Value::Int(i32::from_be_bytes(*raw)),
                    Value::Real(x),
                    Value::Real(0.0),
                    Value::Real(0.0),
                ],
            )
            .unwrap()
        else {
            panic!("unit handle")
        };
        id
    };
    let hero = create(b"H001", 0.0);
    let ordinary = create(b"u001", 0.0);
    Fixture { h, hero, ordinary }
}

fn item(h: &mut ObjectHost, raw: &[u8; 4], x: f32) -> HandleId {
    let Value::Handle(id) = h
        .native(
            "CreateItem",
            &[
                Value::Int(i32::from_be_bytes(*raw)),
                Value::Real(x),
                Value::Real(4.0),
            ],
        )
        .unwrap()
    else {
        panic!("item handle")
    };
    id
}

fn add(h: &mut ObjectHost, unit: HandleId, item: HandleId) -> Result<Value, String> {
    h.native("UnitAddItem", &[Value::Handle(unit), Value::Handle(item)])
}

/// Registers a player pickup trigger for the unit's owner and returns the trigger handle.
fn register_pickup(h: &mut ObjectHost, unit: HandleId) -> HandleId {
    let Value::Handle(trigger) = h.native("CreateTrigger", &[]).unwrap() else {
        panic!("trigger handle")
    };
    let owner = Value::Handle(h.executed_unit(unit).unwrap().owner);
    let event = h
        .native("ConvertPlayerUnitEvent", &[Value::Int(-71)])
        .unwrap();
    h.native(
        "TriggerRegisterPlayerUnitEvent",
        &[Value::Handle(trigger), owner, event, Value::Null],
    )
    .unwrap();
    trigger
}

fn life_state(h: &mut ObjectHost) -> Value {
    h.native("ConvertUnitState", &[Value::Int(LIFE_STATE)])
        .unwrap()
}

fn damage(h: &ObjectHost, unit: HandleId) -> f32 {
    h.hero_combat_bonuses(unit).unwrap().damage
}

#[test]
fn queued_pickup_reads_the_live_item_then_consumes_it_after_delivery() {
    let Fixture { mut h, hero, .. } = open();
    let trigger = register_pickup(&mut h, hero);
    let before = h.unit_vitals(hero).unwrap().unwrap();
    let tome = item(&mut h, b"tstr", 3.0);
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(true)));
    let granted = h.unit_vitals(hero).unwrap().unwrap();
    assert_eq!(
        granted.max_life,
        before.max_life + TOME_STRENGTH * LIFE_PER_STRENGTH
    );
    assert_eq!(
        granted.life,
        before.life + TOME_STRENGTH * LIFE_PER_STRENGTH
    );
    assert_eq!(
        damage(&h, hero),
        (HERO_STR + TOME_STRENGTH) * PRIMARY_DAMAGE
    );
    // The grant is applied; the item stays live until its pickup delivery finishes.
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(tome)]),
        Ok(Value::Bool(false))
    );
    let fired = h.take_fired_event().expect("pickup queued");
    assert_eq!(fired.trigger, trigger);
    h.begin_fired_event(&fired)
        .unwrap()
        .expect("enabled trigger");
    assert_eq!(h.native("GetManipulatedItem", &[]), Ok(Value::Handle(tome)));
    assert_eq!(
        h.native("GetManipulatingUnit", &[]),
        Ok(Value::Handle(hero))
    );
    assert_eq!(
        h.native("GetItemTypeId", &[Value::Handle(tome)]),
        Ok(Value::Int(i32::from_be_bytes(*b"tstr")))
    );
    assert_eq!(
        h.native("GetItemX", &[Value::Handle(tome)]),
        Ok(Value::Real(3.0))
    );
    // Inside the callback the hero already carries the retained strength.
    assert_eq!(
        damage(&h, hero),
        (HERO_STR + TOME_STRENGTH) * PRIMARY_DAMAGE
    );
    // A pending claim refuses a second grant of the same handle.
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(false)));
    assert_eq!(h.unit_vitals(hero).unwrap().unwrap(), granted);
    h.finish_fired_event();
    assert_eq!(
        h.native("GetItemTypeId", &[Value::Handle(tome)]),
        Ok(Value::Int(0))
    );
    assert_eq!(
        h.native("GetItemX", &[Value::Handle(tome)]),
        Ok(Value::Real(0.0))
    );
    assert!(h.executed_items().all(|(id, _)| id != tome));
    assert!(h.take_fired_event().is_none());
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(false)));
}

#[test]
fn powerup_is_granted_with_full_slots_while_ordinary_items_still_refuse() {
    let Fixture { mut h, hero, .. } = open();
    for _ in 0..2 {
        let held = item(&mut h, b"tsrd", 0.0);
        assert_eq!(add(&mut h, hero, held), Ok(Value::Bool(true)));
    }
    let overflow = item(&mut h, b"tsrd", 0.0);
    assert_eq!(add(&mut h, hero, overflow), Ok(Value::Bool(false)));
    register_pickup(&mut h, hero);
    let before = h.unit_vitals(hero).unwrap().unwrap();
    let tome = item(&mut h, b"tstr", 0.0);
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(true)));
    assert_eq!(
        h.unit_vitals(hero).unwrap().unwrap().max_life,
        before.max_life + TOME_STRENGTH * LIFE_PER_STRENGTH
    );
    let fired = h.take_fired_event().expect("pickup queued");
    h.begin_fired_event(&fired).unwrap().unwrap();
    h.finish_fired_event();
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(hero), Value::Handle(overflow)]
        ),
        Ok(Value::Bool(false))
    );
}

#[test]
fn retained_strength_survives_a_later_level_up() {
    let Fixture { mut h, hero, .. } = open();
    let tome = item(&mut h, b"tstr", 0.0);
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(true)));
    let granted = h.unit_vitals(hero).unwrap().unwrap();
    h.native(
        "SetHeroLevel",
        &[Value::Handle(hero), Value::Int(4), Value::Bool(false)],
    )
    .unwrap();
    // STR 10 plus 2.5 per gained level, floored to 12, then the retained 7.
    assert_eq!(damage(&h, hero), (12.0 + TOME_STRENGTH) * PRIMARY_DAMAGE);
    // The level delta adds the change in base STR only; the grant is not counted twice.
    assert_eq!(
        h.unit_vitals(hero).unwrap().unwrap().max_life,
        granted.max_life + (12.0 - HERO_STR) * LIFE_PER_STRENGTH
    );
}

#[test]
fn grant_keeps_the_life_percentage_not_the_absolute_missing_amount() {
    let Fixture { mut h, hero, .. } = open();
    let life = life_state(&mut h);
    h.native(
        "SetUnitState",
        &[Value::Handle(hero), life, Value::Real(HERO_LIFE / 2.0)],
    )
    .unwrap();
    let tome = item(&mut h, b"tstr", 0.0);
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(true)));
    let max = HERO_LIFE + TOME_STRENGTH * LIFE_PER_STRENGTH;
    assert_eq!(
        (
            h.unit_vitals(hero).unwrap().unwrap().life,
            h.unit_vitals(hero).unwrap().unwrap().max_life
        ),
        (max / 2.0, max)
    );
}

#[test]
fn mixed_ability_list_refuses_without_partial_state() {
    let Fixture { mut h, hero, .. } = open();
    register_pickup(&mut h, hero);
    let before = h.unit_vitals(hero).unwrap().unwrap();
    let damage_before = damage(&h, hero);
    let mixed = item(&mut h, b"tmix", 0.0);
    assert!(add(&mut h, hero, mixed).is_err());
    assert_eq!(h.unit_vitals(hero).unwrap().unwrap(), before);
    assert_eq!(damage(&h, hero), damage_before);
    assert!(!h.executed_item(mixed).unwrap().claimed);
    assert!(h.take_fired_event().is_none());
}

#[test]
fn non_hero_target_refuses_without_partial_state() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let tome = item(&mut h, b"tstr", 0.0);
    assert!(add(&mut h, ordinary, tome).is_err());
    assert!(!h.executed_item(tome).unwrap().claimed);
    assert!(h.executed_items().any(|(id, _)| id == tome));
}

#[test]
fn consumption_waits_for_every_queued_pickup_delivery() {
    let Fixture { mut h, hero, .. } = open();
    register_pickup(&mut h, hero);
    register_pickup(&mut h, hero);
    let tome = item(&mut h, b"tstr", 3.0);
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(true)));
    let first = h.take_fired_event().expect("first pickup");
    h.begin_fired_event(&first).unwrap().unwrap();
    h.finish_fired_event();
    // The second queued pickup still reads the live item.
    assert_eq!(
        h.native("GetItemX", &[Value::Handle(tome)]),
        Ok(Value::Real(3.0))
    );
    let second = h.take_fired_event().expect("second pickup");
    h.begin_fired_event(&second).unwrap().unwrap();
    h.finish_fired_event();
    assert_eq!(
        h.native("GetItemX", &[Value::Handle(tome)]),
        Ok(Value::Real(0.0))
    );
    assert!(h.executed_items().all(|(id, _)| id != tome));
}

#[test]
fn disabled_pickup_trigger_still_consumes_the_item() {
    let Fixture { mut h, hero, .. } = open();
    let trigger = register_pickup(&mut h, hero);
    h.native("DisableTrigger", &[Value::Handle(trigger)])
        .unwrap();
    let tome = item(&mut h, b"tstr", 0.0);
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(true)));
    let fired = h.take_fired_event().expect("pickup queued");
    // A skipped delivery has no finish call, so consumption happens at this begin.
    assert!(h.begin_fired_event(&fired).unwrap().is_none());
    assert!(h.executed_items().all(|(id, _)| id != tome));
    assert_eq!(
        h.native("GetItemTypeId", &[Value::Handle(tome)]),
        Ok(Value::Int(0))
    );
}

#[test]
fn consumed_powerup_refuses_set_item_drop_id_while_a_live_item_accepts_it() {
    let Fixture { mut h, hero, .. } = open();
    let live = item(&mut h, b"tsrd", 0.0);
    assert_eq!(
        h.native("SetItemDropID", &[Value::Handle(live), Value::Int(1234)]),
        Ok(Value::Null)
    );
    assert_eq!(h.item_drop_id(live), Some(1234));
    register_pickup(&mut h, hero);
    let tome = item(&mut h, b"tstr", 0.0);
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(true)));
    let fired = h.take_fired_event().expect("pickup queued");
    h.begin_fired_event(&fired).unwrap().unwrap();
    h.finish_fired_event();
    assert!(
        h.native("SetItemDropID", &[Value::Handle(tome), Value::Int(7)])
            .is_err()
    );
    assert_eq!(h.item_drop_id(tome), None);
}

fn strength_and_agility(h: &mut ObjectHost, unit: HandleId) -> (f32, f32) {
    let life = h.unit_vitals(unit).unwrap().unwrap();
    (life.life, life.max_life)
}

fn armor(h: &ObjectHost, unit: HandleId) -> f32 {
    h.hero_combat_bonuses(unit).unwrap().armor
}

/// Source passive bonus on the fixture hero: `pstr` gives 5 strength and 2 agility.
const PASSIVE_STRENGTH: f32 = 5.0;
const PASSIVE_AGILITY: f32 = 2.0;
const BASE_AGILITY: f32 = 5.0;
const ARMOR_BASE: f32 = -1.0;
const ARMOR_PER_AGILITY: f32 = 0.4;

#[test]
fn equipping_passive_strength_reprices_life_by_ratio_and_derives_damage_and_armor() {
    let Fixture { mut h, hero, .. } = open();
    let life = life_state(&mut h);
    h.native(
        "SetUnitState",
        &[Value::Handle(hero), life, Value::Real(HERO_LIFE / 2.0)],
    )
    .unwrap();
    let passive = item(&mut h, b"pstr", 0.0);
    assert_eq!(add(&mut h, hero, passive), Ok(Value::Bool(true)));
    let max = HERO_LIFE + PASSIVE_STRENGTH * LIFE_PER_STRENGTH;
    assert_eq!(strength_and_agility(&mut h, hero), (max / 2.0, max));
    assert_eq!(
        damage(&h, hero),
        (HERO_STR + PASSIVE_STRENGTH) * PRIMARY_DAMAGE
    );
    let agility = BASE_AGILITY + PASSIVE_AGILITY;
    assert!((armor(&h, hero) - (ARMOR_BASE + agility * ARMOR_PER_AGILITY)).abs() < 1e-4);
}

#[test]
fn dropping_a_passive_item_restores_pools_damage_and_armor() {
    let Fixture { mut h, hero, .. } = open();
    let before = strength_and_agility(&mut h, hero);
    let passive = item(&mut h, b"pstr", 0.0);
    assert_eq!(add(&mut h, hero, passive), Ok(Value::Bool(true)));
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(passive)],
    )
    .unwrap();
    assert_eq!(strength_and_agility(&mut h, hero), before);
    assert_eq!(damage(&h, hero), HERO_STR * PRIMARY_DAMAGE);
    assert!((armor(&h, hero) - (ARMOR_BASE + BASE_AGILITY * ARMOR_PER_AGILITY)).abs() < 1e-4);
}

#[test]
fn passive_strength_survives_a_level_up_without_double_counting() {
    let Fixture { mut h, hero, .. } = open();
    let passive = item(&mut h, b"pstr", 0.0);
    assert_eq!(add(&mut h, hero, passive), Ok(Value::Bool(true)));
    let equipped = strength_and_agility(&mut h, hero);
    h.native(
        "SetHeroLevel",
        &[Value::Handle(hero), Value::Int(4), Value::Bool(false)],
    )
    .unwrap();
    // Base STR 10 gains 2.5 per level, floored to 12; the passive 5 is a flat addition.
    assert_eq!(damage(&h, hero), (12.0 + PASSIVE_STRENGTH) * PRIMARY_DAMAGE);
    // The level change moves life by the base STR delta only.
    assert_eq!(
        strength_and_agility(&mut h, hero).1,
        equipped.1 + (12.0 - HERO_STR) * LIFE_PER_STRENGTH
    );
}

#[test]
fn transfer_to_a_non_hero_refuses_and_keeps_the_hero_bonus() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    let before = strength_and_agility(&mut h, hero);
    let passive = item(&mut h, b"pstr", 0.0);
    assert_eq!(add(&mut h, hero, passive), Ok(Value::Bool(true)));
    let equipped = strength_and_agility(&mut h, hero);
    let err = add(&mut h, ordinary, passive).unwrap_err();
    assert!(err.contains("non-hero"), "{err}");
    assert_eq!(strength_and_agility(&mut h, hero), equipped);
    assert_eq!(
        damage(&h, hero),
        (HERO_STR + PASSIVE_STRENGTH) * PRIMARY_DAMAGE
    );
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(hero), Value::Handle(passive)]
        ),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(ordinary), Value::Handle(passive)]
        ),
        Ok(Value::Bool(false))
    );
    assert_ne!(equipped, before);
}

#[test]
fn passive_stat_item_refuses_on_a_non_hero_without_a_held_slot() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let passive = item(&mut h, b"pstr", 0.0);
    let err = add(&mut h, ordinary, passive).unwrap_err();
    assert!(err.contains("non-hero"), "{err}");
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(ordinary), Value::Handle(passive)]
        ),
        Ok(Value::Bool(false))
    );
    assert!(!h.executed_item(passive).unwrap().claimed);
}

#[test]
fn death_drops_the_passive_item_and_removes_its_bonus() {
    let Fixture { mut h, hero, .. } = open();
    let before = strength_and_agility(&mut h, hero);
    let passive = item(&mut h, b"pstr", 0.0);
    assert_eq!(add(&mut h, hero, passive), Ok(Value::Bool(true)));
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(strength_and_agility(&mut h, hero), before);
    assert_eq!(damage(&h, hero), HERO_STR * PRIMARY_DAMAGE);
}

#[test]
fn unsupported_passive_families_refuse_atomically_with_a_diagnostic() {
    let Fixture { mut h, hero, .. } = open();
    let before = strength_and_agility(&mut h, hero);
    let mismatched = item(&mut h, b"pbad", 0.0);
    let err = add(&mut h, hero, mismatched).unwrap_err();
    assert!(err.contains("unsupported passive ability"), "{err}");
    let intelligence = item(&mut h, b"pint", 0.0);
    let err = add(&mut h, hero, intelligence).unwrap_err();
    assert!(err.contains("intelligence passive bonus"), "{err}");
    assert_eq!(strength_and_agility(&mut h, hero), before);
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(hero), Value::Handle(mismatched)]
        ),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(hero), Value::Handle(intelligence)]
        ),
        Ok(Value::Bool(false))
    );
}

#[test]
fn usable_items_are_held_without_a_passive_bonus() {
    let Fixture { mut h, hero, .. } = open();
    let before = strength_and_agility(&mut h, hero);
    let usable = item(&mut h, b"uact", 0.0);
    assert_eq!(add(&mut h, hero, usable), Ok(Value::Bool(true)));
    assert_eq!(strength_and_agility(&mut h, hero), before);
    assert_eq!(damage(&h, hero), HERO_STR * PRIMARY_DAMAGE);
}

#[test]
fn held_armor_on_a_hero_is_derived_and_removed_on_drop() {
    let Fixture { mut h, hero, .. } = open();
    let before = strength_and_agility(&mut h, hero);
    let armor = item(&mut h, b"parm", 0.0);
    assert_eq!(add(&mut h, hero, armor), Ok(Value::Bool(true)));
    assert_eq!(h.held_armor_bonus(hero).unwrap(), 1);
    // Armor is not a strength change: pools and strength damage stay at their base values.
    assert_eq!(strength_and_agility(&mut h, hero), before);
    assert_eq!(damage(&h, hero), HERO_STR * PRIMARY_DAMAGE);
    h.native("UnitRemoveItem", &[Value::Handle(hero), Value::Handle(armor)])
        .unwrap();
    assert_eq!(h.held_armor_bonus(hero).unwrap(), 0);
}

#[test]
fn held_armor_on_a_non_hero_is_supported_and_derived() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let armor = item(&mut h, b"parm", 0.0);
    assert_eq!(add(&mut h, ordinary, armor), Ok(Value::Bool(true)));
    assert_eq!(h.held_armor_bonus(ordinary).unwrap(), 1);
}

#[test]
fn armor_transfer_moves_the_points_to_the_receiver() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    let armor = item(&mut h, b"parm", 0.0);
    assert_eq!(add(&mut h, hero, armor), Ok(Value::Bool(true)));
    assert_eq!(add(&mut h, ordinary, armor), Ok(Value::Bool(true)));
    assert_eq!(h.held_armor_bonus(hero).unwrap(), 0);
    assert_eq!(h.held_armor_bonus(ordinary).unwrap(), 1);
}

#[test]
fn death_drops_held_armor_and_removes_its_points() {
    let Fixture { mut h, hero, .. } = open();
    let armor = item(&mut h, b"parm", 0.0);
    assert_eq!(add(&mut h, hero, armor), Ok(Value::Bool(true)));
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(h.held_armor_bonus(hero).unwrap(), 0);
}

#[test]
fn armor_row_outside_the_admission_refuses_atomically() {
    let Fixture { mut h, hero, .. } = open();
    let before = strength_and_agility(&mut h, hero);
    let refused = item(&mut h, b"parx", 0.0);
    let err = add(&mut h, hero, refused).unwrap_err();
    assert!(err.contains("armor metadata does not admit ability AId8"), "{err}");
    assert_eq!(h.held_armor_bonus(hero).unwrap(), 0);
    assert_eq!(strength_and_agility(&mut h, hero), before);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(refused)]),
        Ok(Value::Bool(false))
    );
}

/// Held armor enters the real damage formula: the target's armor, and so the damage it takes,
/// changes when the armor item is held and returns to the base value when it is dropped.
#[test]
fn held_armor_changes_real_damage_and_its_drop_restores_it() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    let weapon = crate::playercolor_host::Weapon {
        range: 1.0,
        acquire: 1.0,
        cooldown: 1.0,
        damage_point: 0.0,
        base: 10,
        dice: 0,
        sides: 1,
        attack: 0,
        missile_speed: None,
    };
    let bare = h.roll_damage(hero, weapon, ordinary).unwrap();
    let armor = item(&mut h, b"parm", 0.0);
    assert_eq!(add(&mut h, ordinary, armor), Ok(Value::Bool(true)));
    let armored = h.roll_damage(hero, weapon, ordinary).unwrap();
    assert!(armored < bare, "armor reduces damage: {armored} < {bare}");
    h.native("UnitRemoveItem", &[Value::Handle(ordinary), Value::Handle(armor)])
        .unwrap();
    assert_eq!(h.roll_damage(hero, weapon, ordinary).unwrap(), bare);
}

fn hero_attribute(
    h: &mut ObjectHost,
    native: &str,
    unit: HandleId,
    bonuses: bool,
) -> Result<i32, String> {
    match h.native(native, &[Value::Handle(unit), Value::Bool(bonuses)])? {
        Value::Int(n) => Ok(n),
        other => panic!("integer attribute, got {other:?}"),
    }
}

#[test]
fn hero_attribute_queries_split_base_permanent_and_held_totals() {
    let Fixture { mut h, hero, .. } = open();
    let query =
        |h: &mut ObjectHost, native: &str, bonuses: bool| hero_attribute(h, native, hero, bonuses);
    // Fresh hero at the admitted level 3: the base 10 / 5 / 7 with nothing added.
    assert_eq!(query(&mut h, "GetHeroStr", false), Ok(10));
    assert_eq!(query(&mut h, "GetHeroStr", true), Ok(10));
    assert_eq!(query(&mut h, "GetHeroAgi", true), Ok(5));
    assert_eq!(query(&mut h, "GetHeroInt", true), Ok(7));
    // The retained tome strength is permanent: it joins strength without `includeBonuses`.
    let tome = item(&mut h, b"tstr", 0.0);
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(true)));
    assert_eq!(query(&mut h, "GetHeroStr", false), Ok(17));
    assert_eq!(query(&mut h, "GetHeroStr", true), Ok(17));
    // The passive 5 strength and 2 agility of the held item join only with `includeBonuses`.
    let passive = item(&mut h, b"pstr", 0.0);
    assert_eq!(add(&mut h, hero, passive), Ok(Value::Bool(true)));
    assert_eq!(query(&mut h, "GetHeroStr", false), Ok(17));
    assert_eq!(query(&mut h, "GetHeroStr", true), Ok(22));
    assert_eq!(query(&mut h, "GetHeroAgi", false), Ok(5));
    assert_eq!(query(&mut h, "GetHeroAgi", true), Ok(7));
    // Intelligence passive bonuses refuse on pickup, so INT has no held total to add.
    assert_eq!(query(&mut h, "GetHeroInt", true), Ok(7));
    // Growth floors each attribute after the fractional per-level gain: STR 10+2.5, AGI 5+1.5.
    h.native(
        "SetHeroLevel",
        &[Value::Handle(hero), Value::Int(4), Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(query(&mut h, "GetHeroStr", false), Ok(19));
    assert_eq!(query(&mut h, "GetHeroStr", true), Ok(24));
    assert_eq!(query(&mut h, "GetHeroAgi", false), Ok(6));
    assert_eq!(query(&mut h, "GetHeroAgi", true), Ok(8));
    assert_eq!(query(&mut h, "GetHeroInt", false), Ok(7));
    h.native(
        "SetHeroLevel",
        &[Value::Handle(hero), Value::Int(5), Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(query(&mut h, "GetHeroStr", false), Ok(22));
    assert_eq!(query(&mut h, "GetHeroStr", true), Ok(27));
    assert_eq!(query(&mut h, "GetHeroAgi", true), Ok(10));
    assert_eq!(query(&mut h, "GetHeroInt", true), Ok(8));
}

#[test]
fn hero_attribute_queries_read_zero_for_ordinary_units_and_null() {
    let Fixture {
        mut h,
        ordinary,
        hero,
    } = open();
    for native in ["GetHeroStr", "GetHeroAgi", "GetHeroInt"] {
        for bonuses in [false, true] {
            assert_eq!(hero_attribute(&mut h, native, ordinary, bonuses), Ok(0));
        }
    }
    assert_eq!(
        h.native("GetHeroAgi", &[Value::Null, Value::Bool(true)]),
        Ok(Value::Int(0))
    );
    assert_eq!(hero_attribute(&mut h, "GetHeroInt", hero, false), Ok(7));
}

/// Hero attribute declarations of a selected `common.j`, normalized to single spaces. Each
/// must be a plain `native` (not `constant native`) with the signature the query host admits.
fn hero_attribute_declarations(common: &str) -> Result<(), String> {
    const EXPECTED: [&str; 3] = [
        "native GetHeroStr takes unit whichHero, boolean includeBonuses returns integer",
        "native GetHeroAgi takes unit whichHero, boolean includeBonuses returns integer",
        "native GetHeroInt takes unit whichHero, boolean includeBonuses returns integer",
    ];
    let lines: Vec<String> = common
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    for expected in EXPECTED {
        let count = lines
            .iter()
            .filter(|line| line.as_str() == expected)
            .count();
        if count != 1 {
            return Err(format!("{expected}: found {count} declarations"));
        }
    }
    Ok(())
}

#[test]
fn hero_attribute_declarations_must_match_the_admitted_signature() {
    let exact = "native          GetHeroStr          takes unit whichHero, boolean includeBonuses returns integer\n\
        native GetHeroAgi takes unit whichHero, boolean includeBonuses returns integer\n\
        native GetHeroInt takes unit whichHero, boolean includeBonuses returns integer\n";
    assert_eq!(hero_attribute_declarations(exact), Ok(()));
    let constant = exact.replace("native          GetHeroStr", "constant native GetHeroStr");
    assert!(hero_attribute_declarations(&constant).is_err());
    let missing = exact.replace(
        "native GetHeroInt takes unit whichHero, boolean includeBonuses returns integer\n",
        "",
    );
    assert!(
        hero_attribute_declarations(&missing)
            .unwrap_err()
            .contains("GetHeroInt")
    );
    let duplicated = format!("{exact}{exact}");
    assert!(hero_attribute_declarations(&duplicated).is_err());
}

/// Original-input gate: `PCX_WAR3_INSTALL` names a local Warcraft III install. The selected
/// `Scripts\common.j` is read through `MpqSet`, the archive set production configuration uses,
/// and must declare the three hero attribute natives with the admitted signature. Unset, skips.
#[test]
fn selected_install_common_declares_the_hero_attribute_natives() {
    let Some(install) = std::env::var_os("PCX_WAR3_INSTALL") else {
        return;
    };
    let mut set = pcx_mpq::MpqSet::open_wc3(&install).expect("readable install archives");
    let common = set
        .extract(r"Scripts\common.j")
        .expect("selected Scripts\\common.j");
    let text = std::str::from_utf8(&common).expect("common.j is text");
    hero_attribute_declarations(text).expect("selected common.j hero attribute declarations");
    eprintln!("install={install:?} common.j bytes={}", common.len());
}

#[test]
fn removed_hero_reads_zero_because_the_native_handle_gate_fails() {
    let Fixture { mut h, hero, .. } = open();
    h.native("RemoveUnit", &[Value::Handle(hero)]).unwrap();
    assert_eq!(hero_attribute(&mut h, "GetHeroStr", hero, true), Ok(0));
    assert_eq!(hero_attribute(&mut h, "GetHeroInt", hero, false), Ok(0));
}
