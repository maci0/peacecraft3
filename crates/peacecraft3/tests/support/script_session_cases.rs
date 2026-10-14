use crate::fixtures;
use bevy::prelude::*;
use pcx_map::Map;
use pcx_play::map_admission::{
    IdentifiedScript, MapAdmissionBudget, MapAdmissionInputs, ParsedMapContext,
};
use pcx_play::map_scripts_import::MapScriptBudget;
use pcx_play::scripts::{AssetIdentity, ScriptAsset};
use pcx_script::{Host, NullHost, Value, value::FunctionId};
use peacecraft3::map_admission::MapAdmission;
use peacecraft3::map_script_session::{CallbackTicket, MapScriptSession, ScriptSessionError};
use std::{
    convert::Infallible,
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
pub(crate) const COMMON: &str = "native Init takes nothing returns integer\nnative Register takes code callback returns nothing\nnative Capture takes string text returns nothing\nnative Refuse takes nothing returns nothing\nnative GetPlayerNeutralPassive takes nothing returns integer\n";
pub(crate) const BJ: &str = "// source position padding\n\nfunction PreludeFail takes nothing returns nothing\ncall Refuse()\nendfunction\n";
pub(crate) const MAP: &str = "globals\ninteger counter = Init()\nendglobals\nfunction SelectedEntry takes nothing returns nothing\ncall Register(function Callback)\nendfunction\nfunction Callback takes integer amount returns integer\nset counter = counter + amount\ncall Capture(\"TRIGSTR_7\")\nreturn counter\nendfunction\nfunction BadEntry takes nothing returns nothing\ncall Capture(\"TRIGSTR_9\")\nendfunction\nfunction Read takes nothing returns integer\nreturn counter\nendfunction\n";
pub(crate) const WTS: &[u8] =
    b"STRING 7\r\n{\r\n|cffff0000Authored|r|nText\r\n}\r\nSTRING 9\r\n{\r\n\xff\r\n}\r\n";
fn identity(label: &str) -> AssetIdentity {
    let source = fixtures::source();
    AssetIdentity {
        edition: source.edition,
        version: "caller synthetic revision".into(),
        source: label.into(),
    }
}
fn admit(map_source: &str, wts: &[u8], label: &str) -> MapAdmission {
    let files = [
        ("war3map.w3e", fixtures::terrain_bytes(2, 1)),
        ("war3map.wpm", fixtures::pathing_bytes(4, 2)),
        ("war3map.w3i", fixtures::info().canonical_bytes()),
        ("war3mapUnits.doo", fixtures::units().canonical_bytes()),
        ("war3map.j", map_source.as_bytes().to_vec()),
        ("war3map.wts", wts.to_vec()),
    ];
    let raw = fixtures::build_synthetic_map(
        label,
        &files
            .iter()
            .map(|(n, b)| (*n, b.as_slice()))
            .collect::<Vec<_>>(),
    );
    let map = Map::parse(&raw).unwrap();
    let header = pcx_mpq::Header::parse(&mut Cursor::new(map.archive())).unwrap();
    let largest = files.iter().map(|(_, b)| b.len()).max().unwrap();
    let preludes = COMMON.len() + BJ.len();
    let scripts = MapScriptBudget {
        max_script_bytes: map_source.len(),
        max_wts_bytes: wts.len(),
        max_archive_bytes: map.archive().len(),
        max_archive_table_bytes: (header.hash_table_bytes() + header.block_table_bytes()) as usize,
        max_raw_packed_bytes: largest,
        max_decoded_bytes: largest,
        max_prelude_bytes: preludes,
        max_aggregate_input_bytes: map.archive().len() + preludes + map_source.len() + wts.len(),
    };
    let budget = MapAdmissionBudget {
        scripts,
        terrain_bytes: files[0].1.len(),
        pathing_bytes: files[1].1.len(),
        info_bytes: files[2].1.len(),
        units_bytes: files[3].1.len(),
        aggregate_input_bytes: map.archive().len()
            + preludes
            + files.iter().map(|(_, b)| b.len()).sum::<usize>(),
        required_fields: 1,
    };
    let mut admission = MapAdmission::default();
    admission
        .replace(
            MapAdmissionInputs {
                map: &map,
                map_label: label,
                map_identity: &identity(label),
                common: IdentifiedScript {
                    identity: &identity("selected-common"),
                    asset: ScriptAsset {
                        label: "caller/common.j",
                        bytes: COMMON.as_bytes(),
                    },
                },
                blizzard: IdentifiedScript {
                    identity: &identity("selected-BJ"),
                    asset: ScriptAsset {
                        label: "caller/BJ.j",
                        bytes: BJ.as_bytes(),
                    },
                },
                catalog_identity: &identity("selected-catalog"),
                catalog: &fixtures::catalog(),
                required_fields: &[*b"STAT"],
            },
            budget,
        )
        .unwrap();
    assert_eq!(
        admission.context().unwrap().map_digest.hash,
        pcx_script::fnv1a64(&raw)
    );
    admission
}
pub(crate) fn context(source: &str, wts: &[u8], label: &str) -> Arc<ParsedMapContext> {
    // Admission, original map container, catalog and script input owners all drop.
    admit(source, wts, label).shared_context().unwrap()
}
pub(crate) fn id(context: &ParsedMapContext, name: &str) -> FunctionId {
    context.scripts.program().function_id(name).unwrap()
}
pub(crate) struct StatefulHost {
    pub(crate) context: Arc<ParsedMapContext>,
    pub(crate) callbacks: Vec<FunctionId>,
    pub(crate) text: Vec<String>,
    pub(crate) inits: usize,
    pub(crate) drops: Arc<AtomicUsize>,
    pub(crate) external_effect: Option<Arc<AtomicUsize>>,
}
impl StatefulHost {
    pub(crate) fn new(context: Arc<ParsedMapContext>, drops: &Arc<AtomicUsize>) -> Self {
        Self {
            context,
            callbacks: Vec::new(),
            text: Vec::new(),
            inits: 0,
            drops: Arc::clone(drops),
            external_effect: None,
        }
    }
}
impl Drop for StatefulHost {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl Host for StatefulHost {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        match (name, args) {
            ("Init", []) => {
                self.inits += 1;
                if let Some(counter) = &self.external_effect {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
                Ok(Value::Int(5))
            }
            ("Register", [Value::Code(callback)]) => {
                self.callbacks.push(*callback);
                Ok(Value::Null)
            }
            ("Capture", [Value::Str(text)]) => {
                self.text.push(text.clone());
                Ok(Value::Null)
            }
            _ => Err(format!("explicit unsupported fixture native {name}")),
        }
    }
}
pub(crate) fn initialize(
    context: Arc<ParsedMapContext>,
    drops: &Arc<AtomicUsize>,
) -> MapScriptSession<StatefulHost> {
    let entries = [id(&context, "SelectedEntry")];
    let mut session = MapScriptSession::default();
    session
        .replace(
            context,
            |context| Ok::<_, Infallible>(StatefulHost::new(context, drops)),
            &entries,
        )
        .unwrap();
    session
}
#[derive(Resource, Default)]
struct Delivery {
    calls: Vec<(CallbackTicket, Vec<Value>)>,
    results: Vec<Value>,
}
fn update(mut session: ResMut<MapScriptSession<StatefulHost>>, mut delivery: ResMut<Delivery>) {
    let calls = std::mem::take(&mut delivery.calls);
    for (id, args) in calls {
        delivery
            .results
            .push(session.call_callback(&id, &args).unwrap());
    }
}

pub fn admitted_resource_preserves_vm_host_and_callbacks_across_updates() {
    let drops = Arc::new(AtomicUsize::new(0));
    let ctx = context(MAP, WTS, "first-map");
    let weak = Arc::downgrade(&ctx);
    let script_weak = Arc::downgrade(&ctx.scripts);
    let callback = id(&ctx, "Callback");
    let world_x = ctx.world.records[0].placement.position[0];
    let entry = id(&ctx, "SelectedEntry");
    let factories = Arc::new(AtomicUsize::new(0));
    let mut state = MapScriptSession::default();
    state
        .replace(
            ctx,
            |context| {
                factories.fetch_add(1, Ordering::SeqCst);
                Ok::<_, Infallible>(StatefulHost::new(context, &drops))
            },
            &[entry],
        )
        .unwrap();
    assert_eq!(state.host().unwrap().callbacks, [callback]);
    assert_eq!(
        state.host().unwrap().context.world.records[0]
            .placement
            .position[0],
        world_x
    );
    let ticket = state.callback_ticket(callback).unwrap();
    let mut app = App::new();
    app.insert_resource(state);
    app.init_resource::<Delivery>();
    app.add_systems(Update, update);
    let initial_ecs_host = app
        .world_mut()
        .resource_mut::<MapScriptSession<StatefulHost>>()
        .host()
        .unwrap() as *const StatefulHost;
    for _ in 0..3 {
        app.world_mut()
            .resource_mut::<Delivery>()
            .calls
            .push((ticket.clone(), vec![Value::Int(2)]));
        app.update();
    }
    let ecs_host_address = {
        let mut state = app
            .world_mut()
            .resource_mut::<MapScriptSession<StatefulHost>>();
        assert_eq!(state.global("counter"), Some(Value::Int(11)));
        assert_eq!(state.host().unwrap().inits, 1);
        assert_eq!(
            state.host().unwrap().text,
            vec!["|cffff0000Authored|r|nText"; 3]
        );
        assert_eq!(factories.load(Ordering::SeqCst), 1);
        {
            let active_context = Arc::clone(state.context().unwrap());
            assert!(Arc::ptr_eq(&active_context, &state.host().unwrap().context));
        }
        // The resource moves into ECS once; callback delivery keeps the same Host there.
        let ecs_host_address = state.host().unwrap() as *const StatefulHost;
        assert_eq!(ecs_host_address, initial_ecs_host);
        ecs_host_address
    };
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<MapScriptSession<StatefulHost>>()
            .host()
            .unwrap() as *const StatefulHost,
        ecs_host_address
    );
    assert_eq!(
        app.world().resource::<Delivery>().results,
        [Value::Int(7), Value::Int(9), Value::Int(11)]
    );
    assert!(weak.upgrade().is_some());
    drop(app);
    assert!(weak.upgrade().is_none());
    assert!(script_weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

pub fn failed_candidate_keeps_old_context_and_retains_candidate_diagnostics() {
    let drops = Arc::new(AtomicUsize::new(0));
    let old = context(MAP, WTS, "old-map");
    let old_weak = Arc::downgrade(&old);
    let callback = id(&old, "Callback");
    let mut state = initialize(old, &drops);
    state.call_function(callback, &[Value::Int(3)]).unwrap();
    let host_address = state.host().unwrap() as *const StatefulHost;
    let candidate = context(MAP, WTS, "failed-map");
    let candidate_weak = Arc::downgrade(&candidate);
    let entry = id(&candidate, "BadEntry");
    let error = state
        .replace(
            candidate,
            |ctx| Ok::<_, Infallible>(StatefulHost::new(ctx, &drops)),
            &[entry],
        )
        .unwrap_err();
    let ScriptSessionError::Execution(failure) = error else {
        panic!("expected retained execution error");
    };
    assert_eq!(failure.identity.source, "failed-map");
    assert_eq!(failure.function, Some(entry));
    assert_eq!(failure.source_label.as_deref(), Some("the map script"));
    assert!(
        failure.diagnostic.starts_with("failed-map:war3map.j:13:14"),
        "{}",
        failure.diagnostic
    );
    assert!(
        failure
            .diagnostic
            .contains("failed-map:war3map.wts: WTS entry 9 is not UTF-8 at body byte 0")
    );
    assert_eq!(
        &MAP[failure.error.span.start as usize..failure.error.span.end as usize],
        "\"TRIGSTR_9\""
    );
    assert!(candidate_weak.upgrade().is_none());
    assert!(old_weak.upgrade().is_some());
    assert_eq!(state.context().unwrap().sources.map.source, "old-map");
    assert_eq!(state.host().unwrap() as *const StatefulHost, host_address);
    assert_eq!(state.global("counter"), Some(Value::Int(8)));
    assert_eq!(state.host().unwrap().inits, 1);
    assert_eq!(
        state.call_function(callback, &[Value::Int(4)]).unwrap(),
        Value::Int(12)
    );
    state.clear();
    assert!(old_weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

pub fn neutral_global_refusal_preserves_an_existing_session() {
    let safe = context(
        "function ExplicitReady takes nothing returns integer\nreturn 41\nendfunction\n",
        WTS,
        "safe-map",
    );
    let ready = id(&safe, "ExplicitReady");
    let mut state = MapScriptSession::default();
    state
        .replace(safe, |_| Ok::<_, Infallible>(NullHost), &[])
        .unwrap();
    let source = "globals\ninteger neutral = GetPlayerNeutralPassive()\nendglobals\nfunction AuthoredEntry takes nothing returns nothing\nendfunction\n";
    let candidate = context(source, WTS, "neutral-map");
    let weak = Arc::downgrade(&candidate);
    let entry = id(&candidate, "AuthoredEntry");
    let error = state
        .replace(candidate, |_| Ok::<_, Infallible>(NullHost), &[entry])
        .unwrap_err();
    let ScriptSessionError::Execution(failure) = error else {
        panic!("expected unsupported native boundary");
    };
    assert_eq!(failure.function, None);
    assert_eq!(failure.source_label.as_deref(), Some("the map script"));
    assert!(
        failure.diagnostic.starts_with("neutral-map:war3map.j:2:19"),
        "{}",
        failure.diagnostic
    );
    assert!(
        failure
            .error
            .message
            .contains("no host implements the native `GetPlayerNeutralPassive`")
    );
    assert!(weak.upgrade().is_none());
    assert_eq!(state.context().unwrap().sources.map.source, "safe-map");
    assert_eq!(state.call_function(ready, &[]).unwrap(), Value::Int(41));
}

pub fn explicit_factory_and_id_refusals_preserve_active_state() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut empty = MapScriptSession::<StatefulHost>::default();
    assert!(matches!(
        empty.call_function(FunctionId(0), &[]),
        Err(ScriptSessionError::NoSession)
    ));
    let ctx = context(MAP, WTS, "factory-map");
    let factories = AtomicUsize::new(0);
    let error = empty
        .replace(
            Arc::clone(&ctx),
            |ctx| {
                factories.fetch_add(1, Ordering::SeqCst);
                Ok::<_, Infallible>(StatefulHost::new(ctx, &drops))
            },
            &[FunctionId(u32::MAX)],
        )
        .unwrap_err();
    assert!(matches!(
        error,
        ScriptSessionError::UnknownFunction {
            id: FunctionId(u32::MAX),
            ..
        }
    ));
    assert_eq!(factories.load(Ordering::SeqCst), 0);
    let mut state = initialize(ctx, &drops);
    let old = state.context().unwrap().sources.map.clone();
    let candidate = context(MAP, WTS, "factory-refused-map");
    let weak = Arc::downgrade(&candidate);
    let error = state
        .replace(
            candidate,
            |_| Err::<StatefulHost, _>(std::io::Error::other("explicit isolated factory refusal")),
            &[],
        )
        .unwrap_err();
    let ScriptSessionError::Factory { identity, source } = error else {
        panic!("expected factory error");
    };
    assert_eq!(identity.source, "factory-refused-map");
    assert!(source.downcast_ref::<std::io::Error>().is_some());
    assert_eq!(source.to_string(), "explicit isolated factory refusal");
    assert!(weak.upgrade().is_none());
    assert_eq!(state.context().unwrap().sources.map, old);
    assert_eq!(state.global("counter"), Some(Value::Int(5)));
    let error = state.call_function(FunctionId(u32::MAX), &[]).unwrap_err();
    assert!(matches!(error, ScriptSessionError::UnknownFunction { .. }));
    assert_eq!(state.host().unwrap().inits, 1);
}

pub fn replacement_releases_old_context_and_switches_world_wts_and_state_together() {
    let drops = Arc::new(AtomicUsize::new(0));
    let old = context(MAP, WTS, "replace-old");
    let old_weak = Arc::downgrade(&old);
    let scripts_weak = Arc::downgrade(&old.scripts);
    let old_callback = id(&old, "Callback");
    let mut state = initialize(old, &drops);
    state.call_function(old_callback, &[Value::Int(9)]).unwrap();
    let new_source = MAP.replace("counter = Init()", "counter = Init() + 20");
    let next = context(
        &new_source,
        b"STRING 7\r\n{\r\nReplacement body\r\n}\r\n",
        "replace-new",
    );
    let new_weak = Arc::downgrade(&next);
    let callback = id(&next, "Callback");
    let entry = id(&next, "SelectedEntry");
    state
        .replace(
            next,
            |ctx| Ok::<_, Infallible>(StatefulHost::new(ctx, &drops)),
            &[entry],
        )
        .unwrap();
    assert!(old_weak.upgrade().is_none());
    assert!(scripts_weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(state.context().unwrap().sources.map.source, "replace-new");
    assert_eq!(
        state.host().unwrap().context.world.source.map,
        "replace-new"
    );
    assert_eq!(state.global("counter"), Some(Value::Int(25)));
    assert_eq!(
        state.call_function(callback, &[Value::Int(2)]).unwrap(),
        Value::Int(27)
    );
    assert_eq!(state.host().unwrap().text, ["Replacement body"]);
    assert_eq!(state.host().unwrap().inits, 1);
    state.clear();
    assert!(new_weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert!(state.context().is_none());
}

pub fn source_errors_and_external_effects_keep_their_explicit_boundaries() {
    let drops = Arc::new(AtomicUsize::new(0));
    let old = context(MAP, WTS, "boundary-old");
    let callback = id(&old, "Callback");
    let prelude = id(&old, "PreludeFail");
    let mut state = initialize(old, &drops);
    let error = state.call_function(callback, &[]).unwrap_err();
    let ScriptSessionError::Execution(failure) = error else {
        panic!("expected arity refusal");
    };
    assert_eq!(failure.function, Some(callback));
    assert_eq!(failure.source_label.as_deref(), Some("the map script"));
    assert!(failure.diagnostic.starts_with("boundary-old:war3map.j:7:1"));
    let error = state.call_function(prelude, &[]).unwrap_err();
    let ScriptSessionError::Execution(failure) = error else {
        panic!("expected native refusal");
    };
    assert_eq!(failure.source_label.as_deref(), Some("caller/BJ.j"));
    assert!(failure.diagnostic.starts_with("caller/BJ.j:4:1"));
    assert!(
        failure
            .error
            .message
            .contains("explicit unsupported fixture native Refuse")
    );
    assert_eq!(
        state.call_function(callback, &[Value::Int(1)]).unwrap(),
        Value::Int(6)
    );
    let candidate = context(MAP, WTS, "external-effect-candidate");
    let entry = id(&candidate, "BadEntry");
    let effects = Arc::new(AtomicUsize::new(0));
    // Deliberately non-isolated test Host illustrates the documented caller seam.
    let error = state
        .replace(
            candidate,
            |ctx| {
                let mut host = StatefulHost::new(ctx, &drops);
                host.external_effect = Some(Arc::clone(&effects));
                Ok::<_, Infallible>(host)
            },
            &[entry],
        )
        .unwrap_err();
    assert!(matches!(error, ScriptSessionError::Execution(_)));
    assert_eq!(effects.load(Ordering::SeqCst), 1); // The wrapper does not undo external effects.
    assert_eq!(state.context().unwrap().sources.map.source, "boundary-old");
    assert_eq!(state.global("counter"), Some(Value::Int(6)));
}
