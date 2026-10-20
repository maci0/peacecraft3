//! Executed timed text on the published campaign UI camera.
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use pcx_objects::editor::messages::MessageStyle;
use pcx_play::{map_startup_host::MapStartupHost, playercolor_host::TimedMessage};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignHud, CampaignPresentationSet},
    map_binding::{AssetReceipt, receipt},
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::sync::Weak;

#[path = "campaign_dialogue.rs"]
mod dialogue;

#[derive(Resource, Default)]
pub(crate) struct CampaignMessages {
    session: Option<Weak<()>>,
    root: Option<Entity>,
    /// Transmission panel and the start time of the scene it shows.
    scene: Option<(Entity, f64)>,
    dialogue: Option<dialogue::DialogueAssets>,
    dialogue_attempted: bool,
    font: Option<Handle<Font>>,
    style: Option<MessageStyle>,
    attempted: bool,
    visible: Vec<u64>,
    pub receipts: Vec<AssetReceipt>,
    pub last_error: Option<String>,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignMessages>().add_systems(
        Update,
        project
            .after(CampaignCameraSet)
            .before(CampaignPresentationSet),
    );
}
fn remove_root(commands: &mut Commands, state: &mut CampaignMessages) {
    if let Some(root) = state.root.take() {
        commands.entity(root).despawn();
    }
    state.visible.clear();
}
fn remove_scene(commands: &mut Commands, state: &mut CampaignMessages) {
    if let Some((root, _)) = state.scene.take() {
        commands.entity(root).despawn();
    }
}
#[derive(SystemParam)]
struct MessageAssets<'w, 's> {
    fonts: ResMut<'w, Assets<Font>>,
    images: ResMut<'w, Assets<Image>>,
    materials: ResMut<'w, Assets<crate::glue_menu::BackdropImage>>,
    nodes: Query<'w, 's, &'static mut Node>,
}
fn set_active(node: &mut Node, active: bool) {
    node.display = if active { Display::Flex } else { Display::None };
}
fn project(
    mut commands: Commands,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    view: Res<CampaignCamera>,
    sources: Res<SelectedMapSources>,
    requests: Res<MapRequests>,
    assets: MessageAssets,
    mut state: ResMut<CampaignMessages>,
) {
    let MessageAssets {
        mut fonts,
        mut images,
        mut materials,
        mut nodes,
    } = assets;
    let witness = session.session_witness();
    if !match (&state.session, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    } {
        remove_root(&mut commands, &mut state);
        remove_scene(&mut commands, &mut state);
        if let Some(dialogue) = state.dialogue.take() {
            dialogue.release(&mut images, &mut fonts, &mut materials);
        }
        if let Some(font) = state.font.take() {
            fonts.remove(font.id());
        }
        *state = CampaignMessages {
            session: witness,
            ..default()
        };
    }
    let active = view.active();
    for root in state
        .root
        .into_iter()
        .chain(state.scene.map(|(root, _)| root))
    {
        if let Ok(mut node) = nodes.get_mut(root) {
            set_active(&mut node, active);
        }
    }
    if !active {
        return;
    }
    // A scene shows until it ends or its scene duration elapses.
    let scene = session.host().and_then(|h| {
        let objects = h.objects();
        let now = objects.timer_game_time()?;
        objects
            .cinematic_scene()
            .filter(|s| now < s.started + f64::from(s.scene_duration))
            .cloned()
    });
    if state.scene.map(|(_, at)| at) != scene.as_ref().map(|s| s.started) {
        remove_scene(&mut commands, &mut state);
        if let Some(dialogue) = state.dialogue.as_mut() {
            dialogue.clear_materials(&mut materials);
        }
    }
    let pending_scene = scene.filter(|_| state.scene.is_none());
    let Some(entries) = session.host().and_then(|h| {
        let objects = h.objects();
        let entries: Vec<_> = objects
            .messages()
            .zip(objects.timer_game_time())
            .map(|(messages, now)| messages.visible(now).collect())
            .unwrap_or_default();
        let visible: Vec<_> = entries.iter().map(|entry| entry.sequence).collect();
        (visible != state.visible || pending_scene.is_some())
            .then(|| entries.into_iter().cloned().collect::<Vec<_>>())
    }) else {
        return;
    };
    let visible: Vec<u64> = entries.iter().map(|entry| entry.sequence).collect();
    if entries.is_empty() {
        remove_root(&mut commands, &mut state);
        if pending_scene.is_none() {
            return;
        }
    }
    let (Some(context), Some(bindings)) = (session.context(), requests.bindings.as_ref()) else {
        return;
    };
    if !requests
        .admission
        .shared_context()
        .as_ref()
        .is_some_and(|selected| std::sync::Arc::ptr_eq(selected, context))
        || !requests
            .published_receipts
            .as_ref()
            .is_some_and(|published| std::sync::Arc::ptr_eq(published, &bindings.receipts))
    {
        return;
    }
    let Some(reader) = sources.for_receipts(&bindings.receipts) else {
        return;
    };
    if let Some(scene) = &pending_scene {
        if !state.dialogue_attempted {
            let race = session.host().and_then(|host| {
                host.objects()
                    .local_race_preference_constant()
                    .ok()
                    .flatten()
            });
            state.dialogue_attempted = true;
            let result = race
                .as_deref()
                .and_then(|race| race.strip_prefix("RACE_PREF_"))
                .ok_or_else(|| "transmission has no local race preference".to_string())
                .and_then(|race| {
                    dialogue::DialogueAssets::load(&reader, race, &mut images, &mut fonts)
                });
            match result {
                Ok(dialogue) => {
                    state.receipts.extend(dialogue.receipts.clone());
                    if !dialogue.unsupported.is_empty() {
                        state.last_error = Some(dialogue.unsupported.join("; "));
                    }
                    state.dialogue = Some(dialogue);
                }
                Err(error) => state.last_error = Some(error),
            }
        }
        if let Some(dialogue) = state.dialogue.as_mut() {
            match dialogue.paint(&mut commands, scene, &images, &mut materials) {
                Ok(root) => state.scene = Some((root, scene.started)),
                Err(error) => state.last_error = Some(error),
            }
        }
    }
    if entries.is_empty() {
        return;
    }
    if state.font.is_none() {
        if state.attempted {
            return;
        }
        state.attempted = true;
        let result = (|| {
            let misc = reader
                .resolve_bounded("UI\\MiscUI.txt", 1 << 20)
                .map_err(|e| e.to_string())?;
            let skin = reader
                .resolve_bounded("UI\\war3skins.txt", 1 << 20)
                .map_err(|e| e.to_string())?;
            let style = MessageStyle::read(&misc.bytes, &skin.bytes, 1 << 20)?;
            let font = reader
                .resolve_bounded(&style.font_path, 16 << 20)
                .map_err(|e| format!("{}: {e}", style.font_path))?;
            if !crate::hud_render::font_registers(&font.bytes) {
                return Err(format!("{}: font has no readable face", style.font_path));
            }
            Ok((
                style,
                Font::from_bytes(font.bytes.clone()),
                vec![receipt(&misc), receipt(&skin), receipt(&font)],
            ))
        })();
        match result {
            Ok((style, font, receipts)) => {
                state.font = Some(fonts.add(font));
                state.style = Some(style);
                state.receipts.extend(receipts);
            }
            Err(error) => {
                state.last_error = Some(error);
                return;
            }
        }
    }
    if visible == state.visible {
        return;
    }
    let result = paint(
        &mut commands,
        &entries,
        state.style.as_ref().expect("loaded style"),
        state.font.as_ref().expect("loaded font"),
    );
    match result {
        Ok(root) => {
            remove_root(&mut commands, &mut state);
            state.root = Some(root);
            state.visible = visible;
            if !state.dialogue_attempted
                || state
                    .dialogue
                    .as_ref()
                    .is_some_and(|dialogue| dialogue.unsupported.is_empty())
            {
                state.last_error = None;
            }
        }
        Err(error) => {
            remove_root(&mut commands, &mut state);
            state.visible = visible;
            state.last_error = Some(error);
        }
    }
}
pub(crate) fn paint(
    commands: &mut Commands,
    entries: &[TimedMessage],
    style: &MessageStyle,
    font: &Handle<Font>,
) -> Result<Entity, String> {
    // shortcut: relative placement uses the campaign view; qualify native message-frame
    // margins and wrapping before claiming rendered original-game parity.
    let height_vh = style.height / crate::mainmenu::FRAME_H * 100.;
    if !height_vh.is_finite() || height_vh <= 0.0 {
        return Err("message font height is unrepresentable".into());
    }
    let [x, y] = entries.last().ok_or("no live messages")?.position;
    let position = [x * 100., y * 100.];
    if position.iter().any(|v| !v.is_finite()) {
        return Err("message position is unrepresentable".into());
    }
    let parsed = entries
        .iter()
        .map(|entry| crate::rich_text::parse(&entry.text).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    if parsed.iter().any(|text| !text.unsupported.is_empty()) {
        return Err("message contains unsupported text substitutions".into());
    }
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(position[0]),
                bottom: percent(position[1]),
                width: percent((100. - position[0]).max(0.)),
                max_height: percent((100. - position[1]).max(0.)),
                flex_direction: FlexDirection::ColumnReverse,
                overflow: Overflow::clip(),
                ..default()
            },
            CampaignHud,
            Pickable::IGNORE,
            Name::new("Campaign text messages"),
        ))
        .id();
    for text in parsed.into_iter().rev() {
        let text_font = TextFont {
            font: bevy::text::FontSource::Handle(font.clone()),
            font_size: bevy::text::FontSize::Vh(height_vh),
            ..default()
        };
        let line = commands
            .spawn((
                Text::new(""),
                text_font.clone(),
                TextColor(Color::WHITE),
                TextLayout::new(
                    bevy::text::Justify::Left,
                    bevy::text::LineBreak::WordBoundary,
                ),
                Pickable::IGNORE,
            ))
            .id();
        for span in text.spans {
            let color = span
                .colour
                .map(|c| Color::srgba(c[0], c[1], c[2], c[3]))
                .unwrap_or(Color::WHITE);
            let child = commands
                .spawn((
                    TextSpan::new(span.text),
                    text_font.clone(),
                    TextColor(color),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(line).add_child(child);
        }
        commands.entity(root).add_child(line);
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn campaign_nodes_hide_and_restore_without_erasing_layout() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let parent = app
            .world_mut()
            .spawn((
                Node {
                    left: percent(23.),
                    ..default()
                },
                CampaignHud,
            ))
            .id();
        let child = app
            .world_mut()
            .spawn((Node::default(), Visibility::Visible))
            .id();
        app.world_mut().entity_mut(parent).add_child(child);
        app.world_mut()
            .run_system_once(|mut nodes: Query<&mut Node, With<CampaignHud>>| {
                let mut node = nodes.single_mut().unwrap();
                set_active(&mut node, false);
            })
            .unwrap();
        assert_eq!(
            app.world().get::<Node>(parent).unwrap().display,
            Display::None
        );
        assert_eq!(
            *app.world().get::<Visibility>(child).unwrap(),
            Visibility::Visible
        );
        app.world_mut()
            .run_system_once(|mut nodes: Query<&mut Node, With<CampaignHud>>| {
                set_active(&mut nodes.single_mut().unwrap(), true);
            })
            .unwrap();
        let node = app.world().get::<Node>(parent).unwrap();
        assert_eq!(node.display, Display::Flex);
        assert_eq!(node.left, percent(23.));
    }
    #[test]
    fn message_text_preserves_authored_colours_and_scales_with_the_view() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.world_mut()
            .run_system_once(|mut commands: Commands| {
                paint(
                    &mut commands,
                    &[TimedMessage {
                        sequence: 1,
                        position: [0.2, 0.3],
                        text: "a|c7f123456b|r|nc".into(),
                        started_at: 0.,
                        expires_at: 4.,
                    }],
                    &MessageStyle {
                        font_path: "synthetic.ttf".into(),
                        height: 0.03,
                    },
                    &Handle::default(),
                )
                .unwrap();
            })
            .unwrap();
        app.world_mut()
            .run_system_once(|mut commands: Commands| {
                assert!(
                    paint(
                        &mut commands,
                        &[TimedMessage {
                            sequence: 2,
                            position: [0., 0.],
                            text: "x".into(),
                            started_at: 0.,
                            expires_at: 1.
                        }],
                        &MessageStyle {
                            font_path: "synthetic.ttf".into(),
                            height: f32::MAX
                        },
                        &Handle::default()
                    )
                    .is_err()
                );
            })
            .unwrap();
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, With<CampaignHud>>();
        let root = roots.single(world).unwrap();
        assert_eq!(root.left, percent(20.));
        let Val::Percent(bottom) = root.bottom else {
            panic!("relative message placement");
        };
        assert!((bottom - 30.).abs() < 0.00001);
        let mut spans = world.query::<(&TextSpan, &TextColor, &TextFont)>();
        let spans: Vec<_> = spans.iter(world).collect();
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].0.0, "b");
        assert_eq!(
            spans[1].1.0,
            Color::srgba(18. / 255., 52. / 255., 86. / 255., 127. / 255.)
        );
        let bevy::text::FontSize::Vh(size) = spans[0].2.font_size else {
            panic!("relative font size");
        };
        assert!((size - 5.).abs() < 0.00001);
    }
}
