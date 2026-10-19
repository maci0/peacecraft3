use super::{EnumHost, Failure, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Prelude, Value};
use std::sync::Arc;

const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype group extends agent\ntype widget extends agent\ntype unit extends widget\ntype specialunit extends unit\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateGroup takes nothing returns group\nnative DestroyGroup takes group g returns nothing\nnative GroupClear takes group g returns nothing\nnative FirstOfGroup takes group g returns unit\nnative GroupAddUnit takes group g, unit u returns nothing\nnative GroupRemoveUnit takes group g, unit u returns nothing\nconstant native IsUnitInGroup takes unit u, group g returns boolean\nglobals\nconstant playercolor C = ConvertPlayerColor(53)\nendglobals\n";
fn timer_host(common: &str, map: &str, handles: usize) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let p = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    let e = EnumHost::checked_selected(
        p,
        "synthetic:selected-map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: handles,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(e).unwrap()
}
fn host(handles: usize, members: usize) -> ObjectHost {
    timer_host(COMMON, "", handles)
        .with_groups(members)
        .unwrap()
}
fn create(h: &mut ObjectHost) -> Value {
    h.native("CreateGroup", &[]).unwrap()
}
fn unit(h: &mut ObjectHost) -> Value {
    Value::Handle(h.inner.reserve_identity("unit").unwrap())
}
fn first(h: &mut ObjectHost, g: &Value) -> Value {
    h.native("FirstOfGroup", std::slice::from_ref(g)).unwrap()
}
#[test]
fn groups_require_explicit_admission_and_share_timer_and_enum_identity_budget() {
    let mut h = timer_host(COMMON, "", 3);
    assert!(
        h.native("CreateGroup", &[])
            .unwrap_err()
            .contains("unsupported")
    );
    let mut h = h.with_groups(2).unwrap();
    let g = create(&mut h);
    let t = h.native("CreateTimer", &[]).unwrap();
    let c = h.native("ConvertPlayerColor", &[Value::Int(53)]).unwrap();
    assert_ne!(g, t);
    assert_ne!(g, c);
    assert!(
        h.native("CreateGroup", &[])
            .unwrap_err()
            .contains("capacity")
    );
    assert_eq!(h.live_groups(), 1);
    assert_eq!(h.live_timers(), 1);
    assert_eq!(first(&mut h, &g), Value::Null);
}
#[test]
fn membership_is_unique_and_removal_preserves_other_group_order() {
    let mut h = host(10, 6);
    let g = create(&mut h);
    let other = create(&mut h);
    let a = unit(&mut h);
    let b = unit(&mut h);
    let c = Value::Handle(h.inner.reserve_identity("specialunit").unwrap());
    for (group, member) in [(&g, &b), (&other, &a), (&g, &a), (&g, &c), (&g, &b)] {
        assert_eq!(
            h.native("GroupAddUnit", &[group.clone(), member.clone()])
                .unwrap(),
            Value::Null
        );
    }
    assert_eq!(h.memberships(), 4);
    assert_eq!(first(&mut h, &g), b);
    assert_eq!(first(&mut h, &other), a);
    assert_eq!(
        h.native("IsUnitInGroup", &[b.clone(), other.clone()])
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native("IsUnitInGroup", &[b.clone(), g.clone()]).unwrap(),
        Value::Bool(true)
    );
    h.native("GroupRemoveUnit", &[g.clone(), b.clone()])
        .unwrap();
    h.native("GroupRemoveUnit", &[g.clone(), b]).unwrap();
    assert_eq!(first(&mut h, &g), a);
    assert_eq!(first(&mut h, &other), a);
    assert_eq!(h.memberships(), 3);
    h.native("GroupRemoveUnit", &[g.clone(), a]).unwrap();
    assert_eq!(first(&mut h, &g), c);
}
#[test]
fn clear_and_destroy_release_memberships_without_destroying_units_or_other_groups() {
    let mut h = host(8, 4);
    let a = unit(&mut h);
    let g = create(&mut h);
    let other = create(&mut h);
    h.native("GroupAddUnit", &[g.clone(), a.clone()]).unwrap();
    h.native("GroupAddUnit", &[other.clone(), a.clone()])
        .unwrap();
    h.native("GroupClear", std::slice::from_ref(&g)).unwrap();
    assert_eq!(first(&mut h, &g), Value::Null);
    assert_eq!(first(&mut h, &other), a);
    assert_eq!(h.live_groups(), 2);
    h.native("GroupAddUnit", &[g.clone(), a.clone()]).unwrap();
    h.native("DestroyGroup", std::slice::from_ref(&g)).unwrap();
    assert_eq!(h.live_groups(), 1);
    assert_eq!(h.memberships(), 1);
    assert!(
        h.native("FirstOfGroup", std::slice::from_ref(&g))
            .unwrap_err()
            .contains("unknown handle")
    );
    assert!(h.native("DestroyGroup", std::slice::from_ref(&g)).is_err());
    let new = create(&mut h);
    assert_ne!(new, g);
    assert_eq!(first(&mut h, &new), Value::Null);
    let Value::Handle(id) = a else {
        panic!("unit identity")
    };
    assert_eq!(h.inner().kind(id).unwrap(), "unit");
}
#[test]
fn membership_budget_failure_is_atomic_duplicates_do_not_consume_budget() {
    let mut h = host(8, 1);
    let g = create(&mut h);
    let other = create(&mut h);
    let a = unit(&mut h);
    let b = unit(&mut h);
    h.native("GroupAddUnit", &[g.clone(), a.clone()]).unwrap();
    h.native("GroupAddUnit", &[g.clone(), a.clone()]).unwrap();
    assert!(
        h.native("GroupAddUnit", &[other.clone(), b.clone()])
            .unwrap_err()
            .contains("group membership")
    );
    assert_eq!(first(&mut h, &g), a);
    assert_eq!(first(&mut h, &other), Value::Null);
    assert_eq!(h.memberships(), 1);
    h.native("GroupClear", &[g]).unwrap();
    h.native("GroupAddUnit", &[other.clone(), b.clone()])
        .unwrap();
    assert_eq!(first(&mut h, &other), b);
    assert!(timer_host(COMMON, "", 2).with_groups(usize::MAX).is_err());
    let mut zero = host(2, 0);
    let g = create(&mut zero);
    let a = unit(&mut zero);
    assert!(zero.native("GroupAddUnit", &[g.clone(), a]).is_err());
    assert_eq!(first(&mut zero, &g), Value::Null);
}
#[test]
fn null_wrong_family_unknown_and_unowned_group_handles_refuse_without_mutation() {
    let mut h = host(10, 4);
    let g = create(&mut h);
    let a = unit(&mut h);
    let timer = h.native("CreateTimer", &[]).unwrap();
    let unowned = Value::Handle(h.inner.reserve_identity("group").unwrap());
    let unknown = Value::Handle(HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap()));
    for args in [
        vec![],
        vec![g.clone()],
        vec![g.clone(), Value::Null],
        vec![g.clone(), timer.clone()],
        vec![timer, a.clone()],
        vec![unowned, a.clone()],
        vec![unknown, a.clone()],
    ] {
        assert!(h.native("GroupAddUnit", &args).is_err());
    }
    assert!(h.native("CreateGroup", &[Value::Null]).is_err());
    assert!(h.native("GroupClear", &[Value::Null]).is_err());
    assert!(
        h.native("GroupEnumUnitsInRect", &[])
            .unwrap_err()
            .contains("unsupported")
    );
    assert!(
        h.native("ForGroup", &[])
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(h.memberships(), 0);
    assert_eq!(first(&mut h, &g), Value::Null);
}
#[test]
fn incompatible_group_native_shape_type_or_function_override_refuses_admission() {
    for common in [
        COMMON.replace(
            "GroupAddUnit takes group g, unit u returns nothing",
            "GroupAddUnit takes group g, unit u returns boolean",
        ),
        COMMON.replace(
            "FirstOfGroup takes group g returns unit",
            "FirstOfGroup takes group g returns timer",
        ),
        COMMON.replace("constant native IsUnitInGroup", "native IsUnitInGroup"),
        COMMON.replace("type group extends agent", "type group extends handle"),
        COMMON.replace("type unit extends widget", "type unit extends agent"),
        COMMON.replace(
            "GroupRemoveUnit takes group g, unit u",
            "GroupRemoveUnit takes group g",
        ),
    ] {
        assert!(timer_host(&common, "", 4).with_groups(4).is_err());
    }
    let h = timer_host(
        COMMON,
        "function CreateGroup takes nothing returns group\nreturn null\nendfunction\n",
        4,
    );
    assert!(matches!(h.with_groups(4), Err(e) if e.failure == Failure::Signature));
}
#[test]
fn vm_group_and_timer_globals_keep_aliases_and_sticky_failure_in_one_program() {
    let common = format!("{COMMON}native Next takes nothing returns integer\n");
    let map = "globals\ngroup G = CreateGroup()\ngroup A = G\ntimer T = CreateTimer()\nunit first = FirstOfGroup(G)\ninteger failure = Next()\nendglobals\n";
    let mut vm = timer_host(&common, map, 8)
        .with_groups(4)
        .unwrap()
        .into_vm();
    let first = vm.init_globals().unwrap_err();
    assert!(first.to_string().contains("Next"));
    assert_eq!(vm.global("A"), vm.global("G"));
    assert_ne!(vm.global("G"), vm.global("T"));
    assert_eq!(vm.global("first"), Value::Null);
    assert_eq!(vm.host().live_groups(), 1);
    assert_eq!(vm.host().live_timers(), 1);
    let count = vm.host().inner().handles();
    assert_eq!(first, vm.init_globals().unwrap_err());
    assert_eq!(vm.host().inner().handles(), count);
}
#[test]
fn vm_drains_caller_reserved_unit_identities_without_callback_stubs() {
    let map = "globals\ngroup G = CreateGroup()\nunit first = null\nendglobals\nfunction take takes nothing returns nothing\nset first = FirstOfGroup(G)\ncall GroupRemoveUnit(G, first)\nendfunction\n";
    let mut h = timer_host(COMMON, map, 8).with_groups(4).unwrap();
    let a = unit(&mut h);
    let b = unit(&mut h);
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    let g = vm.global("G");
    vm.host()
        .native("GroupAddUnit", &[g.clone(), a.clone()])
        .unwrap();
    vm.host()
        .native("GroupAddUnit", &[g.clone(), b.clone()])
        .unwrap();
    vm.call("take", &[]).unwrap();
    assert_eq!(vm.global("first"), a);
    vm.call("take", &[]).unwrap();
    assert_eq!(vm.global("first"), b);
    assert_eq!(first(vm.host(), &g), Value::Null);
}

#[test]
fn membership_layout_refuses_platform_overflow_without_allocating_the_limit() {
    let count = (u32::MAX as usize / 2) / std::mem::size_of::<(HandleId, HandleId)>() + 1;
    let result = timer_host(COMMON, "", 4).with_groups(count);
    assert_eq!(result.is_err(), usize::BITS == 32);
}

#[path = "group_iteration_tests.rs"]
mod iteration;
