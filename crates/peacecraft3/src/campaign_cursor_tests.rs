use super::*;
use bevy::ecs::system::RunSystemOnce;
use pcx_play::camera_input::edge;

fn hovered(relation: Relation, selected: bool) -> Option<Hovered> {
    Some(Hovered { relation, selected })
}

fn facts(armed: Armed, hover: Option<Hovered>) -> Facts {
    Facts {
        armed,
        hover,
        ..default()
    }
}

#[test]
fn a_pointer_over_nothing_shows_the_normal_cursor() {
    assert_eq!(pointer(Facts::default()), (Pointer::Normal, None));
}

#[test]
fn a_unit_under_the_pointer_selects_the_hover_cursor_of_its_relation() {
    for relation in [Relation::Friend, Relation::Neutral, Relation::Enemy] {
        assert_eq!(
            pointer(facts(Armed::Nothing, hovered(relation, false))),
            (Pointer::Hover(relation), None)
        );
    }
    // A selected own unit is still an own unit.
    assert_eq!(
        pointer(facts(Armed::Nothing, hovered(Relation::Friend, true))).0,
        Pointer::Hover(Relation::Friend)
    );
}

#[test]
fn the_interface_hides_the_unit_behind_it() {
    let over = Facts {
        over_interface: true,
        hover: hovered(Relation::Enemy, false),
        ..default()
    };
    assert_eq!(pointer(over).0, Pointer::Normal);
}

#[test]
fn an_armed_attack_colours_the_target_cursor_by_the_unit_but_not_for_a_selected_one() {
    assert_eq!(
        pointer(facts(Armed::Attack, None)).0,
        Pointer::Target(None),
        "ground"
    );
    for relation in [Relation::Friend, Relation::Neutral, Relation::Enemy] {
        assert_eq!(
            pointer(facts(Armed::Attack, hovered(relation, false))).0,
            Pointer::Target(Some(relation))
        );
    }
    assert_eq!(
        pointer(facts(Armed::Attack, hovered(Relation::Friend, true))).0,
        Pointer::Target(None),
        "the selected hero stayed plain in the live capture"
    );
}

#[test]
fn another_armed_command_keeps_the_plain_target_cursor_over_every_unit() {
    for relation in [Relation::Friend, Relation::Neutral, Relation::Enemy] {
        assert_eq!(
            pointer(facts(Armed::Target, hovered(relation, false))).0,
            Pointer::Target(None)
        );
    }
}

#[test]
fn an_armed_command_over_the_interface_shows_the_normal_cursor() {
    let over = Facts {
        over_interface: true,
        armed: Armed::Attack,
        ..default()
    };
    assert_eq!(pointer(over).0, Pointer::Normal);
}

#[test]
fn an_edge_band_shows_the_scroll_cursor_over_everything() {
    let edge_facts = Facts {
        edges: edge::UP | edge::LEFT,
        over_interface: true,
        armed: Armed::Attack,
        hover: hovered(Relation::Enemy, false),
    };
    assert_eq!(
        pointer(edge_facts),
        (Pointer::Scroll(edge::UP | edge::LEFT), None)
    );
}

#[test]
fn building_placement_falls_back_to_the_normal_cursor_and_says_so() {
    assert_eq!(
        pointer(facts(Armed::Build, hovered(Relation::Enemy, false))),
        (Pointer::Normal, Some(Situation::BuildPlacement))
    );
}

#[test]
fn only_commands_waiting_for_a_target_arm_the_cursor() {
    assert_eq!(Armed::of(Kind::Standard(StandardKind::Attack)), Armed::Attack);
    assert_eq!(Armed::of(Kind::Standard(StandardKind::Move)), Armed::Target);
    assert_eq!(Armed::of(Kind::Standard(StandardKind::Patrol)), Armed::Target);
    assert_eq!(Armed::of(Kind::TargetUnit), Armed::Target);
    assert_eq!(Armed::of(Kind::Standard(StandardKind::Stop)), Armed::Nothing);
    assert_eq!(Armed::of(Kind::Immediate), Armed::Nothing);
}

#[test]
fn the_model_follows_the_pointer_with_the_origin_at_the_window_centre() {
    let window = Vec2::new(1280.0, 960.0);
    assert_eq!(model_position(window / 2.0, window), Vec2::ZERO);
    // Top left corner: half the frame to the left and above; the frame is FRAME_H tall.
    let corner = model_position(Vec2::ZERO, window);
    assert!((corner.y - FRAME_H / 2.0).abs() < 1e-6);
    assert!((corner.x + FRAME_H * 1280.0 / 960.0 / 2.0).abs() < 1e-6);
    // A wide window keeps the scale: the same pixel offset gives the same model offset.
    let wide = Vec2::new(1920.0, 960.0);
    let offset = Vec2::new(100.0, 0.0);
    assert_eq!(
        model_position(wide / 2.0 + offset, wide),
        model_position(window / 2.0 + offset, window)
    );
}

#[test]
fn a_notice_is_kept_once() {
    let mut cursor = CampaignCursor::default();
    let notice = CursorDiagnostic::Unestablished(Situation::BuildPlacement);
    cursor.note(notice.clone());
    cursor.note(notice.clone());
    assert_eq!(cursor.diagnostics, [notice]);
}

#[test]
fn pointer_states_choose_the_sequence_and_tint_the_client_rules_give() {
    let rules = CursorRules::fixture();
    let look = |facts| {
        let look = rules.look(pointer(facts).0);
        (look.sequence.to_string(), look.tint)
    };
    let white = [255, 255, 255];
    assert_eq!(look(Facts::default()), ("Normal".into(), white));
    assert_eq!(
        look(facts(Armed::Nothing, hovered(Relation::Friend, false))),
        ("Select".into(), [0, 255, 0])
    );
    assert_eq!(
        look(facts(Armed::Nothing, hovered(Relation::Neutral, false))),
        ("Select".into(), [255, 255, 0])
    );
    assert_eq!(
        look(facts(Armed::Nothing, hovered(Relation::Enemy, false))),
        ("Select".into(), [255, 0, 0])
    );
    assert_eq!(look(facts(Armed::Target, None)), ("Target".into(), white));
    assert_eq!(
        look(facts(Armed::Attack, hovered(Relation::Enemy, false))),
        ("TargetSelect".into(), [255, 0, 0])
    );
    let scroll = |edges| {
        look(Facts {
            edges,
            ..default()
        })
    };
    assert_eq!(scroll(edge::LEFT).0, "Scroll Left");
    assert_eq!(scroll(edge::UP | edge::RIGHT).0, "Scroll Up Right");
    assert_eq!(scroll(edge::DOWN | edge::LEFT).0, "Scroll Down Left");
}

/// A world with a primary window, a cursor with a model root, and the cursor system.
fn present_world(active: bool, position: Option<Vec2>) -> (World, Entity, Entity) {
    let mut world = World::new();
    world.init_resource::<Assets<MenuMaterial>>();
    let camera = world.spawn_empty().id();
    let mut view = CampaignCamera::active_for_test(camera);
    if !active {
        view.suspend();
    }
    assert_eq!(view.active(), active);
    world.insert_resource(view);
    let window = world
        .spawn((Window::default(), PrimaryWindow, CursorOptions::default()))
        .id();
    let root = world
        .spawn((Transform::default(), Visibility::Hidden))
        .id();
    world.insert_resource(CampaignCursor {
        rules: Some(CursorRules::fixture()),
        model: Some(Model {
            root,
            camera,
            sequences: Vec::new(),
        }),
        position,
        ..default()
    });
    (world, window, root)
}

fn run_present(world: &mut World) {
    world.run_system_once(present).unwrap();
}

#[test]
fn the_system_cursor_hides_while_the_model_is_drawn_and_returns_after() {
    let at = Vec2::new(0.1, -0.2);
    let (mut world, window, root) = present_world(true, Some(at));
    run_present(&mut world);
    assert!(!world.get::<CursorOptions>(window).unwrap().visible);
    assert_eq!(*world.get::<Visibility>(root).unwrap(), Visibility::Inherited);
    assert_eq!(world.get::<Transform>(root).unwrap().translation, at.extend(0.0));
    // The pointer left the window.
    world.resource_mut::<CampaignCursor>().position = None;
    run_present(&mut world);
    assert!(world.get::<CursorOptions>(window).unwrap().visible);
    assert_eq!(*world.get::<Visibility>(root).unwrap(), Visibility::Hidden);
}

#[test]
fn the_system_cursor_stays_while_no_campaign_view_is_active() {
    let (mut world, window, root) = present_world(false, Some(Vec2::ZERO));
    run_present(&mut world);
    assert!(world.get::<CursorOptions>(window).unwrap().visible);
    assert_eq!(*world.get::<Visibility>(root).unwrap(), Visibility::Hidden);
}

fn classify_world(cursor_at: Option<Vec2>, focused: bool, edges: u8) -> World {
    let mut world = World::new();
    world.init_resource::<CampaignCursor>();
    world.init_resource::<CampaignInput>();
    world.init_resource::<HoverMap>();
    world.init_resource::<peacecraft3::game_input::GameInput>();
    world.init_resource::<peacecraft3::map_ground::GroundPublication>();
    world.init_resource::<crate::campaign_actors::UnitActors>();
    world.init_resource::<crate::campaign_inventory::CampaignInventory>();
    world.init_resource::<MapScriptSession<MapStartupHost>>();
    world.insert_resource(crate::campaign_camera_input::CameraInput::with_edges(edges));
    let mut window = Window {
        resolution: bevy::window::WindowResolution::new(1280, 960),
        ..default()
    };
    window.focused = focused;
    window.set_cursor_position(cursor_at);
    world.spawn((window, PrimaryWindow));
    world
}

#[test]
fn the_pointer_position_and_edge_band_reach_the_cursor_state() {
    let mut world = classify_world(Some(Vec2::new(0.0, 0.0)), true, edge::UP | edge::LEFT);
    world.run_system_once(classify).unwrap();
    let cursor = world.resource::<CampaignCursor>();
    assert_eq!(cursor.facts.edges, edge::UP | edge::LEFT);
    assert_eq!(cursor.position, Some(model_position(Vec2::ZERO, Vec2::new(1280.0, 960.0))));
    // No scene camera is published here, so the pointer is not over the world.
    assert!(cursor.facts.over_interface);
    assert_eq!(pointer(cursor.facts).0, Pointer::Scroll(edge::UP | edge::LEFT));
}

#[test]
fn an_unfocused_window_or_a_pointer_outside_it_gives_the_cursor_no_position() {
    for (at, focused) in [(None, true), (Some(Vec2::new(5.0, 5.0)), false)] {
        let mut world = classify_world(at, focused, edge::UP);
        world.run_system_once(classify).unwrap();
        let cursor = world.resource::<CampaignCursor>();
        assert_eq!(cursor.position, None);
        assert_eq!(cursor.facts, Facts::default());
    }
}

/// Real race cursor models. `PCX_WC3_INSTALL` names the Legacy install whose
/// `Warcraft III.exe` holds the rules; `PCX_CURSOR_ASSETS` names a directory holding
/// `UI\war3skins.txt` and `UI\Cursor\*` extracted from the install.
mod original {
    use super::*;
    use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode, OverrideDirectory, Resolver};

    const RACES: [(&str, &str); 4] = [
        ("RACE_PREF_HUMAN", "Human"),
        ("RACE_PREF_ORC", "Orc"),
        ("RACE_PREF_NIGHTELF", "NightElf"),
        ("RACE_PREF_UNDEAD", "Undead"),
    ];

    fn resolver() -> Resolver {
        let assets = std::env::var_os("PCX_CURSOR_ASSETS").expect("PCX_CURSOR_ASSETS");
        let mode = GameMode {
            build: Build::V1_29b,
            addon: Addon::Tft,
            assets: AssetMode::Vanilla,
            behaviour: Behaviour::Vanilla,
        };
        Resolver::new(
            mode,
            Some(OverrideDirectory::open(std::path::Path::new(&assets), "cursor-test", None).unwrap()),
            Vec::new(),
        )
        .unwrap()
    }

    fn rules() -> CursorRules {
        let install = std::env::var("PCX_WC3_INSTALL").expect("PCX_WC3_INSTALL");
        let path = std::path::Path::new(&install).join("Warcraft III.exe");
        CursorRules::read(
            &std::fs::read(&path).unwrap(),
            AssetIdentity {
                source: path.display().to_string(),
                edition: "Legacy target".into(),
                version: "1.29.2.9231".into(),
            },
        )
        .unwrap()
    }

    #[test]
    #[ignore = "requires PCX_WC3_INSTALL and PCX_CURSOR_ASSETS from the original install"]
    fn every_race_cursor_carries_every_sequence_the_client_asks_for() {
        let (reader, rules) = (resolver(), rules());
        for (race, section) in RACES {
            let path = model_path(&reader, Some(race.into())).unwrap();
            assert!(
                path.to_ascii_lowercase()
                    .contains(&format!("{}cursor", section.to_ascii_lowercase())),
                "{race}: {path}"
            );
            let Prepared {
                sequences,
                diagnostics,
                ..
            } = prepare(&reader, &path, &rules).unwrap();
            assert_eq!(diagnostics, [], "{race}");
            assert_eq!(sequences.len(), 14, "{race}");
        }
    }

    #[test]
    #[ignore = "requires PCX_WC3_INSTALL and PCX_CURSOR_ASSETS from the original install"]
    fn synthetic_pointer_states_pose_the_sequence_and_tint_of_their_mode() {
        let (reader, rules) = (resolver(), rules());
        let path = model_path(&reader, Some("RACE_PREF_HUMAN".into())).unwrap();
        let Prepared { plan, sequences, .. } = prepare(&reader, &path, &rules).unwrap();
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<MenuMaterial>::default();
        let mut images = Assets::<Image>::default();
        campaign_score_model::spawn_backgrounds_on(
            &mut world.commands(),
            crate::mainmenu_scene::GeosetAssets {
                meshes: &mut meshes,
                materials: &mut materials,
                images: &mut images,
            },
            root,
            vec![plan],
            CURSOR_LAYER,
        )
        .unwrap();
        world.flush();
        let interval = |name: &str| sequences.iter().find(|(n, _)| n == name).unwrap().1;
        let states: [(Facts, &str, [u8; 3]); 7] = [
            (Facts::default(), "Normal", [255, 255, 255]),
            (
                facts(Armed::Nothing, hovered(Relation::Friend, false)),
                "Select",
                [0, 255, 0],
            ),
            (
                facts(Armed::Nothing, hovered(Relation::Neutral, false)),
                "Select",
                [255, 255, 0],
            ),
            (
                facts(Armed::Nothing, hovered(Relation::Enemy, false)),
                "Select",
                [255, 0, 0],
            ),
            (facts(Armed::Target, None), "Target", [255, 255, 255]),
            (
                facts(Armed::Attack, hovered(Relation::Enemy, false)),
                "TargetSelect",
                [255, 0, 0],
            ),
            (
                Facts {
                    edges: edge::UP | edge::LEFT,
                    ..default()
                },
                "Scroll Up Left",
                [255, 255, 255],
            ),
        ];
        let mut query = world.query::<&mut ScoreBackgroundState>();
        let mut state = query.single_mut(&mut world).unwrap();
        for (facts, name, tint) in states {
            let look = rules.look(pointer(facts).0);
            assert_eq!(look.sequence, name);
            apply(look, &sequences, &mut state, &mut materials).unwrap();
            assert_eq!(state.interval, interval(name), "{name}");
            assert_eq!(state.elapsed, std::time::Duration::ZERO);
            for (_, _, handle) in &state.bound {
                let colour = materials.get(handle).unwrap().base.base_color.to_srgba();
                assert_eq!(
                    [colour.red, colour.green, colour.blue],
                    tint.map(|c| f32::from(c) / 255.0),
                    "{name}"
                );
            }
        }
        // The shipped Human cursor: Normal and Select intervals are the file's.
        assert_eq!(interval("Normal"), [333, 533]);
        assert_eq!(interval("Select"), [1000, 1500]);
    }

    #[test]
    #[ignore = "requires PCX_WC3_INSTALL and PCX_CURSOR_ASSETS from the original install"]
    fn a_sequence_the_model_lacks_is_refused_with_a_typed_diagnostic() {
        let (reader, rules) = (resolver(), rules());
        let path = model_path(&reader, Some("RACE_PREF_HUMAN".into())).unwrap();
        let Prepared { mut sequences, .. } = prepare(&reader, &path, &rules).unwrap();
        sequences.retain(|(name, _)| name != "TargetSelect");
        let look = rules.look(Pointer::Target(Some(Relation::Enemy)));
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let Prepared { plan, .. } = prepare(&reader, &path, &rules).unwrap();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<MenuMaterial>::default();
        let mut images = Assets::<Image>::default();
        campaign_score_model::spawn_backgrounds_on(
            &mut world.commands(),
            crate::mainmenu_scene::GeosetAssets {
                meshes: &mut meshes,
                materials: &mut materials,
                images: &mut images,
            },
            root,
            vec![plan],
            CURSOR_LAYER,
        )
        .unwrap();
        world.flush();
        let mut query = world.query::<&mut ScoreBackgroundState>();
        let mut state = query.single_mut(&mut world).unwrap();
        assert_eq!(
            apply(look, &sequences, &mut state, &mut materials),
            Err(CursorDiagnostic::MissingSequence {
                mode: 6,
                name: "TargetSelect".into()
            })
        );
    }
}
