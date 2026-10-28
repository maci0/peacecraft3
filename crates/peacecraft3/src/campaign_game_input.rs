//! Application side of the input state: the reasons the interface closes game
//! input, the option table read from the client, the control group sources, and
//! the command-log record of a group selection. The state itself is
//! `peacecraft3::game_input::GameInput`.
use std::{collections::BTreeMap, sync::Arc};

use bevy::prelude::*;
use pcx_play::{
    control_group_rules::{ControlGroupRules, ErrorKeys},
    map_startup_host::MapStartupHost,
    neutral_player::RuleInputBudget,
    options_defaults::{Defaults, DiscoveryBudget},
    player_command_log::PlayerCommand,
};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet, session_closers},
    campaign_control_groups::{
        CampaignControlGroups, ControlGroupSelected, ControlGroupSet, ErrorTexts, Source,
    },
    game_input::{Closer, GameInput, SavedScrollOptions, ScrollOptions},
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};

/// `[Errors]` section of `Units\CommandStrings.txt`.
const ERRORS_SECTION: &str = "Errors";
/// Frame names of the Gameplay options the saved values would come from
/// (`EscMenuOptionsPanel.fdf`).
const MOUSE_SCROLL_SLIDER: &str = "MouseScrollSlider";
const KEY_SCROLL_SLIDER: &str = "KeyScrollSlider";
const MOUSE_SCROLL_DISABLE_CHECKBOX: &str = "MouseScrollDisable";
// Application budgets, unrelated to the client's table sizes.
pub(crate) const DISCOVERY: DiscoveryBudget = DiscoveryBudget {
    image: RuleInputBudget {
        image_bytes: 32 << 20,
        profile_bytes: 128 << 10,
        sections: 96,
    },
    string_bytes: 4096,
    candidate_limit: 32,
    work_bytes: 64 << 20,
};

/// Orders the final gate refresh after the interface and before its readers.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GameInputSet;

pub(crate) fn install(app: &mut App) {
    peacecraft3::campaign_control_groups::install(app);
    app.init_resource::<OptionLoad>()
        // Camera keys read the gate the previous frame's interface left.
        .add_systems(Update, refresh_gate.before(CampaignCameraSet))
        .add_systems(
            Update,
            refresh_gate
                .in_set(GameInputSet)
                .after(CampaignCameraSet)
                .after(crate::campaign_hud::CampaignHudSet)
                .after(crate::campaign_script_dialogs::CampaignScriptDialogSet),
        )
        .add_systems(
            Update,
            (load_scroll_options, load_control_group_source)
                .after(ControlGroupSet)
                .after(GameInputSet),
        )
        .add_systems(Update, record_selection.after(ControlGroupSet))
        .configure_sets(
            Update,
            (ControlGroupSet, crate::campaign_input::CampaignInputSet).after(GameInputSet),
        );
}

/// Sets every reason game input is closed.
pub(crate) fn sync_gate(
    gate: &mut GameInput,
    view: &CampaignCamera,
    hud: &crate::campaign_hud::CampaignHud,
    dialogs: &crate::campaign_script_dialogs::CampaignScriptDialogs,
    host: Option<&MapStartupHost>,
) {
    let interface = [
        (Closer::QuestLog, hud.quest_log_open),
        (Closer::ScriptDialog, dialogs.input_blocked()),
    ];
    for (closer, closed) in session_closers(view, host).into_iter().chain(interface) {
        gate.set_closer(closer, closed);
    }
}

pub(crate) fn refresh_gate(
    view: Res<CampaignCamera>,
    hud: Res<crate::campaign_hud::CampaignHud>,
    dialogs: Res<crate::campaign_script_dialogs::CampaignScriptDialogs>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut gate: ResMut<GameInput>,
) {
    sync_gate(
        &mut gate,
        &view,
        &hud,
        &dialogs,
        session.host().map(|host| &*host),
    );
}

#[derive(Resource, Default)]
struct OptionLoad {
    attempted: bool,
    last_error: Option<String>,
}

/// A slider value the options panel reports, as the whole number the option holds.
fn whole(value: f32) -> Option<u32> {
    (value.is_finite() && value >= 0.0 && value.fract() == 0.0 && value <= u32::MAX as f32)
        .then_some(value as u32)
}

/// Saved Gameplay values from the options panel's applied state. The panel binds
/// only the audio controls today, so nothing is returned until it binds these.
fn saved_scroll_options(
    values: &BTreeMap<String, f32>,
    toggles: &BTreeMap<String, bool>,
) -> SavedScrollOptions {
    let slider = |name: &str| values.get(name).copied().and_then(whole);
    SavedScrollOptions {
        mouse_scroll: slider(MOUSE_SCROLL_SLIDER),
        key_scroll: slider(KEY_SCROLL_SLIDER),
        mouse_scroll_disable: toggles.get(MOUSE_SCROLL_DISABLE_CHECKBOX).copied(),
    }
}

fn load_scroll_options(
    mut load: ResMut<OptionLoad>,
    mut gate: ResMut<GameInput>,
    session: Res<MapScriptSession<MapStartupHost>>,
    launcher: Option<Res<crate::launch_gui::Launcher>>,
    navigation: Option<Res<crate::menu_screens::Navigation>>,
) {
    if load.attempted || session.session_witness().is_none() {
        return;
    }
    load.attempted = true;
    let install = launcher
        .as_deref()
        .and_then(crate::launch_gui::selected_install);
    let fallback = crate::menu_lighting::with_client_image(install.as_deref(), |source, image| {
        let origin = pcx_play::campaign_inputs::client_identity(image, source);
        let defaults = Defaults::discover(image, origin, DISCOVERY).map_err(|e| e.to_string())?;
        ScrollOptions::from_defaults(&defaults).map_err(|e| e.to_string())
    });
    match fallback {
        Ok(options) => {
            let saved = navigation
                .map(|nav| saved_scroll_options(&nav.applied.values, &nav.applied.toggles))
                .unwrap_or_default();
            gate.set_scroll_options(options.with_saved(saved));
        }
        Err(error) => load.last_error = Some(error),
    }
}

/// The text of each refusal, from the section the client names.
fn error_texts(strings: &str, keys: &ErrorKeys) -> Result<ErrorTexts, String> {
    let table = crate::menu_skin::SkinTable::parse(strings).map_err(|e| e.to_string())?;
    let text = |key: &str| -> Result<String, String> {
        let found: Vec<&str> = table
            .sections()
            .iter()
            .filter(|section| section.name.eq_ignore_ascii_case(ERRORS_SECTION))
            .flat_map(|section| &section.entries)
            .filter(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.trim())
            .collect();
        match found.as_slice() {
            [text] if !text.is_empty() => Ok((*text).to_owned()),
            _ => Err(format!(
                "[{ERRORS_SECTION}] {key}: missing, empty or ambiguous"
            )),
        }
    };
    Ok(ErrorTexts {
        invalid: text(&keys.invalid)?,
        empty: text(&keys.empty)?,
        non_unit: text(&keys.non_unit)?,
    })
}

#[derive(bevy::ecs::system::SystemParam)]
struct SourceInputs<'w> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
}

fn load_control_group_source(
    mut groups: ResMut<CampaignControlGroups>,
    inputs: SourceInputs,
    session: Res<MapScriptSession<MapStartupHost>>,
) {
    if !groups.needs_source() || session.session_witness().is_none() {
        return;
    }
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
                .as_ref()
                .zip(session.context())
                .is_some_and(|(admitted, active)| Arc::ptr_eq(admitted, active))
    }) else {
        return;
    };
    let Some(reader) = inputs.sources.for_receipts(&bindings.receipts) else {
        return;
    };
    let install = inputs
        .launcher
        .as_deref()
        .and_then(crate::launch_gui::selected_install);
    let source = crate::menu_lighting::with_client_image(install.as_deref(), |label, image| {
        let identity = pcx_play::campaign_inputs::client_identity(image, label);
        ControlGroupRules::read(image, identity)
    })
    .and_then(|rules| {
        let [path] = pcx_objects::editor::TxtRole::CommandStrings.files() else {
            return Err("CommandStrings is not one selected file".into());
        };
        let strings = crate::campaign_hud::upper_buttons::text(&reader, path, &mut Vec::new())?;
        let texts = error_texts(&strings, &rules.error_keys)?;
        Ok(Source { rules, texts })
    });
    groups.admit(source);
}

/// A group selection replaces the selection like a click does, so it is logged
/// as the same command.
fn record_selection(
    mut selected: MessageReader<ControlGroupSelected>,
    mut input: ResMut<crate::campaign_input::CampaignInput>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    for message in selected.read() {
        let clock = session
            .host()
            .and_then(|host| host.objects().timer_game_time());
        let command = PlayerCommand::Select {
            units: message.units.clone(),
            replace: true,
        };
        crate::campaign_input::record_command(&mut input, clock, command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> ErrorKeys {
        ErrorKeys {
            invalid: "Invalidcontrolgroup".into(),
            empty: "Emptycontrolgroup".into(),
            non_unit: "Nonunitincontrolgroup".into(),
        }
    }

    #[test]
    fn refusal_texts_come_from_the_errors_section_by_the_clients_key_names() {
        let strings = "[CmdMove]\nEmptycontrolgroup=wrong section\n[Errors]\r\nEmptycontrolgroup=nothing\r\nInvalidcontrolgroup = not yours \r\nnonunitincontrolgroup=no items\r\nNofood=a,b\r\n";
        let texts = error_texts(strings, &keys()).unwrap();
        assert_eq!(
            texts,
            ErrorTexts {
                invalid: "not yours".into(),
                empty: "nothing".into(),
                non_unit: "no items".into(),
            }
        );
    }

    #[test]
    fn a_missing_empty_or_repeated_text_refuses_by_key() {
        let complete =
            "[Errors]\nEmptycontrolgroup=a\nInvalidcontrolgroup=b\nNonunitincontrolgroup=c\n";
        assert!(error_texts(complete, &keys()).is_ok());
        for broken in [
            complete.replace("Invalidcontrolgroup=b", ""),
            complete.replace("Invalidcontrolgroup=b", "Invalidcontrolgroup="),
            format!("{complete}Emptycontrolgroup=again\n"),
        ] {
            assert!(error_texts(&broken, &keys()).is_err(), "{broken}");
        }
    }

    #[test]
    fn saved_values_need_whole_nonnegative_sliders() {
        let values = BTreeMap::from([
            (MOUSE_SCROLL_SLIDER.to_owned(), 80.0),
            (KEY_SCROLL_SLIDER.to_owned(), 12.5),
        ]);
        let toggles = BTreeMap::from([(MOUSE_SCROLL_DISABLE_CHECKBOX.to_owned(), true)]);
        assert_eq!(
            saved_scroll_options(&values, &toggles),
            SavedScrollOptions {
                mouse_scroll: Some(80),
                key_scroll: None,
                mouse_scroll_disable: Some(true),
            }
        );
        assert_eq!(
            saved_scroll_options(&BTreeMap::new(), &BTreeMap::new()),
            SavedScrollOptions::default()
        );
        assert_eq!(whole(-1.0), None);
        assert_eq!(whole(f32::NAN), None);
        assert_eq!(whole(0.0), Some(0));
    }
}
