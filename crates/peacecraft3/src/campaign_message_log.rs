//! Message log dialog from `UI\FrameDef\UI\LogDialog.fdf`: the text the script
//! showed the local player, newest last, opened by the bar's Log button or the
//! `KEY_LOG` key and closed by OK or Escape.
//!
//! Recreation choices: entries are the `DisplayTimedTextToPlayer` texts recorded
//! by `MessageHistory` (whether the client logs script text is not traced);
//! `TextAreaMaxLines` caps entries rather than wrapped lines; the dialog is
//! centered; the FDF scrollbar is not built (wheel scrolling only); the view
//! jumps to the newest entry whenever one arrives.
use super::{
    Action, CampaignControlsRoot, DialogButton, Source, UpperControls, dialog_button, global,
    label, paint_backdrop, text_area, text_area_number,
};
use crate::{fdf::Fdf, glue_menu, menu_text};
use bevy::prelude::*;

pub(super) const FDF: &str = "UI\\FrameDef\\UI\\LogDialog.fdf";
const DIALOG: &str = "LogDialog";
const BACKDROP: &str = "LogBackdrop";
const TITLE: &str = "LogTitle";
const OK_BUTTON: &str = "LogOkButton";
const AREA_BACKDROP: &str = "LogAreaBackdrop";
const AREA: &str = "LogArea";
const KEY: &str = "KEY_LOG";

pub(super) struct Art {
    pub(super) backdrop: glue_menu::Backdrop,
    pub(super) area_backdrop: glue_menu::Backdrop,
    pub(super) ok: glue_menu::Button,
    pub(super) ok_material: Option<Handle<glue_menu::AdditiveImage>>,
    /// `KEY_LOG` from the selected `GlobalStrings.fdf`.
    pub(super) key: KeyCode,
    /// `TextAreaMaxLines` of the log area.
    pub(super) max_entries: usize,
}
impl Art {
    /// Resolves the dialog's geometry, art and strings from the parsed FDFs.
    pub(super) fn load(fdf: &mut Fdf, globals: &str) -> Result<Self, String> {
        let style = menu_text::render_style(fdf, TITLE)?;
        let width = fdf.width(DIALOG).map_err(|e| e.to_string())?;
        // The title has no authored width; it spans the dialog and centers inside.
        fdf.measure_text(TITLE, (width, style.height));
        let view = view(fdf)?;
        for name in [TITLE, OK_BUTTON, AREA_BACKDROP, AREA] {
            fdf.rect(name, view).map_err(|e| e.to_string())?;
        }
        text_area(fdf, AREA)?;
        let lines = text_area_number(fdf, AREA, "TextAreaMaxLines")?;
        if lines < 1. || lines.fract() != 0. {
            return Err(format!("{AREA}: invalid TextAreaMaxLines"));
        }
        let backdrop = glue_menu::backdrop(fdf, BACKDROP)
            .map_err(|e| e.to_string())?
            .ok_or("missing message log backdrop")?;
        let area_backdrop = glue_menu::backdrop(fdf, AREA_BACKDROP)
            .map_err(|e| e.to_string())?
            .ok_or("missing message log area backdrop")?;
        let ok = glue_menu::button(fdf, OK_BUTTON).map_err(|e| e.to_string())?;
        if ok.normal.is_none()
            || ok.pushed.is_none()
            || ok.disabled.is_none()
            || ok.pushed_text_offset.is_none()
        {
            return Err("message log OK button lacks authored state art/offset".into());
        }
        let ok_text = ok.text.as_deref().ok_or("message log OK text missing")?;
        for frame in [TITLE, ok_text] {
            let key = fdf
                .text_value(frame)
                .ok_or_else(|| format!("{frame}: missing Text"))?;
            let value = menu_text::global_string(globals, &key)
                .ok_or_else(|| format!("missing global string {key}"))?;
            menu_text::Label::parse(&value).map_err(|e| e.to_string())?;
        }
        Ok(Self {
            backdrop,
            area_backdrop,
            ok,
            ok_material: None,
            key: peacecraft3::campaign_camera::authored_function_key(globals, KEY)?,
            max_entries: lines as usize,
        })
    }
    /// Frames whose font the dialog draws.
    pub(super) fn font_styles(&self) -> Vec<String> {
        [TITLE, AREA]
            .map(String::from)
            .into_iter()
            .chain(self.ok.text.clone())
            .collect()
    }
    /// Every backdrop the dialog paints, for image decoding.
    pub(super) fn backdrops(&self) -> impl Iterator<Item = &glue_menu::Backdrop> {
        [&self.backdrop, &self.area_backdrop].into_iter().chain(
            [
                self.ok.normal.as_ref(),
                self.ok.pushed.as_ref(),
                self.ok.disabled.as_ref(),
            ]
            .into_iter()
            .flatten(),
        )
    }
}

/// The dialog entity on screen and the history revision it shows.
#[derive(Default)]
pub(super) struct Shown {
    pub(super) entity: Option<Entity>,
    revision: Option<u64>,
}

fn view(fdf: &Fdf) -> Result<(f32, f32), String> {
    Ok((
        fdf.width(DIALOG).map_err(|e| e.to_string())?,
        fdf.height(DIALOG).map_err(|e| e.to_string())?,
    ))
}

fn place(rect: crate::fdf::Rect, (width, height): (f32, f32)) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(rect.left / width * 100.),
        top: Val::Percent((height - rect.top()) / height * 100.),
        width: Val::Percent(rect.width / width * 100.),
        height: Val::Percent(rect.height / height * 100.),
        ..default()
    }
}

/// Opens, refreshes or removes the dialog to match `UpperControls`, and binds the
/// history cap to the authored area once the art is loaded.
pub(super) fn sync(
    commands: &mut Commands,
    state: &mut UpperControls,
    history: &mut crate::campaign_messages::MessageHistory,
    images: &Assets<Image>,
    materials: &mut Assets<glue_menu::BackdropImage>,
) {
    let UpperControls {
        assets,
        messages,
        messages_open,
        last_error,
        ..
    } = state;
    let Some(source) = assets.as_mut() else {
        return;
    };
    history.limit_to(source.message_log.max_entries);
    if !*messages_open {
        if let Some(entity) = messages.entity.take() {
            commands.entity(entity).despawn();
        }
        messages.revision = None;
        return;
    }
    if messages.revision == Some(history.revision()) {
        return;
    }
    // The bar shows one dialog at a time, so these are the previous dialog's.
    let previous = std::mem::take(&mut source.materials);
    let lines: Vec<&str> = history.lines().collect();
    match dialog(commands, source, &lines, images, materials) {
        Ok(root) => {
            for handle in previous {
                materials.remove(handle.id());
            }
            if let Some(old) = messages.entity.replace(root) {
                commands.entity(old).despawn();
            }
        }
        Err(error) => {
            for handle in source.materials.drain(..) {
                materials.remove(handle.id());
            }
            source.materials = previous;
            *last_error = Some(error);
        }
    }
    messages.revision = Some(history.revision());
}

fn justify(fdf: &Fdf, frame: &str) -> Result<JustifyContent, String> {
    match fdf.string(frame, "FontJustificationH").as_deref() {
        Some("JUSTIFYLEFT") => Ok(JustifyContent::FlexStart),
        Some("JUSTIFYCENTER") => Ok(JustifyContent::Center),
        Some("JUSTIFYRIGHT") => Ok(JustifyContent::FlexEnd),
        other => Err(format!("{frame}: unsupported FontJustificationH {other:?}")),
    }
}

fn dialog(
    commands: &mut Commands,
    source: &mut Source,
    lines: &[&str],
    images: &Assets<Image>,
    materials: &mut Assets<glue_menu::BackdropImage>,
) -> Result<Entity, String> {
    for line in lines {
        let parsed = crate::rich_text::parse(line).map_err(|e| e.to_string())?;
        if !parsed.unsupported.is_empty() {
            return Err("message log entry contains unsupported substitutions".into());
        }
    }
    let view = view(&source.fdf)?;
    let (width, height) = view;
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent((1. - width / crate::mainmenu::FRAME_W) * 50.),
                top: Val::Percent((1. - height / crate::mainmenu::FRAME_H) * 50.),
                width: Val::Percent(width / crate::mainmenu::FRAME_W * 100.),
                height: Val::Percent(height / crate::mainmenu::FRAME_H * 100.),
                ..default()
            },
            Name::new(DIALOG),
            bevy::ui::GlobalZIndex(20),
            CampaignControlsRoot,
            peacecraft3::campaign_presentation::CampaignHud,
        ))
        .id();
    let backdrop = source.message_log.backdrop.clone();
    paint_backdrop(
        commands,
        root,
        source,
        &backdrop,
        Vec2::new(width, height),
        images,
        materials,
    )?;

    let rect = source.fdf.rect(TITLE, view).map_err(|e| e.to_string())?;
    let mut node = place(rect, view);
    node.align_items = AlignItems::Center;
    node.justify_content = justify(&source.fdf, TITLE)?;
    let title = commands
        .spawn((node, Name::new(TITLE), Pickable::IGNORE))
        .id();
    commands.entity(root).add_child(title);
    let key = source
        .fdf
        .text_value(TITLE)
        .ok_or("message log title missing")?;
    label(commands, title, global(source, &key)?, source, TITLE)?;

    let rect = source
        .fdf
        .rect(AREA_BACKDROP, view)
        .map_err(|e| e.to_string())?;
    let area_backdrop = commands
        .spawn((place(rect, view), Name::new(AREA_BACKDROP)))
        .id();
    commands.entity(root).add_child(area_backdrop);
    let art = source.message_log.area_backdrop.clone();
    paint_backdrop(
        commands,
        area_backdrop,
        source,
        &art,
        Vec2::new(rect.width, rect.height),
        images,
        materials,
    )?;

    let rect = source.fdf.rect(AREA, view).map_err(|e| e.to_string())?;
    let (inset, line_height) = text_area(&source.fdf, AREA)?;
    if rect.width <= 2. * inset || rect.height <= 2. * inset {
        return Err(format!("{AREA}: inset consumes text area"));
    }
    let mut node = place(rect, view);
    node.padding = UiRect::all(Val::Vh(inset / crate::mainmenu::FRAME_H * 100.));
    node.flex_direction = FlexDirection::Column;
    node.overflow = Overflow::scroll_y();
    // Layout clamps this to the end of the content, which is the newest entry.
    let area = commands
        .spawn((
            node,
            ScrollPosition(Vec2::new(0., f32::MAX)),
            super::QuestScrollArea,
            Name::new(AREA),
        ))
        .id();
    commands.entity(root).add_child(area);
    for line in lines {
        let text = label(commands, area, (*line).into(), source, AREA)?;
        commands
            .entity(text)
            .insert(bevy::text::LineHeight::RelativeToFont(line_height));
    }

    let ok = source.message_log.ok.clone();
    let material = source.message_log.ok_material.clone();
    dialog_button(
        commands,
        source,
        root,
        DialogButton {
            frame: OK_BUTTON,
            art: ok,
            material,
            action: Action::CloseMessageLog,
        },
        view,
        images,
        materials,
    )?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::super::tests::fixture_source;
    use super::*;
    use crate::campaign_messages::MessageHistory;
    use pcx_play::playercolor_host::TimedMessage;

    const FRAMES: &str = r#"
        Frame "BACKDROP" "State" {
            BackdropBackground "Bg",
        }
        Frame "FRAME" "LogDialog" {
            Width 0.4,
            Height 0.4,
        }
        Frame "BACKDROP" "LogBackdrop" {
            SetAllPoints,
            BackdropBackground "Bg",
        }
        Frame "TEXT" "LogTitle" {
            SetPoint TOP, "LogDialog", TOP, 0.0, -0.03,
            FrameFont "FixtureFont", 0.02,
            FontJustificationH JUSTIFYCENTER,
            Text "MESSAGE_LOG",
        }
        Frame "GLUETEXTBUTTON" "LogOkButton" {
            Width 0.1,
            Height 0.03,
            SetPoint BOTTOM, "LogDialog", BOTTOM, 0.0, 0.03,
            ControlBackdrop "State",
            ControlPushedBackdrop "State",
            ControlDisabledBackdrop "State",
            ButtonPushedTextOffset 0.001 -0.001,
            ButtonText "LogOkButtonText",
        }
        Frame "TEXT" "LogOkButtonText" {
            FrameFont "FixtureFont", 0.012,
            Text "OK",
        }
        Frame "BACKDROP" "LogAreaBackdrop" {
            Width 0.3,
            Height 0.2,
            SetPoint BOTTOM, "LogOkButton", TOP, 0.0, 0.01,
            BackdropBackground "Bg",
        }
        Frame "TEXTAREA" "LogArea" {
            Width 0.28,
            Height 0.18,
            SetPoint LEFT, "LogAreaBackdrop", LEFT, 0.005, 0.0,
            FrameFont "FixtureFont", 0.011,
            TextAreaLineGap 0.001,
            TextAreaMaxLines 4,
            TextAreaLineHeight 0.01,
            TextAreaInset 0.002,
        }
    "#;
    const GLOBALS: &str =
        "MESSAGE_LOG \"Message Log\",\nOK \"OK\",\nKEY_LOG \"Log (|Cfffed312F12|R)\",\n";

    fn art(frames: &str, globals: &str) -> Result<Art, String> {
        Art::load(&mut Fdf::parse(frames).unwrap(), globals)
    }
    fn timed(sequence: u64, text: &str) -> TimedMessage {
        TimedMessage {
            sequence,
            position: [0., 0.],
            text: text.into(),
            started_at: 0.,
            expires_at: 1.,
        }
    }
    fn source() -> Source {
        let mut fdf = Fdf::parse(FRAMES).unwrap();
        let art = Art::load(&mut fdf, GLOBALS).unwrap();
        let mut source = fixture_source(fdf);
        source.message_log = art;
        source.globals = GLOBALS.into();
        source
            .backdrop_images
            .insert(("Bg".into(), false), Handle::default());
        source
    }
    fn entries(world: &World, area: Entity) -> Vec<String> {
        world
            .get::<Children>(area)
            .into_iter()
            .flat_map(|children| children.iter())
            .filter(|child| world.get::<Text>(*child).is_some())
            .map(|text| {
                world
                    .get::<Children>(text)
                    .unwrap()
                    .iter()
                    .map(|span| world.get::<TextSpan>(span).unwrap().0.clone())
                    .collect()
            })
            .collect()
    }
    fn named(world: &mut World, name: &str) -> Entity {
        world
            .query::<(Entity, &Name)>()
            .iter(world)
            .find(|(_, n)| n.as_str() == name)
            .unwrap_or_else(|| panic!("{name} not spawned"))
            .0
    }

    #[test]
    fn art_reads_the_authored_cap_and_function_key() {
        let art = art(FRAMES, GLOBALS).unwrap();
        assert_eq!(art.max_entries, 4);
        assert_eq!(art.key, KeyCode::F12);
        assert_eq!(
            art.font_styles(),
            ["LogTitle", "LogArea", "LogOkButtonText"]
        );
    }

    #[test]
    fn art_refuses_unusable_caps_keys_and_strings() {
        for frames in [
            FRAMES.replace("TextAreaMaxLines 4", "TextAreaMaxLines 0"),
            FRAMES.replace("TextAreaMaxLines 4", "TextAreaMaxLines 2.5"),
            FRAMES.replace("TextAreaMaxLines 4,", ""),
            FRAMES.replace("ControlPushedBackdrop \"State\",", ""),
        ] {
            assert!(art(&frames, GLOBALS).is_err(), "{frames}");
        }
        for globals in [
            GLOBALS.replace("KEY_LOG", "KEY_OTHER"),
            GLOBALS.replace("MESSAGE_LOG", "OTHER"),
            GLOBALS.replace("KEY_LOG \"Log (|Cfffed312F12|R)\"", "KEY_LOG \"Log\""),
        ] {
            assert!(art(FRAMES, &globals).is_err(), "{globals}");
        }
    }

    #[test]
    fn dialog_lists_entries_oldest_first_scrolled_to_the_newest_with_a_closing_ok() {
        let mut source = source();
        let mut world = World::new();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let root = dialog(
            &mut Commands::new(&mut queue, &world),
            &mut source,
            &["first", "|cffff0000second|r"],
            &Assets::default(),
            &mut Assets::default(),
        )
        .unwrap();
        queue.apply(&mut world);
        let area = named(&mut world, AREA);
        assert_eq!(entries(&world, area), ["first", "second"]);
        assert_eq!(world.get::<ScrollPosition>(area).unwrap().y, f32::MAX);
        let ok = named(&mut world, OK_BUTTON);
        assert!(matches!(
            world.get::<Action>(ok),
            Some(Action::CloseMessageLog)
        ));
        assert!(world.get::<ChildOf>(ok).is_some_and(|p| p.parent() == root));
        let title = named(&mut world, TITLE);
        assert_eq!(
            world.get::<Node>(title).unwrap().justify_content,
            JustifyContent::Center
        );
    }

    #[test]
    fn dialog_refuses_unsupported_markup_without_spawning() {
        let mut source = source();
        let mut world = World::new();
        let before = world.query::<Entity>().iter(&world).count();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        assert!(
            dialog(
                &mut Commands::new(&mut queue, &world),
                &mut source,
                &["ok", "^^code|cffff0000^^"],
                &Assets::default(),
                &mut Assets::default(),
            )
            .is_err()
        );
        queue.apply(&mut world);
        assert_eq!(world.query::<Entity>().iter(&world).count(), before);
    }

    #[test]
    fn sync_follows_the_open_flag_and_history_revision_and_caps_history() {
        let mut world = World::new();
        let mut state = UpperControls {
            assets: Some(source()),
            messages_open: true,
            ..default()
        };
        let mut history = MessageHistory::default();
        let mut materials = Assets::default();
        let images = Assets::default();
        let mut run =
            |world: &mut World, state: &mut UpperControls, history: &mut MessageHistory| {
                let mut queue = bevy::ecs::world::CommandQueue::default();
                sync(
                    &mut Commands::new(&mut queue, world),
                    state,
                    history,
                    &images,
                    &mut materials,
                );
                queue.apply(world);
            };
        run(&mut world, &mut state, &mut history);
        let first = state.messages.entity.expect("opened with no entries");
        let area = named(&mut world, AREA);
        assert_eq!(entries(&world, area), Vec::<String>::new());
        run(&mut world, &mut state, &mut history);
        assert_eq!(
            state.messages.entity,
            Some(first),
            "unchanged history keeps the dialog"
        );
        let texts: Vec<_> = (1..=6).map(|n| timed(n, &format!("line {n}"))).collect();
        history.absorb(&texts);
        run(&mut world, &mut state, &mut history);
        let second = state.messages.entity.unwrap();
        assert_ne!(second, first);
        assert!(
            world.get_entity(first).is_err(),
            "replaced dialog is despawned"
        );
        let area = named(&mut world, AREA);
        assert_eq!(
            entries(&world, area),
            ["line 3", "line 4", "line 5", "line 6"],
            "TextAreaMaxLines caps the history"
        );
        state.messages_open = false;
        run(&mut world, &mut state, &mut history);
        assert_eq!(state.messages.entity, None);
        assert!(world.get_entity(second).is_err());
        assert!(state.last_error.is_none());
    }
}
