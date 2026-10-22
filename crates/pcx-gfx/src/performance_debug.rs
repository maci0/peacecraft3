use std::fmt::Write;

use bevy::{
    diagnostic::FrameCount,
    ecs::system::SystemParam,
    prelude::*,
    render::renderer::{RenderAdapterInfo, RenderDevice},
    window::PrimaryWindow,
};

#[path = "performance_memory.rs"]
mod memory;

/// F7 state and optional game-specific diagnostic text. No game state is changed.
#[derive(Resource, Default)]
pub struct DebugOverlay {
    pub visible: bool,
    pub details: String,
}

#[derive(Component)]
pub(super) struct DebugText(pub(super) usize);

pub(super) const SECTIONS: [&str; 7] = [
    "DISPLAY",
    "RENDERER",
    "SCENE",
    "RETAINED GEOMETRY | CPU",
    "TEXTURES",
    "DEVICE MEMORY",
    "CAMPAIGN",
];

#[derive(SystemParam)]
pub(super) struct Sources<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    entities: Query<'w, 's, ()>,
    mesh_instances: Query<'w, 's, Option<&'static ViewVisibility>, With<Mesh3d>>,
    meshes: Option<Res<'w, Assets<Mesh>>>,
    images: Option<Res<'w, Assets<Image>>>,
    adapter: Option<Res<'w, RenderAdapterInfo>>,
    device: Option<Res<'w, RenderDevice>>,
    frames: Option<Res<'w, FrameCount>>,
}

pub(super) fn refresh(
    time: Res<Time<Real>>,
    debug: Res<DebugOverlay>,
    sources: Sources,
    mut counters: Query<(&DebugText, &mut TextSpan)>,
    mut last: Local<f64>,
) {
    if !debug.visible {
        return;
    }
    let now = time.elapsed_secs_f64();
    if now - *last < 0.25 && !debug.is_changed() {
        return;
    }
    *last = now;
    let mut sections: [String; 7] = std::array::from_fn(|_| String::new());
    let mut value = &mut sections[0];
    if let Some(frames) = sources.frames {
        let _ = writeln!(
            value,
            "Frame            {:>10}\nUptime           {now:>10.1} s",
            frames.0
        );
    }
    if let Ok(window) = sources.windows.single() {
        let _ = writeln!(
            value,
            "Display          {}x{} px\nLogical          {:.0}x{:.0} px\nUI scale         {:>10.2}\nPresent mode     {:?}",
            window.physical_width(),
            window.physical_height(),
            window.width(),
            window.height(),
            window.scale_factor(),
            window.present_mode
        );
    }
    value = &mut sections[1];
    if let Some(adapter) = sources.adapter {
        let _ = writeln!(
            value,
            "GPU  {}\nBackend  {:?} | {:?}\nDriver  {}",
            adapter.name, adapter.backend, adapter.device_type, adapter.driver
        );
        match memory::vram(&adapter.device_pci_bus_id) {
            Some((used, total)) => {
                let _ = writeln!(
                    value,
                    "VRAM used        {:>10.0} MiB\nVRAM total       {:>10.0} MiB\nVRAM occupancy   {:>10.1}%\nWhole GPU | includes other apps",
                    used as f64 / (1024.0 * 1024.0),
                    total as f64 / (1024.0 * 1024.0),
                    used as f64 / total as f64 * 100.0
                );
            }
            None => value.push_str("Whole-GPU VRAM report unavailable\n"),
        }
    }
    value = &mut sections[2];
    let _ = writeln!(
        value,
        "Entities         {:>10}\nMesh instances   {:>10}\nView-visible     {:>10}",
        sources.entities.iter().len(),
        sources.mesh_instances.iter().len(),
        sources
            .mesh_instances
            .iter()
            .filter(|visibility| visibility.is_some_and(|visibility| visibility.get()))
            .count(),
    );
    value = &mut sections[3];
    if let Some(meshes) = sources.meshes {
        let mut retained = 0;
        let mut upload_sources = 0;
        let mut released = 0;
        let mut vertices = 0;
        let mut indices = 0;
        let mut triangles = 0;
        let mut bytes = 0;
        for (_, mesh) in meshes.iter() {
            if mesh
                .asset_usage
                .contains(bevy::asset::RenderAssetUsages::RENDER_WORLD)
            {
                upload_sources += 1;
            }
            if let Ok(attributes) = mesh.try_attributes() {
                retained += 1;
                let vertex_count = attributes
                    .map(|(_, values)| {
                        bytes += values.get_bytes().len();
                        values.len()
                    })
                    .min()
                    .unwrap_or(0);
                vertices += vertex_count;
                if mesh.primitive_topology() == bevy::mesh::PrimitiveTopology::TriangleList {
                    triangles += mesh.indices().map_or(vertex_count, |indices| indices.len()) / 3;
                }
                indices += mesh.indices().map_or(0, |indices| indices.len());
                bytes += mesh.indices().map_or(0, |indices| {
                    indices.len()
                        * match indices {
                            bevy::mesh::Indices::U16(_) => 2,
                            bevy::mesh::Indices::U32(_) => 4,
                        }
                });
            } else {
                released += 1;
            }
        }
        let _ = writeln!(
            value,
            "Mesh assets      {:>10}\nRender eligible  {upload_sources:>10}\nCPU released     {released:>10}\nRetained meshes  {retained:>10}\nVertices         {vertices:>10}\nList triangles   {triangles:>10}\nIndices          {indices:>10}\nBuffers          {:>10.2} MiB\nSkinning / updates may need CPU copies\nCounts are assets, not GPU draw totals",
            meshes.len(),
            bytes as f64 / (1024.0 * 1024.0),
        );
    }
    value = &mut sections[4];
    if let Some(images) = sources.images {
        let bytes: usize = images
            .iter()
            .filter_map(|(_, image)| image.data.as_ref())
            .map(Vec::len)
            .sum();
        let _ = writeln!(
            value,
            "Image assets     {:>10}\nCPU image data   {:>10.2} MiB",
            images.len(),
            bytes as f64 / (1024.0 * 1024.0)
        );
    }
    value = &mut sections[5];
    match sources
        .device
        .and_then(|device| device.wgpu_device().generate_allocator_report())
    {
        Some(report) => {
            let _ = writeln!(
                value,
                "Allocated        {:>10.1} MiB\nReserved         {:>10.1} MiB\nDevice allocator | not whole-GPU VRAM",
                report.total_allocated_bytes as f64 / (1024.0 * 1024.0),
                report.total_reserved_bytes as f64 / (1024.0 * 1024.0)
            );
        }
        None => value.push_str("GPU memory report unavailable on this backend\n"),
    }
    value = &mut sections[6];
    if !debug.details.is_empty() {
        value.push_str(&debug.details);
    }
    for (section, mut counter) in &mut counters {
        let text = sections[section.0].trim_end();
        counter.0.clear();
        counter
            .0
            .push_str(if text.is_empty() { "Unavailable" } else { text });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::RenderAssetUsages, ecs::system::RunSystemOnce, mesh::PrimitiveTopology};

    #[test]
    fn reports_vertices_and_unavailable_memory_without_work_when_hidden() {
        let mut world = World::new();
        world.init_resource::<Time<Real>>();
        world.insert_resource(DebugOverlay {
            visible: true,
            details: "Tick 12".into(),
        });
        let mut meshes = Assets::<Mesh>::default();
        let _mesh = meshes.add(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0_f32; 3]; 3]),
        );
        let mut extracted = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0_f32; 3]; 6]);
        let _uploaded = extracted.take_gpu_data().unwrap();
        let _extracted_handle = meshes.add(extracted);
        world.insert_resource(meshes);
        let texts: Vec<_> = (0..SECTIONS.len())
            .map(|index| world.spawn((DebugText(index), TextSpan::new(""))).id())
            .collect();
        world.run_system_once(refresh).unwrap();
        let output = texts
            .iter()
            .map(|&id| world.get::<TextSpan>(id).unwrap().0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(output.contains("Vertices                  3"), "{output}");
        assert!(output.contains("Retained meshes           1"), "{output}");
        assert!(output.contains("Mesh assets               2"), "{output}");
        assert!(output.contains("Indices                   0"), "{output}");
        assert!(output.contains("List triangles            1"), "{output}");
        assert!(output.contains("Render eligible           1"), "{output}");
        assert!(output.contains("CPU released              1"), "{output}");
        assert!(output.contains("Counts are assets, not GPU draw totals"));
        assert!(output.contains("GPU memory report unavailable"));
        assert!(output.contains("Tick 12"));
        world.resource_mut::<DebugOverlay>().visible = false;
        world.resource_mut::<DebugOverlay>().details = "hidden update".into();
        world.run_system_once(refresh).unwrap();
        let hidden = texts
            .iter()
            .map(|&id| world.get::<TextSpan>(id).unwrap().0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(hidden, output);
    }
}
