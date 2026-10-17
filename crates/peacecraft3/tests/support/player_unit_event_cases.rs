//! Factory opt-in and transactional publication of retained associations.
use super::*;

const EXTRA: &str = "\
type playerunitevent extends eventid
constant native ConvertPlayerUnitEvent takes integer i returns playerunitevent
native TriggerRegisterPlayerUnitEvent takes trigger t, player p, playerunitevent e, boolexpr f returns event
globals
constant playerunitevent SAMPLE = ConvertPlayerUnitEvent(27)
endglobals
";
const MAP: &str = "\
globals
trigger t = null
player p = null
filterfunc f = null
event a = null
event b = null
integer hits = 0
endglobals
function predicate takes nothing returns boolean
set hits = hits + 1
return true
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set t = CreateTrigger()
set p = Player(1)
set f = Filter(function predicate)
set a = TriggerRegisterPlayerUnitEvent(t,p,SAMPLE,null)
set b = TriggerRegisterPlayerUnitEvent(t,p,SAMPLE,f)
endfunction
";
fn deliver_events(app: &mut App, revision: u64, budget: Option<usize>) {
    let mut map = fixture(MAP);
    map.common.extend_from_slice(EXTRA.as_bytes());
    map.rebuild();
    let resolver = map.resolver();
    let bound = bind_map(&resolver, map.input(revision), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs
        .enumeration
        .names
        .push("ConvertPlayerUnitEvent".into());
    inputs.budgets.player_unit_events = budget;
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
        label: "synthetic player-unit events".into(),
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
fn published_player_unit_associations_require_opt_in_and_survive_failed_replacement() {
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
        let Value::Handle(f) = session.global("f").unwrap() else {
            panic!("filter")
        };
        let host = session.host().unwrap().objects();
        assert_eq!(host.retained_player_unit_events().unwrap(), 2);
        let first = host.player_unit_event_registration(a).unwrap();
        let second = host.player_unit_event_registration(b).unwrap();
        assert_eq!(first.filter, None);
        assert_eq!(second.filter, Some(f));
        assert_eq!(
            (first.trigger, first.player, first.playerunitevent),
            (second.trigger, second.player, second.playerunitevent)
        );
        assert_ne!(a, b);
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
                .player_unit_event_registration(a)
                .unwrap(),
            first
        );
    }
}
