//! Ordinary unit markers over the same minimap image and visibility raster.
use super::*;
use pcx_assets::Resolver;
use pcx_play::playercolor_host::ObjectHost;
use std::collections::BTreeMap;

pub(super) struct Markers {
    reader: Arc<Resolver>,
    palette: crate::campaign_actors::TeamPalette,
    size: [f32; 2],
    offset: [f32; 2],
    frame: [f32; 2],
    alpha: f32,
    colors: BTreeMap<i32, [u8; 4]>,
}
impl Markers {
    pub(super) fn read(reader: Arc<Resolver>, rules: &Rules) -> Result<Self, String> {
        let skins = reader
            .resolve_bounded("UI/war3skins.txt", PROFILE_BYTES as u64)
            .map_err(|e| e.to_string())?;
        let skin = crate::menu_skin::SkinTable::parse(
            std::str::from_utf8(&skins.bytes).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let path = skin
            .unique_file("MinimapSprite")
            .map_err(|e| e.to_string())?
            .file;
        let path = pcx_mdl::binary_model_path(&path);
        let model = reader
            .resolve_bounded(&path, 1 << 20)
            .map_err(|e| e.to_string())?;
        let (size, offset, alpha) = sprite(&model.bytes)?;
        Ok(Self {
            palette: crate::campaign_actors::TeamPalette::read(&skins.bytes)?,
            reader,
            size,
            offset,
            frame: [rules.frame.width, rules.frame.height],
            alpha,
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
            let Some(point) = fog.raster.point_at(bounds, position)? else {
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
            let mut color = match host.minimap_role_color(unit)? {
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
            color[3] = (f32::from(color[3]) * self.alpha).round() as u8;
            if plan.len() >= 16384 {
                return Err("minimap marker count budget".into());
            }
            plan.try_reserve(1)
                .map_err(|_| "minimap marker allocation")?;
            let point = [
                point[0] + f64::from(self.offset[0] / self.frame[0]) * f64::from(fog.width),
                point[1] - f64::from(self.offset[1] / self.frame[1]) * f64::from(fog.height),
            ];
            plan.push((point, color));
        }
        // shortcut: the common unit sprite is drawn here; hero/resource/camp and remembered-building layers remain separate work.
        let size = [
            f64::from(self.size[0] / self.frame[0]) * f64::from(fog.width),
            f64::from(self.size[1] / self.frame[1]) * f64::from(fog.height),
        ];
        paint(pixels, [fog.width, fog.height], size, &plan, PIXEL_BYTES)
    }
}

fn sprite(bytes: &[u8]) -> Result<([f32; 2], [f32; 2], f32), String> {
    let model = pcx_mdl::Mdlx::parse(bytes).map_err(|e| e.to_string())?;
    let geosets = model.geosets(bytes).map_err(|e| e.to_string())?;
    let [geoset] = geosets.as_slice() else {
        return Err("minimap unit sprite requires one geoset".into());
    };
    let mesh = model.mesh(bytes, geoset).map_err(|e| e.to_string())?;
    let points = mesh.vertices.iter().collect::<Vec<_>>();
    if points.len() != 4
        || mesh.triangles.triangle_count() != 2
        || !model
            .geoset_anims(bytes)
            .map_err(|e| e.to_string())?
            .is_empty()
        || model
            .bones(bytes)
            .map_err(|e| e.to_string())?
            .iter()
            .any(|b| !b.is_root() || !b.tracks.is_empty())
    {
        return Err("unsupported animated or non-quad minimap unit sprite".into());
    }
    let mut minimum = [f32::INFINITY; 2];
    let mut maximum = [f32::NEG_INFINITY; 2];
    for point in &points {
        if point.iter().any(|n| !n.is_finite()) || point[2] != 0. {
            return Err("non-planar minimap unit sprite".into());
        }
        for axis in 0..2 {
            minimum[axis] = minimum[axis].min(point[axis]);
            maximum[axis] = maximum[axis].max(point[axis]);
        }
    }
    let size = [maximum[0] - minimum[0], maximum[1] - minimum[1]];
    if size.iter().any(|v| !v.is_finite() || *v <= 0.)
        || points
            .iter()
            .any(|p| (0..2).any(|a| p[a] != minimum[a] && p[a] != maximum[a]))
    {
        return Err("minimap sprite is not a finite rectangle".into());
    }
    let mut area = 0.;
    for i in 0..mesh.triangles.triangle_count() {
        let indices = mesh
            .triangles
            .triangle(i)
            .ok_or("minimap sprite triangle")?;
        let [a, b, c] = indices
            .map(|i| points.get(i as usize).copied())
            .map(|p| p.ok_or("minimap sprite index"))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .expect("three vertices");
        area += ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5;
    }
    if (area - size[0] * size[1]).abs() > f32::EPSILON * size[0] * size[1] {
        return Err("minimap sprite does not cover its rectangle".into());
    }
    let materials = model.materials(bytes).map_err(|e| e.to_string())?;
    let layers = materials
        .get(mesh.material_id as usize)
        .ok_or("minimap sprite material")?
        .tex_layers();
    let [layer] = layers.as_slice() else {
        return Err("minimap sprite needs one layer".into());
    };
    if layer.filter_mode != 0
        || !layer.static_alpha.is_finite()
        || !(0.0..=1.0).contains(&layer.static_alpha)
    {
        return Err("unsupported minimap sprite material".into());
    }
    let textures = model.textures(bytes).map_err(|e| e.to_string())?;
    let texture = textures
        .get(layer.texture_id as usize)
        .ok_or("minimap sprite texture")?;
    if texture.slot != 1 || !texture.path.is_empty() {
        return Err("unsupported textured minimap unit sprite".into());
    }
    Ok((size, minimum, layer.static_alpha))
}

fn paint(
    pixels: &mut [u8],
    dimensions: [u32; 2],
    size: [f64; 2],
    plan: &[([f64; 2], [u8; 4])],
    budget: usize,
) -> Result<(), String> {
    let [width, height] = dimensions.map(|v| v as usize);
    if width.checked_mul(height).and_then(|n| n.checked_mul(4)) != Some(pixels.len())
        || size.iter().any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err("invalid minimap marker raster".into());
    }
    let mut rectangles = Vec::new();
    let mut work = 0usize;
    for &(point, color) in plan {
        if point.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite minimap marker".into());
        }
        // shortcut: centre the authored quad on the canonical position; qualify native subpixel coverage with a paired render.
        let lo = [point[0] - size[0] * 0.5, point[1] - size[1] * 0.5];
        let hi = [lo[0] + size[0], lo[1] + size[1]];
        let [x0, y0] = [
            (lo[0] - 0.5).ceil().clamp(0., width as f64) as usize,
            (lo[1] - 0.5).ceil().clamp(0., height as f64) as usize,
        ];
        let [x1, y1] = [
            (hi[0] - 0.5).ceil().clamp(0., width as f64) as usize,
            (hi[1] - 0.5).ceil().clamp(0., height as f64) as usize,
        ];
        work = work
            .checked_add(
                (x1 - x0)
                    .checked_mul(y1 - y0)
                    .ok_or("minimap marker work overflow")?,
            )
            .filter(|n| *n <= budget)
            .ok_or("minimap marker work budget")?;
        rectangles
            .try_reserve(1)
            .map_err(|_| "minimap marker rectangle allocation")?;
        rectangles.push(([x0, y0, x1, y1], color));
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marker_pixels_are_source_colored_clipped_and_budgeted_before_any_write() {
        let mut pixels = vec![0; 4 * 4 * 4];
        let color = [7, 19, 41, 255];
        paint(&mut pixels, [4, 4], [2., 2.], &[([2., 2.], color)], 16).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(
                    &pixels[(y * 4 + x) * 4..(y * 4 + x + 1) * 4],
                    if (1..3).contains(&x) && (1..3).contains(&y) {
                        &color
                    } else {
                        &[0; 4]
                    }
                );
            }
        }
        let before = pixels.clone();
        assert!(
            paint(
                &mut pixels,
                [4, 4],
                [2., 2.],
                &[([2., 2.], color), ([2., 2.], color)],
                4
            )
            .is_err()
        );
        assert_eq!(pixels, before);
        assert!(paint(&mut pixels, [4, 4], [f64::NAN, 2.], &[], 16).is_err());
        paint(
            &mut pixels,
            [4, 4],
            [2., 2.],
            &[([0., 0.], [3, 5, 11, 255])],
            16,
        )
        .unwrap();
        assert_eq!(&pixels[..4], &[3, 5, 11, 255]);
        assert!(sprite(b"MDLX").is_err());
    }
    #[test]
    #[ignore = "requires PCX_MINIMAP_SPRITE from the selected local install"]
    fn selected_original_unit_sprite_is_supported() {
        let path = std::env::var("PCX_MINIMAP_SPRITE").expect("set PCX_MINIMAP_SPRITE");
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() <= 1 << 20);
        let (size, offset, alpha) = sprite(&bytes).unwrap();
        assert!(size.iter().all(|v| *v > 0.));
        println!(
            "source={path} sha256={:02x?} dimensions={size:?} offset={offset:?} alpha={alpha}",
            Sha256::digest(&bytes)
        );
    }
}
