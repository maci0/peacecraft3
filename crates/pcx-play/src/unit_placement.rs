//! Bounded creation placement. The host owns positions; this grid is a collision cache.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::script_random::real;
use crate::scripts::AssetIdentity;
use crate::spatial_pool::{PoolOwner, Stamp, Texture, TexturePixels};
use pcx_assets::AssetPath;
use pcx_map::Pathing;
use pcx_script::HandleId;
use pcx_sim::world::Source;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

type BlightQuery<'a> = (&'a crate::terrain_blight::InitialBlight, f64);

#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub normalization: SourceWindow,
    pub initial_position: SourceWindow,
    pub small_radius: SourceWindow,
    pub large_radius: SourceWindow,
    pub one_initializer: SourceWindow,
    pub two_initializer: SourceWindow,
    pub ground_mask: SourceWindow,
    pub fly_mask: SourceWindow,
    pub float_mask: SourceWindow,
    pub amphibious_mask: SourceWindow,
    pub terrain_xy: SourceWindow,
    pub terrain_height: SourceWindow,
    pub texture_channels: SourceWindow,
    pub texture_build_mask: SourceWindow,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TexturePolicy {
    red_test: u8,
    red_flags: u8,
    green_flags: u8,
    blue_test: u8,
    blue_flags: u8,
    build_mask: u8,
}

impl TexturePolicy {
    pub(crate) fn flags(self, [red, green, blue, _alpha]: [u8; 4]) -> u8 {
        (if red & self.red_test != 0 {
            self.red_flags
        } else {
            0
        }) | (if green != 0 { self.green_flags } else { 0 })
            | (if blue & self.blue_test != 0 {
                self.blue_flags
            } else {
                0
            })
    }

    pub(crate) fn checks_construction(self, pixel: [u8; 4]) -> bool {
        self.flags(pixel) & self.build_mask != 0
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unit placement: {0}")]
pub struct Error(pub String, bool);

impl Error {
    pub(crate) fn work_exhausted(&self) -> bool {
        self.1
    }
}

fn work_exhausted(message: &str) -> Error {
    Error(message.into(), true)
}

fn fail(message: impl Into<String>) -> Error {
    Error(message.into(), false)
}

/// A bounded route's termination, independent of its best accepted path.
#[derive(Debug, PartialEq)]
pub(crate) enum RouteOutcome {
    Reached(Vec<[f32; 2]>),
    PartialBudget(Vec<[f32; 2]>),
    Blocked(Vec<[f32; 2]>),
}

#[derive(Clone, Debug)]
pub struct Rules {
    binding: Arc<Binding>,
    exponent: i32,
    initial: u32,
    thresholds: [f32; 3],
    one: f32,
    two: f32,
    mask: u8,
    fly: u8,
    float: u8,
    amphibious: u8,
    terrain_xy: f32,
    height_bias: f32,
    height_factor: f32,
    texture_policy: TexturePolicy,
}

impl Rules {
    /// The caller selects reviewed instruction windows, never measured operands.
    pub fn read(image: &[u8], binding: Binding, budget: RuleInputBudget) -> Result<Self, Error> {
        let source = binding.origin.source.clone();
        Self::read_selected(image, binding, budget)
            .map_err(|error| fail(format!("{source}: {}", error.0)))
    }

    fn read_selected(
        image: &[u8],
        binding: Binding,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        let windows = [
            binding.normalization,
            binding.initial_position,
            binding.small_radius,
            binding.large_radius,
            binding.one_initializer,
            binding.two_initializer,
            binding.ground_mask,
            binding.fly_mask,
            binding.float_mask,
            binding.amphibious_mask,
            binding.terrain_xy,
            binding.terrain_height,
            binding.texture_channels,
            binding.texture_build_mask,
        ];
        if image.len() > budget.image_bytes
            || windows.iter().map(|w| u64::from(w.len)).sum::<u64>() + 24
                > budget.profile_bytes as u64
        {
            return Err(fail("input budget exceeded"));
        }
        if [
            &binding.origin.edition,
            &binding.origin.version,
            &binding.origin.source,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || sha256(image) != binding.image_sha256
        {
            return Err(fail("incomplete identity or image fingerprint mismatch"));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(e.to_string()))?;
        let code = |window: &SourceWindow| pe.window(window, true).map_err(|e| fail(e.to_string()));
        let scale = code(&binding.normalization)?;
        if scale.len() != 5 || scale[0] != 0x05 {
            return Err(fail("normalization instruction"));
        }
        let adjustment = i32::from_le_bytes(scale[1..].try_into().unwrap());
        if adjustment >= 0 || adjustment % (1 << 23) != 0 {
            return Err(fail("normalization exponent"));
        }
        let exponent = -adjustment / (1 << 23);
        if !(1..=23).contains(&exponent) {
            return Err(fail("unsupported cell exponent"));
        }
        let data = |address: u32| -> Result<u32, Error> {
            let bytes = pe
                .read_from(address, false)
                .map_err(|e| fail(e.to_string()))?;
            Ok(u32::from_le_bytes(
                bytes
                    .get(..4)
                    .ok_or_else(|| fail("truncated real"))?
                    .try_into()
                    .unwrap(),
            ))
        };
        let initial = code(&binding.initial_position)?;
        if initial.len() != 5 || initial[0] != 0xa1 {
            return Err(fail("constructor load"));
        }
        let initial = data(u32::from_le_bytes(initial[1..].try_into().unwrap()))?;
        real::from_f32(f32::from_bits(initial)).map_err(|e| fail(e.to_string()))?;
        let threshold = |window: &SourceWindow| -> Result<f32, Error> {
            let bytes = code(window)?;
            if bytes.len() != 8 || bytes[..4] != [0xf3, 0x0f, 0x10, 0x05] {
                return Err(fail("radius threshold load"));
            }
            Ok(f32::from_bits(data(u32::from_le_bytes(
                bytes[4..].try_into().unwrap(),
            ))?))
        };
        let integer = |window: &SourceWindow| -> Result<f32, Error> {
            let bytes = code(window)?;
            let (value, at) = match bytes {
                [0x6a, value, ..] => (i32::from(*value as i8), 2),
                [0x68, a, b, c, d, ..] => (i32::from_le_bytes([*a, *b, *c, *d]), 5),
                _ => return Err(fail("integer initializer")),
            };
            if bytes.len() != at + 14
                || bytes[at] != 0x68
                || bytes[at + 5] != 0xe8
                || bytes[at + 10..] != [0x83, 0xc4, 8, 0xc3]
            {
                return Err(fail("real initializer shape"));
            }
            Ok(f32::from_bits(
                real::from_i32(value).map_err(|e| fail(e.to_string()))?,
            ))
        };
        let one = integer(&binding.one_initializer)?;
        let two = integer(&binding.two_initializer)?;
        let thresholds = [
            threshold(&binding.small_radius)?,
            one,
            threshold(&binding.large_radius)?,
        ];
        if !two.is_finite()
            || two <= one
            || thresholds.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || !thresholds.windows(2).all(|p| p[0] < p[1])
        {
            return Err(fail("unordered radius geometry"));
        }
        let mask = |window: &SourceWindow, label: &str| -> Result<u8, Error> {
            let bytes = code(window)?;
            if bytes.len() != 5 || bytes[0] != 0xb9 {
                return Err(fail(format!("{label} instruction")));
            }
            let value = u8::try_from(u32::from_le_bytes(bytes[1..].try_into().unwrap()))
                .map_err(|_| fail(format!("{label} range")))?;
            if value == 0 {
                return Err(fail(format!("empty {label}")));
            }
            Ok(value)
        };
        let ground = mask(&binding.ground_mask, "ground mask")?;
        let fly = mask(&binding.fly_mask, "fly mask")?;
        let float = mask(&binding.float_mask, "float mask")?;
        let amphibious = mask(&binding.amphibious_mask, "amphibious mask")?;
        let operand = |bytes: &[u8], prefix: &[u8]| -> Result<u8, Error> {
            if bytes.len() != prefix.len() + 1 || !bytes.starts_with(prefix) {
                return Err(fail("texture channel instruction"));
            }
            Ok(bytes[prefix.len()])
        };
        let channels = code(&binding.texture_channels)?;
        if channels.len() != 48 || channels.get(16..20) != Some(&[0x80, 0x79, 0x01, 0x00]) {
            return Err(fail("texture channel instructions"));
        }
        let texture_policy = TexturePolicy {
            red_test: operand(&channels[0..4], &[0xf6, 0x41, 0x02])?,
            red_flags: operand(&channels[9..13], &[0x80, 0x0c, 0x03])?,
            green_flags: operand(&channels[25..29], &[0x80, 0x0c, 0x03])?,
            blue_test: operand(&channels[36..39], &[0xf6, 0x01])?,
            blue_flags: operand(&channels[44..48], &[0x80, 0x0c, 0x01])?,
            build_mask: operand(code(&binding.texture_build_mask)?, &[0xf6, 0x45, 0x18])?,
        };
        if texture_policy.red_test == 0
            || texture_policy.blue_test == 0
            || texture_policy.build_mask == 0
        {
            return Err(fail("empty texture channel mask"));
        }
        let masks = [ground, fly, float, amphibious];
        if masks.iter().any(|m| !m.is_power_of_two())
            || masks
                .iter()
                .enumerate()
                .any(|(i, m)| masks[..i].contains(m))
        {
            return Err(fail("movement masks must be distinct single bits"));
        }
        let xy = code(&binding.terrain_xy)?;
        let height = code(&binding.terrain_height)?;
        if xy.len() != 8
            || xy[..4] != [0xf3, 0x0f, 0x59, 0x05]
            || height.len() != 19
            || height[..4] != [0xf3, 0x0f, 0x58, 0x05]
            || height[8..15] != [0xc1, 0xe6, 0x0e, 0xf3, 0x0f, 0x59, 0x05]
        {
            return Err(fail("terrain coordinate encoding"));
        }
        let terrain_xy = f32::from_bits(data(u32::from_le_bytes(xy[4..8].try_into().unwrap()))?);
        let height_bias =
            f32::from_bits(data(u32::from_le_bytes(height[4..8].try_into().unwrap()))?);
        let height_factor = f32::from_bits(data(u32::from_le_bytes(
            height[15..19].try_into().unwrap(),
        ))?);
        if [terrain_xy, height_bias, height_factor]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
        {
            return Err(fail("finite positive terrain coordinate encoding required"));
        }
        Ok(Self {
            binding: Arc::new(binding),
            exponent,
            initial,
            thresholds,
            one,
            two,
            mask: ground,
            fly,
            float,
            amphibious,
            terrain_xy,
            height_bias,
            height_factor,
            texture_policy,
        })
    }

    /// (tile edge, world-height bias, raw indices per world-height unit).
    pub fn terrain_encoding(&self) -> [f32; 3] {
        [self.terrain_xy, self.height_bias, self.height_factor]
    }

    pub(crate) fn texture_policy(&self) -> TexturePolicy {
        self.texture_policy
    }

    pub fn cell_size(&self) -> f32 {
        (1u32 << self.exponent) as f32
    }

    pub fn ground_mask(&self) -> u8 {
        self.mask
    }

    pub fn fly_mask(&self) -> u8 {
        self.fly
    }

    pub fn float_mask(&self) -> u8 {
        self.float
    }

    pub fn amphibious_mask(&self) -> u8 {
        self.amphibious
    }

    pub fn solid_mask(&self) -> u8 {
        self.mask | self.float | self.amphibious
    }

    pub fn binding(&self) -> &Binding {
        &self.binding
    }

    fn shift(&self, value: u32, exponent: i32) -> Result<u32, Error> {
        if value & 0x7fff_ffff == 0 {
            return Ok(value);
        }
        let encoded = ((value >> 23) & 255) as i32 + exponent;
        if !(1..255).contains(&encoded) {
            return Err(fail("coordinate exponent outside normal range"));
        }
        let result = i64::from(value) + (i64::from(exponent) << 23);
        let result = u32::try_from(result).map_err(|_| fail("coordinate exponent overflow"))?;
        real::from_f32(f32::from_bits(result)).map_err(|e| fail(e.to_string()))?;
        Ok(result)
    }

    fn normalize(&self, point: [f32; 2], origin: [f32; 2]) -> Result<[f32; 2], Error> {
        let mut result = [0.0; 2];
        for axis in 0..2 {
            let word = real::sub(
                real::from_f32(point[axis]).map_err(|e| fail(e.to_string()))?,
                real::from_f32(origin[axis]).map_err(|e| fail(e.to_string()))?,
            )
            .map_err(|e| fail(e.to_string()))?;
            let cell = self.shift(word, -self.exponent)?;
            let cell = real::add(
                self.initial,
                real::sub(cell, self.initial).map_err(|e| fail(e.to_string()))?,
            )
            .map_err(|e| fail(e.to_string()))?;
            result[axis] = f32::from_bits(
                real::add(self.shift(cell, self.exponent)?, origin[axis].to_bits())
                    .map_err(|e| fail(e.to_string()))?,
            );
        }
        Ok(result)
    }

    fn extents(&self, radius: f32) -> [i32; 2] {
        let band = self.thresholds.iter().take_while(|v| radius >= **v).count();
        match band {
            0 => [0, 0],
            1 => [self.one as i32, 0],
            2 => [self.one as i32, self.one as i32],
            _ => [self.two as i32, self.one as i32],
        }
    }
}

/// An explicitly selected, derived static collision raster, including stationary
/// footprints. It is a cache, not another object/handle arena. Authored placement
/// builds it once; script-created destructables add bounded layers at creation.
pub struct Grid {
    pub source: Source,
    pub origin: [f32; 2],
    pub pathing: Pathing,
    /// Terrain-domain labels in pathing-cell order, derived from the selected W3E.
    pub domains: Vec<u8>,
}

/// Already decoded pathing texture. Coordinates are cells relative to its centre.
/// Keeping the decode outside this module reuses the selected texture loader.
#[derive(Clone)]
pub struct Footprint {
    pub origin: AssetIdentity,
    pub cells: Vec<[i32; 2]>,
}

/// Source-derived placement flags in the original texture's pixel order.
pub struct NativeTexture {
    pub origin: AssetIdentity,
    pub width: u16,
    pub height: u16,
    pub flags: Vec<u8>,
}

#[derive(Clone, Copy)]
pub struct Budget {
    pub cells: usize,
    pub footprint_cells: usize,
    /// Total live+dead layer cells retained for stationary destructables.
    pub stationary_cells: usize,
    /// Footprint cells decoded while preparing stationary layers.
    pub stationary_work: usize,
    /// Deterministic cell/occupancy queries per creation, independent of wall time.
    pub queries: usize,
}

/// Identity of one stationary destructable whose collision can change.
///
/// Placed objects are keyed by their authored creation number; script objects by
/// their arena handle. Both share the same overlay path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StationaryKey {
    Placed(u32),
    Destructable(HandleId),
}

/// One destructable's live and dead footprint cells over a shared base raster.
#[derive(Clone, Debug)]
pub struct StationaryStamp {
    pub key: StationaryKey,
    pub live: Vec<usize>,
    pub dead: Vec<usize>,
    pub mask: u8,
    pub active: bool,
}

#[derive(Debug)]
struct StationaryLayer {
    live: Vec<usize>,
    dead: Vec<usize>,
    mask: u8,
    active: bool,
}

/// Preflighted live/dead layer for one script-created destructable. Publishing it
/// is infallible once the shared arena has issued the matching handle.
#[derive(Debug)]
pub(crate) struct PreparedStationary {
    live: Vec<usize>,
    dead: Vec<usize>,
    cells: Vec<usize>,
    mask: u8,
    active: bool,
    work: usize,
    native: Option<Box<StationaryTextures>>,
}

#[derive(Debug)]
struct StationaryTextures {
    live: TexturePixels,
    dead: TexturePixels,
}

pub struct Placement {
    source: Source,
    grid: Grid,
    rules: Rules,
    footprints: BTreeMap<String, Footprint>,
    building_footprints: BTreeMap<String, Footprint>,
    footprint_loader: Option<FootprintLoaderBox>,
    script_paths: Option<ScriptPathsLoader>,
    occupied: Vec<u32>,
    water_occupied: Vec<u32>,
    footprint_occupied: Vec<u32>,
    stamps: Vec<(HandleId, Prepared)>,
    stationary: BTreeMap<StationaryKey, StationaryLayer>,
    stationary_cells: BTreeMap<usize, Vec<(StationaryKey, bool)>>,
    stationary_cells_used: usize,
    stationary_work: usize,
    base_pathing: Option<Pathing>,
    budget: Budget,
    /// Present only in tracked sessions; see [`crate::spatial_pool`].
    spatial: Option<Box<PoolOwner>>,
}

/// Source reader for lazily decoded footprints. Paths are canonical
/// [`AssetPath`] identities. Script loads stay staged until
/// [`Placement::prepare_script_destructable`] accepts the layer; refusing the
/// layer calls [`FootprintLoader::rollback`] for every successful load.
/// Ordinary unit footprints commit as soon as their source load succeeds.
pub trait FootprintLoader: Send + Sync {
    fn load(&mut self, path: &str) -> Result<Footprint, String>;
    fn load_building(&mut self, _path: &str) -> Result<Option<Footprint>, String> {
        Ok(None)
    }
    fn native_texture(&self, _path: &str) -> Option<&NativeTexture> {
        None
    }
    fn commit(&mut self, _paths: &[String]) {}
    fn rollback(&mut self, _paths: &[String]) {}
}

struct ClosureLoader<F>(F);

impl<F> FootprintLoader for ClosureLoader<F>
where
    F: FnMut(&str) -> Result<Footprint, String> + Send + Sync,
{
    fn load(&mut self, path: &str) -> Result<Footprint, String> {
        (self.0)(path)
    }
}

type FootprintLoaderBox = Box<dyn FootprintLoader>;
type ScriptPathsLoader = Box<dyn Fn(i32, bool) -> Result<String, String> + Send + Sync>;

/// Movement retained by collision and route preparation, independent of model presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Movement {
    Ground,
    NoMovement,
    Air,
    Float,
    Amphibious,
}

impl Movement {
    pub fn from_token(token: &str) -> Result<Self, &'static str> {
        match token {
            "" | "_" | "none" => Ok(Self::NoMovement),
            "foot" | "horse" | "hover" => Ok(Self::Ground),
            "fly" => Ok(Self::Air),
            "float" => Ok(Self::Float),
            "amph" => Ok(Self::Amphibious),
            _ => Err("movement type is outside admitted placement"),
        }
    }

    fn ground(self) -> bool {
        matches!(self, Self::Ground | Self::NoMovement | Self::Amphibious)
    }

    fn water(self) -> bool {
        matches!(self, Self::Float | Self::Amphibious)
    }
}

#[derive(Clone, Copy)]
struct Candidate {
    creation: bool,
    movement: Movement,
}

#[derive(Debug)]
pub(crate) struct Prepared {
    pub position: [f32; 2],
    cells: Vec<usize>,
    footprint: PreparedFootprint,
    /// Facing the footprint was oriented with; a later turn of the unit's model does not
    /// rotate a standing footprint, so a reveal or relocation reuses it.
    face: f32,
    active: bool,
    movement: Movement,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PreparedFootprint {
    Plain(Vec<usize>),
    Native(Box<NativeFootprint>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeFootprint {
    cells: Vec<usize>,
    pixels: TexturePixels,
}

impl PreparedFootprint {
    fn native(&self) -> Option<&[(u32, u8)]> {
        match self {
            Self::Plain(_) => None,
            Self::Native(data) => Some(&data.pixels),
        }
    }
}

impl std::ops::Deref for PreparedFootprint {
    type Target = [usize];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Plain(cells) => cells,
            Self::Native(data) => &data.cells,
        }
    }
}

impl Prepared {
    pub(crate) fn deactivate(&mut self) {
        self.active = false;
    }
}

#[cfg(test)]
#[path = "unit_placement_tests.rs"]
pub(crate) mod tests;

impl Placement {
    pub fn diagnostic_sample(&self, point: [f32; 2]) -> Result<(i32, bool), Error> {
        let bounds = self.world_bounds();
        let point = std::array::from_fn(|axis| {
            point[axis].clamp(bounds[0][axis], bounds[1][axis] - self.rules.cell_size())
        });
        let [x, y] = self.cell(point)?;
        let i = self
            .index(i64::from(x), i64::from(y))
            .ok_or_else(|| fail("diagnostic sample outside map"))?;
        Ok((
            i32::from(self.grid.domains[i]),
            self.grid.pathing.cells[i] & self.rules.ground_mask() != 0,
        ))
    }

    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_cell_state(&self, point: [f32; 2]) -> Result<(u8, u32, u32), Error> {
        let [x, y] = self.cell(point)?;
        let i = self
            .index(i64::from(x), i64::from(y))
            .ok_or_else(|| fail("diagnostic cell outside map"))?;
        Ok((
            self.grid.pathing.cells[i],
            self.occupied[i],
            self.footprint_occupied[i],
        ))
    }

    /// Exclude the supplied unit stamps when inspecting reach occupancy.
    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_cell_excluding(
        &self,
        point: [f32; 2],
        excluded: &[HandleId],
    ) -> Result<(u8, u32, u32), Error> {
        let [x, y] = self.cell(point)?;
        let i = self
            .index(i64::from(x), i64::from(y))
            .ok_or_else(|| fail("diagnostic cell outside map"))?;
        if self.occupied[i] == 0 {
            return Ok((self.grid.pathing.cells[i], 0, self.footprint_occupied[i]));
        }
        let removed = self
            .stamps
            .iter()
            .filter(|(id, stamp)| {
                stamp.active
                    && stamp.movement.ground()
                    && stamp.cells.binary_search(&i).is_ok()
                    && excluded.contains(id)
            })
            .count();
        let units = self.occupied[i]
            .checked_sub(
                u32::try_from(removed).map_err(|_| fail("diagnostic unit count overflow"))?,
            )
            .ok_or_else(|| fail("diagnostic unit occupancy mismatch"))?;
        Ok((
            self.grid.pathing.cells[i],
            units,
            self.footprint_occupied[i],
        ))
    }

    /// The unit's movement-stamp bounds in transposed spatial-grid coordinates.
    pub fn movement_stamp_rectangle(
        &self,
        unit: HandleId,
    ) -> Result<crate::native_path_hierarchy::CellRect, Error> {
        let stamp = self
            .stamps
            .iter()
            .find(|(id, _)| *id == unit)
            .ok_or_else(|| fail("unit movement stamp missing"))?;
        let width = self.grid.pathing.width as usize;
        let mut lower = [usize::MAX; 2];
        let mut upper = [0; 2];
        for &index in &stamp.1.cells {
            let point = [index / width, index % width];
            for axis in 0..2 {
                lower[axis] = lower[axis].min(point[axis]);
                upper[axis] = upper[axis].max(point[axis] + 1);
            }
        }
        if lower[0] == usize::MAX {
            return Err(fail("unit movement stamp empty"));
        }
        Ok(crate::native_path_hierarchy::CellRect { lower, upper })
    }

    /// The recreation's unit-stamp bounds, in underlying pathing cells.
    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_unit_rectangle(&self, unit: HandleId) -> Result<[[i32; 2]; 2], Error> {
        let rect = self.movement_stamp_rectangle(unit)?;
        Ok([
            [rect.lower[1] as i32, rect.lower[0] as i32],
            [rect.upper[1] as i32, rect.upper[0] as i32],
        ])
    }

    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_construction_position(
        &mut self,
        point: [f32; 2],
        face: f32,
        path: &str,
        prevent: u8,
        require: u8,
    ) -> Result<Option<[f32; 2]>, Error> {
        self.construction_position(None, point, face, path, prevent, require)
    }

    /// Validate with the supplied unit stamps temporarily excluded.
    pub fn diagnostic_construction_position_excluding(
        &mut self,
        point: [f32; 2],
        face: f32,
        path: &str,
        prevent: u8,
        require: u8,
        excluded: &[HandleId],
    ) -> Result<Option<[f32; 2]>, Error> {
        self.construction_position_excluding_live(
            point,
            face,
            path,
            prevent,
            require,
            (excluded, None),
        )
    }

    pub(crate) fn construction_position_excluding_live(
        &mut self,
        point: [f32; 2],
        face: f32,
        path: &str,
        prevent: u8,
        require: u8,
        context: (&[HandleId], Option<BlightQuery<'_>>),
    ) -> Result<Option<[f32; 2]>, Error> {
        let (excluded, blight) = context;
        for unit in excluded {
            self.stamps
                .iter()
                .find(|(id, _)| id == unit)
                .ok_or_else(|| fail("excluded unit occupancy is missing"))?;
        }
        self.construction_position_inner(
            (None, Some(excluded), blight),
            point,
            face,
            path,
            prevent,
            require,
        )
    }

    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_reach(
        &self,
        excluded: HandleId,
        start: [f32; 2],
        target: [f32; 2],
        radius: f32,
    ) -> Result<bool, Error> {
        let mut work = 4 << 20;
        match self.route_outcome(excluded, start, target, radius, 0., &mut work)? {
            RouteOutcome::Reached(_) => Ok(true),
            RouteOutcome::Blocked(_) => Ok(false),
            RouteOutcome::PartialBudget(_) => Err(fail("diagnostic reach work budget exhausted")),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_cell(&self, cell: usize) -> u8 {
        self.grid.pathing.cells[cell]
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    /// Resolve retained per-pixel flags, refusing when the session did not retain them.
    pub fn native_texture(&mut self, path: &str) -> Result<&NativeTexture, Error> {
        self.ensure_footprint(path)?;
        self.footprint_loader
            .as_ref()
            .and_then(|loader| loader.native_texture(path))
            .ok_or_else(|| fail(format!("native texture flags are not retained for {path}")))
    }

    pub fn source(&self) -> &Source {
        &self.source
    }
    /// Canonical raster extent using the admitted source-derived cell scale.
    pub fn world_bounds(&self) -> [[f32; 2]; 2] {
        let [x, y] = self.grid.origin;
        let cell = self.rules.cell_size();
        [
            [x, y],
            [
                x + self.grid.pathing.width as f32 * cell,
                y + self.grid.pathing.height as f32 * cell,
            ],
        ]
    }
    pub fn new(
        grid: Grid,
        rules: Rules,
        footprints: BTreeMap<String, Footprint>,
        budget: Budget,
    ) -> Result<Self, Error> {
        let count = usize::try_from(u64::from(grid.pathing.width) * u64::from(grid.pathing.height))
            .map_err(|_| fail("grid dimensions"))?;
        let footprint_count = footprints
            .values()
            .try_fold(0usize, |n, f| n.checked_add(f.cells.len()))
            .ok_or_else(|| fail("footprint count overflow"))?;
        if count == 0
            || count > budget.cells
            || grid.pathing.width > i32::MAX as u32
            || grid.pathing.height > i32::MAX as u32
            || grid.pathing.cells.len() != count
            || grid.domains.len() != count
            || footprint_count > budget.footprint_cells
            || budget.queries == 0
        {
            return Err(fail("grid/footprint/query budget or dimensions"));
        }
        if grid.origin.iter().any(|v| !v.is_finite())
            || [&grid.source.install, &grid.source.edition, &grid.source.map]
                .iter()
                .any(|v| v.trim().is_empty())
            || footprints.iter().any(|(path, f)| {
                path.is_empty()
                    || [&f.origin.source, &f.origin.edition, &f.origin.version]
                        .iter()
                        .any(|v| v.trim().is_empty())
            })
        {
            return Err(fail("missing map/footprint provenance or finite origin"));
        }
        let mut canonical = BTreeMap::new();
        for (path, footprint) in footprints {
            let key = Self::footprint_key(&path)?;
            if canonical.insert(key.clone(), footprint).is_some() {
                return Err(fail(format!("duplicate footprint path alias {key}")));
            }
        }
        let footprints = canonical;
        let mut occupied = Vec::new();
        occupied
            .try_reserve_exact(count)
            .map_err(|_| fail("occupancy allocation"))?;
        occupied.resize(count, 0);
        let mut water_occupied = Vec::new();
        water_occupied
            .try_reserve_exact(count)
            .map_err(|_| fail("water occupancy allocation"))?;
        water_occupied.resize(count, 0);
        let mut footprint_occupied = Vec::new();
        footprint_occupied
            .try_reserve_exact(count)
            .map_err(|_| fail("footprint occupancy allocation"))?;
        footprint_occupied.resize(count, 0);
        Ok(Self {
            source: grid.source.clone(),
            grid,
            rules,
            footprints,
            building_footprints: BTreeMap::new(),
            footprint_loader: None,
            script_paths: None,
            occupied,
            water_occupied,
            footprint_occupied,
            stamps: Vec::new(),
            stationary: BTreeMap::new(),
            stationary_cells: BTreeMap::new(),
            stationary_cells_used: 0,
            stationary_work: budget.stationary_work,
            base_pathing: None,
            budget,
            spatial: None,
        })
    }

    /// The live spatial chain owner, in tracked sessions.
    pub fn spatial(&self) -> Option<&PoolOwner> {
        self.spatial.as_deref()
    }

    pub(crate) fn spatial_mut(&mut self) -> Option<&mut PoolOwner> {
        self.spatial.as_deref_mut()
    }

    /// Attach the owner after it has received the session-start registrations.
    pub(crate) fn attach_spatial(&mut self, owner: PoolOwner) {
        self.spatial = Some(Box::new(owner));
    }

    /// Compose stationary destructables over an already decoration-stamped base.
    ///
    /// Each layer owns only its footprint cells. Recomposing a cell starts from
    /// the base and ORs every active layer touching that cell, so killing one
    /// tree cannot erase an overlapping tree or a terrain bit.
    pub fn with_stationary(
        mut self,
        base: Pathing,
        stamps: Vec<StationaryStamp>,
    ) -> Result<Self, Error> {
        let count = self.grid.pathing.cells.len();
        if base.width != self.grid.pathing.width
            || base.height != self.grid.pathing.height
            || base.cells.len() != count
        {
            return Err(fail("stationary base dimensions"));
        }
        if stamps.len() > self.budget.footprint_cells {
            return Err(fail("stationary layer budget"));
        }
        let cells = stamps.iter().try_fold(0usize, |total, stamp| {
            total
                .checked_add(stamp.live.len())?
                .checked_add(stamp.dead.len())
        });
        if cells.is_none_or(|cells| cells > self.budget.stationary_cells) {
            return Err(fail("stationary cell budget"));
        }
        self.base_pathing = Some(base);
        for mut stamp in stamps {
            if stamp.mask == 0 || stamp.live.iter().chain(&stamp.dead).any(|&i| i >= count) {
                return Err(fail("stationary layer mask or cell range"));
            }
            stamp.live.sort_unstable();
            stamp.live.dedup();
            stamp.dead.sort_unstable();
            stamp.dead.dedup();
            if self.stationary.contains_key(&stamp.key) {
                return Err(fail("duplicate stationary key"));
            }
            for &i in &stamp.live {
                self.stationary_cells
                    .entry(i)
                    .or_default()
                    .push((stamp.key, true));
            }
            for &i in &stamp.dead {
                self.stationary_cells
                    .entry(i)
                    .or_default()
                    .push((stamp.key, false));
            }
            self.stationary.insert(
                stamp.key,
                StationaryLayer {
                    live: stamp.live,
                    dead: stamp.dead,
                    mask: stamp.mask,
                    active: stamp.active,
                },
            );
        }
        self.stationary_cells_used = self
            .stationary
            .values()
            .try_fold(0usize, |total, layer| {
                total
                    .checked_add(layer.live.len())?
                    .checked_add(layer.dead.len())
            })
            .ok_or_else(|| fail("stationary cell count overflow"))?;
        let cells: Vec<usize> = self.stationary_cells.keys().copied().collect();
        self.recompose(&cells);
        Ok(self)
    }

    /// Toggle one placed tree. `false` means the state did not change.
    pub fn set_placed_active(&mut self, creation: u32, active: bool) -> bool {
        self.set_stationary_active(StationaryKey::Placed(creation), active)
    }

    /// Toggle one script-created destructable.
    pub fn set_destructable_active(&mut self, id: HandleId, active: bool) -> bool {
        self.set_stationary_active(StationaryKey::Destructable(id), active)
    }

    /// Toggle one retained stationary layer without allocating.
    pub fn set_stationary_active(&mut self, key: StationaryKey, active: bool) -> bool {
        let Some(layer) = self.stationary.get_mut(&key) else {
            return false;
        };
        if layer.active == active {
            return false;
        }
        layer.active = active;
        self.recompose_key(key);
        self.register_stationary(key);
        true
    }

    /// Give the spatial owner a layer's currently effective cells and mask.
    fn register_stationary(&mut self, key: StationaryKey) {
        let (Some(pool), Some(layer)) = (self.spatial.as_deref_mut(), self.stationary.get(&key))
        else {
            return;
        };
        let owner = match key {
            StationaryKey::Placed(creation) => Texture::Placed(creation),
            StationaryKey::Destructable(id) => Texture::Destructable(id),
        };
        pool.stationary_active(owner, layer.active);
    }

    /// Recompose one layer over its retained cells without allocating.
    fn recompose_key(&mut self, key: StationaryKey) {
        let (Some(base), Some(layer)) = (&self.base_pathing, self.stationary.get(&key)) else {
            return;
        };
        let stationary = &self.stationary;
        let stationary_cells = &self.stationary_cells;
        let pathing = &mut self.grid.pathing.cells;
        for &i in layer.live.iter().chain(&layer.dead) {
            let mut value = base.cells[i];
            if let Some(keys) = stationary_cells.get(&i) {
                for &(key, live) in keys {
                    let layer = &stationary[&key];
                    if layer.active == live {
                        value |= layer.mask;
                    }
                }
            }
            pathing[i] = value;
        }
    }

    fn recompose(&mut self, cells: &[usize]) {
        let Some(base) = &self.base_pathing else {
            return;
        };
        for &i in cells {
            let mut value = base.cells[i];
            if let Some(keys) = self.stationary_cells.get(&i) {
                for &(key, live) in keys {
                    let layer = &self.stationary[&key];
                    if layer.active == live {
                        value |= layer.mask;
                    }
                }
            }
            self.grid.pathing.cells[i] = value;
        }
    }

    /// Load executed footprint paths through the caller's retained source reader.
    pub fn with_footprint_loader(
        self,
        loader: impl FnMut(&str) -> Result<Footprint, String> + Send + Sync + 'static,
    ) -> Self {
        self.with_staged_footprint_loader(ClosureLoader(loader))
    }

    /// Load executed footprint paths through a source reader whose successful
    /// loads are staged until script stationary preflight accepts the layer.
    pub fn with_staged_footprint_loader(mut self, loader: impl FootprintLoader + 'static) -> Self {
        self.footprint_loader = Some(Box::new(loader));
        self
    }

    /// Resolve a script rawcode to its authored live and dead pathing textures
    /// through the same selected object catalog used for placed objects.
    pub fn with_script_paths(
        mut self,
        loader: impl Fn(i32, bool) -> Result<String, String> + Send + Sync + 'static,
    ) -> Self {
        self.script_paths = Some(Box::new(loader));
        self
    }

    /// Preflight one script-created stationary layer against the existing base
    /// raster and budgets. No handle, collision cell or layer is changed until
    /// [`Self::publish_script_destructable`].
    pub(crate) fn prepare_script_destructable(
        &mut self,
        objectid: i32,
        point: [f32; 2],
        face: f32,
        requested_dead: bool,
    ) -> Result<PreparedStationary, Error> {
        let mut loaded = Vec::new();
        let result = self.prepare_script_destructable_inner(
            objectid,
            point,
            face,
            requested_dead,
            &mut loaded,
        );
        match result {
            Ok(prepared) => {
                if let Some(loader) = self.footprint_loader.as_mut() {
                    loader.commit(&loaded);
                }
                Ok(prepared)
            }
            Err(error) => {
                for path in &loaded {
                    self.footprints.remove(path);
                }
                if let Some(loader) = self.footprint_loader.as_mut() {
                    loader.rollback(&loaded);
                }
                Err(error)
            }
        }
    }

    fn prepare_script_destructable_inner(
        &mut self,
        objectid: i32,
        point: [f32; 2],
        face: f32,
        requested_dead: bool,
        loaded: &mut Vec<String>,
    ) -> Result<PreparedStationary, Error> {
        if point.iter().any(|v| !v.is_finite()) || !face.is_finite() {
            return Err(fail("nonfinite script stationary pose"));
        }
        if self.base_pathing.is_none() {
            return Err(fail("stationary base pathing is missing"));
        }
        if self.stationary.len() >= self.budget.footprint_cells {
            return Err(fail("stationary layer budget"));
        }
        let (live_path, dead_path) = {
            let resolver = self
                .script_paths
                .as_ref()
                .ok_or_else(|| fail("script stationary path resolver is missing"))?;
            (
                resolver(objectid, false).map_err(fail)?,
                resolver(objectid, true).map_err(fail)?,
            )
        };
        self.ensure_footprint_tracked(&live_path, loaded)?;
        self.ensure_footprint_tracked(&dead_path, loaded)?;
        let mut work = 0usize;
        for path in [&live_path, &dead_path] {
            if !matches!(path.as_str(), "" | "_" | "none") {
                let key = Self::footprint_key(path)?;
                let count = self
                    .footprints
                    .get(&key)
                    .ok_or_else(|| fail(format!("missing selected footprint {path}")))?
                    .cells
                    .len();
                work = work
                    .checked_add(count)
                    .ok_or_else(|| fail("stationary work count overflow"))?;
            }
        }
        if work > self.stationary_work {
            return Err(fail("stationary footprint work budget"));
        }
        let mut live = self.script_footprint_cells(&live_path, point, face)?;
        let mut dead = self.script_footprint_cells(&dead_path, point, face)?;
        live.sort_unstable();
        live.dedup();
        dead.sort_unstable();
        dead.dedup();
        let total = self
            .stationary_cells_used
            .checked_add(live.len())
            .and_then(|total| total.checked_add(dead.len()))
            .ok_or_else(|| fail("stationary cell count overflow"))?;
        if total > self.budget.stationary_cells {
            return Err(fail("stationary cell budget"));
        }
        let mask = self.rules.solid_mask();
        if mask == 0 {
            return Err(fail("stationary layer mask"));
        }
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(live.len() + dead.len())
            .map_err(|_| fail("stationary cell allocation"))?;
        cells.extend_from_slice(&live);
        cells.extend_from_slice(&dead);
        cells.sort_unstable();
        cells.dedup();
        let native = if self.spatial.is_some() {
            let pixels = |path: &str| -> Result<TexturePixels, Error> {
                if matches!(path, "" | "_" | "none") {
                    return Ok(Vec::new());
                }
                let key = Self::footprint_key(path)?;
                let texture = self
                    .footprint_loader
                    .as_ref()
                    .and_then(|loader| loader.native_texture(&key))
                    .ok_or_else(|| {
                        fail(format!(
                            "native stationary texture flags unavailable: {path}"
                        ))
                    })?;
                crate::campaign_placement::project_native_texture(
                    &self.grid,
                    &self.rules,
                    texture,
                    point,
                    f64::from(face) / 90.0,
                )
                .map_err(fail)
            };
            Some(Box::new(StationaryTextures {
                live: pixels(&live_path)?,
                dead: pixels(&dead_path)?,
            }))
        } else {
            None
        };
        Ok(PreparedStationary {
            live,
            dead,
            cells,
            mask,
            active: !requested_dead,
            work,
            native,
        })
    }

    /// Publish a preflighted layer under its issued script handle. Every failure
    /// path belongs to [`Self::prepare_script_destructable`]; this step cannot
    /// refuse after the arena identity exists.
    pub(crate) fn publish_script_destructable(
        &mut self,
        id: HandleId,
        prepared: PreparedStationary,
    ) {
        let key = StationaryKey::Destructable(id);
        debug_assert!(!self.stationary.contains_key(&key));
        for &i in &prepared.live {
            self.stationary_cells
                .entry(i)
                .or_default()
                .push((key, true));
        }
        for &i in &prepared.dead {
            self.stationary_cells
                .entry(i)
                .or_default()
                .push((key, false));
        }
        self.stationary_cells_used += prepared.live.len() + prepared.dead.len();
        self.stationary_work -= prepared.work;
        let cells = prepared.cells;
        self.stationary.insert(
            key,
            StationaryLayer {
                live: prepared.live,
                dead: prepared.dead,
                mask: prepared.mask,
                active: prepared.active,
            },
        );
        self.recompose(&cells);
        if let (Some(pool), Some(native)) = (self.spatial.as_deref_mut(), prepared.native) {
            pool.stationary_pixels(
                Texture::Destructable(id),
                native.live,
                native.dead,
                prepared.active,
            );
        } else {
            self.register_stationary(key);
        }
    }

    fn script_footprint_cells(
        &self,
        path: &str,
        point: [f32; 2],
        face: f32,
    ) -> Result<Vec<usize>, Error> {
        if matches!(path, "" | "_" | "none") {
            return Ok(Vec::new());
        }
        let key = Self::footprint_key(path)?;
        let footprint = self
            .footprints
            .get(&key)
            .ok_or_else(|| fail(format!("missing selected footprint {path}")))?;
        let cell: [f64; 2] = std::array::from_fn(|axis| {
            ((f64::from(point[axis]) - f64::from(self.grid.origin[axis]))
                / f64::from(self.rules.cell_size()))
            .round_ties_even()
        });
        if cell.iter().any(|v| v.abs() > f64::from(i32::MAX)) {
            return Err(fail("stationary cell range"));
        }
        let quarter = (f64::from(face) / 90.0).round_ties_even().rem_euclid(4.0) as u8;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(footprint.cells.len())
            .map_err(|_| fail("stationary cell allocation"))?;
        for &[x, y] in &footprint.cells {
            let (x, y) = match quarter {
                0 => (i64::from(x), i64::from(y)),
                1 => (-i64::from(y) - 1, i64::from(x)),
                2 => (-i64::from(x) - 1, -i64::from(y) - 1),
                _ => (i64::from(y), -i64::from(x) - 1),
            };
            let (x, y) = (cell[0] as i64 + x, cell[1] as i64 + y);
            if let Some(i) = self.index(x, y) {
                cells.push(i);
            }
        }
        Ok(cells)
    }

    fn ensure_footprint(&mut self, path: &str) -> Result<(), Error> {
        let mut loaded = Vec::new();
        match self.ensure_footprint_tracked(path, &mut loaded) {
            Ok(()) => {
                if let Some(loader) = self.footprint_loader.as_mut() {
                    loader.commit(&loaded);
                }
                Ok(())
            }
            Err(error) => {
                for path in &loaded {
                    self.footprints.remove(path);
                }
                if let Some(loader) = self.footprint_loader.as_mut() {
                    loader.rollback(&loaded);
                }
                Err(error)
            }
        }
    }

    fn ensure_footprint_tracked(
        &mut self,
        path: &str,
        loaded: &mut Vec<String>,
    ) -> Result<(), Error> {
        if matches!(path, "" | "_" | "none") {
            return Ok(());
        }
        let key = Self::footprint_key(path)?;
        if self.footprints.contains_key(&key) {
            return Ok(());
        }
        let loader = self
            .footprint_loader
            .as_mut()
            .ok_or_else(|| fail(format!("missing selected footprint {path}")))?;
        let input = loader.load(&key).map_err(fail)?;
        loaded.push(key.clone());
        let count = self
            .footprints
            .values()
            .try_fold(input.cells.len(), |n, f| n.checked_add(f.cells.len()))
            .ok_or_else(|| fail("footprint count overflow"))?;
        if count > self.budget.footprint_cells
            || [
                &input.origin.source,
                &input.origin.edition,
                &input.origin.version,
            ]
            .iter()
            .any(|v| v.trim().is_empty())
        {
            return Err(fail("footprint budget or provenance"));
        }
        self.footprints.insert(key, input);
        Ok(())
    }

    fn footprint_key(path: &str) -> Result<String, Error> {
        AssetPath::new(path)
            .map(|path| path.as_str().to_owned())
            .map_err(|error| fail(format!("{path}: {error}")))
    }

    fn cell(&self, point: [f32; 2]) -> Result<[i32; 2], Error> {
        let mut cell = [0; 2];
        for axis in 0..2 {
            let value = real::sub(
                real::from_f32(point[axis]).map_err(|e| fail(e.to_string()))?,
                real::from_f32(self.grid.origin[axis]).map_err(|e| fail(e.to_string()))?,
            )
            .map_err(|e| fail(e.to_string()))?;
            let value = f32::from_bits(self.rules.shift(value, -self.rules.exponent)?);
            if value < 0.0
                || value >= [self.grid.pathing.width, self.grid.pathing.height][axis] as f32
            {
                return Err(fail("out-of-map creation requires qualified clamping"));
            }
            cell[axis] = value as i32;
        }
        Ok(cell)
    }

    pub(crate) fn pathing_flags(&self, point: [f32; 2]) -> Result<u8, Error> {
        let [x, y] = self.cell(point)?;
        let index = self
            .index(i64::from(x), i64::from(y))
            .ok_or_else(|| fail("pathing point outside map"))?;
        Ok(self.grid.pathing.cells[index])
    }

    fn index(&self, x: i64, y: i64) -> Option<usize> {
        (x >= 0
            && y >= 0
            && x < i64::from(self.grid.pathing.width)
            && y < i64::from(self.grid.pathing.height))
        .then(|| y as usize * self.grid.pathing.width as usize + x as usize)
    }

    fn rectangle(&self, cell: [i32; 2], extents: [i32; 2]) -> Result<Vec<usize>, Error> {
        let mut cells = Vec::new();
        let width = i64::from(extents[0]) + i64::from(extents[1]) + 1;
        let count = width
            .checked_mul(width)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| fail("occupied rectangle size"))?;
        if count > self.budget.footprint_cells {
            return Err(fail("occupied rectangle budget"));
        }
        cells
            .try_reserve_exact(count)
            .map_err(|_| fail("occupied rectangle allocation"))?;
        for y in
            i64::from(cell[1]) - i64::from(extents[0])..=i64::from(cell[1]) + i64::from(extents[1])
        {
            for x in i64::from(cell[0]) - i64::from(extents[0])
                ..=i64::from(cell[0]) + i64::from(extents[1])
            {
                cells.push(
                    self.index(x, y)
                        .ok_or_else(|| fail("occupied rectangle outside map"))?,
                );
            }
        }
        Ok(cells)
    }

    #[cfg(test)]
    pub(crate) fn prepare(
        &mut self,
        request: [f32; 2],
        radius: Option<f32>,
        path: &str,
        face: f32,
    ) -> Result<Prepared, Error> {
        self.prepare_class(request, radius, path, face, Movement::Ground)
    }

    pub(crate) fn construction_reach(
        &self,
        path: &str,
        radius: f32,
        worker_radius: f32,
    ) -> Result<f32, Error> {
        let key = Self::footprint_key(path)?;
        let footprint = self
            .footprints
            .get(&key)
            .ok_or_else(|| fail("construction footprint missing"))?;
        let extent = footprint
            .cells
            .iter()
            .flat_map(|cell| cell.iter())
            .map(|coordinate| {
                i64::from(*coordinate)
                    .abs()
                    .max((i64::from(*coordinate) + 1).abs())
            })
            .max()
            .ok_or_else(|| fail("construction footprint empty"))? as f32
            * self.rules.cell_size();
        let size = self.rules.cell_size();
        let reach = ((radius.max(extent) + worker_radius) / size).ceil() * size;
        if !reach.is_finite() {
            return Err(fail("construction reach overflow"));
        }
        Ok(reach)
    }

    /// A build footprint must fit at the requested grid cell without creation relocation.
    /// Masks are supplied by the source-qualified pathing token reader.
    pub(crate) fn construction_position(
        &mut self,
        worker: Option<HandleId>,
        request: [f32; 2],
        face: f32,
        path: &str,
        prevent: u8,
        require: u8,
    ) -> Result<Option<[f32; 2]>, Error> {
        self.construction_position_inner(
            (worker, None, None),
            request,
            face,
            path,
            prevent,
            require,
        )
    }

    pub(crate) fn construction_position_live(
        &mut self,
        context: (Option<HandleId>, Option<BlightQuery<'_>>),
        request: [f32; 2],
        face: f32,
        path: &str,
        prevent: u8,
        require: u8,
    ) -> Result<Option<[f32; 2]>, Error> {
        self.construction_position_inner(
            (context.0, None, context.1),
            request,
            face,
            path,
            prevent,
            require,
        )
    }

    fn construction_position_inner(
        &mut self,
        exclusions: (
            Option<HandleId>,
            Option<&[HandleId]>,
            Option<BlightQuery<'_>>,
        ),
        request: [f32; 2],
        face: f32,
        path: &str,
        prevent: u8,
        require: u8,
    ) -> Result<Option<[f32; 2]>, Error> {
        if !request.iter().all(|value| value.is_finite()) {
            return Err(fail("finite construction point required"));
        }
        self.ensure_footprint(path)?;
        let key = Self::footprint_key(path)?;
        if !self.building_footprints.contains_key(&key)
            && let Some(loader) = self.footprint_loader.as_mut()
            && let Some(building) = loader.load_building(&key).map_err(fail)?
        {
            let count = self
                .building_footprints
                .values()
                .try_fold(building.cells.len(), |n, f| n.checked_add(f.cells.len()))
                .ok_or_else(|| fail("building footprint count overflow"))?;
            if count > self.budget.footprint_cells
                || [
                    &building.origin.source,
                    &building.origin.edition,
                    &building.origin.version,
                ]
                .iter()
                .any(|value| value.trim().is_empty())
            {
                return Err(fail("building footprint budget or provenance"));
            }
            self.building_footprints.insert(key.clone(), building);
        }
        let input = self
            .building_footprints
            .get(&key)
            .or_else(|| self.footprints.get(&key))
            .ok_or_else(|| fail("construction footprint is not admitted"))?;
        if input.cells.is_empty() {
            return Err(fail("construction requires an authored footprint"));
        }
        let turns = face / 90.;
        if !face.is_finite() || turns.fract() != 0. || turns.abs() > 4. {
            return Err(fail("construction requires quarter-turn facing"));
        }
        let quarter = turns.rem_euclid(4.) as u8;
        let size = self.rules.cell_size();
        let mut anchor = [0i64; 2];
        for axis in 0..2 {
            let cell = ((request[axis] - self.grid.origin[axis]) / size).floor();
            if cell < 0. || cell >= [self.grid.pathing.width, self.grid.pathing.height][axis] as f32
            {
                return Ok(None);
            }
            anchor[axis] = cell as i64;
        }
        let exclude = exclusions.0.and_then(|worker| {
            self.stamps
                .iter()
                .find(|(id, _)| *id == worker)
                .map(|(_, stamp)| stamp)
        });
        for &[x, y] in &input.cells {
            let (x, y) = match quarter {
                0 => (i64::from(x), i64::from(y)),
                1 => (-i64::from(y) - 1, i64::from(x)),
                2 => (-i64::from(x) - 1, -i64::from(y) - 1),
                _ => (i64::from(y), -i64::from(x) - 1),
            };
            let Some(index) = self.index(anchor[0] + x, anchor[1] + y) else {
                return Ok(None);
            };
            let mut flags = self.grid.pathing.cells[index];
            if (prevent | require) & 0x20 != 0 {
                let (blight, now) = exclusions
                    .2
                    .ok_or_else(|| fail("construction blight state or clock missing"))?;
                let point = std::array::from_fn(|axis| {
                    self.grid.origin[axis] + ((anchor[axis] + [x, y][axis]) as f32 + 0.5) * size
                });
                if blight.is_blighted_for_placement(point, now).map_err(fail)? {
                    flags |= 0x20;
                }
            }
            let blocked = if let Some(excluded) = exclusions.1 {
                let removed = self
                    .stamps
                    .iter()
                    .filter(|(id, stamp)| {
                        excluded.contains(id)
                            && stamp.active
                            && stamp.movement.ground()
                            && stamp.cells.binary_search(&index).is_ok()
                    })
                    .count();
                let remaining = (self.occupied[index] as usize)
                    .checked_sub(removed)
                    .ok_or_else(|| fail("excluded unit occupancy mismatch"))?;
                flags & self.rules.ground_mask() != 0
                    || remaining != 0
                    || self.footprint_occupied[index] != 0
            } else {
                self.blocked_class(index, exclude, Movement::Ground)
            };
            #[cfg(feature = "test-fixtures")]
            if std::env::var_os("PCX_AI_CANDIDATE_TRACE").is_some() {
                let coordinates = [anchor[0] + x, anchor[1] + y];
                let requested = std::env::var("PCX_AI_TRACE_CELL")
                    .ok()
                    .is_some_and(|cell| cell == format!("{},{}", coordinates[0], coordinates[1]));
                if blocked || flags & prevent != 0 || flags & require != require || requested {
                    let stamps: Vec<_> = self
                        .stamps
                        .iter()
                        .filter_map(|(unit, stamp)| {
                            (stamp.active && stamp.cells.binary_search(&index).is_ok())
                                .then_some((*unit, stamp.movement))
                        })
                        .collect();
                    let point = std::array::from_fn(|axis| {
                        self.grid.origin[axis] + (coordinates[axis] as f32 + 0.5) * size
                    });
                    if let Some(pool) = self.spatial() {
                        let cell = pool
                            .cell_at(point)
                            .map_err(|error| fail(error.to_string()))?;
                        let links = pool
                            .chain(cell)
                            .map_err(|error| fail(error.to_string()))?
                            .map(|link| (link, pool.live().entry_state(link.identity)))
                            .collect::<Vec<_>>();
                        eprintln!(
                            "AI footprint request={request:?} cell={coordinates:?} world={point:?} flags={flags:#04x} base={:?} occupied={} footprint={} blocked={blocked} stamps={stamps:?} chain={links:?}",
                            self.base_pathing.as_ref().map(|base| base.cells[index]),
                            self.occupied[index],
                            self.footprint_occupied[index]
                        );
                    }
                }
            }
            if flags & prevent != 0 || flags & require != require || blocked {
                return Ok(None);
            }
        }
        // shortcut: source-grid anchor placement; qualify native building snap parity separately.
        Ok(Some(std::array::from_fn(|axis| {
            self.grid.origin[axis] + anchor[axis] as f32 * size
        })))
    }

    pub(crate) fn prepare_class(
        &mut self,
        request: [f32; 2],
        radius: Option<f32>,
        path: &str,
        face: f32,
        movement: Movement,
    ) -> Result<Prepared, Error> {
        self.ensure_footprint(path)?;
        self.stamps
            .try_reserve(1)
            .map_err(|_| fail("unit occupancy allocation"))?;
        self.candidate(
            request,
            radius,
            path,
            face,
            None,
            Candidate {
                creation: true,
                movement,
            },
        )?
        .ok_or_else(|| fail("creation placement refused"))
    }

    #[cfg(test)]
    fn blocked(&self, i: usize, exclude: Option<&Prepared>) -> bool {
        self.blocked_class(i, exclude, Movement::Ground)
    }

    fn blocked_class(&self, i: usize, exclude: Option<&Prepared>, movement: Movement) -> bool {
        let cell = self.grid.pathing.cells[i];
        let terrain_blocked = match movement {
            Movement::Ground | Movement::NoMovement => cell & self.rules.ground_mask() != 0,
            Movement::Air => cell & self.rules.fly_mask() != 0,
            Movement::Float => cell & self.rules.float_mask() != 0,
            Movement::Amphibious => {
                cell & self.rules.amphibious_mask() != 0
                    || (cell & self.rules.ground_mask() != 0 && cell & self.rules.float_mask() != 0)
            }
        };
        let occupies = |water: bool| {
            exclude.is_some_and(|p| {
                (if water {
                    p.movement.water()
                } else {
                    p.movement.ground()
                }) && p.cells.binary_search(&i).is_ok()
            })
        };
        terrain_blocked
            || (movement != Movement::Air
                && exclude.is_none_or(|p| p.active)
                && ((movement.ground() && self.occupied[i] > u32::from(occupies(false)))
                    || (movement.water() && self.water_occupied[i] > u32::from(occupies(true)))
                    || self.footprint_occupied[i]
                        > u32::from(
                            exclude.is_some_and(|p| p.footprint.binary_search(&i).is_ok()),
                        )))
    }

    fn candidate(
        &self,
        request: [f32; 2],
        radius: Option<f32>,
        path: &str,
        face: f32,
        exclude: Option<&Prepared>,
        options: Candidate,
    ) -> Result<Option<Prepared>, Error> {
        let Candidate { creation, movement } = options;
        if request.iter().any(|v| !v.is_finite()) {
            return Err(fail("nonfinite unit position"));
        }
        let start = self.cell(request)?;
        let wanted = self.grid.domains[self
            .index(i64::from(start[0]), i64::from(start[1]))
            .ok_or_else(|| fail("missing domain"))?];
        let mut position = request;
        let normalized_radius = radius
            .map(|value| {
                if !value.is_finite() || value < 0.0 {
                    return Err(fail("unsupported creation collision radius"));
                }
                self.rules
                    .shift(
                        real::from_f32(value).map_err(|e| fail(e.to_string()))?,
                        -self.rules.exponent,
                    )
                    .map(f32::from_bits)
            })
            .transpose()?;
        if let Some(radius) = normalized_radius {
            let extents = self.rules.extents(radius);
            let mut work = 0usize;
            let mut good = |cell: [i32; 2]| -> Result<bool, Error> {
                work = work
                    .checked_add(1)
                    .ok_or_else(|| fail("query count overflow"))?;
                if work > self.budget.queries {
                    return Err(fail("creation query budget exceeded"));
                }
                let [x, y] = cell.map(i64::from);
                if self
                    .index(x - i64::from(extents[0]), y - i64::from(extents[0]))
                    .is_none()
                    || self
                        .index(x + i64::from(extents[1]), y + i64::from(extents[1]))
                        .is_none()
                {
                    return Ok(false);
                }
                let cells = self.rectangle(cell, extents)?;
                Ok(cells
                    .iter()
                    .all(|&i| !self.blocked_class(i, exclude, movement))
                    && (!matches!(movement, Movement::Ground | Movement::NoMovement)
                        || self
                            .index(i64::from(cell[0]), i64::from(cell[1]))
                            .is_some_and(|i| self.grid.domains[i] == wanted)))
            };
            if !good(start)? {
                if !creation {
                    return Ok(None);
                }
                // shortcut: deterministic perimeter search; qualify platform overlays and native boundary failures before admitting them.
                let mut found = None;
                'rings: for ring in
                    1..=i64::from(self.grid.pathing.width.max(self.grid.pathing.height))
                {
                    let [cx, cy] = start.map(i64::from);
                    let (lo_x, lo_y, hi_x, hi_y) = (cx - ring, cy - ring, cx + ring, cy + ring);
                    let edges = [
                        (lo_x + 1, lo_y, 1, 0),
                        (hi_x, lo_y + 1, 0, 1),
                        (hi_x - 1, hi_y, -1, 0),
                        (lo_x, hi_y - 1, 0, -1),
                    ];
                    for (mut x, mut y, dx, dy) in edges {
                        for _ in 0..2 * ring {
                            if let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y))
                                && good([x, y])?
                            {
                                found = Some([x, y]);
                                break 'rings;
                            }
                            x += dx;
                            y += dy;
                        }
                    }
                }
                let found = found.ok_or_else(|| fail("no qualified placement within map"))?;
                for axis in 0..2 {
                    let center = real::add(
                        real::from_i32(found[axis]).map_err(|e| fail(e.to_string()))?,
                        self.rules.thresholds[0].to_bits(),
                    )
                    .map_err(|e| fail(e.to_string()))?;
                    position[axis] = f32::from_bits(
                        real::add(
                            self.rules.shift(center, self.rules.exponent)?,
                            self.grid.origin[axis].to_bits(),
                        )
                        .map_err(|e| fail(e.to_string()))?,
                    );
                }
            }
        }
        let position = if creation {
            self.rules.normalize(position, self.grid.origin)?
        } else {
            position
        };
        let cell = self.cell(position)?;
        let cells = if let Some(radius) = normalized_radius {
            self.rectangle(cell, self.rules.extents(radius))?
        } else {
            Vec::new()
        };
        if cells.iter().any(|&i| {
            (movement.ground() && self.occupied[i] == u32::MAX)
                || (movement.water() && self.water_occupied[i] == u32::MAX)
        }) {
            return Err(fail("occupancy count overflow"));
        }
        let mut footprint = Vec::new();
        let mut native_footprint = None;
        if !matches!(path, "" | "_" | "none") {
            let key = Self::footprint_key(path)?;
            let input = self
                .footprints
                .get(&key)
                .ok_or_else(|| fail(format!("missing selected footprint {path}")))?;
            footprint
                .try_reserve_exact(input.cells.len())
                .map_err(|_| fail("footprint allocation"))?;
            if !face.is_finite() {
                return Err(fail("finite facing required"));
            }
            let turns = f64::from(face) / 90.0;
            if turns.fract() != 0.0 || turns.abs() > 4.0 {
                return Err(fail("only quarter-turn footprint facing is admitted"));
            }
            let quarter = turns.rem_euclid(4.0) as u8;
            if self.spatial.is_some()
                && let Some(texture) = self
                    .footprint_loader
                    .as_ref()
                    .and_then(|loader| loader.native_texture(&key))
            {
                let mut pixels = Vec::new();
                pixels
                    .try_reserve_exact(texture.flags.len())
                    .map_err(|_| fail("native footprint allocation"))?;
                for (i, &flags) in texture.flags.iter().enumerate() {
                    let x = (i / usize::from(texture.width)) as i64 - i64::from(texture.height) / 2;
                    let y = (i % usize::from(texture.width)) as i64 - i64::from(texture.width) / 2;
                    let (x, y) = match quarter {
                        0 => (x, y),
                        1 => (-y - 1, x),
                        2 => (-x - 1, -y - 1),
                        _ => (y, -x - 1),
                    };
                    if flags != 0 {
                        let index = self
                            .index(i64::from(cell[0]) + x, i64::from(cell[1]) + y)
                            .ok_or_else(|| fail("native footprint outside map"))?;
                        pixels.push((
                            u32::try_from(index)
                                .map_err(|_| fail("native footprint index overflow"))?,
                            flags,
                        ));
                    }
                }
                native_footprint = Some(pixels);
            }
            // shortcut: quarter-turn footprint orientation; qualify arbitrary facing before supporting it.
            for &[x, y] in &input.cells {
                let (x, y) = match quarter {
                    0 => (i64::from(x), i64::from(y)),
                    1 => (-i64::from(y) - 1, i64::from(x)),
                    2 => (-i64::from(x) - 1, -i64::from(y) - 1),
                    _ => (i64::from(y), -i64::from(x) - 1),
                };
                footprint.push(
                    self.index(i64::from(cell[0]) + x, i64::from(cell[1]) + y)
                        .ok_or_else(|| fail("footprint outside map"))?,
                );
            }
        }
        footprint.sort_unstable();
        footprint.dedup();
        if footprint
            .iter()
            .any(|&i| self.footprint_occupied[i] == u32::MAX)
        {
            return Err(fail("footprint occupancy overflow"));
        }
        Ok(Some(Prepared {
            position,
            cells,
            footprint: match native_footprint {
                Some(pixels) => PreparedFootprint::Native(Box::new(NativeFootprint {
                    cells: footprint,
                    pixels,
                })),
                None => PreparedFootprint::Plain(footprint),
            },
            face,
            active: exclude.is_none_or(|p| p.active),
            movement,
        }))
    }

    pub(crate) fn hide(&mut self, unit: HandleId) -> Result<(), Error> {
        let stamp = self
            .stamps
            .iter_mut()
            .find(|(id, _)| *id == unit)
            .map(|(_, p)| p)
            .ok_or_else(|| fail("unit occupancy is missing"))?;
        if stamp.active {
            if stamp.movement != Movement::Air {
                for &i in &stamp.cells {
                    if stamp.movement.ground() {
                        self.occupied[i] -= 1;
                    }
                    if stamp.movement.water() {
                        self.water_occupied[i] -= 1;
                    }
                }
                for &i in stamp.footprint.iter() {
                    self.footprint_occupied[i] -= 1;
                }
            }
            stamp.active = false;
        }
        if let Some(pool) = self.spatial.as_deref_mut() {
            pool.hide(unit);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn prepare_show(
        &mut self,
        unit: HandleId,
        request: [f32; 2],
        radius: Option<f32>,
        path: &str,
        face: f32,
    ) -> Result<Prepared, Error> {
        self.prepare_show_class(unit, request, radius, path, face, Movement::Ground)
    }

    /// The facing a unit's standing footprint was oriented with, else `current`.
    fn footprint_face(&self, unit: HandleId, current: f32) -> f32 {
        self.stamps
            .iter()
            .find(|(id, stamp)| *id == unit && !stamp.footprint.is_empty())
            .map_or(current, |(_, stamp)| stamp.face)
    }

    pub(crate) fn prepare_show_class(
        &mut self,
        unit: HandleId,
        request: [f32; 2],
        radius: Option<f32>,
        path: &str,
        face: f32,
        movement: Movement,
    ) -> Result<Prepared, Error> {
        if !self.stamps.iter().any(|(id, p)| *id == unit && !p.active) {
            return Err(fail("hidden unit occupancy required"));
        }
        let face = self.footprint_face(unit, face);
        self.ensure_footprint(path)?;
        if let Some(prepared) = self.candidate(
            request,
            radius,
            path,
            face,
            None,
            Candidate {
                creation: false,
                movement,
            },
        )? {
            return Ok(prepared);
        }
        self.candidate(
            request,
            radius,
            path,
            face,
            None,
            Candidate {
                creation: true,
                movement,
            },
        )?
        .ok_or_else(|| fail("no admitted reveal placement"))
    }

    pub(crate) fn prepare_relocation_class(
        &mut self,
        unit: HandleId,
        request: [f32; 2],
        radius: Option<f32>,
        path: &str,
        face: f32,
        movement: Movement,
    ) -> Result<Prepared, Error> {
        self.ensure_footprint(path)?;
        let face = self.footprint_face(unit, face);
        let stamp = self
            .stamps
            .iter()
            .find(|(id, _)| *id == unit)
            .map(|(_, stamp)| stamp)
            .ok_or_else(|| fail("unit occupancy is missing"))?;
        self.candidate(
            request,
            radius,
            path,
            face,
            Some(stamp),
            Candidate {
                creation: true,
                movement,
            },
        )?
        .ok_or_else(|| fail("no admitted relocation placement"))
    }

    pub(crate) fn prepare_motion(
        &self,
        unit: HandleId,
        point: [f32; 2],
        radius: f32,
    ) -> Result<Option<Prepared>, Error> {
        let stamp = self
            .stamps
            .iter()
            .find(|(id, _)| *id == unit)
            .map(|(_, p)| p)
            .ok_or_else(|| fail("unit occupancy is missing"))?;
        if !stamp.footprint.is_empty() {
            return Err(fail("moving pathing textures are not admitted"));
        }
        self.candidate(
            point,
            Some(radius),
            "_",
            0.0,
            Some(stamp),
            Candidate {
                creation: false,
                movement: stamp.movement,
            },
        )
    }

    pub(crate) fn segment_clear(
        &self,
        unit: HandleId,
        from: [f32; 2],
        to: [f32; 2],
        radius: f32,
        work: &mut usize,
    ) -> Result<bool, Error> {
        Ok(self.clear_fraction(unit, from, to, radius, work)? == 1.0)
    }

    /// Fraction of `from..to` that is clear, sampled every half cell with corner
    /// checks between samples. `1.0` when the whole segment is clear. Otherwise the
    /// fraction of the last clear sample before the first blocked one, `0.0` when
    /// the first sample is blocked. A caller stops the unit at
    /// `from + (to - from) * fraction`, which is the same point the sample used.
    pub(crate) fn clear_fraction(
        &self,
        unit: HandleId,
        from: [f32; 2],
        to: [f32; 2],
        radius: f32,
        work: &mut usize,
    ) -> Result<f64, Error> {
        self.clear_fraction_checked(unit, from, to, radius, work, true)
    }

    fn segment_clear_streaming(
        &self,
        unit: HandleId,
        from: [f32; 2],
        to: [f32; 2],
        radius: f32,
        work: &mut usize,
    ) -> Result<bool, Error> {
        Ok(self.clear_fraction_checked(unit, from, to, radius, work, false)? == 1.0)
    }

    fn clear_fraction_checked(
        &self,
        unit: HandleId,
        from: [f32; 2],
        to: [f32; 2],
        radius: f32,
        work: &mut usize,
        complete_budget: bool,
    ) -> Result<f64, Error> {
        let delta = [
            f64::from(to[0]) - f64::from(from[0]),
            f64::from(to[1]) - f64::from(from[1]),
        ];
        let steps = (delta[0].hypot(delta[1]) / (f64::from(self.rules.cell_size()) * 0.5))
            .ceil()
            .max(1.);
        if !steps.is_finite()
            || steps >= usize::MAX as f64
            || (complete_budget && steps > *work as f64)
        {
            return Err(work_exhausted("movement segment work exhausted"));
        }
        let steps = steps as usize;
        let centre = |p: [i32; 2]| {
            [
                self.grid.origin[0] + (p[0] as f32 + 0.5) * self.rules.cell_size(),
                self.grid.origin[1] + (p[1] as f32 + 0.5) * self.rules.cell_size(),
            ]
        };
        let mut previous = self.cell(from)?;
        for n in 1..=steps {
            *work = work
                .checked_sub(1)
                .ok_or_else(|| work_exhausted("movement segment work exhausted"))?;
            let point = [
                (f64::from(from[0]) + delta[0] * n as f64 / steps as f64) as f32,
                (f64::from(from[1]) + delta[1] * n as f64 / steps as f64) as f32,
            ];
            let before = (n - 1) as f64 / steps as f64;
            if self.prepare_motion(unit, point, radius)?.is_none() {
                return Ok(before);
            }
            let cell = self.cell(point)?;
            if cell[0] != previous[0] && cell[1] != previous[1] {
                for p in [[cell[0], previous[1]], [previous[0], cell[1]]] {
                    *work = work
                        .checked_sub(1)
                        .ok_or_else(|| work_exhausted("movement segment work exhausted"))?;
                    if self.prepare_motion(unit, centre(p), radius)?.is_none() {
                        return Ok(before);
                    }
                }
            }
            previous = cell;
        }
        Ok(1.0)
    }

    /// Bounded deterministic cell routing on this same live collision cache.
    /// shortcut: A* tie order and cell-centre steering are reconstruction policy;
    /// qualify native path choice, local avoidance and steering before parity claims.
    /// A closed component cannot be approached from another component. Reserve half
    /// the remaining route work for forward search; an unfinished proof changes no target.
    fn isolated_route_component(
        &self,
        unit: HandleId,
        first: [i32; 2],
        start: [i32; 2],
        radius: f32,
        work: &mut usize,
    ) -> Result<Option<BTreeSet<[i32; 2]>>, Error> {
        use std::{cmp::Reverse, collections::BinaryHeap};

        if first == start {
            return Ok(None);
        }
        let allowance = *work / 2;
        let mut left = allowance;
        let result = (|| {
            let centre = |p: [i32; 2]| {
                [
                    self.grid.origin[0] + (p[0] as f32 + 0.5) * self.rules.cell_size(),
                    self.grid.origin[1] + (p[1] as f32 + 0.5) * self.rules.cell_size(),
                ]
            };
            // Collision is immutable during this proof; inspect each cell centre only once.
            let mut inspected = BTreeMap::new();
            let mut clear = |point| -> Result<Option<bool>, Error> {
                if let Some(&value) = inspected.get(&point) {
                    return Ok(Some(value));
                }
                if left == 0 {
                    return Ok(None);
                }
                left -= 1;
                let value = self.prepare_motion(unit, centre(point), radius)?.is_some();
                inspected.insert(point, value);
                Ok(Some(value))
            };
            let mut component = BTreeSet::from([first]);
            let distance = |point: [i32; 2]| {
                u64::from(point[0].abs_diff(start[0])) + u64::from(point[1].abs_diff(start[1]))
            };
            // Find a connection before flooding a large, connected component away from the mover.
            let mut pending = BinaryHeap::from([Reverse((distance(first), first))]);
            while let Some(Reverse((_, point))) = pending.pop() {
                for dy in -1..=1 {
                    'neighbour: for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let next = [point[0] + dx, point[1] + dy];
                        if component.contains(&next)
                            || self.index(i64::from(next[0]), i64::from(next[1])).is_none()
                        {
                            continue;
                        }
                        // The forward search seeds the actual start even if its cell centre is blocked.
                        if next != start {
                            match clear(next)? {
                                Some(true) => {}
                                Some(false) => continue,
                                None => return Ok(None),
                            }
                        }
                        if dx != 0 && dy != 0 {
                            for corner in [[next[0], point[1]], [point[0], next[1]]] {
                                match clear(corner)? {
                                    Some(true) => {}
                                    Some(false) => continue 'neighbour,
                                    None => return Ok(None),
                                }
                            }
                        }
                        if next == start {
                            return Ok(None);
                        }
                        component.insert(next);
                        pending.push(Reverse((distance(next), next)));
                    }
                }
            }
            Ok(Some(component))
        })();
        *work -= allowance - left;
        result
    }

    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "pathfinding.route")
    )]
    pub(crate) fn route(
        &self,
        unit: HandleId,
        start: [f32; 2],
        target: [f32; 2],
        radius: f32,
        work: &mut usize,
    ) -> Result<Vec<[f32; 2]>, Error> {
        self.route_within(unit, start, target, radius, 0.0, work)
    }

    /// Reach any clear position in the target region; zero reach preserves point orders.
    pub(crate) fn route_within(
        &self,
        unit: HandleId,
        start: [f32; 2],
        target: [f32; 2],
        radius: f32,
        reach: f32,
        work: &mut usize,
    ) -> Result<Vec<[f32; 2]>, Error> {
        match self.route_outcome(unit, start, target, radius, reach, work)? {
            RouteOutcome::Reached(path)
            | RouteOutcome::PartialBudget(path)
            | RouteOutcome::Blocked(path) => Ok(path),
        }
    }

    pub(crate) fn route_outcome(
        &self,
        unit: HandleId,
        start: [f32; 2],
        target: [f32; 2],
        radius: f32,
        reach: f32,
        work: &mut usize,
    ) -> Result<RouteOutcome, Error> {
        if !reach.is_finite() || reach < 0.0 {
            return Err(fail("invalid route reach"));
        }
        let in_range = |point: [f32; 2]| {
            ((point[0] - target[0]).powi(2) + (point[1] - target[1]).powi(2)).sqrt() <= reach
        };
        let approach = approach_point(start, target, reach);
        if (reach == 0.0 || in_range(approach))
            && self.segment_clear(unit, start, approach, radius, work)?
        {
            let mut path = Vec::new();
            path.try_reserve_exact(1)
                .map_err(|_| fail("route storage"))?;
            path.push(approach);
            return Ok(RouteOutcome::Reached(path));
        }
        self.route_search(unit, start, target, radius, reach, work)
    }

    pub(crate) fn route_outcome_streaming(
        &self,
        unit: HandleId,
        start: [f32; 2],
        target: [f32; 2],
        radius: f32,
        reach: f32,
        work: &mut usize,
    ) -> Result<RouteOutcome, Error> {
        if !reach.is_finite() || reach < 0.0 {
            return Err(fail("invalid route reach"));
        }
        let in_range = |point: [f32; 2]| {
            ((point[0] - target[0]).powi(2) + (point[1] - target[1]).powi(2)).sqrt() <= reach
        };
        let approach = approach_point(start, target, reach);
        if (reach == 0.0 || in_range(approach))
            && self.segment_clear_streaming(unit, start, approach, radius, work)?
        {
            let mut path = Vec::new();
            path.try_reserve_exact(1)
                .map_err(|_| fail("route storage"))?;
            path.push(approach);
            return Ok(RouteOutcome::Reached(path));
        }
        self.route_search(unit, start, target, radius, reach, work)
    }

    fn route_search(
        &self,
        unit: HandleId,
        start: [f32; 2],
        target: [f32; 2],
        radius: f32,
        reach: f32,
        work: &mut usize,
    ) -> Result<RouteOutcome, Error> {
        use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
        #[derive(PartialEq)]
        struct Open {
            cost: f64,
            travelled: f64,
            cell: usize,
        }
        impl Eq for Open {}
        impl PartialOrd for Open {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Open {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                other
                    .cost
                    .total_cmp(&self.cost)
                    .then_with(|| other.cell.cmp(&self.cell))
                    .then_with(|| other.travelled.total_cmp(&self.travelled))
            }
        }
        let in_range = |point: [f32; 2]| {
            ((point[0] - target[0]).powi(2) + (point[1] - target[1]).powi(2)).sqrt() <= reach
        };
        // Preserve the best path, but distinguish an unfinished search from blockage.
        let charge = |work: &mut usize| -> bool {
            match work.checked_sub(1) {
                Some(left) => {
                    *work = left;
                    true
                }
                None => false,
            }
        };
        if !charge(work) {
            return Err(work_exhausted("movement route work exhausted"));
        }
        let target_clear = reach == 0.0 && self.prepare_motion(unit, target, radius)?.is_some();
        let start_cell = self.cell(start)?;
        let target_cell = self.cell(target)?;
        let index = |p: [i32; 2]| {
            self.index(i64::from(p[0]), i64::from(p[1]))
                .ok_or_else(|| fail("movement cell outside map"))
        };
        let begin = index(start_cell)?;
        let end = index(target_cell).ok();
        let xy = |i: usize| {
            [
                (i % self.grid.pathing.width as usize) as i32,
                (i / self.grid.pathing.width as usize) as i32,
            ]
        };
        let centre = |p: [i32; 2]| {
            [
                self.grid.origin[0] + (p[0] as f32 + 0.5) * self.rules.cell_size(),
                self.grid.origin[1] + (p[1] as f32 + 0.5) * self.rules.cell_size(),
            ]
        };
        let heuristic = |p: [i32; 2]| {
            if reach > 0.0 {
                let point = centre(p);
                return ((f64::from(point[0]) - f64::from(target[0]))
                    .hypot(f64::from(point[1]) - f64::from(target[1]))
                    - f64::from(reach))
                .max(0.0)
                    / f64::from(self.rules.cell_size());
            }
            let dx = (i64::from(p[0]) - i64::from(target_cell[0])).unsigned_abs() as f64;
            let dy = (i64::from(p[1]) - i64::from(target_cell[1])).unsigned_abs() as f64;
            dx.max(dy) + (std::f64::consts::SQRT_2 - 1.) * dx.min(dy)
        };
        let nearest = if target_clear || reach > 0.0 {
            0.0
        } else {
            // Visit destination cells in octile-distance order, ignoring connectivity.
            // This proves a lower bound; the forward search still has to reach it.
            let mut candidates = BinaryHeap::new();
            candidates
                .try_reserve(1)
                .map_err(|_| fail("route storage"))?;
            let goal = index(target_cell)?;
            candidates.push(Open {
                cost: 0.0,
                travelled: 0.0,
                cell: goal,
            });
            let mut seen = BTreeSet::from([goal]);
            let mut isolated = BTreeSet::new();
            let mut nearest = f64::INFINITY;
            while let Some(open) = candidates.pop() {
                if !charge(work) {
                    return Err(work_exhausted(
                        "movement route work exhausted proving nearest target",
                    ));
                }
                let point = xy(open.cell);
                if !isolated.contains(&point)
                    && self.prepare_motion(unit, centre(point), radius)?.is_some()
                {
                    if let Some(component) =
                        self.isolated_route_component(unit, point, start_cell, radius, work)?
                    {
                        isolated.extend(component);
                    } else {
                        nearest = open.cost;
                        break;
                    }
                }
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let p = [point[0] + dx, point[1] + dy];
                        if let Ok(next) = index(p)
                            && seen.insert(next)
                        {
                            candidates
                                .try_reserve(1)
                                .map_err(|_| fail("route storage"))?;
                            candidates.push(Open {
                                cost: heuristic(p),
                                travelled: 0.0,
                                cell: next,
                            });
                        }
                    }
                }
            }
            nearest
        };
        let mut frontier = BinaryHeap::new();
        frontier.try_reserve(1).map_err(|_| fail("route storage"))?;
        frontier.push(Open {
            cost: heuristic(start_cell),
            travelled: 0.,
            cell: begin,
        });
        let mut reached = BTreeMap::new();
        reached.insert(begin, (0.0, None));
        // Nearest explored cell to the target: (remaining estimate, travelled, cell).
        let mut best = (heuristic(start_cell), 0.0, begin);
        let mut arrived = false;
        let mut exhausted = false;
        'search: while let Some(open) = frontier.pop() {
            if !charge(work) {
                exhausted = true;
                break;
            }
            if open.travelled > reached[&open.cell].0 {
                continue;
            }
            let remaining = heuristic(xy(open.cell));
            if (remaining, open.travelled) < (best.0, best.1) {
                best = (remaining, open.travelled, open.cell);
            }
            if (reach > 0.0
                && in_range(centre(xy(open.cell)))
                && self
                    .prepare_motion(unit, centre(xy(open.cell)), radius)?
                    .is_some())
                || (Some(open.cell) == end && target_clear)
            {
                best = (0.0, open.travelled, open.cell);
                arrived = true;
                break;
            }
            if reach == 0.0 && !target_clear && remaining <= nearest {
                break;
            }
            let point = xy(open.cell);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let p = [point[0] + dx, point[1] + dy];
                    let Ok(next) = index(p) else {
                        continue;
                    };
                    let distance = if dx != 0 && dy != 0 {
                        std::f64::consts::SQRT_2
                    } else {
                        1.
                    };
                    let travelled = open.travelled + distance;
                    if reached.get(&next).is_some_and(|(old, _)| *old <= travelled) {
                        continue;
                    }
                    if !charge(work) {
                        exhausted = true;
                        break 'search;
                    }
                    if self.prepare_motion(unit, centre(p), radius)?.is_none() {
                        continue;
                    }
                    if dx != 0 && dy != 0 {
                        if !charge(work) {
                            exhausted = true;
                            break 'search;
                        }
                        if self
                            .prepare_motion(unit, centre([p[0], point[1]]), radius)?
                            .is_none()
                        {
                            continue;
                        }
                        if !charge(work) {
                            exhausted = true;
                            break 'search;
                        }
                        if self
                            .prepare_motion(unit, centre([point[0], p[1]]), radius)?
                            .is_none()
                        {
                            continue;
                        }
                    }
                    frontier.try_reserve(1).map_err(|_| fail("route storage"))?;
                    reached.insert(next, (travelled, Some(open.cell)));
                    frontier.push(Open {
                        cost: travelled + heuristic(p),
                        travelled,
                        cell: next,
                    });
                }
            }
        }
        let mut path = Vec::new();
        let mut at = best.2;
        while at != begin {
            path.try_reserve(1).map_err(|_| fail("route storage"))?;
            path.push(centre(xy(at)));
            at = reached[&at].1.ok_or_else(|| fail("broken route"))?;
        }
        path.reverse();
        match path.last_mut() {
            Some(last) if arrived && reach == 0.0 => *last = target,
            Some(_) => {}
            None if arrived => {
                path.try_reserve(1).map_err(|_| fail("route storage"))?;
                path.push(if reach > 0.0 {
                    centre(xy(best.2))
                } else {
                    target
                });
            }
            // Already as near as the unit can get: stay put.
            None => {
                path.try_reserve(1).map_err(|_| fail("route storage"))?;
                path.push(start);
            }
        }
        Ok(if arrived {
            RouteOutcome::Reached(path)
        } else if exhausted {
            RouteOutcome::PartialBudget(path)
        } else {
            RouteOutcome::Blocked(path)
        })
    }

    pub(crate) fn commit(&mut self, unit: HandleId, prepared: Prepared) {
        let old = self.stamps.iter().position(|(id, _)| *id == unit);
        if let Some(at) = old {
            let stamp = &self.stamps[at].1;
            if stamp.active && stamp.movement != Movement::Air {
                for &i in &stamp.cells {
                    if stamp.movement.ground() {
                        self.occupied[i] -= 1;
                    }
                    if stamp.movement.water() {
                        self.water_occupied[i] -= 1;
                    }
                }
                for &i in stamp.footprint.iter() {
                    self.footprint_occupied[i] -= 1;
                }
            }
        }
        if prepared.active && prepared.movement != Movement::Air {
            for &i in &prepared.cells {
                if prepared.movement.ground() {
                    self.occupied[i] += 1;
                }
                if prepared.movement.water() {
                    self.water_occupied[i] += 1;
                }
            }
            for &i in prepared.footprint.iter() {
                self.footprint_occupied[i] += 1;
            }
        }
        let at = old.unwrap_or(self.stamps.len());
        if old.is_some() {
            self.stamps[at].1 = prepared;
        } else {
            self.stamps.push((unit, prepared));
        }
        if self.spatial.is_some() {
            let stamp = &self.stamps[at].1;
            if let Some(pool) = self.spatial.as_deref_mut() {
                pool.commit(
                    unit,
                    Stamp {
                        active: stamp.active,
                        movement: stamp.movement,
                        cells: &stamp.cells,
                        footprint: &stamp.footprint,
                        native_footprint: stamp.footprint.native(),
                    },
                );
            }
        }
    }

    /// A factory route needs the new unit's movement class without publishing collision cells.
    pub(crate) fn begin_creation_probe(
        &mut self,
        unit: HandleId,
        mut prepared: Prepared,
    ) -> Result<bool, Error> {
        if self.stamps.iter().any(|(id, _)| *id == unit) {
            return Err(fail("creation probe identity already exists"));
        }
        self.stamps
            .try_reserve(1)
            .map_err(|_| fail("creation probe capacity"))?;
        let active = prepared.active;
        prepared.active = false;
        self.stamps.push((unit, prepared));
        Ok(active)
    }

    pub(crate) fn finish_creation_probe(&mut self, unit: HandleId, active: bool) -> Prepared {
        let (id, mut prepared) = self.stamps.pop().expect("synchronous creation probe");
        assert_eq!(
            id, unit,
            "creation preflight must not mutate the stamp list"
        );
        prepared.active = active;
        prepared
    }
}

/// Nearest approach on the existing attack-reach circle, rounded inward.
pub(crate) fn approach_point(from: [f32; 2], target: [f32; 2], reach: f32) -> [f32; 2] {
    if reach == 0.0 {
        return target;
    }
    let delta = [
        f64::from(from[0]) - f64::from(target[0]),
        f64::from(from[1]) - f64::from(target[1]),
    ];
    let length = delta[0].hypot(delta[1]);
    if length <= f64::from(reach) {
        return from;
    }
    let ratio = f64::from(reach) / length;
    [0, 1].map(|axis| {
        let point = (f64::from(target[axis]) + delta[axis] * ratio) as f32;
        // One representable step avoids asking for another route due to outward rounding.
        if point < target[axis] {
            point.next_up()
        } else if point > target[axis] {
            point.next_down()
        } else {
            point
        }
    })
}
