//! Bevy HUD spawned from [`crate::hud_assets::resolve`].
//!
//! [`install`] draws nothing until a caller sends [`HudSubmit`] or an admitted
//! map and [`HudSourcePolicy`] are both present. A refused candidate leaves
//! the previous entities and image handles in place. A successful candidate
//! despawns that tree and uploads the new images and fonts before the next
//! frame sees either. Placement is stored as percentages of the parent frame,
//! so a pixel resize does not rebake anchors into absolute pixels.
//!
//! Admitted preparation runs after [`peacecraft3::map_requests::MapRequestSet::Deliver`].
//! It reads named members through the receipt-bound resolver. A direct
//! [`HudSubmit`] keeps the caller's own source identity and is not an admitted map.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

use crate::hud_assets::{
    self, HudError, HudInput, PlannedArt, PlannedFrame, PlannedText, RelativeBox, ResolvedHud,
};
use crate::menu_text::{JustifyH, JustifyV};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::widget::{ImageNode, NodeImageMode};
use pcx_play::map_admission::ParsedMapContext;
use peacecraft3::browser_configuration::SelectedMapSources;
use peacecraft3::map_binding::BindingReceipts;
use peacecraft3::map_requests::{MapRequestSet, MapRequests};

/// Where the HUD root hangs. Absent parent: the root is an unparented UI node.
/// The parent entity must already exist. This does not read map admission.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct HudAnchor {
    /// Existing UI entity that should contain the HUD.
    pub parent: Option<Entity>,
}

/// Pixel size of the UI surface. Shadow offsets scale by this. Node placement
/// stays in percentages either way.
#[derive(Resource, Clone, Copy, Debug)]
pub struct HudSurface {
    /// Width and height in logical pixels.
    pub pixels: Vec2,
}

impl Default for HudSurface {
    fn default() -> Self {
        Self { pixels: Vec2::ZERO }
    }
}

/// The mounted candidate, or the last refusal.
#[derive(Resource, Default)]
pub struct HudMount {
    /// Source identity of the mounted candidate.
    pub source: Option<String>,
    /// Race-section order of the mounted candidate.
    pub race_sections: Vec<String>,
    /// Role and frame pairs of the mounted candidate.
    pub roots: Vec<(String, String)>,
    /// Frame-unit viewport of the mounted candidate.
    pub viewport: Option<(f32, f32)>,
    /// Root entity of the mounted tree.
    pub root: Option<Entity>,
    /// Last refusal. Cleared when a candidate mounts.
    pub error: Option<HudError>,
    /// Increments only when a candidate mounts.
    pub generation: u64,
    /// Last [`HudSurface::pixels`] observed by the layout system.
    pub surface_pixels: Option<Vec2>,
    /// How many times the recorded surface pixels changed.
    pub surface_changes: u64,
    images: Vec<Handle<Image>>,
    fonts: Vec<Handle<Font>>,
}

/// Latest input wins. A later refusal in the same frame does not apply an
/// earlier success from that frame, and it does not remove a HUD mounted
/// on a previous frame.
#[derive(Message)]
pub struct HudSubmit {
    /// Candidate to resolve and, on success, mount.
    pub input: HudInput,
}

/// Root of the mounted tree.
#[derive(Component, Debug)]
pub struct HudRoot;

/// A derived frame.
#[derive(Component, Debug)]
pub struct HudFrameNode;

/// Caller role on a supplied root frame.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct HudRole(pub String);

/// Percent placement restored every frame.
#[derive(Component, Clone, Copy, Debug)]
pub struct HudRelative {
    /// Left percent.
    pub left: f32,
    /// Top percent.
    pub top: f32,
    /// Width percent.
    pub width: f32,
    /// Height percent.
    pub height: f32,
}

/// One resolved image layer.
#[derive(Component, Debug)]
pub struct HudArt {
    /// Path the bytes were supplied under.
    pub path: String,
    /// Authored alpha mode, when the texture block had one.
    pub alpha_mode: Option<String>,
}

/// Face and file bound to a text frame.
#[derive(Component, Debug)]
pub struct HudTextFace {
    /// `FrameFont` face.
    pub face: String,
    /// Skin file.
    pub path: String,
}

/// Viewport fraction of a text shadow, Y down. Multiplied by [`HudSurface::pixels`].
#[derive(Component, Debug)]
pub struct HudTextShadow {
    /// Fraction of the surface.
    pub fraction: Vec2,
}

/// Source fields a command-card spawner can read. This is not a command control.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct HudSourceAttachment {
    /// Admitted map identity. Same role as a command control's source label.
    pub source_label: String,
    /// Caller-selected FDF member. Same role as a command control's source member.
    pub source_member: String,
    /// Authored frame name.
    pub frame: String,
    /// Published map receipt path.
    pub receipt_path: String,
    /// Published binding revision.
    pub binding_revision: u64,
}

/// Markup an adapter reported instead of recolouring.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct HudMarkupNotes(pub Vec<String>);

/// Resolver published for one exact receipt `Arc`.
///
/// This is not a map identity. Preparation accepts it only when
/// `Arc::ptr_eq` holds for the admitted bound receipts. [`SelectedMapSources::for_receipts`]
/// wins when it returns a reader for that same `Arc`.
#[derive(Clone)]
pub struct HudSourceWitness {
    /// Receipts this reader was published with.
    pub receipts: Arc<BindingReceipts>,
    /// Bounded reader for those receipts.
    pub reader: Arc<pcx_assets::Resolver>,
}

/// Caller policy for the admitted-map consumer. Member names are read from the
/// admitted resolver. This resource does not carry file bytes.
#[derive(Resource, Clone)]
pub struct HudSourcePolicy {
    /// Section order. Empty refuses preparation.
    pub race_sections: Vec<String>,
    /// Root roles. Empty refuses preparation.
    pub roots: Vec<crate::hud_assets::RootRole>,
    /// Frame-unit viewport. Mutating it is a policy change and remounts.
    pub viewport: (f32, f32),
    /// Explicit budgets. A read uses these limits before copying a member.
    pub budgets: crate::hud_assets::HudBudgets,
    /// Plain text or a caller adapter.
    pub markup: crate::hud_assets::HudMarkup,
    /// FDF member copied onto each frame. It must be one of `fdf_members`.
    pub source_member: String,
    /// FDF members, caller order, read from the admitted resolver.
    pub fdf_members: Vec<String>,
    /// Skin member read from that resolver.
    pub skin_member: String,
    /// Global-strings member. Empty leaves `Text` values as authored.
    pub global_member: String,
    /// `DecorateFileNames` suffixes.
    pub decorate_suffixes: Vec<String>,
    /// Image members read from that resolver.
    pub image_members: Vec<String>,
    /// Font members read from that resolver.
    pub font_members: Vec<String>,
    /// Explicit reader. Ignored as a source of map identity.
    pub witness: Option<HudSourceWitness>,
}

struct AdmissionStamp {
    context: Weak<ParsedMapContext>,
    revision: u64,
}

struct AdmissionGate {
    stamp: AdmissionStamp,
    context: Arc<ParsedMapContext>,
    receipts: Arc<BindingReceipts>,
    reader: Arc<pcx_assets::Resolver>,
}

/// Candidate produced by the admission consumer for the same frame's mount.
#[derive(Resource, Default)]
struct HudPending {
    input: Option<HudInput>,
    stamp: Option<AdmissionStamp>,
}

/// Last admission preparation. A refusal does not clear a mounted HUD.
///
/// A failed attempt is not recorded as mounted. The same context and an
/// unchanged policy are not read again until the caller changes the policy or
/// the admitted context changes.
#[derive(Resource, Default)]
pub struct HudPrepare {
    mounted: Option<AdmissionStamp>,
    attempted: Option<AdmissionStamp>,
    held: bool,
    /// Why the latest admission was not mounted.
    pub refusal: Option<String>,
}

impl HudPrepare {
    /// Whether the mounted HUD belongs to this exact published map context and revision.
    pub(crate) fn mounted_for(&self, context: &Arc<ParsedMapContext>, revision: u64) -> bool {
        self.mounted.as_ref().is_some_and(|stamp| {
            stamp.revision == revision && Weak::ptr_eq(&stamp.context, &Arc::downgrade(context))
        })
    }
}

/// Registers the HUD systems. Nothing is drawn until [`HudSubmit`] or until
/// [`HudSourcePolicy`] and an admitted [`MapRequests`] outcome are both present.
/// Admitted preparation is ordered after [`MapRequestSet::Deliver`].
pub fn install(app: &mut App) {
    app.init_resource::<HudMount>()
        .init_resource::<HudAnchor>()
        .init_resource::<HudSurface>()
        .add_message::<HudSubmit>()
        .init_resource::<HudPrepare>()
        .init_resource::<HudPending>()
        .init_resource::<SelectedMapSources>()
        .configure_sets(Update, MapRequestSet::Deliver)
        .add_systems(
            Update,
            (
                prepare_admitted_hud.after(MapRequestSet::Deliver),
                apply_hud,
                sync_hud_layout,
                track_hud_surface,
            )
                .chain()
                .before(peacecraft3::campaign_presentation::CampaignPresentationSet),
        );
}

fn prepare_admitted_hud(
    requests: Option<Res<MapRequests>>,
    sources: Res<SelectedMapSources>,
    policy: Option<Res<HudSourcePolicy>>,
    mut prepared: ResMut<HudPrepare>,
    mut pending: ResMut<HudPending>,
) {
    pending.input = None;
    pending.stamp = None;
    let Some(policy) = policy else {
        prepared.refusal = Some("caller HUD policy is absent".into());
        prepared.held = true;
        return;
    };
    let changed = policy.is_changed();
    let gate = match admitted_gate(requests.as_deref(), &sources, &policy) {
        Ok(gate) => gate,
        Err(reason) => {
            prepared.refusal = Some(reason);
            prepared.held = true;
            return;
        }
    };
    if !changed && prepared.held && same_stamp(&prepared.attempted, &gate) {
        return;
    }
    if !changed && !prepared.held && same_stamp(&prepared.mounted, &gate) {
        prepared.refusal = None;
        return;
    }
    match admitted_hud(&gate, &policy) {
        Ok(input) => {
            pending.stamp = Some(gate.stamp);
            pending.input = Some(input);
        }
        Err(reason) => {
            prepared.attempted = Some(gate.stamp);
            prepared.held = true;
            prepared.refusal = Some(reason);
        }
    }
}

fn same_stamp(stamp: &Option<AdmissionStamp>, gate: &AdmissionGate) -> bool {
    stamp.as_ref().is_some_and(|stamp| {
        stamp.revision == gate.stamp.revision && Weak::ptr_eq(&stamp.context, &gate.stamp.context)
    })
}

#[cfg(test)]
fn admitted_input(
    requests: &MapRequests,
    sources: &SelectedMapSources,
    policy: &HudSourcePolicy,
) -> Result<HudInput, String> {
    admitted_hud(&admitted_gate(Some(requests), sources, policy)?, policy)
}

fn admitted_gate(
    requests: Option<&MapRequests>,
    sources: &SelectedMapSources,
    policy: &HudSourcePolicy,
) -> Result<AdmissionGate, String> {
    if policy.race_sections.is_empty() {
        return Err("caller race section order is empty".into());
    }
    if policy.roots.is_empty() {
        return Err("caller root roles are empty".into());
    }
    if policy.source_member.is_empty()
        || policy.fdf_members.is_empty()
        || !policy
            .fdf_members
            .iter()
            .any(|member| member == &policy.source_member)
    {
        return Err("caller FDF member is empty or not one of the admitted reads".into());
    }
    if policy.skin_member.is_empty() {
        return Err("caller skin member is empty".into());
    }
    if !(policy.viewport.0.is_finite()
        && policy.viewport.1.is_finite()
        && policy.viewport.0 > 0.0
        && policy.viewport.1 > 0.0)
    {
        return Err("caller viewport is not a finite positive pair".into());
    }
    let requests = requests.ok_or("no admitted map receipt")?;
    let outcome = requests
        .last_outcome
        .as_ref()
        .ok_or("no admitted map outcome")?;
    if let Err(error) = &outcome.result {
        return Err(format!("admitted map outcome refused: {error}"));
    }
    let bindings = requests.bindings.as_ref().ok_or("no map bindings")?;
    let context = requests
        .admission
        .shared_context()
        .ok_or("admitted map outcome has no parsed context")?;
    if outcome.binding_revision != Some(bindings.revision)
        || outcome.identity.as_ref() != Some(&context.sources.map)
        || context.sources.map.edition != bindings.map_identity.edition
        || context.sources.map.version != bindings.map_identity.version
        || !requests
            .published_receipts
            .as_ref()
            .is_some_and(|receipts| Arc::ptr_eq(receipts, &bindings.receipts))
    {
        return Err("admitted outcome is not this context and published receipt".into());
    }
    let receipts = Arc::clone(&bindings.receipts);
    let reader = bound_reader(sources, &receipts, policy)?;
    Ok(AdmissionGate {
        stamp: AdmissionStamp {
            context: Arc::downgrade(&context),
            revision: bindings.revision,
        },
        context,
        receipts,
        reader,
    })
}

fn bound_reader(
    sources: &SelectedMapSources,
    receipts: &Arc<BindingReceipts>,
    policy: &HudSourcePolicy,
) -> Result<Arc<pcx_assets::Resolver>, String> {
    let published = sources.for_receipts(receipts);
    let witnessed = policy.witness.as_ref().and_then(|witness| {
        Arc::ptr_eq(&witness.receipts, receipts).then(|| Arc::clone(&witness.reader))
    });
    match (published, witnessed) {
        (Some(reader), Some(witness)) if !Arc::ptr_eq(&reader, &witness) => {
            Err("selected source reader and the policy witness are different resolvers".into())
        }
        (Some(reader), _) => Ok(reader),
        (None, Some(reader)) => Ok(reader),
        (None, None) => Err("admitted map has no source reader for the published receipts".into()),
    }
}

fn admitted_hud(gate: &AdmissionGate, policy: &HudSourcePolicy) -> Result<HudInput, String> {
    let mut remaining = policy.budgets.max_input_bytes;
    let mut fdf_texts = Vec::new();
    for member in &policy.fdf_members {
        fdf_texts.push(read_text(&gate.reader, member, &mut remaining)?);
    }
    let skin_text = read_text(&gate.reader, &policy.skin_member, &mut remaining)?;
    let global_strings = if policy.global_member.is_empty() {
        String::new()
    } else {
        read_text(&gate.reader, &policy.global_member, &mut remaining)?
    };
    let mut images = Vec::new();
    for member in &policy.image_members {
        let limit = remaining.min(policy.budgets.max_image_input_bytes);
        let bytes = read_limited(&gate.reader, member, limit)?;
        take_remaining(&mut remaining, member, bytes.len())?;
        images.push(crate::hud_assets::NamedBytes {
            path: member.clone(),
            bytes,
        });
    }
    let mut fonts = Vec::new();
    let mut font_aggregate = 0usize;
    for member in &policy.font_members {
        let limit = remaining.min(policy.budgets.max_font_bytes).min(
            policy
                .budgets
                .max_font_aggregate_bytes
                .saturating_sub(font_aggregate),
        );
        let bytes = read_limited(&gate.reader, member, limit)?;
        font_aggregate = font_aggregate
            .checked_add(bytes.len())
            .ok_or_else(|| format!("{member}: font aggregate overflows"))?;
        if font_aggregate > policy.budgets.max_font_aggregate_bytes {
            return Err(format!(
                "{member}: font aggregate {font_aggregate} exceeds {}",
                policy.budgets.max_font_aggregate_bytes
            ));
        }
        take_remaining(&mut remaining, member, bytes.len())?;
        fonts.push(crate::hud_assets::NamedBytes {
            path: member.clone(),
            bytes,
        });
    }
    let identity = &gate.context.sources.map;
    let receipt_path = gate.receipts.map.path.as_str().to_string();
    let source = format!(
        "edition={} version={} source={} revision={} map={receipt_path}",
        identity.edition, identity.version, identity.source, gate.stamp.revision
    );
    Ok(HudInput {
        source: crate::hud_assets::SourceId { identity: source },
        fdf_texts,
        skin_text,
        race_sections: policy.race_sections.clone(),
        roots: policy.roots.clone(),
        viewport: policy.viewport,
        global_strings,
        decorate_suffixes: policy.decorate_suffixes.clone(),
        images,
        fonts,
        budgets: policy.budgets,
        markup: policy.markup.clone(),
        source_member: policy.source_member.clone(),
        receipt_path,
        binding_revision: gate.stamp.revision,
    })
}

fn read_text(
    reader: &pcx_assets::Resolver,
    member: &str,
    remaining: &mut usize,
) -> Result<String, String> {
    let bytes = read_limited(reader, member, *remaining)?;
    take_remaining(remaining, member, bytes.len())?;
    String::from_utf8(bytes).map_err(|_| format!("{member} is not UTF-8"))
}

fn read_limited(
    reader: &pcx_assets::Resolver,
    member: &str,
    limit: usize,
) -> Result<Vec<u8>, String> {
    if member.is_empty() {
        return Err("caller source member is empty".into());
    }
    let asset = reader
        .resolve_bounded(member, limit as u64)
        .map_err(|error| format!("reading {member}: {error}"))?;
    if asset.bytes.len() > limit {
        return Err(format!(
            "{member}: {} bytes exceed caller limit {limit}",
            asset.bytes.len()
        ));
    }
    Ok(asset.bytes)
}

fn take_remaining(remaining: &mut usize, member: &str, length: usize) -> Result<(), String> {
    if length > *remaining {
        return Err(format!(
            "{member}: {length} bytes exceed the remaining input budget {remaining}"
        ));
    }
    *remaining -= length;
    Ok(())
}

#[derive(SystemParam)]
struct HudMountArgs<'w, 's> {
    commands: Commands<'w, 's>,
    images: ResMut<'w, Assets<Image>>,
    fonts: ResMut<'w, Assets<Font>>,
    mount: ResMut<'w, HudMount>,
    anchor: Res<'w, HudAnchor>,
    pending: ResMut<'w, HudPending>,
    submits: MessageReader<'w, 's, HudSubmit>,
    live: Query<'w, 's, Entity>,
    prepared: ResMut<'w, HudPrepare>,
}

fn apply_hud(mut args: HudMountArgs) {
    let pending_input = args.pending.input.take();
    let stamp = args.pending.stamp.take();
    let submitted = args.submits.read().last();
    if pending_input.is_none() && submitted.is_none() {
        return;
    }
    if let Some(parent) = args.anchor.parent
        && args.live.get(parent).is_err()
    {
        let error = HudError::Input(format!("HUD parent {parent} is not alive"));
        warn!("{error}");
        note_failure(&mut args.prepared, stamp, &mut args.mount, error);
        return;
    }
    let resolved = match if let Some(input) = pending_input.as_ref() {
        hud_assets::resolve(input)
    } else if let Some(input) = submitted.as_ref() {
        hud_assets::resolve(&input.input)
    } else {
        return;
    } {
        Ok(resolved) => resolved,
        Err(error) => {
            warn!("HUD candidate refused: {error}");
            note_failure(&mut args.prepared, stamp, &mut args.mount, error);
            return;
        }
    };
    if let Err(error) = mount_resolved(
        &mut args.commands,
        &mut args.images,
        &mut args.fonts,
        &mut args.mount,
        &args.anchor,
        resolved,
    ) {
        warn!("HUD candidate refused: {error}");
        note_failure(&mut args.prepared, stamp, &mut args.mount, error);
        return;
    }
    if let Some(stamp) = stamp {
        args.prepared.mounted = Some(stamp);
        args.prepared.attempted = None;
        args.prepared.held = false;
        args.prepared.refusal = None;
    }
}

fn note_failure(
    prepared: &mut HudPrepare,
    stamp: Option<AdmissionStamp>,
    mount: &mut HudMount,
    error: HudError,
) {
    if stamp.is_some() {
        prepared.attempted = stamp;
        prepared.held = true;
        prepared.refusal = Some(error.to_string());
    }
    mount.error = Some(error);
}

fn mount_resolved(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    fonts: &mut Assets<Font>,
    mount: &mut HudMount,
    anchor: &HudAnchor,
    resolved: ResolvedHud,
) -> Result<(), HudError> {
    let image_handles = upload_images(images, &resolved)?;
    let font_handles = match upload_fonts(fonts, &resolved) {
        Ok(handles) => handles,
        Err(error) => {
            discard_images(images, image_handles);
            return Err(error);
        }
    };
    if let Err(error) = covers(&resolved, &image_handles, &font_handles) {
        discard_images(images, image_handles);
        discard_fonts(fonts, font_handles);
        return Err(error);
    }
    if let Some(root) = mount.root.take() {
        commands.entity(root).try_despawn();
    }
    let old_images = std::mem::take(&mut mount.images);
    let old_fonts = std::mem::take(&mut mount.fonts);
    let generation = mount.generation.saturating_add(1);
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            HudRelative {
                left: 0.0,
                top: 0.0,
                width: 100.0,
                height: 100.0,
            },
            HudRoot,
            peacecraft3::campaign_presentation::CampaignHud,
            peacecraft3::campaign_presentation::CampaignInterface,
            Pickable::IGNORE,
            Name::new("Hud"),
        ))
        .id();
    if let Some(parent) = anchor.parent {
        commands.entity(parent).add_child(root);
    }
    let mut frames = HashMap::new();
    for frame in &resolved.frames {
        let entity = commands
            .spawn((
                frame_node(frame),
                relative_component(frame.relative),
                HudFrameNode,
                HudSourceAttachment {
                    source_label: resolved.source.clone(),
                    source_member: resolved.source_member.clone(),
                    frame: frame.name.clone(),
                    receipt_path: resolved.receipt_path.clone(),
                    binding_revision: resolved.binding_revision,
                },
                Pickable::IGNORE,
                Name::new(frame.name.clone()),
            ))
            .id();
        if let Some(role) = &frame.role {
            commands.entity(entity).insert(HudRole(role.clone()));
        }
        frames.insert(frame.name.clone(), entity);
    }
    for frame in &resolved.frames {
        let entity = frames[&frame.name];
        let parent = frame
            .parent
            .as_ref()
            .and_then(|name| frames.get(name))
            .copied()
            .unwrap_or(root);
        commands.entity(parent).add_child(entity);
        for art in &frame.art {
            spawn_art(commands, entity, art, &image_handles);
        }
        if let Some(text) = &frame.text {
            spawn_text(commands, entity, text, &font_handles[&text.font_path]);
        }
    }
    mount.source = Some(resolved.source);
    mount.race_sections = resolved.race_sections;
    mount.roots = resolved.roots;
    mount.viewport = Some(resolved.viewport);
    mount.root = Some(root);
    mount.error = None;
    mount.generation = generation;
    mount.images = image_handles.into_values().collect();
    mount.fonts = font_handles.into_values().collect();
    for handle in old_images {
        images.remove(handle.id());
    }
    for handle in old_fonts {
        fonts.remove(handle.id());
    }
    info!(
        "mounted HUD {:?} generation {generation}",
        mount.source.as_deref().unwrap_or("")
    );
    Ok(())
}

fn covers(
    resolved: &ResolvedHud,
    images: &HashMap<String, Handle<Image>>,
    fonts: &HashMap<String, Handle<Font>>,
) -> Result<(), HudError> {
    for frame in &resolved.frames {
        for art in &frame.art {
            if !images.contains_key(&art.path) {
                return Err(HudError::Image {
                    path: art.path.clone(),
                    detail: "decoded image was not uploaded".into(),
                });
            }
        }
        if let Some(text) = &frame.text
            && !fonts.contains_key(&text.font_path)
        {
            return Err(HudError::Font {
                path: text.font_path.clone(),
                detail: "font was not uploaded".into(),
            });
        }
    }
    Ok(())
}

fn upload_images(
    images: &mut Assets<Image>,
    resolved: &ResolvedHud,
) -> Result<HashMap<String, Handle<Image>>, HudError> {
    let mut handles = HashMap::new();
    let mut seen = HashSet::new();
    for decoded in &resolved.images {
        if !seen.insert(decoded.path.clone()) {
            discard_images(images, handles);
            return Err(HudError::Image {
                path: decoded.path.clone(),
                detail: "duplicate decoded image".into(),
            });
        }
        let handle = images.add(pcx_gfx::viewer::rgba_image(&decoded.texture));
        handles.insert(decoded.path.clone(), handle);
    }
    Ok(handles)
}

fn upload_fonts(
    fonts: &mut Assets<Font>,
    resolved: &ResolvedHud,
) -> Result<HashMap<String, Handle<Font>>, HudError> {
    let mut handles = HashMap::new();
    for font in &resolved.fonts {
        if handles.contains_key(&font.path) {
            discard_fonts(fonts, handles);
            return Err(HudError::Font {
                path: font.path.clone(),
                detail: "duplicate decoded font".into(),
            });
        }
        if !font_registers(&font.bytes) {
            discard_fonts(fonts, handles);
            return Err(HudError::Font {
                path: font.path.clone(),
                detail: "font bytes are not a readable face; Font::from_bytes does not validate"
                    .into(),
            });
        }
        handles.insert(
            font.path.clone(),
            fonts.add(Font::from_bytes(font.bytes.clone())),
        );
    }
    Ok(handles)
}

pub(crate) fn font_registers(bytes: &[u8]) -> bool {
    let mut fonts = bevy::text::FontCx::default();
    let face = Font::from_bytes(bytes.to_vec());
    !fonts.collection.register_fonts(face.data, None).is_empty()
}

fn discard_images(images: &mut Assets<Image>, handles: HashMap<String, Handle<Image>>) {
    for handle in handles.into_values() {
        images.remove(handle.id());
    }
}

fn discard_fonts(fonts: &mut Assets<Font>, handles: HashMap<String, Handle<Font>>) {
    for handle in handles.into_values() {
        fonts.remove(handle.id());
    }
}

fn spawn_art(
    commands: &mut Commands,
    parent: Entity,
    art: &PlannedArt,
    images: &HashMap<String, Handle<Image>>,
) {
    let handle = images[&art.path].clone();
    let mut image = ImageNode::new(handle).with_mode(NodeImageMode::Stretch);
    image.rect = Some(Rect::new(
        art.uv.min_x,
        art.uv.min_y,
        art.uv.max_x,
        art.uv.max_y,
    ));
    image.flip_x = art.flip_x;
    image.flip_y = art.flip_y;
    let entity = commands
        .spawn((
            relative_node(art.relative),
            relative_component(art.relative),
            image,
            HudArt {
                path: art.path.clone(),
                alpha_mode: art.alpha_mode.clone(),
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(entity);
}

pub(crate) fn spawn_text(
    commands: &mut Commands,
    parent: Entity,
    text: &PlannedText,
    handle: &Handle<Font>,
) -> Entity {
    let handle = handle.clone();
    let font = bevy::text::FontSource::Handle(handle);
    let size = bevy::text::FontSize::Vh(text.height_vh);
    let entity = commands
        .spawn((
            Text::new(""),
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            bevy::text::LineHeight::RelativeToFont(1.0),
            TextColor(srgba(text.colour)),
            TextLayout::new(
                justify_text(text.justify_h),
                if text.wrap {
                    bevy::text::LineBreak::WordBoundary
                } else {
                    bevy::text::LineBreak::NoWrap
                },
            ),
            TextShadow {
                offset: Vec2::ZERO,
                color: srgba(text.shadow),
            },
            HudTextShadow {
                fraction: Vec2::new(text.shadow_fraction[0], text.shadow_fraction[1]),
            },
            HudTextFace {
                face: text.face.clone(),
                path: text.font_path.clone(),
            },
            Pickable::IGNORE,
        ))
        .id();
    if !text.notes.is_empty() {
        commands
            .entity(entity)
            .insert(HudMarkupNotes(text.notes.clone()));
    }
    for span in &text.spans {
        let colour = span.colour.unwrap_or(text.colour);
        let child = commands
            .spawn((
                TextSpan::new(span.text.clone()),
                TextFont {
                    font: font.clone(),
                    font_size: size,
                    ..default()
                },
                bevy::text::LineHeight::RelativeToFont(1.0),
                TextColor(srgba(colour)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(entity).add_child(child);
    }
    commands.entity(parent).add_child(entity);
    entity
}

fn frame_node(frame: &PlannedFrame) -> Node {
    let mut node = relative_node(frame.relative);
    if let Some(text) = &frame.text {
        style_text_node(&mut node, text);
    }
    node
}

pub(crate) fn style_text_node(node: &mut Node, text: &PlannedText) {
    node.flex_direction = FlexDirection::Column;
    node.justify_content = justify_content(text.justify_v);
    node.align_items = align_items(text.justify_h);
    if let Some([x, y]) = text.padding_vw_vh {
        node.padding.left = Val::Vw(x);
        node.padding.top = Val::Vh(y);
    }
}

fn relative_node(relative: RelativeBox) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(relative.left),
        top: Val::Percent(relative.top),
        width: Val::Percent(relative.width),
        height: Val::Percent(relative.height),
        ..default()
    }
}

fn relative_component(relative: RelativeBox) -> HudRelative {
    HudRelative {
        left: relative.left,
        top: relative.top,
        width: relative.width,
        height: relative.height,
    }
}

fn sync_hud_layout(
    surface: Res<HudSurface>,
    mut placed: Query<(&HudRelative, &mut Node)>,
    mut shadows: Query<(&HudTextShadow, &mut TextShadow)>,
) {
    for (relative, mut node) in &mut placed {
        let left = Val::Percent(relative.left);
        let top = Val::Percent(relative.top);
        let width = Val::Percent(relative.width);
        let height = Val::Percent(relative.height);
        if node.position_type != PositionType::Absolute {
            node.position_type = PositionType::Absolute;
        }
        if node.left != left {
            node.left = left;
        }
        if node.top != top {
            node.top = top;
        }
        if node.width != width {
            node.width = width;
        }
        if node.height != height {
            node.height = height;
        }
    }
    let pixels = surface.pixels;
    let offset_of = |fraction: Vec2| {
        if pixels.x.is_finite() && pixels.y.is_finite() && pixels.x > 0.0 && pixels.y > 0.0 {
            fraction * pixels
        } else {
            Vec2::ZERO
        }
    };
    for (shadow, mut text) in &mut shadows {
        let offset = offset_of(shadow.fraction);
        if text.offset != offset {
            text.offset = offset;
        }
    }
}

fn track_hud_surface(surface: Res<HudSurface>, mut mount: ResMut<HudMount>) {
    if mount.surface_pixels != Some(surface.pixels) {
        mount.surface_pixels = Some(surface.pixels);
        mount.surface_changes = mount.surface_changes.saturating_add(1);
    }
}

fn justify_text(value: JustifyH) -> bevy::text::Justify {
    match value {
        JustifyH::Left => bevy::text::Justify::Left,
        JustifyH::Center => bevy::text::Justify::Center,
        JustifyH::Right => bevy::text::Justify::Right,
    }
}

fn justify_content(value: JustifyV) -> JustifyContent {
    match value {
        JustifyV::Top => JustifyContent::FlexStart,
        JustifyV::Middle => JustifyContent::Center,
        JustifyV::Bottom => JustifyContent::FlexEnd,
    }
}

fn align_items(value: JustifyH) -> AlignItems {
    match value {
        JustifyH::Left => AlignItems::FlexStart,
        JustifyH::Center => AlignItems::Center,
        JustifyH::Right => AlignItems::FlexEnd,
    }
}

fn srgba(colour: [f32; 4]) -> Color {
    Color::srgba(colour[0], colour[1], colour[2], colour[3])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud_assets::{HudInput, sample_hud};
    use bevy::ecs::hierarchy::ChildOf;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Image>>();
        app.init_resource::<Assets<Font>>();
        install(&mut app);
        app
    }

    fn submit(app: &mut App, input: HudInput) {
        app.world_mut().write_message(HudSubmit { input });
    }

    fn named(app: &mut App, name: &str) -> Entity {
        let mut query = app.world_mut().query::<(Entity, &Name)>();
        let found: Vec<Entity> = query
            .iter(app.world())
            .filter(|(_, node)| node.as_str() == name)
            .map(|(entity, _)| entity)
            .collect();
        assert_eq!(found.len(), 1, "{name}");
        found[0]
    }

    fn image_handle(app: &App, entity: Entity) -> Handle<Image> {
        app.world().get::<ImageNode>(entity).unwrap().image.clone()
    }

    #[test]
    fn spawns_source_qualified_nodes_with_relative_anchors() {
        let mut app = test_app();
        submit(&mut app, sample_hud(&["Human", "Default"]));
        app.update();
        let mount = app.world().resource::<HudMount>();
        assert!(mount.error.is_none(), "{:?}", mount.error);
        assert_eq!(mount.source.as_deref(), Some("fixture:Human,Default"));
        assert_eq!(mount.race_sections, ["Human", "Default"]);
        assert_eq!(mount.generation, 1);
        let root = mount.root.unwrap();

        let console = named(&mut app, "Console");
        let gold = named(&mut app, "Gold");
        let plate = named(&mut app, "Plate");
        assert_eq!(app.world().get::<ChildOf>(console).unwrap().parent(), root);
        assert_eq!(app.world().get::<ChildOf>(gold).unwrap().parent(), console);
        assert_eq!(app.world().get::<ChildOf>(plate).unwrap().parent(), root);
        assert_eq!(app.world().get::<HudRole>(console).unwrap().0, "console");
        assert_eq!(app.world().get::<HudRole>(plate).unwrap().0, "plate");

        let console_node = app.world().get::<Node>(console).unwrap();
        assert_eq!(console_node.left, Val::Percent(0.0));
        assert_eq!(console_node.width, Val::Percent(100.0));
        assert_eq!(console_node.position_type, PositionType::Absolute);
        let gold_node = app.world().get::<Node>(gold).unwrap();
        assert_eq!(gold_node.left, Val::Percent(0.0));
        assert_eq!(gold_node.top, Val::Percent(0.0));
        assert_eq!(gold_node.width, Val::Percent(25.0));
        assert_eq!(gold_node.height, Val::Percent(25.0));

        let mut art = app.world_mut().query::<(Entity, &HudArt, &ChildOf)>();
        let layers: Vec<_> = art
            .iter(app.world())
            .map(|(entity, layer, parent)| {
                (
                    entity,
                    layer.path.clone(),
                    layer.alpha_mode.clone(),
                    parent.parent(),
                )
            })
            .collect();
        let tile = layers
            .iter()
            .find(|(_, path, _, parent)| path == "art/human-tile.blp" && *parent == console)
            .unwrap();
        assert_eq!(tile.2.as_deref(), Some("ALPHAKEY"));
        let tile_node = app.world().get::<Node>(tile.0).unwrap();
        assert_eq!(tile_node.left, Val::Percent(25.0));
        assert_eq!(tile_node.top, Val::Percent(75.0));
        assert_eq!(tile_node.width, Val::Percent(25.0));
        assert_eq!(tile_node.height, Val::Percent(25.0));
        let (rect, flip_x, flip_y) = {
            let image = app.world().get::<ImageNode>(tile.0).unwrap();
            (image.rect.unwrap(), image.flip_x, image.flip_y)
        };
        assert_eq!(rect.min, Vec2::new(0.0, 0.0));
        assert_eq!(rect.max, Vec2::new(2.0, 2.0));
        assert!(!flip_x);
        assert!(!flip_y);
        let (receipt_path, source_label) = {
            let attachment = app.world().get::<HudSourceAttachment>(console).unwrap();
            (
                attachment.receipt_path.clone(),
                attachment.source_label.clone(),
            )
        };
        assert!(receipt_path.is_empty());
        assert_eq!(source_label, "fixture:Human,Default");
        let plate_layer = layers
            .iter()
            .find(|(_, path, _, parent)| path == "plates/a.blp" && *parent == plate)
            .unwrap();
        assert!(plate_layer.2.is_none());

        let mut texts = app.world_mut().query::<(Entity, &HudTextFace, &ChildOf)>();
        let (text, face_path, face_name, text_parent) = texts
            .iter(app.world())
            .next()
            .map(|(text, face, parent)| {
                (text, face.path.clone(), face.face.clone(), parent.parent())
            })
            .unwrap();
        assert_eq!(text_parent, gold);
        assert_eq!(face_path, "fonts/body.ttf");
        assert_eq!(face_name, "MasterFont");
        let mut spans = app.world_mut().query::<(&TextSpan, &ChildOf)>();
        let visible = spans
            .iter(app.world())
            .find(|(_, child)| child.parent() == text)
            .map(|(span, _)| span.0.clone());
        assert_eq!(visible.as_deref(), Some("Gold"));
    }

    #[test]
    fn a_failed_candidate_keeps_the_mounted_hud_and_a_new_race_replaces_it() {
        let mut app = test_app();
        submit(&mut app, sample_hud(&["Human", "Default"]));
        app.update();
        let human_root = app.world().resource::<HudMount>().root.unwrap();
        let console = named(&mut app, "Console");
        let mut art = app.world_mut().query::<(Entity, &HudArt)>();
        let human_tile = art
            .iter(app.world())
            .find(|(_, layer)| layer.path == "art/human-tile.blp")
            .unwrap()
            .0;
        let human_image = image_handle(&app, human_tile);
        let image_count = app.world().resource::<Assets<Image>>().len();

        let mut missing = sample_hud(&["Orc", "Default"]);
        missing.images.clear();
        submit(&mut app, missing);
        app.update();
        let mount = app.world().resource::<HudMount>();
        assert!(
            mount.error.is_some(),
            "missing art must be visible on the mount"
        );
        assert_eq!(mount.generation, 1);
        assert_eq!(mount.root, Some(human_root));
        assert_eq!(mount.source.as_deref(), Some("fixture:Human,Default"));
        assert!(app.world().get_entity(human_root).is_ok());
        assert!(app.world().get_entity(console).is_ok());
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&human_image)
                .is_some()
        );
        assert_eq!(app.world().resource::<Assets<Image>>().len(), image_count);

        submit(&mut app, sample_hud(&["Orc", "Default"]));
        app.update();
        let mount = app.world().resource::<HudMount>();
        assert!(mount.error.is_none(), "{:?}", mount.error);
        assert_eq!(mount.generation, 2);
        assert_eq!(mount.source.as_deref(), Some("fixture:Orc,Default"));
        assert_eq!(mount.race_sections, ["Orc", "Default"]);
        let orc_root = mount.root.unwrap();
        assert_ne!(orc_root, human_root);
        assert!(app.world().get_entity(human_root).is_err());
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&human_image)
                .is_none()
        );
        let mut art = app.world_mut().query::<&HudArt>();
        let paths: Vec<_> = art
            .iter(app.world())
            .map(|layer| layer.path.clone())
            .collect();
        assert!(paths.iter().any(|path| path == "art/orc-tile.blp"));
        assert!(paths.iter().all(|path| path != "art/human-tile.blp"));
        assert_eq!(app.world().resource::<Assets<Image>>().len(), image_count);
    }

    #[test]
    fn a_later_refusal_in_the_same_frame_does_not_mount_the_earlier_input() {
        let mut app = test_app();
        submit(&mut app, sample_hud(&["Human", "Default"]));
        let mut missing = sample_hud(&["Orc", "Default"]);
        missing.images.clear();
        submit(&mut app, missing);
        app.update();
        let mount = app.world().resource::<HudMount>();
        assert!(mount.root.is_none());
        assert!(mount.error.is_some());
        assert_eq!(mount.generation, 0);
    }

    #[test]
    fn pixel_resize_keeps_percent_placement_and_scales_shadow_offset() {
        let mut app = test_app();
        submit(&mut app, sample_hud(&["Human", "Default"]));
        app.update();
        let changes = app.world().resource::<HudMount>().surface_changes;
        let gold = named(&mut app, "Gold");
        let mut texts = app.world_mut().query::<(Entity, &HudTextFace)>();
        let text = texts.iter(app.world()).next().unwrap().0;
        let font = app.world().get::<TextFont>(text).unwrap().font.clone();
        {
            let mut node = app.world_mut().get_mut::<Node>(gold).unwrap();
            node.left = Val::Px(10.0);
            node.width = Val::Px(40.0);
        }
        app.world_mut().resource_mut::<HudSurface>().pixels = Vec2::new(8.0, 4.0);
        app.update();
        let node = app.world().get::<Node>(gold).unwrap();
        assert_eq!(node.left, Val::Percent(0.0));
        assert_eq!(node.top, Val::Percent(0.0));
        assert_eq!(node.width, Val::Percent(25.0));
        assert_eq!(node.height, Val::Percent(25.0));
        assert_eq!(node.position_type, PositionType::Absolute);
        let offset = app.world().get::<TextShadow>(text).unwrap().offset;
        assert_eq!(offset, Vec2::new(2.0, 1.0));
        assert_eq!(app.world().get::<TextFont>(text).unwrap().font, font);
        let mount = app.world().resource::<HudMount>();
        assert_eq!(mount.generation, 1);
        assert!(mount.surface_changes > changes);
        assert_eq!(mount.surface_pixels, Some(Vec2::new(8.0, 4.0)));
    }

    #[test]
    fn a_dead_parent_refuses_the_candidate_and_keeps_the_previous_root() {
        let mut app = test_app();
        submit(&mut app, sample_hud(&["Human", "Default"]));
        app.update();
        let root = app.world().resource::<HudMount>().root.unwrap();
        let stale = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(stale).despawn();
        app.world_mut().resource_mut::<HudAnchor>().parent = Some(stale);
        submit(&mut app, sample_hud(&["Orc", "Default"]));
        app.update();
        let mount = app.world().resource::<HudMount>();
        assert!(mount.error.is_some());
        assert_eq!(mount.root, Some(root));
        assert_eq!(mount.generation, 1);
        assert_eq!(mount.source.as_deref(), Some("fixture:Human,Default"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "caller-selected original FDF, skin, section order, roots, and art bytes"]
    fn hud_selected_original_source() {
        let identity = env_var("PEACECRAFT_HUD_SOURCE");
        let input = HudInput {
            source: crate::hud_assets::SourceId {
                identity: identity.clone(),
            },
            fdf_texts: vec![std::fs::read_to_string(env_var("PEACECRAFT_HUD_FDF")).unwrap()],
            skin_text: std::fs::read_to_string(env_var("PEACECRAFT_HUD_SKIN")).unwrap(),
            race_sections: split_csv(&env_var("PEACECRAFT_HUD_SECTIONS")),
            roots: parse_roots(&env_var("PEACECRAFT_HUD_ROOTS")),
            viewport: parse_viewport(&env_var("PEACECRAFT_HUD_VIEWPORT")),
            global_strings: match std::env::var("PEACECRAFT_HUD_STRINGS") {
                Ok(path) => std::fs::read_to_string(path).unwrap(),
                Err(_) => String::new(),
            },
            decorate_suffixes: split_csv(&env_var("PEACECRAFT_HUD_SUFFIXES")),
            images: read_blob_map(&env_var("PEACECRAFT_HUD_IMAGES")),
            fonts: match std::env::var("PEACECRAFT_HUD_FONTS") {
                Ok(path) => read_blob_map(&path),
                Err(_) => Vec::new(),
            },
            budgets: parse_budgets(&env_var("PEACECRAFT_HUD_BUDGETS")),
            markup: crate::hud_assets::HudMarkup::Plain,
            source_member: env_var("PEACECRAFT_HUD_MEMBER"),
            receipt_path: std::env::var("PEACECRAFT_HUD_RECEIPT").unwrap_or_default(),
            binding_revision: env_var("PEACECRAFT_HUD_REVISION").parse().unwrap(),
        };
        let mut source_fdf = crate::fdf::Fdf::default();
        for text in &input.fdf_texts {
            source_fdf.parse_into(text).unwrap();
        }
        let root_textures: usize = input
            .roots
            .iter()
            .map(|root| source_fdf.frame(&root.frame).unwrap().textures.len())
            .sum();
        let supplied_images = input.images.len();
        let mut app = test_app();
        submit(&mut app, input);
        app.update();
        let (error, source, generation) = {
            let mount = app.world().resource::<HudMount>();
            (mount.error.clone(), mount.source.clone(), mount.generation)
        };
        assert!(error.is_none(), "{error:?}");
        assert_eq!(source.as_deref(), Some(identity.as_str()));
        assert!(generation >= 1);
        let mut art = app.world_mut().query::<&HudArt>();
        let mounted_layers = art.iter(app.world()).count();
        assert!(mounted_layers > 0);
        assert!(mounted_layers >= root_textures);
        println!(
            "HUD original root_textures={root_textures} mounted_layers={mounted_layers} supplied_images={supplied_images}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn env_var(name: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn split_csv(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn parse_roots(value: &str) -> Vec<crate::hud_assets::RootRole> {
        split_csv(value)
            .into_iter()
            .map(|item| {
                let (role, frame) = item.split_once('=').unwrap_or_else(|| {
                    panic!("root {item:?} is not role=frame");
                });
                crate::hud_assets::RootRole {
                    role: role.to_string(),
                    frame: frame.to_string(),
                }
            })
            .collect()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn parse_viewport(value: &str) -> (f32, f32) {
        let mut numbers = value.split(',').map(str::trim);
        let width: f32 = numbers.next().unwrap().parse().unwrap();
        let height: f32 = numbers.next().unwrap().parse().unwrap();
        assert!(numbers.next().is_none());
        assert!(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0);
        (width, height)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_blob_map(path: &str) -> Vec<crate::hud_assets::NamedBytes> {
        let text = std::fs::read_to_string(path).unwrap();
        let mut rows = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, file) = line.split_once('\t').unwrap_or_else(|| {
                panic!("{path} line {line:?} is not key<TAB>file");
            });
            rows.push(crate::hud_assets::NamedBytes {
                path: key.to_string(),
                bytes: std::fs::read(file).unwrap(),
            });
        }
        rows
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn parse_budgets(value: &str) -> crate::hud_assets::HudBudgets {
        let numbers: Vec<usize> = value
            .split(',')
            .map(|item| item.trim().parse().unwrap())
            .collect();
        assert_eq!(numbers.len(), 8, "budgets are eight caller limits");
        crate::hud_assets::HudBudgets {
            max_input_bytes: numbers[0],
            max_image_input_bytes: numbers[1],
            max_font_bytes: numbers[2],
            max_font_aggregate_bytes: numbers[3],
            max_image_decoded_bytes: numbers[4],
            max_decoded_bytes: numbers[5],
            max_retained_bytes: numbers[6],
            max_transient_bytes: numbers[7],
        }
    }

    #[test]
    fn a_decode_failure_after_a_valid_header_retains_no_new_assets() {
        let mut app = test_app();
        submit(&mut app, sample_hud(&["Human", "Default"]));
        app.update();
        let before = app.world().resource::<Assets<Image>>().len();
        let root = app.world().resource::<HudMount>().root.unwrap();
        let mut art = app.world_mut().query::<(Entity, &HudArt)>();
        let kept = art
            .iter(app.world())
            .find(|(_, layer)| layer.path == "art/human-tile.blp")
            .unwrap()
            .0;
        let kept_image = image_handle(&app, kept);
        let mut late = sample_hud(&["Human", "Default"]);
        late.images[0].bytes = {
            let mut header = vec![0u8; 18];
            header[2] = 2;
            header[12] = 1;
            header[14] = 1;
            header[16] = 32;
            header[17] = 8;
            header
        };
        submit(&mut app, late);
        app.update();
        let mount = app.world().resource::<HudMount>();
        assert!(mount.error.is_some(), "short pixel payload must refuse");
        assert_eq!(mount.root, Some(root));
        assert_eq!(mount.generation, 1);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), before);
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&kept_image)
                .is_some()
        );
    }

    #[test]
    fn a_malformed_font_refuses_before_the_asset_is_inserted() {
        let mut app = test_app();
        submit(&mut app, sample_hud(&["Human", "Default"]));
        app.update();
        let before = app.world().resource::<Assets<Font>>().len();
        let root = app.world().resource::<HudMount>().root.unwrap();
        let mut faces = app.world_mut().query::<(Entity, &HudTextFace)>();
        let face = faces
            .iter(app.world())
            .find(|(_, face)| face.path == "fonts/body.ttf")
            .unwrap()
            .0;
        let kept = app.world().get::<TextFont>(face).unwrap().font.clone();
        let bevy::text::FontSource::Handle(kept) = kept else {
            panic!("mounted text uses a font handle");
        };
        let mut late = sample_hud(&["Human", "Default"]);
        late.fonts[0].bytes = b"not-a-font".to_vec();
        submit(&mut app, late);
        app.update();
        let mount = app.world().resource::<HudMount>();
        assert!(mount.error.is_some(), "unreadable font bytes must refuse");
        assert_eq!(mount.root, Some(root));
        assert_eq!(mount.generation, 1);
        assert_eq!(app.world().resource::<Assets<Font>>().len(), before);
        assert!(app.world().resource::<Assets<Font>>().get(&kept).is_some());
    }

    fn admitted_requests(revision: u64, member: &str) -> peacecraft3::map_requests::MapRequests {
        use pcx_assets::{AssetPath, LayerKind, Provenance, SourceInfo};
        use pcx_play::map_admission::MapAdmissionBudget;
        use pcx_play::map_scripts_import::MapScriptBudget;
        use pcx_play::scripts::AssetIdentity;
        use peacecraft3::map_binding::{AssetReceipt, BindingReceipts};
        use peacecraft3::map_requests::{BoundCatalog, BoundScript, MapBindings, MapOutcome};
        use std::sync::Arc;

        struct UnusedCatalog;
        impl pcx_sim::world::ObjectCatalog for UnusedCatalog {
            fn resolve(
                &self,
                _: pcx_objects::Fourcc,
            ) -> Result<pcx_sim::world::Definition, pcx_sim::world::ResolutionProblem> {
                Err(pcx_sim::world::ResolutionProblem::InvalidRawcode)
            }
        }

        let path = AssetPath::new(member).unwrap();
        let identity = AssetIdentity {
            edition: "roc".into(),
            version: "1.29.2".into(),
            source: "fixture-archive".into(),
        };
        let receipts = BindingReceipts {
            map: AssetReceipt {
                path: path.clone(),
                provenance: Provenance {
                    source: SourceInfo {
                        id: "fixture-archive".into(),
                        kind: LayerKind::Mpq,
                        origin: "fixture-archive".into(),
                        version: Some("1.29.2".into()),
                    },
                    archive_priority: Some(0),
                    locator: member.into(),
                    byte_length: 1,
                },
                hash: 1,
            },
            common: AssetReceipt {
                path: AssetPath::new("scripts/common.j").unwrap(),
                provenance: Provenance {
                    source: SourceInfo {
                        id: "fixture-archive".into(),
                        kind: LayerKind::Mpq,
                        origin: "fixture-archive".into(),
                        version: Some("1.29.2".into()),
                    },
                    archive_priority: Some(0),
                    locator: "scripts/common.j".into(),
                    byte_length: 1,
                },
                hash: 2,
            },
            blizzard: AssetReceipt {
                path: AssetPath::new("scripts/blizzard.j").unwrap(),
                provenance: Provenance {
                    source: SourceInfo {
                        id: "fixture-archive".into(),
                        kind: LayerKind::Mpq,
                        origin: "fixture-archive".into(),
                        version: Some("1.29.2".into()),
                    },
                    archive_priority: Some(0),
                    locator: "scripts/blizzard.j".into(),
                    byte_length: 1,
                },
                hash: 3,
            },
            catalog: Vec::new(),
            missing_catalog: Vec::new(),
            units: None,
            items: None,
        };
        peacecraft3::map_requests::MapRequests {
            bindings: Some(MapBindings {
                map_identity: identity.clone(),
                map_label: "fixture".into(),
                receipts: std::sync::Arc::new(receipts.clone()),
                resolver: None,
                catalog_map_bytes: Arc::<[u8]>::from([]),
                revision,
                common: BoundScript {
                    identity: identity.clone(),
                    label: "common".into(),
                    bytes: Vec::new(),
                },
                blizzard: BoundScript {
                    identity: identity.clone(),
                    label: "blizzard".into(),
                    bytes: Vec::new(),
                },
                catalog_identity: identity,
                catalog: BoundCatalog(Arc::new(UnusedCatalog)),
                required_fields: Vec::new(),
                budget: MapAdmissionBudget {
                    scripts: MapScriptBudget {
                        max_script_bytes: 1,
                        max_wts_bytes: 1,
                        max_archive_bytes: 1,
                        max_archive_table_bytes: 1,
                        max_raw_packed_bytes: 1,
                        max_decoded_bytes: 1,
                        max_prelude_bytes: 1,
                        max_aggregate_input_bytes: 1,
                    },
                    terrain_bytes: 1,
                    info_bytes: 1,
                    pathing_bytes: 1,
                    units_bytes: 1,
                    aggregate_input_bytes: 1,
                    required_fields: 1,
                },
                max_map_file_bytes: 1,
            }),
            admission: peacecraft3::map_admission::MapAdmission::default(),
            published_receipts: Some(Arc::new(receipts)),
            last_outcome: Some(MapOutcome {
                label: "fixture".into(),
                identity: None,
                binding_revision: Some(revision),
                result: Ok(()),
            }),
        }
    }
    #[test]
    fn admitted_receipt_mounts_and_a_race_change_replaces_it() {
        let mut app = test_app();
        let sample = sample_hud(&["Human", "Default"]);
        let (requests, witness, reads) = admitted_delivery(7, "maps/fixture.w3m", &sample);
        let policy = policy_for(&sample, witness);
        app.insert_resource(requests);
        app.insert_resource(policy.clone());
        app.update();
        let mounted_reads = reads.load(std::sync::atomic::Ordering::Relaxed);
        assert!(mounted_reads > 0);
        app.update();
        let source = {
            let mount = app.world().resource::<HudMount>();
            assert!(mount.error.is_none(), "{:?}", mount.error);
            assert_eq!(mount.generation, 1);
            assert_eq!(
                reads.load(std::sync::atomic::Ordering::Relaxed),
                mounted_reads
            );
            assert_eq!(mount.race_sections, ["Human", "Default"]);
            mount.source.clone().unwrap()
        };
        assert!(source.contains("edition=roc"));
        assert!(source.contains("map=maps/fixture.w3m"));
        assert!(!source.contains("fixture:"));
        let console = named(&mut app, "Console");
        {
            let attachment = app.world().get::<HudSourceAttachment>(console).unwrap();
            assert_eq!(attachment.source_member, "ui/framedef/ui/fixture.fdf");
            assert_eq!(attachment.frame, "Console");
            assert_eq!(attachment.receipt_path, "maps/fixture.w3m");
            assert_eq!(attachment.binding_revision, 7);
            assert_eq!(attachment.source_label, source);
        }
        let mut art = app.world_mut().query::<&HudArt>();
        assert!(
            art.iter(app.world())
                .any(|layer| layer.path == "art/human-tile.blp")
        );

        let mut orc = policy.clone();
        orc.race_sections = vec!["Orc".into(), "Default".into()];
        app.insert_resource(orc);
        app.update();
        {
            let mount = app.world().resource::<HudMount>();
            assert_eq!(mount.generation, 2);
            assert_eq!(mount.race_sections, ["Orc", "Default"]);
        }
        let mut art = app.world_mut().query::<&HudArt>();
        let paths: Vec<_> = art
            .iter(app.world())
            .map(|layer| layer.path.clone())
            .collect();
        assert!(paths.iter().any(|path| path == "art/orc-tile.blp"));
        assert!(paths.iter().all(|path| path != "art/human-tile.blp"));
        let replaced_reads = reads.load(std::sync::atomic::Ordering::Relaxed);
        assert!(replaced_reads > mounted_reads);

        let mut empty = policy;
        empty.race_sections.clear();
        app.insert_resource(empty);
        app.update();
        {
            let mount = app.world().resource::<HudMount>();
            assert_eq!(mount.generation, 2);
            assert!(mount.error.is_none());
        }
        let refusal = app.world().resource::<HudPrepare>().refusal.clone();
        assert_eq!(
            refusal.as_deref(),
            Some("caller race section order is empty")
        );
        assert_eq!(
            reads.load(std::sync::atomic::Ordering::Relaxed),
            replaced_reads
        );
        app.update();
        assert_eq!(app.world().resource::<HudMount>().generation, 2);
        assert_eq!(
            app.world().resource::<HudPrepare>().refusal.as_deref(),
            Some("caller race section order is empty")
        );
        assert_eq!(
            reads.load(std::sync::atomic::Ordering::Relaxed),
            replaced_reads
        );
    }

    #[test]
    fn hud_refuses_an_outcome_without_an_admitted_context() {
        let mut requests = admitted_requests(7, "maps/fixture.w3m");
        assert!(requests.admission.shared_context().is_none());
        let receipts = std::sync::Arc::clone(&requests.bindings.as_ref().unwrap().receipts);
        requests.published_receipts = Some(std::sync::Arc::clone(&receipts));
        let identity = requests.bindings.as_ref().unwrap().map_identity.clone();
        requests.last_outcome.as_mut().unwrap().identity = Some(identity);
        let sample = sample_hud(&["Human", "Default"]);
        let (reader, _) = memory_reader(&sample);
        let policy = policy_for(&sample, HudSourceWitness { receipts, reader });
        assert!(
            admitted_input(&requests, &SelectedMapSources::default(), &policy).is_err(),
            "an Ok outcome alone must not qualify source ownership"
        );
    }

    #[test]
    fn hud_refuses_a_different_receipt_with_the_same_map_path() {
        let sample = sample_hud(&["Human", "Default"]);
        let (mut requests, witness, _) = admitted_delivery(7, "maps/fixture.w3m", &sample);
        let mut different = (**requests.published_receipts.as_ref().unwrap()).clone();
        different.map.hash ^= 1;
        requests.published_receipts = Some(std::sync::Arc::new(different));
        let policy = policy_for(&sample, witness);
        assert!(
            admitted_input(&requests, &SelectedMapSources::default(), &policy).is_err(),
            "a matching filename does not identify the bound content"
        );

        let (requests, witness, _) = admitted_delivery(7, "maps/fixture.w3m", &sample);
        let mut lookalike = (*witness.receipts).clone();
        lookalike.map.hash ^= 1;
        let mut policy = policy_for(&sample, witness);
        policy.witness.as_mut().unwrap().receipts = std::sync::Arc::new(lookalike);
        assert!(
            admitted_input(&requests, &SelectedMapSources::default(), &policy).is_err(),
            "a witness for a different receipt arc is not this admission"
        );
    }

    #[test]
    fn hud_font_aggregate_refuses_before_copying_the_next_member() {
        let mut app = test_app();
        let sample = sample_hud(&["Human", "Default"]);
        let (requests, witness, reads) = admitted_delivery(7, "maps/fixture.w3m", &sample);
        let mut policy = policy_for(&sample, witness);
        policy.budgets.max_font_aggregate_bytes = 0;
        app.insert_resource(requests);
        app.insert_resource(policy);
        app.update();
        assert_eq!(app.world().resource::<HudMount>().generation, 0);
        assert!(app.world().resource::<HudPrepare>().refusal.is_some());
        assert_eq!(
            reads.load(std::sync::atomic::Ordering::Relaxed),
            2 + sample.images.len()
        );
    }

    #[test]
    fn hud_reprepares_a_changed_viewport_in_the_same_race_and_revision() {
        let mut app = test_app();
        let sample = sample_hud(&["Human", "Default"]);
        let (requests, witness, reads) = admitted_delivery(7, "maps/fixture.w3m", &sample);
        app.insert_resource(requests);
        app.insert_resource(policy_for(&sample, witness));
        app.update();
        assert_eq!(app.world().resource::<HudMount>().generation, 1);
        let before = app.world().resource::<HudMount>().viewport.unwrap();
        let mounted_reads = reads.load(std::sync::atomic::Ordering::Relaxed);
        app.update();
        assert_eq!(app.world().resource::<HudMount>().generation, 1);
        assert_eq!(
            reads.load(std::sync::atomic::Ordering::Relaxed),
            mounted_reads
        );
        app.world_mut().resource_mut::<HudSourcePolicy>().viewport.0 *= 2.0;
        app.update();
        let mount = app.world().resource::<HudMount>();
        assert_eq!(mount.generation, 2);
        assert_eq!(mount.viewport.unwrap().0, before.0 * 2.0);
        assert!(reads.load(std::sync::atomic::Ordering::Relaxed) > mounted_reads);
    }

    fn policy_for(
        input: &crate::hud_assets::HudInput,
        witness: HudSourceWitness,
    ) -> HudSourcePolicy {
        HudSourcePolicy {
            race_sections: input.race_sections.clone(),
            roots: input.roots.clone(),
            viewport: input.viewport,
            budgets: input.budgets,
            markup: input.markup.clone(),
            source_member: input.source_member.clone(),
            fdf_members: vec![input.source_member.clone()],
            skin_member: "ui/war3skins.txt".into(),
            global_member: String::new(),
            decorate_suffixes: input.decorate_suffixes.clone(),
            image_members: input.images.iter().map(|row| row.path.clone()).collect(),
            font_members: input.fonts.iter().map(|row| row.path.clone()).collect(),
            witness: Some(witness),
        }
    }

    struct MemorySource {
        info: pcx_assets::SourceInfo,
        files: std::collections::BTreeMap<String, Vec<u8>>,
        reads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl pcx_assets::ContentSource for MemorySource {
        fn info(&self) -> &pcx_assets::SourceInfo {
            &self.info
        }
        fn paths(&self) -> Result<Vec<String>, pcx_assets::SourceError> {
            Ok(self.files.keys().cloned().collect())
        }
        fn read(
            &self,
            member: &str,
        ) -> Result<Option<pcx_assets::SourceContent>, pcx_assets::SourceError> {
            self.read_bounded(member, u64::MAX)
        }
        fn read_bounded(
            &self,
            member: &str,
            limit: u64,
        ) -> Result<Option<pcx_assets::SourceContent>, pcx_assets::SourceError> {
            let Some(bytes) = self.files.get(member) else {
                return Ok(None);
            };
            if bytes.len() as u64 > limit {
                return Err(pcx_assets::SourceError::Limit {
                    requested: bytes.len() as u64,
                    limit,
                });
            }
            self.reads
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(Some(pcx_assets::SourceContent {
                bytes: bytes.clone(),
                declared_length: bytes.len() as u64,
                locator: member.to_string(),
            }))
        }
    }

    fn memory_reader(
        input: &crate::hud_assets::HudInput,
    ) -> (
        std::sync::Arc<pcx_assets::Resolver>,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            input.source_member.clone(),
            input.fdf_texts[0].as_bytes().to_vec(),
        );
        files.insert(
            "ui/war3skins.txt".into(),
            input.skin_text.as_bytes().to_vec(),
        );
        for image in &input.images {
            files.insert(image.path.clone(), image.bytes.clone());
        }
        for font in &input.fonts {
            files.insert(font.path.clone(), font.bytes.clone());
        }
        let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let resolver = pcx_assets::Resolver::new(
            pcx_assets::GameMode {
                build: pcx_assets::Build::V1_29b,
                addon: pcx_assets::Addon::Tft,
                assets: pcx_assets::AssetMode::Vanilla,
                behaviour: pcx_assets::Behaviour::Vanilla,
            },
            None,
            vec![pcx_assets::ArchiveLayer {
                priority: 1,
                source: Box::new(MemorySource {
                    info: pcx_assets::SourceInfo {
                        id: "hud-fixture".into(),
                        kind: pcx_assets::LayerKind::Mpq,
                        origin: "hud-fixture".into(),
                        version: Some("1.29.2".into()),
                    },
                    files,
                    reads: std::sync::Arc::clone(&reads),
                }),
            }],
        )
        .expect("memory resolver");
        (std::sync::Arc::new(resolver), reads)
    }

    #[test]
    fn mounted_witness_requires_the_exact_context_and_revision() {
        let sample = sample_hud(&["Human", "Default"]);
        let (first, _, _) = admitted_delivery(7, "same-map.w3m", &sample);
        let (second, _, _) = admitted_delivery(7, "same-map.w3m", &sample);
        let first = first.admission.shared_context().unwrap();
        let second = second.admission.shared_context().unwrap();
        let mut prepared = HudPrepare::default();
        assert!(!prepared.mounted_for(&first, 7));
        prepared.mounted = Some(AdmissionStamp {
            context: Arc::downgrade(&first),
            revision: 7,
        });
        assert!(prepared.mounted_for(&first, 7));
        assert!(!prepared.mounted_for(&first, 8));
        assert!(!prepared.mounted_for(&second, 7));
    }

    struct IdleCatalog;

    impl pcx_sim::world::ObjectCatalog for IdleCatalog {
        fn resolve(
            &self,
            _: pcx_objects::Fourcc,
        ) -> Result<pcx_sim::world::Definition, pcx_sim::world::ResolutionProblem> {
            Err(pcx_sim::world::ResolutionProblem::InvalidRawcode)
        }
    }

    fn admitted_delivery(
        revision: u64,
        member: &str,
        sample: &crate::hud_assets::HudInput,
    ) -> (
        peacecraft3::map_requests::MapRequests,
        HudSourceWitness,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        use pcx_assets::{AssetPath, LayerKind, Provenance, SourceInfo};
        use pcx_play::map_admission::{IdentifiedScript, MapAdmissionBudget, MapAdmissionInputs};
        use pcx_play::map_scripts_import::MapScriptBudget;
        use pcx_play::scripts::{AssetIdentity, ScriptAsset};
        use peacecraft3::map_binding::{AssetReceipt, BindingReceipts};
        use peacecraft3::map_requests::{BoundCatalog, BoundScript, MapBindings, MapOutcome};
        use std::sync::Arc;

        let (reader, reads) = memory_reader(sample);
        let map_identity = AssetIdentity {
            edition: "roc".into(),
            version: "1.29.2".into(),
            source: "fixture-archive".into(),
        };
        let common_identity = AssetIdentity {
            edition: "roc".into(),
            version: "1.29.2".into(),
            source: "supplied-common".into(),
        };
        let blizzard_identity = AssetIdentity {
            edition: "roc".into(),
            version: "1.29.2".into(),
            source: "supplied-blizzard".into(),
        };
        let catalog_identity = AssetIdentity {
            edition: "roc".into(),
            version: "1.29.2".into(),
            source: "supplied-catalog".into(),
        };
        let common = b"native FixtureHeld takes nothing returns integer\n".to_vec();
        let blizzard = b"// supplied BJ\n".to_vec();
        let files = [
            ("war3map.w3e".to_string(), terrain_bytes(2, 1)),
            ("war3map.wpm".to_string(), pathing_bytes(4, 2)),
            ("war3map.w3i".to_string(), info_bytes()),
            ("war3mapUnits.doo".to_string(), units_bytes()),
            ("war3map.j".to_string(), script_bytes()),
        ];
        let owned: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
            .collect();
        let raw = synthetic_w3m("fixture", &owned);
        let map = pcx_map::Map::parse(&raw).expect("synthetic admitted map");
        let header = pcx_mpq::Header::parse(&mut std::io::Cursor::new(map.archive()))
            .expect("synthetic archive header");
        let member_len = |name: &str| {
            files
                .iter()
                .find(|(item, _)| item == name)
                .map(|(_, bytes)| bytes.len())
                .unwrap_or(0)
        };
        let largest = files.iter().map(|(_, bytes)| bytes.len()).max().unwrap();
        let preludes = common.len() + blizzard.len();
        let script = member_len("war3map.j");
        let wts = member_len("war3map.wts");
        let file_total = files.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
        let budget = MapAdmissionBudget {
            scripts: MapScriptBudget {
                max_script_bytes: script,
                max_wts_bytes: wts,
                max_archive_bytes: map.archive().len(),
                max_archive_table_bytes: (header.hash_table_bytes() + header.block_table_bytes())
                    as usize,
                max_raw_packed_bytes: largest,
                max_decoded_bytes: largest,
                max_prelude_bytes: preludes,
                max_aggregate_input_bytes: map.archive().len() + preludes + script + wts,
            },
            terrain_bytes: member_len("war3map.w3e"),
            pathing_bytes: member_len("war3map.wpm"),
            info_bytes: member_len("war3map.w3i"),
            units_bytes: member_len("war3mapUnits.doo"),
            aggregate_input_bytes: map.archive().len() + preludes + file_total,
            required_fields: 0,
        };
        let mut admission = peacecraft3::map_admission::MapAdmission::default();
        admission
            .replace(
                MapAdmissionInputs {
                    map: &map,
                    map_label: "fixture",
                    map_identity: &map_identity,
                    common: IdentifiedScript {
                        identity: &common_identity,
                        asset: ScriptAsset {
                            label: "chosen-common.fixture",
                            bytes: &common,
                        },
                    },
                    blizzard: IdentifiedScript {
                        identity: &blizzard_identity,
                        asset: ScriptAsset {
                            label: "chosen-bj.fixture",
                            bytes: &blizzard,
                        },
                    },
                    catalog_identity: &catalog_identity,
                    catalog: &IdleCatalog,
                    required_fields: {
                        let fields: &[[u8; 4]] = &[];
                        fields
                    },
                },
                budget,
            )
            .expect("parsed map context");
        drop(map);
        let context = admission.shared_context().expect("admitted context");
        assert_eq!(context.sources.map, map_identity);
        let path = AssetPath::new(member).unwrap();
        let provenance = |locator: &str, byte_length: u64| Provenance {
            source: SourceInfo {
                id: "fixture-archive".into(),
                kind: LayerKind::Mpq,
                origin: "fixture-archive".into(),
                version: Some("1.29.2".into()),
            },
            archive_priority: Some(0),
            locator: locator.into(),
            byte_length,
        };
        let receipts = Arc::new(BindingReceipts {
            map: AssetReceipt {
                path: path.clone(),
                provenance: provenance(member, raw.len() as u64),
                hash: 1,
            },
            common: AssetReceipt {
                path: AssetPath::new("scripts/common.j").unwrap(),
                provenance: provenance("scripts/common.j", common.len() as u64),
                hash: 2,
            },
            blizzard: AssetReceipt {
                path: AssetPath::new("scripts/blizzard.j").unwrap(),
                provenance: provenance("scripts/blizzard.j", blizzard.len() as u64),
                hash: 3,
            },
            catalog: Vec::new(),
            missing_catalog: Vec::new(),
            units: None,
            items: None,
        });
        let map_len = raw.len();
        let requests = peacecraft3::map_requests::MapRequests {
            bindings: Some(MapBindings {
                map_identity: map_identity.clone(),
                map_label: "fixture".into(),
                receipts: Arc::clone(&receipts),
                resolver: None,
                catalog_map_bytes: Arc::<[u8]>::from(raw),
                revision,
                common: BoundScript {
                    identity: common_identity,
                    label: "common".into(),
                    bytes: common,
                },
                blizzard: BoundScript {
                    identity: blizzard_identity,
                    label: "blizzard".into(),
                    bytes: blizzard,
                },
                catalog_identity,
                catalog: BoundCatalog(Arc::new(IdleCatalog)),
                required_fields: Vec::new(),
                budget,
                max_map_file_bytes: map_len,
            }),
            admission,
            published_receipts: Some(Arc::clone(&receipts)),
            last_outcome: Some(MapOutcome {
                label: "fixture".into(),
                identity: Some(map_identity),
                binding_revision: Some(revision),
                result: Ok(()),
            }),
        };
        (requests, HudSourceWitness { receipts, reader }, reads)
    }

    fn info_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        for word in [18u32, 0, 0] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.extend([0; 4]);
        bytes.extend([0; 32 + 16 + 8 + 4]);
        bytes.push(0);
        bytes.extend(0u32.to_le_bytes());
        bytes.extend([0; 3]);
        bytes.extend(0u32.to_le_bytes());
        bytes.extend([0; 3]);
        bytes.extend([0; 20]);
        bytes
    }

    fn terrain_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
        bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
        bytes.push(0);
        for word in [0, 0, 0, width + 1, height + 1] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.extend([0; 8]);
        bytes.resize(
            bytes.len() + ((width + 1) * (height + 1)) as usize * pcx_map::CORNER_RECORD,
            0,
        );
        bytes
    }

    fn pathing_bytes(width: u32, height: u32) -> Vec<u8> {
        let cells = [0x12u8, 0x84];
        let mut bytes = b"MP3W".to_vec();
        for word in [0u32, width, height] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.extend((0..width * height).map(|i| cells[i as usize % cells.len()]));
        bytes
    }

    fn units_bytes() -> Vec<u8> {
        let mut bytes = b"W3do".to_vec();
        for word in [8u32, 9, 0] {
            bytes.extend(word.to_le_bytes());
        }
        bytes
    }

    fn script_bytes() -> Vec<u8> {
        b"globals\ninteger AuthoredMarker = 1\ninteger RequiresHost = FixtureHeld()\nendglobals\nfunction config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n".to_vec()
    }

    fn encrypt_mpq_block(data: &mut [u8], key: u32) {
        let table = pcx_mpq::crypt::table();
        let mut seed1 = key;
        let mut seed2 = 0xEEEE_EEEEu32;
        for chunk in data.as_chunks_mut::<4>().0 {
            seed2 = seed2.wrapping_add(table[0x400 + (seed1 & 0xFF) as usize]);
            let plaintext = u32::from_le_bytes(*chunk);
            let ciphertext = plaintext ^ seed1.wrapping_add(seed2);
            chunk.copy_from_slice(&ciphertext.to_le_bytes());
            seed1 = ((!seed1) << 0x15).wrapping_add(0x1111_1111) | (seed1 >> 0x0B);
            seed2 = plaintext
                .wrapping_add(seed2)
                .wrapping_add(seed2 << 5)
                .wrapping_add(3);
        }
    }

    fn synthetic_w3m(map_name: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
        const HASH_ENTRIES: usize = 16;
        let mut payloads = Vec::new();
        let mut block_entries = Vec::new();
        let payload_base_offset = 0x40usize;
        for (_name, data) in files {
            let file_pos = payload_base_offset + payloads.len();
            let file_size = data.len() as u32;
            let flags: u32 = 0x8000_0000 | 0x0100_0000;
            block_entries.push((file_pos as u32, file_size, file_size, flags));
            payloads.extend_from_slice(data);
            while payloads.len() % 4 != 0 {
                payloads.push(0);
            }
        }
        let hash_table_pos = (payload_base_offset + payloads.len()) as u32;
        let mut hash_entries = vec![
            pcx_mpq::HashEntry {
                name1: 0xFFFF_FFFF,
                name2: 0xFFFF_FFFF,
                locale: 0,
                platform: 0,
                block_index: 0xFFFF_FFFF,
            };
            HASH_ENTRIES
        ];
        for (i, (name, _)) in files.iter().enumerate() {
            let (name1, name2) = pcx_mpq::hash::hash_string_both(name);
            let mut slot = pcx_mpq::hash::start_slot(name, HASH_ENTRIES);
            while hash_entries[slot].block_index != 0xFFFF_FFFF {
                slot = (slot + 1) % HASH_ENTRIES;
            }
            hash_entries[slot].name1 = name1;
            hash_entries[slot].name2 = name2;
            hash_entries[slot].block_index = i as u32;
        }
        let mut hash_table_raw = Vec::with_capacity(HASH_ENTRIES * 16);
        for entry in &hash_entries {
            hash_table_raw.extend_from_slice(&entry.name1.to_le_bytes());
            hash_table_raw.extend_from_slice(&entry.name2.to_le_bytes());
            hash_table_raw.extend_from_slice(&entry.locale.to_le_bytes());
            hash_table_raw.extend_from_slice(&entry.platform.to_le_bytes());
            hash_table_raw.extend_from_slice(&entry.block_index.to_le_bytes());
        }
        encrypt_mpq_block(&mut hash_table_raw, pcx_mpq::hash::hash_table_key());
        let block_table_pos = hash_table_pos + (HASH_ENTRIES as u32 * 16);
        let mut block_table_raw = Vec::with_capacity(block_entries.len() * 16);
        for (pos, comp, size, flags) in &block_entries {
            block_table_raw.extend_from_slice(&pos.to_le_bytes());
            block_table_raw.extend_from_slice(&comp.to_le_bytes());
            block_table_raw.extend_from_slice(&size.to_le_bytes());
            block_table_raw.extend_from_slice(&(*flags).to_le_bytes());
        }
        encrypt_mpq_block(&mut block_table_raw, pcx_mpq::hash::block_table_key());
        let archive_size = block_table_pos + (block_entries.len() as u32 * 16);
        let mut mpq = Vec::new();
        mpq.extend_from_slice(b"MPQ\x1a");
        mpq.extend_from_slice(&0x20u32.to_le_bytes());
        mpq.extend_from_slice(&archive_size.to_le_bytes());
        mpq.extend_from_slice(&0u16.to_le_bytes());
        mpq.push(3u8);
        mpq.push(0u8);
        mpq.extend_from_slice(&hash_table_pos.to_le_bytes());
        mpq.extend_from_slice(&block_table_pos.to_le_bytes());
        mpq.extend_from_slice(&(HASH_ENTRIES as u32).to_le_bytes());
        mpq.extend_from_slice(&(files.len() as u32).to_le_bytes());
        mpq.extend_from_slice(&0u32.to_le_bytes());
        mpq.extend_from_slice(&0u32.to_le_bytes());
        while mpq.len() < payload_base_offset {
            mpq.push(0);
        }
        mpq.extend_from_slice(&payloads);
        mpq.extend_from_slice(&hash_table_raw);
        mpq.extend_from_slice(&block_table_raw);
        assert_eq!(mpq.len(), archive_size as usize);
        let mut map = Vec::new();
        map.extend_from_slice(b"HM3W");
        map.extend_from_slice(&0u32.to_le_bytes());
        map.extend_from_slice(map_name.as_bytes());
        map.push(0);
        map.extend_from_slice(&1u32.to_le_bytes());
        map.extend_from_slice(&1u32.to_le_bytes());
        while map.len() < 0x200 {
            map.push(0);
        }
        map.extend_from_slice(&mpq);
        map
    }
}
