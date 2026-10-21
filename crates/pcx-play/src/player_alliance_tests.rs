use super::{COMMON, budget, context, id, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost, PlayerAllianceBudget};
use pcx_script::{Host, Value};
use std::sync::Arc;

const SETTER: &str = "native SetPlayerAlliance takes player sourcePlayer, player otherPlayer, alliancetype setting, boolean value returns nothing";
fn common() -> String {
    COMMON.replacen("globals\n", &format!("type alliancetype extends handle\nconstant native ConvertAllianceType takes integer i returns alliancetype\nconstant native ConvertOtherAlliance takes integer i returns alliancetype\n{SETTER}\nconstant native GetPlayerAlliance takes player a, player b, alliancetype setting returns boolean\nglobals\nconstant alliancetype AX = ConvertAllianceType(-17)\nconstant alliancetype AY = ConvertAllianceType(97)\nconstant alliancetype AZ = ConvertOtherAlliance(800)\n"), 1)
}
fn fixture(common: &str, map: &str, selected: bool) -> (Arc<ParsedMapContext>, ObjectHost) {
    let ctx = context(301, common, map, "synthetic-alliance");
    let mut names = vec![
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertMapControl",
    ];
    if selected {
        names.extend(["ConvertAllianceType", "ConvertOtherAlliance"]);
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
fn startup_alliances_use_source_classes_and_explicit_false_overrides_only_its_direction() {
    let common = common().replacen("globals\n", "globals\nconstant mapcontrol Rescue = ConvertMapControl(92)\nconstant mapcontrol Active = ConvertMapControl(93)\nconstant mapcontrol Unused = ConvertMapControl(94)\n", 1);
    let (ctx, host) = fixture(&common, "// map", true);
    let mut host = host
        .with_player_alliances(PlayerAllianceBudget { max_records: 2 })
        .unwrap();
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let b = host.native("Player", &[Value::Int(1)]).unwrap();
    let kind = host
        .native("ConvertAllianceType", &[Value::Int(-17)])
        .unwrap();
    let mut controls = Vec::new();
    for slot in 0..host.player_slots().unwrap() {
        let code = match slot {
            0 => 91,
            1 => 93,
            _ => 94,
        };
        let control = host
            .native("ConvertMapControl", &[Value::Int(code)])
            .unwrap();
        controls.push((i32::try_from(slot).unwrap(), id(&control)));
    }
    let rules = crate::player_alliance_rules::tests::fixture([91, 92], 93, (-17_i8) as u8).unwrap();
    assert!(
        host.admit_startup_alliances(&ctx, rules.clone(), &controls[..1])
            .is_err()
    );
    host.admit_startup_alliances(&ctx, rules.clone(), &controls)
        .unwrap();
    assert!(
        host.admit_startup_alliances(&ctx, rules, &controls)
            .is_err()
    );
    let read = |host: &ObjectHost, source: &Value, target: &Value| {
        host.effective_alliance(
            host.player_settings(id(source)).unwrap(),
            host.player_settings(id(target)).unwrap(),
            -17,
        )
    };
    assert!(read(&host, &a, &b));
    assert!(read(&host, &b, &a));
    assert!(
        host.player_settings(id(&a)).unwrap().alliances.is_empty(),
        "initial defaults are not executed script writes"
    );
    host.native(
        "SetPlayerAlliance",
        &[a.clone(), b.clone(), kind.clone(), Value::Bool(false)],
    )
    .unwrap();
    assert!(!read(&host, &a, &b));
    assert!(read(&host, &b, &a));
    host.native(
        "SetPlayerAlliance",
        &[a.clone(), b.clone(), kind, Value::Bool(true)],
    )
    .unwrap();
    assert!(read(&host, &a, &b));
}

#[test]
fn owned_vm_keeps_directed_symbolic_assignments_and_updates_at_capacity() {
    let common = common();
    let map = "globals\nplayer A = Player(0)\nplayer Alias = Player(0)\nplayer B = Player(1)\nendglobals\nfunction same_team takes nothing returns nothing\ncall SetPlayerTeam(A,4)\ncall SetPlayerTeam(B,4)\nendfunction\nfunction assign takes player a, player b, alliancetype setting, boolean value returns nothing\ncall SetPlayerAlliance(a,b,setting,value)\nendfunction\n";
    let (ctx, host) = fixture(&common, map, true);
    let host = host
        .with_player_alliances(PlayerAllianceBudget { max_records: 3 })
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    drop(ctx);
    drop(common);
    vm.init_globals().unwrap();
    let a = vm.global("A");
    let alias = vm.global("Alias");
    let b = vm.global("B");
    let x = vm.global("AX");
    let y = vm.global("AY");
    assert_eq!(a, alias);
    vm.call("same_team", &[]).unwrap();
    assert!(
        vm.host()
            .inner
            .player_settings(id(&a))
            .unwrap()
            .alliances
            .is_empty()
    );
    assert!(
        vm.host()
            .inner
            .player_settings(id(&b))
            .unwrap()
            .alliances
            .is_empty()
    );
    let handles = vm.host().inner.inner().handles();
    for args in [
        vec![a.clone(), b.clone(), x.clone(), Value::Bool(true)],
        vec![b.clone(), a.clone(), x.clone(), Value::Bool(false)],
        vec![alias.clone(), b.clone(), y.clone(), Value::Bool(false)],
    ] {
        vm.call("assign", &args).unwrap();
    }
    let first = vec![(id(&b), id(&x), true), (id(&b), id(&y), false)];
    assert_eq!(
        vm.host().inner.player_settings(id(&a)).unwrap().alliances,
        first
    );
    assert_eq!(
        vm.host().inner.player_settings(id(&b)).unwrap().alliances,
        vec![(id(&a), id(&x), false)]
    );
    vm.call(
        "assign",
        &[a.clone(), b.clone(), x.clone(), Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(
        vm.host().inner.player_settings(id(&a)).unwrap().alliances,
        vec![(id(&b), id(&x), false), (id(&b), id(&y), false)]
    );
    let error = vm
        .call(
            "assign",
            &[b.clone(), a.clone(), y.clone(), Value::Bool(true)],
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("player alliance assignments"),
        "{error}"
    );
    assert_eq!(
        vm.host().inner.player_settings(id(&b)).unwrap().alliances,
        vec![(id(&a), id(&x), false)]
    );
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert!(
        vm.host()
            .inner
            .native("GetPlayerAlliance", &[a, b, x])
            .unwrap_err()
            .contains("unsupported")
    );
}
#[test]
fn refused_alliance_calls_leave_prior_records_and_arena_unchanged() {
    let (_, host) = fixture(&common(), "// map", true);
    let mut host = host
        .with_player_alliances(PlayerAllianceBudget { max_records: 2 })
        .unwrap();
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let b = host.native("Player", &[Value::Int(1)]).unwrap();
    let x = host
        .native("ConvertAllianceType", &[Value::Int(-17)])
        .unwrap();
    let outside = host
        .native("ConvertOtherAlliance", &[Value::Int(800)])
        .unwrap();
    let timer = host.native("CreateTimer", &[]).unwrap();
    let unknown = Value::Handle(pcx_script::HandleId::new(
        std::num::NonZeroU64::new(u64::MAX).unwrap(),
    ));
    host.native(
        "SetPlayerAlliance",
        &[a.clone(), b.clone(), x.clone(), Value::Bool(false)],
    )
    .unwrap();
    let handles = host.inner().handles();
    for args in [
        vec![],
        vec![unknown.clone(), b.clone(), x.clone(), Value::Bool(true)],
        vec![a.clone(), unknown.clone(), x.clone(), Value::Bool(true)],
        vec![a.clone(), b.clone(), unknown, Value::Bool(true)],
        vec![a.clone(), b.clone(), x.clone(), Value::Int(1)],
        vec![timer.clone(), b.clone(), x.clone(), Value::Bool(true)],
        vec![a.clone(), timer.clone(), x.clone(), Value::Bool(true)],
        vec![a.clone(), b.clone(), timer, Value::Bool(true)],
        vec![a.clone(), b.clone(), outside, Value::Bool(true)],
    ] {
        assert!(host.native("SetPlayerAlliance", &args).is_err());
        assert_eq!(
            host.player_settings(id(&a)).unwrap().alliances,
            vec![(id(&b), id(&x), false)]
        );
        assert_eq!(host.inner().handles(), handles);
    }
    // Direct self assignments are retained; wrapper guards do not alter this native.
    host.native(
        "SetPlayerAlliance",
        &[a.clone(), a.clone(), x.clone(), Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(host.player_settings(id(&a)).unwrap().alliances.len(), 2);
    let (_, host) = fixture(&common(), "// map", true);
    let mut host = host
        .with_player_alliances(PlayerAllianceBudget { max_records: 0 })
        .unwrap();
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let x = host
        .native("ConvertAllianceType", &[Value::Int(-17)])
        .unwrap();
    let handles = host.inner().handles();
    assert!(
        host.native(
            "SetPlayerAlliance",
            &[a.clone(), a.clone(), x, Value::Bool(false)]
        )
        .is_err()
    );
    assert!(host.player_settings(id(&a)).unwrap().alliances.is_empty());
    assert_eq!(host.inner().handles(), handles);
}
#[test]
fn alliance_admission_requires_selected_contract_source_and_domain() {
    let c = common();
    let ctx = context(301, &c, "// map", "unadmitted-alliance-players");
    let host = ObjectHost::checked(super::enums(&ctx, 32)).unwrap();
    assert!(
        host.with_player_alliances(PlayerAllianceBudget { max_records: 1 })
            .is_err()
    );
    let (_, host) = fixture(&c, "// map", true);
    assert!(
        host.with_player_alliances(PlayerAllianceBudget {
            max_records: usize::MAX
        })
        .is_err()
    );
    for c in [
        c.replace(SETTER, ""),
        c.replace(SETTER, &format!("constant {SETTER}")),
        c.replace(
            "alliancetype setting, boolean value returns nothing",
            "alliancetype setting, integer value returns nothing",
        ),
    ] {
        let (_, host) = fixture(&c, "// map", true);
        assert!(
            host.with_player_alliances(PlayerAllianceBudget { max_records: 1 })
                .is_err()
        );
    }
    let (_, host) = fixture(&c, "// map", false);
    assert!(
        host.with_player_alliances(PlayerAllianceBudget { max_records: 1 })
            .is_err()
    );
    let (_, host) = fixture(&c, "// map", true);
    let host = host
        .with_player_alliances(PlayerAllianceBudget { max_records: 1 })
        .unwrap();
    assert!(
        host.with_player_alliances(PlayerAllianceBudget { max_records: 1 })
            .is_err()
    );
    let (_, host) = fixture(&c.replace(SETTER, ""), &format!("{SETTER}\n"), true);
    assert!(
        host.with_player_alliances(PlayerAllianceBudget { max_records: 1 })
            .is_err()
    );
    let (_, host) = fixture(&c, &format!("{SETTER}\n"), true);
    assert!(
        host.with_player_alliances(PlayerAllianceBudget { max_records: 1 })
            .is_err()
    );
}
