use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};

#[test]
fn static_geosets_release_cpu_vertices_but_bound_geosets_remain_editable() {
    let mut source = pcx_gfx::LoadedMesh {
        positions: vec![[1.0, 2.0, 3.0]],
        normals: vec![[0.0, 0.0, 1.0]],
        tex_coords: vec![[0.0, 0.0]],
        indices: vec![0],
        vertex_groups: vec![0],
        bone_indices: vec![],
        group_counts: vec![],
        geoset: 0,
        material_id: 0,
    };
    let static_mesh = pcx_gfx::to_bevy_mesh(&source);
    assert_eq!(static_mesh.asset_usage, RenderAssetUsages::RENDER_WORLD);
    assert_eq!(
        static_mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap(),
        source.positions
    );
    source.bone_indices.push(9);
    source.group_counts.push(1);
    let dynamic_mesh = pcx_gfx::to_bevy_mesh(&source);
    assert!(
        dynamic_mesh
            .asset_usage
            .contains(RenderAssetUsages::MAIN_WORLD)
    );
    assert!(
        dynamic_mesh
            .asset_usage
            .contains(RenderAssetUsages::RENDER_WORLD)
    );
}

#[test]
fn identical_positions_produce_no_modified_events_but_bit_changes_do() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let positions = vec![[0., 0., 0.]; 3];
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
    let handle = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
    app.update();
    app.world_mut()
        .resource_mut::<Messages<AssetEvent<Mesh>>>()
        .clear();
    let mut previous_updates = 0;
    for _ in 0..100 {
        app.world_mut()
            .resource_mut::<Assets<Mesh>>()
            .get_mut(&handle)
            .unwrap()
            .insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
        app.update();
        previous_updates += app
            .world_mut()
            .resource_mut::<Messages<AssetEvent<Mesh>>>()
            .drain()
            .filter(|e| matches!(e, AssetEvent::Modified { .. }))
            .count();
    }
    assert_eq!(previous_updates, 100);
    eprintln!("identical-position modified events: previous={previous_updates}, current=0");
    for _ in 0..100 {
        assert!(!pcx_gfx::set_mesh_positions(
            &mut app.world_mut().resource_mut(),
            &handle,
            positions.clone()
        ));
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<AssetEvent<Mesh>>>()
                .drain()
                .filter(|e| matches!(e, AssetEvent::Modified { .. }))
                .count(),
            0
        );
    }
    let mut changed = positions;
    changed[0][0] = -0.0;
    assert!(pcx_gfx::set_mesh_positions(
        &mut app.world_mut().resource_mut(),
        &handle,
        changed.clone()
    ));
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<Mesh>>>()
            .drain()
            .filter(|e| matches!(e, AssetEvent::Modified { .. }))
            .count(),
        1
    );
    let meshes = app.world().resource::<Assets<Mesh>>();
    let actual = meshes
        .get(&handle)
        .unwrap()
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .unwrap()
        .as_float3()
        .unwrap();
    assert_eq!(actual[0][0].to_bits(), changed[0][0].to_bits());
}
