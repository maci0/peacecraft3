//! `SetNewHeroes` AI-context flag reader from the reviewed client image. This
//! is a source reader for the immediate native setter only; it makes no
//! gameplay-consumer or default claim about the flag.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct NewHeroesPolicy {
    pub image_sha256: Sha256Digest,
    pub context_mask: u32,
}

#[derive(Debug, thiserror::Error)]
#[error("campaign new-heroes policy: {0}")]
pub struct Error(&'static str);

// VAs and lengths identify instruction windows in the reviewed PE. Every value
// returned below is decoded from an operand within those windows.
const WINDOWS: [(u32, usize, bool); 4] = [
    (0xab6166, 20, true),  // SetNewHeroes registration banner (sig/name/handler)
    (0xab72d0, 71, true),  // SetNewHeroes handler through final ret
    (0xe700b0, 5, false),  // signature bytes "(B)V\0"
    (0xf15aa0, 13, false), // NUL-terminated name bytes
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

impl NewHeroesPolicy {
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
        let [reg, h, sig, name] = w else {
            return Err(Error("incomplete new-heroes windows"));
        };

        // Registration binds the actual signature and name strings plus the
        // handler 0xab72d0 through the registrar.
        at(reg, 0, &[0x68])?;
        if word(reg, 1)? != WINDOWS[2].0 {
            return Err(Error("SetNewHeroes signature pointer changed"));
        }
        at(reg, 5, &[0x68])?;
        if word(reg, 6)? != WINDOWS[3].0 {
            return Err(Error("SetNewHeroes name pointer changed"));
        }
        at(reg, 10, &[0x68])?;
        if word(reg, 11)? != 0xab72d0 {
            return Err(Error("SetNewHeroes handler changed"));
        }
        if call(reg, WINDOWS[0].0, 15)? != 0x8ccc80 {
            return Err(Error("SetNewHeroes registrar changed"));
        }
        at(sig, 0, b"(B)V\0")?;
        at(name, 0, b"SetNewHeroes\0")?;

        // Handler: read AI-context global; null context writes 0x57 and jumps to
        // the error sink; else resolve via the shared resolver; then set/clear
        // the flag on the boolean argument.
        at(h, 0, &[0x55, 0x8b, 0xec])?;
        at(h, 3, &[0xa1])?;
        if word(h, 4)? != 0x11b4a10 {
            return Err(Error("AI context global changed"));
        }
        at(h, 8, &[0x85, 0xc0])?;
        at(h, 10, &[0x75])?;
        if 12 + i32::from(h[11] as i8) != 25 {
            return Err(Error("null context skip does not reach the resolver path"));
        }
        at(h, 12, &[0xc7, 0x45, 0x08, 0x57, 0, 0, 0])?;
        at(h, 19, &[0x5d])?;
        if jmp(h, WINDOWS[1].0, 20)? != 0x43eca0 {
            return Err(Error("error sink changed"));
        }
        at(h, 25, &[0x8d, 0x48, 0x2c])?;
        at(h, 28, &[0x8b, 0x41, 0x04])?;
        at(h, 31, &[0x23, 0x01])?;
        at(h, 33, &[0x83, 0xf8, 0xff])?;
        at(h, 36, &[0x75])?;
        if 38 + i32::from(h[37] as i8) != 42 {
            return Err(Error("sentinel branch does not reach the resolver"));
        }
        at(h, 38, &[0x33, 0xc0])?;
        at(h, 40, &[0xeb])?;
        if 42 + i32::from(h[41] as i8) != 47 {
            return Err(Error("sentinel default does not reach the boolean branch"));
        }
        if call(h, WINDOWS[1].0, 42)? != 0x7fc7e0 {
            return Err(Error("context resolver changed"));
        }
        at(h, 47, &[0x83, 0x7d, 0x08, 0x00])?;
        at(h, 51, &[0x74])?;
        if 53 + i32::from(h[52] as i8) != 62 {
            return Err(Error("boolean branch does not skip to the clear path"));
        }

        // Set: `or r/m32, imm8` (83 /0). The imm8 must be a single positive
        // bit; a sign-extended negative imm8 (bit 7 set) is rejected.
        at(h, 53, &[0x83, 0x88])?;
        let field_offset = word(h, 55)?;
        let or_imm = *h.get(59).ok_or(Error("missing set mask operand"))?;
        if or_imm & 0x80 != 0 {
            return Err(Error("set mask is a negative imm8"));
        }
        if or_imm == 0 || or_imm.count_ones() != 1 {
            return Err(Error("context mask is not a single positive bit"));
        }
        let context_mask = u32::from(or_imm);
        at(h, 60, &[0x5d])?;
        at(h, 61, &[0xc3])?;

        // Clear: `and r/m32, imm8` (83 /4) must target the same field and be
        // the sign-extended complement of the mask.
        at(h, 62, &[0x83, 0xa0])?;
        if word(h, 64)? != field_offset {
            return Err(Error("clear field offset differs from set"));
        }
        let and_imm = *h.get(68).ok_or(Error("missing clear mask operand"))?;
        if and_imm & 0x80 == 0 {
            return Err(Error("clear mask does not sign-extend its complement"));
        }
        if i32::from(and_imm as i8) as u32 != !context_mask {
            return Err(Error("clear mask is not the complement of the set mask"));
        }
        at(h, 69, &[0x5d])?;
        at(h, 70, &[0xc3])?;

        Ok(Self {
            image_sha256,
            context_mask,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let source = synthetic_windows();
        Self::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .expect("synthetic new-heroes instructions")
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
const NONSTOCK_MASK: u8 = 0x40;
#[cfg(any(test, feature = "test-fixtures"))]
const NONSTOCK_FIELD: u32 = 0x330;

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
    let mut w = WINDOWS
        .iter()
        .map(|(_, len, _)| vec![0; *len])
        .collect::<Vec<_>>();

    // Registration.
    put(&mut w, 0, 0, &[0x68]);
    dword(&mut w, 0, 1, 0xe700b0);
    put(&mut w, 0, 5, &[0x68]);
    dword(&mut w, 0, 6, 0xf15aa0);
    put(&mut w, 0, 10, &[0x68]);
    dword(&mut w, 0, 11, 0xab72d0);
    call(&mut w, 0, 15, 0x8ccc80);

    // Handler (full context-control chain and both returns).
    put(&mut w, 1, 0, &[0x55, 0x8b, 0xec]);
    put(&mut w, 1, 3, &[0xa1]);
    dword(&mut w, 1, 4, 0x11b4a10);
    put(&mut w, 1, 8, &[0x85, 0xc0, 0x75, 0x0d]);
    put(&mut w, 1, 12, &[0xc7, 0x45, 0x08, 0x57, 0, 0, 0]);
    put(&mut w, 1, 19, &[0x5d]);
    jmp(&mut w, 1, 20, 0x43eca0);
    put(&mut w, 1, 25, &[0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04]);
    put(&mut w, 1, 31, &[0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x04]);
    put(&mut w, 1, 38, &[0x33, 0xc0, 0xeb, 0x05]);
    call(&mut w, 1, 42, 0x7fc7e0);
    put(&mut w, 1, 47, &[0x83, 0x7d, 0x08, 0x00, 0x74, 0x09]);
    put(&mut w, 1, 53, &[0x83, 0x88]);
    dword(&mut w, 1, 55, NONSTOCK_FIELD);
    put(&mut w, 1, 59, &[NONSTOCK_MASK]);
    put(&mut w, 1, 60, &[0x5d, 0xc3]);
    put(&mut w, 1, 62, &[0x83, 0xa0]);
    dword(&mut w, 1, 64, NONSTOCK_FIELD);
    put(&mut w, 1, 68, &[!NONSTOCK_MASK]);
    put(&mut w, 1, 69, &[0x5d, 0xc3]);

    // Signature and name data strings.
    put(&mut w, 2, 0, b"(B)V\0");
    put(&mut w, 3, 0, b"SetNewHeroes\0");

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
        assert!(NewHeroesPolicy::read(b"MZunsupported", budget).is_err());
    }

    #[test]
    fn synthetic_operands_drive_the_policy() {
        let policy = NewHeroesPolicy::fixture();
        assert_eq!(policy.context_mask, u32::from(NONSTOCK_MASK));
        assert_eq!(policy.image_sha256, [0x42; 32]);
    }

    #[test]
    fn synthetic_nonstock_masks_discriminate() {
        let mut source = synthetic_windows();
        source[1][59] = 0x08;
        source[1][68] = 0xf7;
        let policy = NewHeroesPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("changed set mask operand");
        assert_eq!(policy.context_mask, 0x08);

        source = synthetic_windows();
        source[1][59] = 0x80;
        assert!(
            NewHeroesPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32],
            )
            .is_err(),
            "sign-extended set mask should refuse"
        );

        source = synthetic_windows();
        source[1][59] = 0x60;
        assert!(
            NewHeroesPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32],
            )
            .is_err(),
            "multi-bit set mask should refuse"
        );
    }

    #[test]
    fn malformed_opcode_branch_field_or_clear_relationships_refuse() {
        let invalid: &[fn(&mut Vec<Vec<u8>>)] = &[
            |w| w[0][0] ^= 1,
            |w| w[1][0] ^= 1,
            |w| w[0][11] ^= 0x10,         // handler change
            |w| w[2][0] ^= 1,             // signature string changed
            |w| w[3][0] ^= 1,             // name string changed
            |w| w[2][4] = b'X',           // unterminated signature
            |w| w[3][12] = b'X',          // unterminated name
            |w| w[1][4] ^= 0x10,          // AI context global change
            |w| w[1][11] ^= 1,            // null-context skip changed
            |w| w[1][26] ^= 0x10,         // reference LEA offset changed
            |w| w[1][31] ^= 1,            // reference AND opcode changed
            |w| w[1][35] ^= 1,            // sentinel compare immediate changed
            |w| w[1][37] ^= 1,            // sentinel branch changed
            |w| w[1][52] ^= 1,            // boolean branch skip changed
            |w| w[1][55] ^= 0x10,         // set field offset changed (clear unchanged)
            |w| w[1][64] ^= 0x10,         // clear field offset changed (set unchanged)
            |w| w[1][68] = NONSTOCK_MASK, // clear is not the complement
            |w| w[1][68] = 0x00,          // clear sign bit clear
            |w| w[1][53] ^= 1,            // set opcode changed
            |w| w[1][62] ^= 1,            // clear opcode changed
            |w| w[1][60] ^= 1,            // true-path pop changed
            |w| w[1][69] ^= 1,            // false-path pop changed
            |w| w[1][70] ^= 1,            // false-path ret changed
        ];
        for (index, mutate) in invalid.iter().enumerate() {
            let mut source = synthetic_windows();
            mutate(&mut source);
            assert!(
                NewHeroesPolicy::decode(
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
        let policy = NewHeroesPolicy::read(&image, budget).expect("selected new-heroes operands");
        assert_eq!(policy.image_sha256, REVIEWED_IMAGE);
        assert_eq!(policy.context_mask, 0x4);
    }
}
