//! W3GS action bodies from pinned GHost++ `6ba90f426e96f80fe013a468d1e07151a510adbe`.
//!
//! `RECEIVE_W3GS_OUTGOING_ACTION` stores a caller player id, four CRC bytes,
//! and the remaining bytes as one opaque action. `SEND_W3GS_INCOMING_ACTION`
//! and `SEND_W3GS_INCOMING_ACTION2` prefix a host body, then optional
//! subrecords of player id, little-endian length, and that many payload bytes.
//! `UTIL_AppendByteArray(..., false)` in `ghost/util.cpp` writes the low byte
//! first.
//!
//! Payloads stay ordered and opaque. This module does not decode game orders,
//! call movement acceptance, or check `CCRC::FullCRC`. That function is not in
//! the retained `gameprotocol` or `util` sources. `sendInterval` is the
//! `uint16` argument of `SEND_W3GS_INCOMING_ACTION`; this module does not give
//! it a tick or lockstep meaning. The two prefix bytes of
//! `SEND_W3GS_INCOMING_ACTION2` are stored raw because the source comment does
//! not establish them as a send interval.

#![deny(missing_docs)]

use std::fmt;

use super::W3GS_FAMILY;

/// GHost++ `W3GS_INCOMING_ACTION`.
pub const W3GS_INCOMING_ACTION: u8 = 12;

/// GHost++ `W3GS_OUTGOING_ACTION`.
pub const W3GS_OUTGOING_ACTION: u8 = 38;

/// GHost++ `W3GS_INCOMING_ACTION2`.
pub const W3GS_INCOMING_ACTION2: u8 = 72;

/// Player id `RECEIVE_W3GS_OUTGOING_ACTION` rejects (`PID != 255`).
pub const OUTGOING_REFUSED_PLAYER_ID: u8 = 255;

const PREFIX_LEN: usize = 2;
const INCOMING_CHECKSUM_LEN: usize = 2;
const OUTGOING_CHECKSUM_LEN: usize = 4;
const SUBRECORD_HEADER_LEN: usize = 3;

/// Caller ceiling for incoming subrecords. This is not a protocol default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionLimits {
    /// Most subrecords one incoming message may retain.
    ///
    /// Zero accepts only an empty action queue. One more well-formed
    /// subrecord refuses the whole message.
    pub max_records: usize,
}

/// Player id and opaque payload from one incoming subrecord.
///
/// `payload` is borrowed from the accepted frame. It is not copied and it is
/// not a decoded order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionRecord<'a> {
    /// `CIncomingAction::GetPID` byte written ahead of the length.
    pub player_id: u8,
    /// Bytes after the little-endian length. Empty when that length is zero.
    pub payload: &'a [u8],
}

/// Two bytes ahead of the incoming subrecords.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncomingPrefix {
    /// Little-endian `sendInterval` from `SEND_W3GS_INCOMING_ACTION`.
    Interval(u16),
    /// Two bytes `SEND_W3GS_INCOMING_ACTION2` writes before any checksum.
    ///
    /// The pinned sender stores zeros and marks a send-interval reading as
    /// uncertain. These bytes are not an interval.
    Raw([u8; 2]),
}

/// Incoming action message, ids [`W3GS_INCOMING_ACTION`] and
/// [`W3GS_INCOMING_ACTION2`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingActions<'a> {
    /// Packet prefix before the checksum and subrecords.
    pub prefix: IncomingPrefix,
    /// First two bytes of `FullCRC` over the subrecords, when the queue was
    /// not empty. Not verified.
    pub checksum_prefix: Option<&'a [u8; INCOMING_CHECKSUM_LEN]>,
    /// Subrecords in send order. Absent when the parse fails.
    pub records: Vec<ActionRecord<'a>>,
}

/// One outgoing action, id [`W3GS_OUTGOING_ACTION`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutgoingAction<'a> {
    /// Connection player id passed into `RECEIVE_W3GS_OUTGOING_ACTION`.
    pub player_id: u8,
    /// Four CRC bytes copied from the body. Not verified.
    pub checksum: &'a [u8; OUTGOING_CHECKSUM_LEN],
    /// Bytes after the checksum, in order, including an empty remainder.
    pub payload: &'a [u8],
}

/// Why an action body was not retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionError {
    /// Header family is not [`W3GS_FAMILY`].
    WrongFamily(u8),
    /// Header id is not the action id this call accepts.
    WrongId(u8),
    /// A required prefix, checksum, or subrecord header is short.
    ///
    /// `have` and `need` count the unfinished region, not the whole frame.
    Truncated {
        /// Bytes present in that region.
        have: usize,
        /// Bytes that region needs.
        need: usize,
    },
    /// A subrecord length does not fit in the bytes after its header.
    LengthOverflow {
        /// Little-endian length from the subrecord.
        declared: usize,
        /// Bytes after the player id and length.
        available: usize,
    },
    /// A further well-formed subrecord would pass [`ActionLimits::max_records`].
    CountOverflow {
        /// Caller ceiling that was reached.
        limit: usize,
    },
    /// Outgoing player id is [`OUTGOING_REFUSED_PLAYER_ID`].
    UnassignedPlayer,
    /// The record list could not be reserved.
    Allocation,
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily(family) => write!(f, "action family {family} is not W3GS"),
            Self::WrongId(id) => write!(f, "action id {id} is not accepted for this body"),
            Self::Truncated { have, need } => {
                write!(f, "action region has {have} bytes, needs {need}")
            }
            Self::LengthOverflow {
                declared,
                available,
            } => write!(
                f,
                "action subrecord length {declared} exceeds {available} available bytes"
            ),
            Self::CountOverflow { limit } => {
                write!(f, "action subrecord count exceeds {limit}")
            }
            Self::UnassignedPlayer => write!(
                f,
                "outgoing action player id {OUTGOING_REFUSED_PLAYER_ID} is refused"
            ),
            Self::Allocation => write!(f, "action record allocation failed"),
        }
    }
}

impl std::error::Error for ActionError {}

pub(super) fn parse_incoming<'a>(
    family: u8,
    id: u8,
    body: &'a [u8],
    limits: ActionLimits,
) -> Result<IncomingActions<'a>, ActionError> {
    if family != W3GS_FAMILY {
        return Err(ActionError::WrongFamily(family));
    }
    let prefix = match id {
        W3GS_INCOMING_ACTION => IncomingPrefix::Interval(u16::from_le_bytes(*read_array(body)?)),
        W3GS_INCOMING_ACTION2 => IncomingPrefix::Raw(*read_array(body)?),
        _ => return Err(ActionError::WrongId(id)),
    };
    let rest = &body[PREFIX_LEN..];
    if rest.is_empty() {
        return Ok(IncomingActions {
            prefix,
            checksum_prefix: None,
            records: Vec::new(),
        });
    }
    let checksum_prefix = read_array(rest)?;
    let subpacket = &rest[INCOMING_CHECKSUM_LEN..];
    if subpacket.is_empty() {
        return Err(ActionError::Truncated {
            have: 0,
            need: SUBRECORD_HEADER_LEN,
        });
    }
    Ok(IncomingActions {
        prefix,
        checksum_prefix: Some(checksum_prefix),
        records: walk_records(subpacket, limits)?,
    })
}

pub(super) fn parse_outgoing<'a>(
    family: u8,
    id: u8,
    body: &'a [u8],
    player_id: u8,
) -> Result<OutgoingAction<'a>, ActionError> {
    if family != W3GS_FAMILY {
        return Err(ActionError::WrongFamily(family));
    }
    if id != W3GS_OUTGOING_ACTION {
        return Err(ActionError::WrongId(id));
    }
    let checksum = read_array(body)?;
    if player_id == OUTGOING_REFUSED_PLAYER_ID {
        return Err(ActionError::UnassignedPlayer);
    }
    Ok(OutgoingAction {
        player_id,
        checksum,
        payload: &body[OUTGOING_CHECKSUM_LEN..],
    })
}

fn read_array<const N: usize>(bytes: &[u8]) -> Result<&[u8; N], ActionError> {
    let Some(head) = bytes.get(..N) else {
        return Err(ActionError::Truncated {
            have: bytes.len(),
            need: N,
        });
    };
    <&[u8; N]>::try_from(head).map_err(|_| ActionError::Truncated {
        have: head.len(),
        need: N,
    })
}

fn walk_records<'a>(
    mut rest: &'a [u8],
    limits: ActionLimits,
) -> Result<Vec<ActionRecord<'a>>, ActionError> {
    let mut records = Vec::new();
    while !rest.is_empty() {
        if rest.len() < SUBRECORD_HEADER_LEN {
            return Err(ActionError::Truncated {
                have: rest.len(),
                need: SUBRECORD_HEADER_LEN,
            });
        }
        let declared = u16::from_le_bytes([rest[1], rest[2]]) as usize;
        let available = rest.len() - SUBRECORD_HEADER_LEN;
        if declared > available {
            return Err(ActionError::LengthOverflow {
                declared,
                available,
            });
        }
        if records.len() >= limits.max_records {
            return Err(ActionError::CountOverflow {
                limit: limits.max_records,
            });
        }
        records
            .try_reserve(1)
            .map_err(|_| ActionError::Allocation)?;
        let payload_end = SUBRECORD_HEADER_LEN + declared;
        records.push(ActionRecord {
            player_id: rest[0],
            payload: &rest[SUBRECORD_HEADER_LEN..payload_end],
        });
        rest = &rest[payload_end..];
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::super::{BNCS_FAMILY, Decoder, FrameLimits, decode_one, encode};
    use super::{
        ActionError, ActionLimits, IncomingPrefix, OUTGOING_REFUSED_PLAYER_ID,
        W3GS_INCOMING_ACTION, W3GS_INCOMING_ACTION2, W3GS_OUTGOING_ACTION,
    };

    fn limits(max_records: usize) -> ActionLimits {
        ActionLimits { max_records }
    }

    fn record(body: &mut Vec<u8>, player_id: u8, payload: &[u8]) {
        body.push(player_id);
        let len = u16::try_from(payload.len()).expect("payload fits in u16");
        body.extend_from_slice(&len.to_le_bytes());
        body.extend_from_slice(payload);
    }

    fn borrowed_from(outer: &[u8], inner: &[u8]) -> bool {
        let outer_start = outer.as_ptr();
        let outer_end = outer_start.wrapping_add(outer.len());
        let inner_start = inner.as_ptr();
        let inner_end = inner_start.wrapping_add(inner.len());
        inner_start >= outer_start && inner_end <= outer_end
    }

    #[test]
    fn incoming_and_outgoing_actions_keep_order_and_player_and_reject_truncation_count_and_length()
    {
        let mut body = vec![0x34, 0x12, 0xAB, 0xCD];
        record(&mut body, 1, &[0x10, 0x20]);
        record(&mut body, 5, &[]);
        record(&mut body, 2, &[0x30]);
        let frame =
            encode(super::super::W3GS_FAMILY, W3GS_INCOMING_ACTION, &body).expect("frame fits");
        let decoded = decode_one(&frame, 128).expect("header accepted");
        let parsed = decoded
            .incoming_actions(limits(3))
            .expect("three subrecords");
        assert_eq!(parsed.prefix, IncomingPrefix::Interval(0x1234));
        assert_eq!(parsed.checksum_prefix.copied(), Some([0xAB, 0xCD]));
        assert_eq!(
            parsed
                .records
                .iter()
                .map(|record| (record.player_id, record.payload))
                .collect::<Vec<_>>(),
            vec![(1, &[0x10, 0x20][..]), (5, &[][..]), (2, &[0x30][..])]
        );
        let frame_body = decoded.body();
        assert!(
            parsed
                .records
                .iter()
                .all(|record| borrowed_from(frame_body, record.payload))
        );
        assert!(borrowed_from(
            frame_body,
            parsed.checksum_prefix.expect("checksum present")
        ));

        let mut decoder = Decoder::new(FrameLimits {
            max_frame_bytes: frame.len(),
            max_frames: 1,
        })
        .expect("limits hold one header");
        let owned = decoder.push(&frame).expect("one owned frame");
        let owned_parsed = owned[0]
            .incoming_actions(limits(3))
            .expect("owned frame matches");
        assert_eq!(owned_parsed.prefix, parsed.prefix);
        assert_eq!(owned_parsed.checksum_prefix, parsed.checksum_prefix);
        assert_eq!(owned_parsed.records, parsed.records);
        assert!(borrowed_from(
            owned[0].body(),
            owned_parsed.records[0].payload
        ));

        let empty = encode(
            super::super::W3GS_FAMILY,
            W3GS_INCOMING_ACTION,
            &[0x00, 0x00],
        )
        .expect("empty queue frame");
        let empty = decode_one(&empty, 32).expect("empty queue header");
        let empty = empty.incoming_actions(limits(0)).expect("empty queue");
        assert_eq!(empty.prefix, IncomingPrefix::Interval(0));
        assert_eq!(empty.checksum_prefix, None);
        assert!(empty.records.is_empty());

        let mut overflow = vec![0x00, 0x00, 0x00, 0x00];
        overflow.extend_from_slice(&[7, 0x05, 0x00, 0xFF]);
        let overflow = encode(super::super::W3GS_FAMILY, W3GS_INCOMING_ACTION, &overflow)
            .expect("short payload frame");
        let overflow = decode_one(&overflow, 32).expect("short payload header");
        assert_eq!(
            overflow.incoming_actions(limits(4)),
            Err(ActionError::LengthOverflow {
                declared: 5,
                available: 1,
            })
        );

        let truncated = encode(
            super::super::W3GS_FAMILY,
            W3GS_INCOMING_ACTION,
            &[0x00, 0x00, 0x11, 0x22, 0x01, 0x02],
        )
        .expect("truncated header frame");
        let truncated = decode_one(&truncated, 32).expect("truncated header accepted");
        assert_eq!(
            truncated.incoming_actions(limits(4)),
            Err(ActionError::Truncated { have: 2, need: 3 })
        );

        let checksum_only = encode(
            super::super::W3GS_FAMILY,
            W3GS_INCOMING_ACTION,
            &[0x00, 0x00, 0x11, 0x22],
        )
        .expect("checksum without subrecord");
        let checksum_only = decode_one(&checksum_only, 32).expect("checksum header");
        assert_eq!(
            checksum_only.incoming_actions(limits(4)),
            Err(ActionError::Truncated { have: 0, need: 3 })
        );

        assert_eq!(
            decoded.incoming_actions(limits(1)),
            Err(ActionError::CountOverflow { limit: 1 })
        );

        let mut later_short = vec![0x00, 0x00, 0x00, 0x00];
        record(&mut later_short, 1, &[0x10]);
        later_short.push(0x02);
        let later_short = encode(
            super::super::W3GS_FAMILY,
            W3GS_INCOMING_ACTION,
            &later_short,
        )
        .expect("trailing byte frame");
        let later_short = decode_one(&later_short, 32).expect("trailing byte header");
        assert_eq!(
            later_short.incoming_actions(limits(1)),
            Err(ActionError::Truncated { have: 1, need: 3 })
        );

        let mut action2 = vec![0x0A, 0x0B, 0x01, 0x02];
        record(&mut action2, 3, &[4]);
        let action2 = encode(super::super::W3GS_FAMILY, W3GS_INCOMING_ACTION2, &action2)
            .expect("action2 frame");
        let action2 = decode_one(&action2, 32).expect("action2 header");
        let action2 = action2.incoming_actions(limits(1)).expect("action2 record");
        assert_eq!(action2.prefix, IncomingPrefix::Raw([0x0A, 0x0B]));
        assert_eq!(action2.records[0].player_id, 3);
        assert_eq!(action2.records[0].payload, &[4]);

        let outgoing_body = [1, 2, 3, 4, 9, 8, 7];
        let outgoing = encode(
            super::super::W3GS_FAMILY,
            W3GS_OUTGOING_ACTION,
            &outgoing_body,
        )
        .expect("outgoing frame");
        let outgoing = decode_one(&outgoing, 32).expect("outgoing header");
        let kept = outgoing.outgoing_action(4).expect("player 4");
        assert_eq!(kept.player_id, 4);
        assert_eq!(*kept.checksum, [1, 2, 3, 4]);
        assert_eq!(kept.payload, &[9, 8, 7]);
        assert!(borrowed_from(outgoing.body(), kept.payload));
        assert!(borrowed_from(outgoing.body(), kept.checksum));
        assert_eq!(
            outgoing.outgoing_action(OUTGOING_REFUSED_PLAYER_ID),
            Err(ActionError::UnassignedPlayer)
        );

        let short = encode(super::super::W3GS_FAMILY, W3GS_OUTGOING_ACTION, &[1, 2, 3])
            .expect("short outgoing");
        let short = decode_one(&short, 32).expect("short outgoing header");
        assert_eq!(
            short.outgoing_action(1),
            Err(ActionError::Truncated { have: 3, need: 4 })
        );

        let other = encode(super::super::W3GS_FAMILY, 1, &[]).expect("other id");
        let other = decode_one(&other, 32).expect("other header");
        assert_eq!(
            other.incoming_actions(limits(1)),
            Err(ActionError::WrongId(1))
        );
        let bncs = encode(BNCS_FAMILY, W3GS_INCOMING_ACTION, &[0, 0]).expect("bncs frame");
        let bncs = decode_one(&bncs, 32).expect("bncs header");
        assert_eq!(
            bncs.incoming_actions(limits(1)),
            Err(ActionError::WrongFamily(BNCS_FAMILY))
        );
    }
    #[test]
    fn root_valid_records_do_not_hide_a_malformed_tail() {
        for id in [W3GS_INCOMING_ACTION, W3GS_INCOMING_ACTION2] {
            let mut body = vec![1, 2, 3, 4];
            record(&mut body, 7, &[8, 9]);
            let valid = encode(super::super::W3GS_FAMILY, id, &body).unwrap();
            assert_eq!(
                decode_one(&valid, 64)
                    .unwrap()
                    .incoming_actions(limits(1))
                    .unwrap()
                    .records
                    .len(),
                1
            );
            body.push(10);
            let short = encode(super::super::W3GS_FAMILY, id, &body).unwrap();
            assert!(matches!(
                decode_one(&short, 64).unwrap().incoming_actions(limits(2)),
                Err(ActionError::Truncated { .. })
            ));
            body.extend_from_slice(&[2, 0, 11]);
            let overflow = encode(super::super::W3GS_FAMILY, id, &body).unwrap();
            assert!(matches!(
                decode_one(&overflow, 64)
                    .unwrap()
                    .incoming_actions(limits(2)),
                Err(ActionError::LengthOverflow {
                    declared: 2,
                    available: 1
                })
            ));
        }
    }
}
