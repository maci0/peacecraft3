//! A bounded presentation raster over canonical world-space vision sources.
use pcx_objects::editor::minimap_fog::MinimapFogAlpha;
use pcx_play::playercolor_host::{VisionCircle, VisionState};

pub(crate) struct Raster {
    width: usize,
    height: usize,
    explored: Vec<bool>,
    current: Vec<VisionState>,
    alpha: Vec<u8>,
}

impl Raster {
    pub(crate) fn new(width: u32, height: u32, max_pixels: usize) -> Result<Self, String> {
        let width = width as usize;
        let height = height as usize;
        let count = width
            .checked_mul(height)
            .filter(|n| *n > 0 && *n <= max_pixels)
            .ok_or("minimap visibility raster budget")?;
        fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>, String> {
            let mut data = Vec::new();
            data.try_reserve_exact(count)
                .map_err(|_| "minimap visibility allocation")?;
            data.resize(count, value);
            Ok(data)
        }
        Ok(Self {
            width,
            height,
            explored: filled(count, false)?,
            current: filled(count, VisionState::Masked)?,
            alpha: filled(count, 0)?,
        })
    }

    pub(crate) fn update(
        &mut self,
        bounds: [[f32; 2]; 2],
        sources: &[VisionCircle],
        fog: bool,
        mask: bool,
        opacity: MinimapFogAlpha,
        work_limit: usize,
    ) -> Result<&[u8], String> {
        let [[left, bottom], [right, top]] = bounds.map(|p| p.map(f64::from));
        if ![left, bottom, right, top].iter().all(|n| n.is_finite())
            || right <= left
            || top <= bottom
        {
            return Err("minimap visibility bounds are invalid".into());
        }
        let scale = (self.width as f64 / (right - left)).min(self.height as f64 / (top - bottom));
        let pad = [
            (self.width as f64 - (right - left) * scale) * 0.5,
            (self.height as f64 - (top - bottom) * scale) * 0.5,
        ];
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(sources.len())
            .map_err(|_| "minimap visibility allocation")?;
        let mut work = self.alpha.len();
        for circle in sources {
            if !circle.center.iter().all(|n| n.is_finite())
                || !circle.radius.is_finite()
                || circle.radius < 0.
            {
                return Err("invalid minimap vision circle".into());
            }
            let [cx, cy] = circle.center.map(f64::from);
            let radius = f64::from(circle.radius);
            let x = |v: f64| (v - left) * scale + pad[0];
            let y = |v: f64| (top - v) * scale + pad[1];
            let x0 = x(cx - radius).floor().clamp(0., self.width as f64) as usize;
            let x1 = x(cx + radius).ceil().clamp(0., self.width as f64) as usize;
            let y0 = y(cy + radius).floor().clamp(0., self.height as f64) as usize;
            let y1 = y(cy - radius).ceil().clamp(0., self.height as f64) as usize;
            work = work
                .checked_add(
                    (x1 - x0)
                        .checked_mul(y1 - y0)
                        .ok_or("minimap visibility work overflow")?,
                )
                .filter(|n| *n <= work_limit)
                .ok_or("minimap visibility work budget")?;
            ranges.push((x0..x1, y0..y1));
        }
        if work > work_limit {
            return Err("minimap visibility work budget".into());
        }
        for (cell, explored) in self.current.iter_mut().zip(&self.explored) {
            *cell = if *explored {
                VisionState::Fogged
            } else {
                VisionState::Masked
            };
        }
        // shortcut: circular sight has no terrain/tree occlusion yet; add the shared occluder pass before claiming visibility parity.
        for (source, (xs, ys)) in sources.iter().zip(ranges) {
            let [cx, cy] = source.center.map(f64::from);
            let radius2 = f64::from(source.radius).powi(2);
            for y in ys {
                let wy = top - (y as f64 + 0.5 - pad[1]) / scale;
                if wy < bottom || wy > top {
                    continue;
                }
                for x in xs.clone() {
                    let wx = left + (x as f64 + 0.5 - pad[0]) / scale;
                    if wx < left || wx > right || (wx - cx).powi(2) + (wy - cy).powi(2) > radius2 {
                        continue;
                    }
                    let at = y * self.width + x;
                    self.current[at] = source.state;
                    self.explored[at] = source.state != VisionState::Masked;
                }
            }
        }
        for ((out, state), explored) in self.alpha.iter_mut().zip(&self.current).zip(&self.explored)
        {
            *out = if *state == VisionState::Visible || (!fog && (!mask || *explored)) {
                opacity.visible
            } else if mask && !*explored {
                opacity.masked
            } else {
                opacity.fogged
            };
        }
        Ok(&self.alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const OPACITY: MinimapFogAlpha = MinimapFogAlpha {
        masked: 3,
        fogged: 71,
        visible: 241,
    };
    fn circle(center: [f32; 2]) -> VisionCircle {
        VisionCircle {
            center,
            radius: 0.6,
            state: VisionState::Visible,
            after_units: false,
        }
    }
    #[test]
    fn movement_retains_exploration_and_switches_are_independent() {
        let mut raster = Raster::new(4, 4, 16).unwrap();
        let bounds = [[0., 0.], [4., 4.]];
        assert_eq!(
            raster
                .update(bounds, &[circle([0.5, 3.5])], true, true, OPACITY, 64)
                .unwrap()[0],
            241
        );
        let moved = raster
            .update(bounds, &[circle([3.5, 0.5])], true, true, OPACITY, 64)
            .unwrap();
        assert_eq!([moved[0], moved[1], moved[15]], [71, 3, 241]);
        let no_fog = raster
            .update(bounds, &[], false, true, OPACITY, 64)
            .unwrap();
        assert_eq!([no_fog[0], no_fog[1]], [241, 3]);
        assert!(
            raster
                .update(bounds, &[], true, false, OPACITY, 64)
                .unwrap()
                .iter()
                .all(|v| *v == 71)
        );
        assert!(
            raster
                .update(bounds, &[], false, false, OPACITY, 64)
                .unwrap()
                .iter()
                .all(|v| *v == 241)
        );
    }
    #[test]
    fn masking_order_budget_and_nonfinite_refusal_preserve_history() {
        let mut raster = Raster::new(4, 4, 16).unwrap();
        let bounds = [[0., 0.], [4., 4.]];
        let visible = circle([0.5, 3.5]);
        let masked = VisionCircle {
            state: VisionState::Masked,
            ..visible
        };
        assert_eq!(
            raster
                .update(bounds, &[masked, visible], true, true, OPACITY, 64)
                .unwrap()[0],
            241
        );
        assert!(
            raster
                .update(bounds, &[masked], true, true, OPACITY, 0)
                .is_err()
        );
        assert_eq!(
            raster.update(bounds, &[], true, true, OPACITY, 64).unwrap()[0],
            71
        );
        assert_eq!(
            raster
                .update(bounds, &[visible, masked], true, true, OPACITY, 64)
                .unwrap()[0],
            3
        );
        assert!(
            raster
                .update(bounds, &[circle([f32::NAN, 0.])], true, true, OPACITY, 64)
                .is_err()
        );
        assert!(Raster::new(u32::MAX, u32::MAX, 16).is_err());
    }
}
