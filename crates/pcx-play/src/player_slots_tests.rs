use super::{COMMON, budget, context, id, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;
const SLOTS: &str = "type playerslotstate extends handle
constant native ConvertPlayerSlotState takes integer i returns playerslotstate
constant native ConvertOtherSlot takes integer i returns playerslotstate
native GetPlayerSlotState takes player p returns playerslotstate
globals
constant playerslotstate Empty = ConvertPlayerSlotState(-17)
constant playerslotstate Playing = ConvertPlayerSlotState(29)
constant playerslotstate Left = ConvertPlayerSlotState(83)
constant playerslotstate Other = ConvertOtherSlot(101)
endglobals
";
fn common() -> String {
    format!("{COMMON}\n{SLOTS}")
}
fn enums(ctx: &ParsedMapContext, selected: bool) -> EnumHost {
    let mut names = vec![
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertMapControl",
        "ConvertOtherSlot",
    ];
    if selected {
        names.push("ConvertPlayerSlotState");
    }
    EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic:slot-state",
        &names,
        Limits {
            max_handles: 32,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap()
}
fn object(c: &str, map: &str, selected: bool) -> (ObjectHost, Arc<ParsedMapContext>) {
    let ctx = context(301, c, map, "synthetic:slot-state");
    let inner = enums(&ctx, selected);
    (
        ObjectHost::checked(inner)
            .unwrap()
            .with_players(ctx.clone(), slots_rule(301, -3, 9), budget())
            .unwrap(),
        ctx,
    )
}
fn player(h: &mut ObjectHost, n: i32) -> Value {
    h.native("Player", &[Value::Int(n)]).unwrap()
}
fn state(h: &mut ObjectHost, n: i32) -> Value {
    h.native("ConvertPlayerSlotState", &[Value::Int(n)])
        .unwrap()
}
fn query(h: &mut ObjectHost, p: &Value) -> Result<Value, String> {
    h.native("GetPlayerSlotState", std::slice::from_ref(p))
}
#[test]
fn owned_vm_reads_only_the_explicit_current_slots_without_minting_players_or_defaults() {
    let map = "globals\nplayer A = Player(0)\nplayer Alias = Player(0)\nplayer B = Player(1)\nplayer C = Player(2)\nplayerslotstate Seen = null\nendglobals\nfunction read takes player p returns playerslotstate\nset Seen = GetPlayerSlotState(p)\nreturn Seen\nendfunction\n";
    let (h, ctx) = object(&common(), map, true);
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    let (a, alias, b, c, empty, playing, left) = (
        vm.global("A"),
        vm.global("Alias"),
        vm.global("B"),
        vm.global("C"),
        vm.global("Empty"),
        vm.global("Playing"),
        vm.global("Left"),
    );
    let count = vm.host().inner().handles();
    assert!(query(vm.host(), &a).unwrap_err().contains("unsupported"));
    vm.host()
        .admit_startup_player_slots(&ctx, &[(id(&a), id(&empty)), (id(&b), id(&playing))])
        .unwrap();
    assert_eq!(
        vm.call("read", std::slice::from_ref(&alias)).unwrap(),
        empty
    );
    assert_eq!(vm.call("read", std::slice::from_ref(&b)).unwrap(), playing);
    assert_eq!(vm.global("Seen"), playing);
    assert!(query(vm.host(), &c).unwrap_err().contains("unset"));
    vm.host()
        .admit_startup_player_slots(&ctx, &[(id(&a), id(&left))])
        .unwrap();
    assert_eq!(query(vm.host(), &a).unwrap(), left);
    assert!(query(vm.host(), &b).unwrap_err().contains("unset"));
    assert_eq!(
        vm.host().player_settings(id(&a)).unwrap().slot_state,
        Some(id(&left))
    );
    assert!(
        vm.host()
            .player_settings(id(&a))
            .unwrap()
            .controller
            .is_none()
    );
    assert_eq!(vm.host().inner().handles(), count);
    let later = player(vm.host(), 3);
    assert!(query(vm.host(), &later).unwrap_err().contains("unset"));
    vm.host().admit_startup_player_slots(&ctx, &[]).unwrap();
    assert!(query(vm.host(), &a).unwrap_err().contains("unset"));
    assert!(
        vm.host()
            .player_settings(id(&a))
            .unwrap()
            .slot_state
            .is_none()
    );
    vm.host().invalidate_startup_player_slots();
    assert!(query(vm.host(), &a).unwrap_err().contains("unsupported"));
    while vm.host().inner().handles() < 32 {
        vm.host().native("CreateTimer", &[]).unwrap();
    }
    vm.host()
        .admit_startup_player_slots(&ctx, &[(id(&a), id(&playing))])
        .unwrap();
    assert_eq!(query(vm.host(), &a).unwrap(), playing);
    assert_eq!(vm.host().inner().handles(), 32);
}
#[test]
fn invalid_admission_and_calls_preserve_slots_and_accepted_controller_write_retires_input() {
    let (mut h, ctx) = object(&common(), "// map", true);
    let (a, b) = (player(&mut h, 0), player(&mut h, 1));
    let playing = state(&mut h, 29);
    let empty = state(&mut h, -17);
    let timer = h.native("CreateTimer", &[]).unwrap();
    let other = h.native("ConvertOtherSlot", &[Value::Int(101)]).unwrap();
    h.admit_startup_player_slots(&ctx, &[(id(&a), id(&playing)), (id(&b), id(&empty))])
        .unwrap();
    let handles = h.inner().handles();
    let unknown = HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    for entries in [
        vec![(id(&a), id(&empty)), (id(&a), id(&playing))],
        vec![(id(&timer), id(&empty))],
        vec![(unknown, id(&empty))],
        vec![(id(&a), id(&timer))],
        vec![(id(&a), id(&other))],
        vec![(id(&a), unknown)],
        vec![(id(&a), id(&empty)); h.player_slots().unwrap() + 1],
    ] {
        assert!(h.admit_startup_player_slots(&ctx, &entries).is_err());
        assert_eq!(query(&mut h, &a).unwrap(), playing);
        assert_eq!(query(&mut h, &b).unwrap(), empty);
        assert_eq!(h.inner().handles(), handles);
    }
    let wrong = context(301, &common(), "// map", "synthetic:slot-state");
    assert!(h.admit_startup_player_slots(&wrong, &[]).is_err());
    for args in [
        vec![],
        vec![Value::Null],
        vec![Value::Int(0)],
        vec![timer.clone()],
        vec![Value::Handle(unknown)],
        vec![a.clone(), a.clone()],
    ] {
        assert!(h.native("GetPlayerSlotState", &args).is_err());
        assert_eq!(query(&mut h, &a).unwrap(), playing);
    }
    assert!(
        h.native("SetPlayerController", &[a.clone(), timer])
            .is_err()
    );
    assert_eq!(query(&mut h, &a).unwrap(), playing);
    let control = h.native("ConvertMapControl", &[Value::Int(91)]).unwrap();
    h.native("SetPlayerController", &[a.clone(), control.clone()])
        .unwrap();
    assert!(query(&mut h, &a).unwrap_err().contains("unsupported"));
    assert!(h.player_settings(id(&b)).unwrap().slot_state.is_none());
    assert_eq!(
        h.native("GetPlayerController", std::slice::from_ref(&a))
            .unwrap(),
        control
    );
    h.admit_startup_player_slots(&ctx, &[(id(&a), id(&playing))])
        .unwrap();
    assert_eq!(query(&mut h, &a).unwrap(), playing);
}
#[test]
fn slot_admission_checks_selected_domain_signatures_sources_and_real_player_rows() {
    let c = common();
    let declaration = "native GetPlayerSlotState takes player p returns playerslotstate";
    for c in [
        c.replace(declaration, ""),
        c.replace(declaration, &format!("constant {declaration}")),
        c.replace(
            "GetPlayerSlotState takes player p",
            "GetPlayerSlotState takes integer p",
        ),
        c.replace(
            "GetPlayerSlotState takes player p returns playerslotstate",
            "GetPlayerSlotState takes player p returns integer",
        ),
    ] {
        let (mut h, ctx) = object(&c, "// map", true);
        assert!(h.admit_startup_player_slots(&ctx, &[]).is_err());
    }
    for (c,map) in [(common(),declaration.to_string()),(common().replace(declaration,""),declaration.to_string()),(common(),"function GetPlayerSlotState takes player p returns playerslotstate\nreturn null\nendfunction\n".into()),(common().replace("type playerslotstate extends handle",""),"type playerslotstate extends handle\n".into())] {
        let (mut h,ctx)=object(&c,&map,true);assert!(h.admit_startup_player_slots(&ctx,&[]).is_err());
    }
    let (mut h, ctx) = object(&c, "// map", false);
    assert!(h.admit_startup_player_slots(&ctx, &[]).is_err());
    let mut bare = ObjectHost::checked(super::enums(&ctx, 32)).unwrap();
    assert!(bare.admit_startup_player_slots(&ctx, &[]).is_err());
    let extra = c.replace(
        "type playerslotstate extends handle",
        "type slotbase extends handle\ntype playerslotstate extends slotbase",
    );
    let (mut h, ctx) = object(&extra, "// map", true);
    let p = player(&mut h, 0);
    let v = state(&mut h, -17);
    h.admit_startup_player_slots(&ctx, &[(id(&p), id(&v))])
        .unwrap();
    assert_eq!(query(&mut h, &p).unwrap(), v);
    // Generic player objects are not admitted rows, even when their kind matches.
    let mut inner = enums(&ctx, true);
    let v = inner
        .native("ConvertPlayerSlotState", &[Value::Int(-17)])
        .unwrap();
    let reserved = inner.reserve_identity("player").unwrap();
    let mut h = ObjectHost::checked(inner)
        .unwrap()
        .with_players(ctx.clone(), slots_rule(301, -3, 9), budget())
        .unwrap();
    assert!(
        h.admit_startup_player_slots(&ctx, &[(reserved, id(&v))])
            .is_err()
    );
}
