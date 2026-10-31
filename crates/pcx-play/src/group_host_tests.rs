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

fn target_common() -> String {
    let globals = COMMON.find("\nglobals\n").expect("globals");
    let mut common = COMMON.to_string();
    common.insert_str(
        globals + 1,
        "type player extends agent\nnative CreateUnit takes player p, integer id, real x, real y, real face returns unit\nconstant native OrderId takes string s returns integer\nconstant native OrderId2String takes integer n returns string\nnative TimerStart takes timer t, real timeout, boolean periodic, code handler returns nothing\nnative TimerGetElapsed takes timer t returns real\nnative TimerGetRemaining takes timer t returns real\nnative TimerGetTimeout takes timer t returns real\nnative GetExpiredTimer takes nothing returns timer\nnative GroupTargetOrder takes group g, string s, widget w returns boolean\nnative GroupTargetOrderById takes group g, integer n, widget w returns boolean\nnative IssueTargetOrder takes unit u, string s, widget w returns boolean\nnative IssueTargetOrderById takes unit u, integer n, widget w returns boolean\n",
    );
    common
}

struct UnusedCatalog;

impl pcx_sim::world::ObjectCatalog for UnusedCatalog {
    fn resolve(
        &self,
        _: [u8; 4],
    ) -> Result<pcx_sim::world::Definition, pcx_sim::world::ResolutionProblem> {
        Err(pcx_sim::world::ResolutionProblem::MissingRawcode(
            "unused".into(),
        ))
    }
}

fn target_activity_rules() -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    use crate::{camera_margins::SourceWindow, neutral_player::sha256};
    let (mut image, mut binding, budget) = crate::order_registry::tests::fixture([431, 744]);
    let third = 0x220 + 9 + 15 + 17;
    let patrol_id = 2122i32;
    image[third + 1..third + 5].copy_from_slice(&0x301480u32.to_le_bytes());
    image[third + 6..third + 10].copy_from_slice(&patrol_id.to_le_bytes());
    image[0x680..0x686].copy_from_slice(b"delta\0");
    binding.initializer.digest = sha256(&image[0x220..0x220 + binding.initializer.len as usize]);
    let mut put = |at: usize, prefix: &[u8], id: i32| {
        let mut bytes = prefix.to_vec();
        bytes.extend_from_slice(&id.to_le_bytes());
        image[at..at + bytes.len()].copy_from_slice(&bytes);
        SourceWindow {
            va: 0x301000 + at as u32 - 0x200,
            len: bytes.len() as u32,
            digest: sha256(&bytes),
        }
    };
    let hold = put(0x320, &[0x3d], 431);
    let stop = put(0x330, &[0x81, 0xfb], 744);
    let movement = put(0x340, &[0x3d], 1631);
    let patrol = put(0x350, &[0x3d], patrol_id);
    binding.image_sha256 = sha256(&image);
    let registry = Arc::new(
        crate::order_registry::Registry::read(&image, binding.clone(), budget, 4, 32).unwrap(),
    );
    let rules = crate::unit_activity::Rules::read(
        &image,
        crate::unit_activity::Binding {
            origin: binding.origin,
            image_sha256: binding.image_sha256,
            hold,
            stop,
            movement,
            patrol,
        },
        &registry,
        budget,
    )
    .unwrap()
    .with_follow_profiles(
        &[(
            "synthetic follow misc",
            b"[Misc]\nFollowRange=12\nStructureFollowRange=4\n",
        )],
        4096,
    )
    .unwrap();
    (registry, rules)
}

fn target_host() -> ObjectHost {
    let common = target_common();
    let mut host = timer_host(&common, "", 24)
        .with_groups(8)
        .unwrap()
        .with_unit_catalog(
            Arc::new(UnusedCatalog),
            pcx_sim::world::Source {
                install: "selected-install".into(),
                edition: "selected-edition".into(),
                map: "group-target".into(),
            },
            crate::playercolor_host::UnitBudget {
                max_records: 4,
                max_retained_fields: 4,
                max_modifications: 0,
            },
        )
        .unwrap()
        .with_timer_runtime(0.0)
        .unwrap();
    let (registry, rules) = target_activity_rules();
    let group_rules = crate::group_order_rules::tests::rules(registry.binding().clone(), 2);
    host = host.with_order_registry(registry).unwrap();
    host.admit_unit_activity(rules, 65536).unwrap();
    host.with_group_orders(group_rules).unwrap()
}

#[test]
fn target_orders_resolve_the_widget_before_the_group_and_reuse_follow() {
    let mut host = target_host();
    let group = create(&mut host);
    let timer = host.native("CreateTimer", &[]).unwrap();
    let widget = unit(&mut host);
    let other = unit(&mut host);
    host.native("GroupAddUnit", &[group.clone(), widget.clone()])
        .unwrap();
    host.native("GroupAddUnit", &[group.clone(), other.clone()])
        .unwrap();
    let handles = host.inner().handles();
    assert_eq!(
        host.native(
            "GroupTargetOrder",
            &[timer.clone(), Value::Str("missing".into()), Value::Null]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        host.native(
            "GroupTargetOrderById",
            &[timer.clone(), Value::Int(1631), Value::Null]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        host.native(
            "GroupTargetOrder",
            &[group.clone(), Value::Str("gamma".into()), timer.clone()]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        host.native(
            "GroupTargetOrderById",
            &[Value::Null, Value::Int(1631), widget.clone()]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert!(
        host.native(
            "GroupTargetOrderById",
            &[timer, Value::Int(1631), widget.clone()]
        )
        .is_err()
    );
    let unsupported = host
        .native(
            "GroupTargetOrderById",
            &[group.clone(), Value::Int(1), widget.clone()],
        )
        .unwrap_err();
    assert!(unsupported.contains("not supported"), "{unsupported}");
    assert_eq!(
        host.native(
            "GroupTargetOrder",
            &[group.clone(), Value::Str("gamma".into()), widget.clone()]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        host.native(
            "GroupTargetOrderById",
            &[group.clone(), Value::Int(1631), other]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert!(
        host.native(
            "GroupTargetOrder",
            &[group.clone(), Value::Str("gamma".into()), Value::Real(1.)]
        )
        .is_err()
    );
    assert_eq!(host.memberships(), 2);
    assert_eq!(host.inner().handles(), handles);
    assert_eq!(first(&mut host, &group), widget);
}

#[path = "group_iteration_tests.rs"]
mod iteration;
