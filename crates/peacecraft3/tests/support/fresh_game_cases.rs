//! Fresh roster reaches the production Host's existing arena without a census snapshot.
use super::*;
use pcx_play::map_startup_host::{NewGameInput, resolve_observations};
use pcx_script::Host;

#[test]
fn fresh_roster_keeps_executed_controllers_and_only_occupies_selected_slots() {
    let mut map = fixture(
        "function config takes nothing returns nothing\ncall SetPlayerController(Player(0), USER_CONTROL)\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n",
    );
    map.common.extend_from_slice(b"\ntype playerslotstate extends handle\nconstant native ConvertPlayerSlotState takes integer n returns playerslotstate\nconstant native ConvertMapControl takes integer n returns mapcontrol\nnative GetPlayerSlotState takes player p returns playerslotstate\nconstant native GetLocalPlayer takes nothing returns player\nnative ForceEnumPlayers takes force f, boolexpr filter returns nothing\nglobals\nconstant mapcontrol USER_CONTROL = ConvertMapControl(91)\nconstant mapcontrol MAP_CONTROL_NONE = ConvertMapControl(-31)\nconstant mapcontrol MAP_CONTROL_CREEP = ConvertMapControl(71)\nconstant playerslotstate PLAYER_SLOT_STATE_EMPTY = ConvertPlayerSlotState(-17)\nconstant playerslotstate PLAYER_SLOT_STATE_PLAYING = ConvertPlayerSlotState(29)\nendglobals\n");
    map.rebuild();
    let reader = map.resolver();
    let bound = bind_map(&reader, map.input(1), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.rules.max_players = rule_with_getter(RuleKind::MaxPlayers, getter(257, -20, 57));
    inputs
        .enumeration
        .names
        .extend(["ConvertPlayerSlotState".into(), "ConvertMapControl".into()]);
    let bytes = Arc::clone(&bound.catalog_map_bytes);
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..Default::default()
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic fresh roster".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id,
        binding_revision: 1,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
    let context = app
        .world()
        .resource::<MapRequests>()
        .admission
        .shared_context()
        .unwrap();
    let host = pcx_play::map_startup_host::assemble(Arc::clone(&context), inputs).unwrap();
    let mut vm = Arc::clone(&context.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("config", &[]).unwrap();
    let post = PostConfigObservations {
        new_game: Some(NewGameInput {
            player_colors: None,
            initial_alliances: None,
            orders: None,
            activity: None,
            upgrades: pcx_objects::editor::Overlay::default(),
            group_orders: None,
            seed: 0,
            research: None,
            placement_rules: None,
            unit_range_event: None,
            day_cycle: None,
            game_camera: None,
            hero_vitals: None,
            combat: None,
        }),
        local_player: Some(0),
        participants: Some(vec![0]),
        ..Default::default()
    };
    let resolved =
        resolve_observations(context.scripts.program(), &post, |name| vm.global(name)).unwrap();
    let handles = vm.host().inner.objects().inner().handles();
    vm.host()
        .inner
        .apply_post_config(&context, &resolved)
        .unwrap();
    assert_eq!(vm.host().inner.objects().inner().handles(), handles);
    assert!(vm.host().inner.objects().player_identity(3).is_none());
    let empty = vm.global("PLAYER_SLOT_STATE_EMPTY");
    let playing = vm.global("PLAYER_SLOT_STATE_PLAYING");
    let user = vm.global("USER_CONTROL");
    let none = vm.global("MAP_CONTROL_NONE");
    let creep = vm.global("MAP_CONTROL_CREEP");
    let local = vm.host().inner.native("GetLocalPlayer", &[]).unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native("GetPlayerController", std::slice::from_ref(&local))
            .unwrap(),
        user
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetPlayerSlotState", std::slice::from_ref(&local))
            .unwrap(),
        playing
    );
    let late = vm.host().inner.native("Player", &[Value::Int(3)]).unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native("GetPlayerController", std::slice::from_ref(&late))
            .unwrap(),
        none
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetPlayerSlotState", &[late])
            .unwrap(),
        empty
    );
    let Value::Int(normal) = vm.host().inner.native("GetBJMaxPlayers", &[]).unwrap() else {
        panic!("source count")
    };
    assert!(usize::try_from(normal).unwrap() < vm.host().inner.objects().player_slots().unwrap());
    let neutral = vm
        .host()
        .inner
        .native("Player", &[Value::Int(normal)])
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native("GetPlayerController", std::slice::from_ref(&neutral))
            .unwrap(),
        creep
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetPlayerSlotState", &[neutral])
            .unwrap(),
        playing
    );
    for participants in [vec![0, 0], vec![-1], vec![normal], vec![]] {
        let bad = PostConfigObservations {
            participants: Some(participants),
            ..post.clone()
        };
        let bad =
            resolve_observations(context.scripts.program(), &bad, |name| vm.global(name)).unwrap();
        assert!(vm.host().inner.apply_post_config(&context, &bad).is_err());
        assert_eq!(
            vm.host()
                .inner
                .native("GetPlayerSlotState", std::slice::from_ref(&local))
                .unwrap(),
            playing
        );
    }
}
