//! Original glue-screen navigation and settings. Screen geometry and vocabulary
//! come from supplied FDFs; display modes come from the operating environment.
use std::collections::BTreeMap;

use bevy::{prelude::*, ui::InteractionDisabled, ui_widgets::Button};

use crate::fdf::{Fdf, FdfError};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    #[default]
    Unselected,
    Video,
    Sound,
    Gameplay,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Main,
    Options(Page),
    SinglePlayer,
    Campaign,
    CustomCampaign,
    LoadSaved,
    Replay,
    Skirmish,
    Multiplayer,
}

impl Screen {
    pub fn file(self) -> &'static str {
        match self {
            Self::Main => crate::mainmenu::MAIN_MENU_FDF,
            Self::Options(_) => SCREEN_FILES[0],
            Self::SinglePlayer => SCREEN_FILES[1],
            Self::Campaign => SCREEN_FILES[2],
            Self::CustomCampaign => SCREEN_FILES[3],
            Self::LoadSaved => SCREEN_FILES[4],
            Self::Replay => SCREEN_FILES[5],
            Self::Skirmish => SCREEN_FILES[6],
            Self::Multiplayer => SCREEN_FILES[7],
        }
    }
    pub fn root(self) -> &'static str {
        match self {
            Self::Main => crate::glue_menu::CONTROL_LAYER,
            Self::Options(_) => "OptionsControlLayer",
            Self::SinglePlayer => "SinglePlayerMenu",
            Self::Campaign => "CampaignMenu",
            Self::CustomCampaign => "CustomCampaignMenu",
            Self::LoadSaved => "LoadSavedGameScreen",
            Self::Replay => "ViewReplayScreen",
            Self::Skirmish => "Skirmish",
            Self::Multiplayer => "LocalMultiplayerJoin",
        }
    }
    pub fn back(self) -> Self {
        match self {
            Self::Campaign
            | Self::CustomCampaign
            | Self::LoadSaved
            | Self::Replay
            | Self::Skirmish => Self::SinglePlayer,
            _ => Self::Main,
        }
    }
    pub fn includes(self, fdf: &Fdf, name: &str) -> Result<bool, FdfError> {
        let mut current = Some(name);
        let mut remaining = fdf.frames().len();
        while let Some(name) = current {
            if remaining == 0 {
                return Err(FdfError::Unsupported {
                    frame: name.into(),
                    property: "frame parent",
                    value: "cyclic parent chain".into(),
                });
            }
            remaining -= 1;
            let frame = fdf
                .frame(name)
                .ok_or_else(|| FdfError::UnknownFrame(name.into()))?;
            let excluded = match self {
                Self::Options(page) => match frame.name.as_str() {
                    "VideoPanel" => page != Page::Video,
                    "SoundPanel" => page != Page::Sound,
                    "GameplayPanel" => page != Page::Gameplay,
                    _ => false,
                },
                // Profile editing is opened explicitly, rather than overlapping
                // the ordinary single-player navigation on entry.
                Self::SinglePlayer => frame.name == "ProfilePanel",
                _ => false,
            };
            if excluded {
                return Ok(false);
            }
            current = frame.parent.as_deref();
        }
        Ok(true)
    }
}

/// Only these roots are optional additions to the original main-menu import.
/// Values inside them are never copied into source.
pub const SCREEN_FILES: &[&str] = &[
    r"UI\FrameDef\Glue\OptionsMenu.fdf",
    r"UI\FrameDef\Glue\SinglePlayerMenu.fdf",
    r"UI\FrameDef\Glue\CampaignMenu.fdf",
    r"UI\FrameDef\Glue\CustomCampaignMenu.fdf",
    r"UI\FrameDef\Glue\LoadSavedGameScreen.fdf",
    r"UI\FrameDef\Glue\ViewReplayScreen.fdf",
    r"UI\FrameDef\Glue\Skirmish.fdf",
    r"UI\FrameDef\Glue\LocalMultiplayerJoin.fdf",
];

/// Explicit include edges from the supplied file, not an inferred template list.
pub fn includes(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim().strip_prefix("IncludeFile")?.trim();
            line.strip_prefix('"')?
                .split('"')
                .next()
                .map(str::to_string)
        })
        .collect()
}

/// One offered row. Render extents and qualified client rows share this value,
/// and `render_extent_only` keeps those two claims distinct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfferedResolution {
    pub label: String,
    pub size: (u32, u32),
    pub depth: Option<u32>,
    pub refresh_millihertz: Option<u32>,
    pub adapter: u32,
    pub mode_index: u32,
    pub kind: i32,
    pub flag: u8,
    /// Pass index for a selector-2 popup row. Enumeration rows and render sizes
    /// leave this empty. It is metadata for the current listing pass.
    pub monitor_ordinal: Option<u32>,
    /// Opaque host monitor identity for a popup row. This is not a client
    /// protocol field and it is distinct from `adapter` and `monitor_ordinal`.
    pub monitor: Option<Entity>,
    pub render_extent_only: bool,
}

impl OfferedResolution {
    pub fn render_size(width: u32, height: u32) -> Self {
        Self {
            label: format!("{width} x {height}"),
            size: (width, height),
            depth: None,
            refresh_millihertz: None,
            adapter: 0,
            mode_index: 0,
            kind: 0,
            flag: 0,
            monitor_ordinal: None,
            monitor: None,
            render_extent_only: true,
        }
    }

    pub fn same_mode(&self, other: &Self) -> bool {
        self.size == other.size
            && self.depth == other.depth
            && self.refresh_millihertz == other.refresh_millihertz
            && self.adapter == other.adapter
            && self.mode_index == other.mode_index
            && self.kind == other.kind
            && self.flag == other.flag
            && self.monitor_ordinal == other.monitor_ordinal
            && self.monitor == other.monitor
            && self.render_extent_only == other.render_extent_only
    }
}

pub(crate) const POPUP_WORK_AREA_NOTICE: &str =
    "Windowed work-area sizes are unavailable on this host. Apply changes the window render size.";

pub(crate) fn resolution_popup_title(rows: &[OfferedResolution]) -> &'static str {
    if rows.iter().any(|row| row.monitor.is_some()) {
        POPUP_WORK_AREA_NOTICE
    } else if rows.iter().any(|row| !row.render_extent_only) {
        "Sizes from the qualified client and this monitor. Apply changes the window render size."
    } else if rows.is_empty() {
        "No display sizes are available."
    } else {
        "Render sizes, not exclusive display modes."
    }
}

#[derive(Resource, Default)]
pub struct Navigation {
    /// Currently displayed control owner; requests do not mutate it before admission.
    pub screen: Screen,
    pub(crate) requested: Option<Screen>,
    pub(crate) transition_active: bool,
    pub built: Option<Screen>,
    pub notice: String,
    /// Passive capability guidance shown while inspecting the resolution control.
    pub(crate) display_notice: String,
    pub draft: Settings,
    pub applied: Settings,
    pub resolutions: Vec<OfferedResolution>,
    pub selected_offer: Option<OfferedResolution>,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub resolution: Option<(u32, u32)>,
    /// Authored slider values, not normalized or substituted defaults.
    pub values: BTreeMap<String, f32>,
    pub toggles: BTreeMap<String, bool>,
}

impl Navigation {
    fn restore_draft(&mut self) {
        self.draft = self.applied.clone();
        self.selected_offer = None;
    }

    pub(crate) fn offers_size(&self, size: (u32, u32)) -> bool {
        self.resolutions.iter().any(|row| row.size == size)
    }

    pub(crate) fn resolution_highlighted(&self, row: &OfferedResolution) -> bool {
        if let Some(selected) = &self.selected_offer
            && self.draft.resolution == Some(selected.size)
        {
            return selected.same_mode(row);
        }
        self.draft.resolution == Some(row.size)
            && self
                .resolutions
                .iter()
                .find(|candidate| candidate.size == row.size)
                .is_some_and(|first| first.same_mode(row))
    }

    pub fn open(&mut self, screen: Screen) {
        if self.transition_active || self.requested.is_some() {
            return;
        }
        if matches!(
            (self.screen, screen),
            (Screen::Main, Screen::SinglePlayer | Screen::Options(_))
                | (Screen::SinglePlayer | Screen::Options(_), Screen::Main)
        ) {
            if matches!(screen, Screen::Options(_)) {
                self.restore_draft();
            }
            self.requested = Some(screen);
            self.notice.clear();
            return;
        }
        if matches!(screen, Screen::Options(_)) && !matches!(self.screen, Screen::Options(_)) {
            self.restore_draft();
        }
        self.screen = screen;
        self.notice.clear();
    }
    pub(crate) fn commit_panel_owner(&mut self, screen: Screen) {
        self.screen = screen;
        self.notice.clear();
    }

    pub(crate) fn reset_panels(&mut self) {
        self.requested = None;
        self.transition_active = false;
        self.screen = Screen::Main;
        self.built = None;
        self.notice.clear();
    }

    pub fn action(&mut self, name: &str) -> bool {
        // No queue or interruption: a transition consumes one request; further
        // menu actions are ignored until its incoming phase completes.
        if self.transition_active || self.requested.is_some() {
            return true;
        }

        let next = match name {
            "OptionsButton" => Screen::Options(Page::Unselected),
            "SinglePlayerButton" => Screen::SinglePlayer,
            "MultiPlayerButton" | "LocalAreaNetworkButton" => Screen::Multiplayer,
            "CampaignButton" => Screen::Campaign,
            "CustomCampaignButton" => Screen::CustomCampaign,
            "LoadSavedButton" => Screen::LoadSaved,
            "ViewReplayButton" => Screen::Replay,
            "SkirmishButton" => Screen::Skirmish,
            "GameplayButton" if matches!(self.screen, Screen::Options(_)) => {
                Screen::Options(Page::Gameplay)
            }
            "VideoButton" if matches!(self.screen, Screen::Options(_)) => {
                Screen::Options(Page::Video)
            }
            "SoundButton" if matches!(self.screen, Screen::Options(_)) => {
                Screen::Options(Page::Sound)
            }
            "CancelButton" | "BackButton" => {
                self.restore_draft();
                self.screen.back()
            }
            _ => return false,
        };
        self.open(next);
        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Widget {
    Popup {
        choices: Vec<String>,
    },
    Slider {
        min: f32,
        max: f32,
        initial: f32,
        step: f32,
    },
    Checkbox,
}

pub fn widget(fdf: &Fdf, name: &str) -> Result<Option<Widget>, FdfError> {
    let Some(frame) = fdf.frame(name) else {
        return Err(FdfError::UnknownFrame(name.into()));
    };
    Ok(match frame.kind.as_str() {
        "SLIDER" => {
            let number = |key: &'static str| -> Result<f32, FdfError> {
                match fdf.numbers(name, key)?.as_deref() {
                    Some([v]) if v.is_finite() => Ok(*v),
                    _ => Err(FdfError::Missing {
                        frame: name.into(),
                        property: key,
                    }),
                }
            };
            let (min, max, initial, step) = (
                number("SliderMinValue")?,
                number("SliderMaxValue")?,
                number("SliderInitialValue")?,
                number("SliderStepSize")?,
            );
            if min >= max
                || !(max - min).is_finite()
                || step <= 0.0
                || !(min..=max).contains(&initial)
            {
                return Err(FdfError::Unsupported {
                    frame: name.into(),
                    property: "slider range",
                    value: format!("{min}..{max}, initial {initial}, step {step}"),
                });
            }
            Some(Widget::Slider {
                min,
                max,
                initial,
                step,
            })
        }
        "POPUPMENU" | "GLUEPOPUPMENU" => {
            let choices = fdf
                .string(name, "PopupMenuFrame")
                .and_then(|n| fdf.frame(&n))
                .map(|f| {
                    f.props
                        .records
                        .iter()
                        .filter(|(k, _)| k == "MenuItem")
                        .enumerate()
                        .map(|(index, (_, value))| {
                            let item = value
                                .trim()
                                .strip_prefix('"')
                                .and_then(|tail| tail.split_once('"'));
                            if let Some((label, tail)) = item
                                && !label.trim().is_empty()
                                && tail.trim().strip_prefix(',').is_some_and(|operand| {
                                    operand.trim().parse::<f32>().is_ok_and(f32::is_finite)
                                })
                            {
                                Ok(label.to_owned())
                            } else {
                                Err(FdfError::Unsupported {
                                    frame: f.name.clone(),
                                    property: "MenuItem",
                                    value: format!("item {index}: {value}"),
                                })
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            Some(Widget::Popup { choices })
        }
        "CHECKBOX" | "GLUECHECKBOX" => Some(Widget::Checkbox),
        _ => None,
    })
}

pub fn supported(name: &str) -> bool {
    matches!(
        name,
        "ResolutionMenu"
            | "SoundVolumeSlider"
            | "MusicVolumeSlider"
            | "SoundCheckBox"
            | "MusicCheckBox"
    )
}

#[derive(Component)]
pub struct ScreenPosition(pub String);

/// Dynamic campaign controls have their own action consumer, not screen navigation.
#[derive(Component)]
pub(crate) struct CampaignAction;

#[derive(Component)]
pub struct SettingControl {
    pub name: String,
    pub widget: Widget,
}

#[derive(Component)]
pub struct SettingValue(pub Entity);

#[derive(Component)]
pub struct LabelFontParent(pub String);

/// Follow authored tab links before retaining the file order of other controls.
pub fn focus_order(fdf: &Fdf, visible: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut next = visible
        .iter()
        .find(|n| fdf.flag(n, "TabFocusDefault"))
        .cloned();
    while let Some(name) = next {
        if !visible.contains(&name) || result.contains(&name) {
            break;
        }
        next = fdf.string(&name, "TabFocusNext");
        result.push(name);
    }
    for name in visible {
        if !result.contains(name) {
            result.push(name.clone());
        }
    }
    result
}

pub fn spawn_widget(commands: &mut Commands, node: Entity, name: &str, widget: &Widget, fdf: &Fdf) {
    commands.entity(node).insert((
        Button,
        bevy::picking::hover::Hovered::default(),
        SettingControl {
            name: name.into(),
            widget: widget.clone(),
        },
        crate::glue_menu::GlueButton {
            frame: name.into(),
            shortcut: None,
            pushed_text_offset: None,
        },
    ));
    if !supported(name) {
        commands.entity(node).insert(InteractionDisabled);
    }
    // The source's label style supplies the face, colour and font size. Popup
    // content is dynamic (display modes or authored MenuItem records).
    let text_frame = match widget {
        Widget::Popup { .. } => fdf
            .string(name, "PopupTitleFrame")
            .and_then(|n| fdf.button_text(&n)),
        Widget::Slider { .. } => None,
        Widget::Checkbox => None,
    };
    if let Some(frame) = text_frame {
        let wrapper = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                // Foreground values stay above subsequently attached control art.
                ZIndex(1),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(node).add_child(wrapper);
        let label = commands
            .spawn((
                Text::new(""),
                SettingValue(node),
                crate::glue_menu::GlueLabel {
                    frame,
                    button: node,
                },
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(wrapper).add_child(label);
        if matches!(widget, Widget::Popup { .. })
            && let Some(parent) = fdf.string(name, "PopupTitleFrame")
        {
            commands.entity(label).insert(LabelFontParent(parent));
        }
    }
}

pub fn prime_text_sizes(fdf: &mut Fdf) {
    // Zero width is a pre-layout placeholder, replaced by shaped text extents.
    // It does not stand in for a measured or authored text width.
    let sizes: Vec<_> = fdf
        .frames()
        .iter()
        .filter(|f| f.kind == "TEXT")
        .filter_map(|f| {
            crate::menu_text::render_style(fdf, &f.name)
                .ok()
                .map(|s| (f.name.clone(), s.height))
        })
        .collect();
    for (name, height) in sizes {
        fdf.measure_text(&name, (0.0, height));
    }
}

pub fn slider_value(min: f32, max: f32, step: f32, fraction: f32) -> f32 {
    (min + ((fraction.clamp(0.0, 1.0) * (max - min)) / step).round() * step).clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn popup_fixture(items: &str) -> Fdf {
        Fdf::parse(&format!(
            "Frame \"MENU\" \"ChoiceItems\" {{\n{items}\n}}\n\
             Frame \"POPUPMENU\" \"ChoiceControl\" {{\n\
             PopupMenuFrame \"ChoiceItems\",\n}}\n"
        ))
        .unwrap()
    }

    #[test]
    fn popup_items_keep_authored_order_without_using_the_opaque_operand_as_an_id() {
        let fdf = popup_fixture("MenuItem \"ZEBRA\", 7.25,\nMenuItem \"ALPHA\", 7.25,");
        let Some(Widget::Popup { choices }) = widget(&fdf, "ChoiceControl").unwrap() else {
            panic!("expected source popup");
        };
        assert_eq!(choices, ["ZEBRA", "ALPHA"]);
        let Some(Widget::Popup { choices }) = widget(&popup_fixture(""), "ChoiceControl").unwrap()
        else {
            panic!("expected empty dynamic popup");
        };
        assert!(choices.is_empty());
    }

    #[test]
    fn malformed_popup_items_refuse_the_list_instead_of_shifting_its_indexes() {
        for item in [
            "MenuItem UNQUOTED, 1,",
            "MenuItem \"\", 1,",
            "MenuItem \"UNCLOSED, 1,",
            "MenuItem \"KEY\",",
            "MenuItem \"KEY\", invalid,",
            "MenuItem \"KEY\", NaN,",
            "MenuItem \"KEY\", inf,",
            "MenuItem \"KEY\", 1, 2,",
            "MenuItem \"KEY\" suffix, 1,",
        ] {
            for items in [
                format!("{item}\nMenuItem \"VALID\", 1,"),
                format!("MenuItem \"VALID\", 1,\n{item}"),
            ] {
                assert!(
                    matches!(
                        widget(&popup_fixture(&items), "ChoiceControl"),
                        Err(FdfError::Unsupported {
                            frame,
                            property: "MenuItem",
                            ..
                        }) if frame == "ChoiceItems"
                    ),
                    "must refuse the entire popup: {items}"
                );
            }
        }
    }

    #[test]
    #[ignore = "requires PEACECRAFT3_INSTALL with selected original Options FDFs"]
    fn original_options_popup_items_preserve_source_cardinality() {
        let install = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_INSTALL").expect("explicit original install required"),
        );
        let (fdf, _) =
            crate::launch_gui::read_screen_fdf(Some(&install), Screen::Options(Page::Video))
                .unwrap();
        let mut checked = 0;
        for frame in fdf
            .frames()
            .iter()
            .filter(|frame| matches!(frame.kind.as_str(), "POPUPMENU" | "GLUEPOPUPMENU"))
        {
            let Some(Widget::Popup { choices }) = widget(&fdf, &frame.name).unwrap() else {
                panic!("expected source popup {}", frame.name);
            };
            let count = fdf
                .string(&frame.name, "PopupMenuFrame")
                .and_then(|name| fdf.frame(&name))
                .map_or(0, |menu| {
                    menu.props
                        .records
                        .iter()
                        .filter(|(key, _)| key == "MenuItem")
                        .count()
                });
            assert_eq!(choices.len(), count, "{}", frame.name);
            checked += 1;
        }
        assert!(checked > 0, "selected Options input must supply popups");
    }

    #[test]
    fn popup_values_have_a_foreground_layer_above_control_art() {
        let fdf = Fdf::parse(
            r#"
Frame "TEXT" "Value" {
}
Frame "TEXTBUTTON" "Title" {
    ButtonText "Value",
}
Frame "POPUPMENU" "ResolutionMenu" {
    PopupTitleFrame "Title",
}
"#,
        )
        .unwrap();
        let mut world = World::new();
        let owner = world.spawn_empty().id();
        spawn_widget(
            &mut world.commands(),
            owner,
            "ResolutionMenu",
            &Widget::Popup { choices: vec![] },
            &fdf,
        );
        world.flush();
        let wrapper = world.get::<Children>(owner).unwrap()[0];
        assert_eq!(world.get::<ZIndex>(wrapper).unwrap().0, 1);
        let value = world.get::<Children>(wrapper).unwrap()[0];
        assert_eq!(world.get::<SettingValue>(value).unwrap().0, owner);
    }

    #[test]
    fn cyclic_parent_names_return_an_error_instead_of_hanging() {
        let fdf = Fdf::parse(
            r#"Frame "FRAME" "OptionsControlLayer" {
Frame "FRAME" "A" {
Frame "FRAME" "B" {
Frame "FRAME" "A" {
}
}
}
}
"#,
        )
        .unwrap();
        assert!(matches!(
            Screen::Options(Page::Video).includes(&fdf, "A"),
            Err(FdfError::Unsupported {
                property: "frame parent",
                ..
            })
        ));
    }

    #[test]
    fn visible_and_hidden_page_ancestry_still_follow_the_source() {
        let fdf = Fdf::parse(
            r#"Frame "FRAME" "OptionsControlLayer" {
Frame "FRAME" "SoundPanel" {
Frame "FRAME" "A" {
}
}
}
"#,
        )
        .unwrap();
        assert!(Screen::Options(Page::Sound).includes(&fdf, "A").unwrap());
        assert!(!Screen::Options(Page::Video).includes(&fdf, "A").unwrap());
    }

    #[test]
    fn original_options_do_not_route_to_launcher() {
        let mut n = Navigation::default();
        assert!(n.action("OptionsButton"));
        assert_eq!(n.screen, Screen::Main);
        assert_eq!(n.requested.take(), Some(Screen::Options(Page::Unselected)));
        n.commit_panel_owner(Screen::Options(Page::Unselected));
        assert!(n.action("SoundButton"));
        assert_eq!(n.screen, Screen::Options(Page::Sound));
        assert!(n.action("CancelButton"));
        assert_eq!(n.screen, Screen::Options(Page::Sound));
        assert_eq!(n.requested, Some(Screen::Main));
    }
    #[test]
    fn options_owner_request_preserves_selected_page_and_tab_draft() {
        let mut nav = Navigation::default();
        nav.applied.values.insert("synthetic".into(), 19.0);
        assert!(nav.action("OptionsButton"));
        assert_eq!(nav.draft, nav.applied);
        assert_eq!(
            nav.requested.take(),
            Some(Screen::Options(Page::Unselected))
        );
        nav.commit_panel_owner(Screen::Options(Page::Unselected));
        nav.draft.values.insert("synthetic".into(), 23.0);
        assert!(nav.action("VideoButton"));
        assert_eq!(nav.screen, Screen::Options(Page::Video));
        assert!(nav.requested.is_none());
        assert_eq!(nav.draft.values["synthetic"], 23.0);
        assert!(nav.action("CancelButton"));
        assert_eq!(nav.requested, Some(Screen::Main));
        assert_eq!(nav.draft, nav.applied);
    }

    #[test]
    fn options_landing_keeps_source_focus_without_selecting_a_settings_body() {
        for focused in ["GameplayButton", "SoundButton"] {
            let mut source = String::from("Frame \"FRAME\" \"OptionsControlLayer\" {\n");
            for (page, body) in [
                ("GameplayPanel", "GameplayValue"),
                ("VideoPanel", "VideoValue"),
                ("SoundPanel", "SoundValue"),
            ] {
                source.push_str(&format!(
                    "Frame \"FRAME\" \"{page}\" {{\nFrame \"TEXT\" \"{body}\" {{\n}}\n}}\n"
                ));
            }
            let controls = [
                "GameplayButton",
                "VideoButton",
                "SoundButton",
                "OKButton",
                "CancelButton",
            ];
            for name in controls {
                source.push_str(&format!("Frame \"BUTTON\" \"{name}\" {{\n"));
                if name == focused {
                    source.push_str("TabFocusDefault,\n");
                }
                source.push_str("}\n");
            }
            source.push_str("}\n");
            let fdf = Fdf::parse(&source).unwrap();
            let mut nav = Navigation::default();
            assert!(nav.action("OptionsButton"));
            assert_eq!(nav.screen, Screen::Main);
            let landing = nav.requested.take().unwrap();
            for body in ["GameplayValue", "VideoValue", "SoundValue"] {
                assert!(
                    !landing.includes(&fdf, body).unwrap(),
                    "{body} was selected by {focused}"
                );
            }
            for control in controls {
                assert!(landing.includes(&fdf, control).unwrap());
            }
            let visible = controls.map(str::to_string);
            assert_eq!(focus_order(&fdf, &visible)[0], focused);
            nav.commit_panel_owner(landing);
            nav.draft.values.insert("synthetic".into(), 23.0);
            for (button, selected) in [
                ("GameplayButton", "GameplayValue"),
                ("VideoButton", "VideoValue"),
                ("SoundButton", "SoundValue"),
            ] {
                assert!(nav.action(button));
                assert!(nav.requested.is_none());
                assert_eq!(nav.draft.values["synthetic"], 23.0);
                for body in ["GameplayValue", "VideoValue", "SoundValue"] {
                    assert_eq!(nav.screen.includes(&fdf, body).unwrap(), body == selected);
                }
            }
            assert!(nav.action("CancelButton"));
            assert_eq!(nav.draft, nav.applied);
            assert_eq!(nav.requested, Some(Screen::Main));
            nav.reset_panels();
            assert_eq!(nav.screen, Screen::Main);
            assert!(nav.requested.is_none());
        }
    }
    #[test]
    fn local_network_entry_opens_its_screen_and_returns_to_main() {
        let mut nav = Navigation::default();
        assert!(nav.action("LocalAreaNetworkButton"));
        assert_eq!(nav.screen, Screen::Multiplayer);
        assert!(nav.action("CancelButton"));
        assert_eq!(nav.screen, Screen::Main);
    }
    #[test]
    fn submenus_return_to_their_parent() {
        let mut n = Navigation::default();
        n.action("SinglePlayerButton");
        assert_eq!(n.screen, Screen::Main);
        assert_eq!(n.requested.take(), Some(Screen::SinglePlayer));
        n.commit_panel_owner(Screen::SinglePlayer);
        n.action("CampaignButton");
        n.action("BackButton");
        assert_eq!(n.screen, Screen::SinglePlayer);
        n.action("CancelButton");
        assert_eq!(n.screen, Screen::SinglePlayer);
        assert_eq!(n.requested, Some(Screen::Main));
    }
    #[test]
    fn panel_navigation_requests_are_transactional_and_ignore_interruptions() {
        let mut n = Navigation::default();
        n.open(Screen::SinglePlayer);
        assert_eq!(n.screen, Screen::Main);
        n.action("OptionsButton");
        assert_eq!(n.requested, Some(Screen::SinglePlayer));
        n.requested = None;
        n.transition_active = true;
        n.open(Screen::Options(Page::Sound));
        assert_eq!(n.screen, Screen::Main);
        n.commit_panel_owner(Screen::SinglePlayer);
        assert!(n.action("BackButton"));
        assert!(n.requested.is_none());
        n.reset_panels();
        assert_eq!(n.screen, Screen::Main);
        assert!(!n.transition_active);
        assert!(n.requested.is_none());
    }
    #[test]
    fn cancel_discards_draft_settings() {
        let mut n = Navigation::default();
        n.open(Screen::Options(Page::Sound));
        assert_eq!(n.requested.take(), Some(Screen::Options(Page::Sound)));
        n.commit_panel_owner(Screen::Options(Page::Sound));
        n.draft.values.insert("MusicVolumeSlider".into(), 17.0);
        n.action("CancelButton");
        assert_eq!(n.draft, n.applied);
    }
    #[test]
    fn focus_chain_uses_the_authored_links_before_file_order() {
        let f=Fdf::parse("Frame \"BUTTON\" \"A\" {\n TabFocusNext \"C\",\n}\nFrame \"BUTTON\" \"B\" {\n TabFocusDefault,\n TabFocusNext \"A\",\n}\nFrame \"BUTTON\" \"C\" {\n TabFocusNext \"B\",\n}\n").unwrap();
        assert_eq!(
            focus_order(&f, &["A".into(), "B".into(), "C".into()]),
            ["B", "A", "C"]
        );
        assert_eq!(focus_order(&f, &["A".into(), "B".into()]), ["B", "A"]);
    }
    #[test]
    fn slider_steps_are_relative_to_authored_minimum() {
        assert_eq!(slider_value(7.0, 31.0, 3.0, 0.5), 19.0);
        assert_eq!(slider_value(7.0, 31.0, 3.0, 2.0), 31.0);
    }
}
