use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};

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
