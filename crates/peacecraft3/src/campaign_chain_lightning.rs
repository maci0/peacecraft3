//! Straight chain-lightning ribbons from `Splats\LightningData.slk` and a drained cast.
//!
//! `AvgSegLen`, `NoiseScale`, and `version` are carried on the row and not applied:
//! this tree has no traced subdivision or noise formula. `Missilespeed` is not travel.
//! Missile and target art are not spawned. Duration is the table cell in seconds.
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::Indices,
    prelude::*,
};
use pcx_assets::Resolver;
use pcx_map::Terrain;
use pcx_play::playercolor_host::ObjectHost;
use pcx_slq::Table;

#[derive(Resource, Default)]
pub(super) struct Bolts {
    rows: Option<Vec<Style>>,
    live: Vec<Live>,
    clock: f64,
    pub(super) gap: Option<String>,
    gone: Vec<Owned>,
}

struct Style {
    name: String,
    path: String,
    width: f32,
    color: [f32; 4],
    duration: f32,
    tex_scale: f32,
    unused: bool,
}

struct Owned {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    image: Handle<Image>,
}

struct Live {
    from: [f32; 3],
    to: [f32; 3],
    width: f32,
    color: [f32; 4],
    tex_scale: f32,
    until: f64,
    image: Vec<u8>,
    size: UVec2,
    owned: Option<Owned>,
}

pub(super) fn queue(
    objects: &mut ObjectHost,
    reader: &Resolver,
    terrain: &Terrain,
    now: f64,
    bolts: &mut Bolts,
) -> Result<(), String> {
    advance(bolts, now);
    if objects.chain_lightning_casts().is_empty() {
        return Ok(());
    }
    if bolts.rows.is_none() {
        let asset = reader
            .resolve_bounded(r"Splats\LightningData.slk", 1 << 20)
            .map_err(|error| error.to_string())?;
        bolts.rows = Some(styles(&asset.bytes)?);
    }
    let (staged, notes_unused) = {
        let rows = bolts.rows.as_deref().expect("loaded");
        let mut staged = Vec::new();
        let mut notes_unused = false;
        for cast in objects.chain_lightning_casts() {
            let mut hops = Vec::new();
            for segment in &cast.segments {
                let Some(token) = segment.effect.as_deref() else {
                    return Err("chain lightning hop has no lightning token".into());
                };
                let style = rows
                    .iter()
                    .find(|row| row.name == token)
                    .ok_or_else(|| format!("lightning row {token} is not in LightningData"))?;
                notes_unused |= style.unused;
                let height = |point: [f32; 2]| {
                    pcx_tera::world::sample_world_z(terrain, point[0], point[1]).ok_or_else(|| {
                        format!("lightning point {point:?} is outside admitted terrain")
                    })
                };
                let texture = reader
                    .resolve_bounded(&style.path, 8 << 20)
                    .map_err(|error| error.to_string())?;
                let (header, image) =
                    pcx_blp::decode(&texture.bytes).map_err(|error| error.to_string())?;
                hops.push(Live {
                    from: [segment.from[0], height(segment.from)?, -segment.from[1]],
                    to: [segment.to[0], height(segment.to)?, -segment.to[1]],
                    width: style.width,
                    color: style.color,
                    tex_scale: style.tex_scale,
                    until: now + f64::from(style.duration),
                    image,
                    size: UVec2::new(header.width, header.height),
                    owned: None,
                });
            }
            staged.extend(hops);
        }
        (staged, notes_unused)
    };
    if notes_unused {
        bolts.gap = Some("LightningData AvgSegLen, NoiseScale, and version are not applied".into());
    }
    objects.take_chain_lightning_casts();
    commit(&mut bolts.live, Ok(staged))
}

fn commit<T>(live: &mut Vec<T>, batch: Result<Vec<T>, String>) -> Result<(), String> {
    live.extend(batch?);
    Ok(())
}

pub(super) fn release(bolts: &mut Bolts) {
    for bolt in bolts.live.drain(..) {
        if let Some(owned) = bolt.owned {
            bolts.gone.push(owned);
        }
    }
    bolts.rows = None;
    bolts.gap = None;
    bolts.clock = 0.;
}

fn advance(bolts: &mut Bolts, now: f64) {
    bolts.clock = now;
    let mut kept = Vec::new();
    for bolt in bolts.live.drain(..) {
        if bolt.until > now {
            kept.push(bolt);
        } else if let Some(owned) = bolt.owned {
            bolts.gone.push(owned);
        }
    }
    bolts.live = kept;
}

fn styles(bytes: &[u8]) -> Result<Vec<Style>, String> {
    let table = Table::parse(bytes).map_err(|error| error.to_string())?;
    let mut rows = Vec::new();
    for row in table.data_rows() {
        let required = |name: &str| {
            row.by_name(name)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| format!("lightning row {} missing {name}", row.index()))
        };
        let number = |name: &str| {
            required(name)?
                .parse::<f32>()
                .map_err(|_| format!("lightning row {} {name} is not a number", row.index()))
        };
        let dir = required("Dir")?;
        let file = required("file")?;
        let path = if dir.ends_with(['\\', '/']) {
            format!("{dir}{file}")
        } else {
            format!("{dir}\\{file}")
        };
        let color = [
            number("R")? / 255.,
            number("G")? / 255.,
            number("B")? / 255.,
            number("A")? / 255.,
        ];
        if !color.iter().all(|channel| channel.is_finite()) {
            return Err(format!("lightning row {} color is not finite", row.index()));
        }
        let width = number("Width")?;
        let duration = number("Duration")?;
        let tex_scale = number("TexCoordScale")?;
        if !(width.is_finite() && width > 0. && duration.is_finite() && duration >= 0.) {
            return Err(format!(
                "lightning row {} width or duration is unusable",
                row.index()
            ));
        }
        if !(tex_scale.is_finite() && tex_scale > 0.) {
            return Err(format!(
                "lightning row {} TexCoordScale is unusable",
                row.index()
            ));
        }
        let unused = ["AvgSegLen", "NoiseScale", "version"]
            .into_iter()
            .any(|name| {
                row.by_name(name)
                    .is_some_and(|value| !value.trim().is_empty())
            });
        rows.push(Style {
            name: required("Name")?,
            path,
            width,
            color,
            duration,
            tex_scale,
            unused,
        });
    }
    Ok(rows)
}

pub(super) fn ribbon(from: [f32; 3], to: [f32; 3], width: f32) -> Result<[[f32; 3]; 4], String> {
    let delta = [to[0] - from[0], to[2] - from[2]];
    let length = (delta[0] * delta[0] + delta[1] * delta[1]).sqrt();
    if !(length.is_finite() && length > 0. && width.is_finite() && width > 0.) {
        return Err("lightning segment has no horizontal length".into());
    }
    let side = [
        -delta[1] / length * width * 0.5,
        delta[0] / length * width * 0.5,
    ];
    let lift = |point: [f32; 3], sign: f32| {
        [
            point[0] + side[0] * sign,
            point[1],
            point[2] + side[1] * sign,
        ]
    };
    Ok([lift(from, -1.), lift(from, 1.), lift(to, 1.), lift(to, -1.)])
}

fn ribbon_mesh(corners: [[f32; 3]; 4], repeat: f32) -> Mesh {
    let mut mesh = Mesh::new(
        bevy::render::render_resource::PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, corners.to_vec());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; 4]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0., 0.], [1., 0.], [1., repeat], [0., repeat]],
    );
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    mesh
}

pub(super) fn draw(
    mut commands: Commands,
    mut bolts: ResMut<Bolts>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for owned in bolts.gone.drain(..) {
        commands.entity(owned.entity).try_despawn();
        meshes.remove(&owned.mesh);
        materials.remove(&owned.material);
        images.remove(&owned.image);
    }
    for bolt in &mut bolts.live {
        if bolt.owned.is_some() {
            continue;
        }
        let Ok(corners) = ribbon(bolt.from, bolt.to, bolt.width) else {
            continue;
        };
        let repeat = {
            let dx = bolt.to[0] - bolt.from[0];
            let dz = bolt.to[2] - bolt.from[2];
            (dx * dx + dz * dz).sqrt() / bolt.tex_scale
        };
        let mesh = ribbon_mesh(corners, repeat);
        use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
        let mut image = Image::new(
            Extent3d {
                width: bolt.size.x,
                height: bolt.size.y,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            bolt.image.clone(),
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        image.sampler = repeating_length_sampler();
        let image = images.add(image);
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(bolt.color[0], bolt.color[1], bolt.color[2], bolt.color[3]),
            base_color_texture: Some(image.clone()),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        });
        let mesh = meshes.add(mesh);
        let entity = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::IDENTITY,
            ))
            .id();
        bolt.image.clear();
        bolt.owned = Some(Owned {
            entity,
            mesh,
            material,
            image,
        });
    }
}

fn repeating_length_sampler() -> ImageSampler {
    let mut descriptor = ImageSamplerDescriptor::linear();
    descriptor.address_mode_u = ImageAddressMode::ClampToEdge;
    descriptor.address_mode_v = ImageAddressMode::Repeat;
    ImageSampler::Descriptor(descriptor)
}

#[cfg(test)]
mod tests {
    use super::{commit, ribbon, styles};
    use bevy::{
        asset::RenderAssetUsages,
        image::{ImageAddressMode, ImageSampler},
        prelude::*,
    };

    fn table() -> String {
        [
            "ID;PWXL;N;E",
            r#"C;Y1;X1;K"Name""#,
            r#"C;X2;K"Dir""#,
            r#"C;X3;K"file""#,
            r#"C;X4;K"Width""#,
            r#"C;X5;K"R""#,
            r#"C;X6;K"G""#,
            r#"C;X7;K"B""#,
            r#"C;X8;K"A""#,
            r#"C;X9;K"Duration""#,
            r#"C;X10;K"TexCoordScale""#,
            r#"C;X11;K"AvgSegLen""#,
            r#"C;X12;K"NoiseScale""#,
            r#"C;X13;K"version""#,
            r#"C;Y2;X1;K"TEST""#,
            r#"C;X2;K"effects""#,
            r#"C;X3;K"bolt.blp""#,
            "C;X4;K12",
            "C;X5;K10",
            "C;X6;K20",
            "C;X7;K30",
            "C;X8;K40",
            "C;X9;K2.5",
            "C;X10;K4",
            "C;X11;K50",
            "C;X12;K0.25",
            "C;X13;K1",
            "E",
        ]
        .join("\r\n")
            + "\r\n"
    }

    #[test]
    fn a_row_supplies_width_color_duration_and_texture_without_stock_numbers() {
        let rows = styles(table().as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "TEST");
        assert_eq!(rows[0].path, r"effects\bolt.blp");
        assert_eq!(rows[0].width, 12.);
        assert_eq!(rows[0].duration, 2.5);
        assert_eq!(rows[0].tex_scale, 4.);
        assert_eq!(
            rows[0].color,
            [10. / 255., 20. / 255., 30. / 255., 40. / 255.]
        );
        assert!(rows[0].unused);
    }

    #[test]
    fn the_ribbon_is_the_segment_widened_by_the_row() {
        let corners = ribbon([0., 1., 0.], [10., 1., 0.], 12.).unwrap();
        assert_eq!(corners[0][0], 0.);
        assert_eq!(corners[1][0], 0.);
        assert!((corners[0][2] + 6.).abs() < 1e-4, "{corners:?}");
        assert!((corners[1][2] - 6.).abs() < 1e-4, "{corners:?}");
        assert_eq!(corners[2][0], 10.);
        assert_eq!(corners[3][0], 10.);
    }

    #[test]
    fn time_advance_without_a_new_cast_expires_the_bolt() {
        let mut bolts = super::Bolts::default();
        bolts.live.push(super::Live {
            from: [0., 0., 0.],
            to: [1., 0., 0.],
            width: 2.,
            color: [1., 1., 1., 1.],
            tex_scale: 1.,
            until: 1.,
            image: Vec::new(),
            size: UVec2::ZERO,
            owned: None,
        });
        super::advance(&mut bolts, 2.);
        assert!(bolts.live.is_empty());
        assert!(bolts.gone.is_empty());
    }

    #[test]
    fn the_row_color_is_not_also_stored_on_the_mesh() {
        let corners = ribbon([0., 0., 0.], [4., 0., 0.], 2.).unwrap();
        let mesh = super::ribbon_mesh(corners, 1.);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none());
    }

    #[test]
    fn texture_scale_repeats_along_the_segment() {
        let sampler = super::repeating_length_sampler();
        let ImageSampler::Descriptor(descriptor) = sampler else {
            panic!("texture scale needs an explicit sampler");
        };
        assert_eq!(descriptor.address_mode_u, ImageAddressMode::ClampToEdge);
        assert_eq!(descriptor.address_mode_v, ImageAddressMode::Repeat);
    }

    #[test]
    fn a_failed_later_hop_does_not_keep_the_earlier_hop() {
        let mut published = vec!["kept"];
        let error = commit(
            &mut published,
            Err("second hop has no lightning row".into()),
        );
        assert_eq!(error.unwrap_err(), "second hop has no lightning row");
        assert_eq!(published, ["kept"]);
    }

    #[test]
    fn a_session_release_clears_cached_styles_and_draw_drops_the_assets() {
        let mut app = App::new();
        app.init_resource::<super::Bolts>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .add_systems(Update, super::draw);
        let entity = app.world_mut().spawn_empty().id();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::new(
                bevy::render::render_resource::PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            ));
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let mesh_id = mesh.id();
        {
            let mut bolts = app.world_mut().resource_mut::<super::Bolts>();
            bolts.rows = Some(Vec::new());
            bolts.clock = 40.;
            bolts.gap = Some("stale".into());
            bolts.live.push(super::Live {
                from: [0., 0., 0.],
                to: [1., 0., 0.],
                width: 2.,
                color: [1., 0., 0., 1.],
                tex_scale: 1.,
                until: 80.,
                image: Vec::new(),
                size: UVec2::ZERO,
                owned: Some(super::Owned {
                    entity,
                    mesh,
                    material,
                    image,
                }),
            });
            super::release(&mut bolts);
            assert!(bolts.live.is_empty());
            assert!(bolts.rows.is_none());
            assert!(bolts.gap.is_none());
            assert_eq!(bolts.clock, 0.);
            assert_eq!(bolts.gone.len(), 1);
        }
        app.update();
        let bolts = app.world().resource::<super::Bolts>();
        assert!(bolts.live.is_empty());
        assert!(bolts.gone.is_empty());
        assert!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(mesh_id)
                .is_none()
        );
        assert!(app.world().get_entity(entity).is_err());
    }

    #[test]
    fn expiry_returns_the_spawned_assets_for_removal() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.init_resource::<Assets<Image>>();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::new(
                bevy::render::render_resource::PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            ));
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let mesh_id = mesh.id();
        let mut bolts = super::Bolts::default();
        bolts.live.push(super::Live {
            from: [0., 0., 0.],
            to: [1., 0., 0.],
            width: 2.,
            color: [1., 0., 0., 1.],
            tex_scale: 1.,
            until: 1.,
            image: Vec::new(),
            size: UVec2::ZERO,
            owned: Some(super::Owned {
                entity,
                mesh,
                material,
                image,
            }),
        });
        super::advance(&mut bolts, 2.);
        assert!(bolts.live.is_empty());
        assert_eq!(bolts.gone.len(), 1);
        let owned = bolts.gone.pop().unwrap();
        assert!(
            app.world_mut()
                .resource_mut::<Assets<Mesh>>()
                .remove(&owned.mesh)
                .is_some()
        );
        assert!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(mesh_id)
                .is_none()
        );
        assert!(app.world().get_entity(entity).is_ok());
    }
}
