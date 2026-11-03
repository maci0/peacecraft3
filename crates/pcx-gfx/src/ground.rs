//! Admitted-map ground sheets.
//!
//! Slot variation and corner masks come from [`pcx_tera::tile_texture_selectors`].
//! Each sheet is split into variation cells, one texture-array layer per cell,
//! with the pinned column order: the right half of an eight-column sheet starts
//! at layer 16. The alpha already in those cells is the blend. Numeric blend
//! weights stay refused in `pcx-tera`.
//!
//! The layer numbering and the clamp/linear/mip filter follow the public
//! Warsmash `GroundTexture` loader (commit `f9e0aeed4be372d6016519d0e97b384aa873f374`).
//! That file is a labelled reconstruction, not a native trace. Explicit directional
//! lighting uses source-derived corner normals; cliffs, water and gameplay fog
//! are outside this draw. The optional authored shadow mask
//! and executed boundary overlay are composed in terrain world coordinates.

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::mesh::{
    Indices, Mesh, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology,
};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, SpecializedMeshPipelineError, TextureDataOrder,
    TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension, VertexFormat,
    WgpuLimits,
};
use bevy::shader::ShaderRef;
use pcx_map::Terrain;
use pcx_tera::{GroundSelectError, corner_world, tile_texture_selectors};

/// Capacity of the four-bit source ground-slot index.
pub const MAX_GROUND_SLOTS: usize = 16;
const SHEET_ROWS: u32 = 4;
const NARROW_COLUMNS: u32 = 4;
const WIDE_COLUMNS: u32 = 8;
const SELECTOR_LOCATION: u32 = 7;

/// Caller memory limits for one admitted ground draw.
///
/// `retained_bytes` is the published image, every layer and mip included.
/// `transient_bytes` bounds decoded RGBA and cell/mip scratch space, separately
/// from the retained image. Authored mip cells are copied one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundMemory {
    pub retained_bytes: usize,
    pub transient_bytes: usize,
}

/// Flat per-tile selectors, four `u32`s, mesh corner order.
pub const GROUND_SELECTORS: MeshVertexAttribute = MeshVertexAttribute::new(
    "GroundSelectors",
    0x6F3A_4C11_91D0_0E17,
    VertexFormat::Uint32x4,
);

/// One catalog-resolved ground BLP. This function does not open files.
pub struct SelectedGroundBlp<'a> {
    pub fourcc: [u8; 4],
    pub source: &'a str,
    pub bytes: &'a [u8],
}

/// One mesh and one material. Both handles are absent when the terrain has
/// nothing to rasterize. A terrain that declares slots and receives no
/// textures is [`GroundError::MissingSlots`], not an empty draw.
#[derive(Debug, Clone)]
pub struct GroundDraw {
    pub mesh: Option<Handle<Mesh>>,
    pub material: Option<Handle<GroundMaterial>>,
}

impl GroundDraw {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mesh.is_none() && self.material.is_none()
    }
}

/// Decoded sheets, not yet inserted into [`Assets`].
#[derive(Debug)]
pub struct DecodedGround {
    pub image: Image,
    pub layer_base: [u32; 16],
    pub extended: Vec<bool>,
    pub tile: u32,
    pub retained_bytes: usize,
}

#[derive(Clone, Copy, Debug, ShaderType)]
struct GroundUniform {
    layer_base: [UVec4; 4],
    boundary_color: Vec4,
    boundary_bounds: Vec4,
    shadow_color: Vec4,
    shadow_bounds: Vec4,
    light_direction: Vec4,
    light_diffuse: Vec4,
    light_ambient: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GroundMaterial {
    #[texture(0, dimension = "2d_array")]
    #[sampler(1)]
    pub texture: Handle<Image>,
    #[uniform(2)]
    params: GroundUniform,
    #[texture(3)]
    #[sampler(4)]
    boundary_texture: Option<Handle<Image>>,
    #[texture(5)]
    #[sampler(6)]
    shadow_texture: Option<Handle<Image>>,
}

impl GroundMaterial {
    fn new(texture: Handle<Image>, layer_base: [u32; 16]) -> Self {
        Self {
            texture,
            params: GroundUniform {
                layer_base: pack_layer_base(layer_base),
                boundary_color: Vec4::ZERO,
                boundary_bounds: Vec4::ZERO,
                shadow_color: Vec4::ZERO,
                shadow_bounds: Vec4::ZERO,
                light_direction: Vec4::ZERO,
                light_diffuse: Vec4::ZERO,
                light_ambient: Vec4::ZERO,
            },
            boundary_texture: None,
            shadow_texture: None,
        }
    }

    /// Numeric directional lighting in Bevy world space; absent until explicitly bound.
    pub fn set_directional_light(
        &mut self,
        towards: Vec3,
        diffuse: Vec3,
        ambient: Vec3,
    ) -> Result<(), &'static str> {
        let towards = towards
            .try_normalize()
            .filter(|v| v.is_finite())
            .ok_or("invalid terrain light direction")?;
        if !diffuse.is_finite() || !ambient.is_finite() {
            return Err("nonfinite terrain light channels");
        }
        self.params.light_direction = towards.extend(1.);
        self.params.light_diffuse = diffuse.extend(0.);
        self.params.light_ambient = ambient.extend(0.);
        Ok(())
    }

    /// Remove an explicitly bound light; retained channel values are inactive.
    pub fn clear_directional_light(&mut self) {
        self.params.light_direction = Vec4::ZERO;
    }

    /// Baked map shadow cells, in map X/Y order; no camera-dependent projection.
    pub fn set_shadow(
        &mut self,
        texture: Handle<Image>,
        bounds: Vec4,
        rgba: [u8; 4],
    ) -> Result<(), &'static str> {
        if !bounds.is_finite() || bounds.z <= 0.0 || bounds.w <= 0.0 {
            return Err("invalid terrain shadow bounds");
        }
        self.shadow_texture = Some(texture);
        self.params.shadow_bounds = bounds;
        self.params.shadow_color =
            Vec4::from_array(rgba.map(|c| f32::from(c) / f32::from(u8::MAX)));
        Ok(())
    }

    /// Explicit source boundary input; the absent pass has no color contribution.
    pub fn set_boundary(
        &mut self,
        texture: Handle<Image>,
        bounds: Vec4,
        rgba: [u8; 4],
    ) -> Result<(), &'static str> {
        if !bounds.is_finite() || bounds.z <= 0.0 || bounds.w <= 0.0 {
            return Err("invalid terrain boundary bounds");
        }
        self.boundary_texture = Some(texture);
        self.params.boundary_bounds = bounds;
        self.params.boundary_color =
            Vec4::from_array(rgba.map(|c| f32::from(c) / f32::from(u8::MAX)));
        Ok(())
    }
    pub fn clear_boundary(&mut self) {
        self.params.boundary_color = Vec4::ZERO;
        self.boundary_texture = None;
    }
    #[must_use]
    pub fn layer_base(&self) -> [u32; 16] {
        unpack_layer_base(self.params.layer_base)
    }
}

impl Material for GroundMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://pcx_gfx/ground.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://pcx_gfx/ground.wesl".into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn enable_oit() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let buffer = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            GROUND_SELECTORS.at_shader_location(SELECTOR_LOCATION),
        ])?;
        if descriptor.vertex.buffers.is_empty() {
            descriptor.vertex.buffers.push(buffer);
        } else {
            descriptor.vertex.buffers[0] = buffer;
        }
        Ok(())
    }
}

pub struct GroundMaterialPlugin;

impl Plugin for GroundMaterialPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "ground.wesl");
        app.add_plugins(MaterialPlugin::<GroundMaterial>::default());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GroundError {
    #[error("ground slot list is empty")]
    EmptySlots,
    #[error("ground slot count {got} does not match the terrain's {expected} slots")]
    SlotCount { got: usize, expected: usize },
    #[error("ground slot {index} fourcc {got:?} does not match terrain fourcc {expected:?}")]
    SlotFourcc {
        index: usize,
        got: [u8; 4],
        expected: [u8; 4],
    },
    #[error("ground slot count {count} exceeds 16")]
    TooManySlots { count: usize },
    #[error("terrain declares {expected} ground slots and no textures were supplied")]
    MissingSlots { expected: usize },
    #[error("{path}: ground BLP {fourcc:?}: {detail}")]
    Parse {
        fourcc: [u8; 4],
        path: String,
        detail: String,
    },
    #[error(
        "{path}: ground sheet {fourcc:?} is {width}x{height}, not a 4-row grid of 4 or 8 columns"
    )]
    SheetGrid {
        fourcc: [u8; 4],
        path: String,
        width: u32,
        height: u32,
    },
    #[error("{path}: ground sheet {fourcc:?} retains {needed} mip bytes and {limit} remain")]
    RetainedBytes {
        fourcc: [u8; 4],
        path: String,
        needed: usize,
        limit: usize,
    },
    #[error(
        "{path}: ground sheet {fourcc:?} needs {needed} transient bytes and the limit is {limit}"
    )]
    TransientBytes {
        fourcc: [u8; 4],
        path: String,
        needed: usize,
        limit: usize,
    },
    #[error("{path}: ground sheet {fourcc:?} tile {actual} does not match tile {expected}")]
    MixedTileExtent {
        fourcc: [u8; 4],
        path: String,
        expected: u32,
        actual: u32,
    },
    #[error("{path}: ground BLP {fourcc:?}: {detail}")]
    Decode {
        fourcc: [u8; 4],
        path: String,
        detail: String,
    },
    #[error(transparent)]
    Select(#[from] GroundSelectError),
    #[error("ground corner {x},{y} is missing")]
    Corner { x: u32, y: u32 },
    #[error("ground corner {x},{y} has a non-finite world position")]
    Position { x: u32, y: u32 },
    #[error("ground corner ({x}, {y}) has an invalid finite-difference normal")]
    Normal { x: u32, y: u32 },
    #[error(
        "ground tile extent {tile} or {layers} array layers exceed device limits {dimension} / {layer_limit}"
    )]
    DeviceExtent {
        tile: u32,
        layers: u32,
        dimension: u32,
        layer_limit: u32,
    },
    #[error("ground image retained {actual} bytes and the header chain requires {expected}")]
    MipLength { actual: usize, expected: usize },
    #[error("ground allocation failed")]
    Allocation,
}

struct SheetHeader {
    extended: bool,
    tile: u32,
    width: u32,
    height: u32,
    /// Decoded sheet RGBA. Also the cell-copy size for a complete grid.
    sheet_bytes: usize,
    /// Mip chains of this sheet's layers.
    retained_bytes: usize,
    mip_levels: u32,
    /// Two base sheets bound the decoded authored chain plus one cell.
    transient_bytes: usize,
}

struct Inspected {
    sheets: Vec<SheetHeader>,
    layer_base: [u32; 16],
    extended: Vec<bool>,
    tile: u32,
    mip_levels: u32,
    retained_bytes: usize,
}

/// Header gate, cell split, and per-layer mip chain. No asset is published.
pub fn decode_ground_slots(
    slots: &[SelectedGroundBlp<'_>],
    memory: GroundMemory,
) -> Result<DecodedGround, GroundError> {
    if slots.is_empty() {
        return Err(GroundError::EmptySlots);
    }
    let inspected = inspect_slots(slots, memory, None)?;
    let image = decode_image(slots, &inspected)?;
    Ok(DecodedGround {
        retained_bytes: inspected.retained_bytes,
        image,
        layer_base: inspected.layer_base,
        extended: inspected.extended,
        tile: inspected.tile,
    })
}

/// Build the admitted ground draw. Assets are inserted only after every slot,
/// selector, and mesh check has succeeded. A render caller supplies its active
/// device limits; `None` is for CPU-only asset preparation.
pub fn spawn_admitted_ground(
    terrain: &Terrain,
    slots: &[SelectedGroundBlp<'_>],
    memory: GroundMemory,
    texture_limits: Option<&WgpuLimits>,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<GroundMaterial>,
) -> Result<GroundDraw, GroundError> {
    if terrain.width() == 0 || terrain.height() == 0 {
        if terrain.slots().is_empty() && !slots.is_empty() {
            return Err(GroundError::SlotCount {
                got: slots.len(),
                expected: 0,
            });
        }
        return Ok(GroundDraw {
            mesh: None,
            material: None,
        });
    }
    if terrain.slots().is_empty() {
        return Err(GroundError::EmptySlots);
    }
    if slots.is_empty() {
        return Err(GroundError::MissingSlots {
            expected: terrain.slots().len(),
        });
    }
    if slots.len() != terrain.slots().len() {
        return Err(GroundError::SlotCount {
            got: slots.len(),
            expected: terrain.slots().len(),
        });
    }
    for (index, (slot, fourcc)) in slots.iter().zip(terrain.slots().iter()).enumerate() {
        if slot.fourcc != *fourcc {
            return Err(GroundError::SlotFourcc {
                index,
                got: slot.fourcc,
                expected: *fourcc,
            });
        }
    }
    let inspected = inspect_slots(slots, memory, texture_limits)?;
    let selectors = tile_selectors(terrain, &inspected.extended)?;
    let mesh = ground_mesh(terrain, &selectors)?;
    let image = decode_image(slots, &inspected)?;
    let layer_base = inspected.layer_base;
    let texture = images.add(image);
    let material = materials.add(GroundMaterial::new(texture, layer_base));
    let mesh = meshes.add(mesh);
    Ok(GroundDraw {
        mesh: Some(mesh),
        material: Some(material),
    })
}

fn inspect_slots(
    slots: &[SelectedGroundBlp<'_>],
    memory: GroundMemory,
    texture_limits: Option<&WgpuLimits>,
) -> Result<Inspected, GroundError> {
    if slots.len() > MAX_GROUND_SLOTS {
        return Err(GroundError::TooManySlots { count: slots.len() });
    }
    let mut sheets = Vec::new();
    sheets
        .try_reserve_exact(slots.len())
        .map_err(|_| GroundError::Allocation)?;
    for slot in slots {
        sheets.push(sheet_header(slot)?);
    }
    let mip_levels = sheets
        .iter()
        .map(|sheet| sheet.mip_levels)
        .min()
        .ok_or(GroundError::EmptySlots)?;
    let mut retained_left = memory.retained_bytes;
    let mut expected_tile: Option<u32> = None;
    let mut layer_base = [0u32; MAX_GROUND_SLOTS];
    let mut cursor = 0u32;
    let mut retained_bytes = 0usize;
    for (index, (slot, header)) in slots.iter().zip(sheets.iter_mut()).enumerate() {
        let layers = if header.extended { 32 } else { 16 };
        header.retained_bytes = mip_chain_bytes(header.tile, header.tile, mip_levels)?
            .checked_mul(layers as usize)
            .ok_or(GroundError::Allocation)?;
        if let Some(expected) = expected_tile {
            if header.tile != expected {
                return Err(GroundError::MixedTileExtent {
                    fourcc: slot.fourcc,
                    path: slot.source.to_owned(),
                    expected,
                    actual: header.tile,
                });
            }
        } else {
            expected_tile = Some(header.tile);
        }
        if header.transient_bytes > memory.transient_bytes {
            return Err(GroundError::TransientBytes {
                fourcc: slot.fourcc,
                path: slot.source.to_owned(),
                needed: header.transient_bytes,
                limit: memory.transient_bytes,
            });
        }
        if header.retained_bytes > retained_left {
            return Err(GroundError::RetainedBytes {
                fourcc: slot.fourcc,
                path: slot.source.to_owned(),
                needed: header.retained_bytes,
                limit: retained_left,
            });
        }
        retained_left -= header.retained_bytes;
        retained_bytes = retained_bytes
            .checked_add(header.retained_bytes)
            .ok_or(GroundError::Allocation)?;
        layer_base[index] = cursor;
        cursor = cursor.checked_add(layers).ok_or(GroundError::Allocation)?;
        if let Some(limits) = texture_limits
            && (header.tile > limits.max_texture_dimension_2d
                || cursor > limits.max_texture_array_layers)
        {
            return Err(GroundError::DeviceExtent {
                tile: header.tile,
                layers: cursor,
                dimension: limits.max_texture_dimension_2d,
                layer_limit: limits.max_texture_array_layers,
            });
        }
    }
    let tile = expected_tile.ok_or(GroundError::EmptySlots)?;
    let extended = sheets.iter().map(|sheet| sheet.extended).collect();
    Ok(Inspected {
        sheets,
        layer_base,
        extended,
        tile,
        mip_levels,
        retained_bytes,
    })
}

fn sheet_header(slot: &SelectedGroundBlp<'_>) -> Result<SheetHeader, GroundError> {
    let parsed = pcx_blp::Header::parse(slot.bytes).map_err(|error| GroundError::Parse {
        fourcc: slot.fourcc,
        path: slot.source.to_owned(),
        detail: error.to_string(),
    })?;
    if parsed.magic != *b"BLP1" || parsed.content > 1 {
        return Err(GroundError::Parse {
            fourcc: slot.fourcc,
            path: slot.source.to_owned(),
            detail: format!("magic {:?} content {}", parsed.magic, parsed.content),
        });
    }
    let width = parsed.width;
    let height = parsed.height;
    let grid_ok = height.is_multiple_of(SHEET_ROWS) && height / SHEET_ROWS > 0;
    let tile = if grid_ok { height / SHEET_ROWS } else { 0 };
    let columns = if tile > 0 && width.is_multiple_of(tile) {
        width / tile
    } else {
        0
    };
    if !grid_ok || (columns != NARROW_COLUMNS && columns != WIDE_COLUMNS) {
        return Err(GroundError::SheetGrid {
            fourcc: slot.fourcc,
            path: slot.source.to_owned(),
            width,
            height,
        });
    }
    let sheet_bytes = rgba_bytes(width, height)?;
    let count = parsed.mip_count();
    if count == 0
        || parsed.mip_sizes[..count].contains(&0)
        || count > mip_level_count(width, height) as usize
    {
        return Err(GroundError::Parse {
            fourcc: slot.fourcc,
            path: slot.source.to_owned(),
            detail: "missing, noncontiguous or excessive BLP mip levels".into(),
        });
    }
    let mip_levels = (count as u32).min(mip_level_count(tile, tile));
    for level in 0..mip_levels {
        let (w, h) = parsed.mip_size(level as usize);
        let cell = tile >> level;
        if w != cell * columns || h != cell * SHEET_ROWS {
            return Err(GroundError::Parse {
                fourcc: slot.fourcc,
                path: slot.source.to_owned(),
                detail: format!("mip {level} cannot preserve the authored cell grid"),
            });
        }
    }
    let transient_bytes = sheet_bytes
        .checked_add(sheet_bytes)
        .ok_or(GroundError::Allocation)?;
    Ok(SheetHeader {
        extended: columns == WIDE_COLUMNS,
        tile,
        width,
        height,
        sheet_bytes,
        retained_bytes: 0,
        mip_levels,
        transient_bytes,
    })
}

fn decode_image(
    slots: &[SelectedGroundBlp<'_>],
    inspected: &Inspected,
) -> Result<Image, GroundError> {
    let layer_count: usize = inspected
        .sheets
        .iter()
        .map(|sheet| if sheet.extended { 32 } else { 16 })
        .sum();
    if layer_count < 2 {
        return Err(GroundError::Allocation);
    }
    let mut data = Vec::new();
    data.try_reserve_exact(inspected.retained_bytes)
        .map_err(|_| GroundError::Allocation)?;
    for (slot, sheet) in slots.iter().zip(inspected.sheets.iter()) {
        let loaded =
            crate::load_mipmapped_image_bounded(slot.bytes, slot.source, sheet.transient_bytes)
                .map_err(|error| GroundError::Decode {
                    fourcc: slot.fourcc,
                    path: slot.source.to_owned(),
                    detail: error.to_string(),
                })?;
        if loaded.width != sheet.width
            || loaded.height != sheet.height
            || loaded.rgba.len() != sheet.sheet_bytes
        {
            return Err(GroundError::Decode {
                fourcc: slot.fourcc,
                path: slot.source.to_owned(),
                detail: format!(
                    "decoded {}x{} ({} bytes), header {}x{} needs {} bytes",
                    loaded.width,
                    loaded.height,
                    loaded.rgba.len(),
                    sheet.width,
                    sheet.height,
                    sheet.sheet_bytes
                ),
            });
        }
        for half in 0..if sheet.extended { 2 } else { 1 } {
            for y in 0..SHEET_ROWS {
                for x in 0..NARROW_COLUMNS {
                    for level in 0..inspected.mip_levels {
                        let rgba = if level == 0 {
                            &loaded.rgba
                        } else {
                            &loaded.mipmaps[level as usize - 1]
                        };
                        let cell = copy_cell(
                            rgba,
                            sheet.width >> level,
                            sheet.tile >> level,
                            x + half * NARROW_COLUMNS,
                            y,
                        )?;
                        data.extend_from_slice(&cell);
                    }
                }
            }
        }
    }
    if data.len() != inspected.retained_bytes {
        return Err(GroundError::MipLength {
            actual: data.len(),
            expected: inspected.retained_bytes,
        });
    }
    let mut image = Image::new_uninit(
        Extent3d {
            width: inspected.tile,
            height: inspected.tile,
            depth_or_array_layers: u32::try_from(layer_count)
                .map_err(|_| GroundError::Allocation)?,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.data = Some(data);
    image.data_order = TextureDataOrder::LayerMajor;
    image.texture_descriptor.mip_level_count = inspected.mip_levels;
    image.sampler = ImageSampler::linear();
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..Default::default()
    });
    Ok(image)
}

fn copy_cell(
    rgba: &[u8],
    sheet_width: u32,
    tile: u32,
    cell_x: u32,
    cell_y: u32,
) -> Result<Vec<u8>, GroundError> {
    let tile_len = usize::try_from(tile).map_err(|_| GroundError::Allocation)?;
    let sheet_width = usize::try_from(sheet_width).map_err(|_| GroundError::Allocation)?;
    let cell_x = usize::try_from(cell_x).map_err(|_| GroundError::Allocation)?;
    let cell_y = usize::try_from(cell_y).map_err(|_| GroundError::Allocation)?;
    let row_bytes = tile_len.checked_mul(4).ok_or(GroundError::Allocation)?;
    let mut cell = Vec::new();
    cell.try_reserve_exact(
        row_bytes
            .checked_mul(tile_len)
            .ok_or(GroundError::Allocation)?,
    )
    .map_err(|_| GroundError::Allocation)?;
    let x_offset = cell_x
        .checked_mul(tile_len)
        .ok_or(GroundError::Allocation)?;
    for row in 0..tile_len {
        let start = cell_y
            .checked_mul(tile_len)
            .and_then(|rows| rows.checked_add(row))
            .and_then(|rows| rows.checked_mul(sheet_width))
            .and_then(|pixels| pixels.checked_add(x_offset))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(GroundError::Allocation)?;
        let end = start
            .checked_add(row_bytes)
            .ok_or(GroundError::Allocation)?;
        let row_texels = rgba.get(start..end).ok_or(GroundError::Allocation)?;
        cell.extend_from_slice(row_texels);
    }
    Ok(cell)
}

fn rgba_bytes(width: u32, height: u32) -> Result<usize, GroundError> {
    usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok()?.checked_mul(w))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(GroundError::Allocation)
}

fn next_extent(extent: u32) -> u32 {
    if extent <= 1 { 1 } else { (extent / 2).max(1) }
}

fn mip_level_count(mut width: u32, mut height: u32) -> u32 {
    let mut count = 1u32;
    while width > 1 || height > 1 {
        width = next_extent(width);
        height = next_extent(height);
        count += 1;
    }
    count
}

/// Bytes of the common supplied mip prefix, before any pixel decode.
fn mip_chain_bytes(mut width: u32, mut height: u32, levels: u32) -> Result<usize, GroundError> {
    let mut total = 0usize;
    for _ in 0..levels {
        total = total
            .checked_add(rgba_bytes(width, height)?)
            .ok_or(GroundError::Allocation)?;
        width = next_extent(width);
        height = next_extent(height);
    }
    Ok(total)
}

fn tile_selectors(terrain: &Terrain, extended: &[bool]) -> Result<Vec<[u16; 4]>, GroundError> {
    let tiles = usize::try_from(terrain.width())
        .ok()
        .and_then(|width| usize::try_from(terrain.height()).ok()?.checked_mul(width))
        .ok_or(GroundError::Allocation)?;
    let mut selectors = Vec::new();
    selectors
        .try_reserve_exact(tiles)
        .map_err(|_| GroundError::Allocation)?;
    for ty in 0..terrain.height() {
        for tx in 0..terrain.width() {
            selectors.push(tile_texture_selectors(terrain, extended, tx, ty)?);
        }
    }
    Ok(selectors)
}

fn ground_mesh(terrain: &Terrain, selectors: &[[u16; 4]]) -> Result<Mesh, GroundError> {
    let tiles = selectors.len();
    let vertices = tiles.checked_mul(4).ok_or(GroundError::Allocation)?;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut packed = Vec::new();
    let mut indices = Vec::new();
    positions
        .try_reserve_exact(vertices)
        .map_err(|_| GroundError::Allocation)?;
    normals
        .try_reserve_exact(vertices)
        .map_err(|_| GroundError::Allocation)?;
    uvs.try_reserve_exact(vertices)
        .map_err(|_| GroundError::Allocation)?;
    packed
        .try_reserve_exact(vertices)
        .map_err(|_| GroundError::Allocation)?;
    indices
        .try_reserve_exact(tiles.checked_mul(6).ok_or(GroundError::Allocation)?)
        .map_err(|_| GroundError::Allocation)?;
    let local_uv = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
    for (tile, selector) in selectors.iter().enumerate() {
        let tx = (tile % terrain.width() as usize) as u32;
        let ty = (tile / terrain.width() as usize) as u32;
        let corners = [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)];
        let words = [
            u32::from(selector[0]),
            u32::from(selector[1]),
            u32::from(selector[2]),
            u32::from(selector[3]),
        ];
        let base = u32::try_from(positions.len()).map_err(|_| GroundError::Allocation)?;
        for (corner_index, (cx, cy)) in corners.iter().copied().enumerate() {
            let world =
                corner_world(terrain, cx, cy).ok_or(GroundError::Corner { x: cx, y: cy })?;
            if !world.into_iter().all(f32::is_finite) {
                return Err(GroundError::Position { x: cx, y: cy });
            }
            positions.push(bevy_position(world));
            normals.push(bevy_position(
                pcx_tera::corner_normal(terrain, cx, cy)
                    .ok_or(GroundError::Normal { x: cx, y: cy })?,
            ));
            uvs.push(local_uv[corner_index]);
            packed.push(words);
        }
        indices.extend_from_slice(&[base, base + 1, base + 3, base, base + 3, base + 2]);
    }
    Ok(Mesh::new(
        PrimitiveTopology::TriangleList,
        // Terrain changes rebuild this mesh; no CPU vertex edits follow upload.
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(GROUND_SELECTORS, packed)
    .with_inserted_indices(Indices::U32(indices)))
}

/// Terrain `(x east, y north, z up)` becomes Bevy `(x, z, -y)`, so the tile
/// winding `(0, 1, 3), (0, 3, 2)` keeps its normal at Bevy `+y`.
fn bevy_position(world: [f32; 3]) -> [f32; 3] {
    [world[0], world[2], -world[1]]
}

fn pack_layer_base(layer_base: [u32; 16]) -> [UVec4; 4] {
    let mut packed = [UVec4::ZERO; 4];
    for (index, value) in layer_base.into_iter().enumerate() {
        packed[index / 4][index % 4] = value;
    }
    packed
}

fn unpack_layer_base(packed: [UVec4; 4]) -> [u32; 16] {
    let mut layer_base = [0u32; 16];
    for index in 0..16 {
        layer_base[index] = packed[index / 4][index % 4];
    }
    layer_base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directional_light_validates_before_replacing_the_previous_binding() {
        let mut material = GroundMaterial::new(Handle::default(), [0; 16]);
        material
            .set_directional_light(Vec3::Y * 2., Vec3::splat(0.6), Vec3::splat(0.2))
            .unwrap();
        assert_eq!(material.params.light_direction, Vec3::Y.extend(1.));
        assert!(
            material
                .set_directional_light(Vec3::ZERO, Vec3::ONE, Vec3::ZERO)
                .is_err()
        );
        assert!(
            material
                .set_directional_light(Vec3::Z, Vec3::splat(f32::NAN), Vec3::ZERO)
                .is_err()
        );
        assert_eq!(material.params.light_direction, Vec3::Y.extend(1.));
        assert_eq!(material.params.light_diffuse, Vec4::new(0.6, 0.6, 0.6, 0.));
        material.clear_directional_light();
        assert_eq!(material.params.light_direction, Vec4::ZERO);
    }

    fn blp(width: u32, height: u32, indices: &[u8]) -> Vec<u8> {
        assert_eq!(indices.len(), (width * height) as usize);
        let mut palette = [[0u8; 4]; 256];
        palette[0] = [0, 255, 0, 255];
        palette[1] = [0, 0, 255, 255];
        palette[2] = [255, 0, 0, 255];
        let payload_at = 0x9Cu32 + 256 * 4;
        let mut bytes = vec![0u8; payload_at as usize + indices.len()];
        bytes[0..4].copy_from_slice(b"BLP1");
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        bytes[0x0C..0x10].copy_from_slice(&width.to_le_bytes());
        bytes[0x10..0x14].copy_from_slice(&height.to_le_bytes());
        bytes[0x1C..0x20].copy_from_slice(&payload_at.to_le_bytes());
        bytes[0x5C..0x60].copy_from_slice(&(indices.len() as u32).to_le_bytes());
        for (index, entry) in palette.iter().enumerate() {
            let at = 0x9C + index * 4;
            bytes[at..at + 4].copy_from_slice(entry);
        }
        bytes[payload_at as usize..].copy_from_slice(indices);
        bytes
    }

    #[test]
    fn eight_column_sheet_puts_the_right_half_at_layer_16() {
        let mut indices = vec![0u8; 8 * 4];
        indices[4] = 1;
        let bytes = blp(8, 4, &indices);
        let decoded = decode_ground_slots(
            &[SelectedGroundBlp {
                fourcc: *b"Ldrt",
                source: "synthetic/wide.blp",
                bytes: &bytes,
            }],
            GroundMemory {
                retained_bytes: 128,
                transient_bytes: 256,
            },
        )
        .unwrap();
        assert!(decoded.extended[0]);
        assert_eq!(decoded.tile, 1);
        assert_eq!(decoded.layer_base[0], 0);
        let data = decoded.image.data.unwrap();
        assert_eq!(&data[0..4], &[0, 255, 0, 255]);
        assert_eq!(&data[16 * 4..16 * 4 + 4], &[255, 0, 0, 255]);
        assert_eq!(&data[4 * 4..4 * 4 + 4], &[0, 255, 0, 255]);
    }

    #[test]
    fn supplied_mip_pixels_are_preserved() {
        let mut indices = vec![0u8; 32 * 32];
        for y in 3..5 {
            for x in 3..5 {
                indices[y * 32 + x] = 1;
            }
        }
        let mut bytes = blp(32, 32, &indices);
        let offset = bytes.len() as u32;
        bytes[0x20..0x24].copy_from_slice(&offset.to_le_bytes());
        bytes[0x60..0x64].copy_from_slice(&256u32.to_le_bytes());
        bytes.extend_from_slice(&[2; 256]);
        let decoded = decode_ground_slots(
            &[SelectedGroundBlp {
                fourcc: *b"Ldrt",
                source: "synthetic/cell.blp",
                bytes: &bytes,
            }],
            GroundMemory {
                retained_bytes: 5440,
                transient_bytes: 8192,
            },
        )
        .unwrap();
        assert_eq!(decoded.image.texture_descriptor.mip_level_count, 2);
        assert_eq!(
            decoded.image.texture_descriptor.format,
            TextureFormat::Rgba8Unorm
        );
        let data = decoded.image.data.unwrap();
        assert_eq!(&data[256..260], &[0, 0, 255, 255]);
        assert_eq!(data.len(), 16 * (8 * 8 + 4 * 4) * 4);
    }

    #[test]
    fn base_only_odd_cells_do_not_invent_mips() {
        let mut indices = vec![0u8; 12 * 12];
        indices[2 * 12 + 2] = 1;
        let bytes = blp(12, 12, &indices);
        let decoded = decode_ground_slots(
            &[SelectedGroundBlp {
                fourcc: *b"Ldrt",
                source: "synthetic/odd.blp",
                bytes: &bytes,
            }],
            GroundMemory {
                retained_bytes: 640,
                transient_bytes: 1152,
            },
        )
        .unwrap();
        assert_eq!(decoded.tile, 3);
        assert_eq!(decoded.image.texture_descriptor.mip_level_count, 1);
        let data = decoded.image.data.unwrap();
        assert_eq!(&data[32..36], &[255, 0, 0, 255]);
        assert_eq!(&data[36..40], &[0, 255, 0, 255]);
    }
}
