use super::{budget, common as settings_common, context, id, seed, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Failure, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;
const GET: &str = "native VersionGet takes nothing returns version";
const VERSION: &str = "type version extends handle
constant native ConvertVersion takes integer i returns version
constant native ConvertOtherVersion takes integer i returns version
native VersionGet takes nothing returns version
globals
constant version VLow=ConvertVersion(-17)
constant version VHigh=ConvertVersion(63)
constant version VOther=ConvertOtherVersion(103)
endglobals
";
fn common() -> String {
    format!("{}\n{VERSION}", settings_common())
}
fn object(c: &str, map: &str, selected: bool) -> (ObjectHost, Arc<ParsedMapContext>) {
    let ctx = context(301, c, map, "synthetic:version");
    let mut names = vec![
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertMapControl",
        "ConvertGameSpeed",
        "ConvertMapFlag",
        "ConvertOtherSpeed",
        "ConvertOtherVersion",
    ];
    if selected {
        names.push("ConvertVersion");
    }
    if c.contains("constant native ConvertRace takes") {
        names.push("ConvertRace");
    }
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic:version",
        &names,
        Limits {
            max_handles: 64,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let h = ObjectHost::checked(inner)
        .unwrap()
        .with_players(ctx.clone(), slots_rule(301, -3, 9), budget())
        .unwrap();
    (h, ctx)
}
fn version(h: &mut ObjectHost, n: i32) -> HandleId {
    id(&h.native("ConvertVersion", &[Value::Int(n)]).unwrap())
}

const CAMPAIGN: &str = "type race extends handle
constant native ConvertRace takes integer i returns race
native SetCampaignMenuRace takes race r returns nothing
native SetCampaignMenuRaceEx takes integer index returns nothing
globals
constant version VERSION_FROZEN_THRONE=ConvertVersion(60+3)
constant race TEST_RACE=ConvertRace(91)
endglobals
";

#[test]
fn frozen_throne_ignores_legacy_campaign_race_without_allocating_or_selecting_a_menu() {
    let c = format!("{}\n{CAMPAIGN}", common());
    let map = "globals\ninteger Reached=0\nendglobals\nfunction initialize takes nothing returns nothing\ncall SetCampaignMenuRace(TEST_RACE)\ncall SetCampaignMenuRace(null)\nset Reached=Reached+1\nendfunction\n";
    let (mut h, ctx) = object(&c, map, true);
    let high = version(&mut h, 63);
    h.admit_startup_version(&ctx, high).unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    let handles = vm.host().inner.inner().handles();
    for expected in 1..=3 {
        assert_eq!(vm.call("initialize", &[]).unwrap(), Value::Null);
        assert_eq!(vm.global("Reached"), Value::Int(expected));
        assert_eq!(vm.host().inner.inner().handles(), handles);
        assert_eq!(
            vm.host().native("VersionGet", &[]).unwrap(),
            Value::Handle(high)
        );
    }
    assert!(
        vm.host()
            .native("SetCampaignMenuRaceEx", &[Value::Int(63)])
            .is_err()
    );
    vm.host().inner.invalidate_startup_version();
    assert!(vm.call("initialize", &[]).is_err());
    assert_eq!(vm.global("Reached"), Value::Int(3));
}

#[test]
fn legacy_campaign_race_requires_explicit_supported_version_and_checked_arguments() {
    let c = format!("{}\n{CAMPAIGN}", common());
    let (mut h, ctx) = object(&c, "// map", true);
    assert!(h.native("SetCampaignMenuRace", &[Value::Null]).is_err());
    let low = version(&mut h, -17);
    h.admit_startup_version(&ctx, low).unwrap();
    assert!(h.native("SetCampaignMenuRace", &[Value::Null]).is_err());
    let high = version(&mut h, 63);
    h.admit_startup_version(&ctx, high).unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    let unknown = Value::Handle(HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap()));
    let before = h.inner().handles();
    for args in [
        vec![],
        vec![Value::Int(91)],
        vec![timer],
        vec![unknown],
        vec![Value::Null, Value::Null],
    ] {
        assert!(h.native("SetCampaignMenuRace", &args).is_err());
        assert_eq!(h.inner().handles(), before);
        assert_eq!(h.native("VersionGet", &[]).unwrap(), Value::Handle(high));
    }
    assert_eq!(
        h.native("SetCampaignMenuRace", &[Value::Null]).unwrap(),
        Value::Null
    );
}

#[test]
fn legacy_campaign_race_refuses_missing_changed_and_foreign_declarations() {
    let declaration = "native SetCampaignMenuRace takes race r returns nothing";
    for replacement in [
        "",
        "native SetCampaignMenuRace takes integer r returns nothing",
        "constant native SetCampaignMenuRace takes race r returns nothing",
    ] {
        let c = format!(
            "{}\n{}",
            common(),
            CAMPAIGN.replace(declaration, replacement)
        );
        let (mut h, ctx) = object(&c, "// map", true);
        let high = version(&mut h, 63);
        h.admit_startup_version(&ctx, high).unwrap();
        assert!(h.native("SetCampaignMenuRace", &[Value::Null]).is_err());
    }
    let c = format!("{}\n{}", common(), CAMPAIGN.replace(declaration, ""));
    let (mut h, ctx) = object(&c, declaration, true);
    let high = version(&mut h, 63);
    h.admit_startup_version(&ctx, high).unwrap();
    assert!(
        h.native("SetCampaignMenuRace", &[Value::Null])
            .unwrap_err()
            .contains("multiple declaration sources")
    );
    let version_constant = "constant version VERSION_FROZEN_THRONE=ConvertVersion(60+3)";
    let c = format!("{}\n{}", common(), CAMPAIGN.replace(version_constant, ""));
    let (mut h, ctx) = object(
        &c,
        &format!("globals\n{version_constant}\nendglobals"),
        true,
    );
    let high = version(&mut h, 63);
    h.admit_startup_version(&ctx, high).unwrap();
    assert!(
        h.native("SetCampaignMenuRace", &[Value::Null])
            .unwrap_err()
            .contains("multiple declaration sources")
    );
}
#[test]
fn owned_vm_reads_explicit_version_identity_without_allocating_or_changing_other_settings() {
    let map = "globals\nversion Saved=null\nendglobals\nfunction read takes nothing returns boolean\nset Saved=VersionGet()\nreturn Saved==VLow\nendfunction\n";
    let (mut h, ctx) = object(&common(), map, true);
    let low = version(&mut h, -17);
    assert!(
        h.native("VersionGet", &[])
            .unwrap_err()
            .contains("unsupported")
    );
    h.admit_startup_version(&ctx, low).unwrap();
    assert_eq!(h.native("VersionGet", &[]).unwrap(), Value::Handle(low));
    assert!(
        h.native("GetGameSpeed", &[]).is_err(),
        "version input does not invent settings"
    );
    let settings = seed(&mut h);
    h.admit_startup_settings(&ctx, settings).unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    let before = vm.host().inner.inner().handles();
    for _ in 0..3 {
        assert_eq!(vm.call("read", &[]).unwrap(), Value::Bool(true));
        assert_eq!(vm.global("Saved"), Value::Handle(low));
    }
    assert_eq!(vm.host().inner.inner().handles(), before);
    vm.host().inner.invalidate_startup_settings();
    assert_eq!(vm.call("read", &[]).unwrap(), Value::Bool(true));
    let high = version(&mut vm.host().inner, 63);
    vm.host().inner.admit_startup_version(&ctx, high).unwrap();
    assert_eq!(vm.call("read", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.global("Saved"), Value::Handle(high));
    vm.host()
        .inner
        .admit_startup_settings(&ctx, settings)
        .unwrap();
    vm.host().inner.invalidate_startup_version();
    assert!(
        vm.host()
            .native("VersionGet", &[])
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(
        vm.host().native("GetGameSpeed", &[]).unwrap(),
        Value::Handle(settings.speed)
    );
    vm.host().inner.admit_startup_version(&ctx, low).unwrap();
    assert_eq!(vm.call("read", &[]).unwrap(), Value::Bool(true));
}
#[test]
fn invalid_version_inputs_queries_and_contexts_preserve_current_identity() {
    let c = common();
    let (mut h, ctx) = object(&c, "// map", true);
    let low = version(&mut h, -17);
    h.admit_startup_version(&ctx, low).unwrap();
    let other = id(&h.native("ConvertOtherVersion", &[Value::Int(103)]).unwrap());
    let timer = id(&h.native("CreateTimer", &[]).unwrap());
    let unknown = HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    let before = h.inner().handles();
    for bad in [other, timer, unknown] {
        assert!(h.admit_startup_version(&ctx, bad).is_err());
        assert_eq!(h.native("VersionGet", &[]).unwrap(), Value::Handle(low));
        assert_eq!(h.inner().handles(), before);
    }
    for args in [
        vec![Value::Null],
        vec![Value::Int(0)],
        vec![Value::Handle(low)],
        vec![Value::Bool(false), Value::Null],
    ] {
        assert!(h.native("VersionGet", &args).is_err());
        assert_eq!(h.native("VersionGet", &[]).unwrap(), Value::Handle(low));
        assert_eq!(h.inner().handles(), before);
    }
    let same_text = context(301, &c, "// map", "synthetic:version");
    assert!(h.admit_startup_version(&same_text, low).is_err());
    assert_eq!(h.native("VersionGet", &[]).unwrap(), Value::Handle(low));
    let (mut unselected, ctx) = object(&c, "// map", false);
    let other = id(&unselected
        .native("ConvertOtherVersion", &[Value::Int(103)])
        .unwrap());
    assert!(unselected.admit_startup_version(&ctx, other).is_err());
    assert!(unselected.native("VersionGet", &[]).is_err());
    let mut bare = ObjectHost::checked(super::super::enums(&ctx, 64)).unwrap();
    assert!(
        bare.admit_startup_version(&ctx, low).is_err(),
        "players/context must already be admitted"
    );
}
#[test]
fn version_get_requires_loaded_contract_and_source_ancestry() {
    let c = common();
    for c in [
        c.replace(GET, ""),
        c.replace(GET, &format!("constant {GET}")),
        c.replace("VersionGet takes nothing", "VersionGet takes integer i"),
        c.replace(
            "VersionGet takes nothing returns version",
            "VersionGet takes nothing returns gamespeed",
        ),
    ] {
        let (mut h, ctx) = object(&c, "// map", true);
        let low = version(&mut h, -17);
        assert!(
            matches!(h.admit_startup_version(&ctx,low),Err(e) if e.failure==Failure::Signature)
        );
    }
    for (c, map) in [
        (c.replace(GET, ""), GET.to_string()),
        (c.clone(), GET.to_string()),
        (
            c.clone(),
            "function VersionGet takes nothing returns version\nreturn null\nendfunction\n".into(),
        ),
        (
            c.replace("type version extends handle", ""),
            "type version extends handle\n".into(),
        ),
    ] {
        let (mut h, ctx) = object(&c, &map, true);
        let low = version(&mut h, -17);
        assert!(h.admit_startup_version(&ctx, low).is_err());
    }
    let c = c.replace(
        "type version extends handle",
        "type versionparent extends handle\ntype version extends versionparent",
    );
    let (mut h, ctx) = object(&c, "// map", true);
    let low = version(&mut h, -17);
    h.admit_startup_version(&ctx, low).unwrap();
}
