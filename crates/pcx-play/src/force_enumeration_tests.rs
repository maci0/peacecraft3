use super::{bound, common, host, id};
use crate::playercolor_host::{FilterBudget, ObjectHost};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;
fn player(h: &mut ObjectHost, number: i32) -> Value {
    h.native("Player", &[Value::Int(number)]).unwrap()
}
fn force(h: &mut ObjectHost) -> Value {
    h.native("CreateForce", &[]).unwrap()
}
fn add(h: &mut ObjectHost, f: &Value, p: &Value) {
    h.native("ForceAddPlayer", &[f.clone(), p.clone()]).unwrap();
}
fn enumerate(h: &mut ObjectHost, f: &Value) -> Result<Value, String> {
    h.native("ForceEnumPlayers", &[f.clone(), Value::Null])
}
#[test]
fn owned_vm_null_replacement_uses_only_explicit_players_and_preserves_other_forces() {
    let map = "globals\nplayer A = Player(0)\nplayer B = Player(1)\nplayer C = Player(2)\nforce F = null\nforce Same = null\nforce G = null\nfilterfunc P = null\ninteger hits = 0\nendglobals\nfunction pred takes nothing returns boolean\nset hits = hits + 1\nreturn true\nendfunction\nfunction setup takes nothing returns nothing\nset F = CreateForce()\nset Same = F\nset G = CreateForce()\ncall ForceAddPlayer(F,A)\ncall ForceAddPlayer(F,C)\ncall ForceAddPlayer(G,C)\nset P = Filter(function pred)\nendfunction\nfunction enumerate takes force f returns nothing\ncall ForceEnumPlayers(f,null)\nendfunction\n";
    let h = host(&common(), map, 32, true)
        .with_forces(bound(2, 3))
        .unwrap()
        .with_filters(FilterBudget { max_records: 1 })
        .unwrap();
    let ctx = Arc::clone(h.player_context().unwrap());
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let (a, b, c, f, same, g, p) = (
        vm.global("A"),
        vm.global("B"),
        vm.global("C"),
        vm.global("F"),
        vm.global("Same"),
        vm.global("G"),
        vm.global("P"),
    );
    let handles = vm.host().inner().handles();
    vm.host()
        .admit_startup_force_players(&ctx, &[id(&b), id(&a)])
        .unwrap();
    for _ in 0..2 {
        vm.call("enumerate", std::slice::from_ref(&same)).unwrap();
    }
    assert_eq!(vm.host().force_members(id(&f)).unwrap(), &[id(&b), id(&a)]);
    assert_eq!(vm.host().force_members(id(&g)).unwrap(), &[id(&c)]);
    assert!(
        vm.host()
            .native("ForceEnumPlayers", &[f.clone(), p])
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(vm.global("hits"), Value::Int(0));
    assert!(
        vm.host()
            .admit_startup_force_players(&ctx, &[id(&a), id(&a)])
            .is_err()
    );
    enumerate(vm.host(), &f).unwrap();
    assert_eq!(vm.host().force_members(id(&f)).unwrap(), &[id(&b), id(&a)]);
    assert_eq!(vm.host().inner().handles(), handles);
    assert!(
        vm.host()
            .player_settings(id(&a))
            .unwrap()
            .controller
            .is_none()
    );
}
#[test]
fn aggregate_budget_is_checked_for_final_replacement_and_empty_is_not_invalidated() {
    let mut h = host(&common(), "// map", 16, true)
        .with_forces(bound(2, 3))
        .unwrap();
    let (a, b, c) = (player(&mut h, 0), player(&mut h, 1), player(&mut h, 2));
    let (f, g) = (force(&mut h), force(&mut h));
    add(&mut h, &f, &a);
    add(&mut h, &g, &a);
    add(&mut h, &g, &c);
    let ctx = h.player_context().unwrap().clone();
    h.admit_startup_force_players(&ctx, &[id(&b), id(&c)])
        .unwrap();
    assert!(
        enumerate(&mut h, &f)
            .unwrap_err()
            .contains("force memberships")
    );
    assert_eq!(h.force_members(id(&f)).unwrap(), &[id(&a)]);
    assert_eq!(h.force_members(id(&g)).unwrap(), &[id(&a), id(&c)]);
    h.admit_startup_force_players(&ctx, &[]).unwrap();
    enumerate(&mut h, &f).unwrap();
    assert!(h.force_members(id(&f)).unwrap().is_empty());
    add(&mut h, &f, &b);
    assert!(h.native("ForceAddPlayer", &[f.clone(), c.clone()]).is_err());
    h.admit_startup_force_players(&ctx, &[id(&c)]).unwrap();
    enumerate(&mut h, &f).unwrap();
    assert_eq!(h.force_members(id(&f)).unwrap(), &[id(&c)]);
    h.invalidate_force_enumeration();
    assert!(enumerate(&mut h, &f).unwrap_err().contains("unsupported"));
    assert_eq!(h.force_members(id(&f)).unwrap(), &[id(&c)]);
    let mut zero = host(&common(), "// map", 8, true)
        .with_forces(bound(1, 0))
        .unwrap();
    let f = force(&mut zero);
    let ctx = zero.player_context().unwrap().clone();
    zero.admit_startup_force_players(&ctx, &[]).unwrap();
    enumerate(&mut zero, &f).unwrap();
    assert!(zero.force_members(id(&f)).unwrap().is_empty());
}
#[test]
fn controller_writes_invalidate_only_after_acceptance_and_fresh_input_recovers() {
    let mut h = host(&common(), "// map", 16, true)
        .with_forces(bound(1, 2))
        .unwrap();
    let a = player(&mut h, 0);
    let f = force(&mut h);
    let timer = h.native("CreateTimer", &[]).unwrap();
    let ctx = h.player_context().unwrap().clone();
    h.admit_startup_force_players(&ctx, &[id(&a)]).unwrap();
    assert!(
        h.native("SetPlayerController", &[a.clone(), timer])
            .is_err()
    );
    enumerate(&mut h, &f).unwrap();
    let control = h.native("ConvertMapControl", &[Value::Int(91)]).unwrap();
    h.native("SetPlayerController", &[a.clone(), control])
        .unwrap();
    assert!(enumerate(&mut h, &f).unwrap_err().contains("unsupported"));
    assert_eq!(h.force_members(id(&f)).unwrap(), &[id(&a)]);
    h.admit_startup_force_players(&ctx, &[]).unwrap();
    enumerate(&mut h, &f).unwrap();
    assert!(h.force_members(id(&f)).unwrap().is_empty());
}
#[test]
fn context_contract_and_identity_refusals_preserve_admitted_input_and_destination() {
    use super::super::{budget, context, enums, slots_rule};
    let c = common();
    let ctx = context(301, &c, "// map", "synthetic:force-enumeration");
    let mut inner = enums(&ctx, 32);
    let reserved = inner.reserve_identity("player").unwrap();
    let mut h = ObjectHost::checked(inner)
        .unwrap()
        .with_players(ctx.clone(), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_forces(bound(1, 1))
        .unwrap();
    let a = player(&mut h, 0);
    let b = player(&mut h, 1);
    let f = force(&mut h);
    let timer = h.native("CreateTimer", &[]).unwrap();
    h.admit_startup_force_players(&ctx, &[id(&a)]).unwrap();
    let other = context(301, &c, "// map", "synthetic:force-enumeration");
    assert!(h.admit_startup_force_players(&other, &[id(&b)]).is_err());
    let unknown = HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    for ids in [
        vec![id(&a), id(&b)],
        vec![reserved],
        vec![unknown],
        vec![id(&timer)],
    ] {
        assert!(h.admit_startup_force_players(&ctx, &ids).is_err());
    }
    enumerate(&mut h, &f).unwrap();
    let handles = h.inner().handles();
    for args in [
        vec![],
        vec![f.clone()],
        vec![Value::Null, Value::Null],
        vec![timer, Value::Null],
        vec![Value::Handle(unknown), Value::Null],
        vec![f.clone(), Value::Int(0)],
    ] {
        assert!(h.native("ForceEnumPlayers", &args).is_err());
        assert_eq!(h.force_members(id(&f)).unwrap(), &[id(&a)]);
        assert_eq!(h.inner().handles(), handles);
    }
    let declaration = "native ForceEnumPlayers takes force f, boolexpr b returns nothing";
    for c in [
        c.replace(declaration, ""),
        c.replace(declaration, &format!("constant {declaration}")),
        c.replace(
            "ForceEnumPlayers takes force f, boolexpr b",
            "ForceEnumPlayers takes force f, integer b",
        ),
        c.replace(
            "ForceEnumPlayers takes force f, boolexpr b returns nothing",
            "ForceEnumPlayers takes force f, boolexpr b returns boolean",
        ),
    ] {
        let mut h = host(&c, "// map", 8, true)
            .with_forces(bound(1, 1))
            .unwrap();
        let ctx = h.player_context().unwrap().clone();
        assert!(h.admit_startup_force_players(&ctx, &[]).is_err());
    }
    for map in [
        format!("{declaration}\n"),
        "type boolexpr extends agent\n".into(),
        "function ForceEnumPlayers takes force f, boolexpr b returns nothing\nendfunction\n".into(),
    ] {
        let mut h = host(&c, &map, 8, true).with_forces(bound(1, 1)).unwrap();
        let ctx = h.player_context().unwrap().clone();
        assert!(h.admit_startup_force_players(&ctx, &[]).is_err());
    }
    let moved = c.replace(declaration, "");
    let mut h = host(&moved, declaration, 8, true)
        .with_forces(bound(1, 1))
        .unwrap();
    let ctx = h.player_context().unwrap().clone();
    assert!(h.admit_startup_force_players(&ctx, &[]).is_err());
    let mut h = host(&c, "// map", 8, true);
    let ctx = h.player_context().unwrap().clone();
    assert!(h.admit_startup_force_players(&ctx, &[]).is_err());
}
