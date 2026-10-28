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

/// Virtual time per fixture update.
const STEP_SECONDS: f64 = 1.0 / 64.0;

/// Optional game time, in seconds, from an original-client schedule supplied by
/// the comparison runner (e.g. `PCX_PAIRED_SKIP_AT`, `PCX_PAIRED_CAPTURE_AT`).
fn paired_time(name: &str) -> Option<f64> {
    let value = std::env::var(name).ok()?;
    let seconds = value
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("{name} must be seconds, got {value:?}"));
    assert!(seconds.is_finite() && seconds >= 0., "{name} out of range");
    Some(seconds)
}

/// Steps until the script clock reaches `seconds`, bounded by twice the updates
/// that time needs at [`STEP_SECONDS`].
fn advance_game_time(app: &mut App, seconds: f64) {
    let now = |app: &mut App| {
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .and_then(|host| host.objects().timer_game_time())
    };
    for _ in 0..(2.0 * seconds / STEP_SECONDS).ceil() as usize {
        if now(app).is_some_and(|t| t >= seconds) {
            return;
        }
        step(app);
    }
    panic!("game time {:?} did not reach {seconds}s", now(app));
}

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

fn capture(app: &mut App, output: &Path) -> Image {
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
    image
        .clone()
        .try_into_dynamic()
        .unwrap()
        .save(output)
        .unwrap();
    image
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
    check_mounted_campaign(false, false);
}

#[test]
#[ignore = "requires explicit original install/edition/map, Vulkan and a new PCX_CAMPAIGN_CAPTURE_DIR"]
fn mounted_campaign_hud_and_research_states() {
    check_mounted_campaign(true, false);
}

#[test]
#[ignore = "requires explicit original install/edition/map, Vulkan and a new PCX_CAMPAIGN_CAPTURE_DIR"]
fn mounted_campaign_rendering_state() {
    check_mounted_campaign(false, true);
}

fn check_mounted_campaign(hud_only: bool, render_only: bool) {
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
        Duration::from_secs_f64(STEP_SECONDS),
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
    if let Some(at) = paired_time("PCX_PAIRED_SKIP_AT") {
        advance_game_time(&mut app, at);
    }
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
    if let Some(at) = paired_time("PCX_PAIRED_CAPTURE_AT") {
        advance_game_time(&mut app, at);
    }
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
    let selected_image = capture(&mut app, &output.join("selected-unit.png"));
    if render_only {
        let scene_image = app.world_mut().query_filtered::<&RenderTarget, With<peacecraft3::campaign_camera::CampaignSceneCamera>>()
            .single(app.world()).unwrap().as_image().unwrap().clone();
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&scene_image)
            .unwrap()
            .texture_descriptor
            .usage |= TextureUsages::COPY_SRC;
        let output_image = app.world().resource::<Target>().0.clone();
        app.world_mut().resource_mut::<Target>().0 = scene_image;
        let world_image = capture(&mut app, &output.join("world-target.png"));
        app.world_mut().resource_mut::<Target>().0 = output_image;
        let nodes = app
            .world_mut()
            .query::<(
                Entity,
                &Name,
                Option<&Node>,
                Option<&ComputedNode>,
                Option<&InheritedVisibility>,
                Option<&UiTargetCamera>,
            )>()
            .iter(app.world())
            .filter(|(_, name, ..)| name.as_str() == "World composition image")
            .map(|row| format!("{row:?}"))
            .collect::<Vec<_>>();
        std::fs::write(output.join("composition-nodes.txt"), nodes.join("\n")).unwrap();
        let composition = app
            .world_mut()
            .query::<(
                Entity,
                Option<&Name>,
                &Camera,
                &RenderTarget,
                Option<&bevy::render::view::Msaa>,
                Option<&bevy::camera::CompositingSpace>,
            )>()
            .iter(app.world())
            .map(|row| format!("{row:?}"))
            .collect::<Vec<_>>();
        std::fs::write(
            output.join("camera-composition.txt"),
            composition.join("\n"),
        )
        .unwrap();
        let cameras = app.world_mut().query_filtered::<(&Transform, &Projection, &Camera), With<peacecraft3::campaign_camera::CampaignSceneCamera>>()
            .iter(app.world()).map(|(transform, projection, camera)| format!("{transform:?} {projection:?} {camera:?}")).collect::<Vec<_>>();
        let actors = app.world().resource::<crate::campaign_actors::UnitActors>();
        let destructables = actors.rendered_destructables();
        let actor_error = actors.last_error.clone();
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects();
        let now = objects.timer_game_time().unwrap();
        let models = objects.executed_units().filter_map(|(id, unit)| {
            let animation = unit.animation.as_ref()?;
            Some(serde_json::json!({"unit": format!("{id:?}"), "definition": format!("{:?}", unit.definition.rawcode),
                "model": animation.model.member, "origin": format!("{:?}", animation.model.origin),
                "sequence": animation.sequence, "sample": format!("{:?}", animation.sample(now)),
                "shown": unit.shown}))
        }).collect::<Vec<_>>();
        std::fs::write(output.join("render-state.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "clock": now, "day_fraction": objects.day_fraction(), "cameras": cameras, "models": models,
            "rendered_destructables": destructables, "actor_error": actor_error,
            "scope": "Diagnostic source scene; effective graphics settings and stock camera/time pairing remain unqualified"
        })).unwrap()).unwrap();
        let viewport = app
            .world_mut()
            .query_filtered::<&Camera, With<peacecraft3::campaign_camera::CampaignSceneCamera>>()
            .single(app.world())
            .unwrap()
            .viewport
            .clone()
            .unwrap();
        let pixel = viewport.physical_position + viewport.physical_size / 2;
        let selected_image = selected_image.try_into_dynamic().unwrap().to_rgba8();
        let world_image = world_image.try_into_dynamic().unwrap().to_rgba8();
        let expected = world_image.get_pixel(pixel.x, pixel.y);
        let actual = selected_image.get_pixel(pixel.x, pixel.y);
        assert!(
            expected.0[..3].iter().any(|v| *v != 0),
            "fixture needs visible ground at the viewport centre"
        );
        for channel in 0..3 {
            assert!(
                actual[channel].abs_diff(expected[channel]) <= 2,
                "world was lost beneath HUD: {actual:?} != {expected:?}"
            );
        }
        assert!(
            destructables > 0,
            "fixture needs visible canonical destructables: {actor_error:?}"
        );
        let splat_error = app
            .world()
            .resource::<crate::campaign_uber_splats::CampaignUberSplats>()
            .last_error
            .clone();
        assert!(splat_error.is_none(), "ubersplat failed: {splat_error:?}");
        let splats = app
            .world_mut()
            .query_filtered::<(), With<crate::campaign_uber_splats::UberSplat>>()
            .iter(app.world())
            .count();
        assert!(
            splats > 0,
            "fixture needs source ubersplats under buildings"
        );
        return;
    }
    if hud_only {
        let button = app
            .world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, name)| name.as_str() == "UpperButtonBarQuestsButton")
            .expect("authored Quests button")
            .0;
        let normal = app.world().get::<ImageNode>(button).unwrap().rect;
        #[derive(Resource)]
        struct ButtonVisualDiagnostic {
            frame: &'static str,
            pressed: bool,
        }
        app.add_systems(
            Update,
            (|mut commands: Commands,
              diagnostic: Option<Res<ButtonVisualDiagnostic>>,
              buttons: Query<(Entity, &Name)>| {
                let Some(diagnostic) = diagnostic else {
                    return;
                };
                for (button, name) in &buttons {
                    if name.as_str() != diagnostic.frame {
                        continue;
                    }
                    commands
                        .entity(button)
                        .insert(bevy::picking::hover::Hovered(true));
                    if diagnostic.pressed {
                        commands.entity(button).insert(bevy::ui::Pressed);
                    }
                }
            })
            .before(crate::campaign_hud::upper_buttons::CampaignHudSet),
        );
        app.insert_resource(ButtonVisualDiagnostic {
            frame: "UpperButtonBarQuestsButton",
            pressed: false,
        });
        capture(&mut app, &output.join("upper-button-hover.png"));
        app.world_mut()
            .entity_mut(button)
            .insert(bevy::ui::InteractionDisabled);
        capture(&mut app, &output.join("upper-button-disabled.png"));
        assert_ne!(app.world().get::<ImageNode>(button).unwrap().rect, normal);
        app.world_mut()
            .entity_mut(button)
            .remove::<bevy::ui::InteractionDisabled>();

        app.world_mut().remove_resource::<ButtonVisualDiagnostic>();
        step(&mut app);

        app.world_mut().entity_mut(button).insert(bevy::ui::Pressed);
        capture(&mut app, &output.join("upper-button-pressed.png"));
        assert_ne!(app.world().get::<ImageNode>(button).unwrap().rect, normal);
        assert!(
            !app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .quest_log_open,
            "a visual-state diagnostic must not activate the quest log"
        );
        app.world_mut()
            .entity_mut(button)
            .remove::<bevy::ui::Pressed>();
        step(&mut app);
        assert_eq!(app.world().get::<ImageNode>(button).unwrap().rect, normal);
        app.world_mut()
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities
            .push(button);
        step(&mut app);
        assert!(
            app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .quest_log_open
        );
        let pane = app
            .world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, name)| name.as_str() == "QuestDisplayBackdrop")
            .expect("source details pane")
            .0;
        let parts = app
            .world()
            .get::<Children>(pane)
            .expect("details artwork children");
        assert!(
            parts
                .iter()
                .any(|part| app.world().get::<ImageNode>(part).is_some()),
            "details pane mounts its border"
        );
        assert!(
            parts.iter().any(|part| app
                .world()
                .get::<MaterialNode<crate::glue_menu::BackdropImage>>(part)
                .is_some()),
            "details pane mounts its tiled background"
        );
        capture(&mut app, &output.join("quest-log.png"));
        app.insert_resource(ButtonVisualDiagnostic {
            frame: "QuestAcceptButton",
            pressed: false,
        });
        capture(&mut app, &output.join("quest-done-hover.png"));
        for frame in ["QuestItemListContainer", "QuestDisplay"] {
            let area = app
                .world_mut()
                .query::<(Entity, &Name)>()
                .iter(app.world())
                .find(|(_, name)| name.as_str() == frame)
                .expect("source content frame")
                .0;
            assert!(
                app.world()
                    .get::<bevy::ui_widgets::ScrollArea>(area)
                    .is_some(),
                "{frame} must keep overflowing text reachable"
            );
        }

        app.world_mut()
            .resource_mut::<ButtonVisualDiagnostic>()
            .pressed = true;
        capture(&mut app, &output.join("quest-done-pressed.png"));
        app.world_mut().remove_resource::<ButtonVisualDiagnostic>();
        let close = app
            .world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, name)| name.as_str() == "QuestAcceptButton")
            .expect("current authored Done button")
            .0;
        app.world_mut()
            .entity_mut(close)
            .remove::<bevy::ui::Pressed>();
        app.world_mut()
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities
            .push(close);
        step(&mut app);
        assert!(
            !app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .quest_log_open
        );

        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects();
            assert_eq!(
                objects.current_unit_order(selected).unwrap(),
                0,
                "HUD diagnostic requires an idle selected hero"
            );
            assert!(
                !objects
                    .executed_unit(selected)
                    .unwrap()
                    .activity
                    .as_ref()
                    .unwrap()
                    .held()
            );
        }
        let highlighted = app
            .world_mut()
            .query::<(&crate::campaign_command_card::ButtonOrder, &Children)>()
            .iter(app.world())
            .filter(|(_, children)| {
                children.first().is_some_and(|&child| {
                    app.world()
                        .get::<MaterialNode<crate::glue_menu::AdditiveImage>>(child)
                        .is_some()
                })
            })
            .map(|(button, _)| button.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            highlighted,
            vec![crate::campaign_command_card::Kind::Standard(
                pcx_play::standard_command::Kind::Stop
            )]
        );
    }
    let skill_menu = app
        .world_mut()
        .query::<(Entity, &crate::campaign_command_card::ButtonOrder)>()
        .iter(app.world())
        .find(|(_, button)| button.kind == crate::campaign_command_card::Kind::SkillMenu)
        .map(|(entity, _)| entity);
    if let Some(skill_menu) = skill_menu {
        activate(&mut app, skill_menu);
        capture(&mut app, &output.join("skill-menu.png"));
        assert!(
            app.world()
                .resource::<crate::campaign_command_card::CommandCard>()
                .last_error
                .is_none()
        );
        let rendered = app
            .world_mut()
            .query::<(
                &crate::campaign_command_card::ButtonOrder,
                Has<InteractionDisabled>,
            )>()
            .iter(app.world())
            .filter_map(|(button, disabled)| {
                if let crate::campaign_command_card::Kind::Learn(alias) = button.kind {
                    Some((alias.to_be_bytes(), disabled))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let image_modes = app
            .world_mut()
            .query::<(Entity, &ImageNode)>()
            .iter(app.world())
            .filter_map(|(entity, image)| {
                let mut at = entity;
                loop {
                    if let Some(button) = app
                        .world()
                        .get::<crate::campaign_command_card::ButtonOrder>(at)
                    {
                        return matches!(button.kind, crate::campaign_command_card::Kind::Learn(_))
                            .then(|| image.image_mode.clone());
                    }
                    at = app.world().get::<ChildOf>(at)?.parent();
                }
            })
            .collect::<Vec<_>>();
        assert!(
            !image_modes.is_empty(),
            "research image tree must be mounted"
        );
        assert!(
            image_modes
                .iter()
                .all(|mode| *mode == bevy::ui::widget::NodeImageMode::Stretch),
            "research icons and badge backgrounds must fill their authored frames"
        );
        let ranks = app
            .world_mut()
            .query::<(Entity, &Text)>()
            .iter(app.world())
            .filter_map(|(entity, text)| {
                let mut at = entity;
                loop {
                    if let Some(button) = app
                        .world()
                        .get::<crate::campaign_command_card::ButtonOrder>(at)
                    {
                        return if let crate::campaign_command_card::Kind::Learn(alias) = button.kind
                        {
                            Some((alias.to_be_bytes(), text.0.clone()))
                        } else {
                            None
                        };
                    }
                    at = app.world().get::<ChildOf>(at)?.parent();
                }
            })
            .collect::<Vec<_>>();
        let skills = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .learnable_skills(selected)
            .unwrap();
        for skill in skills {
            let shown = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects()
                .research_icon(selected, skill.alias)
                .unwrap()
                .unwrap()
                .rank;
            assert_eq!(
                ranks
                    .iter()
                    .filter(|(alias, _)| *alias == skill.alias)
                    .map(|(_, text)| text.as_str())
                    .collect::<Vec<_>>(),
                vec![shown.to_string().as_str()],
                "research number {:?}",
                skill.alias
            );
            assert_eq!(
                rendered
                    .iter()
                    .find(|(alias, _)| *alias == skill.alias)
                    .map(|(_, disabled)| *disabled),
                Some(skill.block.is_some()),
                "research availability {:?}",
                skill.alias
            );
        }
        let cancel = app
            .world_mut()
            .query::<(Entity, &crate::campaign_command_card::ButtonOrder)>()
            .iter(app.world())
            .find(|(_, button)| button.kind == crate::campaign_command_card::Kind::SkillCancel)
            .map(|(entity, _)| entity)
            .expect("source research cancel");
        activate(&mut app, cancel);
        step(&mut app);
    }
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
    if hud_only {
        assert!(
            app.world()
                .resource::<peacecraft3::map_ground::GroundPublication>()
                .last_error
                .is_none(),
            "ground projection failed"
        );
        assert!(
            app.world()
                .resource::<peacecraft3::map_ground::GroundPublication>()
                .shadow_error
                .is_none(),
            "source shadow projection failed"
        );
        for error in [
            &app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .last_error,
            &app.world()
                .resource::<crate::campaign_input::CampaignInput>()
                .last_error,
            &app.world()
                .resource::<crate::campaign_unit_panel::UnitPanel>()
                .last_error,
            &app.world()
                .resource::<crate::campaign_command_card::CommandCard>()
                .last_error,
        ] {
            assert!(error.is_none(), "HUD projection: {error:?}");
        }
        assert!(
            skill_menu.is_some(),
            "diagnostic chapter needs a research menu"
        );
        return;
    }
    item_visibility_roundtrip(&mut app, &output);
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
