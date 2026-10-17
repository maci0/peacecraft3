use super::{COMMON, budget, context, enums, id, slots_rule};
use crate::playercolor_host::{ForceBudget, ObjectHost};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;
const CREATE: &str = "native CreateForce takes nothing returns force";
const ADD: &str = "native ForceAddPlayer takes force f, player p returns nothing";
fn common() -> String {
    format!(
        "{COMMON}\ntype force extends agent\n{CREATE}\n{ADD}\nnative DestroyForce takes force f returns nothing\nnative ForceEnumPlayers takes force f, boolexpr b returns nothing\ntype boolexpr extends agent\ntype filterfunc extends boolexpr\nnative Filter takes code c returns filterfunc\n"
    )
}
fn host(c: &str, map: &str, handles: usize, players: bool) -> ObjectHost {
    let ctx = context(301, c, map, "synthetic:forces");
    let host = ObjectHost::checked(enums(&ctx, handles)).unwrap();
    if players {
        host.with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
            .unwrap()
    } else {
        host
    }
}
fn bound(records: usize, members: usize) -> ForceBudget {
    ForceBudget {
        max_records: records,
        max_memberships: members,
    }
}
#[test]
fn owned_vm_keeps_distinct_forces_and_alias_memberships_without_callbacks() {
    let c = common();
    let map = "globals\nplayer A = Player(0)\nplayer Alias = Player(0)\nplayer B = Player(1)\nforce F = null\nforce Same = null\nforce G = null\ninteger hits = 0\nendglobals\nfunction pred takes nothing returns boolean\nset hits = hits + 1\nreturn true\nendfunction\nfunction setup takes nothing returns nothing\nset F = CreateForce()\nset Same = F\nset G = CreateForce()\ncall ForceAddPlayer(F,A)\ncall ForceAddPlayer(G,B)\ncall ForceAddPlayer(Same,Alias)\nendfunction\n";
    let host = host(&c, map, 16, true)
        .with_forces(bound(2, 2))
        .unwrap()
        .with_filters(crate::playercolor_host::FilterBudget { max_records: 1 })
        .unwrap();
    drop(c);
    let mut vm = host.into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let f = vm.global("F");
    let same = vm.global("Same");
    let g = vm.global("G");
    let a = vm.global("A");
    let b = vm.global("B");
    assert_eq!(f, same);
    assert_ne!(f, g);
    assert_eq!(vm.host().force_members(id(&f)).unwrap(), &[id(&a)]);
    assert_eq!(vm.host().force_members(id(&g)).unwrap(), &[id(&b)]);
    let function = vm.program().function_id("pred").unwrap();
    let filter = vm
        .host()
        .native("Filter", &[Value::Code(function)])
        .unwrap();
    let timer = vm.host().native("CreateTimer", &[]).unwrap();
    for other in [&a, &b, &filter, &timer, &g] {
        assert_ne!(&f, other);
    }
    assert_eq!(vm.host().inner().kind(id(&f)).unwrap(), "force");
    let handles = vm.host().inner().handles();
    for args in [vec![f.clone(), b.clone()], vec![g.clone(), a.clone()]] {
        assert!(vm.host().native("ForceAddPlayer", &args).is_err());
    }
    assert_eq!(vm.host().force_members(id(&f)).unwrap(), &[id(&a)]);
    assert_eq!(vm.host().force_members(id(&g)).unwrap(), &[id(&b)]);
    assert_eq!(vm.host().inner().handles(), handles);
    assert_eq!(vm.global("hits"), Value::Int(0));
    for (name, args) in [
        ("ForceEnumPlayers", vec![f.clone(), filter]),
        ("DestroyForce", vec![f.clone()]),
    ] {
        assert!(
            vm.host()
                .native(name, &args)
                .unwrap_err()
                .contains("unsupported")
        );
    }
    assert_eq!(vm.host().force_members(id(&f)).unwrap(), &[id(&a)]);
}
#[test]
fn invalid_force_or_player_never_adopts_generic_objects_or_changes_membership() {
    let ctx = context(301, &common(), "// map", "synthetic:reserved-forces");
    let mut inner = enums(&ctx, 16);
    let reserved_force = Value::Handle(inner.reserve_identity("force").unwrap());
    let reserved_player = Value::Handle(inner.reserve_identity("player").unwrap());
    let mut h = ObjectHost::checked(inner)
        .unwrap()
        .with_players(ctx, slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_forces(bound(2, 2))
        .unwrap();
    let a = h.native("Player", &[Value::Int(0)]).unwrap();
    let f = h.native("CreateForce", &[]).unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    let unknown = Value::Handle(HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap()));
    h.native("ForceAddPlayer", &[f.clone(), a.clone()]).unwrap();
    let handles = h.inner().handles();
    for bad in [&reserved_force, &reserved_player, &timer, &unknown] {
        assert!(h.force_members(id(bad)).is_err());
    }
    for args in [
        vec![],
        vec![f.clone()],
        vec![Value::Null, a.clone()],
        vec![reserved_force, a.clone()],
        vec![unknown.clone(), a.clone()],
        vec![f.clone(), reserved_player],
        vec![f.clone(), timer],
        vec![f.clone(), unknown],
        vec![f.clone(), Value::Int(0)],
    ] {
        assert!(h.native("ForceAddPlayer", &args).is_err());
        assert_eq!(h.force_members(id(&f)).unwrap(), &[id(&a)]);
        assert_eq!(h.inner().handles(), handles);
    }
    assert!(h.native("CreateForce", &[Value::Null]).is_err());
    let g = h.native("CreateForce", &[]).unwrap();
    assert!(h.force_members(id(&g)).unwrap().is_empty());
    h.native("ForceAddPlayer", &[g.clone(), a.clone()]).unwrap();
    assert_eq!(h.force_members(id(&g)).unwrap(), &[id(&a)]);
}
#[test]
fn record_membership_and_shared_arena_limits_refuse_atomically() {
    for records in [0, 1] {
        let mut h = host(&common(), "// map", 8, true)
            .with_forces(bound(records, 0))
            .unwrap();
        if records == 1 {
            h.native("CreateForce", &[]).unwrap();
        }
        let handles = h.inner().handles();
        assert!(
            h.native("CreateForce", &[])
                .unwrap_err()
                .contains("force records")
        );
        assert_eq!(h.inner().handles(), handles);
    }
    let mut h = host(&common(), "// map", 4, true)
        .with_forces(bound(1, 0))
        .unwrap();
    let f = h.native("CreateForce", &[]).unwrap();
    let a = h.native("Player", &[Value::Int(0)]).unwrap();
    assert!(
        h.native("ForceAddPlayer", &[f.clone(), a])
            .unwrap_err()
            .contains("force memberships")
    );
    assert!(h.force_members(id(&f)).unwrap().is_empty());
    let mut h = host(&common(), "// map", 4, true)
        .with_forces(bound(2, 1))
        .unwrap();
    for _ in 0..3 {
        h.native("CreateTimer", &[]).unwrap();
    }
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(
        h.native("CreateForce", &[])
            .unwrap_err()
            .contains("capacity")
    );
    h.native("DestroyTimer", &[timer]).unwrap();
    assert!(h.native("CreateForce", &[]).is_err());
    assert_eq!(h.inner().handles(), 4);
}
#[test]
fn admission_requires_players_and_loaded_contracts_and_ancestry_sources() {
    let c = common();
    assert!(
        host(&c, "// map", 8, false)
            .with_forces(bound(1, 1))
            .is_err()
    );
    let mut bare = host(&c, "// map", 8, true);
    assert!(
        bare.native("CreateForce", &[])
            .unwrap_err()
            .contains("unsupported")
    );
    for changed in [
        c.replace(CREATE, ""),
        c.replace(CREATE, &format!("constant {CREATE}")),
        c.replace(ADD, ""),
        c.replace(ADD, &format!("constant {ADD}")),
        c.replace("CreateForce takes nothing", "CreateForce takes integer i"),
        c.replace(
            "CreateForce takes nothing returns force",
            "CreateForce takes nothing returns player",
        ),
        c.replace(
            "ForceAddPlayer takes force f, player p",
            "ForceAddPlayer takes force f, integer p",
        ),
        c.replace(
            "ForceAddPlayer takes force f, player p returns nothing",
            "ForceAddPlayer takes force f, player p returns boolean",
        ),
        c.replace("force extends agent", "force extends handle"),
    ] {
        assert!(
            host(&changed, "// map", 8, true)
                .with_forces(bound(1, 1))
                .is_err()
        );
    }
    for map in [
        format!("{CREATE}\n"),
        format!("{ADD}\n"),
        "type force extends agent\n".to_string(),
        "function CreateForce takes nothing returns force\nreturn null\nendfunction\n".to_string(),
    ] {
        assert!(host(&c, &map, 8, true).with_forces(bound(1, 1)).is_err());
    }
    let moved = c.replace(ADD, "");
    assert!(host(&moved, ADD, 8, true).with_forces(bound(1, 1)).is_err());
    let extended = c.replace(
        "type force extends agent",
        "type extra extends agent\ntype force extends extra",
    );
    assert!(
        host(&extended, "// map", 8, true)
            .with_forces(bound(1, 1))
            .is_ok()
    );
    assert!(
        host(&c, "// map", 8, true)
            .with_forces(bound(1, 1))
            .unwrap()
            .with_forces(bound(1, 1))
            .is_err()
    );
    for budget in [bound(usize::MAX, 1), bound(1, usize::MAX)] {
        assert!(host(&c, "// map", 8, true).with_forces(budget).is_err());
    }
}

#[path = "force_enumeration_tests.rs"]
mod enumeration;
