//! Source-qualified static terrain queries; this is not unit placement policy.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{NeutralError, RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::scripts::AssetIdentity;

/// Caller-reviewed normalization operand and pathing-type switch in one image.
#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub normalization: SourceWindow,
    pub type_switch: SourceWindow,
}

#[derive(Debug, thiserror::Error)]
#[error("terrain pathing {source_context}: {problem}")]
pub struct Error {
    pub source_context: String,
    pub problem: Problem,
}

#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("input/selection budget exceeded")]
    Budget,
    #[error("missing identity or incorrect complete-image fingerprint")]
    Identity,
    #[error("invalid selected code window: {0}")]
    Image(#[source] NeutralError),
    #[error("unsupported selected normalization or pathing-type switch")]
    Shape,
}

#[derive(Debug)]
pub struct TerrainPathing {
    binding: Binding,
    cell_size: f32,
    masks: Vec<u8>,
}

impl TerrainPathing {
    /// Bind the registered static query and its selector in Legacy 1.29.2.9231.
    pub(crate) fn read_legacy(
        image: &[u8],
        origin: AssetIdentity,
        budget: RuleInputBudget,
    ) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: origin.source.clone(),
            problem,
        };
        if image.len() > budget.image_bytes {
            return Err(fail(Problem::Budget));
        }
        let image_sha256 = sha256(image);
        if image_sha256 != crate::menu_lighting::REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let [normalization, type_switch] =
            legacy_windows(&pe, budget.profile_bytes).map_err(fail)?;
        Self::decode(
            &pe,
            Binding {
                origin,
                image_sha256,
                normalization,
                type_switch,
            },
        )
    }

    /// Reads an x86 real-exponent normalization and bounded byte-result switch.
    /// Addresses and all masks must resolve inside the fingerprinted windows.
    pub fn read(image: &[u8], binding: Binding, budget: RuleInputBudget) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: binding.origin.source.clone(),
            problem,
        };
        if image.len() > budget.image_bytes
            || u64::from(binding.normalization.len) + u64::from(binding.type_switch.len)
                > budget.profile_bytes as u64
        {
            return Err(fail(Problem::Budget));
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
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        Self::decode(&pe, binding)
    }

    fn decode(pe: &Pe<'_>, binding: Binding) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: binding.origin.source.clone(),
            problem,
        };
        let scale = pe
            .window(&binding.normalization, true)
            .map_err(|e| fail(Problem::Image(e)))?;
        if scale.len() != 5 || scale[0] != 0x05 {
            return Err(fail(Problem::Shape));
        }
        let exponent = i32::from_le_bytes(scale[1..].try_into().unwrap());
        let magnitude = exponent.checked_neg().ok_or_else(|| fail(Problem::Shape))?;
        // Binary32 exponent adjustment, not a map's authored cell size.
        if magnitude <= 0 || magnitude % (1 << 23) != 0 {
            return Err(fail(Problem::Shape));
        }
        let cell_size = 1u32
            .checked_shl((magnitude / (1 << 23)) as u32)
            .ok_or_else(|| fail(Problem::Shape))? as f32;
        let code = pe
            .window(&binding.type_switch, true)
            .map_err(|e| fail(Problem::Image(e)))?;
        if code.len() < 18
            || code[..8] != [0x55, 0x8b, 0xec, 0x8b, 0x45, 8, 0x83, 0xf8]
            || code[9] != 0x77
            || code[8] > i8::MAX as u8
            || code[11..14] != [0xff, 0x24, 0x85]
        {
            return Err(fail(Problem::Shape));
        }
        let count = usize::from(code[8]) + 1;
        let fallback = 11i32 + i32::from(code[10] as i8);
        let fallback = usize::try_from(fallback).map_err(|_| fail(Problem::Shape))?;
        if code.get(fallback..fallback + 4) != Some(&[0x32, 0xc0, 0x5d, 0xc3]) {
            return Err(fail(Problem::Shape));
        }
        let table = u32::from_le_bytes(code[14..18].try_into().unwrap());
        let table = table
            .checked_sub(binding.type_switch.va)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| fail(Problem::Shape))?;
        let table = code
            .get(
                table
                    ..table
                        .checked_add(count * 4)
                        .ok_or_else(|| fail(Problem::Shape))?,
            )
            .ok_or_else(|| fail(Problem::Shape))?;
        let mut masks = Vec::new();
        masks
            .try_reserve_exact(count)
            .map_err(|_| fail(Problem::Budget))?;
        for entry in table.as_chunks::<4>().0 {
            let target = u32::from_le_bytes(*entry);
            let offset = target
                .checked_sub(binding.type_switch.va)
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| fail(Problem::Shape))?;
            let handler = code
                .get(offset..offset.checked_add(4).ok_or_else(|| fail(Problem::Shape))?)
                .ok_or_else(|| fail(Problem::Shape))?;
            let mask = match handler {
                [0xb0, value, 0x5d, 0xc3] => *value,
                [0x0c, 0xff, 0x5d, 0xc3] => u8::MAX,
                _ => return Err(fail(Problem::Shape)),
            };
            masks.push(mask);
        }
        Ok(Self {
            binding,
            cell_size,
            masks,
        })
    }

    pub fn binding(&self) -> &Binding {
        &self.binding
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    pub(crate) fn mask(&self, enumerand: i32) -> u8 {
        usize::try_from(enumerand)
            .ok()
            .and_then(|i| self.masks.get(i))
            .copied()
            .unwrap_or(0)
    }
}

fn legacy_windows(pe: &Pe<'_>, byte_budget: usize) -> Result<[SourceWindow; 2], Problem> {
    const SIGNATURE: &[u8] = b"(RRHpathingtype;)B\0";
    const NAME: &[u8] = b"IsTerrainPathable\0";
    const REGISTRATION: u32 = 0x4a0d73;
    const ENTRY: u32 = 0x4998a0;
    const WRAPPER: u32 = 0x7f6a00;
    const QUERY: u32 = 0x7f6890;
    const NORMALIZATION: u32 = 0x8002d1;
    const SELECTOR: u32 = 0x494460;
    if byte_budget < 20 + 36 + 36 + 199 + 5 + 88 + SIGNATURE.len() + NAME.len() {
        return Err(Problem::Budget);
    }
    let read = |va, len, code| {
        pe.read_from(va, code)
            .map_err(Problem::Image)?
            .get(..len)
            .ok_or(Problem::Shape)
    };
    let registration = read(REGISTRATION, 20, true)?;
    let pushed = |offset| match registration.get(offset..offset + 5) {
        Some([0x68, operand @ ..]) => Ok(u32::from_le_bytes(operand.try_into().unwrap())),
        _ => Err(Problem::Shape),
    };
    let calls = |code: &[u8], base, offset, target| match code.get(offset..offset + 5) {
        Some([0xe8, rel @ ..]) => {
            i64::from(base)
                + offset as i64
                + 5
                + i64::from(i32::from_le_bytes(rel.try_into().unwrap()))
                == i64::from(target)
        }
        _ => false,
    };
    let entry = read(ENTRY, 36, true)?;
    let wrapper = read(WRAPPER, 36, true)?;
    let query = read(QUERY, 199, true)?;
    let scale = read(NORMALIZATION, 5, true)?;
    let switch = read(SELECTOR, 88, true)?;
    if read(pushed(0)?, SIGNATURE.len(), false)? != SIGNATURE
        || read(pushed(5)?, NAME.len(), false)? != NAME
        || pushed(10)? != ENTRY
        || !calls(registration, REGISTRATION, 15, 0x8ccc80)
        || !calls(entry, ENTRY, 8, SELECTOR)
        || !calls(entry, ENTRY, 26, WRAPPER)
        || !calls(wrapper, WRAPPER, 26, QUERY)
        || query[0x7b..0x81] != [0x8d, 0x90, scale[1], scale[2], scale[3], scale[4]]
        || query[0xb1..0xb7] != [0x8d, 0x88, scale[1], scale[2], scale[3], scale[4]]
    {
        return Err(Problem::Shape);
    }
    Ok(
        [(NORMALIZATION, scale), (SELECTOR, switch)].map(|(va, bytes)| SourceWindow {
            va,
            len: bytes.len() as u32,
            digest: sha256(bytes),
        }),
    )
}

/// One authored cliff tile: tile-grid position and the maximum W3E elevation
/// layer (`byte6` low nibble) of its four corners.
///
/// Only tiles whose four corners disagree on the layer are retained. Tiles
/// with a uniform layer read that layer from the nearest corner, so they need
/// no overlay entry. Positions are tile indices, not world coordinates, so no
/// world scale is invented here; the caller converts its query to tile units
/// with the same admitted tile size it uses for corner indexing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliffRect {
    pub tile: [u32; 2],
    pub level: u8,
}

/// Authored-terrain population of the spatial-rect layer the selected
/// `0x7044b0` cliff lookup also samples (`docs/re-notes/ai-construction.md`).
///
/// The native combines the terrain corner value with the maximum value from
/// any matching separately stored spatial rectangle. The dynamic runtime
/// registrations behind that layer are unestablished in either original probe,
/// so this populates the authored part only: one rect per W3E tile whose
/// corners disagree on the elevation layer, carrying that tile's maximum.
/// Runtime region, texture or movement registrations carry no qualified cliff
/// contribution and are not consulted here; that mapping stays open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliffOverlay {
    rects: Vec<CliffRect>,
    width: u32,
    height: u32,
}

impl CliffOverlay {
    /// Derive the overlay from the admitted map terrain. Tile corner layers
    /// come from the asset bytes; nothing is measured or defaulted.
    pub fn of_terrain(terrain: &pcx_map::Terrain) -> Result<Self, &'static str> {
        let (width, height) = (terrain.width(), terrain.height());
        let tiles = u64::from(width) * u64::from(height);
        if tiles > usize::MAX as u64 {
            return Err("cliff overlay tile count overflow");
        }
        let mut rects = Vec::new();
        rects
            .try_reserve_exact(tiles as usize)
            .map_err(|_| "cliff overlay allocation")?;
        for ty in 0..height {
            for tx in 0..width {
                let corners = [
                    terrain.corner(tx, ty),
                    terrain.corner(tx + 1, ty),
                    terrain.corner(tx, ty + 1),
                    terrain.corner(tx + 1, ty + 1),
                ];
                let mut layers = [0u8; 4];
                for (slot, corner) in layers.iter_mut().zip(corners) {
                    *slot = corner
                        .ok_or("cliff overlay missing authored corner")?
                        .layer_height();
                }
                if layers.iter().all(|&layer| layer == layers[0]) {
                    continue;
                }
                let level = *layers.iter().max().expect("four corners");
                rects.push(CliffRect {
                    tile: [tx, ty],
                    level,
                });
            }
        }
        rects.shrink_to_fit();
        Ok(Self {
            rects,
            width,
            height,
        })
    }

    /// The tile-grid dimensions the overlay was derived from.
    pub fn dimensions(&self) -> [u32; 2] {
        [self.width, self.height]
    }

    /// Authored cliff rects, one per layer-varying tile, in row-major order.
    pub fn rects(&self) -> &[CliffRect] {
        &self.rects
    }

    /// Maximum authored layer over the tile containing a tile-grid position,
    /// or `None` when that tile's corners agree (the corner value applies).
    pub fn level_at_tile(&self, tile: [u32; 2]) -> Option<u8> {
        let [tx, ty] = tile;
        if tx >= self.width || ty >= self.height {
            return None;
        }
        // Row-major order matches construction; a binary search avoids an
        // unbounded scan while keeping the storage a plain rect list.
        self.rects
            .binary_search_by_key(&(ty, tx), |rect| (rect.tile[1], rect.tile[0]))
            .ok()
            .map(|index| self.rects[index].level)
    }
}

#[cfg(test)]
#[path = "terrain_pathing_tests.rs"]
pub(crate) mod tests;
