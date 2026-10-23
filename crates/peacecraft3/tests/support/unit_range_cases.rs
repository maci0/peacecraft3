//! Range callbacks consume accepted unit records in the production session.
use super::*;
use pcx_objects::editor::{FieldDef, FieldKind, FieldSource, ObjectKind};
use pcx_script::Host;
use pcx_sim::world::{
    Definition, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem, Source,
};
use std::collections::BTreeMap;

const EXTRA: &str = "native ShowUnit takes unit u, boolean shown returns nothing\nconstant native IsUnitHidden takes unit u returns boolean\nconstant native GetTriggerUnit takes nothing returns unit\nconstant native GetEnteringUnit takes nothing returns unit\nconstant native GetFilterUnit takes nothing returns unit\nconstant native GetUnitX takes unit u returns real\nconstant native GetUnitY takes unit u returns real\nnative GroupEnumUnitsInRect takes group g, rect r, boolexpr f returns nothing\nnative TriggerRegisterUnitInRange takes trigger t, unit u, real r, boolexpr f returns event\nglobals\nconstant unitevent TEST_RANGE=ConvertUnitEvent(-47)\nendglobals\n";
const SCENE: &str = r#"
globals
unit A=null
unit B=null
unit EXPECTED=null
trigger T=null
integer SEQ=0
boolean OK=true
endglobals
function predicate takes nothing returns boolean
set SEQ=SEQ*10+1
set OK=OK and GetFilterUnit()==EXPECTED and GetTriggerUnit()==EXPECTED and GetTriggeringTrigger()==null and GetTriggerEventId()==null and GetEnteringUnit()==null
return true
endfunction
function condition takes nothing returns boolean
set SEQ=SEQ*10+2
set OK=OK and GetFilterUnit()==EXPECTED and GetTriggerUnit()==EXPECTED and GetTriggeringTrigger()==T and GetTriggerEventId()==TEST_RANGE and GetEnteringUnit()==EXPECTED
return true
endfunction
function action takes nothing returns nothing
set SEQ=SEQ*10+3
set OK=OK and GetFilterUnit()==EXPECTED and GetTriggerUnit()==EXPECTED and GetTriggeringTrigger()==T and GetTriggerEventId()==TEST_RANGE and GetEnteringUnit()==EXPECTED
endfunction
function birth takes nothing returns nothing
set EXPECTED=CreateUnit(Player(0),'UNIT',16.0,48.0,0.0)
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set A=CreateUnit(Player(0),'UNIT',16.0,16.0,0.0)
set B=CreateUnit(Player(0),'UNIT',48.0,16.0,0.0)
set EXPECTED=B
set T=CreateTrigger()
call TriggerRegisterUnitInRange(T,A,32.0,Filter(function predicate))
call TriggerAddCondition(T,Condition(function condition))
call TriggerAddAction(T,function action)
endfunction
"#;
pub(super) struct Geometry;
impl ObjectCatalog for Geometry {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        if rawcode != *b"UNIT" {
            return Err(ResolutionProblem::MissingRawcode(
                String::from_utf8_lossy(&rawcode).into_owned(),
            ));
        }
        let mut fields = BTreeMap::new();
        for (id, slk, column, kind, value) in [
            (
                *b"G001",
                SlkId::UnitBalance,
                "collision",
                FieldKind::Unreal,
                "2",
            ),
            (
                *b"M001",
                SlkId::UnitData,
                "movetp",
                FieldKind::Token,
                "foot",
            ),
            (*b"P001", SlkId::UnitData, "pathTex", FieldKind::Token, "_"),
            (*b"S001", SlkId::UnitBalance, "spd", FieldKind::Unreal, "8"),
        ] {
            fields.insert(
                id,
                ObjectField {
                    metadata: Some(FieldDef {
                        fourcc: id,
                        from: SlkId::UnitMetaData,
                        field: column.into(),
                        slk: slk.name().into(),
                        index: -1,
                        declared: if kind == FieldKind::Unreal {
                            "unreal"
                        } else if column == "pathTex" {
                            "pathingTexture"
                        } else {
                            "moveType"
                        }
                        .into(),
                        kind,
                        category: "synthetic geometry".into(),
                        source: FieldSource::Column {
                            slk,
                            column: column.into(),
                        },
                        measured: true,
                    }),
                    value: Ok(FieldValue::Cell(value.into())),
                },
            );
        }
        Ok(Definition {
            rawcode,
            base_rawcode: "UNIT".into(),
            kind: ObjectKind::Unit,
            fields,
            modifications: Vec::new(),
        })
    }
}
pub(super) fn placement(source: Source) -> pcx_play::unit_placement::Placement {
    use pcx_play::unit_placement::*;
    let mut image = vec![0; 0x800];
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x2000),
        (0x100, 0x600),
        (0x104, 0x1000),
        (0x108, 0x600),
        (0x10c, 0x200),
        (0x11c, 0x60000020),
    ] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    image[..2].copy_from_slice(b"MZ");
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [(0x84, 0x14cu16), (0x86, 1), (0x94, 0x60), (0x98, 0x10b)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    let address = |at: usize| 0x301000 + (at - 0x200) as u32;
    let mut put = |at: usize, bytes: &[u8]| {
        image[at..at + bytes.len()].copy_from_slice(bytes);
        SourceWindow {
            va: address(at),
            len: bytes.len() as u32,
            digest: digest(bytes),
        }
    };
    let normalization = put(
        0x220,
        &[&[0x05][..], &(-(2i32 << 23)).to_le_bytes()].concat(),
    );
    let initial_position = put(
        0x230,
        &[&[0xa1][..], &address(0x700).to_le_bytes()].concat(),
    );
    let small_radius = put(
        0x240,
        &[&[0xf3, 0x0f, 0x10, 0x05][..], &address(0x704).to_le_bytes()].concat(),
    );
    let large_radius = put(
        0x250,
        &[&[0xf3, 0x0f, 0x10, 0x05][..], &address(0x708).to_le_bytes()].concat(),
    );
    let init = |n| {
        [
            &[0x6a, n, 0x68][..],
            &address(0x710).to_le_bytes(),
            &[0xe8, 0, 0, 0, 0, 0x83, 0xc4, 8, 0xc3],
        ]
        .concat()
    };
    let one_initializer = put(0x280, &init(1));
    let two_initializer = put(0x2a0, &init(2));
    let ground_mask = put(0x2c0, &[0xb9, 8, 0, 0, 0]);
    let fly_mask = put(0x2e0, &[0xb9, 4, 0, 0, 0]);
    for (at, value) in [(0x700, -8192.25f32), (0x704, 0.5), (0x708, 1.5)] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let rules = Rules::read(
        &image,
        Binding {
            origin: origin("synthetic placement PE"),
            image_sha256: digest(&image),
            normalization,
            initial_position,
            small_radius,
            large_radius,
            one_initializer,
            two_initializer,
            ground_mask,
            fly_mask,
        },
        RuleInputBudget {
            image_bytes: 4096,
            profile_bytes: 1024,
            sections: 2,
        },
    )
    .unwrap();
    Placement::new(
        Grid {
            source,
            origin: [0.; 2],
            pathing: pcx_map::Pathing {
                version: 0,
                width: 32,
                height: 32,
                cells: vec![0; 1024],
            },
            domains: vec![5; 1024],
        },
        rules,
        BTreeMap::new(),
        Budget {
            cells: 1024,
            footprint_cells: 64,
            stationary_cells: 0,
            stationary_work: 1 << 20,
            queries: 100,
        },
    )
    .unwrap()
}
fn session(script: &str) -> MapScriptSession<MapStartupHost> {
    session_with_activity(script, false)
}
fn session_with_activity(script: &str, activity: bool) -> MapScriptSession<MapStartupHost> {
    let mut map = fixture(script);
    map.common
        .extend_from_slice(super::script_timer_cases::TIMER_COMMON.as_bytes());
    map.common
        .extend_from_slice(super::timed_trigger_cases::COMMON.as_bytes());
    map.common.extend_from_slice(EXTRA.as_bytes());
    if activity {
        map.common.extend_from_slice(b"\nconstant native OrderId takes string s returns integer\nconstant native OrderId2String takes integer i returns string\nnative IssuePointOrder takes unit u, string s, real x, real y returns boolean\nnative GetUnitCurrentOrder takes unit u returns integer\nconstant native GetUnitMoveSpeed takes unit u returns real\n");
    }
    map.rebuild();
    let reader = map.resolver();
    let bindings = bind_map(&reader, map.input(1), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::new(Geometry));
    inputs.timer_time = Some(10.);
    inputs.enumeration.names.push("ConvertGameEvent".into());
    inputs.enumeration.limits.max_handles = 256;
    inputs.budgets.conditions =
        Some(pcx_play::playercolor_host::ConditionBudget { max_records: 8 });
    inputs.budgets.trigger_conditions = Some(8);
    inputs.budgets.unit_in_range = Some(8);
    inputs.budgets.units.max_retained_fields = 32;
    inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
        initial_enabled: true,
        timer_event_global: "TEST_TIMER_EVENT".into(),
        max_timed_events: 8,
        max_parked_threads: 8,
    });
    let payload = Arc::clone(&bindings.catalog_map_bytes);
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    app.insert_resource(MapRequests {
        bindings: Some(bindings),
        ..Default::default()
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic range scene".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id,
        binding_revision: 1,
        payload: MapPayload::Shared(payload),
    });
    app.update();
    let ctx = app
        .world()
        .resource::<MapRequests>()
        .admission
        .shared_context()
        .unwrap();
    let entries = [
        ctx.scripts.program().function_id("config").unwrap(),
        ctx.scripts.program().function_id("main").unwrap(),
    ];
    let mut session = MapScriptSession::default();
    session
        .replace(
            Arc::clone(&ctx),
            move |ctx| {
                let source = ctx.world.source.clone();
                let mut h = pcx_play::map_startup_host::assemble(ctx, inputs)?;
                if activity {
                    let (registry, rules) = activity_rules();
                    h.objects_mut().admit_order_registry(registry)?;
                    h.objects_mut().admit_unit_activity(rules, 65536)?;
                }
                h.objects_mut().admit_unit_placement(placement(source))?;
                h.objects_mut().admit_unit_range_runtime("TEST_RANGE", 8)?;
                Ok::<_, pcx_play::map_startup_host::MapStartupError>(h)
            },
            &entries,
        )
        .unwrap();
    session
}
#[test]
fn range_filter_conditions_and_actions_share_the_published_vm_and_clear_context() {
    let mut s = session(SCENE);
    assert_eq!(s.global("SEQ"), Some(Value::Int(0)));
    assert_eq!(s.advance_unit_ranges(1000).unwrap(), 1);
    assert_eq!(s.global("SEQ"), Some(Value::Int(123)));
    assert_eq!(s.global("OK"), Some(Value::Bool(true)));
    for name in [
        "GetFilterUnit",
        "GetEnteringUnit",
        "GetTriggerUnit",
        "GetTriggeringTrigger",
        "GetTriggerEventId",
    ] {
        assert_eq!(s.host().unwrap().native(name, &[]).unwrap(), Value::Null);
    }
    assert_eq!(s.advance_unit_ranges(1000).unwrap(), 0);
    let birth = s
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("birth")
        .unwrap();
    s.call_function(birth, &[]).unwrap();
    assert_eq!(s.advance_unit_ranges(1000).unwrap(), 1);
    assert_eq!(s.global("SEQ"), Some(Value::Int(123123)));
    assert_eq!(s.global("OK"), Some(Value::Bool(true)));
}
#[test]
fn false_filters_conditions_and_callback_errors_have_no_extra_action_or_stale_context() {
    for (source, expected) in [
        (SCENE.replacen("return true", "return false", 1), 1),
        (
            SCENE.replace("set SEQ=SEQ*10+2", "set SEQ=SEQ*10+2\nreturn false"),
            12,
        ),
        (
            SCENE
                .replace("set SEQ=SEQ*10+2", "set SEQ=SEQ*10+2\nreturn false")
                .replace("call TriggerAddAction(T,function action)", "call TriggerAddCondition(T,Condition(function tail))\ncall TriggerAddAction(T,function action)")
                + "\nfunction tail takes nothing returns boolean\nset SEQ=SEQ*10+4\nreturn true\nendfunction\n",
            124,
        ),
    ] {
        let mut s = session(&source);
        assert_eq!(s.advance_unit_ranges(1000).unwrap(), 0);
        assert_eq!(s.global("SEQ"), Some(Value::Int(expected)));
        assert_eq!(
            s.host().unwrap().native("GetFilterUnit", &[]).unwrap(),
            Value::Null
        );
    }
    let source = SCENE.replace(
        "set SEQ=SEQ*10+3",
        "set SEQ=SEQ*10+3\ncall TimerStart(null,1.0,false,null)",
    );
    let mut s = session(&source);
    assert!(s.advance_unit_ranges(1000).is_err());
    assert_eq!(s.global("SEQ"), Some(Value::Int(123)));
    assert_eq!(s.advance_unit_ranges(1000).unwrap(), 0);
    for name in [
        "GetFilterUnit",
        "GetEnteringUnit",
        "GetTriggerUnit",
        "GetTriggeringTrigger",
        "GetTriggerEventId",
    ] {
        assert_eq!(s.host().unwrap().native(name, &[]).unwrap(), Value::Null);
    }
}
#[test]
fn runtime_delivers_ranges_at_logical_ticks_independent_of_frame_chunks() {
    use peacecraft3::campaign_runtime::CampaignRuntime;
    use std::time::Duration;
    let run = |chunks: &[u64]| {
        let mut s = session(SCENE);
        let mut r = CampaignRuntime::new(Duration::from_millis(100), 2, 8);
        r.advance(&mut s, Duration::from_secs(50));
        assert_eq!(
            s.global("SEQ"),
            Some(Value::Int(0)),
            "binding/loading is not a simulation tick"
        );
        for &chunk in chunks {
            r.advance(&mut s, Duration::from_millis(chunk));
            assert!(r.last_error.is_none(), "{:?}", r.last_error);
        }
        while !r.pending().is_zero() {
            r.advance(&mut s, Duration::ZERO);
            assert!(r.last_error.is_none());
        }
        (
            r.tick(),
            s.global("SEQ"),
            s.global("OK"),
            s.host()
                .unwrap()
                .objects()
                .timer_game_time()
                .unwrap()
                .to_bits(),
        )
    };
    let expected = run(&[100, 100, 100, 100]);
    assert_eq!(expected.0, 4);
    assert_eq!(expected.1, Some(Value::Int(123)));
    assert_eq!(expected.2, Some(Value::Bool(true)));
    assert_eq!(run(&[400]), expected);
    assert_eq!(run(&[17, 0, 83, 210, 90]), expected);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_unit_range_gate() -> u32 {
    range_filter_conditions_and_actions_share_the_published_vm_and_clear_context();
    false_filters_conditions_and_callback_errors_have_no_extra_action_or_stale_context();
    runtime_delivers_ranges_at_logical_ticks_independent_of_frame_chunks();
    3
}

fn registry_fixture(
    ids: [i32; 2],
) -> (
    Vec<u8>,
    pcx_play::order_registry::Binding,
    pcx_play::neutral_player::RuleInputBudget,
) {
    let mut image = vec![0; 0x800];
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x2000),
        (0x100, 0x600),
        (0x104, 0x1000),
        (0x108, 0x600),
        (0x10c, 0x200),
        (0x11c, 0x60000020),
    ] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    image[..2].copy_from_slice(b"MZ");
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [(0x84, 0x14cu16), (0x86, 1), (0x94, 0x60), (0x98, 0x10b)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    let va = 0x301020;
    let register = 0x301100;
    let mut code = vec![0x56, 0x8b, 0x35, 0, 0, 0x40, 0, 0x8b, 0xce];
    for (at, (name, id)) in [
        (0x301200u32, ids[0]),
        (0x301300, ids[1]),
        (0x301200, ids[0]),
    ]
    .into_iter()
    .enumerate()
    {
        code.push(0x68);
        code.extend_from_slice(&name.to_le_bytes());
        code.push(0x68);
        code.extend_from_slice(&id.to_le_bytes());
        if at != 0 {
            code.extend_from_slice(&[0x8b, 0xce]);
        }
        code.push(0xe8);
        let next = va + code.len() as u32 + 4;
        code.extend_from_slice(&(register as i32 - next as i32).to_le_bytes());
    }
    code.extend_from_slice(&[0x5e, 0xc3]);
    image[0x220..0x220 + code.len()].copy_from_slice(&code);
    image[0x300] = 0xc3;
    image[0x400..0x406].copy_from_slice(b"alpha\0");
    image[0x500..0x505].copy_from_slice(b"beta\0");
    let mut bootstrap = vec![0x68];
    bootstrap.extend_from_slice(&0x301400u32.to_le_bytes());
    bootstrap.extend_from_slice(&[0xff, 0x35]);
    bootstrap.extend_from_slice(&0x301350u32.to_le_bytes());
    for target in [0x301100i32, va as i32] {
        bootstrap.push(0xe8);
        let next = 0x301080 + bootstrap.len() as i32 + 4;
        bootstrap.extend_from_slice(&(target - next).to_le_bytes());
    }
    image[0x280..0x295].copy_from_slice(&bootstrap);
    image[0x550..0x554].copy_from_slice(&(ids[0] + 1200).to_le_bytes());
    image[0x600..0x606].copy_from_slice(b"gamma\0");
    let binding = pcx_play::order_registry::Binding {
        origin: pcx_play::scripts::AssetIdentity {
            edition: "synthetic".into(),
            version: "test".into(),
            source: "synthetic:orders".into(),
        },
        image_sha256: digest(&image),
        initializer: pcx_play::camera_margins::SourceWindow {
            va,
            len: code.len() as u32,
            digest: digest(&code),
        },
        bootstrap: pcx_play::camera_margins::SourceWindow {
            va: 0x301080,
            len: 21,
            digest: digest(&bootstrap),
        },
        direct_register: register,
        register,
    };
    (
        image,
        binding,
        pcx_play::neutral_player::RuleInputBudget {
            image_bytes: 4096,
            profile_bytes: 1024,
            sections: 2,
        },
    )
}

pub(super) fn activity_rules() -> (
    Arc<pcx_play::order_registry::Registry>,
    pcx_play::unit_activity::Rules,
) {
    use pcx_play::camera_margins::SourceWindow;
    let (mut image, mut binding, budget) = registry_fixture([431, 744]);
    let third = 0x220 + 9 + 15 + 17;
    image[third + 1..third + 5].copy_from_slice(&0x301480u32.to_le_bytes());
    image[third + 6..third + 10].copy_from_slice(&2122i32.to_le_bytes());
    image[0x680..0x686].copy_from_slice(b"delta\0");
    binding.initializer.digest = digest(&image[0x220..0x220 + binding.initializer.len as usize]);
    let mut put = |at: usize, prefix: &[u8], id: i32| {
        let mut bytes = prefix.to_vec();
        bytes.extend_from_slice(&id.to_le_bytes());
        image[at..at + bytes.len()].copy_from_slice(&bytes);
        SourceWindow {
            va: 0x301000 + at as u32 - 0x200,
            len: bytes.len() as u32,
            digest: digest(&bytes),
        }
    };
    let hold = put(0x320, &[0x3d], 431);
    let stop = put(0x330, &[0x81, 0xfb], 744);
    let movement = put(0x340, &[0x3d], 1631);
    let patrol = put(0x350, &[0x3d], 2122);
    binding.image_sha256 = digest(&image);
    let registry = Arc::new(
        pcx_play::order_registry::Registry::read(&image, binding.clone(), budget, 4, 32).unwrap(),
    );
    let rules = pcx_play::unit_activity::Rules::read(
        &image,
        pcx_play::unit_activity::Binding {
            origin: binding.origin,
            image_sha256: binding.image_sha256,
            hold,
            stop,
            movement,
            patrol,
        },
        &registry,
        budget,
    )
    .unwrap();
    (registry, rules)
}

#[test]
fn timer_callback_observes_motion_at_its_deadline_independent_of_advance_chunks() {
    const SCRIPT: &str = r#"
globals
unit A=null
real X=0.0
integer ORDER=0
boolean HELD=false
endglobals
function hold takes nothing returns nothing
set X=GetUnitX(A)
set ORDER=GetUnitCurrentOrder(A)
set HELD=IssueImmediateOrder(A,"alpha")
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set A=CreateUnit(Player(0),'UNIT',16.0,16.0,0.0)
call IssuePointOrder(A,"gamma",72.0,16.0)
call TimerStart(CreateTimer(),0.25,false,function hold)
endfunction
"#;
    let run = |times: &[f64]| {
        let mut s = session_with_activity(SCRIPT, true);
        let mut callbacks = 0;
        for &now in times {
            callbacks += s.advance_timers(now, 1000).unwrap();
        }
        assert_eq!(callbacks, 1);
        assert_eq!(s.global("X"), Some(Value::Real(18.)));
        assert_eq!(s.global("ORDER"), Some(Value::Int(1631)));
        assert_eq!(s.global("HELD"), Some(Value::Bool(true)));
        let Some(Value::Handle(id)) = s.global("A") else {
            panic!("unit")
        };
        let record = s.host().unwrap().objects().executed_unit(id).unwrap();
        assert_eq!(record.position, Some([18., 16.]));
        let activity = record.activity.as_ref().unwrap();
        assert!(activity.held() && !activity.moving());
        (record.position, record.face.to_bits(), activity.clone())
    };
    let expected = run(&[11.]);
    assert_eq!(expected, run(&[10., 10.125, 10.25, 10.5, 11.]));
    for chunks in [
        &[1000][..],
        &[100, 100, 100, 100, 100, 100, 100, 100, 100, 100],
        &[17, 0, 83, 210, 90, 600],
    ] {
        use peacecraft3::campaign_runtime::CampaignRuntime;
        use std::time::Duration;
        let mut s = session_with_activity(SCRIPT, true);
        let mut runtime = CampaignRuntime::new(Duration::from_millis(100), 2, 8);
        runtime.advance(&mut s, Duration::from_secs(50));
        assert_eq!(
            s.global("X"),
            Some(Value::Real(0.)),
            "loading does not move units"
        );
        for &chunk in chunks {
            runtime.advance(&mut s, Duration::from_millis(chunk));
            assert!(runtime.last_error.is_none());
        }
        while !runtime.pending().is_zero() {
            runtime.advance(&mut s, Duration::ZERO);
            assert!(runtime.last_error.is_none());
        }
        assert_eq!(runtime.tick(), 10);
        assert_eq!(s.global("X"), Some(Value::Real(18.)));
        assert_eq!(s.global("ORDER"), Some(Value::Int(1631)));
        assert_eq!(s.global("HELD"), Some(Value::Bool(true)));
        let Some(Value::Handle(id)) = s.global("A") else {
            panic!("unit")
        };
        let record = s.host().unwrap().objects().executed_unit(id).unwrap();
        assert_eq!(
            (
                record.position,
                record.face.to_bits(),
                record.activity.as_ref().unwrap().clone()
            ),
            expected
        );
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_unit_activity_gate() -> u32 {
    timer_callback_observes_motion_at_its_deadline_independent_of_advance_chunks();
    1
}

#[test]
fn visibility_excludes_candidates_but_preserves_hidden_anchor_range_delivery() {
    let source = SCENE.replace("set EXPECTED=B", "set EXPECTED=B\ncall ShowUnit(B,false)")
        + r#"
function target_visibility takes boolean shown returns nothing
call ShowUnit(B,shown)
endfunction
function anchor_visibility takes boolean shown returns nothing
call ShowUnit(A,shown)
endfunction
"#;
    let mut s = session(&source);
    assert_eq!(s.advance_unit_ranges(1000).unwrap(), 0);
    assert_eq!(s.global("SEQ"), Some(Value::Int(0)));
    let target = s
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("target_visibility")
        .unwrap();
    let anchor = s
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("anchor_visibility")
        .unwrap();
    s.call_function(target, &[Value::Bool(true)]).unwrap();
    assert_eq!(s.advance_unit_ranges(1000).unwrap(), 1);
    assert_eq!(s.global("SEQ"), Some(Value::Int(123)));
    s.call_function(anchor, &[Value::Bool(false)]).unwrap();
    assert_eq!(s.advance_unit_ranges(1000).unwrap(), 0);
    s.call_function(target, &[Value::Bool(false)]).unwrap();
    assert_eq!(s.advance_unit_ranges(1000).unwrap(), 0);
    s.call_function(target, &[Value::Bool(true)]).unwrap();
    assert_eq!(s.advance_unit_ranges(1000).unwrap(), 1);
    assert_eq!(s.global("SEQ"), Some(Value::Int(123123)));
    assert_eq!(s.global("OK"), Some(Value::Bool(true)));
    s.call_function(anchor, &[Value::Bool(true)]).unwrap();
    assert_eq!(s.advance_unit_ranges(1000).unwrap(), 0);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_unit_visibility_gate() -> u32 {
    visibility_excludes_candidates_but_preserves_hidden_anchor_range_delivery();
    1
}
