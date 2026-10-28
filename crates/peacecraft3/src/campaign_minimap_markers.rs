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
}
/// One marker: its centre in image pixels, colour and size class.
struct Marker {
    centre: [f64; 2],
    color: [u8; 4],
    class: usize,
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
        for (_, unit) in host.executed_units() {
            if !unit.shown
                || unit.combat.as_ref().is_some_and(|c| c.dead)
                || unit.vitals.as_ref().is_some_and(|v| v.life <= 0.)
            {
                continue;
            }
            match crate::campaign_actors::scalar(&unit.definition, "hideOnMinimap")? {
                1. => continue,
                0. => {}
                _ => return Err("unsupported hideOnMinimap value".into()),
            }
            let position = unit.position.ok_or("minimap marker position absent")?;
            let Some(centre) = fog.raster.point_at(bounds, position)? else {
                continue;
            };
            if settings.fog_enabled || settings.fog_mask_enabled {
                let viewer = host
                    .local_player_settings()
                    .ok_or("minimap viewer is not admitted")?
                    .identity;
                if host
                    .vision_presented(
                        viewer,
                        position,
                        settings.fog_enabled,
                        settings.fog_mask_enabled,
                    )
                    .map_err(|error| error.to_string())?
                    != Some(pcx_play::playercolor_host::VisionState::Visible)
                {
                    continue;
                }
            }
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
            // shortcut: the client's class is a unit-type flag set from a type record; isbldg is the matching UnitBalance column.
            let class = match crate::campaign_actors::scalar(&unit.definition, "isbldg")? {
                1. => 1,
                0. => 0,
                _ => return Err("unsupported isbldg value".into()),
            };
            if plan.len() >= MARKER_COUNT {
                return Err("minimap marker count budget".into());
            }
            plan.try_reserve(1)
                .map_err(|_| "minimap marker allocation")?;
            plan.push(Marker {
                centre,
                color,
                class,
            });
        }
        // shortcut: the client's image is fixed at its own edge; other sizes scale the block, the line widths stay.
        let sides = self.draw.marker_sides.map(|side| {
            ((f64::from(side) * f64::from(fog.width) / f64::from(self.draw.edge)).round() as u32)
                .max(1)
        });
        let pads = fog.raster.pads(bounds)?;
        let dimensions = [fog.width, fog.height];
        paint(pixels, dimensions, pads, sides, &plan, PIXEL_BYTES)?;
        paint_border(pixels, dimensions, pads, &self.draw, self.border_color)
    }
}

/// Square blocks of the marker's class, snapped to whole pixels as the client does: the block
/// starts half its side before the floor of the centre, is kept inside the image, and its
/// bottom stays above the map's bottom padding.
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
        let side = *sides
            .get(marker.class)
            .ok_or("unknown minimap marker class")? as usize;
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
        rectangles.push(([column, row, x1, y1], marker.color));
    }
    for ([x0, y0, x1, y1], color) in rectangles {
        for y in y0..y1 {
            for x in x0..x1 {
                pixels[(y * width + x) * 4..(y * width + x + 1) * 4].copy_from_slice(&color);
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
            &[marker([5.4, 6.7], 0)],
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
            &[marker([15.9, 0.2], 1)],
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
            &[marker([8., 15.5], 1)],
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
