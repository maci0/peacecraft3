use super::{COMMON, budget, context, id, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost, PlayerStateRequestBudget};
use pcx_script::{Host, Value};
use std::sync::Arc;

const SETTER: &str =
    "native SetPlayerState takes player p, playerstate setting, integer value returns nothing";
const GETTER: &str =
    "constant native GetPlayerState takes player p, playerstate setting returns integer\n";
fn common() -> String {
    COMMON.replacen("globals\n", &format!("type playerstate extends handle\nconstant native ConvertPlayerState takes integer i returns playerstate\nconstant native ConvertOtherState takes integer i returns playerstate\n{SETTER}\n{GETTER}globals\nconstant playerstate SX = ConvertPlayerState(-11)\nconstant playerstate SY = ConvertPlayerState(37)\nconstant playerstate SZ = ConvertOtherState(999)\n"), 1)
}
fn fixture(common: &str, map: &str, selected: bool) -> (Arc<ParsedMapContext>, ObjectHost) {
    let ctx = context(301, common, map, "synthetic-player-state-requests");
    let mut names = vec![
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertMapControl",
    ];
    if selected {
        names.extend(["ConvertPlayerState", "ConvertOtherState"]);
    }
    let enums = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        &names,
        Limits {
            max_handles: 32,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 2000,
        },
    )
    .unwrap();
    let host = ObjectHost::checked(enums)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    (ctx, host)
}
#[test]
fn owned_vm_retains_signed_requests_without_effective_defaults_or_getters() {
    let c = common();
    let map = "globals\nplayer A = Player(0)\nplayer Alias = Player(0)\nplayer B = Player(1)\nendglobals\nfunction request takes player p, playerstate setting, integer value returns nothing\ncall SetPlayerState(p,setting,value)\nendfunction\n";
    let (ctx, host) = fixture(&c, map, true);
    let host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 3 })
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    drop(ctx);
    drop(c);
    vm.init_globals().unwrap();
    let a = vm.global("A");
    let alias = vm.global("Alias");
    let b = vm.global("B");
    let x = vm.global("SX");
    let y = vm.global("SY");
    assert_eq!(a, alias);
    assert!(
        vm.host()
            .inner
            .player_settings(id(&a))
            .unwrap()
            .state_requests
            .is_empty()
    );
    let handles = vm.host().inner.inner().handles();
    for args in [
        vec![a.clone(), x.clone(), Value::Int(0)],
        vec![a.clone(), y.clone(), Value::Int(-23)],
        vec![b.clone(), x.clone(), Value::Int(i32::MAX)],
    ] {
        vm.call("request", &args).unwrap();
    }
    assert_eq!(
        vm.host()
            .inner
            .player_settings(id(&a))
            .unwrap()
            .state_requests,
        vec![(id(&x), 0), (id(&y), -23)]
    );
    assert_eq!(
        vm.host()
            .inner
            .player_settings(id(&b))
            .unwrap()
            .state_requests,
        vec![(id(&x), i32::MAX)]
    );
    vm.call("request", &[alias, x.clone(), Value::Int(i32::MIN)])
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .player_settings(id(&a))
            .unwrap()
            .state_requests,
        vec![(id(&x), i32::MIN), (id(&y), -23)]
    );
    let error = vm
        .call("request", &[b.clone(), y, Value::Int(0)])
        .unwrap_err();
    assert!(
        error.to_string().contains("player-state requests"),
        "{error}"
    );
    assert_eq!(
        vm.host()
            .inner
            .player_settings(id(&b))
            .unwrap()
            .state_requests,
        vec![(id(&x), i32::MAX)]
    );
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert!(
        vm.host()
            .inner
            .native("GetPlayerState", &[a, x])
            .unwrap_err()
            .contains("unsupported")
    );
}
#[test]
fn refused_requests_preserve_prior_rows_and_zero_budget_allocates_nothing() {
    let (_, host) = fixture(&common(), "// map", true);
    let mut host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
        .unwrap();
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let x = host
        .native("ConvertPlayerState", &[Value::Int(37)])
        .unwrap();
    let color = host
        .native("ConvertPlayerColor", &[Value::Int(37)])
        .unwrap();
    let outside = host
        .native("ConvertOtherState", &[Value::Int(999)])
        .unwrap();
    let unknown = Value::Handle(pcx_script::HandleId::new(
        std::num::NonZeroU64::new(u64::MAX).unwrap(),
    ));
    host.native("SetPlayerState", &[a.clone(), x.clone(), Value::Int(0)])
        .unwrap();
    let handles = host.inner().handles();
    for args in [
        vec![],
        vec![a.clone(), x.clone(), Value::Real(0.)],
        vec![color.clone(), x.clone(), Value::Int(1)],
        vec![a.clone(), color, Value::Int(1)],
        vec![a.clone(), outside, Value::Int(1)],
        vec![unknown.clone(), x.clone(), Value::Int(1)],
        vec![a.clone(), unknown, Value::Int(1)],
    ] {
        assert!(host.native("SetPlayerState", &args).is_err());
        assert_eq!(
            host.player_settings(id(&a)).unwrap().state_requests,
            vec![(id(&x), 0)]
        );
        assert_eq!(host.inner().handles(), handles);
    }
    let (_, host) = fixture(&common(), "// map", true);
    let mut host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 0 })
        .unwrap();
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let x = host
        .native("ConvertPlayerState", &[Value::Int(-11)])
        .unwrap();
    let handles = host.inner().handles();
    assert!(
        host.native("SetPlayerState", &[a.clone(), x, Value::Int(0)])
            .is_err()
    );
    assert!(
        host.player_settings(id(&a))
            .unwrap()
            .state_requests
            .is_empty()
    );
    assert_eq!(host.inner().handles(), handles);
}
#[test]
fn request_admission_is_explicit_source_bound_and_independent_of_the_getter() {
    let c = common();
    let (_, mut host) = fixture(&c, "// map", true);
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let x = host
        .native("ConvertPlayerState", &[Value::Int(-11)])
        .unwrap();
    assert!(
        host.native("SetPlayerState", &[a.clone(), x, Value::Int(0)])
            .unwrap_err()
            .contains("unsupported")
    );
    assert!(
        host.player_settings(id(&a))
            .unwrap()
            .state_requests
            .is_empty()
    );
    let (_, host) = fixture(&c.replace(GETTER, ""), "// map", true);
    assert!(
        host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
            .is_ok()
    );
    for c in [
        c.replace(SETTER, ""),
        c.replace(SETTER, &format!("constant {SETTER}")),
        c.replace(
            "playerstate setting, integer value returns nothing",
            "playerstate setting, real value returns nothing",
        ),
    ] {
        let (_, host) = fixture(&c, "// map", true);
        assert!(
            host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
                .is_err()
        );
    }
    for (c, map, selected) in [
        (&c, "// map".to_string(), false),
        (&c, format!("{SETTER}\n"), true),
    ] {
        let (_, host) = fixture(c, &map, selected);
        assert!(
            host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
                .is_err()
        );
    }
    let (_, host) = fixture(&c.replace(SETTER, ""), &format!("{SETTER}\n"), true);
    assert!(
        host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
            .is_err()
    );
    let (_, host) = fixture(&c, "// map", true);
    let host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
        .unwrap();
    assert!(
        host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
            .is_err()
    );
    let (_, host) = fixture(&c, "// map", true);
    assert!(
        host.with_player_state_requests(PlayerStateRequestBudget {
            max_records: usize::MAX
        })
        .is_err()
    );
    let ctx = context(301, &c, "// map", "no-players");
    let host = ObjectHost::checked(super::enums(&ctx, 32)).unwrap();
    assert!(
        host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
            .is_err()
    );
}
