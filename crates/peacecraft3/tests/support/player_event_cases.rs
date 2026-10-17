//! Factory opt-in and transactional publication of retained associations.
use super::*;

const EXTRA: &str = "\
type playerevent extends eventid
type triggeraction extends handle
native TriggerAddAction takes trigger t, code action returns triggeraction
native DisableTrigger takes trigger t returns nothing
constant native ConvertPlayerEvent takes integer i returns playerevent
native TriggerRegisterPlayerEvent takes trigger t, player p, playerevent e returns event
globals
constant playerevent SAMPLE = ConvertPlayerEvent(27)
endglobals
";
const MAP: &str = "\
globals
trigger t = null
player p = null
event a = null
event b = null
integer hits = 0
endglobals
function action takes nothing returns nothing
set hits = hits + 1
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set t = CreateTrigger()
set p = Player(1)
call DisableTrigger(t)
call TriggerAddAction(t,function action)
set a = TriggerRegisterPlayerEvent(t,p,SAMPLE)
set b = TriggerRegisterPlayerEvent(t,p,SAMPLE)
endfunction
";
fn deliver_events(app: &mut App, revision: u64, budget: Option<usize>) {
    let mut map = fixture(MAP);
    map.common.extend_from_slice(EXTRA.as_bytes());
    map.rebuild();
    let resolver = map.resolver();
    let bound = bind_map(&resolver, map.input(revision), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.enumeration.names.push("ConvertPlayerEvent".into());
    inputs.budgets.player_events = budget;
    let bytes = Arc::clone(&bound.catalog_map_bytes);
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..Default::default()
    });
    app.insert_resource(CampaignHostSupply {
        revision,
        inputs: Some(inputs),
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic player events".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id.clone(),
        binding_revision: revision,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
    assert!(
        app.world()
            .resource::<MapRequests>()
            .last_outcome
            .as_ref()
            .unwrap()
            .result
            .is_ok()
    );
}
#[test]
fn published_player_associations_require_opt_in_and_survive_failed_replacement() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    deliver_events(&mut app, 1, Some(2));
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
    let witness = session.session_witness().unwrap();
    let (a, first, context) = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert_eq!(session.global("hits"), Some(Value::Int(0)));
        let Value::Handle(a) = session.global("a").unwrap() else {
            panic!("event")
        };
        let Value::Handle(b) = session.global("b").unwrap() else {
            panic!("event")
        };
        let host = session.host().unwrap().objects();
        assert_eq!(host.retained_player_events().unwrap(), 2);
        let first = host.player_event_registration(a).unwrap();
        let second = host.player_event_registration(b).unwrap();
        assert_eq!(
            (first.trigger, first.player, first.playerevent),
            (second.trigger, second.player, second.playerevent)
        );
        assert_ne!(a, b);
        assert_eq!(host.trigger_enabled(first.trigger).unwrap(), Some(false));
        (a, first, Arc::clone(session.context().unwrap()))
    };
    for (revision, budget) in [(2, None), (3, Some(0))] {
        deliver_events(&mut app, revision, budget);
        assert!(matches!(
            app.world().resource::<CampaignStartup>().last_error,
            Some(StartupError::Session(ScriptSessionError::Execution(_)))
        ));
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert!(witness.upgrade().is_some());
        assert!(Arc::ptr_eq(session.context().unwrap(), &context));
        assert_eq!(
            session
                .host()
                .unwrap()
                .objects()
                .player_event_registration(a)
                .unwrap(),
            first
        );
    }
}
