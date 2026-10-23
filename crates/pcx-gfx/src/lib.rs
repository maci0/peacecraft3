//! Bevy asset loaders for the pcx-* decoders.
//!
//! This crate is the boundary between the format decoders and the engine, and it
//! is deliberately not the thing that proves the formats right — that is the
//! decoders' own sweeps. Its job is to take a decoded asset and produce
//! something Bevy can draw, without a second copy of the format knowledge.
//!
//! The one design constraint that matters ahead of the WASM target: nothing here
//! may do synchronous file I/O. Assets arrive either from an MPQ mount or, in a
//! browser, as bytes already in memory. So the loaders take `&[u8]` and a path,
//! never open anything themselves.

use bevy::asset::RenderAssetUsages;

pub mod dds;
pub mod dds_gpu;
pub mod ground;
pub mod mdx_billboard;
pub mod mdx_camera;
pub mod mdx_look_at;
pub mod mdx_opacity;
pub mod mdx_particle_render;
pub mod mdx_particles;
pub mod mdx_runtime;
pub mod mdx_texture_selection;
pub mod mdx_uv_translation;
pub mod performance;
pub mod viewer;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

pub use ground::{
    DecodedGround, GroundDraw, GroundError, GroundMaterial, GroundMaterialPlugin, GroundMemory,
    SelectedGroundBlp, decode_ground_slots, spawn_admitted_ground,
};

/// A texture ready for the GPU.
#[derive(Debug, Clone)]
pub struct LoadedTexture {
    /// Same dims and pixels the decoder produced.
    pub width: u32,
    pub height: u32,
    /// Straight RGBA8, row-major.
    pub rgba: Vec<u8>,
    /// Authored lower-resolution RGBA8 levels, starting at mip one.
    pub mipmaps: Vec<Vec<u8>>,
    /// Where the pixels came from, for a trace.
    pub source: String,
}

impl LoadedTexture {
    /// Validate the complete upload and count its decoded bytes before admission.
    pub fn rgba_bytes(&self) -> Option<usize> {
        rgba_mip_bytes(self.width, self.height, &self.rgba, &self.mipmaps)
    }
}

#[cfg(test)]
#[path = "image_mip_tests.rs"]
mod image_mip_tests;

/// Validate a contiguous, possibly partial RGBA8 mip chain without allocating.
pub fn rgba_mip_bytes(
    mut width: u32,
    mut height: u32,
    rgba: &[u8],
    mipmaps: &[Vec<u8>],
) -> Option<usize> {
    let bytes = |w, h| {
        usize::try_from(w)
            .ok()?
            .checked_mul(usize::try_from(h).ok()?)?
            .checked_mul(4)
    };
    if width == 0 || height == 0 || bytes(width, height)? != rgba.len() {
        return None;
    }
    let mut total = rgba.len();
    for mip in mipmaps {
        if width == 1 && height == 1 {
            return None;
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
        if bytes(width, height)? != mip.len() {
            return None;
        }
        total = total.checked_add(mip.len())?;
    }
    Some(total)
}

/// Preserve the supplied BLP mip chain for model textures; never regenerate its pixels.
/// The byte budget covers every level before any decoding allocation.
pub fn load_mipmapped_image_bounded(
    bytes: &[u8],
    source: &str,
    max_rgba_bytes: usize,
) -> Result<LoadedTexture, Error> {
    if AssetKind::detect(bytes) != AssetKind::Blp {
        return load_image_bounded(bytes, source, max_rgba_bytes);
    }
    let fail = |e: String| Error::Decode(format!("{source}: {e}"));
    let header = pcx_blp::Header::parse(bytes).map_err(|e| fail(e.to_string()))?;
    let count = header.mip_count();
    if count == 0 || header.mip_sizes[..count].contains(&0) {
        return Err(fail("missing or noncontiguous BLP mip levels".into()));
    }
    let mut total = 0usize;
    for level in 0..count {
        let (width, height) = header.mip_size(level);
        if width == 0 || height == 0 || (level > 0 && header.mip_size(level - 1) == (1, 1)) {
            return Err(fail("invalid BLP mip dimensions".into()));
        }
        let size = usize::try_from(width)
            .ok()
            .and_then(|w| usize::try_from(height).ok()?.checked_mul(w))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| fail("BLP mip byte count overflow".into()))?;
        total = total
            .checked_add(size)
            .ok_or_else(|| fail("BLP mip byte count overflow".into()))?;
        if total > max_rgba_bytes {
            return Err(fail("decoded RGBA mip budget exceeded".into()));
        }
        pcx_blp::mip_bytes(bytes, &header, level).map_err(|e| fail(e.to_string()))?;
    }
    let mut texture = load_image_bounded(bytes, source, max_rgba_bytes)?;
    texture
        .mipmaps
        .try_reserve_exact(count - 1)
        .map_err(|_| fail("BLP mip allocation failed".into()))?;
    for level in 1..count {
        let rgba = if header.content == 0 {
            pcx_blp::decode_jpeg_with_limit(bytes, &header, level, max_rgba_bytes)
        } else {
            pcx_blp::decode_mip(bytes, &header, level)
        }
        .map_err(|e| fail(e.to_string()))?;
        texture.mipmaps.push(rgba);
    }
    if texture.rgba_bytes() != Some(total) {
        return Err(fail("decoded BLP mip dimensions/length differ".into()));
    }
    Ok(texture)
}

/// Which format an asset turned out to be, decided by content and not by
/// extension alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetKind {
    Blp,
    Dds,
    Tga,
    Mdl,
    Unknown,
}

impl AssetKind {
    /// Classify by magic, not by suffix.
    ///
    /// The shipped set contains `.tga` files that hold no TGA and MDLs whose
    /// names end in `.mdl` while their contents are `MDLX`, so the extension is
    /// only a hint and the bytes decide.
    pub fn detect(bytes: &[u8]) -> Self {
        match bytes.get(0..4) {
            Some(b"BLP1") | Some(b"BLP0") | Some(b"BLP2") => AssetKind::Blp,
            Some(b"DDS ") => AssetKind::Dds,
            Some(b"MDLX") => AssetKind::Mdl,
            _ if looks_like_tga(bytes) => AssetKind::Tga,
            _ => AssetKind::Unknown,
        }
    }
}

/// A TGA footer is 26 bytes ending in `TRUEVISION-XFILE.`; a TGA header's
/// colour-map type is 0 or 1 and its image type is one of a small set.
fn looks_like_tga(d: &[u8]) -> bool {
    if d.len() < 18 {
        return false;
    }
    let colour_map = d[1];
    let image_type = d[2];
    colour_map <= 1 && matches!(image_type, 0 | 1 | 2 | 3 | 9 | 10 | 11 | 32 | 33)
}

/// Validate decoded dimensions against caller policy before allocating a menu image.
pub fn load_image_bounded(
    bytes: &[u8],
    source: &str,
    max_rgba_bytes: usize,
) -> Result<LoadedTexture, Error> {
    let fail = |e| Error::Decode(format!("{source}: {e}"));
    let (width, height, blp) = match AssetKind::detect(bytes) {
        AssetKind::Blp => {
            let h = pcx_blp::Header::parse(bytes).map_err(|e| fail(e.to_string()))?;
            (h.width, h.height, Some(h))
        }
        AssetKind::Tga => {
            let h = pcx_tga::Header::parse(bytes).map_err(|e| fail(e.to_string()))?;
            (u32::from(h.width), u32::from(h.height), None)
        }
        _ => return load_image(bytes, source),
    };
    let size = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok()?.checked_mul(w))
        .and_then(|pixels| pixels.checked_mul(4));
    if width == 0 || height == 0 || size.is_none_or(|size| size > max_rgba_bytes) {
        return Err(fail("decoded RGBA budget exceeded".into()));
    }
    if let Some(h) = blp.filter(|h| h.content == 0) {
        let rgba = pcx_blp::decode_jpeg_with_limit(bytes, &h, 0, max_rgba_bytes)
            .map_err(|e| fail(e.to_string()))?;
        return Ok(LoadedTexture {
            mipmaps: Vec::new(),
            width,
            height,
            rgba,
            source: source.into(),
        });
    }
    load_image(bytes, source)
}

/// Loads bytes into whatever the format turns out to be.
///
/// Returns the decoded texture for image formats. Models are handled by
/// [`load_model`], because a model is not one texture.
/// DDS callers must supply resource limits through [`dds::decode`].
pub fn load_image(bytes: &[u8], source: &str) -> Result<LoadedTexture, crate::Error> {
    let kind = AssetKind::detect(bytes);
    match kind {
        AssetKind::Dds => Err(dds::Error {
            source_name: source.into(),
            offset: 0,
            kind: dds::Failure::Unsupported(
                "DDS requires explicit caller limits; use dds::decode".into(),
            ),
        }
        .into()),
        AssetKind::Blp => {
            // Two content paths, and the header says which: direct (palettised)
            // and JPEG. Routing both through the palettised decode fails on
            // 4,924 of the 6,696 shipped textures, which is most of them.
            let h = pcx_blp::Header::parse(bytes)
                .map_err(|e| crate::Error::Decode(format!("{source}: {e}")))?;
            let (w, hgt, rgba) = if h.content == 0 {
                (
                    h.width,
                    h.height,
                    pcx_blp::decode_jpeg(bytes, &h, 0)
                        .map_err(|e| crate::Error::Decode(format!("{source}: {e}")))?,
                )
            } else {
                let (hh, px) = pcx_blp::decode(bytes)
                    .map_err(|e| crate::Error::Decode(format!("{source}: {e}")))?;
                (hh.width, hh.height, px)
            };
            Ok(LoadedTexture {
                mipmaps: Vec::new(),
                width: w,
                height: hgt,
                rgba,
                source: source.to_string(),
            })
        }
        AssetKind::Tga => {
            let (h, rgba) = pcx_tga::decode(bytes)
                .map_err(|e| crate::Error::Decode(format!("{source}: {e}")))?;
            Ok(LoadedTexture {
                mipmaps: Vec::new(),
                width: u32::from(h.width),
                height: u32::from(h.height),
                rgba,
                source: source.to_string(),
            })
        }
        AssetKind::Mdl | AssetKind::Unknown => Err(crate::Error::Decode(format!(
            "{source}: not an image format ({kind:?})"
        ))),
    }
}

/// A posed skeleton: world-space matrices for every supplied BONE/HELP joint.
#[derive(Debug, Clone)]
pub struct Skeleton {
    /// One 4x4 matrix per joint, in the supplied hierarchy's order.
    pub matrices: Vec<[f32; 16]>,
    /// Each bone's **node id**, parallel to `matrices`.
    ///
    /// A geoset's `MATS` entries name bones by node id, and node ids are not
    /// array positions: the node space also holds helpers, so the ids in a
    /// model with helpers are sparse. Indexing `matrices` with an id therefore
    /// picks the wrong bone — with 374 bones and 417 nodes in `MainMenu3d` the
    /// error compounds down the chain. This is what makes the lookup correct.
    pub ids: Vec<u32>,
    /// Joint names, parallel to `matrices`.
    pub names: Vec<String>,
}

/// Builds a posed skeleton from a model.
///
/// `parent` resolves a bone's parent node, which is what makes a bone that hangs
/// from a helper resolve correctly rather than being treated as a root.
#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "animation.pose_skeleton")
)]
pub fn pose_skeleton(
    _model: &LoadedModel,
    bones: &[pcx_mdl::Bone],
    pivots: &pcx_mdl::Pivots,
    frame: u32,
) -> Skeleton {
    // `parentId` addresses the whole node space, so resolve it against the
    // supplied BONE/HELP list, not LoadedModel's bones-only name table.
    let by_id: std::collections::HashMap<u32, usize> = bones
        .iter()
        .enumerate()
        .map(|(i, b)| (b.object_id, i))
        .collect();
    let parent = |object_id: u32| -> Option<u32> {
        let b = bones.iter().find(|b| b.object_id == object_id)?;
        if b.parent_id < 0 {
            return None;
        }
        let p = b.parent_id as u32;
        by_id.contains_key(&p).then_some(p)
    };
    let matrices = pcx_mdl::bone_matrices(bones, &parent, pivots, frame);
    Skeleton {
        matrices,
        ids: bones.iter().map(|b| b.object_id).collect(),
        names: bones.iter().map(|joint| joint.name.clone()).collect(),
    }
}

/// Builds a posed skeleton from poses sampled elsewhere.
///
/// [`pose_skeleton`] samples the model's own tracks at a frame; this is the
/// same composition when the poses have already been evaluated — by Bevy's
/// animation system, say. It borrows poses directly through the decoder's
/// composition, including its existing hierarchy and cycle behavior.
///
/// `bones` and `poses` are parallel; `pivots` is the model's own pivot table.
pub fn pose_from_poses(
    _model: &LoadedModel,
    bones: &[pcx_mdl::Bone],
    pivots: &pcx_mdl::Pivots,
    poses: &[pcx_mdl::Pose],
) -> Skeleton {
    let by_id: std::collections::HashMap<u32, usize> = bones
        .iter()
        .enumerate()
        .map(|(i, b)| (b.object_id, i))
        .collect();
    let parent = |object_id: u32| -> Option<u32> {
        let b = bones.iter().find(|b| b.object_id == object_id)?;
        if b.parent_id < 0 {
            return None;
        }
        let p = b.parent_id as u32;
        by_id.contains_key(&p).then_some(p)
    };
    let matrices = pcx_mdl::bone_matrices_from_poses(bones, &parent, pivots, poses);
    Skeleton {
        matrices,
        ids: bones.iter().map(|b| b.object_id).collect(),
        names: bones.iter().map(|joint| joint.name.clone()).collect(),
    }
}

/// Skins a mesh's vertices by a posed skeleton.
///
/// Each vertex's matrix group comes from `GNDX`, and that group's bone is the
/// slice of `MATS` the group counts select. This is the whole point of the mesh
/// path: without it a model shows its rest pose, which looks fine and is wrong.
#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "animation.skin_mesh")
)]
pub fn skin_mesh(mesh: &LoadedMesh, skeleton: &Skeleton) -> Vec<[f32; 3]> {
    // Flatten the group counts into a per-group bone index, exactly as the
    // decoder's invariant says they nest.
    let mut group_matrix = Vec::with_capacity(mesh.group_counts.len());
    let mut at = 0usize;
    for &c in &mesh.group_counts {
        let n = c as usize;
        // The first bone of a group is the one that carries the whole group,
        // which is what a single-matrix skin needs.
        // Resolve sparse node ids once per group, rather than per vertex.
        let matrix = mesh
            .bone_indices
            .get(at)
            .and_then(|id| skeleton.ids.iter().position(|found| found == id))
            .and_then(|index| skeleton.matrices.get(index));
        group_matrix.push(matrix);
        at += n;
    }

    mesh.positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let bone = mesh
                .vertex_groups
                .get(i)
                .and_then(|&g| group_matrix.get(g as usize).copied().flatten());
            match bone {
                Some(m) => pcx_mdl::transform_point(m, *p),
                None => *p,
            }
        })
        .collect()
}

/// Avoid marking a mesh changed (and uploading it again) when its position bits match.
/// Missing handles retain the skinning callers' existing no-op behavior.
#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "render.mesh_positions")
)]
pub fn set_mesh_positions(
    meshes: &mut Assets<Mesh>,
    handle: &Handle<Mesh>,
    positions: Vec<[f32; 3]>,
) -> bool {
    let Some(mesh) = meshes.get(handle) else {
        return false;
    };
    if mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|a| a.as_float3())
        .is_some_and(|old| {
            old.len() == positions.len()
                && old
                    .iter()
                    .zip(&positions)
                    .all(|(a, b)| a.map(f32::to_bits) == b.map(f32::to_bits))
        })
    {
        return false;
    }
    meshes
        .get_mut(handle)
        .expect("checked mesh")
        .insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    true
}

/// A Bevy-ready mesh built from a decoded geoset.
#[derive(Debug, Clone)]
pub struct LoadedMesh {
    /// Positions, one `[f32; 3]` per vertex.
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tex_coords: Vec<[f32; 2]>,
    /// Zero-based indices into the arrays above.
    pub indices: Vec<u16>,
    /// Matrix group per vertex, indexing into [`LoadedMesh::bone_indices`]
    /// through the group counts.
    pub vertex_groups: Vec<u8>,
    /// Bone indices per group.
    pub bone_indices: Vec<u32>,
    /// Indices into the arrays above, per group.
    pub group_counts: Vec<u32>,
    /// Name of the geoset this came from, for a trace.
    pub geoset: usize,
    /// Index into the model's `MTLS` materials, and the answer to "which
    /// texture does this geoset draw with".
    ///
    /// Read from the geoset's own trailer: the first `u32` after the `MATS`
    /// body. It is measured, not inferred — see
    /// `docs/re-notes/004-mdx.md`, "Geoset→material — measured, not inferred",
    /// for the framing proof and the 8,770/8,770 geoset sweep.
    pub material_id: u32,
}

/// A decoded model, with every geoset the format carries.
#[derive(Debug, Clone)]
pub struct LoadedModel {
    pub name: String,
    pub minimum_extent: [f32; 3],
    pub maximum_extent: [f32; 3],
    pub meshes: Vec<LoadedMesh>,
    /// Bone names in file order; `parent_id[i]` indexes this same list or the
    /// wider node space, matching the decoder's own reading.
    pub bone_names: Vec<String>,
    pub bone_parents: Vec<i32>,
    /// Animation sequences, in file order, with their duration and looping.
    pub sequences: Vec<Sequence>,
}

/// Animation state for one model: which clip is playing and where it is.
#[derive(Debug, Clone)]
pub struct Playback {
    /// Index into the model's sequence list.
    pub sequence: usize,
    /// Milliseconds into the sequence.
    pub time_ms: f32,
    /// True when the clip repeats.
    pub looping: bool,
}

impl Playback {
    /// Advances a clip by `dt_ms`, wrapping or clamping at its end.
    ///
    /// Wrapping a loop and clamping a one-shot are different behaviours and get
    /// them backwards a one-shot would spin forever, which looks like nothing
    /// more than an attack that never finishes.
    pub fn advance(&mut self, dt_ms: f32, duration_ms: f32, loops: bool) {
        if duration_ms <= 0.0 {
            return;
        }
        self.time_ms += dt_ms;
        if self.time_ms > duration_ms {
            if loops {
                self.time_ms %= duration_ms;
            } else {
                self.time_ms = duration_ms;
            }
        }
    }
}

/// A named animation clip.
#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    pub name: String,
    /// Duration in milliseconds.
    pub duration_ms: u32,
    /// Clip start, in milliseconds.
    pub start_ms: u32,
    pub loops: bool,
}

/// Decode a model into Bevy-shaped geometry.
///
/// A model with no geosets remains legal and distinct from a parse failure.
/// Graphics import requires finite bounds; extent consumers validate their use.
#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "assets.model_load")
)]
pub fn load_model(bytes: &[u8], source: &str) -> Result<LoadedModel, crate::Error> {
    let m =
        pcx_mdl::Mdlx::parse(bytes).map_err(|e| crate::Error::Decode(format!("{source}: {e}")))?;
    let model = m
        .model(bytes)
        .map_err(|e| crate::Error::Decode(format!("{source}: {e}")))?;
    for axis in 0..3 {
        let minimum = model.minimum_extent[axis];
        let maximum = model.maximum_extent[axis];
        if !minimum.is_finite() || !maximum.is_finite() {
            return Err(crate::Error::Decode(format!(
                "{source}: nonfinite MODL extent axis {axis}: {minimum}..{maximum}"
            )));
        }
    }

    let mut meshes = Vec::new();
    let geosets = m
        .geosets(bytes)
        .map_err(|e| crate::Error::Decode(format!("{source}: {e}")))?;
    for (i, g) in geosets.iter().enumerate() {
        let mesh = m
            .mesh(bytes, g)
            .map_err(|e| crate::Error::Decode(format!("{source} geoset {i}: {e}")))?;
        meshes.push(LoadedMesh {
            positions: mesh.vertices.iter().collect(),
            normals: mesh.normals.iter().collect(),
            tex_coords: (0..mesh.tex_coords.count as usize)
                .filter_map(|k| mesh.tex_coords.get(k))
                .collect(),
            indices: mesh
                .triangles
                .raw
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect(),
            // `MTGC` says how many indices the groups actually use. The MATS
            // span is wider than that — a footman's block ends in float rows
            // the group counts do not reach — so trim to the semantic count
            // rather than handing Bevy numbers that index nothing.
            vertex_groups: mesh.vertex_groups.clone(),
            bone_indices: {
                let want: usize = mesh.matrix_groups.iter().map(|&c| c as usize).sum();
                mesh.matrix_indices[..want.min(mesh.matrix_indices.len())].to_vec()
            },
            group_counts: mesh.matrix_groups.clone(),
            geoset: i,
            material_id: mesh.material_id,
        });
    }

    let bones = m
        .bones(bytes)
        .map_err(|e| crate::Error::Decode(format!("{source}: {e}")))?;
    let (bone_names, bone_parents) = bones.iter().map(|b| (b.name.clone(), b.parent_id)).unzip();

    let sequences = m
        .sequences(bytes)
        .map_err(|e| crate::Error::Decode(format!("{source}: {e}")))?
        .into_iter()
        .map(|s| crate::Sequence {
            duration_ms: s.duration(),
            start_ms: s.interval_start,
            loops: !s.non_looping,
            name: s.name,
        })
        .collect();

    Ok(LoadedModel {
        name: model.name,
        minimum_extent: model.minimum_extent,
        maximum_extent: model.maximum_extent,
        meshes,
        bone_names,
        bone_parents,
        sequences,
    })
}

/// Number of bone assignments per vertex a model needs.
///
/// 1.28c/1.29b classic models use up to four influences per vertex, expressed
/// through the matrix-group tables rather than per-vertex weights, so this is a
/// property of the group tables rather than of the vertex stream.
pub const MAX_INFLUENCES: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{source_name}: invalid terrain preview bounds on axis {axis}: {minimum}..{maximum}")]
    TerrainBounds {
        source_name: String,
        axis: usize,
        minimum: f32,
        maximum: f32,
    },
    #[error(transparent)]
    Terrain(#[from] viewer::TerrainTileError),
    #[error(transparent)]
    Dds(#[from] dds::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("cannot decode: {0}")]
    Decode(String),
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
}

/// A Bevy plugin wiring the loaders as asset sources.
pub struct PcxGfxPlugin;

impl Plugin for PcxGfxPlugin {
    fn build(&self, _app: &mut App) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install() -> Option<std::path::PathBuf> {
        for k in ["PEACECRAFT3_INSTALL", "PEACECRAFT3_LEGACY_DIR"] {
            if let Ok(v) = std::env::var(k) {
                let p = std::path::PathBuf::from(v);
                if p.is_dir() {
                    return Some(p);
                }
            }
        }
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .parent()?
            .join("wc3/game/app");
        p.is_dir().then_some(p)
    }

    fn shipped(rel: &str) -> Option<Vec<u8>> {
        let dir = install()?;
        pcx_mpq::MpqSet::open_wc3(dir).ok()?.extract(rel).ok()
    }

    const FOOTMAN: &str = "Units\\Human\\Footman\\Footman.mdx";

    /// Both BLP content paths must load, because they are both shipped.
    ///
    /// `load_image` used to route everything through the palettised decode,
    /// which fails on every JPEG-content texture — 4,924 of the 6,696 in the
    /// 1.29b set. A texture that fails to load falls back to flat colour, so
    /// nothing looks broken; the model just has no texture on it.
    #[test]
    fn loads_both_blp_content_paths() {
        let Some(dir) = install() else { return };
        let mut s = pcx_mpq::MpqSet::open_wc3(dir).unwrap();

        // Palettised, and JPEG — one of each.
        for (what, rel) in [
            (
                "palettised",
                "ReplaceableTextures\\Selection\\SelectionCircleLarge.blp",
            ),
            ("jpeg", "TerrainArt\\Ashenvale\\Ashen_DirtGrass.blp"),
        ] {
            let Some(b) = s.extract(rel).ok() else {
                continue;
            };
            let t = load_image(&b, rel).unwrap_or_else(|e| panic!("{what} texture {rel}: {e}"));
            assert!(t.width > 0 && t.height > 0, "{what}: zero-sized");
            assert_eq!(
                t.rgba.len(),
                (t.width * t.height * 4) as usize,
                "{what}: wrong pixel count"
            );
        }
    }

    #[test]
    fn detects_formats_by_magic_not_by_name() {
        assert_eq!(AssetKind::detect(b"BLP1...."), AssetKind::Blp);
        assert_eq!(AssetKind::detect(b"MDLXVERS"), AssetKind::Mdl);
        // A real TGA header the shipped set uses: 24bpp, no colour map.
        // 18 bytes is the minimum the detector trusts, so pad the probe.
        let mut tga = [0u8; 18];
        tga[1] = 0; // no colour map
        tga[2] = 2; // uncompressed truecolour
        assert_eq!(AssetKind::detect(&tga), AssetKind::Tga);
        // Something with neither magic nor a plausible TGA header is Unknown,
        // even if its name says otherwise.
        assert_eq!(
            AssetKind::detect(b"\x00\x01\x02\x03garbage"),
            AssetKind::Unknown
        );
    }

    #[test]
    fn loads_a_texture_through_the_bevy_path() {
        let Some(d) = shipped("ReplaceableTextures\\Selection\\SelectionCircleLarge.blp") else {
            return;
        };
        let t = load_image(&d, "SelectionCircleLarge.blp").expect("decode");
        assert_eq!(t.width, 128);
        assert_eq!(t.height, 128);
        assert_eq!(t.rgba.len(), 128 * 128 * 4);
        assert_eq!(t.source, "SelectionCircleLarge.blp");
    }

    #[test]
    fn loads_the_footman_with_every_geoset_and_its_skeleton() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = load_model(&d, FOOTMAN).unwrap_or_else(|e| panic!("{FOOTMAN}: {e}"));

        assert_eq!(m.name, "Footman");
        assert_eq!(m.meshes.len(), 5, "five geosets");
        assert_eq!(m.sequences.len(), 13, "thirteen clips");
        assert_eq!(m.bone_parents.len(), 25, "twenty-five bones");

        let body = &m.meshes[0];
        assert_eq!(body.positions.len(), 447);
        assert_eq!(body.normals.len(), 447);
        assert_eq!(body.tex_coords.len(), 447);
        assert_eq!(body.indices.len(), 864);
        // The skin tables must survive the trip into Bevy's shapes intact.
        let total: usize = body.group_counts.iter().map(|&c| c as usize).sum();
        assert_eq!(total, body.bone_indices.len(), "MTGC must consume MATS");
        assert!(
            body.bone_indices
                .iter()
                .all(|&b| (b as usize) < m.bone_parents.len()),
            "every bone index must address a real bone"
        );
        // Exactly one root, as the decoder found.
        assert_eq!(m.bone_parents.iter().filter(|&&p| p == -1).count(), 1);
    }

    /// Poses evaluated outside the decoder must compose exactly as the
    /// decoder's own track sampling composes them.
    ///
    /// [`pose_from_poses`] is the adapter that lets Bevy's animation system
    /// pose the skeleton, and it is only safe if the matrices it builds are
    /// the ones [`pose_skeleton`] would have built at the same instant. The
    /// Footman is checked at rest, at the 7,000 ms capture pose, and past the
    /// end of its first clip.
    #[test]
    fn externally_posed_bones_compose_exactly_as_the_decoder_sampler() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let m = load_model(&d, FOOTMAN).unwrap_or_else(|e| panic!("{FOOTMAN}: {e}"));
        let parsed = pcx_mdl::Mdlx::parse(&d).unwrap();
        let bones = parsed.bones(&d).unwrap_or_default();
        let pivots = parsed.pivots(&d).unwrap_or_default();

        for frame in [0u32, 7_000, 12_000] {
            let sampled = pose_skeleton(&m, &bones, &pivots, frame);
            let poses = pcx_mdl::sample_skeleton(&bones, frame);
            let posed = pose_from_poses(&m, &bones, &pivots, &poses);
            assert_eq!(
                sampled.matrices, posed.matrices,
                "frame {frame}: external poses must compose to the decoder's matrices"
            );
        }
    }

    /// A model with no geosets is a legal model, not a parse failure.
    #[test]
    fn an_empty_model_is_not_an_error() {
        let Some(d) = shipped("UI\\Glues\\ScoreScreen\\ScoreScreen-CenterBar.mdl") else {
            return;
        };
        match load_model(&d, "ScoreScreen-CenterBar.mdl") {
            Ok(m) => assert!(m.meshes.len() <= 5, "unexpected geoset count"),
            Err(crate::Error::Decode(msg)) => {
                panic!("a shipped must be parseable, got {msg}")
            }
            Err(e) => panic!("{e}"),
        }
    }
}
/// Builds a Bevy mesh from a decoded geoset.
///
/// Positions, normals and UVs pass straight through, which is safe because the
/// decoder already produced exactly this shape. The only thing added is Bevy's
/// mesh layout.
/// Unbound geosets release the uploaded vertex data; bound ones retain it for CPU skinning.
pub fn to_bevy_mesh(m: &LoadedMesh) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        if m.bone_indices.is_empty() {
            RenderAssetUsages::RENDER_WORLD
        } else {
            // CPU skinning still rewrites bound vertices each animation update.
            RenderAssetUsages::default()
        },
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, m.tex_coords.clone())
    .with_inserted_indices(Indices::U16(m.indices.clone()))
}

/// Texture paths a model's materials refer to, in material order.
///
/// Deliberately returns *paths* rather than loaded pixels: resolving a path to
/// bytes is the caller's job, and it is the one thing that differs between a
/// desktop mount and a browser fetch. Keeping it out is what lets the same
/// loader serve both.
pub fn model_texture_paths(bytes: &[u8], source: &str) -> Result<Vec<String>, Error> {
    let m = pcx_mdl::Mdlx::parse(bytes).map_err(|e| Error::Decode(format!("{source}: {e}")))?;
    let texs = m
        .textures(bytes)
        .map_err(|e| Error::Decode(format!("{source}: {e}")))?;
    let materials = m
        .materials(bytes)
        .map_err(|e| Error::Decode(format!("{source}: {e}")))?;

    // One slot per material. The counts do *not* line up with the geosets — a
    // footman ships five geosets and three materials — so this is per-material
    // information and nothing more; [`to_bevy`] makes the join through each
    // geoset's `material_id`.
    Ok(materials
        .iter()
        .map(|mat| {
            mat.tex_layers()
                .first()
                .and_then(|l| texs.get(l.texture_id as usize))
                .filter(|t| !t.path.is_empty())
                .map(|t| t.path.clone())
                .unwrap_or_default()
        })
        .collect())
}

/// A model as Bevy geometry, plus the texture path each geoset draws with.
///
/// The geoset→material chain is resolved from the bytes rather than inferred.
/// `GEOS` carries a `materialId` as the first `u32` after the `MATS` body, the
/// decoder reads it there, and each geoset's texture is the path of the
/// material it names. `docs/re-notes/004-mdx.md` section "Geoset→material —
/// measured, not inferred" holds the framing proof.
pub fn to_bevy(bytes: &[u8], source: &str) -> Result<Vec<(Mesh, String)>, Error> {
    let model = load_model(bytes, source)?;
    let paths = model_texture_paths(bytes, source)?;
    // The geoset names its material on disk, so this is a lookup by
    // `material_id` and not a guess from the geoset's position.
    //
    // It is the measurement that matters: the Footman's five geosets carry
    // `0, 1, 0, 0, 2`, where geoset 0 is the body and geoset 4 is the blood
    // piece that takes `Textures\gutz.blp`. Index cycling against the three
    // materials gave `0, 1, 2, 0, 1` instead — the blood geoset drawn with the
    // empty-texture material and a body piece with `gutz.blp`. That pairing
    // held for only 81.9% of the shipped set, so **521 models and 3,121
    // geosets** drew with the wrong texture.
    //
    // Falling back to the first path is defensive and never taken by the
    // shipped models: `materialId` is below the model's own material count for
    // 8,770 of 8,770 geosets across 3,249 models, with ids up to 56 in a model
    // that has 56 materials. A geoset that does reach the fallback names a
    // material the model does not carry, which the tests assert against rather
    // than resting on this branch.
    Ok(model
        .meshes
        .iter()
        .map(|m| {
            let path = paths
                .get(m.material_id as usize)
                .or_else(|| paths.first())
                .cloned()
                .unwrap_or_default();
            (to_bevy_mesh(m), path)
        })
        .collect())
}

#[cfg(test)]
mod skinning_tests {
    use super::*;

    /// A synthetic skeleton, so the skinning math is checked against known
    /// numbers rather than against a shipped model whose key frames move.
    fn bone(object_id: u32, parent: i32) -> pcx_mdl::Bone {
        pcx_mdl::Bone {
            offset: 0,
            span: 0,
            name: format!("b{object_id}"),
            object_id,
            parent_id: parent,
            flags: 0x100,
            tracks: Vec::new(),
            geoset_id: -1,
            geoset_anim_id: -1,
        }
    }

    /// A bone with one translation key of `value`, held from frame 0.
    fn translating(object_id: u32, parent: i32, value: [f32; 3]) -> pcx_mdl::Bone {
        let mut b = bone(object_id, parent);
        let mut raw = Vec::new();
        raw.extend_from_slice(&0u32.to_le_bytes()); // frame
        for v in value {
            raw.extend_from_slice(&v.to_le_bytes());
        }
        raw.extend_from_slice(&0.0f32.to_le_bytes()); // key slot padding to w
        b.tracks.push(pcx_mdl::BoneTrack {
            kind: pcx_mdl::TrackKind::Translation,
            count: 1,
            interpolation: pcx_mdl::Interpolation::None,
            global_sequence_id: -1,
            offset: 0,
            stride: 16,
            raw,
        });
        b
    }

    #[test]
    fn a_posed_bone_moves_the_vertices_it_owns() {
        let rig = vec![bone(0, -1), bone(1, 0)];
        let pivots = pcx_mdl::Pivots(vec![[0.0; 3], [0.0; 3]]);
        let model = LoadedModel {
            name: "t".into(),
            minimum_extent: [0.0; 3],
            maximum_extent: [0.0; 3],
            meshes: Vec::new(),
            bone_names: vec!["a".into(), "b".into()],
            bone_parents: vec![-1, 0],
            sequences: Vec::new(),
        };
        let sk = pose_skeleton(&model, &rig, &pivots, 0);
        assert_eq!(sk.matrices.len(), 2);
        assert_eq!(sk.names.len(), 2, "names stay parallel to matrices");

        // A mesh whose one vertex is owned by group 0, which owns bone 0.
        let mesh = LoadedMesh {
            positions: vec![[0.0, 0.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]],
            tex_coords: vec![[0.0, 0.0]],
            indices: vec![0],
            vertex_groups: vec![0],
            bone_indices: vec![0, 0],
            group_counts: vec![1, 1],
            geoset: 0,
            material_id: 0,
        };
        let moved = skin_mesh(&mesh, &sk);
        assert_eq!(moved.len(), 1, "one vertex in, one vertex out");
        // With an identity skeleton the vertex must not move.
        assert_eq!(
            moved[0],
            [0.0, 0.0, 0.0],
            "an unposed skeleton must not move a vertex"
        );
    }

    #[test]
    fn a_mesh_inherits_its_animated_helper_and_keeps_parallel_joint_names() {
        let helper = pcx_mdl::Node {
            offset: 0,
            size: 128,
            name: "animated helper".into(),
            object_id: 2,
            parent_id: -1,
            flags: 0,
            tracks: vec![pcx_mdl::KeyTrack {
                magic: *b"KGTR",
                count: 1,
                interpolation: pcx_mdl::Interpolation::None,
                global_seq_id: pcx_mdl::NO_ID,
                width: 3,
                stride: 16,
                raw: 0u32
                    .to_le_bytes()
                    .into_iter()
                    .chain([4.0f32, 0.0, 0.0].into_iter().flat_map(f32::to_le_bytes))
                    .collect(),
            }],
        };
        let plan = mdx_runtime::JointPlan::checked(
            &[bone(7, 2)],
            &[helper],
            &pcx_mdl::Pivots(vec![[0.0; 3]; 8]),
        )
        .unwrap();
        // LoadedModel exposes bones only. HELP must still have a parallel name.
        let model = LoadedModel {
            name: "helper hierarchy".into(),
            minimum_extent: [0.0; 3],
            maximum_extent: [0.0; 3],
            meshes: vec![],
            bone_names: vec!["b7".into()],
            bone_parents: vec![2],
            sequences: vec![],
        };
        let poses = pcx_mdl::sample_skeleton(plan.joints(), 0);
        let posed = pose_from_poses(&model, plan.joints(), plan.pivots(), &poses);
        assert_eq!(posed.ids, [7, 2]);
        assert_eq!(posed.names, ["b7", "animated helper"]);
        assert_eq!(posed.names.len(), posed.matrices.len());
        let mesh = LoadedMesh {
            positions: vec![[1.0, 0.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]],
            tex_coords: vec![[0.0; 2]],
            indices: vec![0],
            vertex_groups: vec![0],
            bone_indices: vec![7],
            group_counts: vec![1],
            geoset: 0,
            material_id: 0,
        };
        assert_eq!(skin_mesh(&mesh, &posed), [[5.0, 0.0, 0.0]]);
    }

    #[test]
    fn a_vertex_owned_by_no_group_survives_unchanged() {
        let rig = vec![bone(0, -1)];
        let pivots = pcx_mdl::Pivots(vec![[0.0; 3]]);
        let sk = pose_skeleton(
            &LoadedModel {
                name: "t".into(),
                minimum_extent: [0.0; 3],
                maximum_extent: [0.0; 3],
                meshes: Vec::new(),
                bone_names: vec!["a".into()],
                bone_parents: vec![-1],
                sequences: Vec::new(),
            },
            &rig,
            &pivots,
            0,
        );
        // Group 5 does not exist, so this vertex must pass through untouched
        // rather than being dropped or zeroed.
        let mesh = LoadedMesh {
            positions: vec![[3.0, 4.0, 5.0]],
            normals: vec![[0.0, 0.0, 1.0]],
            tex_coords: vec![[0.0, 0.0]],
            indices: vec![0],
            vertex_groups: vec![5],
            bone_indices: vec![0],
            group_counts: vec![1],
            geoset: 0,
            material_id: 0,
        };
        assert_eq!(skin_mesh(&mesh, &sk)[0], [3.0, 4.0, 5.0]);
    }

    #[test]
    fn a_child_bone_inherits_its_parents_transform() {
        // Parent sits at +10 on X and the child at +1, so the child's origin
        // must land at +11 — which only happens if the parent's world matrix is
        // composed in rather than started from identity.
        let rig = vec![
            translating(0, -1, [10.0, 0.0, 0.0]),
            translating(1, 0, [1.0, 0.0, 0.0]),
        ];
        let pivots = pcx_mdl::Pivots(vec![[0.0; 3], [0.0; 3]]);
        let by_id: std::collections::HashMap<u32, usize> = rig
            .iter()
            .enumerate()
            .map(|(i, b)| (b.object_id, i))
            .collect();
        let parent = |id: u32| -> Option<u32> {
            let b = rig.iter().find(|b| b.object_id == id)?;
            (b.parent_id >= 0)
                .then_some(b.parent_id as u32)
                .filter(|p| by_id.contains_key(p))
        };

        let world = pcx_mdl::bone_matrices(&rig, &parent, &pivots, 0);
        let child_origin = pcx_mdl::transform_point(&world[1], [0.0, 0.0, 0.0]);
        assert_eq!(
            child_origin[0], 11.0,
            "the child must sit at parent + own offset, not its own offset alone"
        );
        let parent_origin = pcx_mdl::transform_point(&world[0], [0.0, 0.0, 0.0]);
        assert_eq!(parent_origin[0], 10.0);
    }
}

#[cfg(test)]
mod playback_tests {
    use super::*;

    #[test]
    fn a_looping_clip_wraps_and_a_one_shot_clamps() {
        let mut looping = Playback {
            sequence: 0,
            time_ms: 0.0,
            looping: true,
        };
        looping.advance(1_500.0, 1_000.0, true);
        assert_eq!(looping.time_ms, 500.0, "a looping clip wraps, not runs on");

        let mut once = Playback {
            sequence: 0,
            time_ms: 0.0,
            looping: false,
        };
        once.advance(1_500.0, 1_000.0, false);
        assert_eq!(
            once.time_ms, 1_000.0,
            "a one-shot clip holds its last frame"
        );
    }

    #[test]
    fn a_zero_length_clip_cannot_be_advanced() {
        let mut p = Playback {
            sequence: 0,
            time_ms: 0.0,
            looping: true,
        };
        // This must not panic, and must not move.
        p.advance(100.0, 0.0, true);
        assert_eq!(p.time_ms, 0.0);
    }

    #[test]
    fn sequences_carry_their_duration_and_looping() {
        let Some(bytes) = shipped_model(r#"Units\Human\Footman\Footman.mdx"#) else {
            return;
        };
        let m = load_model(&bytes, "Footman").unwrap();
        assert!(!m.sequences.is_empty(), "a footman animates");
        let s = &m.sequences[0];
        assert_eq!(s.name, "Stand - 1");
        assert_eq!(s.duration_ms, 1_500);
        assert!(s.loops, "a stand cycles");
        // Attacks do not loop.
        if let Some(attack) = m.sequences.iter().find(|x| x.name.starts_with("Attack")) {
            assert!(!attack.loops, "an attack is one-shot: {}", attack.name);
        }
    }

    fn shipped_model(rel: &str) -> Option<Vec<u8>> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .parent()?
            .join("wc3/game/app");
        if !dir.is_dir() {
            return None;
        }
        pcx_mpq::MpqSet::open_wc3(dir).ok()?.extract(rel).ok()
    }
}

#[cfg(test)]
mod bevy_tests {
    use super::*;

    fn shipped(rel: &str) -> Option<Vec<u8>> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .parent()?
            .join("wc3/game/app");
        if !dir.is_dir() {
            return None;
        }
        pcx_mpq::MpqSet::open_wc3(dir).ok()?.extract(rel).ok()
    }

    const FOOTMAN: &str = "Units\\Human\\Footman\\Footman.mdx";

    #[test]
    fn the_footman_becomes_bevy_meshes_with_texture_paths() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let parts = to_bevy(&d, FOOTMAN).unwrap_or_else(|e| panic!("{FOOTMAN}: {e}"));

        assert_eq!(parts.len(), 5, "one Bevy mesh per geoset");

        // The body geoset must draw with the body texture, and the attachment
        // geosets must not all collapse onto it.
        assert!(
            parts[0].1.to_lowercase().contains("footman"),
            "the body geoset should draw with the body texture, got {:?}",
            parts[0].1
        );
        assert!(
            parts.iter().skip(1).any(|(_, p)| !p.contains("Footman")),
            "attachment geosets should use the other materials: {:?}",
            parts.iter().map(|(_, p)| p).collect::<Vec<_>>()
        );

        // The body geoset must carry its full geometry into the Bevy mesh.
        let body = &parts[0].0;
        use bevy::mesh::VertexAttributeValues as V;
        let n_pos = match body.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(V::Float32x3(x)) => x.len(),
            _ => panic!("positions must be Float32x3"),
        };
        let n_uv = match body.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(V::Float32x2(x)) => x.len(),
            _ => panic!("uvs must be Float32x2"),
        };
        assert_eq!(n_pos, 447, "every vertex must survive into Bevy's layout");
        assert_eq!(n_uv, 447);
    }

    /// The geoset→material binding is measured, and the Footman is the model
    /// that was wrong before it was.
    ///
    /// Its five geosets read `0, 1, 0, 0, 2`. Cycling against its three
    /// materials gave `0, 1, 2, 0, 1`, which drew the blood geoset with the
    /// empty-texture material and a body piece with `Textures\gutz.blp` —
    /// across the shipped set, 521 models and 3,121 geosets drawn wrong.
    #[test]
    fn a_geoset_draws_with_the_material_its_own_trailer_names() {
        let Some(d) = shipped(FOOTMAN) else { return };
        let model = load_model(&d, FOOTMAN).unwrap_or_else(|e| panic!("{FOOTMAN}: {e}"));
        let paths = model_texture_paths(&d, FOOTMAN).unwrap_or_else(|e| panic!("{FOOTMAN}: {e}"));
        assert_eq!(paths.len(), 3, "the footman ships three materials");

        let ids: Vec<u32> = model.meshes.iter().map(|m| m.material_id).collect();
        assert_eq!(ids, [0, 1, 0, 0, 2], "materialId is read, not cycled");

        // The measured bound, checked per geoset rather than as a property of
        // the model: every geoset's materialId addresses one of the model's
        // own materials. This replaces the old assertion that pinned the geoset
        // and material counts *unequal*, which only ever guarded the shape of
        // the mismatch and never the mapping itself.
        for m in &model.meshes {
            assert!(
                (m.material_id as usize) < paths.len(),
                "geoset {} names material {} of {}",
                m.geoset,
                m.material_id,
                paths.len()
            );
        }

        let parts = to_bevy(&d, FOOTMAN).unwrap_or_else(|e| panic!("{FOOTMAN}: {e}"));
        let bound: Vec<&str> = parts.iter().map(|(_, p)| p.as_str()).collect();
        assert_eq!(
            bound,
            [
                "Textures\\Footman.blp",
                "",
                "Textures\\Footman.blp",
                "Textures\\Footman.blp",
                "Textures\\gutz.blp",
            ],
            "geoset 4 is the blood geoset and must draw with gutz.blp"
        );
    }
}

#[cfg(test)]
mod sparse_id_tests {
    use super::*;

    /// A geoset's `MATS` entries are **node ids**, and node ids are sparse once a
    /// model has helpers. `skin_mesh` used to index the matrix list with the id,
    /// so a group pointing at node 9 in a two-bone model read a matrix that did
    /// not exist (falling back to rest) or, worse, the wrong bone's.
    #[test]
    fn a_geoset_matrix_is_found_by_node_id_not_array_position() {
        let mesh = LoadedMesh {
            positions: vec![[0.0, 0.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]],
            tex_coords: vec![[0.0, 0.0]],
            indices: vec![0],
            vertex_groups: vec![0],
            // The geoset's one group is carried by node **9**.
            bone_indices: vec![9],
            group_counts: vec![1],
            geoset: 0,
            material_id: 0,
        };
        // Two bones, at node ids 5 and 9 — neither is its own array position.
        let translate = |dx: f32| {
            let mut m = pcx_mdl::identity();
            m[12] = dx;
            m
        };
        let skeleton = Skeleton {
            matrices: vec![translate(100.0), translate(7.0)],
            ids: vec![5, 9],
            names: vec!["five".into(), "nine".into()],
        };
        let out = skin_mesh(&mesh, &skeleton);
        assert_eq!(out.len(), 1);
        // Node 9 is the second bone, so the vertex moves by 7 — not by 100
        // (index 0, which an id-as-index lookup could reach) and not by 0 (the
        // rest fallback when the id looked out of bounds).
        assert!(
            (out[0][0] - 7.0).abs() < 1e-6,
            "expected node 9's matrix, got x={}",
            out[0][0]
        );
    }
}
