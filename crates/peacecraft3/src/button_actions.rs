//! Keep button activations until every action/audio consumer has run this frame.
use bevy::{
    picking::events::PointerClick,
    prelude::*,
    ui::{InteractionDisabled, Pressed},
    ui_widgets::{Activate, ActivateOnPress, Button},
};

#[derive(Resource, Default)]
pub(crate) struct ButtonActions {
    pub entities: Vec<Entity>,
}

pub(crate) fn install(app: &mut App) {
    if app.world().contains_resource::<ButtonActions>() {
        return;
    }
    app.init_resource::<ButtonActions>()
        .add_observer(collect)
        .add_observer(collect_click)
        .add_observer(clear_disabled_press);
}

// Press insertion may be deferred until after the control becomes disabled.
type DisabledPressFilter = (With<Button>, With<InteractionDisabled>, With<Pressed>);

fn clear_disabled_press(
    event: On<Add<(InteractionDisabled, Pressed)>>,
    disabled: Query<(), DisabledPressFilter>,
    mut commands: Commands,
) {
    if disabled.contains(event.entity) {
        commands.entity(event.entity).try_remove::<Pressed>();
    }
}

fn collect(
    event: On<Activate>,
    enabled: Query<(), (With<Button>, Without<InteractionDisabled>)>,
    mut actions: ResMut<ButtonActions>,
) {
    if enabled.contains(event.entity) && !actions.entities.contains(&event.entity) {
        actions.entities.push(event.entity);
    }
}

// Picking emits Click only for a completed click on its press target. Preserve
// that release behavior even when the widget's transient Pressed insertion and
// removal share one command flush. Keyboard Activate and pointer Click can both
// arrive for the same button; retain one action per button in a frame.
type ClickButtonFilter = (
    With<Button>,
    Without<InteractionDisabled>,
    Without<ActivateOnPress>,
);

fn collect_click(
    event: On<PointerClick>,
    enabled: Query<(), ClickButtonFilter>,
    mut actions: ResMut<ButtonActions>,
) {
    if enabled.contains(event.entity) && !actions.entities.contains(&event.entity) {
        actions.entities.push(event.entity);
    }
}

// The caller orders this after all consumers, never between input and gameplay.
pub(crate) fn clear(mut actions: ResMut<ButtonActions>) {
    actions.entities.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::NormalizedRenderTarget,
        ecs::system::RunSystemOnce,
        picking::{
            backend::HitData,
            events::{Pointer, PointerClick, PointerPress, PointerRelease},
            pointer::{Location, PointerButton, PointerId},
        },
        ui::Pressed,
        ui_widgets::{ActivateOnPress, ButtonPlugin},
    };

    fn pointer() -> Pointer {
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: 1,
                    height: 1,
                },
                position: Vec2::ZERO,
            },
        )
    }
    fn press(entity: Entity) -> PointerPress {
        PointerPress {
            entity,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: HitData::new(entity, 0.0, None, None),
            count: 1,
        }
    }
    fn release(entity: Entity) -> PointerRelease {
        PointerRelease {
            entity,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: HitData::new(entity, 0.0, None, None),
        }
    }
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(ButtonPlugin);
        install(&mut app);
        install(&mut app); // Intro's standalone install must not duplicate the observer.
        app
    }

    #[test]
    fn a_fast_click_survives_pressed_removal_before_consumers_run() {
        let mut app = app();
        let world = app.world_mut();
        let button = world.spawn(Button).id();
        world.trigger(press(button));
        world.flush();
        world.trigger(PointerClick {
            entity: button,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: HitData::new(button, 0.0, None, None),
            duration: std::time::Duration::from_millis(1),
            count: 1,
        });
        world.trigger(release(button));
        world.flush();
        assert!(!world.entity(button).contains::<Pressed>());
        assert_eq!(world.resource::<ButtonActions>().entities, [button]);
        world.run_system_once(clear).unwrap();
        assert!(world.resource::<ButtonActions>().entities.is_empty());
    }

    #[test]
    fn press_click_and_release_in_one_flush_yield_one_action() {
        let mut app = app();
        let world = app.world_mut();
        let button = world.spawn(Button).id();
        let mut commands = world.commands();
        commands.trigger(press(button));
        commands.trigger(PointerClick {
            entity: button,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: HitData::new(button, 0.0, None, None),
            duration: std::time::Duration::from_millis(1),
            count: 1,
        });
        commands.trigger(release(button));
        world.flush();
        assert!(!world.entity(button).contains::<Pressed>());
        assert_eq!(world.resource::<ButtonActions>().entities, [button]);
    }

    #[test]
    fn disabled_buttons_are_filtered_even_if_activation_is_triggered_directly() {
        let mut app = app();
        let world = app.world_mut();
        let button = world.spawn((Button, InteractionDisabled)).id();
        world.trigger(press(button));
        world.trigger(Activate { entity: button });
        world.flush();
        assert!(world.resource::<ButtonActions>().entities.is_empty());
        assert!(!world.entity(button).contains::<Pressed>());
    }

    #[test]
    fn disabling_during_press_clears_state_even_with_deferred_press_insertion() {
        for deferred in [false, true] {
            let mut app = app();
            let world = app.world_mut();
            let button = world.spawn(Button).id();
            if deferred {
                world.commands().trigger(press(button));
                world.commands().entity(button).insert(InteractionDisabled);
            } else {
                world.trigger(press(button));
                world.flush();
                assert!(world.entity(button).contains::<Pressed>());
                world.entity_mut(button).insert(InteractionDisabled);
            }
            world.flush();
            world.trigger(release(button));
            world.flush();
            assert!(world.resource::<ButtonActions>().entities.is_empty());
            assert!(
                !world.entity(button).contains::<Pressed>(),
                "deferred={deferred}"
            );

            world.entity_mut(button).remove::<InteractionDisabled>();
            world.trigger(press(button));
            world.flush();
            assert!(world.entity(button).contains::<Pressed>());
            world.trigger(PointerClick {
                entity: button,
                pointer: pointer(),
                button: PointerButton::Primary,
                hit: HitData::new(button, 0.0, None, None),
                duration: std::time::Duration::from_millis(1),
                count: 1,
            });
            world.trigger(release(button));
            world.flush();
            assert!(!world.entity(button).contains::<Pressed>());
            assert_eq!(world.resource::<ButtonActions>().entities, [button]);
        }
    }

    #[test]
    fn holding_a_button_does_not_create_another_activation() {
        let mut app = app();
        let world = app.world_mut();
        let button = world.spawn((Button, ActivateOnPress)).id();
        world.trigger(press(button));
        world.flush();
        assert_eq!(world.resource::<ButtonActions>().entities, [button]);
        world.run_system_once(clear).unwrap();
        world.trigger(press(button));
        world.flush();
        assert!(world.entity(button).contains::<Pressed>());
        assert!(world.resource::<ButtonActions>().entities.is_empty());
        world.trigger(release(button));
        world.flush();
        assert!(world.resource::<ButtonActions>().entities.is_empty());
    }
}
