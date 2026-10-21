//! Initial W3E blight. Script/player layers and terrain presentation are separate.
use crate::scripts::AssetIdentity;
use pcx_map::Terrain;

/// Map-authored corner flags on their source-derived world grid.
/// The nearest-corner footprint is a recreation convention, not native parity.
/// shortcut: initial layer only; add player-owned layers when admitting SetBlight*.
#[derive(Debug)]
pub struct InitialBlight {
    pub origin: AssetIdentity,
    world_origin: [f64; 2],
    spacing: f64,
    size: [usize; 2],
    corners: Vec<bool>,
}

impl InitialBlight {
    /// `spacing` is derived by the caller from admitted pathing normalization and
    /// the map's terrain/pathing dimension ratio; no measured tile size is copied.
    pub fn read(
        terrain: &Terrain,
        spacing: f32,
        origin: AssetIdentity,
        max_corners: usize,
    ) -> Result<Self, String> {
        if terrain.corner_count() > max_corners {
            return Err("initial blight corner budget exceeded".into());
        }
        let (x, y) = terrain.origin();
        if !spacing.is_finite() || spacing <= 0.0 || !x.is_finite() || !y.is_finite() {
            return Err("initial blight needs finite origin and positive spacing".into());
        }
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|s| s.trim().is_empty())
        {
            return Err("initial blight needs explicit source identity".into());
        }
        let size = [terrain.width(), terrain.height()].map(|n| n as usize + 1);
        if size[0].checked_mul(size[1]) != Some(terrain.corner_count()) {
            return Err("initial blight corner dimensions disagree".into());
        }
        let mut corners = Vec::new();
        corners
            .try_reserve_exact(terrain.corner_count())
            .map_err(|_| "initial blight corner storage")?;
        corners.extend(terrain.corners().iter().map(|c| c.is_blighted()));
        Ok(Self {
            origin,
            world_origin: [f64::from(x), f64::from(y)],
            spacing: f64::from(spacing),
            size,
            corners,
        })
    }

    fn grid(&self, p: [f32; 2]) -> Result<[f64; 2], String> {
        if p.iter().any(|v| !v.is_finite()) {
            return Err("initial blight needs finite position".into());
        }
        Ok(std::array::from_fn(|i| {
            (f64::from(p[i]) - self.world_origin[i]) / self.spacing + 0.5
        }))
    }

    /// Outside the corner footprints there is no map-authored blight.
    pub fn is_blighted(&self, position: [f32; 2]) -> Result<bool, String> {
        Ok(self.at_grid(self.grid(position)?))
    }

    fn at_grid(&self, p: [f64; 2]) -> bool {
        if (0..2).any(|i| p[i] < 0.0 || p[i] >= self.size[i] as f64) {
            return false;
        }
        self.corners[p[1].floor() as usize * self.size[0] + p[0].floor() as usize]
    }

    /// Exact time fraction inside blighted footprints on one straight segment.
    /// Every visited corner consumes caller work; no step-size sampling is used.
    pub(crate) fn seconds(
        &self,
        from: [f32; 2],
        to: [f32; 2],
        duration: f64,
        work: &mut usize,
    ) -> Result<f64, String> {
        if !duration.is_finite() || duration < 0.0 {
            return Err("initial blight needs finite nonnegative duration".into());
        }
        let a = self.grid(from)?;
        let b = self.grid(to)?;
        let d: [f64; 2] = std::array::from_fn(|i| b[i] - a[i]);
        let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
        for i in 0..2 {
            if d[i] == 0.0 {
                if a[i] < 0.0 || a[i] >= self.size[i] as f64 {
                    return Ok(0.0);
                }
            } else {
                let t0 = -a[i] / d[i];
                let t1 = (self.size[i] as f64 - a[i]) / d[i];
                enter = enter.max(t0.min(t1));
                leave = leave.min(t0.max(t1));
            }
        }
        if enter >= leave || duration == 0.0 {
            return Ok(0.0);
        }
        let mut edge = [f64::INFINITY; 2];
        let mut stride = [f64::INFINITY; 2];
        for i in 0..2 {
            let p = a[i] + d[i] * enter;
            if d[i] > 0.0 {
                edge[i] = (p.floor() + 1.0 - a[i]) / d[i];
            } else if d[i] < 0.0 {
                edge[i] = (p.ceil() - 1.0 - a[i]) / d[i];
            }
            if d[i] != 0.0 {
                stride[i] = 1.0 / d[i].abs();
            }
        }
        let mut total = 0.0;
        while enter < leave {
            *work = work.checked_sub(1).ok_or("initial blight work exhausted")?;
            let end = leave.min(edge[0]).min(edge[1]);
            if end <= enter {
                return Err("initial blight crossing did not advance".into());
            }
            let middle = (enter + end) * 0.5;
            if self.at_grid(std::array::from_fn(|i| a[i] + d[i] * middle)) {
                total += (end - enter) * duration;
            }
            for i in 0..2 {
                if edge[i] <= end {
                    edge[i] += stride[i];
                }
            }
            enter = end;
        }
        Ok(total)
    }
}

#[cfg(test)]
#[path = "terrain_blight_tests.rs"]
pub(crate) mod tests;
