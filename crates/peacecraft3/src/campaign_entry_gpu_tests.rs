//! Explicit local-asset diagnostic of the production UI and renderer.
use bevy::{
    asset::AssetPlugin,
    camera::RenderTarget,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    render::{
        RenderPlugin,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        settings::{Backends, RenderCreation, WgpuSettings},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    ui::{InteractionDisabled, UiSystems},
    ui_widgets::{Activate, Button},
    window::{ExitCondition, PrimaryWindow},
    winit::WinitPlugin,
};
use pcx_play::map_startup_host::MapStartupHost;
use peacecraft3::{
    campaign_camera::CampaignCamera, campaign_startup::CampaignStartup,
    map_script_session::MapScriptSession,
};
use std::{path::Path, time::Duration};

#[derive(Resource)]
struct Target(Handle<Image>);

#[derive(Resource, Default)]
struct Capture(Option<Image>);

fn offscreen(target: Res<Target>, mut cameras: Query<&mut RenderTarget>) {
    for mut camera in &mut cameras {
        if matches!(*camera, RenderTarget::Window(_)) {
            *camera = RenderTarget::Image(target.0.clone().into());
        }
    }
}

fn step(app: &mut App) {
    app.update();
    app.world()
        .resource::<RenderDevice>()
        .wgpu_device()
        .poll(PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(20)),
        })
        .expect("GPU completion");
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "startup: {:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
}

fn until(app: &mut App, label: &str, ready: impl Fn(&mut World) -> bool) {
    for _ in 0..600 {
        step(app);
        if ready(app.world_mut()) {
            return;
        }
    }
    let navigation = app.world().resource::<crate::menu_screens::Navigation>();
    panic!(
        "{label} did not complete in 600 updates: screen={:?}, requested={:?}, transition={}, built={:?}, notice={:?}",
        navigation.screen,
        navigation.requested,
        navigation.transition_active,
        navigation.built,
        navigation.notice
    );
}

fn escape(app: &mut App, state: ButtonState) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Escape,
        logical_key: Key::Escape,
        state,
        repeat: false,
        window,
        text: None,
    });
}

fn activate(app: &mut App, entity: Entity) {
    assert!(app.world().entity(entity).contains::<Button>());
    assert!(!app.world().entity(entity).contains::<InteractionDisabled>());
    println!(
        "activate {:?}",
        app.world()
            .entity(entity)
            .get::<crate::glue_menu::GlueButton>()
            .map(|b| &b.frame)
    );
    app.world_mut().trigger(Activate { entity });
}

fn menu_key(app: &mut App, state: ButtonState) {
    let code = app
        .world()
        .resource::<CampaignCamera>()
        .menu_key
        .clone()
        .unwrap()
        .unwrap();
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        state,
        repeat: false,
        window,
        text: None,
    });
}

fn screen(app: &mut App, expected: crate::menu_screens::Screen) {
    until(app, "menu navigation", |world| {
        let navigation = world.resource::<crate::menu_screens::Navigation>();
        navigation.screen == expected
            && navigation.requested.is_none()
            && !navigation.transition_active
            && navigation.built == Some(expected)
            && if expected == crate::menu_screens::Screen::Campaign {
                world.contains_resource::<crate::mainmenu_scene::MainMenuScene>()
                    && world
                        .get_resource::<crate::menu_sprites::MenuPanels>()
                        .is_none_or(|panels| panels.retired())
            } else {
                world
                    .get_resource::<crate::menu_sprites::MenuPanels>()
                    .is_some_and(|panels| panels.navigation_settled())
            }
    });
}

fn frame_button(app: &mut App, frame: &str) -> Entity {
    app.world_mut()
        .query::<(Entity, &crate::glue_menu::GlueButton)>()
        .iter(app.world())
        .find(|(_, b)| b.frame == frame)
        .expect(frame)
        .0
}

fn capture(app: &mut App, output: &Path) {
    assert!(!output.exists(), "capture must be new");
    for _ in 0..12 {
        step(app);
    }
    let target = app.world().resource::<Target>().0.clone();
    app.world_mut().resource_mut::<Capture>().0 = None;
    app.world_mut().spawn(Screenshot::image(target)).observe(
        |event: On<ScreenshotCaptured>, mut capture: ResMut<Capture>| {
            capture.0 = Some(event.image.clone());
        },
    );
    until(app, "GPU screenshot", |w| {
        w.resource::<Capture>().0.is_some()
    });
    let image = app.world_mut().resource_mut::<Capture>().0.take().unwrap();
    image.try_into_dynamic().unwrap().save(output).unwrap();
}

fn color_snapshot(
    app: &mut App,
) -> (
    std::collections::BTreeMap<pcx_script::HandleId, i32>,
    Vec<u8>,
) {
    let colors = app
        .world()
        .resource::<crate::campaign_actors::UnitActors>()
        .rendered_colors();
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let objects = session.host().unwrap().objects();
    let mut units = 0;
    for (&id, &color) in &colors {
        if let Ok(unit) = objects.executed_unit(id) {
            let original = crate::campaign_actors::original_team_color(objects, unit).unwrap();
            assert_eq!(color, objects.display_team_color(unit, original).unwrap());
            units += 1;
        }
    }
    assert!(
        units > 0,
        "actual model instances must use the host's effective palette"
    );
    let image = app
        .world()
        .resource::<crate::campaign_minimap::CampaignMinimap>()
        .diagnostic_image()
        .unwrap();
    let pixels = app
        .world()
        .resource::<Assets<Image>>()
        .get(&image)
        .unwrap()
        .data
        .clone()
        .unwrap();
    (colors, pixels)
}

fn item_visibility_roundtrip(app: &mut App, output: &Path) {
    use pcx_script::{Host, Value};
    let id = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .executed_items()
        .find(|(_, item)| item.visible() == Some(true))
        .map(|(id, _)| id)
        .expect("diagnostic chapter needs a visible ground item");
    let (root, ray, far) = app
        .world()
        .resource::<crate::campaign_actors::UnitActors>()
        .item_probe(id)
        .expect("source item actor and nonempty bounds");
    let hit = |app: &mut App| {
        app.world_mut().resource_scope(
            |world, mut session: Mut<MapScriptSession<MapStartupHost>>| {
                let witness = session.session_witness().unwrap();
                world
                    .resource::<crate::campaign_actors::UnitActors>()
                    .item_hit(ray, far, &witness, session.host().unwrap().objects())
            },
        )
    };
    assert_eq!(hit(app), Some(id));
    for shown in [false, true] {
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .native("SetItemVisible", &[Value::Handle(id), Value::Bool(shown)])
            .unwrap();
        assert_eq!(
            hit(app),
            shown.then_some(id),
            "picking must see canonical state before projection"
        );
        step(app);
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            if shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            }
        );
        assert_eq!(hit(app), shown.then_some(id));
        capture(
            app,
            &output.join(if shown {
                "item-shown.png"
            } else {
                "item-hidden.png"
            }),
        );
    }
    std::fs::write(output.join("item-visibility.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "canonical_picking_before_projection": true, "actor_visibility_roundtrip": true,
        "scope": "source ground item actor root and actual ray picking; captures retained, no original pixel-parity claim"
    })).unwrap()).unwrap();
}

#[test]
#[ignore = "requires explicit original install/edition/map, Vulkan and a new PCX_CAMPAIGN_CAPTURE_DIR"]
fn mounted_menu_enters_the_rendered_campaign() {
    let install = std::env::var("PCX_NATIVE_CHAPTER_INSTALL").expect("PCX_NATIVE_CHAPTER_INSTALL");
    let map = std::env::var("PCX_NATIVE_CHAPTER_MAP").expect("PCX_NATIVE_CHAPTER_MAP");
    let expansion = match std::env::var("PCX_NATIVE_CHAPTER_ADDON").as_deref() {
        Ok("roc") => false,
        Ok("tft") => true,
        _ => panic!("select roc or tft explicitly"),
    };
    let output = std::path::PathBuf::from(
        std::env::var_os("PCX_CAMPAIGN_CAPTURE_DIR").expect("PCX_CAMPAIGN_CAPTURE_DIR"),
    );
    assert!(!output.exists(), "output directory must be new");
    std::fs::create_dir_all(&output).unwrap();
    // Diagnostic viewport and work bounds, not source layout or timing values.
    let [width, height] = [1280, 960];
    let plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (width, height).into(),
                ..default()
            }),
            exit_condition: ExitCondition::DontExit,
            ..default()
        })
        .set(AssetPlugin {
            file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
            ..default()
        })
        .set(RenderPlugin {
            synchronous_pipeline_compilation: true,
            render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                backends: Some(Backends::VULKAN),
                ..default()
            })),
            ..default()
        })
        .disable::<WinitPlugin>()
        .disable::<bevy::audio::AudioPlugin>();
    let mut app = crate::launch_gui::app_with_plugins(plugins, |_| {});
    app.init_asset::<AudioSource>();
    app.world_mut()
        .resource_mut::<peacecraft3::campaign_startup::CampaignSeed>()
        .requested = Some(1);
    app.insert_resource(crate::launch_gui::Launcher::test_source(
        install.clone(),
        expansion,
    ))
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f64(1.0 / 64.0),
    ))
    .init_resource::<Capture>();
    let mut image = Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(Target(target)).add_systems(
        PostUpdate,
        offscreen
            .before(UiSystems::Layout)
            .before(bevy::camera::CameraUpdateSystems),
    );
    app.finish();
    app.cleanup();
    step(&mut app);
    let mut text = app.world_mut().query::<(Entity, &Text)>();
    let mut button = text
        .iter(app.world())
        .find(|(_, t)| t.0 == "Main menu")
        .expect("launcher setup action that starts the intro")
        .0;
    while !app.world().entity(button).contains::<Button>() {
        button = app
            .world()
            .entity(button)
            .get::<ChildOf>()
            .unwrap()
            .parent();
    }
    activate(&mut app, button);
    escape(&mut app, ButtonState::Pressed);
    step(&mut app);
    escape(&mut app, ButtonState::Released);
    // The game main menu follows the intro; the launcher is only setup.
    screen(&mut app, crate::menu_screens::Screen::Main);
    capture(&mut app, &output.join("main-menu.png"));
    let button = frame_button(&mut app, "SinglePlayerButton");
    activate(&mut app, button);
    screen(&mut app, crate::menu_screens::Screen::SinglePlayer);
    let button = frame_button(&mut app, "CampaignButton");
    activate(&mut app, button);
    screen(&mut app, crate::menu_screens::Screen::Campaign);
    let chapters = app.world().resource::<crate::native_campaign::Chapters>();
    assert!(chapters.error.is_none(), "{:?}", chapters.error);
    let (campaign, mission) = chapters
        .campaigns
        .iter()
        .enumerate()
        .find_map(|(c, entry)| {
            entry
                .missions
                .iter()
                .position(|m| m.map.eq_ignore_ascii_case(&map))
                .map(|m| (c, m))
        })
        .expect("map in selected source index");
    assert!(
        chapters.campaigns[campaign].default_open,
        "campaign must be source-unlocked"
    );
    until(&mut app, "mounted campaign buttons", |w| {
        w.query::<&crate::native_campaign::CampaignButton>()
            .iter(w)
            .any(|b| b.0 == campaign)
    });
    let button = app
        .world_mut()
        .query::<(Entity, &crate::native_campaign::CampaignButton)>()
        .iter(app.world())
        .find(|(_, b)| b.0 == campaign)
        .unwrap()
        .0;
    activate(&mut app, button);
    until(&mut app, "chapter panel", |w| {
        w.resource::<crate::native_campaign::CampaignMenuState>()
            .selected_campaign
            == Some(campaign)
    });
    assert!(
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .is_none()
    );
    capture(&mut app, &output.join("chapter-selection.png"));
    until(&mut app, "mounted mission buttons", |w| {
        w.query::<&crate::native_campaign::MissionButton>()
            .iter(w)
            .any(|b| b.campaign == campaign && b.mission == mission)
    });
    let button = app
        .world_mut()
        .query::<(Entity, &crate::native_campaign::MissionButton)>()
        .iter(app.world())
        .find(|(_, b)| b.campaign == campaign && b.mission == mission)
        .unwrap()
        .0;
    activate(&mut app, button);
    until(&mut app, "campaign startup", |w| {
        w.resource::<CampaignCamera>().active()
            && w.resource::<MapScriptSession<MapStartupHost>>()
                .context()
                .is_some()
    });
    capture(&mut app, &output.join("campaign-entry.png"));
    let mut controlled = false;
    // Diagnostic input schedule: do not assume when the map enables cinematic skipping.
    for _ in 0..300 {
        escape(&mut app, ButtonState::Pressed);
        step(&mut app);
        escape(&mut app, ButtonState::Released);
        step(&mut app);
        controlled = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .is_some_and(|host| {
                let objects = host.objects();
                objects.presentation().is_some_and(|state| {
                    state.control_enabled == Some(true)
                        && state.interface_visible == Some(true)
                        && state.filter.as_ref().is_none_or(|filter| {
                            filter
                                .sample(objects.timer_game_time())
                                .is_ok_and(|sample| sample.color[3] == 0.)
                        })
                })
            });
        if controlled {
            break;
        }
    }
    assert!(
        controlled,
        "cinematic Escape input must return control within 600 updates"
    );
    capture(&mut app, &output.join("campaign-control.png"));
    item_visibility_roundtrip(&mut app, &output);
    // Exercise the real mounted console, rather than supplying a synthetic parent.
    let selected = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects_mut();
        let unit = objects
            .executed_units()
            .filter(|(_, unit)| {
                unit.shown
                    && unit.vitals.as_ref().is_some_and(|v| v.life > 0.)
                    && objects
                        .player_settings(unit.owner)
                        .is_ok_and(|player| Some(player.number) == objects.local_player_number())
            })
            .max_by_key(|(_, unit)| unit.abilities.as_ref().is_some_and(|a| a.hero.is_some()))
            .map(|(id, _)| id)
            .expect("a live local campaign unit");
        objects.select_units(&[unit], true);
        unit
    };
    capture(&mut app, &output.join("selected-unit.png"));
    let attachments = app
        .world_mut()
        .query::<&crate::hud_render::HudSourceAttachment>()
        .iter(app.world())
        .map(|a| {
            serde_json::json!({
                "frame": a.frame, "member": a.source_member,
                "map": a.receipt_path, "revision": a.binding_revision
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(
        output.join("selected-unit.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "unit": format!("{selected:?}"), "attachments": attachments,
            "scope": "Canonical local selection after the unchanged campaign-control capture"
        }))
        .unwrap(),
    )
    .unwrap();
    // Retain every stage before refusing a known renderer gap.
    let errors = [
        (
            "actors",
            app.world()
                .resource::<crate::campaign_actors::UnitActors>()
                .last_error
                .clone(),
        ),
        (
            "ground",
            app.world()
                .resource::<peacecraft3::map_ground::GroundPublication>()
                .last_error
                .as_ref()
                .map(ToString::to_string),
        ),
        (
            "camera",
            app.world().resource::<CampaignCamera>().last_error.clone(),
        ),
        (
            "HUD",
            app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .last_error
                .clone(),
        ),
        (
            "input",
            app.world()
                .resource::<crate::campaign_input::CampaignInput>()
                .last_error
                .clone(),
        ),
        (
            "unit panel",
            app.world()
                .resource::<crate::campaign_unit_panel::UnitPanel>()
                .last_error
                .clone(),
        ),
        (
            "inventory",
            app.world()
                .resource::<crate::campaign_inventory::CampaignInventory>()
                .last_error
                .clone(),
        ),
        (
            "command card",
            app.world()
                .resource::<crate::campaign_command_card::CommandCard>()
                .last_error
                .clone(),
        ),
        (
            "portrait",
            app.world()
                .resource::<crate::campaign_portrait::CampaignPortrait>()
                .last_error
                .clone(),
        ),
        (
            "minimap",
            app.world()
                .resource::<crate::campaign_minimap::CampaignMinimap>()
                .last_error
                .clone(),
        ),
    ];
    let requests = app
        .world()
        .resource::<peacecraft3::map_requests::MapRequests>();
    let context = requests.admission.shared_context().expect("admitted map");
    assert!(std::sync::Arc::ptr_eq(
        &context,
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .unwrap()
    ));
    let recipe = app
        .world()
        .resource::<peacecraft3::browser_configuration::SelectedMapSources>()
        .campaign_for_receipts(&requests.bindings.as_ref().unwrap().receipts)
        .unwrap();
    let image_sha256 = recipe.image_sha256();
    let color_rules = pcx_play::ally_color::Rules::read(recipe.image()).unwrap();
    let units = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .executed_units()
        .count();
    use sha2::{Digest, Sha256};
    let mut frames = Vec::new();
    let minimap = app
        .world()
        .resource::<crate::campaign_minimap::CampaignMinimap>();
    for name in [
        "main-menu.png",
        "chapter-selection.png",
        "campaign-entry.png",
        "campaign-control.png",
    ] {
        let bytes = std::fs::read(output.join(name)).unwrap();
        frames
            .push(serde_json::json!({"path":name, "sha256_bytes":Sha256::digest(bytes).to_vec()}));
    }
    std::fs::write(output.join("scope.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "install": install, "map": map, "expansion": expansion,
        "image_sha256": image_sha256, "executed_units": units, "frames": frames,
        "minimap": {"member": minimap.member, "shown": minimap.shown},
        "subsystem_errors": errors,
        "scope": "Production app offscreen Vulkan; mounted widget activation and synthetic keyboard messages; no physical input, audio playback or original paired-frame parity claim"
    })).unwrap()).unwrap();
    // Retain the unchanged campaign above; the following mutation is a separate rendering diagnostic.
    let before = {
        use pcx_script::{Host, Value};
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let host = session.host().unwrap();
        let settings = host.objects().startup_settings().unwrap();
        host.native("FogEnable", &[Value::Bool(false)]).unwrap();
        host.native("FogMaskEnable", &[Value::Bool(false)]).unwrap();
        [settings.fog_enabled, settings.fog_mask_enabled]
    };
    until(&mut app, "explicit no-fog minimap diagnostic", |world| {
        let minimap = world.resource::<crate::campaign_minimap::CampaignMinimap>();
        minimap.shown && minimap.member.is_some()
    });
    capture(&mut app, &output.join("minimap-no-fog-diagnostic.png"));
    let minimap = app
        .world()
        .resource::<crate::campaign_minimap::CampaignMinimap>();
    std::fs::write(output.join("minimap-no-fog-diagnostic.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "natural_fog_and_mask": before, "diagnostic_fog_and_mask": [false, false],
        "member": minimap.member, "shown": minimap.shown, "error": minimap.last_error,
        "scope": "Explicit post-checkpoint host-native mutation; image rendering only, no unchanged campaign visibility or native pixel parity claim"
    })).unwrap()).unwrap();
    assert!(minimap.last_error.is_none());
    // Freeze the clock: toggling A -> B -> A must recolor live models and restore exact minimap pixels.
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::ZERO,
    ));
    let original_filter = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .ally_color_filter_state()
        .unwrap();
    let individual = (-256..=256)
        .find(|&n| !color_rules.filters_minimap(n) && !color_rules.filters_units(n))
        .unwrap();
    let filtered = (-256..=256)
        .find(|&n| color_rules.filters_units(n))
        .unwrap();
    let mut snapshots = Vec::new();
    for (state, name) in [
        (individual, "individual"),
        (filtered, "filtered"),
        (individual, "restored"),
    ] {
        use pcx_script::{Host, Value};
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .native("SetAllyColorFilterState", &[Value::Int(state)])
            .unwrap();
        capture(&mut app, &output.join(format!("ally-colors-{name}.png")));
        snapshots.push(color_snapshot(&mut app));
    }
    assert_ne!(
        snapshots[0].0, snapshots[1].0,
        "live model palette must change"
    );
    assert_ne!(
        snapshots[0].1, snapshots[1].1,
        "live minimap markers must change"
    );
    assert_eq!(
        snapshots[0], snapshots[2],
        "restoring the filter must leave no stale models or marker trails"
    );
    {
        use pcx_script::{Host, Value};
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let host = session.host().unwrap();
        host.native("SetAllyColorFilterState", &[Value::Int(original_filter)])
            .unwrap();
        host.native("FogEnable", &[Value::Bool(before[0])]).unwrap();
        host.native("FogMaskEnable", &[Value::Bool(before[1])])
            .unwrap();
    }
    std::fs::write(output.join("ally-colors.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "individual_state": individual, "filtered_state": filtered, "restored_original_state": original_filter,
        "model_palette_changed": true, "minimap_pixels_changed": true, "roundtrip_exact": true,
        "scope": "Explicit post-checkpoint native mutations at a frozen clock, ordinary unit markers; no original pixel parity claim"
    })).unwrap()).unwrap();
    // Actual menu widgets pause/return/quit before a second chapter-button activation.
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f64(1. / 64.),
    ));
    menu_key(&mut app, ButtonState::Pressed);
    step(&mut app);
    menu_key(&mut app, ButtonState::Released);
    step(&mut app);
    assert!(app.world().resource::<CampaignCamera>().active());
    assert!(app.world().resource::<CampaignCamera>().menu_open());
    let paused_clock = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .timer_game_time()
        .unwrap();
    capture(&mut app, &output.join("escape-main.png"));
    assert!(
        app.world()
            .resource::<crate::campaign_escape_menu::EscapeMenu>()
            .last_error
            .is_none(),
        "{:?}",
        app.world()
            .resource::<crate::campaign_escape_menu::EscapeMenu>()
            .last_error
    );
    assert_eq!(
        paused_clock,
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .timer_game_time()
            .unwrap()
    );
    let button = frame_button(&mut app, "ReturnButton");
    activate(&mut app, button);
    until(&mut app, "return to game", |w| {
        !w.resource::<CampaignCamera>().menu_open()
    });
    for _ in 0..4 {
        step(&mut app);
    }
    assert!(
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .timer_game_time()
            .unwrap()
            > paused_clock
    );
    let button = app
        .world_mut()
        .query_filtered::<(Entity, &Name), With<Button>>()
        .iter(app.world())
        .find(|(_, name)| name.as_str() == "UpperButtonBarMenuButton")
        .unwrap()
        .0;
    activate(&mut app, button);
    until(&mut app, "HUD menu button", |w| {
        w.resource::<CampaignCamera>().menu_open()
    });
    let button = frame_button(&mut app, "EndGameButton");
    activate(&mut app, button);
    capture(&mut app, &output.join("escape-end.png"));
    let button = frame_button(&mut app, "ExitButton");
    activate(&mut app, button);
    capture(&mut app, &output.join("escape-confirm-exit.png"));
    let button = frame_button(&mut app, "ConfirmQuitCancelButton");
    activate(&mut app, button);
    step(&mut app);
    assert!(app.world().resource::<Messages<AppExit>>().is_empty());
    let restart_from = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    let button = frame_button(&mut app, "RestartButton");
    activate(&mut app, button);
    until(&mut app, "restart mission widget", |world| {
        world.resource::<CampaignCamera>().active()
            && world
                .resource::<MapScriptSession<MapStartupHost>>()
                .session_witness()
                .is_some_and(|next| !std::sync::Weak::ptr_eq(&next, &restart_from))
    });
    assert!(restart_from.upgrade().is_none());
    assert!(!app.world().resource::<CampaignCamera>().menu_open());
    menu_key(&mut app, ButtonState::Pressed);
    step(&mut app);
    menu_key(&mut app, ButtonState::Released);
    step(&mut app);
    assert!(app.world().resource::<CampaignCamera>().menu_open());
    let button = frame_button(&mut app, "EndGameButton");
    activate(&mut app, button);
    step(&mut app);
    let first_session = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    let first_camera = app.world().resource::<CampaignCamera>().entity.unwrap();
    let first_console = app
        .world()
        .resource::<crate::hud_render::HudMount>()
        .root
        .unwrap();
    let button = frame_button(&mut app, "QuitButton");
    activate(&mut app, button);
    until(&mut app, "menu mission exit", |world| {
        world
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .is_none()
            && !world.resource::<CampaignCamera>().active()
    });
    capture(&mut app, &output.join("campaign-exit.png"));
    assert!(app.world().get_entity(first_camera).is_err());
    assert!(app.world().get_entity(first_console).is_err());
    assert!(first_session.upgrade().is_none());
    let button = app
        .world_mut()
        .query::<(Entity, &crate::native_campaign::MissionButton)>()
        .iter(app.world())
        .find(|(_, b)| b.campaign == campaign && b.mission == mission)
        .expect("chapter button after exit")
        .0;
    activate(&mut app, button);
    until(&mut app, "replacement campaign", |world| {
        world.resource::<CampaignCamera>().active()
            && world
                .resource::<MapScriptSession<MapStartupHost>>()
                .session_witness()
                .is_some_and(|next| !std::sync::Weak::ptr_eq(&next, &first_session))
    });
    capture(&mut app, &output.join("campaign-restarted.png"));
    assert!(app.world().get_entity(first_console).is_err());
    std::fs::write(output.join("restart.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "old_session_retired": true, "old_camera_retired": true,
        "old_console_retired": true, "new_session_published": true,
        "menu_clock_paused_and_resumed": true,
        "restart_widget_replaced_session": true,
        "scope": "Source menu key and HUD Menu, Return, End Game, exit cancellation, Restart Mission and Quit Mission; subsequent chapter launch. Native quit/score timing parity remains unqualified."
    })).unwrap()).unwrap();
    assert!(
        errors.iter().all(|(_, error)| error.is_none()),
        "campaign rendering: {errors:?}"
    );
}
