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
}

#[derive(Debug, thiserror::Error)]
#[error("campaign gathering policy: {0}")]
pub struct Error(&'static str);

// VAs and lengths identify instruction windows in the reviewed PE. Every value
// returned below is decoded from an operand within those windows.
const WINDOWS: [(u32, usize, bool); 6] = [
    (0xac3f5b, 7, true),  // context clear (andl $-2, ctx+0x2d0)
    (0xabb4e9, 7, true),  // context reader (testb $1, ctx+0x2d0)
    (0xabf9e2, 10, true), // constructor zero-initialize
    (0xabfa1e, 16, true), // constructor conditional OR (kind gate)
    (0x696b50, 49, true), // candidate structure/excluded-state predicate
    (0xac3f9d, 7, true),  // immediate-candidate mask (testb $0x10, unit+0x454)
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

impl GatheringPolicy {
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
        let [clear, reader, init_zero, init_cond, candidate, immediate] = w else {
            return Err(Error("incomplete gathering-policy windows"));
        };

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
        at(candidate, 34, &[0xb8])?;
        // Both "reject" JE branches must converge on the shared false return
        // after the true-return block: `16 + je1.disp == 34 + je2.disp`.
        let je1 = *candidate.get(15).ok_or(Error("missing structure JE"))? as i8;
        let je2 = *candidate.get(33).ok_or(Error("missing excluded JE"))? as i8;
        if je1 < 0 || je2 < 0 || 16 + je1 as usize != 34 + je2 as usize {
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

    w
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(policy.initial_flags, 0);
        assert_eq!(policy.conditional_initial_mask, 0x0008_0181);
        assert_eq!(policy.conditional_excluded_kind, 0x18);
        assert!(policy.gathering_default(0x17));
        assert!(!policy.gathering_default(0x18));
    }
}
