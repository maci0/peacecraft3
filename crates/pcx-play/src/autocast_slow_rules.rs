//! Selected Slow acquisition's ranked numeric gate; positive acquisition remains separate.
use super::{shape, word};

#[derive(Clone, Debug)]
pub struct Rules {
    pub class: [u8; 4],
    pub data_column: String,
    pub empty_data: f32,
}

fn call(bytes: &[u8], base: u32, at: usize) -> Result<u32, String> {
    shape(bytes, at, &[0xe8])?;
    Ok(base
        .wrapping_add(at as u32 + 5)
        .wrapping_add(word(bytes, at + 1)?))
}

impl Rules {
    pub(super) fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let table = 0xf6b1f8_u32;
        let getter = word(&read(table + 0x1c, 4, false)?, 0)?;
        let class = read(getter, 6, true)?;
        shape(&class, 0, &[0xb8])?;
        shape(&class, 5, &[0xc3])?;
        let class = word(&class, 1)?.to_be_bytes();
        let candidate = word(&read(table + 0x458, 4, false)?, 0)?;
        let gate = read(candidate + 0x9c, 0x3b, true)?;
        shape(
            &gate,
            0,
            &[0xff, 0x77, 0x50, 0x8d, 0x45, 0x08, 0x8b, 0xcf, 0x50],
        )?;
        let numeric = call(&gate, candidate + 0x9c, 9)?;
        shape(&gate, 14, &[0xf3, 0x0f, 0x10, 0x00, 0x0f, 0x2e, 0x05])?;
        let zero = word(&gate, 21)?;
        // Unequal DataC jumps past the attack-component and accepted-target tests.
        shape(&gate, 25, &[0x9f, 0xf6, 0xc4, 0x44, 0x7a, 0x5a, 0x8b, 0xce])?;
        if call(&gate, candidate + 0x9c, 33)? != 0x67ea50 {
            return Err("Slow attack-component predicate differs".into());
        }
        shape(
            &gate,
            38,
            &[
                0x85, 0xc0, 0x74, 0x70, 0x8b, 0x8e, 0xf4, 0x03, 0, 0, 0x83, 0xc1, 0x70,
            ],
        )?;

        let ranked = read(numeric + 0x33, 32, true)?;
        shape(&ranked, 0, &[0x6b, 0xce, 0x70])?;
        shape(&ranked, 20, &[0x8b, 0x49])?;
        let column_offset = ranked[22];
        let fallback = read(numeric + 0x97, 15, true)?;
        shape(&fallback, 0, &[0x8b, 0x45, 0x08, 0x8b, 0x0d])?;
        shape(&fallback, 9, &[0x5f, 0x5e, 0x89, 0x08, 0x5d, 0xc2])?;
        if word(&fallback, 5)? != zero {
            return Err("Slow ranked numeric fallback differs from its comparison".into());
        }

        let load = read(0x6ad1e5, 43, true)?;
        shape(&load, 0, &[0xff, 0x35])?;
        shape(&load, 40, &[0x89, 0x47, column_offset])?;
        if call(&load, 0x6ad1e5, 32)? != 0x6b06a0 {
            return Err("Slow ranked column loader differs".into());
        }
        let column = word(&read(word(&load, 2)?, 4, false)?, 0)?;
        let column = read(column, 6, false)?;
        if column.get(..4) != Some(b"Data") || !column[4].is_ascii_uppercase() || column[5] != 0 {
            return Err("Slow ranked data column is invalid".into());
        }
        // The typed table real getter returns the same initialized scalar for nonnumeric cells.
        let typed = read(0x95d220, 98, true)?;
        shape(&typed, 0x21, &[0x8a, 0x08, 0x80, 0xf9, b'F'])?;
        shape(&typed, 0x3e, &[0x80, 0xf9, b'I'])?;
        shape(&typed, 0x5d, &[0xa1])?;
        let init = read(0x401300, 16, true)?;
        shape(&init, 0, &[0x6a])?;
        shape(&init, 2, &[0x68])?;
        shape(&init, 12, &[0x83, 0xc4, 0x08, 0xc3])?;
        let converter = call(&init, 0x401300, 7)?;
        if word(&typed, 0x5e)? != zero
            || word(&init, 3)? != zero
            || call(&typed, 0x95d220, 0x4a)? != converter
            || converter != 0x9a1620
        {
            return Err("Slow numeric default initialization differs".into());
        }
        let empty_data = f32::from_bits(
            crate::script_random::real::from_i32(i32::from(init[1] as i8))
                .map_err(|e| format!("Slow numeric initialization: {e}"))?,
        );
        Ok(Self {
            class,
            data_column: String::from_utf8(column[..5].to_vec())
                .map_err(|_| "Slow data column encoding")?,
            empty_data,
        })
    }

    pub fn requires_attack_target(&self, cell: &str) -> Result<bool, String> {
        let value = match cell.trim() {
            "" | "-" | "_" => self.empty_data,
            text => text
                .parse::<f32>()
                .map_err(|_| "Slow acquisition data is not numeric")?,
        };
        if !value.is_finite() {
            return Err("Slow acquisition data is not finite".into());
        }
        Ok(value == self.empty_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Memory = Vec<(u32, Vec<u8>)>;

    fn fixture() -> Memory {
        let mut memory = vec![];
        let mut put = |va, bytes: Vec<u8>| memory.push((va, bytes));
        put(0xf6b214, 0x21200_u32.to_le_bytes().to_vec());
        put(0xf6b650, 0x21400_u32.to_le_bytes().to_vec());
        let mut class = vec![0xb8];
        class.extend(u32::from_be_bytes(*b"Xx01").to_le_bytes());
        class.push(0xc3);
        put(0x21200, class);
        let mut gate = vec![0xcc; 0x3b];
        gate[..9].copy_from_slice(&[0xff, 0x77, 0x50, 0x8d, 0x45, 0x08, 0x8b, 0xcf, 0x50]);
        let relative = |b: &mut [u8], base: u32, at: usize, target: u32| {
            b[at] = 0xe8;
            b[at + 1..at + 5]
                .copy_from_slice(&target.wrapping_sub(base + at as u32 + 5).to_le_bytes());
        };
        relative(&mut gate, 0x2149c, 9, 0x22100);
        gate[14..21].copy_from_slice(&[0xf3, 0x0f, 0x10, 0x00, 0x0f, 0x2e, 0x05]);
        gate[21..25].copy_from_slice(&0x73300_u32.to_le_bytes());
        gate[25..33].copy_from_slice(&[0x9f, 0xf6, 0xc4, 0x44, 0x7a, 0x5a, 0x8b, 0xce]);
        relative(&mut gate, 0x2149c, 33, 0x67ea50);
        gate[38..51].copy_from_slice(&[
            0x85, 0xc0, 0x74, 0x70, 0x8b, 0x8e, 0xf4, 0x03, 0, 0, 0x83, 0xc1, 0x70,
        ]);
        put(0x2149c, gate);
        let mut ranked = vec![0xcc; 32];
        ranked[..3].copy_from_slice(&[0x6b, 0xce, 0x70]);
        ranked[20..23].copy_from_slice(&[0x8b, 0x49, 0x34]);
        put(0x22133, ranked);
        let mut fallback = vec![0x8b, 0x45, 0x08, 0x8b, 0x0d];
        fallback.extend(0x73300_u32.to_le_bytes());
        fallback.extend([0x5f, 0x5e, 0x89, 0x08, 0x5d, 0xc2]);
        put(0x22197, fallback);
        let mut load = vec![0xcc; 43];
        load[..2].copy_from_slice(&[0xff, 0x35]);
        load[2..6].copy_from_slice(&0x74400_u32.to_le_bytes());
        relative(&mut load, 0x6ad1e5, 32, 0x6b06a0);
        load[40..43].copy_from_slice(&[0x89, 0x47, 0x34]);
        put(0x6ad1e5, load);
        put(0x74400, 0x75500_u32.to_le_bytes().to_vec());
        put(0x75500, b"DataQ\0".to_vec());
        let mut typed = vec![0xcc; 98];
        typed[0x21..0x26].copy_from_slice(&[0x8a, 0x08, 0x80, 0xf9, b'F']);
        typed[0x3e..0x41].copy_from_slice(&[0x80, 0xf9, b'I']);
        relative(&mut typed, 0x95d220, 0x4a, 0x9a1620);
        typed[0x5d] = 0xa1;
        typed[0x5e..].copy_from_slice(&0x73300_u32.to_le_bytes());
        put(0x95d220, typed);
        let mut init = vec![0x6a, 7, 0x68];
        init.extend(0x73300_u32.to_le_bytes());
        init.resize(16, 0xcc);
        relative(&mut init, 0x401300, 7, 0x9a1620);
        init[12..].copy_from_slice(&[0x83, 0xc4, 0x08, 0xc3]);
        put(0x401300, init);
        memory
    }

    fn decode(memory: &Memory) -> Result<Rules, String> {
        Rules::read(&mut |va, n, _| {
            memory
                .iter()
                .find(|(base, b)| va >= *base && (va - base) as usize + n <= b.len())
                .map(|(base, b)| b[(va - base) as usize..][..n].to_vec())
                .ok_or_else(|| "Slow synthetic mapping".into())
        })
    }

    #[test]
    fn relocated_numeric_gate_and_ranked_column_follow_nonstock_source_operands() {
        let rules = decode(&fixture()).unwrap();
        assert_eq!(rules.class, *b"Xx01");
        assert_eq!(rules.data_column, "DataQ");
        assert_eq!(rules.empty_data, 7.);
    }

    #[test]
    fn inconsistent_ranked_column_default_and_attack_branch_refuse() {
        for (address, at) in [(0x22197, 5), (0x6ad1e5, 42), (0x2149c, 30), (0x401300, 3)] {
            let mut memory = fixture();
            memory.iter_mut().find(|(va, _)| *va == address).unwrap().1[at] ^= 1;
            assert!(decode(&memory).is_err(), "{address:x}+{at}");
        }
        let mut truncated = fixture();
        truncated.pop();
        assert!(decode(&truncated).is_err());
    }

    #[test]
    fn empty_acquisition_data_uses_the_bound_scalar_and_nonzero_is_not_idle_rejection() {
        let rules = Rules {
            class: *b"Xx01",
            data_column: "DataQ".into(),
            empty_data: 7.,
        };
        for cell in ["-", "_", "", "7.0"] {
            assert!(rules.requires_attack_target(cell).unwrap());
        }
        assert!(!rules.requires_attack_target("0").unwrap());
        assert!(rules.requires_attack_target("NaN").is_err());
        assert!(rules.requires_attack_target("malformed").is_err());
    }
}
