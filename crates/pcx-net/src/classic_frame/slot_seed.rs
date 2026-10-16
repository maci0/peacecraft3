//! Slot-info and slot-info-join bodies from pinned GHost++ `6ba90f426e96f80fe013a468d1e07151a510adbe`.
//!
//! `EncodeSlotInfo` in `ghost/gameprotocol.cpp` writes the slot count, each
//! `CGameSlot::GetByteArray()` from `ghost/gameslot.cpp`, then `randomSeed`
//! with the same `false` byte order as `AssignLength`, then `layoutStyle` and
//! `playerSlots`. `SEND_W3GS_SLOTINFO` and `SEND_W3GS_SLOTINFOJOIN` prefix that
//! blob with a `uint16` length. The join packet then appends its fixed suffix.
//! `SEND_W3GS_COUNTDOWN_START` carries no seed.
//!
//! The caller names the packet profile. `playerSlots` is not a record count.
//! Short slot records are not filled from the `CGameSlot(BYTEARRAY)` defaults.
//! The seed remains the four source bytes. This is not an RNG stream, a lobby
//! store, or an original-client match.

use super::{BNCS_FAMILY, Frame, OwnedFrame, W3GS_FAMILY};

/// Pinned GHost++ revision for the layout this module reads.
pub const GHOST_REVISION: &str = "6ba90f426e96f80fe013a468d1e07151a510adbe";

/// `CGameProtocol::W3GS_SLOTINFOJOIN`.
pub const SLOTINFOJOIN_ID: u8 = 4;

/// `CGameProtocol::W3GS_SLOTINFO`.
pub const SLOTINFO_ID: u8 = 9;

/// Fields `CGameSlot::GetByteArray` appends, in source order.
const SLOT_FIELD_COUNT: usize = 9;

/// `randomSeed` width in `EncodeSlotInfo`.
const SEED_LEN: usize = 4;

/// `layoutStyle` plus `playerSlots`.
const SLOT_TAIL_LEN: usize = 2;

/// Bytes after the length-prefixed blob in `SEND_W3GS_SLOTINFOJOIN`.
///
/// PID, two address-family bytes, two port bytes, four external-IP bytes, and
/// the two four-byte regions the source marks as unspecified.
const JOIN_SUFFIX_LEN: usize = 1 + 2 + 2 + 4 + 4 + 4;

/// Which length-prefixed slot packet the caller is reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotSeedProfile {
    /// `W3GS_SLOTINFO`. The body ends at the length-prefixed slot blob.
    SlotInfo,
    /// `W3GS_SLOTINFOJOIN`. The same blob, then the join suffix.
    SlotInfoJoin,
}

impl SlotSeedProfile {
    /// Header identifier byte for this profile.
    pub const fn id(self) -> u8 {
        match self {
            Self::SlotInfo => SLOTINFO_ID,
            Self::SlotInfoJoin => SLOTINFOJOIN_ID,
        }
    }
}

/// One `GetByteArray` record. Each value is the source byte, unchecked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSlot {
    /// `m_PID`.
    pub pid: u8,
    /// `m_DownloadStatus`.
    pub download_status: u8,
    /// `m_SlotStatus`.
    pub slot_status: u8,
    /// `m_Computer`.
    pub computer: u8,
    /// `m_Team`.
    pub team: u8,
    /// `m_Colour`.
    pub colour: u8,
    /// `m_Race`.
    pub race: u8,
    /// `m_ComputerType`.
    pub computer_type: u8,
    /// `m_Handicap`.
    pub handicap: u8,
}

impl RawSlot {
    /// The nine bytes in `GetByteArray` order.
    pub fn bytes(self) -> [u8; SLOT_FIELD_COUNT] {
        [
            self.pid,
            self.download_status,
            self.slot_status,
            self.computer,
            self.team,
            self.colour,
            self.race,
            self.computer_type,
            self.handicap,
        ]
    }
}

/// `EncodeSlotInfo` payload after its length prefix has been removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedSlotInfo {
    /// Records named by the count byte. Empty when that byte is zero.
    pub slots: Vec<RawSlot>,
    /// Four `randomSeed` bytes, source order.
    pub random_seed: [u8; SEED_LEN],
    /// `layoutStyle`, including values outside the source comment.
    pub layout_style: u8,
    /// `playerSlots` (non-observer count in the source comment). Not `slots.len()`.
    pub player_slots: u8,
}

/// Fixed suffix of `SEND_W3GS_SLOTINFOJOIN`. Bytes are not checked against the
/// constants the sender writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinSuffix {
    /// Receiver PID.
    pub pid: u8,
    /// Two address-family bytes.
    pub address_family: [u8; 2],
    /// Port bytes in source order.
    pub port: [u8; 2],
    /// External IP bytes.
    pub external_ip: [u8; 4],
    /// First unspecified four-byte region.
    pub unspecified_0: [u8; 4],
    /// Second unspecified four-byte region.
    pub unspecified_1: [u8; 4],
}

/// A profile-qualified slot-info body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotSeedRecord {
    /// `W3GS_SLOTINFO`.
    SlotInfo(EncodedSlotInfo),
    /// `W3GS_SLOTINFOJOIN`, with its suffix kept separate from the slot records.
    SlotInfoJoin {
        /// Shared slot blob.
        info: EncodedSlotInfo,
        /// Bytes that follow the blob only on this profile.
        join: JoinSuffix,
    },
}

impl SlotSeedRecord {
    /// Slot blob for either profile.
    pub fn info(&self) -> &EncodedSlotInfo {
        match self {
            Self::SlotInfo(info) | Self::SlotInfoJoin { info, .. } => info,
        }
    }
}

/// Why a validated frame was not a slot-info or slot-info-join body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotSeedError {
    /// Header family was not W3GS. BNCS is named when that is the byte seen.
    Family {
        /// Header family byte.
        found: u8,
    },
    /// Header id was not the caller's profile.
    Profile {
        /// Identifier the caller requested.
        expected: u8,
        /// Identifier on the frame.
        found: u8,
    },
    /// The body does not contain the length prefix, the declared blob, or the
    /// join suffix the profile requires.
    Truncated {
        /// Body bytes present.
        have: usize,
        /// Body bytes that profile still needs.
        need: usize,
    },
    /// The declared blob is fully present and its length is not
    /// `1 + count * 9 + 6`.
    CountSize {
        /// Count byte when the declared blob contains one.
        count: Option<u8>,
        /// Little-endian length prefix.
        declared: usize,
        /// `EncodeSlotInfo` size for `count`, when a count byte was present.
        encoded: Option<usize>,
    },
    /// Bytes follow a complete profile body.
    Trailing {
        /// Bytes after the profile body.
        remaining: usize,
    },
    /// `try_reserve` failed before copying slot records.
    Allocation,
}

impl std::fmt::Display for SlotSeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Family { found } => {
                if *found == BNCS_FAMILY {
                    write!(f, "BNCS frame is not a W3GS slot-info body")
                } else {
                    write!(f, "header family {found} is not W3GS")
                }
            }
            Self::Profile { expected, found } => {
                write!(
                    f,
                    "header id {found} does not match slot profile {expected}"
                )
            }
            Self::Truncated { have, need } => {
                write!(f, "slot-info body has {have} bytes and needs {need}")
            }
            Self::CountSize {
                count,
                declared,
                encoded,
            } => match (count, encoded) {
                (Some(count), Some(encoded)) => write!(
                    f,
                    "slot count {count} encodes {encoded} bytes, length prefix says {declared}"
                ),
                _ => write!(
                    f,
                    "slot-info length prefix {declared} does not contain a count and seed"
                ),
            },
            Self::Trailing { remaining } => {
                write!(f, "trailing bytes after slot-info body: {remaining}")
            }
            Self::Allocation => write!(f, "slot record allocation failed"),
        }
    }
}

impl std::error::Error for SlotSeedError {}

impl Frame<'_> {
    /// Reads the caller's slot profile from this accepted frame.
    pub fn slot_seed(self, profile: SlotSeedProfile) -> Result<SlotSeedRecord, SlotSeedError> {
        decode(self.family(), self.id(), self.body(), profile)
    }
}

impl OwnedFrame {
    /// Reads the caller's slot profile from this accepted frame.
    pub fn slot_seed(&self, profile: SlotSeedProfile) -> Result<SlotSeedRecord, SlotSeedError> {
        decode(self.family(), self.id(), self.body(), profile)
    }
}

fn encoded_len(count: u8) -> usize {
    1 + usize::from(count) * SLOT_FIELD_COUNT + SEED_LEN + SLOT_TAIL_LEN
}

fn decode(
    family: u8,
    id: u8,
    body: &[u8],
    profile: SlotSeedProfile,
) -> Result<SlotSeedRecord, SlotSeedError> {
    if family != W3GS_FAMILY {
        return Err(SlotSeedError::Family { found: family });
    }
    let expected = profile.id();
    if id != expected {
        return Err(SlotSeedError::Profile {
            expected,
            found: id,
        });
    }
    if body.len() < 2 {
        return Err(SlotSeedError::Truncated {
            have: body.len(),
            need: 2,
        });
    }
    let declared = u16::from_le_bytes([body[0], body[1]]) as usize;
    let blob_end = 2 + declared;
    if body.len() < blob_end {
        return Err(SlotSeedError::Truncated {
            have: body.len(),
            need: blob_end,
        });
    }
    let suffix_len = match profile {
        SlotSeedProfile::SlotInfo => 0,
        SlotSeedProfile::SlotInfoJoin => JOIN_SUFFIX_LEN,
    };
    let total = blob_end + suffix_len;
    if body.len() < total {
        return Err(SlotSeedError::Truncated {
            have: body.len(),
            need: total,
        });
    }
    if body.len() > total {
        return Err(SlotSeedError::Trailing {
            remaining: body.len() - total,
        });
    }
    let info = decode_blob(&body[2..blob_end], declared)?;
    Ok(match profile {
        SlotSeedProfile::SlotInfo => SlotSeedRecord::SlotInfo(info),
        SlotSeedProfile::SlotInfoJoin => SlotSeedRecord::SlotInfoJoin {
            info,
            join: decode_join(&body[blob_end..total])?,
        },
    })
}

fn decode_blob(blob: &[u8], declared: usize) -> Result<EncodedSlotInfo, SlotSeedError> {
    if declared == 0 || blob.is_empty() {
        return Err(SlotSeedError::CountSize {
            count: None,
            declared,
            encoded: None,
        });
    }
    let count = blob[0];
    let encoded = encoded_len(count);
    if declared != encoded || blob.len() != encoded {
        return Err(SlotSeedError::CountSize {
            count: Some(count),
            declared,
            encoded: Some(encoded),
        });
    }
    let mut slots = Vec::new();
    slots
        .try_reserve(usize::from(count))
        .map_err(|_| SlotSeedError::Allocation)?;
    for index in 0..usize::from(count) {
        let at = 1 + index * SLOT_FIELD_COUNT;
        let record = blob
            .get(at..at + SLOT_FIELD_COUNT)
            .ok_or(SlotSeedError::CountSize {
                count: Some(count),
                declared,
                encoded: Some(encoded),
            })?;
        slots.push(RawSlot {
            pid: record[0],
            download_status: record[1],
            slot_status: record[2],
            computer: record[3],
            team: record[4],
            colour: record[5],
            race: record[6],
            computer_type: record[7],
            handicap: record[8],
        });
    }
    let seed_at = 1 + usize::from(count) * SLOT_FIELD_COUNT;
    let Some(seed) = blob.get(seed_at..seed_at + SEED_LEN) else {
        return Err(SlotSeedError::CountSize {
            count: Some(count),
            declared,
            encoded: Some(encoded),
        });
    };
    let random_seed = [seed[0], seed[1], seed[2], seed[3]];
    let tail = seed_at + SEED_LEN;
    if blob.len() != tail + SLOT_TAIL_LEN {
        return Err(SlotSeedError::CountSize {
            count: Some(count),
            declared,
            encoded: Some(encoded),
        });
    }
    Ok(EncodedSlotInfo {
        slots,
        random_seed,
        layout_style: blob[tail],
        player_slots: blob[tail + 1],
    })
}

fn decode_join(suffix: &[u8]) -> Result<JoinSuffix, SlotSeedError> {
    if suffix.len() != JOIN_SUFFIX_LEN {
        return Err(SlotSeedError::Truncated {
            have: suffix.len(),
            need: JOIN_SUFFIX_LEN,
        });
    }
    Ok(JoinSuffix {
        pid: suffix[0],
        address_family: [suffix[1], suffix[2]],
        port: [suffix[3], suffix[4]],
        external_ip: [suffix[5], suffix[6], suffix[7], suffix[8]],
        unspecified_0: [suffix[9], suffix[10], suffix[11], suffix[12]],
        unspecified_1: [suffix[13], suffix[14], suffix[15], suffix[16]],
    })
}

#[cfg(test)]
mod tests {
    use super::super::{Decoder, FrameLimits, W3GS_FAMILY, decode_one, encode};
    use super::{
        EncodedSlotInfo, SLOTINFO_ID, SLOTINFOJOIN_ID, SlotSeedError, SlotSeedProfile,
        SlotSeedRecord,
    };

    const SEED: [u8; 4] = [0x11, 0x22, 0x33, 0x44];
    const SLOT: [u8; 9] = [0xA1, 0x07, 0x02, 0x00, 0x03, 0x04, 0x20, 0x02, 0x63];

    fn length_prefix(blob: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&(blob.len() as u16).to_le_bytes());
        body.extend_from_slice(blob);
        body
    }

    fn one_slot_blob() -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(1);
        blob.extend_from_slice(&SLOT);
        blob.extend_from_slice(&SEED);
        blob.push(0x03);
        blob.push(0x0C);
        blob
    }

    fn zero_slot_blob() -> Vec<u8> {
        let mut blob = vec![0];
        blob.extend_from_slice(&SEED);
        blob.push(0x01);
        blob.push(0x0C);
        blob
    }

    fn framed(id: u8, body: &[u8]) -> Vec<u8> {
        encode(W3GS_FAMILY, id, body).unwrap()
    }

    fn parse(packet: &[u8], profile: SlotSeedProfile) -> Result<SlotSeedRecord, SlotSeedError> {
        decode_one(packet, packet.len()).unwrap().slot_seed(profile)
    }

    fn assert_seed(info: &EncodedSlotInfo, slots: usize) {
        assert_eq!(info.random_seed, SEED);
        assert_eq!(u32::from_le_bytes(info.random_seed), 0x4433_2211);
        assert_eq!(info.slots.len(), slots);
        assert_eq!(info.player_slots, 0x0C);
        assert_ne!(info.slots.len(), usize::from(info.player_slots));
    }

    #[test]
    fn every_incomplete_body_and_wrong_packet_profile_is_refused() {
        for profile in [SlotSeedProfile::SlotInfo, SlotSeedProfile::SlotInfoJoin] {
            let mut body = length_prefix(&one_slot_blob());
            if profile == SlotSeedProfile::SlotInfoJoin {
                body.extend_from_slice(&[0; 17]);
            }
            for end in 0..body.len() {
                assert!(
                    parse(&framed(profile.id(), &body[..end]), profile).is_err(),
                    "accepted {end} bytes of {profile:?}"
                );
            }
            let wrong_family = encode(super::BNCS_FAMILY, profile.id(), &body).unwrap();
            assert!(matches!(
                parse(&wrong_family, profile),
                Err(SlotSeedError::Family { .. })
            ));
            let wrong_id = framed(profile.id() ^ 0xff, &body);
            assert!(matches!(
                parse(&wrong_id, profile),
                Err(SlotSeedError::Profile { .. })
            ));
        }
        assert!(matches!(
            parse(&framed(SLOTINFO_ID, &[0, 0]), SlotSeedProfile::SlotInfo),
            Err(SlotSeedError::CountSize { count: None, .. })
        ));
    }

    #[test]
    fn decoded_frame_keeps_seed_and_rejects_truncation_and_count_mismatch() {
        let body = length_prefix(&one_slot_blob());
        let parsed =
            parse(&framed(SLOTINFO_ID, &body), SlotSeedProfile::SlotInfo).expect("one slot record");
        let SlotSeedRecord::SlotInfo(info) = parsed.clone() else {
            panic!("slot-info profile returned {parsed:?}");
        };
        assert_seed(&info, 1);
        assert_eq!(info.layout_style, 0x03);
        assert_eq!(info.slots[0].bytes(), SLOT);
        assert_eq!(info.slots[0].handicap, 0x63);
        assert_eq!(info.slots[0].download_status, 0x07);

        let mut decoder = Decoder::new(FrameLimits {
            max_frame_bytes: 64,
            max_frames: 1,
        })
        .unwrap();
        let packet = encode(W3GS_FAMILY, SLOTINFO_ID, &body).unwrap();
        let owned = decoder.push(&packet).unwrap();
        assert_eq!(owned.len(), 1);
        assert_eq!(
            owned[0].slot_seed(SlotSeedProfile::SlotInfo).unwrap(),
            parsed
        );

        let SlotSeedRecord::SlotInfo(empty_info) = parse(
            &framed(SLOTINFO_ID, &length_prefix(&zero_slot_blob())),
            SlotSeedProfile::SlotInfo,
        )
        .unwrap() else {
            panic!("zero records still use the slot-info profile");
        };
        assert_seed(&empty_info, 0);
        assert!(empty_info.slots.is_empty());

        assert_eq!(
            parse(
                &framed(SLOTINFO_ID, &[0x14, 0x00, 0x01, 0x02]),
                SlotSeedProfile::SlotInfo
            )
            .unwrap_err(),
            SlotSeedError::Truncated { have: 4, need: 22 }
        );

        let mismatch = length_prefix(&[0x02, 0x11, 0x22, 0x33, 0x44, 0x01, 0x08]);
        assert_eq!(
            parse(&framed(SLOTINFO_ID, &mismatch), SlotSeedProfile::SlotInfo).unwrap_err(),
            SlotSeedError::CountSize {
                count: Some(2),
                declared: 7,
                encoded: Some(25),
            }
        );

        let mut join_body = length_prefix(&one_slot_blob());
        join_body.extend_from_slice(&[
            0x02, 0x02, 0x00, 0x61, 0x1F, 10, 0, 0, 1, 9, 8, 7, 6, 5, 4, 3, 2,
        ]);
        let SlotSeedRecord::SlotInfoJoin { info, join } = parse(
            &framed(SLOTINFOJOIN_ID, &join_body),
            SlotSeedProfile::SlotInfoJoin,
        )
        .unwrap() else {
            panic!("join profile dropped its suffix");
        };
        assert_seed(&info, 1);
        assert_eq!(info.slots[0].bytes(), SLOT);
        assert_eq!(join.pid, 0x02);
        assert_eq!(join.address_family, [0x02, 0x00]);
        assert_eq!(join.port, [0x61, 0x1F]);
        assert_eq!(join.external_ip, [10, 0, 0, 1]);
        assert_eq!(join.unspecified_0, [9, 8, 7, 6]);
        assert_eq!(join.unspecified_1, [5, 4, 3, 2]);
        assert!(matches!(
            parse(&framed(SLOTINFO_ID, &join_body), SlotSeedProfile::SlotInfo),
            Err(SlotSeedError::Trailing { remaining: 17 })
        ));
        let mut cut = join_body.clone();
        cut.pop();
        assert_eq!(
            parse(
                &framed(SLOTINFOJOIN_ID, &cut),
                SlotSeedProfile::SlotInfoJoin
            )
            .unwrap_err(),
            SlotSeedError::Truncated {
                have: cut.len(),
                need: cut.len() + 1,
            }
        );
    }
}
