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

/// Source fog-grid geometry; the shared ledger supplies each cell's presented state.
#[derive(Clone, Copy)]
pub(crate) struct FogGrid {
    pub world_bounds: [[f32; 2]; 2],
    pub cell_world: f32,
    pub fraction_bits: u8,
}

pub(crate) struct Raster {
    width: usize,
    height: usize,
    alpha: Vec<u8>,
    /// The previous projection's buffer, reused so a refused projection keeps `alpha`.
    scratch: Vec<u8>,
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
        // shortcut: floating image fit remains; qualify native fog-grid sampling and rounding with a paired replay.
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
            scratch: Vec::new(),
        })
    }

    /// The alpha plane of the last accepted projection.
    pub(crate) fn alpha(&self) -> &[u8] {
        &self.alpha
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
        let pad = self.pads(bounds)?.map(f64::from);
        // Image-space admission helper; CPU marker placement uses PointProjection.
        let pixel = [
            ((f64::from(point[0]) - m.left) * m.scale).floor() + pad[0],
            self.height as f64 - ((f64::from(point[1]) - m.bottom) * m.scale).floor() - pad[1],
        ];
        Ok((pixel[0] >= 0.
            && pixel[1] >= 0.
            && pixel[0] < self.width as f64
            && pixel[1] < self.height as f64)
            .then_some(pixel))
    }
    /// Reproduce the native fixed-point scan and horizontal transition average.
    pub(crate) fn project(
        &mut self,
        bounds: [[f32; 2]; 2],
        grid: FogGrid,
        base: &[u8],
        opacity: MinimapFogAlpha,
        work_limit: usize,
        mut classify: impl FnMut([f64; 2]) -> Result<VisionState, String>,
    ) -> Result<&[u8], String> {
        Mapping::new(self.width, self.height, bounds)?;
        Mapping::new(self.width, self.height, grid.world_bounds)?;
        if base.len() != self.alpha.len()
            || base.len() > work_limit
            || !grid.cell_world.is_finite()
            || grid.cell_world <= 0.
            || grid.fraction_bits == 0
            || grid.fraction_bits >= 31
        {
            return Err("invalid minimap fog grid or work budget".into());
        }
        let origin = grid.world_bounds[0].map(f64::from);
        let cell = f64::from(grid.cell_world);
        let mut lower = [0u64; 2];
        let mut extent = [0u64; 2];
        for axis in 0..2 {
            let lo = ((f64::from(bounds[0][axis]) - origin[axis]) / cell).floor();
            let hi = ((f64::from(bounds[1][axis]) - origin[axis]) / cell).floor();
            let limit = ((f64::from(grid.world_bounds[1][axis]) - origin[axis]) / cell).floor();
            if lo < 0. || hi <= lo || hi > limit || hi > f64::from(u32::MAX) {
                return Err("minimap fog rectangle is outside its source grid".into());
            }
            lower[axis] = lo as u64;
            extent[axis] = hi as u64 - lower[axis];
        }
        let pad = self.pads(bounds)?.map(|p| p as usize);
        let span = [self.width, self.height].map(|v| v.saturating_sub(1));
        let span = [
            span[0].checked_sub(pad[0] * 2),
            span[1].checked_sub(pad[1] * 2),
        ];
        let [Some(sx), Some(sy)] = span else {
            return Err("minimap fog image span".into());
        };
        if sx == 0 || sy == 0 {
            return Err("minimap fog image span".into());
        }
        let unit = 1u64 << grid.fraction_bits;
        let step = [
            (extent[0] * unit) / sx as u64,
            (extent[1] * unit) / sy as u64,
        ];
        let mut alpha = std::mem::take(&mut self.scratch);
        alpha.clear();
        alpha
            .try_reserve_exact(base.len())
            .map_err(|_| "minimap visibility allocation")?;
        alpha.extend_from_slice(base);
        let mut work = base.len();
        let mut charge = || -> Result<(), String> {
            work = work
                .checked_add(1)
                .filter(|n| *n <= work_limit)
                .ok_or("minimap visibility work budget")?;
            Ok(())
        };
        let mut yacc = 0u64;
        let mut gy = lower[1];
        for y in (pad[1]..self.height - pad[1]).rev() {
            gy += yacc >> grid.fraction_bits;
            yacc &= unit - 1;
            let mut gx = lower[0];
            let mut xacc = unit;
            let mut previous = 0u8;
            let mut value = 0u8;
            for x in pad[0]..self.width - 1 - pad[0] {
                let count = xacc >> grid.fraction_bits;
                xacc &= unit - 1;
                for _ in 0..count {
                    charge()?;
                    // The ledger adapter samples the centre of each source fog cell.
                    let point = [
                        origin[0] + (gx as f64 + 0.5) * cell,
                        origin[1] + (gy as f64 + 0.5) * cell,
                    ];
                    let selected = match classify(point)? {
                        VisionState::Visible => opacity.visible,
                        VisionState::Fogged => opacity.fogged,
                        VisionState::Masked => opacity.masked,
                    };
                    value = if selected == previous {
                        selected
                    } else {
                        ((u16::from(selected) + u16::from(previous)) >> 1) as u8
                    };
                    previous = selected;
                    gx += 1;
                }
                charge()?;
                alpha[y * self.width + x] = value;
                xacc += step[0];
            }
            yacc += step[1];
        }
        std::mem::swap(&mut self.alpha, &mut alpha);
        self.scratch = alpha;
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
    fn grid(bounds: [[f32; 2]; 2]) -> FogGrid {
        FogGrid {
            world_bounds: bounds,
            cell_world: 1.,
            fraction_bits: 8,
        }
    }
    #[test]
    fn source_scan_averages_horizontal_transitions_and_preserves_unwritten_base() {
        let mut raster = Raster::new(4, 4, 16).unwrap();
        let bounds = [[0., 0.], [2., 2.]];
        let alpha = raster
            .project(bounds, grid(bounds), &[9; 16], OPACITY, 36, |[x, y]| {
                Ok(if y >= 1. {
                    VisionState::Masked
                } else if x < 1. {
                    VisionState::Visible
                } else {
                    VisionState::Fogged
                })
            })
            .unwrap();
        assert_eq!(
            alpha,
            [1, 1, 3, 9, 1, 1, 3, 9, 120, 120, 156, 9, 120, 120, 156, 9]
        );
    }
    #[test]
    fn failed_grid_samples_and_work_budgets_keep_the_accepted_alpha() {
        let mut raster = Raster::new(4, 4, 16).unwrap();
        let bounds = [[0., 0.], [2., 2.]];
        let visible = |_: [f64; 2]| Ok(VisionState::Visible);
        let accepted = raster
            .project(bounds, grid(bounds), &[9; 16], OPACITY, 36, visible)
            .unwrap()
            .to_vec();
        let mut seen = 0;
        assert!(
            raster
                .project(bounds, grid(bounds), &[9; 16], OPACITY, 36, |_| {
                    seen += 1;
                    if seen == 3 {
                        Err("vision refused".into())
                    } else {
                        Ok(VisionState::Masked)
                    }
                })
                .is_err()
        );
        assert!(
            raster
                .project(bounds, grid(bounds), &[9; 16], OPACITY, 35, visible)
                .is_err()
        );
        assert!(
            raster
                .project(bounds, grid(bounds), &[9; 15], OPACITY, 36, visible)
                .is_err()
        );
        assert_eq!(raster.alpha(), accepted);
        assert_eq!(
            raster
                .project(bounds, grid(bounds), &[9; 16], OPACITY, 36, visible)
                .unwrap(),
            accepted
        );
        assert!(Raster::new(u32::MAX, u32::MAX, 16).is_err());
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "test-fixtures"))]
    #[test]
    #[ignore = "requires PCX_NATIVE_MINIMAP_EVIDENCE and PCX_PLAYER_RESOURCE_IMAGE; local original-client snapshots"]
    fn supplied_native_fog_grid_reproduces_full_alpha_plane() {
        use sha2::Digest;
        use std::path::Path;
        fn read(path: &Path, limit: usize) -> Vec<u8> {
            assert!(std::fs::metadata(path).unwrap().len() <= limit as u64);
            std::fs::read(path).unwrap()
        }
        let root = std::path::PathBuf::from(std::env::var("PCX_NATIVE_MINIMAP_EVIDENCE").unwrap());
        let source = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").unwrap();
        let image = read(Path::new(&source), 32 << 20);
        let draw = pcx_play::minimap_draw_layout::Draw::reviewed_legacy(
            &image,
            pcx_play::scripts::AssetIdentity {
                source,
                edition: "explicit local Legacy snapshot".into(),
                version: "reviewed minimap image".into(),
            },
        )
        .unwrap();
        let hashes: serde_json::Value =
            serde_json::from_slice(&read(&root.join("native-held-hashes.json"), 16 << 10)).unwrap();
        let input: serde_json::Value =
            serde_json::from_slice(&read(&root.join("reader-inputs.json"), 16 << 10)).unwrap();
        let digest = |bytes: &[u8]| {
            sha2::Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        assert_eq!(input["image_sha256"].as_str().unwrap(), digest(&image));
        let evidence = |name: &str| {
            let bytes = read(&root.join(format!("native-held-{name}")), 16 << 20);
            assert_eq!(hashes[name].as_str().unwrap(), digest(&bytes));
            bytes
        };
        let object = evidence("minimap-object.bin");
        let context = evidence("minimap-fog-context.bin");
        assert!(object.len() >= 0x9e4 && context.len() >= 0x70);
        let word =
            |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |bytes: &[u8], at| f32::from_bits(word(bytes, at));
        let world = [
            [float(&context, 0x48), float(&context, 0x4c)],
            [float(&context, 0x54), float(&context, 0x50)],
        ];
        let columns = word(&context, 0x68) as usize;
        assert!((1..=4096).contains(&columns));
        let cell = (world[1][0] - world[0][0]) / columns as f32;
        let rows = (world[1][1] - world[0][1]) / cell;
        assert!(
            cell.is_finite()
                && cell > 0.
                && rows.is_finite()
                && (1. ..=4096.).contains(&rows)
                && rows.fract() == 0.
        );
        let count = columns * rows as usize;
        // Acquisition filenames predate the corrected +2c/+30 plane labels.
        let unexplored_complement = evidence("minimap-current-grid.bin");
        let current_visible = evidence("minimap-explored-grid.bin");
        assert_eq!(unexplored_complement.len(), count * 4);
        assert_eq!(current_visible.len(), count * 4);
        let mask = word(&context, 0x3c);
        let bounds = [[0x234, 0x238], [0x230, 0x23c]].map(|pair| {
            std::array::from_fn(|axis| world[0][axis] + word(&object, pair[axis]) as f32 * cell)
        });
        let native_bounds =
            [[0x234, 0x238], [0x230, 0x23c]].map(|p| p.map(|offset| word(&object, offset) as i32));
        let native_pads = [word(&object, 0x220), word(&object, 0x224)];
        assert_eq!(
            draw.points
                .scales(native_bounds, draw.edge, native_pads)
                .unwrap()
                .map(f32::to_bits),
            [word(&object, 0x228), word(&object, 0x22c)]
        );
        let frame = pcx_play::inventory_layout::Rect {
            x: 0.,
            y: 0.,
            width: float(&object, 0x200),
            height: float(&object, 0x1ec),
        };
        let half = draw.hero.half_size(world).unwrap();
        assert_eq!(half.to_bits(), word(&object, 0x50c));
        assert_eq!((-half).to_bits(), word(&object, 0x508));
        let [scale, bias] = draw.hero.projection(frame, bounds).unwrap();
        assert_eq!(
            scale.map(f32::to_bits),
            [word(&object, 0x93c), word(&object, 0x94c)]
        );
        assert_eq!(
            bias.map(f32::to_bits),
            [word(&object, 0x954), word(&object, 0x958)]
        );
        let base = evidence("minimap-base.bgra");
        let native = evidence("minimap-cpu.bgra");
        assert_eq!(base.len(), draw.edge as usize * draw.edge as usize * 4);
        assert_eq!(native.len(), base.len());
        assert_eq!(
            object[0xb4],
            u8::MAX,
            "this native gate requires full frame alpha"
        );
        let opacity = MinimapFogAlpha {
            masked: object[0x9cc],
            fogged: object[0x9d4],
            visible: object[0x9d8],
        };
        let mut raster = Raster::new(draw.edge, draw.edge, 1 << 20).unwrap();
        let pads = raster.pads(bounds).unwrap();
        assert_eq!(pads, [word(&object, 0x220), word(&object, 0x224)]);
        let mut alpha = raster
            .project(
                bounds,
                FogGrid {
                    world_bounds: world,
                    cell_world: cell,
                    fraction_bits: draw.fog_fraction_bits,
                },
                &base
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| p[3])
                    .collect::<Vec<_>>(),
                opacity,
                16 << 20,
                |point| {
                    let x =
                        ((point[0] - f64::from(world[0][0])) / f64::from(cell)).floor() as usize;
                    let y =
                        ((point[1] - f64::from(world[0][1])) / f64::from(cell)).floor() as usize;
                    assert!(x < columns && y < rows as usize);
                    let at = (y * columns + x) * 4;
                    let index = usize::from(!word(&unexplored_complement, at) & mask != 0)
                        + usize::from(word(&current_visible, at) & mask != 0);
                    Ok([
                        VisionState::Masked,
                        VisionState::Fogged,
                        VisionState::Visible,
                    ][index])
                },
            )
            .unwrap()
            .to_vec();
        if pads[0].abs_diff(pads[1]) > draw.border.minimum_pad_difference {
            let border_alpha = object[0x9e3];
            if pads[1] > pads[0] {
                for row in draw
                    .border
                    .rows
                    .iter()
                    .filter_map(|e| e.index(draw.edge, pads[1]))
                {
                    alpha[row as usize * draw.edge as usize
                        ..(row as usize + 1) * draw.edge as usize]
                        .fill(border_alpha);
                }
            } else {
                for column in draw
                    .border
                    .columns
                    .iter()
                    .filter_map(|e| e.index(draw.edge, pads[0]))
                {
                    for row in alpha.chunks_mut(draw.edge as usize) {
                        row[column as usize] = border_alpha;
                    }
                }
            }
        }
        let differences = alpha
            .iter()
            .zip(native.as_chunks::<4>().0)
            .filter(|(a, b)| **a != b[3])
            .count();
        assert_eq!(differences, 0, "full native CPU alpha plane differs");
    }

    #[test]
    fn pointer_inverse_matches_pixels_and_clamps_padding_and_drag() {
        let bounds = [[-100., -40.], [300., 160.]];
        for (width, height) in [(10, 10), (20, 10), (10, 20)] {
            let raster = Raster::new(width, height, 200).unwrap();
            assert_eq!(raster.world_at(bounds, [0.5, 0.5]).unwrap(), [100., 60.]);
            assert_eq!(raster.world_at(bounds, [-1., -1.]).unwrap(), [-100., 160.]);
            assert_eq!(raster.world_at(bounds, [2., 2.]).unwrap(), [300., -40.]);
            let fraction = [0.5 + 0.5 / width as f32, 0.5 + 0.5 / height as f32];
            let center = raster.world_at(bounds, fraction).unwrap();
            let pixel = raster.pixel_of(bounds, center).unwrap();
            assert!((pixel[0] - f64::from(fraction[0]) * f64::from(width)).abs() < 1e-5);
            assert!((pixel[1] - f64::from(fraction[1]) * f64::from(height)).abs() < 1e-5);
            assert!(raster.world_at(bounds, [f32::NAN, 0.]).is_err());
            assert!(raster.world_at([[0.; 2]; 2], [0.5; 2]).is_err());
        }
    }

    #[test]
    fn marker_points_use_the_image_fit_and_refuse_nonfinite_input() {
        let bounds = [[0., 0.], [8., 4.]];
        let raster = Raster::new(8, 8, 64).unwrap();
        assert_eq!(raster.point_at(bounds, [4., 2.]).unwrap(), Some([4., 4.]));
        assert_eq!(
            raster.point_at(bounds, [4.75, 2.25]).unwrap(),
            Some([4., 4.])
        );
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
