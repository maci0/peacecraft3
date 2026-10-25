use super::combat;
use super::*;
use pcx_objects::editor::misc::HeroVitals;

const DECLARATION: &str =
    "native CreateCorpse takes player p, integer kind, real x, real y, real facing returns unit\n";
fn fixture() -> pcx_script::OwnedVm<ObjectHost> {
    fixture_with_decay(None)
}
fn fixture_with_decay(decay: Option<(&str, &str, &str)>) -> pcx_script::OwnedVm<ObjectHost> {
    let mut fields = if decay.is_some() {
        combat::hero_fields("STR")
    } else {
        combat::armed("64", "normal")
    };
    for (id, field) in &mut fields {
        if id == b"H006" {
            field.value = Ok(FieldValue::Cell("3.5".into()));
        }
        if id == b"H008" {
            field.value = Ok(FieldValue::Cell("always".into()));
        }
    }
    fields.extend([
        combat::column(b"F001", SlkId::UnitBalance, "fused", FieldKind::Int, "7"),
        combat::column(b"F002", SlkId::UnitBalance, "fmade", FieldKind::Int, "13"),
    ]);
    if let Some((death, flags, structure)) = decay {
        fields.extend([
            combat::column(b"D001", SlkId::UnitData, "death", FieldKind::Unreal, death),
            combat::column(
                b"D002",
                SlkId::UnitData,
                "deathType",
                FieldKind::Token,
                flags,
            ),
            combat::column(
                b"D003",
                SlkId::UnitBalance,
                "isbldg",
                FieldKind::Bool,
                structure,
            ),
        ]);
    }
    for (_, field) in fields
        .iter_mut()
        .filter(|(id, _)| matches!(id, b"F001" | b"F002"))
    {
        field.metadata.as_mut().unwrap().declared = "int".into();
    }
    let declarations = format!(
        "{}{}{}{}\nnative RemoveUnit takes unit u returns nothing\nnative SetUnitAnimation takes unit u, string s returns nothing\nnative UnitSuspendDecay takes unit u, boolean suspend returns nothing\n",
        super::enumeration::TRIGGERS,
        super::activity::ORDERS,
        combat::DECLARATIONS,
        DECLARATION
    );
    let h = position_host_admitting(
        10000,
        16,
        "// synthetic corpse map",
        &declarations,
        &fields,
        |h| h,
    );
    let (registry, activity) = super::activity::rules();
    let mut h = h
        .with_order_registry(registry)
        .unwrap()
        .with_player_unit_events(8)
        .unwrap()
        .with_unit_states(UnitStateBudget { max_records: 16 })
        .unwrap()
        .with_player_alliances(crate::playercolor_host::PlayerAllianceBudget { max_records: 8 })
        .unwrap()
        .with_player_state_requests(crate::playercolor_host::PlayerStateRequestBudget {
            max_records: 16,
        })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules())
        .unwrap()
        .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
        .unwrap();
    if decay.is_some() {
        let origin = crate::scripts::AssetIdentity {
            source: source().install,
            edition: "fixture".into(),
            version: "fixture".into(),
        };
        let tables = pcx_objects::editor::defaults::DefaultTables::from_bytes(
            pcx_objects::editor::defaults::Identity {
                install: origin.source.clone(),
                edition_version: origin.version.clone(),
            },
            &[],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 0,
                max_asset_bytes: 1024,
                max_aggregate_input_bytes: 1024,
            },
        )
        .unwrap();
        h = h
            .with_hero_skills(
                tables,
                origin,
                Default::default(),
                crate::hero_skills::tests::rules(),
                crate::playercolor_host::HeroBudget {
                    abilities_per_unit: 8,
                    overlay_objects: 0,
                    modifications_per_ability: 0,
                },
            )
            .unwrap();
    }
    h.admit_unit_activity(activity, 10000).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.,
        mana_per_intelligence: 15.,
        life_regen_per_strength: 0.,
        mana_regen_per_intelligence: 0.,
    })
    .unwrap();
    h.admit_combat(combat::rules(), "alpha").unwrap();
    h.set_neutral_passive(15);
    if decay.is_some() {
        let mut rules = crate::unit_decay::Rules::fixture();
        rules.image_sha256 = h.player_client_sha256().unwrap();
        h.admit_unit_decay(rules).unwrap();
    }
    let mut model = crate::unit_model::tests::model();
    if decay.is_some() {
        let mut bytes = model.bytes.to_vec();
        let parsed = pcx_mdl::Mdlx::parse(&bytes).unwrap();
        let chunk = parsed.chunk(pcx_mdl::tag::SEQS).unwrap();
        let size = chunk.size;
        let offset = chunk.offset;
        assert_eq!(chunk.payload_offset + chunk.size, bytes.len());
        for (name, start, end) in [
            ("Decay Flesh", 5000u32, 5700u32),
            ("Decay Bone", 7000, 7750),
            ("Dissipate", 9000, 9750),
        ] {
            let mut sequence = vec![0; pcx_mdl::SEQUENCE_SIZE];
            sequence[..name.len()].copy_from_slice(name.as_bytes());
            sequence[80..84].copy_from_slice(&start.to_le_bytes());
            sequence[84..88].copy_from_slice(&end.to_le_bytes());
            sequence[92..96].copy_from_slice(&1u32.to_le_bytes());
            bytes.extend(sequence);
        }
        bytes[offset + 4..offset + 8]
            .copy_from_slice(&((size + 3 * pcx_mdl::SEQUENCE_SIZE) as u32).to_le_bytes());
        model = Arc::new(
            crate::unit_model::Model::read(
                model.origin.clone(),
                model.member.clone(),
                bytes.into(),
                4096,
            )
            .unwrap(),
        );
    }
    h.admit_unit_models(move |d| {
        if d.rawcode == *b"FAIL" {
            return Err("synthetic unreadable corpse model".into());
        }
        Ok(Some(Arc::clone(&model)))
    })
    .unwrap();
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm
}
fn id(v: &Value) -> pcx_script::HandleId {
    let Value::Handle(id) = v else {
        panic!("handle")
    };
    *id
}
fn args(owner: Value, rawcode: &[u8; 4]) -> [Value; 5] {
    [
        owner,
        Value::Int(i32::from_be_bytes(*rawcode)),
        Value::Real(72.0005),
        Value::Real(48.),
        Value::Real(37.),
    ]
}
#[test]
fn corpse_is_dead_on_the_canonical_unit_without_a_kill_or_occupancy() {
    let mut vm = fixture();
    let life = vm.global("UNIT_STATE_LIFE");
    let event = vm.global("EVENT_PLAYER_UNIT_DEATH");
    let food = vm.global("PLAYER_STATE_RESOURCE_FOOD_USED");
    let h = vm.host();
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    h.native(
        "TriggerRegisterPlayerUnitEvent",
        &[trigger, owner.clone(), event, Value::Null],
    )
    .unwrap();
    let before = h
        .native("GetPlayerState", &[owner.clone(), food.clone()])
        .unwrap();
    let corpse = h
        .native("CreateCorpse", &args(owner.clone(), b"u001"))
        .unwrap();
    let record = h.executed_unit(id(&corpse)).unwrap();
    assert!(record.combat.as_ref().unwrap().dead);
    assert_eq!(record.owner, id(&owner));
    assert_eq!(record.face, 37.);
    assert_eq!(record.vitals.unwrap().life, 0.);
    assert_eq!(record.vitals.unwrap().max_life, 20.);
    assert!(!record.summoned && !record.cached);
    let position = record.position;
    let animation = record.animation.as_ref().unwrap();
    assert_eq!(
        animation.model.sequences[animation.sequence.unwrap()].name,
        "Death"
    );
    assert_eq!(
        h.native("GetUnitState", &[corpse.clone(), life]).unwrap(),
        Value::Real(0.)
    );
    assert_eq!(
        h.native("GetPlayerState", &[owner.clone(), food.clone()])
            .unwrap(),
        before
    );
    assert!(h.take_fired_event().is_none());
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                corpse.clone(),
                Value::Str("gamma".into()),
                Value::Real(16.),
                Value::Real(16.)
            ]
        )
        .unwrap(),
        Value::Bool(false)
    );
    h.native("KillUnit", std::slice::from_ref(&corpse)).unwrap();
    assert!(h.take_fired_event().is_none());
    let living = h
        .native("CreateUnit", &args(owner.clone(), b"u001"))
        .unwrap();
    assert_eq!(h.executed_unit(id(&living)).unwrap().position, position);
    let Value::Int(before) = before else {
        panic!("food")
    };
    assert_eq!(
        h.native("GetPlayerState", &[owner.clone(), food]).unwrap(),
        Value::Int(before + 7)
    );
    for shown in [false, true] {
        h.native("ShowUnit", &[corpse.clone(), Value::Bool(shown)])
            .unwrap();
    }
    assert_eq!(h.executed_unit(id(&corpse)).unwrap().position, position);
    h.native("KillUnit", std::slice::from_ref(&living)).unwrap();
    assert!(
        h.take_fired_event().is_some(),
        "the registered live-unit death is a real positive control"
    );
    h.native("RemoveUnit", &[living]).unwrap();
    let replacement = h.native("CreateUnit", &args(owner, b"u001")).unwrap();
    assert_eq!(
        h.executed_unit(id(&replacement)).unwrap().position,
        position
    );
    h.set_timer_game_time(4.).unwrap();
    h.advance_unit_activity(4., &mut 10000).unwrap();
    assert_eq!(
        h.executed_unit(id(&corpse)).unwrap().vitals.unwrap().life,
        0.
    );
}
#[test]
fn corpse_refusals_allocate_nothing_and_validate_the_owned_abi() {
    let mut vm = fixture();
    let h = vm.host();
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    let before = h.inner().handles();
    for bad in [
        args(timer, b"u001"),
        args(owner.clone(), b"FAIL"),
        [
            owner.clone(),
            Value::Int(0),
            Value::Real(f32::NAN),
            Value::Real(1.),
            Value::Real(2.),
        ],
    ] {
        let error = h.native("CreateCorpse", &bad).unwrap_err();
        assert!(error.contains("CreateCorpse"), "{error}");
        assert_eq!(h.inner().handles(), before);
        assert_eq!(h.executed_units().count(), 0);
    }
    assert_eq!(
        h.native("CreateCorpse", &args(Value::Null, b"u001"))
            .unwrap(),
        Value::Null
    );
    assert_eq!(h.inner().handles(), before);
    for _ in 0..8 {
        h.native("CreateCorpse", &args(owner.clone(), b"u001"))
            .unwrap();
    }
    assert_eq!(h.executed_units().count(), 8);
    let full = h.inner().handles();
    assert!(h.native("CreateCorpse", &args(owner, b"u001")).is_err());
    assert_eq!(h.inner().handles(), full);
    for declaration in [
        DECLARATION.replace("real facing", "integer facing"),
        DECLARATION.replace("native CreateCorpse", "constant native CreateCorpse"),
        DECLARATION.replace("returns unit", "returns item"),
    ] {
        let common = script_common_with("widget", true, &declaration);
        let ctx = context(300, &common, "// synthetic corpse ABI", "corpse-ABI");
        let (catalog, _, _) = counting();
        let host = object(&ctx, 32);
        assert!(
            host.with_unit_catalog(catalog, source(), limits(2))
                .is_err()
        );
    }
}

fn decay_step(h: &mut ObjectHost, at: f64) {
    h.set_timer_game_time(at).unwrap();
    h.advance_unit_activity(at, &mut 10000).unwrap();
}
fn phase(h: &ObjectHost, unit: &Value) -> crate::unit_decay::Phase {
    h.executed_unit(id(unit))
        .unwrap()
        .decay
        .as_ref()
        .unwrap()
        .phase()
}
#[test]
fn suspension_changes_the_shared_decay_clock_but_not_the_animation_or_other_corpses() {
    use crate::unit_decay::Phase::*;
    let mut vm = fixture_with_decay(Some(("0.5", "12", "0")));
    let h = vm.host();
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let a = h
        .native("CreateCorpse", &args(owner.clone(), b"u001"))
        .unwrap();
    let b = h.native("CreateCorpse", &args(owner, b"u002")).unwrap();
    assert_eq!(phase(h, &a), Dying);
    h.native("UnitSuspendDecay", &[a.clone(), Value::Bool(true)])
        .unwrap();
    assert!(
        !h.executed_unit(id(&a))
            .unwrap()
            .decay
            .as_ref()
            .unwrap()
            .suspended()
    );
    decay_step(h, 0.75);
    assert_eq!(phase(h, &a), Flesh);
    assert_eq!(
        h.executed_unit(id(&a))
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .sample(0.75)
            .unwrap()
            .unwrap()
            .frame,
        5233
    );
    h.native("UnitSuspendDecay", &[a.clone(), Value::Bool(true)])
        .unwrap();
    let paused = h.executed_unit(id(&a)).unwrap().decay.clone();
    h.native("UnitSuspendDecay", &[a.clone(), Value::Bool(true)])
        .unwrap();
    assert_eq!(h.executed_unit(id(&a)).unwrap().decay, paused);
    decay_step(h, 4.);
    assert!(h.executed_unit(id(&b)).is_err());
    assert_eq!(phase(h, &a), Flesh);
    assert_eq!(
        h.executed_unit(id(&a))
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .sample(4.)
            .unwrap()
            .unwrap()
            .frame,
        5700
    );
    h.native("UnitSuspendDecay", &[a.clone(), Value::Bool(false)])
        .unwrap();
    assert_eq!(
        h.executed_unit(id(&a))
            .unwrap()
            .decay
            .as_ref()
            .unwrap()
            .deadline(),
        Some(4.5)
    );
    h.native("UnitSuspendDecay", &[a.clone(), Value::Bool(false)])
        .unwrap();
    decay_step(h, 4.5);
    assert_eq!(phase(h, &a), Bone);
    assert_eq!(
        h.executed_unit(id(&a))
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .sample(4.5)
            .unwrap()
            .unwrap()
            .frame,
        7000
    );
    decay_step(h, 5.75);
    assert!(h.executed_unit(id(&a)).is_err());
    assert!(h.take_fired_event().is_none());
    h.native("UnitSuspendDecay", &[a, Value::Bool(false)])
        .unwrap();
}
#[test]
fn ordinary_death_uses_the_same_decay_and_large_steps_preserve_authored_boundaries() {
    use crate::unit_decay::Phase::*;
    let run = |split: bool| {
        let mut vm = fixture_with_decay(Some(("0.5", "12", "0")));
        let h = vm.host();
        let owner = h.native("Player", &[Value::Int(4)]).unwrap();
        let unit = h.native("CreateUnit", &args(owner, b"u001")).unwrap();
        h.native("UnitSuspendDecay", &[unit.clone(), Value::Bool(true)])
            .unwrap();
        assert_eq!(phase(h, &unit), Alive);
        h.native("KillUnit", std::slice::from_ref(&unit)).unwrap();
        if split {
            for t in [0.5, 0.75, 1., 1.25, 1.5] {
                decay_step(h, t)
            }
        } else {
            decay_step(h, 1.5)
        }
        assert_eq!(phase(h, &unit), Bone);
        let record = h.executed_unit(id(&unit)).unwrap().clone();
        assert_eq!(record.animation.as_ref().unwrap().started, 1.25);
        (
            record.decay,
            record.animation.as_ref().unwrap().sample(1.5).unwrap(),
        )
    };
    assert_eq!(run(false), run(true));
}
#[test]
fn structure_and_hero_decay_use_their_source_durations_and_hero_identity_survives() {
    use crate::unit_decay::Phase::*;
    let mut vm = fixture_with_decay(Some(("0.5", "8", "1")));
    let h = vm.host();
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let building = h
        .native("CreateCorpse", &args(owner.clone(), b"u001"))
        .unwrap();
    let hero = h.native("CreateCorpse", &args(owner, b"H001")).unwrap();
    decay_step(h, 0.5);
    assert_eq!(phase(h, &hero), Dissipating);
    decay_step(h, 1.25);
    assert_eq!(phase(h, &hero), Hidden);
    assert!(!h.executed_unit(id(&hero)).unwrap().shown);
    assert_eq!(phase(h, &building), Bone);
    assert_eq!(
        h.executed_unit(id(&building))
            .unwrap()
            .decay
            .as_ref()
            .unwrap()
            .deadline(),
        Some(3.75)
    );
    decay_step(h, 3.75);
    assert!(h.executed_unit(id(&building)).is_err());
    assert!(h.executed_unit(id(&hero)).is_ok());
}
#[test]
fn decay_refusals_and_work_exhaustion_preserve_unit_state_and_removal_retires_it() {
    let mut vm = fixture_with_decay(Some(("0.5", "12", "0")));
    let h = vm.host();
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let unit = h.native("CreateCorpse", &args(owner, b"u001")).unwrap();
    let before = h.executed_unit(id(&unit)).unwrap().clone();
    for args in [
        vec![],
        vec![unit.clone(), Value::Int(1)],
        vec![Value::Int(1), Value::Bool(true)],
    ] {
        assert!(h.native("UnitSuspendDecay", &args).is_err());
        assert_eq!(h.executed_unit(id(&unit)).unwrap(), &before);
    }
    h.set_timer_game_time(1.5).unwrap();
    assert!(h.advance_unit_activity(1.5, &mut 0).is_err());
    assert_eq!(h.executed_unit(id(&unit)).unwrap(), &before);
    h.advance_unit_activity(1.5, &mut 10000).unwrap();
    h.native("RemoveUnit", std::slice::from_ref(&unit)).unwrap();
    decay_step(h, 10.);
    assert!(h.executed_unit(id(&unit)).is_err());
    for (death, flags) in [("NaN", "12"), ("-1", "12"), ("0.5", "1")] {
        let mut vm = fixture_with_decay(Some((death, flags, "0")));
        let h = vm.host();
        let owner = h.native("Player", &[Value::Int(4)]).unwrap();
        let count = h.inner().handles();
        assert!(h.native("CreateCorpse", &args(owner, b"u001")).is_err());
        assert_eq!(h.inner().handles(), count);
    }
}

#[test]
fn suspend_decay_admission_rejects_malformed_abi_and_foreign_client_before_state() {
    for declaration in [
        "native UnitSuspendDecay takes unit u, integer flag returns nothing",
        "constant native UnitSuspendDecay takes unit u, boolean flag returns nothing",
        "native UnitSuspendDecay takes unit u, boolean flag returns boolean",
    ] {
        let declarations = format!(
            "{}{}\n{declaration}\n",
            super::enumeration::TRIGGERS,
            super::activity::ORDERS
        );
        let mut h = position_host_admitting(
            1000,
            16,
            "// synthetic decay ABI",
            &declarations,
            &combat::armed("64", "normal"),
            |h| h,
        );
        let (registry, activity) = super::activity::rules();
        h = h.with_order_registry(registry).unwrap();
        h.admit_unit_activity(activity, 10000).unwrap();
        let mut rules = crate::unit_decay::Rules::fixture();
        rules.image_sha256 = h.player_client_sha256().unwrap();
        let before = h.inner().handles();
        assert!(h.admit_unit_decay(rules).is_err());
        assert_eq!(h.inner().handles(), before);
    }
    let mut vm = fixture();
    let h = vm.host();
    let mut foreign = crate::unit_decay::Rules::fixture();
    foreign.image_sha256 = [0x79; 32];
    assert!(h.admit_unit_decay(foreign).is_err());
    let mut rules = crate::unit_decay::Rules::fixture();
    rules.image_sha256 = h.player_client_sha256().unwrap();
    h.admit_unit_decay(rules.clone()).unwrap();
    assert!(h.admit_unit_decay(rules).is_err());
}
