//! Console HUD policy for the published campaign session.
//!
//! Once a session publishes, this reads `UI\FrameDef\UI\ConsoleUI.fdf` and
//! `UI\war3skins.txt` through the map's resolver and hands
//! [`crate::hud_render`] a [`HudSourcePolicy`] for the `ConsoleUI` frame. The
//! skin section comes from the local player's executed race preference constant
//! (`RACE_PREF_<NAME>` matched case-insensitively to a skin section), followed by
//! `[Default]`. An unset race refuses; no race is assumed.
#[path = "campaign_resource_bar.rs"]
mod resource_bar;
#[path = "campaign_upper_buttons.rs"]
pub(crate) mod upper_buttons;
pub(crate) use upper_buttons::{CampaignHudAction, CampaignHudSet};

use bevy::prelude::*;
use pcx_play::map_startup_host::MapStartupHost;
use peacecraft3::{
    browser_configuration::SelectedMapSources, map_requests::MapRequests,
    map_script_session::MapScriptSession,
};

use crate::hud_assets::{HudBudgets, HudMarkup, RootRole};
use crate::hud_render::HudSourcePolicy;
use crate::menu_skin::SkinTable;

const CONSOLE_FDF: &str = "UI\\FrameDef\\UI\\ConsoleUI.fdf";
const CONSOLE_FRAME: &str = "ConsoleUI";
const CONSOLE_ROLE: &str = "console";
const SKIN: &str = "UI\\war3skins.txt";
const DEFAULT_SECTION: &str = "Default";
const RACE_PREFIX: &str = "RACE_PREF_";
const IMAGE_SUFFIX: &str = ".blp";
/// Read limit for the FDF and skin text.
const MAX_TEXT_BYTES: usize = 1 << 20;
const BUDGETS: HudBudgets = HudBudgets {
    max_input_bytes: 16 << 20,
    max_image_input_bytes: 4 << 20,
    // ponytail: the console names no font yet; raise when text frames join.
    max_font_bytes: 0,
    max_font_aggregate_bytes: 0,
    max_image_decoded_bytes: 16 << 20,
    max_decoded_bytes: 64 << 20,
    max_retained_bytes: 64 << 20,
    max_transient_bytes: 96 << 20,
};

#[derive(Resource, Default)]
pub(crate) struct CampaignHud {
    attempted: bool,
    pub last_error: Option<String>,
    pub quest_log_open: bool,
    pub quest_escape_consumed: bool,
    pub pointer_consumed: bool,
    pub resource_error: Option<String>,
    pub resource_receipts: Vec<peacecraft3::map_binding::AssetReceipt>,
    pub control_receipts: Vec<peacecraft3::map_binding::AssetReceipt>,
}

pub(crate) fn install(app: &mut App) {
    upper_buttons::install(app);
    resource_bar::install(app);
    app.init_resource::<CampaignHud>()
        .add_systems(Update, produce_policy);
}

fn produce_policy(
    mut commands: Commands,
    mut hud: ResMut<CampaignHud>,
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    if hud.attempted || session.context().is_none() {
        return;
    }
    let Some(bindings) = requests.bindings.as_ref() else {
        return;
    };
    let Some(reader) = sources.for_receipts(&bindings.receipts) else {
        return;
    };
    let Some(host) = session.host() else {
        return;
    };
    hud.attempted = true;
    let race = host.objects().local_race_preference_constant();
    match policy(&reader, race.map_err(|e| e.to_string())) {
        Ok(policy) => {
            commands.insert_resource(policy);
        }
        Err(error) => hud.last_error = Some(error),
    }
}

fn read_text(reader: &pcx_assets::Resolver, member: &str) -> Result<String, String> {
    let asset = reader
        .resolve_bounded(member, MAX_TEXT_BYTES as u64)
        .map_err(|e| format!("{member}: {e}"))?;
    String::from_utf8(asset.bytes).map_err(|_| format!("{member} is not UTF-8"))
}

fn policy(
    reader: &pcx_assets::Resolver,
    race: Result<Option<String>, String>,
) -> Result<HudSourcePolicy, String> {
    let fdf = read_text(reader, CONSOLE_FDF)?;
    let skin_text = read_text(reader, SKIN)?;
    let skin = SkinTable::parse(&skin_text).map_err(|e| format!("{SKIN}: {e:?}"))?;
    let race_sections = race_sections(&skin, race?)?;
    let viewport = (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H);
    let suffixes = vec![IMAGE_SUFFIX.to_string()];
    let has = |path: &str| {
        pcx_assets::AssetPath::new(path).is_ok_and(|path| reader.paths().any(|p| *p == path))
    };
    let image_members = crate::hud_assets::texture_members(
        std::slice::from_ref(&fdf),
        &skin_text,
        &race_sections,
        &[CONSOLE_FRAME],
        viewport,
        &suffixes,
        &has,
    )
    .map_err(|e| format!("{CONSOLE_FDF}: {e:?}"))?;
    Ok(HudSourcePolicy {
        race_sections,
        roots: vec![RootRole {
            role: CONSOLE_ROLE.into(),
            frame: CONSOLE_FRAME.into(),
        }],
        viewport,
        budgets: BUDGETS,
        markup: HudMarkup::Plain,
        source_member: CONSOLE_FDF.into(),
        fdf_members: vec![CONSOLE_FDF.into()],
        skin_member: SKIN.into(),
        global_member: String::new(),
        decorate_suffixes: suffixes,
        image_members,
        font_members: Vec::new(),
        witness: None,
    })
}

/// The skin section named by the race constant, then `[Default]`.
pub(crate) fn race_sections(skin: &SkinTable, race: Option<String>) -> Result<Vec<String>, String> {
    let race = race.ok_or("local player race preference is unset")?;
    let name = race
        .strip_prefix(RACE_PREFIX)
        .ok_or_else(|| format!("race constant {race} lacks {RACE_PREFIX}"))?;
    let section = skin
        .sections()
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("{SKIN} has no section for {race}"))?;
    Ok(vec![section.name.clone(), DEFAULT_SECTION.into()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn race_constants_pick_their_skin_section_then_default() {
        let skin = SkinTable::parse("[Default]\nA=x\n[NightElf]\nA=y\n").unwrap();
        assert_eq!(
            race_sections(&skin, Some("RACE_PREF_NIGHTELF".into())).unwrap(),
            ["NightElf", "Default"]
        );
        assert!(race_sections(&skin, None).is_err());
        assert!(race_sections(&skin, Some("RACE_PREF_DEMON".into())).is_err());
        assert!(race_sections(&skin, Some("NIGHTELF".into())).is_err());
    }
}
