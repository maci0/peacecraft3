//! Fixed-work render CPU workloads (`render-cpu-v1`) for the mounted campaign.
//!
//! Every phase runs a fixed number of `App::update` calls at the manual
//! 1/64 s time step from campaign seed 1. Counters are work counts, never
//! elapsed time: allocation calls and requested bytes inside `App::update`,
//! ECS system runs, entities spawned and despawned, asset adds, modifications
//! and removals, mesh vertices and image bytes written, and a digest of the
//! final rendered frame. `docs/performance/render-cpu.md` has the commands.
use super::{STEP_SECONDS, capture, step};
use crate::counting_allocator::totals;
use bevy::{
    asset::{AssetEvent, UntypedAssetId},
    camera::visibility::ViewVisibility,
    ecs::{
        message::{MessageCursor, Messages},
        schedule::Schedules,
    },
    pbr::{RenderMaterialInstances, RenderMeshInstances, StandardMaterial},
    prelude::*,
    render::{RenderApp, render_resource::PollType, renderer::RenderDevice},
};
mod budget;
mod spans;

use pcx_play::map_startup_host::MapStartupHost;
use peacecraft3::map_script_session::MapScriptSession;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::{Read, Write},
    path::Path,
    time::Duration,
};

/// Workload identity recorded in every summary; bump it when a phase changes.
pub(super) const WORKLOAD: &str = "render-cpu-v1";
/// Updates per phase. Work volume, not a tuned threshold.
const FRAMES: usize = 256;
/// Spans listed per phase in the summary.
const SPAN_REPORT_ROWS: usize = 80;
/// Updates between scripted HUD events in the `campaign_hud` phase.
const HUD_EVENT_PERIOD: usize = 32;
/// On-screen seconds of each scripted message.
const HUD_MESSAGE_SECONDS: f32 = 3.0;

/// Event counts and written payload for one asset type.
#[derive(Default, Clone, Copy, serde::Serialize)]
struct AssetCounts {
    added: u64,
    modified: u64,
    removed: u64,
    /// Mesh vertices or image bytes carried by added and modified assets.
    payload: u64,
    /// Mesh vertex and index buffer bytes (meshes) or image bytes (images) of the same assets.
    payload_bytes: u64,
    live: u64,
    /// Modified assets whose mesh entity was not in view that frame (meshes only).
    modified_unseen: u64,
    /// Of those, meshes whose own `Visibility` is `Hidden` (geoset alpha 0).
    modified_unseen_hidden: u64,
    /// Of those, meshes visible by inheritance that the frustum test rejected.
    modified_unseen_culled: u64,
}

impl AssetCounts {
    fn add(&mut self, [units, bytes]: [u64; 2]) {
        self.payload += units;
        self.payload_bytes += bytes;
    }
}

impl std::ops::AddAssign for AssetCounts {
    fn add_assign(&mut self, other: Self) {
        self.added += other.added;
        self.modified += other.modified;
        self.removed += other.removed;
        self.payload += other.payload;
        self.payload_bytes += other.payload_bytes;
        self.modified_unseen += other.modified_unseen;
        self.modified_unseen_hidden += other.modified_unseen_hidden;
        self.modified_unseen_culled += other.modified_unseen_culled;
        self.live = other.live;
    }
}

/// Counts for one frame and the ids of the assets modified in it.
type AssetProbe = Box<dyn FnMut(&World) -> (AssetCounts, Vec<UntypedAssetId>)>;

fn probe_asset<A: Asset>(
    world: &World,
    payload: impl Fn(&A) -> [u64; 2] + 'static,
) -> Option<AssetProbe> {
    let mut cursor: MessageCursor<AssetEvent<A>> = world
        .get_resource::<Messages<AssetEvent<A>>>()?
        .get_cursor_current();
    Some(Box::new(move |world| {
        let (Some(events), Some(assets)) = (
            world.get_resource::<Messages<AssetEvent<A>>>(),
            world.get_resource::<Assets<A>>(),
        ) else {
            return (AssetCounts::default(), Vec::new());
        };
        let mut modified = Vec::new();
        let mut counts = AssetCounts {
            live: assets.len() as u64,
            ..default()
        };
        for event in cursor.read(events) {
            match event {
                AssetEvent::Added { id } => {
                    counts.added += 1;
                    counts.add(assets.get(*id).map_or([0; 2], &payload));
                }
                AssetEvent::Modified { id } => {
                    counts.modified += 1;
                    modified.push(id.untyped());
                    counts.add(assets.get(*id).map_or([0; 2], &payload));
                }
                AssetEvent::Removed { .. } => counts.removed += 1,
                AssetEvent::Unused { .. } | AssetEvent::LoadedWithDependencies { .. } => {}
            }
        }
        (counts, modified)
    }))
}

/// Per-frame counters, plus the running phase totals.
#[derive(Default, Clone, serde::Serialize)]
struct Frame {
    allocation_calls: u64,
    reallocation_calls: u64,
    requested_bytes: u64,
    entities_spawned: u64,
    entities_despawned: u64,
    systems_ran: u64,
    live_entities: u64,
    render_world_entities: u64,
    mesh_entities: u64,
    visible_mesh_entities: u64,
    ui_nodes: u64,
    text_entities: u64,
    /// Mesh entities whose `Visibility` is `Hidden`.
    hidden_mesh_entities: u64,
    /// Distinct mesh and material assets among the visible mesh entities.
    visible_distinct_meshes: u64,
    visible_distinct_materials: u64,
    /// Entries of the render world's mesh and material instance tables after the frame.
    render_mesh_instances: u64,
    render_material_instances: u64,
    /// Renderer submission counters from `bevy_render::render_stats` during the update.
    draw_calls: u64,
    multi_draw_calls: u64,
    pipelines_set: u64,
    bind_groups_set: u64,
    bind_groups_created: u64,
    buffers_created: u64,
    buffer_bytes_created: u64,
    textures_created: u64,
    assets: BTreeMap<&'static str, AssetCounts>,
}

struct Probe {
    assets: Vec<(&'static str, AssetProbe)>,
    live: HashSet<Entity>,
    /// `(schedule, system key, name)` to the change tick of its latest run.
    systems: HashMap<String, u32>,
}

fn system_ticks(world: &World, prefix: &str, into: &mut HashMap<String, u32>) {
    let Some(schedules) = world.get_resource::<Schedules>() else {
        return;
    };
    for (label, schedule) in schedules.iter() {
        let Ok(systems) = schedule.systems() else {
            continue;
        };
        for (key, system) in systems {
            into.insert(
                format!("{prefix}{label:?}::{}#{key:?}", system.name()),
                system.get_last_run().get(),
            );
        }
    }
}

fn live_entities(world: &World) -> HashSet<Entity> {
    world.iter_entities().map(|entity| entity.id()).collect()
}

impl Probe {
    fn new(app: &App) -> Self {
        let world = app.world();
        let mut assets = Vec::new();
        macro_rules! track {
            ($name:literal, $ty:ty, $payload:expr) => {
                if let Some(probe) = probe_asset::<$ty>(world, $payload) {
                    assets.push(($name, probe));
                }
            };
        }
        track!("mesh", Mesh, |mesh: &Mesh| [
            mesh.count_vertices() as u64,
            (mesh.get_vertex_buffer_size() + mesh.get_index_buffer_bytes().map_or(0, <[u8]>::len))
                as u64,
        ]);
        track!("image", Image, |image: &Image| {
            let bytes = image.data.as_ref().map_or(0, |data| data.len() as u64);
            [bytes, bytes]
        });
        track!("standard_material", StandardMaterial, |_| [0; 2]);
        track!("menu_material", crate::menu_material::MenuMaterial, |_| [0;
            2]);
        track!(
            "particle_material",
            crate::mainmenu_scene::ParticleMaterial,
            |_| [0; 2]
        );
        track!("ground_material", pcx_gfx::GroundMaterial, |_| [0; 2]);
        track!(
            "filter_material",
            peacecraft3::campaign_presentation::FilterMaterial,
            |_| [0; 2]
        );
        track!("additive_image", crate::glue_menu::AdditiveImage, |_| [0;
            2]);
        track!("backdrop_image", crate::glue_menu::BackdropImage, |_| [0;
            2]);
        let mut systems = HashMap::new();
        system_ticks(world, "", &mut systems);
        if let Some(render) = app.get_sub_app(RenderApp) {
            system_ticks(render.world(), "render:", &mut systems);
        }
        Self {
            assets,
            live: live_entities(world),
            systems,
        }
    }

    /// Updates once with only the update itself inside the counted window.
    fn frame(&mut self, app: &mut App, ran: &mut BTreeMap<String, u64>) -> Frame {
        let before = totals();
        let stats_before = bevy::render::render_stats::snapshot();
        app.update();
        let allocated = totals() - before;
        let submitted = bevy::render::render_stats::snapshot() - stats_before;
        app.world()
            .resource::<RenderDevice>()
            .wgpu_device()
            .poll(PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(20)),
            })
            .expect("GPU completion");
        let mut frame = Frame {
            allocation_calls: allocated.calls,
            reallocation_calls: allocated.reallocations,
            requested_bytes: allocated.bytes,
            draw_calls: submitted.draw_calls,
            multi_draw_calls: submitted.multi_draw_calls,
            pipelines_set: submitted.pipelines_set,
            bind_groups_set: submitted.bind_groups_set,
            bind_groups_created: submitted.bind_groups_created,
            buffers_created: submitted.buffers_created,
            buffer_bytes_created: submitted.buffer_bytes_created,
            textures_created: submitted.textures_created,
            ..default()
        };
        let world = app.world();
        let after = live_entities(world);
        frame.entities_spawned = after.difference(&self.live).count() as u64;
        frame.entities_despawned = self.live.difference(&after).count() as u64;
        frame.live_entities = after.len() as u64;
        self.live = after;
        let mut ticks = HashMap::new();
        system_ticks(world, "", &mut ticks);
        if let Some(render) = app.get_sub_app(RenderApp) {
            frame.render_world_entities = render.world().iter_entities().count() as u64;
            frame.render_mesh_instances = match render.world().resource::<RenderMeshInstances>() {
                RenderMeshInstances::CpuBuilding(instances) => instances.len(),
                RenderMeshInstances::GpuBuilding(instances) => instances.len(),
            } as u64;
            frame.render_material_instances = render
                .world()
                .resource::<RenderMaterialInstances>()
                .instances
                .len() as u64;
            system_ticks(render.world(), "render:", &mut ticks);
        }
        for (name, tick) in &ticks {
            if self.systems.get(name) != Some(tick) {
                frame.systems_ran += 1;
                *ran.entry(name.clone()).or_default() += 1;
            }
        }
        self.systems = ticks;
        let mut modified_meshes = Vec::new();
        for (name, probe) in &mut self.assets {
            let (counts, modified) = probe(app.world());
            if *name == "mesh" {
                modified_meshes = modified;
            }
            frame.assets.insert(name, counts);
        }
        let world = app.world_mut();
        frame.mesh_entities = world.query::<&Mesh3d>().iter(world).count() as u64;
        frame.visible_mesh_entities = world
            .query_filtered::<&ViewVisibility, With<Mesh3d>>()
            .iter(world)
            .filter(|visible| visible.get())
            .count() as u64;
        frame.hidden_mesh_entities = world
            .query_filtered::<&Visibility, With<Mesh3d>>()
            .iter(world)
            .filter(|visibility| **visibility == Visibility::Hidden)
            .count() as u64;
        let (mut meshes, mut materials) = (HashSet::new(), HashSet::new());
        for (mesh, menu, standard, ground, visible) in world
            .query::<(
                &Mesh3d,
                Option<&MeshMaterial3d<crate::menu_material::MenuMaterial>>,
                Option<&MeshMaterial3d<StandardMaterial>>,
                Option<&MeshMaterial3d<pcx_gfx::GroundMaterial>>,
                &ViewVisibility,
            )>()
            .iter(world)
        {
            if !visible.get() {
                continue;
            }
            meshes.insert(mesh.0.id());
            materials.extend(
                menu.map(|m| m.0.id().untyped())
                    .or(standard.map(|m| m.0.id().untyped()))
                    .or(ground.map(|m| m.0.id().untyped())),
            );
        }
        let mut hidden_meshes = HashSet::new();
        let mut culled_meshes = HashSet::new();
        for (mesh, visibility, inherited, visible) in world
            .query::<(&Mesh3d, &Visibility, &InheritedVisibility, &ViewVisibility)>()
            .iter(world)
        {
            if visible.get() {
                continue;
            }
            if *visibility == Visibility::Hidden {
                hidden_meshes.insert(mesh.0.id());
            } else if inherited.get() {
                culled_meshes.insert(mesh.0.id());
            }
        }
        frame.visible_distinct_meshes = meshes.len() as u64;
        if let Some(counts) = frame.assets.get_mut("mesh") {
            let unseen: Vec<_> = modified_meshes
                .iter()
                .map(|id| id.typed_unchecked::<Mesh>())
                .filter(|id| !meshes.contains(id))
                .collect();
            counts.modified_unseen = unseen.len() as u64;
            counts.modified_unseen_hidden = unseen
                .iter()
                .filter(|id| hidden_meshes.contains(id))
                .count() as u64;
            counts.modified_unseen_culled = unseen
                .iter()
                .filter(|id| culled_meshes.contains(id))
                .count() as u64;
        }
        frame.visible_distinct_materials = materials.len() as u64;
        frame.ui_nodes = world.query::<&Node>().iter(world).count() as u64;
        frame.text_entities = world.query::<&Text>().iter(world).count() as u64;
        frame
    }
}

#[derive(serde::Serialize)]
struct Phase {
    name: &'static str,
    frames: usize,
    totals: Frame,
    /// Frames in which each system ran at least once.
    system_frames: BTreeMap<String, u64>,
    /// SHA-256 of the captured RGBA frame after the phase.
    frame_sha256: String,
    game_time: Option<f64>,
    /// Exclusive per-span allocation counters (needs `profile-cpu`), largest first.
    spans: Vec<(String, spans::SpanStats)>,
}

impl Phase {
    /// The counters a budget bounds, keyed `<phase>.<counter>`.
    fn budget_counters(&self) -> impl Iterator<Item = (String, u64)> + '_ {
        let t = &self.totals;
        let asset = |name: &str| t.assets.get(name).copied().unwrap_or_default();
        let (mesh, image, menu_material) = (asset("mesh"), asset("image"), asset("menu_material"));
        [
            (
                "allocation_calls",
                t.allocation_calls + t.reallocation_calls,
            ),
            ("requested_bytes", t.requested_bytes),
            ("entities_spawned", t.entities_spawned),
            ("entities_despawned", t.entities_despawned),
            ("mesh_entities", t.mesh_entities),
            ("render_world_entities", t.render_world_entities),
            ("render_mesh_instances", t.render_mesh_instances),
            ("draw_calls", t.draw_calls),
            ("multi_draw_calls", t.multi_draw_calls),
            ("pipelines_set", t.pipelines_set),
            ("bind_groups_set", t.bind_groups_set),
            ("bind_groups_created", t.bind_groups_created),
            ("buffers_created", t.buffers_created),
            ("buffer_bytes_created", t.buffer_bytes_created),
            ("textures_created", t.textures_created),
            ("mesh_modified", mesh.modified),
            ("mesh_upload_bytes", mesh.payload_bytes),
            ("image_modified", image.modified),
            ("image_upload_bytes", image.payload_bytes),
            ("menu_material_modified", menu_material.modified),
        ]
        .into_iter()
        .map(|(counter, value)| (format!("{}.{counter}", self.name), value))
    }
}

fn game_time(app: &mut App) -> Option<f64> {
    app.world_mut()
        .get_resource_mut::<MapScriptSession<MapStartupHost>>()?
        .host()?
        .objects()
        .timer_game_time()
}

/// `perf stat` attached to this process for the phase named by `PCX_PERF_PHASE`: per-thread
/// user instructions, counted only between the `enable` and `disable` control commands.
/// Instruction counts have no in-process API and signalling needs `kill`, so both are
/// external processes.
struct PerfWindow {
    perf: std::process::Child,
    ctl: std::fs::File,
    ack: std::fs::File,
}

impl PerfWindow {
    fn start(name: &str) -> Option<Self> {
        (std::env::var("PCX_PERF_PHASE").ok()? == name).then_some(())?;
        let ctl = std::env::var_os("PCX_PERF_CTL")?;
        let ack = std::env::var_os("PCX_PERF_ACK")?;
        let out = std::env::var_os("PCX_PERF_OUT")?;
        let mut control = std::ffi::OsString::from("fifo:");
        control.push(&ctl);
        control.push(",");
        control.push(&ack);
        // `PCX_PERF_RECORD` samples every N-th user instruction into `PCX_PERF_OUT` (a perf.data
        // file) instead of counting them, for a per-symbol breakdown of the same window.
        let mut command = std::process::Command::new("perf");
        match std::env::var("PCX_PERF_RECORD") {
            Ok(period) => command.args(["record", "-e", "instructions:u", "-c", &period]),
            Err(_) => command.args(["stat", "--per-thread", "-x,", "-e", "instructions:u"]),
        };
        let perf = command
            .args(["-D", "-1", "-p"])
            .arg(std::process::id().to_string())
            .arg("--control")
            .arg(control)
            .arg("-o")
            .arg(out)
            .spawn()
            .expect("perf stat");
        let ctl = std::fs::OpenOptions::new().write(true).open(ctl).unwrap();
        let ack = std::fs::File::open(ack).unwrap();
        Some(Self { perf, ctl, ack })
    }

    fn send(&mut self, command: &str) {
        self.ctl.write_all(command.as_bytes()).unwrap();
        // perf terminates each reply with a newline and a NUL byte.
        let mut reply = Vec::new();
        let mut byte = [0_u8; 1];
        while byte[0] != b'\n' {
            self.ack.read_exact(&mut byte).unwrap();
            if byte[0] != 0 {
                reply.push(byte[0]);
            }
        }
        assert_eq!(reply, b"ack\n", "perf stat control reply");
    }

    /// Stops perf so it prints its counts.
    fn stop(mut self) {
        let status = std::process::Command::new("kill")
            .args(["-INT", &self.perf.id().to_string()])
            .status()
            .expect("kill");
        assert!(status.success());
        self.perf.wait().unwrap();
    }
}

fn frame_digest(image: &Image) -> String {
    let data = image.data.as_ref().expect("captured frame pixels");
    let mut hash = Sha256::new();
    hash.update(image.width().to_le_bytes());
    hash.update(image.height().to_le_bytes());
    hash.update(data);
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Sequential phases share one app, so each starts from the previous phase's
/// end state; the order is part of the workload identity.
/// Installs the span allocation layer when `PCX_SPAN_PROFILE` is set. Call before the
/// app is built so Bevy's log plugin finds a subscriber already installed.
pub(super) fn install_span_profile() {
    use tracing_subscriber::layer::SubscriberExt as _;
    if std::env::var_os("PCX_SPAN_PROFILE").is_some() {
        tracing::subscriber::set_global_default(
            tracing_subscriber::registry().with(spans::SpanCharge),
        )
        .expect("no subscriber installed before the workload");
    }
}

pub(super) struct Harness {
    phases: Vec<Phase>,
}

impl Harness {
    pub(super) fn new() -> Self {
        Self { phases: Vec::new() }
    }

    /// Runs `frames` counted updates, calling `each(app, frame_index)` before each.
    fn run(
        &mut self,
        app: &mut App,
        name: &'static str,
        output: &Path,
        mut each: impl FnMut(&mut App, usize),
    ) {
        // Settle spawns from the previous phase's captures before counting.
        for _ in 0..12 {
            step(app);
        }
        let mut probe = Probe::new(app);
        let mut totals = Frame::default();
        let mut ran = BTreeMap::new();
        let mut rows = Vec::with_capacity(FRAMES);
        let mut window = PerfWindow::start(name);
        if let Some(window) = window.as_mut() {
            window.send("enable\n");
        }
        spans::take();
        spans::set_active(true);
        for index in 0..FRAMES {
            each(app, index);
            let frame = probe.frame(app, &mut ran);
            totals.allocation_calls += frame.allocation_calls;
            totals.reallocation_calls += frame.reallocation_calls;
            totals.requested_bytes += frame.requested_bytes;
            totals.entities_spawned += frame.entities_spawned;
            totals.entities_despawned += frame.entities_despawned;
            totals.systems_ran += frame.systems_ran;
            totals.draw_calls += frame.draw_calls;
            totals.multi_draw_calls += frame.multi_draw_calls;
            totals.pipelines_set += frame.pipelines_set;
            totals.bind_groups_set += frame.bind_groups_set;
            totals.bind_groups_created += frame.bind_groups_created;
            totals.buffers_created += frame.buffers_created;
            totals.buffer_bytes_created += frame.buffer_bytes_created;
            totals.textures_created += frame.textures_created;
            for (asset, counts) in &frame.assets {
                *totals.assets.entry(asset).or_default() += *counts;
            }
            totals.live_entities = frame.live_entities;
            totals.render_world_entities = frame.render_world_entities;
            totals.mesh_entities = frame.mesh_entities;
            totals.visible_mesh_entities = frame.visible_mesh_entities;
            totals.ui_nodes = frame.ui_nodes;
            totals.text_entities = frame.text_entities;
            totals.hidden_mesh_entities = frame.hidden_mesh_entities;
            totals.visible_distinct_meshes = frame.visible_distinct_meshes;
            totals.visible_distinct_materials = frame.visible_distinct_materials;
            totals.render_mesh_instances = frame.render_mesh_instances;
            totals.render_material_instances = frame.render_material_instances;
            rows.push(frame);
        }
        spans::set_active(false);
        let mut spans: Vec<_> = spans::take().into_iter().collect();
        spans.sort_by(|a, b| {
            b.1.requested_bytes
                .cmp(&a.1.requested_bytes)
                .then_with(|| a.0.cmp(&b.0))
        });
        spans.truncate(SPAN_REPORT_ROWS);
        if let Some(mut window) = window {
            window.send("disable\n");
            window.stop();
        }
        let image = capture(app, &output.join(format!("{name}.png")));
        let phase = Phase {
            name,
            frames: FRAMES,
            totals,
            system_frames: ran,
            frame_sha256: frame_digest(&image),
            game_time: game_time(app),
            spans,
        };
        std::fs::write(
            output.join(format!("{name}-frames.json")),
            serde_json::to_vec(&rows).unwrap(),
        )
        .unwrap();
        self.phases.push(phase);
    }

    pub(super) fn menu(&mut self, app: &mut App, output: &Path) {
        self.run(app, "menu", output, |_, _| {});
    }

    pub(super) fn campaign(&mut self, app: &mut App, output: &Path, selected: HandleId) {
        self.run(app, "campaign_idle", output, |_, _| {});
        self.run(app, "campaign_hud", output, |app, index| {
            if index % HUD_EVENT_PERIOD != 0 {
                return;
            }
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let host = session.host().unwrap();
            if index % (2 * HUD_EVENT_PERIOD) == 0 {
                let player = host.native("Player", &[Value::Int(0)]).unwrap();
                let text = Value::Str(format!("render-cpu message {index}"));
                host.native(
                    "DisplayTimedTextToPlayer",
                    &[
                        player,
                        Value::Real(0.),
                        Value::Real(0.),
                        Value::Real(HUD_MESSAGE_SECONDS),
                        text,
                    ],
                )
                .unwrap();
            } else {
                host.objects_mut().select_units(&[selected], index % 4 == 0);
            }
        });
        self.run(app, "campaign_motion", output, |app, index| {
            if index != 0 {
                return;
            }
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let host = session.host().unwrap();
            let [x, y] = host
                .objects()
                .executed_unit(selected)
                .unwrap()
                .position
                .expect("selected unit position");
            host.native(
                "IssuePointOrder",
                &[
                    Value::Handle(selected),
                    Value::Str("move".into()),
                    Value::Real(x + MOVE_DISTANCE),
                    Value::Real(y),
                ],
            )
            .unwrap();
        });
    }

    pub(super) fn finish(self, app: &mut App, output: &Path) {
        let errors = {
            let world = app.world();
            [
                world
                    .resource::<crate::campaign_actors::UnitActors>()
                    .last_error
                    .clone(),
                world
                    .resource::<crate::campaign_hud::CampaignHud>()
                    .last_error
                    .clone(),
                world
                    .resource::<crate::campaign_minimap::CampaignMinimap>()
                    .last_error
                    .clone(),
            ]
        };
        let summary = serde_json::json!({
            "workload": WORKLOAD,
            "step_seconds": STEP_SECONDS,
            "frames_per_phase": FRAMES,
            "campaign_seed": 1,
            "phases": self.phases,
            "subsystem_errors": errors,
            "scope": "Counted work inside App::update with software Vulkan and no window; wall time and rasterizer work are not counted",
        });
        std::fs::write(
            output.join("render-cpu-summary.json"),
            serde_json::to_vec_pretty(&summary).unwrap(),
        )
        .unwrap();
        let observed: BTreeMap<String, u64> = self
            .phases
            .iter()
            .flat_map(Phase::budget_counters)
            .collect();
        std::fs::write(
            output.join("render-budget-observed.json"),
            budget::render(WORKLOAD, &observed),
        )
        .unwrap();
        if let Some(path) = std::env::var_os("PCX_RENDER_BUDGET") {
            let budget = std::fs::read_to_string(&path).expect("PCX_RENDER_BUDGET file");
            if let Err(failures) = budget::check(WORKLOAD, &observed, &budget) {
                panic!("render budget exceeded:\n{}", failures.join("\n"));
            }
        }
    }
}

/// World units the motion phase moves the selected unit; a workload parameter.
const MOVE_DISTANCE: f32 = 512.;

use pcx_script::{HandleId, Host as _, Value};

/// Counters of one scene-entry interval (menu or campaign), written to
/// `entry-<name>.json`. The interval is a variable number of `App::update` calls that ends when
/// the scene is ready, so it reports work totals, never a per-frame rate.
pub(super) struct EntryWindow {
    name: &'static str,
    perf: Option<PerfWindow>,
    allocated: crate::counting_allocator::Totals,
    submitted: bevy::render::render_stats::RenderStats,
}

impl EntryWindow {
    pub(super) fn start(name: &'static str) -> Self {
        let mut perf = PerfWindow::start(name);
        if let Some(window) = perf.as_mut() {
            window.send("enable\n");
        }
        Self {
            name,
            perf,
            allocated: totals(),
            submitted: bevy::render::render_stats::snapshot(),
        }
    }

    pub(super) fn finish(self, output: &Path) {
        let allocated = totals() - self.allocated;
        let submitted = bevy::render::render_stats::snapshot() - self.submitted;
        if let Some(mut window) = self.perf {
            window.send("disable\n");
            window.stop();
        }
        let summary = serde_json::json!({
            "workload": WORKLOAD,
            "entry": self.name,
            "allocation_calls": allocated.calls,
            "reallocation_calls": allocated.reallocations,
            "requested_bytes": allocated.bytes,
            "draw_calls": submitted.draw_calls,
            "multi_draw_calls": submitted.multi_draw_calls,
            "pipelines_set": submitted.pipelines_set,
            "bind_groups_set": submitted.bind_groups_set,
            "bind_groups_created": submitted.bind_groups_created,
            "buffers_created": submitted.buffers_created,
            "buffer_bytes_created": submitted.buffer_bytes_created,
            "textures_created": submitted.textures_created,
        });
        std::fs::write(
            output.join(format!("entry-{}.json", self.name)),
            serde_json::to_vec_pretty(&summary).unwrap(),
        )
        .unwrap();
    }
}
