//! Pure PRE2 texture admission and quad construction. No GPU or scene writes.
use crate::mdx_particles::{self, Appearance, Particle, Plan, SORT_FAR_Z, UNFOGGED, UNSHADED};
use pcx_mdl::Texture;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Failure {
    #[error("malformed {0}")]
    Malformed(&'static str),
    #[error("unsupported {0}")]
    Unsupported(&'static str),
    #[error("missing {0}")]
    Missing(&'static str),
    #[error("conflicting {0}")]
    Conflict(&'static str),
    #[error("{0} policy exceeded")]
    Budget(&'static str),
    #[error("allocation failed")]
    Allocation,
    #[error("CPU appearance: {0}")]
    Appearance(mdx_particles::Failure),
    #[error("nonfinite/unrepresentable geometry")]
    Arithmetic,
}
#[derive(Debug, thiserror::Error)]
#[error("{source_name}: emitter {emitter:?}, TEXS {texture:?}: {failure}")]
pub struct Error {
    pub source_name: String,
    pub emitter: Option<u32>,
    pub texture: Option<u32>,
    pub failure: Failure,
}
fn error(source: &str, emitter: Option<u32>, texture: Option<u32>, failure: Failure) -> Error {
    Error {
        source_name: source.into(),
        emitter,
        texture,
        failure,
    }
}
fn canonical(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

/// All candidates must identify the caller's qualified source choice; no basename fallback.
#[derive(Clone, Copy)]
pub struct Decoded<'a> {
    pub source_context: &'a str,
    pub path: &'a str,
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}
#[derive(Debug, Clone, Copy)]
pub struct ImageLimits {
    pub max_requests: usize,
}
#[derive(Debug)]
pub struct ImageBinding {
    pub texture_id: u32,
    pub backing: usize,
}
#[derive(Debug)]
pub struct Backing {
    pub canonical_path: String,
    pub sampler_flags: u32,
}
#[derive(Debug)]
pub struct ImageGraph {
    source: String,
    bindings: Vec<ImageBinding>,
    backings: Vec<Backing>,
}
impl ImageGraph {
    /// Union existing geometry static/KMTF IDs with every checked PRE2 TEXS edge.
    /// Use the same TEXS snapshot supplied to CPU admission. No decoding or writes occur.
    pub fn checked(
        plan: &Plan,
        textures: &[Texture],
        geometry_ids: &[u32],
        limits: ImageLimits,
    ) -> Result<Self, Error> {
        let source = plan.source();
        let mut ids = Vec::new();
        let edge_count = geometry_ids
            .len()
            .checked_add(plan.profiles().len())
            .ok_or_else(|| error(source, None, None, Failure::Budget("request arithmetic")))?;
        ids.try_reserve_exact(edge_count)
            .map_err(|_| error(source, None, None, Failure::Allocation))?;
        ids.extend_from_slice(geometry_ids);
        ids.extend(plan.profiles().iter().map(|p| p.properties.texture_id));
        ids.sort_unstable();
        ids.dedup();
        if ids.len() > limits.max_requests {
            return Err(error(
                source,
                None,
                None,
                Failure::Budget("texture requests"),
            ));
        }
        let mut graph = Self {
            source: source.into(),
            bindings: Vec::new(),
            backings: Vec::new(),
        };
        graph
            .bindings
            .try_reserve_exact(ids.len())
            .map_err(|_| error(source, None, None, Failure::Allocation))?;
        graph
            .backings
            .try_reserve_exact(ids.len())
            .map_err(|_| error(source, None, None, Failure::Allocation))?;
        for id in ids {
            let emitter = plan
                .profiles()
                .iter()
                .find(|p| p.properties.texture_id == id)
                .map(|p| p.object_id);
            let fail = |failure| error(source, emitter, Some(id), failure);
            let texture = textures
                .get(id as usize)
                .ok_or_else(|| fail(Failure::Missing("TEXS record")))?;
            if texture.path.is_empty() || texture.slot != 0 {
                return Err(fail(Failure::Unsupported(
                    "empty path/nonzero TEXS leading word",
                )));
            }
            if texture.flags & !(pcx_mdl::TEXTURE_WRAP_WIDTH | pcx_mdl::TEXTURE_WRAP_HEIGHT) != 0 {
                return Err(fail(Failure::Unsupported("sampler bits")));
            }
            if plan
                .profiles()
                .iter()
                .any(|p| p.properties.texture_id == id && p.texture_flags != texture.flags)
            {
                return Err(fail(Failure::Conflict("CPU plan/TEXS sampler")));
            }
            let path = canonical(&texture.path);
            let backing = match graph
                .backings
                .iter()
                .position(|b| b.canonical_path == path && b.sampler_flags == texture.flags)
            {
                Some(index) => index,
                None => {
                    let index = graph.backings.len();
                    graph.backings.push(Backing {
                        canonical_path: path,
                        sampler_flags: texture.flags,
                    });
                    index
                }
            };
            graph.bindings.push(ImageBinding {
                texture_id: id,
                backing,
            });
        }
        Ok(graph)
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn bindings(&self) -> &[ImageBinding] {
        &self.bindings
    }
    pub fn backings(&self) -> &[Backing] {
        &self.backings
    }
    /// Validate the complete image batch before allocating GPU images or changing entities.
    /// Returned immutable borrows keep the accepted pixels/dimensions tied to the preflight.
    pub fn preflight<'a>(
        &self,
        decoded: &[Decoded<'a>],
        max_backing_rgba_bytes: usize,
    ) -> Result<PreparedImages<'a>, Error> {
        let mut prepared = PreparedImages {
            images: Vec::new(),
            rgba_bytes: 0,
        };
        prepared
            .images
            .try_reserve_exact(self.backings.len())
            .map_err(|_| error(&self.source, None, None, Failure::Allocation))?;
        for (backing_index, backing) in self.backings.iter().enumerate() {
            let texture = self
                .bindings
                .iter()
                .find(|b| b.backing == backing_index)
                .map(|b| b.texture_id);
            let fail = |failure| error(&self.source, None, texture, failure);
            let mut candidate: Option<usize> = None;
            for (index, image) in decoded
                .iter()
                .enumerate()
                .filter(|(_, image)| canonical(image.path) == backing.canonical_path)
            {
                if image.source_context != self.source {
                    return Err(fail(Failure::Conflict("decoded source context")));
                }
                let length = usize::try_from(image.width)
                    .ok()
                    .and_then(|w| usize::try_from(image.height).ok()?.checked_mul(w))
                    .and_then(|pixels| pixels.checked_mul(4));
                if image.width == 0 || image.height == 0 || length != Some(image.rgba.len()) {
                    return Err(fail(Failure::Malformed("decoded dimensions/RGBA length")));
                }
                if candidate.is_some_and(|old| {
                    let old = decoded[old];
                    old.width != image.width || old.height != image.height || old.rgba != image.rgba
                }) {
                    return Err(fail(Failure::Conflict("decoded canonical pixels")));
                }
                candidate = Some(index);
            }
            let candidate =
                candidate.ok_or_else(|| fail(Failure::Missing("decoded canonical image")))?;
            prepared.rgba_bytes = prepared
                .rgba_bytes
                .checked_add(decoded[candidate].rgba.len())
                .ok_or_else(|| fail(Failure::Budget("decoded backing bytes")))?;
            if prepared.rgba_bytes > max_backing_rgba_bytes {
                return Err(fail(Failure::Budget("decoded backing bytes")));
            }
            prepared.images.push(decoded[candidate]);
        }
        Ok(prepared)
    }
    pub fn backing_for(&self, id: u32) -> Option<usize> {
        self.bindings
            .binary_search_by_key(&id, |b| b.texture_id)
            .ok()
            .map(|i| self.bindings[i].backing)
    }
}

/// Images indexed by ImageGraph backing; no source pixels are rewritten.
pub struct PreparedImages<'a> {
    pub images: Vec<Decoded<'a>>,
    pub rgba_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    SourceAlpha,
    Additive,
}
#[derive(Debug, Clone, Copy)]
pub struct DrawRequirements {
    pub emitter: u32,
    pub texture: u32,
    pub flags: u32,
    pub priority_plane: i32,
    pub sort_far_z: bool,
    pub blend: Blend,
    pub filter_mode: u32,
    pub lighting_required: bool,
    pub fog_required: bool,
}
pub fn draw_requirements(plan: &Plan, index: usize) -> Result<DrawRequirements, Error> {
    let profile = plan.profiles().get(index).ok_or_else(|| {
        error(
            plan.source(),
            None,
            None,
            Failure::Malformed("emitter ordinal"),
        )
    })?;
    let fail = |failure| {
        error(
            plan.source(),
            Some(profile.object_id),
            Some(profile.properties.texture_id),
            failure,
        )
    };
    let p = profile.properties;
    let blend = match p.filter_mode {
        0 => Blend::SourceAlpha,
        1 => Blend::Additive,
        _ => return Err(fail(Failure::Unsupported("filter mode"))),
    };
    Ok(DrawRequirements {
        emitter: profile.object_id,
        texture: p.texture_id,
        flags: profile.flags,
        priority_plane: p.priority_plane,
        sort_far_z: profile.flags & SORT_FAR_Z != 0,
        blend,
        filter_mode: p.filter_mode,
        lighting_required: profile.flags & UNSHADED == 0,
        fog_required: profile.flags & UNFOGGED == 0,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct CameraBasis {
    right: [f64; 3],
    up: [f64; 3],
    toward_eye: [f64; 3],
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
impl CameraBasis {
    /// Camera-to-source vectors; no projection, scale fitting or coordinate conversion.
    pub fn checked(right: [f32; 3], up: [f32; 3], toward_eye: [f32; 3]) -> Result<Self, Failure> {
        let (right, up, toward_eye) = (
            right.map(f64::from),
            up.map(f64::from),
            toward_eye.map(f64::from),
        );
        // Numerical acceptance policy for a camera rotation, not an asset measurement.
        if !right
            .into_iter()
            .chain(up)
            .chain(toward_eye)
            .all(f64::is_finite)
            || [right, up, toward_eye]
                .into_iter()
                .any(|v| (dot(v, v) - 1.).abs() > 1e-5)
            || dot(right, up).abs() > 1e-5
            || dot(cross(right, up), toward_eye) < 1. - 1e-5
        {
            return Err(Failure::Malformed("orthonormal camera-to-source basis"));
        }
        Ok(Self {
            right,
            up,
            toward_eye,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece {
    Head,
    Tail,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub piece: Piece,
    pub positions: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub rgba: [f32; 4],
}
impl Quad {
    pub const INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];
}
fn checked_vector(v: [f64; 3]) -> Result<[f32; 3], Failure> {
    if v.into_iter()
        .any(|v| !v.is_finite() || v.abs() > f64::from(f32::MAX))
    {
        return Err(Failure::Arithmetic);
    }
    Ok(v.map(|v| v as f32))
}

/// Source-space quads using the retained shader UV/tail equations.
/// Head corner-to-camera mapping is an explicit reconstruction convention.
/// The draw requirement check and app's single source-to-scene conversion are separate.
pub fn quads(
    plan: &Plan,
    particle: &Particle,
    camera: CameraBasis,
) -> Result<[Option<Quad>; 2], Error> {
    let appearance = plan.appearance(particle).map_err(|e| {
        error(
            &e.source_name,
            e.emitter,
            None,
            Failure::Appearance(e.failure),
        )
    })?;
    let fail = |failure| {
        error(
            plan.source(),
            plan.profiles().get(particle.emitter).map(|p| p.object_id),
            Some(appearance.texture_id),
            failure,
        )
    };
    build_quads(particle, &appearance, camera).map_err(fail)
}
fn build_quads(
    particle: &Particle,
    a: &Appearance,
    camera: CameraBasis,
) -> Result<[Option<Quad>; 2], Failure> {
    let mut pieces = [None, None];
    if let Some(uv) = a.cell_corners(false) {
        let mut positions = [[0.; 3]; 4];
        for (out, [x, y]) in positions
            .iter_mut()
            .zip([[-1., -1.], [-1., 1.], [1., 1.], [1., -1.]])
        {
            let offset = if let Some(angle) = a.xy_facing {
                let (sin, cos) = f64::from(angle).sin_cos();
                [x * cos - y * sin, x * sin + y * cos, 0.]
            } else {
                std::array::from_fn(|i| x * camera.right[i] + y * camera.up[i])
            };
            *out = checked_vector(std::array::from_fn(|i| {
                f64::from(particle.position[i]) + offset[i] * f64::from(a.scale)
            }))?;
        }
        pieces[0] = Some(Quad {
            piece: Piece::Head,
            positions,
            uv,
            rgba: a.rgba,
        });
    }
    if let Some(uv) = a.cell_corners(true) {
        let start = a.tail_start.map(f64::from);
        let end = particle.position.map(f64::from);
        let direction = std::array::from_fn(|i| end[i] - start[i]);
        let length = dot(direction, direction).sqrt();
        // A degenerate/eye-parallel tail collapses naturally. No invented perpendicular.
        let normal = if length == 0. {
            [0.; 3]
        } else {
            cross(camera.toward_eye, direction.map(|v| v / length))
        };
        // Flow does not normalize this cross product: view-parallel tails foreshorten.
        let boundary = normal.map(|v| v * f64::from(a.scale));
        let positions = [
            checked_vector(std::array::from_fn(|i| start[i] - boundary[i]))?,
            checked_vector(std::array::from_fn(|i| end[i] - boundary[i]))?,
            checked_vector(std::array::from_fn(|i| end[i] + boundary[i]))?,
            checked_vector(std::array::from_fn(|i| start[i] + boundary[i]))?,
        ];
        pieces[1] = Some(Quad {
            piece: Piece::Tail,
            positions,
            uv,
            rgba: a.rgba,
        });
    }
    Ok(pieces)
}
