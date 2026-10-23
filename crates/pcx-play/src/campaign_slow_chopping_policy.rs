//! `SetSlowChopping` AI-context flag reader from the reviewed client image. This
//! binds the explicit flag and ordinary harvest damage/deposit overrides.
//! It does not qualify modified workers, controller actions or initial defaults.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct SlowChoppingPolicy {
    pub image_sha256: Sha256Digest,
    pub context_mask: u32,
    pub tree_damage: f32,
    pub deposit_lumber: i32,
}

#[derive(Debug, thiserror::Error)]
#[error("campaign slow-chopping policy: {0}")]
pub struct Error(&'static str);

// VAs and lengths identify instruction windows in the reviewed PE. Every value
// returned below is decoded from an operand within those windows.
const WINDOWS: [(u32, usize, bool); 8] = [
    (0xab697d, 20, true),  // SetSlowChopping registration banner (sig/name/handler)
    (0xab74a0, 71, true),  // SetSlowChopping handler through final ret
    (0xe700b0, 5, false),  // signature bytes "(B)V\0"
    (0xf16168, 16, false), // NUL-terminated name bytes
    (0xac3e80, 62, true),  // canonical owner-to-context flag query
    (0xb79757, 106, true), // damage substitution, packet, original carry amount
    (0xb7986b, 96, true),  // deposit substitution and player credit
    (0x10db078, 4, false), // damage value selected by the conditional move
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

impl SlowChoppingPolicy {
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
        let [reg, h, sig, name, query, hit, deposit, damage] = w else {
            return Err(Error("incomplete slow-chopping windows"));
        };

        for (window, (_, length, _)) in w.iter().zip(WINDOWS) {
            if window.len() < length {
                return Err(Error("truncated selected window"));
            }
        }

        // Registration binds the actual signature and name strings plus the
        // handler 0xab74a0 through the registrar.
        at(reg, 0, &[0x68])?;
        if word(reg, 1)? != WINDOWS[2].0 {
            return Err(Error("SetSlowChopping signature pointer changed"));
        }
        at(reg, 5, &[0x68])?;
        if word(reg, 6)? != WINDOWS[3].0 {
            return Err(Error("SetSlowChopping name pointer changed"));
        }
        at(reg, 10, &[0x68])?;
        if word(reg, 11)? != 0xab74a0 {
            return Err(Error("SetSlowChopping handler changed"));
        }
        if call(reg, WINDOWS[0].0, 15)? != 0x8ccc80 {
            return Err(Error("SetSlowChopping registrar changed"));
        }
        at(sig, 0, b"(B)V\0")?;
        at(name, 0, b"SetSlowChopping\0")?;

        let (field, context_mask) =
            crate::campaign_ai_flag::decode(h, WINDOWS[1].0).map_err(|error| Error(error.0))?;

        // The unit behavior queries the canonical owner's context and the same
        // field/bit as the setter, returning false for an absent context.
        at(query, 0, &[0x55, 0x8b, 0xec, 0xff, 0x75, 0x08, 0x8b, 0x0d])?;
        if word(query, 8)? != 0x113b6f4 || call(query, WINDOWS[4].0, 12)? != 0x4777d0 {
            return Err(Error("owner player lookup changed"));
        }
        at(
            query,
            17,
            &[
                0x8d, 0x88, 0x94, 0x03, 0, 0, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x74,
                0x19,
            ],
        )?;
        if call(query, WINDOWS[4].0, 33)? != 0x7fc7e0 {
            return Err(Error("owner context resolver changed"));
        }
        at(query, 38, &[0x85, 0xc0, 0x74, 0x10, 0xf6, 0x80])?;
        if word(query, 44)? != field || u32::from(query[48]) != context_mask {
            return Err(Error("owner query differs from the flag setter"));
        }
        at(
            query,
            49,
            &[
                0x74, 0x07, 0xb8, 1, 0, 0, 0, 0x5d, 0xc3, 0x33, 0xc0, 0x5d, 0xc3,
            ],
        )?;

        // The selected real replaces tree damage only. Carry still receives
        // the original integer from the same source member after the packet.
        at(hit, 0, &[0xff, 0xb6])?;
        at(hit, 6, &[0x8d, 0x45, 0xf8, 0x50])?;
        if call(hit, WINDOWS[5].0, 10)? != 0x9a1620 {
            return Err(Error("harvest amount conversion changed"));
        }
        at(
            hit,
            15,
            &[
                0x8b, 0x07, 0x83, 0xc4, 0x08, 0x8b, 0xcf, 0xff, 0x90, 0xec, 0, 0, 0, 0x50,
            ],
        )?;
        if call(hit, WINDOWS[5].0, 29)? != WINDOWS[4].0 {
            return Err(Error("damage owner query changed"));
        }
        at(
            hit,
            34,
            &[
                0x8b, 0x4d, 0xf8, 0x83, 0xc4, 0x04, 0x85, 0xc0, 0x8d, 0x45, 0xf8, 0x0f, 0x45, 0x0d,
            ],
        )?;
        if word(hit, 48)? != WINDOWS[7].0 {
            return Err(Error("damage conditional move uses a different real"));
        }
        at(
            hit,
            52,
            &[
                0x6a, 0x16, 0x6a, 0, 0x57, 0x6a, 0x10, 0x89, 0x4d, 0xf8, 0x8d, 0x4d, 0xd0, 0x50,
            ],
        )?;
        if call(hit, WINDOWS[5].0, 66)? != 0x6a9940 {
            return Err(Error("destructable packet builder changed"));
        }
        at(
            hit,
            71,
            &[
                0x8b, 0x03, 0x8d, 0x4d, 0xd0, 0x6a, 0, 0x6a, 0, 0x51, 0x8d, 0x4d, 0xf4, 0x51, 0x8b,
                0xcb, 0xff, 0x90, 0x20, 1, 0, 0, 0xff, 0xb6,
            ],
        )?;
        if word(hit, 95)? != word(hit, 2)? || call(hit, WINDOWS[5].0, 101)? != 0xb791f0 {
            return Err(Error(
                "carry no longer receives the original harvest amount",
            ));
        }
        at(hit, 99, &[0x8b, 0xce])?;

        // For the ordinary return path the bit selects an integer credit,
        // independently of the carried load. Modified-unit adjustment is not modeled.
        at(
            deposit,
            0,
            &[
                0x8b, 0x07, 0x8b, 0xcf, 0xff, 0x90, 0xec, 0, 0, 0, 0x8b, 0xf0, 0x56,
            ],
        )?;
        if call(deposit, WINDOWS[6].0, 13)? != WINDOWS[4].0 {
            return Err(Error("deposit owner query changed"));
        }
        at(
            deposit,
            18,
            &[0x8b, 0x4d, 0xfc, 0x83, 0xc4, 0x04, 0x85, 0xc0, 0xba],
        )?;
        at(
            deposit,
            31,
            &[0x0f, 0x45, 0xca, 0x89, 0x4d, 0xfc, 0x8b, 0x4d, 0xf8, 0x56],
        )?;
        if call(deposit, WINDOWS[6].0, 41)? != 0x4777d0 {
            return Err(Error("deposit player lookup changed"));
        }
        at(deposit, 46, &[0xf6, 0x87])?;
        if deposit[52] == 0 || !deposit[52].is_power_of_two() {
            return Err(Error("modified-worker guard is not a single flag"));
        }
        at(
            deposit,
            53,
            &[
                0x8b, 0xf0, 0x74, 0x17, 0x8b, 0x17, 0x8d, 0x45, 0xfc, 0x50, 0x8b, 0xcf, 0xff, 0x92,
                0xec, 0, 0, 0, 0x50,
            ],
        )?;
        if call(deposit, WINDOWS[6].0, 72)? != 0xabfbb0 {
            return Err(Error("modified-worker adjustment changed"));
        }
        at(deposit, 77, &[0x83, 0xc4, 0x08])?;
        at(
            deposit,
            80,
            &[0x6a, 1, 0x6a, 1, 0x6a, 1, 0xff, 0x75, 0xfc, 0x8b, 0xce],
        )?;
        if call(deposit, WINDOWS[6].0, 91)? != 0x4b5860 {
            return Err(Error("player lumber credit changed"));
        }
        let tree_damage = f32::from_bits(word(damage, 0)?);
        if !tree_damage.is_normal() || tree_damage <= 0.0 {
            return Err(Error("tree damage must be positive, finite and normal"));
        }
        let deposit_lumber = i32::try_from(word(deposit, 27)?)
            .map_err(|_| Error("deposit credit exceeds the signed range"))?;
        if deposit_lumber <= 0 {
            return Err(Error("deposit credit must be positive"));
        }

        Ok(Self {
            image_sha256,
            context_mask,
            tree_damage,
            deposit_lumber,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let source = synthetic_windows();
        Self::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .expect("synthetic slow-chopping instructions")
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
const NONSTOCK_MASK: u8 = 0x02;
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
    dword(&mut w, 0, 6, 0xf16168);
    put(&mut w, 0, 10, &[0x68]);
    dword(&mut w, 0, 11, 0xab74a0);
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

    put(
        &mut w,
        4,
        0,
        &[0x55, 0x8b, 0xec, 0xff, 0x75, 0x08, 0x8b, 0x0d],
    );
    dword(&mut w, 4, 8, 0x113b6f4);
    call(&mut w, 4, 12, 0x4777d0);
    put(
        &mut w,
        4,
        17,
        &[
            0x8d, 0x88, 0x94, 3, 0, 0, 0x8b, 0x41, 4, 0x23, 1, 0x83, 0xf8, 0xff, 0x74, 0x19,
        ],
    );
    call(&mut w, 4, 33, 0x7fc7e0);
    put(&mut w, 4, 38, &[0x85, 0xc0, 0x74, 0x10, 0xf6, 0x80]);
    dword(&mut w, 4, 44, NONSTOCK_FIELD);
    put(
        &mut w,
        4,
        48,
        &[
            NONSTOCK_MASK,
            0x74,
            7,
            0xb8,
            1,
            0,
            0,
            0,
            0x5d,
            0xc3,
            0x33,
            0xc0,
            0x5d,
            0xc3,
        ],
    );

    put(&mut w, 5, 0, &[0xff, 0xb6]);
    dword(&mut w, 5, 2, 0x240);
    put(&mut w, 5, 6, &[0x8d, 0x45, 0xf8, 0x50]);
    call(&mut w, 5, 10, 0x9a1620);
    put(
        &mut w,
        5,
        15,
        &[
            0x8b, 7, 0x83, 0xc4, 8, 0x8b, 0xcf, 0xff, 0x90, 0xec, 0, 0, 0, 0x50,
        ],
    );
    call(&mut w, 5, 29, WINDOWS[4].0);
    put(
        &mut w,
        5,
        34,
        &[
            0x8b, 0x4d, 0xf8, 0x83, 0xc4, 4, 0x85, 0xc0, 0x8d, 0x45, 0xf8, 0x0f, 0x45, 0x0d,
        ],
    );
    dword(&mut w, 5, 48, WINDOWS[7].0);
    put(
        &mut w,
        5,
        52,
        &[
            0x6a, 0x16, 0x6a, 0, 0x57, 0x6a, 0x10, 0x89, 0x4d, 0xf8, 0x8d, 0x4d, 0xd0, 0x50,
        ],
    );
    call(&mut w, 5, 66, 0x6a9940);
    put(
        &mut w,
        5,
        71,
        &[
            0x8b, 3, 0x8d, 0x4d, 0xd0, 0x6a, 0, 0x6a, 0, 0x51, 0x8d, 0x4d, 0xf4, 0x51, 0x8b, 0xcb,
            0xff, 0x90, 0x20, 1, 0, 0, 0xff, 0xb6,
        ],
    );
    dword(&mut w, 5, 95, 0x240);
    put(&mut w, 5, 99, &[0x8b, 0xce]);
    call(&mut w, 5, 101, 0xb791f0);

    put(
        &mut w,
        6,
        0,
        &[
            0x8b, 7, 0x8b, 0xcf, 0xff, 0x90, 0xec, 0, 0, 0, 0x8b, 0xf0, 0x56,
        ],
    );
    call(&mut w, 6, 13, WINDOWS[4].0);
    put(
        &mut w,
        6,
        18,
        &[0x8b, 0x4d, 0xfc, 0x83, 0xc4, 4, 0x85, 0xc0, 0xba],
    );
    dword(&mut w, 6, 27, 3);
    put(
        &mut w,
        6,
        31,
        &[0x0f, 0x45, 0xca, 0x89, 0x4d, 0xfc, 0x8b, 0x4d, 0xf8, 0x56],
    );
    call(&mut w, 6, 41, 0x4777d0);
    put(&mut w, 6, 46, &[0xf6, 0x87]);
    dword(&mut w, 6, 48, 0x180);
    put(
        &mut w,
        6,
        52,
        &[
            0x10, 0x8b, 0xf0, 0x74, 0x17, 0x8b, 0x17, 0x8d, 0x45, 0xfc, 0x50, 0x8b, 0xcf, 0xff,
            0x92, 0xec, 0, 0, 0, 0x50,
        ],
    );
    call(&mut w, 6, 72, 0xabfbb0);
    put(&mut w, 6, 77, &[0x83, 0xc4, 0x08]);
    put(
        &mut w,
        6,
        80,
        &[0x6a, 1, 0x6a, 1, 0x6a, 1, 0xff, 0x75, 0xfc, 0x8b, 0xce],
    );
    call(&mut w, 6, 91, 0x4b5860);
    dword(&mut w, 7, 0, 0.25_f32.to_bits());

    // Signature and name data strings.
    put(&mut w, 2, 0, b"(B)V\0");
    put(&mut w, 3, 0, b"SetSlowChopping\0");

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
        assert!(SlowChoppingPolicy::read(b"MZunsupported", budget).is_err());
    }

    #[test]
    fn synthetic_operands_drive_the_policy() {
        let policy = SlowChoppingPolicy::fixture();
        assert_eq!(policy.context_mask, u32::from(NONSTOCK_MASK));
        assert_eq!(policy.image_sha256, [0x42; 32]);
        assert_eq!(policy.tree_damage, 0.25);
        assert_eq!(policy.deposit_lumber, 3);
    }

    #[test]
    fn synthetic_nonstock_masks_discriminate() {
        let mut source = synthetic_windows();
        source[1][59] = 0x08;
        source[1][68] = 0xf7;
        source[4][48] = 0x08;
        let policy = SlowChoppingPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("changed set mask operand");
        assert_eq!(policy.context_mask, 0x08);

        source = synthetic_windows();
        source[1][59] = 0x80;
        assert!(
            SlowChoppingPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32],
            )
            .is_err(),
            "sign-extended set mask should refuse"
        );

        source = synthetic_windows();
        source[1][59] = 0x60;
        assert!(
            SlowChoppingPolicy::decode(
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
            |w| w[3][15] = b'X',          // unterminated name
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
                SlowChoppingPolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32],
                )
                .is_err(),
                "case {index} should refuse"
            );
        }
    }

    #[test]
    fn every_truncated_window_refuses() {
        let source = synthetic_windows();
        for index in 0..source.len() {
            for length in 0..source[index].len() {
                let mut shortened = source.clone();
                shortened[index].truncate(length);
                assert!(
                    SlowChoppingPolicy::decode(
                        &shortened.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                        [0; 32],
                    )
                    .is_err(),
                    "window {index}, length {length}"
                );
            }
        }
    }

    #[test]
    fn harvest_overrides_follow_independent_source_operands() {
        let mut source = synthetic_windows();
        source[7].copy_from_slice(&0.75_f32.to_le_bytes());
        source[6][27..31].copy_from_slice(&5_u32.to_le_bytes());
        let policy = SlowChoppingPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .unwrap();
        assert_eq!(policy.tree_damage, 0.75);
        assert_eq!(policy.deposit_lumber, 5);
    }

    #[test]
    fn changed_owner_queries_branches_and_carry_lane_refuse() {
        for (window, offset) in [
            (4, 31),
            (4, 40),
            (4, 44),
            (4, 48),
            (5, 29),
            (5, 46),
            (5, 48),
            (5, 95),
            (5, 101),
            (6, 13),
            (6, 31),
            (6, 56),
            (6, 88),
            (6, 91),
        ] {
            let mut source = synthetic_windows();
            source[window][offset] ^= 1;
            assert!(
                SlowChoppingPolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32],
                )
                .is_err(),
                "window {window}, offset {offset}"
            );
        }
    }

    #[test]
    fn nonfinite_or_nonpositive_harvest_overrides_refuse() {
        for damage in [f32::NAN, f32::INFINITY, -1.0, 0.0, f32::from_bits(1)] {
            let mut source = synthetic_windows();
            source[7].copy_from_slice(&damage.to_le_bytes());
            assert!(
                SlowChoppingPolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32],
                )
                .is_err()
            );
        }
        for credit in [0_u32, 0x80000000, u32::MAX] {
            let mut source = synthetic_windows();
            source[6][27..31].copy_from_slice(&credit.to_le_bytes());
            assert!(
                SlowChoppingPolicy::decode(
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
        let policy =
            SlowChoppingPolicy::read(&image, budget).expect("selected slow-chopping operands");
        assert_eq!(policy.image_sha256, REVIEWED_IMAGE);
        assert_eq!(policy.context_mask, 0x40);
        assert_eq!(policy.tree_damage, 0.1);
        assert_eq!(policy.deposit_lumber, 1);
    }
}
