//! Log each campaign subsystem's error once when it changes, so a stalled
//! campaign view names the stage that refused.
use bevy::{ecs::system::SystemParam, prelude::*};
use peacecraft3::{
    campaign_camera::CampaignCamera, campaign_runtime::CampaignRuntime,
    campaign_startup::CampaignStartup, map_ground::GroundPublication,
};
use std::fmt::Write;

#[derive(Resource, Default)]
struct Reported(Vec<Option<String>>);

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Reported>()
        .add_systems(Last, (report, debug_info).chain());
}

#[derive(SystemParam)]
struct DebugSources<'w> {
    runtime: Option<Res<'w, CampaignRuntime>>,
    seed: Option<Res<'w, peacecraft3::campaign_startup::CampaignSeed>>,
    requests: Option<Res<'w, peacecraft3::map_requests::MapRequests>>,
    session: Option<
        ResMut<
            'w,
            peacecraft3::map_script_session::MapScriptSession<
                pcx_play::map_startup_host::MapStartupHost,
            >,
        >,
    >,
}

fn debug_info(
    time: Res<Time<Real>>,
    debug: Option<ResMut<pcx_gfx::performance::DebugOverlay>>,
    sources: DebugSources,
    reported: Res<Reported>,
    mut last: Local<f64>,
) {
    let Some(mut debug) = debug.filter(|debug| debug.visible) else {
        return;
    };
    let now = time.elapsed_secs_f64();
    if now - *last < 0.25 && !debug.details.is_empty() {
        return;
    }
    *last = now;
    debug.details.clear();
    if let Some(bindings) = sources
        .requests
        .as_ref()
        .and_then(|requests| requests.bindings.as_ref())
    {
        let _ = writeln!(debug.details, "Map: {}", bindings.map_label);
    }
    if let Some(runtime) = sources.runtime {
        let _ = writeln!(
            debug.details,
            "Tick {} | speed {}x | paused {}\nView suspended {} | queued {:.3} s",
            runtime.tick(),
            runtime.speed(),
            runtime.paused,
            runtime.view_paused,
            runtime.pending().as_secs_f64()
        );
    }
    if let Some(seed) = sources.seed.and_then(|seed| seed.prepared) {
        let _ = writeln!(debug.details, "Seed: {seed}");
    }
    if let Some(mut session) = sources.session {
        // The existing host accessor is mutable; these diagnostics only read it.
        if let Some(host) = session.bypass_change_detection().host() {
            let objects = host.objects();
            if let Some(clock) = objects.timer_game_time() {
                let _ = writeln!(debug.details, "Game clock: {clock:.3} s");
            }
            let _ = writeln!(
                debug.details,
                "Units: {} | selected: {}",
                objects.executed_units().count(),
                objects.selection().len()
            );
        } else {
            debug.details.push_str("Campaign not running\n");
        }
    }
    for error in reported.0.iter().flatten().take(3) {
        let _ = writeln!(
            debug.details,
            "Error: {}",
            error.chars().take(160).collect::<String>()
        );
    }
}

#[derive(SystemParam)]
struct Stages<'w> {
    transition: Option<Res<'w, peacecraft3::campaign_runtime::CampaignTransition>>,
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
    unit_panel: Option<Res<'w, crate::campaign_unit_panel::UnitPanel>>,
    command_card: Option<Res<'w, crate::campaign_command_card::CommandCard>>,
    minimap: Option<Res<'w, crate::campaign_minimap::CampaignMinimap>>,
}

fn report(stages: Stages, mut reported: ResMut<Reported>) {
    let Stages {
        transition,
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
        unit_panel,
        command_card,
        minimap,
    } = stages;
    let current = [
        (
            "unit panel",
            unit_panel.and_then(|p| {
                p.last_error
                    .as_ref()
                    .map(|e| format!("{e} ({} source receipts)", p.receipt_count()))
            }),
        ),
        (
            "campaign minimap",
            minimap.and_then(|state| state.last_error.clone()),
        ),
        (
            "campaign transition",
            transition.and_then(|state| state.last_error.as_ref().map(ToString::to_string)),
        ),
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
        (
            "command card",
            command_card.and_then(|card| {
                card.last_error
                    .as_ref()
                    .map(|error| format!("{error} ({} source receipts)", card.receipt_count()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn debug_panel_reports_seed_and_idle_campaign_without_mutating_the_session() {
        let mut world = World::new();
        world.init_resource::<Time<Real>>();
        world.insert_resource(pcx_gfx::performance::DebugOverlay {
            visible: true,
            details: String::new(),
        });
        world.insert_resource(Reported(vec![Some("missing fixture".into())]));
        world.insert_resource(CampaignRuntime::new(
            std::time::Duration::from_secs_f64(1.0 / 64.0),
            64,
            4096,
        ));
        world.insert_resource(peacecraft3::campaign_startup::CampaignSeed {
            requested: Some(42),
            prepared: Some(42),
        });
        world.init_resource::<peacecraft3::map_script_session::MapScriptSession<
            pcx_play::map_startup_host::MapStartupHost,
        >>();
        world.run_system_once(debug_info).unwrap();
        let details = &world
            .resource::<pcx_gfx::performance::DebugOverlay>()
            .details;
        assert!(details.contains("Tick 0 | speed 1x"), "{details}");
        assert!(details.contains("Seed: 42"));
        assert!(details.contains("Campaign not running"));
        assert!(details.contains("Error: missing fixture"));
        assert_eq!(world.resource::<CampaignRuntime>().tick(), 0);
    }
}
