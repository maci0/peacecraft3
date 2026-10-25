use super::*;
use bevy::{
    app::SubApp,
    asset::{AssetEvent, AssetPlugin},
    render::{
        ExtractSchedule, MainWorld, RenderApp,
        mesh::RenderMesh,
        render_asset::{ExtractedAssets, RenderAssetPlugin},
    },
};
fn quad() -> draw::Quad {
    draw::Quad {
        piece: draw::Piece::Head,
        positions: [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
        uv: [[0., 0.], [1., 0.], [1., 1.], [0., 1.]],
        rgba: [1.; 4],
    }
}
fn populated() -> Mesh {
    let mut buffer = Buffer::default();
    buffer.reserve(1).unwrap();
    buffer.push(quad()).unwrap();
    buffer.into_mesh()
}
#[test]
fn born_and_retired_empty_meshes_keep_cpu_data_without_gpu_usage() {
    for mesh in [empty_mesh(), Buffer::default().into_mesh()] {
        assert_eq!(mesh.asset_usage, RenderAssetUsages::MAIN_WORLD);
        assert_eq!(mesh.count_vertices(), 0);
        assert_eq!(mesh.indices().unwrap().len(), 0);
    }
    let mesh = populated();
    assert_eq!(mesh.count_vertices(), 4);
    assert_eq!(mesh.indices().unwrap().len(), 6);
    assert_eq!(
        mesh.asset_usage,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD
    );
}
#[test]
fn actual_bevy_mesh_extraction_skips_empty_then_reextracts_same_handle_when_populated() {
    let mut app = App::new();
    app.add_plugins(AssetPlugin::default());
    app.init_asset::<Mesh>();
    app.insert_sub_app(RenderApp, SubApp::new());
    app.add_plugins(RenderAssetPlugin::<RenderMesh>::default());
    let handle = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(empty_mesh());
    let id = handle.id();
    app.world_mut()
        .resource_mut::<Messages<AssetEvent<Mesh>>>()
        .write(AssetEvent::Added { id });
    let mut main = MainWorld::default();
    *main = std::mem::take(app.world_mut());
    let render = app.sub_app_mut(RenderApp);
    render.world_mut().insert_resource(main);
    render.world_mut().run_schedule(ExtractSchedule);
    assert!(
        render
            .world()
            .resource::<ExtractedAssets<RenderMesh>>()
            .extracted
            .is_empty()
    );
    for (mesh, expected_vertices) in [
        (populated(), Some(4)),
        (Buffer::default().into_mesh(), None),
        (populated(), Some(4)),
    ] {
        {
            let mut main = render.world_mut().resource_mut::<MainWorld>();
            *main
                .resource_mut::<Assets<Mesh>>()
                .get_mut(&handle)
                .unwrap() = mesh;
            main.resource_mut::<Messages<AssetEvent<Mesh>>>()
                .write(AssetEvent::Modified { id });
        }
        render.world_mut().run_schedule(ExtractSchedule);
        let extracted = render.world().resource::<ExtractedAssets<RenderMesh>>();
        assert!(extracted.modified.contains(&id));
        assert_eq!(
            extracted.extracted.len(),
            usize::from(expected_vertices.is_some())
        );
        if let Some(vertices) = expected_vertices {
            assert_eq!(extracted.extracted[0].0, id);
            assert_eq!(extracted.extracted[0].1.count_vertices(), vertices);
        }
        let main = render.world().resource::<MainWorld>();
        assert!(main.resource::<Assets<Mesh>>().get(&handle).is_some());
        assert_eq!(main.resource::<Assets<Mesh>>().len(), 1);
    }
}

#[test]
fn source_depth_flag_sorts_only_its_emitter_and_preserves_other_source_order() {
    let (bytes, _) = super::super::particle_tests::synthetic(|_, flags| *flags |= cpu::SORT_FAR_Z);
    let model = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let plan = cpu::Plan::checked(
        &model.particles2(&bytes).unwrap(),
        &model.textures(&bytes).unwrap(),
        &model.pivots(&bytes).unwrap(),
        &[],
        cpu::Clip {
            interval: [13, 213],
            looping: true,
        },
        "synthetic:depth-sort",
        Policy::default().cpu,
    )
    .unwrap();
    let piece = |z: f32| {
        let mut q = quad();
        for p in &mut q.positions {
            p[2] = z;
        }
        q
    };
    let mut pieces = vec![
        (0, piece(17.)),
        (1, piece(5.)),
        (0, piece(-13.)),
        (1, piece(-8.)),
        (0, piece(3.)),
    ];
    sort_pieces(&mut pieces, plan.profiles(), Vec3::Z);
    assert_eq!(
        pieces
            .iter()
            .map(|(i, q)| (*i, q.positions[0][2]))
            .collect::<Vec<_>>(),
        vec![(0, -13.), (0, 3.), (0, 17.), (1, 5.), (1, -8.)]
    );
    sort_pieces(&mut pieces, plan.profiles(), -Vec3::Z);
    assert_eq!(
        pieces
            .iter()
            .take(3)
            .map(|(_, q)| q.positions[0][2])
            .collect::<Vec<_>>(),
        vec![17., 3., -13.]
    );
}
