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
    resource_images: [pcx_gfx::LoadedTexture; 3],
    hero_image: pcx_gfx::LoadedTexture,
    hero_texture: Option<Handle<Image>>,
    hero_nodes: BTreeMap<pcx_script::HandleId, Entity>,
    heroes: Vec<(pcx_script::HandleId, [f32; 2], [u8; 4])>,
    hero_phase: f32,
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
}
fn texture(
    reader: &Resolver,
    skin: &crate::menu_skin::SkinTable,
    key: &str,
    image_members: &[String],
) -> Result<pcx_gfx::LoadedTexture, String> {
    let path = skin.unique_file(key).map_err(|e| e.to_string())?.file;
    let (stem, _) = path
        .rsplit_once('.')
        .ok_or_else(|| format!("minimap texture has no extension: {path}"))?;
    let mut selected = None;
    for member in image_members {
        let (_, extension) = member
            .rsplit_once('.')
            .ok_or("minimap source extension absent")?;
        let candidate = format!("{stem}.{extension}");
        match reader.resolve_bounded(&candidate, 1 << 20) {
            Ok(asset) => {
                selected = Some((candidate, asset));
                break;
            }
            Err(pcx_assets::Error::MissingAsset { .. }) => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    let (selected_path, asset) =
        selected.ok_or_else(|| format!("minimap texture {path} missing in native lookup order"))?;
    let image = pcx_gfx::load_image_bounded(&asset.bytes, &selected_path, 1 << 20)
        .map_err(|e| e.to_string())?;
    Ok(image)
}

impl Markers {
    pub(super) fn read(
        reader: Arc<Resolver>,
        draw: Draw,
        image_members: &[String],
    ) -> Result<Self, String> {
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
        let skin = crate::menu_skin::SkinTable::parse(
            std::str::from_utf8(&skins.bytes).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let mut resource_images = Vec::new();
        for key in &draw.resources.keys {
            let image = texture(&reader, &skin, key, image_members)?;
            if image.width != draw.resources.side || image.height != draw.resources.side {
                return Err(format!(
                    "minimap resource texture {} dimensions differ from native blit",
                    image.source
                ));
            }
            resource_images.push(image);
        }
        let resource_images = resource_images
            .try_into()
            .map_err(|_| "minimap resource texture count")?;
        let hero_image = texture(&reader, &skin, &draw.hero.key, image_members)?;
        let hero_phase = draw.hero.initial_phase;
        Ok(Self {
            palette: crate::campaign_actors::TeamPalette::read(&skins.bytes)?,
            reader,
            draw,
            resource_images,
            hero_image,
            hero_texture: None,
            hero_nodes: BTreeMap::new(),
            heroes: Vec::new(),
            hero_phase,
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
    fn centre(
        &self,
        host: &ObjectHost,
        fog: &FogImage,
        bounds: [[f32; 2]; 2],
        position: [f32; 2],
        hero: bool,
    ) -> Result<Option<[f64; 2]>, String> {
        if hero {
            return fog.raster.point_at(bounds, position);
        }
        if [fog.width, fog.height] != [self.draw.edge; 2] {
            return Err("minimap CPU image differs from the native marker edge".into());
        }
        let placement = host
            .unit_placement_rules()
            .ok_or("minimap marker placement rules absent")?;
        if placement.binding().image_sha256 != self.draw.image_sha256 {
            return Err("minimap marker placement source differs".into());
        }
        let origin = host
            .vision_world_bounds()
            .ok_or("minimap marker full-map bounds absent")?[0];
        let cell = placement.cell_size();
        let convert = |point: [f32; 2]| -> Result<[i32; 2], String> {
            let mut result = [0; 2];
            for axis in 0..2 {
                let value = ((point[axis] - origin[axis]) / cell).floor();
                if !value.is_finite()
                    || f64::from(value) < f64::from(i32::MIN)
                    || f64::from(value) > f64::from(i32::MAX)
                {
                    return Err("minimap grid coordinate outside integer range".into());
                }
                result[axis] = (value as i32) >> self.draw.points.grid_shift;
            }
            Ok(result)
        };
        self.draw.points.project(
            [convert(bounds[0])?, convert(bounds[1])?],
            self.draw.edge,
            fog.raster.pads(bounds)?,
            convert(position)?,
        )
    }

    pub(super) fn draw(
        &mut self,
        host: &ObjectHost,
        fog: &FogImage,
        pixels: &mut [u8],
    ) -> Result<(), String> {
        let settings = host.startup_settings().ok_or(SETTINGS_ERROR)?;
        let bounds = fog.bounds(host)?;
        let mut plan = Vec::new();
        let mut resources = Vec::new();
        let mut heroes = Vec::new();
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
            let layer = host.minimap_special_layer(unit);
            let position = unit.position.ok_or("minimap marker position absent")?;
            let Some(centre) = self.centre(
                host,
                fog,
                bounds,
                position,
                matches!(layer, pcx_play::ally_color::SpecialMinimapLayer::Hero),
            )?
            else {
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
            let base_class = match self.scalar(id, unit, "isbldg")? {
                1. => 1,
                0. => 0,
                _ => return Err("unsupported isbldg value".into()),
            };
            if matches!(layer, pcx_play::ally_color::SpecialMinimapLayer::Resource) {
                if visible {
                    let kind = host.minimap_resource_kind(unit, self.draw.resources.classes)?;
                    if resources.len() >= MARKER_COUNT {
                        return Err("minimap resource count budget".into());
                    }
                    resources.push((centre, kind, color));
                }
                continue;
            }
            if matches!(layer, pcx_play::ally_color::SpecialMinimapLayer::Hero) {
                if visible {
                    if heroes.len() >= MARKER_COUNT {
                        return Err("minimap hero count budget".into());
                    }
                    heroes.push((id, position, color));
                }
                continue;
            }
            let class = base_class;
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
        paint_resources(
            pixels,
            dimensions,
            pads,
            &self.draw.resources,
            &self.resource_images,
            &resources,
            PIXEL_BYTES,
        )?;
        paint(pixels, dimensions, pads, sides, &plan, PIXEL_BYTES)?;
        paint_border(pixels, dimensions, pads, &self.draw, self.border_color)?;
        self.heroes = heroes;
        Ok(())
    }

    pub(super) fn retire(&mut self, images: &mut Assets<Image>) {
        if let Some(image) = self.hero_texture.take() {
            images.remove(image.id());
        }
    }

    pub(super) fn sync_heroes(
        &mut self,
        commands: &mut Commands,
        images: &mut Assets<Image>,
        fog: &FogImage,
        host: &ObjectHost,
        frame: (Entity, Entity, bool, f32),
    ) -> Result<(), String> {
        let (root, parent, shown, delta) = frame;
        let alpha = self.draw.hero.advance(&mut self.hero_phase, delta, shown)?;
        let mapping = fog.mapping.as_ref().ok_or("minimap hero mapping absent")?;
        let bounds = fog.bounds(host)?;
        let full = host
            .vision_world_bounds()
            .ok_or("minimap hero full-map bounds absent")?;
        if self.hero_texture.is_none() {
            let mut image = crate::menu_composition::image(&self.hero_image);
            image.sampler = bevy::image::ImageSampler::Descriptor(image_sampler(
                self.draw.hero.filters,
                self.draw.hero.addresses,
            )?);
            self.hero_texture = Some(images.add(image));
        }
        let image = self.hero_texture.as_ref().expect("hero texture loaded");
        let [u0, v0, u1, v1] = self.draw.hero.uv;
        let uv = Rect::from_corners(
            Vec2::new(
                u0.min(u1) * self.hero_image.width as f32,
                v0.min(v1) * self.hero_image.height as f32,
            ),
            Vec2::new(
                u0.max(u1) * self.hero_image.width as f32,
                v0.max(v1) * self.hero_image.height as f32,
            ),
        );
        let live: std::collections::BTreeSet<_> =
            self.heroes.iter().map(|(id, _, _)| *id).collect();
        self.hero_nodes.retain(|id, entity| {
            let retained = live.contains(id);
            if !retained {
                commands.entity(*entity).try_despawn();
            }
            retained
        });
        for (id, position, color) in &self.heroes {
            let rect = self
                .draw
                .hero
                .quad(mapping.frame, bounds, full, *position)?;
            let mut rgba = color.map(|v| f32::from(v) / f32::from(u8::MAX));
            rgba[3] *= f32::from(alpha) / f32::from(u8::MAX);
            let art = ImageNode {
                image: image.clone(),
                color: crate::menu_composition::colour(rgba),
                rect: Some(uv),
                flip_x: u0 > u1,
                flip_y: v0 > v1,
                ..default()
            };
            let source = image_rect(rect, mapping.viewport, root);
            if let Some(entity) = self.hero_nodes.get(id) {
                commands.entity(*entity).insert((source, art));
            } else {
                let entity = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.),
                            height: Val::Percent(100.),
                            display: if shown { Display::Flex } else { Display::None },
                            ..default()
                        },
                        art,
                        source,
                        MinimapPart::Image,
                        ZIndex(1),
                        Pickable::IGNORE,
                        ChildOf(parent),
                    ))
                    .id();
                self.hero_nodes.insert(*id, entity);
            }
        }
        Ok(())
    }

    /// Which drawn marker block contains `pixel` (image pixels, same space as
    /// [`Raster::point_at`](crate::campaign_visibility::Raster::point_at)).
    /// The hit-test reuses the drawn blocks from [`paint`]: the same
    /// shown/alive/`hideOnMinimap`/`isbldg`/visibility gates, the same scaled
    /// sides and floor-then-clamp placement; heroes use their separate UI quads,
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
        let bounds = fog.bounds(host)?;
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
            let layer = host.minimap_special_layer(unit);
            let position = unit.position.ok_or("minimap marker position absent")?;
            let Some(centre) = self.centre(
                host,
                fog,
                bounds,
                position,
                matches!(layer, pcx_play::ally_color::SpecialMinimapLayer::Hero),
            )?
            else {
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
            if matches!(layer, pcx_play::ally_color::SpecialMinimapLayer::Hero) {
                if !visible {
                    continue;
                }
                let mapping = fog.mapping.as_ref().ok_or("minimap hero mapping absent")?;
                let full = host
                    .vision_world_bounds()
                    .ok_or("minimap hero full-map bounds absent")?;
                let rect = self.draw.hero.quad(mapping.frame, bounds, full, position)?;
                let local = [
                    mapping.frame.x + pixel[0] as f32 / fog.width as f32 * mapping.frame.width,
                    mapping.frame.y
                        + (1. - pixel[1] as f32 / fog.height as f32) * mapping.frame.height,
                ];
                if local[0] < rect.x
                    || local[0] >= rect.x + rect.width
                    || local[1] < rect.y
                    || local[1] >= rect.y + rect.height
                {
                    continue;
                }
                let centre = [
                    f64::from((rect.x + rect.width * 0.5 - mapping.frame.x) / mapping.frame.width)
                        * f64::from(fog.width),
                    f64::from(
                        1. - (rect.y + rect.height * 0.5 - mapping.frame.y) / mapping.frame.height,
                    ) * f64::from(fog.height),
                ];
                let distance = (pixel[0] - centre[0]).powi(2) + (pixel[1] - centre[1]).powi(2);
                if best.is_none_or(|(_, known)| distance < known) {
                    best = Some((id, distance));
                }
                continue;
            }
            let (centre, class) = if visible {
                (centre, base_class)
            } else if base_class == 1
                && let Some((centre, _, class)) = self.remembered.get(&id)
            {
                (*centre, *class)
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
            let resource = matches!(layer, pcx_play::ally_color::SpecialMinimapLayer::Resource);
            let side = if resource {
                self.draw.resources.side as usize
            } else {
                *sides.get(class).ok_or("unknown minimap marker class")? as usize
            };
            let half = (side / 2) as i64;
            let right = if resource {
                width.saturating_sub(
                    (self.draw.resources.clamp_inset as usize).saturating_add(pads[0] as usize),
                )
            } else {
                width.saturating_sub(side)
            };
            let column = (centre[0].floor() as i64 - half).clamp(0, right as i64) as usize;
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

fn paint_resources(
    pixels: &mut [u8],
    dimensions: [u32; 2],
    pads: [u32; 2],
    rules: &pcx_play::minimap_draw_layout::ResourceIcons,
    images: &[pcx_gfx::LoadedTexture; 3],
    plan: &[([f64; 2], usize, [u8; 4])],
    budget: usize,
) -> Result<(), String> {
    let [width, height] = dimensions.map(|v| v as usize);
    let side = rules.side as usize;
    if width.checked_mul(height).and_then(|n| n.checked_mul(4)) != Some(pixels.len())
        || side == 0
        || side > width
        || side > height
        || rules.clamp_inset < rules.side
        || plan
            .len()
            .checked_mul(side)
            .and_then(|n| n.checked_mul(side))
            .is_none_or(|n| n > budget)
        || plan
            .iter()
            .any(|(point, kind, _)| point.iter().any(|v| !v.is_finite()) || *kind >= images.len())
        || images.iter().any(|image| {
            image.width != rules.side
                || image.height != rules.side
                || image.rgba.len() != side * side * 4
        })
    {
        return Err("invalid minimap resource raster or work budget".into());
    }
    for (centre, kind, color) in plan {
        let half = i64::from(rules.half);
        let max_x =
            width.saturating_sub((rules.clamp_inset as usize).saturating_add(pads[0] as usize));
        let max_y =
            height.saturating_sub((rules.clamp_inset as usize).saturating_add(pads[1] as usize));
        let x = (centre[0].floor() as i64 - half).clamp(0, max_x as i64) as usize;
        let y = (centre[1].floor() as i64 - half).clamp(0, max_y as i64) as usize;
        for row in 0..side {
            for column in 0..side {
                let source: [u8; 4] = images[*kind].rgba[(row * side + column) * 4..][..4]
                    .try_into()
                    .expect("validated source pixel");
                let mut source = if *kind == 0 {
                    source
                } else {
                    rules.tint(source, *color)
                };
                let mask = source[3];
                source[3] = 255;
                let at = ((y + row) * width + x + column) * 4;
                for channel in 0..4 {
                    pixels[at + channel] =
                        (pixels[at + channel] & !mask) | (source[channel] & mask);
                }
            }
        }
    }
    Ok(())
}

/// Solid blocks written into the CPU image; only the final UI texture sampler filters them.
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
            let start = (y * width + x0) * 4;
            let end = (y * width + x1) * 4;
            for pixel in pixels[start..end].as_chunks_mut::<4>().0 {
                *pixel = color;
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
            fog_fraction_bits: 8,
            points: pcx_play::minimap_draw_layout::PointProjection {
                grid_shift: 2,
                bias: 512.,
                bit_shift: 14,
            },
            hero: pcx_play::minimap_draw_layout::Hero {
                key: "Champion".into(),
                filters: [2, 2, 1],
                addresses: [1, 1],
                uv: [0., 0., 1., 1.],
                initial_phase: 0.,
                reset_phase: 0.,
                midpoint: 1.,
                period: 2.,
                alpha_scale: 100.,
                alpha_floor: 50.,
                geometry_edge: 16.,
                world_scale: 0.25,
                size_scale: 0.02,
                half: 0.5,
            },
            resources: pcx_play::minimap_draw_layout::ResourceIcons {
                keys: ["Mine".into(), "Roots".into(), "Ghost".into()],
                classes: [*b"Mine", *b"Root", *b"Ghst"],
                side: 4,
                half: 2,
                clamp_inset: 5,
                luminance: [77, 151, 28],
                color_shift: 8,
                tint_bias: 255,
            },
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
            outline_minimum_separation: 0.125,
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
    fn resources_mask_channels_tint_variants_and_clamp_to_map_padding() {
        let rules = draw(2).resources;
        let images = std::array::from_fn(|_| pcx_gfx::LoadedTexture {
            width: 4,
            height: 4,
            rgba: [240, 80, 16, 128].repeat(16),
            mipmaps: vec![],
            source: "synthetic resource".into(),
        });
        let mut pixels = vec![85; 16 * 16 * 4];
        paint_resources(
            &mut pixels,
            [16, 16],
            [1, 2],
            &rules,
            &images,
            &[([15.75, 15.25], 0, [255; 4])],
            16,
        )
        .unwrap();
        // Clamp origins are 16 - 5 - padding, independent of the fractional centre.
        let expected = [213, 85, 85, 213];
        assert_eq!(&pixels[(9 * 16 + 10) * 4..][..4], &expected);
        assert_eq!(&pixels[(12 * 16 + 13) * 4..][..4], &expected);
        assert_eq!(&pixels[(8 * 16 + 10) * 4..][..4], &[85; 4]);
        let mut pixels = vec![0; 16 * 16 * 4];
        let color = [64, 255, 128, 255];
        paint_resources(
            &mut pixels,
            [16, 16],
            [0, 0],
            &rules,
            &images,
            &[([4., 4.], 1, color)],
            16,
        )
        .unwrap();
        let tint = rules.tint([240, 80, 16, 128], color);
        assert_eq!(
            &pixels[(2 * 16 + 2) * 4..][..4],
            &[tint[0] & 128, tint[1] & 128, tint[2] & 128, 128]
        );
        let before = pixels.clone();
        assert!(
            paint_resources(
                &mut pixels,
                [16, 16],
                [0, 0],
                &rules,
                &images,
                &[([4., 4.], 0, color)],
                15
            )
            .is_err()
        );
        assert!(
            paint_resources(
                &mut pixels,
                [16, 16],
                [0, 0],
                &rules,
                &images,
                &[([f64::NAN, 4.], 0, color)],
                16
            )
            .is_err()
        );
        assert_eq!(pixels, before);
    }

    #[test]
    fn fractional_centres_write_solid_snapped_blocks() {
        let mut pixels = vec![113; 16 * 16 * 4];
        paint(
            &mut pixels,
            [16, 16],
            [0, 0],
            [2, 4],
            &[marker([5.75, 6.25], 0)],
            64,
        )
        .unwrap();
        assert_eq!(
            marked(&pixels, 16, [7, 19, 41, 255]),
            [[4, 5], [5, 5], [4, 6], [5, 6]]
        );
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| { *p == [113; 4] || *p == [7, 19, 41, 255] })
        );
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
