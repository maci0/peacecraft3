//! Selected Slow acquisition's ranked numeric gate; positive acquisition remains separate.
use super::{shape, word};

#[derive(Clone, Debug)]
pub struct Rules {
    pub class: [u8; 4],
    pub data_column: String,
    pub empty_data: f32,
    pub attack_victim_alliance: i32,
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
        let attack_victim_alliance = Self::victim_alliance(read, candidate)?;
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
            attack_victim_alliance,
        })
    }

    fn victim_alliance(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        candidate: u32,
    ) -> Result<i32, String> {
        shape(&read(candidate + 0x25, 2, true)?, 0, &[0x8b, 0xf9])?;
        shape(&read(candidate + 0x3b, 3, true)?, 0, &[0x8b, 0x5f, 0x30])?;
        let relation = read(candidate + 0xf9, 16, true)?;
        shape(
            &relation,
            0,
            &[0x8b, 0x01, 0xff, 0x90, 0xec, 0, 0, 0, 0x50, 0x8b, 0xcb],
        )?;
        let helper = call(&relation, candidate + 0xf9, 11)?;
        let owner = read(helper, 25, true)?;
        shape(&owner, 3, &[0xff, 0x75, 0x08, 0xff, 0xb1, 0x60, 0x01, 0, 0])?;
        let mask = call(&owner, helper, 18)?;
        let mask_code = read(mask + 0x36, 0x13, true)?;
        let player_data = call(&mask_code, mask + 0x36, 0)?;
        shape(&mask_code, 11, &[0x8d, 0x48])?;
        let mask_getter = call(&mask_code, mask + 0x36, 14)?;
        let indexed = read(0x494510, 13, true)?;
        shape(&indexed, 0, &[0xc1, 0xe6])?;
        shape(&indexed, 3, &[0x8d, 0x4e])?;
        shape(&indexed, 6, &[0x03, 0xc8])?;
        let writer = read(0x4c2027, 15, true)?;
        shape(&writer, 0, &[0x8b, 0xc3, 0xc1, 0xe0])?;
        shape(&writer, 5, &[0x03, 0xf0, 0x8d, 0x4e])?;
        let getter_call = read(0x4944ea, 5, true)?;
        let enable = read(0x4c2036, 12, true)?;
        shape(&enable, 0, &[0x0b, 0x45, 0x08, 0x8d, 0x4e])?;
        shape(&enable, 6, &[0x50])?;
        if call(&getter_call, 0x4944ea, 0)? != player_data
            || call(&indexed, 0x494510, 8)? != mask_getter
            || call(&writer, 0x4c2027, 10)? != mask_getter
            || indexed[2] != writer[4]
            || indexed[5] != writer[9]
            || indexed[5] != enable[5]
            || call(&enable, 0x4c2036, 7)? != 0x7fb380
        {
            return Err("Slow victim alliance layout differs".into());
        }
        let shift = indexed[2];
        if mask_code[13] > i8::MAX as u8 || indexed[5] > i8::MAX as u8 {
            return Err("Slow victim alliance field displacement is negative".into());
        }
        let delta = mask_code[13]
            .checked_sub(indexed[5])
            .ok_or("Slow victim alliance field precedes the indexed array")?;
        let stride = 1_u32
            .checked_shl(u32::from(shift))
            .ok_or("Slow victim alliance stride is invalid")?;
        if u32::from(delta) % stride != 0 {
            return Err("Slow victim alliance field is unaligned".into());
        }
        let kind = u32::from(delta) / stride;
        let startup = read(0x47ce7d, 10, true)?;
        shape(&startup, 0, &[0x53, 0x6a])?;
        shape(&startup, 3, &[0x8b, 0xce])?;
        let row = read(0x47ce41, 8, true)?;
        shape(&row, 0, &[0x8b, 0x0e, 0x53])?;
        let player = read(0x47ce60, 9, true)?;
        shape(&player, 0, &[0x8b, 0x0e])?;
        shape(&player, 7, &[0x8b, 0xf0])?;
        if u32::from(startup[2]) != kind
            || call(&startup, 0x47ce7d, 5)? != 0x4c1f70
            || call(&row, 0x47ce41, 3)? != 0x4c0c00
            || call(&player, 0x47ce60, 2)? != player_data
        {
            return Err("Slow victim self-alliance initialization differs".into());
        }
        Ok(kind as i32)
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
        put(0x21425, vec![0x8b, 0xf9]);
        put(0x2143b, vec![0x8b, 0x5f, 0x30]);
        let mut relation = vec![0x8b, 0x01, 0xff, 0x90, 0xec, 0, 0, 0, 0x50, 0x8b, 0xcb];
        relation.resize(16, 0xcc);
        relative(&mut relation, 0x214f9, 11, 0x81200);
        put(0x214f9, relation);
        let mut owner = vec![0xcc; 25];
        owner[3..12].copy_from_slice(&[0xff, 0x75, 0x08, 0xff, 0xb1, 0x60, 1, 0, 0]);
        relative(&mut owner, 0x81200, 18, 0x81500);
        put(0x81200, owner);
        let mut mask = vec![0xcc; 19];
        relative(&mut mask, 0x81536, 0, 0x4b5bb0);
        mask[11..14].copy_from_slice(&[0x8d, 0x48, 0x78]);
        relative(&mut mask, 0x81536, 14, 0x7fb2b0);
        put(0x81536, mask);
        let mut indexed = vec![0xc1, 0xe6, 5, 0x8d, 0x4e, 0x18, 0x03, 0xc8];
        indexed.resize(13, 0xcc);
        relative(&mut indexed, 0x494510, 8, 0x7fb2b0);
        put(0x494510, indexed);
        let mut writer = vec![0x8b, 0xc3, 0xc1, 0xe0, 5, 0x03, 0xf0, 0x8d, 0x4e, 0x18];
        writer.resize(15, 0xcc);
        relative(&mut writer, 0x4c2027, 10, 0x7fb2b0);
        put(0x4c2027, writer);
        let mut enable = vec![0x0b, 0x45, 0x08, 0x8d, 0x4e, 0x18, 0x50];
        enable.resize(12, 0xcc);
        relative(&mut enable, 0x4c2036, 7, 0x7fb380);
        put(0x4c2036, enable);
        let mut getter = vec![0xcc; 5];
        relative(&mut getter, 0x4944ea, 0, 0x4b5bb0);
        put(0x4944ea, getter);
        let mut startup = vec![0x53, 0x6a, 3, 0x8b, 0xce];
        startup.resize(10, 0xcc);
        relative(&mut startup, 0x47ce7d, 5, 0x4c1f70);
        put(0x47ce7d, startup);
        let mut row = vec![0x8b, 0x0e, 0x53];
        row.resize(8, 0xcc);
        relative(&mut row, 0x47ce41, 3, 0x4c0c00);
        put(0x47ce41, row);
        let mut player = vec![0x8b, 0x0e, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0x8b, 0xf0];
        relative(&mut player, 0x47ce60, 2, 0x4b5bb0);
        put(0x47ce60, player);
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
        assert_eq!(rules.attack_victim_alliance, 3);
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
    fn victim_relation_rejects_receiver_layout_and_self_initializer_drift() {
        for (address, at) in [
            (0x214f9, 10),
            (0x81200, 9),
            (0x81536, 13),
            (0x494510, 2),
            (0x4c2027, 9),
            (0x4c2036, 0),
            (0x47ce7d, 2),
        ] {
            let mut memory = fixture();
            memory.iter_mut().find(|(va, _)| *va == address).unwrap().1[at] ^= 1;
            assert!(decode(&memory).is_err(), "{address:x}+{at}");
        }
    }

    #[test]
    fn empty_acquisition_data_uses_the_bound_scalar_and_nonzero_is_not_idle_rejection() {
        let rules = Rules {
            class: *b"Xx01",
            data_column: "DataQ".into(),
            empty_data: 7.,
            attack_victim_alliance: 3,
        };
        for cell in ["-", "_", "", "7.0"] {
            assert!(rules.requires_attack_target(cell).unwrap());
        }
        assert!(!rules.requires_attack_target("0").unwrap());
        assert!(rules.requires_attack_target("NaN").is_err());
        assert!(rules.requires_attack_target("malformed").is_err());
    }
}
