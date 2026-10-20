//! Complete background PRE2 binding; source inputs share the geometry transaction.
use super::{MainMenuSceneEntity, TextureBindings};
use crate::menu_particle_order::{OrderFailure, SourceKind, SourceOrder, SourceOrderView};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    math::DVec3,
    mesh::MeshVertexBufferLayoutRef,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    render::{
        batching::NoAutomaticBatching,
        sync_world::{MainEntity, MainEntityHashMap},
    },
    shader::ShaderRef,
};
use pcx_gfx::{
    mdx_particle_render::{
        self as draw, DrawRequirements, ImageGraph, ImageLimits, PreparedImages,
    },
    mdx_particles::{self as cpu, Clip, Limits, Plan, Simulation},
    mdx_runtime::RuntimePlan,
};
use std::{sync::Arc, time::Duration};

/// Particle-only forward fragment; geometry has its own menu material.
pub type ParticleMaterial = ExtendedMaterial<StandardMaterial, ParticleFog>;

/// Black fog is a reconstruction convention for additive light contribution.
/// Camera falloff/opacity and the source material fog flag remain authoritative.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct ParticleFog {}
impl MaterialExtension for ParticleFog {
    fn fragment_shader() -> ShaderRef {
        "embedded://peacecraft3/menu_particle.wesl".into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = &mut descriptor.fragment {
            fragment.shader_defs.push("MENU_PARTICLE_FOG".into());
        }
        Ok(())
    }
}

pub struct ParticleAssets<'a> {
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<ParticleMaterial>,
    pub images: &'a mut Assets<Image>,
}

pub struct ParticleMaterialPlugin;
impl Plugin for ParticleMaterialPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "menu_particle.wesl");
        app.add_plugins(MaterialPlugin::<ParticleMaterial>::default());
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Cpu(#[from] cpu::Error),
    #[error(transparent)]
    Draw(#[from] draw::Error),
    #[error("source decode: {0}")]
    Decode(String),
    #[error("submission profile: {0:?}")]
    Order(OrderFailure),
    #[error("{0}")]
    Invalid(&'static str),
}

/// Engine work/memory policies and deterministic seed; none is measured asset content.
#[derive(Clone, Copy)]
struct Policy {
    cpu: Limits,
    seed: u64,
    max_items: usize,
    max_image_bytes: usize,
    max_upload_bytes: usize,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            cpu: Limits {
                max_emitters: 1024,
                max_keys: 1_000_000,
                particle_capacity: 16_384,
                max_ticks: 240,
                max_updates: 4_000_000,
                tick: Duration::from_nanos(8_333_333),
            },
            seed: 1,
            max_items: 4096,
            max_image_bytes: super::MENU_IMAGE_BYTES,
            max_upload_bytes: 32 * 1024 * 1024,
        }
    }
}

pub enum Admission<'a> {
    Absent,
    Unsupported(Error),
    Ready(Box<Prepared<'a>>),
}

pub struct Prepared<'a> {
    simulation: Simulation,
    graph: ImageGraph,
    images: PreparedImages<'a>,
    requirements: Vec<DrawRequirements>,
    emitters: Vec<Emitter>,
    orders: MainEntityHashMap<SourceOrder>,
    source: Arc<str>,
    policy: Policy,
}

pub fn prepare<'a>(
    source: &'a str,
    model: &pcx_mdl::Mdlx,
    bytes: &[u8],
    runtime: &RuntimePlan,
    textures: &'a [pcx_gfx::LoadedTexture],
    clip: Option<Clip>,
    native_parameters: Option<&pcx_play::menu_lighting::Parameters>,
) -> Result<Admission<'a>, Error> {
    let emitters = model
        .particles2(bytes)
        .map_err(|e| Error::Decode(e.to_string()))?;
    if emitters.is_empty() {
        return Ok(Admission::Absent);
    }
    let policy = Policy::default();
    let source_textures = model
        .textures(bytes)
        .map_err(|e| Error::Decode(e.to_string()))?;
    let pivots = model
        .pivots(bytes)
        .map_err(|e| Error::Decode(e.to_string()))?;
    let plan = match Plan::checked(
        &emitters,
        &source_textures,
        &pivots,
        &runtime.global_sequences,
        clip.ok_or(Error::Invalid("PRE2 requires an authored selected clip"))?,
        source,
        policy.cpu,
    ) {
        Ok(plan) => plan,
        Err(error) if matches!(error.failure, cpu::Failure::Unsupported(_)) => {
            return Ok(Admission::Unsupported(Error::Cpu(error)));
        }
        Err(error) => return Err(Error::Cpu(error)),
    };
    // Import validation is data-only; the live scene supplies its qualified client parameters.
    let plan = if let Some(parameters) = native_parameters {
        let appearance = parameters.particle_appearance;
        match plan.with_native_appearance(
            &format!(
                "{} {} {} {:?}",
                parameters.origin.source,
                parameters.origin.edition,
                parameters.origin.version,
                parameters.image_sha256
            ),
            [appearance.phase_scale, appearance.phase_bias],
            [appearance.colour_scale, appearance.packing_bias],
            appearance.packing_shift,
        ) {
            Ok(plan) => plan,
            Err(error) if matches!(error.failure, cpu::Failure::Unsupported(_)) => {
                return Ok(Admission::Unsupported(Error::Cpu(error)));
            }
            Err(error) => return Err(Error::Cpu(error)),
        }
    } else {
        plan
    };
    let plan = Arc::new(plan);
    let mut geometry_ids = Vec::new();
    let item_count = runtime
        .geosets
        .len()
        .checked_add(plan.profiles().len())
        .filter(|count| *count <= policy.max_items)
        .ok_or(Error::Invalid("scene submission item budget exceeded"))?;
    for geo in &runtime.geosets {
        let material = runtime
            .materials
            .get(geo.material_id)
            .ok_or(Error::Invalid(
                "GEOS has no checked MTLS for particle composition",
            ))?;
        let layer = material.layers.first().ok_or(Error::Invalid(
            "GEOS has no checked first layer for particle composition",
        ))?;
        geometry_ids.push(layer.properties.texture_id);
        geometry_ids.extend(
            layer
                .texture_tracks
                .iter()
                .flat_map(|track| track.keys.iter().map(|&(_, id)| id)),
        );
    }
    let mut requirements = Vec::new();
    requirements
        .try_reserve_exact(plan.profiles().len())
        .map_err(|_| Error::Invalid("draw allocation failed"))?;
    for index in 0..plan.profiles().len() {
        let requirement = draw::draw_requirements(&plan, index)?;
        if requirement.sort_far_z {
            return Ok(Admission::Unsupported(Error::Order(
                OrderFailure::SortFarZNeedsParticleSort,
            )));
        }
        requirements.push(requirement);
    }
    // Opaque geometry precedes the translucent priority sort (pinned setupgroups.ts).
    let graph = ImageGraph::checked(
        &plan,
        &source_textures,
        &geometry_ids,
        ImageLimits {
            max_requests: policy.max_items,
        },
    )?;
    // This immutable caller batch and both plans belong to the same selected model transaction.
    // Its context names that transaction; it does not claim an independently resolved archive identity.
    let decoded = textures
        .iter()
        .map(|image| draw::Decoded {
            source_context: source,
            path: &image.source,
            width: image.width,
            height: image.height,
            rgba: &image.rgba,
            mipmaps: &image.mipmaps,
        })
        .collect::<Vec<_>>();
    let images = graph.preflight(&decoded, policy.max_image_bytes)?;
    let simulation = Simulation::new(plan, policy.seed, policy.cpu)?;
    let mut live_emitters = Vec::new();
    live_emitters
        .try_reserve_exact(requirements.len())
        .map_err(|_| Error::Invalid("emitter allocation failed"))?;
    let mut orders = MainEntityHashMap::default();
    orders
        .try_reserve(item_count)
        .map_err(|_| Error::Invalid("order allocation failed"))?;
    Ok(Admission::Ready(Box::new(Prepared {
        simulation,
        graph,
        images,
        requirements,
        emitters: live_emitters,
        orders,
        source: Arc::from(source),
        policy,
    })))
}

struct Emitter {
    mesh: Handle<Mesh>,
    material: Handle<ParticleMaterial>,
    entity: Entity,
}

pub struct ParticleScene {
    simulation: Simulation,
    emitters: Vec<Emitter>,
    camera: Entity,
    enabled: bool,
    policy: Policy,
}

impl Prepared<'_> {
    pub fn count(&self) -> usize {
        self.requirements.len()
    }

    /// Admission has completed; all new images join the geometry's existing sampler cache.
    pub fn spawn(
        mut self,
        commands: &mut Commands,
        assets: ParticleAssets,
        bindings: &mut TextureBindings,
        camera: Entity,
        geometry: &[super::BoundGeoset],
        runtime: &RuntimePlan,
    ) -> (ParticleScene, SourceOrderView) {
        let owner = MainEntity::from(camera);
        for (backing, image) in self.images.images.iter().enumerate() {
            let existing = bindings.images.iter().find_map(|(id, handle)| {
                (self.graph.backing_for(*id) == Some(backing)).then(|| handle.clone())
            });
            let handle = existing.unwrap_or_else(|| {
                use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
                let mut gpu = Image::new(
                    Extent3d {
                        width: image.width,
                        height: image.height,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    image.rgba.to_vec(),
                    TextureFormat::Rgba8Unorm,
                    RenderAssetUsages::default(),
                );
                let data = gpu.data.as_mut().expect("RGBA image has CPU data");
                for mip in image.mipmaps {
                    data.extend_from_slice(mip);
                }
                gpu.texture_descriptor.mip_level_count = 1 + image.mipmaps.len() as u32;
                gpu.sampler = pcx_gfx::viewer::sampler_for_texture_flags(
                    self.graph.backings()[backing].sampler_flags,
                );
                assets.images.add(gpu)
            });
            for binding in self
                .graph
                .bindings()
                .iter()
                .filter(|binding| binding.backing == backing)
            {
                bindings.images.insert(binding.texture_id, handle.clone());
            }
        }
        for (geoset, entity, _) in geometry {
            let material = runtime.geosets[*geoset].material_id;
            self.orders.insert(
                (*entity).into(),
                SourceOrder {
                    owner,
                    priority_plane: runtime.materials[material].priority_plane,
                    kind: SourceKind::Geometry,
                    ordinal: *geoset as u32,
                    sort_far_z: false,
                },
            );
        }
        for (ordinal, requirement) in self.requirements.into_iter().enumerate() {
            let material = assets.materials.add(particle_material(
                requirement,
                bindings.images[&requirement.texture].clone(),
            ));
            let mesh = assets.meshes.add(empty_mesh());
            let entity = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Visibility::Hidden,
                    MainMenuSceneEntity,
                    NoAutomaticBatching,
                    // Dynamic cloud vertices must not retain a stale first-frame AABB.
                    NoFrustumCulling,
                ))
                .id();
            self.orders.insert(
                entity.into(),
                SourceOrder {
                    owner,
                    priority_plane: requirement.priority_plane,
                    kind: SourceKind::Particle,
                    ordinal: ordinal as u32,
                    sort_far_z: requirement.sort_far_z,
                },
            );
            self.emitters.push(Emitter {
                mesh,
                material,
                entity,
            });
        }
        (
            ParticleScene {
                simulation: self.simulation,
                emitters: self.emitters,
                camera,
                enabled: true,
                policy: self.policy,
            },
            SourceOrderView {
                owners: Arc::from([owner]),
                source: self.source,
                orders: Arc::new(self.orders),
                max_phase_items: self.policy.max_items,
            },
        )
    }
}

fn particle_material(source: DrawRequirements, image: Handle<Image>) -> ParticleMaterial {
    ParticleMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(image),
            unlit: !source.lighting_required,
            fog_enabled: source.fog_required,
            alpha_mode: match source.blend {
                draw::Blend::SourceAlpha => AlphaMode::Blend,
                draw::Blend::Additive => AlphaMode::Add,
            },
            cull_mode: None,
            double_sided: true,
            perceptual_roughness: 1.0,
            ..default()
        },
        extension: ParticleFog::default(),
    }
}
// Visibility does not suppress asset extraction; empty emitters stay CPU-owned.
fn empty_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

#[derive(Default)]
struct Buffer {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    rgba: Vec<[f32; 4]>,
    indices: Vec<u32>,
}
impl Buffer {
    fn reserve(&mut self, quads: usize) -> Result<(), Error> {
        let vertices = quads
            .checked_mul(4)
            .ok_or(Error::Invalid("vertex count overflow"))?;
        let indices = quads
            .checked_mul(6)
            .ok_or(Error::Invalid("index count overflow"))?;
        if vertices > u32::MAX as usize {
            return Err(Error::Invalid("u32 vertex index exceeded"));
        }
        macro_rules! reserve {
            ($field:ident, $n:expr) => {
                self.$field
                    .try_reserve_exact($n)
                    .map_err(|_| Error::Invalid("mesh allocation failed"))?;
            };
        }
        reserve!(positions, vertices);
        reserve!(normals, vertices);
        reserve!(uv, vertices);
        reserve!(rgba, vertices);
        reserve!(indices, indices);
        Ok(())
    }
    fn push(&mut self, quad: draw::Quad) -> Result<(), Error> {
        let positions = quad.positions.map(|p| DVec3::from_array(p.map(f64::from)));
        let raw = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
        if !raw.is_finite() {
            return Err(Error::Invalid("nonfinite particle normal"));
        }
        // Degenerate source pieces have no fragments; zero is inert there.
        let normal = raw
            .try_normalize()
            .unwrap_or(DVec3::ZERO)
            .as_vec3()
            .to_array();
        let base = self.positions.len() as u32;
        self.positions.extend(quad.positions);
        self.normals.extend([normal; 4]);
        self.uv.extend(quad.uv);
        self.rgba.extend([quad.rgba; 4]);
        self.indices
            .extend(draw::Quad::INDICES.map(|index| index + base));
        Ok(())
    }
    fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            if self.positions.is_empty() {
                RenderAssetUsages::MAIN_WORLD
            } else {
                RenderAssetUsages::default()
            },
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uv);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.rgba);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

impl ParticleScene {
    pub fn camera(&self) -> Entity {
        self.camera
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn advance(
        &mut self,
        delta: Duration,
        elapsed: Duration,
        camera: &Transform,
        meshes: &mut Assets<Mesh>,
        materials: &Assets<ParticleMaterial>,
        mut entity_exists: impl FnMut(Entity) -> bool,
    ) -> Result<Vec<(Entity, bool)>, Error> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        if self.emitters.iter().any(|emitter| {
            !entity_exists(emitter.entity)
                || meshes.get(&emitter.mesh).is_none()
                || materials.get(&emitter.material).is_none()
        }) {
            return Err(Error::Invalid(
                "particle entity/mesh/material ownership was lost",
            ));
        }
        let expected = self
            .simulation
            .elapsed()
            .checked_add(self.simulation.pending())
            .and_then(|previous| previous.checked_add(delta))
            .ok_or(Error::Invalid("particle clock overflow"))?;
        if expected != elapsed {
            return Err(Error::Invalid("particle and scene clocks diverged"));
        }
        let basis = draw::CameraBasis::checked(
            (camera.rotation * Vec3::X).to_array(),
            (camera.rotation * Vec3::Y).to_array(),
            (camera.rotation * Vec3::Z).to_array(),
        )
        .map_err(|_| Error::Invalid("invalid source camera basis"))?;
        self.simulation.advance(delta, self.policy.cpu)?;
        let mut counts = vec![0usize; self.emitters.len()];
        let mut pieces = Vec::new();
        let max_pieces = self
            .simulation
            .particles()
            .len()
            .checked_mul(2)
            .ok_or(Error::Invalid("piece count overflow"))?;
        pieces
            .try_reserve_exact(max_pieces)
            .map_err(|_| Error::Invalid("piece allocation failed"))?;
        let pose_ms = u64::try_from(elapsed.as_millis())
            .map_err(|_| Error::Invalid("particle clock overflow"))?;
        for particle in self.simulation.particles() {
            for quad in draw::quads_at(self.simulation.plan(), particle, basis, pose_ms)?
                .into_iter()
                .flatten()
            {
                counts[particle.emitter] += 1;
                pieces.push((particle.emitter, quad));
            }
        }
        // Vertex position/normal/UV/RGBA and six u32 indices per quad.
        let bytes = pieces
            .len()
            .checked_mul(4 * (3 + 3 + 2 + 4) * 4 + 6 * 4)
            .ok_or(Error::Invalid("mesh upload size overflow"))?;
        if bytes > self.policy.max_upload_bytes {
            return Err(Error::Invalid("particle mesh upload budget exceeded"));
        }
        let mut buffers = Vec::new();
        buffers
            .try_reserve_exact(self.emitters.len())
            .map_err(|_| Error::Invalid("buffer allocation failed"))?;
        for count in counts {
            let mut buffer = Buffer::default();
            buffer.reserve(count)?;
            buffers.push(buffer);
        }
        for (emitter, quad) in pieces {
            buffers[emitter].push(quad)?;
        }
        let mut changes = Vec::new();
        changes
            .try_reserve_exact(self.emitters.len())
            .map_err(|_| Error::Invalid("visibility allocation failed"))?;
        for (emitter, buffer) in self.emitters.iter().zip(buffers) {
            let visible = !buffer.positions.is_empty();
            *meshes
                .get_mut(&emitter.mesh)
                .expect("all particle meshes preflighted") = buffer.into_mesh();
            changes.push((emitter.entity, visible));
        }
        Ok(changes)
    }
    pub fn disable(&mut self) -> impl Iterator<Item = Entity> + '_ {
        self.enabled = false;
        self.emitters.iter().map(|emitter| emitter.entity)
    }
    pub fn source(&self) -> &str {
        self.simulation.plan().source()
    }
}

#[cfg(test)]
#[path = "menu_particle_mesh_tests.rs"]
mod mesh_tests;
