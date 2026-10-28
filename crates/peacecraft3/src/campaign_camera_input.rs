//! Player camera input on the published campaign session: six-level zoom on the mouse
//! wheel and Page Up/Down, Insert/Delete rotation states, arrow-key, screen-edge and
//! middle-button scrolling, and the Space, Backspace, F8 and backtick jump keys.
//!
//! The tables, ease times, wheel unit, repeat window and scroll multipliers are read
//! from the client image by [`pcx_play::camera_input`]; the screen-edge margin is an
//! observation (`EDGE_MARGIN_UI`). Evidence labels and the decisions taken where the
//! original is not established are in `docs/re-notes/input-camera.md`. Everything is
//! off while scripts disable user control, a menu, dialog or the quest log is open.
use std::{collections::VecDeque, sync::Weak};

use bevy::{
    input::{
        ButtonState,
        keyboard::KeyboardInput,
        mouse::{MouseScrollUnit, MouseWheel},
    },
    prelude::*,
};
use pcx_play::{
    camera_input::{CameraInputRules, Cycle, Ease, Swing, View, WHEEL_NOTCH, edge, edge_mask},
    map_startup_host::MapStartupHost,
    player_command_log::PlayerCommand,
    playercolor_host::ObjectHost,
    scripts::AssetIdentity,
};
use pcx_script::{HandleId, Value};
use peacecraft3::{
    campaign_camera::{CampaignCamera, CampaignCameraSet, scroll_to},
    map_script_session::MapScriptSession,
};

use crate::campaign_input::{
    Buttons, CampaignInput, Picking, commands_available, ground_point, record_command,
};

/// Alerts the Space key can step back through. Project policy: the capacity of the
/// client's history was not read.
const ALERT_HISTORY_LIMIT: usize = 32;

/// Camera points of past alerts, oldest first. Nothing in the recreation raises
/// alerts yet, so Space has an empty history until a producer calls [`Self::push`].
#[derive(Default)]
pub(crate) struct AlertHistory {
    points: VecDeque<[f32; 2]>,
}

impl AlertHistory {
    #[allow(
        dead_code,
        reason = "no alert producer exists yet; Space reads this history"
    )]
    pub(crate) fn push(&mut self, at: [f32; 2]) {
        if self.points.len() == ALERT_HISTORY_LIMIT {
            self.points.pop_front();
        }
        self.points.push_back(at);
    }
}

#[derive(Resource, Default)]
pub(crate) struct CameraInput {
    /// Rules of the selected client; an error keeps zoom, edge scroll and jumps off.
    rules: Option<Result<CameraInputRules, String>>,
    session: Option<Weak<()>>,
    view: View,
    /// Wheel units not yet worth a level, so fine-grained wheels still zoom.
    wheel: i32,
    space: Cycle,
    backspace: Cycle,
    idle: Cycle,
    /// Ground point held by a middle-button drag.
    grab: Option<[f32; 2]>,
    pub alerts: AlertHistory,
    pub last_error: Option<String>,
}

#[cfg(test)]
impl CameraInput {
    pub(crate) fn with_rules(rules: CameraInputRules) -> Self {
        Self {
            rules: Some(Ok(rules)),
            ..default()
        }
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CameraInput>().add_systems(
        Update,
        (session_state, load_rules, camera_keys, scroll)
            .chain()
            .before(CampaignCameraSet),
    );
}

/// A new session starts at the camera's configured zoom level with no history.
fn session_state(
    mut state: ResMut<CameraInput>,
    session: Res<MapScriptSession<MapStartupHost>>,
    camera: Res<CampaignCamera>,
) {
    let witness = session.session_witness();
    if match (&state.session, &witness) {
        (Some(old), Some(new)) => Weak::ptr_eq(old, new),
        (None, None) => true,
        _ => false,
    } {
        return;
    }
    let rules = state.rules.take();
    *state = CameraInput {
        rules,
        session: witness,
        view: View {
            level: camera.zoom(),
            swing: Swing::Normal,
        },
        ..default()
    };
    clamp_level(&mut state);
}

fn clamp_level(state: &mut CameraInput) {
    if let Some(Ok(rules)) = &state.rules {
        state.view.level = state.view.level.min(rules.levels() - 1);
    }
}

fn load_rules(
    mut state: ResMut<CameraInput>,
    session: Res<MapScriptSession<MapStartupHost>>,
    launcher: Option<Res<crate::launch_gui::Launcher>>,
) {
    if state.rules.is_some() || session.context().is_none() {
        return;
    }
    let install = launcher
        .as_deref()
        .and_then(crate::launch_gui::selected_install);
    let loaded = crate::menu_lighting::with_client_image(install.as_deref(), |source, image| {
        CameraInputRules::read(
            image,
            AssetIdentity {
                source: source.into(),
                edition: "selected campaign client".into(),
                version: "reviewed Legacy 1.29.2.9231 camera input profile".into(),
            },
        )
    });
    if let Err(error) = &loaded {
        // Without the rules arrow keys keep the [CameraRates] speed; nothing else is offered.
        state.last_error = Some(format!("camera input unavailable: {error}"));
    }
    state.rules = Some(loaded);
    clamp_level(&mut state);
}

fn apply(objects: &mut ObjectHost, ease: Option<Ease>, error: &mut Option<String>) {
    if let Some(ease) = ease
        && let Err(e) = objects.ease_player_camera(ease.pose, ease.seconds)
    {
        *error = Some(e.to_string());
    }
}

fn centre(objects: &mut ObjectHost, at: [f32; 2], error: &mut Option<String>) {
    if let Err(e) = scroll_to(objects, at) {
        *error = Some(e);
    }
}

/// Wheel, Page Up/Down, Insert/Delete and the jump keys, in message order.
#[allow(
    clippy::too_many_arguments,
    reason = "one Bevy system over the input messages and session"
)]
fn camera_keys(
    mut keyboard: MessageReader<KeyboardInput>,
    mut wheel: MessageReader<MouseWheel>,
    view: Res<CampaignCamera>,
    hud: Res<crate::campaign_hud::CampaignHud>,
    dialogs: Res<crate::campaign_script_dialogs::CampaignScriptDialogs>,
    time: Res<Time<Real>>,
    mut state: ResMut<CameraInput>,
    mut input: ResMut<CampaignInput>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let keys: Vec<KeyboardInput> = keyboard.read().cloned().collect();
    let wheels: Vec<MouseWheel> = wheel.read().cloned().collect();
    let Some(host) = session.host() else {
        return;
    };
    let open = commands_available(&view, &hud, &dialogs, host);
    let objects = host.objects_mut();
    let state = &mut *state;
    let Some(Ok(rules)) = &state.rules else {
        return;
    };
    let now = time.elapsed_secs_f64();
    let view = &mut state.view;
    let error = &mut state.last_error;
    for event in wheels {
        let delta = match event.unit {
            MouseScrollUnit::Line => event.y * WHEEL_NOTCH as f32,
            MouseScrollUnit::Pixel => event.y,
        } as i32;
        if state.wheel.signum() != delta.signum() {
            state.wheel = 0;
        }
        state.wheel += delta;
        if !open {
            state.wheel = 0;
        } else if let Some(ease) = rules.wheel(view, state.wheel) {
            apply(objects, Some(ease), error);
            state.wheel = 0;
        }
    }
    for event in keys {
        let down = event.state == ButtonState::Pressed;
        match (event.key_code, down) {
            (KeyCode::PageUp, true) if open => apply(objects, rules.zoom_in(view), error),
            (KeyCode::PageDown, true) if open => apply(objects, rules.zoom_out(view), error),
            (KeyCode::Insert, true) if open => {
                apply(objects, rules.swing_press(view, Swing::Insert), error)
            }
            (KeyCode::Delete, true) if open => {
                apply(objects, rules.swing_press(view, Swing::Delete), error)
            }
            // A release always ends its state, so a control lock cannot strand the camera.
            (KeyCode::Insert | KeyCode::Numpad0, false) => {
                apply(objects, rules.swing_release(view, Swing::Insert), error)
            }
            (KeyCode::Delete | KeyCode::NumpadDecimal, false) => {
                apply(objects, rules.swing_release(view, Swing::Delete), error)
            }
            (KeyCode::Space, true) if open => {
                let points = &state.alerts.points;
                let newest = points.len().saturating_sub(1);
                if let Some(index) =
                    state
                        .space
                        .previous(now, rules.repeat_window, points.len(), newest)
                {
                    centre(objects, points[index], error);
                }
            }
            (KeyCode::Backspace, true) if open => {
                jump_to_hall(objects, &mut state.backspace, rules, now, error)
            }
            (KeyCode::F8 | KeyCode::Backquote, true) if open && !event.repeat => {
                match objects
                    .local_player_number()
                    .map(|owner| objects.idle_workers(owner))
                {
                    Some(Ok(idle)) => jump_to_idle(
                        objects,
                        &mut state.idle,
                        &mut input,
                        rules.repeat_window,
                        now,
                        &idle,
                        error,
                    ),
                    Some(Err(e)) => *error = Some(format!("idle workers: {e}")),
                    None => {}
                }
            }
            _ => {}
        }
    }
}

/// Backspace: the own Town Halls in creation order; a repeat inside the window moves to
/// the next, otherwise the first.
fn jump_to_hall(
    objects: &mut ObjectHost,
    cycle: &mut Cycle,
    rules: &CameraInputRules,
    now: f64,
    error: &mut Option<String>,
) {
    let Some(owner) = objects.local_player_number() else {
        return;
    };
    match objects.town_halls(owner) {
        Ok(halls) => {
            if let Some(index) = cycle.next(now, rules.repeat_window, halls.len()) {
                centre(objects, halls[index].1, error);
            }
        }
        Err(e) => *error = Some(format!("town halls: {e}")),
    }
}

/// F8 and backtick: select the next idle worker and centre on it. Without idle workers
/// nothing changes and the cycle is not advanced.
fn jump_to_idle(
    objects: &mut ObjectHost,
    cycle: &mut Cycle,
    input: &mut CampaignInput,
    window: f32,
    now: f64,
    idle: &[HandleId],
    error: &mut Option<String>,
) {
    let Some(index) = cycle.next(now, window, idle.len()) else {
        return;
    };
    let unit = idle[index];
    objects.select_units(&[unit], true);
    let clock = objects.timer_game_time();
    record_command(
        input,
        clock,
        PlayerCommand::Select {
            units: vec![unit],
            replace: true,
        },
    );
    match objects.unit_camera_point(unit) {
        Ok(at) => centre(objects, at, error),
        Err(e) => *error = Some(e),
    }
}

/// Scroll direction strengths: a held arrow key scrolls at the key multiplier, the
/// pointer at a screen edge at the mouse multiplier. Both share one slot per direction
/// in the client; a held key wins here.
fn directions(keys: &ButtonInput<KeyCode>, edges: u8, key: f32, mouse: f32) -> [f32; 4] {
    [
        (KeyCode::ArrowUp, edge::UP),
        (KeyCode::ArrowDown, edge::DOWN),
        (KeyCode::ArrowLeft, edge::LEFT),
        (KeyCode::ArrowRight, edge::RIGHT),
    ]
    .map(|(arrow, bit)| {
        if keys.pressed(arrow) {
            key
        } else if edges & bit != 0 {
            mouse
        } else {
            0.0
        }
    })
}

/// Scrolls along the live camera rotation at the `[CameraRates]` speeds scaled by the
/// option multipliers, and drags the ground under a held middle button. Clamped to the
/// executed camera bounds. Real time paces it.
fn scroll(
    buttons: Buttons,
    picking: Picking,
    hud: Res<crate::campaign_hud::CampaignHud>,
    time: Res<Time<Real>>,
    mut state: ResMut<CameraInput>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let (Some(keys), Some(mouse)) = (buttons.keys.as_deref(), buttons.mouse.as_deref()) else {
        return;
    };
    let rotation = session.global("CAMERA_FIELD_ROTATION");
    if session.host().is_none() {
        return;
    }
    if !picking.game_input.open() {
        state.grab = None;
        return;
    }
    let window = picking.windows.single().ok();
    let cursor = window
        .filter(|w| w.focused)
        .and_then(|w| Some((w.cursor_position()?, w.size())));
    let (key, mouse_speed, edges_on) = match &state.rules {
        Some(Ok(rules)) => (
            rules.scroll.multiplier(rules.fallback.key),
            rules.scroll.multiplier(rules.fallback.mouse),
            rules.fallback.mouse_disable == 0,
        ),
        _ => (1.0, 1.0, false),
    };
    let edges = match cursor {
        Some((at, size)) if edges_on => edge_mask(at.to_array(), size.to_array()),
        _ => 0,
    };
    let [up, down, left, right] = directions(keys, edges, key, mouse_speed);
    let (forward, strafe) = (up - down, right - left);
    if !mouse.pressed(MouseButton::Middle) {
        state.grab = None;
    }
    let ray = |at: Vec2| {
        let (camera, transform, projection) = picking.cameras.single().ok()?;
        let ray = camera.viewport_to_world(transform, at).ok()?;
        let terrain = &picking.ground.context.as_ref()?.terrain;
        ground_point(ray, terrain, projection.far())
    };
    if mouse.just_pressed(MouseButton::Middle)
        && let Some((at, _)) = cursor
        && picking
            .cameras
            .single()
            .ok()
            .and_then(|(camera, ..)| camera.logical_viewport_rect())
            .is_some_and(|rect| rect.contains(at))
        && crate::campaign_box_select::pointer_open(&picking, &hud, true)
    {
        state.grab = ray(at);
    }
    let grabbed = state
        .grab
        .zip(cursor)
        .and_then(|(grab, (at, _))| Some((grab, ray(at)?)));
    if forward == 0.0 && strafe == 0.0 && grabbed.is_none() {
        return;
    }
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects_mut();
    let (Some([x, y]), Some([forward_rate, strafe_rate])) =
        (objects.camera_target(), objects.camera_scroll_rates())
    else {
        return;
    };
    let angle = match rotation {
        Some(Value::Handle(field)) => objects.camera_field(field).ok().flatten(),
        _ => None,
    };
    let Some(angle) = angle else {
        return;
    };
    let (sin, cos) = angle.to_radians().sin_cos();
    let dt = time.delta_secs();
    let mut target = [
        x + (cos * forward * forward_rate + sin * strafe * strafe_rate) * dt,
        y + (sin * forward * forward_rate - cos * strafe * strafe_rate) * dt,
    ];
    if let Some((grab, under)) = grabbed {
        // Move so the grabbed ground point comes back under the pointer.
        target[0] += grab[0] - under[0];
        target[1] += grab[1] - under[1];
    }
    if let Err(e) = scroll_to(objects, target) {
        state.last_error = Some(e);
    }
}

#[cfg(test)]
#[path = "campaign_camera_input_tests.rs"]
mod tests;
