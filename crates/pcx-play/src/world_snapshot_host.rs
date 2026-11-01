//! Asset-derived world snapshots: counts and executed presentation state.
//! No invented defaults; absent admissions read as `None`.
use crate::map_startup_host::MapStartupHost;

/// Serializable snapshot of the shared world for saves, diagnostics and the
/// initial-state gate. Values are the script's own executed state plus
/// admitted map extents; no wall clock or renderer state is included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldSnapshot {
    pub world_bounds: Option<[i32; 4]>,
    pub ground_items: usize,
    pub destructables: usize,
    pub live_weather: usize,
    pub enabled_weather: usize,
    pub placed_doodads: Option<(usize, usize, usize)>,
    pub terrain_fog_style: Option<i32>,
    pub sky_override: Option<Option<String>>,
    pub camera_destination: Option<[i32; 2]>,
    pub game_time_bits: Option<u64>,
    pub preload: Option<(usize, usize, usize, u64)>,
}

fn to_bits(value: f32) -> i32 {
    value.to_bits() as i32
}

impl MapStartupHost {
    /// Capture the current shared world without mutating it.
    pub fn capture_world_snapshot(&self) -> WorldSnapshot {
        let objects = self.objects();
        let (ground_items, destructables) = objects.widget_snapshot_counts();
        let mut live_weather = 0;
        let mut enabled_weather = 0;
        for (_, effect) in objects.weather_effects() {
            live_weather += 1;
            enabled_weather += usize::from(effect.enabled);
        }
        WorldSnapshot {
            world_bounds: objects.world_bounds_snapshot().map(|b| b.map(to_bits)),
            ground_items,
            destructables,
            live_weather,
            enabled_weather,
            placed_doodads: objects.placed_doodad_snapshot_counts(),
            terrain_fog_style: objects.terrain_fog_3d().map(|fog| fog.style),
            sky_override: objects.sky_model_3d(),
            camera_destination: objects
                .applied_camera_setup()
                .map(|view| [to_bits(view.destination.x), to_bits(view.destination.y)]),
            game_time_bits: objects.timer_game_time().map(f64::to_bits),
            preload: self.preload_progress(),
        }
    }
}
