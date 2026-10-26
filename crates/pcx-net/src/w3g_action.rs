//! Action bytes inside one `.w3g` command block.
//!
//! Lengths follow the public post-1.14 action notes and were checked by
//! consuming every nonempty command in the qualified local sample, whose
//! header fields are game version 29 and build 6060. An id this module does
//! not know is refused at its first byte. Nothing after that byte is treated
//! as a later action.
//!
//! `ObjectId` is the two little-endian dwords stored in the file. Binding that
//! pair to a host unit is a later step: correlate it with units on the pinned
//! map, or supply an explicit checked table for one bounded fixture. The pair
//! is not an allocator pointer. `0xFFFFFFFF, 0xFFFFFFFF` is the notes' absent
//! object. Location words stay raw dwords until a world-boundary conversion
//! checks that they are finite.

use super::{Record, Replay};

/// Header `game_version` for which this action layout was consumed.
///
/// This is the qualified sample's header field, not a product-version string.
pub const QUALIFIED_GAME_VERSION: u32 = 29;
/// Header `build` for that same layout.
pub const QUALIFIED_BUILD: u16 = 6060;

/// Which replay layout `decode_actions` will read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionVersion {
    pub game_version: u32,
    pub build: u16,
}

impl ActionVersion {
    /// Admits only the layout consumed exactly on the qualified sample.
    pub fn qualified(self) -> Result<Self, ActionError> {
        if self.game_version == QUALIFIED_GAME_VERSION && self.build == QUALIFIED_BUILD {
            Ok(self)
        } else {
            Err(ActionError::UnsupportedVersion {
                game_version: self.game_version,
                build: self.build,
            })
        }
    }
}

/// Which replay-wide counter was exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetLimit {
    /// Command blocks accepted from time slots.
    Commands,
    /// Actions decoded across those commands.
    Actions,
    /// Object pairs materialized across those actions.
    Objects,
    /// Action-slice bytes accepted across those commands.
    Bytes,
}

/// Replay-wide ceilings. One [`decode_replay`] call does not reset them per command.
///
/// They are caller limits. A `u16` count in the file is not permission to allocate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionBudget {
    /// Most command blocks in the replay.
    pub max_commands: usize,
    /// Most actions in the replay.
    pub max_actions: usize,
    /// Most object pairs in the replay.
    pub max_objects: usize,
    /// Most action-slice bytes in the replay.
    pub max_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct Usage {
    commands: usize,
    actions: usize,
    objects: usize,
    bytes: usize,
}

/// Two dwords of an object reference, in file order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectId {
    pub first: u32,
    pub second: u32,
}

impl ObjectId {
    /// The notes' no-object pair, also the target of the sample's ground order.
    pub fn is_absent(self) -> bool {
        self.first == u32::MAX && self.second == u32::MAX
    }
}

/// `0x16` select mode. Other bytes are refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode {
    /// Add the listed objects to the selection.
    Add,
    /// Remove the listed objects from the selection.
    Remove,
}

/// One decoded action. Location words stay the file's dwords, not world floats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// `0x01`.
    Pause,
    /// `0x02`.
    Resume,
    /// `0x03`. Speed is `0` slow, `1` normal, `2` fast.
    SetSpeed(u8),
    /// `0x04`.
    IncreaseSpeed,
    /// `0x05`.
    DecreaseSpeed,
    /// `0x10` ability with no point and no target.
    Immediate {
        flags: u16,
        item: [u8; 4],
        unknown: [u32; 2],
    },
    /// `0x11` ability at a point.
    Point {
        flags: u16,
        item: [u8; 4],
        unknown: [u32; 2],
        x: u32,
        y: u32,
    },
    /// `0x12` ability at a point and an object.
    Target {
        flags: u16,
        item: [u8; 4],
        unknown: [u32; 2],
        x: u32,
        y: u32,
        object: ObjectId,
    },
    /// `0x16` selection change.
    Selection {
        mode: SelectMode,
        objects: Vec<ObjectId>,
    },
    /// `0x17` assign control group. `group` is `0..=9`.
    AssignGroup { group: u8, objects: Vec<ObjectId> },
    /// `0x18` recall a control group. `marker` is kept raw.
    SelectGroup { group: u8, marker: u8 },
    /// `0x19` subgroup selection, post-1.14 layout.
    SelectSubgroup { item: [u8; 4], object: ObjectId },
    /// `0x1A`.
    PreSubselection,
    /// `0x61`.
    Escape,
}

/// Actions from one time slot, stamped with the running sum of slot increments.
///
/// `time_ms` is that sum. It is not [`super::Header::length_ms`]. On the
/// qualified sample the increment sum is 94147 and the header length is 73625.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimedActions {
    pub time_ms: u32,
    pub player_id: u8,
    pub actions: Vec<Action>,
}

/// Why action bytes were not decoded.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ActionError {
    #[error(
        "action layout is qualified only for game {qualified_game} build {qualified_build}, got game {game_version} build {build}",
        qualified_game = QUALIFIED_GAME_VERSION,
        qualified_build = QUALIFIED_BUILD
    )]
    UnsupportedVersion { game_version: u32, build: u16 },
    #[error("unsupported action 0x{id:02x} at byte {at}")]
    Unsupported { at: usize, id: u8 },
    #[error("action at byte {at} does not fit in the command block")]
    Truncated { at: usize, need: usize },
    #[error("replay action budget exceeded ({limit:?}, allowed {allowed})")]
    Budget { limit: BudgetLimit, allowed: usize },
    #[error("command range is outside the replay content")]
    Range,
    #[error("accumulated time-slot increments overflow")]
    TimeOverflow,
    #[error("action allocation failed")]
    Capacity,
}

struct Input<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Input<'a> {
    fn u8(&mut self) -> Result<u8, ActionError> {
        let byte = *self.bytes.get(self.at).ok_or(ActionError::Truncated {
            at: self.at,
            need: 1,
        })?;
        self.at += 1;
        Ok(byte)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ActionError> {
        let end = self.at.checked_add(n).ok_or(ActionError::Truncated {
            at: self.at,
            need: n,
        })?;
        let slice = self.bytes.get(self.at..end).ok_or(ActionError::Truncated {
            at: self.at,
            need: n,
        })?;
        self.at = end;
        Ok(slice)
    }

    fn u16(&mut self) -> Result<u16, ActionError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, ActionError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn item(&mut self) -> Result<[u8; 4], ActionError> {
        let bytes = self.take(4)?;
        Ok([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    fn object(&mut self) -> Result<ObjectId, ActionError> {
        Ok(ObjectId {
            first: self.u32()?,
            second: self.u32()?,
        })
    }

    fn ability(&mut self) -> Result<(u16, [u8; 4], [u32; 2]), ActionError> {
        Ok((self.u16()?, self.item()?, [self.u32()?, self.u32()?]))
    }

    fn charge_object(
        &mut self,
        usage: &mut Usage,
        budget: &ActionBudget,
    ) -> Result<ObjectId, ActionError> {
        let next = usage.objects.checked_add(1).ok_or(ActionError::Capacity)?;
        if next > budget.max_objects {
            return Err(ActionError::Budget {
                limit: BudgetLimit::Objects,
                allowed: budget.max_objects,
            });
        }
        let object = self.object()?;
        usage.objects = next;
        Ok(object)
    }

    fn objects(
        &mut self,
        count: u16,
        usage: &mut Usage,
        budget: &ActionBudget,
    ) -> Result<Vec<ObjectId>, ActionError> {
        let count = usize::from(count);
        let next = usage
            .objects
            .checked_add(count)
            .ok_or(ActionError::Capacity)?;
        if next > budget.max_objects {
            return Err(ActionError::Budget {
                limit: BudgetLimit::Objects,
                allowed: budget.max_objects,
            });
        }
        let need = count.checked_mul(8).ok_or(ActionError::Capacity)?;
        if self.bytes.len().saturating_sub(self.at) < need {
            return Err(ActionError::Truncated { at: self.at, need });
        }
        let mut objects = Vec::new();
        objects
            .try_reserve(count)
            .map_err(|_| ActionError::Capacity)?;
        for _ in 0..count {
            objects.push(self.object()?);
        }
        usage.objects = next;
        Ok(objects)
    }
}

fn admit_command(
    usage: &mut Usage,
    budget: &ActionBudget,
    nbytes: usize,
) -> Result<(), ActionError> {
    let commands = usage.commands.checked_add(1).ok_or(ActionError::Capacity)?;
    if commands > budget.max_commands {
        return Err(ActionError::Budget {
            limit: BudgetLimit::Commands,
            allowed: budget.max_commands,
        });
    }
    let bytes = usage
        .bytes
        .checked_add(nbytes)
        .ok_or(ActionError::Capacity)?;
    if bytes > budget.max_bytes {
        return Err(ActionError::Budget {
            limit: BudgetLimit::Bytes,
            allowed: budget.max_bytes,
        });
    }
    usage.commands = commands;
    usage.bytes = bytes;
    Ok(())
}

fn group_number(at: usize, id: u8, group: u8) -> Result<u8, ActionError> {
    if group > 9 {
        Err(ActionError::Unsupported { at, id })
    } else {
        Ok(group)
    }
}

fn decode_body(
    bytes: &[u8],
    budget: &ActionBudget,
    usage: &mut Usage,
) -> Result<Vec<Action>, ActionError> {
    let mut input = Input { bytes, at: 0 };
    let mut actions = Vec::new();
    while input.at < bytes.len() {
        let next = usage.actions.checked_add(1).ok_or(ActionError::Capacity)?;
        if next > budget.max_actions {
            return Err(ActionError::Budget {
                limit: BudgetLimit::Actions,
                allowed: budget.max_actions,
            });
        }
        let at = input.at;
        let id = input.u8()?;
        let action = match id {
            0x01 => Action::Pause,
            0x02 => Action::Resume,
            0x03 => {
                let speed = input.u8()?;
                if speed > 2 {
                    return Err(ActionError::Unsupported { at, id });
                }
                Action::SetSpeed(speed)
            }
            0x04 => Action::IncreaseSpeed,
            0x05 => Action::DecreaseSpeed,
            0x10 => {
                let (flags, item, unknown) = input.ability()?;
                Action::Immediate {
                    flags,
                    item,
                    unknown,
                }
            }
            0x11 => {
                let (flags, item, unknown) = input.ability()?;
                Action::Point {
                    flags,
                    item,
                    unknown,
                    x: input.u32()?,
                    y: input.u32()?,
                }
            }
            0x12 => {
                let (flags, item, unknown) = input.ability()?;
                Action::Target {
                    flags,
                    item,
                    unknown,
                    x: input.u32()?,
                    y: input.u32()?,
                    object: input.charge_object(usage, budget)?,
                }
            }
            0x16 => {
                let mode = match input.u8()? {
                    0x01 => SelectMode::Add,
                    0x02 => SelectMode::Remove,
                    _ => return Err(ActionError::Unsupported { at, id }),
                };
                let count = input.u16()?;
                let objects = input.objects(count, usage, budget)?;
                Action::Selection { mode, objects }
            }
            0x17 => {
                let group = group_number(at, id, input.u8()?)?;
                let count = input.u16()?;
                let objects = input.objects(count, usage, budget)?;
                Action::AssignGroup { group, objects }
            }
            0x18 => {
                let group = group_number(at, id, input.u8()?)?;
                Action::SelectGroup {
                    group,
                    marker: input.u8()?,
                }
            }
            0x19 => Action::SelectSubgroup {
                item: input.item()?,
                object: input.charge_object(usage, budget)?,
            },
            0x1A => Action::PreSubselection,
            0x61 => Action::Escape,
            other => return Err(ActionError::Unsupported { at, id: other }),
        };
        usage.actions = next;
        actions.try_reserve(1).map_err(|_| ActionError::Capacity)?;
        actions.push(action);
    }
    Ok(actions)
}

/// Decodes every action in one command block.
///
/// `bytes` is the action region already sliced by the structural reader
/// (`Command::action`). A time slot's millisecond increment is not in these
/// bytes. This call spends one command and `bytes.len()` against `budget`.
pub fn decode_actions(
    bytes: &[u8],
    version: ActionVersion,
    budget: ActionBudget,
) -> Result<Vec<Action>, ActionError> {
    version.qualified()?;
    let mut usage = Usage::default();
    admit_command(&mut usage, &budget, bytes.len())?;
    decode_body(bytes, &budget, &mut usage)
}

/// Decodes every time-slot command. Checksum, leave, and countdown records stay
/// on the structural replay and contribute no actions. A command that fails
/// fails the whole call.
pub fn decode_replay(
    replay: &Replay,
    budget: ActionBudget,
) -> Result<Vec<TimedActions>, ActionError> {
    ActionVersion {
        game_version: replay.header.game_version,
        build: replay.header.build,
    }
    .qualified()?;
    let mut time_ms = 0u32;
    let mut usage = Usage::default();
    let mut timed = Vec::new();
    for record in &replay.records {
        let Record::TimeSlot {
            time_increment,
            commands,
        } = record
        else {
            continue;
        };
        time_ms = time_ms
            .checked_add(u32::from(*time_increment))
            .ok_or(ActionError::TimeOverflow)?;
        for command in commands {
            let bytes = replay
                .content
                .get(command.action.clone())
                .ok_or(ActionError::Range)?;
            admit_command(&mut usage, &budget, bytes.len())?;
            timed.try_reserve(1).map_err(|_| ActionError::Capacity)?;
            timed.push(TimedActions {
                time_ms,
                player_id: command.player_id,
                actions: decode_body(bytes, &budget, &mut usage)?,
            });
        }
    }
    Ok(timed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::w3g::{Command, Header, PlayerRecord, Record, Slot, Startup};

    fn version() -> ActionVersion {
        ActionVersion {
            game_version: QUALIFIED_GAME_VERSION,
            build: QUALIFIED_BUILD,
        }
    }

    fn budget() -> ActionBudget {
        ActionBudget {
            max_commands: 8,
            max_actions: 8,
            max_objects: 12,
            max_bytes: 256,
        }
    }

    fn le16(n: u16) -> [u8; 2] {
        n.to_le_bytes()
    }

    fn hand_replay(content: Vec<u8>, records: Vec<Record>) -> Replay {
        Replay {
            header: Header {
                game_version: QUALIFIED_GAME_VERSION,
                build: QUALIFIED_BUILD,
                flags: 0,
                length_ms: 73_625,
                decompressed_size: u32::try_from(content.len()).unwrap_or(u32::MAX),
                block_fields: Vec::new(),
            },
            startup: Startup {
                host: PlayerRecord {
                    player_id: 1,
                    name: Vec::new(),
                },
                game_name: Vec::new(),
                settings: [0; 13],
                map_path: Vec::new(),
                creator_name: Vec::new(),
                encoded_tail: Vec::new(),
                player_count: 0,
                game_type: 0,
                language_id: 0,
                players: Vec::new(),
                slots: Vec::<Slot>::new(),
                random_seed: 0,
                select_mode: 0,
                start_spots: 0,
            },
            content,
            records,
        }
    }

    #[test]
    fn qualified_sample_action_sequence_consumes_exactly() {
        // Synthetic non-stock bytes in the qualified layout sizes.
        let mut bytes = vec![0x01];
        bytes.extend_from_slice(&[0x12, 0x00, 0x00]);
        bytes.extend_from_slice(b"Zq9x");
        bytes.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        bytes.extend_from_slice(&[0x10, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00]);
        bytes.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        bytes.push(0x16);
        bytes.push(0x01);
        bytes.extend_from_slice(&le16(1));
        bytes.extend_from_slice(&0x32deu32.to_le_bytes());
        bytes.extend_from_slice(&0x32deu32.to_le_bytes());
        bytes.push(0x19);
        bytes.extend_from_slice(b"Zq9x");
        bytes.extend_from_slice(&0x32deu32.to_le_bytes());
        bytes.extend_from_slice(&0x32deu32.to_le_bytes());
        bytes.push(0x1A);
        bytes.push(0x61);
        let actions = decode_actions(&bytes, version(), budget()).unwrap();
        assert_eq!(actions.len(), 6);
        assert!(matches!(actions[0], Action::Pause));
        match &actions[1] {
            Action::Target { object, .. } => assert!(object.is_absent()),
            other => panic!("{other:?}"),
        }
        match &actions[2] {
            Action::Selection { mode, objects } => {
                assert_eq!(*mode, SelectMode::Add);
                assert_eq!(
                    objects.as_slice(),
                    &[ObjectId {
                        first: 0x32de,
                        second: 0x32de
                    }]
                );
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(actions[3], Action::SelectSubgroup { .. }));
        assert_eq!(actions[4], Action::PreSubselection);
        assert_eq!(actions[5], Action::Escape);
    }

    #[test]
    fn unknown_action_does_not_yield_the_following_pause() {
        let error = decode_actions(&[0x99, 0x01], version(), budget()).unwrap_err();
        assert_eq!(error, ActionError::Unsupported { at: 0, id: 0x99 });
    }

    #[test]
    fn other_versions_are_refused() {
        let refused = ActionVersion {
            game_version: 28,
            build: QUALIFIED_BUILD,
        };
        assert!(matches!(
            decode_actions(&[0x01], refused, budget()),
            Err(ActionError::UnsupportedVersion { .. })
        ));
        let other_build = ActionVersion {
            game_version: QUALIFIED_GAME_VERSION,
            build: 6059,
        };
        assert!(other_build.qualified().is_err());
    }

    #[test]
    fn orders_groups_and_budget_are_explicit() {
        let mut point = vec![0x11, 0x01, 0x00];
        point.extend_from_slice(b"Zq9x");
        point.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        point.extend_from_slice(&100u32.to_le_bytes());
        point.extend_from_slice(&200u32.to_le_bytes());
        let actions = decode_actions(&point, version(), budget()).unwrap();
        assert!(matches!(actions[0], Action::Point { x: 100, y: 200, .. }));

        let mut group = vec![0x17, 0x00];
        group.extend_from_slice(&le16(1));
        group.extend_from_slice(&1u32.to_le_bytes());
        group.extend_from_slice(&1u32.to_le_bytes());
        group.extend_from_slice(&[0x18, 0x00, 0x03]);
        let actions = decode_actions(&group, version(), budget()).unwrap();
        assert_eq!(
            actions[0],
            Action::AssignGroup {
                group: 0,
                objects: vec![ObjectId {
                    first: 1,
                    second: 1
                }]
            }
        );
        assert_eq!(
            actions[1],
            Action::SelectGroup {
                group: 0,
                marker: 0x03
            }
        );

        let tight = ActionBudget {
            max_commands: 1,
            max_actions: 8,
            max_objects: 0,
            max_bytes: 256,
        };
        assert_eq!(
            decode_actions(&group, version(), tight),
            Err(ActionError::Budget {
                limit: BudgetLimit::Objects,
                allowed: 0,
            })
        );
    }

    #[test]
    fn a_short_order_does_not_return_a_partial_action() {
        let error = decode_actions(&[0x11, 0x00, 0x00], version(), budget()).unwrap_err();
        assert!(matches!(error, ActionError::Truncated { .. }));
    }

    #[test]
    fn replay_budget_is_not_reset_per_command() {
        let replay = hand_replay(
            vec![0x01, 0x01],
            vec![Record::TimeSlot {
                time_increment: 10,
                commands: vec![
                    Command {
                        player_id: 1,
                        action: 0..1,
                    },
                    Command {
                        player_id: 1,
                        action: 1..2,
                    },
                ],
            }],
        );
        let tight = ActionBudget {
            max_commands: 2,
            max_actions: 1,
            max_objects: 0,
            max_bytes: 2,
        };
        assert_eq!(
            decode_replay(&replay, tight),
            Err(ActionError::Budget {
                limit: BudgetLimit::Actions,
                allowed: 1,
            })
        );
        let bytes = ActionBudget {
            max_commands: 2,
            max_actions: 2,
            max_objects: 0,
            max_bytes: 1,
        };
        assert_eq!(
            decode_replay(&replay, bytes),
            Err(ActionError::Budget {
                limit: BudgetLimit::Bytes,
                allowed: 1,
            })
        );
    }

    #[test]
    fn a_hand_built_range_outside_the_content_is_refused() {
        let replay = hand_replay(
            vec![0x01],
            vec![Record::TimeSlot {
                time_increment: 1,
                commands: vec![Command {
                    player_id: 1,
                    action: 0..4,
                }],
            }],
        );
        assert_eq!(decode_replay(&replay, budget()), Err(ActionError::Range));
    }

    #[test]
    fn slot_increments_sum_without_reading_checksums() {
        let replay = hand_replay(
            vec![0x01],
            vec![
                Record::TimeSlot {
                    time_increment: 100,
                    commands: vec![Command {
                        player_id: 1,
                        action: 0..1,
                    }],
                },
                Record::Checksum(0x1122_3344),
                Record::TimeSlot {
                    time_increment: 50,
                    commands: vec![Command {
                        player_id: 1,
                        action: 0..1,
                    }],
                },
            ],
        );
        let timed = decode_replay(&replay, budget()).unwrap();
        assert_eq!(timed[0].time_ms, 100);
        assert_eq!(timed[1].time_ms, 150);
        assert!(matches!(timed[0].actions[0], Action::Pause));
        assert_ne!(timed[1].time_ms, replay.header.length_ms);
    }

    #[test]
    fn slot_increment_addition_is_checked() {
        let records = (0..65_538)
            .map(|_| Record::TimeSlot {
                time_increment: u16::MAX,
                commands: Vec::new(),
            })
            .collect();
        let replay = hand_replay(Vec::new(), records);
        assert_eq!(
            decode_replay(&replay, budget()),
            Err(ActionError::TimeOverflow)
        );
    }
}
