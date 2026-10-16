#![forbid(unsafe_code)]
use super::slider as menu_slider;
use crate::button_actions;
use bevy::ecs::system::RunSystemOnce;
use bevy::{
    app::{HierarchyPropagatePlugin, PropagateSet},
    camera::{
        ComputedCameraValues, NormalizedRenderTarget, RenderTarget, RenderTargetInfo, Viewport,
    },
    input::{ButtonState, InputPlugin, mouse::MouseButtonInput},
    picking::{
        DefaultPickingPlugins,
        events::{PointerClick, PointerPress, PointerRelease},
        hover::Hovered,
        pointer::{
            Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
        },
    },
    prelude::*,
    ui::{
        ComputedUiRenderTargetInfo, ComputedUiTargetCamera, UiGlobalTransform, UiStack,
        picking_backend::UiPickingPlugin,
    },
    ui_widgets::{Button, ButtonPlugin},
    window::{CursorMoved, PrimaryWindow, WindowEvent, WindowRef},
};

#[derive(Resource, Default, Debug)]
struct Trace {
    press_positions: Vec<(Entity, Vec2)>,
    presses: Vec<Entity>,
    clicks: Vec<Entity>,
    releases: Vec<Entity>,
}
struct Fixture {
    app: App,
    window: Entity,
    button: Entity,
    scale: f32,
}
#[derive(Debug)]
struct Outcome {
    presses: usize,
    clicks: usize,
    releases: usize,
    actions: usize,
    pressed: bool,
}

fn fixture(scale: f32) -> Fixture {
    fixture_with_mode(scale, true)
}
fn fixture_with_mode(scale: f32, ordered: bool) -> Fixture {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        InputPlugin,
        bevy::input_focus::InputFocusPlugin,
        bevy::input_focus::InputDispatchPlugin,
        bevy::input_focus::pointer_focus::PointerFocusPlugin,
        DefaultPickingPlugins,
        ButtonPlugin,
        UiPickingPlugin,
        HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate),
        HierarchyPropagatePlugin::<ComputedUiRenderTargetInfo>::new(PostUpdate),
    ));
    app.add_message::<WindowEvent>()
        .init_resource::<UiStack>()
        .init_resource::<UiScale>()
        .init_resource::<Trace>();
    app.add_systems(
        PostUpdate,
        bevy::ui::update::propagate_ui_target_cameras
            .before(PropagateSet::<ComputedUiTargetCamera>::default())
            .before(PropagateSet::<ComputedUiRenderTargetInfo>::default()),
    );
    button_actions::install(&mut app);
    if ordered {
        app.insert_resource(bevy::picking::ordered::OrderedPointerInput);
    }
    app.add_observer(|event: On<PointerPress>, mut trace: ResMut<Trace>| {
        trace.presses.push(event.entity);
        trace
            .press_positions
            .push((event.entity, event.pointer.position));
    });
    app.add_observer(|event: On<PointerClick>, mut trace: ResMut<Trace>| {
        trace.clicks.push(event.entity)
    });
    app.add_observer(|event: On<PointerRelease>, mut trace: ResMut<Trace>| {
        trace.releases.push(event.entity)
    });
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let camera = app
        .world_mut()
        .spawn((
            Camera {
                viewport: Some(Viewport {
                    physical_position: UVec2::new(37, 23),
                    physical_size: UVec2::new(400, 300),
                    ..default()
                }),
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(600, 450),
                        scale_factor: scale,
                    }),
                    ..default()
                },
                ..default()
            },
            RenderTarget::Window(WindowRef::Entity(window)),
        ))
        .id();
    let button = app
        .world_mut()
        .spawn((
            Node::default(),
            Button,
            bevy::input_focus::tab_navigation::TabIndex(0),
            Hovered::default(),
            ComputedNode {
                size: Vec2::new(120.0, 40.0),
                inverse_scale_factor: 1.0 / scale,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(180.0, 90.0)),
            InheritedVisibility::VISIBLE,
            UiTargetCamera(camera),
        ))
        .id();
    {
        let mut stack = app.world_mut().resource_mut::<UiStack>();
        stack.uinodes = vec![button];
        stack.partition = std::iter::once(0..1).collect();
    }
    // Startup creates normal mouse pointer; PostUpdate propagates the synthetic UI camera/layout.
    app.update();
    app.update();
    Fixture {
        app,
        window,
        button,
        scale,
    }
}
fn inside(f: &Fixture) -> Vec2 {
    (Vec2::new(37.0, 23.0) + Vec2::new(180.0, 90.0)) / f.scale
}
fn outside(f: &Fixture) -> Vec2 {
    Vec2::new(45.0, 30.0) / f.scale
}
fn move_window(f: &mut Fixture, p: Vec2) {
    f.app
        .world_mut()
        .write_message(WindowEvent::CursorMoved(CursorMoved {
            window: f.window,
            position: p,
            delta: None,
        }));
}
fn button_window(f: &mut Fixture, state: ButtonState) {
    f.app
        .world_mut()
        .write_message(WindowEvent::MouseButtonInput(MouseButtonInput {
            window: f.window,
            button: MouseButton::Left,
            state,
        }));
}
fn direct(f: &mut Fixture, p: Vec2, action: PointerAction) {
    f.app.world_mut().write_message(PointerInput::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(f.window).normalize(None).unwrap(),
            ),
            position: p,
        },
        action,
    ));
}
fn outcome(f: &Fixture) -> Outcome {
    let t = f.app.world().resource::<Trace>();
    Outcome {
        presses: t.presses.iter().filter(|e| **e == f.button).count(),
        clicks: t.clicks.iter().filter(|e| **e == f.button).count(),
        releases: t.releases.iter().filter(|e| **e == f.button).count(),
        actions: f
            .app
            .world()
            .resource::<button_actions::ButtonActions>()
            .entities
            .iter()
            .filter(|e| **e == f.button)
            .count(),
        pressed: f
            .app
            .world()
            .entity(f.button)
            .contains::<bevy::ui::Pressed>(),
    }
}
fn position(f: &mut Fixture) -> Option<Vec2> {
    let mut q = f
        .app
        .world_mut()
        .query_filtered::<&PointerLocation, With<PointerId>>();
    q.single(f.app.world())
        .unwrap()
        .location
        .as_ref()
        .map(|l| l.position)
}
fn click_window(f: &mut Fixture) {
    button_window(f, ButtonState::Pressed);
    button_window(f, ButtonState::Released);
}
fn add_second_button(f: &mut Fixture) -> Entity {
    let camera = f
        .app
        .world()
        .get::<ComputedUiTargetCamera>(f.button)
        .unwrap()
        .get()
        .unwrap();
    let b = f
        .app
        .world_mut()
        .spawn((
            Node::default(),
            Button,
            bevy::input_focus::tab_navigation::TabIndex(0),
            Hovered::default(),
            ComputedNode {
                size: Vec2::new(120.0, 40.0),
                inverse_scale_factor: 1.0 / f.scale,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(180.0, 190.0)),
            InheritedVisibility::VISIBLE,
            UiTargetCamera(camera),
        ))
        .id();
    {
        let mut stack = f.app.world_mut().resource_mut::<UiStack>();
        stack.uinodes.push(b);
        stack.partition.push(1..2);
    }
    f.app.update();
    b
}
fn at_b(f: &Fixture) -> Vec2 {
    (Vec2::new(37.0, 23.0) + Vec2::new(180.0, 190.0)) / f.scale
}

#[test]
fn fresh_same_frame_window_and_direct_clicks() {
    for scale in [1.0, 2.0] {
        for window_driver in [true, false] {
            let mut f = fixture(scale);
            let p = inside(&f);
            if window_driver {
                move_window(&mut f, p);
                click_window(&mut f);
            } else {
                direct(&mut f, p, PointerAction::Move { delta: p });
                direct(&mut f, p, PointerAction::Press(PointerButton::Primary));
                direct(&mut f, p, PointerAction::Release(PointerButton::Primary));
            }
            f.app.update();
            assert_eq!(position(&mut f), Some(p));
            let o = outcome(&f);
            println!("fresh scale={scale} window={window_driver} {o:?}");
            assert_eq!((o.presses, o.clicks, o.releases, o.actions), (1, 1, 1, 1));
            assert!(!o.pressed);
        }
    }
}
#[test]
fn event_a_click_with_final_b_never_activates_b() {
    for scale in [1.0, 2.0] {
        let mut f = fixture(scale);
        let b = add_second_button(&mut f);
        let pb = at_b(&f);
        move_window(&mut f, pb);
        f.app.update();
        let pa = inside(&f);
        move_window(&mut f, pa);
        click_window(&mut f);
        move_window(&mut f, pb);
        f.app.update();
        let trace = f.app.world().resource::<Trace>();
        assert_eq!(trace.presses, [f.button]);
        assert_eq!(trace.clicks, [f.button]);
        assert_eq!(
            f.app
                .world()
                .resource::<button_actions::ButtonActions>()
                .entities,
            [f.button]
        );
        assert!(!f.app.world().entity(b).contains::<bevy::ui::Pressed>());
        println!(
            "false-B control scale={scale} A={:?} B={b:?} events={trace:?}",
            f.button
        );
    }
}
#[test]
fn release_elsewhere_and_cancel_clear_press_without_action() {
    for cancel in [false, true] {
        let mut f = fixture(2.0);
        let p = inside(&f);
        move_window(&mut f, p);
        button_window(&mut f, ButtonState::Pressed);
        f.app.update();
        assert!(outcome(&f).pressed);
        let p = outside(&f);
        if cancel {
            direct(&mut f, p, PointerAction::Cancel);
        } else {
            move_window(&mut f, p);
            button_window(&mut f, ButtonState::Released);
        }
        f.app.update();
        let o = outcome(&f);
        println!("outside cleanup cancel={cancel} {o:?}");
        assert_eq!(o.actions, 0);
        assert!(!o.pressed);
        let state = f
            .app
            .world()
            .resource::<bevy::picking::events::PointerState>()
            .get(PointerId::Mouse, PointerButton::Primary)
            .unwrap();
        assert!(state.pressing.is_empty());
    }
}
#[test]
fn observer_despawn_between_inputs_removes_stale_target() {
    let mut f = fixture(1.0);
    let b = add_second_button(&mut f);
    let a = f.button;
    f.app
        .add_observer(move |event: On<PointerPress>, mut commands: Commands| {
            if event.entity == a {
                commands.entity(b).despawn();
            }
        });
    let pa = inside(&f);
    let pb = at_b(&f);
    move_window(&mut f, pa);
    click_window(&mut f);
    move_window(&mut f, pb);
    click_window(&mut f);
    f.app.update();
    assert!(f.app.world().get_entity(b).is_err());
    assert_eq!(
        f.app
            .world()
            .resource::<button_actions::ButtonActions>()
            .entities,
        [a]
    );
}

#[test]
fn observer_components_capture_and_final_position_are_consistent() {
    use bevy::picking::{
        events::PointerState,
        hover::{HoverMap, PickingInteraction, PointerCaptureMap},
        pointer::{PointerInteraction, PointerMap, PointerPressState},
    };
    let mut f = fixture(2.0);
    let b = add_second_button(&mut f);
    let a = f.button;
    let pa = inside(&f);
    let pb = at_b(&f);
    f.app.add_observer(
        move |event: On<PointerPress>,
              map: Res<PointerMap>,
              locations: Query<(&PointerLocation, &PointerPressState, &PointerInteraction)>,
              hover: Res<HoverMap>,
              state: Res<PointerState>,
              interactions: Query<&PickingInteraction>,
              mut capture: ResMut<PointerCaptureMap>| {
            if event.entity == a {
                let pointer = map.get_entity(event.pointer.id).unwrap();
                let (location, pressed, interaction) = locations.get(pointer).unwrap();
                assert_eq!(location.location.as_ref().unwrap().position, pa);
                assert!(pressed.is_primary_pressed());
                assert!(interaction.iter().any(|(entity, _)| *entity == a));
                assert!(hover.get(&event.pointer.id).unwrap().contains_key(&a));
                assert_eq!(*interactions.get(a).unwrap(), PickingInteraction::Pressed);
                assert!(
                    state
                        .get(event.pointer.id, PointerButton::Primary)
                        .unwrap()
                        .pressing
                        .contains_key(&a)
                );
                capture.capture(event.pointer.id, a, event.hit.clone());
            }
        },
    );
    move_window(&mut f, pa);
    button_window(&mut f, ButtonState::Pressed);
    move_window(&mut f, pb);
    button_window(&mut f, ButtonState::Released);
    f.app.update();
    assert_eq!(
        f.app
            .world()
            .resource::<button_actions::ButtonActions>()
            .entities,
        [a]
    );
    assert_eq!(position(&mut f), Some(pb));
    assert!(
        !f.app
            .world()
            .resource::<PointerCaptureMap>()
            .is_captured(&PointerId::Mouse)
    );
    assert!(!f.app.world().entity(a).contains::<bevy::ui::Pressed>());
    assert!(f.app.world().get::<Hovered>(b).unwrap().get());
    assert_eq!(
        f.app
            .world()
            .resource::<bevy::input_focus::InputFocus>()
            .get(),
        Some(a)
    );
}
#[test]
fn observer_pickable_changes_affect_same_batch_release() {
    let mut f = fixture(1.0);
    let a = f.button;
    f.app
        .add_observer(move |event: On<PointerPress>, mut commands: Commands| {
            if event.entity == a {
                commands.entity(a).insert(bevy::picking::Pickable::IGNORE);
            }
        });
    let p = inside(&f);
    move_window(&mut f, p);
    click_window(&mut f);
    f.app.update();
    assert!(
        f.app
            .world()
            .resource::<button_actions::ButtonActions>()
            .entities
            .is_empty()
    );
    assert!(!f.app.world().entity(a).contains::<bevy::ui::Pressed>());
}
#[test]
fn current_app_disabled_midpress_cleanup_preserves_stock_and_ordered_paths() {
    for ordered in [false, true] {
        let mut f = fixture_with_mode(1.0, ordered);
        let a = f.button;
        f.app
            .add_observer(move |event: On<PointerPress>, mut commands: Commands| {
                if event.entity == a {
                    commands.entity(a).insert(bevy::ui::InteractionDisabled);
                }
            });
        let p = inside(&f);
        move_window(&mut f, p);
        f.app.update();
        button_window(&mut f, ButtonState::Pressed);
        f.app.update();
        button_window(&mut f, ButtonState::Released);
        f.app.update();
        let o = outcome(&f);
        println!("EXISTING disabled-midpress ordered={ordered} {o:?}");
        assert_eq!(o.actions, 0);
        assert!(!o.pressed, "current app must clean disabled widget Pressed");
        assert!(
            f.app
                .world()
                .resource::<bevy::picking::events::PointerState>()
                .get(PointerId::Mouse, PointerButton::Primary)
                .unwrap()
                .pressing
                .is_empty()
        );
    }
}

fn touch(f: &mut Fixture, phase: bevy::input::touch::TouchPhase, p: Vec2, id: u64) {
    let event = bevy::input::touch::TouchInput {
        phase,
        position: p,
        window: f.window,
        force: None,
        id,
    };
    f.app.world_mut().write_message(event);
    f.app
        .world_mut()
        .write_message(WindowEvent::TouchInput(event));
}
#[test]
fn touch_end_without_current_hover_and_cancel_do_not_need_previous_frame_target() {
    use bevy::input::touch::TouchPhase;
    for cancel in [false, true] {
        let mut f = fixture(2.0);
        let p = inside(&f);
        touch(&mut f, TouchPhase::Started, p, 17);
        f.app.update();
        {
            let mut q = f
                .app
                .world_mut()
                .query::<(&PointerId, &mut PointerLocation)>();
            for (id, mut location) in q.iter_mut(f.app.world_mut()) {
                if *id == PointerId::Touch(17) {
                    location.location = None;
                }
            }
        }
        touch(
            &mut f,
            if cancel {
                TouchPhase::Canceled
            } else {
                TouchPhase::Ended
            },
            p,
            17,
        );
        f.app.update();
        let o = outcome(&f);
        assert_eq!(o.actions, usize::from(!cancel));
        assert!(!o.pressed);
    }
}
#[test]
fn simultaneous_touch_pointers_keep_targets_and_press_state_separate() {
    use bevy::input::touch::TouchPhase;
    let mut f = fixture(1.0);
    let b = add_second_button(&mut f);
    let pa = inside(&f);
    let pb = at_b(&f);
    touch(&mut f, TouchPhase::Started, pa, 17);
    touch(&mut f, TouchPhase::Started, pb, 18);
    f.app.update();
    touch(&mut f, TouchPhase::Ended, pa, 17);
    touch(&mut f, TouchPhase::Ended, pb, 18);
    f.app.update();
    let actions = &f
        .app
        .world()
        .resource::<button_actions::ButtonActions>()
        .entities;
    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&f.button) && actions.contains(&b));
    assert!(
        !f.app
            .world()
            .entity(f.button)
            .contains::<bevy::ui::Pressed>()
            && !f.app.world().entity(b).contains::<bevy::ui::Pressed>()
    );
}
fn sprite_backend(f: &mut Fixture) -> Entity {
    use bevy::camera::visibility::RenderLayers;
    f.app
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<bevy::image::TextureAtlasLayout>>()
        .add_plugins(bevy::sprite::SpritePickingPlugin);
    let mut q = f.app.world_mut().query_filtered::<Entity, With<Camera>>();
    let camera = q.single(f.app.world()).unwrap();
    f.app.world_mut().entity_mut(camera).insert((
        GlobalTransform::IDENTITY,
        Projection::Orthographic(OrthographicProjection::default_2d()),
        RenderLayers::layer(0),
    ));
    camera
}
fn sprite_at(f: &mut Fixture, camera: Entity, p: Vec2, alpha: u8) -> Entity {
    let camera = f.app.world().get::<Camera>(camera).unwrap();
    let ray = camera
        .viewport_to_world(&GlobalTransform::IDENTITY, p)
        .unwrap();
    let t = -ray.origin.z / ray.direction.z;
    let point = ray.get_point(t);
    let mut image = Image::default();
    image.data.as_mut().unwrap()[3] = alpha;
    let image = f.app.world_mut().resource_mut::<Assets<Image>>().add(image);
    f.app
        .world_mut()
        .spawn((
            Sprite {
                image,
                custom_size: Some(Vec2::splat(0.2)),
                ..default()
            },
            GlobalTransform::from_translation(point),
            bevy::camera::visibility::ViewVisibility::VISIBLE,
            bevy::picking::Pickable::default(),
            Button,
            bevy::input_focus::tab_navigation::TabIndex(1),
        ))
        .id()
}
#[test]
fn stock_ui_sprite_window_priority_and_sprite_pixel_alpha_are_preserved() {
    for transparent in [false, true] {
        let mut f = fixture(1.0);
        let camera = sprite_backend(&mut f);
        let pa = inside(&f);
        let sprite = sprite_at(&mut f, camera, pa, if transparent { 0 } else { 255 });
        // UI's normal half-layer precedence beats Sprite in the same camera.
        move_window(&mut f, pa);
        click_window(&mut f);
        f.app.update();
        assert_eq!(
            f.app
                .world()
                .resource::<button_actions::ButtonActions>()
                .entities,
            [f.button]
        );
        f.app
            .world_mut()
            .run_system_once(button_actions::clear)
            .unwrap();
        f.app
            .world_mut()
            .entity_mut(f.button)
            .insert(bevy::picking::Pickable::IGNORE);
        click_window(&mut f);
        f.app.update();
        let actions = &f
            .app
            .world()
            .resource::<button_actions::ButtonActions>()
            .entities;
        if transparent {
            assert!(actions.is_empty());
        } else {
            assert_eq!(actions.as_slice(), [sprite]);
        }
        // With both content targets ignored, the window remains the final fallback hit.
        f.app
            .world_mut()
            .entity_mut(sprite)
            .insert(bevy::picking::Pickable::IGNORE);
        f.app
            .world_mut()
            .run_system_once(button_actions::clear)
            .unwrap();
        click_window(&mut f);
        f.app.update();
        assert!(
            f.app
                .world()
                .resource::<button_actions::ButtonActions>()
                .entities
                .is_empty()
        );
        let h = f.app.world().resource::<bevy::picking::hover::HoverMap>();
        assert!(h.get(&PointerId::Mouse).unwrap().contains_key(&f.window));
    }
}
#[test]
fn high_camera_sprite_at_a_not_stale_sprite_b_receives_batched_click() {
    let mut f = fixture(2.0);
    let ui_b = add_second_button(&mut f);
    let camera = sprite_backend(&mut f);
    let pa = inside(&f);
    let pb = at_b(&f);
    let a = sprite_at(&mut f, camera, pa, 255);
    let b = sprite_at(&mut f, camera, pb, 255);
    let mut high = f.app.world().get::<Camera>(camera).unwrap().clone();
    high.order = 1;
    f.app.world_mut().spawn((
        high,
        RenderTarget::Window(WindowRef::Entity(f.window)),
        GlobalTransform::IDENTITY,
        Projection::Orthographic(OrthographicProjection::default_2d()),
    ));
    move_window(&mut f, pb);
    f.app.update();
    move_window(&mut f, pa);
    click_window(&mut f);
    move_window(&mut f, pb);
    f.app.update();
    assert_eq!(
        f.app
            .world()
            .resource::<button_actions::ButtonActions>()
            .entities,
        [a]
    );
    let trace = f.app.world().resource::<Trace>();
    assert!(
        trace.presses.contains(&a) && !trace.presses.contains(&b) && !trace.presses.contains(&ui_b)
    );
}

#[test]
fn press_a_release_b_same_batch_neither_clicks_nor_strands_a() {
    let mut f = fixture(2.0);
    let b = add_second_button(&mut f);
    let pa = inside(&f);
    let pb = at_b(&f);
    move_window(&mut f, pa);
    button_window(&mut f, ButtonState::Pressed);
    move_window(&mut f, pb);
    button_window(&mut f, ButtonState::Released);
    f.app.update();
    assert!(
        f.app
            .world()
            .resource::<button_actions::ButtonActions>()
            .entities
            .is_empty()
    );
    assert!(
        !f.app
            .world()
            .entity(f.button)
            .contains::<bevy::ui::Pressed>()
            && !f.app.world().entity(b).contains::<bevy::ui::Pressed>()
    );
}
#[test]
#[ignore = "intentional opt-out negative: standard final-position dispatcher loses fresh A"]
fn negative_optout_fresh_a_requires_new_ordered_path() {
    let mut f = fixture_with_mode(2.0, false);
    let p = inside(&f);
    move_window(&mut f, p);
    click_window(&mut f);
    f.app.update();
    assert_eq!(outcome(&f).actions, 1);
}

#[path = "menu_picking_slider_tests.rs"]
mod slider_controls;

#[test]
fn first_input_of_new_pointer_reports_no_future_press() {
    use bevy::picking::{
        events::PointerMove,
        pointer::{PointerMap, PointerPressState},
    };
    let mut f = fixture(1.0);
    let id = PointerId::Touch(900);
    f.app.world_mut().spawn(id);
    f.app.add_observer(
        move |event: On<PointerMove>, map: Res<PointerMap>, pointers: Query<&PointerPressState>| {
            if event.pointer.id == id {
                assert!(
                    !pointers
                        .get(map.get_entity(id).unwrap())
                        .unwrap()
                        .is_primary_pressed()
                );
            }
        },
    );
    let location = Location {
        target: NormalizedRenderTarget::Window(
            WindowRef::Entity(f.window).normalize(None).unwrap(),
        ),
        position: inside(&f),
    };
    for action in [
        PointerAction::Move { delta: Vec2::ZERO },
        PointerAction::Press(PointerButton::Primary),
    ] {
        f.app
            .world_mut()
            .write_message(PointerInput::new(id, location.clone(), action));
    }
    f.app.update();
    assert_eq!(outcome(&f).presses, 1);
}

#[test]
fn review_stationary_drag_target_removed_without_input_matches_stock() {
    for ordered in [false, true] {
        let mut f = fixture_with_mode(1.0, ordered);
        let b = add_second_button(&mut f);
        let a_position = inside(&f);
        let b_position = at_b(&f);
        direct(&mut f, a_position, PointerAction::Move { delta: Vec2::ONE });
        f.app.update();
        direct(
            &mut f,
            a_position,
            PointerAction::Press(PointerButton::Primary),
        );
        f.app.update();
        direct(
            &mut f,
            b_position,
            PointerAction::Move {
                delta: b_position - a_position,
            },
        );
        f.app.update();
        let state = f
            .app
            .world()
            .resource::<bevy::picking::events::PointerState>()
            .get(PointerId::Mouse, PointerButton::Primary)
            .unwrap();
        assert!(state.dragging.contains_key(&f.button));
        assert!(state.dragging_over.contains_key(&b));
        f.app.world_mut().despawn(b);
        f.app.update();
        let state = f
            .app
            .world()
            .resource::<bevy::picking::events::PointerState>()
            .get(PointerId::Mouse, PointerButton::Primary)
            .unwrap();
        println!(
            "ordered={ordered}, removed target stale_dragging_over={}",
            state.dragging_over.contains_key(&b)
        );
        assert!(
            !state.dragging_over.contains_key(&b),
            "removed target remains in ordered dragging_over without another input"
        );
    }
}

#[test]
fn review_stationary_drag_target_moved_without_input_matches_stock() {
    for ordered in [false, true] {
        let mut f = fixture_with_mode(1.0, ordered);
        let b = add_second_button(&mut f);
        let a_position = inside(&f);
        let b_position = at_b(&f);
        direct(&mut f, a_position, PointerAction::Move { delta: Vec2::ONE });
        f.app.update();
        direct(
            &mut f,
            a_position,
            PointerAction::Press(PointerButton::Primary),
        );
        f.app.update();
        direct(
            &mut f,
            b_position,
            PointerAction::Move {
                delta: b_position - a_position,
            },
        );
        f.app.update();
        let state = f
            .app
            .world()
            .resource::<bevy::picking::events::PointerState>()
            .get(PointerId::Mouse, PointerButton::Primary)
            .unwrap();
        assert!(state.dragging.contains_key(&f.button));
        assert!(state.dragging_over.contains_key(&b));
        *f.app.world_mut().get_mut::<UiGlobalTransform>(b).unwrap() =
            UiGlobalTransform::from_translation(Vec2::new(180.0, 290.0));
        f.app.update();
        let hover = f.app.world().resource::<bevy::picking::hover::HoverMap>();
        assert!(!hover.get(&PointerId::Mouse).unwrap().contains_key(&b));
        let state = f
            .app
            .world()
            .resource::<bevy::picking::events::PointerState>()
            .get(PointerId::Mouse, PointerButton::Primary)
            .unwrap();
        println!(
            "ordered={ordered}, moved target stale_dragging_over={}",
            state.dragging_over.contains_key(&b)
        );
        assert!(
            !state.dragging_over.contains_key(&b),
            "moved target remains in ordered dragging_over without another input"
        );
    }
}

#[derive(Resource, Default)]
struct ReviewIdleTrace {
    moves: usize,
    drags: usize,
    enters: Vec<Entity>,
}
#[test]
fn review_drop_target_enters_stationary_drag_without_fake_move_or_drag() {
    for ordered in [false, true] {
        let mut f = fixture_with_mode(1.0, ordered);
        let b = add_second_button(&mut f);
        f.app.init_resource::<ReviewIdleTrace>();
        f.app.add_observer(
            |_: On<bevy::picking::events::PointerMove>, mut trace: ResMut<ReviewIdleTrace>| {
                trace.moves += 1
            },
        );
        f.app.add_observer(
            |_: On<bevy::picking::events::PointerDrag>, mut trace: ResMut<ReviewIdleTrace>| {
                trace.drags += 1
            },
        );
        f.app.add_observer(
            |event: On<bevy::picking::events::PointerDragEnter>,
             mut trace: ResMut<ReviewIdleTrace>| trace.enters.push(event.entity),
        );
        *f.app.world_mut().get_mut::<UiGlobalTransform>(b).unwrap() =
            UiGlobalTransform::from_translation(Vec2::new(180.0, 290.0));
        let a_position = inside(&f);
        let b_position = at_b(&f);
        direct(&mut f, a_position, PointerAction::Move { delta: Vec2::ONE });
        f.app.update();
        direct(
            &mut f,
            a_position,
            PointerAction::Press(PointerButton::Primary),
        );
        f.app.update();
        direct(
            &mut f,
            b_position,
            PointerAction::Move {
                delta: b_position - a_position,
            },
        );
        f.app.update();
        let state = f
            .app
            .world()
            .resource::<bevy::picking::events::PointerState>()
            .get(PointerId::Mouse, PointerButton::Primary)
            .unwrap();
        assert!(state.dragging.contains_key(&f.button));
        assert!(!state.dragging_over.contains_key(&b));
        *f.app.world_mut().resource_mut::<ReviewIdleTrace>() = ReviewIdleTrace::default();
        *f.app.world_mut().get_mut::<UiGlobalTransform>(b).unwrap() =
            UiGlobalTransform::from_translation(Vec2::new(180.0, 190.0));
        f.app.update();
        let hover = f.app.world().resource::<bevy::picking::hover::HoverMap>();
        assert!(hover.get(&PointerId::Mouse).unwrap().contains_key(&b));
        let trace = f.app.world().resource::<ReviewIdleTrace>();
        let entered = trace.enters.contains(&b);
        assert_eq!(
            trace.moves, 0,
            "idle reconciliation must not emit fake PointerMove"
        );
        assert_eq!(
            trace.drags, 0,
            "idle reconciliation must not emit fake PointerDrag"
        );
        let state = f
            .app
            .world()
            .resource::<bevy::picking::events::PointerState>()
            .get(PointerId::Mouse, PointerButton::Primary)
            .unwrap();
        println!(
            "ordered={ordered}, entered={entered}, current_drop_target={}, moves={}, drags={}",
            state.dragging_over.contains_key(&b),
            trace.moves,
            trace.drags
        );
        assert!(
            state.dragging_over.contains_key(&b),
            "new stationary drop target absent from ordered dragging_over"
        );
        assert!(
            entered,
            "new stationary drop target misses normal PointerDragEnter"
        );
    }
}

#[derive(Resource, Default)]
struct AncestorDragTrace {
    enters: usize,
    leaves: usize,
}
#[test]
fn idle_drag_reconciliation_preserves_capture_ancestor_events_and_cancel() {
    use bevy::picking::{
        events::{PointerDragEnter, PointerDragLeave, PointerDragStart, PointerState},
        hover::PointerCaptureMap,
    };
    for captured in [false, true] {
        let mut f = fixture(2.0);
        let b = add_second_button(&mut f);
        let a = f.button;
        f.app.init_resource::<AncestorDragTrace>();
        let parent = f.app.world_mut().spawn(Node::default()).id();
        f.app.world_mut().entity_mut(b).insert(ChildOf(parent));
        f.app
            .world_mut()
            .entity_mut(parent)
            .observe(
                |_: On<PointerDragEnter>, mut trace: ResMut<AncestorDragTrace>| trace.enters += 1,
            )
            .observe(
                |_: On<PointerDragLeave>, mut trace: ResMut<AncestorDragTrace>| trace.leaves += 1,
            );
        if captured {
            f.app.add_observer(
                move |event: On<PointerDragStart>, mut capture: ResMut<PointerCaptureMap>| {
                    if event.entity == a {
                        capture.capture(event.pointer.id, a, event.hit.clone());
                    }
                },
            );
        }
        let pa = inside(&f);
        let pb = at_b(&f);
        direct(&mut f, pa, PointerAction::Move { delta: Vec2::ONE });
        f.app.update();
        direct(&mut f, pa, PointerAction::Press(PointerButton::Primary));
        f.app.update();
        direct(&mut f, pb, PointerAction::Move { delta: pb - pa });
        f.app.update();
        // Capture requested by DragStart cannot retroactively change this Move's resolved B.
        // Its final hover reconciliation leaves B; subsequent idle geometry must respect capture.
        {
            let initial = f.app.world().resource::<AncestorDragTrace>();
            assert_eq!((initial.enters, initial.leaves), (1, usize::from(captured)));
        }
        *f.app.world_mut().resource_mut::<AncestorDragTrace>() = AncestorDragTrace::default();
        let mut moves =
            bevy::ecs::message::MessageCursor::<bevy::picking::events::PointerMove>::default();
        let mut drags =
            bevy::ecs::message::MessageCursor::<bevy::picking::events::PointerDrag>::default();
        moves
            .read(
                f.app
                    .world()
                    .resource::<Messages<bevy::picking::events::PointerMove>>(),
            )
            .for_each(drop);
        drags
            .read(
                f.app
                    .world()
                    .resource::<Messages<bevy::picking::events::PointerDrag>>(),
            )
            .for_each(drop);
        *f.app.world_mut().get_mut::<UiGlobalTransform>(b).unwrap() =
            UiGlobalTransform::from_translation(Vec2::new(180.0, 290.0));
        f.app.update();
        *f.app.world_mut().get_mut::<UiGlobalTransform>(b).unwrap() =
            UiGlobalTransform::from_translation(Vec2::new(180.0, 190.0));
        f.app.update();
        let trace = f.app.world().resource::<AncestorDragTrace>();
        assert_eq!(
            (trace.enters, trace.leaves),
            if captured { (0, 0) } else { (1, 1) }
        );
        assert_eq!(
            moves
                .read(
                    f.app
                        .world()
                        .resource::<Messages<bevy::picking::events::PointerMove>>()
                )
                .count(),
            0
        );
        assert_eq!(
            drags
                .read(
                    f.app
                        .world()
                        .resource::<Messages<bevy::picking::events::PointerDrag>>()
                )
                .count(),
            0
        );
        assert_eq!(
            f.app
                .world()
                .resource::<PointerCaptureMap>()
                .is_captured(&PointerId::Mouse),
            captured
        );
        direct(&mut f, pb, PointerAction::Cancel);
        f.app.update();
        f.app.update();
        let state = f
            .app
            .world()
            .resource::<PointerState>()
            .get(PointerId::Mouse, PointerButton::Primary)
            .unwrap();
        assert!(
            state.pressing.is_empty()
                && state.dragging.is_empty()
                && state.dragging_over.is_empty()
        );
        assert!(
            !f.app
                .world()
                .resource::<PointerCaptureMap>()
                .is_captured(&PointerId::Mouse)
        );
        assert!(
            f.app
                .world()
                .resource::<button_actions::ButtonActions>()
                .entities
                .is_empty()
        );
        assert!(!f.app.world().entity(a).contains::<bevy::ui::Pressed>());
    }
}
