use super::{SettingsCog, spawn_main_menu, sync_settings_cog};
use crate::{
    button_actions,
    menu_screens::{Navigation, Page, Screen},
};
use bevy::{
    camera::NormalizedRenderTarget,
    ecs::system::RunSystemOnce,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
    picking::{
        backend::HitData,
        events::{Pointer, PointerClick},
        pointer::{Location, PointerButton, PointerId},
    },
    prelude::*,
    ui::{InteractionDisabled, Pressed},
    ui_widgets::{Activate, ButtonPlugin},
};

fn off_main_screens() -> [Screen; 11] {
    [
        Screen::Options(Page::Unselected),
        Screen::Options(Page::Video),
        Screen::Options(Page::Sound),
        Screen::Options(Page::Gameplay),
        Screen::SinglePlayer,
        Screen::Campaign,
        Screen::CustomCampaign,
        Screen::LoadSaved,
        Screen::Replay,
        Screen::Skirmish,
        Screen::Multiplayer,
    ]
}

fn fixture() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(ButtonPlugin);
    button_actions::install(&mut app);
    app.init_resource::<Navigation>();
    spawn_main_menu(&mut app.world_mut().commands());
    app.world_mut().flush();
    let mut cogs = app
        .world_mut()
        .query_filtered::<Entity, With<SettingsCog>>();
    let cog = cogs.single(app.world()).unwrap();
    (app, cog)
}

fn scope(app: &mut App, screen: Screen) {
    app.world_mut().resource_mut::<Navigation>().screen = screen;
    app.world_mut().run_system_once(sync_settings_cog).unwrap();
}

fn assert_scope(world: &World, cog: Entity, enabled: bool) {
    let entity = world.entity(cog);
    assert_eq!(
        entity.get::<Node>().unwrap().display,
        if enabled {
            Display::Flex
        } else {
            Display::None
        }
    );
    assert_eq!(
        *entity.get::<Visibility>().unwrap(),
        if enabled {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    );
    assert_eq!(
        *entity.get::<Pickable>().unwrap(),
        if enabled {
            Pickable::default()
        } else {
            Pickable::IGNORE
        }
    );
    assert_eq!(
        entity.get::<TabIndex>().unwrap().0,
        if enabled { i32::MAX } else { -1 }
    );
    assert_eq!(entity.contains::<InteractionDisabled>(), !enabled);
    let accessible = entity.get::<bevy::a11y::AccessibilityNode>().unwrap();
    assert_eq!(accessible.is_hidden(), !enabled);
    assert_eq!(accessible.is_disabled(), !enabled);
}

#[test]
fn every_off_main_screen_hides_disables_and_removes_the_cog_hit_surface() {
    let (mut app, cog) = fixture();
    for screen in off_main_screens() {
        scope(&mut app, Screen::Main);
        app.world_mut().entity_mut(cog).insert(Pressed);
        app.insert_resource(InputFocus::from_entity(cog));
        scope(&mut app, screen);
        assert_scope(app.world(), cog, false);
        assert!(!app.world().entity(cog).contains::<Pressed>());
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
    }
}

#[test]
fn fresh_main_and_return_restore_scope_without_stealing_other_focus() {
    let (mut app, cog) = fixture();
    let other = app.world_mut().spawn_empty().id();
    app.insert_resource(InputFocus::from_entity(other));
    scope(&mut app, Screen::Main);
    assert_scope(app.world(), cog, true);
    for screen in off_main_screens() {
        scope(&mut app, screen);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(other));
        scope(&mut app, Screen::Main);
        assert_scope(app.world(), cog, true);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(other));
    }
}

#[test]
fn scope_is_safe_without_an_input_focus_resource() {
    let (mut app, cog) = fixture();
    assert!(!app.world().contains_resource::<InputFocus>());
    for screen in off_main_screens() {
        scope(&mut app, screen);
        assert_scope(app.world(), cog, false);
    }
    scope(&mut app, Screen::Main);
    assert_scope(app.world(), cog, true);
    assert!(!app.world().contains_resource::<InputFocus>());
}

// Trigger real widget input/observers; do not bypass ButtonPlugin for keyboard.
fn send_activation(world: &mut World, cog: Entity, source: usize) {
    match source {
        0 | 1 => {
            let window = world.spawn_empty().id();
            let key_code = if source == 0 {
                KeyCode::Enter
            } else {
                KeyCode::Space
            };
            let logical_key = if source == 0 { Key::Enter } else { Key::Space };
            world.trigger(FocusedInput::new(
                cog,
                KeyboardInput {
                    key_code,
                    logical_key,
                    state: ButtonState::Pressed,
                    text: None,
                    repeat: false,
                    window,
                },
                window,
            ));
        }
        2 => world.trigger(Activate { entity: cog }),
        3 => world.trigger(PointerClick {
            entity: cog,
            pointer: Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: 1,
                        height: 1,
                    },
                    position: Vec2::ZERO,
                },
            ),
            button: PointerButton::Primary,
            hit: HitData::new(cog, 0.0, None, None),
            duration: std::time::Duration::ZERO,
            count: 1,
        }),
        _ => unreachable!(),
    }
    world.flush();
}

#[test]
fn actual_keyboard_activate_and_click_paths_enqueue_only_on_main() {
    let (mut app, cog) = fixture();
    for screen in off_main_screens() {
        scope(&mut app, screen);
        for source in 0..4 {
            send_activation(app.world_mut(), cog, source);
            assert!(
                app.world()
                    .resource::<button_actions::ButtonActions>()
                    .entities
                    .is_empty(),
                "{screen:?}, source {source}"
            );
        }
        scope(&mut app, Screen::Main);
        for source in 0..4 {
            send_activation(app.world_mut(), cog, source);
            assert_eq!(
                app.world()
                    .resource::<button_actions::ButtonActions>()
                    .entities,
                [cog],
                "source {source}"
            );
            app.world_mut()
                .run_system_once(button_actions::clear)
                .unwrap();
        }
    }
}
