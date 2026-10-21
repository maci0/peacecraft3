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
pub(super) struct DebugText;

#[derive(SystemParam)]
pub(super) struct Sources<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    entities: Query<'w, 's, ()>,
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
    mut counters: Query<&mut Text, With<DebugText>>,
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
    let mut value = String::from("Debug (F7)\n");
    if let Some(frames) = sources.frames {
        let _ = writeln!(value, "Frame {} | uptime {now:.1} s", frames.0);
    }
    if let Ok(window) = sources.windows.single() {
        let _ = writeln!(
            value,
            "Display: {}x{} px | scale {:.2}",
            window.physical_width(),
            window.physical_height(),
            window.scale_factor()
        );
    }
    if let Some(adapter) = sources.adapter {
        let _ = writeln!(
            value,
            "GPU: {}\nBackend: {:?} | {}",
            adapter.name, adapter.backend, adapter.driver
        );
        match memory::vram(&adapter.device_pci_bus_id) {
            Some((used, total)) => {
                let _ = writeln!(
                    value,
                    "VRAM (whole GPU): {:.0} / {:.0} MiB ({:.1}%)",
                    used as f64 / (1024.0 * 1024.0),
                    total as f64 / (1024.0 * 1024.0),
                    used as f64 / total as f64 * 100.0
                );
            }
            None => value.push_str("Whole-GPU VRAM report unavailable\n"),
        }
    }
    let _ = writeln!(value, "Entities: {}", sources.entities.iter().len());
    if let Some(meshes) = sources.meshes {
        let mut retained = 0;
        let mut vertices = 0;
        for (_, mesh) in meshes.iter() {
            if let Ok(attributes) = mesh.try_attributes() {
                retained += 1;
                vertices += attributes
                    .map(|(_, values)| values.len())
                    .min()
                    .unwrap_or(0);
            }
        }
        let _ = writeln!(
            value,
            "CPU-retained meshes: {} | vertices: {vertices}",
            retained
        );
    }
    if let Some(images) = sources.images {
        let _ = writeln!(value, "Image assets: {}", images.len());
    }
    match sources
        .device
        .and_then(|device| device.wgpu_device().generate_allocator_report())
    {
        Some(report) => {
            let _ = writeln!(
                value,
                "GPU allocations: {:.1} MiB used\nGPU blocks: {:.1} MiB reserved\nDevice allocator, not total VRAM usage",
                report.total_allocated_bytes as f64 / (1024.0 * 1024.0),
                report.total_reserved_bytes as f64 / (1024.0 * 1024.0)
            );
        }
        None => value.push_str("GPU memory report unavailable on this backend\n"),
    }
    value.push_str(&debug.details);
    for mut counter in &mut counters {
        counter.0.clone_from(&value);
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
        let text = world.spawn((DebugText, Text::new(""))).id();
        world.run_system_once(refresh).unwrap();
        let output = world.get::<Text>(text).unwrap().0.clone();
        assert!(output.contains("vertices: 3"), "{output}");
        assert!(output.contains("CPU-retained meshes: 1"), "{output}");
        assert!(output.contains("GPU memory report unavailable"));
        assert!(output.contains("Tick 12"));
        world.resource_mut::<DebugOverlay>().visible = false;
        world.resource_mut::<DebugOverlay>().details = "hidden update".into();
        world.run_system_once(refresh).unwrap();
        assert_eq!(world.get::<Text>(text).unwrap().0, output);
    }
}
