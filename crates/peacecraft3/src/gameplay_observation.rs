//! Structured observation of canonical campaign state for gameplay harnesses.
//!
//! Read-only sensor bridge over `ObjectHost` (the executed VM state), not a
//! pixel/vision benchmark: labels it produces say `structured-state sensors`.
//! Only units the local player owns or can currently see (`vision_cell` raw
//! `Visible`) are listed; hidden enemies are never enumerated. No raw asset
//! bytes, local paths or secrets leave this module — opaque `HandleId`
//! tokens are rendered as hex, coordinates come from live unit records.
//!
//! `#[cfg(feature = "test-fixtures")]` only; no new CLI.

use bevy::prelude::Vec3;
use pcx_play::playercolor_host::ObjectHost;
use pcx_play::playercolor_host::VisionState;
use pcx_script::HandleId;
use serde::Serialize;

/// One observable unit: identity, position, health, current order, and the
/// viewport projection the input pipeline would click to reach it.
#[derive(Debug, Clone, Serialize)]
pub struct ObservedUnit {
    /// Opaque handle token, hex. Stable within one session only.
    pub id: String,
    pub rawcode: String,
    /// World position from the live unit record (`position` when admitted,
    /// else the executed creation request — labelled by `position_source`).
    pub position: Option<[f32; 2]>,
    pub position_source: &'static str,
    pub life: Option<f32>,
    pub max_life: Option<f32>,
    /// `GetUnitCurrentOrder` result when the activity runtime is admitted.
    pub current_order: Option<i32>,
    /// Viewport pixel the camera projects this unit to, when it is in front
    /// of the camera and inside the viewport rect.
    pub screen: Option<[f32; 2]>,
    pub in_viewport: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Observation {
    pub scope: &'static str,
    pub game_time: Option<f64>,
    pub local_player: Option<i32>,
    pub selection: Vec<String>,
    pub units: Vec<ObservedUnit>,
    pub camera_bounds: Option<[f32; 8]>,
}

/// Bounded read of own/visible units through the canonical vision ledger.
/// `terrain_z` supplies ground height for the projection ray origin; when
/// `None`, projection uses z=0 and `screen` may be null.
pub fn observe(
    objects: &ObjectHost,
    camera: Option<(&bevy::camera::Camera, &bevy::prelude::GlobalTransform)>,
    terrain_z: Option<impl Fn([f32; 2]) -> f32>,
    limit: usize,
) -> Observation {
    let local = objects.local_player_number();
    let viewer = objects.local_player_settings().map(|p| p.identity);
    let game_time = objects.timer_game_time();
    let selection = objects
        .selection()
        .iter()
        .map(|id| handle_hex(*id))
        .collect();
    let mut units = Vec::new();
    let mut scanned = 0usize;
    for (id, record) in objects.executed_units() {
        scanned += 1;
        if scanned > limit.saturating_mul(64).max(1024) {
            break;
        }
        let owned = local.is_some_and(|n| {
            objects
                .player_settings(record.owner)
                .is_ok_and(|p| p.number == n)
        });
        let visible = viewer.is_some_and(|v| {
            let pos = record.position.unwrap_or([record.x, record.y]);
            objects
                .vision_cell(v, pos)
                .ok()
                .flatten()
                .is_some_and(|cell| cell.raw == VisionState::Visible)
        });
        if !owned && !visible {
            continue;
        }
        let (position, source) = match record.position {
            Some(p) => (Some(p), "live"),
            None => (Some([record.x, record.y]), "creation-request"),
        };
        let screen = position.and_then(|p| {
            let (camera, transform) = camera?;
            let z = terrain_z
                .as_ref()
                .map(|f| f(p))
                .unwrap_or(0.0);
            camera
                .world_to_viewport(transform, Vec3::new(p[0], z, -p[1]))
                .ok()
                .map(|v| [v.x, v.y])
        });
        let in_viewport = screen.is_some_and(|s| {
            camera
                .and_then(|(c, _)| c.logical_viewport_size())
                .is_some_and(|size| s[0] >= 0.0 && s[1] >= 0.0 && s[0] <= size.x && s[1] <= size.y)
        });
        units.push(ObservedUnit {
            id: handle_hex(id),
            rawcode: String::from_utf8_lossy(&record.definition.rawcode).into_owned(),
            position,
            position_source: source,
            life: record.vitals.as_ref().map(|v| v.life),
            max_life: record.vitals.as_ref().map(|v| v.max_life),
            current_order: objects.current_unit_order(id).ok(),
            screen,
            in_viewport,
        });
        if units.len() >= limit {
            break;
        }
    }
    let camera_bounds = objects.camera_state().and_then(|c| c.bounds);
    Observation {
        scope: "structured-state sensors (not a vision benchmark)",
        game_time,
        local_player: local,
        selection,
        units,
        camera_bounds,
    }
}

fn handle_hex(id: HandleId) -> String {
    format!("{:x}", id.token())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_hex_is_opaque_and_stable() {
        let id = HandleId::new(std::num::NonZeroU64::new(0x2a).unwrap());
        assert_eq!(handle_hex(id), "2a");
    }
}
