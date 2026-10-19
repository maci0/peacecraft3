//! Executed timed text on the published campaign UI camera.
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

#[derive(Resource, Default)]
pub(crate) struct CampaignMessages {
    session: Option<Weak<()>>,
    root: Option<Entity>,
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
fn project(
    mut commands: Commands,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    view: Res<CampaignCamera>,
    sources: Res<SelectedMapSources>,
    requests: Res<MapRequests>,
    mut fonts: ResMut<Assets<Font>>,
    mut state: ResMut<CampaignMessages>,
) {
    let witness = session.session_witness();
    if !match (&state.session, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    } {
        remove_root(&mut commands, &mut state);
        if let Some(font) = state.font.take() {
            fonts.remove(font.id());
        }
        *state = CampaignMessages {
            session: witness,
            ..default()
        };
    }
    let active = view.active();
    if let Some(root) = state.root {
        commands.entity(root).insert(if active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
    if !active {
        return;
    }
    let Some(entries) = session.host().and_then(|h| {
        let objects = h.objects();
        let entries: Vec<_> = objects
            .messages()
            .zip(objects.timer_game_time())
            .map(|(messages, now)| messages.visible(now).collect())
            .unwrap_or_default();
        let visible: Vec<_> = entries.iter().map(|entry| entry.sequence).collect();
        (visible != state.visible).then(|| entries.into_iter().cloned().collect::<Vec<_>>())
    }) else {
        return;
    };
    let visible = entries.iter().map(|entry| entry.sequence).collect();
    if entries.is_empty() {
        remove_root(&mut commands, &mut state);
        return;
    }
    if state.font.is_none() {
        if state.attempted {
            return;
        }
        let (Some(context), Some(bindings)) = (session.context(), requests.bindings.as_ref())
        else {
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
                state.receipts = receipts;
            }
            Err(error) => {
                state.last_error = Some(error);
                return;
            }
        }
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
            state.last_error = None;
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
