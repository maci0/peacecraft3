//! Factory opt-in and transactional publication of retained associations.
use super::*;
use pcx_play::playercolor_host::{RegionBudget, RegionEventKind};

const EXTRA: &str = "\
type region extends agent
type triggeraction extends handle
native TriggerAddAction takes trigger t, code action returns triggeraction
native DisableTrigger takes trigger t returns nothing
native CreateRegion takes nothing returns region
native RegionAddRect takes region g, rect r returns nothing
native TriggerRegisterEnterRegion takes trigger t, region g, boolexpr f returns event
native TriggerRegisterLeaveRegion takes trigger t, region g, boolexpr f returns event
";
const MAP: &str = "\
globals
trigger t = null
region g = null
rect r = null
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
call DisableTrigger(t)
call TriggerAddAction(t,function action)
set g = CreateRegion()
set r = Rect(-4,2,8,10)
call RegionAddRect(g,r)
set a = TriggerRegisterEnterRegion(t,g,null)
set b = TriggerRegisterLeaveRegion(t,g,null)
endfunction
";
fn deliver_regions(app: &mut App, revision: u64, budget: Option<usize>) {
    let mut map = fixture(MAP);
    map.common.extend_from_slice(EXTRA.as_bytes());
    map.rebuild();
    let resolver = map.resolver();
    let bound = bind_map(&resolver, map.input(revision), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.budgets.regions = budget.map(|max_events| RegionBudget {
        max_regions: 1,
        max_rect_additions: 1,
        max_events,
    });
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
        label: "synthetic regions".into(),
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
fn published_region_requests_require_opt_in_and_survive_failed_replacement() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    deliver_regions(&mut app, 1, Some(2));
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
        assert_eq!(host.retained_region_events().unwrap(), 2);
        let first = host.region_event_registration(a).unwrap();
        let second = host.region_event_registration(b).unwrap();
        assert_eq!(
            (first.trigger, first.region, first.filter),
            (second.trigger, second.region, second.filter)
        );
        assert_eq!(first.kind, RegionEventKind::Enter);
        assert_eq!(second.kind, RegionEventKind::Leave);
        let requests = host.region_rect_requests(first.region).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].bounds, [-4.0, 2.0, 8.0, 10.0]);
        assert_eq!(host.retained_regions().unwrap(), 1);
        assert_eq!(host.retained_region_rect_requests().unwrap(), 1);
        assert_ne!(a, b);
        assert_eq!(host.trigger_enabled(first.trigger).unwrap(), Some(false));
        (a, first, Arc::clone(session.context().unwrap()))
    };
    for (revision, budget) in [(2, None), (3, Some(0))] {
        deliver_regions(&mut app, revision, budget);
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
                .region_event_registration(a)
                .unwrap(),
            first
        );
    }
}
