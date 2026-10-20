use super::{COMMON, budget, context, object, slots_rule};

fn shop() -> crate::playercolor_host::ShopCapacity {
    crate::playercolor_host::ShopCapacity::new(crate::shop_table::ShopTable {
        origin: crate::scripts::AssetIdentity {
            edition: "synthetic".into(),
            version: "synthetic".into(),
            source: "synthetic".into(),
        },
        image_sha256: [0; 32],
        slot_ceiling: 11,
        entries: 12,
    })
}
use crate::playercolor_host::{Failure, ObjectHost, StockBudget, UnitBudget};
use pcx_objects::editor::ObjectKind;
use pcx_script::{HandleId, Host, Value};
use pcx_sim::world::{Definition, ObjectCatalog, ResolutionProblem, Source};
use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::Arc;

const DECL: &str = "type widget extends agent\ntype unit extends widget\nnative CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nnative SetAllItemTypeSlots takes integer slots returns nothing\nnative SetAllUnitTypeSlots takes integer slots returns nothing\nnative SetItemTypeSlots takes unit whichUnit, integer slots returns nothing\nnative SetUnitTypeSlots takes unit whichUnit, integer slots returns nothing\nnative AddItemToStock takes unit whichUnit, integer itemId, integer currentStock, integer stockMax returns nothing\nnative RemoveItemFromStock takes unit whichUnit, integer itemId returns nothing\n";

const MAP: &str = "globals\ninteger slots = 0\nplayer P = null\nunit U = null\nunit U2 = null\ntimer T = null\nendglobals\nfunction clear takes nothing returns nothing\ncall SetAllUnitTypeSlots(0)\nendfunction\nfunction main takes nothing returns nothing\nset slots = -4\ncall SetAllItemTypeSlots(slots)\ncall SetAllUnitTypeSlots(slots - 3)\nendfunction\nfunction place takes nothing returns nothing\nset P = Player(1)\nset U = CreateUnit(P, 1, 0.0, 0.0, 0.0)\nset U2 = CreateUnit(P, 1, 1.0, 0.0, 0.0)\nset T = CreateTimer()\nendfunction\nfunction configure takes nothing returns nothing\ncall SetItemTypeSlots(U, 0)\ncall SetItemTypeSlots(U, -1)\nendfunction\nfunction other_kind takes nothing returns nothing\ncall SetUnitTypeSlots(U, 5)\nendfunction\nfunction second takes nothing returns nothing\ncall SetItemTypeSlots(U2, 4)\nendfunction\nfunction replace takes nothing returns nothing\ncall SetAllItemTypeSlots(7)\nendfunction\nfunction bad_global takes nothing returns nothing\ncall SetAllItemTypeSlots(U)\nendfunction\nfunction bad_unit takes nothing returns nothing\ncall SetItemTypeSlots(T, 3)\nendfunction\nfunction stock_item takes nothing returns nothing\ncall AddItemToStock(U, 1, 1, 1)\nendfunction\nfunction stock_again takes nothing returns nothing\ncall AddItemToStock(U, 1, 4, 5)\nendfunction\nfunction stock_other takes nothing returns nothing\ncall AddItemToStock(U, 2, 1, 1)\nendfunction\nfunction stock_on_timer takes nothing returns nothing\ncall AddItemToStock(T, 1, 1, 1)\nendfunction\nfunction unstock_item takes nothing returns nothing\ncall RemoveItemFromStock(U, 1)\nendfunction\nfunction unstock_absent takes nothing returns nothing\ncall RemoveItemFromStock(U, 9)\nendfunction\n";

struct Fixture;

impl ObjectCatalog for Fixture {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        if rawcode == 1i32.to_be_bytes() {
            Ok(Definition {
                rawcode,
                base_rawcode: "fixture".into(),
                kind: ObjectKind::Unit,
                fields: BTreeMap::new(),
                modifications: Vec::new(),
            })
        } else {
            Err(ResolutionProblem::MissingRawcode("fixture".into()))
        }
    }
}

fn source() -> Source {
    Source {
        install: "selected-install".into(),
        edition: "selected-edition".into(),
        map: "stock-units".into(),
    }
}

fn common_with(extra: &str) -> String {
    let globals = COMMON.find("\nglobals\n").expect("globals");
    let mut common = COMMON.to_string();
    common.insert_str(globals + 1, extra);
    common
}

fn prepared(
    extra: &str,
    map: &str,
    label: &str,
) -> (ObjectHost, Arc<crate::map_admission::ParsedMapContext>) {
    let common = common_with(extra);
    let ctx = context(301, &common, map, label);
    (object(&ctx, 32), ctx)
}

fn opened(records: usize) -> pcx_script::OwnedVm<ObjectHost> {
    let (host, ctx) = prepared(DECL, MAP, "synthetic:stock-vm");
    host.with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(
            Arc::new(Fixture),
            source(),
            UnitBudget {
                max_records: 4,
                max_retained_fields: 1,
                max_modifications: 0,
            },
        )
        .unwrap()
        .with_stock(StockBudget {
            max_records: records,
            shop: shop(),
        })
        .unwrap()
        .into_vm()
}

fn handle(value: &Value) -> HandleId {
    let Value::Handle(id) = value else {
        panic!("handle");
    };
    *id
}

#[test]
fn owned_vm_retains_executed_slot_integers_without_inheriting_or_allocating() {
    let mut vm = opened(1);
    vm.init_globals().unwrap();
    assert_eq!(vm.host().all_item_type_slots().unwrap(), None);
    assert_eq!(vm.host().all_unit_type_slots().unwrap(), None);
    assert_eq!(vm.host().stock_unit_records().unwrap(), 0);
    let before = vm.host().inner().handles();
    vm.call("clear", &[]).unwrap();
    assert_eq!(vm.host().all_item_type_slots().unwrap(), None);
    assert_eq!(vm.host().all_unit_type_slots().unwrap(), Some(0));
    assert_eq!(vm.host().inner().handles(), before);
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.host().all_item_type_slots().unwrap(), Some(-4));
    assert_eq!(vm.host().all_unit_type_slots().unwrap(), Some(-7));
    assert_eq!(vm.host().stock_unit_records().unwrap(), 0);
    assert_eq!(vm.host().inner().handles(), before);
    vm.call("place", &[]).unwrap();
    let unit = handle(&vm.global("U"));
    let other = handle(&vm.global("U2"));
    let timer = handle(&vm.global("T"));
    let placed = vm.host().inner().handles();
    assert!(placed > before);
    assert_eq!(vm.host().unit_item_type_slots(unit).unwrap(), None);
    assert_eq!(vm.host().unit_type_slots(unit).unwrap(), None);
    vm.call("configure", &[]).unwrap();
    assert_eq!(vm.host().unit_item_type_slots(unit).unwrap(), Some(-1));
    assert_eq!(vm.host().unit_type_slots(unit).unwrap(), None);
    assert_eq!(vm.host().unit_item_type_slots(other).unwrap(), None);
    assert_eq!(vm.host().stock_unit_records().unwrap(), 1);
    assert_eq!(vm.host().all_item_type_slots().unwrap(), Some(-4));
    assert_eq!(vm.host().inner().handles(), placed);
    let refused = vm.call("other_kind", &[]).unwrap_err();
    assert!(
        refused.to_string().contains("stock unit slots"),
        "{refused}"
    );
    assert_eq!(vm.host().unit_item_type_slots(unit).unwrap(), Some(-1));
    assert_eq!(vm.host().unit_type_slots(unit).unwrap(), None);
    assert_eq!(vm.host().stock_unit_records().unwrap(), 1);
    assert_eq!(vm.host().all_unit_type_slots().unwrap(), Some(-7));
    let second = vm.call("second", &[]).unwrap_err();
    assert!(second.to_string().contains("stock unit slots"), "{second}");
    assert_eq!(vm.host().unit_item_type_slots(other).unwrap(), None);
    assert_eq!(vm.host().unit_item_type_slots(unit).unwrap(), Some(-1));
    vm.call("replace", &[]).unwrap();
    assert_eq!(vm.host().all_item_type_slots().unwrap(), Some(7));
    assert_eq!(vm.host().all_unit_type_slots().unwrap(), Some(-7));
    assert_eq!(vm.host().unit_item_type_slots(unit).unwrap(), Some(-1));
    let bad_global = vm.call("bad_global", &[]).unwrap_err();
    assert!(bad_global.to_string().contains("arguments"), "{bad_global}");
    assert_eq!(vm.host().all_item_type_slots().unwrap(), Some(7));
    let bad_unit = vm.call("bad_unit", &[]).unwrap_err();
    assert!(
        bad_unit.to_string().contains("another family"),
        "{bad_unit}"
    );
    assert_eq!(vm.host().stock_unit_records().unwrap(), 1);
    assert_eq!(vm.host().unit_item_type_slots(unit).unwrap(), Some(-1));
    let full = vm.call("stock_item", &[]).unwrap_err();
    assert!(full.to_string().contains("stock item records"), "{full}");
    assert_eq!(vm.host().item_stock(unit, 1).unwrap(), None);
    assert_eq!(vm.host().stock_item_records().unwrap(), 0);
    assert_eq!(vm.host().inner().handles(), placed);
    vm.host()
        .native("SetAllItemTypeSlots", &[Value::Int(i32::MIN)])
        .unwrap();
    assert!(
        vm.host()
            .native("SetAllItemTypeSlots", &[Value::Real(1.0)])
            .is_err()
    );
    assert_eq!(vm.host().all_item_type_slots().unwrap(), Some(i32::MIN));
    vm.host()
        .native("SetAllUnitTypeSlots", &[Value::Int(i32::MAX)])
        .unwrap();
    assert_eq!(vm.host().all_unit_type_slots().unwrap(), Some(i32::MAX));
    assert_eq!(vm.host().unit_item_type_slots(unit).unwrap(), Some(-1));
    assert!(vm.host().unit_item_type_slots(timer).is_err());
}

#[test]
fn item_stock_cells_evict_by_age_and_removal_and_shrink_follow_the_engine() {
    let mut vm = opened(16);
    vm.init_globals().unwrap();
    vm.call("place", &[]).unwrap();
    let unit = handle(&vm.global("U"));
    let placed = vm.host().inner().handles();
    let refused = vm
        .host()
        .native(
            "AddItemToStock",
            &[
                Value::Handle(unit),
                Value::Int(1),
                Value::Int(1),
                Value::Int(1),
            ],
        )
        .unwrap_err();
    assert!(refused.contains("slot count is not assigned"), "{refused}");
    vm.host()
        .native("SetItemTypeSlots", &[Value::Handle(unit), Value::Int(2)])
        .unwrap();
    let add = |vm: &mut pcx_script::OwnedVm<ObjectHost>, item: i32, cur: i32, max: i32| {
        vm.host()
            .native(
                "AddItemToStock",
                &[
                    Value::Handle(unit),
                    Value::Int(item),
                    Value::Int(cur),
                    Value::Int(max),
                ],
            )
            .unwrap();
    };
    add(&mut vm, 1, 1, 1);
    add(&mut vm, 2, 2, 2);
    assert_eq!(vm.host().item_stock(unit, 1).unwrap(), Some((1, 1)));
    add(&mut vm, 3, 3, 3);
    assert_eq!(vm.host().item_stock(unit, 1).unwrap(), None);
    assert_eq!(vm.host().item_stock(unit, 2).unwrap(), Some((2, 2)));
    assert_eq!(vm.host().item_stock(unit, 3).unwrap(), Some((3, 3)));
    assert_eq!(vm.host().stock_item_records().unwrap(), 2);
    add(&mut vm, 2, 9, 9);
    assert_eq!(vm.host().item_stock(unit, 2).unwrap(), Some((9, 9)));
    assert_eq!(vm.host().stock_item_records().unwrap(), 2);
    vm.host()
        .native("RemoveItemFromStock", &[Value::Handle(unit), Value::Int(3)])
        .unwrap();
    assert_eq!(vm.host().item_stock(unit, 3).unwrap(), None);
    assert_eq!(vm.host().stock_item_records().unwrap(), 1);
    vm.host()
        .native("RemoveItemFromStock", &[Value::Handle(unit), Value::Int(7)])
        .unwrap();
    assert_eq!(vm.host().stock_item_records().unwrap(), 1);
    vm.host()
        .native("SetItemTypeSlots", &[Value::Handle(unit), Value::Int(1)])
        .unwrap();
    assert_eq!(vm.host().item_stock(unit, 2).unwrap(), None);
    assert_eq!(vm.host().stock_item_records().unwrap(), 0);
    assert_eq!(vm.host().inner().handles(), placed);
}

#[test]
fn a_new_stock_record_past_the_caller_budget_refuses_and_changes_no_cell() {
    let mut vm = opened(2);
    vm.init_globals().unwrap();
    vm.call("place", &[]).unwrap();
    let unit = handle(&vm.global("U"));
    vm.host()
        .native("SetItemTypeSlots", &[Value::Handle(unit), Value::Int(2)])
        .unwrap();
    vm.host()
        .native(
            "AddItemToStock",
            &[
                Value::Handle(unit),
                Value::Int(1),
                Value::Int(1),
                Value::Int(1),
            ],
        )
        .unwrap();
    let full = vm
        .host()
        .native(
            "AddItemToStock",
            &[
                Value::Handle(unit),
                Value::Int(2),
                Value::Int(1),
                Value::Int(1),
            ],
        )
        .unwrap_err();
    assert!(full.contains("stock item records"), "{full}");
    assert_eq!(vm.host().item_stock(unit, 1).unwrap(), Some((1, 1)));
    assert_eq!(vm.host().stock_item_records().unwrap(), 1);
    vm.host()
        .native(
            "AddItemToStock",
            &[
                Value::Handle(unit),
                Value::Int(1),
                Value::Int(5),
                Value::Int(5),
            ],
        )
        .unwrap();
    assert_eq!(vm.host().item_stock(unit, 1).unwrap(), Some((5, 5)));
}

#[test]
fn failed_slot_calls_and_the_record_budget_preserve_prior_state() {
    let mut vm = opened(1);
    vm.init_globals().unwrap();
    vm.call("place", &[]).unwrap();
    let unit = handle(&vm.global("U"));
    let timer = handle(&vm.global("T"));
    vm.host()
        .native("SetAllItemTypeSlots", &[Value::Int(4)])
        .unwrap();
    vm.host()
        .native("SetItemTypeSlots", &[Value::Handle(unit), Value::Int(3)])
        .unwrap();
    let unknown = Value::Handle(HandleId::new(NonZeroU64::new(u64::MAX).unwrap()));
    for bad in [
        vec![],
        vec![Value::Int(9), Value::Int(1)],
        vec![Value::Handle(unit), Value::Real(3.0)],
        vec![Value::Null, Value::Int(3)],
        vec![Value::Handle(timer), Value::Int(3)],
        vec![unknown, Value::Int(3)],
    ] {
        assert!(vm.host().native("SetItemTypeSlots", &bad).is_err());
        assert_eq!(vm.host().all_item_type_slots().unwrap(), Some(4));
        assert_eq!(vm.host().unit_item_type_slots(unit).unwrap(), Some(3));
        assert_eq!(vm.host().stock_unit_records().unwrap(), 1);
    }
    for bad in [vec![], vec![Value::Real(0.0)], vec![Value::Handle(unit)]] {
        assert!(vm.host().native("SetAllItemTypeSlots", &bad).is_err());
        assert_eq!(vm.host().all_item_type_slots().unwrap(), Some(4));
    }
    let mut zero = opened(0);
    assert_eq!(zero.host().all_item_type_slots().unwrap(), None);
    zero.init_globals().unwrap();
    zero.call("clear", &[]).unwrap();
    assert_eq!(zero.host().all_unit_type_slots().unwrap(), Some(0));
    zero.call("place", &[]).unwrap();
    let created = handle(&zero.global("U"));
    let refused = zero
        .host()
        .native("SetItemTypeSlots", &[Value::Handle(created), Value::Int(2)])
        .unwrap_err();
    assert!(refused.contains("stock unit slots"), "{refused}");
    assert_eq!(zero.host().unit_item_type_slots(created).unwrap(), None);
    assert_eq!(zero.host().all_unit_type_slots().unwrap(), Some(0));
    assert_eq!(zero.host().stock_unit_records().unwrap(), 0);
}

#[test]
fn stock_admission_rejects_override_source_mismatch_and_capacity() {
    let (host, _) = prepared(DECL, "// map", "synthetic:stock-once");
    let host = host
        .with_stock(StockBudget {
            max_records: 1,
            shop: shop(),
        })
        .unwrap();
    let again = match host.with_stock(StockBudget {
        max_records: 1,
        shop: shop(),
    }) {
        Err(error) => error,
        Ok(_) => panic!("stock was admitted twice"),
    };
    assert_eq!(again.failure, Failure::StockState("already admitted"));
    let (bare, _) = prepared(DECL, "// map", "synthetic:stock-off");
    let mut bare = bare;
    let unsupported = bare
        .native("SetAllItemTypeSlots", &[Value::Int(1)])
        .unwrap_err();
    assert!(
        unsupported.contains("unsupported") && unsupported.contains("SetAllItemTypeSlots"),
        "{unsupported}"
    );
    assert_eq!(
        bare.all_item_type_slots().unwrap_err().failure,
        Failure::StockState("not admitted")
    );
    let (overflow, _) = prepared(DECL, "// map", "synthetic:stock-overflow");
    let overflow = match overflow.with_stock(StockBudget {
        max_records: usize::MAX,
        shop: shop(),
    }) {
        Err(error) => error,
        Ok(_) => panic!("overflowing record bound was admitted"),
    };
    assert_eq!(overflow.failure, Failure::Capacity);
    let missing = DECL.replace(
        "native SetAllUnitTypeSlots takes integer slots returns nothing\n",
        "",
    );
    let (host, _) = prepared(&missing, "// map", "synthetic:stock-missing");
    let missing = match host.with_stock(StockBudget {
        max_records: 1,
        shop: shop(),
    }) {
        Err(error) => error,
        Ok(_) => panic!("missing declaration was admitted"),
    };
    assert_eq!(missing.failure, Failure::Signature);
    let constant = DECL.replace(
        "native SetAllItemTypeSlots",
        "constant native SetAllItemTypeSlots",
    );
    let (host, _) = prepared(&constant, "// map", "synthetic:stock-constant");
    let constant = match host.with_stock(StockBudget {
        max_records: 1,
        shop: shop(),
    }) {
        Err(error) => error,
        Ok(_) => panic!("constant declaration was admitted"),
    };
    assert_eq!(constant.failure, Failure::Signature);
    let wrong = DECL.replace(
        "native SetAllItemTypeSlots takes integer slots returns nothing",
        "native SetAllItemTypeSlots takes integer slots returns integer",
    );
    let (host, _) = prepared(&wrong, "// map", "synthetic:stock-signature");
    let wrong = match host.with_stock(StockBudget {
        max_records: 1,
        shop: shop(),
    }) {
        Err(error) => error,
        Ok(_) => panic!("wrong declaration was admitted"),
    };
    assert_eq!(wrong.failure, Failure::Signature);
    let (host, _) = prepared(
        DECL,
        "function SetAllItemTypeSlots takes integer slots returns nothing\nendfunction\n",
        "synthetic:stock-override",
    );
    let overridden = match host.with_stock(StockBudget {
        max_records: 1,
        shop: shop(),
    }) {
        Err(error) => error,
        Ok(_) => panic!("function override was admitted"),
    };
    assert_eq!(overridden.failure, Failure::Signature);
    let foreign = DECL
        .replace(
            "native SetAllItemTypeSlots takes integer slots returns nothing\n",
            "",
        )
        .replace(
            "native SetAllUnitTypeSlots takes integer slots returns nothing\n",
            "",
        )
        .replace(
            "native SetItemTypeSlots takes unit whichUnit, integer slots returns nothing\n",
            "",
        )
        .replace(
            "native SetUnitTypeSlots takes unit whichUnit, integer slots returns nothing\n",
            "",
        );
    let map = "native SetAllItemTypeSlots takes integer slots returns nothing\nnative SetAllUnitTypeSlots takes integer slots returns nothing\nnative SetItemTypeSlots takes unit whichUnit, integer slots returns nothing\nnative SetUnitTypeSlots takes unit whichUnit, integer slots returns nothing\n";
    let (host, _) = prepared(&foreign, map, "synthetic:stock-source");
    let source = match host.with_stock(StockBudget {
        max_records: 1,
        shop: shop(),
    }) {
        Err(error) => error,
        Ok(_) => panic!("foreign source was admitted"),
    };
    assert_eq!(source.failure, Failure::Source);
}
