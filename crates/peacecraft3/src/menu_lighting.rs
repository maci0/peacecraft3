//! Menu Gouraud lighting in numeric source channels, before texture modulation.
use bevy::prelude::*;
use pcx_play::menu_lighting::Parameters;

// Local parsing/work ceiling, independent of original record measurements.
const MAX_LIGHT_BYTES: usize = 1024 * 1024;

pub(crate) fn parameters(install: Option<&std::path::Path>) -> Result<Parameters, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = install;
        crate::browser::with_committed_client_image(read_parameters)
            .unwrap_or_else(|| Err("menu lighting: selected client image unavailable".into()))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Read;
        let path = install
            .ok_or("menu lighting: selected installation unavailable")?
            .join("Warcraft III.exe");
        let mut file =
            std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut bytes = Vec::new();
        // Local resource ceiling, not an original asset measurement.
        file.by_ref()
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        read_parameters(&path.display().to_string(), &bytes)
    }
}

fn read_parameters(source: &str, bytes: &[u8]) -> Result<Parameters, String> {
    Parameters::read(
        bytes,
        pcx_play::scripts::AssetIdentity {
            source: source.into(),
            edition: "selected menu client".into(),
            version: "reviewed Legacy 1.29.2.9231 uncapped D3D9 profile".into(),
        },
    )
    .map_err(|e| e.to_string())
}

struct Light {
    position: Vec3,
    colour: [f32; 3],
    intensity: f32,
    ambient_colour: [f32; 3],
    ambient_intensity: f32,
    tracks: Vec<pcx_mdl::AnimTrack>,
}

pub(crate) struct Lighting {
    source: String,
    parameters: Parameters,
    lights: Vec<Light>,
    durations: Vec<u32>,
    interval: [u32; 2],
    lit: Vec<bool>,
}

impl Lighting {
    pub(crate) fn checked(
        source: &str,
        model: &pcx_mdl::Mdlx,
        bytes: &[u8],
        runtime: &pcx_gfx::mdx_runtime::RuntimePlan,
        interval: [u32; 2],
        parameters: Parameters,
    ) -> Result<Self, String> {
        let fail = |reason: &str| format!("{source}: LITE: {reason}");
        if interval[0] > interval[1] {
            return Err(fail("invalid selected clip interval"));
        }
        if model
            .payload(bytes, pcx_mdl::tag::LITE)
            .is_some_and(|data| data.len() > MAX_LIGHT_BYTES)
        {
            return Err(fail("light source byte budget exceeded"));
        }
        let input = model.lights(bytes).map_err(|e| fail(&e.to_string()))?;
        // Selection beyond the source slot budget needs its light-ranking caller.
        if input.len() > parameters.max_lights {
            return Err(fail(
                "light slot budget exceeded; source light ranking is unsupported",
            ));
        }
        let mut lights = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for l in input {
            if !ids.insert(l.object_id) || runtime.joints.index_of(l.object_id).is_some() {
                return Err(fail("duplicate light node identity"));
            }
            if l.kind != pcx_mdl::LightKind::Omnidirectional
                || l.parent_id != -1
                || !l.node_tracks.is_empty()
            {
                return Err(fail(
                    "only unparented point lights without node animation are supported",
                ));
            }
            let position = runtime
                .joints
                .pivots()
                .position(l.object_id)
                .ok_or_else(|| fail("missing authored light pivot"))?;
            if !position.iter().all(|n| n.is_finite())
                || ![
                    l.attenuation_start,
                    l.attenuation_end,
                    l.intensity,
                    l.ambient_intensity,
                ]
                .iter()
                .all(|n| n.is_finite() && *n >= 0.0)
                || !l
                    .colour
                    .iter()
                    .chain(&l.ambient_colour)
                    .all(|n| n.is_finite() && (0.0..=1.0).contains(n))
            {
                return Err(fail("unusable source light value"));
            }
            let mut tracks = Vec::new();
            for track in l.tracks {
                let width = match &track.magic {
                    b"KLAC" | b"KLBC" => 3,
                    b"KLAI" | b"KLBI" | b"KLAV" => 1,
                    _ => return Err(fail("unsupported light state track")),
                };
                let tangent_count = if track.interpolation.has_tangents() {
                    3
                } else {
                    1
                };
                let stride = 4 + width * 4 * tangent_count;
                if track.width != width
                    || track.stride != stride
                    || track.count == 0
                    || track.raw.len() != track.count as usize * stride
                    || tracks
                        .iter()
                        .any(|t: &pcx_mdl::AnimTrack| t.magic == track.magic)
                    || (track.global_seq_id != pcx_mdl::NO_ID
                        && runtime
                            .global_sequences
                            .get(track.global_seq_id as usize)
                            .is_none_or(|n| *n == 0))
                {
                    return Err(fail("invalid light track layout or global clock"));
                }
                let mut previous = None;
                for key in track.raw.chunks_exact(stride) {
                    let frame = u32::from_le_bytes(key[..4].try_into().unwrap());
                    if frame > i32::MAX as u32
                        || previous.replace(frame).is_some_and(|old| frame <= old)
                        || key[4..]
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .any(|n| !f32::from_le_bytes(*n).is_finite())
                    {
                        return Err(fail("invalid light key or tangent"));
                    }
                }
                if track.magic == *b"KLAV"
                    && (track.interpolation != pcx_mdl::Interpolation::None
                        || track.raw.chunks_exact(stride).any(|key| {
                            !matches!(f32::from_le_bytes(key[4..8].try_into().unwrap()), 0.0 | 1.0)
                        }))
                {
                    return Err(fail(
                        "fractional/interpolated light visibility is unsupported",
                    ));
                }
                let raw: Vec<u8> = if track.global_seq_id == pcx_mdl::NO_ID {
                    track
                        .raw
                        .chunks_exact(stride)
                        .filter(|key| {
                            let frame = u32::from_le_bytes(key[..4].try_into().unwrap());
                            (interval[0]..=interval[1]).contains(&frame)
                        })
                        .flatten()
                        .copied()
                        .collect()
                } else {
                    let duration = runtime.global_sequences[track.global_seq_id as usize];
                    if track
                        .raw
                        .chunks_exact(stride)
                        .any(|key| u32::from_le_bytes(key[..4].try_into().unwrap()) > duration)
                    {
                        return Err(fail("light key exceeds its global clock"));
                    }
                    track.raw
                };
                if raw.is_empty() {
                    return Err(fail("no light state keys in selected clip"));
                }
                tracks.push(pcx_mdl::AnimTrack {
                    magic: track.magic,
                    count: (raw.len() / stride) as u32,
                    interpolation: track.interpolation,
                    global_seq_id: track.global_seq_id,
                    raw,
                });
            }
            lights.push(Light {
                position: position.into(),
                colour: l.colour,
                intensity: l.intensity,
                ambient_colour: l.ambient_colour,
                ambient_intensity: l.ambient_intensity,
                tracks,
            });
        }
        Ok(Self {
            source: source.into(),
            parameters,
            lights,
            durations: runtime.global_sequences.clone(),
            interval,
            lit: Vec::new(),
        })
    }

    pub(crate) fn count(&self) -> usize {
        self.lights.len()
    }

    /// Retain authored shading flags before the materials switch off PBR.
    pub(crate) fn bind(
        &mut self,
        bound: &[crate::mainmenu_scene::BoundGeoset],
        materials: &Assets<crate::menu_material::MenuMaterial>,
    ) {
        self.lit = bound
            .iter()
            .map(|(_, _, h)| {
                !materials
                    .get(h)
                    .expect("newly allocated menu material")
                    .base
                    .unlit
            })
            .collect();
    }

    pub(crate) fn apply(
        &self,
        model: &pcx_gfx::LoadedModel,
        skeleton: &pcx_gfx::Skeleton,
        (handles, bound): (&[Handle<Mesh>], &[crate::mainmenu_scene::BoundGeoset]),
        (frame, elapsed_ms): (u32, u64),
        (meshes, materials): (
            &mut Assets<Mesh>,
            &mut Assets<crate::menu_material::MenuMaterial>,
        ),
    ) -> Result<(), String> {
        if model.meshes.len() != handles.len()
            || handles.len() != bound.len()
            || bound.len() != self.lit.len()
        {
            return Err(format!("{}: lighting binding count mismatch", self.source));
        }
        let lights = self.sample(frame, elapsed_ms)?;
        for (index, ((rest, handle), (_, _, material))) in
            model.meshes.iter().zip(handles).zip(bound).enumerate()
        {
            if !self.lit[index] {
                continue;
            }
            let mut material = materials
                .get_mut(material)
                .ok_or("menu lighting material ownership lost")?;
            let base = &mut material.base;
            let colour = base.base_color.to_linear().to_f32_array();
            if colour[3] == 0.0 {
                continue;
            }
            let mut mesh = meshes
                .get_mut(handle)
                .ok_or("menu lighting mesh ownership lost")?;
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                return Err(format!("{}: missing posed vertex positions", self.source));
            };
            let normals = normals(rest, skeleton).map_err(|e| format!("{}: {e}", self.source))?;
            if positions.iter().any(|p| p.iter().any(|v| !v.is_finite())) {
                return Err(format!("{}: nonfinite posed vertex position", self.source));
            }
            if positions.len() != normals.len() {
                return Err(format!("{}: vertex/normal count mismatch", self.source));
            }
            let colours: Vec<[f32; 4]> = positions
                .iter()
                .zip(normals)
                .map(|(p, n)| {
                    let mut rgb = Vec3::ZERO;
                    for light in &lights {
                        let delta = light.position - Vec3::from(*p);
                        let distance = delta.length();
                        let lambert = delta
                            .try_normalize()
                            .map_or(0.0, |direction| n.dot(direction).max(0.0));
                        rgb += (light.ambient + light.diffuse * lambert)
                            * self.parameters.attenuation(distance);
                    }
                    let rgb = (rgb * Vec3::from_array([colour[0], colour[1], colour[2]]))
                        .clamp(Vec3::ZERO, Vec3::ONE);
                    [rgb.x, rgb.y, rgb.z, 1.0]
                })
                .collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
            base.base_color = crate::menu_composition::colour([1.0, 1.0, 1.0, colour[3]]);
            base.unlit = true;
        }
        Ok(())
    }

    fn sample(&self, frame: u32, elapsed_ms: u64) -> Result<Vec<Sample>, String> {
        self.lights
            .iter()
            .map(|l| {
                let mut colour = l.colour;
                let mut ambient_colour = l.ambient_colour;
                let mut intensity = l.intensity;
                let mut ambient_intensity = l.ambient_intensity;
                let mut visibility = 1.0;
                for t in &l.tracks {
                    let time = if t.global_seq_id == pcx_mdl::NO_ID {
                        frame.clamp(self.interval[0], self.interval[1])
                    } else {
                        (elapsed_ms % u64::from(self.durations[t.global_seq_id as usize])) as u32
                    };
                    match &t.magic {
                        b"KLAC" => colour = pcx_mdl::sample_colour(t, time),
                        b"KLBC" => ambient_colour = pcx_mdl::sample_colour(t, time),
                        b"KLAI" => intensity = pcx_mdl::sample_scalar(t, time),
                        b"KLBI" => ambient_intensity = pcx_mdl::sample_scalar(t, time),
                        b"KLAV" => visibility = pcx_mdl::sample_scalar(t, time),
                        _ => unreachable!("checked light track"),
                    }
                }
                if ![intensity, ambient_intensity, visibility]
                    .iter()
                    .all(|n| n.is_finite() && *n >= 0.0)
                    || !colour
                        .iter()
                        .chain(&ambient_colour)
                        .all(|n| n.is_finite() && (0.0..=1.0).contains(n))
                {
                    return Err(format!("{}: unusable sampled light state", self.source));
                }
                let enabled = if visibility > 0.0 { 1.0 } else { 0.0 };
                Ok(Sample {
                    position: l.position,
                    diffuse: channels(colour)
                        * (intensity * self.parameters.channel_scale)
                        * enabled,
                    ambient: channels(ambient_colour)
                        * (ambient_intensity * self.parameters.channel_scale)
                        * enabled,
                })
            })
            .collect()
    }
}

struct Sample {
    position: Vec3,
    diffuse: Vec3,
    ambient: Vec3,
}

// Original light submission reads packed colour bytes before intensity scaling.
fn channels(colour: [f32; 3]) -> Vec3 {
    Vec3::from_array(colour.map(|n| f32::from((n * f32::from(u8::MAX)) as u8)))
}

fn normals(
    mesh: &pcx_gfx::LoadedMesh,
    skeleton: &pcx_gfx::Skeleton,
) -> Result<Vec<Vec3>, &'static str> {
    // shortcut: match position skinning's first matrix; upgrade both for multi-matrix groups.
    let mut groups = Vec::new();
    let mut at = 0usize;
    for &count in &mesh.group_counts {
        let matrix = mesh
            .bone_indices
            .get(at)
            .and_then(|id| skeleton.ids.iter().position(|n| n == id))
            .and_then(|i| skeleton.matrices.get(i))
            .map(Mat4::from_cols_array);
        let normal = match matrix {
            Some(m) if !m.is_finite() || m.determinant() == 0.0 => {
                return Err("unusable posed normal matrix");
            }
            Some(m) => Some(m.inverse().transpose()),
            None => None,
        };
        groups.push(normal);
        at = at
            .checked_add(count as usize)
            .ok_or("normal group count overflow")?;
    }
    mesh.normals
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let n = Vec3::from(*n);
            let n = mesh
                .vertex_groups
                .get(i)
                .and_then(|g| groups.get(*g as usize))
                .and_then(Option::as_ref)
                .map_or(n, |m| m.transform_vector3(n));
            n.try_normalize().ok_or("nonfinite or zero posed normal")
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn parameters() -> Parameters {
        Parameters {
            origin: pcx_play::scripts::AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            image_sha256: [71; 32],
            constant: 2.0,
            linear: 0.0,
            quadratic: 0.0,
            range: 10.0,
            channel_scale: 1.0 / f32::from(u8::MAX),
            max_lights: 4,
        }
    }
    pub(crate) fn lighting() -> Lighting {
        Lighting {
            source: "synthetic".into(),
            parameters: parameters(),
            lights: vec![Light {
                position: Vec3::Z * 2.0,
                colour: [1.0, 0.5, 0.25],
                intensity: 8.0,
                ambient_colour: [1.0; 3],
                ambient_intensity: 0.0,
                tracks: vec![],
            }],
            durations: vec![],
            interval: [0, 100],
            lit: vec![],
        }
    }
    fn mesh() -> pcx_gfx::LoadedMesh {
        pcx_gfx::LoadedMesh {
            positions: vec![[0.0; 3]; 2],
            normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, -1.0]],
            tex_coords: vec![[0.0; 2]; 2],
            indices: vec![],
            vertex_groups: vec![],
            bone_indices: vec![],
            group_counts: vec![],
            geoset: 0,
            material_id: 0,
        }
    }
    fn fixture_record(track: &[u8]) -> Vec<u8> {
        let mut record = Vec::new();
        record.extend((144u32 + track.len() as u32).to_le_bytes());
        record.extend(96u32.to_le_bytes());
        record.extend([0; 80]);
        for value in [0u32, u32::MAX, 0x200, 0] {
            record.extend(value.to_le_bytes());
        }
        for value in [2.0f32, 9.0, 0.5, 0.25, 0.75, 4.0, 1.0, 1.0, 1.0, 0.25] {
            record.extend(value.to_le_bytes());
        }
        record.extend(track);
        record
    }
    fn checked_fixture(
        records: &[u8],
        pivots: Vec<[f32; 3]>,
        interval: [u32; 2],
    ) -> Result<Lighting, String> {
        let mut bytes = b"MDLXVERS".to_vec();
        bytes.extend(4u32.to_le_bytes());
        bytes.extend(800u32.to_le_bytes());
        bytes.extend(b"LITE");
        bytes.extend((records.len() as u32).to_le_bytes());
        bytes.extend(records);
        let mdx = pcx_mdl::Mdlx::parse(&bytes).unwrap();
        let runtime =
            pcx_gfx::mdx_runtime::RuntimePlan::checked(pcx_gfx::mdx_runtime::RuntimeInput {
                bones: vec![],
                helpers: vec![],
                pivots: pcx_mdl::Pivots(pivots),
                geosets: vec![],
                geoset_animations: vec![],
                materials: vec![],
                textures: vec![],
                texture_animations: vec![],
                global_sequences: vec![],
            })
            .unwrap();
        Lighting::checked("synthetic", &mdx, &bytes, &runtime, interval, parameters())
    }
    fn visibility_track(value: f32, global: u32) -> Vec<u8> {
        let mut track = b"KLAV".to_vec();
        for n in [2u32, 0, global] {
            track.extend(n.to_le_bytes());
        }
        for (frame, value) in [(0u32, value), (100, 1.0)] {
            track.extend(frame.to_le_bytes());
            track.extend(value.to_le_bytes());
        }
        track
    }
    #[test]
    fn source_light_admission_refuses_missing_or_inconsistent_inputs() {
        assert!(
            checked_fixture(&vec![0; MAX_LIGHT_BYTES + 1], vec![[0.0; 3]], [0, 100])
                .err()
                .unwrap()
                .contains("budget")
        );
        let record = fixture_record(&[]);
        assert_eq!(
            checked_fixture(&record, vec![[0.0; 3]], [0, 100])
                .unwrap()
                .count(),
            1
        );
        assert!(
            checked_fixture(&record, vec![], [0, 100])
                .err()
                .unwrap()
                .contains("pivot")
        );
        assert!(checked_fixture(&record, vec![[0.0; 3]], [100, 0]).is_err());
        let mut duplicate = record.clone();
        duplicate.extend(&record);
        assert!(
            checked_fixture(&duplicate, vec![[0.0; 3]], [0, 100])
                .err()
                .unwrap()
                .contains("identity")
        );
        for (offset, bits) in [(92, 0u32), (100, 1), (124, f32::NAN.to_bits())] {
            let mut bad = record.clone();
            bad[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
            assert!(checked_fixture(&bad, vec![[0.0; 3]], [0, 100]).is_err());
        }
    }
    #[test]
    fn visibility_admission_preserves_discrete_keys_and_refuses_guessed_thresholds() {
        let record = fixture_record(&visibility_track(0.0, pcx_mdl::NO_ID));
        let lighting = checked_fixture(&record, vec![[0.0; 3]], [0, 100]).unwrap();
        assert_eq!(lighting.sample(50, 0).unwrap()[0].diffuse, Vec3::ZERO);
        assert!(lighting.sample(100, 0).unwrap()[0].diffuse.length() > 0.0);
        for (value, global) in [(0.5, pcx_mdl::NO_ID), (0.0, 0)] {
            let record = fixture_record(&visibility_track(value, global));
            assert!(checked_fixture(&record, vec![[0.0; 3]], [0, 100]).is_err());
        }
    }
    fn skeleton() -> pcx_gfx::Skeleton {
        pcx_gfx::Skeleton {
            matrices: vec![],
            ids: vec![],
            names: vec![],
        }
    }
    #[test]
    fn vertex_lighting_clamps_after_material_colour_without_an_ambient_floor() {
        let mut lighting = lighting();
        let rest = mesh();
        let model = pcx_gfx::LoadedModel {
            name: "fixture".into(),
            minimum_extent: [0.0; 3],
            maximum_extent: [0.0; 3],
            meshes: vec![rest.clone()],
            bone_names: vec![],
            bone_parents: vec![],
            sequences: vec![],
        };
        let mut meshes = Assets::<Mesh>::default();
        let handle = meshes.add(pcx_gfx::to_bevy_mesh(&rest));
        let mut materials = Assets::<crate::menu_material::MenuMaterial>::default();
        let material = materials.add(
            crate::menu_material::checked_source_mode(
                StandardMaterial {
                    base_color: crate::menu_composition::colour([0.25, 0.5, 0.75, 0.37]),
                    unlit: false,
                    ..default()
                },
                0,
                0,
            )
            .unwrap(),
        );
        let bound = vec![(0, Entity::PLACEHOLDER, material.clone())];
        lighting.bind(&bound, &materials);
        lighting
            .apply(
                &model,
                &skeleton(),
                (std::slice::from_ref(&handle), &bound),
                (0, 0),
                (&mut meshes, &mut materials),
            )
            .unwrap();
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(values)) = meshes
            .get(&handle)
            .unwrap()
            .attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("missing lighting output")
        };
        let expected = [1.0, 127.0 / 255.0 * 2.0, 63.0 / 255.0 * 3.0, 1.0];
        for (got, expected) in values[0].iter().zip(expected) {
            assert!((got - expected).abs() < 0.000001, "{got} != {expected}");
        }
        assert_eq!(values[1], [0.0, 0.0, 0.0, 1.0]);
        let material = &materials.get(&material).unwrap().base;
        assert!(material.unlit);
        assert_eq!(
            material.base_color.to_linear().to_f32_array(),
            [1.0, 1.0, 1.0, 0.37]
        );
    }
    #[test]
    fn global_light_state_uses_elapsed_time_and_packed_source_channels() {
        let mut l = lighting();
        l.durations = vec![80];
        let mut raw = Vec::new();
        for (frame, value) in [(0u32, 0.0f32), (80, 8.0)] {
            raw.extend(frame.to_le_bytes());
            raw.extend(value.to_le_bytes());
        }
        l.lights[0].tracks.push(pcx_mdl::AnimTrack {
            magic: *b"KLAI",
            count: 2,
            interpolation: pcx_mdl::Interpolation::Linear,
            global_seq_id: 0,
            raw,
        });
        assert_eq!(l.sample(50, 0).unwrap()[0].diffuse, Vec3::ZERO);
        let a = l.sample(0, 40).unwrap()[0].diffuse;
        let b = l.sample(100, 120).unwrap()[0].diffuse;
        assert_eq!(a, b);
        assert_eq!(a.x, 4.0);
        assert_eq!(channels([0.5, 1.0, 0.0]), Vec3::new(127.0, 255.0, 0.0));
    }
    #[test]
    fn animated_light_colours_keep_all_three_channels() {
        for magic in [*b"KLAC", *b"KLBC"] {
            let mut lighting = lighting();
            lighting.lights[0].ambient_intensity = 2.0;
            let mut raw = Vec::new();
            for (frame, values) in [(0u32, [0.0f32, 0.0, 1.0]), (100, [1.0, 1.0, 0.0])] {
                raw.extend(frame.to_le_bytes());
                for value in values {
                    raw.extend(value.to_le_bytes());
                }
            }
            lighting.lights[0].tracks.push(pcx_mdl::AnimTrack {
                magic,
                count: 2,
                interpolation: pcx_mdl::Interpolation::Linear,
                global_seq_id: pcx_mdl::NO_ID,
                raw,
            });
            let sampled = lighting.sample(50, 50).unwrap();
            let (got, intensity) = if magic == *b"KLAC" {
                (sampled[0].diffuse, 8.0)
            } else {
                (sampled[0].ambient, 2.0)
            };
            let expected = 127.0 * (intensity / 255.0);
            assert!((got - Vec3::splat(expected)).length() < 0.000001);
        }
    }

    #[test]
    fn normal_pose_resolves_sparse_joint_ids_and_excludes_translation() {
        let mut mesh = mesh();
        mesh.vertex_groups = vec![0; 2];
        mesh.group_counts = vec![1];
        mesh.bone_indices = vec![19];
        let matrix = Mat4::from_translation(Vec3::splat(73.0))
            * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let skeleton = pcx_gfx::Skeleton {
            matrices: vec![matrix.to_cols_array()],
            ids: vec![19],
            names: vec!["fixture".into()],
        };
        let normals = normals(&mesh, &skeleton).unwrap();
        assert!((normals[0] - Vec3::X).length() < 0.000001);
        assert!((normals[1] + Vec3::X).length() < 0.000001);
    }
    #[test]
    #[ignore = "requires explicitly selected original client and MDX files"]
    fn selected_original_light_plan_is_admitted_without_substituted_values() {
        let client = std::env::var("PCX_LIGHTING_CLIENT_IMAGE").expect("PCX_LIGHTING_CLIENT_IMAGE");
        let model = std::env::var("PCX_LIGHTING_MODEL").expect("PCX_LIGHTING_MODEL");
        let parameters = read_parameters(&client, &std::fs::read(&client).unwrap()).unwrap();
        let bytes = std::fs::read(&model).unwrap();
        let mdx = pcx_mdl::Mdlx::parse(&bytes).unwrap();
        let runtime = pcx_gfx::mdx_runtime::RuntimePlan::parse(&bytes).unwrap();
        let camera = mdx.cameras(&bytes).unwrap().remove(0);
        let camera = Transform::from_translation(Vec3::from(camera.eye))
            .looking_at(Vec3::from(camera.target), Vec3::Z);
        let camera_pose =
            pcx_gfx::mdx_billboard::CameraPosePlan::checked(&runtime.joints, camera.rotation)
                .unwrap();
        let clips = mdx.sequences(&bytes).unwrap();
        let clip = clips.iter().find(|c| c.name == "Stand").unwrap();
        let mut lighting = Lighting::checked(
            &model,
            &mdx,
            &bytes,
            &runtime,
            [clip.interval_start, clip.interval_end],
            parameters,
        )
        .unwrap();
        assert_eq!(lighting.count(), mdx.lights(&bytes).unwrap().len());
        for elapsed in [0, 1, 137, 1001, 16000] {
            assert_eq!(
                lighting.sample(clip.interval_start, elapsed).unwrap().len(),
                lighting.count()
            );
        }
        let loaded = pcx_gfx::load_model(&bytes, &model).unwrap();
        let source_materials = mdx.materials(&bytes).unwrap();
        let opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(
            &runtime,
            [clip.interval_start, clip.interval_end],
        )
        .unwrap();
        let mut meshes = Assets::<Mesh>::default();
        let handles: Vec<_> = loaded
            .meshes
            .iter()
            .map(|m| meshes.add(pcx_gfx::to_bevy_mesh(m)))
            .collect();
        let mut materials = Assets::<crate::menu_material::MenuMaterial>::default();
        let bound: Vec<_> = loaded
            .meshes
            .iter()
            .map(|m| {
                let flags =
                    crate::mainmenu_scene::layer_flags(&source_materials, m.material_id as usize);
                let material = materials.add(
                    crate::menu_material::checked_source_mode(
                        StandardMaterial {
                            unlit: flags.unshaded,
                            alpha_mode: flags.alpha_mode,
                            ..default()
                        },
                        runtime.materials[m.material_id as usize].layers[0]
                            .properties
                            .filter_mode,
                        runtime.materials[m.material_id as usize].layers[0]
                            .properties
                            .shading_flags,
                    )
                    .unwrap(),
                );
                (m.geoset, Entity::PLACEHOLDER, material)
            })
            .collect();
        lighting.bind(&bound, &materials);
        for step in 0..=100u64 {
            let elapsed = u64::from(clip.interval_end - clip.interval_start) * step / 100;
            let frame = clip.interval_start + elapsed as u32;
            for (geo, _, handle) in &bound {
                let rgb = opacity
                    .sample_material_colour(*geo, frame, elapsed)
                    .unwrap();
                let alpha = opacity.sample(*geo, frame, elapsed).unwrap();
                materials.get_mut(handle).unwrap().base.base_color =
                    crate::menu_composition::colour([rgb[0], rgb[1], rgb[2], alpha]);
            }
            let skeleton = crate::mainmenu_scene::skin_geosets(
                crate::mainmenu_scene::Rig {
                    model: &loaded,
                    bones: runtime.joints.joints(),
                    pivots: runtime.joints.pivots(),
                    global_durations: &runtime.global_sequences,
                    camera_pose: Some(&camera_pose),
                },
                &handles,
                frame,
                elapsed as u32,
                &mut meshes,
            )
            .unwrap();
            lighting
                .apply(
                    &loaded,
                    &skeleton,
                    (&handles, &bound),
                    (frame, elapsed),
                    (&mut meshes, &mut materials),
                )
                .unwrap_or_else(|e| panic!("source frame {frame}: {e}"));
        }
        println!(
            "admitted {} original lights, 101 CPU lighting frames; no model or engine parameter values substituted",
            lighting.count()
        );
    }
}
