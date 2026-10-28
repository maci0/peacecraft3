//! Left-button drag-box selection and double-click group selection.
//!
//! A left press that no armed command claims starts a gesture; the plain click
//! it ends as is handed to [`crate::campaign_input`], which selects as before. A
//! swept rectangle is outlined in `[SelectionCircle] ColorFriend` and, on
//! release, selects the units whose projected selection circle it touches. The
//! pick rules, the selection cap and the double-click window come from
//! [`pcx_play::selection_box`]; evidence is in `docs/re-notes/campaign-box-selection.md`.
use std::sync::Weak;

use bevy::{picking::Pickable, prelude::*, window::PrimaryWindow};
use pcx_play::{
    map_startup_host::MapStartupHost,
    player_command_log::PlayerCommand,
    playercolor_host::{ObjectHost, VisionState},
    scripts::AssetIdentity,
    selection_box::{
        Candidate, Finish, Gesture, Parameters, Pick, Rect, add_to_selection, pick_box,
    },
};
use pcx_script::HandleId;
use peacecraft3::map_script_session::MapScriptSession;

use crate::campaign_input::{Buttons, CampaignInput, Picking};

/// Points sampled around a selection circle when projecting it to the screen
/// (numerical method only).
const FOOTPRINT_SAMPLES: usize = 16;
/// Stroke width of the outline in logical pixels; the original draws a one pixel
/// line at 1024x768 (`campaign-box-selection.md`, observation).
const OUTLINE_PX: f32 = 1.0;
/// Between the minimap root (10) and the world.
const OUTLINE_Z: i32 = 9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Click {
    pub at: [f32; 2],
    pub additive: bool,
    pub double: bool,
}

#[derive(Resource, Default)]
pub(crate) struct BoxSelect {
    gesture: Gesture,
    parameters: Option<Result<Parameters, String>>,
    session: Option<Weak<()>>,
    finished: Option<Click>,
    outline: Option<Entity>,
}

impl BoxSelect {
    /// Rules are admitted, so a left press starts a gesture rather than selecting at once.
    pub fn ready(&self) -> bool {
        matches!(self.parameters, Some(Ok(_)))
    }

    pub fn take_click(&mut self) -> Option<Click> {
        self.finished.take()
    }

    pub fn cap(&self) -> Option<usize> {
        match &self.parameters {
            Some(Ok(parameters)) => Some(parameters.selection_cap),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn with_parameters(parameters: Parameters) -> Self {
        Self {
            parameters: Some(Ok(parameters)),
            ..default()
        }
    }
}

#[derive(Component)]
pub(crate) struct Outline;

/// Runs inside the input chain of [`crate::campaign_input`], after the minimap
/// has claimed or released the pointer and before a click selects.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct BoxSelectInputSet;

pub(crate) fn install(app: &mut App) {
    app.init_resource::<BoxSelect>()
        .add_systems(Update, draw_outline.after(BoxSelectInputSet));
}

pub(crate) fn load_parameters(
    mut state: ResMut<BoxSelect>,
    mut input: ResMut<CampaignInput>,
    session: Res<MapScriptSession<MapStartupHost>>,
    launcher: Option<Res<crate::launch_gui::Launcher>>,
) {
    if state.parameters.is_some() || session.context().is_none() {
        return;
    }
    let install = launcher
        .as_deref()
        .and_then(crate::launch_gui::selected_install);
    let loaded = crate::menu_lighting::with_client_image(install.as_deref(), |source, image| {
        Parameters::read(
            image,
            AssetIdentity {
                source: source.into(),
                edition: "selected campaign client".into(),
                version: "reviewed Legacy 1.29.2.9231 selection image profile".into(),
            },
        )
    });
    if let Err(error) = &loaded {
        // Without the rules a press selects as a plain click; no box is offered.
        input.last_error = Some(format!("drag-box selection unavailable: {error}"));
    }
    state.parameters = Some(loaded);
}

/// Everything that must let a press reach the session. The press position is the
/// cursor, which must lie in the scene viewport.
fn pointer_open(picking: &Picking, hud: &crate::campaign_hud::CampaignHud, pressed: bool) -> bool {
    picking.game_input.open()
        && !(pressed && (hud.pointer_consumed || picking.inventory.pointer_consumed))
}

pub(crate) fn gesture_input(
    buttons: Buttons,
    time: Res<Time<Real>>,
    picking: Picking,
    hud: Res<crate::campaign_hud::CampaignHud>,
    mut state: ResMut<BoxSelect>,
    mut input: ResMut<CampaignInput>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    input.escape_cancelled_drag = false;
    // A click not taken by `click` in its own frame (no selection profile, say) is stale.
    state.finished = None;
    let witness = session.session_witness();
    if !match (&state.session, &witness) {
        (Some(old), Some(new)) => Weak::ptr_eq(old, new),
        (None, None) => true,
        _ => false,
    } {
        state.session = witness;
        state.gesture.cancel();
        state.finished = None;
    }
    let (Some(mouse), Some(keys)) = (buttons.mouse, buttons.keys) else {
        return;
    };
    let (Ok(window), Ok((camera, transform, _))) =
        (picking.windows.single(), picking.cameras.single())
    else {
        state.gesture.cancel();
        return;
    };
    let Some(viewport) = camera.logical_viewport_rect() else {
        state.gesture.cancel();
        return;
    };
    let armed = input.command.is_some() || input.build.is_some();
    if state.gesture.is_active() && (armed || !pointer_open(&picking, &hud, false)) {
        state.gesture.cancel();
    }
    if state.gesture.is_active() && keys.just_pressed(KeyCode::Escape) {
        state.gesture.cancel();
        input.escape_cancelled_drag = true;
    }
    let additive = picking.game_input.shift.held();
    if mouse.just_pressed(MouseButton::Left)
        && !armed
        && state.ready()
        && let Some(cursor) = window.cursor_position()
        && viewport.contains(cursor)
        && pointer_open(&picking, &hud, true)
        && let Some(Ok(parameters)) = state.parameters.clone()
    {
        let now_ms = time.elapsed().as_secs_f64() * 1000.0;
        state.gesture.press(
            cursor.to_array(),
            additive,
            now_ms,
            viewport.height(),
            &parameters,
        );
    }
    if state.gesture.is_active() {
        if let Some(cursor) = window.cursor_position() {
            state
                .gesture
                .track(cursor.clamp(viewport.min, viewport.max).to_array());
        }
        if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
            let cursor = window
                .cursor_position()
                .map_or(viewport.center(), |c| c.clamp(viewport.min, viewport.max));
            match state.gesture.release(cursor.to_array()) {
                Some(Finish::Click {
                    at,
                    additive,
                    double,
                }) => {
                    state.finished = Some(Click {
                        at,
                        additive,
                        double,
                    });
                }
                Some(Finish::Box { rect, additive }) => {
                    let result = match picking.ground.context.as_ref() {
                        Some(context) => select_box(
                            &Projector {
                                terrain: &context.terrain,
                                camera,
                                transform,
                            },
                            &mut input,
                            &mut session,
                            &state,
                            rect,
                            additive,
                        ),
                        None => Err("box selection has no published terrain".into()),
                    };
                    if let Err(error) = result {
                        input.last_error = Some(error);
                    }
                }
                None => {}
            }
        }
    }
}

/// Projects one unit's selection circle to the screen through `project`.
fn footprint(
    centre: [f32; 2],
    radius: f32,
    height: impl Fn([f32; 2]) -> Option<f32>,
    project: impl Fn(Vec3) -> Option<Vec2>,
) -> Option<Rect> {
    let mut bounds: Option<Rect> = None;
    for step in 0..FOOTPRINT_SAMPLES {
        let angle = step as f32 / FOOTPRINT_SAMPLES as f32 * std::f32::consts::TAU;
        let point = [
            centre[0] + radius * angle.cos(),
            centre[1] + radius * angle.sin(),
        ];
        let Some(z) = height(point) else { continue };
        let Some(screen) = project(Vec3::new(point[0], z, -point[1])) else {
            continue;
        };
        let here = Rect {
            min: screen.to_array(),
            max: screen.to_array(),
        };
        bounds = Some(match bounds {
            None => here,
            Some(b) => Rect {
                min: [b.min[0].min(here.min[0]), b.min[1].min(here.min[1])],
                max: [b.max[0].max(here.max[0]), b.max[1].max(here.max[1])],
            },
        });
    }
    bounds
}

/// The scene camera and the terrain its picks stand on.
pub(crate) struct Projector<'a> {
    pub terrain: &'a pcx_map::Terrain,
    pub camera: &'a Camera,
    pub transform: &'a GlobalTransform,
}

/// Candidates whose projected selection circle touches `region`, in unit creation
/// order, each with its unit type rawcode.
fn hits(
    view: &Projector,
    input: &mut CampaignInput,
    objects: &ObjectHost,
    region: Rect,
) -> Result<Vec<(Candidate, [u8; 4])>, String> {
    let circles = input
        .circles
        .clone()
        .ok_or("box selection has no SelectionCircle profile")?;
    let sight = crate::campaign_input::pick_sight(objects, input)
        .map_err(|()| "box selection has no vision ledger".to_string())?;
    let mut found = Vec::new();
    for (id, unit) in objects.executed_units() {
        let Some(candidate) = objects.box_candidate(id)? else {
            continue;
        };
        let Some([x, y]) = unit.position else {
            continue;
        };
        if let Some((viewer, fog, mask)) = sight {
            match objects
                .vision_presented(viewer, [x, y], fog, mask)
                .map_err(|e| e.to_string())?
            {
                Some(VisionState::Visible) => {}
                _ => continue,
            }
        }
        let scale = crate::campaign_actors::scalar(&unit.definition, "scale")?;
        let radius = scale * circles.scale_factor * 0.5;
        let shape = footprint(
            [x, y],
            radius,
            |[px, py]| pcx_tera::world::sample_world_z(view.terrain, px, py),
            |world| view.camera.world_to_viewport(view.transform, world).ok(),
        );
        if shape.is_some_and(|shape| shape.intersects(&region)) {
            found.push((candidate, unit.definition.rawcode));
        }
    }
    Ok(found)
}

fn select_box(
    view: &Projector,
    input: &mut CampaignInput,
    session: &mut MapScriptSession<MapStartupHost>,
    state: &BoxSelect,
    rect: Rect,
    additive: bool,
) -> Result<(), String> {
    let cap = state.cap().ok_or("box selection rules are not admitted")?;
    let Some(host) = session.host() else {
        return Ok(());
    };
    let objects = host.objects_mut();
    let found: Vec<Candidate> = hits(view, input, objects, rect)?
        .into_iter()
        .map(|(candidate, _)| candidate)
        .collect();
    let Some(pick) = pick_box(&found, cap).map_err(|e| e.to_string())? else {
        return Ok(());
    };
    apply(objects, input, pick, additive, cap)
}

/// Selects every own unit of the clicked unit's type inside the viewport.
pub(crate) fn double_click_group(
    view: &Projector,
    input: &mut CampaignInput,
    objects: &mut ObjectHost,
    clicked: HandleId,
    cap: usize,
    additive: bool,
) -> Result<(), String> {
    let viewport = view
        .camera
        .logical_viewport_rect()
        .ok_or("double click has no scene viewport")?;
    let region = Rect {
        min: viewport.min.to_array(),
        max: viewport.max.to_array(),
    };
    let found = hits(view, input, objects, region)?;
    let Some(pick) = pcx_play::selection_box::same_type(&found, clicked, cap) else {
        return Ok(());
    };
    apply(objects, input, pick, additive, cap)
}

fn apply(
    objects: &mut ObjectHost,
    input: &mut CampaignInput,
    pick: Pick,
    additive: bool,
    cap: usize,
) -> Result<(), String> {
    let pick = if additive {
        let current: Vec<Candidate> = objects
            .selection()
            .iter()
            .filter_map(|&id| objects.box_candidate(id).transpose())
            .collect::<Result<_, _>>()?;
        add_to_selection(&current, &pick, cap).map_err(|e| e.to_string())?
    } else {
        pick
    };
    let units = pick.ids();
    objects.select_units(&units, true);
    let clock = objects.timer_game_time();
    crate::campaign_input::record_command(
        input,
        clock,
        PlayerCommand::Select {
            units,
            replace: true,
        },
    );
    if pick.truncated {
        input.last_error = Some(format!(
            "more than {cap} selectable units; the original's choice among them is not \
             established, kept the first {cap} in unit creation order"
        ));
    }
    Ok(())
}

pub(crate) fn draw_outline(
    mut commands: Commands,
    mut state: ResMut<BoxSelect>,
    input: Res<CampaignInput>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut nodes: Query<(&mut Node, &mut BorderColor), With<Outline>>,
) {
    let rect = state.gesture.rect();
    let (Some(rect), Ok(window), Some(circles)) = (rect, windows.single(), input.circles.as_ref())
    else {
        if let Some(entity) = state.outline.take() {
            commands.entity(entity).try_despawn();
        }
        return;
    };
    let [a, r, g, b] = circles.friend;
    let colour = Color::srgba_u8(r, g, b, a);
    let (w, h) = (window.width(), window.height());
    let place = |node: &mut Node| {
        node.position_type = PositionType::Absolute;
        node.left = Val::Percent(rect.min[0] / w * 100.0);
        node.top = Val::Percent(rect.min[1] / h * 100.0);
        node.width = Val::Percent((rect.max[0] - rect.min[0]) / w * 100.0);
        node.height = Val::Percent((rect.max[1] - rect.min[1]) / h * 100.0);
        node.border = UiRect::all(Val::Px(OUTLINE_PX));
    };
    if let Some(entity) = state.outline
        && let Ok((mut node, mut border)) = nodes.get_mut(entity)
    {
        place(&mut node);
        *border = BorderColor::all(colour);
        return;
    }
    let mut node = Node::default();
    place(&mut node);
    let entity = commands
        .spawn((
            node,
            BorderColor::all(colour),
            GlobalZIndex(OUTLINE_Z),
            peacecraft3::campaign_presentation::CampaignHud,
            peacecraft3::campaign_presentation::CampaignInterface,
            Pickable::IGNORE,
            Outline,
            Name::new("Selection rectangle"),
        ))
        .id();
    state.outline = Some(entity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footprint_spans_the_projected_circle_and_skips_off_terrain_samples() {
        let rect = footprint(
            [10.0, 20.0],
            4.0,
            |_| Some(0.0),
            |world| Some(Vec2::new(world.x * 2.0, -world.z * 2.0)),
        )
        .unwrap();
        assert_eq!(rect.min, [12.0, 32.0]);
        assert_eq!(rect.max, [28.0, 48.0]);
        // Off-terrain or behind-camera samples shrink the footprint; none leaves no shape.
        let half = footprint(
            [10.0, 20.0],
            4.0,
            |[x, _]| (x >= 10.0).then_some(0.0),
            |world| Some(Vec2::new(world.x, -world.z)),
        )
        .unwrap();
        assert!(half.min[0] >= 10.0 - f32::EPSILON * 16.0);
        assert!(footprint([0.0, 0.0], 1.0, |_| None, |_| None).is_none());
        assert!(footprint([0.0, 0.0], 1.0, |_| Some(0.0), |_| None).is_none());
    }
}
