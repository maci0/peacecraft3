//! Unit selection and order acknowledgement voices.
//!
//! The accepted player commands drive the voices: a selection plays the unit's
//! `What` row, a move order its `Yes` row and an attack order its `YesAttack` row of
//! `UI\SoundInfo\UnitAckSounds.slk`. The row prefix is the unit's UnitUI `unitSound`.
//! The `<unitSound><Suffix>` row names are shipped bytes; which order plays which
//! suffix is an inference (`docs/re-notes/campaign-audio.md`).

use crate::campaign_audio::{CampaignAudio, CampaignAudioSet, GameSound, Sources};
use crate::campaign_input::{CampaignInput, CampaignInputSet};
use bevy::prelude::*;
use pcx_objects::editor::{FieldSource, SlkId};
use pcx_play::{map_startup_host::MapStartupHost, player_command_log::PlayerCommand};
use pcx_script::HandleId;
use pcx_sim::world::{Definition, FieldValue};
use peacecraft3::map_script_session::MapScriptSession;

const ACK_TABLE: &str = "UI\\SoundInfo\\UnitAckSounds.slk";
const UNIT_SOUND_COLUMN: &str = "unitSound";

pub(crate) fn install(app: &mut App) {
    app.add_systems(
        Update,
        unit_voices.after(CampaignInputSet).after(CampaignAudioSet),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ack {
    What,
    Yes,
    YesAttack,
}

impl Ack {
    fn suffix(self) -> &'static str {
        match self {
            Self::What => "What",
            Self::Yes => "Yes",
            Self::YesAttack => "YesAttack",
        }
    }
}

/// The unit that speaks and what it says for one accepted command.
fn ack_for(
    command: &PlayerCommand,
    enemy: impl Fn(HandleId, HandleId) -> bool,
) -> Option<(HandleId, Ack)> {
    match command {
        PlayerCommand::Queued(inner) => ack_for(inner, enemy),
        PlayerCommand::Select { units, .. } => units.first().map(|&unit| (unit, Ack::What)),
        PlayerCommand::Point { unit, order, .. } => match order.as_str() {
            "attack" => Some((*unit, Ack::YesAttack)),
            "move" | "smart" | "patrol" => Some((*unit, Ack::Yes)),
            _ => None,
        },
        PlayerCommand::Target {
            unit,
            order,
            target,
        } => match order.as_str() {
            "attack" => Some((*unit, Ack::YesAttack)),
            "smart" if enemy(*unit, *target) => Some((*unit, Ack::YesAttack)),
            "smart" | "move" | "patrol" | "harvest" | "repair" => Some((*unit, Ack::Yes)),
            _ => None,
        },
        _ => None,
    }
}

/// The UnitUI `unitSound` of a unit definition. `-`, `_` and empty mean no sound set.
pub(crate) fn unit_sound(definition: &Definition) -> Result<Option<String>, String> {
    let mut fields = definition.fields.iter().filter(|(_, field)| {
        field.metadata.as_ref().is_some_and(|metadata| {
            matches!(&metadata.source,
                FieldSource::Column { slk: SlkId::UnitUi, column } if column == UNIT_SOUND_COLUMN)
        })
    });
    let (_, field) = fields
        .next()
        .ok_or_else(|| format!("missing UnitUI/{UNIT_SOUND_COLUMN} metadata"))?;
    if fields.next().is_some() {
        return Err(format!("ambiguous UnitUI/{UNIT_SOUND_COLUMN} metadata"));
    }
    let text = match &field.value {
        Ok(FieldValue::Cell(text)) => text.as_str(),
        Ok(FieldValue::Override(pcx_objects::Value::Str(text))) => {
            std::str::from_utf8(text).map_err(|_| "unitSound is not UTF-8")?
        }
        _ => return Err(format!("unsupported UnitUI/{UNIT_SOUND_COLUMN} value")),
    }
    .trim();
    Ok((!matches!(text, "" | "_" | "-")).then(|| text.to_owned()))
}

#[allow(clippy::too_many_arguments)]
fn unit_voices(
    mut commands: Commands,
    input: Res<CampaignInput>,
    mut audio: ResMut<CampaignAudio>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut assets: ResMut<Assets<AudioSource>>,
    sources: Sources,
    live: Query<(Entity, &GameSound)>,
    preferences: Option<Res<crate::menu_screens::Navigation>>,
) {
    // UnitCheckBox (`unit` preference) gates this producer only; text, orders
    // and music keep their own consumers. Draft never gates: applied only.
    if !crate::menu_settings::sound_preference::unit_voices_enabled(
        preferences.as_deref().map(|nav| &nav.applied),
    ) {
        return;
    }
    let entries = input.commands.entries();
    if entries.len() < audio.commands_seen {
        audio.commands_seen = 0;
    }
    let fresh = &entries[audio.commands_seen..];
    audio.commands_seen = entries.len();
    if fresh.is_empty() || session.session_witness().is_none() {
        return;
    }
    audio.ensure_bank(&sources, &mut session);
    let Some(host) = session.host() else { return };
    let objects = host.objects();
    let local = objects.local_player_number();
    let owned = |unit: HandleId| {
        objects
            .executed_unit(unit)
            .ok()
            .and_then(|record| objects.player_settings(record.owner).ok())
            .is_some_and(|player| Some(player.number) == local)
    };
    let Some((unit, ack)) = fresh
        .iter()
        .filter_map(|entry| ack_for(&entry.command, |a, b| objects.is_enemy(a, b)))
        .find(|&(unit, _)| owned(unit))
    else {
        return;
    };
    let Ok(record) = objects.executed_unit(unit) else {
        return;
    };
    let name = match unit_sound(&record.definition) {
        Ok(Some(sound)) => format!("{sound}{}", ack.suffix()),
        Ok(None) => return,
        Err(error) => {
            audio.report(error);
            return;
        }
    };
    let effects = sources.settings.audio.effective_effects();
    if let Err(error) = audio.play(ACK_TABLE, &name, effects, &mut commands, &mut assets, &live) {
        audio.report(error);
    } else {
        // The portrait holds its Talk variant while this row stays live.
        // The row name is the shipped `<unitSound><Suffix>` key, so the unit link
        // is source-derived rather than a second command log.
        audio.speaking = Some((unit, name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use pcx_play::player_command_log::StartIdentity;

    fn id(token: u64) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(token).unwrap())
    }

    fn identity() -> StartIdentity {
        use pcx_play::map_admission::MapInputSources;
        use pcx_play::scripts::AssetIdentity;
        let asset = |source: &str| AssetIdentity {
            edition: "fixture".into(),
            version: "fixture".into(),
            source: source.into(),
        };
        StartIdentity {
            seed: 7,
            map_label: "fixture.w3x".into(),
            map_sha256: [0xA5; 32],
            sources: MapInputSources {
                map: asset("map"),
                common: asset("common"),
                blizzard: asset("blizzard"),
                catalog: asset("catalog"),
            },
        }
    }

    /// Runs the real `unit_voices` system fn once with one pending unit
    /// command and the given applied/draft preference state. Returns
    /// (`commands_seen`, spawned `GameSound` count). No session witness, so an
    /// enabled run records the entry then returns before playback; a gated
    /// run returns before even recording. Default (unpublished) sources keep
    /// `ensure_bank` from retaining a bank on either arm.
    fn run_producer(applied: bool, draft: bool) -> (usize, usize) {
        use crate::campaign_input::CampaignInput;
        use crate::menu_screens::Navigation;
        use crate::menu_settings::sound_preference::UNIT_TOGGLE;
        use peacecraft3::{
            browser_configuration::SelectedMapSources, campaign_startup::CampaignSeed,
            map_requests::MapRequests,
        };

        let mut world = World::new();
        world.init_resource::<Assets<AudioSource>>();
        world.init_resource::<CampaignAudio>();
        world.init_resource::<MapScriptSession<MapStartupHost>>();
        world.init_resource::<CampaignInput>();
        world.init_resource::<MapRequests>();
        world.init_resource::<SelectedMapSources>();
        world.init_resource::<CampaignSeed>();
        world.init_resource::<crate::menu_settings::Backend>();
        let mut nav = Navigation::default();
        nav.applied.toggles.insert(UNIT_TOGGLE.into(), applied);
        nav.draft.toggles.insert(UNIT_TOGGLE.into(), draft);
        world.insert_resource(nav);
        {
            let mut input = world.resource_mut::<CampaignInput>();
            input.commands.begin(identity());
            input.commands.record(Some(0.0), point("move")).unwrap();
        }
        world.run_system_once(unit_voices).unwrap();
        let seen = world.resource::<CampaignAudio>().commands_seen;
        let sounds = world.query::<&GameSound>().iter(&world).count();
        (seen, sounds)
    }

    fn point(order: &str) -> PlayerCommand {
        PlayerCommand::Point {
            unit: id(1),
            order: order.into(),
            x: 0.0,
            y: 0.0,
        }
    }

    fn target(order: &str) -> PlayerCommand {
        PlayerCommand::Target {
            unit: id(1),
            order: order.into(),
            target: id(2),
        }
    }

    #[test]
    fn selection_speaks_the_first_selected_unit() {
        let select = PlayerCommand::Select {
            units: vec![id(4), id(5)],
            replace: true,
        };
        assert_eq!(ack_for(&select, |_, _| false), Some((id(4), Ack::What)));
        let empty = PlayerCommand::Select {
            units: vec![],
            replace: true,
        };
        assert_eq!(ack_for(&empty, |_, _| false), None);
    }

    #[test]
    fn movement_and_attack_orders_pick_their_rows() {
        let unit = id(1);
        for (order, expected) in [
            ("move", Ack::Yes),
            ("smart", Ack::Yes),
            ("patrol", Ack::Yes),
            ("attack", Ack::YesAttack),
        ] {
            assert_eq!(
                ack_for(&point(order), |_, _| false),
                Some((unit, expected)),
                "{order}"
            );
        }
        assert_eq!(ack_for(&point("holyBolt"), |_, _| false), None);
    }

    #[test]
    fn a_smart_target_is_an_attack_only_against_an_enemy() {
        let unit = id(1);
        assert_eq!(
            ack_for(&target("smart"), |_, _| true),
            Some((unit, Ack::YesAttack))
        );
        assert_eq!(
            ack_for(&target("smart"), |_, _| false),
            Some((unit, Ack::Yes))
        );
        assert_eq!(
            ack_for(&target("attack"), |_, _| false),
            Some((unit, Ack::YesAttack))
        );
        assert_eq!(ack_for(&target("holybolt"), |_, _| true), None);
    }

    #[test]
    fn work_orders_and_queued_orders_speak_like_the_order_they_carry() {
        let unit = id(1);
        for order in ["harvest", "repair"] {
            assert_eq!(
                ack_for(&target(order), |_, _| false),
                Some((unit, Ack::Yes)),
                "{order}"
            );
        }
        for (inner, expected) in [
            (target("attack"), Some((unit, Ack::YesAttack))),
            (point("move"), Some((unit, Ack::Yes))),
            (point("setrally"), None),
        ] {
            let queued = PlayerCommand::Queued(Box::new(inner));
            assert_eq!(ack_for(&queued, |_, _| false), expected);
        }
    }

    #[test]
    fn producer_system_applied_false_skips_pending_command_and_draft_false_does_not_gate() {
        // Real `unit_voices` system fn via RunSystemOnce (no audio plugin, no
        // device; consistent with campaign_audio tests). Pending command is
        // recorded (commands_seen advances) only on the enabled path.
        let (gated_seen, gated_sounds) = run_producer(false, false);
        assert_eq!(gated_seen, 0, "applied=false must not consume the pending command");
        assert_eq!(gated_sounds, 0, "applied=false must spawn no voice");

        let (enabled_seen, enabled_sounds) = run_producer(true, false);
        assert_eq!(enabled_seen, 1, "draft=false must not gate the producer");
        assert_eq!(enabled_sounds, 0, "no session witness: no voice spawns");
    }

    #[test]
    fn unit_preference_gates_the_producer_not_the_mapping() {
        use crate::menu_screens::Settings;
        use crate::menu_settings::sound_preference::{UNIT_TOGGLE, unit_voices_enabled};
        // Mapping still resolves; the gate lives in unit_voices before playback.
        assert_eq!(
            ack_for(&point("move"), |_, _| false),
            Some((id(1), Ack::Yes))
        );
        let mut applied = Settings::default();
        applied.toggles.insert(UNIT_TOGGLE.into(), false);
        assert!(!unit_voices_enabled(Some(&applied)));
        applied.toggles.insert(UNIT_TOGGLE.into(), true);
        assert!(unit_voices_enabled(Some(&applied)));
        assert!(unit_voices_enabled(None));
    }

    #[test]
    fn other_commands_are_silent() {
        let learn = PlayerCommand::Learn {
            unit: id(1),
            ability: 0,
        };
        let event = PlayerCommand::LocalEvent { name: "x".into() };
        for command in [learn, event] {
            assert_eq!(ack_for(&command, |_, _| true), None);
        }
    }
}
