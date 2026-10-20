//! Native campaign chapter selection from the selected install.
//!
//! Chapters come from that install's `UI\CampaignStrings.txt` (Reign of Chaos)
//! or `UI\CampaignStrings_exp.txt` (The Frozen Throne). Choosing one configures
//! its map through the same bounded binding path the browser uses, then requests
//! it; the campaign startup path takes over from there. The source mounts every
//! present archive in the client's default priority (campaign maps live in the
//! local archives for both editions), with Huffman weights read from the selected
//! client executable. Edition-specific `_V0`/`_V1` data variants are not selected
//! yet; that remains an open fidelity question.
use crate::launch::{Addon, AssetMode, Behaviour, GameMode};
use bevy::{prelude::*, ui_widgets::Button};
use pcx_assets::{ArchiveLayer, ContentSource, Resolver};
use pcx_play::install_source::{InstallSource, MAX_ASSET_BYTES, MAX_AGGREGATE_BYTES, MAX_LISTED_PATHS};
use pcx_objects::editor::campaign_index::{CampaignSlot, IndexLimits, parse_campaign_index};
use pcx_objects::editor::{EditorReadLimits, SlkId};
use pcx_play::MapScriptBudget;
use pcx_play::map_admission::MapAdmissionBudget;
use pcx_play::scripts::AssetIdentity;
use peacecraft3::browser_configuration::{Configuration, ConfigurationInbox, Member, Poll};
use peacecraft3::map_binding::{BindingLimits, OverlayLimits};
use peacecraft3::map_requests::{MapRequest, MapRequestSet, MapRequests};
use std::path::{Path, PathBuf};

const INDEX_LIMITS: IndexLimits = IndexLimits {
    max_bytes: 1 << 20,
    max_campaign_slots: 32,
    max_missions_per_campaign: 128,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Chapter {
    pub campaign: String,
    pub header: String,
    pub name: String,
    pub map: String,
}

/// Chapters of the selected install and edition, refreshed when either changes.
#[derive(Resource, Default)]
pub(crate) struct Chapters {
    key: Option<(PathBuf, GameMode)>,
    pub entries: Vec<Chapter>,
    pub error: Option<String>,
    /// Configuration request in flight and the binding revision it carries.
    pending: Option<(u32, u64)>,
    revision: u64,
}

#[derive(Component)]
pub(crate) struct ChapterButton(pub usize);

#[derive(Component)]
pub(crate) struct ChapterList;

pub(crate) fn install(app: &mut App) {
    let inbox = ConfigurationInbox::default();
    peacecraft3::browser_configuration::install(app, inbox);
    app.init_resource::<Chapters>().add_systems(
        Update,
        (refresh, populate, launch, request)
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

fn read_chapters(dir: &Path, mode: &GameMode) -> Result<Vec<Chapter>, String> {
    let source = InstallSource::open(dir)?;
    let member = index_member(mode);
    let content = source
        .read_bounded(member, INDEX_LIMITS.max_bytes as u64)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("{member} is absent from the selected install"))?;
    let index =
        parse_campaign_index(member, &content.bytes, INDEX_LIMITS).map_err(|e| e.to_string())?;
    let mut chapters = Vec::new();
    for slot in index.slots {
        let CampaignSlot::Entry(entry) = slot else {
            continue;
        };
        let campaign = entry.name.or(entry.header).unwrap_or(entry.id);
        for mission in entry.missions {
            chapters.push(Chapter {
                campaign: campaign.clone(),
                header: mission.header,
                name: mission.name,
                map: mission.map_file,
            });
        }
    }
    Ok(chapters)
}

fn refresh(launcher: Res<crate::launch_gui::Launcher>, mut chapters: ResMut<Chapters>) {
    let Some(dir) = crate::launch_gui::selected_install(&launcher) else {
        return;
    };
    let key = (dir, launcher.mode());
    if chapters.key.as_ref() == Some(&key) {
        return;
    }
    match read_chapters(&key.0, &key.1) {
        Ok(entries) => {
            chapters.entries = entries;
            chapters.error = None;
        }
        Err(error) => {
            chapters.entries.clear();
            chapters.error = Some(error);
        }
    }
    chapters.key = Some(key);
}

/// Fill each chapter list container once per chapter set.
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
    dir: &Path,
    mode: GameMode,
    chapter: &Chapter,
    revision: u64,
) -> Result<Configuration, String> {
    let source = InstallSource::open(dir)?;
    let info = source.info().clone();
    let edition = format!("{:?} campaign chapter", mode.addon);
    let member = |path: &str, label: &str| Member {
        path: path.into(),
        label: label.into(),
        identity: AssetIdentity {
            edition: edition.clone(),
            version: "selected install".into(),
            source: path.into(),
        },
        source: info.clone(),
    };
    let resolver = Resolver::new(
        asset_mode(mode)?,
        None,
        vec![ArchiveLayer {
            priority: 0,
            source: Box::new(source),
        }],
    )
    .map_err(|e| e.to_string())?;
    Ok(Configuration {
        resolver,
        map: member(&chapter.map, &chapter.name),
        common: member("Scripts\\common.j", "Scripts\\common.j"),
        blizzard: member("Scripts\\Blizzard.j", "Scripts\\Blizzard.j"),
        catalog_identity: AssetIdentity {
            edition,
            version: "selected install".into(),
            source: dir.display().to_string(),
        },
        catalog_label: "selected install object data".into(),
        catalog_sources: vec![info],
        required_catalog_tables: SlkId::ALL.to_vec(),
        required_fields: Vec::new(),
        revision,
        limits: limits(),
    })
}

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
        let Some(dir) = crate::launch_gui::selected_install(&launcher) else {
            chapters.error = Some("no selected install".into());
            continue;
        };
        chapters.revision += 1;
        let revision = chapters.revision;
        match configuration(&dir, launcher.mode(), &chapter, revision)
            .and_then(|request| inbox.enqueue(request).map_err(|e| e.to_string()))
        {
            Ok(id) => chapters.pending = Some((id, revision)),
            Err(error) => chapters.error = Some(error),
        }
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
