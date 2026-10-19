use super::{COMMON, budget, context, object, player, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::ObjectHost;
use pcx_script::{Host, Value};
use std::sync::Arc;

const DECLARATION: &str = "constant native GetLocalPlayer takes nothing returns player\n";

fn selected(declaration: &str, map: &str) -> (ObjectHost, Arc<ParsedMapContext>) {
    let ctx = context(
        301,
        &format!("{COMMON}{declaration}"),
        map,
        "synthetic:local-player",
    );
    let host = object(&ctx, 32)
        .with_players(ctx.clone(), slots_rule(301, -3, 9), budget())
        .unwrap();
    (host, ctx)
}

#[test]
fn local_player_is_explicit_and_aliases_the_canonical_lazy_player() {
    let (mut host, ctx) = selected(DECLARATION, "// map");
    assert!(host.native("GetLocalPlayer", &[]).is_err());
    let handles = host.inner().handles();
    host.admit_local_player(&ctx, 5).unwrap();
    assert_eq!(host.inner().handles(), handles);
    let local = host.native("GetLocalPlayer", &[]).unwrap();
    assert_eq!(host.inner().handles(), handles + 1);
    assert_eq!(player(&mut host, 5), local);
    assert_eq!(host.native("GetLocalPlayer", &[]).unwrap(), local);
    assert_eq!(host.inner().handles(), handles + 1);
    assert_eq!(
        host.native("GetPlayerId", std::slice::from_ref(&local))
            .unwrap(),
        Value::Int(5)
    );
    assert!(
        host.native("GetLocalPlayer", std::slice::from_ref(&local))
            .is_err()
    );
    let Value::Handle(identity) = local else {
        panic!("player")
    };
    let settings = host.player_settings(identity).unwrap();
    assert!(
        settings.team.is_none() && settings.controller.is_none() && settings.slot_state.is_none()
    );
}

#[test]
fn selecting_an_existing_player_never_allocates_or_changes_its_setup() {
    let (mut host, ctx) = selected(DECLARATION, "// map");
    let original = player(&mut host, 2);
    host.native("SetPlayerTeam", &[original.clone(), Value::Int(3)])
        .unwrap();
    let handles = host.inner().handles();
    host.admit_local_player(&ctx, 2).unwrap();
    assert_eq!(host.native("GetLocalPlayer", &[]).unwrap(), original);
    assert_eq!(host.inner().handles(), handles);
    assert_eq!(
        host.native("GetPlayerTeam", &[original]).unwrap(),
        Value::Int(3)
    );
}

#[test]
fn invalid_choices_and_other_map_contexts_preserve_the_prior_choice() {
    let (mut host, ctx) = selected(DECLARATION, "// map");
    host.admit_local_player(&ctx, 3).unwrap();
    let local = host.native("GetLocalPlayer", &[]).unwrap();
    let (_, other) = selected(DECLARATION, "// same declarations, different context");
    assert!(host.admit_local_player(&other, 1).is_err());
    for number in [-1, 9, i32::MAX] {
        assert!(host.admit_local_player(&ctx, number).is_err());
        assert_eq!(host.native("GetLocalPlayer", &[]).unwrap(), local);
    }
}

#[test]
fn local_player_requires_its_exact_constant_same_source_declaration() {
    for declaration in [
        "",
        "native GetLocalPlayer takes nothing returns player\n",
        "constant native GetLocalPlayer takes integer n returns player\n",
        "constant native GetLocalPlayer takes nothing returns integer\n",
    ] {
        let (mut host, ctx) = selected(declaration, "// map");
        assert!(host.admit_local_player(&ctx, 1).is_err());
        assert!(host.native("GetLocalPlayer", &[]).is_err());
    }
    let (mut host, ctx) = selected("", DECLARATION);
    assert!(host.admit_local_player(&ctx, 1).is_err());
    assert!(host.native("GetLocalPlayer", &[]).is_err());
    let (mut host, ctx) = selected(DECLARATION, DECLARATION);
    assert!(host.admit_local_player(&ctx, 1).is_err());
}

#[test]
fn owning_vm_camera_style_branch_uses_selected_identity_and_new_sessions_start_unset() {
    let map = "function applies takes nothing returns boolean\nreturn GetLocalPlayer() == Player(4)\nendfunction\n";
    for (number, applies) in [(4, true), (7, false)] {
        let (host, ctx) = selected(DECLARATION, map);
        let mut vm = host.into_vm();
        vm.init_globals().unwrap();
        vm.host().admit_local_player(&ctx, number).unwrap();
        assert_eq!(vm.call("applies", &[]).unwrap(), Value::Bool(applies));
        let (host, _) = selected(DECLARATION, map);
        let mut fresh = host.into_vm();
        fresh.init_globals().unwrap();
        assert!(fresh.call("applies", &[]).is_err());
    }
}
