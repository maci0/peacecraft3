use super::*;
const DECL: &str = r#"
type raritycontrol extends handle
constant native ConvertRarityControl takes integer n returns raritycontrol
native SetUnitAnimationWithRarity takes unit u, string animation, raritycontrol rarity returns nothing
native SetUnitAnimation takes unit u, string animation returns nothing
native QueueUnitAnimation takes unit u, string animation returns nothing
native RemoveUnit takes unit u returns nothing
native TimerStart takes timer t, real seconds, boolean repeating, code callback returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
native GetRandomInt takes integer low, integer high returns integer
native GetRandomReal takes real low, real high returns real
native SetRandomSeed takes integer seed returns nothing
globals
constant raritycontrol FREQUENT=ConvertRarityControl(2)
constant raritycontrol RARE=ConvertRarityControl(7)
endglobals
"#;
struct Catalog {
    inner: Arc<Counting>,
    value: Result<FieldValue, pcx_sim::world::FieldProblem>,
}
impl ObjectCatalog for Catalog {
    fn resolve(&self, kind: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let mut d = self.inner.resolve(kind)?;
        d.fields.insert(
            *b"tags",
            ObjectField {
                metadata: Some(FieldDef {
                    fourcc: *b"tags",
                    field: "motionTags".into(),
                    from: SlkId::UnitMetaData,
                    source: FieldSource::Profile {
                        key: "motionTags".into(),
                        index: -1,
                        slots: 1,
                    },
                    index: -1,
                    kind: FieldKind::Token,
                    declared: "stringList".into(),
                    slk: "Profile".into(),
                    category: "synthetic".into(),
                    measured: true,
                }),
                value: self.value.clone(),
            },
        );
        Ok(d)
    }
}
fn open(value: Result<FieldValue, pcx_sim::world::FieldProblem>) -> VmHost {
    let common = script_common_with("widget", true, DECL);
    let map = format!(
        "{MAP}\nfunction choose takes raritycontrol r returns nothing\ncall SetUnitAnimationWithRarity(U,\"still\",r)\nendfunction\nfunction override takes nothing returns nothing\ncall SetUnitAnimationWithRarity(U,\"still\",FREQUENT)\ncall SetUnitAnimation(U,\"still wave\")\nendfunction\nfunction schedule_override takes nothing returns nothing\ncall TimerStart(CreateTimer(),0.1,false,function override)\nendfunction\n"
    );
    let ctx = context(300, &common, &map, "unit-catalog");
    let (catalog, hits, seen) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(Catalog {
        inner: catalog,
        value,
    });
    let enums = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertRarityControl",
        ],
        Limits {
            max_handles: 96,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let mut host = ObjectHost::checked(enums)
        .unwrap()
        .with_map_config(crate::playercolor_host::MapConfigBudget {
            text_bytes: 128,
            start_locations: 2,
        })
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(2))
        .unwrap()
        .with_timer_runtime(0.)
        .unwrap();
    let policy = Arc::new(crate::script_random::profile::fixture());
    host = host.with_script_random(Arc::clone(&policy), 64).unwrap();
    let mut rules = crate::unit_animation_rarity::tests::rules();
    rules.image_sha256 = *policy.image_sha256();
    host.admit_unit_animation_rarity(Ok(rules)).unwrap();
    let model = Arc::new(crate::unit_animation_rarity::tests::model(&[
        ("still 2", 0.2),
        ("still", 0.6),
        ("still cinema", 0.6),
        ("still wave", 0.9),
    ]));
    host.admit_unit_models(move |_| Ok(Some(Arc::clone(&model))))
        .unwrap();
    let mut h = VmHost {
        vm: Arc::clone(&ctx.scripts).owned_vm(host),
        hits,
        seen,
        catalog: shared,
    };
    h.vm.init_globals().unwrap();
    h.vm.call("main", &[]).unwrap();
    h.vm.host()
        .inner
        .native("SetRandomSeed", &[Value::Int(91)])
        .unwrap();
    h
}
fn id(h: &VmHost) -> pcx_script::HandleId {
    let Value::Handle(u) = h.vm.global("U") else {
        panic!("unit")
    };
    u
}
fn index(h: &mut VmHost) -> usize {
    let u = id(h);
    h.vm.host()
        .inner
        .executed_unit(u)
        .unwrap()
        .animation
        .as_ref()
        .unwrap()
        .sequence
        .unwrap()
}
#[test]
fn public_rarity_dispatch_uses_separate_stream_and_explicit_set_overrides() {
    let mut a = open(Ok(FieldValue::Cell("_".into())));
    let mut b = open(Ok(FieldValue::Cell("_".into())));
    let before = a.vm.host().inner.script_random_state().cloned();
    let u = id(&a);
    let rarity = a.vm.global("FREQUENT");
    assert!(
        a.vm.host()
            .inner
            .native(
                "SetUnitAnimationWithRarity",
                &[Value::Handle(u), Value::Str("é".into()), rarity]
            )
            .is_err()
    );
    for _ in 0..8 {
        let r = a.vm.global("FREQUENT");
        a.vm.call("choose", &[r]).unwrap();
        let r = b.vm.global("FREQUENT");
        b.vm.call("choose", &[r]).unwrap();
        assert_eq!(index(&mut a), index(&mut b));
        assert!([1, 2].contains(&index(&mut a)));
    }
    assert_eq!(a.vm.host().inner.script_random_state(), before.as_ref());
    let r = a.vm.global("RARE");
    a.vm.call("choose", &[r]).unwrap();
    assert_eq!(index(&mut a), 0);
    a.vm.call("override", &[]).unwrap();
    assert_eq!(index(&mut a), 3);
}
#[test]
fn authored_properties_and_override_select_real_track_and_missing_is_empty() {
    for value in [
        Ok(FieldValue::Cell("wave".into())),
        Ok(FieldValue::Override(pcx_objects::Value::Str(
            b"wave".to_vec(),
        ))),
    ] {
        let mut h = open(value);
        let r = h.vm.global("FREQUENT");
        h.vm.call("choose", &[r]).unwrap();
        assert_eq!(index(&mut h), 3);
    }
    let mut h = open(Err(pcx_sim::world::FieldProblem::Missing));
    let r = h.vm.global("RARE");
    h.vm.call("choose", &[r]).unwrap();
    assert_eq!(index(&mut h), 0);
}
#[test]
fn malformed_properties_wrong_family_and_retired_targets_preserve_state() {
    let mut h = open(Ok(FieldValue::Override(pcx_objects::Value::Int(9))));
    let u = id(&h);
    let before =
        h.vm.host()
            .inner
            .executed_unit(u)
            .unwrap()
            .animation
            .clone();
    let random = h.vm.host().inner.script_random_state().cloned();
    let r = h.vm.global("FREQUENT");
    assert!(h.vm.call("choose", std::slice::from_ref(&r)).is_err());
    assert_eq!(
        h.vm.host().inner.executed_unit(u).unwrap().animation,
        before
    );
    assert_eq!(h.vm.host().inner.script_random_state(), random.as_ref());
    let p = Value::Handle(u);
    assert!(
        h.vm.host()
            .inner
            .native(
                "SetUnitAnimationWithRarity",
                &[Value::Handle(u), Value::Str("still".into()), p]
            )
            .is_err()
    );
    h.vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(u)])
        .unwrap();
    assert_eq!(
        h.vm.host()
            .inner
            .native(
                "SetUnitAnimationWithRarity",
                &[Value::Handle(u), Value::Str("still".into()), r.clone()]
            )
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        h.vm.host()
            .inner
            .native(
                "SetUnitAnimationWithRarity",
                &[Value::Null, Value::Str("still".into()), r]
            )
            .unwrap(),
        Value::Null
    );
}

#[test]
fn timer_callback_explicit_animation_overrides_prior_rarity_request() {
    let mut h = open(Ok(FieldValue::Cell("_".into())));
    h.vm.call("schedule_override", &[]).unwrap();
    let expiry =
        h.vm.host()
            .inner
            .next_timer_expiry(0.2, &mut 100)
            .unwrap()
            .unwrap();
    h.vm.run_thread(&[expiry.callback.unwrap()]).unwrap();
    h.vm.host().inner.finish_timer_expiry(expiry).unwrap();
    assert_eq!(index(&mut h), 3);
}
