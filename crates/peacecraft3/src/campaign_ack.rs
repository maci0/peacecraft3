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
            "smart" | "move" | "patrol" => Some((*unit, Ack::Yes)),
            _ => None,
        },
        _ => None,
    }
}

/// The UnitUI `unitSound` of a unit definition. `-`, `_` and empty mean no sound set.
fn unit_sound(definition: &Definition) -> Result<Option<String>, String> {
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

fn unit_voices(
    mut commands: Commands,
    input: Res<CampaignInput>,
    mut audio: ResMut<CampaignAudio>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut assets: ResMut<Assets<AudioSource>>,
    sources: Sources,
    live: Query<(Entity, &GameSound)>,
) {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(token: u64) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(token).unwrap())
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
