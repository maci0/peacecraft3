//! Log each campaign subsystem's error once when it changes, so a stalled
//! campaign view names the stage that refused.
use bevy::{ecs::system::SystemParam, prelude::*};
use peacecraft3::{
    campaign_camera::CampaignCamera, campaign_runtime::CampaignRuntime,
    campaign_startup::CampaignStartup, map_ground::GroundPublication,
};

#[derive(Resource, Default)]
struct Reported(Vec<Option<String>>);

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Reported>().add_systems(Last, report);
}

#[derive(SystemParam)]
struct Stages<'w> {
    startup: Option<Res<'w, CampaignStartup>>,
    ground: Option<Res<'w, GroundPublication>>,
    camera: Option<Res<'w, CampaignCamera>>,
    runtime: Option<Res<'w, CampaignRuntime>>,
    actors: Option<Res<'w, crate::campaign_actors::UnitActors>>,
    input: Option<Res<'w, crate::campaign_input::CampaignInput>>,
    selection: Option<Res<'w, crate::campaign_selection::CampaignSelection>>,
    hud: Option<Res<'w, crate::campaign_hud::CampaignHud>>,
    messages: Option<Res<'w, crate::campaign_messages::CampaignMessages>>,
    dialogs: Option<Res<'w, crate::campaign_script_dialogs::CampaignScriptDialogs>>,
    hud_prepare: Option<Res<'w, crate::hud_render::HudPrepare>>,
    inventory: Option<Res<'w, crate::campaign_inventory::CampaignInventory>>,
}

fn report(stages: Stages, mut reported: ResMut<Reported>) {
    let Stages {
        startup,
        ground,
        camera,
        runtime,
        actors,
        input,
        selection,
        hud,
        messages,
        dialogs,
        hud_prepare,
        inventory,
    } = stages;
    let current = [
        (
            "campaign startup",
            startup.and_then(|s| s.last_error.as_ref().map(ToString::to_string)),
        ),
        (
            "campaign ground",
            ground.and_then(|g| g.last_error.as_ref().map(ToString::to_string)),
        ),
        (
            "campaign camera",
            camera.as_ref().and_then(|c| c.last_error.clone()),
        ),
        (
            "campaign menu key",
            camera.and_then(|c| {
                c.menu_key
                    .as_ref()
                    .and_then(|key| key.as_ref().err().cloned())
            }),
        ),
        (
            "campaign runtime",
            runtime.and_then(|r| r.last_error.as_ref().map(ToString::to_string)),
        ),
        ("campaign units", actors.and_then(|a| a.last_error.clone())),
        ("campaign input", input.and_then(|i| i.last_error.clone())),
        (
            "campaign selection",
            selection.and_then(|s| s.last_error.clone()),
        ),
        (
            "campaign hud",
            hud.as_ref().and_then(|h| h.last_error.clone()),
        ),
        (
            "campaign resources",
            hud.and_then(|h| h.resource_error.clone()),
        ),
        (
            "campaign messages",
            messages.and_then(|m| m.last_error.clone()),
        ),
        ("script dialogs", dialogs.and_then(|d| d.last_error.clone())),
        ("hud mount", hud_prepare.and_then(|h| h.refusal.clone())),
        (
            "campaign inventory",
            inventory.and_then(|i| {
                i.last_error
                    .as_ref()
                    .map(|error| format!("{error} ({} source receipts)", i.receipts.len()))
            }),
        ),
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
