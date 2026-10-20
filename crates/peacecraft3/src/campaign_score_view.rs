//! Campaign score screen view. Spawns the selected `ScoreScreen.fdf` chrome through the
//! glue helpers and fills labels from `GlobalStrings` keys. Score rows are not drawn: their
//! counters are not qualified yet. Refusals go to diagnostics until source-authored refusal
//! UI exists; no game copy is invented for them.
use crate::glue_menu::{self, GlueButton, GlueLabel};
use crate::mainmenu::{FRAME_H, FRAME_W};
use bevy::prelude::*;
use peacecraft3::campaign_score::{ActionOutcome, ScoreScreen, ScoreScreenState, press};
use peacecraft3::campaign_score_assets::ScoreScreenAssets;
use peacecraft3::campaign_score_layout::ROOT_FRAME;
use std::collections::HashSet;

/// The frame the score layout is authored against, as the glue screens use it.
const VIEWPORT: (f32, f32) = (FRAME_W, FRAME_H);

#[derive(Component)]
struct ScoreScreenRoot;

pub fn install(app: &mut App) {
    app.add_systems(
        Update,
        (
            rebuild.after(peacecraft3::campaign_score_assets::load_assets),
            handle_buttons.before(crate::button_actions::clear),
            fill_labels,
        )
            .chain(),
    );
}

/// Every entity under the score screen roots, excluding the roots themselves.
fn owned_entities(
    roots: &Query<Entity, With<ScoreScreenRoot>>,
    children: &Query<&Children>,
) -> HashSet<Entity> {
    let mut owned = HashSet::new();
    let mut pending: Vec<Entity> = roots.iter().collect();
    while let Some(entity) = pending.pop() {
        if let Ok(kids) = children.get(entity) {
            for kid in kids.iter() {
                if owned.insert(kid) {
                    pending.push(kid);
                }
            }
        }
    }
    owned
}

fn rebuild(
    mut commands: Commands,
    state: Res<ScoreScreenState>,
    assets: Res<ScoreScreenAssets>,
    roots: Query<Entity, With<ScoreScreenRoot>>,
) {
    if !(state.is_changed() || assets.is_changed()) {
        return;
    }
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let Some(ScoreScreen::Refused {
        missing, context, ..
    }) = &state.0
    else {
        return;
    };
    for input in missing.iter() {
        warn!("score screen refused: {input}");
    }
    let Some(layout) = assets.layout_for(context) else {
        return;
    };
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ScoreScreenRoot,
        ))
        .id();
    match glue_menu::control_items_for(layout.fdf(), ROOT_FRAME, VIEWPORT, |_| Ok(true)) {
        Ok((items, unsupported)) => {
            for frame in unsupported {
                warn!("score screen control without a verified view: {frame}");
            }
            glue_menu::spawn(&mut commands, root, &items);
        }
        Err(e) => warn!("score screen layout refused: {e:?}"),
    }
}

/// Applies the score buttons pressed this frame. OK dismisses the presentation; Save Replay
/// is refused, since no replay writer exists. Presses on buttons outside this screen are
/// ignored.
fn handle_buttons(
    activations: Res<crate::button_actions::ButtonActions>,
    buttons: Query<&GlueButton>,
    roots: Query<Entity, With<ScoreScreenRoot>>,
    children: Query<&Children>,
    mut state: ResMut<ScoreScreenState>,
) {
    let owned = owned_entities(&roots, &children);
    for entity in &activations.entities {
        if !owned.contains(entity) {
            continue;
        }
        let Ok(button) = buttons.get(*entity) else {
            continue;
        };
        if let Some(ActionOutcome::ReplayUnsupported) = press(&mut state, &button.frame) {
            warn!("score screen: Save Replay refused, no replay writer exists");
        }
    }
}

/// Writes labels that belong to the score screen. A key that does not resolve is logged
/// with its frame and key, and the label is left unchanged rather than filled with a guess.
fn fill_labels(
    state: Res<ScoreScreenState>,
    assets: Res<ScoreScreenAssets>,
    roots: Query<Entity, With<ScoreScreenRoot>>,
    children: Query<&Children>,
    mut labels: Query<(Entity, &GlueLabel, &mut Text)>,
) {
    let Some(ScoreScreen::Refused { context, .. }) = &state.0 else {
        return;
    };
    let Some(layout) = assets.layout_for(context) else {
        return;
    };
    let owned = owned_entities(&roots, &children);
    for (entity, label, mut text) in &mut labels {
        if !owned.contains(&entity) {
            continue;
        }
        match layout.label(&label.frame) {
            Ok(value) => {
                if text.0 != value {
                    text.0 = value;
                }
            }
            Err(e) => warn_once!("score label {} unresolved: {e:?}", label.frame),
        }
    }
}
