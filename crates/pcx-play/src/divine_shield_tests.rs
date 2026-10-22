//! Divine Shield (`AHds`) harness: synthetic ranked cells deliberately unlike
//! the original (Dur 3/6/9, Cool 10/20/30, Cost 5/6/7, levels 3) so any
//! accidental dependence on copied source constants fails.
use super::*;
use crate::playercolor_host::HeroBudget;
use pcx_objects::editor::misc::HeroVitals;
use pcx_objects::editor::{Overlay, SlkId};

const DECLS: &str = r#"
native IssueImmediateOrder takes unit u, string order returns boolean
native SelectHeroSkill takes unit u, integer ability returns nothing
constant native GetHeroLevel takes unit u returns integer
constant native GetUnitAbilityLevel takes unit u, integer ability returns integer
native SetUnitInvulnerable takes unit u, boolean flag returns nothing
constant native BlzIsUnitInvulnerable takes unit u returns boolean
native SetPlayerAbilityAvailable takes player p, integer ability, boolean available returns nothing
globals
real ObsMana = 0.0
endglobals
"#;

const MAP: &str = r#"
globals
unit H=null
unit F=null
endglobals
function setup takes nothing returns nothing
set H=CreateUnit(Player(4),'H001',16.0,16.0,0.0)
set F=CreateUnit(Player(4),'h001',48.0,16.0,0.0)
endfunction
function learn takes integer ability returns nothing
call SelectHeroSkill(H,ability)
endfunction
function shield_on takes nothing returns boolean
return IssueImmediateOrder(H,"divineshield")
endfunction
function shield_off takes nothing returns boolean
return IssueImmediateOrder(H,"undivineshield")
endfunction
function record_mana takes nothing returns nothing
set ObsMana = GetUnitState(H,UNIT_STATE_MANA)
endfunction
function set_prot takes boolean flag returns nothing
call SetUnitInvulnerable(H,flag)
endfunction
function observed_prot takes nothing returns boolean
return BlzIsUnitInvulnerable(H)
endfunction
"#;

fn shield_tables() -> DefaultTables {
    let headers = [
        "alias",
        "code",
        "levels",
        "reqLevel",
        "levelSkip",
        "hero",
        "Dur1",
        "Dur2",
        "Dur3",
        "Cool1",
        "Cool2",
        "Cool3",
        "Cost1",
        "Cost2",
        "Cost3",
        "BuffID1",
        "BuffID2",
        "BuffID3",
    ];
    let rows: [Vec<&str>; 2] = [
        vec![
            "AHds", "AHds", "3", "1", "0", "1", "3", "6", "9", "10", "20", "30", "5", "6", "7",
            "BHds", "BHds", "BHds",
        ],
        vec![
            "Yher", "Yher", "1", "0", "0", "1", "0", "0", "0", "0", "0", "0", "0", "0", "0",
            "BHds", "BHds", "BHds",
        ],
    ];
    let metadata = sylk(
        &[
            "ID", "field", "slk", "index", "type", "category", "repeat", "data",
        ],
        &[
            &[
                "fDur",
                "Dur",
                "AbilityData",
                "-1",
                "unreal",
                "stats",
                "1",
                "0",
            ],
            &[
                "fCoo",
                "Cool",
                "AbilityData",
                "-1",
                "unreal",
                "stats",
                "1",
                "0",
            ],
            &[
                "fCos",
                "Cost",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "1",
                "0",
            ],
            &[
                "fLev",
                "levels",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "0",
                "0",
            ],
            &[
                "fReq",
                "reqLevel",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "0",
                "0",
            ],
            &[
                "fSkp",
                "levelSkip",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "0",
                "0",
            ],
            &[
                "fBuf",
                "BuffID",
                "AbilityData",
                "-1",
                "buffCode",
                "stats",
                "1",
                "0",
            ],
        ],
    );
    let mut slk = String::from("ID;PWXL;N;E\r\n");
    for (column, name) in headers.iter().enumerate() {
        slk.push_str(&format!("C;X{};Y1;K\"{name}\"\r\n", column + 1));
    }
    for (index, row) in rows.iter().enumerate() {
        for (column, value) in row.iter().enumerate() {
            let value = if value.parse::<f64>().is_ok() {
                (*value).to_owned()
            } else {
                format!("\"{value}\"")
            };
            slk.push_str(&format!("C;X{};Y{};K{value}\r\n", column + 1, index + 2));
        }
    }
    slk.push_str("E\r\n");
    let profile = "[AHds]\r\nOrder=divineshield\r\nUnorder=undivineshield\r\nBuffID=BHds\r\n";
    DefaultTables::from_bytes(
        Identity {
            install: source().install,
            edition_version: "fixture".into(),
        },
        &[
            (SlkId::AbilityData.path(), slk.as_bytes()),
            (SlkId::AbilityMetaData.path(), &metadata),
        ],
        &[(
            "Units\\HumanAbilityFunc.txt",
            format!("{profile}\r\n[AHwe]\r\nOrder=gamma\r\nHotkey=W\r\n").as_bytes(),
        )],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 128 << 10,
        },
    )
    .expect("synthetic divine shield tables")
}

fn open() -> (Vm, HandleId) {
    let mut fields = hero_fields("STR");
    fields.push(column(
        b"H001",
        SlkId::UnitBalance,
        "HP",
        FieldKind::Int,
        "1000",
    ));
    fields.push(column(
        b"H001",
        SlkId::UnitBalance,
        "mana0",
        FieldKind::Int,
        "300",
    ));
    fields.push(column(
        b"h001",
        SlkId::UnitBalance,
        "HP",
        FieldKind::Int,
        "420",
    ));
    fields.push(column(
        b"A010",
        SlkId::UnitBalance,
        "type",
        FieldKind::Token,
        "ground,human",
    ));
    fields.push(column(
        b"A008",
        SlkId::UnitAbilities,
        "abilList",
        FieldKind::Token,
        "AHds",
    ));
    fields.push(column(
        b"A009",
        SlkId::UnitAbilities,
        "heroAbilList",
        FieldKind::Token,
        "AHds",
    ));
    let extra = format!(
        "{}\n{}{DECLARATIONS}{CLOCK_DECLARATIONS}{DECLS}",
        enumeration::TRIGGERS,
        super::activity::ORDERS,
    );
    let (registry, activity) = super::activity::rules();
    let h = position_host_admitting(1000, 16, MAP, &extra, &fields, |h| {
        h.with_startup_clock(crate::startup_clock::tests::input(9), "CLOCK", 64, 16, 1000)
            .unwrap()
    })
    .with_player_abilities(16)
    .unwrap()
    .with_player_unit_events(8)
    .unwrap()
    .with_unit_states(crate::playercolor_host::UnitStateBudget { max_records: 8 })
    .unwrap()
    .with_player_alliances(crate::playercolor_host::PlayerAllianceBudget { max_records: 8 })
    .unwrap()
    .with_unit_explosions(8)
    .unwrap()
    .with_order_registry(registry)
    .unwrap()
    .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
    .unwrap();
    let origin = crate::scripts::AssetIdentity {
        source: source().install,
        edition: "fixture".into(),
        version: "fixture".into(),
    };
    let hero_budget = HeroBudget {
        abilities_per_unit: 8,
        overlay_objects: 0,
        modifications_per_ability: 0,
    };
    let (rules, combat) = hero_rules();
    let mut h = h
        .with_hero_skills(
            shield_tables(),
            origin,
            Overlay::default(),
            rules,
            hero_budget,
        )
        .unwrap();
    h.admit_unit_activity(activity, 65536).unwrap();
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.0,
        mana_per_intelligence: 100.0,
        life_regen_per_strength: 0.0,
        mana_regen_per_intelligence: 0.0,
    })
    .unwrap();
    h.admit_combat(combat, "beta").unwrap();
    // The synthetic activity registry has no divine orders; seed the shield
    // rules directly to exercise the cast/expiry/protection mechanics. The
    // admission path is tested separately for its registry-refusal.
    h.seed_divine_shield_for_test(9001, 9002);
    h.set_neutral_passive(15);
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let id = match vm.global("H") {
        Value::Handle(id) => id,
        value => panic!("hero missing {value:?}"),
    };
    (vm, id)
}

fn learn_rank_3(vm: &mut Vm, hero: HandleId) {
    for _ in 0..3 {
        vm.host()
            .inner
            .native(
                "SelectHeroSkill",
                &[Value::Handle(hero), Value::Int(0x41486473)],
            )
            .unwrap();
    }
}

#[test]
fn shield_on_off_mana_and_fresh_active_off_false() {
    let (mut vm, hero) = open();
    learn_rank_3(&mut vm, hero);
    // fresh off is false (observed)
    assert!(!vm.host().inner.uncast_divine_shield(hero).unwrap());
    vm.call("record_mana", &[]).unwrap();
    let before = match vm.global("ObsMana") {
        Value::Real(value) => value,
        value => panic!("mana missing {value:?}"),
    };
    // cast uses the learned rank's authored cost
    assert!(vm.host().inner.cast_divine_shield(hero).unwrap());
    vm.call("record_mana", &[]).unwrap();
    let after = match vm.global("ObsMana") {
        Value::Real(value) => value,
        value => panic!("mana missing {value:?}"),
    };
    let rank = match vm
        .host()
        .inner
        .native(
            "GetUnitAbilityLevel",
            &[Value::Handle(hero), Value::Int(0x41486473)],
        )
        .unwrap()
    {
        Value::Int(rank) => rank,
        value => panic!("rank missing {value:?}"),
    };
    let cost = match rank {
        1 => 5.0,
        2 => 6.0,
        3 => 7.0,
        other => panic!("unexpected rank {other}"),
    };
    assert_eq!(after, before - cost);
    // active off is still false (observed)
    assert!(!vm.host().inner.uncast_divine_shield(hero).unwrap());
}

#[test]
fn shield_getter_inverts_effective_protection() {
    let (mut vm, hero) = open();
    learn_rank_3(&mut vm, hero);
    let getter = |vm: &mut Vm| {
        vm.host()
            .inner
            .native("BlzIsUnitInvulnerable", &[Value::Handle(hero)])
            .unwrap()
    };
    let setter = |vm: &mut Vm, flag: bool| {
        vm.host()
            .inner
            .native(
                "SetUnitInvulnerable",
                &[Value::Handle(hero), Value::Bool(flag)],
            )
            .unwrap();
    };
    // initial unknown refuses
    assert!(
        vm.host()
            .inner
            .native("BlzIsUnitInvulnerable", &[Value::Handle(hero)])
            .unwrap_err()
            .contains("initial")
    );
    setter(&mut vm, true);
    assert_eq!(getter(&mut vm), Value::Bool(false));
    setter(&mut vm, false);
    assert_eq!(getter(&mut vm), Value::Bool(true));
    // active shield contributes protection: explicit false + shield -> getter false
    assert!(vm.host().inner.cast_divine_shield(hero).unwrap());
    assert_eq!(getter(&mut vm), Value::Bool(false));
}

#[test]
fn explicit_settler_persists_after_shield_expiry() {
    let (mut vm, hero) = open();
    learn_rank_3(&mut vm, hero);
    // explicit protected during active effect (rank 3 duration = 9s)
    assert!(vm.host().inner.cast_divine_shield(hero).unwrap());
    vm.host()
        .inner
        .native(
            "SetUnitInvulnerable",
            &[Value::Handle(hero), Value::Bool(true)],
        )
        .unwrap();
    step(&mut vm, 10.0);
    // shield expired; explicit true still protected -> getter false
    assert_eq!(
        vm.host()
            .inner
            .native("BlzIsUnitInvulnerable", &[Value::Handle(hero)])
            .unwrap(),
        Value::Bool(false)
    );
    vm.host()
        .inner
        .native(
            "SetUnitInvulnerable",
            &[Value::Handle(hero), Value::Bool(false)],
        )
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native("BlzIsUnitInvulnerable", &[Value::Handle(hero)])
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn family_removed_and_no_ability_do_not_write_early() {
    let (mut vm, hero) = open();
    learn_rank_3(&mut vm, hero);
    let other = match vm.global("F") {
        Value::Handle(id) => id,
        value => panic!("F missing {value:?}"),
    };
    let player = match vm.host().inner.native("Player", &[Value::Int(4)]).unwrap() {
        Value::Handle(id) => id,
        value => panic!("player missing {value:?}"),
    };
    // A non-unit handle is not live, so it returns false without writing.
    assert!(!vm.host().inner.cast_divine_shield(player).unwrap());
    assert!(!vm.host().inner.uncast_divine_shield(player).unwrap());
    // A non-hero unit without the ability returns false (no early write).
    assert!(!vm.host().inner.cast_divine_shield(other).unwrap());
}
