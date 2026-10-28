//! Presentation fit of the shared vision ledger onto the minimap image.
//! Exploration lives on the ledger. This raster only maps pixels.
use pcx_objects::editor::minimap_fog::MinimapFogAlpha;
use pcx_play::playercolor_host::VisionState;

struct Mapping {
    left: f64,
    bottom: f64,
    right: f64,
    top: f64,
    scale: f64,
    pad: [f64; 2],
}
impl Mapping {
    fn new(width: usize, height: usize, bounds: [[f32; 2]; 2]) -> Result<Self, String> {
        let [[left, bottom], [right, top]] = bounds.map(|p| p.map(f64::from));
        if ![left, bottom, right, top].iter().all(|n| n.is_finite())
            || right <= left
            || top <= bottom
        {
            return Err("minimap visibility bounds are invalid".into());
        }
        let scale = (width as f64 / (right - left)).min(height as f64 / (top - bottom));
        Ok(Self {
            left,
            bottom,
            right,
            top,
            scale,
            pad: [
                (width as f64 - (right - left) * scale) * 0.5,
                (height as f64 - (top - bottom) * scale) * 0.5,
            ],
        })
    }
}

pub(crate) struct Raster {
    width: usize,
    height: usize,
    alpha: Vec<u8>,
}

impl Raster {
    /// Inverse of the sight projection; dragging into padding clamps to the map edge.
    pub(crate) fn world_at(
        &self,
        bounds: [[f32; 2]; 2],
        fraction: [f32; 2],
    ) -> Result<[f32; 2], String> {
        if !fraction.iter().all(|v| v.is_finite()) {
            return Err("minimap pointer is not finite".into());
        }
        // shortcut: share the terrain-image fit; qualify native camera margins/padding with a paired replay.
        let m = Mapping::new(self.width, self.height, bounds)?;
        Ok([
            (m.left + (f64::from(fraction[0]) * self.width as f64 - m.pad[0]) / m.scale)
                .clamp(m.left, m.right) as f32,
            (m.top - (f64::from(fraction[1]) * self.height as f64 - m.pad[1]) / m.scale)
                .clamp(m.bottom, m.top) as f32,
        ])
    }

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
            alpha: filled(count, 0)?,
        })
    }

    /// Image pixel of a world point, outside the image included.
    pub(crate) fn pixel_of(
        &self,
        bounds: [[f32; 2]; 2],
        point: [f32; 2],
    ) -> Result<[f64; 2], String> {
        if point.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite minimap point".into());
        }
        let m = Mapping::new(self.width, self.height, bounds)?;
        Ok([
            (f64::from(point[0]) - m.left) * m.scale + m.pad[0],
            (m.top - f64::from(point[1])) * m.scale + m.pad[1],
        ])
    }

    /// Whole-pixel padding of the map inside the image: the slack of each axis, truncated and halved.
    pub(crate) fn pads(&self, bounds: [[f32; 2]; 2]) -> Result<[u32; 2], String> {
        let m = Mapping::new(self.width, self.height, bounds)?;
        let slack =
            |size: usize, extent: f64| ((size as f64 - extent * m.scale).max(0.) as u32) >> 1;
        Ok([
            slack(self.width, m.right - m.left),
            slack(self.height, m.top - m.bottom),
        ])
    }

    pub(crate) fn point_at(
        &self,
        bounds: [[f32; 2]; 2],
        point: [f32; 2],
    ) -> Result<Option<[f64; 2]>, String> {
        if point.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite minimap point".into());
        }
        let m = Mapping::new(self.width, self.height, bounds)?;
        let pixel = [
            (f64::from(point[0]) - m.left) * m.scale + m.pad[0],
            (m.top - f64::from(point[1])) * m.scale + m.pad[1],
        ];
        Ok((pixel[0] >= 0.
            && pixel[1] >= 0.
            && pixel[0] < self.width as f64
            && pixel[1] < self.height as f64)
            .then_some(pixel))
    }
    /// Project an already sampled ledger. The callback owns fog, mask, and exploration.
    pub(crate) fn project(
        &mut self,
        bounds: [[f32; 2]; 2],
        opacity: MinimapFogAlpha,
        work_limit: usize,
        mut classify: impl FnMut([f64; 2]) -> Result<VisionState, String>,
    ) -> Result<&[u8], String> {
        let Mapping {
            left,
            bottom,
            right,
            top,
            scale,
            pad,
        } = Mapping::new(self.width, self.height, bounds)?;
        if self.alpha.len() > work_limit {
            return Err("minimap visibility work budget".into());
        }
        let mut alpha = Vec::new();
        alpha
            .try_reserve_exact(self.alpha.len())
            .map_err(|_| "minimap visibility allocation")?;
        for y in 0..self.height {
            let wy = top - (y as f64 + 0.5 - pad[1]) / scale;
            for x in 0..self.width {
                let wx = left + (x as f64 + 0.5 - pad[0]) / scale;
                let state = if wx < left || wx > right || wy < bottom || wy > top {
                    VisionState::Masked
                } else {
                    classify([wx, wy])?
                };
                alpha.push(match state {
                    VisionState::Visible => opacity.visible,
                    VisionState::Masked => opacity.masked,
                    VisionState::Fogged => opacity.fogged,
                });
            }
        }
        self.alpha = alpha;
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
    #[test]
    fn projection_copies_presented_states_and_a_refused_budget_keeps_pixels() {
        let mut raster = Raster::new(2, 2, 4).unwrap();
        let bounds = [[0., 0.], [2., 2.]];
        let alpha = raster
            .project(bounds, OPACITY, 4, |[x, y]| {
                Ok(if x < 1. && y > 1. {
                    VisionState::Visible
                } else if y > 1. {
                    VisionState::Fogged
                } else {
                    VisionState::Masked
                })
            })
            .unwrap();
        assert_eq!(alpha, &[241, 71, 3, 3]);
        assert!(
            raster
                .project(bounds, OPACITY, 0, |_| Ok(VisionState::Visible))
                .is_err()
        );
        assert_eq!(
            raster
                .project(bounds, OPACITY, 4, |[x, y]| {
                    Ok(if x < 1. && y > 1. {
                        VisionState::Visible
                    } else if y > 1. {
                        VisionState::Fogged
                    } else {
                        VisionState::Masked
                    })
                })
                .unwrap(),
            &[241, 71, 3, 3]
        );
        assert!(Raster::new(u32::MAX, u32::MAX, 16).is_err());
    }

    #[test]
    fn pointer_inverse_matches_pixels_and_clamps_padding_and_drag() {
        let bounds = [[-100., -40.], [300., 160.]];
        for (width, height) in [(10, 10), (20, 10), (10, 20)] {
            let mut raster = Raster::new(width, height, 200).unwrap();
            assert_eq!(raster.world_at(bounds, [0.5, 0.5]).unwrap(), [100., 60.]);
            assert_eq!(raster.world_at(bounds, [-1., -1.]).unwrap(), [-100., 160.]);
            assert_eq!(raster.world_at(bounds, [2., 2.]).unwrap(), [300., -40.]);
            let fraction = [0.5 + 0.5 / width as f32, 0.5 + 0.5 / height as f32];
            let center = raster.world_at(bounds, fraction).unwrap();
            let alpha = raster
                .project(bounds, OPACITY, 1000, |point| {
                    Ok(
                        if (point[0] - f64::from(center[0])).hypot(point[1] - f64::from(center[1]))
                            <= 1.
                        {
                            VisionState::Visible
                        } else {
                            VisionState::Masked
                        },
                    )
                })
                .unwrap();
            assert_eq!(alpha[(height / 2 * width + width / 2) as usize], 241);
            assert!(raster.world_at(bounds, [f32::NAN, 0.]).is_err());
            assert!(raster.world_at([[0.; 2]; 2], [0.5; 2]).is_err());
        }
    }

    #[test]
    fn marker_points_use_the_image_fit_and_refuse_nonfinite_input() {
        let bounds = [[0., 0.], [8., 4.]];
        let raster = Raster::new(8, 8, 64).unwrap();
        assert_eq!(raster.point_at(bounds, [4., 2.]).unwrap(), Some([4., 4.]));
        assert_eq!(raster.point_at(bounds, [-1., 2.]).unwrap(), None);
        assert!(raster.point_at(bounds, [f32::NAN, 0.]).is_err());
    }

    #[test]
    fn pixels_outside_the_image_and_whole_pixel_paddings_follow_the_fit() {
        let raster = Raster::new(32, 32, 1024).unwrap();
        // 6 by 2 world units fit the width at 32/6 pixels per unit: 32 by 10.67, slack 21.33.
        let wide = [[0., 0.], [6., 2.]];
        assert_eq!(raster.pads(wide).unwrap(), [0, 10]);
        let tall = [[0., 0.], [2., 8.]];
        assert_eq!(raster.pads(tall).unwrap(), [12, 0]);
        let square = [[-3., -3.], [3., 3.]];
        assert_eq!(raster.pads(square).unwrap(), [0, 0]);
        let [x, y] = raster.pixel_of(wide, [-3., 5.]).unwrap();
        assert!((x + 16.).abs() < 1e-9 && (y + 16. / 3.).abs() < 1e-9);
        assert!(raster.pixel_of(wide, [f32::INFINITY, 0.]).is_err());
        assert!(raster.pads([[0.; 2]; 2]).is_err());
    }
}
