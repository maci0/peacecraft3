//! Gathering/policy flag operands from the reviewed client image. This is a
//! source reader: it decodes instruction operands and does not implement world,
//! dispatch, or candidate-selection semantics, and it does not map context-kind
//! to `StartCampaignAI`.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct GatheringPolicy {
    pub image_sha256: Sha256Digest,
    pub context_gathering_mask: u32,
    pub structure_mask: u32,
    pub excluded_state_mask: u32,
    pub excluded_state_value: u32,
    pub immediate_candidate_mask: u32,
    pub initial_flags: u32,
    pub conditional_initial_mask: u32,
    pub conditional_excluded_kind: u32,
    pub harvest_current_masks: [u8; 2],
    pub harvest_previous_masks: [u8; 2],
    pub harvest_town_slots: u32,
}

#[derive(Debug, thiserror::Error)]
#[error("campaign gathering policy: {0}")]
pub struct Error(&'static str);

// VAs and lengths identify instruction windows in the reviewed PE. Every value
// returned below is decoded from an operand within those windows.
const WINDOWS: [(u32, usize, bool); 17] = [
    (0xac3f5b, 7, true),   // context clear (andl $-2, ctx+0x2d0)
    (0xabb4e9, 7, true),   // context reader (testb $1, ctx+0x2d0)
    (0xabf9e2, 10, true),  // constructor zero-initialize
    (0xabfa1e, 16, true),  // constructor conditional OR (kind gate)
    (0x696b50, 49, true),  // candidate structure/excluded-state predicate
    (0xac3f9d, 7, true),   // immediate-candidate mask (testb $0x10, unit+0x454)
    (0xab63b9, 20, true),  // ClearHarvestAI primary registration
    (0xab44d0, 54, true),  // owned-context handler
    (0xe70fac, 4, false),  // ()V
    (0xf15cb8, 15, false), // ClearHarvestAI
    (0xabc550, 176, true), // assignment rollover, without unit orders
    (0x6933c0, 27, true),  // canonical unit's AI-component accessor
    (0xab63cd, 20, true),  // HarvestGold registration
    (0xab57b0, 84, true),  // positive-count/owned-context forwarding
    (0xabf11a, 9, true),   // unsigned town bound
    (0xe6f5ec, 6, false),  // (II)V
    (0xf15cc8, 12, false), // HarvestGold
];

fn at(bytes: &[u8], offset: usize, expected: &[u8]) -> Result<(), Error> {
    (bytes.get(offset..offset + expected.len()) == Some(expected))
        .then_some(())
        .ok_or(Error("reviewed instruction relationship changed"))
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(Error("truncated instruction operand"))?
            .try_into()
            .map_err(|_| Error("truncated instruction operand"))?,
    ))
}

fn budgeted_windows(budget: RuleInputBudget) -> Result<(), Error> {
    let mut total = 0usize;
    for (_, len, _) in WINDOWS {
        total = total
            .checked_add(len)
            .ok_or(Error("window budget exceeded"))?;
        if total > budget.profile_bytes {
            return Err(Error("window budget exceeded"));
        }
    }
    Ok(())
}

fn relative(bytes: &[u8], va: u32, offset: usize, opcode: u8, target: u32) -> Result<(), Error> {
    at(bytes, offset, &[opcode])?;
    (va.wrapping_add(offset as u32 + 5)
        .wrapping_add(word(bytes, offset + 1)?)
        == target)
        .then_some(())
        .ok_or(Error("harvest control-flow target changed"))
}

fn harvest_windows(w: &[&[u8]]) -> Result<([u8; 2], [u8; 2]), Error> {
    let [registration, handler, signature, name, body, accessor] = w else {
        return Err(Error("incomplete harvest windows"));
    };
    for (offset, target) in [(0, WINDOWS[8].0), (5, WINDOWS[9].0), (10, WINDOWS[7].0)] {
        at(registration, offset, &[0x68])?;
        if word(registration, offset + 1)? != target {
            return Err(Error("harvest registration pointer changed"));
        }
    }
    relative(registration, WINDOWS[6].0, 15, 0xe8, 0x8ccc80)?;
    at(signature, 0, b"()V\0")?;
    at(name, 0, b"ClearHarvestAI\0")?;
    at(
        handler,
        0,
        &[
            0xa1, 0x10, 0x4a, 0x1b, 0x01, 0x85, 0xc0, 0x75, 0x0b, 0x6a, 0x57,
        ],
    )?;
    relative(handler, WINDOWS[7].0, 11, 0xe8, 0x43eca0)?;
    at(
        handler,
        16,
        &[
            0x83, 0xc4, 0x04, 0xc3, 0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8,
            0xff, 0x75, 0x09, 0x33, 0xc0, 0x8b, 0xc8,
        ],
    )?;
    relative(handler, WINDOWS[7].0, 37, 0xe9, WINDOWS[10].0)?;
    relative(handler, WINDOWS[7].0, 42, 0xe8, 0x7fc7e0)?;
    at(handler, 47, &[0x8b, 0xc8])?;
    relative(handler, WINDOWS[7].0, 49, 0xe9, WINDOWS[10].0)?;
    at(
        accessor,
        0,
        &[
            0x8b, 0x81, 0x14, 0x05, 0, 0, 0x81, 0xc1, 0x10, 0x05, 0, 0, 0x23, 0x01, 0x83, 0xf8,
            0xff, 0x75, 0x03, 0x33, 0xc0, 0xc3,
        ],
    )?;
    relative(accessor, WINDOWS[11].0, 22, 0xe9, 0x7fc7e0)?;
    at(
        body,
        0,
        &[
            0x8b, 0x41, 0x2c, 0x83, 0xc1, 0x28, 0x56, 0x57, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75,
            0x2d, 0x33, 0xf6, 0x33, 0xc9, 0x33, 0xff, 0x85, 0xc9, 0x0f, 0x84, 0x90, 0, 0, 0, 0x0f,
            0x1f, 0,
        ],
    )?;
    relative(body, WINDOWS[10].0, 32, 0xe8, WINDOWS[11].0)?;
    at(body, 37, &[0x85, 0xc0, 0x74, 0x44])?;
    relative(body, WINDOWS[10].0, 60, 0xe8, 0x7fc7e0)?;
    at(body, 65, &[0x8b, 0xf0, 0x85, 0xf6, 0x74, 0xca, 0x8b, 0xce])?;
    relative(body, WINDOWS[10].0, 73, 0xe8, 0x7f2610)?;
    at(body, 78, &[0x8b, 0xc8, 0xeb, 0xc1])?;
    let field = body[43];
    let mut current = [0; 2];
    let mut previous = [0; 2];
    let mut used = 0u8;
    for (lane, read, test, branch, clear, set, store, end, remove) in [
        (0, 41, 44, 47, 49, 52, 55, 58, 82),
        (1, 86, 89, 92, 94, 97, 100, 103, 105),
    ] {
        at(body, read, &[0x8b, 0x48, field])?;
        at(body, test, &[0xf6, 0xc1])?;
        at(
            body,
            branch,
            if lane == 0 {
                &[0x74, 0x21]
            } else {
                &[0x74, 0x0b]
            },
        )?;
        at(body, clear, &[0x83, 0xe1])?;
        at(body, set, &[0x83, 0xc9])?;
        at(body, store, &[0x89, 0x48, field])?;
        at(
            body,
            end,
            if lane == 0 {
                &[0xeb, 0x1a]
            } else {
                &[0xeb, 0x04]
            },
        )?;
        at(body, remove, &[0x83, 0x60, field])?;
        current[lane] = body[test + 2];
        previous[lane] = body[set + 2];
        let a = current[lane];
        let b = previous[lane];
        if !a.is_power_of_two()
            || !b.is_power_of_two()
            || (a | b) & 0x80 != 0
            || a == b
            || used & (a | b) != 0
            || body[clear + 2] != !a
            || body[remove + 3] != !b
        {
            return Err(Error(
                "harvest current/previous masks or signed clears differ",
            ));
        }
        used |= a | b;
    }
    at(
        body,
        109,
        &[
            0x85, 0xf6, 0x74, 0x27, 0x8b, 0x46, 0x28, 0x8d, 0x4e, 0x24, 0x23, 0x01, 0x83, 0xf8,
            0xff, 0x75, 0x04, 0x33, 0xf6, 0xeb, 0x07,
        ],
    )?;
    relative(body, WINDOWS[10].0, 130, 0xe8, 0x7fc7e0)?;
    at(body, 135, &[0x8b, 0xf0, 0x85, 0xf6, 0x74, 0x0d, 0x8b, 0xce])?;
    relative(body, WINDOWS[10].0, 143, 0xe8, 0x7f2610)?;
    at(
        body,
        148,
        &[
            0x8b, 0xc8, 0xeb, 0x04, 0x33, 0xf6, 0x8b, 0xcf, 0x33, 0xc0, 0x85, 0xf6, 0x0f, 0x45,
            0xc7, 0x8b, 0xf8, 0x85, 0xc9, 0x0f, 0x85, 0x73, 0xff, 0xff, 0xff, 0x5f, 0x5e, 0xc3,
        ],
    )?;
    Ok((current, previous))
}

impl GatheringPolicy {
    pub(crate) fn clear_harvest_flags(&self, mut flags: u8) -> u8 {
        for (current, previous) in self
            .harvest_current_masks
            .into_iter()
            .zip(self.harvest_previous_masks)
        {
            let assigned = flags & current != 0;
            flags &= !(current | previous);
            if assigned {
                flags |= previous;
            }
        }
        flags
    }

    /// The context bit is set on construction unless the context kind equals the
    /// excluded kind (source comparison). Consumers must resolve context-kind to
    /// `StartCampaignAI` separately; do not guess a kind value here.
    pub fn gathering_default(&self, context_kind: u32) -> bool {
        let conditional = if context_kind != self.conditional_excluded_kind {
            self.conditional_initial_mask
        } else {
            0
        };
        ((self.initial_flags | conditional) & self.context_gathering_mask) != 0
    }

    pub fn read(image: &[u8], budget: RuleInputBudget) -> Result<Self, Error> {
        if image.len() > budget.image_bytes {
            return Err(Error("image budget exceeded"));
        }
        budgeted_windows(budget)?;
        if sha256(image) != REVIEWED_IMAGE {
            return Err(Error("unsupported client image"));
        }
        let pe = Pe::parse(image, budget).map_err(|_| Error("invalid selected PE"))?;
        let mut windows = Vec::with_capacity(WINDOWS.len());
        for (va, len, code) in WINDOWS {
            windows.push(
                pe.read_from(va, code)
                    .map_err(|_| Error("missing selected instruction window"))?
                    .get(..len)
                    .ok_or(Error("short selected instruction window"))?,
            );
        }
        Self::decode(&windows, REVIEWED_IMAGE)
    }

    fn decode(w: &[&[u8]], image_sha256: Sha256Digest) -> Result<Self, Error> {
        if w.len() != WINDOWS.len()
            || w.iter()
                .zip(WINDOWS)
                .any(|(bytes, (_, len, _))| bytes.len() < len)
        {
            return Err(Error("incomplete gathering-policy windows"));
        }
        let [clear, reader, init_zero, init_cond, candidate, immediate] = &w[..6] else {
            return Err(Error("incomplete gathering-policy windows"));
        };
        let (harvest_current_masks, harvest_previous_masks) = harvest_windows(&w[6..12])?;
        let harvest_town_slots = gold_windows(&w[12..])?;

        at(clear, 0, &[0x83, 0xa0])?;
        let ctx_offset = word(clear, 2)?;
        let clear_imm = *clear.get(6).ok_or(Error("missing clear operand"))?;
        // `andl r/m32, imm8` sign-extends the imm8; the real clear is a
        // negative widened mask (e.g. 0xfe -> 0xfffffffe). Reject a clear imm
        // whose sign bit is clear so an 8-bit-wide "mask" cannot masquerade as
        // a 32-bit clear.
        if clear_imm & 0x80 == 0 {
            return Err(Error(
                "context clear imm8 does not sign-extend a wide clear",
            ));
        }
        let ctx_mask = u32::from(!clear_imm);
        if ctx_mask == 0 || ctx_mask.count_ones() != 1 {
            return Err(Error("context gathering mask is not a single cleared bit"));
        }

        at(reader, 0, &[0xf6, 0x80])?;
        if word(reader, 2)? != ctx_offset {
            return Err(Error("context gathering reader offset differs from clear"));
        }
        let reader_mask = *reader.get(6).ok_or(Error("missing reader operand"))?;
        if reader_mask & 0x80 != 0 {
            return Err(Error("context gathering reader mask is a negative imm8"));
        }
        if u32::from(reader_mask) != ctx_mask {
            return Err(Error("context gathering reader mask differs from clear"));
        }

        at(init_zero, 0, &[0xc7, 0x86])?;
        if word(init_zero, 2)? != ctx_offset {
            return Err(Error("gathering constructor offset differs from clear"));
        }
        let initial_flags = word(init_zero, 6)?;
        if initial_flags != 0 {
            return Err(Error("gathering initial flags are not zero"));
        }

        at(init_cond, 0, &[0x83, 0xf8])?;
        let kind_byte = *init_cond
            .get(2)
            .ok_or(Error("missing excluded-kind operand"))?;
        if kind_byte & 0x80 != 0 {
            return Err(Error("excluded kind is a negative imm8"));
        }
        let conditional_excluded_kind = u32::from(kind_byte);
        at(init_cond, 3, &[0x74])?;
        // The conditional OR occupies bytes 5..15; the JE skip must land just
        // past it (displacement 0x0a -> target 15).
        let skip = *init_cond.get(4).ok_or(Error("missing excluded-kind JE"))? as i8;
        if skip < 0 || 5 + skip as usize != 15 {
            return Err(Error("gathering excluded-kind JE does not skip the OR"));
        }
        at(init_cond, 5, &[0x81, 0x8e])?;
        if word(init_cond, 7)? != ctx_offset {
            return Err(Error("gathering conditional OR offset differs from clear"));
        }
        let conditional_initial_mask = word(init_cond, 11)?;
        if (conditional_initial_mask & ctx_mask) == 0 {
            return Err(Error(
                "gathering conditional mask does not set the context bit",
            ));
        }

        at(candidate, 0, &[0x55, 0x8b, 0xec, 0x8b, 0x81])?;
        if word(candidate, 5)? != 0x164 {
            return Err(Error("candidate flag offset changed"));
        }
        at(candidate, 9, &[0xa9])?;
        let structure_mask = word(candidate, 10)?;
        if structure_mask == 0 || structure_mask.count_ones() != 1 {
            return Err(Error("candidate structure mask is not a single bit"));
        }
        at(candidate, 14, &[0x74])?;
        at(candidate, 16, &[0x83, 0x7d, 0x08, 0x00])?;
        at(candidate, 20, &[0x74])?;
        at(candidate, 22, &[0x25])?;
        let excluded_state_mask = word(candidate, 23)?;
        at(candidate, 27, &[0x3d])?;
        let excluded_state_value = word(candidate, 28)?;
        at(candidate, 32, &[0x74])?;
        at(candidate, 20, &[0x74, 0x0c])?;
        at(candidate, 34, &[0xb8, 1, 0, 0, 0, 0x5d, 0xc2, 4, 0])?;
        at(candidate, 43, &[0x33, 0xc0, 0x5d, 0xc2, 4, 0])?;
        // Both "reject" JE branches must converge on the shared false return
        // after the true-return block: `16 + je1.disp == 34 + je2.disp`.
        let je1 = *candidate.get(15).ok_or(Error("missing structure JE"))? as i8;
        let je2 = *candidate.get(33).ok_or(Error("missing excluded JE"))? as i8;
        if je1 < 0 || je2 < 0 || 16 + je1 as usize != 43 || 34 + je2 as usize != 43 {
            return Err(Error("candidate reject branches do not converge"));
        }
        if excluded_state_mask == 0
            || excluded_state_value == 0
            || (excluded_state_mask & excluded_state_value) != excluded_state_value
        {
            return Err(Error("candidate excluded-state mask/value mismatch"));
        }

        at(immediate, 0, &[0xf6, 0x86])?;
        if word(immediate, 2)? != 0x454 {
            return Err(Error("immediate-candidate offset changed"));
        }
        let imm_byte = *immediate
            .get(6)
            .ok_or(Error("missing immediate mask operand"))?;
        if imm_byte & 0x80 != 0 {
            return Err(Error("immediate-candidate mask is a negative imm8"));
        }
        let immediate_candidate_mask = u32::from(imm_byte);
        if immediate_candidate_mask == 0 {
            return Err(Error("immediate-candidate mask is empty"));
        }

        Ok(Self {
            image_sha256,
            context_gathering_mask: ctx_mask,
            structure_mask,
            excluded_state_mask,
            excluded_state_value,
            immediate_candidate_mask,
            initial_flags,
            conditional_initial_mask,
            conditional_excluded_kind,
            harvest_current_masks,
            harvest_previous_masks,
            harvest_town_slots,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let source = synthetic_windows();
        Self::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .expect("synthetic gathering-policy instructions")
    }
}

fn gold_windows(w: &[&[u8]]) -> Result<u32, Error> {
    let [registration, handler, bound, signature, name] = w else {
        return Err(Error("incomplete gold-harvest windows"));
    };
    for (offset, target) in [(0, WINDOWS[15].0), (5, WINDOWS[16].0), (10, WINDOWS[13].0)] {
        at(registration, offset, &[0x68])?;
        if word(registration, offset + 1)? != target {
            return Err(Error("HarvestGold registration changed"));
        }
    }
    relative(registration, WINDOWS[12].0, 15, 0xe8, 0x8ccc80)?;
    at(signature, 0, b"(II)V\0")?;
    at(name, 0, b"HarvestGold\0")?;
    at(
        handler,
        0,
        &[
            0x55, 0x8b, 0xec, 0xa1, 0x10, 0x4a, 0x1b, 0x01, 0x85, 0xc0, 0x75, 0x0c, 0x6a, 0x57,
        ],
    )?;
    relative(handler, WINDOWS[13].0, 14, 0xe8, 0x43eca0)?;
    at(
        handler,
        19,
        &[
            0x83, 0xc4, 0x04, 0x5d, 0xc3, 0x83, 0x7d, 0x0c, 0, 0x7e, 0x34, 0x8d, 0x48, 0x2c, 0x8b,
            0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x13, 0x6a, 1, 0xff, 0x75, 0x0c, 0x33,
            0xc0, 0xff, 0x75, 0x08, 0x8b, 0xc8,
        ],
    )?;
    relative(handler, WINDOWS[13].0, 55, 0xe8, 0xabf110)?;
    at(handler, 60, &[0x5d, 0xc3])?;
    relative(handler, WINDOWS[13].0, 62, 0xe8, 0x7fc7e0)?;
    at(
        handler,
        67,
        &[0x6a, 1, 0xff, 0x75, 0x0c, 0x8b, 0xc8, 0xff, 0x75, 0x08],
    )?;
    relative(handler, WINDOWS[13].0, 77, 0xe8, 0xabf110)?;
    at(handler, 82, &[0x5d, 0xc3])?;
    at(bound, 0, &[0x83, 0xfe])?;
    at(bound, 3, &[0x0f, 0x87])?;
    if bound[2] & 0x80 != 0
        || WINDOWS[14].0.wrapping_add(9).wrapping_add(word(bound, 5)?) != 0xabf1e1
    {
        return Err(Error("gold-harvest town bound changed"));
    }
    Ok(u32::from(bound[2]) + 1)
}

// Non-stock synthetic operands prove decode reads operands, not constants.
#[cfg(any(test, feature = "test-fixtures"))]
const CTX_OFF: u32 = 0x330;
#[cfg(any(test, feature = "test-fixtures"))]
const CTX_MASK: u8 = 0x40;
#[cfg(any(test, feature = "test-fixtures"))]
const INIT_FLAGS: u32 = 0;
#[cfg(any(test, feature = "test-fixtures"))]
const KIND: u8 = 0x25;
#[cfg(any(test, feature = "test-fixtures"))]
const COND_MASK: u32 = 0x4040;
#[cfg(any(test, feature = "test-fixtures"))]
const STRUCT_MASK: u32 = 0x4000;
#[cfg(any(test, feature = "test-fixtures"))]
const EXCL_MASK: u32 = 0x8000_0060;
#[cfg(any(test, feature = "test-fixtures"))]
const EXCL_VAL: u32 = 0x40;
#[cfg(any(test, feature = "test-fixtures"))]
const IMM_MASK: u8 = 0x20;

#[cfg(any(test, feature = "test-fixtures"))]
fn synthetic_windows() -> Vec<Vec<u8>> {
    fn put(w: &mut [Vec<u8>], i: usize, at: usize, data: &[u8]) {
        w[i][at..at + data.len()].copy_from_slice(data);
    }
    fn dword(w: &mut [Vec<u8>], i: usize, at: usize, data: u32) {
        put(w, i, at, &data.to_le_bytes());
    }
    let mut w = WINDOWS
        .iter()
        .map(|(_, len, _)| vec![0; *len])
        .collect::<Vec<_>>();

    for (offset, target) in [(0, WINDOWS[15].0), (5, WINDOWS[16].0), (10, WINDOWS[13].0)] {
        put(&mut w, 12, offset, &[0x68]);
        dword(&mut w, 12, offset + 1, target);
    }
    relative(&mut w, 12, 15, 0xe8, 0x8ccc80);
    put(&mut w, 15, 0, b"(II)V\0");
    put(&mut w, 16, 0, b"HarvestGold\0");
    put(
        &mut w,
        13,
        0,
        &[
            0x55, 0x8b, 0xec, 0xa1, 0x10, 0x4a, 0x1b, 0x01, 0x85, 0xc0, 0x75, 0x0c, 0x6a, 0x57,
        ],
    );
    relative(&mut w, 13, 14, 0xe8, 0x43eca0);
    put(
        &mut w,
        13,
        19,
        &[
            0x83, 0xc4, 0x04, 0x5d, 0xc3, 0x83, 0x7d, 0x0c, 0, 0x7e, 0x34, 0x8d, 0x48, 0x2c, 0x8b,
            0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x13, 0x6a, 1, 0xff, 0x75, 0x0c, 0x33,
            0xc0, 0xff, 0x75, 0x08, 0x8b, 0xc8,
        ],
    );
    relative(&mut w, 13, 55, 0xe8, 0xabf110);
    put(&mut w, 13, 60, &[0x5d, 0xc3]);
    relative(&mut w, 13, 62, 0xe8, 0x7fc7e0);
    put(
        &mut w,
        13,
        67,
        &[0x6a, 1, 0xff, 0x75, 0x0c, 0x8b, 0xc8, 0xff, 0x75, 0x08],
    );
    relative(&mut w, 13, 77, 0xe8, 0xabf110);
    put(&mut w, 13, 82, &[0x5d, 0xc3]);
    put(&mut w, 14, 0, &[0x83, 0xfe, 5, 0x0f, 0x87]);
    dword(&mut w, 14, 5, 0xabf1e1u32.wrapping_sub(WINDOWS[14].0 + 9));

    put(&mut w, 0, 0, &[0x83, 0xa0]);
    dword(&mut w, 0, 2, CTX_OFF);
    put(&mut w, 0, 6, &[!CTX_MASK]);

    put(&mut w, 1, 0, &[0xf6, 0x80]);
    dword(&mut w, 1, 2, CTX_OFF);
    put(&mut w, 1, 6, &[CTX_MASK]);

    put(&mut w, 2, 0, &[0xc7, 0x86]);
    dword(&mut w, 2, 2, CTX_OFF);
    dword(&mut w, 2, 6, INIT_FLAGS);

    put(&mut w, 3, 0, &[0x83, 0xf8, KIND]);
    put(&mut w, 3, 3, &[0x74, 0x0a]);
    put(&mut w, 3, 5, &[0x81, 0x8e]);
    dword(&mut w, 3, 7, CTX_OFF);
    dword(&mut w, 3, 11, COND_MASK);

    put(&mut w, 4, 0, &[0x55, 0x8b, 0xec, 0x8b, 0x81]);
    dword(&mut w, 4, 5, 0x164);
    put(&mut w, 4, 9, &[0xa9]);
    dword(&mut w, 4, 10, STRUCT_MASK);
    put(&mut w, 4, 14, &[0x74, 0x1b]);
    put(&mut w, 4, 16, &[0x83, 0x7d, 0x08, 0x00]);
    put(&mut w, 4, 20, &[0x74, 0x0c]);
    put(&mut w, 4, 22, &[0x25]);
    dword(&mut w, 4, 23, EXCL_MASK);
    put(&mut w, 4, 27, &[0x3d]);
    dword(&mut w, 4, 28, EXCL_VAL);
    put(&mut w, 4, 32, &[0x74, 0x09]);
    put(&mut w, 4, 34, &[0xb8, 0x01, 0, 0, 0]);
    put(&mut w, 4, 39, &[0x5d, 0xc2, 0x04, 0x00]);
    put(&mut w, 4, 43, &[0x33, 0xc0, 0x5d, 0xc2, 0x04, 0x00]);

    put(&mut w, 5, 0, &[0xf6, 0x86]);
    dword(&mut w, 5, 2, 0x454);
    put(&mut w, 5, 6, &[IMM_MASK]);

    fn relative(w: &mut [Vec<u8>], i: usize, offset: usize, opcode: u8, target: u32) {
        put(w, i, offset, &[opcode]);
        dword(
            w,
            i,
            offset + 1,
            target.wrapping_sub(WINDOWS[i].0 + offset as u32 + 5),
        );
    }
    for (offset, target) in [(0, WINDOWS[8].0), (5, WINDOWS[9].0), (10, WINDOWS[7].0)] {
        put(&mut w, 6, offset, &[0x68]);
        dword(&mut w, 6, offset + 1, target);
    }
    relative(&mut w, 6, 15, 0xe8, 0x8ccc80);
    put(&mut w, 8, 0, b"()V\0");
    put(&mut w, 9, 0, b"ClearHarvestAI\0");
    put(
        &mut w,
        7,
        0,
        &[
            0xa1, 0x10, 0x4a, 0x1b, 0x01, 0x85, 0xc0, 0x75, 0x0b, 0x6a, 0x57,
        ],
    );
    relative(&mut w, 7, 11, 0xe8, 0x43eca0);
    put(
        &mut w,
        7,
        16,
        &[
            0x83, 0xc4, 0x04, 0xc3, 0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8,
            0xff, 0x75, 0x09, 0x33, 0xc0, 0x8b, 0xc8,
        ],
    );
    relative(&mut w, 7, 37, 0xe9, WINDOWS[10].0);
    relative(&mut w, 7, 42, 0xe8, 0x7fc7e0);
    put(&mut w, 7, 47, &[0x8b, 0xc8]);
    relative(&mut w, 7, 49, 0xe9, WINDOWS[10].0);
    put(
        &mut w,
        11,
        0,
        &[
            0x8b, 0x81, 0x14, 0x05, 0, 0, 0x81, 0xc1, 0x10, 0x05, 0, 0, 0x23, 0x01, 0x83, 0xf8,
            0xff, 0x75, 0x03, 0x33, 0xc0, 0xc3,
        ],
    );
    relative(&mut w, 11, 22, 0xe9, 0x7fc7e0);
    put(
        &mut w,
        10,
        0,
        &[
            0x8b, 0x41, 0x2c, 0x83, 0xc1, 0x28, 0x56, 0x57, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75,
            0x2d, 0x33, 0xf6, 0x33, 0xc9, 0x33, 0xff, 0x85, 0xc9, 0x0f, 0x84, 0x90, 0, 0, 0, 0x0f,
            0x1f, 0,
        ],
    );
    relative(&mut w, 10, 32, 0xe8, WINDOWS[11].0);
    put(&mut w, 10, 37, &[0x85, 0xc0, 0x74, 0x44]);
    relative(&mut w, 10, 60, 0xe8, 0x7fc7e0);
    put(
        &mut w,
        10,
        65,
        &[0x8b, 0xf0, 0x85, 0xf6, 0x74, 0xca, 0x8b, 0xce],
    );
    relative(&mut w, 10, 73, 0xe8, 0x7f2610);
    put(&mut w, 10, 78, &[0x8b, 0xc8, 0xeb, 0xc1]);
    for (read, test, branch, clear, set, store, end, remove, current, previous, skip, jump) in [
        (41, 44, 47, 49, 52, 55, 58, 82, 0x10u8, 0x01u8, 0x21, 0x1a),
        (
            86, 89, 92, 94, 97, 100, 103, 105, 0x20u8, 0x02u8, 0x0b, 0x04,
        ),
    ] {
        put(&mut w, 10, read, &[0x8b, 0x48, 0x38]);
        put(&mut w, 10, test, &[0xf6, 0xc1, current]);
        put(&mut w, 10, branch, &[0x74, skip]);
        put(&mut w, 10, clear, &[0x83, 0xe1, !current]);
        put(&mut w, 10, set, &[0x83, 0xc9, previous]);
        put(&mut w, 10, store, &[0x89, 0x48, 0x38]);
        put(&mut w, 10, end, &[0xeb, jump]);
        put(&mut w, 10, remove, &[0x83, 0x60, 0x38, !previous]);
    }
    put(
        &mut w,
        10,
        109,
        &[
            0x85, 0xf6, 0x74, 0x27, 0x8b, 0x46, 0x28, 0x8d, 0x4e, 0x24, 0x23, 0x01, 0x83, 0xf8,
            0xff, 0x75, 0x04, 0x33, 0xf6, 0xeb, 0x07,
        ],
    );
    relative(&mut w, 10, 130, 0xe8, 0x7fc7e0);
    put(
        &mut w,
        10,
        135,
        &[0x8b, 0xf0, 0x85, 0xf6, 0x74, 0x0d, 0x8b, 0xce],
    );
    relative(&mut w, 10, 143, 0xe8, 0x7f2610);
    put(
        &mut w,
        10,
        148,
        &[
            0x8b, 0xc8, 0xeb, 0x04, 0x33, 0xf6, 0x8b, 0xcf, 0x33, 0xc0, 0x85, 0xf6, 0x0f, 0x45,
            0xc7, 0x8b, 0xf8, 0x85, 0xc9, 0x0f, 0x85, 0x73, 0xff, 0xff, 0xff, 0x5f, 0x5e, 0xc3,
        ],
    );

    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gold_town_bound_and_owned_handler_are_source_bound() {
        let mut source = synthetic_windows();
        assert_eq!(GatheringPolicy::fixture().harvest_town_slots, 6);
        source[14][2] = 12;
        let decoded = GatheringPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .unwrap();
        assert_eq!(decoded.harvest_town_slots, 13);
        for (window, offset) in [
            (12, 1),
            (12, 16),
            (13, 10),
            (13, 26),
            (13, 29),
            (13, 40),
            (13, 44),
            (13, 56),
            (13, 61),
            (13, 78),
            (13, 83),
            (14, 3),
            (14, 5),
            (15, 5),
            (16, 11),
        ] {
            let mut altered = source.clone();
            altered[window][offset] ^= 1;
            assert!(
                GatheringPolicy::decode(
                    &altered.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0x42; 32]
                )
                .is_err(),
                "window={window} offset={offset}"
            );
        }
        source[14][2] = 0x80;
        assert!(
            GatheringPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0x42; 32]
            )
            .is_err()
        );
    }

    #[test]
    fn harvest_rollover_uses_decoded_nonstock_masks_and_preserves_other_flags() {
        let policy = GatheringPolicy::fixture();
        assert_eq!(policy.harvest_current_masks, [0x10, 0x20]);
        assert_eq!(policy.harvest_previous_masks, [0x01, 0x02]);
        for current in 0..4 {
            for previous in 0..4 {
                let input = 0x80 | ((current & 1) << 4) | ((current & 2) << 4) | previous;
                let result = policy.clear_harvest_flags(input);
                assert_eq!(result, 0x80 | current);
                assert_eq!(policy.clear_harvest_flags(result), 0x80);
            }
        }
        let source = synthetic_windows();
        for (i, offset) in [
            (6, 1),
            (6, 16),
            (7, 7),
            (7, 38),
            (8, 3),
            (9, 14),
            (10, 46),
            (10, 48),
            (10, 51),
            (10, 54),
            (10, 85),
            (10, 91),
            (10, 96),
            (10, 99),
            (10, 108),
            (10, 131),
            (11, 23),
        ] {
            let mut broken = source.clone();
            broken[i][offset] ^= 1;
            assert!(
                GatheringPolicy::decode(
                    &broken.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32]
                )
                .is_err(),
                "window {i} byte {offset}"
            );
        }
        for (i, bytes) in source.iter().enumerate() {
            for length in 0..bytes.len() {
                let mut short = source.iter().map(Vec::as_slice).collect::<Vec<_>>();
                short[i] = &bytes[..length];
                assert!(GatheringPolicy::decode(&short, [0; 32]).is_err());
            }
        }
    }

    #[test]
    fn window_budget_is_exact() {
        let total = WINDOWS.iter().map(|(_, len, _)| len).sum();
        let budget = RuleInputBudget {
            image_bytes: 1,
            profile_bytes: total,
            sections: 1,
        };
        assert!(budgeted_windows(budget).is_ok());
        assert!(
            budgeted_windows(RuleInputBudget {
                profile_bytes: total - 1,
                ..budget
            })
            .is_err()
        );
    }

    #[test]
    fn changed_source_image_refuses_before_parsing() {
        let budget = RuleInputBudget {
            image_bytes: 1024,
            profile_bytes: 1024,
            sections: 1,
        };
        assert!(GatheringPolicy::read(b"MZunsupported", budget).is_err());
    }

    #[test]
    fn synthetic_instruction_operands_drive_the_policy() {
        let policy = GatheringPolicy::fixture();
        assert_eq!(policy.context_gathering_mask, u32::from(CTX_MASK));
        assert_eq!(policy.structure_mask, STRUCT_MASK);
        assert_eq!(policy.excluded_state_mask, EXCL_MASK);
        assert_eq!(policy.excluded_state_value, EXCL_VAL);
        assert_eq!(policy.immediate_candidate_mask, u32::from(IMM_MASK));
        assert_eq!(policy.initial_flags, INIT_FLAGS);
        assert_eq!(policy.conditional_initial_mask, COND_MASK);
        assert_eq!(policy.conditional_excluded_kind, u32::from(KIND));
        assert!(policy.gathering_default(u32::from(KIND) - 1));
        assert!(!policy.gathering_default(u32::from(KIND)));
    }

    #[test]
    fn synthetic_nonstock_operands_discriminate() {
        let mut source = synthetic_windows();
        source[3][11] ^= 0x20;
        let policy = GatheringPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("changed conditional mask operand");
        assert_eq!(policy.conditional_initial_mask, COND_MASK ^ 0x20);

        source = synthetic_windows();
        source[3][2] = 0x11;
        let policy = GatheringPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("changed excluded kind operand");
        assert_eq!(policy.conditional_excluded_kind, 0x11);
        assert!(!policy.gathering_default(0x11));

        source = synthetic_windows();
        source[4][28] = (EXCL_VAL as u8) | 0x20;
        let policy = GatheringPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("changed excluded-state value operand");
        assert_eq!(policy.excluded_state_value, EXCL_VAL | 0x20);

        source = synthetic_windows();
        source[5][6] = 0x40;
        let policy = GatheringPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("changed immediate mask operand");
        assert_eq!(policy.immediate_candidate_mask, 0x40);
    }

    #[test]
    fn malformed_or_mismatched_offsets_refuse() {
        let cases: &[fn(&mut Vec<Vec<u8>>)] = &[
            |w| w[0][0] ^= 1,
            |w| w[1][0] ^= 1,
            |w| w[2][0] ^= 1,
            |w| w[3][0] ^= 1,
            |w| w[4][0] ^= 1,
            |w| w[5][0] ^= 1,
            |w| w[0][2] ^= 0x10, // context offset mismatch
            |w| w[1][2] ^= 0x10, // reader offset mismatch
            |w| w[3][7] ^= 0x10, // conditional OR offset mismatch
            |w| w[4][5] ^= 0x10, // candidate offset mismatch
            |w| w[5][2] ^= 0x10, // immediate offset mismatch
            |w| w[0][6] = 0x40,  // clear imm sign bit clear (not a wide clear)
            |w| w[1][6] = 0x80,  // sign-extended reader mask
            |w| w[3][2] = 0x80,  // sign-extended excluded kind
            |w| w[5][6] = 0x80,  // sign-extended immediate mask
            |w| w[3][4] = 0x09,  // excluded-kind JE does not skip the OR
            |w| w[3][11] = 0,    // conditional mask missing context bit
            |w| w[2][6] = 1,     // nonzero initial flags
            |w| w[4][9] ^= 1,    // structure test opcode changed
            |w| w[4][22] ^= 1,   // excluded AND opcode changed
            |w| w[4][27] ^= 1,   // excluded CMP opcode changed
            |w| w[4][15] = 0xff, // structure JE backward (negative disp)
            |w| w[4][33] += 1,   // reject branches stop converging
            |w| {
                w[4][15] -= 1;
                w[4][33] -= 1;
            }, // convergent branches must still reach the false return
            |w| w[4][21] -= 1,   // argument-zero branch must reach the true return
            |w| w[4][35] = 0,    // true return changed to false
            |w| w[4][44] ^= 1,   // false return no longer clears eax
            |w| {
                w[4][10] = 0x00;
                w[4][11] = 0x30;
                w[4][12] = 0x00;
                w[4][13] = 0x00;
            }, // multi-bit structure mask
            |w| w[4][23] = 0,    // empty excluded mask
            |w| w[5][6] = 0,     // empty immediate mask
        ];
        for (index, mutate) in cases.iter().enumerate() {
            let mut source = synthetic_windows();
            mutate(&mut source);
            assert!(
                GatheringPolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32],
                )
                .is_err(),
                "case {index} should refuse"
            );
        }
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
    fn reviewed_original_image_decodes() {
        let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE required"))
            .expect("read reviewed client");
        let budget = RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: WINDOWS.iter().map(|(_, len, _)| len).sum(),
            sections: 96,
        };
        let policy =
            GatheringPolicy::read(&image, budget).expect("selected gathering-policy operands");
        assert_eq!(policy.image_sha256, REVIEWED_IMAGE);
        assert_eq!(policy.context_gathering_mask, 0x1);
        assert_eq!(policy.structure_mask, 0x10000);
        assert_eq!(policy.excluded_state_mask, 0x0800_0080);
        assert_eq!(policy.excluded_state_value, 0x80);
        assert_eq!(policy.immediate_candidate_mask, 0x10);
        assert_eq!(policy.harvest_current_masks, [0x01, 0x02]);
        assert_eq!(policy.harvest_previous_masks, [0x04, 0x08]);
        assert_eq!(policy.harvest_town_slots, 4);
        assert_eq!(policy.initial_flags, 0);
        assert_eq!(policy.conditional_initial_mask, 0x0008_0181);
        assert_eq!(policy.conditional_excluded_kind, 0x18);
        assert!(policy.gathering_default(0x17));
        assert!(!policy.gathering_default(0x18));
    }
}
