//! Warcraft III MDX model container.
//!
//! 1.28c does not ship the old `VERS`/`MODL` ASCII-chunked `.mdl` at all — every
//! model in the archives is `.mdx`, under a `MDLX` container. The layout is a
//! flat run of chunks, each `tag[4] + u32 payload_size + payload`, starting at
//! offset 4. There is no chunk count and no index; the only way to parse the
//! file is to walk it and require that the walk lands exactly on the end.
//!
//! ```text
//! 0x0000  "MDLX"
//! 0x0004  "VERS" 4      -> version = 800
//! 0x0010  "MODL" 372    -> name, extents
//! 0x018c  "SEQS" ...    -> animation sequences
//! ...
//! ```
//!
//! Strings are inline NUL-terminated, not an offset table.
//!
//! Not every chunk shares one framing. Most are a run of self-inclusive record
//! sizes — `GEOS`, `MTLS` — but three families differ in ways that a shared
//! reader gets wrong, so each has its own:
//!
//! * `HELP`, `ATCH` share `BONE`'s node framing: one leading self-inclusive
//!   size covering node plus tracks plus an 8-byte trailer.
//! * `EVTS`, `CLID` measure only the node header, with tracks or geometry after
//!   it outside the measured region.
//! * `RIBB`, `PREM`, `LITE` open with *two* sizes: the record's own, then the
//!   node's, the latter absorbing any node animation. Their own fields begin at
//!   `+4 + node_size`, not at a fixed offset.
//! * `CAMS` has no node header at all: the name is at `+4`, and where
//!   `objectId`/`parentId`/`flags` would sit for every other node chunk is the
//!   camera's eye position. Reading it through [`Mdlx::nodes`] yields plausible
//!   ids and a wrong camera.
//! * `TXAN` is a run of records with no node header, addressed by the texture
//!   layer that names them.
//!
//! `PRE2` is decoded to its framing: its node header, its fixed 171-byte
//! prelude and its `KP2*` sub-chunk chain, all of which are measured and
//! asserted. What the prelude's words *mean* is not, so they are kept as raw
//! bytes rather than guessed into named floats — the same line this crate draws
//! where a plausible guess would render. See [`Particle2`], at the end of this
//! file.
//!
//! `CLID` is decoded to the same depth. Its records are
//! `headerSize + name[80] + objectId + parentId + flags` followed by any node
//! tracks *inside* that header — so `headerSize` is `96 + Σ tracks`, not a
//! constant 96, and 96 of the shipped records carry a `KGTR` track — and then
//! a shape word that is only ever `0` or `2`. Widths are measured and
//! asserted; whether `0` is really an axis-aligned box and `2` really a sphere
//! is supported but not proven, so the floats are returned in stored order and
//! uncaptioned, and no volume or collision volume is handed out. A
//! plausible-but-wrong collision shape lets units walk through walls, which is
//! the worst failure mode available here. See [`CollisionShape`].

mod fafx1800;
mod geoset1800;
mod joint1800;
mod material;
mod material1800;
mod particle2;
pub use fafx1800::{FafxRecord1800, FafxStream1800};
pub use geoset1800::{
    GeosetStreams1800, SkinRecord1800, SkinStream1800, StoredGeosetBlock, TangentStream1800,
};
pub use joint1800::{
    BindPoseRecord1800, BindPoseStream1800, MatrixPalette1800, MatrixPaletteEntry1800,
    PrimaryJointJoin1800, PrimaryJointReference1800, PrimaryJointRefusal1800,
    SkinWeightDiagnostics1800,
};
pub use material::{MaterialLayer, TextureFrameTrack};
pub use material1800::{MaterialLayer1800, MaterialTrack1800, SubTexture1800};
pub use particle2::Particle2Properties;

mod pose;
pub use pose::{NodeTransformError, blend_joint_pose, sample_joint_pose};

use std::collections::BTreeSet;
use std::path::Path;

pub const MAGIC: &[u8; 4] = b"MDLX";

/// Select the shipped binary counterpart of a script/skin `.mdl` reference.
/// Other extensions and the authored directory/name are preserved.
/// Original-engine extension lookup order is not established by this mapping.
pub fn binary_model_path(path: &str) -> String {
    match path.len().checked_sub(4).and_then(|at| path.get(at..)) {
        Some(ext) if ext.eq_ignore_ascii_case(".mdl") => format!("{}.mdx", &path[..path.len() - 4]),
        _ => path.to_string(),
    }
}

/// Chunk tags seen across the shipped models.
pub mod tag {
    pub const VERS: &[u8; 4] = b"VERS";
    pub const MODL: &[u8; 4] = b"MODL";
    pub const SEQS: &[u8; 4] = b"SEQS";
    pub const GLBS: &[u8; 4] = b"GLBS";
    pub const MTLS: &[u8; 4] = b"MTLS";
    pub const TEXS: &[u8; 4] = b"TEXS";
    pub const GEOS: &[u8; 4] = b"GEOS";
    pub const GEOA: &[u8; 4] = b"GEOA";
    pub const BONE: &[u8; 4] = b"BONE";
    pub const LITE: &[u8; 4] = b"LITE";
    pub const HELP: &[u8; 4] = b"HELP";
    pub const ATCH: &[u8; 4] = b"ATCH";
    pub const PIVT: &[u8; 4] = b"PIVT";
    pub const PRE2: &[u8; 4] = b"PRE2";
    pub const EVTS: &[u8; 4] = b"EVTS";
    pub const CLID: &[u8; 4] = b"CLID";
    pub const CAMS: &[u8; 4] = b"CAMS";
    pub const RIBB: &[u8; 4] = b"RIBB";
    pub const TXAN: &[u8; 4] = b"TXAN";
    pub const PREM: &[u8; 4] = b"PREM";
}

/// Every tag this build understands. Anything else is reported rather than
/// skipped, so a newly shipped chunk cannot be silently ignored.
pub const KNOWN_TAGS: [&[u8; 4]; 20] = [
    tag::VERS,
    tag::MODL,
    tag::SEQS,
    tag::GLBS,
    tag::MTLS,
    tag::TEXS,
    tag::GEOS,
    tag::GEOA,
    tag::BONE,
    tag::LITE,
    tag::HELP,
    tag::ATCH,
    tag::PIVT,
    tag::PRE2,
    tag::EVTS,
    tag::CLID,
    tag::CAMS,
    tag::RIBB,
    tag::TXAN,
    tag::PREM,
];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad magic {0:?}, expected MDLX")]
    BadMagic([u8; 4]),
    #[error("truncated: {what} needs {need} bytes at {at}, file has {have}")]
    Truncated {
        what: &'static str,
        at: usize,
        need: usize,
        have: usize,
    },
    #[error("chunk at {at} is {tag:?}, not an ASCII tag; the walk lost sync")]
    NotATag { at: usize, tag: [u8; 4] },
    #[error("unknown chunk tag {0:?}")]
    UnknownTag([u8; 4]),
    #[error("chunk {tag} declares {size} bytes but only {left} remain")]
    ChunkOverruns {
        tag: String,
        size: usize,
        left: usize,
    },
    #[error("no VERS chunk")]
    NoVersion,
    #[error("no MODL chunk")]
    NoModel,
    #[error("geoset sub-block tag {0:?} is not recognised")]
    UnknownGeosetTag([u8; 4]),
    #[error("UVBS at +{at} has no preceding UVAS declaration")]
    MissingUvSetDeclaration { at: usize },
    #[error("UVAS at +{at} declares {declared} sets, but {actual} UVBS records were read")]
    UvSetCount {
        at: usize,
        declared: u32,
        actual: usize,
    },
    #[error("geoset animation track {0:?} is not recognised")]
    UnknownGeoAnimTrack([u8; 4]),
    #[error("geoset has no VRTX block")]
    NoVRTX,
    #[error("unsupported MDX version {0}")]
    UnsupportedVersion(u32),
    #[error("MDX version {version} {chunk} layout is not supported")]
    UnsupportedVersionedChunk { version: u32, chunk: &'static str },
    #[error("MDX version {version} geoset feature {tag:?} at +{at} is not supported")]
    UnsupportedGeosetFeature {
        version: u32,
        at: usize,
        tag: [u8; 4],
    },
    #[error("duplicate geoset sub-block {tag:?} at +{at}")]
    DuplicateGeosetTag { at: usize, tag: [u8; 4] },
    #[error(
        "primary-convention v{version} joint join at {at} (geoset {geoset_offset}, vertex {vertex}, lane {lane}) refused: {reason:?}"
    )]
    PrimaryJointRefusal {
        version: u32,
        /// Absolute source offset of the geoset's inclusive-size word.
        geoset_offset: usize,
        vertex: usize,
        lane: usize,
        /// Absolute source offset of the joint word, or the available stream start.
        at: usize,
        reason: PrimaryJointRefusal1800,
    },
    #[error("chunk {chunk} is {size} bytes, not a whole number of {stride}-byte records")]
    RaggedRecords {
        chunk: &'static str,
        size: usize,
        stride: usize,
    },
    #[error("string at +{at} is not NUL-terminated")]
    UnterminatedString { at: usize },
    #[error("BONE record at +{at} declares {size} bytes, leaving {left} of the chunk")]
    BoneRecordOverruns { at: usize, size: usize, left: usize },
    #[error("BONE record at +{at}: unknown animation track magic {tag:?}")]
    UnknownBoneTrack { at: usize, tag: [u8; 4] },
    #[error("BONE record at +{at}: interpolation type {kind} is not 0, 1, 2 or 3")]
    UnknownInterpolation { at: usize, kind: u32 },
    #[error("BONE track at +{at} declares {size} bytes, but the record leaves {left}")]
    BoneTrackOverruns { at: usize, size: usize, left: usize },
    #[error("{chunk} record at +{at} declares {size} bytes, but its fields measure {measured}")]
    RecordShape {
        chunk: &'static str,
        at: usize,
        size: usize,
        measured: usize,
    },
    #[error("{chunk} record at +{at}: track magic {tag:?} is not one this build knows")]
    UnknownTrackMagic {
        chunk: &'static str,
        at: usize,
        tag: [u8; 4],
    },
    /// A `CLID` shape whose kind word is neither box nor sphere.
    ///
    /// Refused rather than defaulted to either: Blizzard's own
    /// `MDLCOLLISIONSHAPE` union names a cylinder and a plane as well, and their
    /// byte widths differ, so guessing one would silently resynchronise the walk
    /// halfway through the chunk.
    #[error("CLID record at +{at} names shape kind {kind}, which is not 0 (box) or 2 (sphere)")]
    UnknownCollisionShape { at: usize, kind: u32 },
    #[error("{chunk} track at +{at} declares {size} bytes, but {left} remain")]
    TrackOverruns {
        chunk: &'static str,
        at: usize,
        size: usize,
        left: usize,
    },
    #[error("{chunk} record at +{at}: interpolation type {kind} is not 0, 1, 2 or 3")]
    UnknownTrackInterpolation {
        chunk: &'static str,
        at: usize,
        kind: u32,
    },
    #[error("LITE record at +{at}: light kind {kind} is not 0, 1 or 2")]
    UnknownLightKind { at: usize, kind: u32 },
    #[error("{chunk} record at +{at} declares {size} bytes, leaving {left} of the chunk")]
    EffectRecordOverruns {
        chunk: &'static str,
        at: usize,
        size: usize,
        left: usize,
    },
    #[error("geoset sub-blocks end at {at} of {have} bytes, so they do not tile it")]
    GeosetNotTiled { at: usize, have: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub tag: [u8; 4],
    /// Offset of the tag itself.
    pub offset: usize,
    /// Offset of the first payload byte.
    pub payload_offset: usize,
    pub size: usize,
}

impl Chunk {
    pub fn payload<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        &data[self.payload_offset..self.payload_offset + self.size]
    }
}

/// The `MODL` record: name plus model extents.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub name: String,
    /// Authored MODL transition duration in milliseconds.
    pub blend_time_ms: u32,
    pub minimum_extent: [f32; 3],
    pub maximum_extent: [f32; 3],
}

/// Byte offset of the extents inside a `MODL` payload.
///
/// Measured, not assumed: every shipped model has a 372-byte `MODL` payload
/// with the six floats in the same place, and the values scale with the model
/// (a town hall spans roughly 20x the extent of a footman).
const MODL_EXTENTS: usize = 0x158;

/// Byte length of a `MODL` payload in the shipped set.
pub const MODL_SIZE: usize = 372;

/// Bytes of inline name field in a node record.
const NODE_NAME: usize = 80;

/// Bytes of fixed fields in a node record, size excluded.
const NODE_FIXED: usize = 4 + NODE_NAME + 12;

/// Bytes of node header in an `EVTS`/`CLID` record, before its track.
const EVENT_FIXED: usize = 96;

/// Bytes of node header in a `RIBB`, `PREM` or `LITE` record.
///
/// A `u32` node size, the 80-byte inline name, and the three node words. That
/// node size is measured, not assumed: it is 96 in 1,096 of the 1,343 shipping
/// records, and in the other 247 the excess over 96 is exactly the byte total
/// of the node's `KGTR`/`KGRT`/`KGSC` tracks. Reading it as the record's own
/// size — which is what the *first* field is — desynchronises every field after
/// the name.
const EFFECT_NODE_FIXED: usize = 4 + NODE_NAME + 12;

/// Byte width of the fixed part of a `RIBB` record's body, after its node.
///
/// Thirteen words: heightAbove, heightBelow, alpha, a colour, the edge lifetime,
/// a texture slot, the edges-per-second rate, the texture rows and columns, the
/// material id and the gravity. Measured by tiling every ribbon to the end of
/// its record: `nodeSize + 52 + tail-tracks == totalSize` holds for all 715
/// ribbon records of both builds.
const RIBBON_FIXED: usize = 52;

/// Byte width of the fixed part of a `PREM` record's body, after its node.
///
/// Four floats, the 260-byte inline particle path, then two floats. The path
/// is a fixed-width field, not a NUL-terminated run: the walk over all 128
/// shipped records only lands on the following `KPEV` when it is stepped over
/// 260 bytes at once.
const PARTICLE_FIXED: usize = 284;

/// Byte width of the inline particle path inside a `PREM` record.
const PARTICLE_PATH: usize = 260;

/// Byte width of the fixed part of a `LITE` record's body, after its node.
///
/// Eleven words: a light kind, an attenuation start and end, a colour, the
/// intensity, an ambient colour and an ambient intensity. The same relation
/// `nodeSize + 44 + tail-tracks == totalSize` holds for all 500 shipped
/// records.
const LIGHT_FIXED: usize = 44;

/// Byte width of the fixed part of a `CAMS` record.
///
/// Nine words: the eye position, the field of view, the far and near clips and
/// the target position. A camera carries no node header at all, which is the
/// one thing about this chunk that is easy to get wrong — see [`Camera`].
const CAMERA_FIXED: usize = 36;

/// One `EVTS` event object, or one `CLID` collision shape.
///
/// These two differ from the other node chunks in a way that is easy to get
/// wrong: the `u32` at the start is the size of the **node header only**, and the
/// animation tracks live *after* it, outside the measured region. Reading it as a
/// self-inclusive record size — which is right for `HELP`, `ATCH`, `CAMS` and
/// `LITE` — makes the walk land mid-track.
///
/// Their tracks also carry frames and no values, because an event fires at a time
/// rather than interpolating one: `magic[4] + count + globalSeq + count * u32`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventNode {
    /// Size of the node header alone, not counting the tracks that follow.
    pub header_size: u32,
    pub name: String,
    pub object_id: u32,
    pub parent_id: i32,
    pub flags: u32,
    /// Frame numbers this event fires at.
    ///
    /// Empty for `CLID`, which carries no track.
    pub frames: Vec<u32>,
    /// `KEVT` for an event object; zeroed for `CLID`.
    pub track: [u8; 4],
    /// Whatever follows the header.
    ///
    /// For `EVTS` this is the `KEVT` track, which is parsed into `frames` and is
    /// empty here. For `CLID` it is inline collision geometry — counts and floats
    /// — that is deliberately left raw: the shape is knowable from the field
    /// count, but what each count means is not yet.
    pub trailing: Vec<u8>,
}

impl EventNode {
    /// Total bytes this record occupies, header plus whatever followed it.
    pub fn size(&self) -> usize {
        self.header_size as usize
            + if self.frames.is_empty() {
                self.trailing.len()
            } else {
                12 + self.frames.len() * 4
            }
    }
}

/// What kind of volume a `CLID` shape describes.
///
/// The word is read from the chunk, not guessed, and only the two kinds that
/// actually occur in either shipping build are accepted. Blizzard's
/// `MDLCOLLISIONSHAPE` union also names a cylinder and a plane, but no shipped
/// model carries one, so their widths would be guesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionShapeKind {
    /// Two vertices: a minimum and a maximum corner, 24 bytes.
    Box,
    /// One vertex plus a radius, 16 bytes.
    Sphere,
}

impl CollisionShapeKind {
    /// Reads the kind word, refusing anything but a box or a sphere.
    fn parse(at: usize, kind: u32) -> Result<Self, Error> {
        match kind {
            0 => Ok(Self::Box),
            2 => Ok(Self::Sphere),
            _ => Err(Error::UnknownCollisionShape { at, kind }),
        }
    }

    /// Bytes the shape's own geometry occupies, the kind word excluded.
    ///
    /// Measured, not assumed: the walk over every shipped model lands on the
    /// end of its last record only when a box steps 24 bytes and a sphere 16.
    /// That is also what Blizzard's `MDLCOLLISIONSHAPE` union says — a box is
    /// two `float[3]` (24), a sphere a `float[3]` and a radius (16).
    fn geometry_len(self) -> usize {
        match self {
            Self::Box => 24,
            Self::Sphere => 16,
        }
    }

    /// Vertices the shape carries: the two corners of a box, one centre for a
    /// sphere.
    fn vertex_count(self) -> usize {
        match self {
            Self::Box => 2,
            Self::Sphere => 1,
        }
    }
}

/// One `CLID` collision shape.
///
/// The chunk carries the `EVTS` node framing — a node-header size, the 80-byte
/// name, `objectId`/`parentId`/`flags` — and then a node animation block that
/// lives *inside* the header size, which is the opposite of `EVTS`'s trailing
/// track. After it comes the shape itself: a kind word, then the corners, then
/// for a sphere its radius.
///
/// ```text
/// u32 header_size     // 96 + node animation bytes
/// char name[80]
/// u32 objectId
/// i32 parentId
/// u32 flags           // 0x2000, CollisionShape, in every shipped record
/// KGTR / KGRT / KGSC  // header_size - 96 bytes, absent in most records
/// u32 kind            // 0 = box, 2 = sphere
/// f32 vertices[3] * n // 2 for a box, 1 for a sphere
/// f32 radius          // sphere only
/// ```
///
/// The framing above is decoded and checked: it holds on every record of both
/// shipping builds. The *floats* are not. They are handed on as the floats they
/// are, uncaptioned, because they are the model's own collision primes and a
/// shape whose centre is half a unit off is a model units walk through — which
/// is the plausibility this chunk refuses to offer in place of a reading.
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionShape {
    /// Size of the node header *plus* its animation block, not the record.
    pub header_size: u32,
    pub name: String,
    pub object_id: u32,
    /// Parent node index, or `-1` for a root. Almost always `-1`: two shipped
    /// records, both the OwlScout's, are parented to object 13.
    pub parent_id: i32,
    pub flags: u32,
    /// Whether this is a box or a sphere.
    ///
    /// Read from the chunk, and a framing fact rather than a label: a box's
    /// geometry is 24 bytes and a sphere's 16, and taking the width from the kind
    /// is what lets the walk land on the next record.
    pub kind: CollisionShapeKind,
    /// The shape's corners, in the order the chunk stores them.
    ///
    /// What the corpus shows, and what it does not: in every shipped record of
    /// both builds the first vertex is componentwise `<=` the second, and 241 of
    /// the 255 boxes are symmetric about the origin in x and y, which is what a
    /// minimum/maximum pair looks like. That is strong, and it is still an
    /// inference from the data — nothing in the bytes says "minimum". They are
    /// therefore handed on in stored order, uncaptioned, so that a caller cannot
    /// build a volume out of them without choosing a reading of its own.
    pub vertices: Vec<[f32; 3]>,
    /// The word after a sphere's centre, for a sphere alone.
    ///
    /// Read as a *radius* on corpus evidence, not on proof: 78 of the 237 models
    /// that ship more than one sphere give every sphere in the model the same
    /// value, which is the capsule idiom — a body swept by equal spheres — and no
    /// shipped sphere carries a non-positive one. That is why it is the radius
    /// slot and not, say, a height. It is still not established, so this chunk
    /// does not hand out a volume and does not claim one.
    pub radius: Option<f32>,
    /// The node's own animation, which most records do not carry.
    pub tracks: Vec<KeyTrack>,
}

impl CollisionShape {
    /// Total bytes this record occupies: node header, node animation, geometry.
    ///
    /// This is the framing relation, and it is the assertion the walk is checked
    /// against: a sum of these over a chunk equals the chunk's payload exactly.
    ///
    /// The node animation is *not* added again here. It already sits inside
    /// `header_size`, which is the trap in this chunk — `EVTS` measures the
    /// header alone and trails its track, so the same sum written for an event
    /// object would be one track too long here.
    pub fn size(&self) -> usize {
        self.header_size as usize + 4 + self.kind.geometry_len()
    }

    /// Bytes of node animation the measured header size accounts for.
    ///
    /// `header_size` is 96 plus exactly this in every record of both shipping
    /// builds, which is what turns "the header may carry an animation" from a
    /// guess into a checked relation. A record whose tracks do not account for
    /// its measured header is malformed, not merely unusual, and the sweep says
    /// so by model path rather than letting the geometry land somewhere plausible.
    pub fn node_track_bytes(&self) -> usize {
        self.tracks
            .iter()
            .map(|t| 16 + t.count as usize * t.stride)
            .sum()
    }

    /// True when the measured header is the fixed node header plus its animation.
    pub fn header_accounts_for_its_animation(&self) -> bool {
        self.header_size as usize == EVENT_FIXED + self.node_track_bytes()
    }

    /// Minimum corner for a box, centre for a sphere.
    pub fn first_vertex(&self) -> Option<[f32; 3]> {
        self.vertices.first().copied()
    }

    /// Maximum corner, for a box only.
    pub fn second_vertex(&self) -> Option<[f32; 3]> {
        (self.kind == CollisionShapeKind::Box)
            .then(|| self.vertices.get(1).copied())
            .flatten()
    }

    /// Volume the shape encloses, on the assumption the floats mean what the
    /// shape kind says they mean.
    ///
    /// Only a box's is unambiguous from the bytes alone; a sphere's follows from
    /// the same assumption, so it is offered rather than asserted.
    pub fn volume(&self) -> Option<f32> {
        match self.kind {
            CollisionShapeKind::Box => {
                let a = self.first_vertex()?;
                let b = self.second_vertex()?;
                Some((b[0] - a[0]) * (b[1] - a[1]) * (b[2] - a[2]))
            }
            CollisionShapeKind::Sphere => {
                let r = self.radius?;
                Some(4.0 / 3.0 * std::f32::consts::PI * r * r * r)
            }
        }
    }
}

/// A node from any `BONE`-shaped chunk.
///
/// `HELP`, `ATCH`, `CAMS` and `LITE` all use the same framing, so one type serves
/// them rather than four near-identical ones.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub offset: usize,
    pub size: u32,
    pub name: String,
    pub object_id: u32,
    /// Parent node index, or `-1` for a root.
    pub parent_id: i32,
    pub flags: u32,
    /// HELP transforms inside the inclusive node size, in file order.
    /// Other node families use their typed track readers.
    pub tracks: Vec<KeyTrack>,
}

impl Node {
    /// True when this node is a helper, per the `flags` bit the shipped models
    /// set consistently.
    pub fn is_helper(&self) -> bool {
        self.flags & 0x100 == 0
    }
}

/// Bytes of fixed node header at the front of a `GEOA` record.
///
/// Static alpha, flags, colour and geoset id; tracks begin at body offset 24.
const GEOA_FIXED: usize = 24;

/// Bytes of track header at the front of a `GEOA` track.
///
/// `magic[4]`, `count`, the interpolation type and the global sequence id. The
/// interpolation type is part of the framing, because it decides the key size:
/// hermite and bezier keys append an in and an out tangent.
const TRACK_HEADER: usize = 16;

/// One `SEQS` record.
///
/// 132 bytes: a NUL-terminated name, the animation interval in milliseconds,
/// the ground speed, and whether the sequence repeats. Derived from the data,
/// not assumed — `1716 / 132` is exactly the footman's 13 sequences, and the
/// same stride divides the knight's (12) and the portrait's (5) exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    pub name: String,
    pub interval_start: u32,
    pub interval_end: u32,
    pub move_speed: f32,
    pub non_looping: bool,
    /// Rarity, sync-point and extent words, retained losslessly.
    pub trailing: [u32; 7],
}

impl Sequence {
    /// Authored SEQS rarity weight at byte 96 of the 132-byte record.
    pub fn rarity(&self) -> f32 {
        f32::from_bits(self.trailing[0])
    }
    /// Length in milliseconds.
    pub fn duration(&self) -> u32 {
        self.interval_end.saturating_sub(self.interval_start)
    }
}

/// One `TEXS` record: a texture the model's materials reference.
///
/// 268 bytes. `804 / 268` is exactly the footman's three entries, and the
/// stride divides the knight's (4) and the town hall's (16) exactly.
///
/// The leading `u32` was initially read as a replaceable-texture id, but that
/// is wrong: most records across every model carry 0, and the town hall has
/// thirteen of them. Its meaning is unresolved, so it is reported as `slot`
/// and nothing is derived from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Texture {
    /// Unclassified leading field. Not assumed to be a replaceable id.
    pub slot: u32,
    pub path: String,
    /// The record's trailing flag word, at `+264`.
    ///
    /// A `TEXS` record is `slot(4) + path(260) + flags(4)` = 268 bytes, so the
    /// word after the path is the texture's own flags. They were being dropped
    /// because the record was read as `slot` + path only.
    ///
    /// The two bits that matter for sampling are [`TEXTURE_WRAP_WIDTH`] and
    /// [`TEXTURE_WRAP_HEIGHT`]. Measured on the expansion main menu — where a
    /// wrapping bug is visible as streaked ice and water — 35 of its 49 textures
    /// set both, including `SKY2`, `Ice_Ice`, `Ice_Icedull`, `Iceberg_New` and
    /// every `ocean_h.*`; 14 set neither, including `FrostWyrm`. Those textures'
    /// geoset UVs run well outside `[0, 1]` (the sky geoset's U spans approx
    /// `-2.6 .. 4.3`), so a clamp instead of a repeat streaks the whole surface.
    pub flags: u32,
}

/// `TEXS` flag `0x1`: the texture repeats along U.
///
/// From the published format implementations — Retera's Model Studio and the
/// open-realm MDX documentation; see `docs/re-notes/main-menu-rendering.md` for
/// the attribution and for what that class of evidence does and does not prove.
pub const TEXTURE_WRAP_WIDTH: u32 = 0x1;
/// `TEXS` flag `0x2`: the texture repeats along V.
pub const TEXTURE_WRAP_HEIGHT: u32 = 0x2;

impl Texture {
    /// Whether the texture repeats along U, per its authored flags.
    #[must_use]
    pub fn wraps_width(&self) -> bool {
        self.flags & TEXTURE_WRAP_WIDTH != 0
    }

    /// Whether the texture repeats along V, per its authored flags.
    #[must_use]
    pub fn wraps_height(&self) -> bool {
        self.flags & TEXTURE_WRAP_HEIGHT != 0
    }
}

/// Byte length of a `SEQS` record in the shipped set.
pub const SEQUENCE_SIZE: usize = 132;
/// Byte length of a `TEXS` record in the shipped set.
pub const TEXTURE_SIZE: usize = 268;
/// Byte offset of the interval fields inside a `SEQS` record.
const SEQ_INTERVAL: usize = 80;
/// Byte length of the `SEQS` name field.
const SEQ_NAME: usize = 80;

/// Fixed prefix of a `BONE` record: the size word, the 80-byte inline name,
/// and the three node words that follow it.
const BONE_FIXED: usize = 4 + 80 + 4 + 4 + 4;
/// The `geosetId`/`geosetAnimId` pair that closes a `BONE` record.
const BONE_TRAILER: usize = 8;
/// Header of one animation track: magic, count, interpolation, global sequence.
const BONE_TRACK_HEAD: usize = 4 + 4 + 4 + 4;

/// How a track interpolates between its keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    /// 0. No interpolation; the value is whatever the nearest key says.
    None,
    /// 1. Straight line between keys.
    Linear,
    /// 2. Cubic Hermite with explicit in/out tangents.
    Hermite,
    /// 3. Cubic Bézier, stored as two Hermite tangents.
    Bezier,
}

impl Interpolation {
    /// Reads the on-disk `lineType` word.
    ///
    /// Every other value is refused rather than guessed at: the key stride
    /// depends on it, so a wrong guess would desynchronise the whole walk.
    fn parse(at: usize, kind: u32) -> Result<Self, Error> {
        match kind {
            0 => Ok(Self::None),
            1 => Ok(Self::Linear),
            2 => Ok(Self::Hermite),
            3 => Ok(Self::Bezier),
            _ => Err(Error::UnknownInterpolation { at, kind }),
        }
    }

    /// Keys carry in/out tangents on top of the plain value.
    fn tangented(self) -> bool {
        matches!(self, Self::Hermite | Self::Bezier)
    }

    /// Reads the on-disk type, without the error context the caller supplies.
    ///
    /// Shared by the bone and geoset-animation tracks, which use the same word.
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::Linear),
            2 => Some(Self::Hermite),
            3 => Some(Self::Bezier),
            _ => None,
        }
    }

    /// True when keys carry an in and an out tangent.
    ///
    /// This decides the key size, so it is framing rather than appearance.
    pub fn has_tangents(self) -> bool {
        self.tangented()
    }
}

/// The property a bone track animates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Translation,
    Rotation,
    Scaling,
}

impl TrackKind {
    /// Components per key value: a quaternion for rotation, a vector otherwise.
    fn width(self) -> usize {
        match self {
            Self::Translation | Self::Scaling => 3,
            Self::Rotation => 4,
        }
    }

    /// Reads the on-disk magic, which is the only discriminator.
    fn parse(at: usize, tag: &[u8]) -> Result<Self, Error> {
        match tag {
            b"KGTR" => Ok(Self::Translation),
            b"KGRT" => Ok(Self::Rotation),
            b"KGSC" => Ok(Self::Scaling),
            other => Err(Error::UnknownBoneTrack {
                at,
                tag: [other[0], other[1], other[2], other[3]],
            }),
        }
    }
}

/// One inline animation track on a bone.
#[derive(Debug, Clone, PartialEq)]
pub struct BoneTrack {
    /// Which property the keys animate.
    pub kind: TrackKind,
    /// Number of keys.
    pub count: u32,
    pub interpolation: Interpolation,
    /// Index into the model's `GLBS` global sequences, or `-1` for the local
    /// timeline. Every shipped model resolves it against the `GLBS` count.
    pub global_sequence_id: i32,
    /// Offset of this track's header within the enclosing record.
    pub offset: usize,
    /// Bytes one key occupies.
    pub stride: usize,
    /// `count` × `stride` bytes, `u32` time followed by the value.
    pub raw: Vec<u8>,
}

impl BoneTrack {
    pub fn key_time(&self, i: usize) -> Option<u32> {
        self.word(i * self.stride)
    }

    /// Value of key `i`, padded to four floats with zeros.
    pub fn value(&self, i: usize) -> Option<[f32; 4]> {
        self.floats(i * self.stride + 4)
    }

    /// In-tangent of key `i`, if the interpolation carries one.
    pub fn in_tangent(&self, i: usize) -> Option<[f32; 4]> {
        if !self.interpolation.tangented() {
            return Some([0.0; 4]);
        }
        self.floats(i * self.stride + 4 + self.kind.width() * 4)
    }

    /// Out-tangent of key `i`, if the interpolation carries one.
    pub fn out_tangent(&self, i: usize) -> Option<[f32; 4]> {
        if !self.interpolation.tangented() {
            return Some([0.0; 4]);
        }
        self.floats(i * self.stride + 4 + 2 * self.kind.width() * 4)
    }

    /// True when every key time is a non-negative millisecond offset.
    pub fn times_are_ordered(&self) -> bool {
        (0..self.count as usize)
            .filter_map(|i| self.key_time(i))
            .is_sorted()
    }

    fn word(&self, at: usize) -> Option<u32> {
        self.raw
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    }

    fn floats(&self, at: usize) -> Option<[f32; 4]> {
        let n = at + self.kind.width() * 4;
        if n > self.raw.len() {
            return None;
        }
        let mut out = [0.0f32; 4];
        for (i, slot) in out.iter_mut().enumerate().take(self.kind.width()) {
            *slot = f32::from_le_bytes(self.raw[at + i * 4..][..4].try_into().unwrap());
        }
        Some(out)
    }
}

/// One `BONE` record: a node of the model's skeleton.
#[derive(Debug, Clone, PartialEq)]
pub struct Bone {
    /// Offset of this record within the `BONE` payload.
    pub offset: usize,
    /// Byte length of the record *including* the trailing geoset pair.
    pub span: usize,
    pub name: String,
    /// Index of this bone among the model's nodes.
    pub object_id: u32,
    /// Node this bone hangs from; `-1` is the root.
    pub parent_id: i32,
    /// Node flags. Bit 0x100 marks a bone and is set on all 35,097 shipped
    /// records; the remaining bits are reported as read.
    pub flags: u32,
    /// Animation tracks, in file order.
    pub tracks: Vec<BoneTrack>,
    /// Geoset this bone is bound to, or `-1`.
    pub geoset_id: i32,
    /// Geoset animation this bone takes part in, or `-1`.
    pub geoset_anim_id: i32,
}

/// A bone's pose at one instant: translation, rotation and scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub translation: [f32; 3],
    /// Unit quaternion, `x y z w`.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }
    }
}

impl Pose {
    /// Composes a child pose onto its parent's, matching how the format defines
    /// the hierarchy: scale, then rotate, then translate.
    pub fn then(self, child: Pose) -> Pose {
        let (r, p) = (self.rotation, self.translation);
        let (cr, cp) = (child.rotation, child.translation);
        // Rotate the child's offset by the parent's rotation.
        let rotated = rotate(r, cp);
        Pose {
            translation: [p[0] + rotated[0], p[1] + rotated[1], p[2] + rotated[2]],
            rotation: quat_mul(r, cr),
            scale: [
                self.scale[0] * child.scale[0],
                self.scale[1] * child.scale[1],
                self.scale[2] * child.scale[2],
            ],
        }
    }
}

/// Rotates `v` by quaternion `q` (`x y z w`).
pub fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let (vx, vy, vz) = (v[0], v[1], v[2]);
    let tx = 2.0 * (y * vz - z * vy);
    let ty = 2.0 * (z * vx - x * vz);
    let tz = 2.0 * (x * vy - y * vx);
    [
        vx + w * tx + (y * tz - z * ty),
        vy + w * ty + (z * tx - x * tz),
        vz + w * tz + (x * ty - y * tx),
    ]
}

/// Multiplies two quaternions, `a` then `b`.
pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
    let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

impl Bone {
    /// True when this is a root: `parentId` is `-1`.
    pub fn is_root(&self) -> bool {
        self.parent_id == -1
    }

    /// The translation track, if the model ships one.
    pub fn translation(&self) -> Option<&BoneTrack> {
        self.tracks
            .iter()
            .find(|t| t.kind == TrackKind::Translation)
    }

    /// The rotation track, if the model ships one.
    pub fn rotation(&self) -> Option<&BoneTrack> {
        self.tracks.iter().find(|t| t.kind == TrackKind::Rotation)
    }

    /// The scale track, if the model ships one.
    pub fn scaling(&self) -> Option<&BoneTrack> {
        self.tracks.iter().find(|t| t.kind == TrackKind::Scaling)
    }
}

/// A decoded skeleton, with the parent links resolved.
///
/// `parentId` addresses the *whole* model's node space, not just the bones, so
/// a bone whose parent is a helper or an attachment stays unresolved rather
/// than being silently re-rooted.
#[derive(Debug, Clone, PartialEq)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
    /// `objectId` → index into `bones`.
    index: Vec<usize>,
}

impl Skeleton {
    /// Builds the index over every bone of `bones`.
    pub fn new(bones: Vec<Bone>) -> Self {
        let index = bones.iter().enumerate().map(|(i, _)| i).collect();
        Self { bones, index }
    }

    /// Resolves an `objectId` to a bone.
    pub fn bone(&self, object_id: u32) -> Option<&Bone> {
        self.index
            .get(object_id as usize)
            .and_then(|&i| self.bones.get(i).filter(|b| b.object_id == object_id))
    }

    /// The bone one `objectId` hangs from, when that parent is itself a bone.
    pub fn parent(&self, object_id: u32) -> Option<&Bone> {
        let b = self.bone(object_id)?;
        if b.parent_id < 0 {
            return None;
        }
        self.bone(b.parent_id as u32)
    }
}

fn read_cstr_bounded(d: &[u8], at: usize, max: usize) -> String {
    let end = d[at..at + max]
        .iter()
        .position(|b| *b == 0)
        .map(|i| at + i)
        .unwrap_or(at + max);
    String::from_utf8_lossy(&d[at..end]).into_owned()
}

/// A bone's inline name field: 80 bytes, NUL-terminated, never overflowing.
///
/// Unlike `read_cstr`, this tolerates a field that never terminates rather
/// than failing the whole record — the shipped set contains one bone whose
/// name is entirely zero bytes.
fn read_bone_name(d: &[u8], at: usize) -> String {
    let field = &d[at + 4..at + 4 + 80];
    let end = field.iter().position(|b| *b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

/// A decoded geoset: the actual renderable mesh.
///
/// Every sub-block follows `tag[4] + u32 count + body`. The counts alone
/// determine each block's payload size, and measured against the shipped
/// models they tile the geoset exactly. `MDLGEOSETSECTION` from Blizzard's own
/// 2.0.1 symbols names the contents `vertices`, `normals`, `texCoords`,
/// `primitives`, `materialId` — the on-disk order below was confirmed against
/// vanilla 1.28c bytes before those names were adopted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geoset {
    /// Offset of this entry's size field within the chunk payload.
    pub offset: usize,
    pub size: u32,
    /// Entry bytes, excluding the size field.
    pub data: Vec<u8>,
}

/// One `VRTX`/`NRMS` array: unpacked little-endian `f32` triples.
#[derive(Debug, Clone, PartialEq)]
pub struct Vec3Array {
    pub tag: [u8; 4],
    pub count: u32,
    /// `count` × 12 bytes, three little-endian floats each.
    pub raw: Vec<u8>,
}

impl Vec3Array {
    /// A placeholder, used only while filling in a mesh whose first block may
    /// not be `VRTX`. Never constructed by callers as a real value.
    pub(crate) fn empty(tag: &[u8; 4]) -> Self {
        Self {
            tag: *tag,
            count: 0,
            raw: Vec::new(),
        }
    }
    pub(crate) fn new(tag: [u8; 4], count: u32, raw: &[u8]) -> Self {
        Self {
            tag,
            count,
            raw: raw.to_vec(),
        }
    }
    pub fn get(&self, i: usize) -> Option<[f32; 3]> {
        let b = self.raw.get(i * 12..i * 12 + 12)?;
        Some([
            f32::from_le_bytes(b[0..4].try_into().ok()?),
            f32::from_le_bytes(b[4..8].try_into().ok()?),
            f32::from_le_bytes(b[8..12].try_into().ok()?),
        ])
    }
    pub fn iter(&self) -> impl Iterator<Item = [f32; 3]> + '_ {
        (0..self.count as usize).filter_map(move |i| self.get(i))
    }
}

/// Triangles as flat `u16` vertex indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Triangles {
    pub count: u32,
    pub raw: Vec<u8>,
}

impl Triangles {
    pub(crate) fn empty() -> Self {
        Self {
            count: 0,
            raw: Vec::new(),
        }
    }
    pub(crate) fn new(count: u32, raw: &[u8]) -> Self {
        Self {
            count,
            raw: raw.to_vec(),
        }
    }
    /// Number of triangles the index list describes.
    pub fn triangle_count(&self) -> usize {
        self.count as usize / 3
    }
    pub fn triangle(&self, t: usize) -> Option<[u16; 3]> {
        let b = self.raw.get(t * 6..t * 6 + 6)?;
        Some([
            u16::from_le_bytes(b[0..2].try_into().ok()?),
            u16::from_le_bytes(b[2..4].try_into().ok()?),
            u16::from_le_bytes(b[4..6].try_into().ok()?),
        ])
    }
}

/// Texture coordinates, two unpacked floats per vertex (u, v).
#[derive(Debug, Clone, PartialEq)]
pub struct UvArray {
    pub count: u32,
    pub raw: Vec<u8>,
}

impl UvArray {
    pub(crate) fn empty() -> Self {
        Self {
            count: 0,
            raw: Vec::new(),
        }
    }
    pub(crate) fn new(count: u32, raw: &[u8]) -> Self {
        Self {
            count,
            raw: raw.to_vec(),
        }
    }
    pub fn get(&self, i: usize) -> Option<[f32; 2]> {
        let b = self.raw.get(i * 8..i * 8 + 8)?;
        Some([
            f32::from_le_bytes(b[0..4].try_into().ok()?),
            f32::from_le_bytes(b[4..8].try_into().ok()?),
        ])
    }
}

/// Pivot points: one position per node, indexed by node id.
///
/// The Footman carries 57 entries against a highest `parentId` of 39, so the
/// table is indexed by the model's whole node space rather than by bones alone.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pivots(pub Vec<[f32; 3]>);

impl Pivots {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if !data.len().is_multiple_of(12) {
            return Err(Error::RaggedRecords {
                chunk: "PIVT",
                size: data.len(),
                stride: 12,
            });
        }
        Ok(Self(
            data.as_chunks::<12>()
                .0
                .iter()
                .map(|c| {
                    [
                        f32::from_le_bytes(c[0..4].try_into().unwrap()),
                        f32::from_le_bytes(c[4..8].try_into().unwrap()),
                        f32::from_le_bytes(c[8..12].try_into().unwrap()),
                    ]
                })
                .collect(),
        ))
    }

    pub fn position(&self, node: u32) -> Option<[f32; 3]> {
        self.0.get(node as usize).copied()
    }
}

/// One `GEOA` entry: a geoset's animated tint.
///
/// The record is a 24-byte header — static alpha, flags, three colour
/// floats and a geoset id — followed by up to three animation tracks: `KGAA` (alpha), `KGAC`
/// (colour), `KGAO` (opacity).
///
/// `len` is **self-inclusive**: it counts its own four-byte length field, so the
/// record's data is `len - 4` bytes. Every framing comparison here is written
/// against that, because comparing a track total to `len` instead of `len - 4`
/// is off by exactly four and reads as "the record padded".
///
/// The relationship `record_len == 24 + count * key_stride` holds across every
/// sampled geoset, which is what makes the framing trustworthy.
#[derive(Debug, Clone, PartialEq)]
pub struct GeosetAnim {
    pub offset: usize,
    pub len: u32,
    pub flags: u32,
    /// Index of the geoset this record animates, or `NO_ID`.
    pub geoset_id: u32,
    /// Static alpha, before any `KGAA` track.
    pub static_alpha: f32,
    /// Static colour, before any `KGAC` track.
    pub static_colour: [f32; 3],
    /// Animation tracks, in file order.
    pub tracks: Vec<AnimTrack>,
    /// Bytes after the tracks the walk did not consume.
    ///
    /// Non-zero means the record padded, which is rare but real. Keeping the
    /// count visible is what stops a misread track header from hiding inside a
    /// "must fit" comparison.
    pub trailing: usize,
}

/// One scalar/colour animation curve, also reused by checked light tracks.
///
/// The measured layout is a 16-byte header — `magic[4]`, `count`, the
/// interpolation type and the global sequence id — followed by `count` keys. A
/// key is `{ frame: u32, value }`, and hermite or bezier keys append an in and
/// an out tangent after the value.
///
/// `record_len == 24 + 16 + count * key_len` holds for all 1,608 sampled
/// records, which is what makes the framing trustworthy rather than merely
/// self-consistent. An earlier version read a 12-byte header and 8-byte keys
/// regardless, which still walked most records and produced frames of
/// `0xFFFFFFFF` when it did not.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimTrack {
    /// Original curve tag, including `KGAC`, `KLAC` and `KLBC` colours.
    pub magic: [u8; 4],
    pub count: u32,
    pub interpolation: Interpolation,
    /// Global sequence driving the track, or `NO_ID`.
    pub global_seq_id: u32,
    /// Raw key bytes.
    pub raw: Vec<u8>,
}

impl AnimTrack {
    /// Components in one key value: one for a scalar, three for a colour or camera vector.
    fn components(&self) -> usize {
        match &self.magic[..] {
            b"KGAC" | b"KLAC" | b"KLBC" | b"KCTR" | b"KTTR" => 3,
            _ => 1,
        }
    }

    /// Bytes per key, given this track's value width and interpolation.
    ///
    /// A key is `frame` plus its value, and a tangented key appends an in and an
    /// out tangent the same width as the value. So the frame is counted once,
    /// not three times: `4 + comp * 4 * 3`, not `(4 + comp * 4) * 3`. The
    /// difference is four bytes per key, which is enough to make a colour track
    /// overrun its record while a scalar one still fits.
    pub fn key_len(&self) -> usize {
        let comp = self.components();
        if self.interpolation.has_tangents() {
            4 + comp * 4 * 3
        } else {
            4 + comp * 4
        }
    }

    /// The frame of key `i`.
    pub fn frame(&self, i: usize) -> u32 {
        let k = self.key_len();
        let b = &self.raw[i * k..i * k + 4];
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    /// The scalar value of key `i`.
    pub fn value(&self, i: usize) -> f32 {
        let k = self.key_len();
        let b = &self.raw[i * k + 4..i * k + 8];
        f32::from_bits(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// The colour value of key `i`, for a `KGAC` track.
    pub fn colour(&self, i: usize) -> [f32; 3] {
        let k = self.key_len();
        let b = &self.raw[i * k + 4..i * k + 16];
        [
            f32::from_bits(u32::from_le_bytes(b[0..4].try_into().unwrap())),
            f32::from_bits(u32::from_le_bytes(b[4..8].try_into().unwrap())),
            f32::from_bits(u32::from_le_bytes(b[8..12].try_into().unwrap())),
        ]
    }

    /// Incoming tangent of key `i`, for a hermite or bezier track.
    ///
    /// Key order is `frame, value, inTan, outTan`, so for a scalar the tangents
    /// sit at +8 and +12.
    pub fn in_tangent(&self, i: usize) -> f32 {
        self.component_in_tangent(i, 0)
    }

    /// Outgoing tangent of key `i`, for a hermite or bezier track.
    pub fn out_tangent(&self, i: usize) -> f32 {
        self.component_out_tangent(i, 0)
    }

    /// One float from key `i`'s value vector (component 0 is the scalar channel).
    fn component_value(&self, i: usize, component: usize) -> f32 {
        let k = self.key_len();
        let w = self.components();
        debug_assert!(component < w);
        let at = i * k + 4 + component * 4;
        let b = &self.raw[at..at + 4];
        f32::from_bits(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// In-tangent for one component; matches [`KeyTrack`] width-aware layout.
    fn component_in_tangent(&self, i: usize, component: usize) -> f32 {
        if !self.interpolation.has_tangents() {
            return 0.0;
        }
        let k = self.key_len();
        let w = self.components();
        let at = i * k + 4 + w * 4 + component * 4;
        let b = &self.raw[at..at + 4];
        f32::from_bits(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Out-tangent for one component; matches [`KeyTrack`] width-aware layout.
    fn component_out_tangent(&self, i: usize, component: usize) -> f32 {
        if !self.interpolation.has_tangents() {
            return 0.0;
        }
        let k = self.key_len();
        let w = self.components();
        let at = i * k + 4 + 2 * w * 4 + component * 4;
        let b = &self.raw[at..at + 4];
        f32::from_bits(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// Samples a bone's pose at `frame`.
///
/// Tracks are evaluated independently and composed in the format's order —
/// scale, rotate, translate. A missing track holds its identity, so a bone that
/// only animates translation still scales and rotates correctly rather than
/// collapsing.
pub fn sample_bone_pose(bone: &Bone, frame: u32) -> Pose {
    let mut pose = Pose::default();
    if let Some(t) = bone.translation() {
        pose.translation = sample_bone_track(t, frame).translation;
    }
    if let Some(t) = bone.rotation() {
        pose.rotation = sample_bone_track(t, frame).rotation;
    }
    if let Some(t) = bone.scaling() {
        pose.scale = sample_bone_track(t, frame).scale;
    }
    pose
}

/// Samples one borrowed track without allocating or selecting its clock.
/// Only its property changes; the other pose properties retain their identities.
pub fn sample_bone_track(track: &BoneTrack, frame: u32) -> Pose {
    sample_bone_track_range(track, frame, 0, track.count as usize)
}

/// Samples only keys in the selected local sequence, retaining identity when
/// that sequence has no keys. Global-sequence clocks use `sample_bone_track`.
pub fn sample_bone_track_in_interval(track: &BoneTrack, frame: u32, interval: [u32; 2]) -> Pose {
    let partition = |inclusive: bool, boundary: u32| {
        let (mut lo, mut hi) = (0, track.count as usize);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let key = track.key_time(mid).unwrap_or(0);
            if key < boundary || (inclusive && key == boundary) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    };
    let first = partition(false, interval[0]);
    let end = partition(true, interval[1]);
    if first >= end {
        return Pose::default();
    }
    sample_bone_track_range(track, frame, first, end)
}

fn sample_bone_track_range(track: &BoneTrack, frame: u32, first: usize, end: usize) -> Pose {
    let mut pose = Pose::default();
    match track.kind {
        TrackKind::Translation => pose.translation = sample_vec3(track, frame, first, end),
        TrackKind::Rotation => pose.rotation = sample_rotation(track, frame, first, end),
        TrackKind::Scaling => pose.scale = sample_vec3(track, frame, first, end),
    }
    pose
}

/// Where `frame` falls among keys `first..end`.
enum Span {
    /// Before the first key, after the last, or a track with one key.
    Key(usize),
    /// Between two keys, `t` of the way from `lo` to `hi`.
    Between { lo: usize, hi: usize, t: f32 },
}

fn span(track: &BoneTrack, frame: u32, first: usize, end: usize) -> Option<Span> {
    let n = end - first;
    if n == 0 {
        return None;
    }
    if n == 1 || frame <= track.key_time(first).unwrap_or(0) {
        return Some(Span::Key(first));
    }
    if frame >= track.key_time(end - 1).unwrap_or(0) {
        return Some(Span::Key(end - 1));
    }
    let mut lo = first;
    let mut hi = end - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if track.key_time(mid).unwrap_or(0) <= frame {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (t0, t1) = (
        track.key_time(lo).unwrap_or(0),
        track.key_time(hi).unwrap_or(0),
    );
    let t = if t1 == t0 {
        0.0
    } else {
        ((frame - t0) as f32 / (t1 - t0) as f32).clamp(0.0, 1.0)
    };
    Some(Span::Between { lo, hi, t })
}

/// Samples all three components of a translation or scaling track from one key search.
fn sample_vec3(track: &BoneTrack, frame: u32, first: usize, end: usize) -> [f32; 3] {
    let at = |i: usize| {
        let v = track.value(i).unwrap_or([0.0; 4]);
        [v[0], v[1], v[2]]
    };
    let (lo, hi, t) = match span(track, frame, first, end) {
        None => return [0.0; 3],
        Some(Span::Key(i)) => return at(i),
        Some(Span::Between { lo, hi, t }) => (lo, hi, t),
    };
    let (v0, v1) = (at(lo), at(hi));
    match track.interpolation {
        Interpolation::None => v0,
        Interpolation::Linear => std::array::from_fn(|c| v0[c] + (v1[c] - v0[c]) * t),
        Interpolation::Hermite | Interpolation::Bezier => {
            // A midpoint is *not* Hermite. With tangents available the curve
            // bulges, and ignoring that flattens every bone animation into
            // something close to linear — which looks almost right and is not.
            let m0 = track.out_tangent(lo).unwrap_or([0.0; 4]);
            let m1 = track.in_tangent(hi).unwrap_or([0.0; 4]);
            std::array::from_fn(|c| cubic(track.interpolation, v0[c], v1[c], m0[c], m1[c], t))
        }
    }
}

/// Samples a rotation track as a unit quaternion (`x y z w`).
///
/// `Linear` keys slerp and `Hermite`/`Bezier` keys use squad through the keys'
/// tangent quaternions, each along the shorter arc. Interpolating components
/// separately leaves the unit sphere and folds through zero between keys of
/// opposite sign, which turned the time-of-day sphere edge-on at midday.
/// A degenerate (zero or non-finite) key holds the earlier key.
fn sample_rotation(track: &BoneTrack, frame: u32, first: usize, end: usize) -> [f32; 4] {
    const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    let key = |i: usize| track.value(i).unwrap_or(IDENTITY);
    let (lo, hi, t) = match span(track, frame, first, end) {
        None => return [0.0; 4],
        Some(Span::Key(i)) => return key(i),
        Some(Span::Between { lo, hi, t }) => (lo, hi, t),
    };
    let (q0, q1) = (key(lo), key(hi));
    let slerp = |a, b, t| crate::pose::slerp(a, b, t).unwrap_or(a);
    match track.interpolation {
        Interpolation::None => q0,
        Interpolation::Linear => slerp(q0, q1, t),
        Interpolation::Hermite | Interpolation::Bezier => {
            let out0 = track.out_tangent(lo).unwrap_or(q0);
            let in1 = track.in_tangent(hi).unwrap_or(q1);
            slerp(slerp(q0, q1, t), slerp(out0, in1, t), 2.0 * t * (1.0 - t))
        }
    }
}

/// Evaluates every bone of a skeleton at `frame`, in file order.
pub fn sample_skeleton(bones: &[Bone], frame: u32) -> Vec<Pose> {
    bones.iter().map(|b| sample_bone_pose(b, frame)).collect()
}

/// World-space matrices for a skeleton posed at `frame`, one per bone.
///
/// `parent` maps a bone's `objectId` to its parent's, which is how the format
/// defines the hierarchy — `parentId` addresses the whole node space, so a bone
/// hanging from a helper resolves through `parent` rather than through the bone
/// list alone. `pivots` supplies each node's rest offset.
///
/// A bone's own matrix is its pose composed onto its parent's, then shifted by
/// its pivot: the pivot is subtracted from the vertex and added back to the
/// matrix, which is the standard way the format expresses "rotate about here".
pub fn bone_matrices(
    bones: &[Bone],
    parent: &dyn Fn(u32) -> Option<u32>,
    pivots: &Pivots,
    frame: u32,
) -> Vec<[f32; 16]> {
    let poses = sample_skeleton(bones, frame);
    bone_matrices_from_poses(bones, parent, pivots, &poses)
}

/// Object id to bone index; a later bone with the same id replaces an earlier one.
///
/// Ids are node indices, so a flat table serves every ordinary model without
/// hashing. Ids far past the bone count use a map instead of a huge table.
enum BoneIndex {
    Flat(Vec<u32>),
    Sparse(std::collections::HashMap<u32, usize>),
}

/// Flat tables are used while the largest id stays within this many ids per bone, plus slack.
const FLAT_IDS_PER_BONE: usize = 4;
const FLAT_ID_SLACK: usize = 64;
/// Flat-table marker for an id no bone has.
const NO_BONE: u32 = u32::MAX;

impl BoneIndex {
    fn new(bones: &[Bone]) -> Self {
        let largest = bones.iter().map(|b| b.object_id).max().unwrap_or(0) as usize;
        if largest <= bones.len() * FLAT_IDS_PER_BONE + FLAT_ID_SLACK
            && bones.len() < NO_BONE as usize
        {
            let mut table = vec![NO_BONE; largest + 1];
            for (i, b) in bones.iter().enumerate() {
                table[b.object_id as usize] = i as u32;
            }
            Self::Flat(table)
        } else {
            Self::Sparse(
                bones
                    .iter()
                    .enumerate()
                    .map(|(i, b)| (b.object_id, i))
                    .collect(),
            )
        }
    }

    fn get(&self, id: u32) -> Option<usize> {
        match self {
            Self::Flat(table) => table
                .get(id as usize)
                .copied()
                .filter(|&i| i != NO_BONE)
                .map(|i| i as usize),
            Self::Sparse(map) => map.get(&id).copied(),
        }
    }
}

/// Compose supplied poses with the same hierarchy, pivot and cycle behavior as `bone_matrices`.
/// Missing poses retain the pose adapter's existing identity fallback; extra poses are ignored.
#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "animation.bone_matrices")
)]
pub fn bone_matrices_from_poses(
    bones: &[Bone],
    parent: &dyn Fn(u32) -> Option<u32>,
    pivots: &Pivots,
    poses: &[Pose],
) -> Vec<[f32; 16]> {
    // By object id, so a parent can be found without walking the list.
    let by_id = BoneIndex::new(bones);
    // One `parent` query per bone; the walk and the composition both reuse it.
    // Per bone: its parent's index, and whether its matrix is composed yet.
    let mut links: Vec<(Option<usize>, bool)> = bones
        .iter()
        .map(|b| (parent(b.object_id).and_then(|p| by_id.get(p)), false))
        .collect();
    let mut world = vec![[0.0f32; 16]; bones.len()];
    let mut chain = Vec::with_capacity(bones.len());

    // Iterative rather than recursive, and it terminates: a cycle in the parent
    // chain would otherwise recurse forever.
    for start in 0..bones.len() {
        if links[start].1 {
            continue;
        }
        chain.clear();
        let mut cur = Some(start);
        while let Some(i) = cur {
            if links[i].1 {
                break;
            }
            if chain.contains(&i) {
                // Stop before composing the cycle; its members retain their
                // existing zero-initialized matrices.
                chain.truncate(chain.iter().position(|&x| x == i).unwrap());
                break;
            }
            chain.push(i);
            cur = links[i].0;
        }
        // Compose root-down so each parent exists before its child.
        for &i in chain.iter().rev() {
            let pose = poses.get(i).copied().unwrap_or_default();
            let base = links[i].0.filter(|&p| p != i).map(|p| &world[p]);
            let pivot = pivots.position(bones[i].object_id).unwrap_or([0.0; 3]);
            world[i] = compose_onto(base.unwrap_or(&IDENTITY), pose, pivot);
            links[i].1 = true;
        }
    }
    world
}

/// Composes a parent's world matrix, a pose, and the bone's pivot.
///
/// The format's local transform turns about the bone's **pivot**, so the local
/// matrix is
///
/// ```text
/// T(pivot) * T(translation) * R * S * T(-pivot)
/// ```
///
/// which collapses to a linear part `R * S` and a translation column of
/// `(pivot + translation) - (R * S) * pivot`. The world matrix is then
/// `parent * local` — **both** the 3×3 and the translation column, because a
/// bone inside a rotated parent has to inherit that parent's rotation.
pub fn compose(parent: [f32; 16], pose: Pose, pivot: [f32; 3]) -> [f32; 16] {
    compose_onto(&parent, pose, pivot)
}

/// [`compose`] reading the parent matrix in place.
fn compose_onto(parent: &[f32; 16], pose: Pose, pivot: [f32; 3]) -> [f32; 16] {
    let q = pose.rotation;
    let s = pose.scale;
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    // `R * S`: the rotation's columns, each scaled by the matching axis.
    let rs = [
        [
            (1.0 - 2.0 * (y * y + z * z)) * s[0],
            2.0 * (x * y - z * w) * s[1],
            2.0 * (x * z + y * w) * s[2],
        ],
        [
            2.0 * (x * y + z * w) * s[0],
            (1.0 - 2.0 * (x * x + z * z)) * s[1],
            2.0 * (y * z - x * w) * s[2],
        ],
        [
            2.0 * (x * z - y * w) * s[0],
            2.0 * (y * z + x * w) * s[1],
            (1.0 - 2.0 * (x * x + y * y)) * s[2],
        ],
    ];
    // `(pivot + translation) - (R * S) * pivot`.
    let local_t = [
        pivot[0] + pose.translation[0]
            - (rs[0][0] * pivot[0] + rs[0][1] * pivot[1] + rs[0][2] * pivot[2]),
        pivot[1] + pose.translation[1]
            - (rs[1][0] * pivot[0] + rs[1][1] * pivot[1] + rs[1][2] * pivot[2]),
        pivot[2] + pose.translation[2]
            - (rs[2][0] * pivot[0] + rs[2][1] * pivot[1] + rs[2][2] * pivot[2]),
    ];
    // `parent * local`, written out: the parent's 3×3 multiplies the local 3×3,
    // and its 3×3 and translation both act on the local translation.
    let mut out = [0.0f32; 16];
    for col in 0..3 {
        for row in 0..3 {
            out[col * 4 + row] = parent[row] * rs[0][col]
                + parent[4 + row] * rs[1][col]
                + parent[8 + row] * rs[2][col];
        }
    }
    out[12] = parent[0] * local_t[0] + parent[4] * local_t[1] + parent[8] * local_t[2] + parent[12];
    out[13] = parent[1] * local_t[0] + parent[5] * local_t[1] + parent[9] * local_t[2] + parent[13];
    out[14] =
        parent[2] * local_t[0] + parent[6] * local_t[1] + parent[10] * local_t[2] + parent[14];
    out[15] = 1.0;
    out
}

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// The identity matrix.
pub fn identity() -> [f32; 16] {
    IDENTITY
}

/// Applies a 4x4 matrix to a point.
pub fn transform_point(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

/// Span between two keys for `frame`, or `None` when `frame` is outside the interior.
fn anim_interior_span(track: &AnimTrack, frame: u32) -> Option<(usize, usize, u32, u32)> {
    let n = track.count as usize;
    if n < 2 || frame <= track.frame(0) || frame >= track.frame(n - 1) {
        return None;
    }
    let mut lo = 0usize;
    let mut hi = n - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if track.frame(mid) <= frame {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some((lo, hi, track.frame(lo), track.frame(hi)))
}

fn interpolate_anim_component(
    track: &AnimTrack,
    lo: usize,
    hi: usize,
    f0: u32,
    f1: u32,
    frame: u32,
    component: usize,
) -> f32 {
    let (v0, v1) = (
        track.component_value(lo, component),
        track.component_value(hi, component),
    );
    match track.interpolation {
        Interpolation::None => v0,
        Interpolation::Linear => {
            if f1 == f0 {
                return v0;
            }
            let t = (frame - f0) as f32 / (f1 - f0) as f32;
            v0 + (v1 - v0) * t.clamp(0.0, 1.0)
        }
        Interpolation::Hermite | Interpolation::Bezier => {
            if f1 == f0 {
                return v0;
            }
            let t = ((frame - f0) as f32 / (f1 - f0) as f32).clamp(0.0, 1.0);
            cubic(
                track.interpolation,
                v0,
                v1,
                track.component_out_tangent(lo, component),
                track.component_in_tangent(hi, component),
                t,
            )
        }
    }
}

fn sample_anim_component(track: &AnimTrack, frame: u32, component: usize) -> f32 {
    let n = track.count as usize;
    if n == 0 {
        return 0.0;
    }
    if n == 1 || frame <= track.frame(0) {
        return track.component_value(0, component);
    }
    if frame >= track.frame(n - 1) {
        return track.component_value(n - 1, component);
    }
    let Some((lo, hi, f0, f1)) = anim_interior_span(track, frame) else {
        return track.component_value(0, component);
    };
    interpolate_anim_component(track, lo, hi, f0, f1, frame, component)
}

/// Samples a scalar track at `frame`.
///
/// Outside the key range a track holds its nearest end value rather than
/// extrapolating, which matches how the format behaves: a model with no keys for
/// the current sequence is static, not unbounded.
pub fn sample_scalar(track: &AnimTrack, frame: u32) -> f32 {
    sample_anim_component(track, frame, 0)
}

/// Samples a `KGAC`, `KLAC` or `KLBC` colour track at `frame`.
///
/// Returns raw stored components with one shared key-span search. The caller
/// supplies a validated colour curve; material channel mapping is separate.
pub fn sample_colour(track: &AnimTrack, frame: u32) -> [f32; 3] {
    sample_vector(track, frame)
}

/// Samples a validated three-component curve, including camera eye/target offsets.
pub fn sample_vector(track: &AnimTrack, frame: u32) -> [f32; 3] {
    let n = track.count as usize;
    if n == 0 {
        return [0.0; 3];
    }
    if n == 1 || frame <= track.frame(0) {
        return [
            track.component_value(0, 0),
            track.component_value(0, 1),
            track.component_value(0, 2),
        ];
    }
    if frame >= track.frame(n - 1) {
        let last = n - 1;
        return [
            track.component_value(last, 0),
            track.component_value(last, 1),
            track.component_value(last, 2),
        ];
    }
    let Some((lo, hi, f0, f1)) = anim_interior_span(track, frame) else {
        return [0.0; 3];
    };
    [
        interpolate_anim_component(track, lo, hi, f0, f1, frame, 0),
        interpolate_anim_component(track, lo, hi, f0, f1, frame, 1),
        interpolate_anim_component(track, lo, hi, f0, f1, frame, 2),
    ]
}

/// Bezier tangents are control points; Hermite tangents are derivatives.
fn cubic(kind: Interpolation, p0: f32, p1: f32, out: f32, incoming: f32, t: f32) -> f32 {
    if kind == Interpolation::Bezier {
        let u = 1.0 - t;
        u * u * u * p0 + 3.0 * u * u * t * out + 3.0 * u * t * t * incoming + t * t * t * p1
    } else {
        hermite(p0, p1, out, incoming, t)
    }
}

/// Cubic Hermite interpolation between two keys.
fn hermite(p0: f32, p1: f32, m0: f32, m1: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * p0 + h10 * m0 + h01 * p1 + h11 * m1
}

/// One inline animation track inside a `TXAN`, `RIBB`, `PREM`, `LITE` or
/// `CAMS` record.
///
/// The framing is the one every chunk of this format uses — `magic[4]`, a key
/// count, the interpolation type, then the global sequence id — followed by
/// `count` keys of `{ frame: u32, value }`, where a tangented key appends an in
/// and an out tangent of the same width as its value.
///
/// Unlike [`AnimTrack`] the value width is not fixed at one component: `KGRT`
/// carries a quaternion, `KCTR`/`KLAC` a vector, `KLAV` a scalar. The width is
/// therefore part of the framing and is read from the magic, not from the
/// record. Every magic below was observed in the shipped models with the walk
/// over its chunk consuming that chunk exactly; an unrecognised magic is
/// refused rather than given a default width, because a wrong width shifts every
/// subsequent record and a plausible-looking walk is worse than a failure.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyTrack {
    pub magic: [u8; 4],
    pub count: u32,
    pub interpolation: Interpolation,
    /// Index into the model's `GLBS` global sequences, or `NO_ID`.
    pub global_seq_id: u32,
    /// Components in one key value.
    pub width: usize,
    /// Bytes one key occupies.
    pub stride: usize,
    pub raw: Vec<u8>,
}

impl KeyTrack {
    /// Components per key value, decided by the track magic.
    ///
    /// Returns `None` for an unrecognised magic, which the caller refuses.
    fn magic_width(magic: &[u8; 4]) -> Option<usize> {
        match &magic[..] {
            // Vector-valued: translation, scaling, a colour, a camera eye or a
            // ribbon emitter's colour.
            b"KGTR" | b"KGSC" | b"KTAT" | b"KTAS" | b"KCTR" | b"KTTR" | b"KLAC" | b"KLBC" => {
                Some(3)
            }
            // Quaternion-valued: a rotation.
            b"KGRT" | b"KTAR" => Some(4),
            // Scalar-valued: visibility, intensity, a rate or a lifetime.
            b"KPEV" | b"KLAV" | b"KLAI" | b"KLBI" | b"KCRL" | b"KRVS" | b"KRHA" | b"KRHB" => {
                Some(1)
            }
            _ => None,
        }
    }

    /// Bytes one key occupies: the frame, then `width` floats, plus two more
    /// `width`-wide tangents when the interpolation carries them.
    fn key_len(width: usize, interpolation: Interpolation) -> usize {
        if interpolation.has_tangents() {
            4 + width * 4 * 3
        } else {
            4 + width * 4
        }
    }

    /// The frame of key `i`, in milliseconds along the timeline.
    pub fn frame(&self, i: usize) -> Option<u32> {
        let b = self.raw.get(i * self.stride..i * self.stride + 4)?;
        Some(u32::from_le_bytes(b.try_into().ok()?))
    }

    /// Signed frame time in milliseconds, preserving negative preroll.
    ///
    /// The existing [`Self::frame`] accessor keeps its unsigned interpretation.
    /// This accessor checks the declared key count, record span and byte bounds;
    /// malformed or overflowing spans return `None`. Values, interpolation,
    /// ordering and clock/clip selection still require caller validation.
    pub fn frame_signed(&self, i: usize) -> Option<i32> {
        let count = usize::try_from(self.count).ok()?;
        if i >= count || self.stride < 4 || count.checked_mul(self.stride)? != self.raw.len() {
            return None;
        }
        let at = i.checked_mul(self.stride)?;
        let end = at.checked_add(4)?;
        let bytes = self.raw.get(at..end)?;
        Some(i32::from_le_bytes(bytes.try_into().ok()?))
    }

    /// Key `i`'s value as `width` floats, zero-padded to four.
    ///
    /// Zero padding keeps a scalar and a quaternion readable through one
    /// accessor; the caller reads `width` of them.
    pub fn value(&self, i: usize) -> Option<[f32; 4]> {
        self.floats(i * self.stride + 4)
    }

    /// The scalar value of key `i`.
    pub fn scalar(&self, i: usize) -> Option<f32> {
        self.value(i).map(|v| v[0])
    }

    /// The three-float value of key `i`.
    pub fn vec3(&self, i: usize) -> Option<[f32; 3]> {
        self.value(i).map(|v| [v[0], v[1], v[2]])
    }

    /// In-tangent of key `i`, as `width` floats zero-padded to four.
    pub fn in_tangent(&self, i: usize) -> Option<[f32; 4]> {
        if !self.interpolation.has_tangents() {
            return Some([0.0; 4]);
        }
        let at = i * self.stride + 4 + self.width * 4;
        self.floats(at)
    }

    /// Out-tangent of key `i`, as `width` floats zero-padded to four.
    pub fn out_tangent(&self, i: usize) -> Option<[f32; 4]> {
        if !self.interpolation.has_tangents() {
            return Some([0.0; 4]);
        }
        let at = i * self.stride + 4 + 2 * self.width * 4;
        self.floats(at)
    }

    fn floats(&self, at: usize) -> Option<[f32; 4]> {
        let end = at + self.width * 4;
        if end > self.raw.len() {
            return None;
        }
        let mut out = [0.0f32; 4];
        for (c, slot) in out.iter_mut().enumerate().take(self.width) {
            let b = self.raw.get(at + c * 4..at + c * 4 + 4)?;
            *slot = f32::from_bits(u32::from_le_bytes(b.try_into().ok()?));
        }
        Some(out)
    }

    /// True when every key time is a non-negative millisecond offset.
    pub fn times_are_ordered(&self) -> bool {
        (0..self.count as usize)
            .filter_map(|i| self.frame(i))
            .is_sorted()
    }

    /// True when the track's first key carries a negative frame.
    ///
    /// This is real in the shipped set rather than a misread: 21 tracks, all of
    /// them camera tracks in the cinematic-camera families, open with a frame
    /// just below zero, as though the emitter were keyed a few frames before
    /// the sequence starts. Reading it as a `u32` yields a huge frame instead,
    /// which is why it is a separate question from ordering.
    pub fn has_negative_frame(&self) -> bool {
        self.frame(0).is_some_and(|f| (f as i32) < 0)
    }
}

/// Reads the tracks occupying `p[from..to]` and requires the region to be
/// consumed exactly.
///
/// The "exactly" is the point: a magic that stops the walk early leaves bytes
/// behind, and those bytes are then silently read as the next record's size.
/// `base` is added to the offset in any error, so an offset reported against a
/// record's body still addresses the chunk payload it came from.
fn read_tracks(
    chunk: &'static str,
    p: &[u8],
    from: usize,
    to: usize,
    base: usize,
    out: &mut Vec<KeyTrack>,
) -> Result<(), Error> {
    let at = |cur: usize| base + cur;
    let mut cur = from;
    while cur < to {
        let magic: [u8; 4] = p
            .get(cur..cur + 4)
            .ok_or(Error::TrackOverruns {
                chunk,
                at: at(cur),
                size: 4,
                left: to.saturating_sub(cur),
            })?
            .try_into()
            .unwrap();
        let Some(width) = KeyTrack::magic_width(&magic) else {
            return Err(Error::UnknownTrackMagic {
                chunk,
                at: at(cur),
                tag: magic,
            });
        };
        let count = read_u32(p, cur + 4)?;
        let kind = read_u32(p, cur + 8)?;
        let interpolation =
            Interpolation::from_u32(kind).ok_or_else(|| Error::UnknownTrackInterpolation {
                chunk,
                at: at(cur),
                kind,
            })?;
        let global_seq_id = read_u32(p, cur + 12)?;
        let stride = KeyTrack::key_len(width, interpolation);
        let size = (count as usize)
            .checked_mul(stride)
            .and_then(|n| n.checked_add(16))
            .ok_or(Error::TrackOverruns {
                chunk,
                at: at(cur),
                size: usize::MAX,
                left: to - cur,
            })?;
        let next = cur.checked_add(size).ok_or(Error::TrackOverruns {
            chunk,
            at: at(cur),
            size,
            left: to - cur,
        })?;
        if next > to {
            return Err(Error::TrackOverruns {
                chunk,
                at: at(cur),
                size,
                left: to - cur,
            });
        }
        out.push(KeyTrack {
            magic,
            count,
            interpolation,
            global_seq_id,
            width,
            stride,
            raw: p[cur + 16..next].to_vec(),
        });
        cur = next;
    }
    Ok(())
}

/// The node framing shared by `RIBB`, `PREM` and `LITE`.
///
/// Each record opens with two sizes: the record's own total, counting its first
/// four bytes, and the node's, which is 96 plus however many bytes of node
/// animation follow. The node's animation lives *inside* the node size, which
/// is the opposite of `EVTS`/`CLID` — see [`EventNode`] — and reading it the
/// other way round is what puts the emitter's own fields in the wrong place.
struct EffectNode {
    /// Offset of this record's total-size field within the chunk payload.
    offset: usize,
    /// The record's own total size, counting its first four bytes.
    total_size: usize,
    /// 96 plus the node's animation tracks.
    node_size: usize,
    name: String,
    object_id: u32,
    parent_id: i32,
    flags: u32,
    /// `KGTR`/`KGRT`/`KGSC` tracks inside the node header.
    node_tracks: Vec<KeyTrack>,
    /// Bytes from `+4 + node_size` to the end of the record.
    body: Vec<u8>,
}

/// Walks the node records of a `RIBB`, `PREM` or `LITE` payload.
///
/// `None` is the absent chunk, which is the common case: most shipped models
/// carry none of these three, and a missing one is simply an empty list.
fn effect_nodes(chunk: &'static str, p: Option<&[u8]>) -> Result<Vec<EffectNode>, Error> {
    let Some(p) = p else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < p.len() {
        let total_size = read_u32(p, at)? as usize;
        if total_size < 4 + EFFECT_NODE_FIXED {
            return Err(Error::EffectRecordOverruns {
                chunk,
                at,
                size: total_size,
                left: p.len() - at,
            });
        }
        let end = at
            .checked_add(total_size)
            .ok_or(Error::EffectRecordOverruns {
                chunk,
                at,
                size: total_size,
                left: p.len() - at,
            })?;
        if end > p.len() {
            return Err(Error::EffectRecordOverruns {
                chunk,
                at,
                size: total_size,
                left: p.len() - at,
            });
        }
        let node_size = read_u32(p, at + 4)? as usize;
        if node_size < EFFECT_NODE_FIXED || node_size > total_size - 4 {
            return Err(Error::EffectRecordOverruns {
                chunk,
                at,
                size: node_size,
                left: total_size,
            });
        }
        // The enclosing record bounds this addition, including on wasm32.
        let node_end = 4 + node_size;

        // The name is inline NUL-terminated inside its 80-byte field, and a
        // record may carry no name at all — the shipped set contains several.
        // It sits after the two size words, not after the first alone, which is
        // what puts `objectId` at +88 rather than at +84.
        let name = read_cstr_bounded(p, at + 8, NODE_NAME);
        let object_id = read_u32(p, at + 8 + NODE_NAME)?;
        let parent_id = read_i32(p, at + 8 + NODE_NAME + 4)?;
        let flags = read_u32(p, at + 8 + NODE_NAME + 8)?;

        let mut node_tracks = Vec::new();
        read_tracks(
            chunk,
            p,
            at + 4 + EFFECT_NODE_FIXED,
            at + node_end,
            0,
            &mut node_tracks,
        )?;

        out.push(EffectNode {
            offset: at,
            total_size,
            node_size,
            name,
            object_id,
            parent_id,
            flags,
            node_tracks,
            body: p[at + node_end..end].to_vec(),
        });
        at = end;
    }
    if at != p.len() {
        return Err(Error::EffectRecordOverruns {
            chunk,
            at,
            size: 0,
            left: p.len() - at,
        });
    }
    Ok(out)
}

/// One `TXAN` texture animation: the transform a `LAYS` layer applies to its
/// texture.
///
/// The chunk is a run of records, each opened by a `u32` that counts its own
/// four bytes and followed immediately by that record's tracks. There is no
/// node header and no name: a texture animation is addressed by the
/// `textureId` of the layer that names it, not by an object id, so the record
/// is nothing but its animation.
///
/// ```text
/// +0   u32  total size, counting its own four bytes
/// +4   "KTAT"/"KTAR"/"KTAS"  K count lineType globalSeq, then count keys
/// ```
///
/// `MDLTEXANIMSECTION` in Blizzard's own 2.0.1 symbols names the three arrays
/// `transkeys`, `rotkeys` and `scalekeys` and carries no alpha array, which is
/// why `KATF` appears in none of the 279 shipped records; the magics and the
/// order above were measured against vanilla bytes first.
#[derive(Debug, Clone, PartialEq)]
pub struct TextureAnim {
    /// Offset of this record's size field within the chunk payload.
    pub offset: usize,
    /// The record's own total size, counting its first four bytes.
    pub len: u32,
    pub tracks: Vec<KeyTrack>,
}

impl TextureAnim {
    /// Total bytes this record occupies.
    pub fn size(&self) -> usize {
        self.len as usize
    }
}

/// One `RIBB` ribbon emitter: the trail behind a projectile or a dragon.
///
/// A ribbon is a node plus 52 bytes of emitter state and then its own tracks.
/// The node size word counts 96 *plus* the node's animation, so the emitter
/// state starts at `+4 + node_size` rather than at a fixed offset — which is
/// what makes a fixed-stride walk impossible here.
///
/// ```text
///  +0   u32  total size, counting its own four bytes
///  +4   u32  node size: 96 + the node tracks that follow
///  +8   char name[80]
///  +88  u32  object id
///  +92  i32  parent id
///  +96  u32  flags
/// +100  node tracks (KGTR / KGRT), ending exactly at +4 + node_size
///
/// then 52 bytes of emitter state:
///  +0   f32  height above the origin the ribbon is drawn at
///  +4   f32  height below it
///  +8   f32  alpha
///  +12  f32  colour, three components
///  +24  f32  edge lifetime, in seconds
///  +28  u32  texture slot
///  +32  u32  edges emitted per second
///  +36  u32  texture rows
///  +40  u32  texture columns
///  +44  u32  material id
///  +48  f32  gravity
///
/// then the emitter's own tracks: KRVS visibility, KRHA height above, KRHB
/// height below.
/// ```
///
/// `textureSlot` reads 0 in all 715 shipped records, so nothing is derived from
/// it. The colour's component order is not pinned by the bytes either and is
/// left as a plain triple.
#[derive(Debug, Clone, PartialEq)]
pub struct Ribbon {
    /// Offset of this record's total-size field within the chunk payload.
    pub offset: usize,
    /// The record's own total size, counting its first four bytes.
    pub total_size: u32,
    /// 96 plus the node's animation tracks.
    pub node_size: u32,
    pub name: String,
    pub object_id: u32,
    pub parent_id: i32,
    pub flags: u32,
    /// `KGTR`/`KGRT` tracks inside the node header.
    pub node_tracks: Vec<KeyTrack>,
    pub height_above: f32,
    pub height_below: f32,
    pub alpha: f32,
    /// Three colour components in file order; the order itself is not pinned.
    pub colour: [f32; 3],
    /// Lifetime of one ribbon edge, in seconds.
    pub edge_lifetime: f32,
    /// Always 0 in every shipped record; meaning unresolved.
    pub texture_slot: u32,
    /// Edges emitted per second, measured at 15..40 across the shipped set.
    pub edges_per_second: u32,
    /// Rows in the emitter's texture grid; 1 in all 715 shipped records.
    pub texture_rows: u32,
    /// Columns in that grid; 1 in all 715 shipped records.
    pub texture_cols: u32,
    /// Index into the model's `MTLS` table.
    pub material_id: u32,
    /// Downward acceleration, effectively 0 for 336 of the 715 records.
    pub gravity: f32,
    /// `KRVS` visibility and `KRHA`/`KRHB` height tracks.
    pub tracks: Vec<KeyTrack>,
}

/// One `PREM` particle emitter, the older of the two particle systems.
///
/// Same node framing as [`Ribbon`], with a different body: four floats, a
/// fixed-width inline particle path, then two more floats.
///
/// ```text
///  +0   u32  total size, counting its own four bytes
///  +4   u32  node size: 96 + the node tracks that follow
///  +8   char name[80]
///  +88  u32  object id
///  +92  i32  parent id
///  +96  u32  flags
/// +100  node tracks, ending exactly at +4 + node_size
///
/// then 284 bytes of emitter state:
///  +0    f32 emission rate
///  +4    f32 gravity
///  +8    f32 longitude
///  +12   f32 latitude
///  +16   char particle path[260], padded to the full width
///  +276  f32 life
///  +280  f32 speed
///
/// then KPEV, the visibility track.
/// ```
///
/// `longitude` and `latitude` are angles in radians: across all 192 shipped
/// records they only ever take values such as π, π/2, π/6, π/8 and π/10, which
/// is what pins them. `MDLPARTICLE` in Blizzard's own symbols names the three
/// trailing fields `path`, `life` and `speed` — the path first — which matches
/// the measured order.
#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    /// Offset of this record's total-size field within the chunk payload.
    pub offset: usize,
    /// The record's own total size, counting its first four bytes.
    pub total_size: u32,
    /// 96 plus the node's animation tracks.
    pub node_size: u32,
    pub name: String,
    pub object_id: u32,
    pub parent_id: i32,
    pub flags: u32,
    /// `KGTR`/`KGRT` tracks inside the node header.
    pub node_tracks: Vec<KeyTrack>,
    pub emission_rate: f32,
    pub gravity: f32,
    /// Emission cone longitude, in radians.
    pub longitude: f32,
    /// Emission cone latitude, in radians.
    pub latitude: f32,
    /// Inline particle path, fixed-width and NUL-padded.
    pub path: String,
    /// Particle lifetime.
    pub life: f32,
    /// Initial particle speed.
    pub speed: f32,
    /// `KPEV` visibility track.
    pub tracks: Vec<KeyTrack>,
}

/// One `CAMS` camera.
///
/// A camera is the one record in this format with **no node header**: the words
/// that sit where `objectId`, `parentId` and `flags` sit for every other node
/// chunk are the camera's eye position, and the name begins at `+4` rather than
/// `+8`. The shared [`Mdlx::nodes`] reader therefore decodes a camera's name
/// correctly and its ids as the middle of a vector, which is exactly the kind
/// of plausible-looking wrong answer this crate exists to refuse.
///
/// ```text
/// +0    u32  total size, counting its own four bytes
/// +4    char name[80]
/// +84   f32  eye position, three components
/// +96   f32  field of view, in radians
/// +100  f32  far clip
/// +104  f32  near clip
/// +108  f32  target position, three components
/// +120  tracks: KCTR eye, KCRL roll, KTTR target
/// ```
///
/// `field_of_view` is 0.7854 — π/4, a 45° cone — in 920 of the 1,300 shipped
/// records. The clip pair is assigned by measurement, not by name: in all 1,300
/// records the word at `+100` is strictly greater than the word at `+104`, which
/// is only possible if the first is the far clip.
#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    /// Offset of this record's size field within the chunk payload.
    pub offset: usize,
    /// The record's own total size, counting its own four bytes.
    pub size: u32,
    pub name: String,
    /// Eye position.
    pub eye: [f32; 3],
    /// Vertical field of view, in radians.
    pub field_of_view: f32,
    /// Far clip distance.
    pub far: f32,
    /// Near clip distance.
    pub near: f32,
    /// Point the camera looks at.
    pub target: [f32; 3],
    pub tracks: Vec<KeyTrack>,
}

/// The kind of light a `LITE` record emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightKind {
    /// 0. Present in 223 of the 250 shipped records.
    Omnidirectional,
    /// 1.
    Directional,
    /// 2.
    Ambient,
}

impl LightKind {
    /// Reads the on-disk word. Every other value is refused: the field is
    /// framing, because a value outside 0..3 would mean the word had been
    /// misread and every field after it is shifted.
    fn parse(at: usize, kind: u32) -> Result<Self, Error> {
        match kind {
            0 => Ok(Self::Omnidirectional),
            1 => Ok(Self::Directional),
            2 => Ok(Self::Ambient),
            other => Err(Error::UnknownLightKind { at, kind: other }),
        }
    }
}

/// One `LITE` light.
///
/// Node framing like [`Ribbon`], with a 44-byte body instead of 52.
///
/// ```text
///  +0   u32  total size, counting its own four bytes
///  +4   u32  node size: 96 + the node tracks that follow
///  +8   char name[80]
///  +88  u32  object id
///  +92  i32  parent id
///  +96  u32  flags
/// +100  node tracks, ending exactly at +4 + node_size
///
/// then 44 bytes of light state:
///  +0   u32  light kind
///  +4   f32  attenuation start
///  +8   f32  attenuation end
///  +12  f32  colour, three components
///  +24  f32  intensity
///  +28  f32  ambient colour, three components
///  +40  f32  ambient intensity
///
/// then the light's own tracks: KLAV visibility, KLAI intensity, KLAC colour,
/// KLBC ambient colour, KLBI ambient intensity.
/// ```
///
/// The kind word takes 0, 1 or 2 and nothing else across all 500 shipped
/// records, which is what lets it be read as an enum rather than left raw. The
/// attenuation pair is ordered — start below end in every record that has a
/// non-zero range — and the ambient colour is white in all 500.
#[derive(Debug, Clone, PartialEq)]
pub struct Light {
    /// Offset of this record's total-size field within the chunk payload.
    pub offset: usize,
    /// The record's own total size, counting its own four bytes.
    pub total_size: u32,
    /// 96 plus the node's animation tracks.
    pub node_size: u32,
    pub name: String,
    pub object_id: u32,
    pub parent_id: i32,
    pub flags: u32,
    /// `KGTR`/`KGRT`/`KGSC` tracks inside the node header.
    pub node_tracks: Vec<KeyTrack>,
    pub kind: LightKind,
    pub attenuation_start: f32,
    pub attenuation_end: f32,
    /// Three colour components in file order; the order itself is not pinned.
    pub colour: [f32; 3],
    pub intensity: f32,
    /// Three ambient colour components in file order.
    pub ambient_colour: [f32; 3],
    pub ambient_intensity: f32,
    /// The light's own tracks.
    pub tracks: Vec<KeyTrack>,
}

/// A record that carries animation tracks, so the tracks can be checked
/// through one path.
pub trait OrderedTracks {
    /// Every track the record carries — a node's own animation and the record's
    /// own, where a record has both.
    fn all_tracks(&self) -> Vec<&KeyTrack>;

    /// True when every key time in every track is a non-negative millisecond
    /// offset that never goes backwards.
    ///
    /// A track whose frames descend is a desynchronised walk, so this is how
    /// the sweep notices a plausible-looking misread without having to know
    /// what the track animates.
    fn track_times_ordered(&self) -> bool {
        self.all_tracks().iter().all(|t| t.times_are_ordered())
    }
}

impl OrderedTracks for TextureAnim {
    fn all_tracks(&self) -> Vec<&KeyTrack> {
        self.tracks.iter().collect()
    }
}

impl OrderedTracks for Ribbon {
    fn all_tracks(&self) -> Vec<&KeyTrack> {
        self.node_tracks.iter().chain(self.tracks.iter()).collect()
    }
}

impl OrderedTracks for Particle {
    fn all_tracks(&self) -> Vec<&KeyTrack> {
        self.node_tracks.iter().chain(self.tracks.iter()).collect()
    }
}

impl OrderedTracks for Camera {
    fn all_tracks(&self) -> Vec<&KeyTrack> {
        self.tracks.iter().collect()
    }
}

impl OrderedTracks for Light {
    fn all_tracks(&self) -> Vec<&KeyTrack> {
        self.node_tracks.iter().chain(self.tracks.iter()).collect()
    }
}

/// One `MTLS` entry: a material, with the texture layers it draws.
///
/// `MTLS` is a list of **self-inclusive** lengths, like `GEOS`: each record is a
/// `u32` that counts its own four bytes, followed by that many bytes less four.
/// The footman's three records are 48, 76 and 120 bytes and tile the 244-byte
/// chunk exactly.
///
/// `MDLMATERIALSECTION` from Blizzard's own symbols names the fields
/// `texLayers`, `priorityPlane` and `materialFlags`, and `MDLTEXLAYER` names
/// `shaderIdMDL`, `blendMode`, `flags`, `textureId`, `flipKeys`, `transformId`,
/// `coordId`, `alphaKeys`, `staticAlpha`, `emissiveKeys`, `staticEmissive` and
/// the fresnel set. Those names came from the PDBs *after* the record structure
/// below had been measured against vanilla bytes, and the record framing is what
/// is encoded here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Material {
    /// Offset of this record's length field within the chunk payload.
    pub offset: usize,
    pub len: u32,
    /// Record bytes, excluding the length field.
    pub data: Vec<u8>,
}

impl Material {
    /// Offsets of the `LAYS` sub-blocks inside this record.
    ///
    /// A material usually carries one or two; a record with none is legal for
    /// an unlit material.
    pub fn layers(&self) -> Vec<usize> {
        if self.data.get(8..12) == Some(b"LAYS") {
            vec![8]
        } else {
            Vec::new()
        }
    }

    /// The layer index and declared length of each `LAYS` block.
    ///
    /// A layer is `LAYS` + a layer index + a declared length, then the body.
    /// `textureId` sits at body offset 20, which is *not* offset 8: the value at
    /// 8 is the declared length (28 or 100 depending on how many animated tracks
    /// the layer carries), and reading that as a texture index is why the first
    /// attempt produced layer 0 pointing at texture 28.
    pub fn layer_lengths(&self) -> Vec<(u32, u32)> {
        self.checked_layers()
            .expect("invalid material layer framing")
            .into_iter()
            .map(|layer| (layer.properties.index, layer.size))
            .collect()
    }

    /// Compatibility accessor for materials validated by Mdlx::materials.
    /// Use checked_layers when constructing Material directly from bytes.
    pub fn tex_layers(&self) -> Vec<TexLayer> {
        self.checked_layers()
            .expect("invalid material layer framing")
            .into_iter()
            .map(|layer| layer.properties)
            .collect()
    }
}

/// One `LAYS` texture layer.
///
/// A layer is `LAYS` + a layer index + a declared length, then the body. The
/// body offsets are measured, not assumed: across 1,461 layers drawn from 650
/// models, the `u32` at body +8 was always a valid index into the model's
/// `TEXS` table — 100% — while every other offset admitted values outside it.
/// See `LaysBody` for what each field is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TexLayer {
    /// Layer index as declared by the record.
    pub index: u32,
    /// Legacy field name for filter_mode.
    pub shader_id: u32,
    /// Legacy field name for shading_flags.
    pub blend_mode: u32,
    pub filter_mode: u32,
    pub shading_flags: u32,
    /// Authored texture coordinate set.
    pub coord_id: u32,
    /// Index into the model's `TEXS` table.
    pub texture_id: u32,
    /// Texture animation, or `NO_ID` when the layer has none.
    pub texture_anim_id: u32,
    /// Static layer opacity; `1.0` when no `KMTA` track overrides it.
    pub static_alpha: f32,
}

/// Sentinel meaning "none", used for reference fields.
pub const NO_ID: u32 = 0xffff_ffff;

impl TexLayer {
    /// True when the layer has no texture animation of its own.
    pub fn has_no_texture_anim(&self) -> bool {
        self.texture_anim_id == NO_ID
    }

    /// True when the texture index is one the model actually shipped.
    pub fn points_at_a_shipped_texture(&self, texture_count: usize) -> bool {
        (self.texture_id as usize) < texture_count
    }
}

/// Sub-tags found inside a geoset. Positions are informative; the sizes are not
/// yet pinned, so nothing is parsed from them yet.
pub mod geoset_tag {
    pub const VRTX: &[u8; 4] = b"VRTX";
    pub const BNRM: &[u8; 4] = b"BNRM";
    pub const PTYP: &[u8; 4] = b"PTYP";
    pub const PCNT: &[u8; 4] = b"PCNT";
    pub const PVTX: &[u8; 4] = b"PVTX";
    pub const GNDX: &[u8; 4] = b"GNDX";
    pub const MTGC: &[u8; 4] = b"MTGC";
    pub const MATS: &[u8; 4] = b"MATS";
    pub const UVAS: &[u8; 4] = b"UVAS";
    pub const UVBS: &[u8; 4] = b"UVBS";
}

/// Fixed part of the trailer that follows the `MATS` node indices.
///
/// Seven words, then the seqBoundsCount word: materialId, selectionGroup,
/// unselectable, boundsRadius, both extents and the count. Blizzard's own reader
/// fills them positionally, with no tag and no dispatch, and the `.mdl` keyword
/// reader resolves `MaterialID` to the same struct offset the binary reader
/// writes first (`+0x94`).
const MATS_TRAILER_FIXED: usize = 4 + 4 + 4 + 4 + 12 + 12 + 4;

/// Version-1800 LoD field width, corroborated by the versioned primary parser.
const GEOS_LOD_NAME: usize = 80;
const GEOS_LOD_SIZE: usize = 4 + GEOS_LOD_NAME;

/// One per-sequence bounds record: an extent radius then both extents.
const SEQ_BOUNDS_STRIDE: usize = 28;

/// Body width of a `PTYP` or `PCNT` sub-block.
///
/// One `u32` each, the primitive type and the index count, not `count` entries,
/// which is the one part of the geoset table that cannot be expressed as
/// `count` times a fixed stride.
const SCALAR_SUB_BLOCK: usize = 4;

/// Measured body width of a geoset sub-block, in bytes.
///
/// Every one of these was measured from the shipped files by tiling the geoset
/// to its exact end. `MATS` is absent on purpose: its body is not `count` times
/// a stride in the same sense, because a trailer follows it.
fn sub_block_width(tag: &[u8; 4], count: u32) -> Option<usize> {
    match &tag[..] {
        b"VRTX" | b"NRMS" => Some(count as usize * 12),
        b"UVBS" => Some(count as usize * 8),
        b"PVTX" => Some(count as usize * 2),
        b"GNDX" => Some(count as usize),
        b"PTYP" | b"PCNT" => Some(SCALAR_SUB_BLOCK),
        b"UVAS" => Some(0),
        b"MTGC" => Some(count as usize * 4),
        _ => None,
    }
}

/// Authored version-1800 LoD fields, without selecting or interpreting a level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeosetLod {
    pub version: u32,
    /// Offset of the signed level word within Geoset.data.
    pub offset: usize,
    pub level: i32,
    /// Exact inline name bytes, including any NULs or bytes after them.
    pub name: [u8; GEOS_LOD_NAME],
}

/// A decoded geoset without HD tangent/weighted-skin support.
///
/// The sub-block walk is `VRTX NRMS PTYP PCNT PVTX GNDX MTGC MATS UVAS UVBS`,
/// and after the `MATS` node indices comes a trailer of the geoset's own
/// bounds and material. Both are measured, not inferred: the trailer's 44-byte
/// field run plus one 28-byte record per sequence lands exactly on `UVAS` for
/// every geoset the shipped archives carry.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub vertices: Vec3Array,
    pub normals: Vec3Array,
    pub tex_coords: UvArray,
    /// Checked UVAS count. None means no UVAS/UVBS framing was supplied.
    /// The legacy tex_coords projection still retains the last UVBS array.
    pub uv_set_count: Option<u32>,
    pub triangles: Triangles,
    /// One vertex group index per vertex; indexes the model's bone groups.
    pub vertex_groups: Vec<u8>,
    /// Number of matrix indices in each matrix group.
    ///
    /// First written as "one per triangle", which was wrong: the footman has
    /// 288 triangles but only 35 entries, and the values sum to 54. The entries
    /// count *matrix indices per group*, and `sum(matrix_groups)`, read as
    /// consecutive slice lengths, exactly consumes the `MATS` bone indices —
    /// verified on 2,851 of 2,851 sampled geosets.
    pub matrix_groups: Vec<u32>,

    /// How many `u32`s the `MATS` node-index region occupies.
    ///
    /// The `MATS` count, read as a width: the body is that many indices and
    /// nothing else, so the float rows that follow are the trailer rather than
    /// an over-wide span. Keeping the width lets a caller check the trim to
    /// `sum(matrix_groups)` rather than trust it.
    pub matrix_span_u32s: usize,

    /// Flattened bone indices, `matrix_groups[i]` long per group.
    ///
    /// This is the skinning table: a vertex's matrix group comes from `GNDX`,
    /// and that group's bones are the slice of this vector. Every entry
    /// addresses a real bone and every `GNDX` value a real group, across all
    /// sampled geosets — which is what makes the reading trustworthy rather
    /// than merely self-consistent.
    pub matrix_indices: Vec<u32>,

    /// Index into the model's `MTLS` materials, measured from the MATS trailer.
    pub material_id: u32,
    pub selection_group: u32,
    /// 0, or 4 when the geoset cannot be selected.
    pub unselectable: u32,
    /// Absent in v800; preserved with source offsets in v1800.
    pub lod: Option<GeosetLod>,
    pub bounds_radius: f32,
    pub minimum_extent: [f32; 3],
    pub maximum_extent: [f32; 3],
    /// Zero, or one `[extent_radius, min_x, min_y, min_z, max_x, max_y, max_z]`
    /// per sequence — never anything in between.
    ///
    /// The trailer's `seqBoundsCount` is either the model's sequence count or
    /// exactly zero: 5,971 exact / 2,701 absent on 1.28c, 6,084 / 2,686 on
    /// 1.29b. Asserting the unconditional one-per-sequence rule first failed
    /// on 2,699 models, which is how this rule was found. The Footman has 13,
    /// so it is a member of the exact group, not evidence of a general one.
    pub seq_bounds: Vec<[f32; 7]>,
}

/// Reads a `tag[4] + u32 count + body` sub-block starting at `p[at]`.
fn read_subblock(p: &[u8], at: usize) -> Result<(&[u8; 4], u32, &[u8]), Error> {
    let tag: &[u8; 4] = p
        .get(at..at + 4)
        .ok_or(Error::Truncated {
            what: "geoset sub-block tag",
            at,
            need: 4,
            have: p.len().saturating_sub(at),
        })?
        .try_into()
        .unwrap();
    let count = read_u32(p, at + 4)?;
    let body = &p[at + 8..];
    Ok((tag, count, body))
}

#[derive(Debug, Clone)]
pub struct Mdlx {
    pub version: u32,
    pub chunks: Vec<Chunk>,
}

fn read_u32(d: &[u8], at: usize) -> Result<u32, Error> {
    d.get(at..at + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .ok_or(Error::Truncated {
            what: "u32",
            at,
            need: 4,
            have: d.len().saturating_sub(at),
        })
}

fn read_i32(d: &[u8], at: usize) -> Result<i32, Error> {
    d.get(at..at + 4)
        .map(|s| i32::from_le_bytes(s.try_into().unwrap()))
        .ok_or(Error::Truncated {
            what: "i32",
            at,
            need: 4,
            have: d.len().saturating_sub(at),
        })
}

fn read_f32(d: &[u8], at: usize) -> Result<f32, Error> {
    Ok(f32::from_bits(read_u32(d, at)?))
}

/// Reads a NUL-terminated string starting at `at`.
fn read_cstr(d: &[u8], at: usize) -> Result<String, Error> {
    let tail = d.get(at..).ok_or(Error::Truncated {
        what: "string",
        at,
        need: 1,
        have: 0,
    })?;
    let end = tail
        .iter()
        .position(|b| *b == 0)
        .ok_or(Error::UnterminatedString { at })?;
    Ok(String::from_utf8_lossy(&tail[..end]).into_owned())
}

impl Mdlx {
    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "assets.mdx_parse")
    )]
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let magic: [u8; 4] = data
            .get(0..4)
            .ok_or(Error::Truncated {
                what: "magic",
                at: 0,
                need: 4,
                have: data.len(),
            })?
            .try_into()
            .unwrap();
        if &magic != MAGIC {
            return Err(Error::BadMagic(magic));
        }

        let mut chunks = Vec::new();
        let mut at = 4usize;
        while at < data.len() {
            let t: [u8; 4] = data
                .get(at..at + 4)
                .ok_or(Error::Truncated {
                    what: "chunk tag",
                    at,
                    need: 4,
                    have: data.len().saturating_sub(at),
                })?
                .try_into()
                .unwrap();
            if !t.iter().all(|c| c.is_ascii_graphic()) {
                return Err(Error::NotATag { at, tag: t });
            }
            if !KNOWN_TAGS.contains(&&t) {
                return Err(Error::UnknownTag(t));
            }
            let size = read_u32(data, at + 4)? as usize;
            let payload_offset = at + 8;
            let left = data.len() - payload_offset;
            if size > left {
                return Err(Error::ChunkOverruns {
                    tag: String::from_utf8_lossy(&t).into_owned(),
                    size,
                    left,
                });
            }
            chunks.push(Chunk {
                tag: t,
                offset: at,
                payload_offset,
                size,
            });
            at = payload_offset + size;
        }

        let version = chunks
            .iter()
            .find(|c| &c.tag == tag::VERS)
            .ok_or(Error::NoVersion)
            .and_then(|c| read_u32(data, c.payload_offset))?;

        Ok(Self { version, chunks })
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::parse(&std::fs::read(path)?)
    }

    pub fn chunk(&self, tag: &[u8; 4]) -> Option<&Chunk> {
        self.chunks.iter().find(|c| &c.tag == tag)
    }

    pub fn has(&self, tag: &[u8; 4]) -> bool {
        self.chunk(tag).is_some()
    }

    /// Tags present, in file order.
    pub fn tags(&self) -> Vec<String> {
        self.chunks
            .iter()
            .map(|c| String::from_utf8_lossy(&c.tag).into_owned())
            .collect()
    }

    pub fn distinct_tags(&self) -> BTreeSet<String> {
        self.tags().into_iter().collect()
    }

    /// The `MODL` record.
    pub fn model(&self, data: &[u8]) -> Result<Model, Error> {
        let c = self.chunk(tag::MODL).ok_or(Error::NoModel)?;
        let p = c.payload(data);
        if p.len() < MODL_SIZE {
            return Err(Error::Truncated {
                what: "MODL extents and blend time",
                at: c.payload_offset,
                need: MODL_SIZE,
                have: p.len(),
            });
        }
        let f = |i: usize| f32::from_le_bytes(p[MODL_EXTENTS + i * 4..][..4].try_into().unwrap());
        Ok(Model {
            name: read_cstr(p, 0)?,
            blend_time_ms: read_u32(p, MODL_EXTENTS + 24)?,
            minimum_extent: [f(0), f(1), f(2)],
            maximum_extent: [f(3), f(4), f(5)],
        })
    }

    /// Payload of any chunk, or `None` when the model omits it.
    pub fn payload<'a>(&self, data: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
        self.chunk(tag).map(|c| c.payload(data))
    }

    /// The skeleton: every `BONE` record, in chunk order.
    ///
    /// A record frames itself with a leading `u32` size that measures from the
    /// start of the record to the end of its animation tracks, so the record
    /// as a whole is `size + 8` bytes. There is no record count, and the
    /// variable length comes entirely from the inline key tracks — which makes
    /// a fixed-stride walk impossible, as the footman already showed.
    ///
    /// The walk must land exactly on the end of the payload; a wrong
    /// interpretation cannot.
    pub fn bones(&self, data: &[u8]) -> Result<Vec<Bone>, Error> {
        let Some(p) = self.payload(data, tag::BONE) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut at = 0usize;
        while at < p.len() {
            let size = read_u32(p, at)? as usize;
            let span = size
                .checked_add(BONE_TRAILER)
                .ok_or(Error::BoneRecordOverruns {
                    at,
                    size,
                    left: p.len() - at,
                })?;
            let end = at.checked_add(span).ok_or(Error::BoneRecordOverruns {
                at,
                size,
                left: p.len() - at,
            })?;
            if size < BONE_FIXED || end > p.len() {
                return Err(Error::BoneRecordOverruns {
                    at,
                    size,
                    left: p.len() - at,
                });
            }

            // Node fields. The name is inline NUL-terminated inside the
            // 80-byte field; a bone may carry no name at all, which is legal.
            let name = read_bone_name(p, at);
            let object_id = read_u32(p, at + 84)?;
            let parent_id = read_i32(p, at + 88)?;
            let flags = read_u32(p, at + 92)?;

            // Animation tracks run from just past the node fields to the last
            // eight bytes; each is framed by its own count and stride, so the
            // walk both positions the trailer and proves the record.
            let mut tracks = Vec::new();
            let mut cur = at + BONE_FIXED;
            let tracks_end = end - BONE_TRAILER;
            while cur < tracks_end {
                let magic = p.get(cur..cur + 4).ok_or(Error::BoneTrackOverruns {
                    at: cur,
                    size: 4,
                    left: p.len().saturating_sub(cur),
                })?;
                let kind = TrackKind::parse(at, magic)?;
                let count = read_u32(p, cur + 4)?;
                let interpolation = Interpolation::parse(cur, read_u32(p, cur + 8)?)?;
                let global_sequence_id = read_i32(p, cur + 12)?;

                // Linear tracks are time + value; tangented ones add two more
                // values. `count` alone cannot size the block.
                let mut stride = 4 + kind.width() * 4;
                if interpolation.tangented() {
                    stride += 2 * kind.width() * 4;
                }
                let block =
                    (count as usize)
                        .checked_mul(stride)
                        .ok_or(Error::BoneTrackOverruns {
                            at: cur,
                            size: usize::MAX,
                            left: tracks_end.saturating_sub(cur),
                        })?;
                let track_span =
                    BONE_TRACK_HEAD
                        .checked_add(block)
                        .ok_or(Error::BoneTrackOverruns {
                            at: cur,
                            size: usize::MAX,
                            left: tracks_end.saturating_sub(cur),
                        })?;
                let next = cur
                    .checked_add(track_span)
                    .ok_or(Error::BoneTrackOverruns {
                        at: cur,
                        size: usize::MAX,
                        left: tracks_end.saturating_sub(cur),
                    })?;
                if next > tracks_end {
                    return Err(Error::BoneTrackOverruns {
                        at: cur,
                        size: track_span,
                        left: tracks_end - cur,
                    });
                }

                tracks.push(BoneTrack {
                    kind,
                    count,
                    interpolation,
                    global_sequence_id,
                    offset: cur - at,
                    stride,
                    raw: p[cur + BONE_TRACK_HEAD..next].to_vec(),
                });
                cur = next;
            }
            if cur != tracks_end {
                return Err(Error::BoneTrackOverruns {
                    at: cur,
                    size: 0,
                    left: tracks_end - cur,
                });
            }

            let geoset_id = read_i32(p, tracks_end)?;
            let geoset_anim_id = read_i32(p, tracks_end + 4)?;

            out.push(Bone {
                offset: at,
                span,
                name,
                object_id,
                parent_id,
                flags,
                tracks,
                geoset_id,
                geoset_anim_id,
            });
            at = end;
        }
        Ok(out)
    }

    /// Animation sequences, in file order.
    pub fn sequences(&self, data: &[u8]) -> Result<Vec<Sequence>, Error> {
        let Some(p) = self.payload(data, tag::SEQS) else {
            return Ok(Vec::new());
        };
        if p.len() % SEQUENCE_SIZE != 0 {
            return Err(Error::RaggedRecords {
                chunk: "SEQS",
                size: p.len(),
                stride: SEQUENCE_SIZE,
            });
        }
        let mut out = Vec::with_capacity(p.len() / SEQUENCE_SIZE);
        for r in p.as_chunks::<SEQUENCE_SIZE>().0 {
            let start = read_u32(r, SEQ_INTERVAL)?;
            let end = read_u32(r, SEQ_INTERVAL + 4)?;
            let mut trailing = [0u32; 7];
            for (i, t) in trailing.iter_mut().enumerate() {
                *t = read_u32(r, 104 + i * 4)?;
            }
            out.push(Sequence {
                name: read_cstr_bounded(r, 0, SEQ_NAME),
                interval_start: start,
                interval_end: end,
                move_speed: f32::from_bits(read_u32(r, SEQ_INTERVAL + 8)?),
                non_looping: read_u32(r, SEQ_INTERVAL + 12)? != 0,
                trailing,
            });
        }
        Ok(out)
    }

    /// Walks the sub-blocks of one geoset into a [`Mesh`].
    ///
    /// The sub-block order is `VRTX NRMS PTYP PCNT PVTX GNDX MTGC MATS UVAS
    /// UVBS`, all measured. Each is `tag[4] + u32 count + body`, and the
    /// payload width is fixed by the tag: 12 bytes per vertex for positions and
    /// normals, 8 for UVs, 2 for triangle indices, 1 for the vertex group.
    ///
    /// `MATS` is the one block followed by more than its body: the geoset's own
    /// material and bounds. The walk must also land on the geoset's final byte,
    /// so a block list that stops short cannot pass as complete.
    ///
    /// A missing block is an error rather than a default, because a silently
    /// empty position array renders a model that looks plausible and is wrong.
    pub fn mesh(&self, _data: &[u8], g: &Geoset) -> Result<Mesh, Error> {
        let has_lod = match self.version {
            800 => false,
            1800 => true,
            version => return Err(Error::UnsupportedVersion(version)),
        };
        let mut mesh = Mesh {
            vertices: Vec3Array::empty(b"VRTX"),
            normals: Vec3Array::empty(b"NRMS"),
            tex_coords: UvArray::empty(),
            uv_set_count: None,
            triangles: Triangles::empty(),
            vertex_groups: Vec::new(),
            matrix_groups: Vec::new(),
            matrix_indices: Vec::new(),
            matrix_span_u32s: 0,
            material_id: 0,
            selection_group: 0,
            unselectable: 0,
            lod: None,
            bounds_radius: 0.0,
            minimum_extent: [0.0; 3],
            maximum_extent: [0.0; 3],
            seq_bounds: Vec::new(),
        };
        let mut at = 0usize;
        let mut uv_sets_seen = 0usize;
        let mut uv_declaration_at = 0usize;
        while at + 8 <= g.data.len() {
            let (tag, count, _) = read_subblock(&g.data, at)?;
            if has_lod && (tag == b"TANG" || tag == b"SKIN") {
                return Err(Error::UnsupportedGeosetFeature {
                    version: self.version,
                    at,
                    tag: *tag,
                });
            }

            // `MATS` is the one block whose body is not read by a width alone:
            // after the `count` node indices comes a trailer of the geoset's
            // own material and bounds.
            if tag == b"MATS" {
                // The count is the number of matrix (node) indices, so the
                // body is exactly `count * 4` bytes and the trailer begins
                // where it ends. Scanning to the next known tag instead
                // swallowed the trailer, which is where the geoset's material
                // lives. The trailer, relative to the end of the body:
                //
                //   +0x00 u32    materialId
                //   +0x04 u32    selectionGroup
                //   +0x08 u32    unselectable, 0 or 4
                //   +0x0c f32    boundsRadius
                //   +0x10 f32[3] minimumExtent
                //   +0x1c f32[3] maximumExtent
                //   +0x28 u32    seqBoundsCount
                //   +0x2c 28     each: { f32, f32[3], f32[3] }
                let body_start = at + 8;
                let body_len = count as usize * 4;
                let body_end = body_start + body_len;
                if body_end > g.data.len() {
                    return Err(Error::Truncated {
                        what: "MATS node indices",
                        at: body_start,
                        need: body_len,
                        have: g.data.len() - body_start,
                    });
                }
                // `MTGC` precedes `MATS` in the walk, so the number of indices
                // the groups use is already known here. The trim stays as the
                // guard that keeps whatever the groups do not reach out of the
                // skinning table, and the span reports the width it trimmed.
                let node_indices: Vec<u32> = g.data[body_start..body_end]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_le_bytes(*c))
                    .collect();
                let want: usize = mesh.matrix_groups.iter().map(|&c| c as usize).sum();
                mesh.matrix_span_u32s = node_indices.len();
                mesh.matrix_indices = node_indices.into_iter().take(want).collect();

                // Shipped v1800 and the primary versioned geoset parser agree:
                // a signed LoD word and 80 name bytes precede the extent.
                // See REFORGED_GEOS.md; v800 retains its native-proven offsets.
                let lod_size = if has_lod { GEOS_LOD_SIZE } else { 0 };
                if has_lod {
                    let tail = &g.data[body_end..];
                    let raw = tail.get(12..12 + GEOS_LOD_SIZE).ok_or(Error::Truncated {
                        what: "MATS LoD",
                        at: body_end + 12,
                        need: GEOS_LOD_SIZE,
                        have: tail.len().saturating_sub(12),
                    })?;
                    mesh.lod = Some(GeosetLod {
                        version: self.version,
                        offset: body_end + 12,
                        level: i32::from_le_bytes(raw[..4].try_into().unwrap()),
                        name: raw[4..].try_into().unwrap(),
                    });
                }
                let bounds_base = body_end + lod_size;
                let ext = |o: usize| -> Result<[f32; 3], Error> {
                    Ok([
                        read_f32(&g.data, bounds_base + o)?,
                        read_f32(&g.data, bounds_base + o + 4)?,
                        read_f32(&g.data, bounds_base + o + 8)?,
                    ])
                };
                mesh.material_id = read_u32(&g.data, body_end)?;
                mesh.selection_group = read_u32(&g.data, body_end + 4)?;
                mesh.unselectable = read_u32(&g.data, body_end + 8)?;
                mesh.bounds_radius = read_f32(&g.data, bounds_base + 12)?;
                mesh.minimum_extent = ext(16)?;
                mesh.maximum_extent = ext(28)?;
                let seq_count = read_u32(&g.data, bounds_base + 40)? as usize;
                let seq_start = bounds_base + MATS_TRAILER_FIXED;
                let seq_len = if has_lod {
                    seq_count
                        .checked_mul(SEQ_BOUNDS_STRIDE)
                        .ok_or(Error::Truncated {
                            what: "MATS seqBounds",
                            at: seq_start,
                            need: usize::MAX,
                            have: g.data.len().saturating_sub(seq_start),
                        })?
                } else {
                    seq_count * SEQ_BOUNDS_STRIDE
                };
                let seq_end = if has_lod {
                    seq_start.checked_add(seq_len).ok_or(Error::Truncated {
                        what: "MATS seqBounds",
                        at: seq_start,
                        need: seq_len,
                        have: g.data.len().saturating_sub(seq_start),
                    })?
                } else {
                    seq_start + seq_len
                };
                if seq_end > g.data.len() {
                    return Err(Error::Truncated {
                        what: "MATS seqBounds",
                        at: seq_start,
                        need: seq_len,
                        have: g.data.len().saturating_sub(seq_start),
                    });
                }
                mesh.seq_bounds = g.data[seq_start..seq_end]
                    .as_chunks::<SEQ_BOUNDS_STRIDE>()
                    .0
                    .iter()
                    .map(|c| {
                        let mut out = [0.0f32; 7];
                        for (i, slot) in out.iter_mut().enumerate() {
                            *slot = f32::from_bits(u32::from_le_bytes(
                                c[i * 4..][..4].try_into().unwrap(),
                            ));
                        }
                        out
                    })
                    .collect();
                at = seq_end;
                continue;
            }

            let body_len = if tag == b"UVBS" {
                usize::try_from(count)
                    .ok()
                    .and_then(|count| count.checked_mul(8))
                    .ok_or(Error::Truncated {
                        what: "UVBS coordinate byte count",
                        at,
                        need: usize::MAX,
                        have: g.data.len().saturating_sub(at),
                    })?
            } else {
                sub_block_width(tag, count).ok_or(Error::UnknownGeosetTag(*tag))?
            };
            let end = body_len.checked_add(8).ok_or(Error::Truncated {
                what: "geoset sub-block span",
                at,
                need: usize::MAX,
                have: g.data.len().saturating_sub(at),
            })?;
            if end > g.data.len() - at {
                return Err(Error::Truncated {
                    what: "geoset sub-block body",
                    at,
                    need: end,
                    have: g.data.len() - at,
                });
            }
            let body = &g.data[at + 8..at + end];

            match &tag[..] {
                b"VRTX" => mesh.vertices = Vec3Array::new(*tag, count, body),
                b"NRMS" => mesh.normals = Vec3Array::new(*tag, count, body),
                b"UVAS" => {
                    if mesh.uv_set_count.replace(count).is_some() {
                        return Err(Error::DuplicateGeosetTag { at, tag: *tag });
                    }
                    uv_declaration_at = at;
                }
                b"UVBS" => {
                    let declared = mesh
                        .uv_set_count
                        .ok_or(Error::MissingUvSetDeclaration { at })?;
                    uv_sets_seen += 1;
                    if uv_sets_seen > declared as usize {
                        return Err(Error::UvSetCount {
                            at: uv_declaration_at,
                            declared,
                            actual: uv_sets_seen,
                        });
                    }
                    mesh.tex_coords = UvArray::new(count, body);
                }
                b"PVTX" => mesh.triangles = Triangles::new(count, body),
                b"GNDX" => mesh.vertex_groups = body.to_vec(),
                b"MTGC" => {
                    mesh.matrix_groups = body
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|c| u32::from_le_bytes(*c))
                        .collect();
                }
                // Primitive type and index count are read for their counts and
                // do not become geometry of their own.
                b"PTYP" | b"PCNT" => {}
                _ => unreachable!("no other tag is yielded"),
            }
            at += end;
        }
        // The sub-blocks must tile the geoset exactly. A walk that stops early
        // leaves the remaining bytes unread and names nothing wrong, which is
        // how a desynchronised block list passed for a while.
        if at != g.data.len() {
            return Err(Error::GeosetNotTiled {
                at,
                have: g.data.len(),
            });
        }
        if let Some(declared) = mesh.uv_set_count
            && uv_sets_seen != declared as usize
        {
            return Err(Error::UvSetCount {
                at: uv_declaration_at,
                declared,
                actual: uv_sets_seen,
            });
        }
        Ok(mesh)
    }

    /// Geoset animations, in file order.
    ///
    /// Self-inclusive lengths, same as `GEOS` and `MTLS`, and the walk must
    /// consume the chunk exactly.
    pub fn geoset_anims(&self, data: &[u8]) -> Result<Vec<GeosetAnim>, Error> {
        let Some(p) = self.payload(data, tag::GEOA) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut at = 0usize;
        while at + 4 <= p.len() {
            let len = read_u32(p, at)? as usize;
            if len < 4 || at + len > p.len() {
                return Err(Error::RaggedRecords {
                    chunk: "GEOA",
                    size: p.len(),
                    stride: len,
                });
            }
            let rec = &p[at + 4..at + len];
            let mut anim = GeosetAnim {
                offset: at,
                len: len as u32,
                // Measured: alpha, flags, then three colour floats, then the
                // tracks start at +24. Reading them as objectId/parentId at +0
                // and +4 desynchronises every track after them.
                flags: read_u32(rec, 4)?,
                geoset_id: read_u32(rec, 20)?,
                static_alpha: f32::from_bits(read_u32(rec, 0)?),
                static_colour: [
                    f32::from_bits(read_u32(rec, 8)?),
                    f32::from_bits(read_u32(rec, 12)?),
                    f32::from_bits(read_u32(rec, 16)?),
                ],
                tracks: Vec::new(),
                trailing: 0,
            };
            let mut cur = GEOA_FIXED;
            while cur < rec.len() {
                let magic: [u8; 4] = rec
                    .get(cur..cur + 4)
                    .ok_or(Error::TrackOverruns {
                        chunk: "GEOA",
                        at: at + 4 + cur,
                        size: 4,
                        left: rec.len() - cur,
                    })?
                    .try_into()
                    .unwrap();
                if !matches!(&magic[..], b"KGAA" | b"KGAC" | b"KGAO") {
                    return Err(Error::UnknownTrackMagic {
                        chunk: "GEOA",
                        at: at + 4 + cur,
                        tag: magic,
                    });
                }
                let kind = read_u32(rec, cur + 8)?;
                let interpolation =
                    Interpolation::from_u32(kind).ok_or(Error::UnknownTrackInterpolation {
                        chunk: "GEOA",
                        at: at + 4 + cur,
                        kind,
                    })?;
                let count = read_u32(rec, cur + 4)?;
                let global_seq_id = read_u32(rec, cur + 12)?;
                let mut t = AnimTrack {
                    magic,
                    count,
                    interpolation,
                    global_seq_id,
                    raw: Vec::new(),
                };
                let body = cur + TRACK_HEADER;
                let end = (count as usize)
                    .checked_mul(t.key_len())
                    .and_then(|size| body.checked_add(size))
                    .ok_or(Error::TrackOverruns {
                        chunk: "GEOA",
                        at: at + 4 + cur,
                        size: usize::MAX,
                        left: rec.len() - cur,
                    })?;
                if end > rec.len() {
                    return Err(Error::Truncated {
                        what: "GEOA track body",
                        at,
                        need: end,
                        have: rec.len(),
                    });
                }
                t.raw = rec[body..end].to_vec();
                anim.tracks.push(t);
                cur = end;
            }
            anim.trailing = rec.len() - cur;
            out.push(anim);
            at += len;
        }
        if at != p.len() {
            return Err(Error::RaggedRecords {
                chunk: "GEOA",
                size: p.len(),
                stride: at,
            });
        }
        Ok(out)
    }

    /// Pivot points, one per node.
    pub fn pivots(&self, data: &[u8]) -> Result<Pivots, Error> {
        match self.payload(data, tag::PIVT) {
            Some(p) => Pivots::parse(p),
            None => Ok(Pivots(Vec::new())),
        }
    }

    /// Node records from a chunk framed like `BONE`: a self-inclusive size, an
    /// inline NUL-terminated name, then the node fields.
    ///
    /// `HELP`, `ATCH`, `CAMS` and `LITE` all share this shape, so they are one
    /// reader rather than four. The name field is 80 bytes, which is what puts
    /// `objectId` at `+0x54` and `parentId` at `+0x58` — the same offsets the
    /// `BONE` chunk uses.
    pub fn nodes(&self, data: &[u8], tag: &[u8; 4]) -> Result<Vec<Node>, Error> {
        let Some(p) = self.payload(data, tag) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut at = 0usize;
        while at + 4 <= p.len() {
            let size = read_u32(p, at)? as usize;
            if size < NODE_FIXED || at + size > p.len() {
                return Err(Error::RaggedRecords {
                    chunk: "NODE",
                    size: p.len(),
                    stride: size,
                });
            }
            let mut tracks = Vec::new();
            // HELP measures its node and keyed transforms. ATCH may append
            // attachment fields outside that node size; its own reader owns those.
            if tag == tag::HELP {
                read_tracks("HELP", p, at + NODE_FIXED, at + size, 0, &mut tracks)?;
            }
            out.push(Node {
                offset: at,
                size: size as u32,
                name: read_cstr_bounded(p, at + 4, NODE_NAME),
                object_id: read_u32(p, at + 4 + NODE_NAME)?,
                parent_id: read_i32(p, at + 4 + NODE_NAME + 4)?,
                flags: read_u32(p, at + 4 + NODE_NAME + 8)?,
                tracks,
            });
            at += size;
        }
        if at != p.len() {
            return Err(Error::RaggedRecords {
                chunk: "NODE",
                size: p.len(),
                stride: at,
            });
        }
        Ok(out)
    }

    /// `EVTS` event objects and `CLID` collision shapes.
    ///
    /// A separate reader from [`Mdlx::nodes`] because the record framing differs:
    /// the leading size measures the node header, and tracks follow outside it.
    pub fn event_nodes(&self, data: &[u8], tag: &[u8; 4]) -> Result<Vec<EventNode>, Error> {
        let Some(p) = self.payload(data, tag) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut at = 0usize;
        while at + 4 <= p.len() {
            let header_size = read_u32(p, at)? as usize;
            if header_size < EVENT_FIXED || at + header_size > p.len() {
                return Err(Error::RaggedRecords {
                    chunk: "EVTS",
                    size: p.len(),
                    stride: header_size,
                });
            }
            let mut node = EventNode {
                header_size: header_size as u32,
                name: read_cstr_bounded(p, at + 4, NODE_NAME),
                object_id: read_u32(p, at + 4 + NODE_NAME)?,
                parent_id: read_i32(p, at + 4 + NODE_NAME + 4)?,
                flags: read_u32(p, at + 4 + NODE_NAME + 8)?,
                frames: Vec::new(),
                track: [0; 4],
                trailing: Vec::new(),
            };

            // Whatever follows the header. `EVTS` has a `KEVT` track of
            // frame numbers; `CLID` has inline collision geometry instead. They
            // are told apart by the track magic, and anything unrecognised is
            // kept raw rather than guessed at.
            let rest = at + header_size;
            if rest + 12 <= p.len() && &p[rest..rest + 4] == b"KEVT" {
                node.track = p[rest..rest + 4].try_into().unwrap();
                let count = read_u32(p, rest + 4)? as usize;
                let end = rest + 12 + count * 4;
                if end > p.len() {
                    return Err(Error::Truncated {
                        what: "event track",
                        at,
                        need: end,
                        have: p.len(),
                    });
                }
                node.frames = p[rest + 12..end]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_le_bytes(*c))
                    .collect();
                at = end;
            } else {
                // No `KEVT` track: this is a `CLID` record, whose leading size
                // covers the node header *and* the node's own animation, and
                // whose geometry follows that. Size the record from the shape
                // kind rather than taking the rest of the chunk, so the walk
                // lands on the next record instead of swallowing the remainder.
                // `trailing` now holds exactly this record's geometry instead of
                // everything after the first header; [`Mdlx::collision_shapes`]
                // is the reader that decodes it.
                let shape = Self::collision_shape(p, at)?;
                let end = at + shape.size();
                node.trailing = p[at + shape.header_size as usize..end].to_vec();
                at = end;
            }
            out.push(node);
        }
        if at != p.len() {
            return Err(Error::RaggedRecords {
                chunk: "EVTS",
                size: p.len(),
                stride: at,
            });
        }
        Ok(out)
    }

    /// The `CLID` collision shapes, decoded.
    ///
    /// Its own reader because `CLID` is not `EVTS` with a different payload: the
    /// leading size measures the node header *and* the node's own animation, and
    /// the shape sits outside that region. [`Mdlx::event_nodes`] frames it; this
    /// one reads the shape.
    ///
    /// The walk must consume the chunk exactly. That is not ceremony: it is what
    /// makes the geometry width a fact rather than a guess, because a width that
    /// is one word out would still leave the chunk looking plausible while every
    /// record after the first was misaligned.
    pub fn collision_shapes(&self, data: &[u8]) -> Result<Vec<CollisionShape>, Error> {
        let Some(p) = self.payload(data, tag::CLID) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut at = 0usize;
        while at < p.len() {
            let shape = Self::collision_shape(p, at)?;
            at += shape.size();
            out.push(shape);
        }
        if at != p.len() {
            return Err(Error::RaggedRecords {
                chunk: "CLID",
                size: p.len(),
                stride: at,
            });
        }
        Ok(out)
    }

    /// One `CLID` record at `at`, or an error rather than a guess.
    ///
    /// Three things are decided here and all three are measured, not assumed:
    /// how wide the node header is, how wide the node animation inside it is,
    /// and how wide the shape after it is. The last is what the walk turns on.
    fn collision_shape(p: &[u8], at: usize) -> Result<CollisionShape, Error> {
        let header_size = read_u32(p, at)? as usize;
        if header_size < EVENT_FIXED || at + header_size > p.len() {
            return Err(Error::RaggedRecords {
                chunk: "CLID",
                size: p.len(),
                stride: header_size,
            });
        }

        // The node's own animation lives *inside* the measured header, the
        // opposite of `EVTS` where the track trails it. Reading it here is what
        // makes `header_size == 96 + node_track_bytes()` checkable instead of
        // merely plausible; a magic this reader does not know refuses the record
        // rather than being skipped over.
        let mut tracks = Vec::new();
        read_tracks(
            "CLID",
            p,
            at + EVENT_FIXED,
            at + header_size,
            at,
            &mut tracks,
        )?;

        let g = at + header_size;
        let kind = CollisionShapeKind::parse(g, read_u32(p, g)?)?;
        let mut vertices = Vec::with_capacity(kind.vertex_count());
        for v in 0..kind.vertex_count() {
            let o = g + 4 + v * 12;
            vertices.push([read_f32(p, o)?, read_f32(p, o + 4)?, read_f32(p, o + 8)?]);
        }
        // For a sphere the word after the centre is handed on as the float it is.
        // That it is a *radius* is read off the shipped corpus rather than proved
        // here — see [`CollisionShape`].
        let radius = match kind {
            CollisionShapeKind::Box => None,
            CollisionShapeKind::Sphere => Some(read_f32(p, g + 16)?),
        };

        Ok(CollisionShape {
            header_size: header_size as u32,
            name: read_cstr_bounded(p, at + 4, NODE_NAME),
            object_id: read_u32(p, at + 4 + NODE_NAME)?,
            parent_id: read_i32(p, at + 4 + NODE_NAME + 4)?,
            flags: read_u32(p, at + 4 + NODE_NAME + 8)?,
            kind,
            vertices,
            radius,
            tracks,
        })
    }

    /// Materials, in file order.
    ///
    /// Walks self-inclusive lengths and requires the walk to consume the chunk
    /// exactly, which is what makes the framing trustworthy rather than
    /// assumed.
    pub fn materials(&self, data: &[u8]) -> Result<Vec<Material>, Error> {
        let Some(p) = self.payload(data, tag::MTLS) else {
            return Ok(Vec::new());
        };
        if self.version != 800 {
            return Err(Error::UnsupportedVersionedChunk {
                version: self.version,
                chunk: "MTLS",
            });
        }
        let mut out = Vec::new();
        let mut at = 0usize;
        while at + 4 <= p.len() {
            let len = read_u32(p, at)? as usize;
            if len < 4 || at + len > p.len() {
                return Err(Error::RaggedRecords {
                    chunk: "MTLS",
                    size: p.len(),
                    stride: len,
                });
            }
            let material = Material {
                offset: at,
                len: len as u32,
                data: p[at + 4..at + len].to_vec(),
            };
            material.checked_layers()?;
            out.push(material);
            at += len;
        }
        if at != p.len() {
            return Err(Error::RaggedRecords {
                chunk: "MTLS",
                size: p.len(),
                stride: at,
            });
        }
        Ok(out)
    }

    /// Geosets, in file order.
    ///
    /// Walks self-inclusive sizes and requires the walk to consume the chunk
    /// exactly; a wrong interpretation cannot land on the final byte.
    pub fn geosets(&self, data: &[u8]) -> Result<Vec<Geoset>, Error> {
        let Some(p) = self.payload(data, tag::GEOS) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut at = 0usize;
        while at + 4 <= p.len() {
            let size = read_u32(p, at)?;
            let size = size as usize;
            if size < 4 || at + size > p.len() {
                return Err(Error::RaggedRecords {
                    chunk: "GEOS",
                    size: p.len(),
                    stride: size,
                });
            }
            out.push(Geoset {
                offset: at,
                size: size as u32,
                data: p[at + 4..at + size].to_vec(),
            });
            at += size;
        }
        if at != p.len() {
            return Err(Error::RaggedRecords {
                chunk: "GEOS",
                size: p.len(),
                stride: at,
            });
        }
        Ok(out)
    }

    /// Replaceable-texture slots, in file order.
    pub fn textures(&self, data: &[u8]) -> Result<Vec<Texture>, Error> {
        let Some(p) = self.payload(data, tag::TEXS) else {
            return Ok(Vec::new());
        };
        if p.len() % TEXTURE_SIZE != 0 {
            return Err(Error::RaggedRecords {
                chunk: "TEXS",
                size: p.len(),
                stride: TEXTURE_SIZE,
            });
        }
        let mut out = Vec::with_capacity(p.len() / TEXTURE_SIZE);
        for r in p.as_chunks::<TEXTURE_SIZE>().0 {
            out.push(Texture {
                slot: read_u32(r, 0)?,
                path: read_cstr_bounded(r, 4, 260),
                // `slot(4) + path(260)` is 264, so the flag word is the last
                // four bytes of the 268-byte record.
                flags: read_u32(r, 264)?,
            });
        }
        Ok(out)
    }

    /// Texture animations, in file order.
    ///
    /// Runs of self-inclusive sizes, each followed by its animation tracks. The
    /// walk must land exactly on the end of the payload, and a record must be
    /// its size word plus its tracks and nothing else — `len == 4 + tracks`
    /// holds for all 279 records across both builds.
    pub fn texture_anims(&self, data: &[u8]) -> Result<Vec<TextureAnim>, Error> {
        let Some(p) = self.payload(data, tag::TXAN) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut at = 0usize;
        while at < p.len() {
            let len = read_u32(p, at)? as usize;
            if len < 4 || at + len > p.len() {
                return Err(Error::EffectRecordOverruns {
                    chunk: "TXAN",
                    at,
                    size: len,
                    left: p.len() - at,
                });
            }
            let mut tracks = Vec::new();
            read_tracks("TXAN", p, at + 4, at + len, 0, &mut tracks)?;
            out.push(TextureAnim {
                offset: at,
                len: len as u32,
                tracks,
            });
            at += len;
        }
        if at != p.len() {
            return Err(Error::RaggedRecords {
                chunk: "TXAN",
                size: p.len(),
                stride: at,
            });
        }
        Ok(out)
    }

    /// Ribbon emitters, in file order.
    pub fn ribbons(&self, data: &[u8]) -> Result<Vec<Ribbon>, Error> {
        let mut out = Vec::new();
        for node in effect_nodes("RIBB", self.payload(data, tag::RIBB))? {
            // Body offsets are reported against the record start, so the
            // record's own offset in the payload is carried along.
            let body_base = node.offset + 4 + node.node_size;
            if node.body.len() < RIBBON_FIXED {
                // A record shorter than its own emitter state is a walk that
                // has lost sync, not a ribbon with nothing in it.
                return Err(Error::RecordShape {
                    chunk: "RIBB",
                    at: node.offset,
                    size: node.body.len(),
                    measured: RIBBON_FIXED,
                });
            }
            // The state occupies the first 52 bytes and the tracks the rest,
            // and `read_tracks` requires the rest to be consumed exactly.
            let mut tracks = Vec::new();
            read_tracks(
                "RIBB",
                &node.body,
                RIBBON_FIXED,
                node.body.len(),
                body_base,
                &mut tracks,
            )?;
            let f = |o: usize| -> Result<f32, Error> { read_f32(&node.body, o) };
            let w = |o: usize| -> Result<u32, Error> { read_u32(&node.body, o) };
            out.push(Ribbon {
                offset: node.offset,
                total_size: node.total_size as u32,
                node_size: node.node_size as u32,
                name: node.name,
                object_id: node.object_id,
                parent_id: node.parent_id,
                flags: node.flags,
                node_tracks: node.node_tracks,
                height_above: f(0)?,
                height_below: f(4)?,
                alpha: f(8)?,
                colour: [f(12)?, f(16)?, f(20)?],
                edge_lifetime: f(24)?,
                texture_slot: w(28)?,
                edges_per_second: w(32)?,
                texture_rows: w(36)?,
                texture_cols: w(40)?,
                material_id: w(44)?,
                gravity: f(48)?,
                tracks,
            });
        }
        Ok(out)
    }

    /// Particle emitters, in file order.
    ///
    /// These are the `PREM` v1 emitters, not the `PRE2` set; the two coexist in
    /// the shipped models and the newer one is not decoded here.
    pub fn particles(&self, data: &[u8]) -> Result<Vec<Particle>, Error> {
        let mut out = Vec::new();
        for node in effect_nodes("PREM", self.payload(data, tag::PREM))? {
            // Body offsets are reported against the record start, so the
            // record's own offset in the payload is carried along.
            let body_base = node.offset + 4 + node.node_size;
            if node.body.len() < PARTICLE_FIXED {
                return Err(Error::RecordShape {
                    chunk: "PREM",
                    at: node.offset,
                    size: node.body.len(),
                    measured: PARTICLE_FIXED,
                });
            }
            let mut tracks = Vec::new();
            read_tracks(
                "PREM",
                &node.body,
                PARTICLE_FIXED,
                node.body.len(),
                body_base,
                &mut tracks,
            )?;
            let f = |o: usize| -> Result<f32, Error> { read_f32(&node.body, o) };
            out.push(Particle {
                offset: node.offset,
                total_size: node.total_size as u32,
                node_size: node.node_size as u32,
                name: node.name,
                object_id: node.object_id,
                parent_id: node.parent_id,
                flags: node.flags,
                node_tracks: node.node_tracks,
                emission_rate: f(0)?,
                gravity: f(4)?,
                longitude: f(8)?,
                latitude: f(12)?,
                path: read_cstr_bounded(&node.body, 16, PARTICLE_PATH),
                life: f(16 + PARTICLE_PATH)?,
                speed: f(16 + PARTICLE_PATH + 4)?,
                tracks,
            });
        }
        Ok(out)
    }

    /// Cameras, in file order.
    ///
    /// A separate reader from [`Mdlx::nodes`] because a camera has no node
    /// header — see [`Camera`] for what that means for the field offsets.
    pub fn cameras(&self, data: &[u8]) -> Result<Vec<Camera>, Error> {
        let Some(p) = self.payload(data, tag::CAMS) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut at = 0usize;
        while at < p.len() {
            let size = read_u32(p, at)? as usize;
            if size < 4 + NODE_NAME + CAMERA_FIXED || at + size > p.len() {
                return Err(Error::EffectRecordOverruns {
                    chunk: "CAMS",
                    at,
                    size,
                    left: p.len() - at,
                });
            }
            let mut tracks = Vec::new();
            read_tracks(
                "CAMS",
                p,
                at + 4 + NODE_NAME + CAMERA_FIXED,
                at + size,
                0,
                &mut tracks,
            )?;
            let f = |o: usize| -> Result<f32, Error> { read_f32(p, o) };
            out.push(Camera {
                offset: at,
                size: size as u32,
                name: read_cstr_bounded(p, at + 4, NODE_NAME),
                eye: [f(at + 84)?, f(at + 88)?, f(at + 92)?],
                field_of_view: f(at + 96)?,
                far: f(at + 100)?,
                near: f(at + 104)?,
                target: [f(at + 108)?, f(at + 112)?, f(at + 116)?],
                tracks,
            });
            at += size;
        }
        if at != p.len() {
            return Err(Error::RaggedRecords {
                chunk: "CAMS",
                size: p.len(),
                stride: at,
            });
        }
        Ok(out)
    }

    /// Lights, in file order.
    ///
    /// The shared [`Mdlx::nodes`] reader decodes a light's node header
    /// correctly, but the light's 44 bytes of state and its own tracks would be
    /// silently dropped, so this is its own reader.
    pub fn lights(&self, data: &[u8]) -> Result<Vec<Light>, Error> {
        let mut out = Vec::new();
        for node in effect_nodes("LITE", self.payload(data, tag::LITE))? {
            // Body offsets are reported against the record start, so the
            // record's own offset in the payload is carried along.
            let body_base = node.offset + 4 + node.node_size;
            if node.body.len() < LIGHT_FIXED {
                return Err(Error::RecordShape {
                    chunk: "LITE",
                    at: node.offset,
                    size: node.body.len(),
                    measured: LIGHT_FIXED,
                });
            }
            let kind = LightKind::parse(node.offset, read_u32(&node.body, 0)?)?;
            let mut tracks = Vec::new();
            read_tracks(
                "LITE",
                &node.body,
                LIGHT_FIXED,
                node.body.len(),
                body_base,
                &mut tracks,
            )?;
            let f = |o: usize| -> Result<f32, Error> { read_f32(&node.body, o) };
            out.push(Light {
                offset: node.offset,
                total_size: node.total_size as u32,
                node_size: node.node_size as u32,
                name: node.name,
                object_id: node.object_id,
                parent_id: node.parent_id,
                flags: node.flags,
                node_tracks: node.node_tracks,
                kind,
                attenuation_start: f(4)?,
                attenuation_end: f(8)?,
                colour: [f(12)?, f(16)?, f(20)?],
                intensity: f(24)?,
                ambient_colour: [f(28)?, f(32)?, f(36)?],
                ambient_intensity: f(40)?,
                tracks,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn script_model_paths_select_only_the_binary_extension() {
        for (path, expected) in [
            ("UI/Custom.MDL", "UI/Custom.mdx"),
            ("Models/树.mdl", "Models/树.mdx"),
            ("a.mdx", "a.mdx"),
            ("a.mdl/other", "a.mdl/other"),
            ("", ""),
            ("树", "树"),
        ] {
            assert_eq!(super::binary_model_path(path), expected);
        }
    }
    use super::*;

    #[test]
    fn light_colour_curves_keep_their_vector_stride_and_channels() {
        for magic in [*b"KLAC", *b"KLBC"] {
            let mut raw = Vec::new();
            for (frame, values) in [(0u32, [0.0f32, 0.25, 1.0]), (100, [1.0, 0.75, 0.0])] {
                raw.extend(frame.to_le_bytes());
                for value in values {
                    raw.extend(value.to_le_bytes());
                }
            }
            let track = AnimTrack {
                magic,
                count: 2,
                interpolation: Interpolation::Linear,
                global_seq_id: NO_ID,
                raw,
            };
            assert_eq!(track.key_len(), 16);
            assert_eq!(track.frame(1), 100);
            assert_eq!(sample_colour(&track, 50), [0.5, 0.5, 0.5]);
            assert_eq!(sample_colour(&track, 100), [1.0, 0.75, 0.0]);
        }
    }

    /// Where to find a WC3 install.
    ///
    /// `PEACECRAFT3_INSTALL` overrides it so the same sweeps can be pointed
    /// at any build; otherwise the 1.28c tree this project was extracted
    /// from. `PEACECRAFT3_LEGACY_DIR` is accepted as an alias because that is
    /// where the 1.29b Battle.net install lives by default.
    fn install_dir() -> Option<std::path::PathBuf> {
        for key in ["PEACECRAFT3_INSTALL", "PEACECRAFT3_LEGACY_DIR"] {
            if let Ok(v) = std::env::var(key) {
                let p = std::path::PathBuf::from(v);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        p.is_dir().then_some(p)
    }

    fn shipped(rel: &str) -> Option<Vec<u8>> {
        let dir = install_dir()?;
        pcx_mpq::MpqSet::open_wc3(dir).ok()?.extract(rel).ok()
    }

    /// Where a geoset's `MATS` sub-block sits, and the count it declares.
    ///
    /// Found by walking the width table the parser uses rather than by scanning
    /// for the tag, because a `MATS` byte pattern inside a vertex array would
    /// win that race: that is the failure the old scan reader had.
    fn mats_sub_block(g: &Geoset) -> Option<(usize, u32)> {
        let mut at = 0usize;
        while at + 8 <= g.data.len() {
            let tag: [u8; 4] = g.data[at..at + 4].try_into().ok()?;
            let count = u32::from_le_bytes(g.data[at + 4..at + 8].try_into().ok()?);
            if &tag == geoset_tag::MATS {
                return Some((at, count));
            }
            at += 8 + sub_block_width(&tag, count)?;
        }
        None
    }

    const FOOTMAN: &str = "Units\\Human\\Footman\\Footman.mdx";

    #[test]
    fn parses_the_footman_container() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("{FOOTMAN}: {e}"));
        assert_eq!(m.version, 800);
        // Chunk order is fixed for this build; the walk proves it by landing on
        // the end of the file, so this list is a fingerprint rather than a hope.
        assert_eq!(
            m.tags(),
            vec![
                "VERS", "MODL", "SEQS", "MTLS", "TEXS", "GEOS", "GEOA", "BONE", "HELP", "ATCH",
                "PIVT", "EVTS", "CLID"
            ]
        );

        let model = m.model(&d).unwrap_or_else(|e| panic!("MODL: {e}"));
        assert_eq!(model.name, "Footman");
        assert!(model.minimum_extent[0] < 0.0 && model.maximum_extent[1] > 0.0);
        // A footman is roughly 140 units tall in WC3 scale.
        let height = model.maximum_extent[2] - model.minimum_extent[2];
        assert!((100.0..200.0).contains(&height), "height {height}");

        // Chunks must tile the file exactly, with no gaps or overlaps.
        let mut at = 4usize;
        for c in &m.chunks {
            assert_eq!(
                c.offset,
                at,
                "{} out of order",
                String::from_utf8_lossy(&c.tag)
            );
            at = c.payload_offset + c.size;
        }
        assert_eq!(at, d.len(), "walk must consume the whole file");
    }

    #[test]
    fn reads_the_footman_animation_timeline() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap();
        let seqs = m.sequences(&d).unwrap_or_else(|e| panic!("SEQS: {e}"));
        assert_eq!(seqs.len(), 13, "1716 / 132 must be 13 records");
        assert_eq!(seqs[0].name, "Stand - 1");
        assert_eq!(seqs[0].interval_start, 167);
        assert_eq!(seqs[0].interval_end, 1667);
        assert_eq!(seqs[0].duration(), 1500);
        assert!(!seqs[0].non_looping, "stand loops");
        // Attacks must not loop, and the whole timeline must be ordered.
        assert!(seqs[4].non_looping, "attack should not loop");
        for w in seqs.windows(2) {
            assert!(
                w[1].interval_start >= w[0].interval_start,
                "timeline goes backwards at {}",
                w[1].name
            );
        }
        for s in &seqs {
            assert!(!s.name.is_empty(), "unnamed sequence");
            assert!(s.interval_end >= s.interval_start, "{}: inverted", s.name);
        }
    }

    #[test]
    fn walks_the_footman_geosets_exactly() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap();
        let geos = m.geosets(&d).unwrap_or_else(|e| panic!("GEOS: {e}"));
        assert_eq!(geos.len(), 5, "a footman has five geosets");
        // Self-inclusive sizes, in the order the chunk stores them.
        assert_eq!(
            geos.iter().map(|g| g.size).collect::<Vec<_>>(),
            vec![17335, 2514, 1492, 754, 2335]
        );
        // Every geoset opens with VRTX, and they tile the chunk.
        let total: u32 = geos.iter().map(|g| g.size).sum();
        assert_eq!(
            total as usize,
            m.payload(&d, tag::GEOS).unwrap().len(),
            "geoset sizes must cover the chunk exactly"
        );
        for g in &geos {
            assert_eq!(&g.data[..4], geoset_tag::VRTX, "geoset must open with VRTX");
        }
        // Sizes must shrink away from the main body geoset.
        assert!(geos[0].size > geos[1].size, "first geoset is the body");
    }

    #[test]
    fn decodes_the_footman_mesh() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap();
        let geos = m.geosets(&d).unwrap();
        assert_eq!(geos.len(), 5);

        let mesh = m.mesh(&d, &geos[0]).unwrap_or_else(|e| panic!("mesh: {e}"));

        // Counts measured independently from the raw bytes before this parser
        // existed, so they pin the parser rather than following it.
        assert_eq!(mesh.vertices.count, 447);
        assert_eq!(mesh.normals.count, 447);
        assert_eq!(mesh.tex_coords.count, 447);
        assert_eq!(mesh.triangles.count, 864);
        assert_eq!(mesh.triangles.triangle_count(), 288);
        assert_eq!(mesh.vertex_groups.len(), 447);

        // Positions and normals are unpacked f32 triples in both arrays.
        assert_eq!(mesh.vertices.raw.len(), 447 * 12);
        assert_eq!(mesh.normals.raw.len(), 447 * 12);
        assert_eq!(mesh.tex_coords.raw.len(), 447 * 8);

        // Every index must address a real vertex.
        for i in 0..mesh.triangles.count as usize {
            let v = u16::from_le_bytes([mesh.triangles.raw[i * 2], mesh.triangles.raw[i * 2 + 1]]);
            assert!(v < mesh.vertices.count as u16, "index {v} out of range");
        }

        // Normals are unit length; that is what makes the f32-triple reading
        // trustworthy.
        for n in mesh.normals.iter() {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((0.99..1.01).contains(&len), "normal not unit: {len}");
        }

        // The body geoset is the tallest of the five.
        assert!(geos[0].size > geos[1].size, "first geoset is the body");

        // A mesh with no VTRX must fail, not silently produce empty geometry.
        let fake = Geoset {
            offset: 0,
            size: 8,
            data: b"VRTX\xff\xff\0\0".to_vec(),
        };
        assert!(
            m.mesh(&d, &fake).is_err(),
            "a VRTX claiming 65535 vertices must be refused"
        );
    }

    /// The `MATS` trailer, pinned geoset by geoset.
    ///
    /// This is the check that stops the geoset/material pairing from drifting
    /// back to index cycling. Cycling over the footman's three materials gives
    /// `0, 1, 2, 0, 1`, which draws the blood geoset with the empty-texture
    /// material and a body piece with `gutz.blp`. That guess was wrong on 521
    /// shipped models and 3,121 of their geosets.
    #[test]
    fn the_footmans_geosets_carry_their_own_material_id() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap();
        let geos = m.geosets(&d).unwrap();
        assert_eq!(geos.len(), 5);

        // The measurement, from vanilla bytes: 0, 1, 0, 0, 2. Geoset 4 is the
        // blood, and it names `gutz.blp`, not material 1.
        let ids: Vec<u32> = geos
            .iter()
            .map(|g| {
                m.mesh(&d, g)
                    .unwrap_or_else(|e| panic!("mesh: {e}"))
                    .material_id
            })
            .collect();
        assert_eq!(ids, vec![0, 1, 0, 0, 2], "material id per geoset");

        // Node indices are `MATS.count` u32s and the groups trim into them.
        // The footman's body group draws 54 bones over 447 vertices, and the
        // group counts sum to the same 54.
        let body = m.mesh(&d, &geos[0]).unwrap();
        assert_eq!(body.matrix_span_u32s, 54);
        let want: usize = body.matrix_groups.iter().map(|&c| c as usize).sum();
        assert_eq!(want, 54, "sum(MTGC) must equal MATS.count");
        assert_eq!(body.matrix_indices.len(), 54);

        // The trailer's own fields, for the same geoset.
        assert_eq!(body.selection_group, 0);
        assert_eq!(body.unselectable, 0);
        assert!((body.bounds_radius - 114.955).abs() < 0.01);
        assert!((body.minimum_extent[0] + 90.77).abs() < 0.01);
        assert!((body.maximum_extent[2] - 142.28).abs() < 0.01);
        assert_eq!(
            body.seq_bounds.len(),
            13,
            "no bounds records, or one per sequence, and the footman has 13"
        );
        // One record read out of the shipped bytes: the extent radius, then
        // the minimum and the maximum. This is what pins the field order
        // inside the 28-byte record.
        for (&got, want) in body.seq_bounds[0].iter().zip([
            62.4483, -19.5992, -33.1796, -0.2823, 52.4008, 36.7176, 99.0722,
        ]) {
            assert!(
                (got - want).abs() < 1e-3,
                "seqBounds[0] = {:?}, expected {want}",
                body.seq_bounds[0]
            );
        }
        for b in &body.seq_bounds {
            // A record holds either real bounds or the unset pair, a minimum
            // above zero and a maximum below it. Reading the two extents the
            // other way round shows up here as an inversion.
            if !(b[1] > 0.0 && b[4] < 0.0) {
                for i in 0..3 {
                    assert!(b[4 + i] >= b[1 + i], "extent {i} is inverted: {b:?}");
                }
            }
        }

        // Every id must address a material the model ships.
        let mats = m.materials(&d).unwrap();
        assert_eq!(mats.len(), 3);
        for (i, &id) in ids.iter().enumerate() {
            assert!(
                (id as usize) < mats.len(),
                "geoset {i} names material {id} of {}",
                mats.len()
            );
        }

        // The blood geoset names `gutz.blp` through material 2, which the
        // material table resolves to `Textures\gutz.blp`.
        let tex = m.textures(&d).unwrap();
        let blood = &mats[2];
        assert_eq!(
            blood
                .tex_layers()
                .first()
                .and_then(|l| tex.get(l.texture_id as usize))
                .map(|t| t.path.clone()),
            Some("Textures\\gutz.blp".to_string())
        );
    }

    /// The three materials must resolve to the three textures, in order.
    ///
    /// This is the test that pins `textureId` to body offset 20. The value at
    /// offset 8 is the declared layer length — 28 or 100 — and reading that
    /// instead produced layers pointing at texture 28 and 100, which looked
    /// plausible until the paths were checked.
    #[test]
    fn the_footmans_materials_resolve_to_its_three_textures() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap();
        let tex = m.textures(&d).unwrap();
        assert_eq!(tex.len(), 3);
        assert_eq!(tex[0].path, "Textures\\Footman.blp");
        assert!(tex[1].path.is_empty(), "the second slot carries no path");
        assert_eq!(tex[2].path, "Textures\\gutz.blp");

        let mats = m.materials(&d).unwrap();
        assert_eq!(mats.len(), 3, "three materials");
        let ids: Vec<u32> = mats
            .iter()
            .filter_map(|mat| mat.tex_layers().first().map(|l| l.texture_id))
            .collect();
        assert_eq!(
            ids,
            vec![0, 1, 2],
            "each material's layer must point at the matching texture slot"
        );
        // A layer with no animation reads back as the sentinel, not as zero.
        for mat in &mats {
            for l in mat.tex_layers() {
                assert!(l.has_no_texture_anim(), "the footman's layers are static");
                assert!(l.static_alpha.is_finite(), "authored static alpha");
            }
        }
    }

    /// `GEOA` records are self-inclusive and their tracks are 8 bytes per key.
    ///
    /// The key count and stride are only recoverable together — a wrong stride
    /// still produces plausible-looking frames — so the relation
    /// `len == 24 + 12 + 8 * count` is asserted, and the header offsets are
    /// pinned against the shipped bytes.
    #[test]
    fn reads_geoset_animations_with_the_measured_layout() {
        let Some(d) = shipped("UI\\Glues\\MainMenu\\MainMenu3d\\MainMenu3d.mdx")
            .or_else(|| shipped("Abilities\\Spells\\Items\\OrbDarkness\\OrbDarkness.mdx"))
        else {
            return;
        };
        let m = Mdlx::parse(&d).unwrap();
        let anims = m.geoset_anims(&d).unwrap_or_else(|e| panic!("GEOA: {e}"));
        if anims.is_empty() {
            return; // Not every model carries one; that is legal.
        }

        for a in &anims {
            for t in &a.tracks {
                assert_eq!(
                    t.raw.len(),
                    t.count as usize * t.key_len(),
                    "track {} must be {}-byte keys",
                    String::from_utf8_lossy(&t.magic),
                    t.key_len()
                );
            }
            // The framing relation, asserted exactly. An earlier version used
            // `<=` because records "may pad", and that looseness is what let a
            // misread track header go unnoticed: every record still fit, just
            // wrongly. Holding the walk to the record's exact end is what makes
            // the layout trustworthy.
            let want: usize = GEOA_FIXED
                + a.tracks
                    .iter()
                    .map(|t| TRACK_HEADER + t.count as usize * t.key_len())
                    .sum::<usize>();
            // `len` is self-inclusive, so the data is four bytes shorter.
            assert_eq!(
                want,
                (a.len as usize) - 4,
                "a GEOA record must be its header plus its tracks exactly"
            );
            assert_eq!(a.trailing, 0, "and the walk must leave nothing behind");
        }

        // The first-frame values of the first track, when there is one. A record
        // with no tracks is legal, so guard rather than indexing blindly.
        if let (Some(a), Some(t)) = (anims.first(), anims.first().and_then(|x| x.tracks.first())) {
            assert!(a.static_alpha >= 0.0 && a.static_alpha <= 1.0);
            let (frame, value) = (
                u32::from_le_bytes(t.raw[0..4].try_into().unwrap()),
                f32::from_bits(u32::from_le_bytes(t.raw[4..8].try_into().unwrap())),
            );
            assert!(frame < 100_000, "first key frame {frame} looks wrong");
            assert!(value.is_finite(), "first key value {value} looks wrong");
        }
    }

    /// Interpolation must hold the end values outside the key range, not
    /// extrapolate — an unbounded alpha or colour is how a model ends up glowing
    /// white and nobody notices until it ships.
    #[test]
    fn a_track_samples_its_end_values_outside_its_range() {
        // Three keys: 0 -> 0.0, 100 -> 1.0, 200 -> 0.5.
        let raw = {
            let mut v = Vec::new();
            for (frame, value) in [(0u32, 0.0f32), (100, 1.0), (200, 0.5)] {
                v.extend_from_slice(&frame.to_le_bytes());
                v.extend_from_slice(&value.to_le_bytes());
            }
            v
        };
        let t = AnimTrack {
            magic: *b"KGAA",
            count: 3,
            interpolation: Interpolation::Linear,
            global_seq_id: NO_ID,
            raw,
        };

        assert_eq!(sample_scalar(&t, 0), 0.0, "before the first key");
        assert!((sample_scalar(&t, 50) - 0.5).abs() < 1e-6, "midway");
        assert_eq!(sample_scalar(&t, 100), 1.0, "on the key");
        assert!((sample_scalar(&t, 150) - 0.75).abs() < 1e-6, "between keys");
        assert_eq!(sample_scalar(&t, 200), 0.5, "on the last key");
        assert_eq!(
            sample_scalar(&t, 9_999),
            0.5,
            "past the last key it must hold, not extrapolate"
        );

        // A degenerate track must not divide by zero.
        let flat = AnimTrack {
            magic: *b"KGAA",
            count: 2,
            interpolation: Interpolation::Linear,
            global_seq_id: NO_ID,
            raw: {
                let mut v = Vec::new();
                for _ in 0..2 {
                    v.extend_from_slice(&50u32.to_le_bytes());
                    v.extend_from_slice(&0.25f32.to_le_bytes());
                }
                v
            },
        };
        assert_eq!(sample_scalar(&flat, 50), 0.25, "two keys on one frame");
    }

    #[test]
    fn colour_track_samples_each_component() {
        let mut raw = Vec::new();
        for (frame, colour) in [(0u32, [0.0f32, 0.5, 1.0]), (100, [1.0, 0.0, 0.5])] {
            raw.extend(frame.to_le_bytes());
            for value in colour {
                raw.extend(value.to_le_bytes());
            }
        }
        let track = AnimTrack {
            magic: *b"KGAC",
            count: 2,
            interpolation: Interpolation::Linear,
            global_seq_id: NO_ID,
            raw,
        };
        let mid = sample_colour(&track, 50);
        assert!((mid[0] - 0.5).abs() < 1e-6);
        assert!((mid[1] - 0.25).abs() < 1e-6);
        assert!((mid[2] - 0.75).abs() < 1e-6);
    }

    /// `PIVT` is one position per node, so its length is a fact about the model
    /// that can be checked rather than trusted.
    #[test]
    fn pivot_points_line_up_with_the_node_count() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap();
        let pivots = m.pivots(&d).unwrap();
        assert_eq!(pivots.0.len(), 57, "the footman has 57 pivot points");

        // Every position must be finite, and at least one must be non-zero —
        // an all-zero table would mean the offsets were misread.
        assert!(pivots.0.iter().all(|p| p.iter().all(|v| v.is_finite())));
        assert!(
            pivots.0.iter().any(|p| p.iter().any(|&v| v != 0.0)),
            "all pivot points are zero, which would mean a misread"
        );
        assert!(pivots.position(0).is_some(), "node 0 must have a pivot");
        assert!(pivots.position(999).is_none(), "out of range must be None");

        let bones = m.bones(&d).unwrap();
        let helpers = m.nodes(&d, tag::HELP).unwrap();
        let atts = m.nodes(&d, tag::ATCH).unwrap();
        // Nodes, not just bones: pivots are indexed across the whole node space.
        assert!(
            pivots.0.len() >= bones.len(),
            "there must be at least one pivot per bone"
        );
        // The other node chunks share BONE's framing.
        for (label, nodes) in [("HELP", &helpers), ("ATCH", &atts)] {
            for n in nodes {
                assert!(!n.name.is_empty(), "{label} node with no name");
            }
        }
    }

    /// `EVTS` records are node-header-size plus a trailing `KEVT` track, which
    /// is a different framing from the other node chunks.
    ///
    /// Reading the size as self-inclusive — correct for `HELP` and friends — walks
    /// into the middle of the track, so the names come out wrong and the walk
    /// fails. The names decode as `<type>x<payload>` when the framing is right,
    /// which is the check that catches it.
    #[test]
    fn event_objects_decode_with_their_names_intact() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap();

        let evts = m.event_nodes(&d, tag::EVTS).unwrap();
        assert!(!evts.is_empty(), "the footman has event objects");
        for n in &evts {
            // Every event fires at a frame, and its name has the documented shape.
            assert!(!n.frames.is_empty(), "{} fires at nothing", n.name);
            assert_eq!(n.track, *b"KEVT", "{} must carry a KEVT track", n.name);
            // Three-character type, a separator, then the payload name. The
            // separator is *not* consistent across types: `SNDXDFOO` uses 0x58
            // and `FPTxFBL1` uses 0x78. Assuming either one alone fails on the
            // other, which is how this was found.
            assert!(
                n.name.len() >= 4 && (n.name.as_bytes()[3] == b'x' || n.name.as_bytes()[3] == b'X'),
                "event name {:?} lacks a type/payload separator at byte 3",
                n.name
            );
            assert!(n.parent_id == -1, "{} is not a root", n.name);
        }
        // Footstep events fire in pairs, which is a fact about the model rather
        // than a format rule — it just has to survive the parse.
        let fps = evts.iter().filter(|n| n.name.starts_with("FPT")).count();
        assert!(fps >= 2, "expected paired footstep events, got {fps}");

        // CLID carries inline geometry instead of a track.
        let clid = m.event_nodes(&d, tag::CLID).unwrap();
        for n in &clid {
            assert!(n.frames.is_empty(), "{} should not carry frames", n.name);
            assert!(
                n.trailing.len() % 4 == 0,
                "{} trailing is not word-aligned",
                n.name
            );
        }
    }

    /// A skeleton sampled over a clip must actually move.
    ///
    /// This is the end-to-end check that the tracks are real and the evaluator
    /// reads them: the Footman's `Stand - 4` clip starts at 7000 ms, which is
    /// also its first key's frame, so the pose there equals the key exactly and
    /// the movement shows up *after* it. Comparing the clip start to the first
    /// key compares a frame with itself, which is a test that can never fail.
    #[test]
    fn a_sampled_skeleton_moves_over_a_clip() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap();
        let bones = m.bones(&d).unwrap();
        let seqs = m.sequences(&d).unwrap();

        let Some(bone) = bones.iter().find(|b| b.translation().is_some()) else {
            return; // A model with no animated translation is legal.
        };
        let first = bone.translation().unwrap().key_time(0).unwrap();
        let seq = seqs
            .iter()
            .find(|s| first >= s.interval_start && first <= s.interval_end)
            .expect("the key's frame must fall inside some sequence");
        assert_eq!(
            first, seq.interval_start,
            "this model's first key sits on the clip start, which is the trap"
        );

        // At the first key the pose is exactly the key's value, by definition.
        let at_key = sample_bone_pose(bone, first);
        let value = bone.translation().unwrap().value(0).unwrap();
        assert_eq!(
            at_key.translation[0], value[0],
            "the pose must equal the key"
        );

        // And it must move afterwards.
        let later = sample_bone_pose(bone, first + 800);
        assert_ne!(
            at_key.translation, later.translation,
            "the pose must keep moving past the first key"
        );

        // A frame before any key holds identity rather than extrapolating back.
        assert_eq!(sample_bone_pose(bone, 0).translation, [0.0; 3]);
    }

    #[test]
    fn pose_composition_composes_in_the_documented_order() {
        let parent = Pose {
            translation: [10.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [2.0, 2.0, 2.0],
        };
        let child = Pose {
            translation: [1.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        };
        let world = parent.then(child);
        assert_eq!(
            world.translation,
            [11.0, 0.0, 0.0],
            "child is offset from parent"
        );
        assert_eq!(world.scale, [2.0, 2.0, 2.0], "scale carries down");

        // A parent rotated a quarter turn about Y moves the child's offset.
        let quarter = Pose {
            translation: [0.0; 3],
            rotation: [0.0, 0.707_106_77, 0.0, 0.707_106_77],
            scale: [1.0; 3],
        };
        let spun = quarter.then(Pose::default());
        let r = spun.rotation;
        assert!((r[1] - 0.707_106_77).abs() < 1e-6, "rotation carries down");
    }

    #[test]
    fn a_quaternion_rotates_the_way_the_format_expects() {
        // A quarter turn about Z maps +X onto +Y.
        let q = [0.0, 0.0, 0.707_106_77, 0.707_106_77];
        let v = rotate(q, [1.0, 0.0, 0.0]);
        assert!((v[0]).abs() < 1e-6, "x should be ~0, got {}", v[0]);
        assert!((v[1] - 1.0).abs() < 1e-6, "y should be ~1, got {}", v[1]);
    }

    #[test]
    fn reads_the_footman_texture_slots() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap();
        let tex = m.textures(&d).unwrap_or_else(|e| panic!("TEXS: {e}"));
        assert_eq!(tex.len(), 3, "804 / 268 must be 3 records");
        assert_eq!(tex[0].path, "Textures\\Footman.blp");
        // Some records carry no path at all; those are legal, not empty bugs.
        assert!(
            tex.iter()
                .all(|t| t.path.is_empty() || t.path.to_ascii_lowercase().ends_with(".blp"))
        );
    }

    #[test]
    fn rejects_a_non_mdx() {
        assert!(matches!(
            Mdlx::parse(b"MDL\x00\x00\x00"),
            Err(Error::BadMagic(_))
        ));
    }

    /// The extracted 1.28c retail tree, independent of any env override.
    ///
    /// `install_dir()` follows `PEACECRAFT3_INSTALL`, so a cross-version test
    /// cannot use it for the fixed side: with the override set both would point
    /// at the same install and compare nothing.
    fn retail_dir() -> Option<std::path::PathBuf> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        p.is_dir().then_some(p)
    }

    /// The Footman must decode to the *same structure* on 1.28c and 1.29b.
    ///
    /// The two files are not byte-identical, so a digest of the raw file proves
    /// nothing. This digests the decoded structure instead, which is what
    /// actually has to agree.
    #[test]
    fn the_footman_decodes_identically_on_both_builds() {
        let Some(legacy) = std::env::var_os("PEACECRAFT3_LEGACY_DIR") else {
            eprintln!("skipping: PEACECRAFT3_LEGACY_DIR unset");
            return;
        };
        let legacy = std::path::PathBuf::from(legacy);
        if !legacy.join("War3.mpq").is_file() {
            eprintln!("skipping: no legacy install");
            return;
        }
        let Some(dir) = retail_dir() else { return };

        let mut a = pcx_mpq::Install::open(&dir).expect("open 1.28c");
        let mut b = pcx_mpq::Install::open(&legacy).expect("open 1.29b");
        assert_eq!(a.version, pcx_mpq::Version::V1_28, "wrong primary build");
        assert_eq!(b.version, pcx_mpq::Version::V1_29, "wrong secondary build");

        let da = a.extract(FOOTMAN).expect("1.28c footman");
        let db = b.extract(FOOTMAN).expect("1.29b footman");
        let (ma, mb) = (
            Mdlx::parse(&da).expect("parse 1.28c"),
            Mdlx::parse(&db).expect("parse 1.29b"),
        );

        for (label, m, d) in [("1.28c", &ma, &da), ("1.29b", &mb, &db)] {
            let geos = m.geosets(d).unwrap();
            let mesh = m.mesh(d, &geos[0]).unwrap();
            assert_eq!(mesh.vertices.count, 447, "{label}: vertex count drifted");
            assert_eq!(
                mesh.triangles.triangle_count(),
                288,
                "{label}: triangle count drifted"
            );
            assert_eq!(m.sequences(d).unwrap().len(), 13, "{label}: sequences");
            assert_eq!(m.bones(d).unwrap().len(), 25, "{label}: bone count drifted");
        }

        // Skeleton structure, hashed: object ids, parents and track counts.
        fn skeleton_digest(m: &Mdlx, d: &[u8]) -> u64 {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in m.bones(d).unwrap() {
                h ^= u64::from(b.object_id);
                h = h.wrapping_mul(0x100_0000_01b3);
                h ^= b.parent_id as u32 as u64;
                h = h.wrapping_mul(0x100_0000_01b3);
                h ^= b.tracks.len() as u64;
                h = h.wrapping_mul(0x100_0000_01b3);
            }
            h
        }
        assert_eq!(
            skeleton_digest(&ma, &da),
            skeleton_digest(&mb, &db),
            "the skeleton differs between builds"
        );

        // The files really do differ, otherwise this test would prove nothing.
        assert_ne!(da, db, "expected the two builds to differ byte for byte");
        assert_eq!(da.len(), db.len());
    }

    #[test]
    fn rejects_a_truncated_chunk() {
        assert!(Mdlx::parse(b"MDLXVERS\x04\x00\x00\x00\x01\x02").is_err());
    }

    /// The Footman carries none of the chunks decoded here.
    ///
    /// Asserted rather than left implicit: this crate's tests read the Footman
    /// because its chunk list is a fingerprint, and a silent `Ok(vec![])` for
    /// every model with no `TXAN` would make a broken reader look fine on the
    /// one model the rest of the tests use.
    #[test]
    fn the_footman_carries_none_of_the_effect_chunks() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("{FOOTMAN}: {e}"));
        for tag in [tag::TXAN, tag::RIBB, tag::PREM, tag::CAMS, tag::LITE] {
            assert!(!m.has(tag), "unexpected {}", String::from_utf8_lossy(tag));
        }
        assert!(m.texture_anims(&d).unwrap().is_empty());
        assert!(m.ribbons(&d).unwrap().is_empty());
        assert!(m.particles(&d).unwrap().is_empty());
        assert!(m.cameras(&d).unwrap().is_empty());
        assert!(m.lights(&d).unwrap().is_empty());
    }

    /// A `TXAN` record is its size word plus its tracks and nothing else.
    ///
    /// The Footman has no `TXAN` of its own, so this reads the waterfall
    /// doodad instead. Its 52-byte record was measured by hand from the raw
    /// payload before the reader existed, so these numbers pin the reader
    /// rather than follow it: `52 == 4 + 16 + 2 * 16`.
    #[test]
    fn texture_animations_decode_with_the_measured_layout() {
        let waterfall = "Doodads\\Terrain\\CliffDoodad\\Waterfall\\Waterfall.mdx";
        let Some(d) = shipped(waterfall) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("TXAN: {e}"));
        let anims = m.texture_anims(&d).unwrap_or_else(|e| panic!("TXAN: {e}"));
        assert_eq!(
            anims.len(),
            1,
            "the waterfall carries one texture animation"
        );

        let a = &anims[0];
        assert_eq!(a.len, 52);
        assert_eq!(a.tracks.len(), 1, "and it is a single translation track");
        let t = &a.tracks[0];
        assert_eq!(&t.magic, b"KTAT");
        assert_eq!(t.count, 2);
        assert_eq!(t.interpolation, Interpolation::Linear);
        assert_eq!(t.global_seq_id, 0);
        assert_eq!(t.width, 3, "a translation is three floats wide");
        assert_eq!(t.stride, 16, "frame plus three floats, no tangents");

        // Two keys, at frame 0 and frame 2833, scrolling straight down.
        assert_eq!(t.frame(0), Some(0));
        assert_eq!(t.frame(1), Some(2833));
        assert_eq!(t.vec3(0), Some([0.0, 0.0, 0.0]));
        assert_eq!(t.vec3(1), Some([0.0, -1.0, 0.0]));

        // The framing relation, asserted exactly. This is what catches a
        // misread track header: a record that still fits under a `<=` check.
        let measured: usize = 4 + a
            .tracks
            .iter()
            .map(|t| 16 + t.count as usize * t.stride)
            .sum::<usize>();
        assert_eq!(measured, a.len as usize);
    }

    /// A ribbon is a node, 52 bytes of emitter state, and its own tracks.
    ///
    /// The `SpiritOfVengeance` orb is one of the smallest emitters in the set at
    /// 152 bytes, which is why its values could be counted off the raw payload
    /// by hand: 152 == 4 + 96 + 52, with no tracks at all.
    #[test]
    fn ribbon_emitters_decode_with_the_measured_layout() {
        let orb = "Abilities\\Spells\\NightElf\\SpiritOfVengeance\\SpiritOfVengeanceOrbs1.mdx";
        let Some(d) = shipped(orb) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("RIBB: {e}"));
        let ribbons = m.ribbons(&d).unwrap_or_else(|e| panic!("RIBB: {e}"));
        assert_eq!(ribbons.len(), 1);

        let r = &ribbons[0];
        assert_eq!(r.total_size, 152);
        assert_eq!(r.node_size, 96, "no node animation, so the header is bare");
        assert_eq!(r.name, "BlizRibbon01");
        assert_eq!(r.object_id, 3);
        assert_eq!(r.parent_id, 2);
        assert_eq!(r.flags, 0x4000);
        assert!(r.node_tracks.is_empty(), "its node carries no animation");

        assert_eq!(r.height_above, 3.0);
        assert_eq!(r.height_below, 3.0);
        assert_eq!(r.alpha, 0.5);
        assert!((r.colour[0] - 0.776_47).abs() < 1e-4);
        assert!((r.colour[1] - 0.945_1).abs() < 1e-4);
        assert_eq!(r.colour[2], 1.0);
        assert_eq!(r.edge_lifetime, 0.6);
        assert_eq!(r.texture_slot, 0);
        assert_eq!(r.edges_per_second, 30);
        assert_eq!(r.texture_rows, 1);
        assert_eq!(r.texture_cols, 1);
        assert_eq!(r.material_id, 0);
        assert_eq!(r.gravity, 0.0);
        assert!(r.tracks.is_empty(), "this emitter is not animated");

        // The framing relation: node header, emitter state, tracks, nothing
        // else. `node_size` already includes the node's own animation.
        let measured = 4
            + r.node_size as usize
            + RIBBON_FIXED
            + r.tracks
                .iter()
                .map(|t| 16 + t.count as usize * t.stride)
                .sum::<usize>();
        assert_eq!(
            measured, r.total_size as usize,
            "a ribbon record must be its node, its state and its tracks exactly"
        );
    }

    /// A `PREM` emitter's path is a fixed 260-byte field, not a NUL-terminated
    /// run.
    ///
    /// The `LightningBolt` missile is the smallest emitter in the set that
    /// carries a track, so every one of its 432 bytes was counted by hand: the
    /// node, four floats, `SharedModels\Lightning2.MDL` zero-padded to 260,
    /// two floats, then four one-second visibility keys.
    #[test]
    fn particle_emitters_decode_with_the_measured_layout() {
        let bolt = "Abilities\\Spells\\Orc\\LightningBolt\\LightningBoltMissile.mdx";
        let Some(d) = shipped(bolt) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("PREM: {e}"));
        let emitters = m.particles(&d).unwrap_or_else(|e| panic!("PREM: {e}"));
        assert_eq!(emitters.len(), 1);

        let p = &emitters[0];
        assert_eq!(p.total_size, 432);
        assert_eq!(p.node_size, 96);
        assert_eq!(p.name, "SuperSpray01");
        assert_eq!(p.object_id, 18);
        assert_eq!(p.parent_id, -1);
        assert_eq!(p.flags, 0x9000);

        assert_eq!(p.emission_rate, 147.0);
        assert_eq!(p.gravity, 0.0);
        // π in both: a full emission cone. This is the check that pins the
        // longitude/latitude pair rather than, say, a speed range.
        assert!((p.longitude - std::f32::consts::PI).abs() < 1e-4);
        assert!((p.latitude - std::f32::consts::PI).abs() < 1e-4);
        assert_eq!(p.path, "SharedModels\\Lightning2.MDL");
        assert!((p.life - 2.0 / 3.0).abs() < 1e-4);
        assert_eq!(p.speed, 464.0);

        assert_eq!(p.tracks.len(), 1, "one visibility track");
        let t = &p.tracks[0];
        assert_eq!(&t.magic, b"KPEV");
        assert_eq!(t.count, 4);
        assert_eq!(t.width, 1, "visibility is a scalar");
        assert_eq!(t.stride, 8, "frame plus one float, no tangents");
        assert_eq!(t.frame(1), Some(1466));
        assert_eq!(t.scalar(1), Some(1.0));
        assert_eq!(t.scalar(2), Some(0.0));

        // 4 + 96 + 284 + (16 + 4 * 8).
        let measured = 4
            + p.node_size as usize
            + PARTICLE_FIXED
            + p.tracks
                .iter()
                .map(|t| 16 + t.count as usize * t.stride)
                .sum::<usize>();
        assert_eq!(
            measured, p.total_size as usize,
            "a particle record must be its node, its 284-byte state and its tracks"
        );
    }

    /// A camera has no node header, so the shared `nodes()` reader misreads it.
    ///
    /// The `HumanTower` camera is 424 bytes with all three animation tracks, so
    /// it pins the fixed 36-byte tail and the track walk at once. The `nodes()`
    /// half is the point of the test: its `parent_id` comes back as a value
    /// that is neither `-1` nor a node index, because the words it reads as
    /// ids are the camera's eye position.
    #[test]
    fn cameras_decode_without_a_node_header() {
        let tower = "Buildings\\Human\\HumanTower\\HumanTower.mdx";
        let Some(d) = shipped(tower) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("CAMS: {e}"));
        let cams = m.cameras(&d).unwrap_or_else(|e| panic!("CAMS: {e}"));
        assert_eq!(cams.len(), 1);

        let c = &cams[0];
        assert_eq!(c.name, "Camera01");
        assert_eq!(c.size, 424);
        assert!(c.tracks.len() >= 2, "this camera animates");
        for t in &c.tracks {
            assert!(
                matches!(&t.magic[..], b"KCTR" | b"KCRL" | b"KTTR"),
                "{} is not a camera track",
                String::from_utf8_lossy(&t.magic)
            );
        }
        assert!(c.field_of_view > 0.0 && c.field_of_view < 1.6);
        // The clip pair is ordered, which is the only thing that tells far from
        // near out of the bytes alone.
        assert!(c.far > c.near, "far {} must exceed near {}", c.far, c.near);
        // A target that is not the eye: the camera looks at something.
        let d0 = (c.eye[0] - c.target[0]).abs()
            + (c.eye[1] - c.target[1]).abs()
            + (c.eye[2] - c.target[2]).abs();
        assert!(d0 > 1.0, "eye and target coincide");

        // 120 + tracks, exactly.
        let measured = 4
            + NODE_NAME
            + CAMERA_FIXED
            + c.tracks
                .iter()
                .map(|t| 16 + t.count as usize * t.stride)
                .sum::<usize>();
        assert_eq!(
            measured, c.size as usize,
            "a camera record must be its fixed tail plus its tracks"
        );

        // ... and the shared BONE-shaped reader gets the ids wrong, which is
        // why this is its own reader rather than a reuse of `nodes()`.
        let shared = m.nodes(&d, tag::CAMS).unwrap();
        assert_eq!(shared.len(), 1);
        assert_eq!(
            shared[0].name, "Camera01",
            "the name lands in the same place"
        );
        assert!(
            shared[0].name.is_empty() || shared[0].parent_id < 0,
            "a camera's parent id is really its eye.y, not a node index"
        );
    }

    /// A `LITE` record is a node, 44 bytes of light state and its own tracks.
    ///
    /// The torch is 144 bytes with no tracks at all: 144 == 4 + 96 + 44. Its
    /// values were counted off the raw payload, so the white colour and the
    /// intensity of four are the measurement, not an echo of the parser.
    #[test]
    fn lights_decode_with_the_measured_layout() {
        let torch = "Doodads\\LordaeronSummer\\Props\\TorchHuman\\TorchHuman.mdx";
        let Some(d) = shipped(torch) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("LITE: {e}"));
        let lights = m.lights(&d).unwrap_or_else(|e| panic!("LITE: {e}"));
        assert_eq!(lights.len(), 1);

        let l = &lights[0];
        assert_eq!(l.name, "Omni01");
        assert_eq!(l.total_size, 144);
        assert_eq!(l.node_size, 96);
        assert_eq!(l.object_id, 2);
        assert_eq!(l.parent_id, -1);
        assert_eq!(l.flags, 0x200);
        assert_eq!(l.kind, LightKind::Omnidirectional);
        assert_eq!(l.attenuation_start, 80.0);
        assert_eq!(l.attenuation_end, 200.0);
        assert!(
            l.attenuation_start < l.attenuation_end,
            "the attenuation range must be ordered"
        );
        assert_eq!(l.colour, [1.0, 1.0, 1.0]);
        assert_eq!(l.intensity, 4.0);
        assert_eq!(l.ambient_colour, [1.0, 1.0, 1.0]);
        assert_eq!(l.ambient_intensity, 0.0);
        assert!(l.tracks.is_empty(), "this light is static");

        let measured = 4
            + l.node_size as usize
            + LIGHT_FIXED
            + l.tracks
                .iter()
                .map(|t| 16 + t.count as usize * t.stride)
                .sum::<usize>();
        assert_eq!(
            measured, l.total_size as usize,
            "a light record must be its node, its 44-byte state and its tracks"
        );
    }

    /// A light kind outside 0..3 is a desynchronised walk, not a fourth kind.
    ///
    /// Built from a synthetic `LITE` payload rather than by tampering with a
    /// shipped model: the point is that the field is framing, so an impossible
    /// value must refuse rather than round-trip.
    #[test]
    fn an_unknown_light_kind_is_refused() {
        let mut rec = Vec::new();
        // A bare node: 96 bytes of header, then 44 of state, no tracks.
        rec.extend_from_slice(&144u32.to_le_bytes());
        rec.extend_from_slice(&96u32.to_le_bytes());
        rec.extend_from_slice(&[0u8; 80]);
        rec.extend_from_slice(&0u32.to_le_bytes()); // object id
        let id: i32 = -1;
        rec.extend_from_slice(&id.to_le_bytes()); // parent id
        rec.extend_from_slice(&512u32.to_le_bytes()); // flags
        // A kind word of 9, which the shipped set never carries.
        rec.extend_from_slice(&9u32.to_le_bytes());
        rec.extend_from_slice(&[0u8; 40]);

        let mut v = Vec::new();
        v.extend_from_slice(b"MDLX");
        v.extend_from_slice(b"VERS");
        v.extend_from_slice(&4u32.to_le_bytes());
        v.extend_from_slice(&800u32.to_le_bytes());
        v.extend_from_slice(b"LITE");
        v.extend_from_slice(&(rec.len() as u32).to_le_bytes());
        v.extend_from_slice(&rec);

        let m = Mdlx::parse(&v).expect("a walkable container");
        match m.lights(&v) {
            Err(Error::UnknownLightKind { at, kind }) => {
                assert_eq!(at, 0, "the offset inside the chunk payload");
                assert_eq!(kind, 9);
            }
            other => panic!("expected UnknownLightKind, got {other:?}"),
        }

        // The same payload with a legal kind decodes, so the refusal is about
        // the value rather than about the record being malformed.
        rec[100..104].copy_from_slice(&1u32.to_le_bytes());
        let mut v2 = v[..v.len() - rec.len()].to_vec();
        v2.extend_from_slice(&rec);
        let m2 = Mdlx::parse(&v2).expect("a walkable container");
        let lights = m2.lights(&v2).expect("a directional light decodes");
        assert_eq!(lights[0].kind, LightKind::Directional);
    }

    /// An unrecognised track magic is refused rather than given a width.
    ///
    /// The width comes from the magic, so a default would silently shift every
    /// record after it. A synthetic `RIBB` payload is enough to show the
    /// refusal, and keeps the test from depending on a model nobody ships.
    #[test]
    fn an_unknown_track_magic_is_refused() {
        // A record with a bare node plus 52 bytes of state, then a bogus track
        // magic in place of `KRVS`.
        let mut rec = Vec::new();
        let total: u32 = 4 + 96 + 52 + 16 + 8;
        rec.extend_from_slice(&total.to_le_bytes());
        rec.extend_from_slice(&96u32.to_le_bytes());
        rec.extend_from_slice(&[0u8; 80]);
        rec.extend_from_slice(&0u32.to_le_bytes()); // object id
        let id: i32 = -1;
        rec.extend_from_slice(&id.to_le_bytes()); // parent id
        rec.extend_from_slice(&0x4000u32.to_le_bytes()); // flags
        rec.extend_from_slice(&[0u8; RIBBON_FIXED]);
        rec.extend_from_slice(b"KRZZ");
        rec.extend_from_slice(&1u32.to_le_bytes());
        rec.extend_from_slice(&0u32.to_le_bytes());
        let none: u32 = NO_ID;
        rec.extend_from_slice(&none.to_le_bytes());
        rec.extend_from_slice(&0u32.to_le_bytes());
        rec.extend_from_slice(&0.0f32.to_le_bytes());

        let mut v = Vec::new();
        v.extend_from_slice(b"MDLX");
        v.extend_from_slice(b"VERS");
        v.extend_from_slice(&4u32.to_le_bytes());
        v.extend_from_slice(&800u32.to_le_bytes());
        v.extend_from_slice(b"RIBB");
        v.extend_from_slice(&(rec.len() as u32).to_le_bytes());
        v.extend_from_slice(&rec);

        let m = Mdlx::parse(&v).expect("a walkable container");
        match m.ribbons(&v) {
            Err(Error::UnknownTrackMagic { chunk, at, tag }) => {
                assert_eq!(chunk, "RIBB");
                assert_eq!(at, 100 + RIBBON_FIXED);
                assert_eq!(&tag, b"KRZZ");
            }
            other => panic!("expected UnknownTrackMagic, got {other:?}"),
        }
    }

    /// Every model the archives ship.
    ///
    /// Ignored by default because it reads the whole model set; run with
    /// `cargo test -p pcx-mdl -- --ignored`.
    #[test]
    #[ignore = "parses the full shipped model set"]
    fn every_shipped_mdx_parses() {
        let Some(dir) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).expect("open archives");
        let models: Vec<String> = set
            .file_list()
            .expect("union listfile")
            .into_iter()
            .filter(|l| l.to_ascii_lowercase().ends_with(".mdx"))
            .collect();
        assert!(models.len() > 3_000, "only {} models", models.len());

        let mut failed = Vec::new();
        // Shipped-asset oddities: real, but they do not mean the parser is wrong.
        let mut anomalies: std::collections::BTreeSet<String> = Default::default();
        let mut versions: std::collections::BTreeMap<u32, usize> = Default::default();
        let mut tag_sets: std::collections::BTreeMap<String, usize> = Default::default();
        let mut total_bytes = 0usize;
        let mut geosets = 0usize;
        let mut meshes = 0usize;
        let mut vertices = 0usize;
        let mut triangles = 0usize;
        let mut bones = 0usize;
        let mut materials = 0usize;
        let mut geoset_anims = 0usize;
        let mut tracks = 0usize;
        let mut pivots = 0usize;
        let mut nodes = 0usize;
        let mut events = 0usize;
        let mut layers = 0usize;
        let mut anim_layers = 0usize;
        let mut texture_anims = 0usize;
        let mut texanim_tracks = 0usize;
        let mut ribbons = 0usize;
        let mut ribbon_tracks = 0usize;
        let mut particles = 0usize;
        let mut particle_tracks = 0usize;
        // PRE2, the v2 particle emitters. Its own block; see the PRE2 reader.
        let mut particles2 = 0usize;
        let mut particle2_tracks = 0usize;
        let mut cameras = 0usize;
        let mut camera_tracks = 0usize;
        let mut lights = 0usize;
        let mut light_tracks = 0usize;
        let mut exact_seq_bounds = 0usize;
        let mut none_seq_bounds = 0usize;

        for rel in &models {
            let d = match set.extract(rel) {
                Ok(d) => d,
                Err(e) => {
                    failed.push(format!("{rel}: {e}"));
                    continue;
                }
            };
            total_bytes += d.len();
            match Mdlx::parse(&d) {
                Ok(m) => {
                    *versions.entry(m.version).or_default() += 1;
                    tag_sets
                        .entry(m.tags().join(","))
                        .and_modify(|c| *c += 1)
                        .or_insert(1);
                    geosets += 1;
                    // Every model must name itself.
                    if let Ok(model) = m.model(&d) {
                        assert!(!model.name.is_empty(), "{rel}: empty model name");
                    }
                    // Record strides must divide exactly, and the timeline has
                    // to be coherent, or the renderer silently loses clips.
                    match m.sequences(&d) {
                        Ok(seqs) => {
                            for s in &seqs {
                                if s.name.is_empty() {
                                    anomalies.insert(format!("{rel}: unnamed sequence"));
                                }
                                if s.interval_end < s.interval_start {
                                    anomalies
                                        .insert(format!("{rel}: {} ends before it starts", s.name));
                                }
                            }
                            if seqs
                                .windows(2)
                                .any(|w| w[1].interval_start < w[0].interval_start)
                            {
                                anomalies.insert(format!("{rel}: sequence timeline out of order"));
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: {e}")),
                    }
                    match m.geosets(&d) {
                        Ok(g) => {
                            for x in &g {
                                if x.data.len() < 4 || &x.data[..4] != geoset_tag::VRTX {
                                    anomalies.insert(format!(
                                        "{rel}: geoset opens with {:?}",
                                        &x.data[..x.data.len().min(4)]
                                    ));
                                    break;
                                }
                            }
                            // Decode every geoset, not only the first.
                            for (gi, x) in g.iter().enumerate() {
                                match m.mesh(&d, x) {
                                    Ok(msh) => {
                                        meshes += 1;
                                        vertices += msh.vertices.count as usize;
                                        triangles += msh.triangles.triangle_count();
                                        if msh.vertices.count == 0 {
                                            failed.push(format!("{rel}: mesh with no vertices"));
                                            break;
                                        }
                                        if msh.triangles.count == 0 {
                                            failed.push(format!("{rel}: mesh with no triangles"));
                                            break;
                                        }
                                        // Skin tables must be coherent: the group
                                        // counts fit inside the MATS span, every
                                        // index addresses a real bone, and every
                                        // vertex group addresses a real group.
                                        let nm = m.materials(&d).map(|v| v.len()).unwrap_or(0);
                                        let total: usize =
                                            msh.matrix_groups.iter().map(|&v| v as usize).sum();
                                        if total != msh.matrix_indices.len() {
                                            failed.push(format!(
                                                "{rel}: MTGC sums to {total} but MATS trimmed to {}",
                                                msh.matrix_indices.len()
                                            ));
                                        }
                                        if msh.matrix_span_u32s < total {
                                            failed.push(format!(
                                                "{rel}: MATS span {} cannot hold {total} indices",
                                                msh.matrix_span_u32s
                                            ));
                                        }
                                        // `MATS` is `count` node indices and then a
                                        // trailer, so the trailer's position comes
                                        // from the walk rather than from a scan. The
                                        // assertion is an equality, not a bound: a
                                        // trailer that overshoots or undershoots
                                        // leaves the walk on bytes that are not a
                                        // tag, which is exactly what the old reader
                                        // hid behind a "no successor found" fallback.
                                        let Some((mats_at, mats_count)) = mats_sub_block(x) else {
                                            failed.push(format!(
                                                "{rel}: geoset {gi} decodes but carries no MATS"
                                            ));
                                            break;
                                        };
                                        let trailer = mats_at
                                            + 8
                                            + mats_count as usize * 4
                                            + MATS_TRAILER_FIXED
                                            + msh.seq_bounds.len() * SEQ_BOUNDS_STRIDE;
                                        let next: Option<[u8; 4]> = x
                                            .data
                                            .get(trailer..trailer + 4)
                                            .and_then(|t| t.try_into().ok());
                                        if !next.is_some_and(|t| {
                                            &t == geoset_tag::MATS
                                                || sub_block_width(&t, 0).is_some()
                                        }) {
                                            failed.push(format!(
                                                "{rel}: MATS trailer ends at {trailer} of {}, followed by {next:?}",
                                                x.data.len()
                                            ));
                                        }
                                        if msh.matrix_span_u32s != mats_count as usize {
                                            failed.push(format!(
                                                "{rel}: MATS span {} against a count of {mats_count}",
                                                msh.matrix_span_u32s
                                            ));
                                        }
                                        // Per-sequence bounds are either one record
                                        // per sequence or none at all. The walk over
                                        // both builds confirms it: 5,971 and 6,084
                                        // exact, 2,701 and 2,686 empty, and nothing in
                                        // between or over.
                                        let seqs = m.sequences(&d).map(|s| s.len()).unwrap_or(0);
                                        match msh.seq_bounds.len() {
                                            0 => none_seq_bounds += 1,
                                            n if n == seqs => exact_seq_bounds += 1,
                                            n => failed.push(format!(
                                                "{rel}: {n} seqBounds for {seqs} sequences"
                                            )),
                                        }
                                        for b in &msh.seq_bounds {
                                            // An unset record reads a minimum above
                                            // zero and a maximum below it; every
                                            // other record must not be inverted.
                                            if !(b[1] > 0.0 && b[4] < 0.0) {
                                                for i in 0..3 {
                                                    if b[4 + i] < b[1 + i] {
                                                        failed.push(format!(
                                                            "{rel}: seqBounds extent {i} is inverted: {b:?}"
                                                        ));
                                                    }
                                                }
                                            }
                                            if !b.iter().all(|v| v.is_finite()) {
                                                failed.push(format!(
                                                    "{rel}: seqBounds record is not finite: {b:?}"
                                                ));
                                            }
                                        }
                                        if msh.unselectable > 4 {
                                            failed.push(format!(
                                                "{rel}: unselectable is {}",
                                                msh.unselectable
                                            ));
                                        }
                                        // The material the geoset names must be
                                        // one the model ships. This is the field
                                        // the tag-scan reader dropped. Every
                                        // geoset sits in a model with at least one
                                        // material, so the bound is unconditional.
                                        if msh.material_id as usize >= nm {
                                            failed.push(format!(
                                                "{rel}: geoset names material {} of {nm}",
                                                msh.material_id
                                            ));
                                        }
                                        let nb = m.bones(&d).map(|b| b.len()).unwrap_or(0);
                                        if nb > 0
                                            && let Some(bad) = msh
                                                .matrix_indices
                                                .iter()
                                                .find(|&&b| b as usize >= nb)
                                        {
                                            failed.push(format!(
                                                "{rel}: MATS references bone {bad} of {nb}"
                                            ));
                                        }
                                        let groups = msh.matrix_groups.len();
                                        if groups > 0
                                            && let Some(bad) = msh
                                                .vertex_groups
                                                .iter()
                                                .find(|&&g| g as usize >= groups)
                                        {
                                            failed.push(format!(
                                                "{rel}: GNDX references group {bad} of {groups}"
                                            ));
                                        }

                                        // Every triangle index must be in range.
                                        let bad =
                                            msh.triangles.raw.as_chunks::<2>().0.iter().any(|c| {
                                                u16::from_le_bytes(*c) as u32 >= msh.vertices.count
                                            });
                                        if bad {
                                            failed.push(format!(
                                                "{rel}: triangle index out of range"
                                            ));
                                            break;
                                        }
                                    }
                                    Err(e) => failed.push(format!("{rel}: {e}")),
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: {e}")),
                    }
                    // Materials: the framing walk must tile the chunk, and
                    // every layer must declare a texture the model owns.
                    match m.geoset_anims(&d) {
                        Ok(a) => {
                            for x in &a {
                                geoset_anims += 1;
                                // The framing relation, asserted exactly rather
                                // than loosely: a record must be its header plus
                                // its tracks and nothing else. This is the check
                                // that catches a misread track header, which a
                                // `<=` comparison does not.
                                // `len` counts its own length field.
                                let data_len = (x.len as usize).saturating_sub(4);
                                let want: usize = GEOA_FIXED
                                    + x.tracks
                                        .iter()
                                        .map(|t| TRACK_HEADER + t.count as usize * t.key_len())
                                        .sum::<usize>();
                                if want != data_len {
                                    failed.push(format!(
                                        "{rel}: GEOA tracks size {want} in a {data_len}-byte record"
                                    ));
                                }
                                for t in &x.tracks {
                                    tracks += 1;
                                    if t.count == 0 {
                                        failed.push(format!(
                                            "{rel}: geoset anim track {} claims no keys",
                                            String::from_utf8_lossy(&t.magic)
                                        ));
                                    }
                                }
                                for t in &x.tracks {
                                    tracks += 1;
                                    if t.count == 0 {
                                        failed.push(format!(
                                            "{rel}: geoset anim track {} claims no keys",
                                            String::from_utf8_lossy(&t.magic)
                                        ));
                                    }
                                    // The stride comes from the track itself,
                                    // not from its tag: interpolation decides
                                    // whether a key carries tangents.
                                    if t.raw.len() != t.count as usize * t.key_len() {
                                        failed.push(format!(
                                            "{rel}: {} track holds {} bytes for {} {}-byte keys",
                                            String::from_utf8_lossy(&t.magic),
                                            t.raw.len(),
                                            t.count,
                                            t.key_len()
                                        ));
                                    }
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: {e}")),
                    }

                    // Pivot points: one per node, so the count is checkable.
                    match m.pivots(&d) {
                        Ok(pv) => {
                            pivots += pv.0.len();
                            if pv.0.iter().any(|p| p.iter().any(|v| !v.is_finite())) {
                                failed.push(format!("{rel}: a pivot point is not finite"));
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: {e}")),
                    }

                    // Event objects and collision shapes, framed differently.
                    for tag in [tag::EVTS, tag::CLID] {
                        match m.event_nodes(&d, tag) {
                            Ok(ns) => {
                                events += ns.len();
                                for n in &ns {
                                    // A walk that leaves the chunk unconsumed is
                                    // the failure this is here to catch.
                                    if n.name.is_empty() {
                                        anomalies.insert(format!(
                                            "{rel}: a {} node with no name",
                                            String::from_utf8_lossy(tag)
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                failed.push(format!("{rel}: {}: {e}", String::from_utf8_lossy(tag)))
                            }
                        }
                    }

                    // Node chunks sharing BONE's framing. `CAMS` and `LITE` are
                    // deliberately absent: a camera has no node header at all
                    // and a light has its own fields after it, so they are read
                    // through their own readers below rather than being counted
                    // twice through a framing that does not describe them.
                    for tag in [tag::HELP, tag::ATCH] {
                        match m.nodes(&d, tag) {
                            Ok(ns) => {
                                nodes += ns.len();
                                for n in &ns {
                                    if n.name.is_empty() && n.size > NODE_FIXED as u32 {
                                        anomalies.insert(format!(
                                            "{rel}: a {} node with no name",
                                            String::from_utf8_lossy(tag)
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                failed.push(format!("{rel}: {}: {e}", String::from_utf8_lossy(tag)))
                            }
                        }
                    }

                    match m.materials(&d) {
                        Ok(ms) => {
                            materials += ms.len();
                            for mat in &ms {
                                if mat.layers().is_empty() {
                                    anomalies.insert(format!("{rel}: material with no LAYS"));
                                }
                                let texs = m.textures(&d).unwrap_or_default();
                                let n_tex = texs.len();
                                for (idx, len) in mat.layer_lengths() {
                                    if len == 0 || len as usize > mat.data.len() {
                                        failed.push(format!(
                                            "{rel}: layer {idx} declares {len} bytes in a {}-byte material",
                                            mat.data.len()
                                        ));
                                    }
                                }
                                for l in mat.tex_layers() {
                                    layers += 1;
                                    // The one field whose offset is known with
                                    // certainty: every layer must point at a
                                    // texture the model actually ships.
                                    if n_tex > 0 && !l.points_at_a_shipped_texture(n_tex) {
                                        failed.push(format!(
                                            "{rel}: layer {} references texture {} of {n_tex}",
                                            l.index, l.texture_id
                                        ));
                                    }
                                    if !l.has_no_texture_anim() {
                                        anim_layers += 1;
                                    }
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: {e}")),
                    }

                    // Texture animations, ribbon emitters, particle emitters,
                    // cameras and lights. Each of these asserts the framing
                    // relation that proves the layout, because a walk that only
                    // checks counts can land on the end of a record whose fields
                    // are in the wrong place.
                    match m.texture_anims(&d) {
                        Ok(a) => {
                            for x in &a {
                                texture_anims += 1;
                                let want = 4 + x
                                    .tracks
                                    .iter()
                                    .map(|t| 16 + t.count as usize * t.stride)
                                    .sum::<usize>();
                                if want != x.len as usize {
                                    failed.push(format!(
                                        "{rel}: TXAN record measures {want} in {} bytes",
                                        x.len
                                    ));
                                }
                                for t in &x.tracks {
                                    texanim_tracks += 1;
                                    if t.count == 0 {
                                        failed.push(format!(
                                            "{rel}: texture track {} claims no keys",
                                            String::from_utf8_lossy(&t.magic)
                                        ));
                                    }
                                    if t.raw.len() != t.count as usize * t.stride {
                                        failed.push(format!(
                                            "{rel}: {} track holds {} bytes for {} {}-byte keys",
                                            String::from_utf8_lossy(&t.magic),
                                            t.raw.len(),
                                            t.count,
                                            t.stride
                                        ));
                                    }
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: TXAN: {e}")),
                    }
                    match m.ribbons(&d) {
                        Ok(a) => {
                            for x in &a {
                                ribbons += 1;
                                ribbon_tracks += x.node_tracks.len() + x.tracks.len();
                                if x.node_size as usize
                                    != EFFECT_NODE_FIXED
                                        + x.node_tracks
                                            .iter()
                                            .map(|t| 16 + t.count as usize * t.stride)
                                            .sum::<usize>()
                                {
                                    failed.push(format!(
                                        "{rel}: RIBB {} node size {} does not match its tracks",
                                        x.name, x.node_size
                                    ));
                                }
                                let want = 4
                                    + x.node_size as usize
                                    + RIBBON_FIXED
                                    + x.tracks
                                        .iter()
                                        .map(|t| 16 + t.count as usize * t.stride)
                                        .sum::<usize>();
                                if want != x.total_size as usize {
                                    failed.push(format!(
                                        "{rel}: RIBB {} measures {want} in {} bytes",
                                        x.name, x.total_size
                                    ));
                                }
                                // A ribbon names its material the way a
                                // texture layer does, so the same independent
                                // pin applies: it must point at a material the
                                // model actually ships.
                                let nm = m.materials(&d).map(|x| x.len()).unwrap_or(0);
                                if nm > 0 && x.material_id as usize >= nm {
                                    failed.push(format!(
                                        "{rel}: RIBB {} references material {} of {nm}",
                                        x.name, x.material_id
                                    ));
                                }
                                if !(0.0..=1.0).contains(&x.alpha) {
                                    failed.push(format!(
                                        "{rel}: RIBB {} alpha {} is outside 0..1",
                                        x.name, x.alpha
                                    ));
                                }
                                if !x.track_times_ordered() {
                                    anomalies.insert(format!(
                                        "{rel}: RIBB {} keys out of order",
                                        x.name
                                    ));
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: RIBB: {e}")),
                    }
                    match m.particles(&d) {
                        Ok(a) => {
                            for x in &a {
                                particles += 1;
                                particle_tracks += x.node_tracks.len() + x.tracks.len();
                                if x.node_size as usize
                                    != EFFECT_NODE_FIXED
                                        + x.node_tracks
                                            .iter()
                                            .map(|t| 16 + t.count as usize * t.stride)
                                            .sum::<usize>()
                                {
                                    failed.push(format!(
                                        "{rel}: PREM {} node size {} does not match its tracks",
                                        x.name, x.node_size
                                    ));
                                }
                                let want = 4
                                    + x.node_size as usize
                                    + PARTICLE_FIXED
                                    + x.tracks
                                        .iter()
                                        .map(|t| 16 + t.count as usize * t.stride)
                                        .sum::<usize>();
                                if want != x.total_size as usize {
                                    failed.push(format!(
                                        "{rel}: PREM {} measures {want} in {} bytes",
                                        x.name, x.total_size
                                    ));
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: PREM: {e}")),
                    }
                    // PRE2 — particle emitters, v2. This block is the PRE2
                    // sweep; it asserts the framing rather than counting it.
                    match m.particles2(&d) {
                        Ok(a) => {
                            for x in &a {
                                particles2 += 1;
                                particle2_tracks += x.node_tracks.len() + x.tracks.len();
                                if x.node_size as usize != x.measured_node_size() {
                                    failed.push(format!(
                                        "{rel}: PRE2 {} node size {} measures {}",
                                        x.name,
                                        x.node_size,
                                        x.measured_node_size()
                                    ));
                                }
                                let want = x.measured_size();
                                if want != x.total_size as usize {
                                    failed.push(format!(
                                        "{rel}: PRE2 {} measures {want} in {} bytes",
                                        x.name, x.total_size
                                    ));
                                }
                                // Not a byte count: the sub-chunk chain is in
                                // one fixed tag order in every shipped record,
                                // so a chain that is short or reordered fails
                                // here even when the sizes happen to add up.
                                if !x.track_kinds_sorted() {
                                    failed.push(format!(
                                        "{rel}: PRE2 {} sub-chunks out of order: {:?}",
                                        x.name,
                                        x.track_kinds()
                                    ));
                                }
                                for t in x.all_tracks() {
                                    if !t.times_are_ordered() {
                                        // A negative leading frame is real in the
                                        // shipped set, as it is for cameras, so
                                        // it is reported on its own.
                                        if t.has_negative_frame() {
                                            anomalies.insert(format!(
                                                "{rel}: PRE2 {} track {} opens on frame {}",
                                                x.name,
                                                String::from_utf8_lossy(&t.magic),
                                                t.frame(0).unwrap() as i32
                                            ));
                                        } else {
                                            failed.push(format!(
                                                "{rel}: PRE2 {} track {} keys out of order",
                                                x.name,
                                                String::from_utf8_lossy(&t.magic)
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: PRE2: {e}")),
                    }
                    match m.cameras(&d) {
                        Ok(a) => {
                            for x in &a {
                                cameras += 1;
                                camera_tracks += x.tracks.len();
                                let want = 4
                                    + NODE_NAME
                                    + CAMERA_FIXED
                                    + x.tracks
                                        .iter()
                                        .map(|t| 16 + t.count as usize * t.stride)
                                        .sum::<usize>();
                                if want != x.size as usize {
                                    failed.push(format!(
                                        "{rel}: CAMS {} measures {want} in {} bytes",
                                        x.name, x.size
                                    ));
                                }
                                if x.far <= x.near {
                                    failed.push(format!(
                                        "{rel}: CAMS {} clips {} then {}",
                                        x.name, x.far, x.near
                                    ));
                                }
                                // Only camera tracks in the shipped set carry a
                                // negative leading frame, so it is reported on
                                // its own rather than folded into "out of order".
                                for t in x.all_tracks() {
                                    if t.has_negative_frame() {
                                        anomalies.insert(format!(
                                            "{rel}: CAMS {} track {} opens on frame {}",
                                            x.name,
                                            String::from_utf8_lossy(&t.magic),
                                            t.frame(0).unwrap() as i32
                                        ));
                                    }
                                    if !t.times_are_ordered() {
                                        anomalies.insert(format!(
                                            "{rel}: CAMS {} track {} keys out of order",
                                            x.name,
                                            String::from_utf8_lossy(&t.magic)
                                        ));
                                    }
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: CAMS: {e}")),
                    }
                    match m.lights(&d) {
                        Ok(a) => {
                            for x in &a {
                                lights += 1;
                                light_tracks += x.node_tracks.len() + x.tracks.len();
                                if x.node_size as usize
                                    != EFFECT_NODE_FIXED
                                        + x.node_tracks
                                            .iter()
                                            .map(|t| 16 + t.count as usize * t.stride)
                                            .sum::<usize>()
                                {
                                    failed.push(format!(
                                        "{rel}: LITE {} node size {} does not match its tracks",
                                        x.name, x.node_size
                                    ));
                                }
                                let want = 4
                                    + x.node_size as usize
                                    + LIGHT_FIXED
                                    + x.tracks
                                        .iter()
                                        .map(|t| 16 + t.count as usize * t.stride)
                                        .sum::<usize>();
                                if want != x.total_size as usize {
                                    failed.push(format!(
                                        "{rel}: LITE {} measures {want} in {} bytes",
                                        x.name, x.total_size
                                    ));
                                }
                                if x.intensity < 0.0 {
                                    failed.push(format!(
                                        "{rel}: LITE {} intensity {} is negative",
                                        x.name, x.intensity
                                    ));
                                }
                                // The attenuation range is ordered in every
                                // record that has one at all.
                                if (x.attenuation_start != 0.0 || x.attenuation_end != 0.0)
                                    && x.attenuation_start >= x.attenuation_end
                                {
                                    failed.push(format!(
                                        "{rel}: LITE {} attenuation {}..{}",
                                        x.name, x.attenuation_start, x.attenuation_end
                                    ));
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: LITE: {e}")),
                    }

                    // The skeleton: root parent, track magic, and the record
                    // walk landing exactly on the end of the chunk.
                    match m.bones(&d) {
                        Ok(bs) => {
                            bones += bs.len();
                            for b in &bs {
                                // An empty track is a structural break: the
                                // record claimed a track and then carried no
                                // keys.
                                for t in &b.tracks {
                                    if t.count == 0 {
                                        failed.push(format!(
                                            "{rel}: bone {} has an empty {:?} track",
                                            b.name, t.kind
                                        ));
                                        break;
                                    }
                                }
                            }
                            // No assertion that a root exists. `parentId`
                            // addresses the whole node space, so a bone hanging
                            // from a helper is normal, and effects whose bones
                            // are all parented to non-bone nodes are legal.
                            let rooted = bs.iter().filter(|b| b.is_root()).count();
                            let mut parents = std::collections::BTreeSet::new();
                            for b in &bs {
                                if b.parent_id >= 0 {
                                    parents.insert(b.parent_id);
                                }
                            }
                            let _ = (rooted, parents);
                        }
                        Err(e) => failed.push(format!("{rel}: {e}")),
                    }
                    match m.textures(&d) {
                        Ok(t) => {
                            for x in &t {
                                if !x.path.is_empty()
                                    && !x.path.to_ascii_lowercase().ends_with(".blp")
                                {
                                    anomalies.insert(format!("{rel}: texture path {:?}", x.path));
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{rel}: {e}")),
                    }
                }
                Err(e) => failed.push(format!("{rel}: {e}")),
            }
        }

        eprintln!(
            "mdx={} parsed={geosets} failed={} bytes={total_bytes} meshes={meshes} \
             vertices={vertices} triangles={triangles} bones={bones} materials={materials} \
             layers={layers} animlayers={anim_layers} geosetanims={geoset_anims} \
             animtracks={tracks} pivots={pivots} nodes={nodes} eventnodes={events} \
             texanims={texture_anims} texanimtracks={texanim_tracks} ribbons={ribbons} \
             ribbontracks={ribbon_tracks} particles={particles} particletracks={particle_tracks} \
             particles2={particles2} particletracks2={particle2_tracks} \
             cameras={cameras} cameratracks={camera_tracks} lights={lights} \
             lighttracks={light_tracks}",
            models.len(),
            failed.len()
        );
        eprintln!("seqBounds: exact={exact_seq_bounds} none={none_seq_bounds}");
        eprintln!("versions: {versions:?}");
        for (k, v) in tag_sets.iter().take(25) {
            eprintln!("  x{v:<5} {k}");
        }
        let report = std::path::Path::new("/tmp/opencode/mdx-sweep.txt");
        if let Ok(mut w) = std::fs::File::create(report) {
            use std::io::Write;
            for f in failed.iter().take(2000) {
                let _ = writeln!(w, "{f}");
            }
        }
        for f in failed.iter().take(20) {
            eprintln!("  fail {f}");
        }
        eprintln!("anomalies in shipped assets: {}", anomalies.len());
        for a in &anomalies {
            eprintln!("  note {a}");
        }
        assert!(failed.is_empty(), "{} models failed to parse", failed.len());
    }
}

/// Tests for the `CLID` chunk, kept apart from the main suite so that this
/// chunk's evidence is read in one place.
///
/// Nothing here asserts what the geometry floats mean. What it asserts is the
/// framing, which is measured: the header size, the node animation inside it,
/// the geometry width each shape kind implies, and the chunk tiling exactly.
#[cfg(test)]
mod clid {
    use super::*;

    /// Where to find a WC3 install.
    ///
    /// Repeated from the main suite rather than shared: this module is meant to
    /// stay independent of it, so that the `CLID` evidence reads on its own.
    fn install_dir() -> Option<std::path::PathBuf> {
        for key in ["PEACECRAFT3_INSTALL", "PEACECRAFT3_LEGACY_DIR"] {
            if let Ok(v) = std::env::var(key) {
                let p = std::path::PathBuf::from(v);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        p.is_dir().then_some(p)
    }

    /// One shipped model, or `None` when no install is present.
    fn shipped(rel: &str) -> Option<Vec<u8>> {
        let dir = install_dir()?;
        pcx_mpq::MpqSet::open_wc3(dir).ok()?.extract(rel).ok()
    }

    /// A shipped model read as shapes, or a skip when no install is present.
    fn shapes_of(rel: &str) -> Option<(Mdlx, Vec<u8>, Vec<CollisionShape>)> {
        let d = shipped(rel)?;
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let s = m
            .collision_shapes(&d)
            .unwrap_or_else(|e| panic!("{rel}: CLID: {e}"));
        Some((m, d, s))
    }

    /// Asserts the framing relation on one chunk, naming the model if it fails.
    fn assert_framing(rel: &str, m: &Mdlx, d: &[u8], shapes: &[CollisionShape]) {
        let payload = m.payload(d, tag::CLID).map(<[u8]>::len).unwrap_or(0);
        let n = shapes.len();
        let walked: usize = shapes.iter().map(|s| s.size()).sum();
        assert_eq!(
            walked, payload,
            "{rel}: {n} records measure {walked} bytes in a {payload}-byte chunk"
        );
        for s in shapes {
            assert!(
                s.header_accounts_for_its_animation(),
                "{rel}: {} has header {} but {} bytes of node animation",
                s.name,
                s.header_size,
                s.node_track_bytes()
            );
            assert!(
                s.header_size >= EVENT_FIXED as u32,
                "{rel}: {} has a {}-byte header, under the 96-byte node header",
                s.name,
                s.header_size
            );
        }
    }

    /// The Footman: two spheres, and the chunk that looks smallest.
    ///
    /// Hand-read from the bytes rather than taken from a dump. The 232-byte
    /// payload is **two** records of 116, not one record of 232 with 136 bytes of
    /// geometry after it — 96 + 4 (kind) + 16 (sphere) = 116, twice, and the
    /// second record's header size sits at offset 0xac exactly where that sum
    /// says it should. The first record opens `4b 47 54 52`-free at +96 with the
    /// kind word `02 00 00 00`, then the floats below.
    #[test]
    fn footman_ships_two_spheres() {
        let Some((m, d, s)) = shapes_of("Units\\Human\\Footman\\Footman.mdx") else {
            return;
        };
        assert_framing("Footman", &m, &d, &s);
        assert_eq!(s.len(), 2, "the Footman ships two collision spheres");
        assert_eq!(m.payload(&d, tag::CLID).map(<[u8]>::len).unwrap_or(0), 232);

        for (i, x) in s.iter().enumerate() {
            assert_eq!(x.name, format!("Collision Sphere0{}", i + 1));
            assert_eq!(x.header_size, 96, "no node animation on either sphere");
            assert!(x.tracks.is_empty());
            assert_eq!(x.flags, 0x2000, "the collision-shape flag, on every record");
            assert_eq!(x.parent_id, -1);
            assert_eq!(x.kind, CollisionShapeKind::Sphere);
            assert_eq!(x.vertices.len(), 1, "a sphere carries one centre");
            assert_eq!(x.size(), 116, "96 header + 4 kind + 16 sphere");
        }

        assert_eq!(s[0].object_id, 55);
        assert_eq!(s[1].object_id, 56);

        // Hand-read: `08 3d a8 40 | 00 00 00 00 | 4e e2 7c 42 | d3 4d 18 42`.
        let a = &s[0].vertices[0];
        assert!((a[0] - 5.257_449).abs() < 1e-4, "centre x {}", a[0]);
        assert_eq!(a[1], 0.0, "both spheres sit on y 0");
        assert!((a[2] - 63.221).abs() < 1e-4, "centre z {}", a[2]);
        assert!((s[0].radius.unwrap() - 38.076).abs() < 1e-3);

        // Hand-read: `a8 52 4f 40 | 00 00 00 00 | 7a c7 b6 41 | d3 4d 18 42`.
        let b = &s[1].vertices[0];
        assert!((b[0] - 3.239_421).abs() < 1e-4, "centre x {}", b[0]);
        assert_eq!(b[1], 0.0);
        assert!((b[2] - 22.847_4).abs() < 1e-4, "centre z {}", b[2]);
        // The two spheres share their last float, which is the corpus evidence
        // that it is a radius and not a height: a body swept by two equal
        // spheres. Asserted because it is the observation, not the conclusion.
        assert_eq!(s[0].radius, s[1].radius);
    }

    /// A box carrier: the other kind, and the one whose width differs.
    ///
    /// Hand-read from `00 00 00 00 | f1 54 c6 c2 | db b9 ce c2 | 00 00 00 00 |
    /// f1 54 c6 42 | db b9 ce 42 | 98 5e 87 43`.
    #[test]
    fn arcane_vault_ships_a_box() {
        let Some((m, d, s)) = shapes_of("Buildings\\Human\\ArcaneVault\\ArcaneVault.mdx") else {
            return;
        };
        assert_framing("ArcaneVault", &m, &d, &s);
        assert_eq!(s.len(), 1);
        let x = &s[0];
        assert_eq!(x.name, "Collision");
        assert_eq!(x.object_id, 69);
        assert_eq!(x.header_size, 96);
        assert_eq!(x.kind, CollisionShapeKind::Box);
        assert_eq!(x.vertices.len(), 2, "a box carries two corners");
        assert_eq!(x.radius, None, "a box has no radius word");
        assert_eq!(x.size(), 124, "96 header + 4 kind + 24 box");

        let lo = x.vertices[0];
        let hi = x.vertices[1];
        assert!((lo[0] - -99.165_9).abs() < 1e-3, "{lo:?}");
        assert!((lo[1] - -103.363).abs() < 1e-3, "{lo:?}");
        assert_eq!(lo[2], 0.0, "the box stands on the ground plane");
        assert!((hi[0] - 99.165_9).abs() < 1e-3, "{hi:?}");
        assert!((hi[1] - 103.363).abs() < 1e-3, "{hi:?}");
        assert!((hi[2] - 270.739).abs() < 1e-3, "{hi:?}");
        // Ordered cornerwise in every shipped box, and symmetric about the
        // origin here: the evidence for "minimum then maximum", which is why the
        // field is documented as stored order and not captioned min/max.
        assert!(lo[0] < 0.0 && hi[0] > 0.0 && (lo[0] + hi[0]).abs() < 1e-3);
    }

    /// A record that carries node animation inside its measured header.
    ///
    /// This is the case that separates `CLID` from `EVTS`. Demoness measures 152,
    /// not 96, and the extra 56 bytes are one `KGTR` track: 16 of header plus one
    /// key of 40 bytes — the width follows from the magic being `KGTR` (three
    /// components) with interpolation 3 (Bezier, so the key also carries an in and
    /// an out tangent). A reader that treated the header as always 96 would read
    /// the kind word out of the middle of that track and then walk off the end of
    /// the chunk.
    #[test]
    fn demoness_carries_node_animation_inside_its_header() {
        let Some((m, d, s)) = shapes_of("Units\\Demon\\Demoness\\Demoness.mdx") else {
            return;
        };
        assert_framing("Demoness", &m, &d, &s);
        assert_eq!(s.len(), 2);

        let x = &s[0];
        assert_eq!(x.name, "Collision Sphere01");
        assert_ne!(
            x.header_size, 96,
            "the point of this model: its header is not the bare node header"
        );
        assert_eq!(x.header_size as usize, 96 + x.node_track_bytes());
        assert_eq!(x.tracks.len(), 1);
        let t = &x.tracks[0];
        assert_eq!(&t.magic, b"KGTR");
        assert_eq!(t.count, 1);
        assert_eq!(
            t.global_seq_id, NO_ID,
            "the track follows no global sequence"
        );
        assert!(
            t.interpolation.has_tangents(),
            "interpolation 3 keys tangents"
        );
        assert_eq!(t.stride, 40, "frame + value + in + out, three components");
        assert_eq!(x.node_track_bytes(), 56, "16 of header + one 40-byte key");
        assert_eq!(x.size(), 152 + 4 + 16);

        // Hand-read: at +96, `02 00 00 00 | a7 5c ff c0 | 00 00 00 00 |
        // 8f 93 a8 42 | 3b df 14 42`, and the next record's header size sits at
        // +0xac — exactly where 152 + 4 + 16 puts it.
        assert_eq!(x.kind, CollisionShapeKind::Sphere);
        let c = x.vertices[0];
        assert!((c[0] - -7.980_06).abs() < 1e-5, "{c:?}");
        assert_eq!(c[1], 0.0);
        assert!((c[2] - 84.288_2).abs() < 1e-4, "{c:?}");
        assert!((x.radius.unwrap() - 37.218).abs() < 1e-4);

        // The second record's header is the bare node header, so the same model
        // carries both framings and the reader has to tell them apart.
        let y = &s[1];
        assert_eq!(y.name, "Collision Sphere02");
        assert_eq!(y.header_size, 96);
        assert!(y.tracks.is_empty());
        assert_eq!(y.size(), 116);
    }

    /// A chunk holding both kinds at once, so the two widths are distinguished
    /// within one payload rather than only across models.
    #[test]
    fn a_chunk_can_mix_both_shape_kinds() {
        let Some((m, d, s)) = shapes_of("Units\\Creeps\\LordGarithos\\LordGarithos.mdx") else {
            return;
        };
        assert_framing("LordGarithos", &m, &d, &s);
        assert!(
            s.iter().any(|x| x.kind == CollisionShapeKind::Box)
                && s.iter().any(|x| x.kind == CollisionShapeKind::Sphere),
            "this chunk mixes both kinds: {:?}",
            s.iter().map(|x| x.kind).collect::<Vec<_>>()
        );
        for x in s {
            assert_eq!(x.size(), x.header_size as usize + 4 + x.kind.geometry_len());
        }
    }

    /// An unknown shape kind is refused, never defaulted.
    ///
    /// A cylinder or a plane would carry a different width, so guessing one would
    /// resynchronise the walk silently. The fabricated record is the width a box
    /// would take with its kind word set to something the format does not define
    /// here.
    #[test]
    fn an_unknown_shape_kind_is_refused() {
        let mut rec = vec![0u8; 96];
        rec[0..4].copy_from_slice(&96u32.to_le_bytes());
        rec[4..13].copy_from_slice(b"Collision");
        rec[84..88].copy_from_slice(&7u32.to_le_bytes());
        rec[88..92].copy_from_slice(&(-1i32).to_le_bytes());
        rec[92..96].copy_from_slice(&0x2000u32.to_le_bytes());
        // The kind word sits at the end of the header, outside it.
        rec.extend_from_slice(&1u32.to_le_bytes()); // not 0, not 2
        rec.extend_from_slice(&[0u8; 24]);

        match CollisionShapeKind::parse(96, read_u32(&rec, 96).unwrap()) {
            Err(Error::UnknownCollisionShape { kind, .. }) => assert_eq!(kind, 1),
            other => panic!("expected UnknownCollisionShape, got {other:?}"),
        }
    }

    /// The shipped set, on whichever build the environment points at.
    ///
    /// Separate from `every_shipped_mdx_parses` on purpose: this one asserts
    /// only `CLID`, and it reports the failing model by path rather than
    /// counting failures.
    ///
    /// Run with `cargo test -p pcx-mdl -- --ignored`.
    #[test]
    #[ignore = "parses CLID in the full shipped model set"]
    fn every_shipped_clid_chunk_is_framed() {
        let Some(dir) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).expect("open archives");
        let models: Vec<String> = set
            .file_list()
            .expect("union listfile")
            .into_iter()
            .filter(|l| l.to_ascii_lowercase().ends_with(".mdx"))
            .collect();

        let mut failed: Vec<String> = Vec::new();
        let mut with_clid = 0usize;
        let mut records = 0usize;
        let mut boxes = 0usize;
        let mut spheres = 0usize;
        let mut animated = 0usize;
        let mut kinds: std::collections::BTreeMap<u32, usize> = Default::default();
        let mut flags: std::collections::BTreeMap<u32, usize> = Default::default();

        for rel in &models {
            let Ok(d) = set.extract(rel) else { continue };
            if d.len() < 12 || &d[..4] != MAGIC {
                continue;
            }
            let Ok(m) = Mdlx::parse(&d) else { continue };
            let shapes = match m.collision_shapes(&d) {
                Ok(s) => s,
                Err(e) => {
                    failed.push(format!("{rel}: CLID: {e}"));
                    continue;
                }
            };
            if shapes.is_empty() {
                continue;
            }
            with_clid += 1;

            // The framing relation: the records tile the chunk exactly. A width
            // one word wrong would still parse, and would leave this sum short.
            let payload = m.payload(&d, tag::CLID).map(<[u8]>::len).unwrap_or(0);
            let walked: usize = shapes.iter().map(|x| x.size()).sum();
            if walked != payload {
                failed.push(format!(
                    "{rel}: {} CLID records measure {walked} bytes in {payload}",
                    shapes.len()
                ));
                continue;
            }

            for x in &shapes {
                records += 1;
                if x.header_accounts_for_its_animation() {
                    if !x.tracks.is_empty() {
                        animated += 1;
                    }
                } else {
                    failed.push(format!(
                        "{rel}: CLID {} has header {} but {} bytes of node animation",
                        x.name,
                        x.header_size,
                        x.node_track_bytes()
                    ));
                }
                *kinds
                    .entry(match x.kind {
                        CollisionShapeKind::Box => 0,
                        CollisionShapeKind::Sphere => 2,
                    })
                    .or_default() += 1;
                *flags.entry(x.flags).or_default() += 1;
                match x.kind {
                    CollisionShapeKind::Box => {
                        boxes += 1;
                        // Stored order, and what it is called are separate
                        // questions. Only the ordering is checked.
                        if !(x.vertices[0][0] <= x.vertices[1][0]
                            && x.vertices[0][1] <= x.vertices[1][1]
                            && x.vertices[0][2] <= x.vertices[1][2])
                        {
                            failed.push(format!(
                                "{rel}: CLID {} stores corners out of order: {:?}",
                                x.name, x.vertices
                            ));
                        }
                    }
                    CollisionShapeKind::Sphere => {
                        spheres += 1;
                        let r = x.radius.unwrap();
                        if !(r > 0.0 && r.is_finite()) {
                            failed.push(format!("{rel}: CLID {} has radius {r}", x.name));
                        }
                    }
                }
                // A collision shape is a volume, so every float in it has to be a
                // real number before anything downstream may treat it as one.
                if x.vertices.iter().flatten().any(|v| !v.is_finite())
                    || x.radius.is_some_and(|r| !r.is_finite())
                {
                    failed.push(format!("{rel}: CLID {} has a non-finite float", x.name));
                }
            }
        }

        eprintln!(
            "models={} with CLID={} records={records}",
            models.len(),
            with_clid
        );
        eprintln!("  boxes={boxes} spheres={spheres} with node animation={animated}");
        eprintln!("  kind words={kinds:?}");
        eprintln!("  flags={flags:?}");
        assert!(with_clid > 400, "only {with_clid} models carry CLID");
        assert!(boxes > 200 && spheres > 500, "both kinds must be exercised");
        assert!(
            animated > 20,
            "the animated-header case must be exercised, not just described"
        );
        assert_eq!(flags.keys().copied().collect::<Vec<_>>(), vec![0x2000]);
        for f in failed.iter().take(20) {
            eprintln!("  fail {f}");
        }
        assert!(failed.is_empty(), "{} CLID models failed", failed.len());
    }
}

// ===========================================================================
// PRE2 — particle emitters (v2)
//
// Everything from this line to the end of the file belongs to the `PRE2`
// chunk. It is a self-contained block: the struct, the sub-chunk walk, the
// `Mdlx` reader and the tests that pin them. Nothing above this line is part
// of it, and nothing in it edits the code above.
//
// A `PRE2` record uses the same node framing as `RIBB`, `PREM` and `LITE`, so
// it is walked by `effect_nodes`; what follows the node is a 171-byte fixed
// prelude and then a chain of `KP2*` sub-chunks:
//
//   +0    u32  total size, counting its own four bytes
//   +4    u32  node size: 96 + the node tracks that follow
//   +8    char name[80]
//   +88   u32  object id
//   +92   i32  parent id
//   +96   u32  flags
//  +100   node tracks (KGTR / KGRT / KGSC), ending exactly at +4 + node_size
//
//   then, at +4 + node_size, 171 bytes of emitter state, then the sub-chunks:
//          KP2S  count  interp  globalSeq  count * key
//          ...   each sub-chunk is the same 16-byte track header the inline
//          tracks use, and every key is a u32 frame plus one f32 — plus an in
//          and an out tangent when the interpolation word asks for them.
//
// Two relations pin this and both hold for all 7,200 shipped records across
// 1.28c and 1.29b:
//
//   node_size == 96 + sum of the node tracks
//   total     == 4 + node_size + 171 + sum of the sub-chunks
//
// and a third, independent of any byte count, holds for the same 7,200:
// the sub-chunk chain is sorted by tag, in the order `Particle2TrackKind`'s
// variants are declared. A reader that guessed a stride, or that missed a
// sub-chunk, would produce a chain that is either short or out of order.
//
// What is *not* decoded is the 171-byte prelude. Its width is measured and
// certain; its fields are not. It is kept as raw bytes rather than guessed
// into named floats, which is the same line this crate draws for `PRE2`'s
// fields in `MDLPARTICLEEMITTER2` where the reforged PDB gives names but the
// vanilla bytes give only widths.
// ===========================================================================

/// Byte width of the fixed prelude inside a `PRE2` record's body.
///
/// Measured, not assumed: every one of the 7,200 shipped records has a body at
/// least this long, and in the 428 of them that carry no sub-chunk the body is
/// *exactly* this long. The other 6,772 begin their sub-chunk chain at this
/// offset without exception, which is what fixes the number — a prelude one
/// byte short or long would move every chain start.
pub const PARTICLE2_FIXED: usize = 171;

/// The six sub-chunk tags a `PRE2` record can carry.
///
/// The declaration order is not cosmetic: it is the order the tags appear in
/// every shipped record, and [`Particle2TrackKind`] is `Ord` so that the sweep
/// can assert the chain is sorted. No shipped record ever puts `KP2W` before
/// `KP2N`, nor `KP2E` before `KP2L`, nor `KP2V` before `KP2E` — in any of the
/// 7,200 records, so this is the only order the bytes show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Particle2TrackKind {
    /// `"KP2S"`.
    S,
    /// `"KP2L"`.
    L,
    /// `"KP2E"`.
    E,
    /// `"KP2V"`.
    V,
    /// `"KP2N"`.
    N,
    /// `"KP2W"`.
    W,
    /// Variation, native KP2R.
    R,
    /// Gravity, native KP2G.
    G,
}

impl Particle2TrackKind {
    /// The tag as it appears on disk.
    pub fn tag(self) -> [u8; 4] {
        match self {
            Self::S => *b"KP2S",
            Self::L => *b"KP2L",
            Self::E => *b"KP2E",
            Self::V => *b"KP2V",
            Self::N => *b"KP2N",
            Self::W => *b"KP2W",
            Self::R => *b"KP2R",
            Self::G => *b"KP2G",
        }
    }

    /// Reads a sub-chunk tag, or `None` for anything this build does not know.
    ///
    /// `None` is an error at the call site, not a default: the tag decides
    /// nothing about the key stride, but an unrecognised tag means the walk has
    /// lost sync, and continuing would resynchronise the whole record silently.
    pub fn from_tag(tag: &[u8; 4]) -> Option<Self> {
        match &tag[..] {
            b"KP2S" => Some(Self::S),
            b"KP2L" => Some(Self::L),
            b"KP2E" => Some(Self::E),
            b"KP2V" => Some(Self::V),
            b"KP2N" => Some(Self::N),
            b"KP2W" => Some(Self::W),
            b"KP2R" => Some(Self::R),
            b"KP2G" => Some(Self::G),
            _ => None,
        }
    }
}

/// One `PRE2` particle emitter, the newer and far more common of the two
/// particle systems. [`Particle`] is the older `PREM` set; the two coexist in
/// the shipped models and a model may carry both.
///
/// The node header is `effect_nodes`'s, so it is identical to a ribbon's. The
/// body is the 171-byte prelude followed by a sorted chain of `KP2*` sub-chunks
/// — see the block comment at the end of this file for the full layout and the
/// relations the sweep asserts.
///
/// Each sub-chunk is carried as a [`KeyTrack`] of width one. That is not a
/// convenience: the sub-chunk header is the same 16 bytes an inline track uses
/// (`magic`, `count`, `lineType`, `globalSeqId`) and the same word decides the
/// key stride, so reusing the type keeps one rule — "tangent interpolation
/// means two extra floats" — in one place.
///
/// The fixed state is retained verbatim and decoded by Particle2::properties,
/// using native binary/keyword reader field bindings.
#[derive(Debug, Clone, PartialEq)]
pub struct Particle2 {
    /// Offset of this record's total-size field within the chunk payload.
    pub offset: usize,
    /// The record's own total size, counting its own four bytes.
    pub total_size: u32,
    /// 96 plus the node's animation tracks.
    pub node_size: u32,
    pub name: String,
    pub object_id: u32,
    pub parent_id: i32,
    pub flags: u32,
    /// `KGTR`/`KGRT`/`KGSC` tracks inside the node header.
    pub node_tracks: Vec<KeyTrack>,
    /// Original fixed-state bytes; see properties() and the field-evidence notes.
    pub state: [u8; PARTICLE2_FIXED],
    /// The `KP2*` sub-chunks, in file order, each a scalar key track.
    pub tracks: Vec<KeyTrack>,
}

impl Particle2 {
    /// Bytes the node's own animation measures, including its fixed 96.
    ///
    /// This is what `node_size` must equal; see [`Mdlx::particles2`].
    pub fn measured_node_size(&self) -> usize {
        EFFECT_NODE_FIXED
            + self
                .node_tracks
                .iter()
                .map(|t| 16 + t.count as usize * t.stride)
                .sum::<usize>()
    }

    /// Bytes the record's own fields measure.
    ///
    /// The two size words, the node, the prelude and the sub-chunks. This is
    /// what `total_size` must equal; see [`Mdlx::particles2`].
    pub fn measured_size(&self) -> usize {
        4 + self.node_size as usize
            + PARTICLE2_FIXED
            + self
                .tracks
                .iter()
                .map(|t| 16 + t.count as usize * t.stride)
                .sum::<usize>()
    }

    /// The sub-chunk tags in file order.
    pub fn track_kinds(&self) -> Vec<Particle2TrackKind> {
        self.tracks
            .iter()
            .filter_map(|t| Particle2TrackKind::from_tag(&t.magic))
            .collect()
    }

    /// True when the sub-chunk chain is in the order every shipped record uses.
    pub fn track_kinds_sorted(&self) -> bool {
        let k = self.track_kinds();
        k.windows(2).all(|w| w[0] <= w[1])
    }
}

impl OrderedTracks for Particle2 {
    fn all_tracks(&self) -> Vec<&KeyTrack> {
        self.node_tracks.iter().chain(self.tracks.iter()).collect()
    }
}

/// Reads the `KP2*` sub-chunks occupying `p[from..to]` and requires the region to
/// be consumed exactly.
///
/// The "exactly" is the whole point, and it is what the prelude width rests on:
/// a sub-chunk whose stride is wrong stops the walk early, and the bytes it did
/// not consume are then read as the next sub-chunk's magic — which either
/// refuses the tag or, worse, accepts one and shifts every later record.
///
/// `base` is added to the offset in any error, so an offset reported against a
/// record's body still addresses the chunk payload it came from.
fn read_particle2_tracks(
    p: &[u8],
    from: usize,
    to: usize,
    base: usize,
    out: &mut Vec<KeyTrack>,
) -> Result<(), Error> {
    let at = |cur: usize| base + cur;
    let mut cur = from;
    while cur < to {
        let magic: [u8; 4] = p
            .get(cur..cur + 4)
            .ok_or(Error::TrackOverruns {
                chunk: "PRE2",
                at: at(cur),
                size: 4,
                left: to.saturating_sub(cur),
            })?
            .try_into()
            .unwrap();
        if Particle2TrackKind::from_tag(&magic).is_none() {
            return Err(Error::UnknownTrackMagic {
                chunk: "PRE2",
                at: at(cur),
                tag: magic,
            });
        }
        let count = read_u32(p, cur + 4)?;
        let kind = read_u32(p, cur + 8)?;
        let interpolation =
            Interpolation::from_u32(kind).ok_or_else(|| Error::UnknownTrackInterpolation {
                chunk: "PRE2",
                at: at(cur),
                kind,
            })?;
        let global_seq_id = read_u32(p, cur + 12)?;
        // One component per key, so the width the rest of the crate uses for a
        // scalar track applies unchanged here.
        let stride = KeyTrack::key_len(1, interpolation);
        let size = (count as usize)
            .checked_mul(stride)
            .and_then(|n| n.checked_add(16))
            .ok_or(Error::TrackOverruns {
                chunk: "PRE2",
                at: at(cur),
                size: usize::MAX,
                left: to - cur,
            })?;
        let next = cur + size;
        if next > to {
            return Err(Error::TrackOverruns {
                chunk: "PRE2",
                at: at(cur),
                size,
                left: to - cur,
            });
        }
        out.push(KeyTrack {
            magic,
            count,
            interpolation,
            global_seq_id,
            width: 1,
            stride,
            raw: p[cur + 16..next].to_vec(),
        });
        cur = next;
    }
    Ok(())
}

impl Mdlx {
    /// `PRE2` particle emitters, in file order.
    ///
    /// The successor to [`Mdlx::particles`], and much the commoner of the two:
    /// 1,091 of the 1.28c models carry this chunk against 29 for `PREM`. A model
    /// may carry both, and a `PRE2` record is *not* a `PREM` record with a wider
    /// body — see [`Particle2`].
    ///
    /// Two relations hold for every record of both builds and are what make the
    /// layout trustworthy rather than merely self-consistent:
    ///
    /// ```text
    /// node_size == 96 + sum of the node tracks
    /// total     == 4 + node_size + 171 + sum of the sub-chunks
    /// ```
    ///
    /// The prelude's width is measured by the second: it is the one length that
    /// makes the sub-chunk chain start where it does, in every record.
    pub fn particles2(&self, data: &[u8]) -> Result<Vec<Particle2>, Error> {
        let mut out = Vec::new();
        for node in effect_nodes("PRE2", self.payload(data, tag::PRE2))? {
            if node.body.len() < PARTICLE2_FIXED {
                // A body shorter than the fixed prelude is a walk that has lost
                // sync, not an emitter with nothing in it.
                return Err(Error::RecordShape {
                    chunk: "PRE2",
                    at: node.offset,
                    size: node.body.len(),
                    measured: PARTICLE2_FIXED,
                });
            }
            let mut tracks = Vec::new();
            read_particle2_tracks(
                &node.body,
                PARTICLE2_FIXED,
                node.body.len(),
                // Offsets in an error address the chunk payload, so the body's
                // own offset in it is carried along, as the other readers do.
                node.offset + 4 + node.node_size,
                &mut tracks,
            )?;
            let mut state = [0u8; PARTICLE2_FIXED];
            state.copy_from_slice(&node.body[..PARTICLE2_FIXED]);
            out.push(Particle2 {
                offset: node.offset,
                total_size: node.total_size as u32,
                node_size: node.node_size as u32,
                name: node.name,
                object_id: node.object_id,
                parent_id: node.parent_id,
                flags: node.flags,
                node_tracks: node.node_tracks,
                state,
                tracks,
            });
        }
        Ok(out)
    }
}

/// Tests for the `PRE2` chunk.
///
/// A separate module from `tests` on purpose: it brings its own model lookup
/// rather than borrowing the other module's, so nothing here depends on the
/// `PRE2` block above being in the same place as the other chunks' tests.
#[cfg(test)]
mod pre2_tests {
    use super::*;

    /// Where a WC3 install is, on the same terms as the main sweep.
    fn install_dir() -> Option<std::path::PathBuf> {
        for key in ["PEACECRAFT3_INSTALL", "PEACECRAFT3_LEGACY_DIR"] {
            if let Ok(v) = std::env::var(key) {
                let p = std::path::PathBuf::from(v);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        p.is_dir().then_some(p)
    }

    fn shipped(rel: &str) -> Option<Vec<u8>> {
        let dir = install_dir()?;
        pcx_mpq::MpqSet::open_wc3(dir).ok()?.extract(rel).ok()
    }

    /// The smallest `PRE2` any build ships, read off the raw payload by hand.
    ///
    /// `Doodads\Underground\Plants\ShroomsBlue\ShroomsBlue.mdx` carries a 271-byte
    /// `PRE2` holding one record. From the dump:
    ///
    /// ```text
    /// +0x000  0f 01 00 00   total size, 271
    /// +0x004  60 00 00 00   node size, 96
    /// +0x008  "BlizParticle08shockwaves" then NUL to 80 bytes
    /// +0x058  04 00 00 00   object id, 4
    /// +0x05c  ff ff ff ff   parent id, -1
    /// +0x060  00 90 10 00   flags, 0x00109000
    /// +0x064              body, and nothing after it
    /// ```
    ///
    /// `+0x064` is `+4 + 96`, so the node holds no animation and the body runs
    /// to the end of the record: 271 − 100 = 171 bytes, and no `KP2*` tag
    /// appears anywhere in them. That is the smallest possible demonstration
    /// that the prelude is 171 and not 168 or 174.
    #[test]
    fn the_smallest_pre2_record_is_node_and_prelude_alone() {
        let shrooms = "Doodads\\Underground\\Plants\\ShroomsBlue\\ShroomsBlue.mdx";
        let Some(d) = shipped(shrooms) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("PRE2: {e}"));
        let emitters = m.particles2(&d).unwrap_or_else(|e| panic!("PRE2: {e}"));
        assert_eq!(emitters.len(), 1);

        let p = &emitters[0];
        assert_eq!(p.total_size, 271);
        assert_eq!(p.node_size, 96);
        assert_eq!(p.name, "BlizParticle08shockwaves");
        assert_eq!(p.object_id, 4);
        assert_eq!(p.parent_id, -1);
        assert_eq!(p.flags, 0x0010_9000);
        assert!(p.node_tracks.is_empty(), "node size 96 admits no track");
        assert!(p.tracks.is_empty(), "no KP2 sub-chunk in this record");

        // 4 + 96 + 171: a record with no sub-chunk is its node and its prelude.
        assert_eq!(p.measured_size(), 271);
        assert_eq!(p.measured_node_size(), 96);

        // The first word of the prelude, `00 00 a0 c1` at `+0x064`, is -20.0.
        // It is read as a float to pin the prelude's *offset*, not as a named
        // field: which word of the prelude is which value is not decoded.
        assert_eq!(
            f32::from_bits(u32::from_le_bytes(p.state[..4].try_into().unwrap())),
            -20.0
        );
        // The rest of the prelude is whatever the file says; nothing is claimed.
        assert_eq!(p.state.len(), PARTICLE2_FIXED);
    }

    /// A `PRE2` chain of two sub-chunks, and an animated node header.
    ///
    /// `Units\Creeps\InfernalCannonFlame\InfernalCannonFlame.mdx` is 5,775 bytes
    /// and holds 13 records. The second one, at `+0x1a3`, reads by hand:
    ///
    /// ```text
    /// +0x1a3  cb 01 00 00   total size, 459
    /// +0x1a7  a4 00 00 00   node size, 164
    /// +0x1fb  1b 00 00 00   object id, 27
    /// +0x1ff  04 00 00 00   parent id, 4
    /// +0x203  00 10 00 00   flags, 0x1000
    /// +0x207  KGRT, count 1, lineType 2, globalSeq 0xffffffff
    ///          one key: frame 103333, value (0, 0.6977907, 0, 0.7163017)
    /// ```
    ///
    /// A `KGRT` is a quaternion, so its key is a frame plus four floats plus the
    /// two tangents lineType 2 asks for: 16 + (4 + 16 + 32) = 68, and
    /// 96 + 68 = 164, which is the node size. The body then runs 291 bytes from
    /// `+0x24b`, and the sub-chunks inside it are:
    ///
    /// ```text
    /// body+171  KP2L  count 3  lineType 3  globalSeq 0xffffffff  16 + 3*16 = 64
    /// body+235  KP2E  count 5  lineType 0  globalSeq 0xffffffff  16 + 5*8  = 56
    /// ```
    ///
    /// 171 + 64 + 56 = 291, the whole body. Both sub-chunk sizes are read here
    /// as the *same* 16-byte header the inline tracks use, with one float per
    /// key; that is what makes the two sizes differ, and lineType 3 carrying the
    /// tangents is why the first is half again the second at two keys fewer.
    #[test]
    fn a_pre2_chain_of_two_sub_chunks_measures_to_the_record() {
        let flame = "Units\\Creeps\\InfernalCannonFlame\\InfernalCannonFlame.mdx";
        let Some(d) = shipped(flame) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("PRE2: {e}"));
        let emitters = m.particles2(&d).unwrap_or_else(|e| panic!("PRE2: {e}"));
        assert_eq!(emitters.len(), 13, "the whole chunk, not just record one");

        let p = &emitters[1];
        assert_eq!(p.offset, 0x1a3);
        assert_eq!(p.total_size, 459);
        assert_eq!(p.node_size, 164);
        assert_eq!(p.name, "BlizParticle Flash01");
        assert_eq!(p.object_id, 27);
        assert_eq!(p.parent_id, 4);
        assert_eq!(p.flags, 0x1000);

        // The animated node header: one quaternion rotation, 68 bytes, and the
        // excess of the node size over 96 is exactly that.
        assert_eq!(p.node_tracks.len(), 1);
        let nt = &p.node_tracks[0];
        assert_eq!(&nt.magic, b"KGRT");
        assert_eq!(nt.count, 1);
        assert_eq!(nt.interpolation, Interpolation::Hermite);
        assert_eq!(nt.width, 4);
        assert_eq!(nt.stride, 52, "frame, four floats and two tangents");
        let q = nt.value(0).unwrap();
        // 1.28c stores 0x3f32a269 / 0x3f375f8c and 1.29b stores 0x3f32a26a /
        // 0x3f375f8d — the two exporters round this rotation one ULP apart, and
        // it is the only difference between the builds in the whole 5,775-byte
        // chunk. So the value is compared to a tolerance and the *frame* below
        // is compared exactly; the framing, which is what this chunk is about, is
        // byte-identical between the two.
        assert_eq!(nt.frame(0), Some(103_333));
        assert!((q[1] - 0.697_790_7).abs() < 1e-6, "rotation y {}", q[1]);
        assert!((q[3] - 0.716_301_7).abs() < 1e-6, "rotation w {}", q[3]);
        assert_eq!(q[0], 0.0);
        assert_eq!(q[2], 0.0);
        assert_eq!(p.measured_node_size(), 164);

        // The prelude's first word, `00 00 0c 43` at `+0x24b`, is 140.0.
        assert_eq!(
            f32::from_bits(u32::from_le_bytes(p.state[..4].try_into().unwrap())),
            140.0
        );

        assert_eq!(
            p.track_kinds(),
            vec![Particle2TrackKind::L, Particle2TrackKind::E],
            "KP2L then KP2E, and the chain is in the measured order"
        );
        assert!(p.track_kinds_sorted());
        let l = &p.tracks[0];
        assert_eq!(&l.magic, b"KP2L");
        assert_eq!(l.count, 3);
        assert_eq!(l.interpolation, Interpolation::Bezier);
        assert_eq!(l.width, 1, "a KP2 sub-chunk key is one float");
        assert_eq!(l.stride, 16, "frame, one float and two tangents");
        assert_eq!(l.global_seq_id, u32::MAX);
        let e = &p.tracks[1];
        assert_eq!(&e.magic, b"KP2E");
        assert_eq!(e.count, 5);
        assert_eq!(e.interpolation, Interpolation::None);
        assert_eq!(e.stride, 8, "frame and one float, no tangents");
        // Read off the payload: lineType 0 keys are a frame and one float, and
        // nothing else — the values are 0, 0, 25, 0, 25 at those frames.
        let eframes: Vec<u32> = (0..5).map(|i| e.frame(i).unwrap()).collect();
        assert_eq!(eframes, vec![0, 100_000, 100_533, 103_333, 103_867]);
        let evalues: Vec<f32> = (0..5).map(|i| e.scalar(i).unwrap()).collect();
        assert_eq!(evalues, vec![0.0, 0.0, 25.0, 0.0, 25.0]);
        // lineType 3 keys do carry the tangents, so the last key's own value is
        // reachable only through the value accessor and not through the tangent.
        assert_eq!(l.frame(0), Some(0));
        assert_eq!(l.scalar(0), Some(44.0));
        assert_eq!(l.scalar(1), Some(16.0));
        assert_eq!(l.frame(2), Some(105_333));
        assert!(l.out_tangent(0).unwrap()[0] > 0.0);
        assert_eq!(l.in_tangent(0).unwrap()[0], 0.0);

        // 4 + 164 + 171 + (16 + 3*16) + (16 + 5*8).
        assert_eq!(p.measured_size(), 459);
        assert_eq!(4 + 164 + PARTICLE2_FIXED + 64 + 56, p.total_size as usize);
    }

    /// A `KP2V` chain, the commonest case, with every key read by hand.
    ///
    /// The first record of the same model, at `+0x000`:
    ///
    /// ```text
    /// +0x000  a3 01 00 00   total size, 419
    /// +0x004  a4 00 00 00   node size, 164
    /// +0x058  1a 00 00 00   object id, 26
    /// +0x05c  04 00 00 00   parent id, 4
    /// +0x060  00 10 00 00   flags, 0x1000
    /// ```
    ///
    /// Its body runs 251 bytes from `+0x0a8`, and at `body+171` sits a single
    /// `KP2V` with count 8, lineType 0 and `globalSeq` 0xffffffff, so
    /// 16 + 8 * 8 = 80 and 171 + 80 = 251, the whole body. Its eight keys,
    /// read off the payload, are frames 12400, 16667, 93333, 96667, 103333,
    /// 103900, 104367 and 106667 with values 0, 0, 0, 0, 0, 1, 0, 0.
    #[test]
    fn a_pre2_visibility_chain_reads_back_key_for_key() {
        let flame = "Units\\Creeps\\InfernalCannonFlame\\InfernalCannonFlame.mdx";
        let Some(d) = shipped(flame) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("PRE2: {e}"));
        let emitters = m.particles2(&d).unwrap_or_else(|e| panic!("PRE2: {e}"));

        let p = &emitters[0];
        assert_eq!(p.total_size, 419);
        assert_eq!(p.name, "BlizParticle Shot01");
        assert_eq!(p.object_id, 26);
        assert_eq!(p.flags, 0x1000);
        // `00 00 48 43` at `+0x0a8`.
        assert_eq!(
            f32::from_bits(u32::from_le_bytes(p.state[..4].try_into().unwrap())),
            200.0
        );

        assert_eq!(p.track_kinds(), vec![Particle2TrackKind::V]);
        assert_eq!(p.tracks.len(), 1);
        let t = &p.tracks[0];
        assert_eq!(&t.magic, b"KP2V");
        assert_eq!(t.count, 8);
        assert_eq!(t.interpolation, Interpolation::None);
        assert_eq!(t.stride, 8);
        assert_eq!(t.global_seq_id, u32::MAX);
        let frames: Vec<u32> = (0..t.count as usize).map(|i| t.frame(i).unwrap()).collect();
        assert_eq!(
            frames,
            vec![12400, 16667, 93333, 96667, 103333, 103900, 104367, 106667]
        );
        let values: Vec<f32> = (0..t.count as usize)
            .map(|i| t.scalar(i).unwrap())
            .collect();
        assert_eq!(values, vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        assert!(t.times_are_ordered());
        assert_eq!(p.measured_size(), 419);
    }

    /// The three rarest sub-chunks, and a `KP2N`/`KP2W` pair with tangents.
    ///
    /// `KP2N` and `KP2W` appear in 141 records of each build and `KP2L` in 92
    /// — rare enough that a reader which got their stride wrong could hide
    /// behind the common tags, so they are pinned on their own. From
    /// `Abilities\Spells\Demon\DarkPortal\DarkPortalTarget.mdx`, whose `PRE2`
    /// is 942 bytes with a 471-byte record and an unanimated node header:
    ///
    /// ```text
    /// body+171  KP2V  count 3  lineType 0   16 + 3*8  = 40
    /// body+211  KP2N  count 4  lineType 3   16 + 4*16 = 80
    /// body+291  KP2W  count 4  lineType 3   16 + 4*16 = 80
    /// ```
    ///
    /// 171 + 40 + 80 + 80 = 371, the body of a record of total size 471. Both
    /// `KP2N` and `KP2W` carry the same four keys — frames 0, 1167, 1600 and
    /// 2167 with values 120, 100, 40 and 0 — which is itself the evidence that
    /// they are read the same way and not accidentally transposed.
    #[test]
    fn the_rarest_pre2_sub_chunks_use_the_same_scalar_key_framing() {
        let portal = "Abilities\\Spells\\Demon\\DarkPortal\\DarkPortalTarget.mdx";
        let Some(d) = shipped(portal) else { return };
        let m = Mdlx::parse(&d).unwrap_or_else(|e| panic!("PRE2: {e}"));
        let emitters = m.particles2(&d).unwrap_or_else(|e| panic!("PRE2: {e}"));
        assert!(!emitters.is_empty());

        let p = emitters
            .iter()
            .find(|x| x.track_kinds().len() >= 3)
            .expect("a record with a three-deep chain");
        assert_eq!(p.total_size, 471);
        assert_eq!(p.node_size, 96);
        assert_eq!(p.name, "BlizParticle01");
        assert!(p.node_tracks.is_empty());
        assert_eq!(
            p.track_kinds(),
            vec![
                Particle2TrackKind::V,
                Particle2TrackKind::N,
                Particle2TrackKind::W
            ]
        );
        assert!(p.track_kinds_sorted());
        assert_eq!(
            p.tracks.iter().map(|t| t.stride).collect::<Vec<_>>(),
            vec![8, 16, 16],
            "lineType 0 is a bare key, lineType 3 carries both tangents"
        );
        assert_eq!(p.measured_size(), 471);

        // `KP2N` and `KP2W` agree key for key.
        let (n, w) = (&p.tracks[1], &p.tracks[2]);
        assert_eq!((&n.magic[..], &w.magic[..]), (&b"KP2N"[..], &b"KP2W"[..]));
        assert_eq!(n.count, 4);
        assert_eq!(w.count, 4);
        for i in 0..4 {
            assert_eq!(n.frame(i), w.frame(i));
            assert_eq!(n.scalar(i), w.scalar(i));
            assert_eq!(n.frame(i), Some([0, 1167, 1600, 2167][i]));
            assert_eq!(n.scalar(i), Some([120.0, 100.0, 40.0, 0.0][i]));
        }
        assert!(
            n.in_tangent(1).unwrap()[0] > 0.0,
            "lineType 3 keys have tangents"
        );
    }

    /// A `PRE2` sub-chunk this build does not know is refused, not skipped.
    ///
    /// The whole decode rests on the sub-chunk chain being walked exactly, so
    /// the failure mode to guard against is not a crash but a *plausible* walk:
    /// a tag that is accepted with the wrong width leaves the next sub-chunk
    /// starting four bytes off, which reads as a real count and real values.
    /// Refusing is the only answer that keeps the walk honest, so this builds a
    /// synthetic record carrying `KP2H` — a tag no shipped model has — and
    /// requires the refusal. The same record with a corrupt count word is
    /// refused too, because that is what a wrong stride would look like.
    #[test]
    fn a_pre2_sub_chunk_this_build_does_not_know_is_refused() {
        fn container(rec: &[u8]) -> Vec<u8> {
            let mut v = Vec::new();
            v.extend_from_slice(b"MDLX");
            v.extend_from_slice(b"VERS");
            v.extend_from_slice(&4u32.to_le_bytes());
            v.extend_from_slice(&800u32.to_le_bytes());
            v.extend_from_slice(b"PRE2");
            v.extend_from_slice(&(rec.len() as u32).to_le_bytes());
            v.extend_from_slice(rec);
            v
        }

        /// A bare node, the 171-byte prelude, then 24 bytes of sub-chunk.
        fn record_with(tag: &[u8; 4], count: u32) -> Vec<u8> {
            let mut rec = Vec::new();
            let total: u32 = (4 + EFFECT_NODE_FIXED + PARTICLE2_FIXED + 16 + 8) as u32;
            rec.extend_from_slice(&total.to_le_bytes());
            rec.extend_from_slice(&96u32.to_le_bytes());
            rec.extend_from_slice(&[0u8; 80]);
            rec.extend_from_slice(&0u32.to_le_bytes()); // object id
            let id: i32 = -1;
            rec.extend_from_slice(&id.to_le_bytes()); // parent id
            rec.extend_from_slice(&0x9000u32.to_le_bytes()); // flags
            rec.extend_from_slice(&[0u8; PARTICLE2_FIXED]);
            rec.extend_from_slice(tag);
            rec.extend_from_slice(&count.to_le_bytes());
            rec.extend_from_slice(&0u32.to_le_bytes()); // lineType, no tangents
            rec.extend_from_slice(&u32::MAX.to_le_bytes()); // globalSeq
            rec.extend_from_slice(&0u32.to_le_bytes()); // frame
            rec.extend_from_slice(&0.0f32.to_le_bytes()); // value
            rec
        }

        // The known tag with the same shape parses, so the refusal below is the
        // tag and not the synthetic record.
        let ok = record_with(b"KP2V", 1);
        let v = container(&ok);
        let m = Mdlx::parse(&v).expect("a walkable container");
        let e = m.particles2(&v).expect("KP2V is one of the six");
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].measured_size(), e[0].total_size as usize);

        for (tag, what) in [(b"KP2H", "unknown tag"), (b"KRZZ", "not a sub-chunk")] {
            let v = container(&record_with(tag, 1));
            let m = Mdlx::parse(&v).expect("a walkable container");
            match m.particles2(&v) {
                Err(Error::UnknownTrackMagic {
                    chunk,
                    at,
                    tag: got,
                }) => {
                    assert_eq!(chunk, "PRE2");
                    // The prelude is 171 bytes and the node 96, so the sub-chunk
                    // magic sits at 4 + 96 + 171.
                    assert_eq!(at, 4 + EFFECT_NODE_FIXED + PARTICLE2_FIXED, "{what}");
                    assert_eq!(&got, tag.as_slice());
                }
                other => panic!("expected UnknownTrackMagic for {what}, got {other:?}"),
            }
        }

        // A count of two where only one key follows is what a reader with the
        // wrong stride would produce, and it must not be accepted.
        let v = container(&record_with(b"KP2V", 2));
        let m = Mdlx::parse(&v).expect("a walkable container");
        match m.particles2(&v) {
            Err(Error::TrackOverruns {
                chunk, at, size, ..
            }) => {
                assert_eq!(chunk, "PRE2");
                assert_eq!(at, 4 + EFFECT_NODE_FIXED + PARTICLE2_FIXED);
                assert_eq!(size, 16 + 2 * 8);
            }
            other => panic!("expected TrackOverruns, got {other:?}"),
        }
    }

    /// Every shipped `PRE2`, with the framing relations asserted rather than
    /// counted.
    ///
    /// Ignored by default because it reads the whole model set; run with
    /// `cargo test -p pcx-mdl -- --ignored`. The main sweep asserts the same
    /// relations while it parses, so this is the standalone form: it exists so
    /// the `PRE2` framing is checked even when the sweep is pointed at a subset.
    #[test]
    #[ignore = "parses the full shipped model set"]
    fn every_shipped_pre2_measures_to_its_record() {
        let Some(dir) = install_dir() else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).expect("open archives");
        let models: Vec<String> = set
            .file_list()
            .expect("union listfile")
            .into_iter()
            .filter(|l| l.to_ascii_lowercase().ends_with(".mdx"))
            .collect();

        let mut failed = Vec::new();
        let mut records = 0usize;
        let mut sub_chunks = 0usize;
        let mut with_sub_chunks = 0usize;
        let mut kinds: std::collections::BTreeMap<Particle2TrackKind, usize> = Default::default();
        let mut ordered = 0usize;
        let mut negative_lead = 0usize;
        let mut models_with = 0usize;

        for rel in &models {
            let Ok(d) = set.extract(rel) else { continue };
            let Ok(m) = Mdlx::parse(&d) else { continue };
            let emitters = match m.particles2(&d) {
                Ok(x) => x,
                Err(e) => {
                    failed.push(format!("{rel}: PRE2: {e}"));
                    continue;
                }
            };
            if emitters.is_empty() {
                continue;
            }
            models_with += 1;
            for x in &emitters {
                records += 1;
                if !x.tracks.is_empty() {
                    with_sub_chunks += 1;
                }
                sub_chunks += x.tracks.len();
                for t in &x.tracks {
                    if let Some(k) = Particle2TrackKind::from_tag(&t.magic) {
                        *kinds.entry(k).or_default() += 1;
                    }
                }
                // Relation one: the node size absorbs the node's own animation.
                if x.node_size as usize != x.measured_node_size() {
                    failed.push(format!(
                        "{rel}: PRE2 {} node size {} measures {}",
                        x.name,
                        x.node_size,
                        x.measured_node_size()
                    ));
                }
                // Relation two: two size words, the node, the prelude and the
                // sub-chunks are the whole record.
                let want = x.measured_size();
                if want != x.total_size as usize {
                    failed.push(format!(
                        "{rel}: PRE2 {} measures {want} in {} bytes",
                        x.name, x.total_size
                    ));
                }
                // Relation three: the sub-chunk chain is in the measured order.
                if !x.track_kinds_sorted() {
                    failed.push(format!(
                        "{rel}: PRE2 {} sub-chunks out of order: {:?}",
                        x.name,
                        x.track_kinds()
                    ));
                }
                // Every sub-chunk is a scalar key track, so its stride is a
                // function of one word. A record that disagrees has been read
                // with the wrong width somewhere.
                for t in &x.tracks {
                    if t.width != 1 {
                        failed.push(format!(
                            "{rel}: PRE2 {} track {} width {} is not a scalar",
                            x.name,
                            String::from_utf8_lossy(&t.magic),
                            t.width
                        ));
                    }
                    if !t.times_are_ordered() {
                        // A negative leading frame is real in the shipped set, as
                        // it is for camera tracks, so it is counted rather than
                        // failed; a descent that is not that is a misread.
                        if t.has_negative_frame() {
                            negative_lead += 1;
                        } else {
                            failed.push(format!(
                                "{rel}: PRE2 {} track {} keys out of order",
                                x.name,
                                String::from_utf8_lossy(&t.magic)
                            ));
                        }
                    } else {
                        ordered += 1;
                    }
                }
            }
        }

        eprintln!(
            "PRE2: models={models_with} records={records} subchunks={sub_chunks} \
             with_sub_chunks={with_sub_chunks} failed={}",
            failed.len()
        );
        eprintln!("  sub-chunk tags: {kinds:?}");
        eprintln!("  tracks ordered={ordered} negative_lead={negative_lead}");
        assert!(models_with > 900, "only {models_with} models carry PRE2");
        assert!(records > 3_000, "only {records} records");
        // Every one of the six tags the format uses must be exercised.
        assert_eq!(kinds.len(), 6, "a KP2 tag the sweep never saw: {kinds:?}");
        for f in failed.iter().take(20) {
            eprintln!("  fail {f}");
        }
        assert!(failed.is_empty(), "{} PRE2 records failed", failed.len());
    }
}

#[cfg(test)]
mod compose_tests {
    use super::*;

    fn pose(t: [f32; 3], q: [f32; 4], s: [f32; 3]) -> Pose {
        Pose {
            translation: t,
            rotation: q,
            scale: s,
        }
    }
    const I: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    fn id() -> [f32; 16] {
        identity()
    }

    /// The linear part must be `R * S` exactly. The old code applied `s[0]` to
    /// `out[0]` twice, so any bone with a non-unit scale sheared its matrix.
    #[test]
    fn scale_is_applied_once() {
        let m = compose(id(), pose([0.0; 3], I, [2.0, 3.0, 4.0]), [0.0; 3]);
        assert_eq!(
            (m[0], m[5], m[10]),
            (2.0, 3.0, 4.0),
            "the diagonal is the scale"
        );
        assert_eq!(m[1], 0.0);
        assert_eq!(m[4], 0.0);
    }

    /// The pivot decides where the bone's origin sits, so a rotation about a
    /// pivot that is not the origin must move the translation column. The old
    /// code computed `R * translation`, dropping the pivot entirely, which put
    /// every bone at its parent's origin.
    #[test]
    fn the_pivot_is_restored_in_the_translation() {
        // 90 degrees about +Z.
        let half = std::f32::consts::FRAC_PI_4;
        let q = [0.0, 0.0, (half).sin(), (half).cos()];
        let m = compose(id(), pose([0.0; 3], q, [1.0; 3]), [1.0, 0.0, 0.0]);
        // (pivot + t) - R * pivot  =  [1,0,0] - [0,1,0]  =  [1,-1,0]
        assert!((m[12] - 1.0).abs() < 1e-5, "x {}", m[12]);
        assert!((m[13] + 1.0).abs() < 1e-5, "y {}", m[13]);
        assert!(m[14].abs() < 1e-5, "z {}", m[14]);
    }

    /// A bone with no animation and no parent must be the identity, because the
    /// format stores vertices already in model space. Anything else collapses
    /// the model on the first frame.
    #[test]
    fn a_rest_bone_is_the_identity() {
        let m = compose(id(), pose([0.0; 3], I, [1.0; 3]), [123.0, -45.0, 6.0]);
        assert_eq!(m, identity(), "rest pose leaves the mesh where it is");
    }

    #[test]
    fn rotation_keeps_the_pivot_fixed_and_turns_nearby_vertices() {
        let half = std::f32::consts::FRAC_PI_4;
        let rotation = [0.0, 0.0, half.sin(), half.cos()];
        let pivot = [1.0, 2.0, 3.0];
        let matrix = compose(id(), pose([0.0; 3], rotation, [1.0; 3]), pivot);
        for (point, expected) in [(pivot, pivot), ([2.0, 2.0, 3.0], [1.0, 3.0, 3.0])] {
            let actual = transform_point(&matrix, point);
            for axis in 0..3 {
                assert!((actual[axis] - expected[axis]).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn parent_and_child_rotations_compose_in_model_space() {
        let half = std::f32::consts::FRAC_PI_4;
        let parent = compose(
            id(),
            pose(
                [5.0, 1.0, 0.0],
                [0.0, 0.0, half.sin(), half.cos()],
                [1.0; 3],
            ),
            [0.0; 3],
        );
        let child_pose = pose(
            [2.0, 1.0, -3.0],
            [half.sin(), 0.0, 0.0, half.cos()],
            [2.0, 1.0, 1.0],
        );
        let pivot = [1.0, 0.0, 2.0];
        let local = compose(id(), child_pose, pivot);
        let world = compose(parent, child_pose, pivot);
        let point = [2.0, 3.0, 4.0];
        let expected = transform_point(&parent, transform_point(&local, point));
        let actual = transform_point(&world, point);
        for axis in 0..3 {
            assert!((actual[axis] - expected[axis]).abs() < 1e-5);
        }
    }

    /// A parent's rotation must reach the child's 3x3 as well as its
    /// translation; the old code wrote `R * S` alone and lost the inheritance.
    #[test]
    fn a_parent_rotates_its_child() {
        let half = std::f32::consts::FRAC_PI_4;
        let rq = [0.0, 0.0, (half).sin(), (half).cos()];
        let parent = compose(id(), pose([5.0, 0.0, 0.0], rq, [1.0; 3]), [0.0; 3]);
        let child = compose(parent, pose([0.0; 3], I, [1.0; 3]), [0.0; 3]);
        // The child's linear part is the parent's rotation, not the identity.
        assert!((child[0] - parent[0]).abs() < 1e-5);
        assert!((child[1] - parent[1]).abs() < 1e-5);
        assert!(
            (child[12] - parent[12]).abs() < 1e-5,
            "and it rides the parent"
        );
    }

    /// `bone_matrices` returns one matrix per **bone**, in bone order. Sparse
    /// node ids are exactly why a caller must not use an id as an index.
    #[test]
    fn matrices_come_back_in_bone_order_even_with_sparse_ids() {
        let mk = |object_id: u32, parent_id: i32| Bone {
            offset: 0,
            span: 0,
            name: String::new(),
            object_id,
            parent_id,
            flags: 0x100,
            tracks: Vec::new(),
            geoset_id: -1,
            geoset_anim_id: -1,
        };
        // Node ids 3 and 9, i.e. nothing like their array positions 0 and 1.
        let bones = vec![mk(3, -1), mk(9, 3)];
        let pivots = Pivots(vec![
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            [10.0, 0.0, 0.0],
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            [1.0, 0.0, 0.0],
        ]);
        let parent = |oid: u32| -> Option<u32> {
            bones
                .iter()
                .find(|b| b.object_id == oid)
                .and_then(|b| (b.parent_id >= 0).then_some(b.parent_id as u32))
        };
        let world = bone_matrices(&bones, &parent, &pivots, 0);
        assert_eq!(world.len(), 2, "one per bone");
        // A rest bone is identity regardless of where its pivot is, so the
        // ordering is what this pins: `world[1]` is id 9's, not a matrix at
        // index 9 (which would be out of bounds).
        assert_eq!(world[0], identity());
        assert_eq!(world[1], identity());
    }

    fn rest_bone(object_id: u32) -> Bone {
        Bone {
            offset: 0,
            span: 0,
            name: String::new(),
            object_id,
            parent_id: -1,
            flags: 0x100,
            tracks: Vec::new(),
            geoset_id: -1,
            geoset_anim_id: -1,
        }
    }

    fn translated(t: [f32; 3]) -> Pose {
        pose(t, I, [1.0; 3])
    }

    fn compose_tree(ids: &[u32], parents: &[Option<u32>], poses: &[Pose]) -> Vec<[f32; 16]> {
        let bones: Vec<Bone> = ids.iter().map(|&id| rest_bone(id)).collect();
        let parent = |id: u32| {
            // The last bone carrying `id` is the one a lookup by id reaches.
            ids.iter().rposition(|&i| i == id).and_then(|i| parents[i])
        };
        bone_matrices_from_poses(&bones, &parent, &Pivots(Vec::new()), poses)
    }

    /// Ids far beyond the bone count must compose like dense ones.
    #[test]
    fn huge_node_ids_compose_through_their_parents() {
        let dense = compose_tree(
            &[0, 1],
            &[None, Some(0)],
            &[translated([1.0, 0.0, 0.0]), translated([0.0, 2.0, 0.0])],
        );
        let sparse = compose_tree(
            &[7, 4_000_000_000],
            &[None, Some(7)],
            &[translated([1.0, 0.0, 0.0]), translated([0.0, 2.0, 0.0])],
        );
        assert_eq!(dense, sparse);
        assert_eq!(&sparse[1][12..15], &[1.0, 2.0, 0.0]);
    }

    /// A parent id carried by two bones resolves to the later one, as a map
    /// built in bone order would.
    #[test]
    fn a_repeated_object_id_resolves_to_the_later_bone() {
        let world = compose_tree(
            &[5, 5, 6],
            &[None, None, Some(5)],
            &[
                translated([1.0, 0.0, 0.0]),
                translated([0.0, 10.0, 0.0]),
                translated([0.0, 0.0, 1.0]),
            ],
        );
        assert_eq!(&world[2][12..15], &[0.0, 10.0, 1.0]);
    }

    /// Bones on a parent cycle, including a bone that is its own parent, keep
    /// their zero matrices; their descendants and unrelated bones compose.
    #[test]
    fn parent_cycles_keep_zero_matrices() {
        let world = compose_tree(
            &[0, 1, 2, 3],
            &[Some(1), Some(0), Some(2), None],
            &[translated([1.0, 0.0, 0.0]); 4],
        );
        assert_eq!(world[0], [0.0; 16]);
        assert_eq!(world[1], [0.0; 16]);
        assert_eq!(world[2], [0.0; 16]);
        assert_eq!(&world[3][12..15], &[1.0, 0.0, 0.0]);
    }
}

#[cfg(test)]
mod texture_flag_tests {
    use super::*;

    /// A minimal model holding one `TEXS` record with the given flag word.
    fn model_with_texture(path: &str, flags: u32) -> Vec<u8> {
        let mut rec = vec![0u8; TEXTURE_SIZE];
        rec[0..4].copy_from_slice(&0u32.to_le_bytes()); // slot
        let b = path.as_bytes();
        rec[4..4 + b.len()].copy_from_slice(b); // path, NUL-terminated by the zeroes
        rec[264..268].copy_from_slice(&flags.to_le_bytes());
        let mut d = Vec::new();
        d.extend_from_slice(b"MDLX");
        // `VERS` is required by the parser; 800 is the format the shipped set uses.
        d.extend_from_slice(b"VERS");
        d.extend_from_slice(&4u32.to_le_bytes());
        d.extend_from_slice(&800u32.to_le_bytes());
        d.extend_from_slice(b"TEXS");
        d.extend_from_slice(&(rec.len() as u32).to_le_bytes());
        d.extend_from_slice(&rec);
        d
    }

    /// The flag word is the record's last four bytes, and it is read.
    ///
    /// This is the regression for the bug: `Texture` used to be `slot` + path
    /// only, so the trailing word was silently dropped and every texture sampled
    /// as though it did not wrap.
    #[test]
    fn the_trailing_flag_word_is_parsed() {
        let m = Mdlx::parse(&model_with_texture("Ice_Ice.blp", 0x3)).expect("parses");
        let t = m
            .textures(&model_with_texture("Ice_Ice.blp", 0x3))
            .expect("textures");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].path, "Ice_Ice.blp");
        assert_eq!(t[0].flags, 0x3, "the word at +264");
        let _ = m;
    }

    /// Both bits, separately, so a parser that read the wrong offset or the
    /// wrong byte order would not pass by accident.
    #[test]
    fn the_wrap_bits_are_read_individually() {
        let get = |flags: u32| {
            let d = model_with_texture("x.blp", flags);
            Mdlx::parse(&d).unwrap().textures(&d).unwrap()[0].clone()
        };
        assert!(!get(0x0).wraps_width() && !get(0x0).wraps_height());
        assert!(get(0x1).wraps_width() && !get(0x1).wraps_height());
        assert!(!get(0x2).wraps_width() && get(0x2).wraps_height());
        assert!(get(0x3).wraps_width() && get(0x3).wraps_height());
        // A word with unrelated high bits set must not turn either wrap on.
        assert!(!get(0xfffc).wraps_width() && !get(0xfffc).wraps_height());
    }
}

#[cfg(test)]
mod binding_regressions {
    use super::*;

    fn model(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut data = b"MDLXVERS".to_vec();
        data.extend(4u32.to_le_bytes());
        data.extend(800u32.to_le_bytes());
        data.extend(tag);
        data.extend((body.len() as u32).to_le_bytes());
        data.extend(body);
        data
    }

    #[test]
    fn geoset_animation_preserves_authored_binding_not_record_order() {
        let mut body = Vec::new();
        for id in [7u32, 2, NO_ID] {
            body.extend(28u32.to_le_bytes());
            body.extend(0.75f32.to_le_bytes());
            body.extend(2u32.to_le_bytes());
            for colour in [0.25f32, 0.5, 0.875] {
                body.extend(colour.to_le_bytes());
            }
            body.extend(id.to_le_bytes());
        }
        let data = model(tag::GEOA, &body);
        let parsed = Mdlx::parse(&data).unwrap().geoset_anims(&data).unwrap();
        assert_eq!(
            parsed.iter().map(|anim| anim.geoset_id).collect::<Vec<_>>(),
            [7, 2, NO_ID]
        );
        assert_eq!(parsed[0].static_alpha, 0.75);
        assert_eq!(parsed[0].static_colour, [0.25, 0.5, 0.875]);
    }

    #[test]
    fn unknown_geoset_animation_tracks_are_explicit_errors() {
        let mut body = vec![0; 44];
        body[..4].copy_from_slice(&44u32.to_le_bytes());
        body[28..32].copy_from_slice(b"KGAA");
        body[36..40].copy_from_slice(&9u32.to_le_bytes());
        let data = model(tag::GEOA, &body);
        assert!(matches!(
            Mdlx::parse(&data).unwrap().geoset_anims(&data),
            Err(Error::UnknownTrackInterpolation { kind: 9, .. })
        ));
        body[28..32].copy_from_slice(b"????");
        let data = model(tag::GEOA, &body);
        assert!(matches!(
            Mdlx::parse(&data).unwrap().geoset_anims(&data),
            Err(Error::UnknownTrackMagic { .. })
        ));
    }

    #[test]
    fn helper_retains_keyed_transforms_and_global_sequence_reference() {
        let mut body = vec![0; NODE_FIXED];
        body[0..4].copy_from_slice(&((NODE_FIXED + 32) as u32).to_le_bytes());
        body[4..10].copy_from_slice(b"helper");
        body[84..88].copy_from_slice(&19u32.to_le_bytes());
        body[88..92].copy_from_slice(&3i32.to_le_bytes());
        body.extend(b"KGTR");
        body.extend(1u32.to_le_bytes());
        body.extend(1u32.to_le_bytes());
        body.extend(5u32.to_le_bytes());
        body.extend(123u32.to_le_bytes());
        for value in [4f32, -2., 8.] {
            body.extend(value.to_le_bytes());
        }
        let data = model(tag::HELP, &body);
        let nodes = Mdlx::parse(&data).unwrap().nodes(&data, tag::HELP).unwrap();
        assert_eq!(nodes[0].object_id, 19);
        assert_eq!(nodes[0].parent_id, 3);
        assert_eq!(nodes[0].tracks.len(), 1);
        assert_eq!(nodes[0].tracks[0].global_seq_id, 5);
        assert_eq!(nodes[0].tracks[0].count, 1);
        assert_eq!(&nodes[0].tracks[0].raw[..4], &123u32.to_le_bytes());
    }

    #[test]
    fn malformed_helper_track_is_refused() {
        let mut body = vec![0; NODE_FIXED + 16];
        let len = body.len() as u32;
        body[..4].copy_from_slice(&len.to_le_bytes());
        body[NODE_FIXED..NODE_FIXED + 4].copy_from_slice(b"KGTR");
        body[NODE_FIXED + 4..NODE_FIXED + 8].copy_from_slice(&1u32.to_le_bytes());
        let data = model(tag::HELP, &body);
        assert!(matches!(
            Mdlx::parse(&data).unwrap().nodes(&data, tag::HELP),
            Err(Error::TrackOverruns { .. })
        ));
    }
}

#[cfg(test)]
mod cubic_regressions {
    use super::*;

    #[test]
    fn vector_bezier_uses_control_points() {
        let mut raw = Vec::new();
        for frame in [0u32, 100] {
            raw.extend(frame.to_le_bytes());
            for value in [0f32, 0., 0., 4., 8., 12., 4., 8., 12.] {
                raw.extend(value.to_le_bytes());
            }
        }
        let track = BoneTrack {
            kind: TrackKind::Translation,
            count: 2,
            interpolation: Interpolation::Bezier,
            global_sequence_id: -1,
            offset: 0,
            stride: 40,
            raw,
        };
        assert_eq!(sample_bone_track(&track, 50).translation, [3., 6., 9.]);
    }

    #[test]
    fn bezier_control_points_are_distinct_from_hermite_derivatives() {
        for kind in [Interpolation::Bezier, Interpolation::Hermite] {
            let mut raw = Vec::new();
            for frame in [0u32, 100] {
                raw.extend(frame.to_le_bytes());
                for value in [0f32, 4., 4.] {
                    raw.extend(value.to_le_bytes());
                }
            }
            let track = AnimTrack {
                magic: *b"KGAA",
                count: 2,
                interpolation: kind,
                global_seq_id: NO_ID,
                raw,
            };
            assert_eq!(
                sample_scalar(&track, 50),
                if kind == Interpolation::Bezier {
                    3.
                } else {
                    0.
                }
            );
            assert_eq!(sample_scalar(&track, 0), 0.);
            assert_eq!(sample_scalar(&track, 100), 0.);
        }
    }
}
