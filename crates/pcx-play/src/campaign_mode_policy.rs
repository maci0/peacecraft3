//! Selected campaign context configuration and per-unit mode propagation.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct CampaignModePolicy {
    pub image_sha256: Sha256Digest,
    pub context_mask: u32,
    pub unit_mask: u32,
    pub campaign_mode: bool,
    pub replacement_count_field: u32,
}

#[derive(Debug, thiserror::Error)]
#[error("campaign mode policy: {0}")]
pub struct Error(&'static str);

const WINDOWS: [(u32, usize, bool); 12] = [
    (0xab6887, 20, true),
    (0xab6e80, 60, true),
    (0xe70fac, 4, false),
    (0xf160a0, 14, false),
    (0xac2620, 35, true),
    (0xac2660, 45, true),
    (0xabb3ce, 25, true),
    (0xac39d9, 12, true),
    (0xab69bc, 20, true),
    (0xab7450, 67, true),
    (0xe6f4a0, 5, false),
    (0xf161a0, 20, false),
];

fn at(bytes: &[u8], offset: usize, expected: &[u8]) -> Result<(), Error> {
    (bytes.get(offset..offset + expected.len()) == Some(expected))
        .then_some(())
        .ok_or(Error("instruction relationship changed"))
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(Error("short operand"))?
            .try_into()
            .map_err(|_| Error("short operand"))?,
    ))
}

fn call(bytes: &[u8], va: u32, offset: usize, target: u32) -> Result<(), Error> {
    at(bytes, offset, &[0xe8])?;
    (va.wrapping_add(offset as u32 + 5)
        .wrapping_add(word(bytes, offset + 1)?)
        == target)
        .then_some(())
        .ok_or(Error("call target changed"))
}

impl CampaignModePolicy {
    pub fn read(image: &[u8], budget: RuleInputBudget) -> Result<Self, Error> {
        if image.len() > budget.image_bytes {
            return Err(Error("image budget exceeded"));
        }
        if WINDOWS.iter().map(|(_, length, _)| length).sum::<usize>() > budget.profile_bytes {
            return Err(Error("window budget exceeded"));
        }
        if sha256(image) != REVIEWED_IMAGE {
            return Err(Error("unsupported client image"));
        }
        let pe = Pe::parse(image, budget).map_err(|_| Error("invalid selected PE"))?;
        let windows = WINDOWS
            .iter()
            .map(|&(va, length, code)| {
                pe.read_from(va, code)
                    .map_err(|_| Error("missing window"))?
                    .get(..length)
                    .ok_or(Error("short window"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::decode(&windows, REVIEWED_IMAGE)
    }

    fn decode(windows: &[&[u8]], image_sha256: Sha256Digest) -> Result<Self, Error> {
        let [
            registration,
            handler,
            signature,
            name,
            mode,
            units,
            admission,
            consumer,
            replacement_registration,
            replacement_handler,
            replacement_signature,
            replacement_name,
        ] = windows
        else {
            return Err(Error("incomplete windows"));
        };
        for (bytes, (_, length, _)) in windows.iter().zip(WINDOWS) {
            if bytes.len() < length {
                return Err(Error("truncated window"));
            }
        }
        for (offset, target) in [(0, WINDOWS[2].0), (5, WINDOWS[3].0), (10, WINDOWS[1].0)] {
            at(registration, offset, &[0x68])?;
            if word(registration, offset + 1)? != target {
                return Err(Error("registration pointer changed"));
            }
        }
        call(registration, WINDOWS[0].0, 15, 0x8ccc80)?;
        at(signature, 0, b"()V\0")?;
        at(name, 0, b"SetCampaignAI\0")?;
        at(handler, 0, &[0xa1])?;
        if word(handler, 1)? != 0x11b4a10 {
            return Err(Error("owned AI context changed"));
        }
        at(handler, 5, &[0x85, 0xc0, 0x75, 0x0b, 0x6a, 0x57])?;
        call(handler, WINDOWS[1].0, 11, 0x43eca0)?;
        at(handler, 16, &[0x83, 0xc4, 0x04, 0xc3])?;
        at(
            handler,
            20,
            &[
                0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x0c, 0x33,
                0xc0, 0x6a,
            ],
        )?;
        at(handler, 37, &[0x8b, 0xc8])?;
        call(handler, WINDOWS[1].0, 39, WINDOWS[4].0)?;
        at(handler, 44, &[0xc3])?;
        call(handler, WINDOWS[1].0, 45, 0x7fc7e0)?;
        at(handler, 50, &[0x6a])?;
        at(handler, 52, &[0x8b, 0xc8])?;
        call(handler, WINDOWS[1].0, 54, WINDOWS[4].0)?;
        at(handler, 59, &[0xc3])?;
        if handler[36] > 1 || handler[36] != handler[51] {
            return Err(Error("mode arguments differ or are not boolean"));
        }
        at(
            mode,
            0,
            &[
                0x55, 0x8b, 0xec, 0x53, 0x8b, 0x5d, 0x08, 0x56, 0x57, 0x85, 0xdb, 0x75, 0x0c, 0x81,
                0x89,
            ],
        )?;
        let field = word(mode, 15)?;
        let context_mask = word(mode, 19)?;
        at(mode, 23, &[0xeb, 0x0a, 0x81, 0xa1])?;
        if !context_mask.is_power_of_two()
            || word(mode, 27)? != field
            || word(mode, 31)? != !context_mask
        {
            return Err(Error("context set/clear relationship changed"));
        }
        call(units, WINDOWS[5].0, 0, 0x6933c0)?;
        at(
            units,
            5,
            &[0x85, 0xc0, 0x74, 0x24, 0x85, 0xdb, 0x74, 0x1c, 0x83, 0x60],
        )?;
        at(units, 17, &[0xeb, 0x1a])?;
        at(units, 41, &[0x83, 0x48])?;
        let unit_mask = u32::from(units[44]);
        if !unit_mask.is_power_of_two()
            || units[44] >= 0x80
            || units[15] != units[43]
            || (i32::from(units[16] as i8) as u32) != !unit_mask
        {
            return Err(Error("unit set/clear relationship changed"));
        }
        at(admission, 0, &[0x8b, 0x80])?;
        at(admission, 6, &[0xc1, 0xe8])?;
        at(
            admission,
            9,
            &[0xf7, 0xd0, 0xa8, 0x01, 0x74, 0x06, 0x83, 0x66],
        )?;
        at(admission, 19, &[0xeb, 0x04, 0x83, 0x4e])?;
        if word(admission, 2)? != field
            || admission[8] >= 32
            || 1u32 << admission[8] != context_mask
            || admission[17] != units[15]
            || admission[23] != units[15]
            || admission[18] != units[16]
            || admission[24] != units[44]
        {
            return Err(Error("future unit admission differs from mode transition"));
        }
        at(consumer, 0, &[0xf7, 0x87])?;
        at(consumer, 10, &[0x74])?;
        if word(consumer, 2)? != field
            || word(consumer, 6)? != context_mask
            || WINDOWS[7]
                .0
                .wrapping_add(12)
                .wrapping_add(consumer[11] as i8 as u32)
                != 0xac3a36
        {
            return Err(Error("mode consumer differs"));
        }
        for (offset, target) in [(0, WINDOWS[10].0), (5, WINDOWS[11].0), (10, WINDOWS[9].0)] {
            at(replacement_registration, offset, &[0x68])?;
            if word(replacement_registration, offset + 1)? != target {
                return Err(Error("replacement registration changed"));
            }
        }
        call(replacement_registration, WINDOWS[8].0, 15, 0x8ccc80)?;
        at(replacement_signature, 0, b"(I)V\0")?;
        at(replacement_name, 0, b"SetReplacementCount\0")?;
        let h = replacement_handler;
        at(h, 0, &[0x55, 0x8b, 0xec, 0xa1])?;
        if word(h, 4)? != word(handler, 1)? {
            return Err(Error("replacement AI context differs"));
        }
        at(h, 8, &[0x85, 0xc0, 0x75, 0x0d, 0xc7, 0x45, 0x08])?;
        if word(h, 15)? != 0x57 {
            return Err(Error("replacement context error differs"));
        }
        at(h, 19, &[0x5d, 0xe9])?;
        if WINDOWS[9].0.wrapping_add(25).wrapping_add(word(h, 21)?) != 0x43eca0 {
            return Err(Error("replacement error target differs"));
        }
        at(
            h,
            25,
            &[
                0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x0d, 0x8b,
                0x4d, 0x08, 0x33, 0xc0, 0x89, 0x88,
            ],
        )?;
        let replacement_count_field = word(h, 45)?;
        at(h, 49, &[0x5d, 0xc3])?;
        call(h, WINDOWS[9].0, 51, 0x7fc7e0)?;
        at(h, 56, &[0x8b, 0x4d, 0x08, 0x89, 0x88])?;
        at(h, 65, &[0x5d, 0xc3])?;
        if word(h, 61)? != replacement_count_field
            || replacement_count_field == 0
            || !replacement_count_field.is_multiple_of(4)
            || replacement_count_field == field
        {
            return Err(Error("replacement integer storage differs"));
        }
        Ok(Self {
            image_sha256,
            context_mask,
            unit_mask,
            campaign_mode: handler[36] != 0,
            replacement_count_field,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let source = synthetic();
        Self::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .expect("synthetic campaign mode")
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
fn synthetic() -> Vec<Vec<u8>> {
    let mut w: Vec<_> = WINDOWS
        .iter()
        .map(|(_, length, _)| vec![0; *length])
        .collect();
    fn put(w: &mut [Vec<u8>], i: usize, offset: usize, bytes: &[u8]) {
        w[i][offset..offset + bytes.len()].copy_from_slice(bytes);
    }
    fn word(w: &mut [Vec<u8>], i: usize, offset: usize, value: u32) {
        put(w, i, offset, &value.to_le_bytes());
    }
    fn call(w: &mut [Vec<u8>], i: usize, offset: usize, target: u32) {
        put(w, i, offset, &[0xe8]);
        word(
            w,
            i,
            offset + 1,
            target.wrapping_sub(WINDOWS[i].0 + offset as u32 + 5),
        );
    }
    for (offset, target) in [(0, WINDOWS[2].0), (5, WINDOWS[3].0), (10, WINDOWS[1].0)] {
        put(&mut w, 0, offset, &[0x68]);
        word(&mut w, 0, offset + 1, target);
    }
    call(&mut w, 0, 15, 0x8ccc80);
    put(&mut w, 2, 0, b"()V\0");
    put(&mut w, 3, 0, b"SetCampaignAI\0");
    put(&mut w, 1, 0, &[0xa1]);
    word(&mut w, 1, 1, 0x11b4a10);
    put(&mut w, 1, 5, &[0x85, 0xc0, 0x75, 0x0b, 0x6a, 0x57]);
    call(&mut w, 1, 11, 0x43eca0);
    put(
        &mut w,
        1,
        16,
        &[
            0x83, 0xc4, 0x04, 0xc3, 0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8,
            0xff, 0x75, 0x0c, 0x33, 0xc0, 0x6a, 1, 0x8b, 0xc8,
        ],
    );
    call(&mut w, 1, 39, WINDOWS[4].0);
    put(&mut w, 1, 44, &[0xc3]);
    call(&mut w, 1, 45, 0x7fc7e0);
    put(&mut w, 1, 50, &[0x6a, 1, 0x8b, 0xc8]);
    call(&mut w, 1, 54, WINDOWS[4].0);
    put(&mut w, 1, 59, &[0xc3]);
    put(
        &mut w,
        4,
        0,
        &[
            0x55, 0x8b, 0xec, 0x53, 0x8b, 0x5d, 0x08, 0x56, 0x57, 0x85, 0xdb, 0x75, 0x0c, 0x81,
            0x89,
        ],
    );
    word(&mut w, 4, 15, 0x350);
    word(&mut w, 4, 19, 0x8000);
    put(&mut w, 4, 23, &[0xeb, 0x0a, 0x81, 0xa1]);
    word(&mut w, 4, 27, 0x350);
    word(&mut w, 4, 31, !0x8000);
    call(&mut w, 5, 0, 0x6933c0);
    put(
        &mut w,
        5,
        5,
        &[
            0x85, 0xc0, 0x74, 0x24, 0x85, 0xdb, 0x74, 0x1c, 0x83, 0x60, 0x30, !0x20, 0xeb, 0x1a,
        ],
    );
    put(&mut w, 5, 41, &[0x83, 0x48, 0x30, 0x20]);
    put(&mut w, 6, 0, &[0x8b, 0x80]);
    word(&mut w, 6, 2, 0x350);
    put(
        &mut w,
        6,
        6,
        &[
            0xc1, 0xe8, 15, 0xf7, 0xd0, 0xa8, 1, 0x74, 6, 0x83, 0x66, 0x30, !0x20, 0xeb, 4, 0x83,
            0x4e, 0x30, 0x20,
        ],
    );
    put(&mut w, 7, 0, &[0xf7, 0x87]);
    word(&mut w, 7, 2, 0x350);
    word(&mut w, 7, 6, 0x8000);
    put(&mut w, 7, 10, &[0x74, 0x51]);
    for (offset, target) in [(0, WINDOWS[10].0), (5, WINDOWS[11].0), (10, WINDOWS[9].0)] {
        put(&mut w, 8, offset, &[0x68]);
        word(&mut w, 8, offset + 1, target);
    }
    call(&mut w, 8, 15, 0x8ccc80);
    put(&mut w, 10, 0, b"(I)V\0");
    put(&mut w, 11, 0, b"SetReplacementCount\0");
    put(&mut w, 9, 0, &[0x55, 0x8b, 0xec, 0xa1]);
    word(&mut w, 9, 4, 0x11b4a10);
    put(&mut w, 9, 8, &[0x85, 0xc0, 0x75, 0x0d, 0xc7, 0x45, 0x08]);
    word(&mut w, 9, 15, 0x57);
    put(&mut w, 9, 19, &[0x5d, 0xe9]);
    word(&mut w, 9, 21, 0x43eca0u32.wrapping_sub(WINDOWS[9].0 + 25));
    put(
        &mut w,
        9,
        25,
        &[
            0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x0d, 0x8b,
            0x4d, 0x08, 0x33, 0xc0, 0x89, 0x88,
        ],
    );
    word(&mut w, 9, 45, 0x380);
    put(&mut w, 9, 49, &[0x5d, 0xc3]);
    call(&mut w, 9, 51, 0x7fc7e0);
    put(&mut w, 9, 56, &[0x8b, 0x4d, 0x08, 0x89, 0x88]);
    word(&mut w, 9, 61, 0x380);
    put(&mut w, 9, 65, &[0x5d, 0xc3]);
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(w: &[Vec<u8>]) -> Result<CampaignModePolicy, Error> {
        CampaignModePolicy::decode(&w.iter().map(Vec::as_slice).collect::<Vec<_>>(), [0x42; 32])
    }

    #[test]
    fn mode_and_nonstock_masks_are_source_operands() {
        let mut w = synthetic();
        let p = decode(&w).unwrap();
        assert_eq!(
            (p.context_mask, p.unit_mask, p.campaign_mode),
            (0x8000, 0x20, true)
        );
        assert_eq!(p.replacement_count_field, 0x380);
        for offset in [45, 61] {
            w[9][offset..offset + 4].copy_from_slice(&0x398u32.to_le_bytes());
        }
        assert_eq!(decode(&w).unwrap().replacement_count_field, 0x398);
        w[1][36] = 0;
        w[1][51] = 0;
        assert!(!decode(&w).unwrap().campaign_mode);
    }

    #[test]
    fn relationships_strings_and_all_truncations_refuse() {
        for (i, offset) in [
            (0, 11),
            (1, 8),
            (1, 32),
            (1, 36),
            (1, 40),
            (2, 3),
            (3, 13),
            (4, 19),
            (4, 31),
            (5, 12),
            (5, 16),
            (5, 44),
            (6, 8),
            (6, 18),
            (7, 6),
            (7, 11),
            (8, 11),
            (9, 4),
            (9, 10),
            (9, 21),
            (9, 27),
            (9, 37),
            (9, 40),
            (9, 47),
            (9, 50),
            (9, 52),
            (9, 58),
            (9, 61),
            (9, 66),
            (10, 4),
            (11, 19),
        ] {
            let mut w = synthetic();
            w[i][offset] ^= 1;
            assert!(decode(&w).is_err(), "window {i} offset {offset}");
        }
        for (i, (_, length, _)) in WINDOWS.iter().enumerate() {
            for length in 0..*length {
                let mut w = synthetic();
                w[i].truncate(length);
                assert!(decode(&w).is_err(), "window {i} length {length}");
            }
        }
    }

    #[test]
    fn unqualified_image_and_insufficient_budget_refuse() {
        let budget = RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 0,
            sections: 96,
        };
        assert!(CampaignModePolicy::read(b"MZ", budget).is_err());
        assert!(
            CampaignModePolicy::read(
                b"MZ",
                RuleInputBudget {
                    profile_bytes: 1024,
                    ..budget
                }
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
    fn reviewed_original_image_decodes() {
        let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE")).unwrap();
        let total = WINDOWS.iter().map(|(_, length, _)| length).sum();
        let budget = RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: total,
            sections: 96,
        };
        let p = CampaignModePolicy::read(&image, budget).unwrap();
        assert_eq!(
            (p.context_mask, p.unit_mask, p.campaign_mode),
            (0x4000, 0x40, true)
        );
        assert_eq!(p.image_sha256, REVIEWED_IMAGE);
        assert_eq!(p.replacement_count_field, 0x1d4);
        assert!(
            CampaignModePolicy::read(
                &image,
                RuleInputBudget {
                    profile_bytes: total - 1,
                    ..budget
                }
            )
            .is_err()
        );
    }
}
