//! Source `SlidingDoors` sprite over the complete menu, independent of scene ownership.
use std::{path::Path, time::Duration};

use bevy::{
    camera::{
        ScalingMode,
        visibility::{NoFrustumCulling, RenderLayers},
    },
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
};

use crate::{mainmenu_scene as scene, menu_material::MenuMaterial, menu_screens::Screen};

// Render allocation and ordering, not authored content.
const LAYER: usize = 8;

#[derive(Component, Clone)]
pub(crate) struct FadeEntity;

struct Clip {
    interval: [u32; 2],
    opacity: pcx_gfx::mdx_opacity::OpacityPlan,
}

impl Clip {
    fn checked(
        runtime: &pcx_gfx::mdx_runtime::RuntimePlan,
        sequences: &[pcx_mdl::Sequence],
        name: &str,
        ends: [f32; 2],
    ) -> Result<Self, String> {
        let mut matches = sequences.iter().filter(|sequence| sequence.name == name);
        let sequence = matches.next().ok_or_else(|| format!("missing {name}"))?;
        if matches.next().is_some()
            || sequence.interval_start >= sequence.interval_end
            || runtime.geosets.is_empty()
        {
            return Err(format!("duplicate or empty {name}/geometry"));
        }
        let interval = [sequence.interval_start, sequence.interval_end];
        let opacity = pcx_gfx::mdx_opacity::OpacityPlan::checked(runtime, interval)
            .map_err(|e| e.to_string())?;
        for geoset in &runtime.geosets {
            for (frame, expected) in interval.into_iter().zip(ends) {
                if opacity.sample(geoset.geoset_id, frame, 0) != Some(expected) {
                    return Err(format!("{name}: unsupported fade endpoints"));
                }
            }
        }
        Ok(Self { interval, opacity })
    }

    fn duration(&self) -> Duration {
        Duration::from_millis(u64::from(self.interval[1] - self.interval[0]))
    }

    fn frame(&self, elapsed: Duration) -> u32 {
        self.interval[0]
            + elapsed
                .as_millis()
                .min(u128::from(self.interval[1] - self.interval[0])) as u32
    }
}

pub(crate) struct Prepared {
    path: String,
    bytes: Vec<u8>,
    model: pcx_gfx::LoadedModel,
    runtime: pcx_gfx::mdx_runtime::RuntimePlan,
    textures: Vec<pcx_gfx::LoadedTexture>,
    selection: pcx_gfx::mdx_texture_selection::TextureSelectionPlan,
    uv: pcx_gfx::mdx_uv_translation::UvTranslationPlan,
    cover: Clip,
    reveal: Clip,
}

impl Prepared {
    pub(crate) fn read(install: Option<&Path>, fdf: &crate::fdf::Fdf) -> Result<Self, String> {
        let frame = fdf
            .frame("SlidingDoors")
            .ok_or("missing SlidingDoors sprite")?;
        if frame.kind != "SPRITE" || !fdf.flag("SlidingDoors", "SetAllPoints") {
            return Err("SlidingDoors requires a full-frame sprite".into());
        }
        let path = pcx_mdl::binary_model_path(
            &fdf.string("SlidingDoors", "BackgroundArt")
                .ok_or("SlidingDoors has no BackgroundArt")?,
        );
        let bytes = crate::mainmenu::art_bytes_named(install, &path, "campaign-fade/model.mdx")
            .ok_or_else(|| format!("{path}: unavailable"))?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(format!("{path}: menu model memory budget exceeded"));
        }
        let result = Self::decode(path.clone(), bytes, install);
        result.map_err(|error| format!("{path}: {error}"))
    }

    fn decode(path: String, bytes: Vec<u8>, install: Option<&Path>) -> Result<Self, String> {
        let m = pcx_mdl::Mdlx::parse(&bytes).map_err(|e| e.to_string())?;
        let model = pcx_gfx::load_model(&bytes, &path).map_err(|e| e.to_string())?;
        let runtime =
            pcx_gfx::mdx_runtime::RuntimePlan::parse(&bytes).map_err(|e| e.to_string())?;
        if model.meshes.is_empty()
            || model
                .minimum_extent
                .iter()
                .chain(&model.maximum_extent)
                .any(|x| !x.is_finite())
            || model.minimum_extent[2] >= model.maximum_extent[2]
            || runtime
                .joints
                .joints()
                .iter()
                .any(|joint| !joint.tracks.is_empty())
            || !runtime.texture_animations.is_empty()
            || runtime
                .geoset_animations
                .iter()
                .any(|animation| animation.tracks.iter().any(|track| track.magic == *b"KGAC"))
            || runtime.materials.iter().any(|material| {
                material.layers.len() != 1 || !material.layers[0].texture_tracks.is_empty()
            })
        {
            return Err("unsupported fade geometry, transform or texture animation".into());
        }
        let sequences = m.sequences(&bytes).map_err(|e| e.to_string())?;
        let cover = Clip::checked(&runtime, &sequences, "Birth", [0.0, 1.0])?;
        let reveal = Clip::checked(&runtime, &sequences, "Death", [1.0, 0.0])?;
        let textures = scene::load_textures_in(install, &bytes, "campaign-fade");
        let selection =
            scene::texture_selection(&runtime, cover.interval, &path).map_err(|e| e.to_string())?;
        let uv = pcx_gfx::mdx_uv_translation::UvTranslationPlan::from_runtime(
            &runtime,
            cover.interval,
            &path,
        )
        .map_err(|e| e.to_string())?;
        let source = scene::GeosetSource {
            m: &m,
            bytes: &bytes,
            source: &path,
            model: &model,
            textures: &textures,
            runtime: &runtime,
            selection: &selection,
            uv_translation: &uv,
            native_cutout_reference: None,
            material_layers: None,
            culls_back_faces: false,
        };
        for clip in [&cover, &reveal] {
            scene::preflight_geosets(&source, &clip.opacity, clip.interval[0])
                .map_err(|e| e.to_string())?;
        }
        Ok(Self {
            path,
            bytes,
            model,
            runtime,
            textures,
            selection,
            uv,
            cover,
            reveal,
        })
    }

    pub(crate) fn spawn(
        self,
        commands: &mut Commands,
        assets: scene::GeosetAssets,
        destination: Screen,
        source_key: String,
    ) -> Result<Fade, String> {
        let m = pcx_mdl::Mdlx::parse(&self.bytes).map_err(|e| e.to_string())?;
        let geosets = scene::spawn_geosets(
            commands,
            assets,
            scene::GeosetSource {
                m: &m,
                bytes: &self.bytes,
                source: &self.path,
                model: &self.model,
                textures: &self.textures,
                runtime: &self.runtime,
                selection: &self.selection,
                uv_translation: &self.uv,
                native_cutout_reference: None,
                material_layers: None,
                culls_back_faces: false,
            },
            &self.cover.opacity,
            self.cover.interval[0],
            &mut scene::SceneInfo::default(),
            (FadeEntity, RenderLayers::layer(LAYER), NoFrustumCulling),
        )
        .map_err(|e| e.to_string())?;
        commands.spawn((
            Camera3d::default(),
            crate::menu_composition::MenuColourCamera,
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: crate::mainmenu::FRAME_W,
                    height: crate::mainmenu::FRAME_H,
                },
                near: -self.model.maximum_extent[2],
                far: -self.model.minimum_extent[2],
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_xyz(
                crate::mainmenu::FRAME_W / 2.0,
                crate::mainmenu::FRAME_H / 2.0,
                0.0,
            ),
            Camera {
                order: 2,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            Tonemapping::None,
            DebandDither::Disabled,
            RenderLayers::layer(LAYER),
            FadeEntity,
        ));
        Ok(Fade {
            cover: self.cover,
            reveal: self.reveal,
            geosets,
            destination,
            source_key,
            stage: Stage::Covering,
            elapsed: Duration::ZERO,
            wait_for_panels: false,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Covering,
    Loading,
    Revealing,
}

#[derive(Resource)]
pub(crate) struct Fade {
    cover: Clip,
    reveal: Clip,
    geosets: scene::Geosets,
    pub(crate) destination: Screen,
    pub(crate) source_key: String,
    pub(crate) stage: Stage,
    pub(crate) wait_for_panels: bool,
    elapsed: Duration,
}

impl Fade {
    /// The black endpoint is held until both the outgoing owner and incoming scene are ready.
    pub(crate) fn tick(
        &mut self,
        delta: Duration,
        outgoing_done: bool,
        incoming_ready: bool,
    ) -> (bool, bool) {
        match self.stage {
            Stage::Covering => {
                if self.wait_for_panels {
                    if outgoing_done {
                        self.wait_for_panels = false;
                    }
                    // The panel controller already consumed this presentation delta.
                    return (false, false);
                }
                self.elapsed = self
                    .elapsed
                    .saturating_add(delta)
                    .min(self.cover.duration());
                if self.elapsed == self.cover.duration() && outgoing_done {
                    self.stage = Stage::Loading;
                    return (true, false);
                }
            }
            Stage::Loading if incoming_ready => {
                self.stage = Stage::Revealing;
                self.elapsed = Duration::ZERO;
            }
            Stage::Revealing => {
                self.elapsed = self
                    .elapsed
                    .saturating_add(delta)
                    .min(self.reveal.duration());
                if self.elapsed == self.reveal.duration() {
                    return (false, true);
                }
            }
            Stage::Loading => {}
        }
        (false, false)
    }

    pub(crate) fn draw(
        &self,
        materials: &mut Assets<MenuMaterial>,
        visibility: &mut Query<&mut Visibility>,
    ) {
        let (clip, elapsed) = if self.stage == Stage::Revealing {
            (&self.reveal, self.elapsed)
        } else {
            (&self.cover, self.elapsed)
        };
        scene::apply_opacity(
            &clip.opacity,
            &self.geosets.bound,
            clip.frame(elapsed),
            0,
            materials,
            visibility,
        );
    }
}

pub(crate) fn follow_ui_viewport(
    ui: Query<&Camera, (With<bevy::ui::IsDefaultUiCamera>, Without<FadeEntity>)>,
    mut fade: Query<&mut Camera, With<FadeEntity>>,
) {
    if let Ok(ui) = ui.single() {
        for mut camera in &mut fade {
            camera.viewport = ui.viewport.clone();
            camera.is_active = ui.is_active;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use pcx_gfx::mdx_runtime::{GeosetInput, RuntimeInput, RuntimePlan};

    fn fixture() -> (RuntimePlan, Vec<pcx_mdl::Sequence>) {
        let mut track = b"KMTA".to_vec();
        for word in [6u32, 1, pcx_mdl::NO_ID] {
            track.extend(word.to_le_bytes());
        }
        for (frame, alpha) in [
            (11u32, 0f32),
            (41, 0.2),
            (131, 1.),
            (211, 1.),
            (241, 0.7),
            (251, 0.),
        ] {
            track.extend(frame.to_le_bytes());
            track.extend(alpha.to_le_bytes());
        }
        let mut data = vec![0u8; 8];
        data.extend(b"LAYS");
        data.extend(1u32.to_le_bytes());
        for word in [28 + track.len() as u32, 2, 1, 0, pcx_mdl::NO_ID, 0] {
            data.extend(word.to_le_bytes());
        }
        data.extend(1f32.to_le_bytes());
        data.extend(track);
        let runtime = RuntimePlan::checked(RuntimeInput {
            bones: vec![],
            helpers: vec![],
            pivots: pcx_mdl::Pivots(vec![]),
            geosets: vec![GeosetInput {
                vertex_count: 4,
                material_id: 0,
                matrix_joint_ids: vec![],
            }],
            geoset_animations: vec![],
            materials: vec![pcx_mdl::Material {
                offset: 0,
                len: data.len() as u32 + 4,
                data,
            }],
            textures: vec![pcx_mdl::Texture {
                slot: 0,
                path: "nonstock-fade.blp".into(),
                flags: 0,
            }],
            texture_animations: vec![],
            global_sequences: vec![],
        })
        .unwrap();
        let sequences = [("Birth", 11, 131), ("Death", 211, 251)]
            .into_iter()
            .map(|(name, start, end)| pcx_mdl::Sequence {
                name: name.into(),
                interval_start: start,
                interval_end: end,
                move_speed: 0.,
                non_looping: true,
                trailing: [0; 7],
            })
            .collect();
        (runtime, sequences)
    }

    pub(crate) fn fade() -> Fade {
        let (runtime, sequences) = fixture();
        Fade {
            cover: Clip::checked(&runtime, &sequences, "Birth", [0., 1.]).unwrap(),
            reveal: Clip::checked(&runtime, &sequences, "Death", [1., 0.]).unwrap(),
            geosets: scene::Geosets {
                handles: vec![],
                bound: vec![],
                textures: default(),
            },
            destination: Screen::Campaign,
            source_key: "synthetic".into(),
            stage: Stage::Covering,
            elapsed: Duration::ZERO,
            wait_for_panels: true,
        }
    }

    #[test]
    fn source_curve_and_durations_survive_a_slow_owner_and_load() {
        let mut fade = fade();
        let ms = Duration::from_millis;
        assert_eq!(fade.cover.duration(), ms(120));
        assert_eq!(fade.reveal.duration(), ms(40));
        assert_eq!(fade.tick(Duration::MAX, false, false), (false, false));
        assert_eq!(
            fade.elapsed,
            Duration::ZERO,
            "the panels leave before the screen darkens"
        );
        assert_eq!(fade.tick(Duration::MAX, true, false), (false, false));
        assert_eq!(
            fade.elapsed,
            Duration::ZERO,
            "do not consume the panels' last delta twice"
        );
        assert_eq!(fade.tick(ms(30), true, false), (false, false));
        assert_eq!(
            fade.cover
                .opacity
                .sample(0, fade.cover.frame(fade.elapsed), 0),
            Some(0.2)
        );
        assert_eq!(fade.tick(Duration::MAX, true, false), (true, false));
        assert_eq!(fade.elapsed, ms(120));
        assert_eq!(fade.stage, Stage::Loading);
        assert_eq!(fade.tick(Duration::MAX, true, false), (false, false));
        assert_eq!(fade.elapsed, ms(120));
        assert_eq!(fade.tick(Duration::MAX, true, true), (false, false));
        assert_eq!(fade.stage, Stage::Revealing);
        assert_eq!(
            fade.elapsed,
            Duration::ZERO,
            "loading time cannot skip the reveal"
        );
        assert_eq!(fade.tick(ms(30), true, true), (false, false));
        assert_eq!(
            fade.reveal
                .opacity
                .sample(0, fade.reveal.frame(fade.elapsed), 0),
            Some(0.7)
        );
        assert_eq!(fade.tick(ms(10), true, true), (false, true));
        assert_eq!(
            fade.reveal
                .opacity
                .sample(0, fade.reveal.frame(fade.elapsed), 0),
            Some(0.)
        );
    }

    #[test]
    fn incomplete_or_ambiguous_source_clips_refuse() {
        let (runtime, mut sequences) = fixture();
        assert!(Clip::checked(&runtime, &sequences, "Missing", [0., 1.]).is_err());
        sequences[0].interval_end = 41;
        assert!(Clip::checked(&runtime, &sequences, "Birth", [0., 1.]).is_err());
        sequences[0].interval_end = 11;
        assert!(Clip::checked(&runtime, &sequences, "Birth", [0., 1.]).is_err());
        sequences[0].interval_end = 131;
        sequences.push(sequences[0].clone());
        assert!(Clip::checked(&runtime, &sequences, "Birth", [0., 1.]).is_err());
    }

    #[test]
    #[ignore = "requires PEACECRAFT3_INSTALL with original Campaign FDF, model and textures"]
    fn original_campaign_fade_is_admitted_and_spawns_above_the_ui() {
        use bevy::ecs::system::SystemState;
        let install = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_INSTALL").expect("explicit original input"),
        );
        let (fdf, _) =
            crate::launch_gui::read_screen_fdf(Some(&install), Screen::Campaign).unwrap();
        let prepared = Prepared::read(Some(&install), &fdf).unwrap();
        let count = prepared.model.meshes.len();
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<MenuMaterial>>();
        world.init_resource::<Assets<Image>>();
        let mut system = SystemState::<(
            Commands,
            ResMut<Assets<Mesh>>,
            ResMut<Assets<MenuMaterial>>,
            ResMut<Assets<Image>>,
        )>::new(&mut world);
        let (mut commands, mut meshes, mut materials, mut images) =
            system.get_mut(&mut world).unwrap();
        let fade = prepared
            .spawn(
                &mut commands,
                scene::GeosetAssets {
                    meshes: &mut meshes,
                    materials: &mut materials,
                    images: &mut images,
                },
                Screen::Campaign,
                "original".into(),
            )
            .unwrap();
        assert_eq!(fade.geosets.bound.len(), count);
        system.apply(&mut world);
        assert_eq!(
            world
                .query_filtered::<&Camera, With<FadeEntity>>()
                .single(&world)
                .unwrap()
                .order,
            2
        );
        assert_eq!(
            world
                .query_filtered::<&Mesh3d, With<FadeEntity>>()
                .iter(&world)
                .count(),
            count
        );
    }
}
