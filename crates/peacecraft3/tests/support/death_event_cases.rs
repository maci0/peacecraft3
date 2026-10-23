//! Factory opt-in and transactional publication of retained associations.
use super::*;

const EXTRA: &str = "\
type triggeraction extends handle
type widgetevent extends eventid
constant native ConvertWidgetEvent takes integer value returns widgetevent
native TriggerAddAction takes trigger t, code action returns triggeraction
native DisableTrigger takes trigger t returns nothing
native TriggerRegisterDeathEvent takes trigger t, widget w returns event
globals
constant widgetevent EVENT_WIDGET_DEATH = ConvertWidgetEvent(47)
endglobals
";
const MAP: &str = "\
globals
trigger t = null
destructable d = null
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
set d = CreateDestructable('Zxyz',1.0,2.0,3.0,1.0,0)
call DisableTrigger(t)
call TriggerAddAction(t,function action)
set a = TriggerRegisterDeathEvent(t,d)
set b = TriggerRegisterDeathEvent(t,d)
endfunction
";
fn deliver_deaths(app: &mut App, revision: u64, budget: Option<usize>) {
    let mut map = fixture(MAP);
    map.common.extend_from_slice(EXTRA.as_bytes());
    map.rebuild();
    let resolver = map.resolver();
    let bound = bind_map(&resolver, map.input(revision), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.enumeration.names.push("ConvertWidgetEvent".into());
    inputs.budgets.death_events = budget;
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
        label: "synthetic death events".into(),
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
fn published_death_associations_require_opt_in_and_survive_failed_replacement() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    deliver_deaths(&mut app, 1, Some(2));
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
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
        assert_eq!(host.retained_death_events().unwrap(), 2);
        let first = host.death_event_registration(a).unwrap();
        let second = host.death_event_registration(b).unwrap();
        assert_eq!(
            (first.trigger, first.widget),
            (second.trigger, second.widget)
        );
        assert_ne!(a, b);
        assert_eq!(host.trigger_enabled(first.trigger).unwrap(), Some(false));
        (a, first, Arc::clone(session.context().unwrap()))
    };
    for (revision, budget) in [(2, None), (3, Some(0))] {
        deliver_deaths(&mut app, revision, budget);
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
                .death_event_registration(a)
                .unwrap(),
            first
        );
    }
}
