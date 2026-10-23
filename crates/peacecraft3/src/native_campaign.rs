//! Native campaign chapter selection from the selected install.
//!
//! Chapters come from that install's `UI\CampaignStrings.txt` (Reign of Chaos)
//! or `UI\CampaignStrings_exp.txt` (The Frozen Throne). The campaign index keeps
//! the authored `CampaignList` order, each campaign's `DefaultOpen` availability
//! and background skin, and its mission rows. Choosing a mission configures its
//! map through the same bounded binding path the browser uses, then requests it;
//! the campaign startup path takes over from there. The source mounts every
//! reviewed client's base priority, with Huffman weights and data namespace
//! selection derived from the pinned executable and each map header.
//!
//! The real `CampaignMenu` screen is bound here (native only): two authored
//! frames from the FDF host the dynamic campaign and mission lists. The source
//! FDF declares no button anchors for those lists, so item geometry comes from
//! the authored `CampaignButtonTemplate` and placement inside the hosted frames
//! is a labelled recreation choice (see `docs/re-notes/menu-entry.md`).
use crate::launch::{Addon, AssetMode, Behaviour, GameMode};
use bevy::{prelude::*, ui::InteractionDisabled, ui_widgets::Button};
use pcx_objects::editor::campaign_index::{CampaignSlot, IndexLimits, parse_campaign_index};
use pcx_objects::editor::{EditorReadLimits, SlkId};
use pcx_play::MapScriptBudget;
use pcx_play::campaign_data_set::{CampaignRecipe, ImageRules};
use pcx_play::install_source::{MAX_AGGREGATE_BYTES, MAX_ASSET_BYTES, MAX_LISTED_PATHS};
use pcx_play::map_admission::MapAdmissionBudget;
use pcx_play::scripts::AssetIdentity;
use peacecraft3::browser_configuration::{Configuration, ConfigurationInbox, Member, Poll};
use peacecraft3::map_binding::{BindingLimits, OverlayLimits};
use peacecraft3::map_requests::{MapRequest, MapRequestSet, MapRequests};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const INDEX_LIMITS: IndexLimits = IndexLimits {
    max_bytes: 1 << 20,
    max_campaign_slots: 32,
    max_missions_per_campaign: 128,
};

/// One mission row (`Header`, `Name`, map filename) as authored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Mission {
    pub header: String,
    pub name: String,
    pub map: String,
}

/// One campaign section of the selected index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Campaign {
    /// The section key (`CampaignList` id), not a display string.
    pub key: String,
    /// The section's `Header` value.
    pub header: String,
    /// The section's `Name` value, the campaign's display title.
    pub title: String,
    /// The authored `DefaultOpen` availability. It is not completion.
    pub default_open: bool,
    /// The authored `Background` skin key, resolved through war3skins.txt.
    pub background: Option<String>,
    pub missions: Vec<Mission>,
}

/// Flat chapter view for the fallback launcher card; derived from the same parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Chapter {
    pub campaign: String,
    pub header: String,
    pub name: String,
    pub map: String,
}

/// Campaigns of the selected install and edition, refreshed when either changes.
#[derive(Resource, Default)]
pub(crate) struct Chapters {
    key: Option<(PathBuf, GameMode)>,
    recipe: Option<Arc<CampaignRecipe>>,
    /// Structured campaigns for the real `CampaignMenu` screen.
    pub campaigns: Vec<Campaign>,
    /// Flat chapter rows for the fallback launcher card.
    pub entries: Vec<Chapter>,
    pub error: Option<String>,
    /// Configuration request in flight and the binding revision it carries.
    pending: Option<(u32, u64)>,
    revision: u64,
}

/// Selection state for the real `CampaignMenu` screen.
#[derive(Resource, Default)]
pub(crate) struct CampaignMenuState {
    /// The selected campaign index, or `None` before any selection.
    pub selected_campaign: Option<usize>,
    /// Source identity the mounted panel was last built for: install, edition,
    /// index revision and selected campaign.
    built_for: Option<(PathBuf, GameMode, u64, Option<usize>)>,
}

#[derive(Component)]
pub(crate) struct CampaignPanel;

#[derive(Component)]
pub(crate) struct CampaignButton(pub usize);

#[derive(Component)]
pub(crate) struct MissionButton {
    pub campaign: usize,
    pub mission: usize,
}

/// The fallback launcher card's flat list container.
#[derive(Component)]
pub(crate) struct ChapterList;

#[derive(Component)]
pub(crate) struct ChapterButton(pub usize);

pub(crate) fn install(app: &mut App) {
    let inbox = ConfigurationInbox::default();
    peacecraft3::browser_configuration::install(app, inbox);
    app.init_resource::<Chapters>()
        .init_resource::<CampaignMenuState>()
        .add_systems(
            Update,
            (
                refresh,
                populate,
                launch,
                request,
                render_campaign_panel,
                select_campaign,
                launch_mission,
            )
                .chain()
                .run_if(crate::button_actions::input_allowed)
                .before(MapRequestSet::Deliver)
                .before(crate::button_actions::clear),
        );
}

fn index_member(mode: &GameMode) -> &'static str {
    match mode.addon {
        Addon::Roc => "UI\\CampaignStrings.txt",
        Addon::Tft => "UI\\CampaignStrings_exp.txt",
    }
}

/// The asset-layer mode for a vanilla 1.29 launcher mode; other modes refuse.
fn asset_mode(mode: GameMode) -> Result<pcx_assets::GameMode, String> {
    if mode.build != pcx_mpq::Version::V1_29
        || mode.assets != AssetMode::Vanilla
        || mode.behaviour != Behaviour::Vanilla
    {
        return Err(format!(
            "native campaign play supports vanilla 1.29 only, not {}",
            mode.id()
        ));
    }
    Ok(pcx_assets::GameMode {
        build: pcx_assets::Build::V1_29b,
        addon: match mode.addon {
            Addon::Roc => pcx_assets::Addon::NoAddon,
            Addon::Tft => pcx_assets::Addon::Tft,
        },
        assets: pcx_assets::AssetMode::Vanilla,
        behaviour: pcx_assets::Behaviour::Vanilla,
    })
}

/// Parse structured campaigns (and their flat chapter view) from validated text.
fn parse_campaigns(member: &str, text: &str) -> Result<(Vec<Campaign>, Vec<Chapter>), String> {
    let index =
        parse_campaign_index(member, text.as_bytes(), INDEX_LIMITS).map_err(|e| e.to_string())?;
    let mut campaigns = Vec::new();
    let mut chapters = Vec::new();
    for slot in index.slots {
        let CampaignSlot::Entry(entry) = slot else {
            continue;
        };
        let label = entry
            .name
            .as_ref()
            .or(entry.header.as_ref())
            .cloned()
            .unwrap_or_else(|| entry.id.clone());
        let title = entry.name.clone().unwrap_or_else(|| entry.id.clone());
        let default_open = entry.default_open == Some(true);
        let background = entry.background.clone();
        let missions: Vec<Mission> = entry
            .missions
            .iter()
            .map(|m| Mission {
                header: m.header.clone(),
                name: m.name.clone(),
                map: m.map_file.clone(),
            })
            .collect();
        for mission in &missions {
            chapters.push(Chapter {
                campaign: label.clone(),
                header: mission.header.clone(),
                name: mission.name.clone(),
                map: mission.map.clone(),
            });
        }
        campaigns.push(Campaign {
            key: entry.id.clone(),
            header: entry.header.clone().unwrap_or_default(),
            title,
            default_open,
            background,
            missions,
        });
    }
    Ok((campaigns, chapters))
}

fn read_campaigns(dir: &Path, mode: &GameMode) -> Result<Chapters, String> {
    let recipe = campaign_recipe(dir, *mode)?;
    let reader = recipe.base_resolver().map_err(|error| error.to_string())?;
    let member = index_member(mode);
    let content = reader
        .resolve_bounded(member, INDEX_LIMITS.max_bytes as u64)
        .map_err(|e| e.to_string())?;
    let text = std::str::from_utf8(&content.bytes).map_err(|e| e.to_string())?;
    let (campaigns, chapters) = parse_campaigns(member, text)?;
    Ok(Chapters {
        recipe: Some(recipe),
        campaigns,
        entries: chapters,
        ..default()
    })
}

fn refresh(launcher: Res<crate::launch_gui::Launcher>, mut chapters: ResMut<Chapters>) {
    let Some(dir) = crate::launch_gui::selected_install(&launcher) else {
        chapters.key = None;
        chapters.recipe = None;
        chapters.campaigns.clear();
        chapters.entries.clear();
        chapters.error = None;
        return;
    };
    let key = (dir, launcher.mode());
    if chapters.key.as_ref() == Some(&key) {
        return;
    }
    match read_campaigns(&key.0, &key.1) {
        Ok(index) => {
            chapters.recipe = index.recipe;
            chapters.campaigns = index.campaigns;
            chapters.entries = index.entries;
            chapters.error = None;
        }
        Err(error) => {
            chapters.recipe = None;
            chapters.campaigns.clear();
            chapters.entries.clear();
            chapters.error = Some(error);
        }
    }
    chapters.key = Some(key);
}

/// Fill the fallback launcher card's flat chapter list once per chapter set.
fn populate(
    mut commands: Commands,
    chapters: Res<Chapters>,
    lists: Query<(Entity, Option<&Children>), With<ChapterList>>,
) {
    for (list, children) in &lists {
        if !chapters.is_changed() && children.is_some_and(|c| !c.is_empty()) {
            continue;
        }
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|parent| {
            if let Some(error) = &chapters.error {
                parent.spawn(crate::launch_gui::tint(
                    error.clone(),
                    12.0,
                    crate::launch_gui::WARN,
                ));
            }
            for (index, chapter) in chapters.entries.iter().enumerate() {
                parent
                    .spawn((
                        Button,
                        ChapterButton(index),
                        crate::launch_gui::LauncherControl,
                        Node {
                            padding: UiRect::axes(Val::Rem(0.75), Val::Rem(0.25)),
                            ..default()
                        },
                    ))
                    .with_children(|button| {
                        button.spawn(crate::launch_gui::tint(
                            format!("{}: {}: {}", chapter.campaign, chapter.header, chapter.name),
                            13.0,
                            crate::launch_gui::TEXT,
                        ));
                    });
            }
        });
    }
}

fn limits() -> BindingLimits {
    let scripts = MapScriptBudget {
        max_script_bytes: MAX_ASSET_BYTES,
        max_wts_bytes: MAX_ASSET_BYTES,
        max_archive_bytes: MAX_ASSET_BYTES,
        max_archive_table_bytes: MAX_ASSET_BYTES,
        max_raw_packed_bytes: MAX_ASSET_BYTES,
        max_decoded_bytes: MAX_ASSET_BYTES,
        max_prelude_bytes: MAX_ASSET_BYTES,
        max_aggregate_input_bytes: MAX_AGGREGATE_BYTES,
    };
    BindingLimits {
        reader: EditorReadLimits {
            max_listed_paths: MAX_LISTED_PATHS,
            max_asset_bytes: MAX_ASSET_BYTES,
            max_aggregate_input_bytes: MAX_AGGREGATE_BYTES,
        },
        overlay: OverlayLimits {
            unit_bytes: MAX_ASSET_BYTES,
            item_bytes: MAX_ASSET_BYTES,
            aggregate_bytes: MAX_AGGREGATE_BYTES,
        },
        admission: MapAdmissionBudget {
            scripts,
            terrain_bytes: MAX_ASSET_BYTES,
            info_bytes: MAX_ASSET_BYTES,
            pathing_bytes: MAX_ASSET_BYTES,
            units_bytes: MAX_ASSET_BYTES,
            aggregate_input_bytes: MAX_AGGREGATE_BYTES,
            required_fields: 0,
        },
        max_map_file_bytes: MAX_ASSET_BYTES,
        max_resolver_input_bytes: MAX_AGGREGATE_BYTES,
    }
}

fn configuration(
    recipe: Arc<CampaignRecipe>,
    mode: GameMode,
    chapter: &Chapter,
    revision: u64,
) -> Result<Configuration, String> {
    let base = recipe.base_resolver().map_err(|error| error.to_string())?;
    let map = base
        .resolve_bounded(&chapter.map, MAX_ASSET_BYTES as u64)
        .map_err(|error| error.to_string())?;
    let reader = recipe
        .open_map(&map.bytes, &chapter.map, map_read_budget())
        .map_err(|error| error.to_string())?;
    let resolver = reader.resolver;
    let selected = resolver
        .resolve_bounded(&chapter.map, MAX_ASSET_BYTES as u64)
        .map_err(|error| error.to_string())?;
    if selected.bytes != map.bytes {
        return Err("selected campaign namespace changes the map used to select its data".into());
    }
    let edition = format!("{:?} campaign chapter", mode.addon);
    let member = |path: &str, label: &str| -> Result<Member, String> {
        Ok(Member {
            path: path.into(),
            label: label.into(),
            identity: AssetIdentity {
                edition: edition.clone(),
                version: "selected install".into(),
                source: path.into(),
            },
            source: resolver
                .source_for(path)
                .map_err(|error| error.to_string())?
                .clone(),
        })
    };
    let map = member(&chapter.map, &chapter.name)?;
    let common = member("Scripts\\common.j", "Scripts\\common.j")?;
    let blizzard = member("Scripts\\Blizzard.j", "Scripts\\Blizzard.j")?;
    let mut catalog_sources = Vec::new();
    for path in resolver.paths() {
        let source = resolver
            .source_for(path.as_str())
            .map_err(|error| error.to_string())?;
        if !catalog_sources.contains(source) {
            catalog_sources.push(source.clone());
        }
    }
    let catalog_identity = AssetIdentity {
        edition,
        version: recipe.rules().origin.version.clone(),
        source: format!("{}::{}", recipe.dir().display(), reader.selection.prefix),
    };
    Ok(Configuration {
        resolver,
        campaign: Some(recipe),
        map,
        common,
        blizzard,
        catalog_identity,
        catalog_label: "selected install object data".into(),
        catalog_sources,
        required_catalog_tables: SlkId::ALL.to_vec(),
        required_fields: Vec::new(),
        revision,
        limits: limits(),
    })
}

fn campaign_recipe(dir: &Path, mode: GameMode) -> Result<Arc<CampaignRecipe>, String> {
    crate::menu_lighting::with_client_image(Some(dir), |source, image| {
        let identity = pcx_play::campaign_inputs::client_identity(image, source);
        let rules = ImageRules::read(image, identity).map_err(|error| error.to_string())?;
        let global = match mode.addon {
            Addon::Roc => "VERSION_REIGN_OF_CHAOS",
            Addon::Tft => "VERSION_FROZEN_THRONE",
        };
        CampaignRecipe::pin(dir, asset_mode(mode)?, rules, image, None, global)
            .map(Arc::new)
            .map_err(|error| error.to_string())
    })
}

fn map_read_budget() -> pcx_mpq::checked::ReadBudget {
    let limits = limits();
    pcx_mpq::checked::ReadBudget {
        max_archive_bytes: limits.max_map_file_bytes,
        max_archive_table_bytes: limits.admission.scripts.max_archive_table_bytes,
        max_raw_packed_bytes: limits.admission.scripts.max_raw_packed_bytes,
        max_decoded_bytes: limits.admission.info_bytes,
    }
}

/// Launch one selected mission through the shared configuration/map-request path.
fn enqueue_chapter(
    launcher: &crate::launch_gui::Launcher,
    inbox: &ConfigurationInbox,
    chapters: &mut Chapters,
    chapter: &Chapter,
) {
    let Some(dir) = crate::launch_gui::selected_install(launcher) else {
        chapters.error = Some("no selected install".into());
        return;
    };
    if chapters.key.as_ref() != Some(&(dir, launcher.mode())) {
        chapters.error =
            Some("campaign index does not match the selected install and edition".into());
        return;
    }
    let Some(recipe) = chapters.recipe.clone() else {
        chapters.error = Some("campaign index has no admitted source recipe".into());
        return;
    };
    chapters.revision += 1;
    let revision = chapters.revision;
    match configuration(recipe, launcher.mode(), chapter, revision)
        .and_then(|request| inbox.enqueue(request).map_err(|e| e.to_string()))
    {
        Ok(id) => chapters.pending = Some((id, revision)),
        Err(error) => chapters.error = Some(error),
    }
}

/// The fallback launcher card's flat chapter launch.
fn launch(
    actions: Res<crate::button_actions::ButtonActions>,
    buttons: Query<&ChapterButton>,
    launcher: Res<crate::launch_gui::Launcher>,
    inbox: Res<ConfigurationInbox>,
    mut chapters: ResMut<Chapters>,
) {
    for entity in &actions.entities {
        let Ok(button) = buttons.get(*entity) else {
            continue;
        };
        if chapters.pending.is_some() {
            continue;
        }
        let Some(chapter) = chapters.entries.get(button.0).cloned() else {
            continue;
        };
        enqueue_chapter(&launcher, &inbox, &mut chapters, &chapter);
    }
}

/// Select a campaign on the real `CampaignMenu` screen. Selection never launches.
fn select_campaign(
    actions: Res<crate::button_actions::ButtonActions>,
    buttons: Query<&CampaignButton>,
    mut state: ResMut<CampaignMenuState>,
    chapters: Res<Chapters>,
) {
    for entity in &actions.entities {
        let Ok(button) = buttons.get(*entity) else {
            continue;
        };
        if chapters
            .campaigns
            .get(button.0)
            .is_some_and(|campaign| campaign.default_open)
        {
            state.selected_campaign = Some(button.0);
        }
    }
}

/// Launch the selected mission from the real `CampaignMenu` screen.
fn launch_mission(
    actions: Res<crate::button_actions::ButtonActions>,
    buttons: Query<&MissionButton>,
    launcher: Res<crate::launch_gui::Launcher>,
    inbox: Res<ConfigurationInbox>,
    mut chapters: ResMut<Chapters>,
) {
    for entity in &actions.entities {
        let Ok(mission) = buttons.get(*entity) else {
            continue;
        };
        if chapters.pending.is_some() {
            continue;
        }
        let Some(campaign) = chapters.campaigns.get(mission.campaign) else {
            continue;
        };
        if !campaign.default_open {
            continue;
        }
        let Some(mission) = campaign.missions.get(mission.mission) else {
            continue;
        };
        let chapter = Chapter {
            campaign: campaign.title.clone(),
            header: mission.header.clone(),
            name: mission.name.clone(),
            map: mission.map.clone(),
        };
        enqueue_chapter(&launcher, &inbox, &mut chapters, &chapter);
    }
}

/// Once the configuration is admitted, request its map like the load button does.
fn request(
    inbox: Res<ConfigurationInbox>,
    state: Res<MapRequests>,
    mut chapters: ResMut<Chapters>,
    mut requests: MessageWriter<MapRequest>,
) {
    let Some((id, _)) = chapters.pending else {
        return;
    };
    match inbox.poll(id) {
        Ok(Poll::Pending) => {}
        Ok(Poll::Completed(outcome)) => {
            chapters.pending = None;
            match outcome.result {
                Ok(()) => match state.selected_map_request() {
                    Ok(request) => {
                        requests.write(request);
                    }
                    Err(error) => chapters.error = Some(error.to_string()),
                },
                Err(error) => chapters.error = Some(error.to_string()),
            }
        }
        Err(error) => {
            chapters.pending = None;
            chapters.error = Some(error.to_string());
        }
    }
}

/// A label styled from an authored FDF text frame: source face, size and colour.
fn source_label(
    install: Option<&std::path::Path>,
    fdf: &crate::fdf::Fdf,
    fonts: &mut Assets<Font>,
    frame: &str,
    text: String,
) -> Result<(Text, TextFont, TextColor, ZIndex), String> {
    let style = crate::menu_text::render_style_with_parent(fdf, frame, frame)
        .map_err(|e| format!("campaign label {frame}: {e}"))?;
    let install = install.ok_or_else(|| "no selected install for campaign labels".to_string())?;
    let skin_bytes = crate::mainmenu::art_bytes(Some(install), crate::mainmenu::SKIN_TABLE)
        .ok_or_else(|| "missing menu skin table".to_string())?;
    let skin = std::str::from_utf8(&skin_bytes).map_err(|e| e.to_string())?;
    let binding = crate::mainmenu::menu_font_file(skin, &style.font)
        .map_err(|e| format!("campaign label font {:?}: {e}", style.font))?;
    let served = crate::mainmenu::served_font_name(&binding.file)
        .ok_or_else(|| format!("campaign label font has no served name: {:?}", binding.file))?;
    let bytes = crate::mainmenu::art_bytes_named(Some(install), &binding.file, &served)
        .ok_or_else(|| format!("campaign label font bytes unavailable: {:?}", binding.file))?;
    let face = fonts.add(Font::from_bytes(bytes));
    let size = bevy::text::FontSize::Vh(style.height / crate::mainmenu::FRAME_H * 100.0);
    let [r, g, b, a] = style.colour;
    Ok((
        Text(text),
        TextFont {
            font: bevy::text::FontSource::Handle(face),
            font_size: size,
            ..default()
        },
        TextColor(Color::srgba(r, g, b, a)),
        ZIndex(1),
    ))
}

/// The authored `CampaignButtonTemplate` geometry, in frame units.
fn template_geometry(fdf: &crate::fdf::Fdf) -> Result<(f32, f32), String> {
    let viewport = (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H);
    let rect = fdf
        .rect("CampaignButtonTemplate", viewport)
        .map_err(|e| e.to_string())?;
    Ok((rect.width, rect.height))
}

/// Spawn one campaign/mission button using the shared `GlueButton` renderer.
///
/// The authored `CampaignButtonTemplate` supplies the backdrop, pushed/disabled
/// and mouse-over art and offsets; a source-styled label is added as the button
/// node's child. The node is placed at the absolute frame `rect`.
fn spawn_campaign_button(
    commands: &mut Commands,
    panel: Entity,
    fdf: &crate::fdf::Fdf,
    rect: crate::fdf::Rect,
    label: impl Bundle,
    component: impl Bundle,
    enabled: bool,
) -> Result<(), String> {
    let button =
        crate::glue_menu::button(fdf, "CampaignButtonTemplate").map_err(|e| e.to_string())?;
    let item = crate::glue_menu::Item {
        frame: "CampaignButtonTemplate".to_string(),
        rect,
        kind: crate::glue_menu::Kind::Button(Box::new(button)),
    };
    let wrapper = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        })
        .id();
    commands.entity(wrapper).insert(Pickable::IGNORE);
    commands.entity(panel).add_child(wrapper);
    crate::glue_menu::spawn(commands, wrapper, std::slice::from_ref(&item));
    let frame = "CampaignButtonTemplate".to_string();
    commands.queue(move |world: &mut World| {
        let Some(children) = world.get::<Children>(wrapper) else {
            return;
        };
        let Some(target) = children.iter().find(|child| {
            world
                .get::<Name>(*child)
                .is_some_and(|name| name.as_str() == frame)
        }) else {
            return;
        };
        world.entity_mut(target).insert((
            component,
            crate::menu_screens::CampaignAction,
            bevy::input_focus::tab_navigation::TabIndex::default(),
        ));
        if !enabled {
            world.entity_mut(target).insert(InteractionDisabled);
        }
        // Dynamic rows own their placement; the generic FDF pass resolves the template origin.
        world
            .entity_mut(target)
            .remove::<crate::menu_screens::ScreenPosition>();
        world.entity_mut(target).with_children(|button| {
            button.spawn((label, Pickable::IGNORE));
        });
    });
    Ok(())
}

/// Build the campaign and mission lists into the panel.
///
/// The FDF declares no list anchors, so the campaign and mission columns sit at
/// a labelled recreation offset and stack by the authored
/// `CampaignButtonTemplate` geometry; each item is rendered with the shared
/// `GlueButton` renderer so its art and offsets come from source. See
/// `docs/re-notes/menu-entry.md`.
fn build_campaign_panel(
    commands: &mut Commands,
    panel: Entity,
    fdf: &crate::fdf::Fdf,
    install: Option<&std::path::Path>,
    fonts: &mut Assets<Font>,
    campaigns: &[Campaign],
    selected: Option<usize>,
) -> Result<(), String> {
    let (button_w, button_h) = template_geometry(fdf)?;
    if button_w <= 0.0 || button_h <= 0.0 {
        return Err(format!(
            "CampaignButtonTemplate geometry is not positive: {button_w}x{button_h}"
        ));
    }
    let gap = button_h * 0.25;
    let campaign_left = crate::mainmenu::FRAME_W * 0.04;
    let mission_left = crate::mainmenu::FRAME_W * 0.45;
    let top_start = crate::mainmenu::FRAME_H * 0.30;
    let row = |index: usize, left: f32| crate::fdf::Rect {
        left,
        bottom: crate::mainmenu::FRAME_H
            - top_start
            - (index as f32 + 1.0) * button_h
            - index as f32 * gap,
        width: button_w,
        height: button_h,
    };
    commands.entity(panel).despawn_related::<Children>();
    for (index, campaign) in campaigns.iter().enumerate() {
        let rect = row(index, campaign_left);
        let label = source_label(
            install,
            fdf,
            fonts,
            "MissionNameHeader",
            campaign.title.clone(),
        )?;
        spawn_campaign_button(
            commands,
            panel,
            fdf,
            rect,
            label,
            CampaignButton(index),
            campaign.default_open,
        )?;
    }
    if let Some(sel) = selected
        && let Some(campaign) = campaigns.get(sel)
    {
        for (index, mission) in campaign.missions.iter().enumerate() {
            let rect = row(index, mission_left);
            let label = source_label(
                install,
                fdf,
                fonts,
                "MissionNameHeader",
                format!("{}: {}", mission.header, mission.name),
            )?;
            spawn_campaign_button(
                commands,
                panel,
                fdf,
                rect,
                label,
                MissionButton {
                    campaign: sel,
                    mission: index,
                },
                campaign.default_open,
            )?;
        }
    }
    Ok(())
}

/// Build the real `CampaignMenu` screen's dynamic campaign and mission lists.
fn render_campaign_panel(
    mut commands: Commands,
    launcher: Res<crate::launch_gui::Launcher>,
    mut navigation: ResMut<crate::menu_screens::Navigation>,
    mut chapters: ResMut<Chapters>,
    mut state: ResMut<CampaignMenuState>,
    mut fonts: ResMut<Assets<Font>>,
    surfaces: (
        Query<Entity, With<crate::launch_gui::MenuLayer>>,
        Query<Entity, With<CampaignPanel>>,
    ),
) {
    let (layers, panels) = surfaces;
    let active = navigation.screen == crate::menu_screens::Screen::Campaign;
    if active && let Some(error) = &chapters.error {
        navigation.notice = format!("Campaign: {error}");
    }
    let dir = crate::launch_gui::selected_install(&launcher);
    if !active || dir.is_none() || chapters.recipe.is_none() {
        if state.built_for.take().is_some() || state.selected_campaign.take().is_some() {
            for panel in &panels {
                commands.entity(panel).despawn();
            }
        }
        return;
    }
    let dir = dir.expect("selected install checked above");
    let signature = (
        dir.clone(),
        launcher.mode(),
        chapters.revision,
        state.selected_campaign,
    );
    if state.built_for.as_ref() == Some(&signature) {
        return;
    }
    let (fdf, _) =
        match crate::launch_gui::read_screen_fdf(Some(&dir), crate::menu_screens::Screen::Campaign)
        {
            Ok(parsed) => parsed,
            Err(error) => {
                let error = format!("campaign panel: {error}");
                navigation.notice = error.clone();
                chapters.error = Some(error);
                return;
            }
        };
    let panel = if let Ok(panel) = panels.single() {
        panel
    } else {
        let Ok(layer) = layers.single() else {
            return;
        };
        let panel = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                CampaignPanel,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(layer).add_child(panel);
        panel
    };
    if let Err(error) = build_campaign_panel(
        &mut commands,
        panel,
        &fdf,
        Some(&dir),
        &mut fonts,
        &chapters.campaigns,
        state.selected_campaign,
    ) {
        let error = format!("campaign panel: {error}");
        navigation.notice = error.clone();
        chapters.error = Some(error);
        return;
    }
    state.built_for = Some(signature);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::ui_widgets::Activate;
    use pcx_play::map_startup_host::MapStartupHost;
    use peacecraft3::{
        campaign_startup::{CampaignSeed, CampaignStartup},
        map_script_session::MapScriptSession,
    };

    const FIXTURE: &str = "[Index]\nCampaignList=\"Tutorial\",\"Human\"\n[Tutorial]\nHeader=\"Prologue\"\nName=\"Exodus\"\nDefaultOpen=1\nBackground=\"TutorialBackdrop\"\nMission0=\"Chapter One\",\"Chasing\",\"Maps\\Prologue01.w3m\"\nMission1=\"Chapter Two\",\"Departures\",\"Maps\\Prologue02.w3m\"\n[Human]\nHeader=\"Human Campaign\"\nName=\"The Scourge\"\nMission0=\"Chapter One\",\"Defense\",\"Maps\\Human01.w3m\"\n";

    #[test]
    #[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL, PCX_NATIVE_CHAPTER_MAP and PCX_NATIVE_CHAPTER_ADDON"]
    fn selected_install_chapter_activation_publishes_the_production_session() {
        drop(started_app());
    }

    pub(crate) fn started_app() -> App {
        let install_dir = std::env::var("PCX_NATIVE_CHAPTER_INSTALL")
            .expect("set PCX_NATIVE_CHAPTER_INSTALL to the selected local install");
        let map = std::env::var("PCX_NATIVE_CHAPTER_MAP")
            .expect("set PCX_NATIVE_CHAPTER_MAP to a map member from that install's chapter index");
        let expansion = match std::env::var("PCX_NATIVE_CHAPTER_ADDON").as_deref() {
            Ok("roc") => false,
            Ok("tft") => true,
            _ => panic!("PCX_NATIVE_CHAPTER_ADDON must explicitly select roc or tft"),
        };
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(crate::launch_gui::Launcher::test_source(
                install_dir,
                expansion,
            ))
            .init_resource::<crate::campaign_input::CampaignInput>()
            .init_resource::<crate::menu_screens::Navigation>()
            .init_resource::<Assets<Font>>()
            // The normal map-ground plugin declares this renderer capability too.
            .insert_resource(peacecraft3::map_ground::NoOcclusionPass);
        peacecraft3::map_requests::install(&mut app);
        peacecraft3::campaign_startup::install(&mut app);
        crate::campaign_prepare::install(&mut app);
        crate::button_actions::install(&mut app);
        install(&mut app);
        app.add_systems(Update, crate::button_actions::clear);
        app.world_mut().resource_mut::<CampaignSeed>().requested = Some(1);
        app.update();

        let chapters = app.world().resource::<Chapters>();
        assert!(
            chapters.error.is_none(),
            "chapter index: {:?}",
            chapters.error
        );
        let campaign = chapters
            .campaigns
            .iter()
            .position(|entry| {
                entry
                    .missions
                    .iter()
                    .any(|m| m.map.eq_ignore_ascii_case(&map))
            })
            .expect("requested map must come from the selected install's campaign index");
        let mission = chapters.campaigns[campaign]
            .missions
            .iter()
            .position(|m| m.map.eq_ignore_ascii_case(&map))
            .unwrap();
        let indexed_recipe = chapters.recipe.clone().expect("index retains its recipe");
        // Activate is the widget's keyboard/pointer boundary; physical picking is
        // a separate gate. The mission button is the only launch trigger; merely
        // selecting a campaign never launches.
        let button = app
            .world_mut()
            .spawn((Button, MissionButton { campaign, mission }, Node::default()))
            .id();
        app.world_mut().trigger(Activate { entity: button });
        for _ in 0..6 {
            app.update();
        }
        let chapters = app.world().resource::<Chapters>();
        assert!(
            chapters.error.is_none(),
            "chapter configuration: {:?}",
            chapters.error
        );
        assert!(
            chapters.pending.is_none(),
            "configuration did not complete within six updates"
        );
        let startup = app.world().resource::<CampaignStartup>();
        assert!(
            startup.last_error.is_none(),
            "production startup: {:?}",
            startup.last_error
        );
        let requests = app.world().resource::<MapRequests>();
        let context = requests
            .admission
            .shared_context()
            .expect("map must be admitted");
        let recipe = app
            .world()
            .resource::<peacecraft3::browser_configuration::SelectedMapSources>()
            .campaign_for_receipts(
                &requests
                    .bindings
                    .as_ref()
                    .expect("chapter bindings")
                    .receipts,
            )
            .expect("activated chapter publishes its recipe");
        assert!(Arc::ptr_eq(&recipe, &indexed_recipe,));
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        assert!(
            session
                .context()
                .is_some_and(|published| std::sync::Arc::ptr_eq(published, &context)),
            "the activated mission must publish the admitted context"
        );
        assert_eq!(app.world().resource::<CampaignSeed>().prepared, Some(1));
        let units = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().expect("published production host").objects();
            let flee = objects
                .campaign_flee_policy()
                .expect("campaign preparation admits the selected flee policy");
            assert_eq!(flee.image_sha256, recipe.image_sha256());
            let gathering = objects
                .campaign_gathering_policy()
                .expect("campaign preparation admits the selected gathering policy");
            assert_eq!(gathering.image_sha256, recipe.image_sha256());
            let captains = objects
                .campaign_captain_policy()
                .expect("campaign preparation admits the selected captain policy");
            assert_eq!(captains.image_sha256, recipe.image_sha256());
            let new_heroes = objects
                .campaign_new_heroes_policy()
                .expect("campaign preparation admits the selected new-heroes policy");
            assert_eq!(new_heroes.image_sha256, recipe.image_sha256());
            objects.executed_units().count()
        };
        assert!(units > 0, "unchanged chapter main must create units");
        println!("chapter={map} seed=1 executed_units={units} published=true");
        app
    }

    #[test]
    fn campaign_parse_preserves_order_and_default_open_and_mission_rows() {
        let (campaigns, chapters) = parse_campaigns("UI\\CampaignStrings.txt", FIXTURE).unwrap();
        assert_eq!(campaigns.len(), 2);
        assert_eq!(campaigns[0].key, "Tutorial");
        assert!(campaigns[0].default_open);
        assert_eq!(campaigns[0].background.as_deref(), Some("TutorialBackdrop"));
        assert_eq!(campaigns[0].missions.len(), 2);
        assert_eq!(campaigns[0].missions[0].map, "Maps\\Prologue01.w3m");
        assert!(!campaigns[1].default_open);
        assert_eq!(campaigns[1].title, "The Scourge");
        assert_eq!(campaigns[1].missions[0].name, "Defense");
        assert_eq!(chapters.len(), 3);
        assert_eq!(chapters[0].campaign, "Exodus");
        assert_eq!(chapters[2].map, "Maps\\Human01.w3m");
    }

    #[test]
    fn malformed_default_open_is_refused_instead_of_treated_closed() {
        let text = "[Index]\nCampaignList=\"One\"\n[One]\nDefaultOpen=banana\nMission0=\"H\",\"N\",\"Maps\\One.w3m\"\n";
        let error = parse_campaigns("member", text).unwrap_err();
        assert!(error.contains("DefaultOpen"), "{error}");
        let absent =
            "[Index]\nCampaignList=\"One\"\n[One]\nMission0=\"H\",\"N\",\"Maps\\One.w3m\"\n";
        let (campaigns, _) = parse_campaigns("member", absent).unwrap();
        assert!(
            !campaigns[0].default_open,
            "absent DefaultOpen stays closed"
        );
    }

    #[test]
    fn mission_row_background_key_does_not_match_prefix_fields() {
        let text = "[Index]\nCampaignList=\"One\"\n[One]\nDefaultOpen=1\nBackground=\"HumanBackdrop\"\nBackgroundFogStyle=0\nMission0=\"H\",\"N\",\"Maps\\One.w3m\"\n";
        let (campaigns, _) = parse_campaigns("member", text).unwrap();
        assert_eq!(campaigns[0].background.as_deref(), Some("HumanBackdrop"));
        assert!(campaigns[0].default_open);
    }

    #[test]
    fn losing_the_selected_install_clears_the_index_without_cancelling_pending_work() {
        let (campaigns, entries) = parse_campaigns("member", FIXTURE).unwrap();
        let mut world = World::new();
        world.insert_resource(crate::launch_gui::Launcher::test_source(
            String::new(),
            false,
        ));
        world.insert_resource(Chapters {
            campaigns,
            entries,
            pending: Some((1, 2)),
            ..default()
        });
        world.run_system_once(refresh).unwrap();
        let chapters = world.resource::<Chapters>();
        assert!(chapters.key.is_none());
        assert!(chapters.recipe.is_none());
        assert!(chapters.campaigns.is_empty());
        assert!(chapters.entries.is_empty());
        assert_eq!(chapters.pending, Some((1, 2)));
    }

    #[test]
    fn selecting_a_campaign_never_launches() {
        let (campaigns, _) = parse_campaigns("member", FIXTURE).unwrap();
        let mut world = World::new();
        world.insert_resource(Chapters {
            campaigns,
            ..default()
        });
        let btn = world.spawn((Button, CampaignButton(0))).id();
        world.insert_resource(CampaignMenuState::default());
        world.insert_resource(crate::button_actions::ButtonActions {
            entities: vec![btn],
        });
        world.run_system_once(select_campaign).unwrap();
        let state = world.resource::<CampaignMenuState>();
        assert_eq!(state.selected_campaign, Some(0));
        let chapters = world.resource::<Chapters>();
        assert!(
            chapters.pending.is_none(),
            "campaign selection must not launch"
        );
        let closed = world.spawn((Button, CampaignButton(1))).id();
        world
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities = vec![closed];
        world.run_system_once(select_campaign).unwrap();
        assert_eq!(
            world.resource::<CampaignMenuState>().selected_campaign,
            Some(0)
        );
    }

    #[test]
    fn dynamic_buttons_keep_their_row_position_and_do_not_intercept_label_hits() {
        let fdf = crate::fdf::Fdf::parse(
            "Frame \"TEXTBUTTON\" \"CampaignButtonTemplate\" {\n Width 0.1,\n Height 0.02,\n}\n",
        )
        .unwrap();
        let rect = crate::fdf::Rect {
            left: 0.2,
            bottom: 0.3,
            width: 0.1,
            height: 0.02,
        };
        let mut world = World::new();
        let panel = world.spawn_empty().id();
        spawn_campaign_button(
            &mut world.commands(),
            panel,
            &fdf,
            rect,
            Text::new("Synthetic campaign"),
            CampaignButton(0),
            false,
        )
        .unwrap();
        world.flush();
        let (entity, node) = world
            .query_filtered::<(Entity, &Node), With<CampaignButton>>()
            .single(&world)
            .unwrap();
        assert_eq!(
            node.left,
            Val::Percent(rect.left / crate::mainmenu::FRAME_W * 100.0)
        );
        assert!(
            world
                .get::<crate::menu_screens::ScreenPosition>(entity)
                .is_none()
        );
        assert!(world.get::<InteractionDisabled>(entity).is_some());
        assert!(
            world
                .get::<crate::menu_screens::CampaignAction>(entity)
                .is_some()
        );
        assert!(
            world
                .get::<bevy::input_focus::tab_navigation::TabIndex>(entity)
                .is_some()
        );
        let label = world
            .query_filtered::<Entity, With<Text>>()
            .single(&world)
            .unwrap();
        assert!(!world.get::<Pickable>(label).unwrap().is_hoverable);
    }
}
