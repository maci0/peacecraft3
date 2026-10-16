#![forbid(unsafe_code)]
use super::menu_slider;
use crate::button_actions;
use bevy::{
    app::{HierarchyPropagatePlugin, PropagateSet},
    camera::{ComputedCameraValues, RenderTarget, RenderTargetInfo, Viewport},
    ecs::{message::MessageCursor, system::RunSystemOnce},
    input::{ButtonState, InputPlugin, mouse::MouseButtonInput},
    input_focus::{
        InputDispatchPlugin, InputFocusPlugin, pointer_focus::PointerFocusPlugin,
        tab_navigation::TabNavigationPlugin,
    },
    picking::{
        DefaultPickingPlugins, Pickable,
        events::{PointerClick, PointerPress, PointerRelease, PointerState},
        hover::{HoverMap, Hovered, PointerCaptureMap, PreviousHoverMap},
        pointer::{PointerButton, PointerId, PointerInput, PointerLocation, PointerPressState},
    },
    prelude::*,
    ui::{ComputedUiRenderTargetInfo, ComputedUiTargetCamera, UiGlobalTransform, UiStack},
    ui_widgets::{
        Button, ButtonPlugin, Slider, SliderDragState, SliderOrientation, SliderPlugin,
        SliderRange, SliderStep, SliderThumb, SliderValue, TrackClick, slider_self_update,
    },
    window::{CursorMoved, PrimaryWindow, WindowEvent, WindowRef},
};

struct Fixture {
    app: App,
    window: Entity,
    slider: Entity,
    track: Entity,
    ok: Entity,
    scale: f32,
    press: MessageCursor<PointerPress>,
    click: MessageCursor<PointerClick>,
    release: MessageCursor<PointerRelease>,
    input: MessageCursor<PointerInput>,
}
#[derive(Debug)]
struct Snapshot {
    presses: Vec<Entity>,
    clicks: Vec<Entity>,
    releases: Vec<Entity>,
    actions: Vec<Entity>,
    pressing: Vec<Entity>,
    dragging: Vec<Entity>,
    capture: Option<Entity>,
    hover: Vec<Entity>,
    previous: Vec<Entity>,
    slider_pressed: bool,
    ok_pressed: bool,
    slider_hovered: bool,
    ok_hovered: bool,
    value: f32,
    drag_active: bool,
    physical_button: bool,
}
fn fixture(scale: f32) -> Fixture {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        InputPlugin,
        DefaultPickingPlugins,
        ButtonPlugin,
        SliderPlugin,
        UiPickingPlugin,
        InputFocusPlugin,
        InputDispatchPlugin,
        TabNavigationPlugin,
        PointerFocusPlugin,
        HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate),
        HierarchyPropagatePlugin::<ComputedUiRenderTargetInfo>::new(PostUpdate),
    ));
    app.add_message::<WindowEvent>()
        .init_resource::<UiStack>()
        .init_resource::<UiScale>();
    app.add_systems(
        PostUpdate,
        bevy::ui::update::propagate_ui_target_cameras
            .before(PropagateSet::<ComputedUiTargetCamera>::default())
            .before(PropagateSet::<ComputedUiRenderTargetInfo>::default()),
    );
    app.add_observer(slider_self_update);
    button_actions::install(&mut app);
    app.insert_resource(bevy::picking::ordered::OrderedPointerInput);
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
    let slider = app
        .world_mut()
        .spawn((
            Node::default(),
            Button,
            Hovered::default(),
            Slider {
                track_click: TrackClick::Snap,
                orientation: SliderOrientation::Horizontal,
            },
            SliderValue(50.0),
            SliderRange::new(-50.0, 150.0),
            SliderStep(7.0),
            ComputedNode {
                size: Vec2::new(240.0, 30.0),
                inverse_scale_factor: 1.0 / scale,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(130.0, 70.0)),
            InheritedVisibility::VISIBLE,
            UiTargetCamera(camera),
        ))
        .id();
    let thumb = app
        .world_mut()
        .spawn((
            Node::default(),
            SliderThumb,
            Pickable::IGNORE,
            ChildOf(slider),
            ComputedNode {
                size: Vec2::new(20.0, 30.0),
                inverse_scale_factor: 1.0 / scale,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(130.0, 70.0)),
            InheritedVisibility::VISIBLE,
        ))
        .id();
    menu_slider::attach(&mut app.world_mut().commands(), slider);
    app.world_mut().flush();
    let track = app
        .world()
        .get::<Children>(slider)
        .unwrap()
        .iter()
        .find(|e| *e != thumb)
        .unwrap();
    app.world_mut().entity_mut(track).insert((
        ComputedNode {
            size: Vec2::new(240.0, 30.0),
            inverse_scale_factor: 1.0 / scale,
            ..default()
        },
        UiGlobalTransform::from_translation(Vec2::new(130.0, 70.0)),
        InheritedVisibility::VISIBLE,
    ));
    let ok = app
        .world_mut()
        .spawn((
            Node::default(),
            Button,
            Hovered::default(),
            ComputedNode {
                size: Vec2::new(100.0, 40.0),
                inverse_scale_factor: 1.0 / scale,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(180.0, 190.0)),
            InheritedVisibility::VISIBLE,
            UiTargetCamera(camera),
        ))
        .id();
    {
        let mut stack = app.world_mut().resource_mut::<UiStack>();
        stack.uinodes = vec![slider, thumb, track, ok];
        stack.partition = std::iter::once(0..4).collect();
    }
    app.update();
    app.update();
    println!("FIXTURE scale={scale} slider={slider:?} track={track:?} ok={ok:?}");
    Fixture {
        app,
        window,
        slider,
        track,
        ok,
        scale,
        press: default(),
        click: default(),
        release: default(),
        input: default(),
    }
}
use bevy::ui::picking_backend::UiPickingPlugin;
fn move_to(f: &mut Fixture, local: Vec2) {
    let pos = (Vec2::new(37.0, 23.0) + local) / f.scale;
    f.app
        .world_mut()
        .write_message(WindowEvent::CursorMoved(CursorMoved {
            window: f.window,
            position: pos,
            delta: None,
        }));
}
fn button(f: &mut Fixture, state: ButtonState) {
    f.app
        .world_mut()
        .write_message(WindowEvent::MouseButtonInput(MouseButtonInput {
            window: f.window,
            button: MouseButton::Left,
            state,
        }));
}
fn sorted(values: impl IntoIterator<Item = Entity>) -> Vec<Entity> {
    let mut v: Vec<_> = values.into_iter().collect();
    v.sort_by_key(|e| e.to_bits());
    v
}
fn step(f: &mut Fixture, label: &str) -> Snapshot {
    f.app.update();
    let world = f.app.world_mut();
    let presses = f
        .press
        .read(world.resource::<Messages<PointerPress>>())
        .map(|e| e.entity)
        .collect();
    let clicks = f
        .click
        .read(world.resource::<Messages<PointerClick>>())
        .map(|e| e.entity)
        .collect();
    let releases = f
        .release
        .read(world.resource::<Messages<PointerRelease>>())
        .map(|e| e.entity)
        .collect();
    let inputs: Vec<_> = f
        .input
        .read(world.resource::<Messages<PointerInput>>())
        .map(|e| format!("{e:?}"))
        .collect();
    let state = world
        .resource::<PointerState>()
        .get(PointerId::Mouse, PointerButton::Primary);
    let pressing = sorted(state.into_iter().flat_map(|s| s.pressing.keys().copied()));
    let dragging = sorted(state.into_iter().flat_map(|s| s.dragging.keys().copied()));
    let capture = world
        .resource::<PointerCaptureMap>()
        .get(&PointerId::Mouse)
        .map(|(e, _)| e);
    let hover = sorted(
        world
            .resource::<HoverMap>()
            .get(&PointerId::Mouse)
            .into_iter()
            .flat_map(|m| m.keys().copied()),
    );
    let previous = sorted(
        world
            .resource::<PreviousHoverMap>()
            .get(&PointerId::Mouse)
            .into_iter()
            .flat_map(|m| m.keys().copied()),
    );
    let actions = world
        .resource::<button_actions::ButtonActions>()
        .entities
        .clone();
    let mut q = world.query::<(&PointerId, &PointerLocation, &PointerPressState)>();
    let (_, position, buttons) = q
        .iter(world)
        .find(|(id, _, _)| **id == PointerId::Mouse)
        .unwrap();
    let position = position.location.as_ref().map(|p| p.position);
    let physical_button = buttons.is_primary_pressed();
    let s = Snapshot {
        presses,
        clicks,
        releases,
        actions,
        pressing,
        dragging,
        capture,
        hover,
        previous,
        slider_pressed: world.entity(f.slider).contains::<bevy::ui::Pressed>(),
        ok_pressed: world.entity(f.ok).contains::<bevy::ui::Pressed>(),
        slider_hovered: world.get::<Hovered>(f.slider).unwrap().get(),
        ok_hovered: world.get::<Hovered>(f.ok).unwrap().get(),
        value: world.get::<SliderValue>(f.slider).unwrap().0,
        drag_active: world.get::<SliderDragState>(f.slider).unwrap().dragging,
        physical_button,
    };
    println!(
        "FRAME {label} scale={} pointer={position:?} inputs={inputs:?} snapshot={s:?}",
        f.scale
    );
    world.run_system_once(button_actions::clear).unwrap();
    s
}
fn assert_idle(s: &Snapshot) {
    assert!(
        s.pressing.is_empty() && s.dragging.is_empty() && s.capture.is_none(),
        "{s:?}"
    );
    assert!(
        !s.physical_button && !s.drag_active && !s.slider_pressed && !s.ok_pressed,
        "{s:?}"
    );
}
fn acknowledged_ok(f: &mut Fixture) {
    move_to(f, Vec2::new(180.0, 190.0));
    let hover = step(f, "OK hover");
    assert!(hover.ok_hovered && !hover.slider_hovered && hover.hover.contains(&f.ok));
    button(f, ButtonState::Pressed);
    let down = step(f, "OK down");
    assert!(down.previous.contains(&f.ok));
    assert_eq!(down.presses, [f.ok]);
    assert!(down.ok_pressed && down.pressing.contains(&f.ok));
    assert!(down.actions.is_empty());
    button(f, ButtonState::Released);
    let up = step(f, "OK release");
    assert_eq!(up.clicks, [f.ok]);
    assert_eq!(up.releases, [f.ok]);
    assert_eq!(up.actions, [f.ok]);
    assert_idle(&up);
    let after = step(f, "OK settled");
    assert!(after.actions.is_empty());
    assert_idle(&after);
}
fn slider_lifecycle(f: &mut Fixture, acknowledge_capture: bool) {
    move_to(f, Vec2::new(130.0, 70.0));
    let hover = step(f, "slider hover");
    assert!(hover.hover.contains(&f.track) && hover.slider_hovered);
    button(f, ButtonState::Pressed);
    let down = step(f, "slider down");
    assert_eq!(down.presses, [f.track]);
    assert!(down.slider_pressed && down.pressing.contains(&f.track));
    move_to(f, Vec2::new(350.0, 110.0));
    let drag = step(f, "slider outside drag");
    assert!(drag.drag_active && drag.capture == Some(f.slider));
    assert!(drag.dragging.contains(&f.track));
    // One processed frame while outside establishes the ordinary captured hover state.
    if acknowledge_capture {
        let captured = step(f, "slider outside capture acknowledged");
        assert_eq!(captured.capture, Some(f.slider));
    }
    button(f, ButtonState::Released);
    let up = step(f, "slider outside release");
    assert_idle(&up);
    assert!((up.value - 150.0).abs() < 0.0001, "{up:?}");
    move_to(f, Vec2::new(130.0, 70.0));
    let midpoint = step(f, "slider midpoint hover");
    assert!(midpoint.slider_hovered);
    button(f, ButtonState::Pressed);
    let down = step(f, "slider midpoint down");
    assert_eq!(down.presses, [f.track]);
    assert!((down.value - 50.0).abs() < 0.0001, "{down:?}");
    button(f, ButtonState::Released);
    let up = step(f, "slider midpoint release");
    assert_idle(&up);
    assert_eq!(up.actions, [f.slider]);
}
#[test]
fn acknowledged_ok_without_prior_slider() {
    for scale in [1.0, 2.0] {
        let mut f = fixture(scale);
        acknowledged_ok(&mut f);
    }
}
#[test]
fn acknowledged_ok_after_completed_outside_drag_and_midpoint() {
    for scale in [1.0, 2.0] {
        for capture_ack in [false, true] {
            let mut f = fixture(scale);
            slider_lifecycle(&mut f, capture_ack);
            acknowledged_ok(&mut f);
        }
    }
}
