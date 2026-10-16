#![forbid(unsafe_code)]
use super as menu_slider;
use crate::button_actions;
use bevy::{
    app::{HierarchyPropagatePlugin, PropagateSet},
    camera::{
        ComputedCameraValues, NormalizedRenderTarget, RenderTarget, RenderTargetInfo, Viewport,
    },
    input::{
        ButtonState, InputPlugin,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{
        FocusCause, InputDispatchPlugin, InputFocus, InputFocusPlugin,
        tab_navigation::{TabIndex, TabNavigationPlugin},
    },
    picking::{
        backend::HitData,
        events::{
            Pointer, PointerClick, PointerDrag, PointerDragEnd, PointerDragStart, PointerPress,
            PointerRelease,
        },
        hover::PointerCaptureMap,
        pointer::{Location, PointerButton, PointerId},
    },
    prelude::*,
    ui::{ComputedUiRenderTargetInfo, ComputedUiTargetCamera, UiGlobalTransform, UiScale},
    ui_widgets::{
        Button, ButtonPlugin, Slider, SliderOrientation, SliderPlugin, SliderRange, SliderStep,
        SliderThumb, SliderValue, TrackClick, slider_self_update,
    },
    window::{PrimaryWindow, WindowRef},
};

struct Fixture {
    app: App,
    window: Entity,
    camera: Entity,
    slider: Entity,
    track: Entity,
    scale: f32,
}
fn fixture(corrected: bool, scale: f32, origin: UVec2) -> Fixture {
    fixture_scaled(corrected, scale, origin, 1.0)
}
fn fixture_scaled(corrected: bool, scale: f32, origin: UVec2, ui_scale: f32) -> Fixture {
    let mut app = App::new();
    app.add_plugins((
        InputPlugin,
        InputFocusPlugin,
        InputDispatchPlugin,
        TabNavigationPlugin,
        SliderPlugin,
        ButtonPlugin,
        bevy::input_focus::pointer_focus::PointerFocusPlugin,
        HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate),
        HierarchyPropagatePlugin::<ComputedUiRenderTargetInfo>::new(PostUpdate),
    ));
    app.insert_resource(UiScale(ui_scale))
        .init_resource::<PointerCaptureMap>();
    button_actions::install(&mut app);
    app.add_systems(
        PostUpdate,
        bevy::ui::update::propagate_ui_target_cameras
            .before(PropagateSet::<ComputedUiTargetCamera>::default())
            .before(PropagateSet::<ComputedUiRenderTargetInfo>::default()),
    );
    app.add_observer(slider_self_update);
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let camera = app
        .world_mut()
        .spawn((
            Camera {
                viewport: Some(Viewport {
                    physical_position: origin,
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
            bevy::picking::hover::Hovered::default(),
            ComputedNode {
                size: Vec2::new(240.0, 30.0),
                inverse_scale_factor: 1.0 / (scale * ui_scale),
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(130.0, 70.0)),
            UiTargetCamera(camera),
            Slider {
                track_click: TrackClick::Snap,
                orientation: SliderOrientation::Horizontal,
            },
            Button,
            SliderValue(50.0),
            SliderRange::new(-50.0, 150.0),
            SliderStep(7.0),
            TabIndex(0),
        ))
        .id();
    let thumb = app
        .world_mut()
        .spawn((
            Node::default(),
            ComputedNode {
                size: Vec2::new(20.0, 30.0),
                ..default()
            },
            SliderThumb,
            ChildOf(slider),
        ))
        .id();
    assert_ne!(thumb, slider);
    if corrected {
        let mut commands = app.world_mut().commands();
        menu_slider::attach(&mut commands, slider);
        app.world_mut().flush();
    }
    app.update();
    let track = if corrected {
        app.world()
            .get::<Children>(slider)
            .unwrap()
            .iter()
            .find(|e| *e != thumb)
            .unwrap()
    } else {
        slider
    };
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(slider, FocusCause::Navigated);
    Fixture {
        app,
        window,
        camera,
        slider,
        track,
        scale,
    }
}
fn pointer(f: &Fixture, local: Vec2) -> Pointer {
    let origin = f
        .app
        .world()
        .get::<Camera>(f.camera)
        .unwrap()
        .viewport
        .as_ref()
        .unwrap()
        .physical_position
        .as_vec2();
    Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(f.window).normalize(None).unwrap(),
            ),
            position: (origin + local) / f.scale,
        },
    )
}
fn hit(f: &Fixture) -> HitData {
    HitData {
        camera: f.camera,
        depth: 0.0,
        position: None,
        normal: None,
        extra: None,
    }
}
fn press(f: &mut Fixture, local: Vec2) {
    let pointer = pointer(f, local);
    let hit = hit(f);
    f.app.world_mut().trigger(PointerPress {
        entity: f.track,
        pointer,
        button: PointerButton::Primary,
        hit,
        count: 1,
    });
    f.app.world_mut().flush();
}
fn value(f: &Fixture) -> f32 {
    f.app.world().get::<SliderValue>(f.slider).unwrap().0
}
#[test]
fn corrected_real_handler_midpoint_all_origins_and_scales() {
    for origin in [UVec2::ZERO, UVec2::new(37, 23)] {
        for scale in [1.0, 2.0] {
            let mut f = fixture(true, scale, origin);
            press(&mut f, Vec2::new(130.0, 70.0));
            assert!((value(&f) - 50.0).abs() < 0.0001, "got{}", value(&f));
        }
    }
}
#[test]
fn corrected_real_handler_endpoints_preserves_bounds() {
    let mut f = fixture(true, 2.0, UVec2::new(37, 23));
    for (x, want) in [(20.0, -50.0), (240.0, 150.0)] {
        press(&mut f, Vec2::new(x, 70.0));
        assert!((value(&f) - want).abs() < 0.0001);
    }
}
#[test]
fn corrected_keyboard_and_drag_unchanged() {
    for scale in [1.0, 2.0] {
        for ui_scale in [0.75, 1.0, 1.5] {
            let mut f = fixture_scaled(true, scale, UVec2::new(37, 23), ui_scale);
            f.app.world_mut().write_message(KeyboardInput {
                key_code: KeyCode::ArrowRight,
                logical_key: Key::ArrowRight,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: f.window,
            });
            f.app.update();
            assert!((value(&f) - 57.0).abs() < 0.0001);
            let pointer = pointer(&f, Vec2::new(130.0, 70.0));
            let hit = hit(&f);
            f.app.world_mut().trigger(PointerDragStart {
                entity: f.track,
                pointer: pointer.clone(),
                button: PointerButton::Primary,
                hit: hit.clone(),
            });
            f.app.world_mut().flush();
            assert!(
                f.app
                    .world()
                    .resource::<PointerCaptureMap>()
                    .is_captured(&PointerId::Mouse)
            );
            f.app.world_mut().trigger(PointerDrag {
                entity: f.slider,
                pointer: pointer.clone(),
                button: PointerButton::Primary,
                distance: Vec2::new(22.0 / scale, 0.0),
                delta: Vec2::new(22.0 / scale, 0.0),
            });
            f.app.world_mut().flush();
            assert!((value(&f) - 77.0).abs() < 0.0001, "got{}", value(&f));
            f.app.world_mut().trigger(PointerDragEnd {
                entity: f.slider,
                pointer,
                button: PointerButton::Primary,
                distance: Vec2::new(22.0 / scale, 0.0),
            });
            f.app.world_mut().flush();
            assert!(
                !f.app
                    .world()
                    .resource::<PointerCaptureMap>()
                    .is_captured(&PointerId::Mouse)
            );
        }
    }
}

#[test]
fn corrected_ui_scale_and_pointer_focus() {
    for scale in [1.0, 2.0] {
        for ui_scale in [0.75, 1.5] {
            let mut f = fixture_scaled(true, scale, UVec2::new(37, 23), ui_scale);
            f.app.world_mut().resource_mut::<InputFocus>().clear();
            press(&mut f, Vec2::new(130.0, 70.0));
            assert!((value(&f) - 50.0).abs() < 0.0001);
            assert_eq!(f.app.world().resource::<InputFocus>().get(), Some(f.slider));
        }
    }
}
#[test]
fn child_click_keeps_one_actual_button_action_on_parent() {
    let mut f = fixture(true, 1.0, UVec2::new(37, 23));
    press(&mut f, Vec2::new(130.0, 70.0));
    let pointer = pointer(&f, Vec2::new(130.0, 70.0));
    let hit = hit(&f);
    f.app.world_mut().trigger(PointerClick {
        entity: f.track,
        pointer: pointer.clone(),
        button: PointerButton::Primary,
        hit: hit.clone(),
        duration: std::time::Duration::ZERO,
        count: 1,
    });
    f.app.world_mut().trigger(PointerRelease {
        entity: f.track,
        pointer,
        button: PointerButton::Primary,
        hit,
    });
    f.app.world_mut().flush();
    assert_eq!(
        f.app
            .world()
            .resource::<button_actions::ButtonActions>()
            .entities,
        [f.slider]
    );
    assert!(
        !f.app
            .world()
            .entity(f.slider)
            .contains::<bevy::ui::Pressed>()
    );
}
#[test]
fn disabled_parent_and_missing_camera_refuse_changes() {
    let mut f = fixture(true, 1.0, UVec2::new(37, 23));
    f.app
        .world_mut()
        .entity_mut(f.slider)
        .insert(bevy::ui::InteractionDisabled);
    press(&mut f, Vec2::new(240.0, 70.0));
    assert_eq!(value(&f), 50.0);
    f.app
        .world_mut()
        .entity_mut(f.slider)
        .remove::<bevy::ui::InteractionDisabled>();
    f.app.world_mut().entity_mut(f.camera).remove::<Camera>();
    // Build the previously valid full-target event without using the removed camera.
    let p = Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(f.window).normalize(None).unwrap(),
            ),
            position: Vec2::new(277.0, 93.0),
        },
    );
    let hit = hit(&f);
    f.app.world_mut().trigger(PointerPress {
        entity: f.track,
        pointer: p,
        button: PointerButton::Primary,
        hit,
        count: 1,
    });
    f.app.world_mut().flush();
    assert_eq!(value(&f), 50.0);
}
#[test]
fn actual_ui_picking_keeps_full_target_coordinates() {
    use bevy::{
        picking::{backend::PointerHits, pointer::PointerLocation},
        ui::{UiStack, picking_backend::UiPickingPlugin},
    };
    let mut f = fixture(true, 2.0, UVec2::new(37, 23));
    f.app
        .add_plugins(UiPickingPlugin)
        .init_resource::<UiStack>()
        .init_resource::<bevy::picking::pointer::PointerMap>()
        .add_message::<PointerHits>();
    for e in [f.slider, f.track] {
        f.app.world_mut().entity_mut(e).insert((
            ComputedNode {
                size: Vec2::new(240.0, 30.0),
                inverse_scale_factor: 0.5,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(130.0, 70.0)),
            InheritedVisibility::VISIBLE,
        ));
    }
    {
        let mut stack = f.app.world_mut().resource_mut::<UiStack>();
        stack.uinodes = vec![f.slider, f.track];
        stack.partition = std::iter::once(0..2).collect();
    }
    let p = pointer(&f, Vec2::new(130.0, 70.0));
    f.app
        .world_mut()
        .spawn((PointerId::Mouse, PointerLocation::new(p.location())));
    let mut cursor = f
        .app
        .world()
        .resource::<Messages<PointerHits>>()
        .get_cursor();
    f.app.update();
    let hits: Vec<_> = cursor
        .read(f.app.world().resource::<Messages<PointerHits>>())
        .flat_map(|h| h.picks.iter().map(|(entity, _)| *entity))
        .collect();
    assert!(hits.contains(&f.track), "actual hits: {hits:?}");
    use bevy::ecs::system::RunSystemOnce;
    f.app.init_resource::<bevy::picking::hover::HoverMap>();
    let current_hit = hit(&f);
    f.app
        .world_mut()
        .resource_mut::<bevy::picking::hover::HoverMap>()
        .insert(
            PointerId::Mouse,
            std::iter::once((f.track, current_hit)).collect(),
        );
    f.app
        .world_mut()
        .run_system_once(bevy::picking::hover::update_is_hovered)
        .unwrap();
    f.app.world_mut().flush();
    assert!(
        f.app
            .world()
            .get::<bevy::picking::hover::Hovered>(f.slider)
            .unwrap()
            .get()
    );
    press(&mut f, Vec2::new(130.0, 70.0));
    assert_eq!(value(&f), 50.0);
    let mut pointers = f
        .app
        .world_mut()
        .query_filtered::<&PointerLocation, With<PointerId>>();
    let location = pointers.single(f.app.world()).unwrap();
    assert_eq!(location.location.as_ref().unwrap().position, p.position);
}
