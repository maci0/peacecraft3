//! The minimap pair: `war3mapMap.blp` (the minimap image) and `war3map.mmp`
//! (the markers drawn on it).
//!
//! # The tracker names `war3map.map` and `war3map.ter` do not exist
//!
//! This is a measurement, not an opinion. The literal files `war3map.map` and
//! `war3map.ter` occur in **0 of the 133 shipped maps** (89 on 1.28c, 44 on
//! 1.29b), in **0 of 200 loose community maps** on disk across both builds,
//! in no `(listfile)`, and in neither build's name tables: `game.dll` strings
//! name `war3mapMap.blp` / `war3mapMap.tga`, and the Reforged PDBs (names
//! only) name `war3map.mmp`, `war3mapMap.blp`, `war3map.shd`, `war3map.doo`
//! — never `.map` or `.ter`.
//!
//! What the tracker describes by function ships under those two names:
//!
//! | tracker | measures as | format |
//! |---|---|---|
//! | `war3map.map` — minimap image | `war3mapMap.blp` (older maps: `.tga`) | BLP1 / TGA |
//! | `war3map.ter` — terrain minimap | `war3map.mmp` | marker table, below |
//!
//! A map that really did contain `war3map.map` or `war3map.ter` is refused
//! ([`crate::Error::UnprovenContent`]): no shipped byte proves any layout for
//! those names, and a plausible invented one is exactly the whole-world error
//! this project refuses.
//!
//! # `war3mapMap.blp` is BLP, handed to `pcx-blp`
//!
//! All 132 shipped minimap images are `BLP1`, JPEG content, `256×256`, one
//! mip. They are ordinary BLP files: [`pcx_blp::Header::parse`] accepts them
//! and [`pcx_blp::Framing`] proves both of that crate's relations —
//! `file_len == mip_offsets[0] + Σ mip_sizes` and the gap between the shared
//! JPEG header and mip 0 is zeros-then-`FFD9` — on 132 of 132. This crate
//! therefore decodes no image bytes of its own; it frames and defers.
//!
//! # `war3map.mmp` — the marker table
//!
//! ```text
//! 0x00  4  section version          (0 on all 132)
//! 0x04  4  n  marker count
//!       16n marker records
//! ```
//!
//! Marker record, 16 bytes:
//!
//! ```text
//!  0  4  kind         (observed 0, 1, 2 only)
//!  4  4  x, i32
//!  8  4  y, i32
//! 12  4  colour, u32  (0xAARRGGBB)
//! ```
//!
//! Framing: `8 + 16·n == file_len`, exact on all 132.
//!
//! Colours are exactly the game's twelve player-colour constants
//! (`0xFFFF0303` red … `0xFF4E2A04` brown) plus `0xFFFFFFFF` white — the
//! `03 03 ff ff` byte pattern is little-endian `0xFFFF0303`, which is how the
//! relation was proved. Kind 2 records are the ones carrying player colours;
//! kinds 0 and 1 carry white. The meanings of kind 0 and 1 are unattributed.
//!
//! Coordinates are `i32` in a 256-space (pixels of the `256×256` minimap
//! image), with measured outliers to −13 and 269 where a marker hangs over
//! the edge. This crate hands back the raw `i32` and does not rescale.

use crate::Error;

/// `war3map.mmp`, the marker table. See the module docs for the naming fact.
pub const MINIMAP_MARKERS_NAME: &str = "war3map.mmp";

/// The minimap image, preferred name.
pub const MINIMAP_IMAGE_NAME: &str = "war3mapMap.blp";

/// The minimap image, the older TGA spelling. Not present on any shipped map;
/// supported because `game.dll` names it.
pub const MINIMAP_IMAGE_TGA_NAME: &str = "war3mapMap.tga";

/// The tracker's names for the minimap pair. Measured absent everywhere; a
/// map carrying them is refused rather than parsed.
pub const UNPROVEN_MINIMAP_NAMES: [&str; 2] = ["war3map.map", "war3map.ter"];

/// Byte length of one marker record.
pub const MARKER_RECORD: usize = 16;

/// One minimap marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Marker {
    /// Marker kind. Observed 0, 1, 2; meanings of 0 and 1 unattributed.
    pub kind: u32,
    /// Minimap-space x. Observed −13..=269 on a 256-wide image.
    pub x: i32,
    /// Minimap-space y.
    pub y: i32,
    /// `0xAARRGGBB`; shipped values are player-colour constants or white.
    pub colour: u32,
}

/// A decoded `war3map.mmp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimapMarkers {
    version: u32,
    markers: Vec<Marker>,
    raw: Vec<u8>,
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, Error> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or(Error::MinimapTruncated {
            have: data.len(),
            need: at + 4,
        })
}

impl MinimapMarkers {
    /// Parse marker bytes and prove `8 + 16·n == file_len`.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        let version = u32_at(&bytes, 0)?;
        let n = u32_at(&bytes, 4)? as usize;
        let need = 8 + n * MARKER_RECORD;
        if need != bytes.len() {
            return Err(Error::MinimapSize {
                markers: n,
                have: bytes.len(),
                need,
            });
        }
        let mut markers = Vec::with_capacity(n);
        for i in 0..n {
            let at = 8 + i * MARKER_RECORD;
            let r = &bytes[at..at + MARKER_RECORD];
            markers.push(Marker {
                kind: u32::from_le_bytes(r[0..4].try_into().unwrap()),
                x: i32::from_le_bytes(r[4..8].try_into().unwrap()),
                y: i32::from_le_bytes(r[8..12].try_into().unwrap()),
                colour: u32::from_le_bytes(r[12..16].try_into().unwrap()),
            });
        }
        Ok(Self {
            version,
            markers,
            raw: bytes,
        })
    }

    /// The file exactly as stored.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Section version; 0 on all 132 shipped files.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Every marker, in file order.
    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }

    /// Every distinct kind value present.
    pub fn kinds_used(&self) -> std::collections::BTreeSet<u32> {
        self.markers.iter().map(|m| m.kind).collect()
    }

    /// Every distinct colour value present.
    pub fn colours_used(&self) -> std::collections::BTreeSet<u32> {
        self.markers.iter().map(|m| m.colour).collect()
    }

    /// Smallest and largest x and y over all markers.
    pub fn coordinate_range(&self) -> Option<(i32, i32, i32, i32)> {
        let mut it = self.markers.iter();
        let first = it.next()?;
        let mut r = (first.x, first.x, first.y, first.y);
        for m in it {
            r.0 = r.0.min(m.x);
            r.1 = r.1.max(m.x);
            r.2 = r.2.min(m.y);
            r.3 = r.3.max(m.y);
        }
        Some(r)
    }

    /// Canonical byte form: the file re-serialised field by field, which on
    /// every shipped file is the stored bytes exactly.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.raw.len());
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&(self.markers.len() as u32).to_le_bytes());
        for m in &self.markers {
            out.extend_from_slice(&m.kind.to_le_bytes());
            out.extend_from_slice(&m.x.to_le_bytes());
            out.extend_from_slice(&m.y.to_le_bytes());
            out.extend_from_slice(&m.colour.to_le_bytes());
        }
        out
    }

    /// FNV-1a 64 over [`Self::canonical_bytes`].
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.canonical_bytes())
    }
}

/// Which container the minimap image arrived in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinimapImageKind {
    /// `war3mapMap.blp`: parsed and framed by [`pcx_blp`].
    Blp {
        header: pcx_blp::Header,
        framing: pcx_blp::Framing,
    },
    /// `war3mapMap.tga`: parsed by [`pcx_tga`]. No shipped map uses it.
    Tga { header: pcx_tga::Header },
}

/// The minimap image, `war3mapMap.blp` or `war3mapMap.tga`.
///
/// Image bytes are never interpreted here: BLPs go through `pcx-blp`, TGAs
/// through `pcx-tga`. What this crate proves is *which file the map means*
/// and that the container frames.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapImage {
    /// The archive member name it was found under.
    pub name: &'static str,
    kind: MinimapImageKind,
    raw: Vec<u8>,
}

impl MinimapImage {
    /// Parse minimap-image bytes that came from `name`.
    pub fn from_named_bytes(name: &'static str, bytes: Vec<u8>) -> Result<Self, Error> {
        let kind = if bytes.get(..4) == Some(&b"BLP1"[..])
            || bytes.get(..4) == Some(&b"BLP2"[..])
            || bytes.get(..4) == Some(&b"BLP0"[..])
        {
            let header = pcx_blp::Header::parse(&bytes)?;
            let framing = pcx_blp::Framing::of(&bytes, &header);
            MinimapImageKind::Blp { header, framing }
        } else if bytes.len() >= 18 {
            let header = pcx_tga::Header::parse(&bytes)?;
            MinimapImageKind::Tga { header }
        } else {
            return Err(Error::MinimapTruncated {
                have: bytes.len(),
                need: 18,
            });
        };
        Ok(Self {
            name,
            kind,
            raw: bytes,
        })
    }

    /// The file exactly as stored.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// The parsed container, for callers that want `pcx-blp`/`pcx-tga` views.
    pub fn kind(&self) -> &MinimapImageKind {
        &self.kind
    }

    /// `true` when the image is a BLP `pcx-blp` accepted.
    pub fn is_blp(&self) -> bool {
        matches!(self.kind, MinimapImageKind::Blp { .. })
    }

    /// The image's pixel dimensions, whichever container it is.
    pub fn dimensions(&self) -> (u32, u32) {
        match &self.kind {
            MinimapImageKind::Blp { header, .. } => (header.width, header.height),
            MinimapImageKind::Tga { header } => (header.width as u32, header.height as u32),
        }
    }

    /// `pcx-blp`'s framing relation, for BLP images: `file_len ==
    /// mip_offsets[0] + Σ mip_sizes[0..n)` and, for JPEG content, the gap
    /// between the shared JPEG header and mip 0 is zeros-then-`FFD9`.
    ///
    /// `true` for TGA images, which have no such table (nothing to fail).
    pub fn framing_holds(&self) -> bool {
        match &self.kind {
            MinimapImageKind::Blp { header, framing } => {
                framing.sums() && (header.content != 0 || framing.jpeg_gap_is_padding(&self.raw))
            }
            MinimapImageKind::Tga { .. } => true,
        }
    }

    /// FNV-1a 64 over the file bytes. The image *is* the canonical form at
    /// this layer: pixel decoding belongs to `pcx-blp`/`pcx-tga`, and their
    /// own decoders are compared through their own tests.
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.raw)
    }
}
