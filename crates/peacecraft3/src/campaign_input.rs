//! Player selection and orders on the published campaign session.
//!
//! Left click selects the nearest live unit whose source selection circle
//! (UnitUI `scale` times `[SelectionCircle] ScaleFactor`) covers the clicked
//! ground point; Shift adds. Right click attacks an enemy under the cursor or
//! moves to the ground point, for selected units the local player owns. Orders go
//! through the same order natives scripts use. Input is off while scripts disable
//! user control. Escape raises the local player's end-cinematic event.
use std::sync::{Arc, Weak};

use bevy::{ecs::system::SystemParam, prelude::*, window::PrimaryWindow};
use pcx_objects::editor::selection::SelectionCircles;
use pcx_play::map_startup_host::MapStartupHost;
use pcx_script::{HandleId, Host, Value};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet, CampaignSceneCamera},
    map_ground::GroundPublication,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};

const MISC_DATA: &str = "UI\\MiscData.txt";
const END_CINEMATIC: &str = "EVENT_PLAYER_END_CINEMATIC";
/// Bound on selection circle bands read from the profile.
const MAX_BANDS: usize = 16;
/// Ray-march stride and bisection count for cursor picking (numerical method only).
const MARCH_STEP: f32 = 16.0;
const REFINE_STEPS: usize = 24;

#[derive(Resource, Default)]
pub(crate) struct CampaignInput {
    pub circles: Option<SelectionCircles>,
    attempted: bool,
    session: Option<Weak<()>>,
    pub last_error: Option<String>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignInput>()
        .add_systems(
            Update,
            (
                (load_circles, click)
                    .chain()
                    .after(crate::campaign_hud::CampaignHudSet)
                    .after(crate::campaign_inventory::CampaignInventorySet)
                    .after(crate::campaign_script_dialogs::CampaignScriptDialogSet),
                end_cinematic
                    .after(crate::campaign_hud::CampaignHudSet)
                    .after(crate::campaign_script_dialogs::CampaignScriptDialogSet),
            ),
        )
        .add_systems(Update, hud_menu.before(CampaignCameraSet));
}

fn hud_menu(
    mut actions: MessageReader<crate::campaign_hud::CampaignHudAction>,
    mut quits: MessageReader<crate::campaign_script_dialogs::CampaignDialogQuit>,
    mut view: ResMut<CampaignCamera>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    for quit in quits.read() {
        if !session
            .session_witness()
            .is_some_and(|current| Weak::ptr_eq(&current, &quit.session))
        {
            continue;
        }
        let local = session
            .host()
            .and_then(|host| host.objects().local_player_settings())
            .map(|player| player.identity);
        if local != Some(quit.request.player) {
            continue;
        }
        if quit.request.score_screen {
            warn!(
                "script dialog quit requested a score screen; score presentation is not implemented"
            );
        }
        session.clear();
        view.suspend();
    }
    if actions
        .read()
        .any(|action| matches!(action, crate::campaign_hud::CampaignHudAction::Menu))
    {
        view.suspend();
    }
}

fn load_circles(
    mut input: ResMut<CampaignInput>,
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    session: Res<MapScriptSession<MapStartupHost>>,
) {
    let witness = session.session_witness();
    if !match (&input.session, &witness) {
        (Some(old), Some(new)) => Weak::ptr_eq(old, new),
        (None, None) => true,
        _ => false,
    } {
        *input = CampaignInput {
            session: witness,
            ..default()
        };
    }
    if input.attempted || session.context().is_none() {
        return;
    }
    let Some(bindings) = requests.bindings.as_ref() else {
        return;
    };
    if !requests
        .admission
        .shared_context()
        .as_ref()
        .zip(session.context())
        .is_some_and(|(admitted, active)| Arc::ptr_eq(admitted, active))
        || !requests
            .published_receipts
            .as_ref()
            .is_some_and(|published| Arc::ptr_eq(published, &bindings.receipts))
    {
        return;
    }
    let Some(reader) = sources.for_receipts(&bindings.receipts) else {
        return;
    };
    input.attempted = true;
    match reader
        .resolve_bounded(MISC_DATA, 2 << 20)
        .map_err(|e| e.to_string())
        .and_then(|asset| SelectionCircles::read(&asset.bytes, MISC_DATA, MAX_BANDS))
    {
        Ok(circles) => input.circles = Some(circles),
        Err(error) => input.last_error = Some(format!("{MISC_DATA}: {error}")),
    }
}

/// Escape raises the local player's `EVENT_PLAYER_END_CINEMATIC`; the original
/// raises it on every Escape press, not only during cinematics.
fn end_cinematic(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    view: Res<CampaignCamera>,
    hud: Res<crate::campaign_hud::CampaignHud>,
    dialogs: Res<crate::campaign_script_dialogs::CampaignScriptDialogs>,
    mut input: ResMut<CampaignInput>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    if !keys.is_some_and(|k| k.just_pressed(KeyCode::Escape))
        || !view.active()
        || hud.quest_log_open
        || hud.quest_escape_consumed
        || dialogs.input_blocked()
    {
        return;
    }
    let Some(host) = session.host() else {
        return;
    };
    match host.objects_mut().fire_local_player_event(END_CINEMATIC) {
        Ok(fired) => debug!("{END_CINEMATIC}: {fired} registrations queued"),
        Err(error) => input.last_error = Some(error.to_string()),
    }
}

/// Ground point under the cursor, in game coordinates (x, y), by marching the
/// view ray down to the sampled terrain surface.
fn ground_point(ray: Ray3d, terrain: &pcx_map::Terrain, far: f32) -> Option<[f32; 2]> {
    let height = |p: Vec3| pcx_tera::world::sample_world_z(terrain, p.x, -p.z).map(|z| p.y - z);
    if !far.is_finite() || far <= 0.0 {
        return None;
    }
    let mut previous = None;
    let mut t = MARCH_STEP;
    while t <= far {
        if let Some(above) = height(ray.get_point(t)) {
            if above <= 0.0 {
                // Only bracket a surface crossing with two admitted samples.
                let (mut low, mut high) = (previous?, t);
                for _ in 0..REFINE_STEPS {
                    let mid = (low + high) * 0.5;
                    if height(ray.get_point(mid))? > 0.0 {
                        low = mid;
                    } else {
                        high = mid;
                    }
                }
                let hit = ray.get_point(high);
                return Some([hit.x, -hit.z]);
            }
            previous = Some(t);
        } else {
            previous = None;
        }
        let next = t + MARCH_STEP;
        if next <= t {
            return None;
        }
        t = next;
    }
    None
}

#[derive(SystemParam)]
struct Picking<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    cameras: Query<
        'w,
        's,
        (
            &'static Camera,
            &'static GlobalTransform,
            &'static Projection,
        ),
        With<CampaignSceneCamera>,
    >,
    view: Res<'w, CampaignCamera>,
    ground: Res<'w, GroundPublication>,
    dialogs: Res<'w, crate::campaign_script_dialogs::CampaignScriptDialogs>,
    actors: Res<'w, crate::campaign_actors::UnitActors>,
    inventory: Res<'w, crate::campaign_inventory::CampaignInventory>,
}

fn click(
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    picking: Picking,
    hud: Res<crate::campaign_hud::CampaignHud>,
    mut input: ResMut<CampaignInput>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let Picking {
        windows,
        cameras,
        view,
        ground,
        dialogs,
        actors,
        inventory,
    } = picking;
    let (Some(mouse), Some(circles)) = (mouse, input.circles.clone()) else {
        return;
    };
    let select = mouse.just_pressed(MouseButton::Left);
    let order = mouse.just_pressed(MouseButton::Right);
    if !(select || order)
        || !view.active()
        || hud.quest_log_open
        || hud.pointer_consumed
        || inventory.pointer_consumed
        || dialogs.input_blocked()
    {
        return;
    }
    let (Ok(window), Ok((camera, transform, projection))) = (windows.single(), cameras.single())
    else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(transform, cursor) else {
        return;
    };
    let far = match projection {
        Projection::Perspective(p) => p.far,
        _ => return,
    };
    let Some(context) = ground.context.as_ref() else {
        return;
    };
    let Some(point) = ground_point(ray, &context.terrain, far) else {
        return;
    };
    let additive =
        keys.is_some_and(|k| k.pressed(KeyCode::ShiftLeft) || k.pressed(KeyCode::ShiftRight));
    let item_hit = session
        .session_witness()
        .and_then(|witness| actors.item_hit(ray, far, &witness));
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects_mut();
    if objects
        .presentation()
        .is_some_and(|p| p.control_enabled == Some(false))
    {
        return;
    }
    if objects.local_player_settings().is_some_and(|local| {
        objects
            .player_removal_state()
            .and_then(|state| state.get(local.identity))
            .is_some_and(|result| result.left)
    }) {
        return;
    }
    let picked = objects
        .executed_units()
        .filter(|(_, unit)| unit.shown && unit.combat.as_ref().is_none_or(|c| !c.dead))
        .filter_map(|(id, unit)| {
            let [x, y] = unit.position?;
            let scale = crate::campaign_actors::scalar(&unit.definition, "scale").ok()?;
            let distance = ((x - point[0]).powi(2) + (y - point[1]).powi(2)).sqrt();
            (distance <= scale * circles.scale_factor * 0.5).then_some((id, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id);
    debug!(?point, ?picked, select, "campaign pointer hit");
    if select {
        if let Some(unit) = picked {
            objects.select_units(&[unit], !additive);
        }
        return;
    }
    let local = objects.local_player_number();
    let owned: Vec<HandleId> = objects
        .selection()
        .iter()
        .copied()
        .filter(|&u| {
            objects
                .executed_unit(u)
                .ok()
                .and_then(|r| objects.player_settings(r.owner).ok())
                .is_some_and(|p| Some(p.number) == local)
        })
        .collect();
    for unit in owned {
        let result = if let Some(item) = item_hit.filter(|&id| {
            objects
                .executed_item(id)
                .is_ok_and(|item| item.held_by.is_none())
        }) {
            objects.native(
                "IssueTargetOrder",
                &[
                    Value::Handle(unit),
                    Value::Str("smart".into()),
                    Value::Handle(item),
                ],
            )
        } else {
            match picked.filter(|&target| objects.is_enemy(unit, target)) {
                Some(target) => objects.native(
                    "IssueTargetOrder",
                    &[
                        Value::Handle(unit),
                        Value::Str("attack".into()),
                        Value::Handle(target),
                    ],
                ),
                None => objects.native(
                    "IssuePointOrder",
                    &[
                        Value::Handle(unit),
                        Value::Str("move".into()),
                        Value::Real(point[0]),
                        Value::Real(point[1]),
                    ],
                ),
            }
        };
        if let Err(error) = result {
            input.last_error = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> pcx_map::Terrain {
        let mut bytes = b"W3E!".to_vec();
        bytes.extend(11u32.to_le_bytes());
        bytes.push(b'L');
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(b"Ldrt");
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(b"CLdi");
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(0f32.to_le_bytes());
        bytes.extend(0f32.to_le_bytes());
        for _ in 0..4 {
            bytes.extend(8232u16.to_le_bytes());
            bytes.extend(8192u16.to_le_bytes());
            bytes.extend([0, 0, 0]);
        }
        pcx_map::Terrain::from_bytes(bytes).unwrap()
    }

    #[test]
    fn clearing_session_discards_the_previous_selection_source_attempt() {
        let witness = std::sync::Arc::new(());
        let mut app = App::new();
        app.insert_resource(CampaignInput {
            attempted: true,
            session: Some(std::sync::Arc::downgrade(&witness)),
            last_error: Some("old source".into()),
            ..default()
        })
        .init_resource::<MapRequests>()
        .init_resource::<SelectedMapSources>()
        .init_resource::<MapScriptSession<MapStartupHost>>()
        .add_systems(Update, load_circles);
        app.update();
        let input = app.world().resource::<CampaignInput>();
        assert!(!input.attempted);
        assert!(input.session.is_none());
        assert!(input.last_error.is_none());
    }

    #[test]
    fn cursor_rays_land_on_the_sampled_terrain_surface() {
        let terrain = terrain();
        let surface = pcx_tera::world::sample_world_z(&terrain, 40.0, 70.0).unwrap();
        // Slanted ray in render space (x, z, -y) aimed at game point (40, 70).
        let target = Vec3::new(40.0, surface, -70.0);
        let origin = target + Vec3::new(-30.0, 200.0, 45.0);
        let ray = Ray3d::new(origin, Dir3::new(target - origin).unwrap());
        let [x, y] = ground_point(ray, &terrain, 5000.0).unwrap();
        assert!(
            (x - 40.0).abs() < 0.01 && (y - 70.0).abs() < 0.01,
            "{x} {y}"
        );
        let up = Ray3d::new(origin, Dir3::Y);
        assert_eq!(ground_point(up, &terrain, 5000.0), None);
        // Leaving the admitted terrain is no hit, not a clamped guess.
        let away = Ray3d::new(origin, Dir3::new(Vec3::new(-1.0, -0.01, 0.0)).unwrap());
        assert_eq!(ground_point(away, &terrain, 5000.0), None);
    }
    #[test]
    fn cursor_rays_can_enter_the_map_before_crossing_its_surface() {
        let terrain = terrain();
        let surface = pcx_tera::world::sample_world_z(&terrain, 40.0, 70.0).unwrap();
        let target = Vec3::new(40.0, surface, -70.0);
        for offset in [Vec3::new(-200.0, 300.0, 0.0), Vec3::new(0.0, 300.0, 200.0)] {
            let origin = target + offset;
            let ray = Ray3d::new(origin, Dir3::new(target - origin).unwrap());
            let [x, y] = ground_point(ray, &terrain, 5000.0).unwrap();
            assert!((x - 40.0).abs() < 0.01 && (y - 70.0).abs() < 0.01);
            for far in [50.0, 0.0, f32::NAN, f32::INFINITY] {
                assert_eq!(ground_point(ray, &terrain, far), None);
            }
        }
        // Entering below the surface is not a surface intersection.
        let origin = target + Vec3::new(-200.0, -50.0, 0.0);
        assert_eq!(
            ground_point(Ray3d::new(origin, Dir3::X), &terrain, 5000.0),
            None
        );
    }
}
