use super::*;
use pcx_objects::editor::SlkId;
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_script::{HandleId, Value};

fn sylk(headers: &[&str], rows: &[Vec<String>]) -> Vec<u8> {
    let mut text = String::from("ID;PWXL;N;E\r\n");
    for (y, row) in std::iter::once(headers.iter().map(|cell| (*cell).to_owned()).collect())
        .chain(rows.iter().cloned())
        .enumerate()
    {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

fn cells(headers: &[&str], values: &[(&str, &str)]) -> Vec<String> {
    headers
        .iter()
        .map(|header| {
            values
                .iter()
                .find(|(name, _)| name == header)
                .map(|(_, value)| (*value).to_owned())
                .unwrap_or_else(|| "0".to_owned())
        })
        .collect()
}

/// Damage lives in DataD, the count in DataA and the reduction in DataB.
/// DataC is a decoy. Roles come from metadata type and maxVal, not row order.
fn tables(targs: &str) -> DefaultTables {
    let headers = [
        "alias",
        "code",
        "levels",
        "reqLevel",
        "levelSkip",
        "hero",
        "DataA1",
        "DataA2",
        "DataA3",
        "DataB1",
        "DataB2",
        "DataB3",
        "DataC1",
        "DataC2",
        "DataC3",
        "DataD1",
        "DataD2",
        "DataD3",
        "Cost1",
        "Cost2",
        "Cost3",
        "Cool1",
        "Cool2",
        "Cool3",
        "Rng1",
        "Rng2",
        "Rng3",
        "Area1",
        "Area2",
        "Area3",
        "targs1",
        "targs2",
        "targs3",
    ];
    let holy = "air,ground,organic,notself,invu,vuln,nonancient";
    let rows = [
        cells(
            &headers,
            &[
                ("alias", "AHhb"),
                ("code", "AHhb"),
                ("levels", "3"),
                ("reqLevel", "1"),
                ("hero", "1"),
                ("DataA1", "200"),
                ("DataA2", "400"),
                ("DataA3", "600"),
                ("Cost1", "65"),
                ("Cost2", "65"),
                ("Cost3", "65"),
                ("Cool1", "5"),
                ("Cool2", "5"),
                ("Cool3", "5"),
                ("Rng1", "60"),
                ("Rng2", "60"),
                ("Rng3", "60"),
                ("targs1", holy),
                ("targs2", holy),
                ("targs3", holy),
            ],
        ),
        cells(
            &headers,
            &[
                ("alias", "Yher"),
                ("code", "Yher"),
                ("levels", "1"),
                ("hero", "1"),
                ("targs1", "air,ground,invu,vuln"),
            ],
        ),
        cells(
            &headers,
            &[
                ("alias", "Zdmg"),
                ("code", "Zdmg"),
                ("levels", "3"),
                ("reqLevel", "1"),
                ("hero", "1"),
                ("DataA1", "3"),
                ("DataA2", "2"),
                ("DataA3", "1"),
                ("DataB1", "0.25"),
                ("DataB2", "0.5"),
                ("DataB3", "0"),
                ("DataC1", "999"),
                ("DataC2", "999"),
                ("DataC3", "999"),
                ("DataD1", "80"),
                ("DataD2", "100"),
                ("DataD3", "40"),
                ("Cost1", "17"),
                ("Cost2", "19"),
                ("Cost3", "5"),
                ("Cool1", "1.5"),
                ("Cool2", "4"),
                ("Cool3", "1"),
                ("Rng1", "40"),
                ("Rng2", "40"),
                ("Rng3", "10"),
                ("Area1", "25"),
                ("Area2", "25"),
                ("Area3", "0"),
                ("targs1", targs),
                ("targs2", targs),
                ("targs3", targs),
            ],
        ),
        cells(
            &headers,
            &[
                ("alias", "Zhel"),
                ("code", "Zhel"),
                ("levels", "1"),
                ("hero", "1"),
                ("DataA1", "9"),
                ("targs1", "air,ground,friend"),
            ],
        ),
    ];
    let meta_headers = [
        "ID",
        "field",
        "slk",
        "index",
        "type",
        "category",
        "minVal",
        "maxVal",
        "data",
        "useSpecific",
    ];
    let meta = |id, kind, min, max, data| {
        cells(
            &meta_headers,
            &[
                ("ID", id),
                ("field", "Data"),
                ("slk", "AbilityData"),
                ("index", "-1"),
                ("type", kind),
                ("category", "data"),
                ("minVal", min),
                ("maxVal", max),
                ("data", data),
                ("useSpecific", "Zdmg,Zhel"),
            ],
        )
    };
    let metadata = sylk(
        &meta_headers,
        &[
            meta("Ocl1", "int", "1", "16", "1"),
            meta("Ocl2", "unreal", "0", "1", "2"),
            meta("Ocl3", "unreal", "0", "99999", "4"),
        ],
    );
    let profile = "\
[AHhb]\r\n\
Order=alpha\r\n\
Hotkey=T\r\n\
Targetart=Abilities\\Spells\\Human\\HolyBolt\\HolyBoltSpecialArt.mdl\r\n\
UberTip=\"deal half damage to an enemy Undead unit.\"\r\n\
[Zdmg]\r\n\
Order=beta\r\n\
Hotkey=C\r\n\
Researchhotkey=Z\r\n\
LightningEffect=\"ZZPB,ZZSB\"\r\n\
Missileart=bolt.mdl\r\n\
Missilespeed=1500\r\n\
Targetart=hit.mdl\r\n\
UberTip=\"打撃\"\r\n\
Animnames=\"spell,chain\"\r\n\
[Zhel]\r\n\
Order=gamma\r\n\
Hotkey=E\r\n\
LightningEffect=\"HHPB,HHSB\"\r\n\
UberTip=\"heals 10 damage\"\r\n\
";
    DefaultTables::from_bytes(
        Identity {
            install: super::super::source().install,
            edition_version: "fixture".into(),
        },
        &[
            (SlkId::AbilityData.path(), &sylk(&headers, &rows)),
            (SlkId::AbilityMetaData.path(), &metadata),
        ],
        &[("Units\\OrcAbilityFunc.txt", profile.as_bytes())],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 256 << 10,
        },
    )
    .expect("synthetic chain tables")
}

fn open_with(
    types: &str,
    abilities: &str,
    heroes: &str,
    targs: &str,
    overlay: pcx_objects::editor::Overlay,
) -> (Vm, [HandleId; 4]) {
    let (mut vm, ids) = holy_open_with_inputs(
        types,
        abilities,
        heroes,
        HOLY_ART_END_MS,
        None,
        tables(targs),
        overlay,
    );
    vm.host().inner.admit_chain_lightning().unwrap();
    (vm, ids)
}

fn open(types: &str, abilities: &str, heroes: &str, targs: &str) -> (Vm, [HandleId; 4]) {
    open_with(types, abilities, heroes, targs, Default::default())
}

const TARGS: &str = "air,ground,enemy,neutral,organic";

fn cast(vm: &mut Vm, caster: HandleId, target: HandleId) -> bool {
    match vm
        .host()
        .inner
        .native(
            "IssueTargetOrder",
            &[
                Value::Handle(caster),
                Value::Str("beta".into()),
                Value::Handle(target),
            ],
        )
        .unwrap()
    {
        Value::Bool(cast) => cast,
        other => panic!("cast returned {other:?}"),
    }
}

fn life(vm: &mut Vm, unit: HandleId) -> f32 {
    vm.host().inner.unit_vitals(unit).unwrap().unwrap().life
}

fn mana(vm: &mut Vm, unit: HandleId) -> f32 {
    vm.host().inner.unit_vitals(unit).unwrap().unwrap().mana
}

fn set_life(vm: &mut Vm, unit: HandleId, value: f32) {
    let state = vm.global("UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(unit), state, Value::Real(value)],
        )
        .unwrap();
}

fn set_mana(vm: &mut Vm, unit: HandleId, value: f32) {
    let state = vm.global("UNIT_STATE_MANA");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(unit), state, Value::Real(value)],
        )
        .unwrap();
}

fn spawn(vm: &mut Vm, player: i32, x: f32) -> HandleId {
    spawn_at(vm, player, x, 16.0)
}

fn spawn_at(vm: &mut Vm, player: i32, x: f32, y: f32) -> HandleId {
    let owner = vm
        .host()
        .inner
        .native("Player", &[Value::Int(player)])
        .unwrap();
    match vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                owner,
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(x),
                Value::Real(y),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    {
        Value::Handle(id) => id,
        other => panic!("spawn returned {other:?}"),
    }
}

fn placed(vm: &mut Vm, unit: HandleId) -> [f32; 2] {
    let x = match vm
        .host()
        .inner
        .native("GetUnitX", &[Value::Handle(unit)])
        .unwrap()
    {
        Value::Real(value) => value,
        other => panic!("GetUnitX returned {other:?}"),
    };
    let y = match vm
        .host()
        .inner
        .native("GetUnitY", &[Value::Handle(unit)])
        .unwrap()
    {
        Value::Real(value) => value,
        other => panic!("GetUnitY returned {other:?}"),
    };
    [x, y]
}

fn span(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

#[test]
fn healing_row_is_not_a_command_and_its_damage_word_does_not_bind_it() {
    let (mut vm, [caster, _, enemy, _]) = open("", "Zhel", "_", TARGS);
    assert!(
        vm.host()
            .inner
            .chain_lightning_command(caster)
            .unwrap()
            .is_none()
    );
    vm.call("prime", &[]).unwrap();
    set_life(&mut vm, enemy, 500.0);
    let before = (life(&mut vm, enemy), mana(&mut vm, caster));
    assert!(!cast(&mut vm, caster, enemy));
    assert_eq!((life(&mut vm, enemy), mana(&mut vm, caster)), before);
}

#[test]
fn custom_alias_uses_metadata_ordinals_and_ignores_non_english_tip() {
    let mut overlay = pcx_objects::editor::Overlay::default();
    overlay
        .custom
        .insert("Zcus".into(), ("Zdmg".into(), Vec::new()));
    let (mut vm, [caster, _, enemy, _]) = open_with("", "Zcus", "_", TARGS, overlay);
    vm.call("prime", &[]).unwrap();
    let command = vm
        .host()
        .inner
        .chain_lightning_command(caster)
        .unwrap()
        .expect("learned custom alias");
    let order = vm
        .host()
        .inner
        .native("OrderId", &[Value::Str("beta".into())])
        .unwrap();
    assert_eq!(command.ability, *b"Zcus");
    assert_eq!(command.hotkey, 'C');
    assert_eq!(Value::Int(command.order), order);
    set_life(&mut vm, enemy, 500.0);
    let effects = vm.host().inner.test_chain_effect_count();
    assert!(cast(&mut vm, caster, enemy));
    assert_eq!(
        life(&mut vm, enemy),
        420.0,
        "damage is DataD, not the DataC decoy"
    );
    assert_eq!(mana(&mut vm, caster), 200.0 - 17.0);
    let casts = vm.host().inner.take_chain_lightning_casts();
    assert_eq!(casts.len(), 1);
    assert_eq!(casts[0].ability, *b"Zcus");
    assert_eq!(casts[0].missile_art.as_deref(), Some("bolt.mdl"));
    assert_eq!(casts[0].missile_speed.as_deref(), Some("1500"));
    assert_eq!(casts[0].segments.len(), 1);
    assert_eq!(casts[0].segments[0].effect.as_deref(), Some("ZZPB"));
    assert_eq!(casts[0].segments[0].source, caster);
    assert_eq!(casts[0].segments[0].target, enemy);
    assert_eq!(vm.host().inner.test_chain_effect_count(), effects);
}

#[test]
fn chain_pays_once_decays_and_emits_one_spell_event() {
    let (mut vm, [caster, friend, enemy, _]) = open("", "Zdmg", "_", TARGS);
    vm.call("prime", &[]).unwrap();
    vm.call("listen", &[]).unwrap();
    set_life(&mut vm, enemy, 500.0);
    set_life(&mut vm, friend, 500.0);
    let near = spawn(&mut vm, 5, 70.0);
    let next = spawn(&mut vm, 5, 84.0);
    let far = spawn(&mut vm, 5, 115.0);
    for unit in [near, next, far] {
        set_life(&mut vm, unit, 500.0);
    }
    assert!(cast(&mut vm, caster, enemy));
    assert_eq!(
        (
            life(&mut vm, enemy),
            life(&mut vm, near),
            life(&mut vm, next),
            life(&mut vm, far),
            life(&mut vm, friend),
            mana(&mut vm, caster)
        ),
        (420.0, 440.0, 455.0, 500.0, 500.0, 183.0)
    );
    assert!(
        !cast(&mut vm, caster, enemy),
        "cooldown refuses the second pay"
    );
    assert_eq!(mana(&mut vm, caster), 183.0);
    let mut events = Vec::new();
    while let Some(event) = vm.host().inner.take_fired_event() {
        events.push(event);
    }
    let spells: Vec<_> = events
        .iter()
        .filter(|event| event.subject.spell_target.is_some())
        .collect();
    assert_eq!(spells.len(), 1);
    assert_eq!(spells[0].subject.spell_target, Some(enemy));
    assert_eq!(spells[0].subject.unit, Some(caster));
    let casts = vm.host().inner.chain_lightning_casts();
    assert_eq!(
        casts[0]
            .segments
            .iter()
            .map(|segment| segment.effect.as_deref())
            .collect::<Vec<_>>(),
        [Some("ZZPB"), Some("ZZSB"), Some("ZZSB")]
    );
}

#[test]
fn equal_distance_prefers_the_lower_handle() {
    let (mut vm, [caster, _, enemy, _]) = open("", "Zdmg", "_", TARGS);
    vm.call("prime", &[]).unwrap();
    set_life(&mut vm, enemy, 500.0);
    // 12-9-15 from the enemy. Placement may move a requested point, so the
    // tie is asserted from the coordinates the map actually kept.
    let first = spawn_at(&mut vm, 5, 71.0, 16.0);
    let second = spawn_at(&mut vm, 5, 68.0, 25.0);
    set_life(&mut vm, first, 500.0);
    set_life(&mut vm, second, 500.0);
    let enemy_at = placed(&mut vm, enemy);
    let first_at = placed(&mut vm, first);
    let second_at = placed(&mut vm, second);
    let from_enemy = span(enemy_at, first_at);
    let other = span(enemy_at, second_at);
    assert_eq!(
        from_enemy, other,
        "placed {first_at:?} and {second_at:?} are not equidistant from {enemy_at:?}"
    );
    assert!(
        from_enemy <= 25.0 && span(first_at, second_at) <= 25.0,
        "equal pair sits outside the bounce area: {first_at:?} {second_at:?} from {enemy_at:?}"
    );
    assert!(cast(&mut vm, caster, enemy));
    let (low, high) = if first < second {
        (first, second)
    } else {
        (second, first)
    };
    assert_eq!(life(&mut vm, low), 440.0);
    assert_eq!(life(&mut vm, high), 455.0);
}

#[test]
fn refusals_leave_mana_and_life_unchanged() {
    let (mut vm, [caster, friend, enemy, _]) = open("", "Zdmg", "_", TARGS);
    vm.call("prime", &[]).unwrap();
    set_life(&mut vm, enemy, 500.0);
    set_life(&mut vm, friend, 500.0);
    let mana_before = mana(&mut vm, caster);
    assert!(!cast(&mut vm, caster, friend));
    assert_eq!(mana(&mut vm, caster), mana_before);
    vm.host()
        .inner
        .native(
            "SetUnitInvulnerable",
            &[Value::Handle(enemy), Value::Bool(true)],
        )
        .unwrap();
    assert!(!cast(&mut vm, caster, enemy));
    assert_eq!(
        (life(&mut vm, enemy), mana(&mut vm, caster)),
        (500.0, mana_before)
    );
    vm.host()
        .inner
        .native(
            "SetUnitInvulnerable",
            &[Value::Handle(enemy), Value::Bool(false)],
        )
        .unwrap();
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(enemy)])
        .unwrap();
    assert!(!cast(&mut vm, caster, enemy));
    assert_eq!(mana(&mut vm, caster), mana_before);
}

#[test]
fn organic_token_is_required_only_when_present() {
    let (mut vm, [caster, _, enemy, _]) = open("mechanical", "Zdmg", "_", TARGS);
    vm.call("prime", &[]).unwrap();
    set_life(&mut vm, enemy, 500.0);
    let before = mana(&mut vm, caster);
    assert!(!cast(&mut vm, caster, enemy));
    assert_eq!(
        (life(&mut vm, enemy), mana(&mut vm, caster)),
        (500.0, before)
    );

    let (mut vm, [caster, _, enemy, _]) =
        open("mechanical", "Zdmg", "_", "air,ground,enemy,neutral");
    vm.call("prime", &[]).unwrap();
    set_life(&mut vm, enemy, 500.0);
    assert!(cast(&mut vm, caster, enemy));
    assert_eq!(life(&mut vm, enemy), 420.0);
}

#[test]
fn invulnerable_token_allows_a_target_that_damage_unit_does_not_hurt() {
    let (mut vm, [caster, _, enemy, _]) =
        open("", "Zdmg", "_", "air,ground,enemy,neutral,invu,vuln");
    vm.call("prime", &[]).unwrap();
    set_life(&mut vm, enemy, 500.0);
    vm.host()
        .inner
        .native(
            "SetUnitInvulnerable",
            &[Value::Handle(enemy), Value::Bool(true)],
        )
        .unwrap();
    assert!(cast(&mut vm, caster, enemy));
    assert_eq!(life(&mut vm, enemy), 500.0);
    assert_eq!(mana(&mut vm, caster), 183.0);
}

#[test]
fn unlearned_command_is_absent_and_rank_two_reads_its_own_cells() {
    let (mut vm, [caster, _, enemy, _]) = open("", "_", "Zdmg", TARGS);
    assert!(
        vm.host()
            .inner
            .chain_lightning_command(caster)
            .unwrap()
            .is_none()
    );
    vm.call("prime", &[]).unwrap();
    vm.call("learn", &[Value::Int(i32::from_be_bytes(*b"Zdmg"))])
        .unwrap();
    let command = vm
        .host()
        .inner
        .chain_lightning_command(caster)
        .unwrap()
        .expect("rank one");
    assert_eq!((command.ability, command.hotkey), (*b"Zdmg", 'C'));
    set_life(&mut vm, enemy, 500.0);
    assert!(cast(&mut vm, caster, enemy));
    assert_eq!(life(&mut vm, enemy), 420.0);
    step(&mut vm, 1.5);
    vm.call("learn", &[Value::Int(i32::from_be_bytes(*b"Zdmg"))])
        .unwrap();
    let bounce = spawn(&mut vm, 5, 70.0);
    set_life(&mut vm, bounce, 500.0);
    set_life(&mut vm, enemy, 500.0);
    assert!(cast(&mut vm, caster, enemy));
    assert_eq!(life(&mut vm, enemy), 400.0);
    assert_eq!(life(&mut vm, bounce), 450.0);
    assert_eq!(mana(&mut vm, caster), 200.0 - 17.0 - 19.0);
}

#[test]
fn reset_and_death_drop_a_pending_cast() {
    let (mut vm, [caster, _, _, _]) = open("", "Zdmg", "_", TARGS);
    vm.call("prime", &[]).unwrap();
    let far = spawn(&mut vm, 5, 110.0);
    assert!(cast(&mut vm, caster, far));
    assert!(vm.host().inner.test_chain_pending(caster));
    assert_eq!(mana(&mut vm, caster), 200.0);
    vm.host()
        .inner
        .native("UnitResetCooldown", &[Value::Handle(caster)])
        .unwrap();
    assert!(!vm.host().inner.test_chain_pending(caster));
    assert!(cast(&mut vm, caster, far));
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(caster)])
        .unwrap();
    assert!(!vm.host().inner.test_chain_pending(caster));
}

#[test]
fn short_mana_and_alliance_do_not_pay() {
    let (mut vm, [caster, _, enemy, _]) = open("", "Zdmg", "_", TARGS);
    vm.call("prime", &[]).unwrap();
    set_life(&mut vm, enemy, 500.0);
    set_mana(&mut vm, caster, 1.0);
    assert!(!cast(&mut vm, caster, enemy));
    assert_eq!((life(&mut vm, enemy), mana(&mut vm, caster)), (500.0, 1.0));
    set_mana(&mut vm, caster, 200.0);
    let alliance = vm.global("ALLIANCE_PASSIVE");
    let from = vm.host().inner.executed_unit(caster).unwrap().owner;
    let to = vm.host().inner.executed_unit(enemy).unwrap().owner;
    vm.host()
        .inner
        .native(
            "SetPlayerAlliance",
            &[
                Value::Handle(from),
                Value::Handle(to),
                alliance,
                Value::Bool(true),
            ],
        )
        .unwrap();
    assert!(!cast(&mut vm, caster, enemy));
    assert_eq!(
        (life(&mut vm, enemy), mana(&mut vm, caster)),
        (500.0, 200.0)
    );
}
