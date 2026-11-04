//! Explicit local-asset diagnostic of the production UI and renderer.
use bevy::{
    asset::AssetPlugin,
    camera::RenderTarget,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    render::{
        RenderPlugin,
        render_resource::{PollType, TextureFormat, TextureUsages},
        renderer::RenderDevice,
        settings::{Backends, RenderCreation, WgpuSettings},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    ui::{InteractionDisabled, UiSystems},
    ui_widgets::{Activate, Button},
    window::{ExitCondition, PrimaryWindow},
    winit::WinitPlugin,
};
use pcx_play::map_startup_host::MapStartupHost;
use peacecraft3::{
    campaign_camera::CampaignCamera, campaign_startup::CampaignStartup,
    map_script_session::MapScriptSession,
};
use std::{path::Path, time::Duration};

mod bench;

#[test]
#[ignore = "requires PCX_NATIVE_CHAPTER_* and a new PCX_CAMPAIGN_CAPTURE_DIR; CPU-only minimap mapping receipt"]
fn selected_minimap_mapping_receipt() {
    let output = std::path::PathBuf::from(std::env::var("PCX_CAMPAIGN_CAPTURE_DIR").unwrap());
    assert!(!output.exists(), "mapping receipt directory must be new");
    let mut app = crate::native_campaign::tests::started_app();
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let context = session.context().unwrap().clone();
    let objects = session.host().unwrap().objects();
    let camera = objects.camera_state().unwrap();
    let bounds = camera.bounds.unwrap();
    let margins = camera.margins.values();
    let corners = bounds.as_chunks::<2>().0;
    let left = corners.iter().map(|p| p[0]).reduce(f32::min).unwrap() - margins[0];
    let right = corners.iter().map(|p| p[0]).reduce(f32::max).unwrap() + margins[1];
    let top = corners.iter().map(|p| p[1]).reduce(f32::max).unwrap() + margins[2];
    let bottom = corners.iter().map(|p| p[1]).reduce(f32::min).unwrap() - margins[3];
    let path = std::path::PathBuf::from(std::env::var("PCX_NATIVE_CHAPTER_INSTALL").unwrap())
        .join("Warcraft III.exe");
    let rules = pcx_play::minimap_layout::Rules::reviewed_legacy(
        &std::fs::read(&path).unwrap(),
        pcx_play::scripts::AssetIdentity {
            source: path.display().to_string(),
            edition: "selected campaign client".into(),
            version: "reviewed minimap".into(),
        },
    )
    .unwrap();
    let derived = rules.world_bounds(camera).unwrap();
    assert_eq!(derived, [[left, bottom], [right, top]]);
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("minimap-mapping.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "scope": "Executed camera inputs versus old minimap mapping; native field join is separately static evidence",
        "map": context.sources.map.source, "edition": context.sources.map.edition,
        "camera_bounds": bounds, "margins": margins,
        "old_world_bounds": objects.vision_world_bounds(),
        "expanded_camera_bounds": [[left,bottom],[right,top]], "derived_bounds": derived, "texture_filters": rules.texture_filters, "texture_address": rules.texture_address,
        "margin_image": format!("{:?}",camera.margins.binding()),
    })).unwrap()).unwrap();
}

#[test]
#[ignore = "requires PCX_NATIVE_CHAPTER_* and a new PCX_CAMPAIGN_CAPTURE_DIR; CPU-only original terrain mip check"]
fn selected_terrain_authored_mips_reach_renderer() {
    let output = std::path::PathBuf::from(std::env::var("PCX_CAMPAIGN_CAPTURE_DIR").unwrap());
    assert!(!output.exists(), "mip receipt directory must be new");
    let app = crate::native_campaign::tests::started_app();
    let requests = app
        .world()
        .resource::<peacecraft3::map_requests::MapRequests>();
    let bindings = requests.bindings.as_ref().unwrap();
    let context = requests.admission.shared_context().unwrap();
    let reader = app
        .world()
        .resource::<peacecraft3::browser_configuration::SelectedMapSources>()
        .for_receipts(&bindings.receipts)
        .unwrap();
    let table = reader
        .resolve_bounded("TerrainArt\\Terrain.slk", 16 << 20)
        .unwrap();
    let catalog = pcx_tera::TileCatalog::from_terrain_slk(&table.bytes).unwrap();
    let mut sources = Vec::new();
    let slots = peacecraft3::map_ground::load_slot_blps(&context.terrain, &catalog, |path| {
        let asset = reader
            .resolve_bounded(path, 16 << 20)
            .map_err(|error| error.to_string())?;
        sources.push(format!("{:?}", peacecraft3::map_binding::receipt(&asset)));
        Ok(asset.bytes)
    })
    .unwrap();
    let borrowed = slots
        .iter()
        .map(|slot| pcx_gfx::SelectedGroundBlp {
            fourcc: slot.fourcc,
            source: &slot.source,
            bytes: &slot.bytes,
        })
        .collect::<Vec<_>>();
    let decoded = pcx_gfx::decode_ground_slots(
        &borrowed,
        pcx_gfx::GroundMemory {
            retained_bytes: 128 << 20,
            transient_bytes: 64 << 20,
        },
    )
    .unwrap();
    let levels = decoded.image.texture_descriptor.mip_level_count;
    let lengths = (0..levels)
        .map(|level| ((decoded.tile >> level).max(1) as usize).pow(2) * 4)
        .collect::<Vec<_>>();
    let layer_bytes: usize = lengths.iter().sum();
    let data = decoded.image.data.as_ref().unwrap();
    let mut layer_base = 0usize;
    let mut different = 0usize;
    let mut rows = Vec::new();
    for slot in &slots {
        let header = pcx_blp::Header::parse(&slot.bytes).unwrap();
        let loaded =
            pcx_gfx::load_mipmapped_image_bounded(&slot.bytes, &slot.source, 64 << 20).unwrap();
        let columns = header.width / decoded.tile;
        let grid_rows = header.height / decoded.tile;
        let mut prefix = 0usize;
        for level in 0..levels {
            let (width, height) = header.mip_size(level as usize);
            let tile = height / grid_rows;
            let pixels = if level == 0 {
                &loaded.rgba
            } else {
                &loaded.mipmaps[level as usize - 1]
            };
            for row in 0..grid_rows {
                for col in 0..columns {
                    let layer = layer_base
                        + ((col / grid_rows) * grid_rows * grid_rows
                            + row * grid_rows
                            + col % grid_rows) as usize;
                    for y in 0..tile {
                        let source = (((row * tile + y) * width + col * tile) * 4) as usize;
                        let target = layer * layer_bytes + prefix + (y * tile * 4) as usize;
                        let count = (tile * 4) as usize;
                        different += pixels[source..source + count]
                            .iter()
                            .zip(&data[target..target + count])
                            .filter(|(a, b)| a != b)
                            .count();
                    }
                }
            }
            prefix += lengths[level as usize];
        }
        rows.push(serde_json::json!({"path": slot.source, "mips": header.mip_count(), "width": header.width, "height": header.height}));
        layer_base += (columns * grid_rows) as usize;
    }
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("terrain-mips.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "scope": "Source mip pixels versus ground upload; not native sampling or pixel-parity proof",
        "map": context.sources.map.source, "edition": context.sources.map.edition,
        "source_receipts": sources, "sheets": rows, "upload_levels": levels, "different_channels": different,
    })).unwrap()).unwrap();
    assert_eq!(
        different, 0,
        "ground upload must preserve supplied mip pixels"
    );
}

#[test]
#[ignore = "requires explicit PCX_NATIVE_CHAPTER_* and a new PCX_CAMPAIGN_CAPTURE_DIR; CPU-only source inspection"]
fn selected_terrain_lighting_source_receipt() {
    let output = std::path::PathBuf::from(std::env::var("PCX_CAMPAIGN_CAPTURE_DIR").unwrap());
    assert!(!output.exists(), "source receipt directory must be new");
    let mut app = crate::native_campaign::tests::started_app();
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let context = session.context().unwrap().clone();
    let objects = session.host().unwrap().objects();
    let fraction = objects
        .day_fraction()
        .expect("selected chapter day fraction");
    let selected = objects.day_night_state().unwrap().selected().unwrap();
    let models = selected.into_iter().zip(["terrain", "units"]).map(|(model, role)| {
        let sample = match model.sun(fraction) {
            Ok(sun) => serde_json::json!({
                "direction": sun.direction, "colour": sun.colour,
                "intensity": sun.intensity, "ambient_colour": sun.ambient_colour,
                "ambient_intensity": sun.ambient_intensity,
            }),
            Err(error) => serde_json::json!({"unsupported": error}),
        };
        let lights = model.lights().iter().map(|light| serde_json::json!({
            "name": light.name, "kind": format!("{:?}", light.kind),
            "parent": light.parent_id, "intensity": light.intensity,
            "ambient_intensity": light.ambient_intensity,
            "node_tracks": light.node_tracks.iter().map(|track| String::from_utf8_lossy(&track.magic).into_owned()).collect::<Vec<_>>(),
            "tracks": light.tracks.iter().map(|track| String::from_utf8_lossy(&track.magic).into_owned()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>();
        serde_json::json!({
            "role": role, "script_path": model.script_path(),
            "source": model.source_label(), "edition": model.origin().edition,
            "version": model.origin().version,
            "sha256": model.sha256().iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            "lights": lights, "sample": sample,
        })
    }).collect::<Vec<_>>();
    let mesh = pcx_tera::TerrainMesh::build(&context.terrain);
    assert_eq!(mesh.normals.len(), mesh.positions.len());
    assert!(mesh.normals.iter().flatten().all(|value| value.is_finite()));
    for cy in 0..=context.terrain.height() {
        for cx in 0..=context.terrain.width() {
            assert_eq!(
                pcx_tera::corner_normal(&context.terrain, cx, cy),
                Some(mesh.normals[(cy * (context.terrain.width() + 1) + cx) as usize]),
                "source terrain normal at {cx},{cy}"
            );
        }
    }
    let receipt = serde_json::json!({
        "scope": "Supplied MDX records and recreation host sampling; not native rendering or tick identity proof",
        "map": context.sources.map.source, "edition": context.sources.map.edition,
        "version": context.sources.map.version, "day_fraction": fraction,
        "models": models, "terrain_corners": mesh.positions.len(),
        "nonflat_normals": mesh.normals.iter().filter(|normal| normal[0] != 0. || normal[1] != 0.).count(),
        "corner_normal_matches_mesh": true,
    });
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(
        output.join("terrain-light-source.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

/// Controlled original-capture comparison harness for sky illumination,
/// terrain/tree sight occlusion and doodad-particle ownership.
///
/// Correction context (`docs/re-notes/campaign-sky.md#correction-and-own-light-connection-2026-10-25`,
/// TRACKER sky/sight entries): the selected sky model carries its own `LITE`
/// directional light sampled at the canonical day fraction; the earlier
/// no-light inventory was wrong. A headless/muted original-client control
/// pair keeps the same map, sky texture, camera and frozen diagnostic noon
/// while forcing the material unlit bit, plus a null-sky control. This module
/// retains the recreation side of those controls with asset-derived identities
/// only: no brightness, colour, geometry or timing constants are invented.
/// Sight occlusion (`SharedVision::occlusion_applied == Some(false)`,
/// minimap circular-sight rendering) and doodad-particle ownership refusals
/// remain open gaps; the harness records them instead of asserting parity.
/// Captures and receipts stay local under the caller-supplied output
/// directory. Original captures remain untracked. This harness does not turn
/// the strict render gate green.
mod sky_sight_parity {
    use super::*;

    /// Exact RGBA comparison of two PNG file buffers. Refuses differing
    /// extents instead of rescaling. No tolerance, no normalization, no
    /// cropping: any channel difference in a pixel counts as changed. Mirrors
    /// the `tools/original-game-reference/reference.py report` rule for the
    /// sky/sight patch comparison the analyst runs against the original
    /// capture.
    pub(super) struct Comparison {
        pub(super) changed: usize,
        pub(super) total: usize,
        pub(super) bounds: Option<[[u32; 2]; 2]>,
        pub(super) first: Option<serde_json::Value>,
    }

    pub(super) fn compare_png_bytes(
        recreation: &[u8],
        original: &[u8],
        label: &str,
    ) -> Result<Comparison, String> {
        fn decode(bytes: &[u8], label: &str) -> Result<Image, String> {
            Image::from_buffer(
                bytes,
                bevy::image::ImageType::Extension("png"),
                bevy::image::CompressedImageFormats::NONE,
                true,
                bevy::image::ImageSampler::linear(),
                bevy::asset::RenderAssetUsages::default(),
            )
            .map_err(|error| format!("{label}: png decode refused: {error:?}"))
        }
        let recreation = decode(recreation, &format!("{label}: recreation"))?;
        let original = decode(original, &format!("{label}: original"))?;
        let size = recreation.texture_descriptor.size;
        let other = original.texture_descriptor.size;
        if size.width != other.width || size.height != other.height {
            return Err(format!(
                "{label}: extent differs, no rescaling: recreation {}x{} vs original {}x{}",
                size.width, size.height, other.width, other.height,
            ));
        }
        let Some(a) = recreation.data.as_deref() else {
            return Err(format!("{label}: recreation pixels unavailable"));
        };
        let Some(b) = original.data.as_deref() else {
            return Err(format!("{label}: original pixels unavailable"));
        };
        if a.len() != b.len() {
            return Err(format!("{label}: pixel buffer length differs"));
        }
        let width = size.width;
        let mut changed = 0usize;
        let mut bounds: Option<[[u32; 2]; 2]> = None;
        let mut first: Option<serde_json::Value> = None;
        for (index, (pixel_a, pixel_b)) in a
            .as_chunks::<4>()
            .0
            .iter()
            .zip(b.as_chunks::<4>().0)
            .enumerate()
        {
            if pixel_a != pixel_b {
                changed += 1;
                let x = (index as u32) % width;
                let y = (index as u32) / width;
                bounds = Some(match bounds {
                    None => [[x, y], [x, y]],
                    Some([[x0, y0], [x1, y1]]) => [[x0.min(x), y0.min(y)], [x1.max(x), y1.max(y)]],
                });
                if first.is_none() {
                    first = Some(serde_json::json!({
                        "x": x, "y": y, "recreation": pixel_a, "original": pixel_b,
                    }));
                }
            }
        }
        Ok(Comparison {
            changed,
            total: a.len() / 4,
            bounds,
            first,
        })
    }

    /// Retains the recreation side of the controlled sky/sight comparison at
    /// the unchanged campaign-control checkpoint, before the harness mutates
    /// fog for its separate minimap diagnostic. All values are read from the
    /// admitted session, scene camera and renderer; nothing is invented.
    /// The three arms are: source-lit (the current dome with its own `LITE`
    /// sun at the canonical day fraction), forced-unlit (the same model bytes
    /// with the material unlit bit set, applied in the original-client
    /// control only) and null-sky (`SetSkyModel(null)`). The original runner
    /// repeats the same map, sky texture, camera and frozen noon, then the
    /// analyst compares with `reference.py report` under exact RGB equality.
    pub(super) fn retain_control(
        app: &mut App,
        output: &Path,
        install: &str,
        map: &str,
        expansion: bool,
    ) {
        let (sky_script, day_fraction, game_time, vision) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let Some(host) = session.host() else {
                let receipt = serde_json::json!({
                    "install": install, "map": map, "expansion": expansion,
                    "refusal": "no live campaign host at the control checkpoint",
                    "scope": "sky/sight parity harness; unchanged checkpoint; no pixel-parity claim",
                });
                std::fs::write(
                    output.join("sky-sight-parity.json"),
                    serde_json::to_vec_pretty(&receipt).unwrap(),
                )
                .unwrap();
                return;
            };
            let objects = host.objects();
            let sky_script = match objects.environment().map(|state| state.sky_model.clone()) {
                None => "unset".to_owned(),
                Some(None) => "never-executed".to_owned(),
                Some(Some(None)) => "null".to_owned(),
                Some(Some(Some(path))) => format!("executed:{path}"),
            };
            let day_fraction = objects.day_fraction();
            let game_time = objects.timer_game_time();
            let readiness = objects.vision_readiness().err();
            let revision = objects.vision_revision();
            let occlusion_applied = objects.vision_occlusion_applied();
            let stats = objects.vision_sample_stats().map(|stats| {
                serde_json::json!({
                    "cell_visits": stats.cell_visits,
                    "emitted_sources": stats.emitted_sources,
                    "candidates_scanned": stats.candidates_scanned,
                })
            });
            let bounds = objects.vision_world_bounds();
            let viewer = objects.local_player_settings().map(|local| {
                let settings = objects.startup_settings();
                (
                    local.identity,
                    settings.map(|s| (s.fog_enabled, s.fog_mask_enabled)),
                )
            });
            let mut counts = serde_json::json!({
                "visible": 0, "fogged": 0, "masked": 0, "outside": 0, "refused": 0, "units": 0,
            });
            if let Some((viewer, switches)) = viewer
                && let Some((fog, mask)) = switches
            {
                for (_, unit) in objects.executed_units() {
                    counts["units"] = (counts["units"].as_u64().unwrap_or(0) + 1).into();
                    let Some(position) = unit.position else {
                        continue;
                    };
                    match objects.vision_presented(viewer, position, fog, mask) {
                        Ok(Some(state)) => {
                            let key = format!("{state:?}").to_lowercase();
                            if counts.get(&key).is_some() {
                                counts[&key] = (counts[&key].as_u64().unwrap_or(0) + 1).into();
                            }
                        }
                        Ok(None) => {
                            counts["outside"] =
                                (counts["outside"].as_u64().unwrap_or(0) + 1).into();
                        }
                        Err(_) => {
                            counts["refused"] =
                                (counts["refused"].as_u64().unwrap_or(0) + 1).into();
                        }
                    }
                }
            }
            let destructables = objects.destructables_with_handles().count();
            let vision = serde_json::json!({
                "readiness": readiness,
                "revision": revision,
                "occlusion_applied": occlusion_applied,
                "sample_stats": stats,
                "world_bounds": bounds,
                "presented_counts": counts,
                "destructable_handles": destructables,
                "note": "selected-client lattice and blocker graph; grounded-flying suppression and support/deck heights remain unqualified",
            });
            (sky_script, day_fraction, game_time, vision)
        };
        let cameras: Vec<String> = app
            .world_mut()
            .query_filtered::<
                (&Transform, &Projection, &Camera),
                With<peacecraft3::campaign_camera::CampaignSceneCamera>,
            >()
            .iter(app.world())
            .map(|(transform, projection, camera)| {
                format!("{transform:?} {projection:?} {camera:?}")
            })
            .collect();
        let minimap = app
            .world()
            .resource::<crate::campaign_minimap::CampaignMinimap>();
        let minimap_state = serde_json::json!({
            "member": minimap.member,
            "shown": minimap.shown,
            "last_error": minimap.last_error,
        });
        let ground = app
            .world()
            .resource::<peacecraft3::map_ground::GroundPublication>();
        let ground_state = serde_json::json!({
            "last_error": ground.last_error.as_ref().map(|error| format!("{error:?}")),
            "shadow_error": ground.shadow_error,
            "lighting_error": app.world().resource::<crate::campaign_ground_lighting::TerrainLighting>().last_error,
            "lighting_submitted": app.world().resource::<crate::campaign_ground_lighting::TerrainLighting>().last.is_some(),
        });
        let actors = app.world().resource::<crate::campaign_actors::UnitActors>();
        let actor_error = actors.last_error.clone();
        let rendered_destructables = actors.rendered_destructables();
        let sky_cameras = app
            .world_mut()
            .query_filtered::<Entity, With<crate::campaign_actors::sky::SkyCamera>>()
            .iter(app.world())
            .count();
        let particle_meshes = app
            .world_mut()
            .query_filtered::<
                Entity,
                With<bevy::prelude::MeshMaterial3d<crate::mainmenu_scene::ParticleMaterial>>,
            >()
            .iter(app.world())
            .count();
        // Self-comparison exercises the exact-compare path on real
        // asset-derived bytes: identical inputs must report zero changes.
        let self_check = match std::fs::read(output.join("campaign-control.png")) {
            Ok(bytes) => match compare_png_bytes(&bytes, &bytes, "campaign-control self") {
                Ok(comparison) => serde_json::json!({
                    "changed": comparison.changed, "total": comparison.total,
                    "bounds": comparison.bounds, "first": comparison.first,
                }),
                Err(error) => serde_json::json!({ "refusal": error }),
            },
            Err(error) => {
                serde_json::json!({ "refusal": format!("control frame unread: {error}") })
            }
        };
        let receipt = serde_json::json!({
            "install": install, "map": map, "expansion": expansion,
            "sky_script": sky_script,
            "day_fraction": day_fraction,
            "game_time": game_time,
            "scene_cameras": cameras,
            "vision": vision,
            "minimap": minimap_state,
            "ground": ground_state,
            "actors": {
                "last_error": actor_error,
                "rendered_destructables": rendered_destructables,
                "particle_mesh_entities": particle_meshes,
                "sky_cameras": sky_cameras,
                "note": "doodad-particle ownership refusals surface in last_error as doodad {id} diagnostics; deferred publication boundary sits between actor projection and doodad advance_particles",
            },
            "frames_self_check": self_check,
            "arms": {
                "source_lit": "current dome with its own LITE directional sun sampled at the canonical day fraction",
                "forced_unlit": "original-client control only: same model bytes with the material unlit bit set",
                "null_sky": "SetSkyModel(null): no dome; sky patches black while the sampled terrain patch is unchanged",
            },
            "original_runner": {
                "paired_skip_at": "PCX_PAIRED_SKIP_AT",
                "paired_capture_at": "PCX_PAIRED_CAPTURE_AT",
                "report": "tools/original-game-reference/reference.py report",
                "rule": "exact RGB equality, no tolerance, no rescaling, no exclusions",
            },
            "scope": "Controlled sky illumination plus terrain/tree and doodad-particle render harness at the unchanged campaign-control checkpoint; records recreation probes for comparison against the original capture. No pixel-parity claim; the strict render gate still refuses the known sky/sight gaps.",
        });
        std::fs::write(
            output.join("sky-sight-parity.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
    }
}

/// Original-client pixel comparison harness for ally, water and menu frames.
///
/// Scope is ally filter/minimap colors, water/bridge trajectory elevation and
/// menu frames only; sky illumination, terrain/tree sight occlusion and
/// doodad-particle ownership stay with `sky_sight_parity`. All expected pixels
/// come from caller-supplied original captures under `PCX_ORIGINAL_CAPTURE_DIR`
/// (a local, untracked directory of `<label>.png` files produced by
/// `tools/original-game-reference/reference.py capture`); no reference pixels
/// are hard-coded here. Tolerances come from `PCX_PARITY_TOLERANCE` (u8 per
/// channel, default exact) and `PCX_PARITY_MAX_FRACTION` (f32, default exact).
/// `PCX_BRIDGE_SLK`, `PCX_BRIDGE_PATH_COLUMN` and `PCX_BRIDGE_HEIGHT_COLUMN`
/// name the caller-supplied doodad-deck table; without them deck admission is
/// refused, not invented. Receipts stay under the caller-supplied output
/// directory. A missing original capture is a refusal for that label, not a
/// parity pass or failure.
mod ally_water_menu_parity {
    use super::*;

    pub(super) struct ParityConfig {
        pub(super) original_dir: std::path::PathBuf,
        pub(super) comparison: pcx_play::ally_color::PixelComparison,
    }

    pub(super) fn config_from_env() -> Option<ParityConfig> {
        let dir = std::env::var_os("PCX_ORIGINAL_CAPTURE_DIR")?;
        if dir.is_empty() {
            return None;
        }
        let tolerance = std::env::var("PCX_PARITY_TOLERANCE")
            .ok()
            .map(|v| {
                v.parse::<u8>()
                    .unwrap_or_else(|_| panic!("PCX_PARITY_TOLERANCE must be 0..=255, got {v:?}"))
            })
            .unwrap_or(0);
        let max_fraction = std::env::var("PCX_PARITY_MAX_FRACTION")
            .ok()
            .map(|v| {
                v.parse::<f32>().unwrap_or_else(|_| {
                    panic!("PCX_PARITY_MAX_FRACTION must be 0.0..=1.0, got {v:?}")
                })
            })
            .unwrap_or(0.0);
        Some(ParityConfig {
            original_dir: std::path::PathBuf::from(dir),
            comparison: pcx_play::ally_color::PixelComparison {
                tolerance_per_channel: tolerance,
                max_differing_fraction: max_fraction,
            },
        })
    }

    fn decode_png(bytes: &[u8], label: &str) -> Result<Image, String> {
        Image::from_buffer(
            bytes,
            bevy::image::ImageType::Extension("png"),
            bevy::image::CompressedImageFormats::NONE,
            true,
            bevy::image::ImageSampler::linear(),
            bevy::asset::RenderAssetUsages::default(),
        )
        .map_err(|error| format!("{label}: png decode refused: {error:?}"))
    }

    fn compare_buffers(
        recreation: &[u8],
        original: &[u8],
        config: &ParityConfig,
        label: &str,
    ) -> Result<serde_json::Value, String> {
        let recreation = decode_png(recreation, &format!("{label}: recreation"))?;
        let original = decode_png(original, &format!("{label}: original"))?;
        let size = recreation.texture_descriptor.size;
        let other = original.texture_descriptor.size;
        if size.width != other.width || size.height != other.height {
            return Err(format!(
                "{label}: extent differs, no rescaling: recreation {}x{} vs original {}x{}",
                size.width, size.height, other.width, other.height,
            ));
        }
        let Some(a) = recreation.data.as_deref() else {
            return Err(format!("{label}: recreation pixels unavailable"));
        };
        let Some(b) = original.data.as_deref() else {
            return Err(format!("{label}: original pixels unavailable"));
        };
        let pixels = size.width as usize * size.height as usize;
        let fraction = config.comparison.differing_fraction(a, b, pixels)?;
        let passes = config.comparison.passes(a, b, pixels).unwrap_or(false);
        Ok(serde_json::json!({
            "label": label,
            "extent": [size.width, size.height],
            "differing_fraction": fraction,
            "tolerance_per_channel": config.comparison.tolerance_per_channel,
            "max_differing_fraction": config.comparison.max_differing_fraction,
            "passes": passes,
        }))
    }

    /// Compare one recreation capture against its original peer. Missing files
    /// are recorded as refusals; extents must match exactly.
    pub(super) fn compare_capture(
        output: &Path,
        config: &ParityConfig,
        label: &str,
    ) -> serde_json::Value {
        let recreation_path = output.join(label);
        let original_path = config.original_dir.join(label);
        let recreation = std::fs::read(&recreation_path);
        let original = std::fs::read(&original_path);
        match (recreation, original) {
            (Ok(a), Ok(b)) => match compare_buffers(&a, &b, config, label) {
                Ok(entry) => entry,
                Err(error) => serde_json::json!({"label": label, "refusal": error}),
            },
            (Err(error), _) => serde_json::json!({
                "label": label,
                "refusal": format!("recreation capture unread: {error}"),
            }),
            (_, Err(error)) => serde_json::json!({
                "label": label,
                "refusal": format!("no original capture; run the original client first: {error}"),
                "original_expected_at": original_path.display().to_string(),
            }),
        }
    }

    /// Retain the recreation side of the ally/water/menu comparison at the
    /// post-restart checkpoint. Wires `PixelComparison`, `BridgeDeckTable`,
    /// `trajectory_bridge_crossings`, `BankPitch` and
    /// `Progress::{mark_complete, next_incomplete}` into the same offscreen
    /// captures the harness already retains. Never asserts parity: per-label
    /// `passes` values are recorded for the analyst's `reference.py report`
    /// run, and the strict render gate stays red on the known sky/sight gaps.
    pub(super) fn retain(app: &mut App, output: &Path, install: &str, map: &str, expansion: bool) {
        let Some(config) = config_from_env() else {
            std::fs::write(
                output.join("ally-water-menu-parity.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "install": install, "map": map, "expansion": expansion,
                    "refusal": "PCX_ORIGINAL_CAPTURE_DIR is unset; no original captures to compare",
                    "scope": "ally/water/menu pixel comparison harness; caller must supply local original captures",
                }))
                .unwrap(),
            )
            .unwrap();
            return;
        };
        // Ally and menu frames: every label is a file the harness already
        // captured offscreen; each original peer is caller-supplied.
        let mut frames = Vec::new();
        for label in [
            "main-menu.png",
            "chapter-selection.png",
            "campaign-entry.png",
            "campaign-control.png",
            "selected-unit.png",
            "minimap-no-fog-diagnostic.png",
            "ally-colors-individual.png",
            "ally-colors-filtered.png",
            "ally-colors-restored.png",
            "escape-main.png",
            "escape-end.png",
            "escape-confirm-exit.png",
            "campaign-exit.png",
            "campaign-restarted.png",
        ] {
            frames.push(compare_capture(output, &config, label));
        }
        // Water and bridge decks: admit the caller-supplied doodad table when
        // the runner names it; otherwise record the refusal and keep the prior
        // surface-only behavior (`WaterPublication::decks == None`).
        let bridge = admit_bridge_decks(app);
        // Trajectory samples between two accepted live positions. The deck
        // lookup stays `None` until a bridge-footprint index is admitted, so
        // motion keeps the sampled surface and gaps stay observable.
        let trajectory = sample_bridge_trajectory(app);
        // Orientation: `BankPitch::level()` must reproduce the existing
        // yaw-only pose exactly; a caller-derived ground normal supplies the
        // only tilt. Normals come from admitted terrain, never constants.
        let orientation = sample_bank_pitch(app);
        // Completion: authored order from the selected index; `mark_complete`
        // runs only for an executed victory observed through the score
        // handoff, never invented. Otherwise `next_incomplete` stays at the
        // first authored mission.
        let completion = sample_completion(app);
        std::fs::write(
            output.join("ally-water-menu-parity.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "install": install, "map": map, "expansion": expansion,
                "tolerance_per_channel": config.comparison.tolerance_per_channel,
                "max_differing_fraction": config.comparison.max_differing_fraction,
                "frames": frames,
                "bridge": bridge,
                "trajectory": trajectory,
                "orientation": orientation,
                "completion": completion,
                "original_dir": config.original_dir.display().to_string(),
                "scope": "ally/water/menu pixel comparison harness; sky/sight gaps stay with sky_sight_parity",
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn admit_bridge_decks(app: &mut App) -> serde_json::Value {
        let (member, path_column, height_column) = match (
            std::env::var("PCX_BRIDGE_SLK").ok(),
            std::env::var("PCX_BRIDGE_PATH_COLUMN").ok(),
            std::env::var("PCX_BRIDGE_HEIGHT_COLUMN").ok(),
        ) {
            (Some(a), Some(b), Some(c)) if !a.is_empty() && !b.is_empty() && !c.is_empty() => {
                (a, b, c)
            }
            _ => {
                return serde_json::json!({
                    "refusal": "no caller-supplied bridge SLK; set PCX_BRIDGE_SLK, PCX_BRIDGE_PATH_COLUMN and PCX_BRIDGE_HEIGHT_COLUMN",
                    "decks": None::<()>,
                });
            }
        };
        let max_entries = std::env::var("PCX_BRIDGE_MAX_ENTRIES")
            .ok()
            .map(|v| {
                v.parse::<usize>()
                    .unwrap_or_else(|_| panic!("PCX_BRIDGE_MAX_ENTRIES must be a count, got {v:?}"))
            })
            .unwrap_or(4096);
        let bytes = {
            let world = app.world();
            let requests = world.resource::<peacecraft3::map_requests::MapRequests>();
            let sources =
                world.resource::<peacecraft3::browser_configuration::SelectedMapSources>();
            let Some(bindings) = requests.bindings.as_ref() else {
                return serde_json::json!({"refusal": "no admitted map bindings for bridge decks"});
            };
            let Some(reader) = sources.for_receipts(&bindings.receipts) else {
                return serde_json::json!({"refusal": "no source reader for bridge decks"});
            };
            match reader.resolve_bounded(&member, 8 << 20) {
                Ok(asset) => asset.bytes,
                Err(error) => {
                    return serde_json::json!({"refusal": format!("bridge SLK unread: {error}")});
                }
            }
        };
        match peacecraft3::map_ground::BridgeDeckTable::read(
            &bytes,
            &member,
            &path_column,
            &height_column,
            max_entries,
        ) {
            Ok(table) => {
                let len = table.len();
                app.world_mut()
                    .resource_mut::<peacecraft3::map_ground::WaterPublication>()
                    .decks = Some(table);
                serde_json::json!({"member": member, "entries": len, "admitted": true})
            }
            Err(error) => serde_json::json!({"member": member, "refusal": error}),
        }
    }

    fn sample_bridge_trajectory(app: &mut App) -> serde_json::Value {
        let context = {
            let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
            match session.context() {
                Some(context) => context.clone(),
                None => return serde_json::json!({"refusal": "no admitted map context"}),
            }
        };
        let (from, to, rules, movement, has_settings) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let Some(host) = session.host() else {
                return serde_json::json!({"refusal": "no live campaign host for trajectory"});
            };
            let objects = host.objects();
            let mut positions = objects
                .executed_units()
                .filter_map(|(_, unit)| unit.position)
                .take(2);
            let (Some(from), Some(to)) = (positions.next(), positions.next()) else {
                return serde_json::json!({"refusal": "fewer than two accepted unit positions"});
            };
            let rules = match objects.unit_placement_rules() {
                Some(rules) => rules.clone(),
                None => return serde_json::json!({"refusal": "no admitted terrain encoding"}),
            };
            let movement = match objects
                .executed_units()
                .find_map(|(_, unit)| unit.movement().ok().map(|_| unit.movement()))
            {
                Some(Ok(movement)) => movement,
                _ => return serde_json::json!({"refusal": "no admitted movement class"}),
            };
            let has_settings = objects.startup_settings().is_some();
            (from, to, rules, movement, has_settings)
        };
        if !has_settings {
            return serde_json::json!({"refusal": "no live host reader"});
        }
        let (terrain, edition, source, mapping, offset) = {
            let [tile, bias, factor] = rules.terrain_encoding();
            let mapping = {
                match pcx_tera::world::Mapping::new(tile, bias, factor) {
                    Some(mapping) => mapping,
                    None => {
                        return serde_json::json!({"refusal": "invalid admitted terrain encoding"});
                    }
                }
            };
            let offset = {
                let sources = app
                    .world()
                    .resource::<peacecraft3::browser_configuration::SelectedMapSources>();
                let requests = app
                    .world()
                    .resource::<peacecraft3::map_requests::MapRequests>();
                let (Some(bindings), Some(_)) = (
                    requests.bindings.as_ref(),
                    requests.admission.shared_context(),
                ) else {
                    return serde_json::json!({"refusal": "no admitted bindings for water offset"});
                };
                let Some(reader) = sources.for_receipts(&bindings.receipts) else {
                    return serde_json::json!({"refusal": "no source reader for water offset"});
                };
                match reader.resolve_bounded("TerrainArt/Water.slk", 4 << 20) {
                    Ok(asset) => match pcx_slq::Table::parse(&asset.bytes) {
                        Ok(table) => match pcx_tera::WaterProfile::open_scaled(
                            &table,
                            "TerrainArt/Water.slk",
                            pcx_tera::WaterSelection::Tileset,
                            &context.terrain,
                            tile,
                        ) {
                            Ok(profile) => profile.height_offset(),
                            Err(error) => {
                                return serde_json::json!({"refusal": format!("water profile: {error}")});
                            }
                        },
                        Err(error) => {
                            return serde_json::json!({"refusal": format!("water table: {error}")});
                        }
                    },
                    Err(error) => {
                        return serde_json::json!({"refusal": format!("water SLK unread: {error}")});
                    }
                }
            };
            (
                context.terrain.clone(),
                context.sources.map.edition.clone(),
                context.sources.map.source.clone(),
                mapping,
                offset,
            )
        };
        let surface = crate::campaign_actors::WaterSurface { mapping, offset };
        let steps = std::env::var("PCX_PARITY_TRAJECTORY_STEPS")
            .ok()
            .map(|v| {
                v.parse::<usize>().unwrap_or_else(|_| {
                    panic!("PCX_PARITY_TRAJECTORY_STEPS must be 1..=64, got {v:?}")
                })
            })
            .unwrap_or(8);
        if steps == 0 || steps > 64 {
            return serde_json::json!({"refusal": "trajectory steps out of bounds"});
        }
        // No bridge-footprint index is admitted yet, so no point claims deck
        // coverage; the gap stays observable instead of lifting motion onto an
        // invented height.
        match crate::campaign_actors::trajectory_bridge_crossings(
            from,
            to,
            steps,
            surface,
            &terrain,
            &edition,
            &source,
            movement,
            |_, _| None,
        ) {
            Ok(crossings) => {
                let gaps = crossings.iter().map(|c| c.deck_gap()).collect::<Vec<_>>();
                let rendered = crossings
                    .iter()
                    .map(|c| c.rendered_height())
                    .collect::<Vec<_>>();
                serde_json::json!({
                    "from": from, "to": to, "steps": steps,
                    "movement": format!("{movement:?}"),
                    "deck_cover": gaps.iter().filter(|g| g.is_some()).count(),
                    "gaps": gaps, "rendered": rendered,
                })
            }
            Err(error) => serde_json::json!({"refusal": error}),
        }
    }

    fn sample_bank_pitch(app: &mut App) -> serde_json::Value {
        let context = {
            let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
            match session.context() {
                Some(context) => context.clone(),
                None => return serde_json::json!({"refusal": "no admitted map context"}),
            }
        };
        let unit = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let Some(host) = session.host() else {
                return serde_json::json!({"refusal": "no live campaign host for orientation"});
            };
            let Some((_, unit)) = host
                .objects()
                .executed_units()
                .find(|(_, unit)| unit.position.is_some())
            else {
                return serde_json::json!({"refusal": "no accepted unit position for orientation"});
            };
            unit.clone()
        };
        let [x, y] = unit.position.unwrap();
        // Central differences on admitted terrain; the epsilon is a harness
        // sampling policy, not a native orientation constant.
        let eps = 1.0_f32;
        let h = |x: f32, y: f32| pcx_tera::world::sample_world_z(&context.terrain, x, y);
        let (hx0, hx1, hy0, hy1) = (h(x - eps, y), h(x + eps, y), h(x, y - eps), h(x, y + eps));
        let normal = match (hx0, hx1, hy0, hy1) {
            (Some(a), Some(b), Some(c), Some(d)) => {
                let nx = a - b;
                let ny = c - d;
                [nx / (2.0 * eps), ny / (2.0 * eps), 1.0]
            }
            _ => {
                return serde_json::json!({"refusal": "orientation point outside admitted terrain"});
            }
        };
        let derived = match crate::campaign_actors::BankPitch::from_ground_normal(normal) {
            Ok(orientation) => orientation,
            Err(error) => return serde_json::json!({"refusal": error}),
        };
        let level = crate::campaign_actors::BankPitch::level();
        let face = unit.face;
        let (level_rotation, derived_rotation) =
            match (level.rotation(face), derived.rotation(face)) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(error), _) | (_, Err(error)) => {
                    return serde_json::json!({"refusal": error});
                }
            };
        // `level()` must reproduce the yaw-only pose exactly; only a derived
        // normal may tilt the actor.
        let oriented = match crate::campaign_actors::transform_for_oriented(
            &unit,
            &context.terrain,
            1.0,
            0.0,
            None,
            level,
        ) {
            Ok(transform) => transform,
            Err(error) => return serde_json::json!({"refusal": error}),
        };
        let tilted = match crate::campaign_actors::transform_for_oriented(
            &unit,
            &context.terrain,
            1.0,
            0.0,
            None,
            derived,
        ) {
            Ok(transform) => transform,
            Err(error) => return serde_json::json!({"refusal": error}),
        };
        serde_json::json!({
            "face_degrees": face,
            "ground_normal": normal,
            "level": {"pitch": level.pitch, "bank": level.bank},
            "derived": {"pitch": derived.pitch, "bank": derived.bank},
            "level_rotation": level_rotation.to_array(),
            "derived_rotation": derived_rotation.to_array(),
            "positions_equal": oriented.translation == tilted.translation,
            "rotations_equal": level_rotation.to_array() == derived_rotation.to_array(),
        })
    }

    fn sample_completion(app: &mut App) -> serde_json::Value {
        let chapters = app.world().resource::<crate::native_campaign::Chapters>();
        let mut order = Vec::new();
        for campaign in &chapters.campaigns {
            for (row, _) in campaign.missions.iter().enumerate() {
                order.push((
                    i32::try_from(campaign.source_index).unwrap_or(i32::MAX),
                    i32::try_from(row).unwrap_or(i32::MAX),
                ));
            }
        }
        let mut progress = pcx_play::campaign_progress::Progress::default();
        let before = progress.next_incomplete(&order);
        let complete_before = progress.order_complete(&order);
        // Only an executed victory observed through the score handoff marks a
        // mission; anything else leaves completion empty. The exit snapshot
        // carries the verbatim `ChangeLevel` destination; match it against the
        // authored mission maps.
        let mut marked: Option<(i32, i32)> = None;
        let won = app
            .world()
            .get_resource::<peacecraft3::campaign_runtime::CampaignExitResult>()
            .and_then(|exit| exit.next_level().map(str::to_owned));
        if let Some(won) = won {
            let chapters = app.world().resource::<crate::native_campaign::Chapters>();
            for campaign in &chapters.campaigns {
                if let Some(row) = campaign.missions.iter().position(|m| m.map == won) {
                    let key = (
                        i32::try_from(campaign.source_index).unwrap_or(i32::MAX),
                        i32::try_from(row).unwrap_or(i32::MAX),
                    );
                    if progress.mark_complete(key.0, key.1).is_ok() {
                        marked = Some(key);
                    }
                    break;
                }
            }
        }
        let after = progress.next_incomplete(&order);
        serde_json::json!({
            "authored_missions": order.len(),
            "next_before": before,
            "order_complete_before": complete_before,
            "marked_complete": marked,
            "next_after": after,
            "order_complete_after": progress.order_complete(&order),
        })
    }
}

/// Virtual time per fixture update.
const STEP_SECONDS: f64 = 1.0 / 64.0;

/// Optional game time, in seconds, from an original-client schedule supplied by
/// the comparison runner (e.g. `PCX_PAIRED_SKIP_AT`, `PCX_PAIRED_CAPTURE_AT`).
fn paired_time(name: &str) -> Option<f64> {
    let value = std::env::var(name).ok()?;
    let seconds = value
        .parse::<f64>()
        .unwrap_or_else(|_| panic!("{name} must be seconds, got {value:?}"));
    assert!(seconds.is_finite() && seconds >= 0., "{name} out of range");
    Some(seconds)
}

/// Steps until the script clock reaches `seconds`, bounded by twice the updates
/// that time needs at [`STEP_SECONDS`].
fn advance_game_time(app: &mut App, seconds: f64) {
    let now = |app: &mut App| {
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .and_then(|host| host.objects().timer_game_time())
    };
    for _ in 0..(2.0 * seconds / STEP_SECONDS).ceil() as usize {
        if now(app).is_some_and(|t| t >= seconds) {
            return;
        }
        step(app);
    }
    panic!("game time {:?} did not reach {seconds}s", now(app));
}

#[derive(Resource)]
struct Target(Handle<Image>);

#[derive(Resource, Default)]
struct Capture(Option<Image>);

fn offscreen(target: Res<Target>, mut cameras: Query<&mut RenderTarget>) {
    for mut camera in &mut cameras {
        if matches!(*camera, RenderTarget::Window(_)) {
            *camera = RenderTarget::Image(target.0.clone().into());
        }
    }
}

fn step(app: &mut App) {
    app.update();
    app.world()
        .resource::<RenderDevice>()
        .wgpu_device()
        .poll(PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(20)),
        })
        .expect("GPU completion");
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "startup: {:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
}

fn until(app: &mut App, label: &str, ready: impl Fn(&mut World) -> bool) {
    for _ in 0..600 {
        step(app);
        if ready(app.world_mut()) {
            return;
        }
    }
    let navigation = app.world().resource::<crate::menu_screens::Navigation>();
    panic!(
        "{label} did not complete in 600 updates: screen={:?}, requested={:?}, transition={}, built={:?}, notice={:?}",
        navigation.screen,
        navigation.requested,
        navigation.transition_active,
        navigation.built,
        navigation.notice
    );
}

fn escape(app: &mut App, state: ButtonState) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Escape,
        logical_key: Key::Escape,
        state,
        repeat: false,
        window,
        text: None,
    });
}

fn activate(app: &mut App, entity: Entity) {
    assert!(app.world().entity(entity).contains::<Button>());
    assert!(!app.world().entity(entity).contains::<InteractionDisabled>());
    println!(
        "activate {:?}",
        app.world()
            .entity(entity)
            .get::<crate::glue_menu::GlueButton>()
            .map(|b| &b.frame)
    );
    app.world_mut().trigger(Activate { entity });
}

fn menu_key(app: &mut App, state: ButtonState) {
    let code = app
        .world()
        .resource::<CampaignCamera>()
        .menu_key
        .clone()
        .unwrap()
        .unwrap();
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        state,
        repeat: false,
        window,
        text: None,
    });
}

fn screen(app: &mut App, expected: crate::menu_screens::Screen) {
    until(app, "menu navigation", |world| {
        let navigation = world.resource::<crate::menu_screens::Navigation>();
        navigation.screen == expected
            && navigation.requested.is_none()
            && !navigation.transition_active
            && navigation.built == Some(expected)
            && if expected == crate::menu_screens::Screen::Campaign {
                world.contains_resource::<crate::mainmenu_scene::MainMenuScene>()
                    && world
                        .get_resource::<crate::menu_sprites::MenuPanels>()
                        .is_none_or(|panels| panels.retired())
            } else {
                world
                    .get_resource::<crate::menu_sprites::MenuPanels>()
                    .is_some_and(|panels| panels.navigation_settled())
            }
    });
}

fn frame_button(app: &mut App, frame: &str) -> Entity {
    app.world_mut()
        .query::<(Entity, &crate::glue_menu::GlueButton)>()
        .iter(app.world())
        .find(|(_, b)| b.frame == frame)
        .expect(frame)
        .0
}

fn capture_clocks(app: &mut App) -> serde_json::Value {
    let game_time = app
        .world_mut()
        .get_resource_mut::<MapScriptSession<MapStartupHost>>()
        .and_then(|mut session| {
            session
                .host()
                .and_then(|host| host.objects().timer_game_time())
        });
    serde_json::json!({
        "real": app.world().resource::<Time<Real>>().elapsed(),
        "virtual": app.world().resource::<Time<Virtual>>().elapsed(),
        "fixed": app.world().resource::<Time<Fixed>>().elapsed(),
        "game_seconds": game_time,
    })
}

fn with_frozen_capture_clocks<T>(app: &mut App, operation: impl FnOnce(&mut App) -> T) -> T {
    let before = capture_clocks(app);
    let strategy = std::mem::replace(
        &mut *app
            .world_mut()
            .resource_mut::<bevy::time::TimeUpdateStrategy>(),
        bevy::time::TimeUpdateStrategy::ManualDuration(Duration::ZERO),
    );
    let result = operation(app);
    let after = capture_clocks(app);
    *app.world_mut()
        .resource_mut::<bevy::time::TimeUpdateStrategy>() = strategy;
    assert_eq!(before, after, "screenshot acquisition advanced a clock");
    result
}

#[test]
fn capture_clock_hold_preserves_all_clocks_and_restores_the_update_strategy() {
    let mut app = App::new();
    let delta = Duration::from_secs_f64(STEP_SECONDS);
    app.add_plugins(bevy::time::TimePlugin)
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(delta));
    app.update();
    app.update();
    let before = capture_clocks(&mut app);
    with_frozen_capture_clocks(&mut app, |app| {
        for _ in 0..20 {
            app.update();
        }
    });
    assert_eq!(capture_clocks(&mut app), before);
    assert!(matches!(
        app.world().resource::<bevy::time::TimeUpdateStrategy>(),
        bevy::time::TimeUpdateStrategy::ManualDuration(value) if *value == delta
    ));
    app.update();
    assert_ne!(capture_clocks(&mut app), before);
}

fn capture(app: &mut App, output: &Path) -> Image {
    assert!(!output.exists(), "capture must be new");
    let clocks = capture_clocks(app);
    let image = with_frozen_capture_clocks(app, |app| {
        for _ in 0..12 {
            step(app);
        }
        let target = app.world().resource::<Target>().0.clone();
        app.world_mut().resource_mut::<Capture>().0 = None;
        app.world_mut().spawn(Screenshot::image(target)).observe(
            |event: On<ScreenshotCaptured>, mut capture: ResMut<Capture>| {
                capture.0 = Some(event.image.clone());
            },
        );
        until(app, "GPU screenshot", |w| {
            w.resource::<Capture>().0.is_some()
        });
        app.world_mut().resource_mut::<Capture>().0.take().unwrap()
    });
    image
        .clone()
        .try_into_dynamic()
        .unwrap()
        .save(output)
        .unwrap();
    std::fs::write(
        output.with_extension("clock.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "before": clocks,
            "after": capture_clocks(app),
            "scope": "Real, virtual, fixed and map clocks held during GPU acquisition. Original-client tick identity is not established by this receipt.",
        })).unwrap(),
    ).unwrap();
    image
}

fn color_snapshot(
    app: &mut App,
) -> (
    std::collections::BTreeMap<pcx_script::HandleId, i32>,
    Vec<u8>,
) {
    let colors = app
        .world()
        .resource::<crate::campaign_actors::UnitActors>()
        .rendered_colors();
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let objects = session.host().unwrap().objects();
    let mut units = 0;
    for (&id, &color) in &colors {
        if let Ok(unit) = objects.executed_unit(id) {
            let original = crate::campaign_actors::original_team_color(objects, unit).unwrap();
            assert_eq!(color, objects.display_team_color(unit, original).unwrap());
            units += 1;
        }
    }
    assert!(
        units > 0,
        "actual model instances must use the host's effective palette"
    );
    let image = app
        .world()
        .resource::<crate::campaign_minimap::CampaignMinimap>()
        .diagnostic_image()
        .unwrap();
    let pixels = app
        .world()
        .resource::<Assets<Image>>()
        .get(&image)
        .unwrap()
        .data
        .clone()
        .unwrap();
    (colors, pixels)
}

fn item_visibility_roundtrip(app: &mut App, output: &Path) {
    use pcx_script::{Host, Value};
    let items = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .executed_items()
        .map(|(id, item)| (id, item.visible()))
        .collect::<Vec<_>>();
    let Some(id) = items
        .iter()
        .find(|(_, visible)| *visible == Some(true))
        .map(|(id, _)| *id)
    else {
        // Human01 has no ground items at this stage: its six scripted
        // `CreateItemLoc` calls all wait on crate-death and story triggers,
        // and `war3mapUnits.doo` carries unit placements with drop sets
        // rather than item instances. The original shows no ground items
        // here either, so there is nothing to pick; record that instead of
        // inventing an item or forcing gameplay in a render diagnostic.
        std::fs::write(output.join("item-visibility.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "refusal": "no visible ground item at this chapter stage",
            "executed_items": items.len(),
            "scope": "source ground item actor root and actual ray picking; captures retained, no original pixel-parity claim"
        })).unwrap()).unwrap();
        return;
    };
    let (root, ray, far) = app
        .world()
        .resource::<crate::campaign_actors::UnitActors>()
        .item_probe(id)
        .expect("source item actor and nonempty bounds");
    let hit = |app: &mut App| {
        app.world_mut().resource_scope(
            |world, mut session: Mut<MapScriptSession<MapStartupHost>>| {
                let witness = session.session_witness().unwrap();
                world
                    .resource::<crate::campaign_actors::UnitActors>()
                    .item_hit(ray, far, &witness, session.host().unwrap().objects())
            },
        )
    };
    assert_eq!(hit(app), Some(id));
    for shown in [false, true] {
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .native("SetItemVisible", &[Value::Handle(id), Value::Bool(shown)])
            .unwrap();
        assert_eq!(
            hit(app),
            shown.then_some(id),
            "picking must see canonical state before projection"
        );
        step(app);
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            if shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            }
        );
        assert_eq!(hit(app), shown.then_some(id));
        capture(
            app,
            &output.join(if shown {
                "item-shown.png"
            } else {
                "item-hidden.png"
            }),
        );
    }
    std::fs::write(output.join("item-visibility.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "canonical_picking_before_projection": true, "actor_visibility_roundtrip": true,
        "scope": "source ground item actor root and actual ray picking; captures retained, no original pixel-parity claim"
    })).unwrap()).unwrap();
}

/// Controlled original-capture comparison probe for the selected-unit
/// portrait: composition, mask and speech timing from asset-derived captures.
///
/// Context (`docs/re-notes/held-inventory-presentation.md#animated-selected-unit-portrait`,
/// TRACKER portrait entry): idle keeps every authored `portrait` sequence
/// without a `talk` token with SEQS rarity re-drawn over each completed loop
/// through a deterministic splitmix draw (recreation choice, not native RNG
/// parity); speech holds the authored `portrait talk`/`talk` variant while the
/// unit's ack row stays live as a `GameSound` linked by retained unit id;
/// composition is a clipped transparent target over the authored console
/// backing; the portrait camera is explicitly despawned on selection switches
/// and same-handle hero revivals. A circular mask model/texture is not
/// sourced, so no new mask art is invented: checked sources (the `0x6416b0`
/// rectangle, the `0x620ee0` shared light, the admitted ConsoleUI and
/// SimpleInfoPanel FDF) expose no circular mask operand, and this probe
/// records the rectangular `Overflow::clip` composition instead of asserting
/// a fitted circle. All values below are read from the admitted session,
/// layout rules, sequences and renderer; captures and receipts stay local
/// under the caller-supplied output directory. Original captures remain
/// untracked. This harness does not turn any render gate green.
mod portrait_parity {
    use super::*;

    /// Exact RGBA comparison of two PNG file buffers. Refuses differing
    /// extents instead of rescaling. No tolerance, no normalization, no
    /// cropping: any channel difference in a pixel counts as changed. Mirrors
    /// the `tools/original-game-reference/reference.py report` rule the
    /// analyst runs against the original capture.
    pub(super) struct Comparison {
        pub(super) changed: usize,
        pub(super) total: usize,
        pub(super) bounds: Option<[[u32; 2]; 2]>,
        pub(super) first: Option<serde_json::Value>,
    }

    pub(super) fn compare_png_bytes(
        recreation: &[u8],
        original: &[u8],
        label: &str,
    ) -> Result<Comparison, String> {
        fn decode(bytes: &[u8], label: &str) -> Result<Image, String> {
            Image::from_buffer(
                bytes,
                bevy::image::ImageType::Extension("png"),
                bevy::image::CompressedImageFormats::NONE,
                true,
                bevy::image::ImageSampler::linear(),
                bevy::asset::RenderAssetUsages::default(),
            )
            .map_err(|error| format!("{label}: png decode refused: {error:?}"))
        }
        let recreation = decode(recreation, &format!("{label}: recreation"))?;
        let original = decode(original, &format!("{label}: original"))?;
        let size = recreation.texture_descriptor.size;
        let other = original.texture_descriptor.size;
        if size.width != other.width || size.height != other.height {
            return Err(format!(
                "{label}: extent differs, no rescaling: recreation {}x{} vs original {}x{}",
                size.width, size.height, other.width, other.height,
            ));
        }
        let Some(a) = recreation.data.as_deref() else {
            return Err(format!("{label}: recreation pixels unavailable"));
        };
        let Some(b) = original.data.as_deref() else {
            return Err(format!("{label}: original pixels unavailable"));
        };
        if a.len() != b.len() {
            return Err(format!("{label}: pixel buffer length differs"));
        }
        let width = size.width;
        let mut changed = 0usize;
        let mut bounds: Option<[[u32; 2]; 2]> = None;
        let mut first: Option<serde_json::Value> = None;
        for (index, (pixel_a, pixel_b)) in a
            .as_chunks::<4>()
            .0
            .iter()
            .zip(b.as_chunks::<4>().0.iter())
            .enumerate()
        {
            if pixel_a != pixel_b {
                changed += 1;
                let x = (index as u32) % width;
                let y = (index as u32) / width;
                bounds = Some(match bounds {
                    None => [[x, y], [x, y]],
                    Some([[x0, y0], [x1, y1]]) => [[x0.min(x), y0.min(y)], [x1.max(x), y1.max(y)]],
                });
                if first.is_none() {
                    first = Some(serde_json::json!({
                        "x": x, "y": y, "recreation": pixel_a, "original": pixel_b,
                    }));
                }
            }
        }
        Ok(Comparison {
            changed,
            total: a.len() / 4,
            bounds,
            first,
        })
    }

    /// Retains the recreation side of the controlled portrait comparison at
    /// the unchanged campaign-control checkpoint with a live local selection.
    /// Composition (authored rectangle, viewport, computed mask/parent sizes,
    /// transparent clear, rectangular clip), mask (explicit rectangular clip;
    /// no circular operand admitted) and speech timing (authored idle/talk
    /// clips with SEQS rarity/duration/looping, current idle position/cycle,
    /// ack row and live `GameSound` rows) are all read from the admitted
    /// scene, layout rules and renderer. The original runner repeats the same
    /// map, selection, camera and ack timing, then the analyst compares with
    /// `reference.py report` under exact RGB equality.
    pub(super) fn retain_control(
        app: &mut App,
        output: &Path,
        install: &str,
        map: &str,
        expansion: bool,
    ) {
        let scene = app
            .world()
            .get_resource::<crate::campaign_portrait::CampaignPortrait>()
            .and_then(|portrait| portrait.parity_scene());
        let Some(scene) = scene else {
            let receipt = serde_json::json!({
                "install": install, "map": map, "expansion": expansion,
                "refusal": "no live portrait scene at the control checkpoint (no single live selection)",
                "scope": "portrait parity probe; unchanged checkpoint; no pixel-parity claim",
            });
            std::fs::write(
                output.join("portrait-parity.json"),
                serde_json::to_vec_pretty(&receipt).unwrap(),
            )
            .unwrap();
            return;
        };
        let binding = app
            .world()
            .get_resource::<crate::campaign_unit_panel::UnitPanel>()
            .and_then(|panel| {
                panel.portrait_binding().map(|(rules, viewport, parent)| {
                    (
                        rules.portrait.rect(viewport[0] / viewport[1]).ok(),
                        rules.portrait.light,
                        viewport,
                        parent,
                    )
                })
            });
        let (authored_rect, light, viewport, parent) = match binding {
            Some((rect, light, viewport, parent)) => (rect, Some(light), viewport, Some(parent)),
            None => (None, None, [0., 0.], None),
        };
        let authored_rect_json = authored_rect.map(|rect| {
            serde_json::json!({
                "x": rect.x, "y": rect.y,
                "width": rect.width, "height": rect.height,
            })
        });
        let light_json = light.map(|sun| {
            serde_json::json!({
                "direction": sun.direction,
                "colour": sun.colour,
                "intensity": sun.intensity,
                "ambient_colour": sun.ambient_colour,
                "ambient_intensity": sun.ambient_intensity,
            })
        });
        let (parent_size, mask_size, mask_clip, camera_active, camera_clear) = {
            let world = app.world();
            let parent_size = parent
                .and_then(|parent| world.get::<ComputedNode>(parent))
                .map(|node| [node.size().x, node.size().y]);
            let mask_size = world
                .get::<ComputedNode>(scene.mask)
                .map(|node| [node.size().x, node.size().y]);
            let mask_clip = world
                .get::<Node>(scene.mask)
                .is_some_and(|node| node.overflow == Overflow::clip());
            let (camera_active, camera_clear) = world
                .get::<Camera>(scene.camera_entity)
                .map(|camera| (camera.is_active, format!("{:?}", camera.clear_color)))
                .unwrap_or((false, "missing".to_owned()));
            (
                parent_size,
                mask_size,
                mask_clip,
                camera_active,
                camera_clear,
            )
        };
        let (ack, live_rows) = {
            let ack = app
                .world()
                .get_resource::<crate::campaign_audio::CampaignAudio>()
                .and_then(|audio| {
                    audio.speaking.clone().map(
                        |(unit, row)| serde_json::json!({"unit": format!("{unit:?}"), "row": row}),
                    )
                });
            let mut query = app
                .world_mut()
                .query_filtered::<&crate::campaign_audio::GameSound, ()>();
            let world = app.world();
            let mut rows = Vec::new();
            for sound in query.iter(world) {
                rows.push(sound.name.clone());
            }
            rows.sort();
            rows.dedup();
            (ack, rows)
        };
        // Self-comparison exercises the exact-compare path on real
        // asset-derived bytes: identical inputs must report zero changes.
        let self_check = match std::fs::read(output.join("selected-unit.png")) {
            Ok(bytes) => match compare_png_bytes(&bytes, &bytes, "portrait self") {
                Ok(comparison) => serde_json::json!({
                    "changed": comparison.changed, "total": comparison.total,
                    "bounds": comparison.bounds, "first": comparison.first,
                }),
                Err(error) => serde_json::json!({ "refusal": error }),
            },
            Err(error) => {
                serde_json::json!({ "refusal": format!("selected-unit frame unread: {error}") })
            }
        };
        // Optional paired original supplied by the analyst; untracked, never
        // committed. When absent the probe retains the recreation side only.
        let paired = match std::env::var("PCX_PORTRAIT_ORIGINAL") {
            Ok(path) => match (
                std::fs::read(output.join("selected-unit.png")),
                std::fs::read(&path),
            ) {
                (Ok(recreation), Ok(original)) => {
                    match compare_png_bytes(&recreation, &original, "portrait paired") {
                        Ok(comparison) => serde_json::json!({
                            "original": path,
                            "changed": comparison.changed, "total": comparison.total,
                            "bounds": comparison.bounds, "first": comparison.first,
                        }),
                        Err(error) => serde_json::json!({"original": path, "refusal": error}),
                    }
                }
                (Err(error), _) => {
                    serde_json::json!({"original": path, "refusal": format!("recreation unread: {error}")})
                }
                (_, Err(error)) => {
                    serde_json::json!({"original": path, "refusal": format!("original unread: {error}")})
                }
            },
            Err(_) => {
                serde_json::json!({"refusal": "PCX_PORTRAIT_ORIGINAL unset; no original supplied"})
            }
        };
        let receipt = serde_json::json!({
            "install": install, "map": map, "expansion": expansion,
            "unit": format!("{:?}", scene.unit),
            "model": scene.model,
            "source_member": scene.source_member,
            "color": scene.color,
            "viewport": viewport,

            "authored_rect": authored_rect_json,
            "parent_size": parent_size,
            "mask_size": mask_size,
            "mask_clip_rectangular": mask_clip,
            "mask_note": "rectangular Overflow::clip over the authored console backing; no circular mask model/texture operand is admitted, so none is asserted",
            "camera": {
                "name": scene.camera_name,
                "eye": scene.camera_eye, "target": scene.camera_target,
                "fov": scene.camera_fov, "near": scene.camera_near, "far": scene.camera_far,
                "is_active": camera_active, "clear": camera_clear,
                "retirement": "explicit camera despawn on selection switch and same-handle hero revival; a stale camera must not survive",
            },
            "composition": {
                "target_size": scene.target_size,
                "tint": scene.tint,
                "light": light_json,
                "receipts": scene.receipt_members,
            },
            "idle": scene.idle.iter().map(|clip| serde_json::json!({
                "name": clip.name, "rarity": clip.rarity,
                "duration_ms": clip.duration_ms, "non_looping": clip.non_looping,
                "interval": clip.interval,
            })).collect::<Vec<_>>(),
            "talk": scene.talk.as_ref().map(|clip| serde_json::json!({
                "name": clip.name, "rarity": clip.rarity,
                "duration_ms": clip.duration_ms, "non_looping": clip.non_looping,
                "interval": clip.interval,
            })),
            "timing": {
                "idle_pos": scene.idle_pos, "idle_cycle": scene.idle_cycle,
                "speaking": scene.speaking, "ack": ack, "live_rows": live_rows,
                "elapsed_secs": scene.elapsed_secs, "idle_started_secs": scene.idle_started_secs,
                "rule": "Talk holds exactly while the retained unit's ack GameSound stays live; silence returns to the current rarity-weighted idle; completed idle loops re-draw over authored SEQS weights via the deterministic splitmix recreation choice; non-looping Talk loops while the voice plays and non-looping idle advances at its clamp bound",
            },
            "last_error": scene.last_error,
            "frames_self_check": self_check,
            "paired": paired,
            "arms": {
                "composition": "authored portrait rectangle over the console backing with a transparent target",
                "mask": "rectangular clip only; circular mask is not sourced and is not asserted",
                "speech": "authored Talk variant while the ack row is live; rarity-weighted authored idle otherwise",
            },
            "original_runner": {
                "paired_skip_at": "PCX_PAIRED_SKIP_AT",
                "paired_capture_at": "PCX_PAIRED_CAPTURE_AT",
                "portrait_original": "PCX_PORTRAIT_ORIGINAL",
                "report": "tools/original-game-reference/reference.py report",
                "rule": "exact RGB equality, no tolerance, no rescaling, no exclusions",
            },
            "scope": "Controlled portrait composition/mask/speech-timing harness at the unchanged campaign-control checkpoint; records asset-derived recreation probes for comparison against the original capture. No pixel-parity claim.",
        });
        std::fs::write(
            output.join("portrait-parity.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
    }
}

#[test]
#[ignore = "requires explicit original install/edition/map, Vulkan and a new PCX_CAMPAIGN_CAPTURE_DIR"]
fn mounted_menu_enters_the_rendered_campaign() {
    check_mounted_campaign(false, false, false);
}

#[test]
#[ignore = "requires explicit original install/edition/map, Vulkan and a new PCX_CAMPAIGN_CAPTURE_DIR"]
fn mounted_campaign_hud_and_research_states() {
    check_mounted_campaign(true, false, false);
}

#[test]
#[ignore = "requires explicit original install/edition/map, Vulkan and a new PCX_CAMPAIGN_CAPTURE_DIR"]
fn mounted_campaign_rendering_state() {
    check_mounted_campaign(false, true, false);
}

#[test]
#[ignore = "requires explicit original install/edition/map, Vulkan and a new PCX_CAMPAIGN_CAPTURE_DIR"]
fn mounted_campaign_render_cpu_workload() {
    check_mounted_campaign(false, false, true);
}

// Keep layout evidence at the exact screenshot checkpoint, before later HUD interactions.
fn retain_text_layouts(app: &mut App, output: &Path) {
    use bevy::text::TextLayoutInfo;
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &Text, &TextFont, &TextLayoutInfo, Option<&Children>)>();
    let records = query
        .iter(world)
        .map(|(entity, text, font, layout, children)| {
            let spans = children
                .into_iter()
                .flat_map(|children| children.iter())
                .filter_map(|child| world.get::<TextSpan>(child).map(|span| span.0.clone()))
                .collect::<Vec<_>>();
            let glyphs = layout
                .glyphs
                .iter()
                .map(|glyph| {
                    serde_json::json!({
                        "position": glyph.position.to_array(),
                        "atlas_min": glyph.atlas_info.rect.min.to_array(),
                        "atlas_max": glyph.atlas_info.rect.max.to_array(),
                        "atlas_offset": glyph.atlas_info.offset.to_array(),
                        "alpha_mask": glyph.atlas_info.is_alpha_mask,
                        "section": glyph.section_index, "line": glyph.line_index,
                    })
                })
                .collect::<Vec<_>>();
            let mut ancestry = Vec::new();
            let mut current = Some(entity);
            while let Some(node_entity) = current {
                assert!(
                    ancestry.len() < 256,
                    "text ancestry exceeded diagnostic bound"
                );
                ancestry.push(serde_json::json!({
                    "entity": format!("{node_entity:?}"),
                    "center": world.get::<UiGlobalTransform>(node_entity)
                        .map(|transform| transform.translation.to_array()),
                    "size": world.get::<ComputedNode>(node_entity)
                        .map(|node| node.size().to_array()),
                    "unrounded_size": world.get::<ComputedNode>(node_entity)
                        .map(|node| node.unrounded_size().to_array()),
                }));
                current = world.get::<ChildOf>(node_entity).map(ChildOf::parent);
            }
            serde_json::json!({
                "entity": format!("{entity:?}"), "text": text.0, "spans": spans,
                "font_size": format!("{:?}", font.font_size),
                "scale_factor": layout.scale_factor, "size": layout.size.to_array(),
                "glyphs": glyphs,
                "source_text_frame": world.get::<crate::hud_render::SourceTextBox>(entity).map(|frame| format!("{frame:?}")),
                "ancestry": ancestry,
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(output.join("selected-text-layouts.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "scope": "Bevy text layout at selected-unit capture; local diagnostic, not native placement proof",
            "records": records,
        })).unwrap()).unwrap();
}

fn mounted_main_menu(install: &str, expansion: bool, output: &Path, cpu_workload: bool) -> App {
    // Diagnostic viewport and work bounds, not source layout or timing values.
    let [width, height] = [1280, 960];
    let plugins = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (width, height).into(),
                ..default()
            }),
            exit_condition: ExitCondition::DontExit,
            ..default()
        })
        .set(AssetPlugin {
            file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
            ..default()
        })
        .set(RenderPlugin {
            synchronous_pipeline_compilation: true,
            render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                backends: Some(Backends::VULKAN),
                ..default()
            })),
            ..default()
        })
        .disable::<WinitPlugin>()
        .disable::<bevy::audio::AudioPlugin>();
    // The CPU workload keeps the render world on the update thread so one
    // counter window covers extraction and render-graph preparation.
    let plugins = if cpu_workload {
        plugins.disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
    } else {
        plugins
    };
    if cpu_workload {
        bench::install_span_profile();
    }
    let mut app = crate::launch_gui::app_with_plugins(plugins, |_| {});
    app.init_asset::<AudioSource>();
    app.world_mut()
        .resource_mut::<peacecraft3::campaign_startup::CampaignSeed>()
        .requested = Some(1);
    app.insert_resource(crate::launch_gui::Launcher::test_source(
        install.to_owned(),
        expansion,
    ))
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f64(STEP_SECONDS),
    ))
    .init_resource::<Capture>();
    let mut image = Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(Target(target)).add_systems(
        PostUpdate,
        offscreen
            .before(UiSystems::Layout)
            .before(bevy::camera::CameraUpdateSystems),
    );
    app.finish();
    app.cleanup();
    step(&mut app);
    let mut text = app.world_mut().query::<(Entity, &Text)>();
    let mut button = text
        .iter(app.world())
        .find(|(_, t)| t.0 == "Main menu")
        .expect("launcher setup action that starts the intro")
        .0;
    while !app.world().entity(button).contains::<Button>() {
        button = app
            .world()
            .entity(button)
            .get::<ChildOf>()
            .unwrap()
            .parent();
    }
    let menu_entry = cpu_workload.then(|| bench::EntryWindow::start("enter_menu"));
    activate(&mut app, button);
    escape(&mut app, ButtonState::Pressed);
    step(&mut app);
    escape(&mut app, ButtonState::Released);
    // The game main menu follows the intro; the launcher is only setup.
    screen(&mut app, crate::menu_screens::Screen::Main);
    if let Some(window) = menu_entry {
        window.finish(output);
    }
    capture(&mut app, &output.join("main-menu.png"));
    app
}

fn check_mounted_campaign(hud_only: bool, render_only: bool, cpu_workload: bool) {
    let install = std::env::var("PCX_NATIVE_CHAPTER_INSTALL").expect("PCX_NATIVE_CHAPTER_INSTALL");
    let map = std::env::var("PCX_NATIVE_CHAPTER_MAP").expect("PCX_NATIVE_CHAPTER_MAP");
    let expansion = match std::env::var("PCX_NATIVE_CHAPTER_ADDON").as_deref() {
        Ok("roc") => false,
        Ok("tft") => true,
        _ => panic!("select roc or tft explicitly"),
    };
    let output = std::path::PathBuf::from(
        std::env::var_os("PCX_CAMPAIGN_CAPTURE_DIR").expect("PCX_CAMPAIGN_CAPTURE_DIR"),
    );
    assert!(!output.exists(), "output directory must be new");
    std::fs::create_dir_all(&output).unwrap();
    let mut app = mounted_main_menu(&install, expansion, &output, cpu_workload);
    let mut workload = cpu_workload.then(bench::Harness::new);
    if let Some(workload) = workload.as_mut() {
        workload.menu(&mut app, &output);
    }
    let button = frame_button(&mut app, "SinglePlayerButton");
    activate(&mut app, button);
    screen(&mut app, crate::menu_screens::Screen::SinglePlayer);
    let button = frame_button(&mut app, "CampaignButton");
    activate(&mut app, button);
    screen(&mut app, crate::menu_screens::Screen::Campaign);
    let chapters = app.world().resource::<crate::native_campaign::Chapters>();
    assert!(chapters.error.is_none(), "{:?}", chapters.error);
    let (campaign, mission) = chapters
        .campaigns
        .iter()
        .enumerate()
        .find_map(|(c, entry)| {
            entry
                .missions
                .iter()
                .position(|m| m.map.eq_ignore_ascii_case(&map))
                .map(|m| (c, m))
        })
        .expect("map in selected source index");
    assert!(
        chapters.campaigns[campaign].default_open,
        "campaign must be source-unlocked"
    );
    until(&mut app, "mounted campaign buttons", |w| {
        w.query::<&crate::native_campaign::CampaignButton>()
            .iter(w)
            .any(|b| b.0 == campaign)
    });
    let button = app
        .world_mut()
        .query::<(Entity, &crate::native_campaign::CampaignButton)>()
        .iter(app.world())
        .find(|(_, b)| b.0 == campaign)
        .unwrap()
        .0;
    activate(&mut app, button);
    until(&mut app, "chapter panel", |w| {
        w.resource::<crate::native_campaign::CampaignMenuState>()
            .selected_campaign
            == Some(campaign)
    });
    assert!(
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .is_none()
    );
    capture(&mut app, &output.join("chapter-selection.png"));
    until(&mut app, "mounted mission buttons", |w| {
        w.query::<&crate::native_campaign::MissionButton>()
            .iter(w)
            .any(|b| b.campaign == campaign && b.mission == mission)
    });
    let button = app
        .world_mut()
        .query::<(Entity, &crate::native_campaign::MissionButton)>()
        .iter(app.world())
        .find(|(_, b)| b.campaign == campaign && b.mission == mission)
        .unwrap()
        .0;
    let campaign_entry = cpu_workload.then(|| bench::EntryWindow::start("enter_campaign"));
    activate(&mut app, button);
    until(&mut app, "campaign startup", |w| {
        w.resource::<CampaignCamera>().active()
            && w.resource::<MapScriptSession<MapStartupHost>>()
                .context()
                .is_some()
    });
    if let Some(window) = campaign_entry {
        window.finish(&output);
    }
    capture(&mut app, &output.join("campaign-entry.png"));
    if let Some(at) = paired_time("PCX_PAIRED_SKIP_AT") {
        advance_game_time(&mut app, at);
    }
    let mut controlled = false;
    // Diagnostic input schedule: do not assume when the map enables cinematic skipping.
    for _ in 0..300 {
        escape(&mut app, ButtonState::Pressed);
        step(&mut app);
        escape(&mut app, ButtonState::Released);
        step(&mut app);
        controlled = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .is_some_and(|host| {
                let objects = host.objects();
                objects.presentation().is_some_and(|state| {
                    state.control_enabled == Some(true)
                        && state.interface_visible == Some(true)
                        && state.filter.as_ref().is_none_or(|filter| {
                            filter
                                .sample(objects.timer_game_time())
                                .is_ok_and(|sample| sample.color[3] == 0.)
                        })
                })
            });
        if controlled {
            break;
        }
    }
    assert!(
        controlled,
        "cinematic Escape input must return control within 600 updates"
    );
    capture(&mut app, &output.join("campaign-control.png"));
    if let Some(at) = paired_time("PCX_PAIRED_CAPTURE_AT") {
        advance_game_time(&mut app, at);
    }
    // Exercise the real mounted console, rather than supplying a synthetic parent.
    let selected = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects_mut();
        let unit = objects
            .executed_units()
            .filter(|(_, unit)| {
                unit.shown
                    && unit.vitals.as_ref().is_some_and(|v| v.life > 0.)
                    && objects
                        .player_settings(unit.owner)
                        .is_ok_and(|player| Some(player.number) == objects.local_player_number())
            })
            .max_by_key(|(_, unit)| unit.abilities.as_ref().is_some_and(|a| a.hero.is_some()))
            .map(|(id, _)| id)
            .expect("a live local campaign unit");
        objects.select_units(&[unit], true);
        unit
    };
    let selected_image = capture(&mut app, &output.join("selected-unit.png"));
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects();
        let grid = objects
            .native_sight_grid_fixture()
            .expect("selected-client height lattice");
        let heights = grid.heights();
        std::fs::write(
            output.join("sight-height-grid.bin"),
            heights
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let [width, height] = grid.dimensions();
        let bounds = objects.vision_world_bounds().unwrap();
        let viewer = objects.local_player_settings().unwrap().identity;
        let sources = objects.vision_sources(viewer, false, 65536).unwrap();
        let sources: Vec<_> = sources.iter().map(|source| {
            let unit = match source.origin {
                pcx_play::playercolor_host::VisionOrigin::Unit { unit } => objects.executed_unit(unit).ok(),
                _ => None,
            };
            serde_json::json!({ "shape": format!("{:?}", source.shape),
                "rawcode": unit.map(|u| String::from_utf8_lossy(&u.definition.rawcode).into_owned()),
                "owner": unit.map(|u| u.executed_owner),
            })
        }).collect();
        std::fs::write(
            output.join("sight-sources.json"),
            serde_json::to_vec_pretty(&sources).unwrap(),
        )
        .unwrap();
        let mut cells = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let point = [x as f32 + 0.5, y as f32 + 0.5];
                let point = std::array::from_fn(|i| {
                    bounds[0][i]
                        + point[i] * (bounds[1][i] - bounds[0][i]) / [width, height][i] as f32
                });
                let cell = objects.vision_cell(viewer, point).unwrap().unwrap();
                cells.extend([
                    u8::from(cell.raw == pcx_play::playercolor_host::VisionState::Visible),
                    u8::from(cell.explored),
                ]);
            }
        }
        std::fs::write(output.join("sight-viewer-grid.bin"), cells).unwrap();
        std::fs::write(output.join("sight-grid.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "dimensions": [width,height], "world_bounds":bounds,
            "scope":"Row-major current-visible/explored byte pairs for local viewer; native tick identity unqualified",
        })).unwrap()).unwrap();
    }

    let minimap = app
        .world()
        .resource::<crate::campaign_minimap::CampaignMinimap>()
        .diagnostic_image()
        .unwrap();
    let image = app
        .world()
        .resource::<Assets<Image>>()
        .get(&minimap)
        .unwrap();
    std::fs::write(
        output.join("minimap-cpu.rgba"),
        image.data.as_ref().unwrap(),
    )
    .unwrap();
    let (outline, viewport) = app
        .world()
        .resource::<crate::campaign_minimap::CampaignMinimap>()
        .diagnostic_outline()
        .unwrap();
    let geometry = app.world().get::<ComputedNode>(outline).unwrap();
    let line_geometry: Vec<_> = app.world().get::<Children>(outline).unwrap().iter().map(|entity| {
        let node = app.world().get::<ComputedNode>(entity).unwrap();
        let transform = app.world().get::<UiGlobalTransform>(entity).unwrap();
        serde_json::json!({"center": transform.translation.to_array(), "size": node.size().to_array()})
    }).collect();

    let camera_entity = app
        .world()
        .resource::<peacecraft3::campaign_camera::CampaignCamera>()
        .entity
        .unwrap();
    let camera_transform = app.world().get::<GlobalTransform>(camera_entity).unwrap();
    let camera_projection = app.world().get::<Projection>(camera_entity).unwrap();
    let camera_view = app.world().get::<Camera>(camera_entity).unwrap();

    std::fs::write(
        output.join("minimap-outline.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "viewport": viewport,
        "camera_transform": camera_transform.to_matrix().to_cols_array(),
        "camera_projection": format!("{camera_projection:?}"),
        "camera_viewport": camera_view.logical_viewport_size().unwrap().to_array(),
        "computed_projection": camera_view.clip_from_view().to_cols_array(),

            "layer_size": geometry.size().to_array(),
            "lines": line_geometry,
            "unrounded_size": geometry.unrounded_size().to_array(),
            "center": app.world().get::<UiGlobalTransform>(outline).unwrap().translation.to_array(),
            "scope": "Production outline geometry; native camera/tick identity remains unqualified"
        }))
        .unwrap(),
    )
    .unwrap();
    let quads = app
        .world_mut()
        .query::<(
            &ImageNode,
            &crate::hud_render::SourceImageRect,
            Option<&Children>,
        )>()
        .iter(app.world())
        .filter(|(image, _, _)| image.image == minimap)
        .inspect(|(_, _, children)| {
            assert!(
                children.is_none(),
                "fractional minimap image must remain a leaf"
            );
        })
        .count();
    assert_eq!(
        quads, 1,
        "production minimap must use the source image quad"
    );
    let mut raster_roots = 0;
    let mut plain_raster_roots = 0;
    for (_, font, children) in app
        .world_mut()
        .query::<(&crate::hud_render::SourceFont, &TextFont, Option<&Children>)>()
        .iter(app.world())
    {
        assert!(
            matches!(font.font_size, bevy::text::FontSize::Px(_)),
            "source font must reach pixel size before capture"
        );
        plain_raster_roots += usize::from(children.is_none());
        for child in children.into_iter().flat_map(Children::iter) {
            if let Some(span) = app.world().get::<TextFont>(child) {
                assert_eq!(
                    span.font_size, font.font_size,
                    "rich span raster height differs from its root"
                );
            }
        }
        raster_roots += 1;
    }
    assert!(raster_roots > 0, "source font roots are absent");
    assert!(plain_raster_roots > 0, "plain source font roots are absent");
    retain_text_layouts(&mut app, &output);
    if hud_only {
        let repeat = capture(&mut app, &output.join("selected-unit-repeat.png"));
        assert_eq!(
            selected_image.data, repeat.data,
            "same-tick gameplay captures differ"
        );
    }
    assert!(
        selected_image
            .data
            .as_ref()
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[..3] != [0, 0, 0]),
        "selected-unit capture must contain rendered world/HUD pixels"
    );
    // Controlled portrait parity probe at the unchanged checkpoint: the
    // selection above is the live canonical unit the probe snapshots.
    portrait_parity::retain_control(&mut app, &output, &install, &map, expansion);
    if let Some(mut workload) = workload {
        workload.campaign(&mut app, &output, selected);
        workload.finish(&mut app, &output);
        return;
    }
    if render_only {
        let scene_image = app.world_mut().query_filtered::<&RenderTarget, With<peacecraft3::campaign_camera::CampaignSceneCamera>>()
            .single(app.world()).unwrap().as_image().unwrap().clone();
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&scene_image)
            .unwrap()
            .texture_descriptor
            .usage |= TextureUsages::COPY_SRC;
        let output_image = app.world().resource::<Target>().0.clone();
        app.world_mut().resource_mut::<Target>().0 = scene_image;
        let world_image = capture(&mut app, &output.join("world-target.png"));
        app.world_mut().resource_mut::<Target>().0 = output_image;
        let nodes = app
            .world_mut()
            .query::<(
                Entity,
                &Name,
                Option<&Node>,
                Option<&ComputedNode>,
                Option<&InheritedVisibility>,
                Option<&UiTargetCamera>,
            )>()
            .iter(app.world())
            .filter(|(_, name, ..)| name.as_str() == "World composition image")
            .map(|row| format!("{row:?}"))
            .collect::<Vec<_>>();
        std::fs::write(output.join("composition-nodes.txt"), nodes.join("\n")).unwrap();
        let composition = app
            .world_mut()
            .query::<(
                Entity,
                Option<&Name>,
                &Camera,
                &RenderTarget,
                Option<&bevy::render::view::Msaa>,
                Option<&bevy::camera::CompositingSpace>,
            )>()
            .iter(app.world())
            .map(|row| format!("{row:?}"))
            .collect::<Vec<_>>();
        std::fs::write(
            output.join("camera-composition.txt"),
            composition.join("\n"),
        )
        .unwrap();
        let cameras = app.world_mut().query_filtered::<(&Transform, &Projection, &Camera), With<peacecraft3::campaign_camera::CampaignSceneCamera>>()
            .iter(app.world()).map(|(transform, projection, camera)| format!("{transform:?} {projection:?} {camera:?}")).collect::<Vec<_>>();
        let actors = app.world().resource::<crate::campaign_actors::UnitActors>();
        let destructables = actors.rendered_destructables();
        let actor_error = actors.last_error.clone();
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects();
        let now = objects.timer_game_time().unwrap();
        let models = objects.executed_units().filter_map(|(id, unit)| {
            let animation = unit.animation.as_ref()?;
            Some(serde_json::json!({"unit": format!("{id:?}"), "definition": format!("{:?}", unit.definition.rawcode),
                "model": animation.model.member, "origin": format!("{:?}", animation.model.origin),
                "sequence": animation.sequence, "sample": format!("{:?}", animation.sample(now)),
                "shown": unit.shown}))
        }).collect::<Vec<_>>();
        std::fs::write(output.join("render-state.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "clock": now, "day_fraction": objects.day_fraction(), "cameras": cameras, "models": models,
            "rendered_destructables": destructables, "actor_error": actor_error,
            "scope": "Diagnostic source scene; effective graphics settings and stock camera/time pairing remain unqualified"
        })).unwrap()).unwrap();
        let viewport = app
            .world_mut()
            .query_filtered::<&Camera, With<peacecraft3::campaign_camera::CampaignSceneCamera>>()
            .single(app.world())
            .unwrap()
            .viewport
            .clone()
            .unwrap();
        let pixel = viewport.physical_position + viewport.physical_size / 2;
        let selected_image = selected_image.try_into_dynamic().unwrap().to_rgba8();
        let world_image = world_image.try_into_dynamic().unwrap().to_rgba8();
        let expected = world_image.get_pixel(pixel.x, pixel.y);
        let actual = selected_image.get_pixel(pixel.x, pixel.y);
        assert!(
            expected.0[..3].iter().any(|v| *v != 0),
            "fixture needs visible ground at the viewport centre"
        );
        for channel in 0..3 {
            assert!(
                actual[channel].abs_diff(expected[channel]) <= 2,
                "world was lost beneath HUD: {actual:?} != {expected:?}"
            );
        }
        assert!(
            destructables > 0,
            "fixture needs visible canonical destructables: {actor_error:?}"
        );
        let splat_error = app
            .world()
            .resource::<crate::campaign_uber_splats::CampaignUberSplats>()
            .last_error
            .clone();
        assert!(splat_error.is_none(), "ubersplat failed: {splat_error:?}");
        let splats = app
            .world_mut()
            .query_filtered::<(), With<crate::campaign_uber_splats::UberSplat>>()
            .iter(app.world())
            .count();
        assert!(
            splats > 0,
            "fixture needs source ubersplats under buildings"
        );
        return;
    }
    if hud_only {
        until(&mut app, "authored quest publication", |world| {
            world
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .and_then(|host| host.objects().quest_session())
                .is_some_and(|quests| {
                    pcx_play::quest_view::project(quests)
                        .unwrap()
                        .quests
                        .iter()
                        .any(|quest| quest.enabled && quest.discovered && !quest.items.is_empty())
                })
        });
        let button = app
            .world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, name)| name.as_str() == "UpperButtonBarQuestsButton")
            .expect("authored Quests button")
            .0;
        let normal = app.world().get::<ImageNode>(button).unwrap().rect;
        #[derive(Resource)]
        struct ButtonVisualDiagnostic {
            frame: &'static str,
            pressed: bool,
        }
        app.add_systems(
            Update,
            (|mut commands: Commands,
              diagnostic: Option<Res<ButtonVisualDiagnostic>>,
              buttons: Query<(Entity, &Name)>| {
                let Some(diagnostic) = diagnostic else {
                    return;
                };
                for (button, name) in &buttons {
                    if name.as_str() != diagnostic.frame {
                        continue;
                    }
                    commands
                        .entity(button)
                        .insert(bevy::picking::hover::Hovered(true));
                    if diagnostic.pressed {
                        commands.entity(button).insert(bevy::ui::Pressed);
                    }
                }
            })
            .before(crate::campaign_hud::upper_buttons::CampaignHudSet),
        );
        app.insert_resource(ButtonVisualDiagnostic {
            frame: "UpperButtonBarQuestsButton",
            pressed: false,
        });
        capture(&mut app, &output.join("upper-button-hover.png"));
        app.world_mut()
            .entity_mut(button)
            .insert(bevy::ui::InteractionDisabled);
        capture(&mut app, &output.join("upper-button-disabled.png"));
        assert_ne!(app.world().get::<ImageNode>(button).unwrap().rect, normal);
        app.world_mut()
            .entity_mut(button)
            .remove::<bevy::ui::InteractionDisabled>();

        app.world_mut().remove_resource::<ButtonVisualDiagnostic>();
        step(&mut app);

        app.world_mut().entity_mut(button).insert(bevy::ui::Pressed);
        capture(&mut app, &output.join("upper-button-pressed.png"));
        assert_ne!(app.world().get::<ImageNode>(button).unwrap().rect, normal);
        assert!(
            !app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .quest_log_open,
            "a visual-state diagnostic must not activate the quest log"
        );
        app.world_mut()
            .entity_mut(button)
            .remove::<bevy::ui::Pressed>();
        step(&mut app);
        assert_eq!(app.world().get::<ImageNode>(button).unwrap().rect, normal);
        app.world_mut()
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities
            .push(button);
        step(&mut app);
        assert!(
            app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .quest_log_open
        );
        let pane = app
            .world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, name)| name.as_str() == "QuestDisplayBackdrop")
            .expect("source details pane")
            .0;
        let parts = app
            .world()
            .get::<Children>(pane)
            .expect("details artwork children");
        assert!(
            parts
                .iter()
                .any(|part| app.world().get::<ImageNode>(part).is_some()),
            "details pane mounts its border"
        );
        assert!(
            parts.iter().any(|part| app
                .world()
                .get::<MaterialNode<crate::glue_menu::BackdropImage>>(part)
                .is_some()),
            "details pane mounts its tiled background"
        );
        capture(&mut app, &output.join("quest-log.png"));
        app.insert_resource(ButtonVisualDiagnostic {
            frame: "QuestAcceptButton",
            pressed: false,
        });
        capture(&mut app, &output.join("quest-done-hover.png"));
        for frame in ["QuestItemListContainer", "QuestDisplay"] {
            let area = app
                .world_mut()
                .query::<(Entity, &Name)>()
                .iter(app.world())
                .find(|(_, name)| name.as_str() == frame)
                .expect("source content frame")
                .0;
            assert!(
                app.world()
                    .get::<bevy::ui_widgets::ScrollArea>(area)
                    .is_some(),
                "{frame} must keep overflowing text reachable"
            );
        }

        app.world_mut()
            .resource_mut::<ButtonVisualDiagnostic>()
            .pressed = true;
        capture(&mut app, &output.join("quest-done-pressed.png"));
        app.world_mut().remove_resource::<ButtonVisualDiagnostic>();
        let close = app
            .world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, name)| name.as_str() == "QuestAcceptButton")
            .expect("current authored Done button")
            .0;
        app.world_mut()
            .entity_mut(close)
            .remove::<bevy::ui::Pressed>();
        app.world_mut()
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities
            .push(close);
        step(&mut app);
        assert!(
            !app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .quest_log_open
        );

        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects();
            assert_eq!(
                objects.current_unit_order(selected).unwrap(),
                0,
                "HUD diagnostic requires an idle selected hero"
            );
            assert!(
                !objects
                    .executed_unit(selected)
                    .unwrap()
                    .activity
                    .as_ref()
                    .unwrap()
                    .held()
            );
        }
        let highlighted = app
            .world_mut()
            .query::<(&crate::campaign_command_card::ButtonOrder, &Children)>()
            .iter(app.world())
            .filter(|(_, children)| {
                children.iter().any(|child| {
                    app.world()
                        .get::<MaterialNode<crate::glue_menu::AdditiveImage>>(child)
                        .is_some()
                })
            })
            .map(|(button, _)| button.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            highlighted,
            vec![crate::campaign_command_card::Kind::Standard(
                pcx_play::standard_command::Kind::Stop
            )]
        );
    }
    assert!(
        app.world()
            .resource::<crate::campaign_ground_lighting::TerrainLighting>()
            .last_error
            .is_none(),
        "terrain source lighting must be bound at the mounted capture"
    );
    assert!(
        app.world()
            .resource::<crate::campaign_ground_lighting::TerrainLighting>()
            .last
            .is_some(),
        "terrain source light submission must occur"
    );
    let skill_menu = app
        .world_mut()
        .query::<(Entity, &crate::campaign_command_card::ButtonOrder)>()
        .iter(app.world())
        .find(|(_, button)| button.kind == crate::campaign_command_card::Kind::SkillMenu)
        .map(|(entity, _)| entity);
    if let Some(skill_menu) = skill_menu {
        activate(&mut app, skill_menu);
        capture(&mut app, &output.join("skill-menu.png"));
        assert!(
            app.world()
                .resource::<crate::campaign_command_card::CommandCard>()
                .last_error
                .is_none()
        );
        let rendered = app
            .world_mut()
            .query::<(
                &crate::campaign_command_card::ButtonOrder,
                Has<InteractionDisabled>,
            )>()
            .iter(app.world())
            .filter_map(|(button, disabled)| {
                if let crate::campaign_command_card::Kind::Learn(alias) = button.kind {
                    Some((alias.to_be_bytes(), disabled))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let image_modes = app
            .world_mut()
            .query::<(Entity, &ImageNode)>()
            .iter(app.world())
            .filter_map(|(entity, image)| {
                let mut at = entity;
                loop {
                    if let Some(button) = app
                        .world()
                        .get::<crate::campaign_command_card::ButtonOrder>(at)
                    {
                        return matches!(button.kind, crate::campaign_command_card::Kind::Learn(_))
                            .then(|| image.image_mode.clone());
                    }
                    at = app.world().get::<ChildOf>(at)?.parent();
                }
            })
            .collect::<Vec<_>>();
        assert!(
            !image_modes.is_empty(),
            "research image tree must be mounted"
        );
        assert!(
            image_modes
                .iter()
                .all(|mode| *mode == bevy::ui::widget::NodeImageMode::Stretch),
            "research icons and badge backgrounds must fill their authored frames"
        );
        let ranks = app
            .world_mut()
            .query::<(Entity, &Text)>()
            .iter(app.world())
            .filter_map(|(entity, text)| {
                let mut at = entity;
                loop {
                    if let Some(button) = app
                        .world()
                        .get::<crate::campaign_command_card::ButtonOrder>(at)
                    {
                        return if let crate::campaign_command_card::Kind::Learn(alias) = button.kind
                        {
                            Some((alias.to_be_bytes(), text.0.clone()))
                        } else {
                            None
                        };
                    }
                    at = app.world().get::<ChildOf>(at)?.parent();
                }
            })
            .collect::<Vec<_>>();
        let skills = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .learnable_skills(selected)
            .unwrap();
        for skill in skills {
            let shown = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>()
                .host()
                .unwrap()
                .objects()
                .research_icon(selected, skill.alias)
                .unwrap()
                .unwrap()
                .rank;
            assert_eq!(
                ranks
                    .iter()
                    .filter(|(alias, _)| *alias == skill.alias)
                    .map(|(_, text)| text.as_str())
                    .collect::<Vec<_>>(),
                vec![shown.to_string().as_str()],
                "research number {:?}",
                skill.alias
            );
            assert_eq!(
                rendered
                    .iter()
                    .find(|(alias, _)| *alias == skill.alias)
                    .map(|(_, disabled)| *disabled),
                Some(skill.block.is_some()),
                "research availability {:?}",
                skill.alias
            );
        }
        let cancel = app
            .world_mut()
            .query::<(Entity, &crate::campaign_command_card::ButtonOrder)>()
            .iter(app.world())
            .find(|(_, button)| button.kind == crate::campaign_command_card::Kind::SkillCancel)
            .map(|(entity, _)| entity)
            .expect("source research cancel");
        activate(&mut app, cancel);
        step(&mut app);
    }
    let attachments = app
        .world_mut()
        .query::<&crate::hud_render::HudSourceAttachment>()
        .iter(app.world())
        .map(|a| {
            serde_json::json!({
                "frame": a.frame, "member": a.source_member,
                "map": a.receipt_path, "revision": a.binding_revision
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(
        output.join("selected-unit.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "unit": format!("{selected:?}"), "attachments": attachments,
            "scope": "Canonical local selection after the unchanged campaign-control capture"
        }))
        .unwrap(),
    )
    .unwrap();
    if hud_only {
        assert!(
            app.world()
                .resource::<peacecraft3::map_ground::GroundPublication>()
                .last_error
                .is_none(),
            "ground projection failed"
        );
        assert!(
            app.world()
                .resource::<peacecraft3::map_ground::GroundPublication>()
                .shadow_error
                .is_none(),
            "source shadow projection failed"
        );
        for error in [
            &app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .last_error,
            &app.world()
                .resource::<crate::campaign_input::CampaignInput>()
                .last_error,
            &app.world()
                .resource::<crate::campaign_unit_panel::UnitPanel>()
                .last_error,
            &app.world()
                .resource::<crate::campaign_command_card::CommandCard>()
                .last_error,
        ] {
            assert!(error.is_none(), "HUD projection: {error:?}");
        }
        assert!(
            skill_menu.is_some(),
            "diagnostic chapter needs a research menu"
        );
        return;
    }
    item_visibility_roundtrip(&mut app, &output);
    // Retain every stage before refusing a known renderer gap.
    let errors = [
        (
            "actors",
            app.world()
                .resource::<crate::campaign_actors::UnitActors>()
                .last_error
                .clone(),
        ),
        (
            "ground",
            app.world()
                .resource::<peacecraft3::map_ground::GroundPublication>()
                .last_error
                .as_ref()
                .map(ToString::to_string),
        ),
        (
            "camera",
            app.world().resource::<CampaignCamera>().last_error.clone(),
        ),
        (
            "HUD",
            app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .last_error
                .clone(),
        ),
        (
            "input",
            app.world()
                .resource::<crate::campaign_input::CampaignInput>()
                .last_error
                .clone(),
        ),
        (
            "unit panel",
            app.world()
                .resource::<crate::campaign_unit_panel::UnitPanel>()
                .last_error
                .clone(),
        ),
        (
            "inventory",
            app.world()
                .resource::<crate::campaign_inventory::CampaignInventory>()
                .last_error
                .clone(),
        ),
        (
            "command card",
            app.world()
                .resource::<crate::campaign_command_card::CommandCard>()
                .last_error
                .clone(),
        ),
        (
            "portrait",
            app.world()
                .resource::<crate::campaign_portrait::CampaignPortrait>()
                .last_error
                .clone(),
        ),
        (
            "minimap",
            app.world()
                .resource::<crate::campaign_minimap::CampaignMinimap>()
                .last_error
                .clone(),
        ),
    ];
    let requests = app
        .world()
        .resource::<peacecraft3::map_requests::MapRequests>();
    let context = requests.admission.shared_context().expect("admitted map");
    assert!(std::sync::Arc::ptr_eq(
        &context,
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .unwrap()
    ));
    let recipe = app
        .world()
        .resource::<peacecraft3::browser_configuration::SelectedMapSources>()
        .campaign_for_receipts(&requests.bindings.as_ref().unwrap().receipts)
        .unwrap();
    let image_sha256 = recipe.image_sha256();
    let color_rules = pcx_play::ally_color::Rules::read(recipe.image()).unwrap();
    let units = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .executed_units()
        .count();
    use sha2::{Digest, Sha256};
    let mut frames = Vec::new();
    let minimap = app
        .world()
        .resource::<crate::campaign_minimap::CampaignMinimap>();
    for name in [
        "main-menu.png",
        "chapter-selection.png",
        "campaign-entry.png",
        "campaign-control.png",
    ] {
        let bytes = std::fs::read(output.join(name)).unwrap();
        frames
            .push(serde_json::json!({"path":name, "sha256_bytes":Sha256::digest(bytes).to_vec()}));
    }
    std::fs::write(output.join("scope.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "install": install, "map": map, "expansion": expansion,
        "image_sha256": image_sha256, "executed_units": units, "frames": frames,
        "minimap": {"member": minimap.member, "shown": minimap.shown},
        "subsystem_errors": errors,
        "scope": "Production app offscreen Vulkan; mounted widget activation and synthetic keyboard messages; no physical input, audio playback or original paired-frame parity claim"
    })).unwrap()).unwrap();
    // Controlled sky/sight parity harness at the unchanged checkpoint.
    sky_sight_parity::retain_control(&mut app, &output, &install, &map, expansion);
    // Retain the unchanged campaign above; the following mutation is a separate rendering diagnostic.
    let before = {
        use pcx_script::{Host, Value};
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let host = session.host().unwrap();
        let settings = host.objects().startup_settings().unwrap();
        host.native("FogEnable", &[Value::Bool(false)]).unwrap();
        host.native("FogMaskEnable", &[Value::Bool(false)]).unwrap();
        [settings.fog_enabled, settings.fog_mask_enabled]
    };
    until(&mut app, "explicit no-fog minimap diagnostic", |world| {
        let minimap = world.resource::<crate::campaign_minimap::CampaignMinimap>();
        minimap.shown && minimap.member.is_some()
    });
    capture(&mut app, &output.join("minimap-no-fog-diagnostic.png"));
    let minimap = app
        .world()
        .resource::<crate::campaign_minimap::CampaignMinimap>();
    std::fs::write(output.join("minimap-no-fog-diagnostic.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "natural_fog_and_mask": before, "diagnostic_fog_and_mask": [false, false],
        "member": minimap.member, "shown": minimap.shown, "error": minimap.last_error,
        "scope": "Explicit post-checkpoint host-native mutation; image rendering only, no unchanged campaign visibility or native pixel parity claim"
    })).unwrap()).unwrap();
    assert!(minimap.last_error.is_none());
    // Freeze the clock: toggling A -> B -> A must recolor live models and restore exact minimap pixels.
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::ZERO,
    ));
    let original_filter = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .ally_color_filter_state()
        .unwrap();
    let individual = (-256..=256)
        .find(|&n| !color_rules.filters_minimap(n) && !color_rules.filters_units(n))
        .unwrap();
    let filtered = (-256..=256)
        .find(|&n| color_rules.filters_units(n))
        .unwrap();
    let mut snapshots = Vec::new();
    for (state, name) in [
        (individual, "individual"),
        (filtered, "filtered"),
        (individual, "restored"),
    ] {
        use pcx_script::{Host, Value};
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .native("SetAllyColorFilterState", &[Value::Int(state)])
            .unwrap();
        capture(&mut app, &output.join(format!("ally-colors-{name}.png")));
        snapshots.push(color_snapshot(&mut app));
    }
    assert_ne!(
        snapshots[0].0, snapshots[1].0,
        "live model palette must change"
    );
    assert_ne!(
        snapshots[0].1, snapshots[1].1,
        "live minimap markers must change"
    );
    assert_eq!(
        snapshots[0], snapshots[2],
        "restoring the filter must leave no stale models or marker trails"
    );
    {
        use pcx_script::{Host, Value};
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let host = session.host().unwrap();
        host.native("SetAllyColorFilterState", &[Value::Int(original_filter)])
            .unwrap();
        host.native("FogEnable", &[Value::Bool(before[0])]).unwrap();
        host.native("FogMaskEnable", &[Value::Bool(before[1])])
            .unwrap();
    }
    std::fs::write(output.join("ally-colors.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "individual_state": individual, "filtered_state": filtered, "restored_original_state": original_filter,
        "model_palette_changed": true, "minimap_pixels_changed": true, "roundtrip_exact": true,
        "scope": "Explicit post-checkpoint native mutations at a frozen clock, ordinary unit markers; no original pixel parity claim"
    })).unwrap()).unwrap();
    // Actual menu widgets pause/return/quit before a second chapter-button activation.
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_secs_f64(1. / 64.),
    ));
    menu_key(&mut app, ButtonState::Pressed);
    step(&mut app);
    menu_key(&mut app, ButtonState::Released);
    step(&mut app);
    assert!(app.world().resource::<CampaignCamera>().active());
    assert!(app.world().resource::<CampaignCamera>().menu_open());
    let paused_clock = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects()
        .timer_game_time()
        .unwrap();
    capture(&mut app, &output.join("escape-main.png"));
    assert!(
        app.world()
            .resource::<crate::campaign_escape_menu::EscapeMenu>()
            .last_error
            .is_none(),
        "{:?}",
        app.world()
            .resource::<crate::campaign_escape_menu::EscapeMenu>()
            .last_error
    );
    assert_eq!(
        paused_clock,
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .timer_game_time()
            .unwrap()
    );
    let button = frame_button(&mut app, "ReturnButton");
    activate(&mut app, button);
    until(&mut app, "return to game", |w| {
        !w.resource::<CampaignCamera>().menu_open()
    });
    for _ in 0..4 {
        step(&mut app);
    }
    assert!(
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .timer_game_time()
            .unwrap()
            > paused_clock
    );
    let button = app
        .world_mut()
        .query_filtered::<(Entity, &Name), With<Button>>()
        .iter(app.world())
        .find(|(_, name)| name.as_str() == "UpperButtonBarMenuButton")
        .unwrap()
        .0;
    activate(&mut app, button);
    until(&mut app, "HUD menu button", |w| {
        w.resource::<CampaignCamera>().menu_open()
    });
    let button = frame_button(&mut app, "EndGameButton");
    activate(&mut app, button);
    capture(&mut app, &output.join("escape-end.png"));
    let button = frame_button(&mut app, "ExitButton");
    activate(&mut app, button);
    capture(&mut app, &output.join("escape-confirm-exit.png"));
    let button = frame_button(&mut app, "ConfirmQuitCancelButton");
    activate(&mut app, button);
    step(&mut app);
    assert!(app.world().resource::<Messages<AppExit>>().is_empty());
    let restart_from = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    let button = frame_button(&mut app, "RestartButton");
    activate(&mut app, button);
    until(&mut app, "restart mission widget", |world| {
        world.resource::<CampaignCamera>().active()
            && world
                .resource::<MapScriptSession<MapStartupHost>>()
                .session_witness()
                .is_some_and(|next| !std::sync::Weak::ptr_eq(&next, &restart_from))
    });
    assert!(restart_from.upgrade().is_none());
    assert!(!app.world().resource::<CampaignCamera>().menu_open());
    menu_key(&mut app, ButtonState::Pressed);
    step(&mut app);
    menu_key(&mut app, ButtonState::Released);
    step(&mut app);
    assert!(app.world().resource::<CampaignCamera>().menu_open());
    let button = frame_button(&mut app, "EndGameButton");
    activate(&mut app, button);
    step(&mut app);
    let first_session = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    let first_camera = app.world().resource::<CampaignCamera>().entity.unwrap();
    let first_console = app
        .world()
        .resource::<crate::hud_render::HudMount>()
        .root
        .unwrap();
    let button = frame_button(&mut app, "QuitButton");
    activate(&mut app, button);
    until(&mut app, "menu mission exit", |world| {
        world
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .is_none()
            && !world.resource::<CampaignCamera>().active()
    });
    capture(&mut app, &output.join("campaign-exit.png"));
    assert!(app.world().get_entity(first_camera).is_err());
    assert!(app.world().get_entity(first_console).is_err());
    assert!(first_session.upgrade().is_none());
    let button = app
        .world_mut()
        .query::<(Entity, &crate::native_campaign::MissionButton)>()
        .iter(app.world())
        .find(|(_, b)| b.campaign == campaign && b.mission == mission)
        .expect("chapter button after exit")
        .0;
    activate(&mut app, button);
    until(&mut app, "replacement campaign", |world| {
        world.resource::<CampaignCamera>().active()
            && world
                .resource::<MapScriptSession<MapStartupHost>>()
                .session_witness()
                .is_some_and(|next| !std::sync::Weak::ptr_eq(&next, &first_session))
    });
    capture(&mut app, &output.join("campaign-restarted.png"));
    assert!(app.world().get_entity(first_console).is_err());
    std::fs::write(output.join("restart.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "old_session_retired": true, "old_camera_retired": true,
        "old_console_retired": true, "new_session_published": true,
        "menu_clock_paused_and_resumed": true,
        "restart_widget_replaced_session": true,
        "scope": "Source menu key and HUD Menu, Return, End Game, exit cancellation, Restart Mission and Quit Mission; subsequent chapter launch. Native quit/score timing parity remains unqualified."
    })).unwrap()).unwrap();
    // Ally/water/menu original-client comparison: recreation captures above
    // against caller-supplied originals; records parity without asserting it.
    ally_water_menu_parity::retain(&mut app, &output, &install, &map, expansion);
    assert!(
        errors.iter().all(|(_, error)| error.is_none()),
        "campaign rendering: {errors:?}"
    );
}

#[test]
#[ignore = "requires explicit original install/edition, Vulkan, a new capture directory and XDG_CONFIG_HOME within its config subdirectory"]
fn gameplay_camera_options_render_cancel_apply_and_reload() {
    use crate::menu_screens::{Navigation, Page, Screen};
    use bevy::ui::{Checked, InteractionDisabled};
    use bevy::ui_widgets::{SliderValue, ValueChange};
    let install = std::env::var("PCX_NATIVE_CHAPTER_INSTALL").unwrap();
    let expansion = match std::env::var("PCX_NATIVE_CHAPTER_ADDON").as_deref() {
        Ok("roc") => false,
        Ok("tft") => true,
        _ => panic!("explicit edition required"),
    };
    let output = std::path::PathBuf::from(std::env::var_os("PCX_CAMPAIGN_CAPTURE_DIR").unwrap());
    assert!(!output.exists(), "new isolated capture directory required");
    assert_eq!(
        std::path::PathBuf::from(
            std::env::var_os("XDG_CONFIG_HOME").expect("isolated preferences required")
        ),
        output.join("config")
    );
    std::fs::create_dir_all(output.join("config")).unwrap();
    let mut app = mounted_main_menu(&install, expansion, &output, false);
    // The offscreen fixture has no native display-mode witness and requests no resize.
    {
        let mut nav = app.world_mut().resource_mut::<Navigation>();
        nav.applied.resolution = None;
        nav.draft.resolution = None;
    }

    let open = |app: &mut App| {
        let button = frame_button(app, "OptionsButton");
        activate(app, button);
        screen(app, Screen::Options(Page::Unselected));
        let button = frame_button(app, "GameplayButton");
        activate(app, button);
        screen(app, Screen::Options(Page::Gameplay));
    };
    open(&mut app);
    let key = frame_button(&mut app, "KeyScrollSlider");
    let mouse = frame_button(&mut app, "MouseScrollSlider");
    let disable = frame_button(&mut app, "MouseScrollDisableCheckBox");
    let tooltip = frame_button(&mut app, "TooltipsCheckBox");
    for entity in [key, mouse, disable, tooltip] {
        assert!(!app.world().entity(entity).contains::<InteractionDisabled>());
    }
    let before = app.world().resource::<Navigation>().applied.clone();
    let changed = |app: &mut App, entity: Entity| {
        let control = app
            .world()
            .get::<crate::menu_screens::SettingControl>(entity)
            .unwrap();
        let crate::menu_screens::Widget::Slider { min, max, .. } = control.widget else {
            panic!("slider required")
        };
        let value = app.world().get::<SliderValue>(entity).unwrap().0;
        if value == max { min } else { max }
    };
    let key_value = changed(&mut app, key);
    let mouse_value = changed(&mut app, mouse);
    let disabled = !before.toggles["MouseScrollDisableCheckBox"];
    let verbose = !before.toggles["TooltipsCheckBox"];
    app.world_mut().trigger(ValueChange {
        source: key,
        value: key_value,
        is_final: true,
    });
    app.world_mut().trigger(ValueChange {
        source: mouse,
        value: mouse_value,
        is_final: true,
    });
    app.world_mut()
        .trigger(bevy::ui_widgets::ToggleChecked { entity: disable });
    app.world_mut()
        .trigger(bevy::ui_widgets::ToggleChecked { entity: tooltip });
    step(&mut app);
    assert_eq!(app.world().resource::<Navigation>().applied, before);
    capture(&mut app, &output.join("gameplay-draft.png"));
    let button = frame_button(&mut app, "CancelButton");
    activate(&mut app, button);
    screen(&mut app, Screen::Main);
    assert_eq!(app.world().resource::<Navigation>().applied, before);
    open(&mut app);
    for (name, value) in [
        ("KeyScrollSlider", key_value),
        ("MouseScrollSlider", mouse_value),
    ] {
        let entity = frame_button(&mut app, name);
        assert_eq!(
            app.world().get::<SliderValue>(entity).unwrap().0,
            before.values[name]
        );
        app.world_mut().trigger(ValueChange {
            source: entity,
            value,
            is_final: true,
        });
    }
    let disable = frame_button(&mut app, "MouseScrollDisableCheckBox");
    app.world_mut()
        .trigger(bevy::ui_widgets::ToggleChecked { entity: disable });
    step(&mut app);
    let tooltip = frame_button(&mut app, "TooltipsCheckBox");
    assert_eq!(app.world().get::<Checked>(tooltip).is_some(), !verbose);
    app.world_mut()
        .trigger(bevy::ui_widgets::ToggleChecked { entity: tooltip });
    step(&mut app);
    let button = frame_button(&mut app, "OKButton");
    activate(&mut app, button);
    screen(&mut app, Screen::Main);
    let applied = app.world().resource::<Navigation>().applied.clone();
    assert_eq!(applied.toggles["TooltipsCheckBox"], verbose);
    assert_eq!(applied.values["KeyScrollSlider"], key_value);
    assert_eq!(applied.values["MouseScrollSlider"], mouse_value);
    assert_eq!(applied.toggles["MouseScrollDisableCheckBox"], disabled);
    assert!(output.join("config/peacecraft3/settings.json").exists());
    drop(app);
    std::fs::create_dir_all(output.join("reload")).unwrap();
    let mut app = mounted_main_menu(&install, expansion, &output.join("reload"), false);
    assert_eq!(
        app.world().resource::<Navigation>().applied.values["KeyScrollSlider"],
        key_value
    );
    assert_eq!(
        app.world().resource::<Navigation>().applied.values["MouseScrollSlider"],
        mouse_value
    );
    assert_eq!(
        app.world().resource::<Navigation>().applied.toggles["MouseScrollDisableCheckBox"],
        disabled
    );
    assert_eq!(
        app.world().resource::<Navigation>().applied.toggles["TooltipsCheckBox"],
        verbose
    );
    open(&mut app);
    let tooltip = frame_button(&mut app, "TooltipsCheckBox");
    assert_eq!(app.world().get::<Checked>(tooltip).is_some(), verbose);
    let disable = frame_button(&mut app, "MouseScrollDisableCheckBox");
    assert_eq!(app.world().get::<Checked>(disable).is_some(), disabled);
    capture(&mut app, &output.join("gameplay-reloaded.png"));
    std::fs::write(output.join("options-checks.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "draft_did_not_apply": true, "cancel_restored": true, "apply_saved": true, "new_app_reloaded": true,
        "controls": ["KeyScrollSlider", "MouseScrollSlider", "MouseScrollDisableCheckBox", "TooltipsCheckBox"],
        "scope": "Production widgets and isolated preferences, headless/muted; no native timing or pixel-parity claim"
    })).unwrap()).unwrap();
}
