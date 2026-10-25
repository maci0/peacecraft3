use super::*;
use bevy::{ecs::system::RunSystemOnce, mesh::VertexAttributeValues};
use pcx_gfx::mdx_particles as cpu;

pub(super) fn chunk(bytes: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    bytes.extend(tag);
    bytes.extend((payload.len() as u32).to_le_bytes());
    bytes.extend(payload);
}
fn properties(
    texture_id: u32,
    head_or_tail: u32,
    filter_mode: u32,
) -> pcx_mdl::Particle2Properties {
    pcx_mdl::Particle2Properties {
        speed: 13.0,
        variation: 0.1,
        latitude: 0.2,
        gravity: 3.0,
        life_span: 2.0,
        emission_rate: 120.0,
        width: 4.0,
        length: 7.0,
        filter_mode,
        rows: 1,
        columns: 1,
        head_or_tail,
        tail_length: 0.4,
        time: 0.5,
        segment_colours: [[0.2, 0.4, 0.7], [0.8, 0.6, 0.4], [0.1, 0.3, 0.9]],
        segment_alpha: [97, 183, 41],
        particle_scaling: [2.0, 3.0, 1.0],
        life_span_uv_anim: [0, 1, 1],
        decay_uv_anim: [0, 1, 1],
        tail_uv_anim: [0, 1, 1],
        tail_decay_uv_anim: [0, 1, 1],
        texture_id,
        squirt: 0,
        priority_plane: texture_id as i32,
        replaceable_id: 0,
    }
}
pub(crate) fn synthetic(
    change: impl FnOnce(&mut pcx_mdl::Particle2Properties, &mut u32),
) -> (Vec<u8>, Vec<pcx_gfx::LoadedTexture>) {
    let mut bytes = super::fog_camera_tests::empty_scene_with_camera();
    let mut sequence = vec![0; pcx_mdl::SEQUENCE_SIZE];
    sequence[..5].copy_from_slice(b"Stand");
    sequence[80..84].copy_from_slice(&13_u32.to_le_bytes());
    sequence[84..88].copy_from_slice(&213_u32.to_le_bytes());
    chunk(&mut bytes, b"SEQS", &sequence);
    let mut texs = Vec::new();
    for (path, flags) in [
        (r"Synthetic\Shared.blp", 0_u32),
        (r"Synthetic\Shared.blp", 3),
        ("Other.blp", 0),
    ] {
        let mut record = vec![0; pcx_mdl::TEXTURE_SIZE];
        record[4..4 + path.len()].copy_from_slice(path.as_bytes());
        record[264..268].copy_from_slice(&flags.to_le_bytes());
        texs.extend(record);
    }
    chunk(&mut bytes, b"TEXS", &texs);
    let pivots: Vec<u8> = [[17_f32, 19.0, 23.0], [29.0, 31.0, 37.0], [41.0, 43.0, 47.0]]
        .into_iter()
        .flatten()
        .flat_map(f32::to_le_bytes)
        .collect();
    chunk(&mut bytes, b"PIVT", &pivots);
    let mut pre2 = Vec::new();
    let mut first = properties(0, 2, 0);
    let mut first_flags = cpu::PARTICLE_EMITTER_TYPE;
    change(&mut first, &mut first_flags);
    for (id, (props, flags)) in [
        (first, first_flags),
        (
            properties(1, 2, 1),
            cpu::UNSHADED | cpu::PARTICLE_EMITTER_TYPE,
        ),
        (
            properties(2, 1, 0),
            cpu::UNFOGGED | cpu::PARTICLE_EMITTER_TYPE,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut record = vec![0; 4 + 96 + pcx_mdl::PARTICLE2_FIXED];
        let size = record.len() as u32;
        record[..4].copy_from_slice(&size.to_le_bytes());
        record[4..8].copy_from_slice(&96_u32.to_le_bytes());
        record[88..92].copy_from_slice(&(id as u32).to_le_bytes());
        record[92..96].copy_from_slice(&(-1_i32).to_le_bytes());
        record[96..100].copy_from_slice(&flags.to_le_bytes());
        record[100..].copy_from_slice(&props.to_bytes());
        pre2.extend(record);
    }
    chunk(&mut bytes, b"PRE2", &pre2);
    let textures = [
        ("synthetic/shared.blp", [37, 83, 149, 71]),
        ("Other.blp", [173, 113, 59, 199]),
    ]
    .into_iter()
    .map(|(source, rgba)| pcx_gfx::LoadedTexture {
        mipmaps: Vec::new(),
        width: 1,
        height: 1,
        rgba: rgba.to_vec(),
        source: source.into(),
    })
    .collect();
    (bytes, textures)
}
fn fog() -> crate::menu_fog::MenuFog {
    crate::menu_fog::MenuFog {
        color_argb: [179, 31, 53, 79],
        density: 0.3,
        start: 17.0,
        end: 997.0,
    }
}
struct Spawned {
    world: World,
    meshes: Assets<Mesh>,
    materials: Assets<MenuMaterial>,
    particle_materials: Assets<ParticleMaterial>,
    images: Assets<Image>,
    result: Result<SceneInfo, SceneError>,
}
fn spawn_case(bytes: &[u8], textures: &[pcx_gfx::LoadedTexture]) -> Spawned {
    spawn_case_with_parameters(
        bytes,
        textures,
        crate::menu_lighting::tests::parameters(),
        Entrance::Idle,
    )
}
fn spawn_case_with_parameters(
    bytes: &[u8],
    textures: &[pcx_gfx::LoadedTexture],
    parameters: pcx_play::menu_lighting::Parameters,
    entrance: Entrance,
) -> Spawned {
    let mut world = World::new();
    let mut meshes = Assets::default();
    let mut materials = Assets::default();
    let mut particle_materials = Assets::default();
    let mut images = Assets::default();
    let result = spawn(
        &mut world.commands(),
        GeosetAssets {
            meshes: &mut meshes,
            materials: &mut materials,
            images: &mut images,
        },
        &mut particle_materials,
        "synthetic:TFT:selected-model-and-image-batch",
        bytes,
        textures,
        (fog(), parameters, entrance),
    );
    world.flush();
    Spawned {
        world,
        meshes,
        materials,
        particle_materials,
        images,
        result,
    }
}

fn replace_chunk(bytes: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    let parsed = pcx_mdl::Mdlx::parse(bytes).unwrap();
    let old = parsed.chunk(tag).unwrap();
    let range = old.offset..old.payload_offset + old.size;
    let mut replacement = Vec::new();
    chunk(&mut replacement, tag, payload);
    bytes.splice(range, replacement);
}

fn birth_fixture() -> (Vec<u8>, Vec<pcx_gfx::LoadedTexture>) {
    let (mut bytes, textures) = synthetic(|_, _| {});
    let parsed = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let mut sequences = parsed.chunk(b"SEQS").unwrap().payload(&bytes).to_vec();
    let mut birth = vec![0; pcx_mdl::SEQUENCE_SIZE];
    birth[..5].copy_from_slice(b"Birth");
    birth[80..84].copy_from_slice(&701u32.to_le_bytes());
    birth[84..88].copy_from_slice(&918u32.to_le_bytes());
    birth[92..96].copy_from_slice(&1u32.to_le_bytes());
    sequences.extend(birth);
    replace_chunk(&mut bytes, b"SEQS", &sequences);
    (bytes, textures)
}

fn run_at(spawned: &mut Spawned, delta: u64) {
    spawned
        .world
        .resource_mut::<Time<Real>>()
        .advance_by(std::time::Duration::from_millis(delta));
    spawned.world.run_system_once(advance).unwrap();
}

#[test]
fn birth_handoff_preserves_particle_meshes_and_global_clock_across_non_aligned_frames() {
    let (bytes, textures) = birth_fixture();
    let mut birth = spawn_case_with_parameters(
        &bytes,
        &textures,
        crate::menu_lighting::tests::parameters(),
        Entrance::Birth,
    );
    let mut idle = spawn_case(&bytes, &textures);
    assert_eq!(birth.result.as_ref().unwrap().sequence, "Birth");
    assert_eq!(birth.world.resource::<MainMenuScene>().interval_start, 701);
    assert!(birth.world.resource::<MainMenuScene>().following.is_some());
    for spawned in [&mut birth, &mut idle] {
        spawned
            .world
            .insert_resource(std::mem::take(&mut spawned.meshes));
        spawned
            .world
            .insert_resource(std::mem::take(&mut spawned.materials));
        spawned
            .world
            .insert_resource(std::mem::take(&mut spawned.particle_materials));
        spawned.world.init_resource::<Time<Real>>();
    }
    let original_entities: Vec<_> = birth
        .world
        .query_filtered::<Entity, With<Mesh3d>>()
        .iter(&birth.world)
        .collect();
    let mut elapsed = 0;
    for delta in [113, 127, 31, 502] {
        elapsed += delta;
        run_at(&mut birth, delta);
        run_at(&mut idle, delta);
        let scene = birth.world.resource::<MainMenuScene>();
        assert_eq!(scene.elapsed.as_millis(), elapsed.into());
        assert!(scene.particles.as_ref().unwrap().enabled());
        if elapsed >= 217 {
            assert!(scene.following.is_none());
            assert_eq!(scene.clip_origin_ms, 217);
            assert_eq!(scene.interval_start, 13);
            assert_eq!(scene.time_ms, ((elapsed - 217) % 200) as f32);
        }
        let meshes = |spawned: &mut Spawned| {
            let handles: Vec<_> = spawned
                .world
                .query_filtered::<&Mesh3d, With<MeshMaterial3d<ParticleMaterial>>>()
                .iter(&spawned.world)
                .map(|mesh| mesh.0.clone())
                .collect();
            handles
                .into_iter()
                .map(|handle| {
                    let mesh = spawned
                        .world
                        .resource::<Assets<Mesh>>()
                        .get(&handle)
                        .unwrap();
                    [
                        Mesh::ATTRIBUTE_POSITION,
                        Mesh::ATTRIBUTE_UV_0,
                        Mesh::ATTRIBUTE_COLOR,
                    ]
                    .map(|attribute| format!("{:?}", mesh.attribute(attribute).unwrap()))
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            meshes(&mut birth),
            meshes(&mut idle),
            "the idle handoff must not restart the identical static particle batch"
        );
    }
    for entity in original_entities {
        assert!(birth.world.get_entity(entity).is_ok());
    }
}

#[test]
fn birth_and_idle_camera_plans_use_their_own_local_keys() {
    let (mut bytes, textures) = birth_fixture();
    let parsed = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let mut camera = parsed.chunk(b"CAMS").unwrap().payload(&bytes).to_vec();
    camera.extend(b"KCTR");
    for word in [4u32, 1, pcx_mdl::NO_ID] {
        camera.extend(word.to_le_bytes());
    }
    for (frame, x) in [(13u32, 41f32), (213, 61.), (701, 3.), (918, 19.)] {
        camera.extend(frame.to_le_bytes());
        for value in [x, 0., 0.] {
            camera.extend(value.to_le_bytes());
        }
    }
    let len = camera.len() as u32;
    camera[..4].copy_from_slice(&len.to_le_bytes());
    replace_chunk(&mut bytes, b"CAMS", &camera);
    let mut spawned = spawn_case_with_parameters(
        &bytes,
        &textures,
        crate::menu_lighting::tests::parameters(),
        Entrance::Birth,
    );
    spawned.result.as_ref().unwrap();
    spawned
        .world
        .insert_resource(std::mem::take(&mut spawned.meshes));
    spawned
        .world
        .insert_resource(std::mem::take(&mut spawned.materials));
    spawned
        .world
        .insert_resource(std::mem::take(&mut spawned.particle_materials));
    spawned.world.init_resource::<Time<Real>>();
    let camera = spawned
        .world
        .query_filtered::<Entity, With<MainMenuSceneCamera>>()
        .single(&spawned.world)
        .unwrap();
    assert_eq!(
        spawned
            .world
            .get::<Transform>(camera)
            .unwrap()
            .translation
            .x,
        10.
    );
    run_at(&mut spawned, 240);
    assert!(
        (spawned
            .world
            .get::<Transform>(camera)
            .unwrap()
            .translation
            .x
            - 50.3)
            .abs()
            < 0.0001
    );
    assert_eq!(
        spawned
            .world
            .resource::<MainMenuScene>()
            .elapsed
            .as_millis(),
        240
    );
}

#[test]
fn missing_birth_refuses_before_scene_or_particle_allocation() {
    let (bytes, textures) = synthetic(|_, _| {});
    let spawned = spawn_case_with_parameters(
        &bytes,
        &textures,
        crate::menu_lighting::tests::parameters(),
        Entrance::Birth,
    );
    assert!(
        spawned
            .result
            .unwrap_err()
            .to_string()
            .contains("missing Birth")
    );
    assert_eq!(spawned.meshes.len(), 0);
    assert_eq!(spawned.materials.len(), 0);
    assert_eq!(spawned.particle_materials.len(), 0);
    assert!(!spawned.world.contains_resource::<MainMenuScene>());
}

#[test]
fn instantaneous_birth_enters_its_authored_stand_without_allocating_a_second_scene() {
    let (mut bytes, textures) = birth_fixture();
    let parsed = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let seqs = parsed.chunk(b"SEQS").unwrap();
    let at = seqs.payload_offset + pcx_mdl::SEQUENCE_SIZE + 84;
    bytes[at..at + 4].copy_from_slice(&701u32.to_le_bytes());
    let spawned = spawn_case_with_parameters(
        &bytes,
        &textures,
        crate::menu_lighting::tests::parameters(),
        Entrance::Birth,
    );
    assert_eq!(spawned.result.as_ref().unwrap().sequence, "Stand");
    let scene = spawned.world.resource::<MainMenuScene>();
    assert!(scene.following.is_none());
    assert_eq!(scene.interval_start, 13);
    assert_eq!(scene.elapsed, std::time::Duration::ZERO);
}

#[test]
#[ignore = "requires PEACECRAFT3_INSTALL with original TFT campaign index, backdrop and client image"]
fn original_first_campaign_plays_birth_then_stand_without_resetting_its_scene_clock() {
    use pcx_objects::editor::campaign_index::{IndexLimits, parse_campaign_index};
    let install = std::path::PathBuf::from(
        std::env::var_os("PEACECRAFT3_INSTALL").expect("explicit original install"),
    );
    let member = r"UI\CampaignStrings_exp.txt";
    let index_bytes = crate::mainmenu::art_bytes(Some(&install), member).expect("campaign index");
    let index = parse_campaign_index(
        member,
        &index_bytes,
        IndexLimits {
            max_bytes: 1024 * 1024,
            max_campaign_slots: 128,
            max_missions_per_campaign: 1024,
        },
    )
    .unwrap();
    let first = index.first_index_entry().expect("first campaign");
    let profile =
        crate::menu_skin::SkinTable::parse(std::str::from_utf8(&index_bytes).unwrap()).unwrap();
    let fog = crate::menu_fog::MenuFog::campaign(&profile, &first.id)
        .unwrap()
        .unwrap();
    let skin_bytes =
        crate::mainmenu::art_bytes(Some(&install), crate::mainmenu::SKIN_TABLE).unwrap();
    let skin =
        crate::menu_skin::SkinTable::parse(std::str::from_utf8(&skin_bytes).unwrap()).unwrap();
    let path = pcx_mdl::binary_model_path(
        &skin
            .unique_file(&format!("{}_V1", first.background.as_ref().unwrap()))
            .unwrap()
            .file,
    );
    let bytes = crate::mainmenu::art_bytes(Some(&install), &path).unwrap();
    let textures = load_textures_in(Some(&install), &bytes, "campaign-scene");
    let parsed = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let sequences = parsed.sequences(&bytes).unwrap();
    let birth = sequences
        .iter()
        .find(|sequence| sequence.name == "Birth")
        .unwrap();
    let stand = sequences
        .iter()
        .find(|sequence| sequence.name == "Stand")
        .unwrap();
    let mut world = World::new();
    let mut meshes = Assets::default();
    let mut materials = Assets::default();
    let mut particles = Assets::default();
    let mut images = Assets::default();
    let info = spawn(
        &mut world.commands(),
        GeosetAssets {
            meshes: &mut meshes,
            materials: &mut materials,
            images: &mut images,
        },
        &mut particles,
        &path,
        &bytes,
        &textures,
        (
            fog,
            crate::menu_lighting::parameters(Some(&install)).unwrap(),
            Entrance::Birth,
        ),
    )
    .unwrap();
    world.flush();
    assert_eq!(info.sequence, "Birth");
    assert_eq!(info.duration_ms, birth.duration());
    assert_eq!(
        world.resource::<MainMenuScene>().interval_start,
        birth.interval_start
    );
    world.insert_resource(meshes);
    world.insert_resource(materials);
    world.insert_resource(particles);
    world.init_resource::<Time<Real>>();
    let total = u64::from(birth.duration()) + u64::from(stand.duration()) * 2;
    assert!(total < 60_000, "bounded original-input test");
    for _ in 0..total.div_ceil(16) {
        world
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_millis(16));
        world.run_system_once(advance).unwrap();
    }
    let scene = world.resource::<MainMenuScene>();
    assert!(scene.following.is_none());
    assert_eq!(scene.clip_origin_ms, u64::from(birth.duration()));
    assert_eq!(scene.interval_start, stand.interval_start);
    assert!(scene.elapsed.as_millis() >= total.into());
    assert_eq!(
        scene.time_ms as u64,
        (scene.elapsed.as_millis() as u64 - u64::from(birth.duration()))
            % u64::from(stand.duration())
    );
}

#[test]
fn particle_only_images_retain_their_authored_mip_chain() {
    let (bytes, mut textures) = synthetic(|_, _| {});
    for texture in &mut textures {
        texture.width = 2;
        texture.height = 2;
        texture.rgba = texture.rgba.repeat(4);
        texture.mipmaps = vec![vec![23, 67, 131, 197]];
    }
    let spawned = spawn_case(&bytes, &textures);
    assert!(spawned.result.unwrap().particle_gap.is_none());
    assert_eq!(spawned.images.len(), 3);
    for (_, image) in spawned.images.iter() {
        assert_eq!(image.texture_descriptor.mip_level_count, 2);
        assert_eq!(&image.data.as_ref().unwrap()[16..], &[23, 67, 131, 197]);
    }
}

#[test]
fn live_particle_mesh_uses_the_qualified_appearance_parameters() {
    let (bytes, textures) = synthetic(|props, _| {
        props.segment_colours = [[0., 1., 0.5], [1., 0., 0.5], [0.; 3]];
        props.segment_alpha = [0, 255, 0];
    });
    let mut parameters = crate::menu_lighting::tests::parameters();
    // A constant biased phase isolates binding from within-tick particle aging.
    parameters.particle_appearance.phase_scale = 0.;
    parameters.particle_appearance.phase_bias = 0.25;
    parameters.particle_appearance.colour_scale = 128.;
    let mut spawned = spawn_case_with_parameters(&bytes, &textures, parameters, Entrance::Idle);
    assert!(spawned.result.unwrap().particle_gap.is_none());
    let camera = spawned
        .world
        .query_filtered::<Entity, With<MainMenuSceneCamera>>()
        .single(&spawned.world)
        .unwrap();
    let order = spawned
        .world
        .get::<crate::menu_particle_order::SourceOrderView>(camera)
        .unwrap()
        .clone();
    let mesh = spawned
        .world
        .query::<(Entity, &Mesh3d)>()
        .iter(&spawned.world)
        .find(|(entity, _)| {
            order.orders[&bevy::render::sync_world::MainEntity::from(*entity)].ordinal == 0
        })
        .unwrap()
        .1
        .0
        .clone();
    spawned.world.insert_resource(spawned.meshes);
    spawned.world.insert_resource(spawned.materials);
    spawned.world.insert_resource(spawned.particle_materials);
    spawned.world.insert_resource(spawned.images);
    let mut time = Time::<Real>::default();
    time.advance_by(std::time::Duration::from_nanos(16_666_666));
    spawned.world.insert_resource(time);
    spawned.world.run_system_once(advance).unwrap();
    let mesh = spawned.world.resource::<Assets<Mesh>>().get(&mesh).unwrap();
    let VertexAttributeValues::Float32x4(colours) = mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
    else {
        panic!("particle vertex colours");
    };
    assert!(!colours.is_empty());
    assert!(
        colours
            .iter()
            .all(|rgba| *rgba == [32., 96., 64., 63.].map(|v| v / 255.)),
        "unexpected live colours: {colours:?}"
    );
}

#[test]
fn particle_whole_scene_binds_flags_sampler_union_camera_clock_and_owned_handles() {
    let (bytes, textures) = synthetic(|_, _| {});
    assert!(particle_import_needed("synthetic", &bytes).unwrap());
    assert_eq!(
        validate_particle_import("synthetic", &bytes, &textures).unwrap(),
        None
    );
    let mut spawned = spawn_case(&bytes, &textures);
    let info = spawned.result.unwrap();
    assert_eq!(info.particle_emitters, 3);
    assert!(info.particle_gap.is_none());
    assert_eq!(
        (
            spawned.meshes.len(),
            spawned.particle_materials.len(),
            spawned.images.len()
        ),
        (3, 3, 3)
    );
    let camera = spawned
        .world
        .query_filtered::<Entity, With<MainMenuSceneCamera>>()
        .single(&spawned.world)
        .unwrap();
    let order = spawned
        .world
        .get::<crate::menu_particle_order::SourceOrderView>(camera)
        .unwrap()
        .clone();
    let expected = pcx_mdl::Mdlx::parse(&bytes)
        .unwrap()
        .particles2(&bytes)
        .unwrap();
    let mut handles = Vec::new();
    for (entity, mesh, material) in spawned
        .world
        .query::<(Entity, &Mesh3d, &MeshMaterial3d<ParticleMaterial>)>()
        .iter(&spawned.world)
    {
        let source = &expected
            [order.orders[&bevy::render::sync_world::MainEntity::from(entity)].ordinal as usize];
        let props = source.properties();
        let material = &spawned.particle_materials.get(&material.0).unwrap().base;
        assert_eq!(material.unlit, source.flags & cpu::UNSHADED != 0);
        assert_eq!(material.fog_enabled, source.flags & cpu::UNFOGGED == 0);
        assert_eq!(
            material.alpha_mode,
            if props.filter_mode == 0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Add
            }
        );
        assert!(material.double_sided);
        assert_eq!(material.cull_mode, None);
        handles.push((
            entity,
            mesh.0.clone(),
            material.base_color_texture.clone().unwrap(),
        ));
    }
    assert_eq!(handles.len(), expected.len());
    assert_ne!(
        handles[0].2, handles[1].2,
        "same pixels with distinct authored wrap flags need separate images"
    );
    spawned.world.insert_resource(spawned.meshes);
    spawned.world.insert_resource(spawned.materials);
    spawned.world.insert_resource(spawned.particle_materials);
    spawned.world.insert_resource(spawned.images);
    let mut time = Time::<Real>::default();
    time.advance_by(std::time::Duration::from_millis(1200));
    spawned.world.insert_resource(time);
    spawned.world.run_system_once(advance).unwrap();
    let scene = spawned.world.resource::<MainMenuScene>();
    assert_eq!(scene.elapsed.as_millis(), 1200);
    assert_eq!(scene.time_ms, 0.0);
    assert!(scene.particles.as_ref().unwrap().enabled());
    for (entity, handle, _) in &handles {
        assert_eq!(spawned.world.get::<Mesh3d>(*entity).unwrap().0, *handle);
        let mesh = spawned
            .world
            .resource::<Assets<Mesh>>()
            .get(handle)
            .unwrap();
        let VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!("positions")
        };
        assert!(!positions.is_empty());
        assert!(positions.iter().flatten().all(|v| v.is_finite()));
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert_eq!(
            *spawned.world.get::<Visibility>(*entity).unwrap(),
            Visibility::Inherited
        );
    }
    let head = handles
        .iter()
        .find(|(entity, _, _)| {
            order.orders[&bevy::render::sync_world::MainEntity::from(*entity)].ordinal == 0
        })
        .unwrap();
    let before = spawned
        .world
        .resource::<Assets<Mesh>>()
        .get(&head.1)
        .unwrap()
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .clone();
    // This case supplies an external camera pose, independently of an authored CAMS driver.
    spawned
        .world
        .resource_mut::<MainMenuScene>()
        .camera_animation = None;
    spawned
        .world
        .get_mut::<Transform>(camera)
        .unwrap()
        .rotate_local_z(0.37);
    spawned
        .world
        .resource_mut::<Time<Real>>()
        .advance_by(std::time::Duration::ZERO);
    spawned.world.run_system_once(advance).unwrap();
    let after = spawned
        .world
        .resource::<Assets<Mesh>>()
        .get(&head.1)
        .unwrap()
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap();
    assert_ne!(
        &before, after,
        "live source-camera basis must reach particle mesh vertices without advancing births"
    );
    spawned
        .world
        .resource_mut::<Time<Real>>()
        .advance_by(std::time::Duration::from_millis(50));
    spawned.world.run_system_once(advance).unwrap();
    assert_eq!(spawned.world.resource::<MainMenuScene>().time_ms, 50.0);
    assert_eq!(
        (
            spawned.world.resource::<Assets<Mesh>>().len(),
            spawned.world.resource::<Assets<ParticleMaterial>>().len(),
            spawned.world.resource::<Assets<Image>>().len()
        ),
        (3, 3, 3)
    );
    let entities: Vec<_> = spawned
        .world
        .query_filtered::<Entity, With<MainMenuSceneEntity>>()
        .iter(&spawned.world)
        .collect();
    for entity in entities {
        spawned.world.despawn(entity);
    }
    spawned.world.remove_resource::<MainMenuScene>();
    assert!(spawned.world.get_entity(camera).is_err());
    assert!(!spawned.world.contains_resource::<MainMenuScene>());
    assert_eq!(
        spawned
            .world
            .query_filtered::<Entity, With<MainMenuSceneEntity>>()
            .iter(&spawned.world)
            .count(),
        0
    );
}

#[test]
fn missing_emitter_entity_disables_the_whole_owned_batch() {
    let (bytes, textures) = synthetic(|_, _| {});
    let mut spawned = spawn_case(&bytes, &textures);
    assert_eq!(spawned.result.unwrap().particle_emitters, 3);
    let entities: Vec<_> = spawned
        .world
        .query_filtered::<Entity, With<Mesh3d>>()
        .iter(&spawned.world)
        .collect();
    spawned.world.insert_resource(spawned.meshes);
    spawned.world.insert_resource(spawned.materials);
    spawned.world.insert_resource(spawned.particle_materials);
    spawned.world.insert_resource(spawned.images);
    let mut time = Time::<Real>::default();
    time.advance_by(std::time::Duration::from_millis(100));
    spawned.world.insert_resource(time);
    spawned.world.run_system_once(advance).unwrap();
    assert!(entities.iter().all(|entity| {
        *spawned.world.get::<Visibility>(*entity).unwrap() == Visibility::Inherited
    }));
    spawned.world.despawn(entities[0]);
    spawned.world.run_system_once(advance).unwrap();
    assert!(
        !spawned
            .world
            .resource::<MainMenuScene>()
            .particles
            .as_ref()
            .unwrap()
            .enabled(),
        "lost emitter ownership must disable the entire particle batch"
    );
    for entity in &entities[1..] {
        assert_eq!(
            spawned.world.get::<Visibility>(*entity),
            Some(&Visibility::Hidden)
        );
    }
}

#[test]
fn particle_missing_late_image_and_malformed_properties_refuse_before_any_scene_writes() {
    let (bytes, mut textures) = synthetic(|_, _| {});
    textures.pop();
    assert!(
        validate_particle_import("synthetic", &bytes, &textures)
            .unwrap_err()
            .contains("decoded canonical image")
    );
    for (bytes, textures) in [
        (bytes, textures),
        synthetic(|props, _| props.life_span = f32::NAN),
    ] {
        let mut spawned = spawn_case(&bytes, &textures);
        assert!(spawned.result.is_err());
        assert_eq!(
            (
                spawned.meshes.len(),
                spawned.particle_materials.len(),
                spawned.images.len()
            ),
            (0, 0, 0)
        );
        assert!(!spawned.world.contains_resource::<MainMenuScene>());
        assert_eq!(
            spawned
                .world
                .query_filtered::<Entity, With<MainMenuSceneEntity>>()
                .iter(&spawned.world)
                .count(),
            0
        );
    }
}

#[test]
fn particle_unsupported_flags_preserve_geometry_path_without_partial_emitters() {
    let (bytes, textures) = synthetic(|_, flags| *flags |= 1);
    let gap = validate_particle_import("synthetic", &bytes, &textures)
        .unwrap()
        .unwrap();
    assert!(gap.contains("unsupported"));
    let spawned = spawn_case(&bytes, &textures);
    let info = spawned.result.unwrap();
    assert_eq!(info.particle_emitters, 0);
    assert!(info.particle_gap.is_some());
    assert!(
        spawned
            .world
            .resource::<MainMenuScene>()
            .particles
            .is_none()
    );
    assert_eq!(spawned.meshes.len(), 0);
}

#[test]
fn particle_runtime_capacity_failure_disables_the_whole_batch_and_keeps_its_handles() {
    let (bytes, textures) = synthetic(|props, _| props.emission_rate = 1_000_000.0);
    let mut spawned = spawn_case(&bytes, &textures);
    spawned.result.unwrap();
    spawned.world.insert_resource(spawned.meshes);
    spawned.world.insert_resource(spawned.materials);
    spawned.world.insert_resource(spawned.particle_materials);
    let mut time = Time::<Real>::default();
    time.advance_by(std::time::Duration::from_millis(100));
    spawned.world.insert_resource(time);
    spawned.world.run_system_once(advance).unwrap();
    assert!(
        !spawned
            .world
            .resource::<MainMenuScene>()
            .particles
            .as_ref()
            .unwrap()
            .enabled()
    );
    for state in spawned
        .world
        .query_filtered::<&Visibility, With<Mesh3d>>()
        .iter(&spawned.world)
    {
        assert_eq!(*state, Visibility::Hidden);
    }
    spawned.world.run_system_once(advance).unwrap();
    assert_eq!(spawned.world.resource::<Assets<Mesh>>().len(), 3);
}

#[test]
fn particle_image_decode_checks_budget_before_allocating_source_pixels() {
    let mut header = [0_u8; 18];
    header[2] = 2;
    header[12..14].copy_from_slice(&u16::MAX.to_le_bytes());
    header[14..16].copy_from_slice(&u16::MAX.to_le_bytes());
    header[16] = 24;
    let error = pcx_gfx::load_image_bounded(&header, "synthetic oversized TGA", 1024).unwrap_err();
    assert!(error.to_string().contains("RGBA budget"));
}

#[test]
fn particle_visibility_track_retires_and_restarts_one_emitter_with_stable_assets() {
    let (mut bytes, textures) = synthetic(|first, _| first.life_span = 0.02);
    let pre2 = pcx_mdl::Mdlx::parse(&bytes)
        .unwrap()
        .chunk(pcx_mdl::tag::PRE2)
        .unwrap()
        .clone();
    let record_size = u32::from_le_bytes(
        bytes[pre2.payload_offset..pre2.payload_offset + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let mut visibility = pcx_mdl::Particle2TrackKind::V.tag().to_vec();
    visibility.extend(3_u32.to_le_bytes());
    visibility.extend(0_u32.to_le_bytes()); // STEP interpolation, independent clip clock.
    visibility.extend(pcx_mdl::NO_ID.to_le_bytes());
    for (frame, value) in [(13_u32, 1_f32), (63, 0.0), (153, 1.0)] {
        visibility.extend(frame.to_le_bytes());
        visibility.extend(value.to_le_bytes());
    }
    let added = visibility.len();
    bytes[pre2.payload_offset..pre2.payload_offset + 4]
        .copy_from_slice(&u32::try_from(record_size + added).unwrap().to_le_bytes());
    bytes[pre2.offset + 4..pre2.offset + 8]
        .copy_from_slice(&u32::try_from(pre2.size + added).unwrap().to_le_bytes());
    bytes.splice(
        pre2.payload_offset + record_size..pre2.payload_offset + record_size,
        visibility,
    );
    let parsed = pcx_mdl::Mdlx::parse(&bytes)
        .unwrap()
        .particles2(&bytes)
        .unwrap();
    assert_eq!(parsed[0].tracks.len(), 1);
    assert_eq!(
        parsed[0].tracks[0].magic,
        pcx_mdl::Particle2TrackKind::V.tag()
    );
    let mut spawned = spawn_case(&bytes, &textures);
    assert_eq!(spawned.result.unwrap().particle_emitters, 3);
    let camera = spawned
        .world
        .query_filtered::<Entity, With<MainMenuSceneCamera>>()
        .single(&spawned.world)
        .unwrap();
    let order = spawned
        .world
        .get::<crate::menu_particle_order::SourceOrderView>(camera)
        .unwrap()
        .clone();
    let mut handles = spawned
        .world
        .query::<(Entity, &Mesh3d, &MeshMaterial3d<ParticleMaterial>)>()
        .iter(&spawned.world)
        .map(|(entity, mesh, material)| {
            (
                order.orders[&bevy::render::sync_world::MainEntity::from(entity)].ordinal,
                entity,
                mesh.0.clone(),
                material.0.clone(),
            )
        })
        .collect::<Vec<_>>();
    handles.sort_by_key(|entry| entry.0);
    assert_eq!(handles.len(), 3);
    spawned.world.insert_resource(spawned.meshes);
    spawned.world.insert_resource(spawned.materials);
    spawned.world.insert_resource(spawned.particle_materials);
    spawned.world.insert_resource(spawned.images);
    spawned.world.insert_resource(Time::<Real>::default());
    for (delta_ms, elapsed_ms, first_visible) in
        [(40_u64, 40_u128, true), (60, 100, false), (80, 180, true)]
    {
        spawned
            .world
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_millis(delta_ms));
        spawned.world.run_system_once(advance).unwrap();
        let scene = spawned.world.resource::<MainMenuScene>();
        assert!(scene.particles.as_ref().unwrap().enabled());
        assert_eq!(scene.elapsed.as_millis(), elapsed_ms);
        assert_eq!(
            spawned.world.resource::<Assets<Mesh>>().len(),
            handles.len()
        );
        assert_eq!(
            spawned.world.resource::<Assets<ParticleMaterial>>().len(),
            handles.len()
        );
        for (ordinal, entity, mesh_handle, material_handle) in &handles {
            assert_eq!(
                &spawned.world.get::<Mesh3d>(*entity).unwrap().0,
                mesh_handle
            );
            assert_eq!(
                &spawned
                    .world
                    .get::<MeshMaterial3d<ParticleMaterial>>(*entity)
                    .unwrap()
                    .0,
                material_handle
            );
            let mesh = spawned
                .world
                .resource::<Assets<Mesh>>()
                .get(mesh_handle)
                .unwrap();
            let visible = *ordinal != 0 || first_visible;
            assert_eq!(
                mesh.count_vertices() > 0,
                visible,
                "emitter {ordinal} at {elapsed_ms} ms"
            );
            assert_eq!(mesh.indices().unwrap().is_empty(), !visible);
            assert_eq!(
                mesh.asset_usage,
                if visible {
                    bevy::asset::RenderAssetUsages::default()
                } else {
                    bevy::asset::RenderAssetUsages::MAIN_WORLD
                }
            );
            assert_eq!(
                spawned.world.get::<Visibility>(*entity),
                Some(&if visible {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                })
            );
        }
    }
}

#[test]
#[ignore = "requires PEACECRAFT3_INSTALL with original RoC menu assets and selected client"]
fn original_roc_particles_survive_two_authored_loops_with_opaque_geometry() {
    let install = std::path::PathBuf::from(
        std::env::var_os("PEACECRAFT3_INSTALL").expect("explicit original installation"),
    );
    let skin = crate::mainmenu::art_bytes(Some(&install), crate::mainmenu::SKIN_TABLE)
        .expect("source skin");
    let skin = crate::menu_skin::SkinTable::parse(std::str::from_utf8(&skin).unwrap()).unwrap();
    let source = skin_scene_source(&skin, false).unwrap();
    let bytes = crate::mainmenu::art_bytes(Some(&install), &source).expect("source RoC model");
    let model = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let emitters = model.particles2(&bytes).unwrap();
    assert!(!emitters.is_empty());
    let runtime = pcx_gfx::mdx_runtime::RuntimePlan::parse(&bytes).unwrap();
    let depth = runtime
        .materials
        .iter()
        .flat_map(|material| {
            material.layers.iter().map(move |layer| {
                (
                    material.priority_plane,
                    alpha_mode_of(layer.properties.filter_mode).0,
                )
            })
        })
        .filter(|(_, mode)| matches!(mode, AlphaMode::Opaque | AlphaMode::Mask(_)))
        .map(|(plane, _)| plane)
        .max()
        .unwrap();
    let translucent = runtime
        .materials
        .iter()
        .flat_map(|material| {
            material.layers.iter().map(move |layer| {
                (
                    material.priority_plane,
                    alpha_mode_of(layer.properties.filter_mode).0,
                )
            })
        })
        .filter(|(_, mode)| !matches!(mode, AlphaMode::Opaque | AlphaMode::Mask(_)))
        .map(|(plane, _)| plane)
        .chain(
            emitters
                .iter()
                .map(|emitter| emitter.properties().priority_plane),
        )
        .min()
        .unwrap();
    assert!(
        translucent < depth,
        "exercise cross-pass authored priorities"
    );
    let textures = load_textures_in(Some(&install), &bytes, scene_source(false).2);
    let fog = crate::mainmenu::art_bytes(Some(&install), crate::mainmenu::MENU_FOG_CONFIG)
        .expect("source fog");
    let fog = crate::menu_skin::SkinTable::parse(std::str::from_utf8(&fog).unwrap()).unwrap();
    let fog = crate::menu_fog::MenuFog::checked(&fog, 0).unwrap();
    let parameters = crate::menu_lighting::parameters(Some(&install)).unwrap();
    let mut world = World::new();
    let mut meshes = Assets::default();
    let mut materials = Assets::default();
    let mut particles = Assets::default();
    let mut images = Assets::default();
    let info = spawn(
        &mut world.commands(),
        GeosetAssets {
            meshes: &mut meshes,
            materials: &mut materials,
            images: &mut images,
        },
        &mut particles,
        &source,
        &bytes,
        &textures,
        (fog, parameters, Entrance::Idle),
    )
    .unwrap();
    world.flush();
    assert_eq!(info.particle_emitters, emitters.len());
    assert!(info.particle_gap.is_none());
    world.insert_resource(meshes);
    world.insert_resource(materials);
    world.insert_resource(particles);
    let clip = model
        .sequences(&bytes)
        .unwrap()
        .into_iter()
        .find(|sequence| sequence.name == "Stand" && !sequence.non_looping)
        .expect("authored entry clip");
    let step = std::time::Duration::from_millis(16);
    let duration =
        std::time::Duration::from_millis(u64::from(clip.interval_end - clip.interval_start) * 2);
    let frames = duration.as_nanos().div_ceil(step.as_nanos());
    assert!(frames <= 10000, "bounded original-input test");
    world.init_resource::<Time<Real>>();
    for _ in 0..frames {
        world.resource_mut::<Time<Real>>().advance_by(step);
        world.run_system_once(advance).unwrap();
    }
    assert!(
        world
            .resource::<MainMenuScene>()
            .particles
            .as_ref()
            .unwrap()
            .enabled()
    );
    let order = world
        .query::<&crate::menu_particle_order::SourceOrderView>()
        .single(&world)
        .unwrap()
        .clone();
    let mut draw_query = world.query::<(Entity, &Mesh3d)>();
    let meshes = world.resource::<Assets<Mesh>>();
    assert!(draw_query.iter(&world).any(|(entity, mesh)| {
        order
            .orders
            .get(&bevy::render::sync_world::MainEntity::from(entity))
            .is_some_and(|entry| entry.kind == crate::menu_particle_order::SourceKind::Particle)
            && meshes.get(&mesh.0).unwrap().count_vertices() > 0
    }));
}
