//! Local-original probe: which campaign sounds reach Bevy audio entities.
//! Needs the unchanged Human01 chapter selected through `PCX_NATIVE_CHAPTER_*` and
//! a private `XDG_DATA_HOME` under `.scratch`. No audio plugin or device is created.

use bevy::prelude::*;
use pcx_play::{
    map_startup_host::MapStartupHost,
    player_command_log::{PlayerCommand, StartIdentity},
};
use peacecraft3::{
    campaign_runtime::CampaignRuntime,
    campaign_startup::CampaignSeed,
    map_script_session::MapScriptSession,
    script_audio::{ScriptAudioReport, ScriptVoice},
};
use std::time::Duration;

/// Probe pacing, not an authored duration.
const BATCHES: u32 = 240;
const TICKS_PER_BATCH: usize = 64;
const SKIP_AFTER_BATCH: u32 = 120;

#[test]
#[ignore = "requires unchanged Human01 via PCX_NATIVE_CHAPTER_* selecting roc and a private XDG_DATA_HOME under .scratch"]
fn human01_opening_audio_reaches_entities() {
    let mut app = crate::native_campaign::tests::started_app();
    peacecraft3::script_audio::install(&mut app);
    let identity = {
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        StartIdentity::from_context(
            app.world().resource::<CampaignSeed>().prepared.unwrap(),
            session.context().unwrap(),
        )
    };
    app.world_mut()
        .resource_mut::<crate::campaign_input::CampaignInput>()
        .commands
        .begin(identity);
    let mut runtime = CampaignRuntime::new(Duration::from_secs_f64(1.0 / 64.0), 64, 4096);
    {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        runtime.advance_ticks(&mut session, 0).unwrap();
    }
    let mut last_refusal = None;
    let mut members = std::collections::BTreeSet::new();
    for batch in 0..BATCHES {
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            runtime
                .advance_ticks(&mut session, TICKS_PER_BATCH)
                .unwrap();
        }
        app.update();
        if batch == SKIP_AFTER_BATCH {
            app.world_mut().resource_scope(
                |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
                    let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
                    let objects = session.host().unwrap().objects_mut();
                    let name = "EVENT_PLAYER_END_CINEMATIC";
                    objects.fire_local_player_event(name).unwrap();
                    input
                        .commands
                        .record(
                            objects.timer_game_time(),
                            PlayerCommand::LocalEvent { name: name.into() },
                        )
                        .unwrap();
                },
            );
        }
        let world = app.world_mut();
        let mut voices = world.query::<&ScriptVoice>();
        for voice in voices.iter(world) {
            members.insert(voice.member.clone());
        }
        let report = world.resource::<ScriptAudioReport>();
        if report.last_refusal != last_refusal {
            println!("batch {batch}: refusal: {:?}", report.last_refusal);
            last_refusal = report.last_refusal.clone();
        }
        if batch % 20 == 0 {
            let (played, refusals) = (report.played, report.refusal_count);
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects();
            println!(
                "batch {batch}: played={played} refusals={refusals} music={:?} thematic={:?} queued_starts={}",
                objects.map_music_selection(),
                objects.thematic_music(),
                objects.pending_sound_starts().len()
            );
        }
    }
    let report = app.world().resource::<ScriptAudioReport>();
    println!(
        "final: played={} refusals={} admission_error={:?} partial={:?}",
        report.played, report.refusal_count, report.admission_error, report.last_partial
    );
    for member in &members {
        println!("voice member: {member}");
    }
}
