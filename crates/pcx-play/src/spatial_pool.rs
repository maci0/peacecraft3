//! Live owner of the native spatial chains for sessions where an AI player can place
//! structures. One owner receives every collision registration in the order the
//! simulation performs it; the chain pool stamps the sequence.
//!
//! Entry flags and state are provisional where the notes do not decode them:
//! see "Live pool plumbing" in `docs/re-notes/ai-construction.md`. A refused
//! operation leaves the pool incomplete for the rest of the session, and a
//! consumer must check [`PoolOwner::refusal`] before trusting a chain.

use crate::native_path_hierarchy::spatial_chains::{
    ChainLink, Chains, CleanupClock, CleanupOperands, Error as ChainError, LiveChains,
    MovementStateRule, region_cells,
};
use crate::native_path_hierarchy::{CellRect, EntryRules};
use crate::unit_placement::Movement;
use pcx_script::HandleId;
use std::collections::BTreeMap;

/// Session resource budget for chain links, bounded again by the image's index limit.
const LINK_BUDGET: usize = 1 << 20;
/// Session resource budget for registered entries.
const ENTRY_BUDGET: usize = 1 << 16;
/// Identity space above every arena handle token for objects the arena does not own.
const DECORATION_TAG: u64 = 1 << 62;
const PLACED_TAG: u64 = 1 << 63;
const TEXTURE_ENTRY_TAG: u64 = 1 << 61;
type ReachOperands = (u8, u32, u32, usize, i32, u32);

/// Operands read once from the reviewed image for a tracked session.
#[derive(Clone, Debug)]
pub struct SpatialRules {
    entries: EntryRules,
    removal_class: u32,
    object_class: u32,
    region_class: u32,
    link_limit: u32,
    movement: MovementStateRule,
    movement_scale: u32,
    movement_masks: [u8; 5],
    cleanup: CleanupOperands,
    texture_masks: TextureMasks,
    placement_classes: [[u8; 4]; 8],
    town_operands: Option<(u32, usize, usize, u32)>,
    placement: Option<std::sync::Arc<crate::ai_placement::Rules>>,
    reach: Option<ReachOperands>,
}

/// Per-entry pathing masks selected by the texture registration code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureMasks {
    plain: [u8; 3],
    green: [u8; 4],
}

impl TextureMasks {
    /// Select the entries in native order. The constructor sets the extra-entry
    /// flag when any pixel has a nonzero green channel.
    pub fn for_pixels(&self, pixels: &[u8]) -> &[u8] {
        if pixels.iter().any(|flags| flags & self.green[3] != 0) {
            &self.green
        } else {
            &self.plain
        }
    }

    fn from_instructions(plain: &[u8], green: &[u8]) -> Result<Self, String> {
        fn immediate(code: &[u8], offset: usize, target: u8) -> Result<u8, String> {
            let instruction = code
                .get(offset..offset + 7)
                .ok_or("truncated texture masks")?;
            if instruction[..3] != [0xc7, 0x45, target]
                || instruction[4..] != [0, 0, 0]
                || instruction[3] == 0
            {
                return Err("unrecognized texture mask instruction".into());
            }
            Ok(instruction[3])
        }
        let plain = [
            immediate(plain, 0, 0xe4)?,
            immediate(plain, 8, 0xe8)?,
            immediate(plain, 15, 0xec)?,
        ];
        let green = [
            immediate(green, 0, 0xe0)?,
            immediate(green, 8, 0xe4)?,
            immediate(green, 15, 0xe8)?,
            immediate(green, 22, 0xec)?,
        ];
        if plain != green[..3] || green[3] & (green[0] | green[1] | green[2]) != 0 {
            return Err("unsupported texture mask relationship".into());
        }
        Ok(Self { plain, green })
    }
}

impl SpatialRules {
    pub fn reviewed_legacy(
        image: &[u8],
        rules: &crate::ai_placement::Rules,
    ) -> Result<Self, String> {
        let entries = EntryRules::reviewed_legacy(image, rules)?;
        let (removal_class, object_class) = Chains::reviewed_classes(image, rules, entries)?;
        Ok(Self {
            entries,
            removal_class,
            object_class,
            region_class: Chains::reviewed_region_class(image, rules)?,
            link_limit: Chains::reviewed_limit(image, rules)?,
            movement: MovementStateRule::reviewed_legacy(image, rules)?,
            movement_scale: movement_scale(image)?,
            movement_masks: movement_masks(image)?,
            cleanup: Chains::reviewed_cleanup_operands(image, rules)?,
            texture_masks: texture_masks(image)?,
            placement_classes: placement_classes(image)?,
            town_operands: Some(town_operands(image)?),
            placement: Some(std::sync::Arc::new(rules.clone())),
            reach: Some(crate::native_path_hierarchy::reviewed_operands(
                image, rules,
            )?),
        })
    }

    pub(crate) fn cleanup(&self) -> &CleanupOperands {
        &self.cleanup
    }

    /// Source-derived masks for one texture's entry set.
    pub fn texture_masks(&self) -> TextureMasks {
        self.texture_masks
    }

    #[cfg(test)]
    pub(crate) fn synthetic(entries: EntryRules) -> Self {
        Self {
            entries,
            removal_class: 0,
            object_class: 0x3000,
            region_class: 0x2000,
            link_limit: 0xfffff,
            movement: MovementStateRule::synthetic(4.0, 0x40),
            movement_scale: 1.0_f32.to_bits(),
            movement_masks: [8, 0, 4, 2, 1],
            cleanup: CleanupOperands::synthetic(),
            texture_masks: TextureMasks {
                plain: [1, 2, 4],
                green: [1, 2, 4, 8],
            },
            placement_classes: [[0; 4]; 8],
            town_operands: None,
            placement: None,
            reach: None,
        }
    }
}

fn placement_classes(image: &[u8]) -> Result<[[u8; 4]; 8], String> {
    use crate::neutral_player::RuleInputBudget;
    use crate::neutral_rule_input::Pe;

    let pe = Pe::parse(
        image,
        RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 64 << 10,
            sections: 96,
        },
    )
    .map_err(|error| error.to_string())?;
    let mut classes = [[0; 4]; 8];
    for (class, address) in classes.iter_mut().zip([
        0xb2ed80, 0xc19fc0, 0xc485a0, 0xb9d5e0, 0xb9e8c0, 0xb9e5f0, 0xb4b780, 0xc1b3e0,
    ]) {
        let bytes = pe
            .read_from(address, true)
            .map_err(|error| error.to_string())?;
        if bytes.len() < 6 || bytes[0] != 0xb8 || bytes[5] != 0xc3 {
            return Err(format!(
                "unrecognized placement class getter at {address:#x}"
            ));
        }
        *class = u32::from_le_bytes(bytes[1..5].try_into().unwrap()).to_be_bytes();
    }
    Ok(classes)
}

fn town_operands(image: &[u8]) -> Result<(u32, usize, usize, u32), String> {
    use crate::neutral_player::RuleInputBudget;
    use crate::neutral_rule_input::Pe;
    let pe = Pe::parse(
        image,
        RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 64 << 10,
            sections: 96,
        },
    )
    .map_err(|error| error.to_string())?;
    let threshold = pe
        .read_from(0xac745b, true)
        .map_err(|error| error.to_string())?;
    let work = pe
        .read_from(0xac7467, true)
        .map_err(|error| error.to_string())?;
    let increment = pe
        .read_from(0xac7518, true)
        .map_err(|error| error.to_string())?;
    let merge = pe
        .read_from(0xac757d, true)
        .map_err(|error| error.to_string())?;
    decode_town_operands(threshold, work, increment, merge)
}

fn decode_town_operands(
    threshold: &[u8],
    work: &[u8],
    increment: &[u8],
    merge: &[u8],
) -> Result<(u32, usize, usize, u32), String> {
    if threshold.len() < 7
        || threshold[..3] != [0xc7, 0x45, 0xf8]
        || work.len() < 5
        || work[0] != 0xbb
        || increment.len() < 3
        || increment[..2] != [0x83, 0xc0]
        || increment[2] >= 0x80
        || merge.len() < 7
        || merge[..3] != [0xc7, 0x45, 0xe0]
    {
        return Err("unrecognized town-association search operands".into());
    }
    Ok((
        u32::from_le_bytes(threshold[3..7].try_into().unwrap()),
        usize::try_from(u32::from_le_bytes(work[1..5].try_into().unwrap()))
            .map_err(|_| "town work limit")?,
        usize::from(increment[2]),
        u32::from_le_bytes(merge[3..7].try_into().unwrap()),
    ))
}

fn movement_masks(image: &[u8]) -> Result<[u8; 5], String> {
    use crate::neutral_player::RuleInputBudget;
    use crate::neutral_rule_input::Pe;

    let pe = Pe::parse(
        image,
        RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 64 << 10,
            sections: 96,
        },
    )
    .map_err(|error| error.to_string())?;
    let read = |address, code| {
        pe.read_from(address, code)
            .map_err(|error| format!("movement mask source at {address:#x}: {error}"))
    };
    let dispatch = read(0x67ce7d, true)?;
    if dispatch.get(..7) != Some(&[0x33, 0xc9, 0x8d, 0x50, 0xff, 0x83, 0xfa])
        || dispatch.get(8..9) != Some(&[0x77])
        || dispatch.get(10..13) != Some(&[0x0f, 0xb6, 0x82])
        || dispatch.get(17..20) != Some(&[0xff, 0x24, 0x85])
    {
        return Err("unrecognized unit movement-mask dispatch".into());
    }
    let address = |bytes: &[u8]| u32::from_le_bytes(bytes.try_into().expect("checked width"));
    let indices = read(address(&dispatch[13..17]), true)?;
    let targets = read(
        address(
            dispatch
                .get(20..24)
                .ok_or("truncated movement mask dispatch")?,
        ),
        true,
    )?;
    let lookup = read(0x691a9b, true)?;
    if lookup.first() != Some(&0xb8) || lookup.len() < 5 {
        return Err("unrecognized movement-type token table".into());
    }
    let table = read(address(&lookup[1..5]), false)?;
    let mut values = [None; 9];
    let names = [
        "foot", "horse", "hover", "none", "_", "-", "fly", "float", "amph",
    ];
    for record in table.as_chunks::<8>().0.iter().take(32) {
        let pointer = address(&record[..4]);
        if pointer == 0 {
            break;
        }
        let text = read(pointer, false)?;
        let length = text
            .iter()
            .position(|&byte| byte == 0)
            .ok_or("unterminated movement token")?;
        let name = std::str::from_utf8(&text[..length]).map_err(|_| "invalid movement token")?;
        let Some(index) = names.iter().position(|&candidate| candidate == name) else {
            continue;
        };
        let kind = address(&record[4..]);
        let mask = if kind == 0 || kind - 1 > u32::from(dispatch[7]) {
            0
        } else {
            let arm = *indices
                .get((kind - 1) as usize)
                .ok_or("movement mask index missing")? as usize;
            let target = targets
                .get(arm * 4..arm * 4 + 4)
                .ok_or("movement mask target missing")?;
            let instruction = read(address(target), true)?;
            match instruction.first() {
                Some(0xb9) => u8::try_from(address(
                    instruction.get(1..5).ok_or("movement mask truncated")?,
                ))
                .map_err(|_| "movement mask exceeds a byte")?,
                Some(0x89) if instruction.get(..2) == Some(&[0x89, 0x8b]) => 0,
                _ => return Err("unrecognized movement mask case".into()),
            }
        };
        if values[index].replace(mask).is_some() {
            return Err("duplicate movement-type token".into());
        }
    }
    let value = |index: usize| {
        values[index].ok_or_else(|| format!("missing movement token {}", names[index]))
    };
    if value(0)? != value(1)?
        || value(0)? != value(2)?
        || value(3)? != value(4)?
        || value(3)? != value(5)?
    {
        return Err("unsupported movement-type mask grouping".into());
    }
    Ok([value(0)?, value(3)?, value(6)?, value(7)?, value(8)?])
}

fn texture_masks(image: &[u8]) -> Result<TextureMasks, String> {
    use crate::neutral_player::RuleInputBudget;
    use crate::neutral_rule_input::Pe;

    let pe = Pe::parse(
        image,
        RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 64 << 10,
            sections: 96,
        },
    )
    .map_err(|error| error.to_string())?;
    let read = |address, len| -> Result<&[u8], String> {
        pe.read_from(address, true)
            .map_err(|error| error.to_string())?
            .get(..len)
            .ok_or_else(|| format!("truncated texture masks at {address:#x}"))
    };
    let gate = read(0x6c0943, 4)?;
    let constructor = read(0x4deb67, 20)?;
    if gate != [0xf6, 0x46, 0x5c, 0x01]
        || constructor[..4] != [0x80, 0x79, 0x01, 0x00]
        || constructor[9..12] != [0x80, 0x0c, 0x03]
        || constructor[16..20] != [0x83, 0x4f, 0x5c, 0x01]
    {
        return Err("unrecognized texture green-channel selector".into());
    }
    let masks = TextureMasks::from_instructions(read(0x6c0976, 22)?, read(0x6c0952, 29)?)?;
    if constructor[12] != masks.green[3] {
        return Err("texture green flag differs from its entry mask".into());
    }
    Ok(masks)
}

fn movement_scale(image: &[u8]) -> Result<u32, String> {
    use crate::neutral_player::RuleInputBudget;
    use crate::neutral_rule_input::Pe;

    let pe = Pe::parse(
        image,
        RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 64 << 10,
            sections: 96,
        },
    )
    .map_err(|error| error.to_string())?;
    let read = |address, len| -> Result<&[u8], String> {
        pe.read_from(address, true)
            .map_err(|error| error.to_string())?
            .get(..len)
            .ok_or_else(|| format!("truncated movement scaling at {address:#x}"))
    };
    let x = read(0x803f2e, 5)?;
    let y = read(0x803f48, 6)?;
    let limit = read(0x800586, 5)?;
    movement_scale_operands(x, y, limit)
}

fn movement_scale_operands(x: &[u8], y: &[u8], limit: &[u8]) -> Result<u32, String> {
    if x.len() != 5
        || y.len() != 6
        || limit.len() != 5
        || x[0] != 0x05
        || y[..2] != [0x81, 0xc1]
        || limit[0] != 0x05
        || x[1..] != y[2..]
        || x[1..] != limit[1..]
    {
        return Err("unrecognized movement scaling instructions".into());
    }
    let delta = u32::from_le_bytes(x[1..].try_into().expect("checked width"));
    let magnitude = delta.wrapping_neg();
    let shift = magnitude >> 23;
    if magnitude & ((1 << 23) - 1) != 0 || shift == 0 || shift >= 127 {
        return Err("unsupported movement scaling operand".into());
    }
    Ok(1.0_f32.to_bits() - (shift << 23))
}

/// The pathing grid the chains cover. Native axis 0 is the world y axis.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub origin: [f32; 2],
    pub cell_size: f32,
    pub width: usize,
    pub height: usize,
}

impl Geometry {
    fn size(self) -> [usize; 2] {
        [self.height, self.width]
    }
}

fn update_blight_pathing(
    original: &pcx_map::Pathing,
    layer: &mut pcx_map::Pathing,
    geometry: Geometry,
    blight: &crate::terrain_blight::InitialBlight,
    now: f64,
) -> Result<(), String> {
    if geometry.width == 0
        || geometry.height == 0
        || original.width as usize != geometry.width
        || original.height as usize != geometry.height
        || geometry.width.checked_mul(geometry.height) != Some(original.cells.len())
        || original.width != layer.width
        || original.height != layer.height
        || original.cells.len() != layer.cells.len()
    {
        return Err("spatial blight dimensions differ".into());
    }
    for (index, cell) in layer.cells.iter_mut().enumerate() {
        let x = index % geometry.width;
        let y = index / geometry.width;
        let point = [
            geometry.origin[0] + (x as f32 + 0.5) * geometry.cell_size,
            geometry.origin[1] + (y as f32 + 0.5) * geometry.cell_size,
        ];
        *cell = original.cells[index]
            | if blight.is_blighted_at(point, now)? {
                0x20
            } else {
                0
            };
    }
    Ok(())
}

/// The first operation the owner could not apply. Later operations are not applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub identity: u64,
    pub operation: &'static str,
    pub error: ChainError,
}

/// Collision stamp fields the owner reads at a commit.
pub(crate) struct Stamp<'a> {
    pub active: bool,
    pub movement: Movement,
    pub cells: &'a [usize],
    pub footprint: &'a [usize],
    pub native_footprint: Option<&'a [(u32, u8)]>,
}

/// A pathing-texture owner other than a unit.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Texture {
    Decoration(u32),
    Placed(u32),
    Destructable(HandleId),
}

impl Texture {
    fn identity(self) -> u64 {
        match self {
            Self::Decoration(index) => DECORATION_TAG | u64::from(index),
            Self::Placed(creation) => PLACED_TAG | u64::from(creation),
            Self::Destructable(id) => id.token(),
        }
    }
}

enum Tracked {
    Mover(CellRect),
    /// Row runs covering the entry's cells; empty while the object has no pathing texture.
    Texture(Vec<CellRect>),
    Region,
}

pub(crate) type TexturePixels = Vec<(u32, u8)>;

struct StationaryPixels {
    live: TexturePixels,
    dead: TexturePixels,
}

pub struct PoolOwner {
    live: LiveChains,
    rules: SpatialRules,
    geometry: Geometry,
    context: Option<std::sync::Arc<crate::map_admission::ParsedMapContext>>,
    blight_pathing: Option<Box<pcx_map::Pathing>>,
    tracked: BTreeMap<u64, Tracked>,
    texture_entries: BTreeMap<u64, Vec<(u64, u8)>>,
    stationary_pixels: BTreeMap<u64, StationaryPixels>,
    next_texture_entry: u64,
    scratch_cells: Vec<[usize; 2]>,
    /// Last commanded velocity, limit and whether a non-order contributor was present.
    velocities: BTreeMap<HandleId, ([f32; 2], f32, bool)>,
    refusal: Option<Refusal>,
}

impl PoolOwner {
    /// Allocate the pool and entry table at game time `start`.
    pub fn new(
        rules: SpatialRules,
        geometry: Geometry,
        start: f32,
        interval: f32,
    ) -> Result<Self, ChainError> {
        let capacity = LINK_BUDGET.min(usize::try_from(rules.link_limit).unwrap_or(usize::MAX));
        let chains = Chains::new(
            geometry.size(),
            capacity,
            rules.link_limit,
            rules.removal_class,
        )?;
        let live = LiveChains::new(
            chains,
            CleanupClock::new(start, interval)?,
            rules.entries,
            ENTRY_BUDGET,
        )?;
        Ok(Self {
            live,
            rules,
            geometry,
            context: None,
            blight_pathing: None,
            tracked: BTreeMap::new(),
            texture_entries: BTreeMap::new(),
            stationary_pixels: BTreeMap::new(),
            next_texture_entry: 0,
            scratch_cells: Vec::new(),
            velocities: BTreeMap::new(),
            refusal: None,
        })
    }

    /// The first refused operation, or `None` while every operation was applied.
    pub fn refusal(&self) -> Option<&Refusal> {
        self.refusal.as_ref()
    }

    pub(crate) fn retain_context(
        &mut self,
        context: std::sync::Arc<crate::map_admission::ParsedMapContext>,
    ) {
        self.context = Some(context);
    }

    pub(crate) fn placement_rules(&self) -> Option<&crate::ai_placement::Rules> {
        self.rules.placement.as_deref()
    }

    pub(crate) fn placement_classes(&self) -> [[u8; 4]; 8] {
        self.rules.placement_classes
    }

    pub(crate) fn town_operands(&self) -> Option<(u32, usize, usize, u32)> {
        self.rules.town_operands
    }

    pub(crate) fn texture_rectangle(&self, unit: HandleId) -> Option<CellRect> {
        let entries = self.texture_entries.get(&unit.token())?;
        let mut lower = [usize::MAX; 2];
        let mut upper = [0; 2];
        for (entry, _) in entries {
            let Tracked::Texture(runs) = self.tracked.get(entry)? else {
                return None;
            };
            for run in runs {
                for axis in 0..2 {
                    lower[axis] = lower[axis].min(run.lower[axis]);
                    upper[axis] = upper[axis].max(run.upper[axis]);
                }
            }
        }
        (lower[0] != usize::MAX).then_some(CellRect { lower, upper })
    }

    pub(crate) fn reach_inputs(&self) -> Option<(&pcx_map::Pathing, ReachOperands)> {
        Some((
            self.blight_pathing
                .as_deref()
                .unwrap_or(&self.context.as_ref()?.world.pathing),
            self.rules.reach?,
        ))
    }

    pub(crate) fn refresh_blight(
        &mut self,
        blight: &crate::terrain_blight::InitialBlight,
        now: f64,
    ) -> Result<(), String> {
        let original = &self
            .context
            .as_ref()
            .ok_or("spatial blight source missing")?
            .world
            .pathing;
        if original.width as usize != self.geometry.width
            || original.height as usize != self.geometry.height
            || self.geometry.width.checked_mul(self.geometry.height) != Some(original.cells.len())
        {
            return Err("spatial blight dimensions differ".into());
        }
        if self.blight_pathing.is_none() {
            let mut cells = Vec::new();
            cells
                .try_reserve_exact(original.cells.len())
                .map_err(|_| "spatial blight allocation")?;
            cells.extend_from_slice(&original.cells);
            self.blight_pathing = Some(Box::new(pcx_map::Pathing {
                version: original.version,
                width: original.width,
                height: original.height,
                cells,
            }));
        }
        let layer = self.blight_pathing.as_mut().expect("just admitted");
        update_blight_pathing(original, layer, self.geometry, blight, now)
    }

    /// Chains, entry states and the registration sequence.
    pub fn live(&self) -> &LiveChains {
        &self.live
    }

    /// The cell's links, newest first.
    pub fn chain(
        &self,
        cell: [usize; 2],
    ) -> Result<impl Iterator<Item = ChainLink> + '_, ChainError> {
        self.live.chain(cell)
    }

    /// The chain cell containing a world point, clipped to the grid like a region rectangle.
    pub fn cell_at(&self, point: [f32; 2]) -> Result<[usize; 2], ChainError> {
        let rect = region_cells(
            [point[0], point[1], point[0], point[1]],
            self.geometry.origin,
            self.geometry.cell_size,
            self.geometry.size(),
        )?;
        Ok(rect.lower)
    }

    fn note(&mut self, identity: u64, operation: &'static str, result: Result<(), ChainError>) {
        if let Err(error) = result {
            self.refusal.get_or_insert(Refusal {
                identity,
                operation,
                error,
            });
        }
    }

    fn chain_cell(&self, index: usize) -> [usize; 2] {
        [index / self.geometry.width, index % self.geometry.width]
    }

    fn bounds(&self, cells: &[usize]) -> Option<CellRect> {
        let mut lower = [usize::MAX; 2];
        let mut upper = [0; 2];
        for &index in cells {
            let cell = self.chain_cell(index);
            for axis in 0..2 {
                lower[axis] = lower[axis].min(cell[axis]);
                upper[axis] = upper[axis].max(cell[axis] + 1);
            }
        }
        (lower[0] != usize::MAX).then_some(CellRect { lower, upper })
    }

    /// Row runs in chain order: native axis 1 major, axis 0 contiguous.
    fn runs(&mut self, pixels: &[(u32, u8)], mask: u8) -> Vec<CellRect> {
        let width = self.geometry.width;
        self.scratch_cells.clear();
        self.scratch_cells
            .extend(
                pixels
                    .iter()
                    .filter(|&&(_, flags)| flags & mask != 0)
                    .map(|&(cell, _)| {
                        let cell = cell as usize;
                        [cell / width, cell % width]
                    }),
            );
        self.scratch_cells
            .sort_unstable_by_key(|cell| (cell[1], cell[0]));
        self.scratch_cells.dedup();
        let mut runs: Vec<CellRect> = Vec::new();
        for &[x, y] in &self.scratch_cells {
            match runs.last_mut() {
                Some(run) if run.lower[1] == y && run.upper[0] == x => run.upper[0] += 1,
                _ => runs.push(CellRect {
                    lower: [x, y],
                    upper: [x + 1, y + 1],
                }),
            }
        }
        runs
    }

    /// A unit's collision stamp was committed. A unit with a pathing texture is a
    /// texture entry; any other stamped unit is a movement entry.
    pub(crate) fn commit(&mut self, unit: HandleId, stamp: Stamp<'_>) {
        if self.refusal.is_some() {
            return;
        }
        if !stamp.active {
            self.hide(unit);
            return;
        }
        let identity = unit.token();
        if let Some(pixels) = stamp.native_footprint {
            self.set_texture_pixels(identity, pixels);
        } else if !stamp.footprint.is_empty() {
            let error = if matches!(self.tracked.get(&identity), Some(Tracked::Mover(_))) {
                ChainError::DuplicateEntry(identity)
            } else {
                ChainError::MissingTexturePixels(identity)
            };
            self.note(identity, "register texture", Err(error));
        } else if let Some(rect) = self.bounds(stamp.cells) {
            let index = match stamp.movement {
                Movement::Ground => 0,
                Movement::NoMovement => 1,
                Movement::Air => 2,
                Movement::Float => 3,
                Movement::Amphibious => 4,
            };
            self.set_mover(identity, self.rules.movement_masks[index], rect);
        } else {
            self.hide(unit);
        }
    }

    /// A unit left the collision cache: hidden, dead or removed.
    pub(crate) fn hide(&mut self, unit: HandleId) {
        if self.refusal.is_some() {
            return;
        }
        let identity = unit.token();
        if let Some(entries) = self.texture_entries.get(&identity) {
            let mut copy = [(0, 0); 4];
            let len = entries.len();
            copy[..len].copy_from_slice(entries);
            for (entry, mask) in copy[..len].iter().copied() {
                self.set_texture(entry, mask, Vec::new());
            }
            return;
        }
        match self.tracked.get(&identity) {
            Some(Tracked::Mover(_)) => {
                let result = self.live.retire_movement(identity).map(drop);
                self.tracked.remove(&identity);
                self.velocities.remove(&unit);
                self.note(identity, "retire movement", result);
            }
            Some(Tracked::Texture(runs)) if !runs.is_empty() => {
                self.set_texture(identity, 0, Vec::new());
            }
            _ => {}
        }
    }

    pub(crate) fn decoration_pixels(&mut self, owner: Texture, pixels: &[(u32, u8)]) {
        if self.refusal.is_none() {
            self.set_texture_pixels(owner.identity(), pixels);
        }
    }

    pub(crate) fn stationary_pixels(
        &mut self,
        owner: Texture,
        live: TexturePixels,
        dead: TexturePixels,
        active: bool,
    ) {
        if self.refusal.is_some() {
            return;
        }
        let identity = owner.identity();
        self.set_texture_pixels(identity, if active { &live } else { &dead });
        self.stationary_pixels
            .insert(identity, StationaryPixels { live, dead });
    }

    pub(crate) fn stationary_active(&mut self, owner: Texture, active: bool) {
        let Some(layers) = self.stationary_pixels.get(&owner.identity()) else {
            self.note(
                owner.identity(),
                "stationary texture",
                Err(ChainError::MissingTexturePixels(owner.identity())),
            );
            return;
        };
        let pixels = if active { &layers.live } else { &layers.dead }.clone();
        self.decoration_pixels(owner, &pixels);
    }

    fn set_mover(&mut self, identity: u64, mask: u8, rect: CellRect) {
        let object_class = self.rules.object_class;
        if matches!(self.tracked.get(&identity), Some(Tracked::Mover(_)))
            && let Some((flags, state)) = self.live.entry_state(identity)
        {
            let updated = (flags & !u32::from(u8::MAX)) | u32::from(mask);
            if flags != updated {
                let result = self.live.update_state(identity, updated, state);
                self.note(identity, "update movement mask", result);
                if self.refusal.is_some() {
                    return;
                }
            }
        }
        match self.tracked.get(&identity) {
            None => {
                let (flags, state) = self.rules.entries.movement_entry(mask);
                let result = self
                    .live
                    .register(identity, flags, state, rect, object_class, true)
                    .map(drop);
                if result.is_ok() {
                    self.tracked.insert(identity, Tracked::Mover(rect));
                }
                self.note(identity, "register movement", result);
            }
            Some(Tracked::Mover(old)) if *old != rect => {
                let result = self.live.move_entry(identity, rect, object_class).map(drop);
                if result.is_ok() {
                    self.tracked.insert(identity, Tracked::Mover(rect));
                }
                self.note(identity, "move movement", result);
            }
            Some(Tracked::Mover(_)) => {}
            Some(_) => self.note(
                identity,
                "register movement",
                Err(ChainError::DuplicateEntry(identity)),
            ),
        }
    }

    fn set_texture_pixels(&mut self, owner: u64, pixels: &[(u32, u8)]) {
        if matches!(self.tracked.get(&owner), Some(Tracked::Mover(_))) {
            self.note(
                owner,
                "texture replaces movement",
                Err(ChainError::DuplicateEntry(owner)),
            );
            return;
        }
        let has_green = pixels
            .iter()
            .any(|(_, flags)| flags & self.rules.texture_masks.green[3] != 0);
        let texture_masks = self.rules.texture_masks;
        let masks = if has_green {
            texture_masks.green.as_slice()
        } else {
            texture_masks.plain.as_slice()
        };
        let mut entries = [(0, 0); 4];
        if let Some(current) = self.texture_entries.get(&owner) {
            if current
                .iter()
                .map(|(_, mask)| *mask)
                .ne(masks.iter().copied())
            {
                self.note(
                    owner,
                    "texture entry masks changed",
                    Err(ChainError::DuplicateEntry(owner)),
                );
                return;
            }
            entries[..masks.len()].copy_from_slice(current);
        } else {
            for (entry, &mask) in entries.iter_mut().zip(masks) {
                let Some(next) = self
                    .next_texture_entry
                    .checked_add(1)
                    .filter(|next| *next < TEXTURE_ENTRY_TAG)
                else {
                    self.note(owner, "texture entry identity", Err(ChainError::Sequence));
                    return;
                };
                self.next_texture_entry = next;
                *entry = (TEXTURE_ENTRY_TAG | next, mask);
            }
            self.texture_entries
                .insert(owner, entries[..masks.len()].to_vec());
        }
        for (entry, mask) in entries[..masks.len()].iter().copied() {
            if self.refusal.is_some() {
                return;
            }
            let runs = self.runs(pixels, mask);
            self.set_texture(entry, mask, runs);
        }
    }

    fn set_texture(&mut self, identity: u64, mask: u8, runs: Vec<CellRect>) {
        let object_class = self.rules.object_class;
        let (flags, state) = self.rules.entries.texture_entry(mask);
        match self.tracked.get(&identity) {
            None if runs.is_empty() => {}
            None => {
                let mut result = self
                    .live
                    .register(identity, flags, state, runs[0], object_class, false)
                    .map(drop);
                for run in &runs[1..] {
                    result = result.and_then(|()| {
                        self.live
                            .add_rectangle(identity, *run, object_class)
                            .map(drop)
                    });
                }
                self.tracked.insert(identity, Tracked::Texture(runs));
                self.note(identity, "register texture", result);
            }
            Some(Tracked::Texture(old)) if *old == runs => {}
            Some(Tracked::Texture(old)) => {
                let old = old.clone();
                let mut result = Ok(());
                for run in &old {
                    result =
                        result.and_then(|()| self.live.remove_rectangle(identity, *run).map(drop));
                }
                if old.is_empty() != runs.is_empty() {
                    let flags = if runs.is_empty() {
                        let held = self
                            .live
                            .entry_state(identity)
                            .map_or(flags, |(held, _)| held);
                        self.rules.entries.without_active(held)
                    } else {
                        flags
                    };
                    result = result.and_then(|()| self.live.update_state(identity, flags, state));
                }
                for run in &runs {
                    result = result.and_then(|()| {
                        self.live
                            .add_rectangle(identity, *run, object_class)
                            .map(drop)
                    });
                }
                self.tracked.insert(identity, Tracked::Texture(runs));
                self.note(identity, "replace texture", result);
            }
            Some(_) => self.note(
                identity,
                "register texture",
                Err(ChainError::DuplicateEntry(identity)),
            ),
        }
    }

    /// A rectangle was added to a region. The first addition registers the region.
    pub(crate) fn region_added(&mut self, region: HandleId, bounds: [f32; 4]) {
        if self.refusal.is_some() {
            return;
        }
        let identity = region.token();
        let rect = region_cells(
            bounds,
            self.geometry.origin,
            self.geometry.cell_size,
            self.geometry.size(),
        );
        let class = self.rules.region_class;
        let result = rect.and_then(|rect| {
            if self.tracked.contains_key(&identity) {
                self.live.add_rectangle(identity, rect, class).map(drop)
            } else {
                let (flags, state) = self.rules.entries.region_initial_state();
                self.tracked.insert(identity, Tracked::Region);
                self.live
                    .register(identity, flags, state, rect, class, false)
                    .map(drop)
            }
        });
        self.note(identity, "add region rectangle", result);
    }

    /// Apply the commanded movement contribution of a moving unit. Only a change in
    /// velocity or in the presence of a non-order contributor is applied.
    pub(crate) fn command_velocity(
        &mut self,
        unit: HandleId,
        velocity: [f32; 2],
        limit: f32,
        contributor: Option<&'static str>,
    ) {
        if self.refusal.is_some()
            || !matches!(self.tracked.get(&unit.token()), Some(Tracked::Mover(_)))
        {
            return;
        }
        let key = (velocity, limit, contributor.is_some());
        if self
            .velocities
            .get(&unit)
            .copied()
            .unwrap_or(([0.; 2], 0., false))
            == key
        {
            return;
        }
        let scale = |value| {
            crate::script_random::real::from_f32(value)
                .and_then(|bits| crate::script_random::real::mul(bits, self.rules.movement_scale))
                .map(f32::from_bits)
                .map_err(|_| ChainError::MovementState)
        };
        let result = scale(velocity[0]).and_then(|x| {
            let y = scale(velocity[1])?;
            let limit = scale(limit)?;
            self.live.update_walking(
                unit.token(),
                self.rules.movement,
                [x, y],
                limit,
                contributor,
            )
        });
        if key == ([0.; 2], 0., false) {
            self.velocities.remove(&unit);
        } else {
            self.velocities.insert(unit, key);
        }
        self.note(unit.token(), "movement state", result);
    }

    /// Units whose last commanded velocity, limit or contributor flag was not the idle default.
    pub(crate) fn moving_units(&self) -> impl Iterator<Item = HandleId> + '_ {
        self.velocities.keys().copied()
    }

    /// Deliver every cleanup deadline due at game time `now`.
    pub(crate) fn cleanup(&mut self, now: f64) {
        if self.refusal.is_some() {
            return;
        }
        let now = now as f32;
        loop {
            match self.live.cleanup_due(now) {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    self.note(0, "cleanup", Err(error));
                    break;
                }
            }
        }
    }
}

/// The session's live chain owner. `None` in an untracked session and before the
/// first unit creation admits the placement cache.
pub fn live_pool(host: &crate::playercolor_host::ObjectHost) -> Option<&PoolOwner> {
    host.placement_spatial()
}

#[cfg(test)]
#[path = "spatial_pool_tests.rs"]
pub(crate) mod tests;
