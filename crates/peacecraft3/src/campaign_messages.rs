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
    let Some(layout) = session
        .host()
        .and_then(|host| host.objects().message_layout())
        .cloned()
    else {
        state.last_error =
            Some("message frame geometry was not admitted with this map session".into());
        return;
    };
    let result = entries
        .last()
        .ok_or_else(|| "no live messages".to_string())
        .and_then(|entry| {
            let [x, y] = entry.position;
            layout.placement(x, y)
        })
        .and_then(|placement| {
            paint(
                &mut commands,
                &entries,
                state.style.as_ref().expect("loaded style"),
                state.font.as_ref().expect("loaded font"),
                placement,
            )
        });
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
    placement: pcx_play::message_layout::Placement,
) -> Result<Entity, String> {
    let height_vh = style.height / crate::mainmenu::FRAME_H * 100.;
    if !height_vh.is_finite() || height_vh <= 0.0 {
        return Err("message font height is unrepresentable".into());
    }
    let [viewport_x, viewport_y] = placement.viewport;
    let [left, bottom, width, height] = [
        placement.left / viewport_x * 100.,
        placement.bottom / viewport_y * 100.,
        placement.width / viewport_x * 100.,
        placement.height / viewport_y * 100.,
    ];
    if viewport_x <= 0.
        || viewport_y <= 0.
        || [left, bottom, width, height].iter().any(|v| !v.is_finite())
    {
        return Err("message placement is unrepresentable".into());
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
                left: percent(left),
                bottom: percent(bottom),
                width: percent(width),
                max_height: percent(height),
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
                    pcx_play::message_layout::Placement {
                        left: 0.051875,
                        bottom: 0.2305,
                        width: 0.5,
                        height: 0.25,
                        viewport: [0.8, 0.6],
                    },
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
                        &Handle::default(),
                        pcx_play::message_layout::Placement {
                            left: 0.051875,
                            bottom: 0.2305,
                            width: 0.5,
                            height: 0.25,
                            viewport: [0.8, 0.6],
                        },
                    )
                    .is_err()
                );
                assert!(
                    paint(
                        &mut commands,
                        &[TimedMessage {
                            sequence: 3,
                            position: [0., 0.],
                            text: "x".into(),
                            started_at: 0.,
                            expires_at: 1.
                        }],
                        &MessageStyle {
                            font_path: "synthetic.ttf".into(),
                            height: 0.03
                        },
                        &Handle::default(),
                        pcx_play::message_layout::Placement {
                            left: 0.051875,
                            bottom: 0.2305,
                            width: 0.5,
                            height: 0.25,
                            viewport: [0., 0.6],
                        },
                    )
                    .is_err()
                );
            })
            .unwrap();
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, With<CampaignHud>>();
        let root = roots.single(world).unwrap();
        let Val::Percent(left) = root.left else {
            panic!("relative message placement");
        };
        assert!((left - 6.484375).abs() < 0.00001);
        let Val::Percent(bottom) = root.bottom else {
            panic!("relative message placement");
        };
        assert!((bottom - 38.416667).abs() < 0.00001);
        let Val::Percent(width) = root.width else {
            panic!("relative message width");
        };
        assert!((width - 62.5).abs() < 0.00001);
        let Val::Percent(max_height) = root.max_height else {
            panic!("relative message height");
        };
        assert!((max_height - 41.666668).abs() < 0.00001);
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
    #[cfg(feature = "test-fixtures")]
    fn show_synthetic_component_message(host: &mut MapStartupHost, position: [f32; 2], text: &str) {
        use pcx_script::{Host, Value};
        let objects = host.objects_mut();
        let local = objects
            .native("GetLocalPlayer", &[])
            .expect("synthetic component message requires a local player");
        objects
            .native(
                "DisplayTimedTextToPlayer",
                &[
                    local,
                    Value::Real(position[0]),
                    Value::Real(position[1]),
                    Value::Real(3600.),
                    Value::Str(text.into()),
                ],
            )
            .expect("synthetic component message must be admitted");
    }
    /// Component test for host-bound layout admission. The timed text is injected
    /// through each published host's own natives; it is not mission evidence.
    #[cfg(feature = "test-fixtures")]
    #[test]
    #[ignore = "requires PCX_NATIVE_CHAPTER_* selecting an admitted local chapter map"]
    fn launcher_changes_and_lookalike_sessions_cannot_rebind_message_layout() {
        let mut app = crate::native_campaign::tests::started_app();
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let host = session.host().expect("published first host");
            show_synthetic_component_message(host, [0.25, 0.4], "synthetic component A");
        }
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(CampaignCamera::active_for_test(camera))
            .init_resource::<Assets<Font>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<crate::glue_menu::BackdropImage>>();
        install(&mut app);
        for _ in 0..6 {
            app.update();
        }
        let (root, left) = {
            let state = app.world().resource::<CampaignMessages>();
            assert!(
                state.last_error.is_none(),
                "messages: {:?}",
                state.last_error
            );
            let root = state
                .root
                .expect("synthetic component message must display");
            let node = app.world().get::<Node>(root).unwrap();
            let Val::Percent(left) = node.left else {
                panic!("relative message placement");
            };
            (root, left)
        };
        let expansion = std::env::var("PCX_NATIVE_CHAPTER_ADDON").as_deref() == Ok("tft");
        app.insert_resource(crate::launch_gui::Launcher::test_source(
            "/pcx-missing-client".to_string(),
            expansion,
        ));
        for _ in 0..3 {
            app.update();
        }
        {
            let state = app.world().resource::<CampaignMessages>();
            assert!(
                state.last_error.is_none(),
                "launcher change: {:?}",
                state.last_error
            );
            assert_eq!(state.root, Some(root));
            let node = app.world().get::<Node>(root).unwrap();
            assert_eq!(node.left, percent(left));
        }
        let mut next = crate::native_campaign::tests::started_app();
        let mut replacement = next
            .world_mut()
            .remove_resource::<MapScriptSession<MapStartupHost>>()
            .unwrap();
        {
            let host = replacement.host().expect("published second host");
            show_synthetic_component_message(host, [0.75, 0.9], "synthetic component B");
        }
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
        assert!(!std::sync::Arc::ptr_eq(
            &old_context,
            replacement.context().unwrap()
        ));
        app.insert_resource(replacement);
        app.update();
        assert!(app.world().get_entity(root).is_err());
        assert!(
            app.world().resource::<CampaignMessages>().root.is_none(),
            "a lookalike session cannot reuse the previous layout"
        );
        let expected = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects();
            let now = objects
                .timer_game_time()
                .expect("selected map has game time");
            let message = objects
                .messages()
                .expect("admitted messages")
                .visible(now)
                .last()
                .expect("synthetic replacement message");
            assert_eq!(message.text, "synthetic component B");
            let position = message.position;
            let layout = objects.message_layout().expect("admitted message layout");
            let placement = layout.placement(position[0], position[1]).unwrap();
            (
                percent(placement.left / placement.viewport[0] * 100.),
                percent(placement.bottom / placement.viewport[1] * 100.),
            )
        };
        app.insert_resource(next_requests)
            .insert_resource(next_sources);
        for _ in 0..6 {
            app.update();
        }
        let state = app.world().resource::<CampaignMessages>();
        assert!(
            state.last_error.is_none(),
            "replacement messages: {:?}",
            state.last_error
        );
        let new_root = state
            .root
            .expect("replacement session must mount its own messages");
        assert_ne!(new_root, root);
        let node = app.world().get::<Node>(new_root).unwrap();
        assert_eq!(node.left, expected.0);
        assert_eq!(node.bottom, expected.1);
    }
}
