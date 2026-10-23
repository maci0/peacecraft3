//! Captain selector operands from the reviewed client image. This is a source
//! reader: it decodes instruction operands; the captain group/unit/home
//! semantic boundary is unimplemented here.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct CaptainPolicy {
    pub image_sha256: Sha256Digest,
    pub first_selector_mask: u8,
    pub second_selector_mask: u8,
}

#[derive(Debug, thiserror::Error)]
#[error("campaign captain policy: {0}")]
pub struct Error(&'static str);

// VAs and lengths identify instruction windows in the reviewed PE. Every value
// returned below is decoded from an operand within those windows.
const WINDOWS: [(u32, usize, bool); 7] = [
    (0xab645c, 20, true),  // SetCaptainHome registration banner (sig/name/handler)
    (0xab6f10, 80, true),  // SetCaptainHome handler entry
    (0xac26d0, 124, true), // selector (masks + slots + resolver/home write)
    (0xab644d, 15, true),  // CreateCaptains primary registration
    (0xab4771, 17, true),  // CreateCaptains handler -> main-work join (abd160)
    (0xabd2ad, 15, true),  // create first-pair home writes
    (0xabd352, 15, true),  // create second-pair home writes
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

fn call(bytes: &[u8], va: u32, offset: usize) -> Result<u32, Error> {
    at(bytes, offset, &[0xe8])?;
    Ok(va
        .wrapping_add(offset as u32 + 5)
        .wrapping_add(word(bytes, offset + 1)?))
}

fn jmp(bytes: &[u8], va: u32, offset: usize) -> Result<u32, Error> {
    at(bytes, offset, &[0xe9])?;
    Ok(va
        .wrapping_add(offset as u32 + 5)
        .wrapping_add(word(bytes, offset + 1)?))
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

impl CaptainPolicy {
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
        let [
            home_reg,
            home_handler,
            sel,
            capt_reg,
            create_jmp,
            create1,
            create2,
        ] = w
        else {
            return Err(Error("incomplete captain-policy windows"));
        };

        // SetCaptainHome registration binds handler 0xab6f10 via the registrar.
        at(home_reg, 0, &[0x68])?;
        if word(home_reg, 1)? != 0xe6f5a8 {
            return Err(Error("SetCaptainHome signature changed"));
        }
        at(home_reg, 5, &[0x68])?;
        if word(home_reg, 6)? != 0xf15d34 {
            return Err(Error("SetCaptainHome name changed"));
        }
        at(home_reg, 10, &[0x68])?;
        if word(home_reg, 11)? != 0xab6f10 {
            return Err(Error("SetCaptainHome handler changed"));
        }
        if call(home_reg, WINDOWS[0].0, 15)? != 0x8ccc80 {
            return Err(Error("SetCaptainHome registrar changed"));
        }

        // Handler entry reads the AI-context global and dispatches both paths to
        // the selector.
        at(home_handler, 0, &[0x55, 0x8b, 0xec])?;
        at(home_handler, 3, &[0xa1])?;
        if word(home_handler, 4)? != 0x11b4a10 {
            return Err(Error("AI context global changed"));
        }
        if call(home_handler, WINDOWS[1].0, 50)? != 0xac26d0 {
            return Err(Error("SetCaptainHome first dispatch changed"));
        }
        if call(home_handler, WINDOWS[1].0, 73)? != 0xac26d0 {
            return Err(Error("SetCaptainHome second dispatch changed"));
        }

        // Selector derives the two single-bit masks and their context slots, and
        // binds the resolver + home writer for both.
        // F6 TEST r/m8, imm8 does not sign-extend, so bit 0x80 is a valid mask.
        at(sel, 4, &[0xf6, 0x45, 0x08])?;
        let first_selector_mask = *sel.get(7).ok_or(Error("missing first selector mask"))?;
        if first_selector_mask == 0 || first_selector_mask.count_ones() != 1 {
            return Err(Error("first selector mask is not a single nonzero bit"));
        }
        at(sel, 15, &[0x8d, 0x88])?;
        let first_slot = word(sel, 17)?;
        if first_slot == 0 {
            return Err(Error("first selector slot is empty"));
        }
        let first_end = first_slot
            .checked_add(8)
            .ok_or(Error("first selector reference pair overflows"))?;
        // Conditional skip: first test -> second test (offset 56).
        at(sel, 13, &[0x74])?;
        if 15 + i32::from(sel[14] as i8) != 56 {
            return Err(Error("first selector JE does not skip to the second test"));
        }
        if call(sel, WINDOWS[2].0, 31)? != 0x7fc7e0 {
            return Err(Error("first selector resolver changed"));
        }
        if call(sel, WINDOWS[2].0, 48)? != 0xad0630 {
            return Err(Error("first selector home writer changed"));
        }

        at(sel, 56, &[0xf6, 0x45, 0x08])?;
        let second_selector_mask = *sel.get(59).ok_or(Error("missing second selector mask"))?;
        if second_selector_mask == 0
            || second_selector_mask.count_ones() != 1
            || second_selector_mask == first_selector_mask
        {
            return Err(Error(
                "second selector mask is not a distinct single nonzero bit",
            ));
        }
        at(sel, 62, &[0x8d, 0x88])?;
        let second_slot = word(sel, 64)?;
        if second_slot == 0 || second_slot == first_slot {
            return Err(Error("second selector slot differs from first"));
        }
        let second_end = second_slot
            .checked_add(8)
            .ok_or(Error("second selector reference pair overflows"))?;
        if first_slot < second_end && second_slot < first_end {
            return Err(Error("selector reference pairs overlap"));
        }
        // Conditional skip: second test -> end (offset 116).
        at(sel, 60, &[0x74])?;
        if 62 + i32::from(sel[61] as i8) != 116 {
            return Err(Error("second selector JE does not skip to the end"));
        }
        if call(sel, WINDOWS[2].0, 78)? != 0x7fc7e0 {
            return Err(Error("second selector resolver changed"));
        }
        if call(sel, WINDOWS[2].0, 95)? != 0xad0630 {
            return Err(Error("second selector home writer changed"));
        }

        // CreateCaptains primary registration binds handler 0xab4750, and the
        // handler joins the main work (abd160).
        at(capt_reg, 0, &[0x68])?;
        if word(capt_reg, 1)? != 0xf15d24 {
            return Err(Error("CreateCaptains name changed"));
        }
        at(capt_reg, 5, &[0x68])?;
        if word(capt_reg, 6)? != 0xab4750 {
            return Err(Error("CreateCaptains handler changed"));
        }
        if call(capt_reg, WINDOWS[3].0, 10)? != 0x8ccc80 {
            return Err(Error("CreateCaptains registrar changed"));
        }
        // handler -> main-work join: 0xab4750 tail-jumps to 0xabd160.
        at(create_jmp, 2, &[0x8b, 0xc8])?;
        if jmp(create_jmp, WINDOWS[4].0, 4)? != 0xabd160 {
            return Err(Error("CreateCaptains handler does not join the main work"));
        }

        // The main work writes the two encoded reference pairs; the selector's
        // slots must match those create-write offsets (second word = slot + 4).
        at(create1, 0, &[0x89, 0x87])?;
        if word(create1, 2)? != first_slot {
            return Err(Error("create first slot differs from selector"));
        }
        at(create1, 9, &[0x89, 0x87])?;
        if word(create1, 11)? != first_end - 4 {
            return Err(Error("create first slot second word differs"));
        }
        at(create2, 0, &[0x89, 0x87])?;
        if word(create2, 2)? != second_slot {
            return Err(Error("create second slot differs from selector"));
        }
        at(create2, 9, &[0x89, 0x87])?;
        if word(create2, 11)? != second_end - 4 {
            return Err(Error("create second slot second word differs"));
        }

        Ok(Self {
            image_sha256,
            first_selector_mask,
            second_selector_mask,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let source = synthetic_windows();
        Self::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .expect("synthetic captain-policy instructions")
    }
}

// Non-stock synthetic operands prove decode reads operands, not constants. The
// fixture identity [0x42; 32] is shared with GatheringPolicy::fixture() so
// cross-policy admission tests exercise a valid synthetic source identity.
#[cfg(any(test, feature = "test-fixtures"))]
const FIRST_MASK: u8 = 0x10;
#[cfg(any(test, feature = "test-fixtures"))]
const SECOND_MASK: u8 = 0x20;
#[cfg(any(test, feature = "test-fixtures"))]
const FIRST_SLOT: u32 = 0x330;
#[cfg(any(test, feature = "test-fixtures"))]
const SECOND_SLOT: u32 = 0x340;

#[cfg(any(test, feature = "test-fixtures"))]
fn synthetic_windows() -> Vec<Vec<u8>> {
    fn put(w: &mut [Vec<u8>], i: usize, at: usize, data: &[u8]) {
        w[i][at..at + data.len()].copy_from_slice(data);
    }
    fn dword(w: &mut [Vec<u8>], i: usize, at: usize, data: u32) {
        put(w, i, at, &data.to_le_bytes());
    }
    fn call(w: &mut [Vec<u8>], i: usize, at: usize, target: u32) {
        put(w, i, at, &[0xe8]);
        dword(
            w,
            i,
            at + 1,
            target.wrapping_sub(WINDOWS[i].0 + at as u32 + 5),
        );
    }
    fn jmp(w: &mut [Vec<u8>], i: usize, at: usize, target: u32) {
        put(w, i, at, &[0xe9]);
        dword(
            w,
            i,
            at + 1,
            target.wrapping_sub(WINDOWS[i].0 + at as u32 + 5),
        );
    }
    fn move_pair(w: &mut [Vec<u8>], i: usize, slot: u32) {
        put(w, i, 0, &[0x89, 0x87]);
        dword(w, i, 2, slot);
        put(w, i, 6, &[0x8b, 0x41, 0x18]);
        put(w, i, 9, &[0x89, 0x87]);
        dword(w, i, 11, slot + 4);
    }
    let mut w = WINDOWS
        .iter()
        .map(|(_, len, _)| vec![0; *len])
        .collect::<Vec<_>>();

    // SetCaptainHome registration (sig, name, handler, registrar).
    put(&mut w, 0, 0, &[0x68]);
    dword(&mut w, 0, 1, 0xe6f5a8);
    put(&mut w, 0, 5, &[0x68]);
    dword(&mut w, 0, 6, 0xf15d34);
    put(&mut w, 0, 10, &[0x68]);
    dword(&mut w, 0, 11, 0xab6f10);
    call(&mut w, 0, 15, 0x8ccc80);

    // SetCaptainHome handler entry.
    put(&mut w, 1, 0, &[0x55, 0x8b, 0xec]);
    put(&mut w, 1, 3, &[0xa1]);
    dword(&mut w, 1, 4, 0x11b4a10);
    call(&mut w, 1, 50, 0xac26d0);
    call(&mut w, 1, 73, 0xac26d0);

    // Selector.
    put(&mut w, 2, 4, &[0xf6, 0x45, 0x08, FIRST_MASK]);
    put(&mut w, 2, 13, &[0x74, 0x29]); // first test -> second test (56)
    put(&mut w, 2, 15, &[0x8d, 0x88]);
    dword(&mut w, 2, 17, FIRST_SLOT);
    call(&mut w, 2, 31, 0x7fc7e0);
    call(&mut w, 2, 48, 0xad0630);
    put(&mut w, 2, 56, &[0xf6, 0x45, 0x08, SECOND_MASK]);
    put(&mut w, 2, 60, &[0x74, 0x36]); // second test -> end (116)
    put(&mut w, 2, 62, &[0x8d, 0x88]);
    dword(&mut w, 2, 64, SECOND_SLOT);
    call(&mut w, 2, 78, 0x7fc7e0);
    call(&mut w, 2, 95, 0xad0630);

    // CreateCaptains registration.
    put(&mut w, 3, 0, &[0x68]);
    dword(&mut w, 3, 1, 0xf15d24);
    put(&mut w, 3, 5, &[0x68]);
    dword(&mut w, 3, 6, 0xab4750);
    call(&mut w, 3, 10, 0x8ccc80);

    // CreateCaptains handler -> main-work join.
    put(&mut w, 4, 0, &[0x33, 0xc0, 0x8b, 0xc8]);
    jmp(&mut w, 4, 4, 0xabd160);

    // Create home-write pairs (selector slots on both ends).
    move_pair(&mut w, 5, FIRST_SLOT);
    move_pair(&mut w, 6, SECOND_SLOT);

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
        assert!(CaptainPolicy::read(b"MZunsupported", budget).is_err());
    }

    #[test]
    fn synthetic_operands_drive_the_policy() {
        let policy = CaptainPolicy::fixture();
        assert_eq!(policy.first_selector_mask, FIRST_MASK);
        assert_eq!(policy.second_selector_mask, SECOND_MASK);
        assert_ne!(policy.first_selector_mask, policy.second_selector_mask);
        assert_eq!(policy.image_sha256, [0x42; 32]);
    }

    #[test]
    fn synthetic_nonstock_masks_discriminate() {
        // F6 TEST imm8 masks are unsigned u8; 0x80 is a valid single bit.
        let mut source = synthetic_windows();
        source[2][7] = 0x80;
        let policy = CaptainPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("accepted 0x80 first mask");
        assert_eq!(policy.first_selector_mask, 0x80);

        source = synthetic_windows();
        source[2][7] = 0x40;
        let policy = CaptainPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("changed first mask operand");
        assert_eq!(policy.first_selector_mask, 0x40);

        source = synthetic_windows();
        source[2][59] = 0x80;
        let policy = CaptainPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("accepted distinct 0x80 second mask");
        assert_eq!(policy.second_selector_mask, 0x80);

        source = synthetic_windows();
        source[2][59] = FIRST_MASK;
        assert!(
            CaptainPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32],
            )
            .is_err(),
            "non-distinct second selector mask should refuse"
        );
    }

    #[test]
    fn malformed_branch_or_slot_relationships_refuse() {
        let invalid: &[fn(&mut Vec<Vec<u8>>)] = &[
            |w| w[0][0] ^= 1,
            |w| w[1][0] ^= 1,
            |w| w[2][4] ^= 1,
            |w| w[3][0] ^= 1,
            |w| w[0][11] ^= 0x10, // SetCaptainHome handler change
            |w| w[3][6] ^= 0x10,  // CreateCaptains handler change
            |w| w[1][4] ^= 0x10,  // AI context global change
            |w| w[2][14] ^= 1,    // first JE skip target changed
            |w| w[2][61] ^= 1,    // second JE skip target changed
            |w| w[2][64] = FIRST_SLOT.to_le_bytes()[0], // second == first
            |w| w[2][7] = 0x00,   // non-single-bit first mask
            |w| w[2][59] = 0x00,  // non-single-bit second mask
            |w| w[5][2] ^= 0x10,  // create first slot no longer matches selector
            |w| w[6][11] ^= 0x10, // create second slot second word differs
        ];
        for (index, mutate) in invalid.iter().enumerate() {
            let mut source = synthetic_windows();
            mutate(&mut source);
            assert!(
                CaptainPolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32],
                )
                .is_err(),
                "case {index} should refuse"
            );
        }
    }

    #[test]
    fn overflowing_or_overlapping_reference_pairs_refuse() {
        for (first, second) in [
            (u32::MAX, SECOND_SLOT),
            (FIRST_SLOT, u32::MAX),
            (FIRST_SLOT, FIRST_SLOT + 4),
            (FIRST_SLOT, FIRST_SLOT + 7),
        ] {
            let mut source = synthetic_windows();
            source[2][17..21].copy_from_slice(&first.to_le_bytes());
            source[2][64..68].copy_from_slice(&second.to_le_bytes());
            for (index, slot) in [(5, first), (6, second)] {
                source[index][2..6].copy_from_slice(&slot.to_le_bytes());
                source[index][11..15].copy_from_slice(&slot.wrapping_add(4).to_le_bytes());
            }
            assert!(
                CaptainPolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32],
                )
                .is_err()
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
        let policy = CaptainPolicy::read(&image, budget).expect("selected captain-policy operands");
        assert_eq!(policy.image_sha256, REVIEWED_IMAGE);
        assert_eq!(policy.first_selector_mask, 0x01);
        assert_eq!(policy.second_selector_mask, 0x02);
        assert_ne!(policy.first_selector_mask, policy.second_selector_mask);
    }
}
