//! Executed world-fog boundary switch on the published campaign ground.
use crate::{
    browser_configuration::SelectedMapSources,
    map_binding::{AssetReceipt, receipt},
    map_ground::{GroundPublication, GroundSupply},
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    render::renderer::RenderDevice,
};
use pcx_gfx::GroundMaterial;
use pcx_objects::editor::fog_boundary::BoundaryColor;
use pcx_play::map_startup_host::MapStartupHost;
use std::sync::{Arc, Weak};
const PROFILE: &str = "UI\\MiscData.txt";
// Application allocation policy, separate from ground-sheet texture budgets.
const MASK_BYTES: usize = 16 << 20;
#[derive(Resource, Default)]
pub struct BoundaryPublication {
    session: Option<Weak<()>>,
    material: Option<Handle<GroundMaterial>>,
    image: Option<Handle<Image>>,
    attempted: Option<(AssetId<GroundMaterial>, u64, u64)>,
    pub profile_receipt: Option<AssetReceipt>,
    pub last_error: Option<String>,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<BoundaryPublication>().add_systems(
        Update,
        project
            .after(crate::campaign_presentation::CampaignPresentationSet)
            .after(crate::map_ground::publish_admitted_ground),
    );
}
fn clear(
    state: &mut BoundaryPublication,
    images: &mut Assets<Image>,
    materials: &mut Assets<GroundMaterial>,
) {
    if let Some(mut material) = state.material.take().and_then(|h| materials.get_mut(&h)) {
        material.clear_boundary();
    }
    if let Some(image) = state.image.take() {
        images.remove(image.id());
    }
    state.profile_receipt = None;
    state.attempted = None;
}
#[derive(bevy::ecs::system::SystemParam)]
struct Inputs<'w, 's> {
    ground: Res<'w, GroundPublication>,
    supply: Res<'w, GroundSupply>,
    sources: Res<'w, SelectedMapSources>,
    requests: Res<'w, MapRequests>,
    device: Option<Res<'w, RenderDevice>>,
    draws: Query<'w, 's, &'static MeshMaterial3d<GroundMaterial>>,
}
fn project(
    session: Option<ResMut<MapScriptSession<MapStartupHost>>>,
    input: Inputs,
    mut state: ResMut<BoundaryPublication>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<GroundMaterial>>,
) {
    let Some(mut session) = session else {
        clear(&mut state, &mut images, &mut materials);
        state.session = None;
        return;
    };
    let witness = session.session_witness();
    let same = state
        .session
        .as_ref()
        .zip(witness.as_ref())
        .is_some_and(|(a, b)| Weak::ptr_eq(a, b));
    if !same {
        clear(&mut state, &mut images, &mut materials);
        state.session = witness;
        state.last_error = None;
    }
    if state.session.is_none() {
        return;
    }
    let flag = session
        .host()
        .and_then(|h| h.objects().presentation())
        .and_then(|p| p.world_fog_boundary);
    if flag != Some(true) {
        clear(&mut state, &mut images, &mut materials);
        state.last_error = None;
        return;
    }
    let result = (|| {
        let Some(settings) = session.host().and_then(|h| h.objects().startup_settings()) else {
            clear(&mut state, &mut images, &mut materials);
            return Err("boundary pass needs explicit fog settings".into());
        };
        if settings.fog_enabled || settings.fog_mask_enabled {
            clear(&mut state, &mut images, &mut materials);
            return Err("boundary-only pass requires fog and mask disabled; full fog-of-war rendering is unsupported".into());
        }
        let context = session
            .context()
            .ok_or("boundary pass has no script context")?;
        if !input
            .ground
            .context
            .as_ref()
            .is_some_and(|g| Arc::ptr_eq(g, context))
        {
            return Err("boundary pass is waiting for matching ground".to_owned());
        }
        let material = input
            .ground
            .entity
            .and_then(|e| input.draws.get(e).ok())
            .map(|m| m.0.clone())
            .ok_or("boundary pass has no ground material")?;
        if state.material.as_ref() == Some(&material) && state.image.is_some() {
            return Ok(());
        }
        let bindings = input
            .requests
            .bindings
            .as_ref()
            .ok_or("boundary pass has no binding")?;
        if !input
            .requests
            .admission
            .shared_context()
            .as_ref()
            .is_some_and(|c| Arc::ptr_eq(c, context))
            || !input
                .requests
                .published_receipts
                .as_ref()
                .is_some_and(|r| Arc::ptr_eq(r, &bindings.receipts))
        {
            return Err("boundary pass source binding differs from published session".into());
        }
        if input.supply.revision != bindings.revision {
            return Err("boundary pass has no matching read budget".into());
        }
        let limit = input
            .supply
            .read_limit
            .ok_or("boundary pass has no profile read budget")?;
        let reader = input
            .sources
            .for_receipts(&bindings.receipts)
            .ok_or("boundary pass has no matching source reader")?;
        let key = (material.id(), bindings.revision, limit);
        if state.attempted == Some(key) {
            return state.last_error.clone().map_or(Ok(()), Err);
        }
        state.attempted = Some(key);
        let asset = reader
            .resolve_bounded(PROFILE, limit)
            .map_err(|e| format!("{PROFILE}: {e}"))?;
        let color = BoundaryColor::read(
            &asset.bytes,
            usize::try_from(limit).map_err(|_| "boundary profile budget overflow")?,
        )?;
        let grid = context.alignment.pathing.raster;
        if input.device.as_ref().is_some_and(|d| {
            grid.width > d.limits().max_texture_dimension_2d
                || grid.height > d.limits().max_texture_dimension_2d
        }) {
            return Err("boundary mask exceeds active texture limits".into());
        }
        let mask = pcx_tera::boundary::BoundaryMask::read(
            &context.terrain,
            grid.width,
            grid.height,
            MASK_BYTES,
        )?;
        let mut image = Image::new(
            Extent3d {
                width: mask.width,
                height: mask.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            mask.bytes,
            TextureFormat::R8Unorm,
            RenderAssetUsages::default(),
        );
        image.sampler = bevy::image::ImageSampler::linear();
        let (x, y) = context.terrain.origin();
        let [end_x, end_y, _] = pcx_tera::corner_world(
            &context.terrain,
            context.terrain.width(),
            context.terrain.height(),
        )
        .ok_or("missing boundary extent")?;
        let bounds = Vec4::new(x, y, end_x - x, end_y - y);
        if !bounds.is_finite() || bounds.z <= 0.0 || bounds.w <= 0.0 {
            return Err("invalid boundary world extent".into());
        }
        if !materials.contains(material.id()) {
            return Err("boundary material was retired".into());
        }
        clear(&mut state, &mut images, &mut materials);
        let texture = images.add(image);
        materials
            .get_mut(&material)
            .expect("checked boundary material")
            .set_boundary(texture.clone(), bounds, color.0)
            .map_err(str::to_owned)?;
        state.attempted = Some(key);
        state.material = Some(material);
        state.image = Some(texture);
        state.profile_receipt = Some(receipt(&asset));
        Ok::<(), String>(())
    })();
    state.last_error = result.err();
}
