//! Checked MDX runtime data. Drawing and native animation clocks are separate.
use pcx_mdl::{
    Bone, BoneTrack, GeosetAnim, Material, MaterialLayer, Mdlx, NO_ID, Node, Pivots, Texture,
    TextureAnim, TrackKind,
};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("MDX decode: {0}")]
    Decode(#[from] pcx_mdl::Error),
    #[error("duplicate joint object id {0}")]
    DuplicateJoint(u32),
    #[error("joint {joint} has invalid parent {parent}")]
    InvalidParent { joint: u32, parent: i32 },
    #[error("joint {joint} refers to missing BONE/HELP parent {parent}")]
    MissingParent { joint: u32, parent: u32 },
    #[error("joint hierarchy cycle includes object id {0}")]
    Cycle(u32),
    #[error("joint {0} has no authored pivot")]
    MissingPivot(u32),
    #[error("joint {0} has a nonfinite pivot")]
    NonFinitePivot(u32),
    #[error("joint {joint} has malformed transform track {track}")]
    TrackShape { joint: u32, track: usize },
    #[error("helper {joint} has unsupported transform {magic:?}")]
    HelperTrack { joint: u32, magic: [u8; 4] },
    #[error("{context} references {id}, but {count} entries exist")]
    Reference {
        context: String,
        id: u32,
        count: usize,
    },
    #[error("geoset {geoset} refers to missing joint {joint}")]
    MissingMatrixJoint { geoset: usize, joint: u32 },
    #[error("GLBS payload has {0} bytes, not whole words")]
    GlobalSequenceShape(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointKind {
    Bone,
    Helper,
}

/// All BONE records followed by all HELP records. Matrices use this same order.
/// The synthetic Bone representation for HELP is solely a decoder sampler adapter.
#[derive(Debug, Clone)]
pub struct JointPlan {
    joints: Vec<Bone>,
    kinds: Vec<JointKind>,
    pivots: Pivots,
    by_id: BTreeMap<u32, usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JointMatrices {
    pub object_ids: Vec<u32>,
    pub kinds: Vec<JointKind>,
    pub matrices: Vec<[f32; 16]>,
}

impl JointPlan {
    /// Decode and validate the full authored transform hierarchy once at import.
    pub fn parse(bytes: &[u8]) -> Result<Self, PlanError> {
        let model = Mdlx::parse(bytes)?;
        Self::checked(
            &model.bones(bytes)?,
            &model.nodes(bytes, pcx_mdl::tag::HELP)?,
            &model.pivots(bytes)?,
        )
    }

    pub fn checked(bones: &[Bone], helpers: &[Node], pivots: &Pivots) -> Result<Self, PlanError> {
        let mut joints = bones.to_vec();
        let mut kinds = vec![JointKind::Bone; bones.len()];
        for helper in helpers {
            let encoded_len = helper
                .tracks
                .iter()
                .try_fold(0usize, |size, track| {
                    size.checked_add(16)?.checked_add(track.raw.len())
                })
                .ok_or(PlanError::TrackShape {
                    joint: helper.object_id,
                    track: 0,
                })?;
            let header_len =
                (helper.size as usize)
                    .checked_sub(encoded_len)
                    .ok_or(PlanError::TrackShape {
                        joint: helper.object_id,
                        track: 0,
                    })?;
            let mut offset =
                helper
                    .offset
                    .checked_add(header_len)
                    .ok_or(PlanError::TrackShape {
                        joint: helper.object_id,
                        track: 0,
                    })?;
            let mut tracks = Vec::new();
            for track in &helper.tracks {
                let (kind, width) = match &track.magic {
                    b"KGTR" => (TrackKind::Translation, 3),
                    b"KGRT" => (TrackKind::Rotation, 4),
                    b"KGSC" => (TrackKind::Scaling, 3),
                    _ => {
                        return Err(PlanError::HelperTrack {
                            joint: helper.object_id,
                            magic: track.magic,
                        });
                    }
                };
                if track.width != width {
                    return Err(PlanError::TrackShape {
                        joint: helper.object_id,
                        track: tracks.len(),
                    });
                }
                tracks.push(BoneTrack {
                    kind,
                    count: track.count,
                    interpolation: track.interpolation,
                    global_sequence_id: i32::from_le_bytes(track.global_seq_id.to_le_bytes()),
                    offset,
                    stride: track.stride,
                    raw: track.raw.clone(),
                });
                offset = offset
                    .checked_add(16)
                    .and_then(|offset| offset.checked_add(track.raw.len()))
                    .ok_or(PlanError::TrackShape {
                        joint: helper.object_id,
                        track: tracks.len() - 1,
                    })?;
            }
            joints.push(Bone {
                offset: helper.offset,
                span: helper.size as usize,
                name: helper.name.clone(),
                object_id: helper.object_id,
                parent_id: helper.parent_id,
                flags: helper.flags,
                tracks,
                geoset_id: -1,
                geoset_anim_id: -1,
            });
            kinds.push(JointKind::Helper);
        }
        let mut by_id = BTreeMap::new();
        for (index, joint) in joints.iter().enumerate() {
            if by_id.insert(joint.object_id, index).is_some() {
                return Err(PlanError::DuplicateJoint(joint.object_id));
            }
            let pivot = pivots
                .position(joint.object_id)
                .ok_or(PlanError::MissingPivot(joint.object_id))?;
            if pivot.iter().any(|value| !value.is_finite()) {
                return Err(PlanError::NonFinitePivot(joint.object_id));
            }
            for (index, track) in joint.tracks.iter().enumerate() {
                let width = if track.kind == TrackKind::Rotation {
                    4
                } else {
                    3
                };
                let stride = 4 + width
                    * 4
                    * if track.interpolation.has_tangents() {
                        3
                    } else {
                        1
                    };
                if track.stride != stride
                    || (track.count as usize).checked_mul(stride) != Some(track.raw.len())
                    || !track.times_are_ordered()
                    || track.raw.chunks_exact(stride).any(|key| {
                        key[4..]
                            .chunks_exact(4)
                            .any(|value| !f32::from_le_bytes(value.try_into().unwrap()).is_finite())
                    })
                {
                    return Err(PlanError::TrackShape {
                        joint: joint.object_id,
                        track: index,
                    });
                }
            }
        }
        for joint in &joints {
            if joint.parent_id < -1 {
                return Err(PlanError::InvalidParent {
                    joint: joint.object_id,
                    parent: joint.parent_id,
                });
            }
            if joint.parent_id >= 0 && !by_id.contains_key(&(joint.parent_id as u32)) {
                return Err(PlanError::MissingParent {
                    joint: joint.object_id,
                    parent: joint.parent_id as u32,
                });
            }
        }
        // Three-state iterative walk: no recursion or identity fallback on cycles.
        let mut state = vec![0u8; joints.len()];
        for start in 0..joints.len() {
            let mut chain = Vec::new();
            let mut current = Some(start);
            while let Some(index) = current {
                match state[index] {
                    2 => break,
                    1 => return Err(PlanError::Cycle(joints[index].object_id)),
                    _ => {}
                }
                state[index] = 1;
                chain.push(index);
                current = (joints[index].parent_id >= 0)
                    .then(|| by_id[&(joints[index].parent_id as u32)]);
            }
            for index in chain {
                state[index] = 2;
            }
        }
        Ok(Self {
            joints,
            kinds,
            pivots: pivots.clone(),
            by_id,
        })
    }

    pub fn joints(&self) -> &[Bone] {
        &self.joints
    }

    pub fn pivots(&self) -> &Pivots {
        &self.pivots
    }
    pub fn kinds(&self) -> &[JointKind] {
        &self.kinds
    }
    pub fn index_of(&self, object_id: u32) -> Option<usize> {
        self.by_id.get(&object_id).copied()
    }

    /// Uses the decoder's existing frame sampling/composition convention.
    /// Global sequence clocks, inheritance flags and native quaternion behavior
    /// are retained as data, not claimed implemented by this operation.
    pub fn sample_decoder_frame(&self, frame: u32) -> JointMatrices {
        let parent = |id| {
            let joint = &self.joints[self.by_id[&id]];
            (joint.parent_id >= 0).then_some(joint.parent_id as u32)
        };
        JointMatrices {
            object_ids: self.joints.iter().map(|joint| joint.object_id).collect(),
            kinds: self.kinds.clone(),
            matrices: pcx_mdl::bone_matrices(&self.joints, &parent, &self.pivots, frame),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeosetInput {
    pub vertex_count: usize,
    pub material_id: u32,
    pub matrix_joint_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeosetBinding {
    /// Authored GEOS record ordinal; use this as the mesh marker, not vertex count.
    pub geoset_id: usize,
    pub vertex_count: usize,
    pub material_id: usize,
    /// GEOA record ordinals whose actual geoset_id names this geoset.
    pub animation_records: Vec<usize>,
    pub matrix_joint_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialBinding {
    pub priority_plane: i32,
    pub flags: u32,
    /// Every checked layer, including alpha/KMTF tracks and TXAN references.
    pub layers: Vec<MaterialLayer>,
}

/// Typed decoded inputs also allow discriminating synthetic binding fixtures.
#[derive(Debug)]
pub struct RuntimeInput {
    pub bones: Vec<Bone>,
    pub helpers: Vec<Node>,
    pub pivots: Pivots,
    pub geosets: Vec<GeosetInput>,
    pub geoset_animations: Vec<GeosetAnim>,
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    pub texture_animations: Vec<TextureAnim>,
    pub global_sequences: Vec<u32>,
}

#[derive(Debug)]
pub struct RuntimePlan {
    pub joints: JointPlan,
    pub geosets: Vec<GeosetBinding>,
    pub geoset_animations: Vec<GeosetAnim>,
    /// NO_ID records stay in the source list and are listed here without a binding.
    pub unbound_geoset_animations: Vec<usize>,
    pub materials: Vec<MaterialBinding>,
    pub textures: Vec<Texture>,
    pub texture_animations: Vec<TextureAnim>,
    pub global_sequences: Vec<u32>,
}

fn reference(context: String, id: u32, count: usize) -> Result<(), PlanError> {
    if (id as usize) < count {
        Ok(())
    } else {
        Err(PlanError::Reference { context, id, count })
    }
}
fn global_reference(context: String, id: u32, count: usize) -> Result<(), PlanError> {
    if id == NO_ID {
        Ok(())
    } else {
        reference(context, id, count)
    }
}

impl RuntimePlan {
    pub fn parse(bytes: &[u8]) -> Result<Self, PlanError> {
        let mdl = Mdlx::parse(bytes)?;
        let mut geosets = Vec::new();
        for geoset in mdl.geosets(bytes)? {
            let mesh = mdl.mesh(bytes, &geoset)?;
            geosets.push(GeosetInput {
                vertex_count: mesh.vertices.count as usize,
                material_id: mesh.material_id,
                matrix_joint_ids: mesh.matrix_indices,
            });
        }
        let globals = mdl.payload(bytes, pcx_mdl::tag::GLBS).unwrap_or(&[]);
        if globals.len() % 4 != 0 {
            return Err(PlanError::GlobalSequenceShape(globals.len()));
        }
        Self::checked(RuntimeInput {
            bones: mdl.bones(bytes)?,
            helpers: mdl.nodes(bytes, pcx_mdl::tag::HELP)?,
            pivots: mdl.pivots(bytes)?,
            geosets,
            geoset_animations: mdl.geoset_anims(bytes)?,
            materials: mdl.materials(bytes)?,
            textures: mdl.textures(bytes)?,
            texture_animations: mdl.texture_anims(bytes)?,
            global_sequences: globals
                .chunks_exact(4)
                .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
                .collect(),
        })
    }

    pub fn checked(input: RuntimeInput) -> Result<Self, PlanError> {
        let joints = JointPlan::checked(&input.bones, &input.helpers, &input.pivots)?;
        let global_count = input.global_sequences.len();
        for joint in joints.joints() {
            for track in &joint.tracks {
                global_reference(
                    format!("joint {} global sequence", joint.object_id),
                    u32::from_le_bytes(track.global_sequence_id.to_le_bytes()),
                    global_count,
                )?;
            }
        }
        let mut materials = Vec::new();
        for (material_id, material) in input.materials.iter().enumerate() {
            let layers = material.checked_layers()?;
            for (layer_id, layer) in layers.iter().enumerate() {
                let context = format!("material {material_id} layer {layer_id}");
                reference(
                    format!("{context} texture"),
                    layer.properties.texture_id,
                    input.textures.len(),
                )?;
                global_reference(
                    format!("{context} TXAN"),
                    layer.properties.texture_anim_id,
                    input.texture_animations.len(),
                )?;
                for track in &layer.alpha_tracks {
                    global_reference(
                        format!("{context} alpha global sequence"),
                        track.global_seq_id,
                        global_count,
                    )?;
                }
                for track in &layer.texture_tracks {
                    global_reference(
                        format!("{context} KMTF global sequence"),
                        track.global_seq_id,
                        global_count,
                    )?;
                    for &(_, texture) in &track.keys {
                        reference(
                            format!("{context} KMTF texture"),
                            texture,
                            input.textures.len(),
                        )?;
                    }
                }
            }
            materials.push(MaterialBinding {
                priority_plane: material.priority_plane()?,
                flags: material.flags()?,
                layers,
            });
        }
        for (index, animation) in input.texture_animations.iter().enumerate() {
            for track in &animation.tracks {
                global_reference(
                    format!("TXAN {index} global sequence"),
                    track.global_seq_id,
                    global_count,
                )?;
            }
        }
        let mut geosets = Vec::new();
        for (geoset_id, geoset) in input.geosets.into_iter().enumerate() {
            reference(
                format!("geoset {geoset_id} material"),
                geoset.material_id,
                materials.len(),
            )?;
            for &joint in &geoset.matrix_joint_ids {
                if joints.index_of(joint).is_none() {
                    return Err(PlanError::MissingMatrixJoint {
                        geoset: geoset_id,
                        joint,
                    });
                }
            }
            geosets.push(GeosetBinding {
                geoset_id,
                vertex_count: geoset.vertex_count,
                material_id: geoset.material_id as usize,
                animation_records: Vec::new(),
                matrix_joint_ids: geoset.matrix_joint_ids,
            });
        }
        let mut unbound_geoset_animations = Vec::new();
        for (index, animation) in input.geoset_animations.iter().enumerate() {
            for track in &animation.tracks {
                global_reference(
                    format!("GEOA {index} global sequence"),
                    track.global_seq_id,
                    global_count,
                )?;
            }
            if animation.geoset_id == NO_ID {
                unbound_geoset_animations.push(index);
            } else {
                reference(
                    format!("GEOA {index} geoset"),
                    animation.geoset_id,
                    geosets.len(),
                )?;
                geosets[animation.geoset_id as usize]
                    .animation_records
                    .push(index);
            }
        }
        Ok(Self {
            joints,
            geosets,
            geoset_animations: input.geoset_animations,
            unbound_geoset_animations,
            materials,
            textures: input.textures,
            texture_animations: input.texture_animations,
            global_sequences: input.global_sequences,
        })
    }

    /// Concrete collisions for the viewer's current vertex-count keyed cache.
    pub fn duplicate_vertex_counts(&self) -> BTreeMap<usize, Vec<usize>> {
        let mut counts: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for geoset in &self.geosets {
            counts
                .entry(geoset.vertex_count)
                .or_default()
                .push(geoset.geoset_id);
        }
        counts.retain(|_, geosets| geosets.len() > 1);
        counts
    }
}
