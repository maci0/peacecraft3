//! `war3map.doo` — placed doodad instances and the terrain (palette) doodads.
//!
//! # Layout
//!
//! Measured from the bytes of every shipped map on both builds (132 files,
//! all `version == 7`), and refused outside that measurement:
//!
//! ```text
//! 0x00  4   "W3do"
//! 0x04  4   version                       (= 7 everywhere it ships)
//! 0x08  4   subversion                    (9 on all 132; 11 seen once, same layout)
//! 0x0c  4   n   instance count
//!       42n instance records
//!  +0   4   terrain-doodad section version (0 on all 132)
//!  +4   4   m   terrain-doodad count
//!       16m terrain-doodad records
//! ```
//!
//! Instance record, 42 bytes — **fixed width in version 7**:
//!
//! ```text
//!  0  4   doodad type id (fourcc, e.g. "LOsm")
//!  4  4   variation
//!  8  12  position, three f32 (x, y, z)
//! 20  4   rotation angle, one f32 (radians)
//! 24  12  scale, three f32 (x, y, z)
//! 36  1   flags        (observed 0x00/0x02/0x03/0x04/0x06/0x07)
//! 37  1   life         (observed 0..=255; the world-unit meaning is unattributed)
//! 38  4   creation number (the World Editor's per-instance id)
//! ```
//!
//! Version 7 carries **no item-drop data**. Version 8 (accepted, no shipped file)
//! adds a signed `table_id` and per-set pairs ahead of the creation number. The
//! offsets are not proven against shipped bytes: all 132 shipped files are version 7.
//!
//! Terrain-doodad record, 16 bytes (cliff/terrain-palette placements):
//!
//! ```text
//!  0  4   terrain doodad type id
//!  4  4   variation           (0 on all 132)
//!  8  4   x, in whole grid cells
//! 12  4   y, in whole grid cells
//! ```
//!
//! # Framing relation
//!
//! ```text
//! 16 + 42·n + 8 + 16·m == file_len
//! ```
//!
//! holds exactly on all 132 shipped files — header, every instance at its
//! fixed width, the two-word terrain section head, every terrain doodad. A
//! file that fails it is refused rather than guessed at.
//!
//! Creation numbers are unique within a file but are **not** dense and **not**
//! bounded by `n`: campaign maps reach ids well past the instance count
//! (e.g. `Undead03`: 3 941 instances, ids up to 4 146). They are editor
//! identities, not indices.

use std::collections::BTreeSet;

use crate::Error;

/// File magic.
pub const DOO_MAGIC: [u8; 4] = *b"W3do";

/// The only version either shipped build writes. Version 8 is also accepted; its
/// item-drop layout is unproven against shipped bytes (see the module docs).
pub const DOO_VERSION: u32 = 7;

/// Byte length of one version-7 instance record.
pub const DOO_RECORD: usize = 42;

/// Byte length of one terrain-doodad record.
pub const DOO_TERRAIN_RECORD: usize = 16;

/// Magic plus version plus subversion plus count.
pub const DOO_HEADER: usize = 16;

/// The two-word terrain-section head (section version + count).
pub const DOO_TERRAIN_HEAD: usize = 8;

/// One placed doodad instance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Doodad {
    /// The fourcc doodad type id, e.g. `*b"LOsm"`.
    pub type_id: [u8; 4],
    /// Model variation index. Observed 0..=9.
    pub variation: u32,
    /// World position, relative to the terrain file's origin floats.
    pub position: [f32; 3],
    /// Rotation about the doodad's up axis, radians. Shipped files use one
    /// angle, not three.
    pub angle: f32,
    /// Per-axis model scale. Observed equal on trees, unequal elsewhere.
    pub scale: [f32; 3],
    /// Behaviour flags, raw. Observed 0x00/0x02/0x03/0x04/0x06/0x07.
    pub flags: u8,
    /// Life, raw byte. Observed 0..=255; the world-unit meaning is
    /// unattributed and this crate does not convert it.
    pub life: u8,
    /// The World Editor's unique per-instance id. Unique within a file, not
    /// dense, not bounded by the instance count.
    pub creation_number: u32,
}

/// One terrain (cliff/palette) doodad placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainDoodad {
    /// The fourcc terrain doodad type id, e.g. `*b"LCc0"`.
    pub type_id: [u8; 4],
    /// Variation. Zero on all 132 shipped files.
    pub variation: u32,
    /// Grid cell x, from the map's lower-left corner.
    pub grid_x: u32,
    /// Grid cell y.
    pub grid_y: u32,
}

/// Version-8 authored item-table selector and drop sets. Each pair is two stored words. The `[rawcode, chance]` order from `units.rs`
/// is assumed here, not proven for version 8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDropData {
    pub table_id: i32,
    pub sets: Vec<Vec<[u32; 2]>>,
}

/// A decoded `war3map.doo`.
#[derive(Debug, Clone, PartialEq)]
pub struct Doodads {
    version: u32,
    subversion: u32,
    terrain_section_version: u32,
    instances: Vec<Doodad>,
    item_drops: Vec<Option<ItemDropData>>,
    terrain: Vec<TerrainDoodad>,
    raw: Vec<u8>,
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, Error> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or(Error::DoodadTruncated {
            have: data.len(),
            need: at + 4,
        })
}

impl Doodads {
    /// Parse doodad bytes, keeping them for [`Self::raw`].
    ///
    /// Proves the framing relation before returning anything: the declared
    /// instance count at 42 bytes each, the two-word terrain head, and the
    /// declared terrain count at 16 bytes each must consume the file exactly.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        let magic: [u8; 4] = bytes
            .get(..4)
            .ok_or(Error::DoodadTruncated {
                have: bytes.len(),
                need: 4,
            })?
            .try_into()
            .unwrap();
        if magic != DOO_MAGIC {
            return Err(Error::BadDoodadMagic(magic));
        }
        let version = u32_at(&bytes, 4)?;
        if version != DOO_VERSION && version != 8 {
            return Err(Error::UnsupportedDoodadVersion(version));
        }
        let subversion = u32_at(&bytes, 8)?;
        let n = u32_at(&bytes, 12)? as usize;

        let mut cursor = DOO_HEADER;
        let mut instances = Vec::new();
        let mut item_drops = Vec::new();
        for _ in 0..n {
            let r = bytes
                .get(cursor..cursor + 38)
                .ok_or(Error::DoodadTruncated {
                    have: bytes.len(),
                    need: cursor + 38,
                })?;
            cursor += 38;
            let drops = if version == 8 {
                let table_id = u32_at(&bytes, cursor)? as i32;
                cursor += 4;
                let count = u32_at(&bytes, cursor)?;
                cursor += 4;
                let mut sets = Vec::new();
                for _ in 0..count {
                    let item_count = u32_at(&bytes, cursor)?;
                    cursor += 4;
                    let mut items = Vec::new();
                    for _ in 0..item_count {
                        items.push([u32_at(&bytes, cursor)?, u32_at(&bytes, cursor + 4)?]);
                        cursor += 8;
                    }
                    sets.push(items);
                }
                Some(ItemDropData { table_id, sets })
            } else {
                None
            };
            let creation_number = u32_at(&bytes, cursor)?;
            cursor += 4;
            instances.push(Doodad {
                type_id: r[0..4].try_into().unwrap(),
                variation: u32::from_le_bytes(r[4..8].try_into().unwrap()),
                position: [
                    f32::from_le_bytes(r[8..12].try_into().unwrap()),
                    f32::from_le_bytes(r[12..16].try_into().unwrap()),
                    f32::from_le_bytes(r[16..20].try_into().unwrap()),
                ],
                angle: f32::from_le_bytes(r[20..24].try_into().unwrap()),
                scale: [
                    f32::from_le_bytes(r[24..28].try_into().unwrap()),
                    f32::from_le_bytes(r[28..32].try_into().unwrap()),
                    f32::from_le_bytes(r[32..36].try_into().unwrap()),
                ],
                flags: r[36],
                life: r[37],
                creation_number,
            });
            item_drops.push(drops);
        }
        let terrain_section_version = u32_at(&bytes, cursor)?;
        cursor += 4;
        let m = u32_at(&bytes, cursor)? as usize;
        cursor += 4;
        let terrain_head = cursor;
        let need = m
            .checked_mul(DOO_TERRAIN_RECORD)
            .and_then(|size| cursor.checked_add(size))
            .ok_or(Error::DoodadSize {
                instances: n,
                terrain_doodads: m,
                have: bytes.len(),
                need: usize::MAX,
            })?;
        if need != bytes.len() {
            return Err(Error::DoodadSize {
                instances: n,
                terrain_doodads: m,
                have: bytes.len(),
                need,
            });
        }

        let mut terrain = Vec::with_capacity(m);
        for i in 0..m {
            let at = terrain_head + i * DOO_TERRAIN_RECORD;
            let r = &bytes[at..at + DOO_TERRAIN_RECORD];
            terrain.push(TerrainDoodad {
                type_id: r[0..4].try_into().unwrap(),
                variation: u32::from_le_bytes(r[4..8].try_into().unwrap()),
                grid_x: u32::from_le_bytes(r[8..12].try_into().unwrap()),
                grid_y: u32::from_le_bytes(r[12..16].try_into().unwrap()),
            });
        }

        Ok(Self {
            version,
            subversion,
            terrain_section_version,
            instances,
            item_drops,
            terrain,
            raw: bytes,
        })
    }

    /// The file exactly as stored.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// File version; 7 on every shipped file.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// The word after the version. 9 on all 132 shipped files; 11 appears in
    /// the wild with the same record layout. Kept raw: no shipped byte proves
    /// it selects a layout.
    pub fn subversion(&self) -> u32 {
        self.subversion
    }

    /// The terrain section's own version word. 0 on all 132 shipped files.
    pub fn terrain_section_version(&self) -> u32 {
        self.terrain_section_version
    }

    /// Every placed instance, in file order.
    pub fn instances(&self) -> &[Doodad] {
        &self.instances
    }

    /// Version-8 item-table data, aligned with the authored instance index.
    /// Version 7 has no item-table fields and returns None.
    pub fn item_drops(&self, instance: usize) -> Option<&ItemDropData> {
        self.item_drops.get(instance)?.as_ref()
    }

    /// Every terrain (palette) doodad, in file order.
    pub fn terrain_doodads(&self) -> &[TerrainDoodad] {
        &self.terrain
    }

    /// The instance with the given creation number, if any.
    pub fn by_creation_number(&self, id: u32) -> Option<&Doodad> {
        self.instances.iter().find(|d| d.creation_number == id)
    }

    /// Smallest and largest creation number present.
    pub fn creation_number_range(&self) -> Option<(u32, u32)> {
        let mut it = self.instances.iter().map(|d| d.creation_number);
        let first = it.next()?;
        let mut lo = first;
        let mut hi = first;
        for id in it {
            lo = lo.min(id);
            hi = hi.max(id);
        }
        Some((lo, hi))
    }

    /// `true` when every creation number is distinct.
    ///
    /// Measured true on all 132 shipped files. The parser does not require it:
    /// the bytes say "unique", not "a permutation of 0..n", and ids past `n`
    /// are normal.
    pub fn creation_numbers_are_unique(&self) -> bool {
        let set: BTreeSet<u32> = self.instances.iter().map(|d| d.creation_number).collect();
        set.len() == self.instances.len()
    }

    /// Canonical byte form: the file re-serialised field by field, which on
    /// every shipped file is the stored bytes exactly. Hashing this lets two
    /// decoders be compared without either inventing a field order.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.raw.len());
        out.extend_from_slice(&DOO_MAGIC);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&self.subversion.to_le_bytes());
        out.extend_from_slice(&(self.instances.len() as u32).to_le_bytes());
        for (d, drops) in self.instances.iter().zip(&self.item_drops) {
            out.extend_from_slice(&d.type_id);
            out.extend_from_slice(&d.variation.to_le_bytes());
            for v in d.position {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&d.angle.to_le_bytes());
            for v in d.scale {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.push(d.flags);
            out.push(d.life);
            if let Some(drops) = drops {
                out.extend(drops.table_id.to_le_bytes());
                out.extend((drops.sets.len() as u32).to_le_bytes());
                for set in &drops.sets {
                    out.extend((set.len() as u32).to_le_bytes());
                    for pair in set {
                        for word in pair {
                            out.extend(word.to_le_bytes());
                        }
                    }
                }
            }
            out.extend_from_slice(&d.creation_number.to_le_bytes());
        }
        out.extend_from_slice(&self.terrain_section_version.to_le_bytes());
        out.extend_from_slice(&(self.terrain.len() as u32).to_le_bytes());
        for t in &self.terrain {
            out.extend_from_slice(&t.type_id);
            out.extend_from_slice(&t.variation.to_le_bytes());
            out.extend_from_slice(&t.grid_x.to_le_bytes());
            out.extend_from_slice(&t.grid_y.to_le_bytes());
        }
        out
    }

    /// FNV-1a 64 over [`Self::canonical_bytes`].
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.canonical_bytes())
    }
}

#[cfg(test)]
mod version8_regressions {
    use super::*;
    #[test]
    fn item_sets_do_not_displace_the_creation_number_or_terrain_section() {
        let mut data = DOO_MAGIC.to_vec();
        for word in [8u32, 11, 1] {
            data.extend(word.to_le_bytes());
        }
        let mut core = vec![0; 38];
        core[..4].copy_from_slice(b"TEST");
        core[36] = 3;
        core[37] = 128;
        data.extend(core);
        data.extend((-1i32).to_le_bytes());
        for word in [1u32, 2, 25, 0x41424344, 75, 0x45464748, 123, 0, 0] {
            data.extend(word.to_le_bytes());
        }
        let parsed = Doodads::from_bytes(data.clone()).unwrap();
        assert_eq!(parsed.instances()[0].creation_number, 123);
        assert_eq!(parsed.instances()[0].life, 128);
        assert_eq!(parsed.item_drops(0).unwrap().table_id, -1);
        assert_eq!(
            parsed.item_drops(0).unwrap().sets,
            [vec![[25, 0x41424344], [75, 0x45464748]]]
        );
        assert_eq!(parsed.canonical_bytes(), data);
        for length in 0..data.len() {
            assert!(Doodads::from_bytes(data[..length].to_vec()).is_err());
        }
    }
}
