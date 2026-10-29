//! The error line drawn above the console: the text of the latest refusal in the
//! client's gold, held for its hold time and then faded out. Geometry, colour,
//! timing and shadow come from the client image; the font and its height from
//! `UI\war3skins.txt` and `UI\MiscUI.txt`. Evidence and limits are in
//! `docs/re-notes/input-chat-overlays.md`.
use std::sync::{Arc, Weak};

use bevy::prelude::*;
use pcx_objects::editor::messages::MessageStyle;
use pcx_play::{error_line_rules::ErrorLineRules, map_startup_host::MapStartupHost};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_error_line::ErrorLine,
    campaign_presentation::{CampaignHud, CampaignInterface, CampaignPresentationSet},
    map_binding::{AssetReceipt, receipt},
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};

/// Application read bound for the profiles and the font file.
const MAX_PROFILE_BYTES: usize = 1 << 20;
const MAX_FONT_BYTES: u64 = 16 << 20;

#[derive(Resource, Default)]
pub(crate) struct ErrorView {
    session: Option<Weak<()>>,
    attempted: bool,
    loaded: Option<Loaded>,
    /// The line on screen: its root, its text and the post it shows.
    shown: Option<Shown>,
    pub receipts: Vec<AssetReceipt>,
    pub last_error: Option<String>,
}

struct Loaded {
    rules: ErrorLineRules,
    style: MessageStyle,
    font: Handle<Font>,
}

struct Shown {
    root: Entity,
    text: Entity,
    revision: u64,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<ErrorView>().add_systems(
        Update,
        (load, show)
            .chain()
            .after(CampaignCameraSet)
            .before(CampaignPresentationSet),
    );
}

#[derive(bevy::ecs::system::SystemParam)]
struct Sources<'w> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
}

/// Reads the rules, the style and the font once per published session.
fn load(
    mut view: ResMut<ErrorView>,
    mut commands: Commands,
    mut fonts: ResMut<Assets<Font>>,
    inputs: Sources,
    session: Res<MapScriptSession<MapStartupHost>>,
) {
    let witness = session.session_witness();
    let same = match (&view.session, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same {
        if let Some(shown) = view.shown.take() {
            commands.entity(shown.root).despawn();
        }
        if let Some(loaded) = view.loaded.take() {
            fonts.remove(loaded.font.id());
        }
        *view = ErrorView {
            session: witness,
            ..default()
        };
    }
    if view.attempted || session.session_witness().is_none() {
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
    view.attempted = true;
    let install = inputs
        .launcher
        .as_deref()
        .and_then(crate::launch_gui::selected_install);
    let result = crate::menu_lighting::with_client_image(install.as_deref(), |label, image| {
        let identity = pcx_play::campaign_inputs::client_identity(image, label);
        ErrorLineRules::reviewed_legacy(image, identity)
    })
    .and_then(|rules| {
        let misc = reader
            .resolve_bounded("UI\\MiscUI.txt", MAX_PROFILE_BYTES as u64)
            .map_err(|e| e.to_string())?;
        let skin = reader
            .resolve_bounded("UI\\war3skins.txt", MAX_PROFILE_BYTES as u64)
            .map_err(|e| e.to_string())?;
        let style = MessageStyle::read_keyed(
            &misc.bytes,
            &skin.bytes,
            &rules.height_key,
            &rules.font_key,
            MAX_PROFILE_BYTES,
        )?;
        let font = reader
            .resolve_bounded(&style.font_path, MAX_FONT_BYTES)
            .map_err(|e| format!("{}: {e}", style.font_path))?;
        if !crate::hud_render::font_registers(&font.bytes) {
            return Err(format!("{}: font has no readable face", style.font_path));
        }
        let receipts = vec![receipt(&misc), receipt(&skin), receipt(&font)];
        Ok((rules, style, Font::from_bytes(font.bytes.clone()), receipts))
    });
    match result {
        Ok((rules, style, font, receipts)) => {
            view.loaded = Some(Loaded {
                rules,
                style,
                font: fonts.add(font),
            });
            view.receipts = receipts;
        }
        Err(error) => view.last_error = Some(error),
    }
}

/// Spawns the frame and its single line of text. The line sits at the bottom of the
/// frame, as the client stacks its lines from the bottom, and is left justified.
fn paint(commands: &mut Commands, loaded: &Loaded, text: &str, opacity: f32) -> (Entity, Entity) {
    let rules = &loaded.rules;
    let [width, height] = rules.frame.viewport;
    let shadow_offset = Vec2::from(rules.shadow.offset);
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(rules.frame.left / width * 100.),
                bottom: percent(rules.frame.bottom / height * 100.),
                width: percent(rules.frame.width / width * 100.),
                height: percent(rules.frame.height / height * 100.),
                flex_direction: FlexDirection::ColumnReverse,
                overflow: Overflow::clip(),
                ..default()
            },
            CampaignHud,
            CampaignInterface,
            Pickable::IGNORE,
            Name::new("Campaign error line"),
        ))
        .id();
    let line = commands
        .spawn((
            Text::new(text),
            TextFont {
                font: bevy::text::FontSource::Handle(loaded.font.clone()),
                font_size: bevy::text::FontSize::Vh(
                    loaded.style.height / crate::mainmenu::FRAME_H * 100.,
                ),
                ..default()
            },
            TextColor(tinted(rules.colour, opacity)),
            TextLayout::new(
                bevy::text::Justify::Left,
                bevy::text::LineBreak::WordBoundary,
            ),
            // The authored height is the line pitch, as for the other HUD fonts.
            bevy::text::LineHeight::RelativeToFont(1.0),
            TextShadow {
                offset: Vec2::ZERO,
                color: tinted(rules.shadow.colour, opacity),
            },
            crate::glue_menu::FrameShadow(shadow_offset),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(line);
    (root, line)
}

/// `colour` with its alpha scaled by `opacity`; the client scales the shadow by the
/// same alpha as the text.
fn tinted(colour: [u8; 4], opacity: f32) -> Color {
    let [r, g, b, a] = colour;
    Color::srgba_u8(r, g, b, (f32::from(a) * opacity).round() as u8)
}

fn show(
    mut commands: Commands,
    mut view: ResMut<ErrorView>,
    line: Res<ErrorLine>,
    camera: Res<CampaignCamera>,
    time: Option<Res<Time<Real>>>,
    mut nodes: Query<&mut Node>,
    mut colours: Query<(&mut TextColor, &mut TextShadow)>,
) {
    let view = &mut *view;
    let Some(loaded) = &view.loaded else {
        return;
    };
    let Some(time) = time else {
        return;
    };
    let current = line.current().and_then(|(text, at)| {
        let elapsed = (time.elapsed_secs_f64() - at) as f32;
        loaded.rules.opacity(elapsed).map(|opacity| (text, opacity))
    });
    let Some((text, opacity)) = current else {
        if let Some(shown) = view.shown.take() {
            commands.entity(shown.root).despawn();
        }
        return;
    };
    if view
        .shown
        .as_ref()
        .is_some_and(|shown| shown.revision != line.revision())
    {
        let stale = view.shown.take().expect("checked above");
        commands.entity(stale.root).despawn();
    }
    let shown = view.shown.get_or_insert_with(|| {
        let (root, text) = paint(&mut commands, loaded, text, opacity);
        Shown {
            root,
            text,
            revision: line.revision(),
        }
    });
    if let Ok(mut node) = nodes.get_mut(shown.root) {
        node.display = if camera.active() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok((mut colour, mut shadow)) = colours.get_mut(shown.text) {
        colour.0 = tinted(loaded.rules.colour, opacity);
        shadow.color = tinted(loaded.rules.shadow.colour, opacity);
    }
}

#[cfg(test)]
#[path = "campaign_error_view_tests.rs"]
mod tests;
