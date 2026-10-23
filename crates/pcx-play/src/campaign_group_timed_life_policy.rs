//! Source-bound `GroupTimedLife` flag and association-consumer identity.
//! This reader does not qualify unit lifecycle events or captain membership.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct GroupTimedLifePolicy {
    pub image_sha256: Sha256Digest,
    pub context_mask: u32,
}

#[derive(Debug, thiserror::Error)]
#[error("campaign group-timed-life policy: {0}")]
pub struct Error(&'static str);

// Reviewed image locations identify relationships; returned values are operands.
const WINDOWS: [(u32, usize, bool); 5] = [
    (0xab6516, 20, true),
    (0xab5760, 77, true),
    (0xe700b0, 5, false),
    (0xf15dd0, 15, false),
    (0xac472a, 12, true),
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

fn window_budget(budget: RuleInputBudget) -> Result<(), Error> {
    let total = WINDOWS.iter().try_fold(0usize, |total, (_, length, _)| {
        total
            .checked_add(*length)
            .ok_or(Error("window budget exceeded"))
    })?;
    if total > budget.profile_bytes {
        return Err(Error("window budget exceeded"));
    }
    Ok(())
}

impl GroupTimedLifePolicy {
    pub fn read(image: &[u8], budget: RuleInputBudget) -> Result<Self, Error> {
        if image.len() > budget.image_bytes {
            return Err(Error("image budget exceeded"));
        }
        window_budget(budget)?;
        if sha256(image) != REVIEWED_IMAGE {
            return Err(Error("unsupported client image"));
        }
        let pe = Pe::parse(image, budget).map_err(|_| Error("invalid selected PE"))?;
        let mut windows = Vec::with_capacity(WINDOWS.len());
        for (va, length, code) in WINDOWS {
            windows.push(
                pe.read_from(va, code)
                    .map_err(|_| Error("missing selected window"))?
                    .get(..length)
                    .ok_or(Error("short selected window"))?,
            );
        }
        Self::decode(&windows, REVIEWED_IMAGE)
    }

    fn decode(windows: &[&[u8]], image_sha256: Sha256Digest) -> Result<Self, Error> {
        let [registration, handler, signature, name, consumer] = windows else {
            return Err(Error("incomplete selected windows"));
        };
        for (window, (_, length, _)) in windows.iter().zip(WINDOWS) {
            if window.len() < length {
                return Err(Error("truncated selected window"));
            }
        }
        for (offset, target) in [(0, WINDOWS[2].0), (5, WINDOWS[3].0), (10, WINDOWS[1].0)] {
            at(registration, offset, &[0x68])?;
            if word(registration, offset + 1)? != target {
                return Err(Error("registration pointer changed"));
            }
        }
        at(registration, 15, &[0xe8])?;
        if WINDOWS[0]
            .0
            .wrapping_add(20)
            .wrapping_add(word(registration, 16)?)
            != 0x8ccc80
        {
            return Err(Error("registrar changed"));
        }
        at(signature, 0, b"(B)V\0")?;
        at(name, 0, b"GroupTimedLife\0")?;
        let (field, context_mask) = crate::campaign_ai_flag::decode(handler, WINDOWS[1].0)
            .map_err(|error| Error(error.0))?;
        at(consumer, 0, &[0xf7, 0x83])?;
        if word(consumer, 2)? != field || word(consumer, 6)? != context_mask {
            return Err(Error("association consumer differs from setter"));
        }
        at(consumer, 10, &[0x74])?;
        if WINDOWS[4]
            .0
            .wrapping_add(12)
            .wrapping_add(consumer[11] as i8 as u32)
            != 0xac4772
        {
            return Err(Error("association false branch changed"));
        }
        Ok(Self {
            image_sha256,
            context_mask,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let windows = synthetic_windows(0x4000, 0x330);
        Self::decode(
            &windows.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .expect("synthetic group-timed-life instructions")
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
fn synthetic_windows(mask: u32, field: u32) -> Vec<Vec<u8>> {
    fn put(w: &mut [Vec<u8>], i: usize, offset: usize, bytes: &[u8]) {
        w[i][offset..offset + bytes.len()].copy_from_slice(bytes);
    }
    fn word(w: &mut [Vec<u8>], i: usize, offset: usize, value: u32) {
        put(w, i, offset, &value.to_le_bytes());
    }
    fn branch(w: &mut [Vec<u8>], i: usize, offset: usize, opcode: u8, target: u32) {
        put(w, i, offset, &[opcode]);
        word(
            w,
            i,
            offset + 1,
            target.wrapping_sub(WINDOWS[i].0 + offset as u32 + 5),
        );
    }
    let mut w = WINDOWS
        .iter()
        .map(|(_, length, _)| vec![0; *length])
        .collect::<Vec<_>>();
    for (offset, target) in [(0, WINDOWS[2].0), (5, WINDOWS[3].0), (10, WINDOWS[1].0)] {
        put(&mut w, 0, offset, &[0x68]);
        word(&mut w, 0, offset + 1, target);
    }
    branch(&mut w, 0, 15, 0xe8, 0x8ccc80);
    put(&mut w, 1, 0, &[0x55, 0x8b, 0xec, 0xa1]);
    word(&mut w, 1, 4, 0x11b4a10);
    put(
        &mut w,
        1,
        8,
        &[
            0x85, 0xc0, 0x75, 0x0d, 0xc7, 0x45, 0x08, 0x57, 0, 0, 0, 0x5d,
        ],
    );
    branch(&mut w, 1, 20, 0xe9, 0x43eca0);
    put(
        &mut w,
        1,
        25,
        &[
            0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x04, 0x33,
            0xc0, 0xeb, 0x05,
        ],
    );
    branch(&mut w, 1, 42, 0xe8, 0x7fc7e0);
    put(
        &mut w,
        1,
        47,
        &[0x83, 0x7d, 0x08, 0, 0x74, 0x0c, 0x81, 0x88],
    );
    word(&mut w, 1, 55, field);
    word(&mut w, 1, 59, mask);
    put(&mut w, 1, 63, &[0x5d, 0xc3, 0x81, 0xa0]);
    word(&mut w, 1, 67, field);
    word(&mut w, 1, 71, !mask);
    put(&mut w, 1, 75, &[0x5d, 0xc3]);
    put(&mut w, 2, 0, b"(B)V\0");
    put(&mut w, 3, 0, b"GroupTimedLife\0");
    put(&mut w, 4, 0, &[0xf7, 0x83]);
    word(&mut w, 4, 2, field);
    word(&mut w, 4, 6, mask);
    put(&mut w, 4, 10, &[0x74, 0x3c]);
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(w: &[Vec<u8>]) -> Result<GroupTimedLifePolicy, Error> {
        GroupTimedLifePolicy::decode(&w.iter().map(Vec::as_slice).collect::<Vec<_>>(), [0x42; 32])
    }

    #[test]
    fn budget_and_source_identity_refuse_before_pe_parsing() {
        let budget = RuleInputBudget {
            image_bytes: 1024,
            profile_bytes: 129,
            sections: 8,
        };
        assert!(window_budget(budget).is_ok());
        assert!(
            window_budget(RuleInputBudget {
                profile_bytes: 128,
                ..budget
            })
            .is_err()
        );
        assert!(GroupTimedLifePolicy::read(b"MZunsupported", budget).is_err());
    }

    #[test]
    fn wide_operands_drive_the_policy_and_allow_the_high_bit() {
        assert_eq!(GroupTimedLifePolicy::fixture().context_mask, 0x4000);
        for mask in [0x800, 0x8000_0000] {
            let policy = decode(&synthetic_windows(mask, 0x440)).unwrap();
            assert_eq!(policy.context_mask, mask);
            assert_eq!(policy.image_sha256, [0x42; 32]);
        }
    }

    #[test]
    fn malformed_relationships_and_every_truncated_window_refuse() {
        let valid = synthetic_windows(0x4000, 0x330);
        for (index, window) in valid.iter().enumerate() {
            for length in 0..window.len() {
                let mut w = valid.clone();
                w[index].truncate(length);
                assert!(decode(&w).is_err(), "window {index} length {length}");
            }
        }
        for (index, offset) in [
            (0, 1),
            (0, 11),
            (0, 16),
            (1, 11),
            (1, 37),
            (1, 52),
            (1, 53),
            (1, 63),
            (1, 65),
            (1, 67),
            (1, 71),
            (1, 76),
            (2, 4),
            (3, 14),
            (4, 0),
            (4, 2),
            (4, 6),
            (4, 11),
        ] {
            let mut w = valid.clone();
            w[index][offset] ^= 1;
            assert!(decode(&w).is_err(), "window {index} offset {offset}");
        }
        for mask in [0, 0x300] {
            assert!(decode(&synthetic_windows(mask, 0x330)).is_err());
        }
        for length in 0..77 {
            assert!(crate::campaign_ai_flag::decode(&valid[1][..length], WINDOWS[1].0).is_err());
        }
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
    fn reviewed_original_image_decodes() {
        let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE required"))
            .unwrap();
        let policy = GroupTimedLifePolicy::read(
            &image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 129,
                sections: 96,
            },
        )
        .unwrap();
        assert_eq!(policy.image_sha256, REVIEWED_IMAGE);
        assert_eq!(policy.context_mask, 0x100);
    }
}
