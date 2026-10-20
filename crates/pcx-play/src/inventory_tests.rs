use super::*;
use crate::camera_margins::SourceWindow;
use crate::inventory::{InventoryBudget, Rules};
use crate::neutral_player::sha256;
use crate::playercolor_host::{CooldownScope, UseCheck, UseCommit, UseGrant};
use crate::playercolor_host::{UseCheck, UseCommit, UseGrant};
use pcx_objects::editor::Overlay;
use pcx_script::HandleId;

const CALLS: &str = r#"
type item extends widget
type playerunitevent extends eventid
constant native ConvertPlayerUnitEvent takes integer i returns playerunitevent
native TriggerRegisterPlayerUnitEvent takes trigger t, player p, playerunitevent e, boolexpr b returns event
constant native GetTriggerUnit takes nothing returns unit
constant native GetTriggerPlayer takes nothing returns player
constant native GetManipulatingUnit takes nothing returns unit
constant native GetManipulatedItem takes nothing returns item
native CreateItem takes integer i, real x, real y returns item
native GetItemCharges takes item i returns integer
native SetItemCharges takes item i, integer c returns nothing
native RemoveItem takes item i returns nothing
native UnitHasItem takes unit u, item i returns boolean
native UnitAddItem takes unit u, item i returns boolean
native UnitRemoveItem takes unit u, item i returns nothing
native UnitItemInSlot takes unit u, integer i returns item
native UnitRemoveItemFromSlot takes unit u, integer i returns item
native UnitInventorySize takes unit u returns integer
native SetItemDroppable takes item i, boolean b returns nothing
native SetItemDropOnDeath takes item i, boolean b returns nothing
native SetItemInvulnerable takes item i, boolean b returns nothing
native IsItemInvulnerable takes item i returns boolean
native IssueTargetOrder takes unit u, string o, widget target returns boolean
globals
constant playerunitevent EVENT_PLAYER_UNIT_PICKUP_ITEM=ConvertPlayerUnitEvent(-71)
constant playerunitevent EVENT_PLAYER_UNIT_DROP_ITEM=ConvertPlayerUnitEvent(99)
endglobals
"#;
fn tables(slots: &str) -> Arc<DefaultTables> {
    let data = sylk(
        &[
            "alias",
            "code",
            "levels",
            "reqLevel",
            "levelSkip",
            "DataA1",
            "DataB1",
            "DataC1",
            "DataD1",
            "DataE1",
            "Cool1",
        ],
        &[
            &[
                "L000", "Xinv", "1", "0", "0", slots, "0", "1", "1", "1", "0",
            ],
            &["AIh1", "AIhe", "1", "0", "0", "0", "0", "0", "0", "0", "20"],
        ],
    );
    let meta = sylk(
        &[
            "ID",
            "field",
            "slk",
            "index",
            "type",
            "category",
            "data",
            "useSpecific",
            "maxVal",
        ],
        &[
            &[
                "alev",
                "levels",
                "AbilityData",
                "-1",
                "int",
                "data",
                "0",
                "",
                "1",
            ],
            &[
                "arlv",
                "reqLevel",
                "AbilityData",
                "-1",
                "int",
                "data",
                "0",
                "",
                "0",
            ],
            &[
                "alsk",
                "levelSkip",
                "AbilityData",
                "-1",
                "int",
                "data",
                "0",
                "",
                "0",
            ],
            &[
                "inv1",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "data",
                "1",
                "L000",
                "9",
            ],
            &[
                "inv2",
                "Data",
                "AbilityData",
                "-1",
                "bool",
                "data",
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
                "data",
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
                "data",
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
                "data",
                "5",
                "L000",
                "1",
            ],
        ],
    );
    Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic inventory".into(),
                edition_version: "fixture".into(),
            },
            &[
                (SlkId::AbilityData.path(), &data),
                (SlkId::AbilityMetaData.path(), &meta),
            ],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 2,
                max_asset_bytes: 65536,
                max_aggregate_input_bytes: 131072,
            },
        )
        .unwrap(),
    )
}
pub(super) fn order_inputs() -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    let (mut image, mut binding, budget) = crate::order_registry::tests::fixture([431, 744]);
    let mut code = image[0x220..0x220 + binding.initializer.len as usize - 2].to_vec();
    for (address, id) in [(0x301450u32, 937i32), (0x301460, 1133)] {
        code.push(0x68);
        code.extend_from_slice(&address.to_le_bytes());
        code.push(0x68);
        code.extend_from_slice(&id.to_le_bytes());
        code.extend_from_slice(&[0x8b, 0xce, 0xe8]);
        let next = binding.initializer.va + code.len() as u32 + 4;
        code.extend_from_slice(&binding.register.wrapping_sub(next).to_le_bytes());
    }
    code.extend_from_slice(&[0x5e, 0xc3]);
    image[0x220..0x220 + code.len()].copy_from_slice(&code);
    binding.initializer.len = code.len() as u32;
    binding.initializer.digest = sha256(&code);
    image[0x650..0x656].copy_from_slice(b"smart\0");
    image[0x660..0x668].copy_from_slice(b"getitem\0");
    let mut put = |at: usize, prefix: &[u8], id: i32| {
        let mut b = prefix.to_vec();
        b.extend_from_slice(&id.to_le_bytes());
        image[at..at + b.len()].copy_from_slice(&b);
        SourceWindow {
            va: 0x301000 + at as u32 - 0x200,
            len: b.len() as u32,
            digest: sha256(&b),
        }
    };
    let hold = put(0x320, &[0x3d], 431);
    let stop = put(0x330, &[0x81, 0xfb], 744);
    let movement = put(0x340, &[0x3d], 1631);
    binding.image_sha256 = sha256(&image);
    let registry = Arc::new(
        crate::order_registry::Registry::read(&image, binding.clone(), budget, 8, 80).unwrap(),
    );
    let rules = crate::unit_activity::Rules::read(
        &image,
        crate::unit_activity::Binding {
            origin: binding.origin,
            image_sha256: binding.image_sha256,
            hold,
            stop,
            movement,
        },
        &registry,
        budget,
    )
    .unwrap();
    (registry, rules)
}
struct Items;
impl ObjectCatalog for Items {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let base = [
            (*b"ipow", "powerup", "0"),
            (*b"idro", "droppable", "1"),
            (*b"idrp", "drop", "0"),
            (*b"iusa", "usable", "0"),
        ];
        let mut rows = base.to_vec();
        if rawcode == *b"W001" {
            rows.extend([
                (*b"iuse", "uses", "-"),
                (*b"iper", "perishable", "0"),
                (*b"iicd", "ignoreCD", "0"),
            ]);
        }
        if rawcode == *b"P001" {
            rows.extend([
                (*b"iusa", "usable", "1"),
                (*b"iuse", "uses", "2"),
                (*b"iper", "perishable", "1"),
                (*b"iicd", "ignoreCD", "0"),
                (*b"icid", "cooldownID", "AIhe"),
                (*b"iabi", "abilList", "AIh1"),
            ]);
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
fn open() -> ObjectHost {
    open_with_abilities(true)
}
fn open_with_abilities(admit: bool) -> ObjectHost {
    let declarations = format!("{}\n{}\n{}", enumeration::TRIGGERS, activity::ORDERS, CALLS);
    let fields = [(*b"uabi", ability_field("abilList", "L000"))];
    let h = position_host_admitting(65536, 16, "// inventory", &declarations, &fields, |h| h);
    let tables = tables("2");
    let (registry, activity) = order_inputs();
    let rules = Rules::read(
        Arc::clone(&tables),
        Overlay::default(),
        Arc::clone(&registry),
        &[(
            "synthetic MiscGame",
            b"[Misc]\nPickupItemRange=5\nDropItemRange=7\nGiveItemRange=9",
        )],
        1024,
    )
    .unwrap();
    let h = h
        .with_order_registry(registry)
        .unwrap()
        .with_player_unit_events(8)
        .unwrap();
    let h = if admit {
        h.with_hero_skills(
            tables,
            crate::scripts::AssetIdentity {
                source: "synthetic inventory".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            Overlay::default(),
            crate::hero_skills::tests::rules(),
            crate::playercolor_host::HeroBudget {
                abilities_per_unit: 8,
                overlay_objects: 4,
                modifications_per_ability: 8,
            },
        )
        .unwrap()
    } else {
        h
    };
    let mut h = h
        .with_item_catalog(
            Arc::new(Items),
            source(),
            crate::playercolor_host::ItemBudget {
                max_records: 8,
                max_retained_fields: 16,
                max_modifications: 0,
            },
        )
        .unwrap()
        .with_inventory(
            rules,
            InventoryBudget {
                max_slots_per_unit: 9,
                max_pending_orders: 4,
            },
        )
        .unwrap();
    h.admit_unit_activity(activity, 65536).unwrap();
    h
}
fn unit(h: &mut ObjectHost, x: f32) -> Value {
    let p = h.native("Player", &[Value::Int(4)]).unwrap();
    h.native(
        "CreateUnit",
        &[
            p,
            Value::Int(i32::from_be_bytes(*b"u001")),
            Value::Real(x),
            Value::Real(16.),
            Value::Real(0.),
        ],
    )
    .unwrap()
}
fn item(h: &mut ObjectHost, x: f32) -> Value {
    h.native(
        "CreateItem",
        &[
            Value::Int(i32::from_be_bytes(*b"I001")),
            Value::Real(x),
            Value::Real(16.),
        ],
    )
    .unwrap()
}
fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("handle")
    };
    *id
}
#[test]
fn inventory_slots_transfer_drop_remove_and_null_lifetimes_are_canonical() {
    let mut h = open();
    let a = unit(&mut h, 16.);
    let b = unit(&mut h, 32.);
    let first = item(&mut h, 48.);
    let second = item(&mut h, 64.);
    let third = item(&mut h, 80.);
    assert_eq!(
        h.native("UnitHasItem", &[a.clone(), first.clone()])
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native("UnitInventorySize", std::slice::from_ref(&a))
            .unwrap(),
        Value::Int(2)
    );
    for i in [&first, &second] {
        assert_eq!(
            h.native("UnitAddItem", &[a.clone(), i.clone()]).unwrap(),
            Value::Bool(true)
        );
    }
    assert_eq!(
        h.native("UnitAddItem", &[a.clone(), third.clone()])
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native("UnitItemInSlot", &[a.clone(), Value::Int(0)])
            .unwrap(),
        first
    );
    assert_eq!(
        h.native("UnitAddItem", &[b.clone(), first.clone()])
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        h.native("UnitHasItem", &[a.clone(), first.clone()])
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native("UnitRemoveItemFromSlot", &[b.clone(), Value::Int(0)])
            .unwrap(),
        first
    );
    assert_eq!(h.executed_item(id(&first)).unwrap().held_by, None);
    assert_eq!(h.executed_item(id(&first)).unwrap().x, 32.);
    h.native("RemoveItem", std::slice::from_ref(&second))
        .unwrap();
    assert_eq!(
        h.native("UnitHasItem", &[a.clone(), second]).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native("UnitHasItem", &[a, Value::Null]).unwrap(),
        Value::Bool(false)
    );
}
#[test]
fn pickup_routes_then_delivers_real_item_unit_player_context() {
    let mut h = open();
    let a = unit(&mut h, 16.);
    let target = item(&mut h, 80.);
    let t = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    let event = h
        .native("ConvertPlayerUnitEvent", &[Value::Int(-71)])
        .unwrap();
    h.native(
        "TriggerRegisterPlayerUnitEvent",
        &[t, player.clone(), event, Value::Null],
    )
    .unwrap();
    assert_eq!(
        h.native(
            "IssueTargetOrder",
            &[a.clone(), Value::Str("getitem".into()), target.clone()]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        h.native("UnitHasItem", &[a.clone(), target.clone()])
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native("GetUnitCurrentOrder", std::slice::from_ref(&a))
            .unwrap(),
        Value::Int(1133)
    );
    h.next_timer_expiry(9., &mut 1000).unwrap();
    h.advance_unit_activity(9., &mut 65536).unwrap();
    assert_eq!(
        h.native("UnitHasItem", &[a.clone(), target.clone()])
            .unwrap(),
        Value::Bool(true)
    );
    let fired = h.take_fired_event().unwrap();
    h.begin_fired_event(&fired).unwrap().unwrap();
    assert_eq!(h.native("GetManipulatedItem", &[]).unwrap(), target);
    assert_eq!(h.native("GetManipulatingUnit", &[]).unwrap(), a);
    assert_eq!(h.native("GetTriggerPlayer", &[]).unwrap(), player);
    h.finish_fired_event();
    assert!(h.take_fired_event().is_none());
}

#[test]
fn inventory_source_capacity_overlays_and_permission_flags_drive_policy() {
    let (registry, _) = order_inputs();
    let profiles = [(
        "synthetic",
        b"[Misc]\nPickupItemRange=3\nDropItemRange=4\nGiveItemRange=5".as_slice(),
    )];
    let mut overlay = Overlay::default();
    let modification = |field, value| pcx_objects::Mod {
        field,
        value_type: pcx_objects::ValueType::Int,
        selectors: Some([1, 0]),
        value: pcx_objects::Value::Int(value),
        end_word: 0,
    };
    overlay.mods.insert(
        "L000".into(),
        vec![
            modification(*b"inv1", 4),
            modification(*b"inv4", 0),
            modification(*b"inv2", 1),
        ],
    );
    let rules = Rules::read(tables("2"), overlay, Arc::clone(&registry), &profiles, 1024).unwrap();
    let policy = rules.policy([(*b"L000", 1)].into_iter(), 9).unwrap();
    assert_eq!(policy.slots, 4);
    assert!(!policy.get_items);
    assert!(policy.drop_on_death);
    assert!(rules.policy([(*b"L000", 1)].into_iter(), 3).is_err());
    assert!(
        rules
            .policy([(*b"L000", 1), (*b"L000", 1)].into_iter(), 9)
            .is_err()
    );
    assert!(
        Rules::read(
            tables("2"),
            Overlay::default(),
            registry,
            &[("missing", b"[Misc]\nPickupItemRange=3")],
            1024
        )
        .is_err()
    );
}

#[test]
fn inventory_refusals_preserve_ground_ownership_and_pending_replacement() {
    let mut h = open();
    let a = unit(&mut h, 16.);
    let target = item(&mut h, 80.);
    assert!(h.native("UnitAddItem", &[a.clone(), a.clone()]).is_err());
    assert!(
        h.native("UnitHasItem", &[Value::Null, Value::Int(1)])
            .is_err()
    );
    assert_eq!(h.executed_item(id(&target)).unwrap().held_by, None);
    h.native(
        "IssueTargetOrder",
        &[a.clone(), Value::Str("smart".into()), target.clone()],
    )
    .unwrap();
    h.native(
        "IssueImmediateOrder",
        &[a.clone(), Value::Str("beta".into())],
    )
    .unwrap();
    h.next_timer_expiry(20., &mut 1000).unwrap();
    h.advance_unit_activity(20., &mut 65536).unwrap();
    assert_eq!(
        h.native("UnitHasItem", &[a.clone(), target.clone()])
            .unwrap(),
        Value::Bool(false)
    );
    h.native("UnitAddItem", &[a.clone(), target.clone()])
        .unwrap();
    h.forget_inventory_unit(id(&a), true).unwrap();
    assert_eq!(h.executed_item(id(&target)).unwrap().held_by, None);
}

#[test]
#[ignore = "requires PCX_INVENTORY_CATALOG retained selected source copy and PCX_PLAYER_RESOURCE_IMAGE selected original executable"]
fn supplied_inventory_sources_produce_hart_policy() {
    use pcx_objects::editor::defaults::Family;
    let root = std::path::PathBuf::from(
        std::env::var("PCX_INVENTORY_CATALOG").expect("PCX_INVENTORY_CATALOG"),
    );
    let image_path = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").expect("PCX_PLAYER_RESOURCE_IMAGE");
    let owned: Vec<_> = SlkId::ALL
        .iter()
        .filter_map(|id| {
            std::fs::read(root.join(id.path().replace('\\', "/").to_ascii_lowercase()))
                .ok()
                .map(|bytes| (id.path(), bytes))
        })
        .collect();
    let supplied: Vec<_> = owned
        .iter()
        .map(|(path, bytes)| (*path, bytes.as_slice()))
        .collect();
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: root.display().to_string(),
                edition_version: "explicit selected original source copy".into(),
            },
            &supplied,
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 128,
                max_asset_bytes: 16 << 20,
                max_aggregate_input_bytes: 128 << 20,
            },
        )
        .unwrap(),
    );
    let image = std::fs::read(&image_path).unwrap();
    let registry = Arc::new(
        crate::campaign_client::order_registry(
            &image,
            crate::scripts::AssetIdentity {
                source: image_path,
                edition: "Legacy".into(),
                version: "caller selected; image digest checked".into(),
            },
        )
        .unwrap(),
    );
    let misc = std::fs::read(root.join("units/miscgame.txt")).unwrap();
    let rules = Rules::read(
        Arc::clone(&tables),
        Overlay::default(),
        registry,
        &[("selected Units/MiscGame.txt", &misc)],
        8 << 20,
    )
    .unwrap();
    let table = tables.editor().table(SlkId::UnitAbilities).unwrap();
    let row = tables.editor().row(SlkId::UnitAbilities, "Hart").unwrap();
    let list = table
        .get(row, table.column_of("abilList").unwrap())
        .unwrap();
    let aliases: Vec<_> = list
        .split(',')
        .map(|s| {
            let object = tables.object(Family::Ability, s.trim()).unwrap();
            let first = *object.level_columns().keys().next().unwrap();
            (
                s.trim().as_bytes().try_into().unwrap(),
                i32::try_from(first).unwrap(),
            )
        })
        .collect();
    let policy = rules.policy(aliases.into_iter(), 128).unwrap();
    assert!(policy.slots > 0 && policy.get_items && policy.drop_items && policy.use_items);
    eprintln!(
        "original Hart inventory policy={policy:?} pickup_range={} source={:?} assets={:?}",
        rules.pickup_range,
        tables.identity(),
        tables.assets()
    );
}

#[test]
fn inventory_death_and_drop_flags_and_ordinary_unit_lifecycle_remain_distinct() {
    let mut h = open();
    let unit = unit(&mut h, 16.);
    let item = item(&mut h, 32.);
    h.native("UnitAddItem", &[unit.clone(), item.clone()])
        .unwrap();
    assert!(h.can_drop_inventory_item(id(&unit), id(&item)).unwrap());
    assert!(!h.can_use_inventory_item(id(&unit), id(&item)).unwrap());
    h.native("SetItemDroppable", &[item.clone(), Value::Bool(false)])
        .unwrap();
    assert!(!h.can_drop_inventory_item(id(&unit), id(&item)).unwrap());
    h.forget_inventory_unit(id(&unit), false).unwrap();
    assert_eq!(
        h.native("UnitHasItem", &[unit.clone(), item.clone()])
            .unwrap(),
        Value::Bool(true)
    );
    h.native("SetItemDropOnDeath", &[item.clone(), Value::Bool(true)])
        .unwrap();
    h.forget_inventory_unit(id(&unit), false).unwrap();
    assert_eq!(h.executed_item(id(&item)).unwrap().held_by, None);
    let mut ordinary = open_with_abilities(false);
    let value = self::unit(&mut ordinary, 16.);
    assert!(ordinary.inventory_policy(id(&value)).is_err());
    ordinary.forget_inventory_unit(id(&value), false).unwrap();
    ordinary.forget_inventory_unit(id(&value), true).unwrap();
}

/// Inventory host with the game clock and item use admitted. Potions are `P001` rows.
fn open_with_item_use() -> ObjectHost {
    open_with_cooldown_scope(CooldownScope::SharedGroup)
}

fn open_with_cooldown_scope(scope: CooldownScope) -> ObjectHost {
    open()
        .with_item_use(
            crate::item_use_rules::Rules::new(tables("2"), Overlay::default()),
            8,
            scope,
        )
        .unwrap()
}

fn potion(h: &mut ObjectHost) -> HandleId {
    id(&h
        .native(
            "CreateItem",
            &[
                Value::Int(i32::from_be_bytes(*b"P001")),
                Value::Real(0.),
                Value::Real(16.),
            ],
        )
        .unwrap())
}

fn give(h: &mut ObjectHost, unit: HandleId, item: HandleId) {
    assert_eq!(
        h.native("UnitAddItem", &[Value::Handle(unit), Value::Handle(item)])
            .unwrap(),
        Value::Bool(true)
    );
}

fn charges_of(h: &mut ObjectHost, item: HandleId) -> Value {
    h.native("GetItemCharges", &[Value::Handle(item)]).unwrap()
}

/// Two units, and one potion held by the first. Handles are deterministic for each fresh host.
fn holder_setup(h: &mut ObjectHost) -> (HandleId, HandleId, HandleId) {
    let a = id(&unit(h, 16.));
    let b = id(&unit(h, 32.));
    let p = potion(h);
    give(h, a, p);
    (a, b, p)
}

fn grant<E: std::fmt::Debug>(check: Result<UseCheck, E>) -> UseGrant {
    match check.unwrap() {
        UseCheck::Ready(grant) => grant,
        other => panic!("expected ready, got {other:?}"),
    }
}

#[test]
fn use_path_checks_the_holder_then_commits_one_charge_and_the_group_cooldown() {
    let mut h = open_with_item_use();
    let (a, _, p) = holder_setup(&mut h);
    let g = grant(h.check_item_use(a, p));
    assert_eq!((g.unit(), g.item(), g.time()), (a, p, 0.0));
    let commit = h.commit_item_use(g).unwrap();
    assert_eq!(
        commit,
        UseCommit {
            charges: Some(1),
            remove: false
        }
    );
    assert_eq!(charges_of(&mut h, p), Value::Int(1));
    assert_eq!(
        h.check_item_use(a, p).unwrap(),
        UseCheck::OnCooldown { remaining: 20.0 }
    );
}

#[test]
fn a_unit_that_does_not_hold_the_item_is_not_held() {
    let mut h = open_with_item_use();
    let (a, b, p) = holder_setup(&mut h);
    assert_eq!(h.check_item_use(b, p).unwrap(), UseCheck::NotHeld);
    let loose = potion(&mut h);
    assert_eq!(h.check_item_use(a, loose).unwrap(), UseCheck::NotHeld);
    assert_eq!(charges_of(&mut h, p), Value::Int(2));
}

#[test]
fn a_transfer_between_check_and_commit_makes_the_grant_stale_without_writes() {
    let mut h = open_with_item_use();
    let (a, b, p) = holder_setup(&mut h);
    let g = grant(h.check_item_use(a, p));
    h.native("UnitRemoveItem", &[Value::Handle(a), Value::Handle(p)])
        .unwrap();
    give(&mut h, b, p);
    assert!(h.commit_item_use(g).is_err());
    assert_eq!(charges_of(&mut h, p), Value::Int(2));
    assert!(
        h.check_item_use(b, p)
            .map(|c| matches!(c, UseCheck::Ready(_)))
            .unwrap()
    );
}

#[test]
fn a_drop_between_check_and_commit_makes_the_grant_stale() {
    let mut h = open_with_item_use();
    let (a, _, p) = holder_setup(&mut h);
    let g = grant(h.check_item_use(a, p));
    h.native("UnitRemoveItem", &[Value::Handle(a), Value::Handle(p)])
        .unwrap();
    assert!(h.commit_item_use(g).is_err());
    assert_eq!(charges_of(&mut h, p), Value::Int(2));
}

#[test]
fn removing_the_item_between_check_and_commit_refuses_and_reads_zero() {
    let mut h = open_with_item_use();
    let (a, _, p) = holder_setup(&mut h);
    let g = grant(h.check_item_use(a, p));
    h.native("RemoveItem", &[Value::Handle(p)]).unwrap();
    assert!(h.commit_item_use(g).is_err());
    assert_eq!(charges_of(&mut h, p), Value::Int(0));
}

#[test]
fn a_delayed_commit_after_the_game_clock_advances_refuses_and_a_fresh_check_starts_at_the_new_time()
{
    let mut h = open_with_item_use();
    let (a, _, p) = holder_setup(&mut h);
    let stale = grant(h.check_item_use(a, p));
    h.set_timer_game_time(1.0).unwrap();
    let err = h.commit_item_use(stale).unwrap_err();
    assert!(err.to_string().contains("expired"), "{err}");
    assert_eq!(charges_of(&mut h, p), Value::Int(2));
    let fresh = grant(h.check_item_use(a, p));
    assert_eq!(fresh.time(), 1.0);
    h.commit_item_use(fresh).unwrap();
    assert_eq!(
        h.check_item_use(a, p).unwrap(),
        UseCheck::OnCooldown { remaining: 20.0 }
    );
    h.set_timer_game_time(20.5).unwrap();
    assert_eq!(
        h.check_item_use(a, p).unwrap(),
        UseCheck::OnCooldown { remaining: 0.5 }
    );
}

#[test]
fn a_grant_from_another_host_session_refuses_even_with_the_same_handles() {
    let mut first = open_with_item_use();
    let mut second = open_with_item_use();
    let (a1, _, p1) = holder_setup(&mut first);
    let (a2, _, p2) = holder_setup(&mut second);
    assert_eq!((a1, p1), (a2, p2), "same handles in both hosts");
    let g = grant(first.check_item_use(a1, p1));
    let err = second.commit_item_use(g).unwrap_err();
    assert!(err.to_string().contains("another host session"), "{err}");
    assert_eq!(charges_of(&mut second, p2), Value::Int(2));
    let own = grant(second.check_item_use(a2, p2));
    assert!(second.commit_item_use(own).is_ok());
}

/// Two holders, each with one potion of the same group. A's use at t=0 is the only event.
fn two_holder_readiness(scope: CooldownScope) -> UseCheck {
    let mut h = open_with_cooldown_scope(scope);
    let a = id(&unit(&mut h, 16.));
    let b = id(&unit(&mut h, 32.));
    let pa = potion(&mut h);
    let pb = potion(&mut h);
    give(&mut h, a, pa);
    give(&mut h, b, pb);
    h.commit_item_use(grant(h.check_item_use(a, pa))).unwrap();
    h.check_item_use(b, pb).unwrap()
}

/// Provisional: the source does not establish whether holders share a group's cooldown.
#[test]
fn two_holders_under_the_shared_group_scope_wait_on_one_ready_time() {
    assert_eq!(
        two_holder_readiness(CooldownScope::SharedGroup),
        UseCheck::OnCooldown { remaining: 20.0 }
    );
}

#[test]
fn two_holders_under_the_per_holder_scope_each_wait_on_their_own_ready_time() {
    assert!(matches!(
        two_holder_readiness(CooldownScope::PerHolder),
        UseCheck::Ready(_)
    ));
}

#[test]
fn a_refused_effect_drops_the_grant_and_leaves_charges_and_cooldown_untouched() {
    let mut h = open_with_item_use();
    let (a, _, p) = holder_setup(&mut h);
    drop(grant(h.check_item_use(a, p)));
    assert_eq!(charges_of(&mut h, p), Value::Int(2));
    assert!(matches!(
        h.check_item_use(a, p).unwrap(),
        UseCheck::Ready(_)
    ));
}

#[test]
fn duplicate_grants_for_one_group_commit_once() {
    let mut h = open_with_item_use();
    let (a, b, pa) = holder_setup(&mut h);
    let pb = potion(&mut h);
    give(&mut h, b, pb);
    let first = grant(h.check_item_use(a, pa));
    let second = grant(h.check_item_use(b, pb));
    h.commit_item_use(first).unwrap();
    assert!(h.commit_item_use(second).is_err());
    assert_eq!(charges_of(&mut h, pb), Value::Int(2));
}

#[test]
fn the_last_charge_marks_the_perishable_potion_for_removal() {
    let mut h = open_with_item_use();
    let (a, _, p) = holder_setup(&mut h);
    h.commit_item_use(grant(h.check_item_use(a, p))).unwrap();
    h.set_timer_game_time(20.0).unwrap();
    let last = h.commit_item_use(grant(h.check_item_use(a, p))).unwrap();
    assert_eq!(last.charges, Some(0));
    assert!(last.remove);
}

#[test]
fn zero_charges_refuse_the_use_without_a_cooldown_or_writes() {
    let mut h = open_with_item_use();
    let (a, _, p) = holder_setup(&mut h);
    h.native("SetItemCharges", &[Value::Handle(p), Value::Int(0)])
        .unwrap();
    assert_eq!(h.check_item_use(a, p).unwrap(), UseCheck::NoCharges);
    assert_eq!(charges_of(&mut h, p), Value::Int(0));
    assert!(matches!(
        h.check_item_use(a, p).unwrap(),
        UseCheck::NoCharges
    ));
}

fn wand(h: &mut ObjectHost) -> HandleId {
    id(&h
        .native(
            "CreateItem",
            &[
                Value::Int(i32::from_be_bytes(*b"W001")),
                Value::Real(0.),
                Value::Real(16.),
            ],
        )
        .unwrap())
}

#[test]
fn an_uncharged_item_without_a_group_commits_each_check_as_its_own_use_at_one_tick() {
    // Native rule not measured: with no charge and no cooldown group, nothing in the source rows
    // gates a second use, so each check commits as its own use. The grant is move-only, so this is
    // two checks, not one grant committed twice.
    let mut h = open_with_item_use();
    let a = id(&unit(&mut h, 16.));
    let w = wand(&mut h);
    give(&mut h, a, w);
    let first = grant(h.check_item_use(a, w));
    let second = grant(h.check_item_use(a, w));
    assert_eq!(
        h.commit_item_use(first).unwrap(),
        UseCommit {
            charges: None,
            remove: false
        }
    );
    assert_eq!(
        h.commit_item_use(second).unwrap(),
        UseCommit {
            charges: None,
            remove: false
        }
    );
}

#[test]
fn a_second_ready_check_for_a_charged_grouped_item_refuses_on_commit_after_the_first_starts_the_group()
 {
    let mut h = open_with_item_use();
    let (a, _, p) = holder_setup(&mut h);
    let first = grant(h.check_item_use(a, p));
    let second = grant(h.check_item_use(a, p));
    h.commit_item_use(first).unwrap();
    assert!(h.commit_item_use(second).is_err());
    assert_eq!(charges_of(&mut h, p), Value::Int(1));
}

// Combined sale fixture: the inventory fixture plus player gold, stock, sale rules and the
// sell-item event. Only declarations needed by the transaction are added.
const SALE_CALLS: &str = r#"
type timer extends agent
type itemtype extends handle
type playerstate extends handle
native CreateTimer takes nothing returns timer
native SetItemTypeSlots takes unit whichUnit, integer slots returns nothing
native AddItemToStock takes unit whichUnit, integer itemId, integer currentStock, integer stockMax returns nothing
native RemoveItemFromStock takes unit whichUnit, integer itemId returns nothing
native IsItemIdSellable takes integer itemId returns boolean
native ChooseRandomItemEx takes itemtype whichType, integer level returns integer
constant native ConvertItemType takes integer i returns itemtype
constant native ConvertPlayerState takes integer i returns playerstate
native SetPlayerState takes player p, playerstate setting, integer value returns nothing
constant native GetPlayerState takes player p, playerstate setting returns integer
constant native GetSellingUnit takes nothing returns unit
constant native GetSoldItem takes nothing returns item
globals
constant playerunitevent EVENT_PLAYER_UNIT_SELL_ITEM=ConvertPlayerUnitEvent(274)
constant playerstate PLAYER_STATE_RESOURCE_GOLD=ConvertPlayerState(1)
constant playerstate PLAYER_STATE_RESOURCE_LUMBER=ConvertPlayerState(2)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_USED=ConvertPlayerState(5)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_CAP=ConvertPlayerState(4)
constant playerstate PLAYER_STATE_FOOD_CAP_CEILING=ConvertPlayerState(6)
endglobals
"#;

/// Sale-test catalog: `P001` is a powerup row, every other rawcode is a plain sellable item.
struct SaleItems;
impl ObjectCatalog for SaleItems {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let powerup = if rawcode == *b"P001" { "1" } else { "0" };
        let fields = [
            (*b"ipow", "powerup", powerup),
            (*b"idro", "droppable", "1"),
            (*b"idrp", "drop", "0"),
            (*b"iuse", "usable", "0"),
        ]
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

/// The inventory fixture with gold for the owner, a stock budget, sale rules for `I001`
/// (price `price`) and `P001` (price 10), and an item-record budget of `item_budget`.
fn open_sale(price: i32, item_budget: usize) -> ObjectHost {
    use crate::item_sales::{ItemSale, ItemSaleTable};
    let declarations = format!(
        "{}\n{}\n{}\n{}",
        enumeration::TRIGGERS,
        activity::ORDERS,
        CALLS,
        SALE_CALLS
    );
    let fields = [(*b"uabi", ability_field("abilList", "L000"))];
    let h = position_host_admitting(65536, 16, "// sale", &declarations, &fields, |h| {
        h.with_player_state_requests(crate::playercolor_host::PlayerStateRequestBudget {
            max_records: 8,
        })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules())
        .unwrap()
    });
    let tables = tables("2");
    let (registry, activity) = order_inputs();
    let rules = Rules::read(
        Arc::clone(&tables),
        Overlay::default(),
        Arc::clone(&registry),
        &[(
            "synthetic MiscGame",
            b"[Misc]\nPickupItemRange=5\nDropItemRange=7\nGiveItemRange=9",
        )],
        1024,
    )
    .unwrap();
    let origin = crate::scripts::AssetIdentity {
        source: "synthetic sale".into(),
        edition: "fixture".into(),
        version: "fixture".into(),
    };
    let sales = ItemSaleTable::new(
        origin.clone(),
        BTreeMap::from([
            (
                i32::from_be_bytes(*b"I001"),
                ItemSale {
                    level: Some(1),
                    class_ordinal: 0,
                    gold: price,
                    sellable: Some(true),
                },
            ),
            (
                i32::from_be_bytes(*b"P001"),
                ItemSale {
                    level: Some(1),
                    class_ordinal: 0,
                    gold: 10,
                    sellable: Some(true),
                },
            ),
        ]),
    );
    let shop = crate::shop_table::ShopTable {
        origin,
        image_sha256: [0; 32],
        slot_ceiling: 11,
        entries: 12,
    };
    let h = h
        .with_order_registry(registry)
        .unwrap()
        .with_player_unit_events(8)
        .unwrap()
        .with_hero_skills(
            Arc::clone(&tables),
            crate::scripts::AssetIdentity {
                source: "synthetic sale".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            Overlay::default(),
            crate::hero_skills::tests::rules(),
            crate::playercolor_host::HeroBudget {
                abilities_per_unit: 8,
                overlay_objects: 4,
                modifications_per_ability: 8,
            },
        )
        .unwrap();
    let mut h = h
        .with_item_catalog(
            Arc::new(SaleItems),
            source(),
            crate::playercolor_host::ItemBudget {
                max_records: item_budget,
                max_retained_fields: 4,
                max_modifications: 0,
            },
        )
        .unwrap()
        .with_inventory(
            rules,
            InventoryBudget {
                max_slots_per_unit: 9,
                max_pending_orders: 4,
            },
        )
        .unwrap()
        .with_stock(crate::playercolor_host::StockBudget {
            max_records: 16,
            shop: crate::playercolor_host::ShopCapacity::new(shop),
        })
        .unwrap()
        .with_item_sales(sales)
        .unwrap();
    h.admit_unit_activity(activity, 65536).unwrap();
    h
}

/// Owner gold, shop stock, buyer inventory, handles and the sell-event queue at one point.
#[derive(Debug, PartialEq)]
struct SaleState {
    gold: Value,
    stock: Option<(i32, i32)>,
    held: Vec<Value>,
    handles: usize,
}

fn sale_state(
    h: &mut ObjectHost,
    owner: &Value,
    shop: &Value,
    buyer: &Value,
    item: i32,
) -> SaleState {
    let gold_setting = h.native("ConvertPlayerState", &[Value::Int(1)]).unwrap();
    let gold = h
        .native("GetPlayerState", &[owner.clone(), gold_setting])
        .unwrap();
    let slots = h
        .native("UnitInventorySize", std::slice::from_ref(buyer))
        .unwrap();
    let Value::Int(slots) = slots else {
        panic!("inventory size")
    };
    let mut held = Vec::new();
    for slot in 0..slots {
        held.push(
            h.native("UnitItemInSlot", &[buyer.clone(), Value::Int(slot)])
                .unwrap(),
        );
    }
    SaleState {
        gold,
        stock: h.item_stock(id(shop), item).unwrap(),
        held,
        handles: h.inner().handles(),
    }
}

fn set_gold(h: &mut ObjectHost, owner: &Value, amount: i32) {
    let gold_setting = h.native("ConvertPlayerState", &[Value::Int(1)]).unwrap();
    h.native(
        "SetPlayerState",
        &[owner.clone(), gold_setting, Value::Int(amount)],
    )
    .unwrap();
}

fn stock(h: &mut ObjectHost, shop: &Value, item: i32, current: i32) {
    h.native(
        "AddItemToStock",
        &[
            shop.clone(),
            Value::Int(item),
            Value::Int(current),
            Value::Int(current),
        ],
    )
    .unwrap();
}

fn sold(outcome: &crate::playercolor_host::PurchaseOutcome) -> Option<HandleId> {
    match outcome {
        crate::playercolor_host::PurchaseOutcome::Sold { item } => Some(*item),
        crate::playercolor_host::PurchaseOutcome::Refused(_) => None,
    }
}

fn refusal(
    outcome: &crate::playercolor_host::PurchaseOutcome,
) -> crate::playercolor_host::PurchaseRefusal {
    match outcome {
        crate::playercolor_host::PurchaseOutcome::Refused(r) => *r,
        crate::playercolor_host::PurchaseOutcome::Sold { .. } => panic!("expected a refusal"),
    }
}

const I001: i32 = i32::from_be_bytes(*b"I001");
const P001: i32 = i32::from_be_bytes(*b"P001");

#[test]
fn sale_debits_gold_places_the_item_decrements_stock_and_delivers_completed_state() {
    let mut h = open_sale(50, 8);
    let shop = unit(&mut h, 16.);
    let buyer = unit(&mut h, 32.);
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    set_gold(&mut h, &owner, 120);
    stock(&mut h, &shop, I001, 2);
    let t = h.native("CreateTrigger", &[]).unwrap();
    let sell = h
        .native("ConvertPlayerUnitEvent", &[Value::Int(274)])
        .unwrap();
    h.native(
        "TriggerRegisterPlayerUnitEvent",
        &[t, owner.clone(), sell, Value::Null],
    )
    .unwrap();
    let before = sale_state(&mut h, &owner, &shop, &buyer, I001);
    assert_eq!(before.gold, Value::Int(120));
    let outcome = h.purchase_stock_item(id(&shop), id(&buyer), I001).unwrap();
    let sold_item = sold(&outcome).expect("sale succeeds");
    let after = sale_state(&mut h, &owner, &shop, &buyer, I001);
    assert_eq!(after.gold, Value::Int(70), "debited by the loaded price");
    assert_eq!(after.stock, Some((1, 2)), "one loaded stock unit sold");
    assert_eq!(after.handles, before.handles + 1, "one item created");
    assert!(after.held.contains(&Value::Handle(sold_item)));
    let fired = h.take_fired_event().expect("sell event queued");
    h.begin_fired_event(&fired).unwrap().unwrap();
    assert_eq!(h.native("GetSellingUnit", &[]).unwrap(), shop);
    assert_eq!(
        h.native("GetSoldItem", &[]).unwrap(),
        Value::Handle(sold_item)
    );
    // Callback sees the completed sale.
    let during = sale_state(&mut h, &owner, &shop, &buyer, I001);
    assert_eq!(during.gold, Value::Int(70));
    assert_eq!(during.stock, Some((1, 2)));
    assert!(during.held.contains(&Value::Handle(sold_item)));
    h.finish_fired_event();
    assert_eq!(
        h.native("GetSellingUnit", &[]).unwrap(),
        Value::Null,
        "no context after delivery"
    );
    assert_eq!(h.native("GetSoldItem", &[]).unwrap(), Value::Null);
    assert!(h.take_fired_event().is_none());
}

#[test]
fn sale_price_comes_from_the_loaded_row_not_a_default() {
    let mut h = open_sale(70, 8);
    let shop = unit(&mut h, 16.);
    let buyer = unit(&mut h, 32.);
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    set_gold(&mut h, &owner, 100);
    stock(&mut h, &shop, I001, 1);
    h.purchase_stock_item(id(&shop), id(&buyer), I001).unwrap();
    assert_eq!(
        sale_state(&mut h, &owner, &shop, &buyer, I001).gold,
        Value::Int(30)
    );
}

#[test]
fn sale_with_insufficient_gold_refuses_and_changes_nothing() {
    let mut h = open_sale(50, 8);
    let shop = unit(&mut h, 16.);
    let buyer = unit(&mut h, 32.);
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    set_gold(&mut h, &owner, 49);
    stock(&mut h, &shop, I001, 2);
    let t = h.native("CreateTrigger", &[]).unwrap();
    let sell = h
        .native("ConvertPlayerUnitEvent", &[Value::Int(274)])
        .unwrap();
    h.native(
        "TriggerRegisterPlayerUnitEvent",
        &[t, owner.clone(), sell, Value::Null],
    )
    .unwrap();
    let before = sale_state(&mut h, &owner, &shop, &buyer, I001);
    let outcome = h.purchase_stock_item(id(&shop), id(&buyer), I001).unwrap();
    assert_eq!(
        refusal(&outcome),
        crate::playercolor_host::PurchaseRefusal::InsufficientGold
    );
    assert_eq!(sale_state(&mut h, &owner, &shop, &buyer, I001), before);
    assert!(h.take_fired_event().is_none());
}

#[test]
fn sale_into_a_full_inventory_refuses_and_changes_nothing() {
    let mut h = open_sale(50, 8);
    let shop = unit(&mut h, 16.);
    let buyer = unit(&mut h, 32.);
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    set_gold(&mut h, &owner, 500);
    stock(&mut h, &shop, I001, 2);
    let slots = match h
        .native("UnitInventorySize", std::slice::from_ref(&buyer))
        .unwrap()
    {
        Value::Int(n) => n,
        other => panic!("inventory size {other:?}"),
    };
    for _ in 0..slots {
        let filler = item(&mut h, 48.);
        assert_eq!(
            h.native("UnitAddItem", &[buyer.clone(), filler]).unwrap(),
            Value::Bool(true)
        );
    }
    let before = sale_state(&mut h, &owner, &shop, &buyer, I001);
    let outcome = h.purchase_stock_item(id(&shop), id(&buyer), I001).unwrap();
    assert_eq!(
        refusal(&outcome),
        crate::playercolor_host::PurchaseRefusal::NoFreeSlot
    );
    assert_eq!(sale_state(&mut h, &owner, &shop, &buyer, I001), before);
    assert!(h.take_fired_event().is_none());
}

#[test]
fn unsupported_powerup_is_refused_before_any_item_is_created() {
    // Item budget of one record: a refusal that created the item would spend it.
    let mut h = open_sale(10, 1);
    let shop = unit(&mut h, 16.);
    let buyer = unit(&mut h, 32.);
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    set_gold(&mut h, &owner, 100);
    stock(&mut h, &shop, P001, 1);
    let before = sale_state(&mut h, &owner, &shop, &buyer, P001);
    let outcome = h.purchase_stock_item(id(&shop), id(&buyer), P001).unwrap();
    assert_eq!(
        refusal(&outcome),
        crate::playercolor_host::PurchaseRefusal::UnsupportedItem
    );
    assert_eq!(sale_state(&mut h, &owner, &shop, &buyer, P001), before);
    // The budget is intact: one ordinary creation still succeeds.
    item(&mut h, 48.);
}

#[test]
fn item_budget_refusal_is_an_error_that_changes_nothing() {
    let mut h = open_sale(50, 1);
    let shop = unit(&mut h, 16.);
    let buyer = unit(&mut h, 32.);
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    set_gold(&mut h, &owner, 100);
    stock(&mut h, &shop, I001, 2);
    item(&mut h, 48.);
    let before = sale_state(&mut h, &owner, &shop, &buyer, I001);
    let err = h
        .purchase_stock_item(id(&shop), id(&buyer), I001)
        .unwrap_err();
    assert!(err.to_string().contains("item creation"), "{err}");
    assert_eq!(sale_state(&mut h, &owner, &shop, &buyer, I001), before);
    assert!(h.take_fired_event().is_none());
}

#[test]
fn stale_stock_from_a_cleared_cell_refuses_without_changes() {
    let mut h = open_sale(50, 8);
    let shop = unit(&mut h, 16.);
    let buyer = unit(&mut h, 32.);
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    set_gold(&mut h, &owner, 100);
    stock(&mut h, &shop, I001, 2);
    h.native("SetItemTypeSlots", &[shop.clone(), Value::Int(0)])
        .unwrap();
    let before = sale_state(&mut h, &owner, &shop, &buyer, I001);
    let outcome = h.purchase_stock_item(id(&shop), id(&buyer), I001).unwrap();
    assert_eq!(
        refusal(&outcome),
        crate::playercolor_host::PurchaseRefusal::NotInStock
    );
    assert_eq!(sale_state(&mut h, &owner, &shop, &buyer, I001), before);
}

#[test]
fn consecutive_sales_deliver_their_own_context_and_never_leak() {
    let mut h = open_sale(50, 8);
    let shop = unit(&mut h, 16.);
    let buyer = unit(&mut h, 32.);
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    set_gold(&mut h, &owner, 500);
    stock(&mut h, &shop, I001, 2);
    let t = h.native("CreateTrigger", &[]).unwrap();
    let sell = h
        .native("ConvertPlayerUnitEvent", &[Value::Int(274)])
        .unwrap();
    h.native(
        "TriggerRegisterPlayerUnitEvent",
        &[t, owner.clone(), sell, Value::Null],
    )
    .unwrap();
    assert_eq!(
        h.native("GetSoldItem", &[]).unwrap(),
        Value::Null,
        "no context before a sale"
    );
    let first = sold(&h.purchase_stock_item(id(&shop), id(&buyer), I001).unwrap()).unwrap();
    let second = sold(&h.purchase_stock_item(id(&shop), id(&buyer), I001).unwrap()).unwrap();
    let f1 = h.take_fired_event().unwrap();
    let f2 = h.take_fired_event().unwrap();
    assert!(h.take_fired_event().is_none());
    h.begin_fired_event(&f1).unwrap().unwrap();
    assert_eq!(h.native("GetSoldItem", &[]).unwrap(), Value::Handle(first));
    h.finish_fired_event();
    assert_eq!(
        h.native("GetSoldItem", &[]).unwrap(),
        Value::Null,
        "first context ends"
    );
    h.begin_fired_event(&f2).unwrap().unwrap();
    assert_eq!(h.native("GetSoldItem", &[]).unwrap(), Value::Handle(second));
    h.finish_fired_event();
    assert_eq!(h.native("GetSellingUnit", &[]).unwrap(), Value::Null);
}
