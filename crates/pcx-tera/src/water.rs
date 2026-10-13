//! Water investigation boundary: raw W3E records with provenance, and explicit
//! refusals for their unproven interpretation. See `../WATER_EVIDENCE.md`.
//!
//! `TerrainArt\Water.slk` contains water settings, including a `height` column.
//! That proves authored settings exist; it does not establish how a W3E corner
//! selects them, whether it contains water, or its water height in world units.
//! `UI\MiscData.txt` describes depth-based vertex colours and supplies depth
//! settings, but likewise does not decode the W3E words into a water plane.
//! The second word and flag bits remain uninterpreted. No water plane, colour,
//! texture selection, or animation policy is inferred from them here.

use pcx_map::{CORNER_RECORD, Corner, Terrain};

use crate::Unsupported;

/// Caller-declared original input identity, alongside its parsed format version.
/// Archive adjacency alone does not establish an edition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaterProvenance<'a> {
    pub edition: &'a str,
    /// Map path/member containing `war3map.w3e`.
    pub map_source: &'a str,
    pub terrain_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("water input requires explicit {0} provenance")]
pub struct MissingWaterProvenance(pub &'static str);

/// One original seven-byte record, without losing opaque or reserved bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawWaterCorner<'a> {
    /// Offset within the original `war3map.w3e` payload.
    pub byte_offset: usize,
    pub record: &'a [u8; CORNER_RECORD],
    /// The existing map parser's interpretation; not a water classification.
    pub corner: &'a Corner,
    pub provenance: WaterProvenance<'a>,
}

/// A provenance-bearing view of the inputs needed to investigate water.
#[derive(Debug, Clone, Copy)]
pub struct WaterData<'a> {
    terrain: &'a Terrain,
    provenance: WaterProvenance<'a>,
}

impl<'a> WaterData<'a> {
    pub fn new(
        terrain: &'a Terrain,
        edition: &'a str,
        map_source: &'a str,
    ) -> Result<Self, MissingWaterProvenance> {
        for (name, value) in [("edition", edition), ("map source", map_source)] {
            if value.trim().is_empty() {
                return Err(MissingWaterProvenance(name));
            }
        }
        Ok(Self {
            terrain,
            provenance: WaterProvenance {
                edition,
                map_source,
                terrain_version: terrain.version(),
            },
        })
    }

    #[must_use]
    pub fn provenance(&self) -> WaterProvenance<'a> {
        self.provenance
    }

    /// Original record for an in-range corner. Framing comes from the map
    /// parser's exact consumption of the seven-byte row-major corner table.
    /// This includes the second word at +2, slot/flags at +4, and bytes +5/+6.
    #[must_use]
    pub fn raw_corner(&self, cx: u32, cy: u32) -> Option<RawWaterCorner<'a>> {
        let corner = self.terrain.corner(cx, cy)?;
        let stride = (self.terrain.width() as usize).checked_add(1)?;
        let ordinal = (cy as usize)
            .checked_mul(stride)?
            .checked_add(cx as usize)?;
        let body_len = self.terrain.corner_count().checked_mul(CORNER_RECORD)?;
        let body_offset = self.terrain.raw().len().checked_sub(body_len)?;
        let byte_offset = body_offset.checked_add(ordinal.checked_mul(CORNER_RECORD)?)?;
        let end = byte_offset.checked_add(CORNER_RECORD)?;
        let record = self.terrain.raw().get(byte_offset..end)?.try_into().ok()?;
        Some(RawWaterCorner {
            byte_offset,
            record,
            corner,
            provenance: self.provenance,
        })
    }

    /// An in-range corner's water level is unproven, never synthesized from
    /// ground height or the table's `height` column. `Ok(None)` means outside
    /// the corner grid, not dry ground.
    pub fn water_level(&self, cx: u32, cy: u32) -> Result<Option<f32>, Unsupported> {
        if self.terrain.corner(cx, cy).is_none() {
            return Ok(None);
        }
        Err(Unsupported::WaterLevel)
    }

    /// Refuse surface generation until water presence, height conversion and
    /// topology are established. A dry/empty mesh would also be an assumption.
    pub fn mesh(&self) -> Result<WaterMesh, Unsupported> {
        Err(Unsupported::WaterGeometry)
    }
}

/// No measured water geometry is constructible yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterMesh {}
