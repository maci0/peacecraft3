use super::*;
use pcx_script::HandleId;
const DECLARATIONS: &str = r#"
native GroupImmediateOrder takes group g,string s returns boolean
native GroupImmediateOrderById takes group g,integer n returns boolean
native GroupPointOrder takes group g,string s,real x,real y returns boolean
native GroupPointOrderById takes group g,integer n,real x,real y returns boolean
native GroupPointOrderLoc takes group g,string s,location l returns boolean
native GroupPointOrderByIdLoc takes group g,integer n,location l returns boolean
"#;
const MAP: &str = "function setup takes nothing returns nothing\nendfunction\n";
fn host() -> ObjectHost {
    let declarations = format!(
        "{}\n{}\n{}",
        enumeration::TRIGGERS,
        activity::ORDERS,
        DECLARATIONS
    );
    let mut h = position_host_admitting(1000, 32, MAP, &declarations, &[], |h| h)
        .with_locations()
        .unwrap();
    let (registry, rules) = activity::rules();
    let group_rules = crate::group_order_rules::tests::rules(registry.binding().clone(), 2);
    h = h.with_order_registry(registry).unwrap();
    h.admit_unit_activity(rules, 65536).unwrap();
    h.with_group_orders(group_rules).unwrap()
}
fn unit(h: &mut ObjectHost, x: f32) -> Value {
    let p = h.native("Player", &[Value::Int(4)]).unwrap();
    h.native(
        "CreateUnit",
        &[
            p,
            Value::Int(i32::from_be_bytes(*b"U001")),
            Value::Real(x),
            Value::Real(16.),
            Value::Real(0.),
        ],
    )
    .unwrap()
}
fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("identity")
    };
    *id
}
fn group(h: &mut ObjectHost, units: &[Value]) -> Value {
    let g = h.native("CreateGroup", &[]).unwrap();
    for unit in units {
        h.native("GroupAddUnit", &[g.clone(), unit.clone()])
            .unwrap();
    }
    g
}
fn order(h: &mut ObjectHost, u: &Value) -> Value {
    h.native("GetUnitCurrentOrder", std::slice::from_ref(u))
        .unwrap()
}
#[test]
fn group_points_preserve_member_order_and_source_limit_but_stop_reaches_all() {
    let mut h = host();
    let a = unit(&mut h, 16.);
    let b = unit(&mut h, 32.);
    let c = unit(&mut h, 48.);
    let g = group(&mut h, &[b.clone(), a.clone(), c.clone()]);
    let handles = h.inner().handles();
    assert_eq!(
        h.native(
            "GroupPointOrder",
            &[
                g.clone(),
                Value::Str("gamma".into()),
                Value::Real(80.),
                Value::Real(16.)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(order(&mut h, &a), Value::Int(1631));
    assert_eq!(order(&mut h, &b), Value::Int(1631));
    assert_eq!(order(&mut h, &c), Value::Int(0));
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                c.clone(),
                Value::Str("gamma".into()),
                Value::Real(80.),
                Value::Real(16.)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        h.native("GroupImmediateOrderById", &[g, Value::Int(744)])
            .unwrap(),
        Value::Bool(true)
    );
    for u in [&a, &b, &c] {
        assert_eq!(order(&mut h, u), Value::Int(0));
    }
    assert_eq!(h.inner().handles(), handles);
}
#[test]
fn group_location_and_numeric_variants_use_the_same_current_location() {
    let mut h = host();
    let a = unit(&mut h, 16.);
    let g = group(&mut h, std::slice::from_ref(&a));
    let loc = h
        .native("Location", &[Value::Real(72.), Value::Real(16.)])
        .unwrap();
    for (name, args) in [
        (
            "GroupPointOrderLoc",
            vec![g.clone(), Value::Str("gamma".into()), loc.clone()],
        ),
        (
            "GroupPointOrderByIdLoc",
            vec![g.clone(), Value::Int(1631), loc.clone()],
        ),
        (
            "GroupPointOrderById",
            vec![
                g.clone(),
                Value::Int(1631),
                Value::Real(72.),
                Value::Real(16.),
            ],
        ),
    ] {
        assert_eq!(h.native(name, &args).unwrap(), Value::Bool(true));
        assert_eq!(
            h.executed_unit(id(&a))
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .destination(),
            Some([72., 16.])
        );
    }
    assert_eq!(
        h.native("GroupImmediateOrder", &[g, Value::Str("alpha".into())])
            .unwrap(),
        Value::Bool(true)
    );
    assert!(
        h.executed_unit(id(&a))
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .held()
    );
}
#[test]
fn empty_null_invalid_and_unknown_group_orders_do_not_mutate_members() {
    let mut h = host();
    let a = unit(&mut h, 16.);
    let g = group(&mut h, std::slice::from_ref(&a));
    let empty = group(&mut h, &[]);
    for group in [empty, Value::Null] {
        assert_eq!(
            h.native("GroupImmediateOrder", &[group, Value::Str("beta".into())])
                .unwrap(),
            Value::Bool(false)
        );
    }
    assert_eq!(
        h.native(
            "GroupImmediateOrder",
            &[g.clone(), Value::Str("missing".into())]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native(
            "GroupPointOrderLoc",
            &[g.clone(), Value::Str("gamma".into()), Value::Null]
        )
        .unwrap(),
        Value::Bool(false)
    );
    for args in [
        vec![a.clone(), Value::Str("beta".into())],
        vec![g.clone(), Value::Real(1.)],
    ] {
        assert!(h.native("GroupImmediateOrder", &args).is_err());
    }
    assert!(
        h.native(
            "GroupPointOrderLoc",
            &[g.clone(), Value::Str("gamma".into()), a.clone()]
        )
        .is_err()
    );
    let loc = h
        .native("Location", &[Value::Real(72.), Value::Real(16.)])
        .unwrap();
    h.native("RemoveLocation", std::slice::from_ref(&loc))
        .unwrap();
    assert!(
        h.native(
            "GroupPointOrderLoc",
            &[g.clone(), Value::Str("gamma".into()), loc]
        )
        .is_err()
    );
    assert!(
        h.native(
            "GroupPointOrder",
            &[
                g,
                Value::Str("gamma".into()),
                Value::Real(f32::NAN),
                Value::Real(0.)
            ]
        )
        .is_err()
    );
    assert_eq!(order(&mut h, &a), Value::Int(0));
}
#[test]
fn group_orders_admit_once_and_only_after_group_activity_and_registry() {
    let h = host();
    let (registry, _) = activity::rules();
    assert!(
        h.with_group_orders(crate::group_order_rules::tests::rules(
            registry.binding().clone(),
            2
        ))
        .is_err()
    );
    let declarations = format!(
        "{}\n{}\n{}",
        enumeration::TRIGGERS,
        activity::ORDERS,
        DECLARATIONS
    );
    let bare = position_host_admitting(1000, 32, MAP, &declarations, &[], |h| h);
    assert!(
        bare.with_group_orders(crate::group_order_rules::tests::rules(
            registry.binding().clone(),
            2
        ))
        .is_err()
    );
}
