//! Automatic powerup integration: grant, queued pickup delivery, consumption, levels and refusals.
//! Built on the combat hero fixture with the selected inventory registry and synthetic tome rows.
use super::*;
use crate::inventory::{InventoryBudget, Rules as InventoryRules};
use crate::inventory_powerup::Rules as TomeRules;
use crate::playercolor_host::{
    ExecutedItem, ExecutedUnit, HeroBudget, ItemBudget, UnitStateBudget,
};
use crate::scripts::AssetIdentity;
use pcx_objects::editor::defaults::Identity;
use pcx_objects::editor::{EditorReadLimits, Overlay};
use pcx_sim::world::ResolutionProblem;

/// Source-shaped rows: the tome grants 7 STR through `DataC1`, one slot count through `L000`.
const POWERUP_DECLARATIONS: &str = r#"
native UnitResetCooldown takes unit u returns nothing
type gamecache extends agent
native InitGameCache takes string filename returns gamecache
native SaveGameCache takes gamecache cache returns boolean
native StoreUnit takes gamecache cache, string mission, string key, unit u returns boolean
native RestoreUnit takes gamecache cache, string mission, string key, player p, real x, real y, real facing returns unit
native HaveStoredUnit takes gamecache cache, string mission, string key returns boolean
native FlushStoredUnit takes gamecache cache, string mission, string key returns nothing
native StoreInteger takes gamecache cache, string mission, string key, integer value returns nothing
native GetStoredInteger takes gamecache cache, string mission, string key returns integer
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
native GetHeroStr takes unit whichHero, boolean includeBonuses returns integer
native GetHeroAgi takes unit whichHero, boolean includeBonuses returns integer
native GetHeroInt takes unit whichHero, boolean includeBonuses returns integer
native PauseUnit takes unit u, boolean paused returns nothing
native RemoveUnit takes unit u returns nothing
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
            &["pbig", "AIsh", "0", "0", "0", "-"],
            &["pmin", "AImn", "0", "0", "0", "-"],
            &["pint", "AIi4", "0", "0", "0", "-"],
            &["pivi", "AIi9", "0", "0", "0", "-"],
            &["pbad", "AIsm", "0", "0", "0", "-"],
            &["uact", "AIsm", "0", "1", "1", "1"],
            &["parm", "AId1", "0", "0", "0", "-"],
            &["parx", "AId8", "0", "0", "0", "-"],
            &["phea", "AIh1", "0", "1", "1", "1"],
            &["phcd", "AIh1", "0", "1", "1", "2"],
            &["phsm", "AIh2", "0", "1", "1", "1"],
            &["phtg", "AIh3", "0", "1", "1", "1"],
            &["pdm1", "AItf", "0", "0", "0", "-"],
            &["pdm2", "AIat", "0", "0", "0", "-"],
            &["pdmx", "AItz", "0", "0", "0", "-"],
            &["pdmv", "AItf,AIwz", "0", "0", "0", "-"],
            &["pdmm", "AIwz", "0", "0", "0", "-"],
            &["pdmo", "AIt1", "0", "0", "0", "-"],
            &["preg", "Arel", "0", "0", "0", "-"],
            &["pman", "AIrm", "0", "0", "0", "-"],
            &["pmax", "AImx", "0", "0", "0", "-"],
            &["plif", "AIl1", "0", "0", "0", "-"],
            &["plsm", "AIl2", "0", "0", "0", "-"],
            &["plmx", "AIl3", "0", "0", "0", "-"],
            &["plon", "AIl4", "0", "0", "0", "-"],
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
            &[
                "AIsh", "AIab", "1", "0", "0", "1000", "0", "0", "-", "-", "_",
            ],
            &[
                "AImn",
                "AIab",
                "1",
                "0",
                "0",
                "-2147483648",
                "0",
                "0",
                "-",
                "-",
                "_",
            ],
            &["AIa4", "AIab", "1", "2", "0", "0", "0", "0", "-", "-", "_"],
            &["AIi4", "AIab", "1", "0", "3", "0", "0", "0", "-", "-", "_"],
            &[
                "AIi9",
                "AIab",
                "1",
                "0",
                "2147483647",
                "0",
                "0",
                "0",
                "-",
                "-",
                "_",
            ],
            &["AId1", "AIde", "1", "1", "0", "0", "0", "0", "-", "-", "_"],
            &["AId8", "AIde", "1", "8", "0", "0", "0", "0", "-", "-", "_"],
            &[
                "AIh1", "AIhe", "1", "250", "0", "0", "0", "0", "100", "20", "_",
            ],
            &[
                "AIh2", "AIhe", "1", "250", "0", "0", "0", "0", "10", "20", "_",
            ],
            &[
                "AIh3",
                "AIhe",
                "1",
                "250",
                "0",
                "0",
                "0",
                "0",
                "100",
                "20",
                "ground,air",
            ],
            &["AItf", "AIat", "1", "15", "0", "0", "0", "0", "-", "-", "_"],
            &["AIat", "AIat", "1", "20", "0", "0", "0", "0", "-", "-", "_"],
            &["AItz", "AIat", "1", "3", "0", "0", "0", "0", "-", "-", "_"],
            &["AIt1", "AIat", "1", "1", "0", "0", "0", "0", "-", "-", "_"],
            &["Arel", "Arel", "1", "7", "0", "0", "0", "0", "-", "-", "_"],
            &[
                "AIrm", "AIrm", "1", "0.5", "0", "0", "0", "0", "-", "-", "_",
            ],
            &[
                "AImx", "AIrm", "1", "3.4e38", "0", "0", "0", "0", "-", "-", "_",
            ],
            &[
                "AIwz",
                "AIat",
                "1",
                "2147483647",
                "0",
                "0",
                "0",
                "0",
                "-",
                "-",
                "_",
            ],
            &["AIl1", "AIml", "1", "37", "0", "0", "0", "0", "-", "-", "_"],
            &["AIl2", "AIml", "1", "7", "0", "0", "0", "0", "-", "-", "_"],
            &[
                "AIl3",
                "AIml",
                "1",
                "2147483647",
                "0",
                "0",
                "0",
                "0",
                "-",
                "-",
                "_",
            ],
            &["AIl4", "AIml", "1", "1", "0", "0", "0", "0", "-", "-", "_"],
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
                "AIsm,AIxm,AIs4,AIa4,AIi4,AIi9,AIsh,AImn",
                "",
            ],
            &[
                "Iint",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "2",
                "AIsm,AIxm,AIs4,AIa4,AIi4,AIi9,AIsh,AImn",
                "",
            ],
            &[
                "Istr",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "3",
                "AIsm,AIxm,AIs4,AIa4,AIi4,AIi9,AIsh,AImn",
                "",
            ],
            &["Idef", "Data", "AbilityData", "-1", "int", "1", "AId1", ""],
            &["Ihpr", "Data", "AbilityData", "-1", "int", "1", "Arel", ""],
            &[
                "Imrp",
                "Data",
                "AbilityData",
                "-1",
                "real",
                "1",
                "AIrm,AImx",
                "",
            ],
            &[
                "Iatt",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "1",
                "AIat,AItf,AIwz,AIt1",
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
            &[
                "Ilif",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "1",
                "AIl1,AIl2,AIl3,AIl4,AIl5",
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
    use crate::inventory_powerup::{
        CALL_REL32, CLEANUP_RET, HEAL_WINDOW, PUSH_EAX, PUSH_IMM32, REGISTER_FN,
    };
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
        let mut rows = vec![
            (*b"ipow", "powerup", powerup),
            (*b"iuse", "uses", "-"),
            (*b"idro", "drop", "0"),
        ];
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
            b"pman" => rows.push((*b"iabi", "abilList", "AIrm")),
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
    open_with_strength_regen(0.)
}

fn open_with_strength_regen(strength_regen: f32) -> Fixture {
    open_with_regens(strength_regen, None, 0.0)
}

fn open_with_regens(
    strength_regen: f32,
    mana_regen: Option<&str>,
    mana_per_intelligence: f32,
) -> Fixture {
    open_with_regens_overlay(
        Overlay::default(),
        strength_regen,
        mana_regen,
        mana_per_intelligence,
    )
}

fn open_with_regens_overlay(
    overlay: Overlay,
    strength_regen: f32,
    mana_regen: Option<&str>,
    mana_per_intelligence: f32,
) -> Fixture {
    open_with_regens_overlays(
        overlay.clone(),
        overlay,
        strength_regen,
        mana_regen,
        mana_per_intelligence,
    )
}

fn open_with_regens_overlays(
    inventory_overlay: Overlay,
    hero_overlay: Overlay,
    strength_regen: f32,
    mana_regen: Option<&str>,
    mana_per_intelligence: f32,
) -> Fixture {
    let (hero_rules, combat) = hero_rules();
    // `uabi` supplies the one ability list; the hero fixture's own `abilList` would be ambiguous.
    let mut fields: Vec<_> = hero_fields("STR")
        .into_iter()
        .filter(|(id, _)| *id != *b"A008")
        .collect();
    if fields.iter().all(|(id, _)| *id != *b"S001") {
        fields.push(column(
            b"S001",
            SlkId::UnitBalance,
            "spd",
            FieldKind::Real,
            "32",
        ));
    }
    if let Some(rate) = mana_regen {
        let field = fields
            .iter_mut()
            .find(|(id, _)| *id == *b"H007")
            .expect("hero fixture regenMana field");
        field.1.value = Ok(FieldValue::Cell(rate.into()));
    }
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
        inventory_overlay.clone(),
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
        inventory_overlay.clone(),
        origin.clone(),
        [0; 32],
        [
            &registration("AIsm", "AIxm"),
            &registration("AIab", "APbo"),
            &registration("AIde", "APbo"),
            &registration("AIat", "APbo"),
        ],
    )
    .unwrap()
    .with_heal(&healing_registration(), &healing_base())
    .unwrap()
    .with_life_regen(&registration("Arel", "AREB"))
    .unwrap()
    .with_mana_regen(&registration("AIrm", "Arem"))
    .unwrap()
    .with_max_life(&registration("AIml", "APbo"))
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
            hero_overlay,
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
    h.admit_unit_activity(activity, 65536).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: LIFE_PER_STRENGTH,
        mana_per_intelligence: 15.0,
        life_regen_per_strength: strength_regen,
        mana_regen_per_intelligence: mana_per_intelligence,
    })
    .unwrap();
    let mut h = h
        .with_inventory_powerup(tome)
        .unwrap()
        .with_item_use(
            crate::item_use_rules::Rules::new(powerup_tables(), inventory_overlay.clone()),
            8,
        )
        .unwrap();
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_combat(combat, "alpha").unwrap();
    h.set_neutral_passive(15);
    let hero = create_unit(&mut h, b"H001", 0.0);
    let ordinary = create_unit(&mut h, b"u001", 0.0);
    Fixture { h, hero, ordinary }
}

fn create_unit(h: &mut ObjectHost, raw: &[u8; 4], x: f32) -> HandleId {
    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    let Value::Handle(id) = h
        .native(
            "CreateUnit",
            &[
                player,
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

/// Registers a player item event for the unit's owner and returns the trigger handle.
fn register_item_event(h: &mut ObjectHost, unit: HandleId, enumerand: i32) -> HandleId {
    let Value::Handle(trigger) = h.native("CreateTrigger", &[]).unwrap() else {
        panic!("trigger handle")
    };
    let owner = Value::Handle(h.executed_unit(unit).unwrap().owner);
    let event = h
        .native("ConvertPlayerUnitEvent", &[Value::Int(enumerand)])
        .unwrap();
    h.native(
        "TriggerRegisterPlayerUnitEvent",
        &[Value::Handle(trigger), owner, event, Value::Null],
    )
    .unwrap();
    trigger
}

/// Registers a player pickup trigger for the unit's owner and returns the trigger handle.
fn register_pickup(h: &mut ObjectHost, unit: HandleId) -> HandleId {
    register_item_event(h, unit, -71)
}

/// Registers a player drop trigger for the unit's owner and returns the trigger handle.
fn register_drop(h: &mut ObjectHost, unit: HandleId) -> HandleId {
    register_item_event(h, unit, 99)
}

fn issue_pending_pickup(h: &mut ObjectHost, unit: HandleId, ground: HandleId) -> i32 {
    assert_eq!(
        h.native(
            "IssueTargetOrder",
            &[
                Value::Handle(unit),
                Value::Str("smart".into()),
                Value::Handle(ground)
            ]
        ),
        Ok(Value::Bool(true))
    );
    h.inventory_order(unit).expect("pending pickup order")
}

fn life_state(h: &mut ObjectHost) -> Value {
    h.native("ConvertUnitState", &[Value::Int(LIFE_STATE)])
        .unwrap()
}

fn damage(h: &ObjectHost, unit: HandleId) -> f32 {
    h.hero_combat_bonuses(unit).unwrap().damage
}

fn held_damage(h: &ObjectHost, unit: HandleId) -> i32 {
    h.hero_combat_bonuses(unit).unwrap().held_damage as i32
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
        (463.0, max)
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
fn powerup_overflows_with_held_strength_before_any_grant() {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "AIsm".into(),
        vec![pcx_objects::Mod {
            field: *b"Istr",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([1, 3]),
            value: pcx_objects::Value::Int((i32::MAX - 10) as u32),
            end_word: 0,
        }],
    );
    let Fixture { mut h, hero, .. } = open_with_regens_overlay(overlay, 0., None, 0.);
    let held = item(&mut h, b"pbig", 0.0);
    assert_eq!(add(&mut h, hero, held), Ok(Value::Bool(true)));
    let tome = item(&mut h, b"tstr", 0.0);
    let before = snapshot_records(&h, &[hero]);
    let err = add(&mut h, hero, tome).unwrap_err();
    assert!(
        err.contains("powerup strength overflows combined hero bonus"),
        "{err}"
    );
    assert_records_unchanged(&h, &[hero], &before);
    assert_eq!(h.held_slot(hero, 0), Some(held));
    assert_eq!(h.held_slot(hero, 1), None);
    assert!(!h.executed_item(tome).unwrap().claimed);
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
    assert_eq!(strength_and_agility(&mut h, hero), (438.0, max));
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
    assert_eq!(strength_and_agility(&mut h, hero), before);
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(hero), Value::Handle(mismatched)]
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
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(armor)],
    )
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
fn held_max_life_stacks_on_a_hero_and_rounds_the_retained_ratio() {
    let Fixture { mut h, hero, .. } = open();
    set_life(&mut h, hero, HERO_LIFE / 2.0);
    let mana = (mana_of(&h, hero), max_mana_of(&h, hero));
    let first = item(&mut h, b"plif", 0.0);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    assert_eq!(max_life_of(&h, hero), HERO_LIFE + 37.0);
    assert_eq!(life_of(&h, hero), 394.0);
    let second = item(&mut h, b"plsm", 0.0);
    assert_eq!(add(&mut h, hero, second), Ok(Value::Bool(true)));
    assert_eq!(max_life_of(&h, hero), HERO_LIFE + 44.0);
    assert_eq!(life_of(&h, hero), 398.0);
    assert_eq!(
        (mana_of(&h, hero), max_mana_of(&h, hero)),
        mana,
        "life-only items leave both mana pools"
    );
}

#[test]
fn held_max_life_applies_to_a_non_hero_and_transfers_the_pool() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    let hero_max = max_life_of(&h, hero);
    let ordinary_max = max_life_of(&h, ordinary);
    let item = item(&mut h, b"plif", 0.0);
    assert_eq!(add(&mut h, hero, item), Ok(Value::Bool(true)));
    assert_eq!(max_life_of(&h, hero), hero_max + 37.0);
    assert_eq!(max_life_of(&h, ordinary), ordinary_max);
    assert_eq!(add(&mut h, ordinary, item), Ok(Value::Bool(true)));
    assert_eq!(max_life_of(&h, hero), hero_max);
    assert_eq!(max_life_of(&h, ordinary), ordinary_max + 37.0);
}

#[test]
fn dropping_held_max_life_restores_the_pool_and_rounds_the_ratio() {
    let Fixture { mut h, hero, .. } = open();
    set_life(&mut h, hero, HERO_LIFE / 2.0);
    let item = item(&mut h, b"plif", 0.0);
    assert_eq!(add(&mut h, hero, item), Ok(Value::Bool(true)));
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(item)],
    )
    .unwrap();
    assert_eq!(max_life_of(&h, hero), HERO_LIFE);
    assert_eq!(life_of(&h, hero), HERO_LIFE / 2.0);
}

#[test]
fn cleanup_drops_held_max_life_and_restores_the_pool() {
    let Fixture { mut h, hero, .. } = open();
    set_life(&mut h, hero, HERO_LIFE / 2.0);
    let item = item(&mut h, b"plif", 0.0);
    assert_eq!(add(&mut h, hero, item), Ok(Value::Bool(true)));
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(max_life_of(&h, hero), HERO_LIFE);
    assert_eq!(life_of(&h, hero), HERO_LIFE / 2.0);
    assert_eq!(h.held_slot(hero, 0), None);
}

#[test]
fn held_max_life_total_overflow_refuses_transfer_without_writes() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    let donor = item(&mut h, b"plmx", 0.);
    let receiver = item(&mut h, b"plon", 0.);
    assert_eq!(add(&mut h, hero, donor), Ok(Value::Bool(true)));
    assert_eq!(add(&mut h, ordinary, receiver), Ok(Value::Bool(true)));
    let before = snapshot_records(&h, &[hero, ordinary]);
    let _pickup = register_pickup(&mut h, ordinary);
    let _drop = register_drop(&mut h, hero);
    assert!(
        add(&mut h, ordinary, donor)
            .unwrap_err()
            .contains("held passive bonus overflows")
    );
    assert_records_unchanged(&h, &[hero, ordinary], &before);
    assert_eq!(h.held_slot(hero, 0), Some(donor));
    assert_eq!(h.held_slot(ordinary, 0), Some(receiver));
    assert!(h.take_fired_event().is_none());
}

#[test]
fn held_max_life_on_a_typed_override_uses_the_override_amount() {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "AIl1".into(),
        vec![pcx_objects::Mod {
            field: *b"Ilif",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([1, 1]),
            value: pcx_objects::Value::Int(41),
            end_word: 0,
        }],
    );
    let Fixture { mut h, hero, .. } = open_with_regens_overlay(overlay, 0., None, 0.);
    let item = item(&mut h, b"plif", 0.0);
    assert_eq!(add(&mut h, hero, item), Ok(Value::Bool(true)));
    assert_eq!(max_life_of(&h, hero), HERO_LIFE + 41.0);
}

#[test]
fn held_max_life_keeps_a_living_holder_at_one_life() {
    let Fixture { mut h, hero, .. } = open();
    set_life(&mut h, hero, 0.1);
    let item = item(&mut h, b"plif", 0.0);
    assert_eq!(add(&mut h, hero, item), Ok(Value::Bool(true)));
    assert_eq!(max_life_of(&h, hero), HERO_LIFE + 37.0);
    assert_eq!(life_of(&h, hero), 1.0);
}

#[test]
fn tiny_living_hero_keeps_one_life_after_a_strength_powerup() {
    let Fixture { mut h, hero, .. } = open();
    set_life(&mut h, hero, 0.1);
    let tome = item(&mut h, b"tstr", 0.);
    assert_eq!(add(&mut h, hero, tome), Ok(Value::Bool(true)));
    assert_eq!(life_of(&h, hero), 1.0);
    assert_eq!(
        max_life_of(&h, hero),
        HERO_LIFE + TOME_STRENGTH * LIFE_PER_STRENGTH
    );
}

#[test]
fn non_hero_near_zero_drop_keeps_one_life_and_restores_maximum() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let ordinary_max = max_life_of(&h, ordinary);
    set_life(&mut h, ordinary, 0.1);
    let item = item(&mut h, b"plif", 0.0);
    assert_eq!(add(&mut h, ordinary, item), Ok(Value::Bool(true)));
    assert_eq!(max_life_of(&h, ordinary), ordinary_max + 37.0);
    assert_eq!(life_of(&h, ordinary), 1.0);
    set_life(&mut h, ordinary, 0.1);
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(ordinary), Value::Handle(item)],
    )
    .unwrap();
    assert_eq!(max_life_of(&h, ordinary), ordinary_max);
    assert_eq!(life_of(&h, ordinary), 1.0);
}

#[test]
fn dead_retained_max_life_drop_keeps_zero_life_and_restores_maximum() {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L000".into(),
        vec![pcx_objects::Mod {
            field: *b"inv2",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([1, 2]),
            value: pcx_objects::Value::Int(0),
            end_word: 0,
        }],
    );
    let Fixture { mut h, hero, .. } =
        open_with_regens_overlays(overlay, Overlay::default(), 0., None, 0.);
    let item = item(&mut h, b"plif", 0.0);
    assert_eq!(add(&mut h, hero, item), Ok(Value::Bool(true)));
    let held_max = max_life_of(&h, hero);
    h.kill_unit(hero, None).unwrap();
    assert_eq!(h.held_slot(hero, 0), Some(item));
    assert_eq!(max_life_of(&h, hero), held_max);
    assert_eq!(life_of(&h, hero), 0.0);
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(item)],
    )
    .unwrap();
    assert_eq!(max_life_of(&h, hero), held_max - 37.0);
    assert_eq!(life_of(&h, hero), 0.0);
}

#[test]
fn dead_unit_cannot_pick_up_max_life() {
    let Fixture { mut h, hero, .. } = open();
    let before = max_life_of(&h, hero);
    h.kill_unit(hero, None).unwrap();
    let item = item(&mut h, b"plif", 0.0);
    assert_eq!(add(&mut h, hero, item), Ok(Value::Bool(false)));
    assert_eq!(max_life_of(&h, hero), before);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(item)]),
        Ok(Value::Bool(false))
    );
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
    assert!(
        err.contains("armor metadata does not admit ability AId8"),
        "{err}"
    );
    assert_eq!(h.held_armor_bonus(hero).unwrap(), 0);
    assert_eq!(strength_and_agility(&mut h, hero), before);
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(hero), Value::Handle(refused)]
        ),
        Ok(Value::Bool(false))
    );
}

#[test]
fn held_flat_damage_stacks_transfers_and_removes() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    let primary = damage(&h, hero);
    assert_eq!(held_damage(&h, hero), 0);
    let first = item(&mut h, b"pdm1", 0.0);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    assert_eq!(held_damage(&h, hero), 15);
    // Damage is not a strength or agility change.
    assert_eq!(damage(&h, hero), primary);
    let second = item(&mut h, b"pdm2", 0.0);
    assert_eq!(add(&mut h, hero, second), Ok(Value::Bool(true)));
    assert_eq!(held_damage(&h, hero), 35);
    assert_eq!(add(&mut h, ordinary, first), Ok(Value::Bool(true)));
    assert_eq!(held_damage(&h, hero), 20);
    assert_eq!(held_damage(&h, ordinary), 15);
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(ordinary), Value::Handle(first)],
    )
    .unwrap();
    assert_eq!(held_damage(&h, ordinary), 0);
    assert_eq!(held_damage(&h, hero), 20);
}

#[test]
fn death_drops_held_flat_damage() {
    let Fixture { mut h, hero, .. } = open();
    let item = item(&mut h, b"pdm1", 0.0);
    assert_eq!(add(&mut h, hero, item), Ok(Value::Bool(true)));
    assert_eq!(held_damage(&h, hero), 15);
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(held_damage(&h, hero), 0);
}

#[test]
fn damage_row_outside_admission_and_overflow_refuse_atomically() {
    let Fixture { mut h, hero, .. } = open();
    let refused = item(&mut h, b"pdmx", 0.0);
    let err = add(&mut h, hero, refused).unwrap_err();
    assert!(
        err.contains("damage metadata does not admit ability AItz"),
        "{err}"
    );
    assert_eq!(held_damage(&h, hero), 0);
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(hero), Value::Handle(refused)]
        ),
        Ok(Value::Bool(false))
    );
    let overflow = item(&mut h, b"pdmv", 0.0);
    let err = add(&mut h, hero, overflow).unwrap_err();
    assert!(err.contains("passive damage overflows"), "{err}");
    assert_eq!(held_damage(&h, hero), 0);
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(hero), Value::Handle(overflow)]
        ),
        Ok(Value::Bool(false))
    );
}

#[test]
fn combined_held_damage_overflow_refuses_pickup_and_preserves_transfer_holder() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    let maximum = item(&mut h, b"pdmm", 0.0);
    let one = item(&mut h, b"pdmo", 0.0);
    assert_eq!(add(&mut h, hero, maximum), Ok(Value::Bool(true)));
    assert_eq!(h.unit_hero_bonus(hero).unwrap().damage, i32::MAX);
    let before: Vec<_> = h
        .executed_items()
        .map(|(id, record)| (id, record.clone()))
        .collect();
    let vitals = h.executed_unit(hero).unwrap().vitals;
    assert!(
        add(&mut h, hero, one)
            .unwrap_err()
            .contains("held passive bonus overflows")
    );
    assert_eq!(
        h.executed_items()
            .map(|(id, record)| (id, record.clone()))
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(h.executed_unit(hero).unwrap().vitals, vitals);
    assert_eq!(add(&mut h, ordinary, one), Ok(Value::Bool(true)));
    let before: Vec<_> = h
        .executed_items()
        .map(|(id, record)| (id, record.clone()))
        .collect();
    assert!(
        add(&mut h, hero, one)
            .unwrap_err()
            .contains("held passive bonus overflows")
    );
    assert_eq!(
        h.executed_items()
            .map(|(id, record)| (id, record.clone()))
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(h.unit_hero_bonus(hero).unwrap().damage, i32::MAX);
    assert_eq!(h.unit_hero_bonus(ordinary).unwrap().damage, 1);
    assert_eq!(h.executed_unit(hero).unwrap().vitals, vitals);
}

/// Held flat damage enters the real damage formula for heroes and non-heroes, and its transfer
/// moves the contribution to the receiver.
#[test]
fn held_flat_damage_changes_real_damage() {
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
    let factor = h.roll_damage(ordinary, weapon, ordinary).unwrap() / 10.0;
    let hero_bare = h.roll_damage(hero, weapon, ordinary).unwrap();
    let first = item(&mut h, b"pdm1", 0.0);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    let hero_with = h.roll_damage(hero, weapon, ordinary).unwrap();
    assert!(
        (hero_with - hero_bare - 15.0 * factor).abs() < 1e-4,
        "hero: {hero_with} vs {hero_bare} at factor {factor}"
    );
    assert_eq!(add(&mut h, ordinary, first), Ok(Value::Bool(true)));
    let ordinary_with = h.roll_damage(ordinary, weapon, ordinary).unwrap();
    assert!(
        (ordinary_with - 25.0 * factor).abs() < 1e-4,
        "non-hero: {ordinary_with} at factor {factor}"
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
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(ordinary), Value::Handle(armor)],
    )
    .unwrap();
    assert_eq!(h.roll_damage(hero, weapon, ordinary).unwrap(), bare);
}

fn use_item(h: &mut ObjectHost, unit: HandleId, potion: HandleId) -> Result<Value, String> {
    h.native("UnitUseItem", &[Value::Handle(unit), Value::Handle(potion)])
}

fn use_item_on(
    h: &mut ObjectHost,
    unit: HandleId,
    potion: HandleId,
    target: HandleId,
) -> Result<Value, String> {
    h.native(
        "UnitUseItemTarget",
        &[
            Value::Handle(unit),
            Value::Handle(potion),
            Value::Handle(target),
        ],
    )
}

fn set_life(h: &mut ObjectHost, unit: HandleId, life: f32) {
    let state = life_state(h);
    h.native(
        "SetUnitState",
        &[Value::Handle(unit), state, Value::Real(life)],
    )
    .unwrap();
}

fn set_fixture_max_life(h: &mut ObjectHost, unit: HandleId, maximum: f32) {
    h.set_test_maximum(unit, Some(maximum), None);
}

fn life_of(h: &ObjectHost, unit: HandleId) -> f32 {
    h.unit_vitals(unit).unwrap().unwrap().life
}

fn max_life_of(h: &ObjectHost, unit: HandleId) -> f32 {
    h.unit_vitals(unit).unwrap().unwrap().max_life
}

fn prepare_passive_regen(h: &mut ObjectHost, units: &[HandleId]) {
    for &unit in units {
        set_life(h, unit, 100.);
        assert_eq!(
            h.unit_vitals(unit).unwrap().unwrap().regen.life_per_second,
            0.
        );
    }
}

fn advance_passive_regen(h: &mut ObjectHost, now: f64) {
    h.next_timer_expiry(now, &mut 1000).unwrap();
    h.advance_unit_activity(now, &mut 65536).unwrap();
}

const MANA_STATE: i32 = 2;

fn set_state(h: &mut ObjectHost, unit: HandleId, state: i32, value: f32) {
    let state = h.native("ConvertUnitState", &[Value::Int(state)]).unwrap();
    h.native(
        "SetUnitState",
        &[Value::Handle(unit), state, Value::Real(value)],
    )
    .unwrap();
}

fn set_mana(h: &mut ObjectHost, unit: HandleId, mana: f32) {
    set_state(h, unit, MANA_STATE, mana);
}

fn set_fixture_max_mana(h: &mut ObjectHost, unit: HandleId, maximum: f32) {
    h.set_test_maximum(unit, None, Some(maximum));
}

fn mana_of(h: &ObjectHost, unit: HandleId) -> f32 {
    h.unit_vitals(unit).unwrap().unwrap().mana
}

fn max_mana_of(h: &ObjectHost, unit: HandleId) -> f32 {
    h.unit_vitals(unit).unwrap().unwrap().max_mana
}

type RecordSnapshot = (Vec<(HandleId, ExecutedUnit)>, Vec<(HandleId, ExecutedItem)>);

fn snapshot_records(h: &ObjectHost, units: &[HandleId]) -> RecordSnapshot {
    (
        units
            .iter()
            .map(|&unit| (unit, h.executed_unit(unit).unwrap().clone()))
            .collect(),
        h.executed_items()
            .map(|(id, record)| (id, record.clone()))
            .collect(),
    )
}

fn assert_records_unchanged(h: &ObjectHost, units: &[HandleId], before: &RecordSnapshot) {
    assert_eq!(&snapshot_records(h, units), before);
}

fn assert_close(actual: f32, expected: f32, context: &str) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "{context}: {actual} != {expected}"
    );
}

#[test]
fn held_intelligence_adds_mana_pool_and_is_removed_with_ratio_rounding() {
    let Fixture { mut h, hero, .. } = open();
    let base_max = max_mana_of(&h, hero);
    assert_close(base_max, 7.0 * 15.0, "base intelligence pool");
    assert_eq!(hero_attribute(&mut h, "GetHeroInt", hero, true), Ok(7));
    set_mana(&mut h, hero, base_max / 2.0);
    let intelligence = item(&mut h, b"pint", 0.0);
    assert_eq!(add(&mut h, hero, intelligence), Ok(Value::Bool(true)));
    assert_eq!(hero_attribute(&mut h, "GetHeroInt", hero, false), Ok(7));
    assert_eq!(hero_attribute(&mut h, "GetHeroInt", hero, true), Ok(10));
    assert_close(max_mana_of(&h, hero), base_max + 3.0 * 15.0, "held maximum");
    assert_close(
        mana_of(&h, hero),
        base_max * 0.5 + 3.0 * 15.0 * 0.5,
        "held current",
    );
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(intelligence)],
    )
    .unwrap();
    assert_eq!(hero_attribute(&mut h, "GetHeroInt", hero, true), Ok(7));
    assert_close(max_mana_of(&h, hero), base_max, "dropped maximum");
    assert_close(mana_of(&h, hero), 53.0, "dropped ratio rounds half upward");
}

#[test]
fn pure_intelligence_equip_and_drop_leave_tiny_positive_life_untouched() {
    let Fixture { mut h, hero, .. } = open();
    set_life(&mut h, hero, 0.5);
    let life = (life_of(&h, hero), max_life_of(&h, hero));
    let base_max = max_mana_of(&h, hero);
    set_mana(&mut h, hero, base_max / 2.0);
    let intelligence = item(&mut h, b"pint", 0.0);
    assert_eq!(add(&mut h, hero, intelligence), Ok(Value::Bool(true)));
    assert_eq!(
        (life_of(&h, hero), max_life_of(&h, hero)),
        life,
        "equip leaves both life pools"
    );
    assert_close(mana_of(&h, hero), 75.0, "equip reprices current mana");
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(intelligence)],
    )
    .unwrap();
    assert_eq!(
        (life_of(&h, hero), max_life_of(&h, hero)),
        life,
        "drop leaves both life pools"
    );
    assert_close(mana_of(&h, hero), 53.0, "drop reprices current mana");
}

#[test]
fn pure_intelligence_cleanup_keeps_tiny_life_and_matches_sequential_mana() {
    let Fixture { mut h, hero, .. } = open();
    set_life(&mut h, hero, 0.5);
    let life = (life_of(&h, hero), max_life_of(&h, hero));
    let base_max = max_mana_of(&h, hero);
    set_mana(&mut h, hero, base_max / 2.0);
    let first = item(&mut h, b"pint", 0.0);
    let second = item(&mut h, b"pint", 0.0);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    assert_eq!(add(&mut h, hero, second), Ok(Value::Bool(true)));
    assert_eq!(
        (life_of(&h, hero), max_life_of(&h, hero)),
        life,
        "equip leaves both life pools"
    );
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(h.held_slot(hero, 0), None);
    assert_eq!(h.held_slot(hero, 1), None);
    assert_eq!(
        (life_of(&h, hero), max_life_of(&h, hero)),
        life,
        "cleanup leaves both life pools"
    );
    assert_close(max_mana_of(&h, hero), base_max, "cleanup restores maximum");
    assert_close(mana_of(&h, hero), 53.0, "cleanup sequential ratio");
}

#[test]
fn held_intelligence_stacks_and_cleanup_restores_the_mana_pool() {
    let Fixture { mut h, hero, .. } = open();
    let base_max = max_mana_of(&h, hero);
    set_mana(&mut h, hero, base_max / 2.0);
    let first = item(&mut h, b"pint", 0.0);
    let second = item(&mut h, b"pint", 0.0);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    assert_close(max_mana_of(&h, hero), base_max + 45.0, "one held bonus");
    assert_close(mana_of(&h, hero), 75.0, "one held ratio");
    assert_eq!(add(&mut h, hero, second), Ok(Value::Bool(true)));
    assert_eq!(hero_attribute(&mut h, "GetHeroInt", hero, true), Ok(13));
    assert_close(max_mana_of(&h, hero), base_max + 90.0, "two held bonuses");
    assert_close(mana_of(&h, hero), 98.0, "two held ratio rounds half upward");
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(h.held_slot(hero, 0), None);
    assert_eq!(h.held_slot(hero, 1), None);
    assert_close(max_mana_of(&h, hero), base_max, "cleanup maximum");
    assert_close(mana_of(&h, hero), 53.0, "cleanup sequential ratio");
}

#[test]
fn held_intelligence_transfers_between_heroes_and_reprices_both_pools() {
    let Fixture { mut h, hero, .. } = open();
    let receiver = create_unit(&mut h, b"H001", 64.0);
    let base_max = max_mana_of(&h, hero);
    assert_close(max_mana_of(&h, receiver), base_max, "equal base pools");
    set_mana(&mut h, hero, base_max / 2.0);
    set_mana(&mut h, receiver, base_max / 2.0);
    let intelligence = item(&mut h, b"pint", 0.0);
    assert_eq!(add(&mut h, hero, intelligence), Ok(Value::Bool(true)));
    assert_eq!(add(&mut h, receiver, intelligence), Ok(Value::Bool(true)));
    assert_eq!(h.held_slot(hero, 0), None);
    assert_eq!(h.held_slot(receiver, 0), Some(intelligence));
    assert_close(max_mana_of(&h, hero), base_max, "donor maximum");
    assert_close(mana_of(&h, hero), 53.0, "donor ratio rounds half upward");
    assert_close(
        max_mana_of(&h, receiver),
        base_max + 45.0,
        "receiver maximum",
    );
    assert_close(mana_of(&h, receiver), 75.0, "receiver ratio");
    assert_eq!(hero_attribute(&mut h, "GetHeroInt", hero, true), Ok(7));
    assert_eq!(hero_attribute(&mut h, "GetHeroInt", receiver, true), Ok(10));
}

#[test]
fn held_intelligence_overflow_refuses_without_pool_or_item_changes() {
    let Fixture { mut h, hero, .. } = open();
    let huge = item(&mut h, b"pivi", 0.0);
    assert_eq!(add(&mut h, hero, huge), Ok(Value::Bool(true)));
    let max = max_mana_of(&h, hero);
    let mana = mana_of(&h, hero);
    let item = item(&mut h, b"pint", 0.0);
    let err = add(&mut h, hero, item).unwrap_err();
    assert!(err.contains("held passive bonus overflows"), "{err}");
    assert_eq!(h.held_slot(hero, 0), Some(huge));
    assert_eq!(h.held_slot(hero, 1), None);
    assert_close(max_mana_of(&h, hero), max, "overflow maximum");
    assert_close(mana_of(&h, hero), mana, "overflow current");
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(item)]),
        Ok(Value::Bool(false))
    );
}

#[test]
fn dead_retained_intelligence_drop_rounds_mana_and_keeps_it_nonzero() {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L000".into(),
        vec![pcx_objects::Mod {
            field: *b"inv2",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([1, 2]),
            value: pcx_objects::Value::Int(0),
            end_word: 0,
        }],
    );
    let Fixture { mut h, hero, .. } =
        open_with_regens_overlays(overlay, Overlay::default(), 0., None, 0.);
    let base_max = max_mana_of(&h, hero);
    set_mana(&mut h, hero, base_max / 2.0);
    let intelligence = item(&mut h, b"pint", 0.0);
    assert_eq!(add(&mut h, hero, intelligence), Ok(Value::Bool(true)));
    let held_max = max_mana_of(&h, hero);
    h.kill_unit(hero, None).unwrap();
    assert_eq!(h.held_slot(hero, 0), Some(intelligence));
    assert_eq!(life_of(&h, hero), 0.0);
    assert_close(max_mana_of(&h, hero), held_max, "dead retained maximum");
    assert_close(mana_of(&h, hero), held_max / 2.0, "death keeps mana");
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(intelligence)],
    )
    .unwrap();
    assert_close(max_mana_of(&h, hero), base_max, "dead dropped maximum");
    assert_close(mana_of(&h, hero), 53.0, "dead ratio rounds half upward");
    assert_eq!(life_of(&h, hero), 0.0);
}

#[test]
fn pure_intelligence_on_a_non_hero_is_accepted_without_a_modeled_effect() {
    let Fixture {
        mut h, ordinary, ..
    } = open();
    let before_max = max_mana_of(&h, ordinary);
    let before_mana = mana_of(&h, ordinary);
    let intelligence = item(&mut h, b"pint", 0.0);
    assert_eq!(add(&mut h, ordinary, intelligence), Ok(Value::Bool(true)));
    assert_eq!(h.held_slot(ordinary, 0), Some(intelligence));
    assert_close(max_mana_of(&h, ordinary), before_max, "non-hero maximum");
    assert_close(mana_of(&h, ordinary), before_mana, "non-hero current");
    assert_eq!(hero_attribute(&mut h, "GetHeroInt", ordinary, true), Ok(0));
}

#[test]
fn held_intelligence_changes_refuse_stale_projection_without_moving_items() {
    let Fixture { mut h, hero, .. } = open();
    let intelligence = item(&mut h, b"pint", 0.0);
    h.next_timer_expiry(1., &mut 1000).unwrap();
    assert!(
        add(&mut h, hero, intelligence)
            .unwrap_err()
            .contains("projected through")
    );
    assert_eq!(h.held_slot(hero, 0), None);
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(hero), Value::Handle(intelligence)]
        ),
        Ok(Value::Bool(false))
    );
}

#[test]
fn held_mana_fraction_scales_base_and_intelligence_rate() {
    let Fixture { mut h, hero, .. } = open_with_regens(0., Some("2"), 0.05);
    let base = 2.0 + 7.0 * 0.05;
    set_mana(&mut h, hero, 0.);
    advance_passive_regen(&mut h, 1.);
    assert_close(mana_of(&h, hero), base, "base plus intelligence");
    let first = item(&mut h, b"pman", 0.);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    advance_passive_regen(&mut h, 2.);
    assert_close(mana_of(&h, hero), base * 2.5, "one half fraction");
    let second = item(&mut h, b"pman", 0.);
    assert_eq!(add(&mut h, hero, second), Ok(Value::Bool(true)));
    advance_passive_regen(&mut h, 3.);
    assert_close(mana_of(&h, hero), base * 4.5, "two half fractions");
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(first)],
    )
    .unwrap();
    advance_passive_regen(&mut h, 4.);
    assert_close(mana_of(&h, hero), base * 6.0, "drop removes one fraction");
    assert_eq!(h.unit_hero_bonus(hero).unwrap().mana_regen_fraction, 0.5);
}

#[test]
fn held_intelligence_scales_regeneration_before_the_mana_fraction() {
    let Fixture { mut h, hero, .. } = open_with_regens(0., Some("2"), 0.05);
    set_mana(&mut h, hero, 0.);
    let intelligence = item(&mut h, b"pint", 0.0);
    assert_eq!(add(&mut h, hero, intelligence), Ok(Value::Bool(true)));
    advance_passive_regen(&mut h, 1.);
    assert_close(mana_of(&h, hero), 2.5, "base plus held intelligence");
    let fraction = item(&mut h, b"pman", 0.0);
    assert_eq!(add(&mut h, hero, fraction), Ok(Value::Bool(true)));
    advance_passive_regen(&mut h, 2.);
    assert_close(
        mana_of(&h, hero),
        6.25,
        "held fraction scales held intelligence",
    );
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(intelligence)],
    )
    .unwrap();
    advance_passive_regen(&mut h, 3.);
    assert_close(
        mana_of(&h, hero),
        7.525,
        "drop reprices current before regen",
    );
}

#[test]
fn held_mana_fraction_changes_refuse_stale_projection_without_moving_items() {
    let Fixture { mut h, hero, .. } = open_with_regens(0., Some("2"), 0.0);
    set_mana(&mut h, hero, 0.);
    let mana = item(&mut h, b"pman", 0.);
    h.next_timer_expiry(1., &mut 1000).unwrap();
    assert!(
        add(&mut h, hero, mana)
            .unwrap_err()
            .contains("projected through")
    );
    assert!(
        h.executed_items()
            .find(|(id, _)| *id == mana)
            .unwrap()
            .1
            .held_by
            .is_none()
    );
    h.advance_unit_activity(1., &mut 65536).unwrap();
    add(&mut h, hero, mana).unwrap();
    advance_passive_regen(&mut h, 2.);
    assert_close(mana_of(&h, hero), 5.0, "one and a half times base");
    h.next_timer_expiry(3., &mut 1000).unwrap();
    assert!(
        h.native(
            "UnitRemoveItem",
            &[Value::Handle(hero), Value::Handle(mana)]
        )
        .is_err()
    );
    h.advance_unit_activity(3., &mut 65536).unwrap();
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(mana)],
    )
    .unwrap();
    advance_passive_regen(&mut h, 4.);
    assert_close(mana_of(&h, hero), 10.0, "drop restores the base rate");
}

#[test]
fn held_mana_fraction_cleanup_refuses_stale_projection_and_clears_the_rate() {
    let Fixture { mut h, hero, .. } = open_with_regens(0., Some("2"), 0.0);
    set_mana(&mut h, hero, 0.);
    let mana = item(&mut h, b"pman", 0.);
    add(&mut h, hero, mana).unwrap();
    advance_passive_regen(&mut h, 1.);
    assert_close(mana_of(&h, hero), 3.0, "held fraction");
    h.next_timer_expiry(2., &mut 1000).unwrap();
    assert!(
        h.forget_inventory_unit(hero, false)
            .unwrap_err()
            .to_string()
            .contains("projected through")
    );
    h.advance_unit_activity(2., &mut 65536).unwrap();
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(h.unit_hero_bonus(hero).unwrap().mana_regen_fraction, 0.0);
    advance_passive_regen(&mut h, 3.);
    assert_close(mana_of(&h, hero), 8.0, "cleanup clears the held fraction");
}

#[test]
fn held_mana_fraction_transfer_overflow_refuses_without_changing_either_holder() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open_with_regens(0., Some("2"), 0.);
    let donor = item(&mut h, b"pmax", 0.);
    let receiver = item(&mut h, b"pmax", 0.);
    assert_eq!(add(&mut h, hero, donor), Ok(Value::Bool(true)));
    assert_eq!(add(&mut h, ordinary, receiver), Ok(Value::Bool(true)));
    for held in [donor, receiver] {
        let fraction = h.held_attributes(held).unwrap().mana_regen_fraction;
        assert!(fraction.is_finite() && fraction > 3.0e38, "{fraction}");
    }
    let before = snapshot_records(&h, &[hero, ordinary]);
    let _pickup = register_pickup(&mut h, ordinary);
    let _drop = register_drop(&mut h, hero);
    assert!(
        add(&mut h, ordinary, donor)
            .unwrap_err()
            .contains("held passive bonus overflows")
    );
    assert_records_unchanged(&h, &[hero, ordinary], &before);
    assert_eq!(h.held_slot(hero, 0), Some(donor));
    assert_eq!(h.held_slot(ordinary, 0), Some(receiver));
    assert!(h.take_fired_event().is_none());
}

#[test]
fn held_mana_fraction_transfer_refuses_stale_donor_projection_before_changes() {
    let Fixture { mut h, hero, .. } = open_with_regens(0., Some("2"), 0.);
    let mana = item(&mut h, b"pman", 0.);
    add(&mut h, hero, mana).unwrap();
    h.next_timer_expiry(1., &mut 1000).unwrap();
    let receiver = create_unit(&mut h, b"u001", 1.0);
    assert_eq!(h.unit_vitals(hero).unwrap().unwrap().regen.through, 0.);
    assert_eq!(h.unit_vitals(receiver).unwrap().unwrap().regen.through, 1.);
    let before = snapshot_records(&h, &[hero, receiver]);
    let _pickup = register_pickup(&mut h, receiver);
    let _drop = register_drop(&mut h, hero);
    assert!(
        add(&mut h, receiver, mana)
            .unwrap_err()
            .contains("projected through")
    );
    assert_records_unchanged(&h, &[hero, receiver], &before);
    assert_eq!(h.held_slot(hero, 0), Some(mana));
    assert!(h.take_fired_event().is_none());
}

#[test]
fn mixed_item_cleanup_refuses_stale_mana_fraction_before_any_drop() {
    let Fixture { mut h, hero, .. } = open_with_regens(0., Some("2"), 0.);
    let plain = item(&mut h, b"tsrd", 0.);
    let mana = item(&mut h, b"pman", 0.);
    add(&mut h, hero, plain).unwrap();
    add(&mut h, hero, mana).unwrap();
    let before = snapshot_records(&h, &[hero]);
    let _drop = register_drop(&mut h, hero);
    h.next_timer_expiry(1., &mut 1000).unwrap();
    assert!(
        h.forget_inventory_unit(hero, false)
            .unwrap_err()
            .to_string()
            .contains("projected through")
    );
    assert_records_unchanged(&h, &[hero], &before);
    assert!(h.take_fired_event().is_none());
    h.advance_unit_activity(1., &mut 65536).unwrap();
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(h.held_slot(hero, 0), None);
    assert_eq!(h.held_slot(hero, 1), None);
    assert_eq!(std::iter::from_fn(|| h.take_fired_event()).count(), 2);
}

#[test]
fn non_hero_base_mana_regeneration_receives_and_loses_the_held_fraction() {
    let Fixture {
        mut h, ordinary, ..
    } = open_with_regens(0., Some("2"), 0.);
    set_fixture_max_mana(&mut h, ordinary, 100.);
    set_mana(&mut h, ordinary, 0.);
    advance_passive_regen(&mut h, 1.);
    assert_close(mana_of(&h, ordinary), 2.0, "source base rate");
    let mana = item(&mut h, b"pman", 0.);
    assert_eq!(add(&mut h, ordinary, mana), Ok(Value::Bool(true)));
    assert_eq!(
        h.unit_hero_bonus(ordinary).unwrap().mana_regen_fraction,
        0.5
    );
    advance_passive_regen(&mut h, 2.);
    assert_close(mana_of(&h, ordinary), 5.0, "fraction on a non-hero");
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(ordinary), Value::Handle(mana)],
    )
    .unwrap();
    assert_eq!(h.unit_hero_bonus(ordinary).unwrap().mana_regen_fraction, 0.);
    advance_passive_regen(&mut h, 3.);
    assert_close(mana_of(&h, ordinary), 7.0, "drop removes the fraction");
}

#[test]
fn held_mana_fraction_partitions_clamps_and_pauses_with_the_unit() {
    let Fixture {
        h: mut whole,
        hero: whole_hero,
        ..
    } = open_with_regens(0., Some("2"), 0.);
    let Fixture {
        h: mut split,
        hero: split_hero,
        ..
    } = open_with_regens(0., Some("2"), 0.);
    for (h, hero) in [(&mut whole, whole_hero), (&mut split, split_hero)] {
        set_mana(h, hero, 0.);
        let mana = item(h, b"pman", 0.);
        add(h, hero, mana).unwrap();
    }
    advance_passive_regen(&mut whole, 2.);
    for time in [0.5, 1., 2.] {
        advance_passive_regen(&mut split, time);
    }
    assert_close(mana_of(&whole, whole_hero), 6.0, "whole clock");
    assert_close(mana_of(&split, split_hero), 6.0, "split clock");
    let maximum = whole.unit_vitals(whole_hero).unwrap().unwrap().max_mana;
    set_mana(&mut whole, whole_hero, maximum - 1.);
    advance_passive_regen(&mut whole, 3.);
    assert_close(mana_of(&whole, whole_hero), maximum, "clamped");
    set_mana(&mut whole, whole_hero, 0.);
    whole
        .native("PauseUnit", &[Value::Handle(whole_hero), Value::Bool(true)])
        .unwrap();
    advance_passive_regen(&mut whole, 4.);
    assert_close(mana_of(&whole, whole_hero), 0., "paused");
    whole
        .native(
            "PauseUnit",
            &[Value::Handle(whole_hero), Value::Bool(false)],
        )
        .unwrap();
    advance_passive_regen(&mut whole, 5.);
    assert_close(mana_of(&whole, whole_hero), 3.0, "resumed with fraction");
}

#[test]
fn held_life_regeneration_stacks_transfers_drops_and_cleans_up() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    prepare_passive_regen(&mut h, &[hero, ordinary]);
    let first = item(&mut h, b"preg", 0.);
    let second = item(&mut h, b"preg", 0.);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    advance_passive_regen(&mut h, 1.);
    assert_eq!(life_of(&h, hero), 107.);
    assert_eq!(life_of(&h, ordinary), 100.);
    assert_eq!(add(&mut h, hero, second), Ok(Value::Bool(true)));
    advance_passive_regen(&mut h, 2.);
    assert_eq!(life_of(&h, hero), 121.);
    assert_eq!(add(&mut h, ordinary, first), Ok(Value::Bool(true)));
    advance_passive_regen(&mut h, 3.);
    assert_eq!(life_of(&h, hero), 128.);
    assert_eq!(life_of(&h, ordinary), 107.);
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(second)],
    )
    .unwrap();
    advance_passive_regen(&mut h, 4.);
    assert_eq!(life_of(&h, hero), 128.);
    assert_eq!(life_of(&h, ordinary), 114.);
    h.forget_inventory_unit(ordinary, false).unwrap();
    advance_passive_regen(&mut h, 5.);
    assert_eq!(life_of(&h, ordinary), 114.);
    assert_eq!(h.unit_hero_bonus(ordinary).unwrap().life_regen, 0);
}

#[test]
fn held_life_regeneration_is_partition_independent_and_clamped() {
    let Fixture {
        h: mut whole,
        hero: whole_hero,
        ..
    } = open();
    let Fixture {
        h: mut split,
        hero: split_hero,
        ..
    } = open();
    for (h, hero) in [(&mut whole, whole_hero), (&mut split, split_hero)] {
        prepare_passive_regen(h, &[hero]);
        let regen = item(h, b"preg", 0.);
        add(h, hero, regen).unwrap();
    }
    advance_passive_regen(&mut whole, 4.);
    for time in [0.5, 1., 2., 4.] {
        advance_passive_regen(&mut split, time);
    }
    assert_eq!(life_of(&whole, whole_hero), life_of(&split, split_hero));
    assert_eq!(life_of(&whole, whole_hero), 128.);
    let maximum = whole.unit_vitals(whole_hero).unwrap().unwrap().max_life;
    set_life(&mut whole, whole_hero, maximum - 2.);
    advance_passive_regen(&mut whole, 5.);
    assert_eq!(life_of(&whole, whole_hero), maximum);
}

#[test]
fn regeneration_item_changes_refuse_stale_projection_without_moving_items() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    prepare_passive_regen(&mut h, &[hero, ordinary]);
    let regen = item(&mut h, b"preg", 0.);
    h.next_timer_expiry(1., &mut 1000).unwrap();
    assert!(
        add(&mut h, hero, regen)
            .unwrap_err()
            .contains("projected through")
    );
    assert!(
        h.executed_items()
            .find(|(id, _)| *id == regen)
            .unwrap()
            .1
            .held_by
            .is_none()
    );
    h.advance_unit_activity(1., &mut 65536).unwrap();
    add(&mut h, hero, regen).unwrap();
    assert_eq!(life_of(&h, hero), 100.);
    h.next_timer_expiry(2., &mut 1000).unwrap();
    let before = h
        .executed_items()
        .find(|(id, _)| *id == regen)
        .unwrap()
        .1
        .clone();
    assert!(
        add(&mut h, ordinary, regen)
            .unwrap_err()
            .contains("projected through")
    );
    assert!(
        h.native(
            "UnitRemoveItem",
            &[Value::Handle(hero), Value::Handle(regen)]
        )
        .is_err()
    );
    assert_eq!(
        h.executed_items().find(|(id, _)| *id == regen).unwrap().1,
        &before
    );
    h.advance_unit_activity(2., &mut 65536).unwrap();
    assert_eq!(life_of(&h, hero), 107.);
}

#[test]
fn strength_and_item_life_regeneration_do_not_use_base_eligibility() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open_with_strength_regen(0.3);
    prepare_passive_regen(&mut h, &[hero, ordinary]);
    advance_passive_regen(&mut h, 1.);
    assert_eq!(life_of(&h, hero), 103.);
    assert_eq!(life_of(&h, ordinary), 100.);
    let regen = item(&mut h, b"preg", 0.);
    add(&mut h, hero, regen).unwrap();
    advance_passive_regen(&mut h, 2.);
    assert_eq!(life_of(&h, hero), 113.);
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(regen)],
    )
    .unwrap();
    advance_passive_regen(&mut h, 3.);
    assert_eq!(life_of(&h, hero), 116.);
}

#[test]
fn mixed_item_cleanup_refuses_stale_regeneration_before_any_drop() {
    let Fixture { mut h, hero, .. } = open();
    let plain = item(&mut h, b"pdm1", 0.);
    let regen = item(&mut h, b"preg", 0.);
    add(&mut h, hero, plain).unwrap();
    add(&mut h, hero, regen).unwrap();
    let before: Vec<_> = h
        .executed_items()
        .map(|(id, record)| (id, record.clone()))
        .collect();
    let unit = h.executed_unit(hero).unwrap().clone();
    h.next_timer_expiry(1., &mut 1000).unwrap();
    assert!(
        h.forget_inventory_unit(hero, false)
            .unwrap_err()
            .to_string()
            .contains("projected through")
    );
    assert_eq!(
        h.executed_items()
            .map(|(id, record)| (id, record.clone()))
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(h.executed_unit(hero).unwrap(), &unit);
    h.advance_unit_activity(1., &mut 65536).unwrap();
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(h.held_slot(hero, 0), None);
    assert_eq!(h.held_slot(hero, 1), None);
}

#[test]
fn cleanup_refuses_an_oversized_strength_item_before_dropping_the_earlier_plain_item() {
    let Fixture { mut h, hero, .. } = open();
    let plain = item(&mut h, b"tsrd", 0.);
    let huge = item(&mut h, b"pbig", 0.);
    assert_eq!(add(&mut h, hero, plain), Ok(Value::Bool(true)));
    assert_eq!(add(&mut h, hero, huge), Ok(Value::Bool(true)));
    assert_eq!(h.held_slot(hero, 0), Some(plain));
    assert_eq!(h.held_slot(hero, 1), Some(huge));
    set_fixture_max_life(&mut h, hero, 100.);
    set_life(&mut h, hero, 100.);
    let ground = item(&mut h, b"tstr", 64.);
    let _drop = register_drop(&mut h, hero);
    let pending = issue_pending_pickup(&mut h, hero, ground);
    let before = snapshot_records(&h, &[hero]);
    let err = h
        .forget_inventory_unit(hero, false)
        .unwrap_err()
        .to_string();
    assert!(err.contains("new maximum life is not positive"), "{err}");
    assert_records_unchanged(&h, &[hero], &before);
    assert_eq!(h.held_slot(hero, 0), Some(plain));
    assert_eq!(h.held_slot(hero, 1), Some(huge));
    assert_eq!(h.inventory_order(hero), Some(pending));
    assert!(h.take_fired_event().is_none());
    set_fixture_max_life(&mut h, hero, 30_000.);
    set_life(&mut h, hero, 100.);
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(h.held_slot(hero, 0), None);
    assert_eq!(h.held_slot(hero, 1), None);
    assert_eq!(h.inventory_order(hero), None);
    assert_eq!(std::iter::from_fn(|| h.take_fired_event()).count(), 2);
}

#[test]
fn cleanup_refuses_a_minimum_strength_removal_before_any_write() {
    let Fixture { mut h, hero, .. } = open();
    let debuff = item(&mut h, b"pmin", 0.);
    set_fixture_max_life(&mut h, hero, 1.0e12);
    set_life(&mut h, hero, 5.0e11);
    assert_eq!(add(&mut h, hero, debuff), Ok(Value::Bool(true)));
    assert_eq!(h.held_slot(hero, 0), Some(debuff));
    let before = snapshot_records(&h, &[hero]);
    let err = h
        .forget_inventory_unit(hero, false)
        .unwrap_err()
        .to_string();
    assert!(err.contains("strength removal delta overflows"), "{err}");
    assert_records_unchanged(&h, &[hero], &before);
    assert_eq!(h.held_slot(hero, 0), Some(debuff));
    assert!(h.take_fired_event().is_none());
}

#[test]
fn cleanup_preflights_sequential_strength_pools_before_dropping_either_item() {
    let Fixture { mut h, hero, .. } = open();
    let first = item(&mut h, b"pstr", 0.);
    let second = item(&mut h, b"pstr", 0.);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    assert_eq!(add(&mut h, hero, second), Ok(Value::Bool(true)));
    set_fixture_max_life(&mut h, hero, 200.);
    set_life(&mut h, hero, 100.);
    assert!(
        crate::inventory_powerup_host::reprice_life(100.0, 200.0, LIFE_PER_STRENGTH, -5).is_ok()
    );
    let (life, max_life) =
        crate::inventory_powerup_host::reprice_life(100.0, 200.0, LIFE_PER_STRENGTH, -5).unwrap();
    assert!(
        crate::inventory_powerup_host::reprice_life(life, max_life, LIFE_PER_STRENGTH, -5).is_err()
    );
    let ground = item(&mut h, b"tstr", 64.);
    let _drop = register_drop(&mut h, hero);
    let pending = issue_pending_pickup(&mut h, hero, ground);
    let before = snapshot_records(&h, &[hero]);
    let err = h
        .forget_inventory_unit(hero, false)
        .unwrap_err()
        .to_string();
    assert!(err.contains("new maximum life is not positive"), "{err}");
    assert_records_unchanged(&h, &[hero], &before);
    assert_eq!(h.held_slot(hero, 0), Some(first));
    assert_eq!(h.held_slot(hero, 1), Some(second));
    assert_eq!(h.inventory_order(hero), Some(pending));
    assert!(h.take_fired_event().is_none());
    set_fixture_max_life(&mut h, hero, 1_000.);
    set_life(&mut h, hero, 100.);
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(h.held_slot(hero, 0), None);
    assert_eq!(h.held_slot(hero, 1), None);
    assert_eq!(h.unit_vitals(hero).unwrap().unwrap().max_life, HERO_LIFE);
    assert_eq!(std::iter::from_fn(|| h.take_fired_event()).count(), 2);
}

#[test]
fn cleanup_drops_sequential_fractional_max_life_without_spurious_refusal() {
    let Fixture { mut h, hero, .. } = open();
    let first = item(&mut h, b"plif", 0.);
    let second = item(&mut h, b"plon", 0.);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    assert_eq!(add(&mut h, hero, second), Ok(Value::Bool(true)));
    set_fixture_max_life(&mut h, hero, 100.6);
    set_life(&mut h, hero, 100.6);
    let _drop = register_drop(&mut h, hero);
    h.forget_inventory_unit(hero, false).unwrap();
    assert_eq!(h.held_slot(hero, 0), None);
    assert_eq!(h.held_slot(hero, 1), None);
    assert_close(life_of(&h, hero), 62.6, "clamped sequential life");
    assert_close(max_life_of(&h, hero), 62.6, "clamped sequential maximum");
    assert_eq!(std::iter::from_fn(|| h.take_fired_event()).count(), 2);
}

#[test]
fn cached_unit_survives_storage_and_restores_progress_sparse_slots_charges_and_owner() {
    use crate::game_cache::tests::{Memory, budget};
    let store = Arc::new(Memory::default());
    let Fixture { mut h, hero, .. } = open();
    h.admit_game_caches(store.clone(), budget()).unwrap();
    let cache = h
        .native("InitGameCache", &[Value::Str("Campaign.w3v".into())])
        .unwrap();
    h.native(
        "SetHeroLevel",
        &[Value::Handle(hero), Value::Int(4), Value::Bool(false)],
    )
    .unwrap();
    let tome = item(&mut h, b"tstr", 0.);
    add(&mut h, hero, tome).unwrap();
    let first = item(&mut h, b"phcd", 0.);
    let second = item(&mut h, b"pstr", 0.);
    add(&mut h, hero, first).unwrap();
    add(&mut h, hero, second).unwrap();
    h.native("SetItemCharges", &[Value::Handle(second), Value::Int(9)])
        .unwrap();
    // Keep slot one occupied while slot zero is empty; restoration must not compact it.
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(hero), Value::Handle(first)],
    )
    .unwrap();
    set_life(&mut h, hero, 123.5);
    let before = h.executed_unit(hero).unwrap().clone();
    let key = [
        cache.clone(),
        Value::Str("Next".into()),
        Value::Str("Hero".into()),
    ];
    let mut put = key.to_vec();
    put.push(Value::Handle(hero));
    assert_eq!(h.native("StoreUnit", &put).unwrap(), Value::Bool(true));
    let mut primitive = key.to_vec();
    primitive.push(Value::Int(17));
    h.native("StoreInteger", &primitive).unwrap();
    assert_eq!(
        h.native("SaveGameCache", &[cache]).unwrap(),
        Value::Bool(true)
    );
    drop(h);

    let Fixture { mut h, .. } = open();
    h.admit_game_caches(store, budget()).unwrap();
    let cache = h
        .native("InitGameCache", &[Value::Str("campaign.W3V".into())])
        .unwrap();
    let key = [cache, Value::Str("NEXT".into()), Value::Str("HERO".into())];
    let owner = h.native("Player", &[Value::Int(5)]).unwrap();
    h.next_timer_expiry(9., &mut 1000).unwrap();
    h.advance_unit_activity(9., &mut 65536).unwrap();
    let mut restore = key.to_vec();
    restore.extend([
        owner.clone(),
        Value::Real(64.),
        Value::Real(16.),
        Value::Real(90.),
    ]);
    let Value::Handle(restored) = h.native("RestoreUnit", &restore).unwrap() else {
        panic!("restored unit");
    };
    let record = h.executed_unit(restored).unwrap();
    assert_eq!(Value::Handle(record.owner), owner);
    assert_eq!((record.x, record.y, record.face), (64., 16., 90.));
    assert_eq!(record.abilities, before.abilities);
    let vitals = record.vitals.unwrap();
    let previous = before.vitals.unwrap();
    assert_eq!(
        [vitals.life, vitals.max_life, vitals.mana, vitals.max_mana],
        [
            previous.life,
            previous.max_life,
            previous.mana,
            previous.max_mana
        ]
    );
    assert_eq!(vitals.regen.through, 9.);
    assert_eq!(record.permanent_strength, before.permanent_strength);
    assert_eq!(h.held_slot(restored, 0), None);
    let held = h.held_slot(restored, 1).unwrap();
    assert_eq!(
        h.executed_items()
            .find(|(id, _)| *id == held)
            .unwrap()
            .1
            .definition
            .rawcode,
        *b"pstr"
    );
    assert_eq!(
        h.native("GetItemCharges", &[Value::Handle(held)]).unwrap(),
        Value::Int(9)
    );
    h.native(
        "UnitRemoveItem",
        &[Value::Handle(restored), Value::Handle(held)],
    )
    .unwrap();
    assert_eq!(
        h.unit_vitals(restored).unwrap().unwrap().max_life,
        previous.max_life - PASSIVE_STRENGTH * LIFE_PER_STRENGTH
    );
    assert_eq!(h.native("GetStoredInteger", &key).unwrap(), Value::Int(17));
    assert_eq!(h.native("HaveStoredUnit", &key).unwrap(), Value::Bool(true));
    h.native("FlushStoredUnit", &key).unwrap();
    assert_eq!(h.native("RestoreUnit", &restore).unwrap(), Value::Null);
    assert_eq!(h.native("GetStoredInteger", &key).unwrap(), Value::Int(17));
}

#[test]
fn cached_unit_restore_refusals_leave_the_arena_inventory_and_cache_intact() {
    use crate::game_cache::{
        CachedUnit, Kind, Profile, Stored,
        tests::{Memory, budget},
    };
    let store = Arc::new(Memory::default());
    let Fixture { mut h, hero, .. } = open();
    h.admit_game_caches(store.clone(), budget()).unwrap();
    let cache = h
        .native("InitGameCache", &[Value::Str("campaign".into())])
        .unwrap();
    let held = item(&mut h, b"phcd", 0.);
    add(&mut h, hero, held).unwrap();
    h.native(
        "StoreUnit",
        &[
            cache.clone(),
            Value::Str("next".into()),
            Value::Str("hero".into()),
            Value::Handle(hero),
        ],
    )
    .unwrap();
    h.native("SaveGameCache", &[cache]).unwrap();
    let original = store.bytes.lock().unwrap().clone().unwrap();
    drop(h);
    for case in 0..6 {
        let mut profile = Profile::decode(&original, budget()).unwrap();
        let Some(Stored::Unit(bytes)) = profile.get("campaign", "next", "hero", Kind::Unit) else {
            panic!("stored unit");
        };
        let mut saved = CachedUnit::decode(bytes, budget().storage_bytes).unwrap();
        match case {
            0 => saved.items[0].slot = 9,
            1 => saved.install = "foreign install".into(),
            3 => saved.client_sha256[0] ^= 1,
            5 => saved.items[0].rawcode = *b"tstr",
            _ => {}
        }
        profile
            .put(
                "campaign",
                "next".into(),
                "hero".into(),
                Stored::Unit(saved.encode(budget().storage_bytes).unwrap()),
                budget(),
            )
            .unwrap();
        let bytes = profile.encode(budget()).unwrap();
        let store = Arc::new(Memory::default());
        *store.bytes.lock().unwrap() = Some(bytes.clone());
        let Fixture { mut h, .. } = open();
        h.admit_game_caches(store.clone(), budget()).unwrap();
        let cache = h
            .native("InitGameCache", &[Value::Str("campaign".into())])
            .unwrap();
        if case == 2 {
            for _ in 0..16 {
                item(&mut h, b"phcd", 0.);
            }
        }
        if case == 4 {
            for _ in 0..8 {
                let id = item(&mut h, b"phcd", 0.);
                h.native("SetItemCharges", &[Value::Handle(id), Value::Int(3)])
                    .unwrap();
            }
        }
        let units: Vec<_> = h
            .executed_units()
            .map(|(id, record)| (id, record.clone()))
            .collect();
        let items: Vec<_> = h
            .executed_items()
            .map(|(id, record)| (id, record.clone()))
            .collect();
        let handles = h.inner().handles();
        let player = h.native("Player", &[Value::Int(4)]).unwrap();
        let key = [cache, Value::Str("next".into()), Value::Str("hero".into())];
        let mut restore = key.to_vec();
        restore.extend([player, Value::Real(64.), Value::Real(16.), Value::Real(90.)]);
        let error = h.native("RestoreUnit", &restore).unwrap_err();
        let reason = match case {
            0 => "cached inventory exceeds",
            1 => "different install/edition",
            2 => "cached item record policy",
            4 => "cached item charges",
            5 => "cached powerup cannot",
            _ => "different client image",
        };
        assert!(error.contains(reason), "case {case}: {error}");
        assert_eq!(h.inner().handles(), handles);
        assert_eq!(
            h.executed_units()
                .map(|(id, record)| (id, record.clone()))
                .collect::<Vec<_>>(),
            units
        );
        assert_eq!(
            h.executed_items()
                .map(|(id, record)| (id, record.clone()))
                .collect::<Vec<_>>(),
            items
        );
        assert_eq!(h.native("HaveStoredUnit", &key).unwrap(), Value::Bool(true));
        assert_eq!(*store.bytes.lock().unwrap(), Some(bytes));
    }
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
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    h.set_timer_game_time(1.0).unwrap();
    let potion = item(&mut h, b"phea", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    let ordinary_max = h.unit_vitals(ordinary).unwrap().unwrap().max_life;
    set_life(&mut h, ordinary, ordinary_max - 100.0);
    assert_eq!(
        use_item_on(&mut h, hero, potion, ordinary),
        Ok(Value::Bool(true))
    );
    assert_eq!(life_of(&h, ordinary), ordinary_max);
    assert_eq!(
        h.native("UnitHasItem", &[Value::Handle(hero), Value::Handle(potion)]),
        Ok(Value::Bool(false))
    );
}

#[test]
fn use_refuses_without_writes_when_not_held_or_not_a_use() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
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
fn reset_allows_the_next_potion_effect_but_does_not_refund_its_first_charge_or_heal() {
    let Fixture { mut h, hero, .. } = open();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.);
    let potion = item(&mut h, b"phcd", 0.);
    add(&mut h, hero, potion).unwrap();
    assert_eq!(use_item(&mut h, hero, potion), Ok(Value::Bool(true)));
    let first_life = life_of(&h, hero);
    assert_eq!(use_item(&mut h, hero, potion), Ok(Value::Bool(false)));
    h.native("UnitResetCooldown", &[Value::Handle(hero)])
        .unwrap();
    assert_eq!(life_of(&h, hero), first_life);
    assert_eq!(
        h.native("GetItemCharges", &[Value::Handle(potion)])
            .unwrap(),
        Value::Int(1)
    );
    assert_eq!(use_item(&mut h, hero, potion), Ok(Value::Bool(true)));
    assert_eq!(life_of(&h, hero), (first_life + 250.).min(max));
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
    assert_eq!(
        use_item_on(&mut h, hero, potion, far),
        Ok(Value::Bool(false))
    );
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
    assert_eq!(
        h.native("GetManipulatedItem", &[]),
        Ok(Value::Handle(potion))
    );
    assert_eq!(
        h.native("GetManipulatingUnit", &[]),
        Ok(Value::Handle(hero))
    );
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
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    h.set_timer_game_time(1.0).unwrap();
    let max = h.unit_vitals(hero).unwrap().unwrap().max_life;
    set_life(&mut h, hero, max / 2.0);
    let before = life_of(&h, hero);
    let potion = item(&mut h, b"phea", 0.0);
    assert_eq!(add(&mut h, hero, potion), Ok(Value::Bool(true)));
    let local = owner_number(&h, hero);
    assert!(
        !h.player_activate_held_item(local + 1, hero, potion)
            .unwrap()
    );
    // The owning player's input for a unit that does not hold the item is refused too.
    assert!(
        !h.player_activate_held_item(local, ordinary, potion)
            .unwrap()
    );
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
    let err = h
        .player_activate_held_item(local, hero, targeted)
        .unwrap_err();
    assert!(format!("{err:?}").contains("target"), "{err:?}");
    assert_eq!(life_of(&h, hero), before);
    assert_eq!(
        h.native(
            "UnitHasItem",
            &[Value::Handle(hero), Value::Handle(targeted)]
        ),
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
    assert_eq!(
        h.native("GetManipulatedItem", &[]),
        Ok(Value::Handle(potion))
    );
    h.finish_fired_event();
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
    // Intelligence passive bonuses join only with `includeBonuses`.
    let intelligence = item(&mut h, b"pint", 0.0);
    assert_eq!(add(&mut h, hero, intelligence), Ok(Value::Bool(true)));
    assert_eq!(query(&mut h, "GetHeroInt", false), Ok(7));
    assert_eq!(query(&mut h, "GetHeroInt", true), Ok(10));
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
    assert_eq!(query(&mut h, "GetHeroInt", false), Ok(8));
    assert_eq!(query(&mut h, "GetHeroInt", true), Ok(11));
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
