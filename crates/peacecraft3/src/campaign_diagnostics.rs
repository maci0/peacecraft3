//! Log each campaign subsystem's error once when it changes, so a stalled
//! campaign view names the stage that refused.
use bevy::prelude::*;
use peacecraft3::{
    campaign_camera::CampaignCamera, campaign_runtime::CampaignRuntime,
    campaign_startup::CampaignStartup, map_ground::GroundPublication,
};

#[derive(Resource, Default)]
struct Reported(Vec<Option<String>>);

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Reported>().add_systems(Last, report);
}

fn report(
    startup: Option<Res<CampaignStartup>>,
    ground: Option<Res<GroundPublication>>,
    camera: Option<Res<CampaignCamera>>,
    runtime: Option<Res<CampaignRuntime>>,
    actors: Option<Res<crate::campaign_actors::UnitActors>>,
    input: Option<Res<crate::campaign_input::CampaignInput>>,
    mut reported: ResMut<Reported>,
) {
    let current = [
        (
            "campaign startup",
            startup.and_then(|s| s.last_error.as_ref().map(ToString::to_string)),
        ),
        (
            "campaign ground",
            ground.and_then(|g| g.last_error.as_ref().map(ToString::to_string)),
        ),
        ("campaign camera", camera.and_then(|c| c.last_error.clone())),
        (
            "campaign runtime",
            runtime.and_then(|r| r.last_error.as_ref().map(ToString::to_string)),
        ),
        ("campaign units", actors.and_then(|a| a.last_error.clone())),
        ("campaign input", input.and_then(|i| i.last_error.clone())),
    ];
    reported.0.resize(current.len(), None);
    for ((stage, error), seen) in current.into_iter().zip(reported.0.iter_mut()) {
        if error != *seen {
            if let Some(error) = &error {
                warn!("{stage}: {error}");
            }
            *seen = error;
        }
    }
}
