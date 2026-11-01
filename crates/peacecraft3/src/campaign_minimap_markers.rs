//! Unit markers and the map-edge border written into the minimap image after its fog alpha.
use super::*;
use pcx_assets::Resolver;
use pcx_play::{minimap_draw_layout::Draw, playercolor_host::ObjectHost};
use std::collections::BTreeMap;

const MARKER_COUNT: usize = 16384;

pub(super) struct Markers {
    reader: Arc<Resolver>,
    palette: crate::campaign_actors::TeamPalette,
    draw: Draw,
    border_color: [u8; 4],
    colors: BTreeMap<i32, [u8; 4]>,
    /// Definition columns read per unit and definition revision; the lookup scans every field.
    scalars: std::collections::HashMap<(pcx_script::HandleId, u64, &'static str), f32>,
    /// Last visible centre/color/class per building handle, for the
    /// remembered-building layer. Positions update only while the building is
    /// shown, alive and currently visible; fog-hidden buildings redraw from
    /// this memory instead of vanishing. Removed handles are dropped.
    remembered: BTreeMap<pcx_script::HandleId, ([f64; 2], [u8; 4], usize)>,
}
/// One marker: its centre in image pixels, colour and size class.
struct Marker {
    centre: [f64; 2],
    color: [u8; 4],
    class: usize,
    /// Heroes draw one pixel larger per side than their class block.
    enlarged: bool,
}
impl Markers {
    pub(super) fn read(reader: Arc<Resolver>, draw: Draw) -> Result<Self, String> {
        let skins = reader
            .resolve_bounded("UI/war3skins.txt", PROFILE_BYTES as u64)
            .map_err(|e| e.to_string())?;
        let profile = reader
            .resolve_bounded("UI\\MiscData.txt", PROFILE_BYTES as u64)
            .map_err(|e| e.to_string())?;
        let border_color = pcx_objects::editor::minimap_fog::read_argb(
            &profile.bytes,
            PROFILE_BYTES,
            &draw.border.section,
            &draw.border.key,
        )?;
        Ok(Self {
            palette: crate::campaign_actors::TeamPalette::read(&skins.bytes)?,
            reader,
            draw,
            border_color,
            colors: BTreeMap::new(),
            scalars: Default::default(),
            remembered: BTreeMap::new(),
        })
    }
    fn player_color(&mut self, index: i32) -> Result<[u8; 4], String> {
        if let Some(color) = self.colors.get(&index) {
            return Ok(*color);
        }
        let path = self.palette.path(self.reader.paths(), 1, index)?;
        let asset = self
            .reader
            .resolve_bounded(&path, 1 << 20)
            .map_err(|e| e.to_string())?;
        let image =
            pcx_gfx::load_image_bounded(&asset.bytes, &path, 1 << 20).map_err(|e| e.to_string())?;
        let color = image
            .rgba
            .get(..4)
            .ok_or("empty team-color image")?
            .try_into()
            .expect("four channels");
        // The selected palette initializer reads the first decoded texel too.
        self.colors.insert(index, color);
        Ok(color)
    }
    fn scalar(
        &mut self,
        id: pcx_script::HandleId,
        unit: &pcx_play::playercolor_host::ExecutedUnit,
        column: &'static str,
    ) -> Result<f32, String> {
        let key = (id, unit.definition_revision, column);
        if let Some(value) = self.scalars.get(&key) {
            return Ok(*value);
        }
        let value = crate::campaign_actors::scalar(&unit.definition, column)?;
        self.scalars.insert(key, value);
        Ok(value)
    }
    pub(super) fn draw(
        &mut self,
        host: &ObjectHost,
        fog: &FogImage,
        pixels: &mut [u8],
    ) -> Result<(), String> {
        let settings = host.startup_settings().ok_or(SETTINGS_ERROR)?;
        let bounds = host
            .vision_world_bounds()
            .ok_or("minimap marker world extent absent")?;
        let mut plan = Vec::new();
        let mut live: std::collections::BTreeSet<pcx_script::HandleId> =
            std::collections::BTreeSet::new();
        for (id, unit) in host.executed_units() {
            if !unit.shown
                || unit.combat.as_ref().is_some_and(|c| c.dead)
                || unit.vitals.as_ref().is_some_and(|v| v.life <= 0.)
            {
                continue;
            }
            match self.scalar(id, unit, "hideOnMinimap")? {
                1. => continue,
                0. => {}
                _ => return Err("unsupported hideOnMinimap value".into()),
            }
            let position = unit.position.ok_or("minimap marker position absent")?;
            let Some(centre) = fog.raster.point_at(bounds, position)? else {
                continue;
            };
            let visible = if settings.fog_enabled || settings.fog_mask_enabled {
                let viewer = host
                    .local_player_settings()
                    .ok_or("minimap viewer is not admitted")?
                    .identity;
                host.vision_presented(
                    viewer,
                    position,
                    settings.fog_enabled,
                    settings.fog_mask_enabled,
                )
                .map_err(|error| error.to_string())?
                    == Some(pcx_play::playercolor_host::VisionState::Visible)
            } else {
                true
            };
            let color = match host.minimap_role_color(unit)? {
                Some(color) => color,
                None => {
                    let owner = host
                        .player_settings(unit.owner)
                        .map_err(|e| e.to_string())?;
                    let identity = owner.color.ok_or("minimap owner color absent")?;
                    let index = host
                        .inner()
                        .player_color(identity)
                        .map_err(|e| e.to_string())?;
                    self.player_color(index)?
                }
            };
            // The client's class is a unit-type flag set from a type record;
            // isbldg is the matching UnitBalance column. Heroes keep the unit
            // class but draw enlarged; resources and creep camps draw with the
            // building class so they read as structures on the minimap. These
            // are presentation choices; the role color is unchanged.
            let base_class = match self.scalar(id, unit, "isbldg")? {
                1. => 1,
                0. => 0,
                _ => return Err("unsupported isbldg value".into()),
            };
            let layer = host.minimap_special_layer(unit);
            let class = match layer {
                pcx_play::ally_color::SpecialMinimapLayer::Resource
                | pcx_play::ally_color::SpecialMinimapLayer::Camp => 1,
                _ => base_class,
            };
            let enlarged = matches!(layer, pcx_play::ally_color::SpecialMinimapLayer::Hero);
            if plan.len() >= MARKER_COUNT {
                return Err("minimap marker count budget".into());
            }
            if !visible {
                // Remembered-building layer: fog-hidden buildings redraw from
                // their last visible centre/color/class instead of vanishing.
                // Non-buildings leave no memory; their outdated positions must
                // not linger.
                if base_class == 1
                    && let Some((centre, color, class)) = self.remembered.get(&id)
                {
                    plan.try_reserve(1)
                        .map_err(|_| "minimap marker allocation")?;
                    plan.push(Marker {
                        centre: *centre,
                        color: *color,
                        class: *class,
                        enlarged: false,
                    });
                }
                continue;
            }
            live.insert(id);
            if base_class == 1 {
                self.remembered.insert(id, (centre, color, class));
            }
            plan.try_reserve(1)
                .map_err(|_| "minimap marker allocation")?;
            plan.push(Marker {
                centre,
                color,
                class,
                enlarged,
            });
        }
        self.remembered.retain(|id, _| live.contains(id));
        // The client's image is fixed at its own edge; other sizes scale the block, the line widths stay.
        let sides = self.draw.marker_sides.map(|side| {
            ((f64::from(side) * f64::from(fog.width) / f64::from(self.draw.edge)).round() as u32)
                .max(1)
        });
        let pads = fog.raster.pads(bounds)?;
        let dimensions = [fog.width, fog.height];
        paint(pixels, dimensions, pads, sides, &plan, PIXEL_BYTES)?;
        paint_border(pixels, dimensions, pads, &self.draw, self.border_color)
    }

    /// Which drawn marker block contains `pixel` (image pixels, same space as
    /// [`Raster::point_at`](crate::campaign_visibility::Raster::point_at)).
    /// The hit-test reuses the drawn blocks from [`paint`]: the same
    /// shown/alive/`hideOnMinimap`/`isbldg`/visibility gates, the same scaled
    /// sides with the hero enlargement, and the same floor-then-clamp placement
    /// with the bottom held above the map padding. Remembered fog-hidden
    /// buildings hit-test from their last visible block. Overlapping blocks
    /// resolve to the nearest centre.
    pub(super) fn icon_at(
        &mut self,
        host: &ObjectHost,
        fog: &FogImage,
        pixel: [f64; 2],
    ) -> Result<Option<pcx_script::HandleId>, String> {
        if pixel.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite minimap pointer".into());
        }
        let settings = host.startup_settings().ok_or(SETTINGS_ERROR)?;
        let bounds = host
            .vision_world_bounds()
            .ok_or("minimap marker world extent absent")?;
        let sides = self.draw.marker_sides.map(|side| {
            ((f64::from(side) * f64::from(fog.width) / f64::from(self.draw.edge)).round() as u32)
                .max(1)
        });
        let pads = fog.raster.pads(bounds)?;
        let [width, height] = [fog.width as usize, fog.height as usize];
        let mut best: Option<(pcx_script::HandleId, f64)> = None;
        let mut count = 0usize;
        for (id, unit) in host.executed_units() {
            if !unit.shown
                || unit.combat.as_ref().is_some_and(|c| c.dead)
                || unit.vitals.as_ref().is_some_and(|v| v.life <= 0.)
            {
                continue;
            }
            match self.scalar(id, unit, "hideOnMinimap")? {
                1. => continue,
                0. => {}
                _ => return Err("unsupported hideOnMinimap value".into()),
            }
            let position = unit.position.ok_or("minimap marker position absent")?;
            let Some(centre) = fog.raster.point_at(bounds, position)? else {
                continue;
            };
            let visible = if settings.fog_enabled || settings.fog_mask_enabled {
                let viewer = host
                    .local_player_settings()
                    .ok_or("minimap viewer is not admitted")?
                    .identity;
                host.vision_presented(
                    viewer,
                    position,
                    settings.fog_enabled,
                    settings.fog_mask_enabled,
                )
                .map_err(|error| error.to_string())?
                    == Some(pcx_play::playercolor_host::VisionState::Visible)
            } else {
                true
            };
            let base_class = match self.scalar(id, unit, "isbldg")? {
                1. => 1,
                0. => 0,
                _ => return Err("unsupported isbldg value".into()),
            };
            let layer = host.minimap_special_layer(unit);
            let (centre, class, enlarged) = if visible {
                let class = match layer {
                    pcx_play::ally_color::SpecialMinimapLayer::Resource
                    | pcx_play::ally_color::SpecialMinimapLayer::Camp => 1,
                    _ => base_class,
                };
                let enlarged = matches!(layer, pcx_play::ally_color::SpecialMinimapLayer::Hero);
                (centre, class, enlarged)
            } else if base_class == 1
                && let Some((centre, _, class)) = self.remembered.get(&id)
            {
                (*centre, *class, false)
            } else {
                continue;
            };
            count += 1;
            if count > MARKER_COUNT {
                return Err("minimap marker count budget".into());
            }
            if centre.iter().any(|v| !v.is_finite()) {
                return Err("nonfinite minimap marker".into());
            }
            let mut side = *sides.get(class).ok_or("unknown minimap marker class")? as usize;
            if enlarged {
                side = side.saturating_add(2);
            }
            let half = (side / 2) as i64;
            let column = (centre[0].floor() as i64 - half)
                .clamp(0, width.saturating_sub(side) as i64) as usize;
            let last_row = height
                .saturating_sub(1 + pads[1] as usize)
                .saturating_sub(side);
            let row = (centre[1].floor() as i64 - half).clamp(0, last_row as i64) as usize;
            let (x1, y1) = ((column + side).min(width), (row + side).min(height));
            if pixel[0] < column as f64
                || pixel[0] >= x1 as f64
                || pixel[1] < row as f64
                || pixel[1] >= y1 as f64
            {
                continue;
            }
            let distance = (pixel[0] - centre[0]).powi(2) + (pixel[1] - centre[1]).powi(2);
            if best.is_none_or(|(_, known)| distance < known) {
                best = Some((id, distance));
            }
        }
        Ok(best.map(|(id, _)| id))
    }
}

/// Square blocks of the marker's class, snapped to whole pixels as the client does: the block
/// starts half its side before the floor of the centre, is kept inside the image, and its
/// bottom stays above the map's bottom padding. Native subpixel coverage is a
/// reconstruction: the exact overlap of the clamp-shifted marker square with
/// each snapped pixel drives a straight source-over blend, while integer
/// centres and clamped blocks keep the exact solid fill. Heroes draw one pixel
/// larger per side.
fn paint(
    pixels: &mut [u8],
    dimensions: [u32; 2],
    pads: [u32; 2],
    sides: [u32; 2],
    plan: &[Marker],
    budget: usize,
) -> Result<(), String> {
    let [width, height] = dimensions.map(|v| v as usize);
    if width.checked_mul(height).and_then(|n| n.checked_mul(4)) != Some(pixels.len()) {
        return Err("invalid minimap marker raster".into());
    }
    let mut rectangles = Vec::new();
    let mut work = 0usize;
    for marker in plan {
        if marker.centre.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite minimap marker".into());
        }
        let mut side = *sides
            .get(marker.class)
            .ok_or("unknown minimap marker class")? as usize;
        if marker.enlarged {
            side = side.saturating_add(2);
        }
        let half = (side / 2) as i64;
        let column = (marker.centre[0].floor() as i64 - half)
            .clamp(0, width.saturating_sub(side) as i64) as usize;
        let last_row = height
            .saturating_sub(1 + pads[1] as usize)
            .saturating_sub(side);
        let row = (marker.centre[1].floor() as i64 - half).clamp(0, last_row as i64) as usize;
        let (x1, y1) = ((column + side).min(width), (row + side).min(height));
        work = work
            .checked_add((x1 - column) * (y1 - row))
            .filter(|n| *n <= budget)
            .ok_or("minimap marker work budget")?;
        rectangles
            .try_reserve(1)
            .map_err(|_| "minimap marker rectangle allocation")?;
        let side_f = side as f64;
        let half_f = half as f64;
        let dx = column as f64 - (marker.centre[0].floor() - half_f);
        let dy = row as f64 - (marker.centre[1].floor() - half_f);
        let square = [
            marker.centre[0] - side_f / 2.0 + dx,
            marker.centre[1] - side_f / 2.0 + dy,
            marker.centre[0] + side_f / 2.0 + dx,
            marker.centre[1] + side_f / 2.0 + dy,
        ];
        rectangles.push(([column, row, x1, y1], marker.color, square));
    }
    for ([x0, y0, x1, y1], color, square) in rectangles {
        for y in y0..y1 {
            for x in x0..x1 {
                let overlap = |pixel: usize, lo: f64, hi: f64| {
                    (f64::min(pixel as f64 + 1.0, hi) - f64::max(pixel as f64, lo)).clamp(0.0, 1.0)
                };
                let coverage = (overlap(x, square[0], square[2])
                    * overlap(y, square[1], square[3]))
                .clamp(0.0, 1.0);
                let at = (y * width + x) * 4;
                if coverage >= 1.0 - f64::EPSILON {
                    pixels[at..at + 4].copy_from_slice(&color);
                } else {
                    let dst = pixels[at..at + 4].to_owned();
                    for channel in 0..4 {
                        let blended = f64::from(dst[channel]) * (1.0 - coverage)
                            + f64::from(color[channel]) * coverage;
                        pixels[at + channel] = blended.round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }
    Ok(())
}

/// The client draws the lines last, across the whole image and over the fog alpha, when the
/// map's padding on the two axes differs enough. Wider maps get rows, taller maps columns.
fn paint_border(
    pixels: &mut [u8],
    dimensions: [u32; 2],
    pads: [u32; 2],
    draw: &Draw,
    color: [u8; 4],
) -> Result<(), String> {
    let [width, height] = dimensions;
    if (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        != Some(pixels.len())
    {
        return Err("invalid minimap border raster".into());
    }
    if pads[0].abs_diff(pads[1]) <= draw.border.minimum_pad_difference {
        return Ok(());
    }
    let width = width as usize;
    if pads[1] > pads[0] {
        for row in draw
            .border
            .rows
            .iter()
            .filter_map(|edge| edge.index(height, pads[1]))
        {
            let start = row as usize * width * 4;
            for pixel in pixels[start..start + width * 4].as_chunks_mut::<4>().0 {
                *pixel = color;
            }
        }
    } else {
        for column in draw
            .border
            .columns
            .iter()
            .filter_map(|edge| edge.index(width as u32, pads[0]))
        {
            for row in pixels.as_chunks_mut::<4>().0.chunks_mut(width) {
                row[column as usize] = color;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_play::minimap_draw_layout::{Border, Edge};

    fn draw(minimum_pad_difference: u32) -> Draw {
        Draw {
            origin: pcx_play::scripts::AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            image_sha256: [0; 32],
            edge: 16,
            border: Border {
                section: "S".into(),
                key: "K".into(),
                minimum_pad_difference,
                columns: vec![
                    Edge::PadPlus(0),
                    Edge::PadPlus(-1),
                    Edge::EdgeMinusPad(1),
                    Edge::EdgeMinusPad(2),
                ],
                rows: vec![
                    Edge::PadPlus(-1),
                    Edge::PadPlus(0),
                    Edge::EdgeMinusPad(0),
                    Edge::EdgeMinusPad(-1),
                ],
            },
            marker_sides: [2, 4],
            outline_rgb: [1, 2, 3],
        }
    }
    fn marked(pixels: &[u8], width: usize, color: [u8; 4]) -> Vec<[usize; 2]> {
        let mut found = Vec::new();
        for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
            if *pixel == color {
                found.push([index % width, index / width]);
            }
        }
        found
    }
    fn marker(centre: [f64; 2], class: usize) -> Marker {
        Marker {
            centre,
            color: [7, 19, 41, 255],
            class,
            enlarged: false,
        }
    }

    #[test]
    fn marker_blocks_follow_the_class_side_and_the_client_clamps() {
        let color = [7, 19, 41, 255];
        let mut pixels = vec![0; 16 * 16 * 4];
        paint(
            &mut pixels,
            [16, 16],
            [0, 3],
            [2, 4],
            &[marker([5., 6.], 0)],
            64,
        )
        .unwrap();
        assert_eq!(marked(&pixels, 16, color), [[4, 5], [5, 5], [4, 6], [5, 6]]);
        // The right edge clamps to the image, the top to row 0.
        let mut pixels = vec![0; 16 * 16 * 4];
        paint(
            &mut pixels,
            [16, 16],
            [0, 3],
            [2, 4],
            &[marker([15., 0.], 1)],
            64,
        )
        .unwrap();
        let found = marked(&pixels, 16, color);
        assert_eq!(found.len(), 16);
        assert_eq!(found[0], [12, 0]);
        assert_eq!(found[15], [15, 3]);
        // The bottom stops above the map's bottom padding: 16 - 1 - 3 - 4 = row 8.
        let mut pixels = vec![0; 16 * 16 * 4];
        paint(
            &mut pixels,
            [16, 16],
            [0, 3],
            [2, 4],
            &[marker([8., 15.], 1)],
            64,
        )
        .unwrap();
        let found = marked(&pixels, 16, color);
        assert_eq!((found[0], found[15]), ([6, 8], [9, 11]));
    }

    #[test]
    fn marker_refusals_leave_the_pixels_untouched() {
        let mut pixels = vec![9; 16 * 16 * 4];
        let before = pixels.clone();
        let two = [marker([4., 4.], 0), marker([9., 9.], 0)];
        assert!(paint(&mut pixels, [16, 16], [0, 0], [2, 4], &two, 7).is_err());
        assert!(
            paint(
                &mut pixels,
                [16, 16],
                [0, 0],
                [2, 4],
                &[marker([1., 1.], 2)],
                64
            )
            .is_err()
        );
        assert!(
            paint(
                &mut pixels,
                [16, 16],
                [0, 0],
                [2, 4],
                &[marker([f64::NAN, 1.], 0)],
                64
            )
            .is_err()
        );
        assert!(paint(&mut pixels, [15, 16], [0, 0], [2, 4], &[], 64).is_err());
        assert_eq!(pixels, before);
    }

    #[test]
    fn wider_maps_get_rows_and_taller_maps_get_columns_over_the_whole_image() {
        let color = [60, 61, 70, 255];
        let mut pixels = vec![0; 16 * 16 * 4];
        paint_border(&mut pixels, [16, 16], [0, 5], &draw(2), color).unwrap();
        let found = marked(&pixels, 16, color);
        assert_eq!(found.len(), 4 * 16);
        for row in [4, 5, 11, 12] {
            assert!((0..16).all(|x| found.contains(&[x, row])));
        }
        let mut pixels = vec![0; 16 * 16 * 4];
        paint_border(&mut pixels, [16, 16], [4, 1], &draw(2), color).unwrap();
        let found = marked(&pixels, 16, color);
        assert_eq!(found.len(), 4 * 16);
        for column in [3, 4, 10, 11] {
            assert!((0..16).all(|y| found.contains(&[column, y])));
        }
    }

    #[test]
    fn border_waits_for_a_padding_difference_above_the_threshold() {
        let color = [60, 61, 70, 255];
        for pads in [[0, 2], [2, 0], [3, 3]] {
            let mut pixels = vec![0; 16 * 16 * 4];
            paint_border(&mut pixels, [16, 16], pads, &draw(2), color).unwrap();
            assert!(marked(&pixels, 16, color).is_empty(), "{pads:?}");
        }
        let mut pixels = vec![0; 16 * 16 * 4];
        paint_border(&mut pixels, [16, 16], [0, 3], &draw(2), color).unwrap();
        assert!(!marked(&pixels, 16, color).is_empty());
        // Lines that fall outside the image are dropped, not wrapped.
        let mut pixels = vec![0; 16 * 16 * 4];
        paint_border(&mut pixels, [16, 16], [0, 16], &draw(2), color).unwrap();
        assert_eq!(marked(&pixels, 16, color).len(), 16 * 3);
        assert!(paint_border(&mut pixels, [15, 16], [0, 3], &draw(2), color).is_err());
    }
}
