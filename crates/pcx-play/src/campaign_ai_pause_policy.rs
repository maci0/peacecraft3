//! Selected-client nested campaign-script pause and wake-dispatch contract.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub(crate) initial: i32,
    pub(crate) retry_seconds: f64,
}

fn word(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes
        .get(at..at + 4)
        .and_then(|v| v.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "PauseCompAI truncated operand".into())
}
fn shape(bytes: &[u8], at: usize, expected: &[u8]) -> Result<(), String> {
    if bytes.get(at..at + expected.len()) == Some(expected) {
        Ok(())
    } else {
        Err(format!(
            "PauseCompAI instruction relationship differs at {at}"
        ))
    }
}
fn relative(bytes: &[u8], base: u32, at: usize, opcode: u8) -> Result<u32, String> {
    shape(bytes, at, &[opcode])?;
    Ok(base
        .wrapping_add(at as u32 + 5)
        .wrapping_add(word(bytes, at + 1)?))
}
impl Rules {
    pub fn read(image: &[u8]) -> Result<Self, String> {
        if image.len() > (32 << 20) || sha256(image) != REVIEWED_IMAGE {
            return Err("PauseCompAI selected client identity differs".into());
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 4096,
                sections: 96,
            },
        )
        .map_err(|e| e.to_string())?;
        Self::decode(4096, |address, length, code| {
            pe.read_from(address, code)
                .map_err(|e| e.to_string())?
                .get(..length)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "PauseCompAI truncated window".into())
        })
    }
    fn decode(
        mut budget: usize,
        mut source: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut read = |address, length, code| -> Result<Vec<u8>, String> {
            budget = budget
                .checked_sub(length)
                .ok_or("PauseCompAI input work budget")?;
            let b = source(address, length, code)?;
            if b.len() != length {
                return Err("PauseCompAI truncated window".into());
            }
            Ok(b)
        };
        // Addresses identify reviewed code roles; all context fields come from operands.
        let registration = read(0x4a0fda, 20, true)?;
        for at in [0, 5, 10] {
            shape(&registration, at, &[0x68])?;
        }
        if relative(&registration, 0x4a0fda, 15, 0xe8)? != 0x8ccc80 {
            return Err("PauseCompAI registration dispatcher differs".into());
        }
        if read(word(&registration, 1)?, 13, false)? != b"(Hplayer;B)V\0" {
            return Err("PauseCompAI native signature differs".into());
        }
        if read(word(&registration, 6)?, 12, false)? != b"PauseCompAI\0" {
            return Err("PauseCompAI registration name differs".into());
        }
        let handler = word(&registration, 11)?;
        let entry = read(handler, 32, true)?;
        shape(&entry, 0, &[0x55, 0x8b, 0xec, 0xff, 0x75, 0x08, 0xe8])?;
        shape(&entry, 11, &[0x83, 0xc4, 0x04, 0x85, 0xc0, 0x74])?;
        shape(
            &entry,
            18,
            &[0x0f, 0xb6, 0x40, 0x30, 0x89, 0x45, 0x08, 0x5d],
        )?;
        let writer_address = relative(&entry, handler, 26, 0xe9)?;
        let writer = read(writer_address, 64, true)?;
        shape(
            &writer,
            0,
            &[0x55, 0x8b, 0xec, 0xff, 0x75, 0x08, 0x8b, 0x0d],
        )?;
        shape(&writer, 17, &[0x8d, 0x88])?;
        shape(
            &writer,
            42,
            &[0x83, 0x7d, 0x0c, 0x00, 0x74, 0x08, 0xff, 0x80],
        )?;
        let field = word(&writer, 50)?;
        shape(&writer, 54, &[0x5d, 0xc3, 0xff, 0x88])?;
        if word(&writer, 58)? != field {
            return Err("PauseCompAI increment/decrement fields differ".into());
        }
        shape(&writer, 62, &[0x5d, 0xc3])?;
        let init = read(0xab97d8, 10, true)?;
        shape(&init, 0, &[0xc7, 0x87])?;
        if word(&init, 2)? != field {
            return Err("PauseCompAI initializer field differs".into());
        }
        let initial = word(&init, 6)? as i32;
        let wake = read(0xab5d55, 49, true)?;
        if relative(&writer, writer_address, 33, 0xe8)? != relative(&wake, 0xab5d55, 0, 0xe8)? {
            return Err("PauseCompAI context resolver join differs".into());
        }
        shape(&wake, 5, &[0x83, 0xb8])?;
        if word(&wake, 7)? != field {
            return Err("PauseCompAI wake field differs".into());
        }
        shape(&wake, 11, &[0x00, 0x8b, 0xce, 0x74, 0x14, 0x68])?;
        let period = read(0x401460, 16, true)?;
        shape(&period, 0, &[0x6a])?;
        shape(&period, 2, &[0x68])?;
        shape(&period, 12, &[0x83, 0xc4, 0x08, 0xc3])?;
        if word(&period, 3)? != word(&wake, 17)?
            || relative(&period, 0x401460, 7, 0xe8)? != 0x9a1620
        {
            return Err("PauseCompAI retry period initializer join differs".into());
        }
        // A signed-byte integer converts exactly to binary32 through the shared integer-to-real helper.
        let retry_seconds = f64::from(period[1] as i8);
        if retry_seconds <= 0.0 {
            return Err("PauseCompAI retry period is not positive".into());
        }
        let retry = relative(&wake, 0xab5d55, 21, 0xe8)?;
        let run = relative(&wake, 0xab5d55, 36, 0xe8)?;
        let retry_body = read(retry, 28, true)?;
        shape(
            &retry_body,
            0,
            &[0x55, 0x8b, 0xec, 0x6a, 0x00, 0x6a, 0x00, 0x51, 0x68],
        )?;
        let event = read(0xab5d38, 12, true)?;
        shape(&event, 0, &[0x8b, 0x45, 0x08, 0x81, 0x78, 0x08])?;
        if word(&event, 6)? != word(&retry_body, 9)? {
            return Err("PauseCompAI retry does not preserve the thread wake event".into());
        }
        shape(&retry_body, 13, &[0xff, 0x75, 0x08, 0x83, 0xc1, 0x44, 0xe8])?;
        shape(&retry_body, 24, &[0x5d, 0xc2, 0x04, 0x00])?;
        let execution = read(
            run.checked_add(168)
                .ok_or("PauseCompAI VM window address overflow")?,
            38,
            true,
        )?;
        shape(&execution, 0, &[0x68])?;
        shape(&execution, 5, &[0x6a, 0x01, 0x6a, 0x01, 0x6a, 0x00, 0x68])?;
        if read(word(&execution, 12)?, 5, false)? != b"main\0" {
            return Err("PauseCompAI wake does not join VM main".into());
        }
        if initial != 0 {
            return Err("PauseCompAI unsupported initial pause state".into());
        }
        Ok(Self {
            initial,
            retry_seconds,
        })
    }
}

#[cfg(feature = "test-fixtures")]
pub fn fixture(retry_seconds: i8) -> Rules {
    assert!(retry_seconds > 0);
    Rules {
        initial: 0,
        retry_seconds: f64::from(retry_seconds),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn put(b: &mut [u8], at: usize, value: u32) {
        b[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn jump(b: &mut [u8], at: usize, base: u32, target: u32, opcode: u8) {
        b[at] = opcode;
        put(b, at + 1, target.wrapping_sub(base + at as u32 + 5));
    }
    fn fixture(relocation: u32, field: u32) -> BTreeMap<u32, Vec<u8>> {
        let entry = 0x20000 + relocation;
        let writer = 0x21000 + relocation;
        let run = 0x22000 + relocation;
        let retry = 0x23000 + relocation;
        let mut map = BTreeMap::new();
        let mut registration = vec![0; 20];
        for (at, value) in [
            (0, 0x18000 + relocation),
            (5, 0x18100 + relocation),
            (10, entry),
        ] {
            registration[at] = 0x68;
            put(&mut registration, at + 1, value);
        }
        jump(&mut registration, 15, 0x4a0fda, 0x8ccc80, 0xe8);
        map.insert(0x4a0fda, registration);
        map.insert(0x18000 + relocation, b"(Hplayer;B)V\0".to_vec());
        map.insert(0x18100 + relocation, b"PauseCompAI\0".to_vec());
        let mut b = vec![0; 32];
        b[..7].copy_from_slice(&[0x55, 0x8b, 0xec, 0xff, 0x75, 0x08, 0xe8]);
        b[11..17].copy_from_slice(&[0x83, 0xc4, 0x04, 0x85, 0xc0, 0x74]);
        b[18..26].copy_from_slice(&[0x0f, 0xb6, 0x40, 0x30, 0x89, 0x45, 0x08, 0x5d]);
        jump(&mut b, 26, entry, writer, 0xe9);
        map.insert(entry, b);
        let mut b = vec![0; 64];
        b[..8].copy_from_slice(&[0x55, 0x8b, 0xec, 0xff, 0x75, 0x08, 0x8b, 0x0d]);
        b[17..19].copy_from_slice(&[0x8d, 0x88]);
        b[42..50].copy_from_slice(&[0x83, 0x7d, 0x0c, 0x00, 0x74, 0x08, 0xff, 0x80]);
        put(&mut b, 50, field);
        b[54..58].copy_from_slice(&[0x5d, 0xc3, 0xff, 0x88]);
        put(&mut b, 58, field);
        b[62..64].copy_from_slice(&[0x5d, 0xc3]);
        jump(&mut b, 33, writer, 0x24000 + relocation, 0xe8);
        map.insert(writer, b);
        let mut b = vec![0; 10];
        b[..2].copy_from_slice(&[0xc7, 0x87]);
        put(&mut b, 2, field);
        map.insert(0xab97d8, b);
        let mut b = vec![0; 49];
        jump(&mut b, 0, 0xab5d55, 0x24000 + relocation, 0xe8);
        b[5..7].copy_from_slice(&[0x83, 0xb8]);
        put(&mut b, 7, field);
        b[11..17].copy_from_slice(&[0x00, 0x8b, 0xce, 0x74, 0x14, 0x68]);
        jump(&mut b, 21, 0xab5d55, retry, 0xe8);
        jump(&mut b, 36, 0xab5d55, run, 0xe8);
        put(&mut b, 17, 0x19000 + relocation);
        map.insert(0xab5d55, b);
        let mut b = vec![0; 16];
        b[..3].copy_from_slice(&[0x6a, 3, 0x68]);
        put(&mut b, 3, 0x19000 + relocation);
        jump(&mut b, 7, 0x401460, 0x9a1620, 0xe8);
        b[12..].copy_from_slice(&[0x83, 0xc4, 0x08, 0xc3]);
        map.insert(0x401460, b);
        let mut b = vec![0; 28];
        b[..9].copy_from_slice(&[0x55, 0x8b, 0xec, 0x6a, 0x00, 0x6a, 0x00, 0x51, 0x68]);
        put(&mut b, 9, 0x123);
        b[13..20].copy_from_slice(&[0xff, 0x75, 0x08, 0x83, 0xc1, 0x44, 0xe8]);
        b[24..28].copy_from_slice(&[0x5d, 0xc2, 0x04, 0x00]);
        map.insert(retry, b);
        let mut b = vec![0; 12];
        b[..6].copy_from_slice(&[0x8b, 0x45, 0x08, 0x81, 0x78, 0x08]);
        put(&mut b, 6, 0x123);
        map.insert(0xab5d38, b);
        let mut b = vec![0; 38];
        b[0] = 0x68;
        b[5..12].copy_from_slice(&[0x6a, 0x01, 0x6a, 0x01, 0x6a, 0x00, 0x68]);
        put(&mut b, 12, 0x18200 + relocation);
        map.insert(run + 168, b);
        map.insert(0x18200 + relocation, b"main\0".to_vec());
        map
    }
    fn decode(map: &BTreeMap<u32, Vec<u8>>, budget: usize) -> Result<Rules, String> {
        Rules::decode(budget, |address, length, _| {
            map.get(&address)
                .and_then(|b| b.get(..length))
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("missing synthetic role {address:x}"))
        })
    }
    #[test]
    fn relocated_targets_and_nonstock_context_field_preserve_join() {
        for relocation in [0, 0x10000] {
            let rules = decode(&fixture(relocation, 0x3a0), 4096).unwrap();
            assert_eq!(rules.initial, 0);
            assert_eq!(rules.retry_seconds, 3.0);
        }
    }
    #[test]
    fn changed_counter_field_wake_branch_initializer_and_budget_refuse() {
        let original = fixture(0, 0x3a0);
        for (address, offset) in [
            (0x21000, 58),
            (0xab5d55, 7),
            (0xab5d55, 15),
            (0xab97d8, 6),
            (0x22000 + 168, 11),
            (0x401460, 3),
            (0x401460, 7),
            (0xab5d38, 6),
        ] {
            let mut map = original.clone();
            map.get_mut(&address).unwrap()[offset] ^= 1;
            assert!(decode(&map, 4096).is_err(), "{address:x}:{offset}");
        }
        assert!(decode(&original, 32).unwrap_err().contains("budget"));
        let mut map = original;
        map.get_mut(&0x21000).unwrap().truncate(63);
        assert!(decode(&map, 4096).is_err());
    }
    #[test]
    #[ignore = "requires the local selected Legacy client image via PCX_CAMPAIGN_IMAGE"]
    fn selected_client_pause_writer_and_vm_wake_join() {
        let image = std::fs::read(std::env::var("PCX_CAMPAIGN_IMAGE").expect("PCX_CAMPAIGN_IMAGE"))
            .unwrap();
        let rules = Rules::read(&image).unwrap();
        assert_eq!(rules.initial, 0);
        assert!(rules.retry_seconds > 0.0);
    }
}
