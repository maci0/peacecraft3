//! MDX parse, material/geoset/animation decode, and skeleton pose sampling.
use std::collections::HashMap;

use pcx_mdl::{Bone, Mdlx, Pivots, Sequence, tag};

use crate::{
    Context, Failure, Outcome, Prepared, Workload,
    digest::Fnv,
    input::{self, extract_all},
};

pub const WORKLOADS: [Workload; 2] = [
    Workload {
        name: "mdx-parse-v1",
        summary: "parse and decode eight fixed models (geosets, materials, bones, effects)",
        prepare: parse,
    },
    Workload {
        name: "mdx-sample-v1",
        summary: "sample poses and bone matrices over every sequence of the same models",
        prepare: sample,
    },
];

/// Footman, great hall, four heroes, the main menu scene and a campaign background.
const MODELS: [&str; 8] = [
    "Units\\Human\\Footman\\Footman.mdx",
    "Buildings\\Orc\\GreatHall\\GreatHall.mdx",
    "Units\\Human\\HeroPaladin\\HeroPaladin.mdx",
    "Units\\Orc\\HeroBladeMaster\\HeroBladeMaster.mdx",
    "Units\\NightElf\\HeroDemonHunter\\HeroDemonHunter.mdx",
    "Units\\Undead\\HeroDeathKnight\\HeroDeathKnight.mdx",
    "UI\\Glues\\MainMenu\\MainMenu3d\\MainMenu3d.mdx",
    "UI\\Glues\\Loading\\Backgrounds\\Campaigns\\LordaeronBackground.mdx",
];

fn models(ctx: &Context, input_digest: &mut Fnv) -> Result<Vec<(String, Vec<u8>)>, Failure> {
    let mut set = input::open_set_with_codec(&ctx.install)?;
    extract_all(&mut set, &MODELS, input_digest)
}

/// Everything the importers and the unit-model reader decode from one model.
struct Decoded {
    model: pcx_mdl::Model,
    bones: Vec<Bone>,
    helpers: Vec<pcx_mdl::Node>,
    pivots: Pivots,
    sequences: Vec<Sequence>,
    meshes: Vec<pcx_mdl::Mesh>,
    geoset_anims: Vec<pcx_mdl::GeosetAnim>,
    materials: Vec<pcx_mdl::Material>,
    layers: Vec<Vec<pcx_mdl::MaterialLayer>>,
    textures: Vec<pcx_mdl::Texture>,
    texture_anims: Vec<pcx_mdl::TextureAnim>,
    globals: Vec<u32>,
    events: Vec<pcx_mdl::EventNode>,
    collision: Vec<pcx_mdl::CollisionShape>,
    ribbons: Vec<pcx_mdl::Ribbon>,
    particles: Vec<pcx_mdl::Particle>,
    particles2: Vec<pcx_mdl::Particle2>,
    lights: Vec<pcx_mdl::Light>,
    cameras: Vec<pcx_mdl::Camera>,
}

impl Decoded {
    /// Every decoded field, through its `Debug` rendering.
    fn fold(&self, out: &mut Fnv) {
        out.debug(&self.model);
        out.debug(&self.bones);
        out.debug(&self.helpers);
        out.debug(&self.pivots);
        out.debug(&self.sequences);
        out.debug(&self.meshes);
        out.debug(&self.geoset_anims);
        out.debug(&self.materials);
        out.debug(&self.layers);
        out.debug(&self.textures);
        out.debug(&self.texture_anims);
        out.debug(&self.globals);
        out.debug(&self.events);
        out.debug(&self.collision);
        out.debug(&self.ribbons);
        out.debug(&self.particles);
        out.debug(&self.particles2);
        out.debug(&self.lights);
        out.debug(&self.cameras);
    }
}

fn decode(name: &str, bytes: &[u8]) -> Result<Decoded, Failure> {
    let at = |what: &'static str| move |e: pcx_mdl::Error| format!("{name}: {what}: {e}");
    let mdl = Mdlx::parse(bytes).map_err(at("parse"))?;
    let mut meshes = Vec::new();
    for geoset in mdl.geosets(bytes).map_err(at("GEOS"))? {
        meshes.push(mdl.mesh(bytes, &geoset).map_err(at("mesh"))?);
    }
    let materials = mdl.materials(bytes).map_err(at("MTLS"))?;
    let mut layers = Vec::new();
    for material in &materials {
        layers.push(material.checked_layers().map_err(at("layers"))?);
    }
    let globals = mdl
        .payload(bytes, tag::GLBS)
        .unwrap_or(&[])
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect();
    Ok(Decoded {
        model: mdl.model(bytes).map_err(at("MODL"))?,
        bones: mdl.bones(bytes).map_err(at("BONE"))?,
        helpers: mdl.nodes(bytes, tag::HELP).map_err(at("HELP"))?,
        pivots: mdl.pivots(bytes).map_err(at("PIVT"))?,
        sequences: mdl.sequences(bytes).map_err(at("SEQS"))?,
        meshes,
        geoset_anims: mdl.geoset_anims(bytes).map_err(at("GEOA"))?,
        materials,
        layers,
        textures: mdl.textures(bytes).map_err(at("TEXS"))?,
        texture_anims: mdl.texture_anims(bytes).map_err(at("TXAN"))?,
        globals,
        events: mdl.event_nodes(bytes, tag::EVTS).map_err(at("EVTS"))?,
        collision: mdl.collision_shapes(bytes).map_err(at("CLID"))?,
        ribbons: mdl.ribbons(bytes).map_err(at("RIBB"))?,
        particles: mdl.particles(bytes).map_err(at("PREM"))?,
        particles2: mdl.particles2(bytes).map_err(at("PRE2"))?,
        lights: mdl.lights(bytes).map_err(at("LITE"))?,
        cameras: mdl.cameras(bytes).map_err(at("CAMS"))?,
    })
}

fn parse(ctx: &Context) -> Result<Prepared, Failure> {
    let mut input_digest = Fnv::default();
    let files = models(ctx, &mut input_digest)?;
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut all = Vec::new();
            for (name, bytes) in &files {
                all.push(decode(name, bytes)?);
            }
            let vertices: u64 = all
                .iter()
                .flat_map(|d| &d.meshes)
                .map(|m| u64::from(m.vertices.count))
                .sum();
            let bones: u64 = all.iter().map(|d| d.bones.len() as u64).sum();
            Ok(Outcome {
                work: vec![
                    ("models", all.len() as u64),
                    ("vertices", vertices),
                    ("bones", bones),
                ],
                fold: Box::new(move |out| all.iter().for_each(|d| d.fold(out))),
            })
        }),
    })
}

/// One model's sampling inputs, parsed outside the window.
struct Rig {
    joints: Vec<Bone>,
    pivots: Pivots,
    sequences: Vec<Sequence>,
    globals: Vec<u32>,
    parents: HashMap<u32, Option<u32>>,
}

/// Frame step in milliseconds between sampled poses.
const SAMPLE_STEP_MS: u32 = 33;

fn sample(ctx: &Context) -> Result<Prepared, Failure> {
    let mut input_digest = Fnv::default();
    let files = models(ctx, &mut input_digest)?;
    let mut rigs = Vec::new();
    for (name, bytes) in &files {
        let d = decode(name, bytes)?;
        let mut joints = d.bones;
        for helper in &d.helpers {
            joints.push(
                helper
                    .transform_bone()
                    .map_err(|e| format!("{name}: HELP {}: {e}", helper.object_id))?,
            );
        }
        let parents = joints
            .iter()
            .map(|j| (j.object_id, u32::try_from(j.parent_id).ok()))
            .collect();
        rigs.push(Rig {
            joints,
            pivots: d.pivots,
            sequences: d.sequences,
            globals: d.globals,
            parents,
        });
    }
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut matrices: Vec<Vec<[f32; 16]>> = Vec::new();
            let mut poses_sampled = 0u64;
            for rig in &rigs {
                let parent = |id: u32| rig.parents.get(&id).copied().flatten();
                for sequence in &rig.sequences {
                    let mut frame = sequence.interval_start;
                    while frame <= sequence.interval_end {
                        let interval = [sequence.interval_start, sequence.interval_end];
                        let poses: Vec<_> = rig
                            .joints
                            .iter()
                            .map(|joint| {
                                pcx_mdl::sample_joint_pose(
                                    joint,
                                    frame,
                                    frame,
                                    &rig.globals,
                                    interval,
                                )
                            })
                            .collect();
                        poses_sampled += poses.len() as u64;
                        matrices.push(pcx_mdl::bone_matrices_from_poses(
                            &rig.joints,
                            &parent,
                            &rig.pivots,
                            &poses,
                        ));
                        frame += SAMPLE_STEP_MS;
                    }
                }
            }
            Ok(Outcome {
                work: vec![("frames", matrices.len() as u64), ("poses", poses_sampled)],
                fold: Box::new(move |out| {
                    for frame in &matrices {
                        frame.iter().for_each(|m| out.floats(m));
                    }
                }),
            })
        }),
    })
}
