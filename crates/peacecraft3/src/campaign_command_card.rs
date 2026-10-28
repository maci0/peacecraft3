//! Learned command icons under the receipt-bound, authored console parent.
use crate::campaign_hud::upper_buttons::{image_sampler, read, skin_file, text};
use bevy::{
    ecs::system::SystemParam, picking::events::PointerPress, prelude::*, ui_widgets::Button,
};
use pcx_play::{
    ability_profile, command_layout::Rules, hero_bar_layout, map_startup_host::MapStartupHost,
    playercolor_host::ObjectHost,
};
use pcx_script::HandleId;
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignInterface, CampaignPresentationSet},
    map_binding::AssetReceipt,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

const SKIN: &str = r"UI\war3skins.txt";
const CONSOLE: &str = r"UI\FrameDef\UI\ConsoleUI.fdf";
// Application read/cache ceilings, not authored icon dimensions or slot counts.
const MAX_IMAGES: usize = 64;
const MAX_RETAINED: usize = 32 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    TargetUnit,
    Immediate,
    Train(i32),
    BuildMenu,
    Build(i32),
    BuildCancel,
    CancelConstruction,
    Standard(pcx_play::standard_command::Kind),
    /// Opens the research submenu on this same grid.
    SkillMenu,
    /// Returns from the research submenu to the cast grid.
    SkillCancel,
    /// `SelectHeroSkill` for one admitted learnable alias.
    Learn(i32),
    /// A learned ability with authored `Art` and `Buttonpos` but no implemented command.
    /// Drawn in its slot; pressing it issues no order.
    DisplayOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SkillPage {
    #[default]
    Main,
    Skills,
    Builds,
}

#[derive(Component, Clone)]
pub(crate) struct ButtonOrder {
    pub unit: HandleId,
    pub kind: Kind,
    pub session: Weak<()>,
    /// Order id from the learned cast command. Cast clicks issue this id.
    pub order: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Icon {
    active: bool,
    rank: Option<i32>,
    blocked_learning: bool,
    unavailable: Option<pcx_play::playercolor_host::BuildUnavailable>,
    kind: Kind,
    data: IconData,
    tooltip: Result<ability_profile::Tooltip, String>,
    order: Option<i32>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct IconData {
    art: String,
    position: [usize; 2],
    hotkey: Option<char>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct View {
    unit: HandleId,
    icons: Vec<Icon>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Header {
    current: Option<i32>,
    held: bool,
    hero: Option<(i32, i32)>,
    unit: HandleId,
    owner: HandleId,
    local: i32,
    shown: bool,
    dead: Option<bool>,
    movable: bool,
    can_attack: bool,
    constructing: bool,
}
struct Signature {
    header: Header,
    abilities: Vec<([u8; 4], i32, bool)>,
    builds: Vec<pcx_play::playercolor_host::BuildOption>,
}

fn header(objects: &ObjectHost) -> Result<Option<Header>, String> {
    let Some(&unit) = objects.selection().first() else {
        return Ok(None);
    };
    let record = objects
        .executed_unit(unit)
        .map_err(|error| error.to_string())?;
    let local = objects
        .local_player_number()
        .ok_or("command card has no local player")?;
    if objects
        .player_settings(record.owner)
        .map_err(|error| error.to_string())?
        .number
        != local
    {
        return Ok(None);
    }
    Ok(Some(Header {
        current: record
            .activity
            .as_ref()
            .map(|_| objects.current_unit_order(unit))
            .transpose()
            .map_err(|e| e.to_string())?,
        held: record.activity.as_ref().is_some_and(|a| a.held()),
        hero: record
            .abilities
            .as_ref()
            .and_then(|a| a.hero.as_ref())
            .map(|h| (h.level, h.skill_points)),
        unit,
        owner: record.owner,
        local,
        shown: record.shown,
        dead: record.combat.as_ref().map(|combat| combat.dead),
        movable: record.activity.as_ref().is_some_and(|a| a.movable()),
        can_attack: record.combat.as_ref().is_some_and(|c| c.weapon.is_some()),
        constructing: objects
            .unit_construction(unit)
            .map_err(|e| e.to_string())?
            .is_some(),
    }))
}

fn unchanged(objects: &ObjectHost, previous: Option<&Signature>) -> Result<bool, String> {
    let current = header(objects)?;
    let (Some(current), Some(previous)) = (current, previous) else {
        return Ok(current.is_none() && previous.is_none());
    };
    if current != previous.header {
        return Ok(false);
    }
    let record = objects
        .executed_unit(current.unit)
        .map_err(|error| error.to_string())?;
    let mut old = previous.abilities.iter();
    for (ability, rank) in record
        .abilities
        .iter()
        .flat_map(|abilities| abilities.levels())
    {
        let available = objects
            .unit_ability_available(current.unit, ability)
            .map_err(|error| error.to_string())?;
        if old.next() != Some(&(ability, rank, available)) {
            return Ok(false);
        }
    }
    Ok(old.next().is_none() && build_options(objects, current.unit)? == previous.builds)
}

fn build_options(
    objects: &ObjectHost,
    unit: HandleId,
) -> Result<Vec<pcx_play::playercolor_host::BuildOption>, String> {
    if objects
        .unit_build_menu_ability(unit)
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Ok(Vec::new());
    }
    objects.unit_build_options(unit).map_err(|e| e.to_string())
}

fn snapshot(objects: &ObjectHost) -> Result<Option<Signature>, String> {
    let Some(header) = header(objects)? else {
        return Ok(None);
    };
    let record = objects
        .executed_unit(header.unit)
        .map_err(|error| error.to_string())?;
    let abilities = record
        .abilities
        .iter()
        .flat_map(|abilities| abilities.levels())
        .map(|(ability, rank)| {
            objects
                .unit_ability_available(header.unit, ability)
                .map(|available| (ability, rank, available))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let builds = build_options(objects, header.unit)?;
    Ok(Some(Signature {
        header,
        abilities,
        builds,
    }))
}

struct Source {
    active_art: String,
    active_material: Option<Handle<crate::glue_menu::AdditiveImage>>,
    badge_art: String,
    badge_font: Option<Handle<Font>>,
    badge_height: f32,
    disabled_path: String,
    rules: Rules,
    console: String,
    standard: Vec<(pcx_play::standard_command::Command, String)>,
    skill: (pcx_play::standard_command::MenuButton, String),
    cancel: (pcx_play::standard_command::MenuButton, String),
    cancel_build: (pcx_play::standard_command::MenuButton, String),
    /// Independent of the command grid: a failure here leaves the card mounted.
    hero_bar: Result<HeroBarSource, String>,
    images: HashMap<String, Handle<Image>>,
    failed: HashMap<String, String>,
    retained: usize,
    receipts: Vec<AssetReceipt>,
}

/// Hero-bar layout from the client and the skin art for its two status bars.
struct HeroBarSource {
    rules: hero_bar_layout::Rules,
    health_art: String,
    mana_art: String,
}

/// One hero-bar row: the unit and its authored icon.
#[derive(Clone, Debug, PartialEq, Eq)]
struct HeroRow {
    unit: HandleId,
    art: String,
}

/// Selects the hero. The press belongs to the hero bar, not to a command order.
#[derive(Component, Clone)]
pub(crate) struct HeroBarButton {
    unit: HandleId,
    session: Weak<()>,
}

/// A status-bar fill whose width follows the hero's live vitals.
struct HeroFill {
    entity: Entity,
    /// Full bar width as a percentage of the viewport width.
    full: f32,
    fraction: f32,
}

#[derive(Resource, Default)]
pub(crate) struct CommandCard {
    witness: Option<Weak<()>>,
    source: Option<Source>,
    attempted: bool,
    root: Option<Entity>,
    parent: Option<Entity>,
    view: Option<View>,
    page: SkillPage,
    shown: SkillPage,
    signature: Option<Signature>,
    pending_pointer: bool,
    pub last_error: Option<String>,
    hero_root: Option<Entity>,
    hero_parent: Option<Entity>,
    hero_rows: Vec<HeroRow>,
    hero_fills: Vec<HeroFill>,
    pub hero_error: Option<String>,
}

impl CommandCard {
    pub(crate) fn world_frame(&self) -> Option<[f32; 4]> {
        let rules = &self.source.as_ref()?.rules;
        let r = rules.world_frame;
        Some([
            r.x / rules.viewport[0],
            1. - (r.y + r.height) / rules.viewport[1],
            r.width / rules.viewport[0],
            r.height / rules.viewport[1],
        ])
    }

    pub(crate) fn training_hotkeys(
        &self,
        unit: HandleId,
    ) -> impl Iterator<Item = (i32, char)> + '_ {
        self.view
            .iter()
            .filter(move |view| view.unit == unit && self.last_error.is_none())
            .flat_map(|view| view.icons.iter())
            .filter_map(|icon| {
                if let Kind::Train(kind) = icon.kind {
                    icon.data.hotkey.map(|key| (kind, key))
                } else {
                    None
                }
            })
    }
    pub(crate) fn standard_profiles(
        &self,
    ) -> impl Iterator<Item = &pcx_play::standard_command::Command> {
        self.source
            .iter()
            .flat_map(|source| source.standard.iter().map(|(profile, _)| profile))
    }
    pub fn receipt_count(&self) -> usize {
        self.source
            .as_ref()
            .map_or(0, |source| source.receipts.len())
    }

    pub(crate) fn build_hotkeys(&self, unit: HandleId) -> impl Iterator<Item = (Kind, char)> + '_ {
        self.view
            .iter()
            .filter(move |view| view.unit == unit && self.last_error.is_none())
            .flat_map(|view| &view.icons)
            .filter(|icon| icon.unavailable.is_none())
            .filter_map(|icon| match icon.kind {
                Kind::BuildMenu | Kind::Build(_) | Kind::BuildCancel => {
                    icon.data.hotkey.map(|key| (icon.kind, key))
                }
                _ => None,
            })
    }

    pub(crate) fn construction_cancel_hotkey(
        &self,
        unit: HandleId,
    ) -> Option<pcx_play::standard_command::MenuHotkey> {
        self.view
            .as_ref()
            .filter(|view| view.unit == unit && self.last_error.is_none())
            .filter(|view| {
                view.icons
                    .iter()
                    .any(|icon| icon.kind == Kind::CancelConstruction)
            })
            .and(self.source.as_ref())
            .map(|source| source.cancel_build.0.hotkey)
    }

    pub(crate) fn build_page_open(&self) -> bool {
        self.page == SkillPage::Builds
    }

    pub(crate) fn open_builds(&mut self) {
        self.page = SkillPage::Builds;
    }

    pub(crate) fn open_skills(&mut self) {
        self.page = SkillPage::Skills;
    }

    pub(crate) fn close_skills(&mut self) {
        self.page = SkillPage::Main;
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CommandCardSet;

#[derive(SystemParam)]
struct Inputs<'w, 's> {
    materials: ResMut<'w, Assets<crate::glue_menu::AdditiveImage>>,
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    camera: Res<'w, CampaignCamera>,
    prepared: Res<'w, crate::hud_render::HudPrepare>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
    frames: Query<'w, 's, (Entity, &'static crate::hud_render::HudSourceAttachment)>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CommandCard>()
        .init_resource::<Assets<crate::glue_menu::AdditiveImage>>()
        .add_observer(capture_press)
        .add_systems(
            Update,
            (capture, project)
                .chain()
                .in_set(CommandCardSet)
                .after(crate::hud_render::HudRenderSet)
                .after(CampaignCameraSet)
                .after(crate::campaign_hud::CampaignHudSet)
                .before(CampaignPresentationSet),
        )
        .add_systems(
            Update,
            select_hero
                .after(CommandCardSet)
                .before(crate::button_actions::clear),
        );
}

// Presses the card owns: command buttons and hero-bar buttons.
type PressTarget = Or<(With<ButtonOrder>, With<HeroBarButton>)>;

fn capture_press(
    event: On<PointerPress>,
    buttons: Query<(), PressTarget>,
    mut card: ResMut<CommandCard>,
    presentation: Option<Res<peacecraft3::campaign_presentation::CampaignPresentation>>,
) {
    if buttons.contains(event.entity) && crate::button_actions::input_allowed(presentation) {
        card.pending_pointer = true;
    }
}

fn capture(mut card: ResMut<CommandCard>, mut hud: ResMut<crate::campaign_hud::CampaignHud>) {
    hud.pointer_consumed |= std::mem::take(&mut card.pending_pointer);
}

fn wanted(objects: &ObjectHost, source: &Source, page: SkillPage) -> Result<Option<View>, String> {
    let Some(&unit) = objects.selection().first() else {
        return Ok(None);
    };
    let record = objects
        .executed_unit(unit)
        .map_err(|error| error.to_string())?;
    if objects
        .player_settings(record.owner)
        .map_err(|error| error.to_string())?
        .number
        != objects
            .local_player_number()
            .ok_or("command card has no local player")?
    {
        return Ok(None);
    }
    if objects
        .unit_construction(unit)
        .map_err(|e| e.to_string())?
        .is_some()
    {
        let (cancel, art) = &source.cancel_build;
        return Ok(Some(View {
            unit,
            icons: vec![Icon {
                active: false,
                rank: None,
                blocked_learning: false,
                unavailable: None,
                kind: Kind::CancelConstruction,
                data: IconData {
                    art: art.clone(),
                    position: cancel.position,
                    hotkey: menu_letter(cancel.hotkey),
                },
                tooltip: Ok(cancel.tooltip.clone()),
                order: None,
            }],
        }));
    }
    if page == SkillPage::Builds {
        return building_page(objects, unit, source);
    }
    if page == SkillPage::Skills {
        return research_page(objects, unit, source);
    }
    let mut icons = Vec::new();
    let current = record
        .activity
        .as_ref()
        .map(|_| objects.current_unit_order(unit))
        .transpose()
        .map_err(|e| e.to_string())?;
    for (profile, art) in &source.standard {
        if let Some(order) = objects
            .standard_command_order(unit, profile.kind)
            .map_err(|e| e.to_string())?
        {
            if icons
                .iter()
                .any(|icon: &Icon| icon.data.position == profile.position)
            {
                return Err("standard commands collide at the same authored Buttonpos".into());
            }
            icons.push(Icon {
                active: current.is_some_and(|current| {
                    standard_active(
                        profile.kind,
                        order,
                        current,
                        record.activity.as_ref().is_some_and(|a| a.held()),
                    )
                }),
                rank: None,
                blocked_learning: false,
                unavailable: None,
                kind: Kind::Standard(profile.kind),
                data: IconData {
                    art: art.clone(),
                    position: profile.position,
                    hotkey: None,
                },
                tooltip: Ok(profile.tooltip.clone()),
                order: None,
            });
        }
    }
    let mut represented = Vec::new();
    for (kind, command) in [
        (Kind::TargetUnit, objects.holy_light_command(unit)),
        (Kind::TargetUnit, objects.chain_lightning_command(unit)),
        (Kind::Immediate, objects.water_elemental_command(unit)),
    ] {
        if let Some(command) = command.map_err(|error| error.to_string())? {
            represented.push(command.ability);
            let data = objects
                .ability_button(unit, command.ability)
                .map_err(|error| error.to_string())?
                .ok_or("command has no learned presentation")?;
            pcx_assets::AssetPath::new(&data.art).map_err(|error| error.to_string())?;
            if icons
                .iter()
                .any(|icon: &Icon| icon.data.position == data.position)
            {
                return Err("implemented commands collide at the same authored Buttonpos".into());
            }
            let tooltip = objects
                .ability_tooltip(unit, command.ability)
                .map_err(|e| e.to_string())
                .and_then(|tip| tip.ok_or("command has no learned tooltip".into()));
            icons.push(Icon {
                active: false,
                rank: None,
                blocked_learning: false,
                unavailable: None,
                kind,
                data: IconData {
                    art: data.art,
                    position: data.position,
                    hotkey: None,
                },
                tooltip,
                order: Some(command.order),
            });
        }
    }
    for kind in pcx_play::training_command::types(&record.definition)? {
        let definition = objects
            .unit_catalog()
            .map_err(|e| e.to_string())?
            .resolve(kind)
            .map_err(|e| format!("training unit {kind:?}: {e:?}"))?;
        let command = pcx_play::training_command::Command::read(&definition)?;
        if icons
            .iter()
            .any(|icon| icon.data.position == command.position)
        {
            return Err("training commands collide at the same authored Buttonpos".into());
        }
        icons.push(Icon {
            active: false,
            rank: None,
            blocked_learning: false,
            unavailable: None,
            kind: Kind::Train(command.kind),
            data: IconData {
                art: command.art,
                position: command.position,
                hotkey: command.hotkey,
            },
            tooltip: command.tooltip,
            order: None,
        });
    }
    if let Some(command) = objects
        .unit_build_menu_command(unit)
        .map_err(|e| e.to_string())?
    {
        if icons
            .iter()
            .any(|icon| icon.data.position == command.position)
        {
            return Err("build menu collides at the authored Buttonpos".into());
        }
        icons.push(Icon {
            active: false,
            rank: None,
            blocked_learning: false,
            unavailable: None,
            kind: Kind::BuildMenu,
            data: IconData {
                art: command.art,
                position: command.position,
                hotkey: command.hotkey,
            },
            tooltip: command.tooltip,
            order: None,
        });
    }
    let listed = objects
        .learnable_skills(unit)
        .map_err(|error| error.to_string())?;
    if !listed.is_empty() {
        let (button, art) = &source.skill;
        if icons
            .iter()
            .any(|icon: &Icon| icon.data.position == button.position)
        {
            return Err("hero abilities collide at the same authored Buttonpos".into());
        }
        icons.push(Icon {
            active: false,
            rank: None,
            blocked_learning: false,
            unavailable: None,
            kind: Kind::SkillMenu,
            data: IconData {
                art: art.clone(),
                position: button.position,
                hotkey: menu_letter(button.hotkey),
            },
            tooltip: Ok(button.tooltip.clone()),
            order: None,
        });
    }
    represented.extend(
        objects
            .unit_build_menu_ability(unit)
            .map_err(|e| e.to_string())?,
    );
    let learned = objects
        .learned_ability_buttons(unit)
        .map_err(|error| error.to_string())?;
    for data in display_only(learned, &represented, &icons) {
        pcx_assets::AssetPath::new(&data.art).map_err(|error| error.to_string())?;
        let tooltip = objects
            .ability_tooltip(unit, data.ability)
            .map_err(|e| e.to_string())
            .and_then(|tip| tip.ok_or("command has no learned tooltip".into()));
        icons.push(Icon {
            active: false,
            rank: None,
            blocked_learning: false,
            unavailable: None,
            kind: Kind::DisplayOnly,
            data: IconData {
                art: data.art,
                position: data.position,
                hotkey: None,
            },
            tooltip,
            order: None,
        });
    }
    Ok(Some(View { unit, icons }))
}

/// Learned buttons the card does not already draw. The first button in a slot wins, so an
/// implemented command is never displaced; ability order decides between display-only
/// buttons that share a slot (for example gather and return-resources).
fn display_only(
    learned: Vec<ability_profile::Button>,
    represented: &[[u8; 4]],
    icons: &[Icon],
) -> Vec<ability_profile::Button> {
    let mut taken: Vec<[usize; 2]> = icons.iter().map(|icon| icon.data.position).collect();
    let mut shown = Vec::new();
    for button in learned {
        if represented.contains(&button.ability) || taken.contains(&button.position) {
            continue;
        }
        taken.push(button.position);
        shown.push(button);
    }
    shown
}

fn building_page(
    objects: &ObjectHost,
    unit: HandleId,
    source: &Source,
) -> Result<Option<View>, String> {
    if objects
        .unit_build_menu_ability(unit)
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Ok(None);
    }
    let mut icons = Vec::new();
    for choice in crate::campaign_build_ui::choices(objects, unit)? {
        let command = choice.command;
        if icons
            .iter()
            .any(|icon: &Icon| icon.data.position == command.position)
        {
            return Err("building choices collide at the authored Buttonpos".into());
        }
        icons.push(Icon {
            active: false,
            rank: None,
            blocked_learning: false,
            unavailable: choice.unavailable,
            kind: Kind::Build(command.kind),
            data: IconData {
                art: command.art,
                position: command.position,
                hotkey: command.hotkey,
            },
            tooltip: command.tooltip,
            order: None,
        });
    }
    let (cancel, art) = &source.cancel;
    if icons
        .iter()
        .any(|icon| icon.data.position == cancel.position)
    {
        return Err("build cancel collides at the authored Buttonpos".into());
    }
    icons.push(Icon {
        active: false,
        rank: None,
        blocked_learning: false,
        unavailable: None,
        kind: Kind::BuildCancel,
        data: IconData {
            art: art.clone(),
            position: cancel.position,
            hotkey: menu_letter(cancel.hotkey),
        },
        tooltip: Ok(cancel.tooltip.clone()),
        order: None,
    });
    Ok(Some(View { unit, icons }))
}

fn research_page(
    objects: &ObjectHost,
    unit: HandleId,
    source: &Source,
) -> Result<Option<View>, String> {
    let mut icons = Vec::new();
    for skill in objects
        .learnable_skills(unit)
        .map_err(|error| error.to_string())?
    {
        let Some(icon) = objects
            .research_icon(unit, skill.alias)
            .map_err(|error| error.to_string())?
        else {
            continue;
        };
        if icons
            .iter()
            .any(|existing: &Icon| existing.data.position == icon.position)
        {
            return Err("research icons collide at the same authored Researchbuttonpos".into());
        }
        icons.push(Icon {
            active: false,
            rank: Some(icon.rank),
            blocked_learning: icon.block.is_some(),
            unavailable: None,
            kind: Kind::Learn(i32::from_be_bytes(icon.ability)),
            data: IconData {
                art: icon.art,
                position: icon.position,
                hotkey: Some(icon.hotkey),
            },
            tooltip: Ok(pcx_play::ability_profile::Tooltip {
                title: icon.title,
                description: icon.description,
            }),
            order: None,
        });
    }
    let (button, art) = &source.cancel;
    if icons
        .iter()
        .any(|icon| icon.data.position == button.position)
    {
        return Err("research cancel collides with a research icon".into());
    }
    icons.push(Icon {
        active: false,
        rank: None,
        blocked_learning: false,
        unavailable: None,
        kind: Kind::SkillCancel,
        data: IconData {
            art: art.clone(),
            position: button.position,
            hotkey: menu_letter(button.hotkey),
        },
        tooltip: Ok(button.tooltip.clone()),
        order: None,
    });
    Ok(Some(View { unit, icons }))
}

fn menu_letter(hotkey: pcx_play::standard_command::MenuHotkey) -> Option<char> {
    match hotkey {
        pcx_play::standard_command::MenuHotkey::Letter(letter) => Some(letter),
        pcx_play::standard_command::MenuHotkey::Code(_) => None,
    }
}

fn read_standard_profiles(
    reader: &pcx_assets::Resolver,
    receipts: &mut Vec<AssetReceipt>,
) -> Result<
    (
        Vec<pcx_play::standard_command::Command>,
        pcx_play::standard_command::MenuButton,
        pcx_play::standard_command::MenuButton,
        pcx_play::standard_command::MenuButton,
    ),
    String,
> {
    use pcx_objects::editor::{Editor, EditorReadLimits, TxtRole};
    let listed: Vec<String> = [TxtRole::CommandFunc, TxtRole::CommandStrings]
        .into_iter()
        .flat_map(|role| role.files().iter().map(|path| (*path).to_owned()))
        .collect();
    let editor = Editor::from_reader(
        std::path::Path::new("selected command UI resolver"),
        &listed,
        EditorReadLimits {
            max_listed_paths: listed.len(),
            max_asset_bytes: 4 << 20,
            max_aggregate_input_bytes: 8 << 20,
        },
        |path| {
            if listed.iter().any(|member| member == path) {
                read(reader, path, 4 << 20, receipts)
                    .map(Some)
                    .map_err(std::io::Error::other)
            } else {
                Ok(None)
            }
        },
    )
    .map_err(|e| e.to_string())?;
    let standard = pcx_play::standard_command::Kind::ALL
        .into_iter()
        .map(|kind| pcx_play::standard_command::Command::read(&editor, kind))
        .collect::<Result<Vec<_>, _>>()?;
    let skill = pcx_play::standard_command::MenuButton::read(&editor, "CmdSelectSkill")?;
    let cancel = pcx_play::standard_command::MenuButton::read(&editor, "CmdCancel")?;
    let cancel_build = pcx_play::standard_command::MenuButton::read(&editor, "CmdCancelBuild")?;
    Ok((standard, skill, cancel, cancel_build))
}

fn reset(
    commands: &mut Commands,
    card: &mut CommandCard,
    images: &mut Assets<Image>,
    fonts: &mut Assets<Font>,
    materials: &mut Assets<crate::glue_menu::AdditiveImage>,
    witness: Option<Weak<()>>,
) {
    for root in [card.root, card.hero_root].into_iter().flatten() {
        commands.entity(root).try_despawn();
    }
    if let Some(source) = card.source.take() {
        if let Some(material) = source.active_material {
            materials.remove(material.id());
        }
        if let Some(font) = source.badge_font {
            fonts.remove(font.id());
        }
        for image in source.images.values() {
            images.remove(image.id());
        }
    }
    *card = CommandCard {
        witness,
        ..default()
    };
}

fn image(
    reader: &pcx_assets::Resolver,
    source: &mut Source,
    path: &str,
    images: &mut Assets<Image>,
) -> Result<Handle<Image>, String> {
    if let Some(image) = source.images.get(path) {
        return Ok(image.clone());
    }
    if let Some(error) = source.failed.get(path) {
        return Err(error.clone());
    }
    if source.images.len() + source.failed.len() >= MAX_IMAGES {
        return Err("command image cache budget exceeded".into());
    }
    let result: Result<Handle<Image>, String> = (|| {
        let mut receipts = Vec::new();
        let bytes = read(reader, path, 4 << 20, &mut receipts)?;
        let remaining = MAX_RETAINED
            .checked_sub(source.retained)
            .ok_or("command image aggregate budget")?;
        let texture = pcx_gfx::load_image_bounded(&bytes, path, remaining.min(16 << 20))
            .map_err(|error| error.to_string())?;
        let retained = source
            .retained
            .checked_add(texture.rgba_bytes().ok_or("command image size")?)
            .filter(|bytes| *bytes <= MAX_RETAINED)
            .ok_or("command image aggregate budget")?;
        let mut decoded = pcx_gfx::viewer::rgba_image(&texture);
        decoded.sampler =
            bevy::image::ImageSampler::Descriptor(image_sampler(source.rules.icon_repeat));
        let handle = images.add(decoded);
        source.images.insert(path.into(), handle.clone());
        source.retained = retained;
        source.receipts.extend(receipts);
        Ok(handle)
    })();
    if let Err(error) = &result {
        source.failed.insert(path.into(), error.clone());
    }
    result
}

fn standard_active(
    kind: pcx_play::standard_command::Kind,
    order: i32,
    current: i32,
    held: bool,
) -> bool {
    use pcx_play::standard_command::Kind;
    if held {
        return kind == Kind::Hold;
    }
    if kind == Kind::Hold {
        return false;
    }
    (kind == Kind::Stop && current == 0) || current == order
}

fn command_art(source: &Source, icon: &Icon) -> Result<String, String> {
    if icon.blocked_learning {
        source
            .rules
            .disabled_art(&source.disabled_path, &icon.data.art)
    } else {
        Ok(icon.data.art.clone())
    }
}

fn paint(
    commands: &mut Commands,
    root: Entity,
    source: &Source,
    view: &View,
    witness: &Weak<()>,
) -> Result<(), String> {
    let mut nodes = Vec::new();
    for icon in &view.icons {
        let rect = source
            .rules
            .slot(icon.data.position[0], icon.data.position[1])?;
        let art = command_art(source, icon)?;
        let image = source
            .images
            .get(&art)
            .ok_or("command icon is not decoded")?
            .clone();
        nodes.push((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(rect.x / source.rules.viewport[0] * 100.),
                bottom: Val::Percent(rect.y / source.rules.viewport[1] * 100.),
                width: Val::Percent(rect.width / source.rules.viewport[0] * 100.),
                height: Val::Percent(rect.height / source.rules.viewport[1] * 100.),
                ..default()
            },
            image,
            icon.kind,
            icon.active,
            icon.rank,
            icon.order,
            rect,
            &icon.tooltip,
            icon.unavailable.is_some() || icon.blocked_learning,
        ));
    }
    if view.icons.iter().any(|icon| icon.rank.is_some())
        && (source.badge_font.is_none() || !source.images.contains_key(&source.badge_art))
    {
        return Err("command rank overlay is not decoded".into());
    }
    if view.icons.iter().any(|icon| icon.active) && source.active_material.is_none() {
        return Err("active command overlay is not decoded".into());
    }
    commands.entity(root).despawn_children();
    for (node, image, kind, active, rank, order, rect, tooltip, disabled) in nodes {
        commands.entity(root).with_children(|parent| {
            let mut button = parent.spawn((
                node,
                ImageNode::new(image).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
                Button,
                bevy::picking::hover::Hovered::default(),
                crate::campaign_inventory::TooltipAnchor {
                    rect,
                    viewport: source.rules.viewport,
                },
                ButtonOrder {
                    unit: view.unit,
                    kind,
                    session: witness.clone(),
                    order,
                },
            ));
            if disabled {
                button.insert(bevy::ui::InteractionDisabled);
            }
            if active {
                button.with_children(|button| {
                    button.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.),
                            height: Val::Percent(100.),
                            ..default()
                        },
                        MaterialNode(
                            source
                                .active_material
                                .as_ref()
                                .expect("preflighted active art")
                                .clone(),
                        ),
                        Pickable::IGNORE,
                    ));
                });
            }
            if let Some(rank) = rank {
                let badge = &source.rules.badge;
                let art = source
                    .images
                    .get(&source.badge_art)
                    .expect("preflighted badge art");
                let font = source.badge_font.as_ref().expect("preflighted badge font");
                let [a, r, g, b] = badge.colour.to_be_bytes();
                button.with_children(|button| {
                    button
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                right: Val::Percent(-badge.offset[0] / rect.width * 100.),
                                bottom: Val::Percent(badge.offset[1] / rect.height * 100.),
                                width: Val::Percent(badge.size[0] / rect.width * 100.),
                                height: Val::Percent(badge.size[1] / rect.height * 100.),
                                ..default()
                            },
                            ImageNode::new(art.clone())
                                .with_mode(bevy::ui::widget::NodeImageMode::Stretch),
                            Pickable::IGNORE,
                        ))
                        .with_children(|badge_node| {
                            badge_node
                                .spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Percent(
                                            badge.text_offset[0] / badge.size[0] * 100.,
                                        ),
                                        bottom: Val::Percent(
                                            badge.text_offset[1] / badge.size[1] * 100.,
                                        ),
                                        width: Val::Percent(100.),
                                        height: Val::Percent(100.),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    Pickable::IGNORE,
                                ))
                                .with_children(|text| {
                                    text.spawn((
                                        Text::new(rank.to_string()),
                                        TextFont {
                                            font: font.clone().into(),
                                            font_size: bevy::text::FontSize::Vh(
                                                source.badge_height / source.rules.viewport[1]
                                                    * 100.,
                                            ),
                                            ..default()
                                        },
                                        TextColor(Color::srgba_u8(r, g, b, a)),
                                        TextLayout::new(
                                            bevy::text::Justify::Center,
                                            bevy::text::LineBreak::NoWrap,
                                        ),
                                        bevy::text::LineHeight::RelativeToFont(1.0),
                                        Pickable::IGNORE,
                                    ));
                                });
                        });
                });
            }
            match tooltip {
                Ok(tip) => {
                    button.insert(crate::campaign_inventory::ControlTooltip {
                        name: tip.title.clone(),
                        ubertip: tip.description.clone(),
                    });
                }
                Err(error) => {
                    button.insert(crate::campaign_inventory::TooltipFailure(error.clone()));
                }
            }
        });
    }
    Ok(())
}

fn root_node(display: Display) -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.),
        height: Val::Percent(100.),
        display,
        ..default()
    }
}

fn project(
    mut commands: Commands,
    mut card: ResMut<CommandCard>,
    mut inputs: Inputs,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
    mut nodes: Query<&mut Node>,
) {
    let witness = session.session_witness();
    if !same_session(card.witness.as_ref(), witness.as_ref()) {
        reset(
            &mut commands,
            &mut card,
            &mut images,
            &mut fonts,
            &mut inputs.materials,
            witness.clone(),
        );
    }
    let (Some(witness), Some(context)) = (witness, session.context().cloned()) else {
        return;
    };
    let Some(bindings) = inputs.requests.bindings.as_ref().filter(|bindings| {
        inputs
            .requests
            .published_receipts
            .as_ref()
            .is_some_and(|receipts| Arc::ptr_eq(receipts, &bindings.receipts))
            && inputs
                .requests
                .admission
                .shared_context()
                .is_some_and(|admitted| Arc::ptr_eq(&admitted, &context))
    }) else {
        return;
    };
    if !inputs.prepared.mounted_for(&context, bindings.revision) {
        return;
    }
    let Some(reader) = inputs.sources.for_receipts(&bindings.receipts) else {
        return;
    };
    let Some(host) = session.host() else { return };
    if !card.attempted {
        card.attempted = true;
        let result = (|| {
            let install = inputs
                .launcher
                .as_deref()
                .and_then(crate::launch_gui::selected_install);
            let (rules, hero_rules) =
                crate::menu_lighting::with_client_image(install.as_deref(), |label, image| {
                    let identity = pcx_play::campaign_inputs::client_identity(image, label);
                    Ok((
                        Rules::reviewed_legacy(image, identity.clone())?,
                        hero_bar_layout::Rules::reviewed_legacy(image, identity),
                    ))
                })?;
            let mut receipts = Vec::new();
            let skin = crate::menu_skin::SkinTable::parse(&text(&reader, SKIN, &mut receipts)?)
                .map_err(|error| error.to_string())?;
            let sections = crate::campaign_hud::race_sections(
                &skin,
                host.objects()
                    .local_race_preference_constant()
                    .map_err(|error| error.to_string())?,
            )?;
            let console = skin_file(&skin, &sections, &rules.console_key)?;
            let (commands, skill, cancel, cancel_build) =
                read_standard_profiles(&reader, &mut receipts)?;
            let resolve = |art_key: &str, position: [usize; 2]| -> Result<String, String> {
                let art = skin_file(&skin, &sections, art_key)?;
                pcx_assets::AssetPath::new(&art).map_err(|e| e.to_string())?;
                rules.slot(position[0], position[1])?;
                Ok(art)
            };
            let standard = commands
                .into_iter()
                .map(|profile| {
                    let art = resolve(&profile.art_key, profile.position)?;
                    Ok((profile, art))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let skill = {
                let art = resolve(&skill.art_key, skill.position)?;
                (skill, art)
            };
            let cancel = {
                let art = resolve(&cancel.art_key, cancel.position)?;
                (cancel, art)
            };
            let cancel_build = {
                let art = resolve(&cancel_build.art_key, cancel_build.position)?;
                (cancel_build, art)
            };
            let hero_bar = hero_rules.and_then(|hero| {
                let health_art = skin_file(&skin, &sections, &hero.health.fill_key)?;
                let mana_art = skin_file(&skin, &sections, &hero.mana.fill_key)?;
                Ok(HeroBarSource {
                    rules: hero,
                    health_art,
                    mana_art,
                })
            });
            let active_art = skin_file(&skin, &sections, &rules.active_key)?;
            let badge_art = skin_file(&skin, &sections, &rules.badge.art_key)?;
            let font_path = skin_file(&skin, &sections, &rules.badge.font_key)?;
            let font_bytes = read(&reader, &font_path, 16 << 20, &mut receipts)?;
            if !crate::hud_render::font_registers(&font_bytes) {
                return Err(format!("command badge font is unreadable: {font_path}"));
            }
            let profile = crate::menu_skin::SkinTable::parse(&text(
                &reader,
                r"UI\MiscUI.txt",
                &mut receipts,
            )?)
            .map_err(|e| e.to_string())?;
            let heights = profile
                .sections()
                .iter()
                .filter(|s| s.name.eq_ignore_ascii_case(&rules.badge.section))
                .flat_map(|s| &s.entries)
                .filter(|(key, _)| key.eq_ignore_ascii_case(&rules.badge.height_key))
                .map(|(_, value)| value)
                .collect::<Vec<_>>();
            let [height] = heights.as_slice() else {
                return Err("command badge font height missing or ambiguous".into());
            };
            let badge_height = height
                .parse::<f32>()
                .ok()
                .filter(|h| h.is_finite() && *h > 0.)
                .ok_or("command badge font height is invalid")?;
            text(&reader, CONSOLE, &mut receipts)?;
            let badge_font = Some(fonts.add(Font::from_bytes(font_bytes)));
            Ok::<_, String>(Source {
                active_art,
                active_material: None,
                badge_art,
                badge_font,
                badge_height,
                disabled_path: skin_file(&skin, &sections, &rules.disabled_key)?,
                rules,
                console,
                standard,
                skill,
                cancel,
                cancel_build,
                hero_bar,
                receipts,
                images: HashMap::new(),
                failed: HashMap::new(),
                retained: 0,
            })
        })();
        match result {
            Ok(source) => card.source = Some(source),
            Err(error) => {
                card.last_error = Some(error);
                return;
            }
        }
    }
    let Some(mut source) = card.source.take() else {
        return;
    };
    let result = (|| {
        let mut parents = inputs
            .frames
            .iter()
            .filter(|(_, frame)| {
                frame.frame == source.console
                    && frame.source_member == CONSOLE
                    && frame.binding_revision == bindings.revision
                    && frame.receipt_path == bindings.receipts.map.path.as_str()
            })
            .map(|(entity, _)| entity);
        let Some(parent) = parents.next() else {
            return Err("command card awaits its mounted console parent".into());
        };
        if parents.next().is_some() {
            return Err("command card console parent is ambiguous".into());
        }
        let mut rebuilt = false;
        if card.parent != Some(parent) || card.root.is_none_or(|root| nodes.get(root).is_err()) {
            if let Some(root) = card.root {
                commands.entity(root).try_despawn();
            }
            let root = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.),
                        height: Val::Percent(100.),
                        ..default()
                    },
                    CampaignInterface,
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(parent).add_child(root);
            card.parent = Some(parent);
            card.root = Some(root);
            rebuilt = true;
        }
        // Tables are immutable in a published session; avoid rebuilding metadata every frame.
        let changed = !unchanged(host.objects(), card.signature.as_ref())?;
        if changed
            && let (Some(next), Some(previous)) = (header(host.objects())?, card.signature.as_ref())
            && next.unit != previous.header.unit
        {
            card.page = SkillPage::Main;
        }
        let page_changed = card.shown != card.page;
        if changed || page_changed {
            card.view = wanted(host.objects(), &source, card.page)?;
            card.shown = card.page;
            if changed {
                card.signature = snapshot(host.objects())?;
            }
        }
        let root = card.root.expect("command root");
        let visible = inputs.camera.active()
            && card
                .view
                .as_ref()
                .is_some_and(|view| !view.icons.is_empty());
        let display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if nodes.get(root).map_or(true, |node| node.display != display) {
            commands.entity(root).insert(root_node(display));
        }
        if rebuilt || changed || page_changed || card.last_error.is_some() {
            if let Some(view) = &card.view {
                if view.icons.iter().any(|icon| icon.active) && source.active_material.is_none() {
                    let art = source.active_art.clone();
                    let texture = image(&reader, &mut source, &art, &mut images)?;
                    source.active_material = Some(
                        inputs
                            .materials
                            .add(crate::glue_menu::AdditiveImage::new(texture)),
                    );
                }
                if view.icons.iter().any(|icon| icon.rank.is_some()) {
                    let art = source.badge_art.clone();
                    image(&reader, &mut source, &art, &mut images)?;
                }
                for icon in &view.icons {
                    let art = command_art(&source, icon)?;
                    image(&reader, &mut source, &art, &mut images)?;
                }
                paint(&mut commands, root, &source, view, &witness)?;
            } else {
                commands.entity(root).despawn_children();
            }
        }
        Ok::<_, String>(())
    })();
    if result.is_err()
        && let Some(root) = card.root
        && nodes
            .get(root)
            .map_or(true, |node| node.display != Display::None)
    {
        commands.entity(root).insert(root_node(Display::None));
    }
    card.last_error = result.err();
    card.hero_error = sync_hero_bar(
        &mut HeroFrame {
            commands: &mut commands,
            images: &mut images,
            nodes: &mut nodes,
        },
        &mut card,
        &mut source,
        &reader,
        host.objects(),
        &witness,
        inputs.camera.active(),
    )
    .err();
    card.source = Some(source);
}

/// Bevy handles the hero bar writes through.
struct HeroFrame<'a, 'cw, 'cs, 'qw, 'qs, 'n> {
    commands: &'a mut Commands<'cw, 'cs>,
    images: &'a mut Assets<Image>,
    nodes: &'a mut Query<'qw, 'qs, &'n mut Node>,
}

/// One painted hero: its row identity and live status-bar fractions.
struct HeroEntry {
    row: HeroRow,
    health: f32,
    /// Zero when the hero has no mana pool, which leaves the bar empty.
    mana: f32,
}

fn fraction(value: f32, maximum: f32) -> f32 {
    (value / maximum).clamp(0., 1.)
}

/// Living, shown heroes of the local player in unit creation order, at most one per grid row.
/// Native hero ordering, overflow past the grid and dead-hero presentation are not qualified.
fn hero_entries(objects: &ObjectHost, rows: usize) -> Result<Vec<HeroEntry>, String> {
    let Some(local) = objects.local_player_number() else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    for (unit, record) in objects.executed_units() {
        if entries.len() == rows {
            break;
        }
        if !record.abilities.as_ref().is_some_and(|a| a.hero.is_some())
            || !record.shown
            || record.combat.as_ref().is_none_or(|combat| combat.dead)
            || objects
                .player_settings(record.owner)
                .map_err(|error| error.to_string())?
                .number
                != local
        {
            continue;
        }
        let vitals = record
            .vitals
            .as_ref()
            .ok_or("hero bar: live vitals unavailable")?;
        if !vitals.max_life.is_finite()
            || vitals.max_life <= 0.
            || !vitals.max_mana.is_finite()
            || vitals.max_mana < 0.
        {
            return Err("hero bar: invalid live maximum".into());
        }
        entries.push(HeroEntry {
            row: HeroRow {
                unit,
                art: pcx_play::training_command::icon(&record.definition)?,
            },
            health: fraction(vitals.life, vitals.max_life),
            mana: if vitals.max_mana > 0. {
                fraction(vitals.mana, vitals.max_mana)
            } else {
                0.
            },
        });
    }
    Ok(entries)
}

fn hero_source(source: &Source) -> Result<&HeroBarSource, String> {
    source.hero_bar.as_ref().map_err(Clone::clone)
}

fn colour(argb: u32) -> Color {
    let [a, r, g, b] = argb.to_be_bytes();
    Color::srgba_u8(r, g, b, a)
}

fn viewport_node(rect: pcx_play::inventory_layout::Rect, viewport: [f32; 2]) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(rect.x / viewport[0] * 100.),
        bottom: Val::Percent(rect.y / viewport[1] * 100.),
        width: Val::Percent(rect.width / viewport[0] * 100.),
        height: Val::Percent(rect.height / viewport[1] * 100.),
        ..default()
    }
}

/// Mounts the hero rows under the console parent and follows the heroes' live vitals.
/// Rows are rebuilt only when the hero set or an icon changes; otherwise only fill widths move.
fn sync_hero_bar(
    frame: &mut HeroFrame<'_, '_, '_, '_, '_, '_>,
    card: &mut CommandCard,
    source: &mut Source,
    reader: &pcx_assets::Resolver,
    objects: &ObjectHost,
    witness: &Weak<()>,
    active: bool,
) -> Result<(), String> {
    let Some(parent) = card.parent else {
        return Ok(());
    };
    let hero = hero_source(source)?;
    let entries = hero_entries(objects, hero.rules.rows)?;
    let rows: Vec<HeroRow> = entries.iter().map(|entry| entry.row.clone()).collect();
    let display = if active && !entries.is_empty() {
        Display::Flex
    } else {
        Display::None
    };
    let mounted = card
        .hero_root
        .filter(|root| frame.nodes.get(*root).is_ok())
        .filter(|_| card.hero_parent == Some(parent) && card.hero_rows == rows);
    if let Some(root) = mounted {
        if frame
            .nodes
            .get(root)
            .is_ok_and(|node| node.display != display)
        {
            frame.commands.entity(root).insert(root_node(display));
        }
        for (index, entry) in entries.iter().enumerate() {
            for (slot, value) in [(2 * index, entry.health), (2 * index + 1, entry.mana)] {
                let fill = card
                    .hero_fills
                    .get_mut(slot)
                    .ok_or("hero bar fill is missing")?;
                if fill.fraction != value {
                    fill.fraction = value;
                    frame
                        .nodes
                        .get_mut(fill.entity)
                        .map_err(|_| "hero bar fill node is missing")?
                        .width = Val::Percent(fill.full * value);
                }
            }
        }
        return Ok(());
    }
    if let Some(root) = card.hero_root.take() {
        frame.commands.entity(root).try_despawn();
    }
    card.hero_fills.clear();
    card.hero_rows.clear();
    let (health_art, mana_art) = (hero.health_art.clone(), hero.mana_art.clone());
    let icons = rows
        .iter()
        .map(|row| image(reader, source, &row.art, frame.images))
        .collect::<Result<Vec<_>, _>>()?;
    let fills = [
        image(reader, source, &health_art, frame.images)?,
        image(reader, source, &mana_art, frame.images)?,
    ];
    let hero = hero_source(source)?;
    let rules = &hero.rules;
    let root = frame
        .commands
        .spawn((root_node(display), CampaignInterface, Pickable::IGNORE))
        .id();
    frame.commands.entity(parent).add_child(root);
    for (index, (entry, icon)) in entries.iter().zip(icons).enumerate() {
        let button = rules.button(index)?;
        frame.commands.spawn((
            viewport_node(button, rules.viewport),
            ImageNode::new(icon).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
            Button,
            bevy::picking::hover::Hovered::default(),
            HeroBarButton {
                unit: entry.row.unit,
                session: witness.clone(),
            },
            ChildOf(root),
        ));
        for (bar, style, texture, value) in [
            (
                rules.health_bar(index)?,
                &rules.health,
                &fills[0],
                entry.health,
            ),
            (rules.mana_bar(index)?, &rules.mana, &fills[1], entry.mana),
        ] {
            let outset = pcx_play::inventory_layout::Rect {
                x: bar.x - rules.inset,
                y: bar.y - rules.inset,
                width: bar.width + 2. * rules.inset,
                height: bar.height + 2. * rules.inset,
            };
            frame.commands.spawn((
                viewport_node(outset, rules.viewport),
                BackgroundColor(colour(rules.background)),
                Pickable::IGNORE,
                ChildOf(root),
            ));
            let full = bar.width / rules.viewport[0] * 100.;
            let mut node = viewport_node(bar, rules.viewport);
            node.width = Val::Percent(full * value);
            let entity = frame
                .commands
                .spawn((
                    node,
                    ImageNode::new(texture.clone())
                        .with_mode(bevy::ui::widget::NodeImageMode::Stretch)
                        .with_color(colour(style.tint)),
                    Pickable::IGNORE,
                    ChildOf(root),
                ))
                .id();
            card.hero_fills.push(HeroFill {
                entity,
                full,
                fraction: value,
            });
        }
    }
    card.hero_root = Some(root);
    card.hero_parent = Some(parent);
    card.hero_rows = rows;
    Ok(())
}

/// A hero-bar press selects that hero, replacing the selection.
fn select_hero(
    actions: Res<crate::button_actions::ButtonActions>,
    buttons: Query<&HeroBarButton, Without<bevy::ui::InteractionDisabled>>,
    controls: HeroControls,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let witness = session.session_witness();
    let Some(host) = session.host() else { return };
    if !crate::campaign_input::commands_available(
        &controls.view,
        &controls.hud,
        &controls.dialogs,
        host,
    ) {
        return;
    }
    for entity in &actions.entities {
        let Ok(button) = buttons.get(*entity) else {
            continue;
        };
        if !witness
            .as_ref()
            .is_some_and(|current| Weak::ptr_eq(current, &button.session))
        {
            continue;
        }
        let objects = host.objects_mut();
        let local = objects.local_player_number();
        let owned = objects
            .executed_unit(button.unit)
            .ok()
            .and_then(|record| objects.player_settings(record.owner).ok())
            .is_some_and(|player| Some(player.number) == local);
        if owned {
            objects.select_units(&[button.unit], true);
        }
    }
}

#[derive(SystemParam)]
struct HeroControls<'w> {
    view: Res<'w, CampaignCamera>,
    hud: Res<'w, crate::campaign_hud::CampaignHud>,
    dialogs: Res<'w, crate::campaign_script_dialogs::CampaignScriptDialogs>,
}

fn same_session(a: Option<&Weak<()>>, b: Option<&Weak<()>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::NormalizedRenderTarget,
        ecs::system::RunSystemOnce,
        picking::{
            backend::HitData,
            events::Pointer,
            pointer::{Location, PointerButton, PointerId},
        },
    };
    use pcx_play::scripts::AssetIdentity;

    fn fixture_menu() -> (pcx_play::standard_command::MenuButton, String) {
        (
            pcx_play::standard_command::MenuButton {
                art_key: "fixture".into(),
                position: [0, 0],
                hotkey: pcx_play::standard_command::MenuHotkey::Letter('o'),
                tooltip: pcx_play::ability_profile::Tooltip {
                    title: "fixture".into(),
                    description: "fixture".into(),
                },
            },
            "fixture".into(),
        )
    }

    fn selected_icons() -> (Source, View, pcx_assets::Resolver) {
        use pcx_assets::{Addon, ArchiveLayer, AssetMode, Behaviour, Build, GameMode, Resolver};
        use pcx_objects::editor::defaults::{DefaultTables, Family};
        let dir = std::path::PathBuf::from(
            std::env::var_os("PCX_NATIVE_CHAPTER_INSTALL").expect("set PCX_NATIVE_CHAPTER_INSTALL"),
        );
        let path = std::env::var("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE");
        let bytes = std::fs::read(&path).unwrap();
        let identity = pcx_play::campaign_inputs::client_identity(&bytes, &path);
        let rules = Rules::reviewed_legacy(&bytes, identity.clone()).unwrap();
        let tables = DefaultTables::open(&dir, identity.version).unwrap();
        let mut icons = Vec::new();
        for (ability, kind) in [(*b"AHhb", Kind::TargetUnit), (*b"AHwe", Kind::Immediate)] {
            let row = pcx_objects::fourcc_text(ability);
            let object = tables.object(Family::Ability, &row).unwrap();
            let art = object.profile("Art");
            let [art] = art.as_slice() else {
                panic!("{row}: expected one source Art")
            };
            let position = object.profile("Buttonpos");
            let [position] = position.as_slice() else {
                panic!("{row}: expected one source Buttonpos")
            };
            let position: Vec<usize> = position
                .text
                .split(',')
                .map(|v| v.trim().parse().unwrap())
                .collect();
            icons.push(Icon {
                active: false,
                rank: None,
                blocked_learning: false,
                unavailable: None,
                kind,
                tooltip: Err("standalone icon fixture has no learned unit".into()),
                data: IconData {
                    art: art.text.to_owned(),
                    position: position.try_into().unwrap(),
                    hotkey: None,
                },
                order: None,
            });
        }
        let reader = Resolver::new(
            GameMode {
                build: Build::V1_29b,
                addon: Addon::NoAddon,
                assets: AssetMode::Vanilla,
                behaviour: Behaviour::Vanilla,
            },
            None,
            vec![ArchiveLayer {
                priority: 0,
                source: Box::new(pcx_play::install_source::InstallSource::open(&dir).unwrap()),
            }],
        )
        .unwrap();
        (
            Source {
                active_art: String::new(),
                active_material: None,
                badge_art: String::new(),
                badge_font: None,
                badge_height: 0.,
                disabled_path: "fixture-disabled/".into(),
                rules,
                console: "synthetic component parent".into(),
                standard: Vec::new(),
                skill: fixture_menu(),
                cancel: fixture_menu(),
                cancel_build: fixture_menu(),
                hero_bar: Err("fixture".into()),
                images: HashMap::new(),
                failed: HashMap::new(),
                retained: 0,
                receipts: Vec::new(),
            },
            View {
                unit: HandleId::new(std::num::NonZeroU64::new(7).unwrap()),
                icons,
            },
            reader,
        )
    }

    #[test]
    #[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL and PCX_WAR3_EXE; reads local original icons"]
    fn selected_original_icons_decode_once_and_missing_assets_are_cached() {
        let (mut source, view, reader) = selected_icons();
        let mut images = Assets::<Image>::default();
        for icon in &view.icons {
            let handle = image(&reader, &mut source, &icon.data.art, &mut images).unwrap();
            let receipt_count = source.receipts.len();
            let retained = source.retained;
            let decoded = images.get(&handle).unwrap();
            assert!(decoded.width() > 0 && decoded.height() > 0);
            assert!(decoded.data.as_ref().is_some_and(|data| !data.is_empty()));
            assert_eq!(
                image(&reader, &mut source, &icon.data.art, &mut images)
                    .unwrap()
                    .id(),
                handle.id()
            );
            assert_eq!(source.receipts.len(), receipt_count);
            assert_eq!(source.retained, retained);
            assert!(
                source
                    .receipts
                    .iter()
                    .any(|receipt| receipt.path
                        == pcx_assets::AssetPath::new(&icon.data.art).unwrap())
            );
        }
        assert_eq!(images.len(), view.icons.len());
        assert!(source.retained > 0 && source.retained <= MAX_RETAINED);
        let missing = "PCX-Fixture-Missing-Command-Icon.blp";
        let first = image(&reader, &mut source, missing, &mut images).unwrap_err();
        assert_eq!(
            image(&reader, &mut source, missing, &mut images).unwrap_err(),
            first
        );
        assert_eq!(source.failed.len(), 1);
    }

    #[test]
    #[ignore = "requires selected unchanged Human01 PCX_NATIVE_CHAPTER_* inputs; no campaign progression claim"]
    fn selected_chapter_learned_tooltips_expand_both_source_spells_at_every_rank() {
        drop(learned_tooltip_fixture());
    }

    fn learned_tooltip_fixture() -> (
        Option<String>,
        Vec<(ability_profile::Button, ability_profile::Tooltip)>,
    ) {
        use pcx_objects::editor::{
            SlkId,
            defaults::{DefaultTables, Family},
        };
        use pcx_script::{Host, Value};
        let dir = std::path::PathBuf::from(std::env::var_os("PCX_NATIVE_CHAPTER_INSTALL").unwrap());
        let client = dir.join("Warcraft III.exe");
        let identity = pcx_play::campaign_inputs::client_identity(
            &std::fs::read(&client).unwrap(),
            &client.display().to_string(),
        );
        let tables = DefaultTables::open(&dir, identity.version).unwrap();
        let mage = tables
            .rawcodes(Family::Unit)
            .find(|row| {
                tables
                    .object(Family::Unit, row)
                    .unwrap()
                    .cell(SlkId::UnitAbilities, "heroAbilList")
                    .is_ok_and(|cell| cell.text.split(',').any(|id| id.trim() == "AHwe"))
            })
            .expect("a source hero with Water Elemental")
            .as_bytes()
            .try_into()
            .unwrap();
        let mut app = crate::native_campaign::tests::started_app();
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let holy = match session.global("gg_unit_Hart_0023") {
            Some(Value::Handle(unit)) => unit,
            other => panic!("Human01 Arthas: {other:?}"),
        };
        let objects = session.host().unwrap().objects_mut();
        let record = objects.executed_unit(holy).unwrap();
        let position = record.position.unwrap_or([record.x, record.y]);
        let args = [
            Value::Handle(record.owner),
            Value::Int(i32::from_be_bytes(mage)),
            Value::Real(position[0]),
            Value::Real(position[1]),
            Value::Real(record.face),
        ];
        let water = match objects.native("CreateUnit", &args).unwrap() {
            Value::Handle(unit) => unit,
            other => panic!("source mage creation: {other:?}"),
        };
        let race = objects.local_race_preference_constant().unwrap();
        let mut learned = Vec::new();
        for (unit, ability) in [(holy, *b"AHhb"), (water, *b"AHwe")] {
            let row = pcx_objects::fourcc_text(ability);
            let source = tables.object(Family::Ability, &row).unwrap();
            let number = |column: &str| {
                source
                    .cell(SlkId::AbilityData, column)
                    .unwrap()
                    .text
                    .parse::<i32>()
                    .unwrap()
            };
            let levels = number("levels");
            // The native caps this boundary request using the admitted source progression.
            objects
                .native(
                    "SetHeroLevel",
                    &[
                        Value::Handle(unit),
                        Value::Int(i32::MAX),
                        Value::Bool(false),
                    ],
                )
                .unwrap();
            for rank in 1..=levels {
                if objects
                    .executed_unit(unit)
                    .unwrap()
                    .abilities
                    .as_ref()
                    .unwrap()
                    .level(ability)
                    < rank
                {
                    objects
                        .native(
                            "SelectHeroSkill",
                            &[Value::Handle(unit), Value::Int(i32::from_be_bytes(ability))],
                        )
                        .unwrap();
                }
                assert_eq!(
                    objects
                        .executed_unit(unit)
                        .unwrap()
                        .abilities
                        .as_ref()
                        .unwrap()
                        .level(ability),
                    rank
                );
                let tip = objects.ability_tooltip(unit, ability).unwrap().unwrap();
                assert_eq!(
                    tip.title,
                    source
                        .profile_element("Tip", rank - 1, levels as u32)
                        .unwrap()
                        .text
                );
                assert!(!tip.description.is_empty() && !tip.description.contains(['<', '>']));
                let readiness = match &ability {
                    b"AHhb" => objects.holy_light_readiness(unit),
                    b"AHwe" => objects.water_elemental_readiness(unit),
                    _ => unreachable!("fixture only learns the two implemented spells"),
                }
                .unwrap()
                .unwrap();
                assert_eq!(readiness.mana_cost, number(&format!("Cost{rank}")) as f32);
                let record = objects.executed_unit(unit).unwrap();
                assert_eq!(readiness.mana, record.vitals.map(|vitals| vitals.mana));
                assert_eq!(readiness.paused, record.paused_since.is_some());
                assert_eq!(readiness.cooldown_remaining, 0.0);
                let duration_args = [
                    Value::Handle(unit),
                    Value::Int(i32::from_be_bytes(ability)),
                    Value::Int(rank),
                ];
                let original_duration = number(&format!("Cool{rank}")) as f32;
                let source_duration_args =
                    [Value::Int(i32::from_be_bytes(ability)), Value::Int(rank)];
                assert_eq!(
                    objects
                        .native("BlzGetAbilityCooldown", &source_duration_args)
                        .unwrap(),
                    Value::Real(original_duration)
                );
                assert_eq!(
                    objects
                        .native("BlzGetUnitAbilityCooldown", &duration_args)
                        .unwrap(),
                    Value::Real(original_duration)
                );
                let mut duration_set = duration_args.to_vec();
                duration_set.push(Value::Real(original_duration * 1.5));
                objects
                    .native("BlzSetUnitAbilityCooldown", &duration_set)
                    .unwrap();
                assert_eq!(
                    objects
                        .native("BlzGetAbilityCooldown", &source_duration_args)
                        .unwrap(),
                    Value::Real(original_duration)
                );
                assert_eq!(
                    objects
                        .native("BlzGetUnitAbilityCooldown", &duration_args)
                        .unwrap(),
                    Value::Real(original_duration * 1.5)
                );
                duration_set[3] = Value::Real(original_duration);
                objects
                    .native("BlzSetUnitAbilityCooldown", &duration_set)
                    .unwrap();
                assert_eq!(
                    objects
                        .native("UnitResetCooldown", &[Value::Handle(unit)])
                        .unwrap(),
                    Value::Null
                );
                assert_eq!(
                    objects
                        .native(
                            "BlzGetUnitAbilityCooldownRemaining",
                            &[Value::Handle(unit), Value::Int(i32::from_be_bytes(ability))]
                        )
                        .unwrap(),
                    Value::Real(0.0)
                );
                assert_eq!(
                    objects
                        .native(
                            "BlzEndUnitAbilityCooldown",
                            &[Value::Handle(unit), Value::Int(i32::from_be_bytes(ability))]
                        )
                        .unwrap(),
                    Value::Null
                );
                println!("ability={row} rank={rank} references_expanded=true");
                learned.push((objects.ability_button(unit, ability).unwrap().unwrap(), tip));
            }
        }
        (race, learned)
    }

    #[cfg(feature = "test-fixtures")]
    #[test]
    #[ignore = "requires unchanged Human01 and selected PCX_NATIVE_CHAPTER_* inputs and PCX_WAR3_EXE"]
    fn selected_chapter_mounts_the_card_under_its_admitted_console_and_retires_it() {
        use bevy::ecs::system::RunSystemOnce;
        use pcx_script::{Host, Value};
        let mut app = crate::native_campaign::tests::started_app();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(CampaignCamera::active_for_test(camera))
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<Font>>()
            .init_resource::<Assets<crate::glue_menu::BackdropImage>>()
            .init_resource::<crate::campaign_script_dialogs::CampaignScriptDialogs>()
            .init_resource::<ButtonInput<KeyCode>>();
        crate::campaign_hud::install(&mut app);
        crate::hud_render::install(&mut app);
        install(&mut app);
        let unit = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let unit = match session.global("gg_unit_Hart_0023") {
                Some(Value::Handle(unit)) => unit,
                other => panic!("the explicit Human01 fixture needs Arthas: {other:?}"),
            };
            let objects = session.host().unwrap().objects_mut();
            // Operator selection/learning, independent of cinematic progression.
            if objects.holy_light_command(unit).unwrap().is_none() {
                objects
                    .native(
                        "SelectHeroSkill",
                        &[
                            Value::Handle(unit),
                            Value::Int(i32::from_be_bytes(*b"AHhb")),
                        ],
                    )
                    .unwrap();
            }
            for old in objects.selection().to_vec() {
                objects
                    .native("SelectUnit", &[Value::Handle(old), Value::Bool(false)])
                    .unwrap();
            }
            objects
                .native("SelectUnit", &[Value::Handle(unit), Value::Bool(true)])
                .unwrap();
            unit
        };
        for _ in 0..6 {
            app.update();
        }
        let (root, parent, owned, buttons) = {
            let card = app.world().resource::<CommandCard>();
            let hud = app.world().resource::<crate::hud_render::HudMount>();
            assert!(hud.error.is_none(), "HUD mount: {:?}", hud.error);
            assert!(
                app.world()
                    .resource::<crate::hud_render::HudPrepare>()
                    .refusal
                    .is_none(),
                "HUD preparation: {:?}",
                app.world()
                    .resource::<crate::hud_render::HudPrepare>()
                    .refusal
            );
            assert!(
                card.last_error.is_none(),
                "command card: {:?}",
                card.last_error
            );
            let source = card.source.as_ref().expect("selected command source");
            let root = card.root.expect("command root");
            let parent = card.parent.expect("console parent");
            assert_eq!(app.world().get::<ChildOf>(root).unwrap().parent(), parent);
            let attachment = app
                .world()
                .get::<crate::hud_render::HudSourceAttachment>(parent)
                .unwrap();
            assert_eq!(attachment.frame, source.console);
            let buttons = app.world().get::<Children>(root).unwrap();
            assert!(!buttons.is_empty());
            for kind in pcx_play::standard_command::Kind::ALL {
                assert!(
                    buttons.iter().any(|child| app
                        .world()
                        .get::<ButtonOrder>(child)
                        .is_some_and(|button| button.kind == Kind::Standard(kind))),
                    "missing standard {kind:?}"
                );
            }
            for child in buttons {
                assert_eq!(app.world().get::<ButtonOrder>(*child).unwrap().unit, unit);
                assert_eq!(
                    app.world().get::<ImageNode>(*child).unwrap().image_mode,
                    bevy::ui::widget::NodeImageMode::Stretch,
                    "command art must fill the authored frame independently of image aspect"
                );
                let tooltip = app
                    .world()
                    .get::<crate::campaign_inventory::ControlTooltip>(*child)
                    .expect("source-ranked tooltip must reach its mounted button");
                assert!(!tooltip.name.is_empty() && !tooltip.ubertip.contains(['<', '>']));
            }
            assert!(card.hero_error.is_none(), "hero bar: {:?}", card.hero_error);
            let hero_root = card.hero_root.expect("hero bar root");
            assert_eq!(
                app.world().get::<ChildOf>(hero_root).unwrap().parent(),
                parent
            );
            let rows = app
                .world()
                .get::<Children>(hero_root)
                .unwrap()
                .iter()
                .filter(|child| app.world().get::<HeroBarButton>(*child).is_some())
                .count();
            assert!(rows >= 1 && rows == card.hero_rows.len());
            assert_eq!(card.hero_fills.len(), rows * 2);
            for fill in &card.hero_fills {
                assert!((0.0..=1.0).contains(&fill.fraction));
                let node = app.world().get::<Node>(fill.entity).unwrap();
                assert_eq!(node.width, Val::Percent(fill.full * fill.fraction));
            }
            (
                root,
                parent,
                source.images.values().cloned().collect::<Vec<_>>(),
                buttons.iter().collect::<Vec<_>>(),
            )
        };
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<CommandCard>().root, Some(root));
        assert_eq!(
            app.world()
                .get::<Children>(root)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            buttons
        );
        let expected_sections = app
            .world()
            .resource::<crate::hud_render::HudSourcePolicy>()
            .race_sections
            .clone();
        let mut next = crate::native_campaign::tests::started_app();
        let replacement = next
            .world_mut()
            .remove_resource::<MapScriptSession<MapStartupHost>>()
            .unwrap();
        let next_requests = next.world_mut().remove_resource::<MapRequests>().unwrap();
        let next_sources = next
            .world_mut()
            .remove_resource::<SelectedMapSources>()
            .unwrap();
        let old_context = app
            .world()
            .resource::<MapRequests>()
            .admission
            .shared_context()
            .unwrap();
        assert!(!Arc::ptr_eq(&old_context, replacement.context().unwrap()));
        app.insert_resource(replacement);
        app.world_mut().run_system_once(project).unwrap();
        assert!(app.world().get_entity(root).is_err());
        assert!(
            app.world().get_entity(parent).is_ok(),
            "the card does not own the console"
        );
        app.update();
        assert!(
            app.world().get_entity(parent).is_err(),
            "the HUD owner retires the console when its source session changes"
        );
        for image in owned {
            assert!(
                app.world()
                    .resource::<Assets<Image>>()
                    .get(&image)
                    .is_none()
            );
        }
        assert!(app.world().resource::<CommandCard>().source.is_none());
        assert!(
            app.world().resource::<CommandCard>().root.is_none(),
            "a lookalike session cannot use the previous context's bindings"
        );
        assert!(
            !app.world()
                .contains_resource::<crate::hud_render::HudSourcePolicy>(),
            "the previous session's HUD source policy must retire too"
        );
        app.insert_resource(next_requests)
            .insert_resource(next_sources);
        for _ in 0..6 {
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<crate::hud_render::HudSourcePolicy>()
                .race_sections,
            expected_sections
        );
        let card = app.world().resource::<CommandCard>();
        assert!(
            card.last_error.is_none(),
            "replacement card: {:?}",
            card.last_error
        );
        assert!(
            card.source.is_some() && card.root.is_some(),
            "coherent publication must mount a new card"
        );
        assert_ne!(card.root, Some(root));
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .clear();
        app.update();
        assert!(app.world().resource::<CommandCard>().root.is_none());
    }

    #[cfg(feature = "test-fixtures")]
    #[test]
    #[ignore = "requires live desktop, selected install/executable, and PCX_COMMAND_CARD_ABILITY=AHhb|AHwe"]
    fn selected_original_command_icon_native_component() {
        native_component(false);
    }

    #[cfg(feature = "test-fixtures")]
    #[test]
    #[ignore = "requires live desktop and unchanged Human01 PCX_NATIVE_CHAPTER_* plus PCX_COMMAND_CARD_ABILITY=AHhb|AHwe"]
    fn selected_original_command_tooltip_native_component() {
        native_component(true);
    }

    #[cfg(feature = "test-fixtures")]
    fn native_component(with_tooltip: bool) {
        let (mut source, mut view, reader) = selected_icons();
        let ability = std::env::var("PCX_COMMAND_CARD_ABILITY").expect("choose a fixture ability");
        assert!(matches!(ability.as_str(), "AHhb" | "AHwe"));
        view.icons.retain(|icon| match ability.as_str() {
            "AHhb" => icon.kind == Kind::TargetUnit,
            "AHwe" => icon.kind == Kind::Immediate,
            _ => false,
        });
        assert_eq!(view.icons.len(), 1);
        let race = if with_tooltip {
            let (race, learned) = learned_tooltip_fixture();
            let (data, tip) = learned
                .into_iter()
                .rfind(|(data, _)| data.ability.as_slice() == ability.as_bytes())
                .unwrap();
            view.icons[0].data = IconData {
                art: data.art,
                position: data.position,
                hotkey: None,
            };
            view.icons[0].tooltip = Ok(tip);
            race
        } else {
            None
        };
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "PeaceCraft command card fixture".into(),
                        resolution: (1280_u32, 960_u32).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::winit::WinitPlugin {
                    run_on_any_thread: true,
                })
                .disable::<bevy::audio::AudioPlugin>(),
        );
        let camera = app.world_mut().spawn(Camera2d).id();
        {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            for icon in &view.icons {
                image(&reader, &mut source, &icon.data.art, &mut images).unwrap();
            }
        }
        let root = app.world_mut().spawn(root_node(Display::Flex)).id();
        let session = Arc::new(());
        let mut queue = bevy::ecs::world::CommandQueue::default();
        paint(
            &mut Commands::new(&mut queue, app.world()),
            root,
            &source,
            &view,
            &Arc::downgrade(&session),
        )
        .unwrap();
        queue.apply(app.world_mut());
        if with_tooltip {
            let path = std::env::var("PCX_WAR3_EXE").unwrap();
            let bytes = std::fs::read(&path).unwrap();
            crate::campaign_inventory::tooltip_component_fixture(
                &mut app,
                &reader,
                &bytes,
                pcx_play::campaign_inputs::client_identity(&bytes, &path),
                race,
                root,
                camera,
            );
            app.add_systems(
                PostUpdate,
                (|windows: Query<&Window>,
                  buttons: Query<(&ComputedNode, &UiGlobalTransform), With<ButtonOrder>>,
                  mut previous: Local<Option<UVec2>>| {
                    let Ok(window) = windows.single() else { return };
                    let size = window.physical_size();
                    let Ok((node, transform)) = buttons.single() else {
                        return;
                    };
                    if node.size().min_element() <= 0.0 || *previous == Some(size) {
                        return;
                    }
                    let centre = transform.to_scale_angle_translation().2;
                    if centre.x <= 0.0
                        || centre.y <= 0.0
                        || centre.x >= size.x as f32
                        || centre.y >= size.y as f32
                    {
                        return;
                    }
                    println!(
                        "CONTROL_CENTER width={} height={} x={} y={}",
                        size.x, size.y, centre.x, centre.y
                    );
                    *previous = Some(size);
                })
                .after(bevy::ui::UiSystems::PostLayout),
            );
        }
        // Watchdog only; screenshots and exact geometry checks establish the visual result.
        app.add_systems(Update, |time: Res<Time<Real>>| {
            assert!(time.elapsed_secs() < 45.)
        });
        assert!(matches!(app.run(), AppExit::Success));
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE for the selected command geometry; images are synthetic"]
    fn selected_client_card_preflights_replacement_and_releases_only_owned_assets() {
        let path = std::env::var("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE");
        let rules = Rules::reviewed_legacy(
            &std::fs::read(&path).unwrap(),
            AssetIdentity {
                source: path,
                edition: "selected local client".into(),
                version: "reviewed image".into(),
            },
        )
        .unwrap();
        let mut images = Assets::<Image>::default();
        let owned = images.add(Image::default());
        let unrelated = images.add(Image::default());
        let mut fonts = Assets::<Font>::default();
        let owned_font = fonts.add(Font::from_bytes(Vec::new()));
        let unrelated_font = fonts.add(Font::from_bytes(Vec::new()));
        let mut materials = Assets::<crate::glue_menu::AdditiveImage>::default();
        let owned_material = materials.add(crate::glue_menu::AdditiveImage::new(owned.clone()));
        let unrelated_material =
            materials.add(crate::glue_menu::AdditiveImage::new(unrelated.clone()));
        let source = Source {
            active_art: String::new(),
            active_material: Some(owned_material.clone()),
            badge_art: String::new(),
            badge_font: Some(owned_font.clone()),
            badge_height: 0.,
            disabled_path: "fixture-disabled/".into(),
            rules,
            console: "synthetic parent".into(),
            standard: Vec::new(),
            skill: fixture_menu(),
            cancel: fixture_menu(),
            cancel_build: fixture_menu(),
            hero_bar: Err("fixture".into()),
            images: HashMap::from([("fixture".into(), owned.clone())]),
            failed: HashMap::new(),
            retained: 0,
            receipts: Vec::new(),
        };
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        let session = Arc::new(());
        let witness = Arc::downgrade(&session);
        let mut view = View {
            unit: HandleId::new(std::num::NonZeroU64::new(7).unwrap()),
            icons: vec![Icon {
                active: true,
                rank: None,
                blocked_learning: false,
                unavailable: None,
                kind: Kind::TargetUnit,
                tooltip: Err("synthetic icon-only fixture".into()),
                data: IconData {
                    art: "fixture".into(),
                    position: [0, 0],
                    hotkey: None,
                },
                order: None,
            }],
        };
        let mut queue = bevy::ecs::world::CommandQueue::default();
        paint(
            &mut Commands::new(&mut queue, &world),
            root,
            &source,
            &view,
            &witness,
        )
        .unwrap();
        queue.apply(&mut world);
        let child = world.get::<Children>(root).unwrap()[0];
        assert_eq!(world.get::<ButtonOrder>(child).unwrap().unit, view.unit);
        let overlay = world.get::<Children>(child).unwrap()[0];
        assert_eq!(
            world
                .get::<MaterialNode<crate::glue_menu::AdditiveImage>>(overlay)
                .unwrap()
                .0
                .id(),
            owned_material.id()
        );
        let rect = source.rules.slot(0, 0).unwrap();
        assert_eq!(
            world.get::<Node>(child).unwrap().width,
            Val::Percent(rect.width / source.rules.viewport[0] * 100.)
        );
        view.icons[0].rank = Some(7);
        assert!(
            paint(
                &mut Commands::new(&mut queue, &world),
                root,
                &source,
                &view,
                &witness
            )
            .is_err()
        );
        queue.apply(&mut world);
        assert_eq!(world.get::<Children>(root).unwrap()[0], child);
        view.icons[0].rank = None;
        view.icons.push(Icon {
            active: false,
            rank: None,
            blocked_learning: false,
            unavailable: None,
            kind: Kind::Immediate,
            tooltip: Err("synthetic icon-only fixture".into()),
            data: IconData {
                art: "fixture".into(),
                position: [source.rules.columns, 0],
                hotkey: None,
            },
            order: None,
        });
        assert!(
            paint(
                &mut Commands::new(&mut queue, &world),
                root,
                &source,
                &view,
                &witness
            )
            .is_err()
        );
        queue.apply(&mut world);
        assert_eq!(world.get::<Children>(root).unwrap()[0], child);
        assert_eq!(world.get::<Children>(root).unwrap().len(), 1);
        let mut card = CommandCard {
            root: Some(root),
            source: Some(source),
            witness: Some(witness),
            ..default()
        };
        reset(
            &mut Commands::new(&mut queue, &world),
            &mut card,
            &mut images,
            &mut fonts,
            &mut materials,
            None,
        );
        queue.apply(&mut world);
        assert!(world.get_entity(root).is_err());
        assert!(world.get_entity(child).is_err());
        assert!(images.get(&owned).is_none());
        assert!(images.get(&unrelated).is_some());
        assert!(fonts.get(&owned_font).is_none());
        assert!(fonts.get(&unrelated_font).is_some());
        assert!(materials.get(&owned_material).is_none());
        assert!(materials.get(&unrelated_material).is_some());
        assert!(card.root.is_none() && card.source.is_none() && card.witness.is_none());
    }

    #[test]
    fn session_identity_is_not_interchangeable_with_another_live_session() {
        let first = Arc::new(());
        let second = Arc::new(());
        let first_witness = Arc::downgrade(&first);
        assert!(same_session(
            Some(&first_witness),
            Some(&Arc::downgrade(&first))
        ));
        assert!(!same_session(
            Some(&first_witness),
            Some(&Arc::downgrade(&second))
        ));
        assert!(!same_session(Some(&first_witness), None));
        drop(first);
        assert!(!same_session(
            Some(&first_witness),
            Some(&Arc::downgrade(&second))
        ));
        assert!(same_session(None, None));
    }

    #[test]
    fn command_press_consumes_world_pointer_once_without_consuming_unrelated_presses() {
        let mut app = App::new();
        app.init_resource::<CommandCard>()
            .init_resource::<crate::campaign_hud::CampaignHud>()
            .add_observer(capture_press);
        let session = Arc::new(());
        let button = app
            .world_mut()
            .spawn(ButtonOrder {
                unit: HandleId::new(std::num::NonZeroU64::new(7).unwrap()),
                kind: Kind::TargetUnit,
                session: Arc::downgrade(&session),
                order: None,
            })
            .id();
        let unrelated = app.world_mut().spawn_empty().id();
        let press = |entity| PointerPress {
            entity,
            pointer: Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: 1,
                        height: 1,
                    },
                    position: Vec2::ZERO,
                },
            ),
            button: PointerButton::Primary,
            hit: HitData::new(entity, 0.0, None, None),
            count: 1,
        };
        app.world_mut().trigger(press(unrelated));
        app.world_mut().run_system_once(capture).unwrap();
        assert!(
            !app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
        app.world_mut().trigger(press(button));
        app.world_mut().run_system_once(capture).unwrap();
        assert!(
            app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
        app.world_mut()
            .resource_mut::<crate::campaign_hud::CampaignHud>()
            .pointer_consumed = false;
        app.world_mut().run_system_once(capture).unwrap();
        assert!(
            !app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
    }
    #[test]
    fn training_hotkeys_belong_to_the_current_valid_selection() {
        let unit = HandleId::new(std::num::NonZeroU64::new(7).unwrap());
        let other = HandleId::new(std::num::NonZeroU64::new(8).unwrap());
        let mut card = CommandCard {
            view: Some(View {
                unit,
                icons: vec![Icon {
                    active: false,
                    rank: None,
                    blocked_learning: false,
                    unavailable: None,
                    kind: Kind::Train(i32::from_be_bytes(*b"uNEW")),
                    data: IconData {
                        art: "fixture/icon.blp".into(),
                        position: [0, 0],
                        hotkey: Some('Q'),
                    },
                    tooltip: Ok(ability_profile::Tooltip {
                        title: "Source title".into(),
                        description: "Source text".into(),
                    }),
                    order: None,
                }],
            }),
            ..Default::default()
        };
        assert_eq!(
            card.training_hotkeys(unit).collect::<Vec<_>>(),
            vec![(i32::from_be_bytes(*b"uNEW"), 'Q')]
        );
        assert!(card.training_hotkeys(other).next().is_none());
        card.last_error = Some("source mismatch".into());
        assert!(card.training_hotkeys(unit).next().is_none());
    }
    #[test]
    fn building_hotkeys_follow_source_readiness_and_page_changes() {
        let unit = HandleId::new(std::num::NonZeroU64::new(51).unwrap());
        let kind = Kind::Build(i32::from_be_bytes(*b"bNEW"));
        let mut card = CommandCard {
            view: Some(View {
                unit,
                icons: vec![Icon {
                    active: false,
                    rank: None,
                    blocked_learning: false,
                    unavailable: Some(pcx_play::playercolor_host::BuildUnavailable::Resources),
                    kind,
                    data: IconData {
                        art: "fixture/build.blp".into(),
                        position: [2, 1],
                        hotkey: Some('Q'),
                    },
                    tooltip: Err("fixture".into()),
                    order: None,
                }],
            }),
            ..default()
        };
        card.open_builds();
        assert!(card.build_page_open());
        assert!(card.build_hotkeys(unit).next().is_none());
        card.view.as_mut().unwrap().icons[0].unavailable = None;
        assert_eq!(
            card.build_hotkeys(unit).collect::<Vec<_>>(),
            vec![(kind, 'Q')]
        );
        card.close_skills();
        assert!(!card.build_page_open());
    }
}

#[cfg(test)]
mod active_tests {
    use super::standard_active;
    use pcx_play::standard_command::Kind;
    #[test]
    fn idle_stop_hold_and_admitted_nonstock_orders_select_one_standard_icon() {
        assert!(standard_active(Kind::Stop, 17, 0, false));
        assert!(!standard_active(Kind::Hold, 23, 0, false));
        assert!(standard_active(Kind::Hold, 23, 41, true));
        assert!(!standard_active(Kind::Attack, 41, 41, true));
        assert!(standard_active(Kind::Move, 31, 31, false));
        assert!(!standard_active(Kind::Stop, 17, 31, false));
        assert!(standard_active(Kind::Attack, 41, 41, false));
        assert!(!standard_active(Kind::Move, 31, 41, false));
    }
}

#[cfg(test)]
mod display_only_tests {
    use super::{Icon, IconData, Kind, ability_profile::Button, display_only};

    fn button(ability: &[u8; 4], position: [usize; 2]) -> Button {
        Button {
            ability: *ability,
            rank: 1,
            art: format!("{}.blp", String::from_utf8_lossy(ability)),
            position,
        }
    }

    fn icon(position: [usize; 2]) -> Icon {
        Icon {
            active: false,
            rank: None,
            blocked_learning: false,
            unavailable: None,
            kind: Kind::TargetUnit,
            data: IconData {
                art: "implemented.blp".into(),
                position,
                hotkey: None,
            },
            tooltip: Err("fixture".into()),
            order: Some(1),
        }
    }

    #[test]
    fn learned_buttons_fill_only_free_slots_and_skip_represented_abilities() {
        let learned = vec![
            button(b"AAAA", [0, 2]),
            button(b"BBBB", [1, 2]),
            button(b"CCCC", [1, 2]),
            button(b"DDDD", [2, 2]),
            button(b"EEEE", [3, 1]),
        ];
        let shown = display_only(learned, &[*b"DDDD"], &[icon([0, 2])]);
        let abilities: Vec<_> = shown.iter().map(|b| b.ability).collect();
        assert_eq!(abilities, vec![*b"BBBB", *b"EEEE"]);
    }

    #[test]
    fn no_learned_buttons_leave_the_card_unchanged() {
        assert!(display_only(Vec::new(), &[], &[icon([0, 0])]).is_empty());
    }
}
