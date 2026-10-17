use super::{COMMON, budget, context, id, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;
fn common() -> String {
    format!(
        "{COMMON}\nconstant native ConvertOtherControl takes integer i returns mapcontrol\nglobals\nconstant mapcontrol Current = ConvertMapControl(-31)\nconstant mapcontrol Other = ConvertOtherControl(101)\nendglobals\n"
    )
}
fn object(c: &str, map: &str, selected: bool) -> (ObjectHost, Arc<ParsedMapContext>) {
    let ctx = context(301, c, map, "synthetic:current-controller");
    let mut names = vec![
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertOtherControl",
    ];
    if selected {
        names.push("ConvertMapControl");
    }
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic:current-controller",
        &names,
        Limits {
            max_handles: 32,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
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
fn control(h: &mut ObjectHost, n: i32) -> Value {
    h.native("ConvertMapControl", &[Value::Int(n)]).unwrap()
}
fn query(h: &mut ObjectHost, p: &Value) -> Result<Value, String> {
    h.native("GetPlayerController", std::slice::from_ref(p))
}
#[test]
fn owned_vm_reads_current_input_for_later_players_without_replacing_executed_setup() {
    let map = "globals\nplayer A = Player(0)\nplayer Alias = Player(0)\nplayer B = Player(1)\nplayer Later = null\nendglobals\nfunction config takes nothing returns nothing\ncall SetPlayerController(A,M)\ncall SetPlayerController(B,M)\nendfunction\nfunction late takes nothing returns mapcontrol\nset Later = Player(4)\nreturn GetPlayerController(Later)\nendfunction\n";
    let (h, ctx) = object(&common(), map, true);
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm.call("config", &[]).unwrap();
    let (a, alias, b, assigned, current) = (
        vm.global("A"),
        vm.global("Alias"),
        vm.global("B"),
        vm.global("M"),
        vm.global("Current"),
    );
    assert_eq!(query(vm.host(), &a).unwrap(), assigned);
    let handles = vm.host().inner().handles();
    vm.host()
        .admit_startup_controllers(&ctx, &[(0, id(&current)), (4, id(&assigned))])
        .unwrap();
    assert_eq!(vm.host().inner().handles(), handles);
    assert_eq!(query(vm.host(), &alias).unwrap(), current);
    assert_eq!(
        vm.host().player_settings(id(&a)).unwrap().controller,
        Some(id(&assigned))
    );
    assert!(query(vm.host(), &b).unwrap_err().contains("unset"));
    assert_eq!(vm.call("late", &[]).unwrap(), assigned);
    let later = vm.global("Later");
    assert_eq!(vm.host().player_settings(id(&later)).unwrap().number, 4);
    assert!(
        vm.host()
            .player_settings(id(&later))
            .unwrap()
            .controller
            .is_none()
    );
    assert_eq!(vm.host().inner().handles(), handles + 1);
    vm.host().admit_startup_controllers(&ctx, &[]).unwrap();
    assert!(query(vm.host(), &a).unwrap_err().contains("unset"));
    vm.host().invalidate_startup_controllers();
    assert!(query(vm.host(), &a).unwrap_err().contains("invalidated"));
    vm.host()
        .admit_startup_controllers(&ctx, &[(0, id(&current))])
        .unwrap();
    assert_eq!(query(vm.host(), &a).unwrap(), current);
    assert!(query(vm.host(), &later).is_err());
}
#[test]
fn invalid_current_inputs_preserve_snapshot_and_successful_controller_write_retires_it() {
    let (mut h, ctx) = object(&common(), "// map", true);
    let (a, b) = (player(&mut h, 0), player(&mut h, 1));
    let current = control(&mut h, -31);
    let assigned = control(&mut h, 91);
    let timer = h.native("CreateTimer", &[]).unwrap();
    let other = h.native("ConvertOtherControl", &[Value::Int(101)]).unwrap();
    let unknown = HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    h.native("SetPlayerController", &[a.clone(), assigned.clone()])
        .unwrap();
    h.invalidate_startup_controllers();
    assert_eq!(query(&mut h, &a).unwrap(), assigned);
    h.admit_startup_controllers(&ctx, &[(0, id(&current)), (1, id(&assigned))])
        .unwrap();
    let handles = h.inner().handles();
    let slots = i32::try_from(h.player_slots().unwrap()).unwrap();
    for input in [
        vec![(-1, id(&current))],
        vec![(slots, id(&current))],
        vec![(i32::MAX, id(&current))],
        vec![(0, id(&current)), (0, id(&assigned))],
        vec![(0, id(&timer))],
        vec![(0, id(&other))],
        vec![(0, unknown)],
        vec![(0, id(&current)); h.player_slots().unwrap() + 1],
    ] {
        assert!(h.admit_startup_controllers(&ctx, &input).is_err());
        assert_eq!(query(&mut h, &a).unwrap(), current);
        assert_eq!(query(&mut h, &b).unwrap(), assigned);
        assert_eq!(h.inner().handles(), handles);
    }
    let wrong = context(301, &common(), "// map", "synthetic:current-controller");
    assert!(h.admit_startup_controllers(&wrong, &[]).is_err());
    for args in [
        vec![],
        vec![Value::Null],
        vec![Value::Int(0)],
        vec![timer.clone()],
        vec![Value::Handle(unknown)],
        vec![a.clone(), a.clone()],
    ] {
        assert!(h.native("GetPlayerController", &args).is_err());
        assert_eq!(query(&mut h, &a).unwrap(), current);
    }
    assert!(
        h.native("SetPlayerController", &[a.clone(), timer])
            .is_err()
    );
    assert_eq!(query(&mut h, &a).unwrap(), current);
    h.native("SetPlayerController", &[a.clone(), assigned.clone()])
        .unwrap();
    assert_eq!(
        h.player_settings(id(&a)).unwrap().controller,
        Some(id(&assigned))
    );
    assert!(query(&mut h, &a).unwrap_err().contains("invalidated"));
    assert!(query(&mut h, &b).unwrap_err().contains("invalidated"));
    while h.inner().handles() < 32 {
        h.native("CreateTimer", &[]).unwrap();
    }
    h.admit_startup_controllers(&ctx, &[(0, id(&current))])
        .unwrap();
    assert_eq!(query(&mut h, &a).unwrap(), current);
    assert_eq!(h.inner().handles(), 32);
}
#[test]
fn current_controller_admission_requires_selected_conversion_and_same_source_contracts() {
    let c = common();
    let (mut h, ctx) = object(&c, "// map", false);
    assert!(h.admit_startup_controllers(&ctx, &[]).is_err());
    let mut bare = ObjectHost::checked(super::enums(&ctx, 32)).unwrap();
    assert!(bare.admit_startup_controllers(&ctx, &[]).is_err());
    for declaration in [
        "native GetPlayerController takes player p returns mapcontrol",
        "native SetPlayerController takes player p, mapcontrol c returns nothing",
        "type mapcontrol extends handle",
    ] {
        let moved = c.replace(declaration, "");
        let (mut h, ctx) = object(&moved, declaration, true);
        assert!(h.admit_startup_controllers(&ctx, &[]).is_err());
    }
    let converter = "constant native ConvertMapControl takes integer i returns mapcontrol";
    let moved = c
        .replace(converter, "")
        .replace("constant mapcontrol M = ConvertMapControl(91)", "")
        .replace("constant mapcontrol Current = ConvertMapControl(-31)", "");
    let map = format!(
        "{converter}\nglobals\nconstant mapcontrol Foreign = ConvertMapControl(5)\nendglobals\n"
    );
    let (mut h, ctx) = object(&moved, &map, true);
    assert!(h.admit_startup_controllers(&ctx, &[]).is_err());
    let c = c.replace(
        "type mapcontrol extends handle",
        "type controlbase extends handle\ntype mapcontrol extends controlbase",
    );
    let (mut h, ctx) = object(&c, "// map", true);
    let p = player(&mut h, 0);
    let v = control(&mut h, -31);
    h.admit_startup_controllers(&ctx, &[(0, id(&v))]).unwrap();
    assert_eq!(query(&mut h, &p).unwrap(), v);
}
