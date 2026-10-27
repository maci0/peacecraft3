use super::*;

const SETTER: &str = "native SetUnitVertexColor takes unit u, integer r, integer g, integer b, integer a returns nothing\n";
fn open(hero: bool) -> (Vm, HandleId) {
    let map = if hero {
        MAP.replacen("'u001'", "'H001'", 1)
    } else {
        MAP.into()
    };
    let extra = format!(
        "{}\n{}\n{DECLARATIONS}\n{SETTER}\nnative RemoveUnit takes unit u returns nothing\ntype gamecache extends agent\nnative InitGameCache takes string file returns gamecache\nnative StoreUnit takes gamecache c, string m, string k, unit u returns boolean\n",
        enumeration::TRIGGERS,
        super::super::activity::ORDERS
    );
    let (registry, activity) = super::super::activity::rules();
    let h = position_host_admitting(1000, 16, &map, &extra, &hero_fields("STR"), |h| h)
        .with_order_registry(registry)
        .unwrap()
        .with_locations()
        .unwrap()
        .with_player_unit_events(8)
        .unwrap()
        .with_unit_states(crate::playercolor_host::UnitStateBudget { max_records: 8 })
        .unwrap()
        .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
        .unwrap()
        .with_hero_skills(
            holy_tables(),
            crate::scripts::AssetIdentity {
                source: source().install,
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            Default::default(),
            hero_rules().0,
            crate::playercolor_host::HeroBudget {
                abilities_per_unit: 8,
                overlay_objects: 0,
                modifications_per_ability: 0,
            },
        )
        .unwrap();
    let mut h = h;
    h.admit_unit_activity(activity, 65536).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.,
        mana_per_intelligence: 15.,
        life_regen_per_strength: 0.,
        mana_regen_per_intelligence: 0.,
    })
    .unwrap();
    h.admit_combat(rules(), "alpha").unwrap();
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.call("setup", &[]).unwrap();
    let Value::Handle(u) = vm.global("A") else {
        panic!("unit")
    };
    (vm, u)
}
fn args(unit: Value, rgba: [i32; 4]) -> Vec<Value> {
    std::iter::once(unit)
        .chain(rgba.into_iter().map(Value::Int))
        .collect()
}
#[test]
fn script_rgba_saturates_replaces_and_preserves_every_other_unit_property() {
    let (mut vm, u) = open(false);
    let h = &mut vm.host().inner;
    let before = h.executed_unit(u).unwrap().clone();
    assert_eq!(before.vertex_color, None);
    let handles = h.inner().handles();
    for (input, wanted) in [
        ([i32::MIN, -1, 256, i32::MAX], [0, 0, 255, 255]),
        ([9, 41, 107, 0], [9, 41, 107, 0]),
        ([255, 0, 1, 254], [255, 0, 1, 254]),
    ] {
        assert_eq!(
            h.native("SetUnitVertexColor", &args(Value::Handle(u), input))
                .unwrap(),
            Value::Null
        );
        let mut expected = before.clone();
        expected.vertex_color = Some(wanted);
        assert_eq!(h.executed_unit(u).unwrap(), &expected);
        assert_eq!(h.inner().handles(), handles);
    }
    let owner = before.owner;
    h.native(
        "SetUnitOwner",
        &[Value::Handle(u), Value::Handle(owner), Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(
        h.executed_unit(u).unwrap().vertex_color,
        Some([255, 0, 1, 254])
    );
}
#[test]
fn null_retired_unknown_wrong_family_and_bad_arguments_keep_typed_boundaries() {
    let (mut vm, u) = open(false);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.native("SetUnitVertexColor", &args(Value::Null, [1, 2, 3, 4]))
            .unwrap(),
        Value::Null
    );
    let before = h.executed_unit(u).unwrap().clone();
    let owner = before.owner;
    assert!(
        h.native(
            "SetUnitVertexColor",
            &args(Value::Handle(owner), [1, 2, 3, 4])
        )
        .is_err()
    );
    let unknown = HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    assert!(
        h.native(
            "SetUnitVertexColor",
            &args(Value::Handle(unknown), [1, 2, 3, 4])
        )
        .unwrap_err()
        .contains("SetUnitVertexColor")
    );
    let mut invalid = args(Value::Handle(u), [1, 2, 3, 4]);
    invalid[3] = Value::Real(7.);
    assert!(h.native("SetUnitVertexColor", &invalid).is_err());
    assert_eq!(h.executed_unit(u).unwrap(), &before);
    h.native("RemoveUnit", &[Value::Handle(u)]).unwrap();
    let handles = h.inner().handles();
    assert_eq!(
        h.native("SetUnitVertexColor", &args(Value::Handle(u), [1, 2, 3, 4]))
            .unwrap(),
        Value::Null
    );
    assert!(h.executed_unit(u).is_err());
    assert_eq!(h.inner().handles(), handles);
}
#[test]
fn cache_and_mirror_refuse_nonrepresentable_color_before_mutation() {
    let (mut vm, u) = open(true);
    let h = &mut vm.host().inner;
    h.native(
        "SetUnitVertexColor",
        &args(Value::Handle(u), [101, 12, 237, 73]),
    )
    .unwrap();
    let memory = Arc::new(crate::game_cache::tests::Memory::default());
    h.admit_game_caches(memory.clone(), crate::game_cache::tests::budget())
        .unwrap();
    let cache = h
        .native("InitGameCache", &[Value::Str("fixture".into())])
        .unwrap();
    let before = h.executed_unit(u).unwrap().clone();
    let handles = h.inner().handles();
    let random = h.script_random_state().cloned();
    assert!(
        h.native(
            "StoreUnit",
            &[
                cache,
                Value::Str("mission".into()),
                Value::Str("hero".into()),
                Value::Handle(u)
            ]
        )
        .unwrap_err()
        .contains("does not encode explicit vertex color")
    );
    assert!(
        h.test_mirror_image_factory(u, *b"Yher", [80., 80.], [0.25, 2.], true)
            .unwrap_err()
            .to_string()
            .contains("explicit vertex color")
    );
    assert_eq!(h.executed_unit(u).unwrap(), &before);
    assert_eq!(h.inner().handles(), handles);
    assert_eq!(h.script_random_state(), random.as_ref());
    assert!(memory.bytes.lock().unwrap().is_none());
}
#[test]
fn malformed_declaration_is_refused_and_omitted_capability_remains_optional() {
    let fields = armed("64", "normal");
    let h = position_host_admitting(
        1000,
        16,
        MAP,
        &format!(
            "{}\n{}\n{DECLARATIONS}",
            enumeration::TRIGGERS,
            super::super::activity::ORDERS
        ),
        &fields,
        |h| h,
    );
    let mut h = h;
    assert!(
        h.native("SetUnitVertexColor", &args(Value::Null, [1, 2, 3, 4]))
            .is_err()
    );
    let common = format!(
        "{}\n{}",
        script_common("widget", true),
        SETTER.replace("integer b", "real b")
    );
    let ctx = context(
        300,
        &common,
        "function main takes nothing returns nothing\nendfunction\n",
        "unit-catalog",
    );
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let error = match object(&ctx, 4).with_unit_catalog(shared, source(), limits(1)) {
        Err(error) => error,
        Ok(_) => panic!("bad RGBA signature admitted"),
    };
    assert_eq!(error.failure, Failure::Signature);
    assert_eq!(error.native, "SetUnitVertexColor");
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
}
#[test]
fn foreign_vertex_declaration_refuses_before_catalog_resolution() {
    let primary = "type agent extends handle\ntype widget extends agent\ntype unit extends widget\ntype player extends agent\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nglobals\nconstant playercolor C=ConvertPlayerColor(1)\nendglobals\n";
    let map = pcx_script::parse_script("").unwrap();
    let primary = pcx_script::parse_script(primary).unwrap();
    let other = pcx_script::parse_script(SETTER).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                pcx_script::Prelude::new("synthetic/common.j", &primary),
                pcx_script::Prelude::new("synthetic/other.j", &other),
            ],
        )
        .unwrap(),
    );
    let inner = crate::playercolor_host::EnumHost::checked_selected(
        program,
        "split-source",
        &["ConvertPlayerColor"],
        crate::playercolor_host::Limits {
            max_handles: 4,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    let (catalog, hits, _) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let error =
        match ObjectHost::checked(inner)
            .unwrap()
            .with_unit_catalog(shared, source(), limits(1))
        {
            Err(error) => error,
            Ok(_) => panic!("foreign RGBA source admitted"),
        };
    assert_eq!(error.failure, Failure::Source);
    assert_eq!(error.native, "SetUnitVertexColor");
    assert_eq!(error.declaration_source, "synthetic/other.j");
    assert_ne!(error.declaration_span, pcx_script::span::Span::EMPTY);
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
}
