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
native UnitUseItem takes unit whichUnit, item whichItem returns boolean
native UnitUseItemTarget takes unit whichUnit, item whichItem, widget target returns boolean
native GetItemCharges takes item whichItem returns integer
native SetItemCharges takes item whichItem, integer count returns nothing
native UnitHasItem takes unit u, item i returns boolean
native GetItemX takes item i returns real
native GetItemTypeId takes item i returns integer
native SetItemDropID takes item whichItem, integer unitId returns nothing
constant native GetManipulatedItem takes nothing returns item
constant native GetManipulatingUnit takes nothing returns unit
globals
constant playerunitevent EVENT_PLAYER_UNIT_PICKUP_ITEM=ConvertPlayerUnitEvent(-71)
constant playerunitevent EVENT_PLAYER_UNIT_DROP_ITEM=ConvertPlayerUnitEvent(99)
constant playerunitevent EVENT_PLAYER_UNIT_USE_ITEM=ConvertPlayerUnitEvent(50)
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
            &["phea", "AIh1", "0", "1", "1", "1"],
            &["phcd", "AIh1", "0", "1", "1", "2"],
            &["phsm", "AIh2", "0", "1", "1", "1"],
            &["phtg", "AIh3", "0", "1", "1", "1"],
        ],
    );
    let abilities = sylk(
        &[
            "alias", "code", "levels", "DataA1", "DataB1", "DataC1", "DataD1", "DataE1", "Rng1",
            "Cool1", "targs1",
        ],
        &[
            &["L000", "Xinv", "1", "2", "1", "1", "1", "1", "-", "-", "_"],
            &["AIsm", "AIsm", "1", "0", "0", "7", "0", "0", "-", "-", "_"],
            &["AIs4", "AIab", "1", "0", "0", "5", "0", "0", "-", "-", "_"],
            &["AIa4", "AIab", "1", "2", "0", "0", "0", "0", "-", "-", "_"],
            &["AIi4", "AIab", "1", "0", "3", "0", "0", "0", "-", "-", "_"],
            &["AId1", "AIde", "1", "1", "0", "0", "0", "0", "-", "-", "_"],
            &["AId8", "AIde", "1", "8", "0", "0", "0", "0", "-", "-", "_"],
            &["AIh1", "AIhe", "1", "250", "0", "0", "0", "0", "100", "20", "_"],
            &["AIh2", "AIhe", "1", "250", "0", "0", "0", "0", "10", "20", "_"],
            &["AIh3", "AIhe", "1", "250", "0", "0", "0", "0", "100", "20", "ground,air"],
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
            &[
                "Ihpg",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "1",
                "AIh1,AIh2,AIh3",
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

/// Healing registration in the reviewed 25-byte layout, with its base function at a fixed offset.
fn healing_registration() -> Vec<u8> {
    use crate::inventory_powerup::{CALL_REL32, CLEANUP_RET, HEAL_WINDOW, PUSH_EAX, PUSH_IMM32, REGISTER_FN};
    let base_va = i64::from(HEAL_WINDOW) + 0x100;
    let rel_base = (base_va - (i64::from(HEAL_WINDOW) + 10)) as i32;
    let rel_register = (i64::from(REGISTER_FN) - (i64::from(HEAL_WINDOW) + 21)) as i32;
    let mut w = vec![PUSH_IMM32, 1, 2, 3, 4, CALL_REL32];
    w.extend(rel_base.to_le_bytes());
    w.push(PUSH_EAX);
    w.push(PUSH_IMM32);
    w.extend(b"AIhe".iter().rev());
    w.push(CALL_REL32);
    w.extend(rel_register.to_le_bytes());
    w.extend(CLEANUP_RET);
    w
}

/// The base function the healing window calls: `mov eax, "AAsm"; ret`.
fn healing_base() -> Vec<u8> {
    use crate::inventory_powerup::{MOV_EAX_IMM32, RET};
    let mut b = vec![MOV_EAX_IMM32];
    b.extend(b"AAsm".iter().rev());
    b.push(RET);
    b
}

struct PowerupItems;

impl ObjectCatalog for PowerupItems {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let powerup = if matches!(&rawcode, b"tstr" | b"tmix") {
            "1"
        } else {
            "0"
        };
        // Charge, perishable, cooldown and ability fields for the two healing rows: `phea` has no
        // cooldown group; `phcd` has two charges and cooldown group `AIhe`.
        let mut rows = vec![(*b"ipow", "powerup", powerup)];
        match &rawcode {
            b"phea" => rows.extend([
                (*b"iuse", "uses", "1"),
                (*b"iper", "perishable", "1"),
                (*b"iicd", "ignoreCD", "0"),
                (*b"icid", "cooldownID", ""),
                (*b"iabi", "abilList", "AIh1"),
            ]),
            b"phtg" => rows.extend([
                (*b"iuse", "uses", "1"),
                (*b"iper", "perishable", "1"),
                (*b"iicd", "ignoreCD", "0"),
                (*b"icid", "cooldownID", ""),
                (*b"iabi", "abilList", "AIh3"),
            ]),
            b"phsm" => rows.extend([
                (*b"iuse", "uses", "1"),
                (*b"iper", "perishable", "1"),
                (*b"iicd", "ignoreCD", "0"),
                (*b"icid", "cooldownID", ""),
                (*b"iabi", "abilList", "AIh2"),
            ]),
            b"phcd" => rows.extend([
                (*b"iuse", "uses", "2"),
                (*b"iper", "perishable", "1"),
                (*b"iicd", "ignoreCD", "0"),
                (*b"icid", "cooldownID", "AIhe"),
                (*b"iabi", "abilList", "AIh1"),
            ]),
            _ => {}
        }
        let fields = rows
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
    .unwrap()
    .with_heal(&healing_registration(), &healing_base())
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
                max_retained_fields: 8,
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
    let mut h = h
        .with_inventory_powerup(tome)
        .unwrap()
        .with_item_use(
            crate::item_use_rules::Rules::new(powerup_tables(), Overlay::default()),
            8,
        )
        .unwrap();
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

fn use_item(h: &mut ObjectHost, unit: HandleId, potion: HandleId) -> Result<Value, String> {
    h.native("UnitUseItem", &[Value::Handle(unit), Value::Handle(potion)])
}

fn use_item_on(h: &mut ObjectHost, unit: HandleId, potion: HandleId, target: HandleId) -> Result<Value, String> {
    h.native(
        "UnitUseItemTarget",
        &[Value::Handle(unit), Value::Handle(potion), Value::Handle(target)],
    )
}

fn set_life(h: &mut ObjectHost, unit: HandleId, life: f32) {
    let state = life_state(h);
    h.native("SetUnitState", &[Value::Handle(unit), state, Value::Real(life)])
        .unwrap();
}

fn life_of(h: &ObjectHost, unit: HandleId) -> f32 {
    h.unit_vitals(unit).unwrap().unwrap().life
}

#[test]
fn unit_use_heals_the_holder_by_the_source_amount_and_removes_a_perishable_potion() {
    let Fixture { mut h, hero, .. } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let potion = item(&mut h, b"phea", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    assert_eq!(use_item(&mut h, hero, potion), Ok(Value::Bool(true)));
    assert_eq!(life_of(&h, hero), max / 2.0 + 250.0);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(potion)]),
        Ok(Value::Bool(false))
    );
    assert!(h.executed_items().all(|(id, _)| id != potion));
}

#[test]
fn targeted_use_clamps_to_maximum_and_out_of_range_is_false_without_writes() {
    let Fixture { mut h, hero, ordinary } = open();
    h.set_timer_game_time(1.0).unwrap();
    let potion = item(&mut h, b"phea", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    let ordinary_max = h.unit_vitals(ordinary).unwrap().unwrap().max_life;
    set_life(&mut h, ordinary, ordinary_max - 100.0);
    assert_eq!(use_item_on(&mut h, hero, potion, ordinary), Ok(Value::Bool(true)));
    assert_eq!(life_of(&h, ordinary), ordinary_max);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(potion)]),
        Ok(Value::Bool(false))
    );
}

#[test]
fn use_refuses_without_writes_when_not_held_or_not_a_use() {
    let Fixture { mut h, hero, ordinary } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let before = life_of(&h, hero);
    let potion = item(&mut h, b"phea", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    assert_eq!(use_item(&mut h, ordinary, potion), Ok(Value::Bool(false)));
    assert_eq!(life_of(&h, hero), before);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(potion)]),
        Ok(Value::Bool(true))
    );
    let armor = item(&mut h, b"parm", 0.0);
    assert_eq!(add(&mut h, hero, armor), Ok(Value::Bool(true)));
    assert_eq!(use_item(&mut h, hero, armor), Ok(Value::Bool(false)));
    assert_eq!(life_of(&h, hero), before);
}

#[test]
fn charges_and_group_cooldown_refuse_without_writes_until_the_group_is_ready() {
    let Fixture { mut h, hero, .. } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let potion = item(&mut h, b"phcd", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    assert_eq!(use_item(&mut h, hero, potion), Ok(Value::Bool(true)));
    let after_first = life_of(&h, hero);
    assert_eq!(after_first, max / 2.0 + 250.0);
    // Cooling down: refused, and the second charge is still there.
    assert_eq!(use_item(&mut h, hero, potion), Ok(Value::Bool(false)));
    assert_eq!(life_of(&h, hero), after_first);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(potion)]),
        Ok(Value::Bool(true))
    );
    // Ready again: the last charge is used and the perishable potion is removed.
    h.set_timer_game_time(25.0).unwrap();
    assert_eq!(use_item(&mut h, hero, potion), Ok(Value::Bool(true)));
    assert_eq!(life_of(&h, hero), (after_first + 250.0).min(max));
    assert!(h.executed_items().all(|(id, _)| id != potion));
}

#[test]
fn out_of_range_target_is_false_for_a_short_range_potion() {
    let Fixture { mut h, hero, .. } = open();
    h.set_timer_game_time(1.0).unwrap();
    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    let far = match h
        .native(
            "CreateUnit",
            &[
                player,
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(50.0),
                Value::Real(0.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    {
        Value::Handle(id) => id,
        other => panic!("unit handle, got {other:?}"),
    };
    let potion = item(&mut h, b"phsm", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    let far_life = life_of(&h, far);
    assert_eq!(use_item_on(&mut h, hero, potion, far), Ok(Value::Bool(false)));
    assert_eq!(life_of(&h, far), far_life);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(potion)]),
        Ok(Value::Bool(true))
    );
}

/// Registers a player-unit event trigger for the unit's owner; `event` is the script enumerand.
fn register_player_event(h: &mut ObjectHost, unit: HandleId, event: i32) -> HandleId {
    let Value::Handle(trigger) = h.native("CreateTrigger", &[]).unwrap() else {
        panic!("trigger handle")
    };
    let owner = Value::Handle(h.executed_unit(unit).unwrap().owner);
    let event = h
        .native("ConvertPlayerUnitEvent", &[Value::Int(event)])
        .unwrap();
    h.native(
        "TriggerRegisterPlayerUnitEvent",
        &[Value::Handle(trigger), owner, event, Value::Null],
    )
    .unwrap();
    trigger
}

#[test]
fn a_successful_use_queues_one_use_event_and_a_refused_use_queues_none() {
    let Fixture { mut h, hero, .. } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let trigger = register_player_event(&mut h, hero, 50);
    let potion = item(&mut h, b"phcd", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    assert_eq!(use_item(&mut h, hero, potion), Ok(Value::Bool(true)));
    let fired = h.take_fired_event().expect("use event queued");
    assert_eq!(fired.trigger, trigger);
    h.begin_fired_event(&fired).unwrap().unwrap();
    assert_eq!(h.native("GetManipulatedItem", &[]), Ok(Value::Handle(potion)));
    assert_eq!(h.native("GetManipulatingUnit", &[]), Ok(Value::Handle(hero)));
    h.finish_fired_event();
    assert!(h.take_fired_event().is_none());
    // Cooling down: the refused use queues nothing.
    assert_eq!(use_item(&mut h, hero, potion), Ok(Value::Bool(false)));
    assert!(h.take_fired_event().is_none());
}

#[test]
fn player_activation_uses_the_same_path_as_the_native_and_removes_the_potion() {
    let Fixture { mut h, hero, .. } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let potion = item(&mut h, b"phea", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    assert!(h.activate_held_item(hero, potion).unwrap());
    assert_eq!(life_of(&h, hero), max / 2.0 + 250.0);
    assert!(h.executed_items().all(|(id, _)| id != potion));
    // A second activation of a removed potion does nothing.
    assert!(!h.activate_held_item(hero, potion).unwrap());
}

fn owner_number(h: &ObjectHost, unit: HandleId) -> i32 {
    let owner = h.executed_unit(unit).unwrap().owner;
    h.player_settings(owner).unwrap().number
}

#[test]
fn input_by_the_owning_player_heals_and_removes_the_potion() {
    let Fixture { mut h, hero, .. } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let potion = item(&mut h, b"phea", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    let local = owner_number(&h, hero);
    assert!(h.player_activate_held_item(local, hero, potion).unwrap());
    assert_eq!(life_of(&h, hero), max / 2.0 + 250.0);
    assert!(h.executed_items().all(|(id, _)| id != potion));
}

#[test]
fn input_from_another_player_or_for_an_unheld_item_changes_nothing() {
    let Fixture { mut h, hero, ordinary } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let before = life_of(&h, hero);
    let potion = item(&mut h, b"phea", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    let local = owner_number(&h, hero);
    assert!(!h.player_activate_held_item(local + 1, hero, potion).unwrap());
    // The owning player's input for a unit that does not hold the item is refused too.
    assert!(!h.player_activate_held_item(local, ordinary, potion).unwrap());
    assert_eq!(life_of(&h, hero), before);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(potion)]),
        Ok(Value::Bool(true))
    );
}

#[test]
fn input_refuses_a_targeted_item_without_any_write() {
    let Fixture { mut h, hero, .. } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let before = life_of(&h, hero);
    let targeted = item(&mut h, b"phtg", 0.0);
    assert_eq!(add(&mut h, hero, targeted), Ok(Value::Bool(true)));
    let local = owner_number(&h, hero);
    let err = h.player_activate_held_item(local, hero, targeted).unwrap_err();
    assert!(format!("{err:?}").contains("target"), "{err:?}");
    assert_eq!(life_of(&h, hero), before);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(targeted)]),
        Ok(Value::Bool(true))
    );
}

#[test]
fn use_event_callback_observes_the_healed_life_and_the_charge_already_spent() {
    let Fixture { mut h, hero, .. } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let trigger = register_player_event(&mut h, hero, 50);
    let potion = item(&mut h, b"phcd", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    let local = owner_number(&h, hero);
    assert!(h.player_activate_held_item(local, hero, potion).unwrap());
    let fired = h.take_fired_event().expect("use event queued");
    assert_eq!(fired.trigger, trigger);
    // The trigger runs from the queue: the heal and the spent charge are already in place.
    h.begin_fired_event(&fired).unwrap().unwrap();
    assert_eq!(life_of(&h, hero), max / 2.0 + 250.0);
    assert_eq!(
        h.native("GetItemCharges", &[Value::Handle(potion)]),
        Ok(Value::Int(1))
    );
    assert_eq!(h.native("GetManipulatedItem", &[]), Ok(Value::Handle(potion)));
    h.finish_fired_event();
}

#[test]
fn slot_input_activates_the_item_in_that_slot_and_nothing_else() {
    let Fixture { mut h, hero, .. } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let potion = item(&mut h, b"phea", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    let local = owner_number(&h, hero);
    // The first item lands in slot 0; slot 1 is empty and slot 9 is beyond the unit's capacity.
    assert!(!h.player_activate_slot(local, hero, 1).unwrap());
    assert!(!h.player_activate_slot(local, hero, 9).unwrap());
    assert_eq!(life_of(&h, hero), max / 2.0);
    assert!(h.player_activate_slot(local, hero, 0).unwrap());
    assert_eq!(life_of(&h, hero), max / 2.0 + 250.0);
    assert!(h.executed_items().all(|(id, _)| id != potion));
}

#[test]
fn slot_input_from_another_player_changes_nothing() {
    let Fixture { mut h, hero, .. } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let potion = item(&mut h, b"phea", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    let local = owner_number(&h, hero);
    assert!(!h.player_activate_slot(local + 1, hero, 0).unwrap());
    assert_eq!(life_of(&h, hero), max / 2.0);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(potion)]),
        Ok(Value::Bool(true))
    );
}
