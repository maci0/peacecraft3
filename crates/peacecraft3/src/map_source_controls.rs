//! Project-launcher controls for the caller's explicitly configured map.
use crate::{
    button_actions::ButtonActions,
    map_requests::{MapOutcome, MapRequest, MapRequestSet, MapRequests, RequestFailure},
};
use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct LoadSelectedMap;
#[derive(Component)]
pub(crate) struct SelectedMapText;

pub(crate) fn install(app: &mut App) {
    crate::button_actions::install(app);
    app.add_systems(
        Update,
        click_selected_map
            .run_if(crate::button_actions::input_allowed)
            .before(MapRequestSet::Deliver)
            .before(crate::button_actions::clear),
    );
    app.add_systems(Update, update_source_text.after(MapRequestSet::Deliver));
}
fn click_selected_map(
    actions: Res<ButtonActions>,
    buttons: Query<(), With<LoadSelectedMap>>,
    mut state: ResMut<MapRequests>,
    mut requests: MessageWriter<MapRequest>,
) {
    for entity in &actions.entities {
        if buttons.contains(*entity) {
            match state.selected_map_request() {
                Ok(request) => {
                    requests.write(request);
                }
                Err(source) => {
                    state.last_outcome = Some(MapOutcome {
                        label: "map selection".into(),
                        identity: None,
                        binding_revision: None,
                        result: Err(source),
                    });
                }
            }
        }
    }
}
fn update_source_text(state: Res<MapRequests>, mut text: Query<&mut Text, With<SelectedMapText>>) {
    let value = source_text(&state);
    for mut text in &mut text {
        if text.0 != value {
            text.0.clone_from(&value);
        }
    }
}
pub(crate) fn source_text(state: &MapRequests) -> String {
    let Some(binding) = &state.bindings else {
        return "No map sources configured. Game files and a map must be selected before loading."
            .to_owned();
    };
    let source = &binding.receipts.map.provenance;
    let status = match state.last_outcome.as_ref().map(|outcome| &outcome.result) {
        Some(Ok(())) => "Map data parsed. Gameplay is not started.".to_owned(),
        Some(Err(RequestFailure::MissingBindings)) => {
            "Map source configuration required.".to_owned()
        }
        Some(Err(error)) => format!("Map request failed: {error}"),
        None => "Selected map data can be loaded.".to_owned(),
    };
    format!(
        "Selected map: {}\nSource: {} · {}\nDeclared edition/version: {} / {}\n{status}",
        binding.map_label,
        source.source.origin,
        source.locator,
        binding.map_identity.edition,
        binding.map_identity.version
    )
}
