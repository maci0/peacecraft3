//! Selected-client Mirror Image operands. Ranked gameplay cells remain in the
//! admitted ability tables; these rules contain no copied balance values.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub ability: [u8; 4],
    pub buff: [u8; 4],
    pub order: i32,
    pub image_limit: usize,
    pub random_channel: usize,
    pub circle_radians: f32,
}

fn word(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "Mirror Image truncated operand".into())
}
fn shape(b: &[u8], at: usize, expected: &[u8]) -> Result<(), String> {
    if b.get(at..at + expected.len()) == Some(expected) {
        Ok(())
    } else {
        Err(format!(
            "Mirror Image instruction relationship differs at {at}"
        ))
    }
}
fn call(b: &[u8], va: u32, at: usize) -> Result<u32, String> {
    shape(b, at, &[0xe8])?;
    Ok(va
        .wrapping_add(at as u32 + 5)
        .wrapping_add(word(b, at + 1)?))
}

impl Rules {
    pub fn with_random(
        self,
        policy: &crate::script_random::profile::Policy,
    ) -> Result<Self, String> {
        if policy.image_sha256() != &self.image_sha256
            || self.random_channel >= policy.secondary_count
        {
            return Err("Mirror Image random profile or channel differs".into());
        }
        Ok(self)
    }
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("Mirror Image selected client identity differs".into());
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
        Self::decode(origin, sha256(image), 4096, |va, len, code| {
            pe.read_from(va, code)
                .map_err(|e| format!("Mirror Image mapped {va:x}: {e}"))?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "Mirror Image truncated mapped window".into())
        })
    }

    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut budget: usize,
        mut source: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut read = |va, len, code| -> Result<Vec<u8>, String> {
            budget = budget
                .checked_sub(len)
                .ok_or("Mirror Image input work budget")?;
            let bytes = source(va, len, code)?;
            if bytes.len() != len {
                return Err("Mirror Image truncated window".into());
            }
            Ok(bytes)
        };
        // Fixed locations identify reviewed code roles, never gameplay operands.
        let registration = read(0xc2e4b0, 25, true)?;
        shape(&registration, 0, &[0x68])?;
        shape(&registration, 10, &[0x50, 0x68])?;
        shape(&registration, 21, &[0x83, 0xc4, 0x0c, 0xc3])?;
        if call(&registration, 0xc2e4b0, 5)? != 0xb4b180
            || call(&registration, 0xc2e4b0, 16)? != 0x7f6360
        {
            return Err("Mirror Image class registration differs".into());
        }
        let getter = read(0xc2c930, 6, true)?;
        shape(&getter, 0, &[0xb8])?;
        shape(&getter, 5, &[0xc3])?;
        let alias = word(&getter, 1)?;
        if alias != word(&registration, 12)? {
            return Err("Mirror Image class alias differs".into());
        }
        let name = read(0xc2c920, 6, true)?;
        shape(&name, 0, &[0xb8])?;
        shape(&name, 5, &[0xc3])?;
        if read(word(&name, 1)?, 20, false)? != b"CAbilityMirrorImage\0" {
            // The class string is NUL terminated; no balance data is inferred from its name.
            return Err("Mirror Image class name differs".into());
        }
        let ctor = read(0xc2d39e, 6, true)?;
        shape(&ctor, 0, &[0xc7, 0x06])?;
        let table = word(&ctor, 2)?;
        let vtable = read(table, 0x3cc, false)?;
        for (slot, role) in [
            (0x0c, 0xc2cf00),
            (0x1c, 0xc2c930),
            (0x58, 0xc2c920),
            (0x310, 0xc2cc60),
            (0x3c8, 0xc2ccc0),
        ] {
            if word(&vtable, slot)? != role {
                return Err("Mirror Image class vtable differs".into());
            }
        }
        let handler = read(0xc2cf06, 23, true)?;
        shape(&handler, 0, &[0x81, 0x78, 0x08])?;
        shape(&handler, 7, &[0x74, 0x09])?;
        if call(&handler, 0xc2cf06, 18)? != 0xc2e5a0 {
            return Err("Mirror Image finalize handler differs".into());
        }
        let scheduled = read(0xc2cebc, 30, true)?;
        if call(&scheduled, 0xc2cebc, 0)? != 0x46ef80 || call(&scheduled, 0xc2cebc, 25)? != 0x7fbe90
        {
            return Err("Mirror Image delay/scheduler differs".into());
        }
        shape(&scheduled, 10, &[0x68])?;
        if word(&scheduled, 11)? != word(&handler, 3)? {
            return Err("Mirror Image scheduled event differs".into());
        }
        let order = read(0xc2cc60, 6, true)?;
        shape(&order, 0, &[0xb8])?;
        shape(&order, 5, &[0xc3])?;
        let buff = read(0xc2c0c0, 6, true)?;
        shape(&buff, 0, &[0xb8])?;
        shape(&buff, 5, &[0xc3])?;
        let buff_registration = read(0xc2e4f0, 25, true)?;
        shape(&buff_registration, 11, &[0x68])?;
        if word(&buff_registration, 12)? != word(&buff, 1)? {
            return Err("Mirror Image buff alias differs".into());
        }
        let illusion_registration = read(0x49dc85, 20, true)?;
        for at in [0, 5, 10] {
            shape(&illusion_registration, at, &[0x68])?;
        }
        if call(&illusion_registration, 0x49dc85, 15)? != 0x8ccc80
            || read(
                word(&illusion_registration, 6)?,
                b"IsUnitIllusion\0".len(),
                false,
            )? != b"IsUnitIllusion\0"
            || read(
                word(&illusion_registration, 1)?,
                b"(Hunit;)B\0".len(),
                false,
            )? != b"(Hunit;)B\0"
        {
            return Err("Mirror Image illusion query registration differs".into());
        }
        let illusion_query = read(word(&illusion_registration, 11)?, 34, true)?;
        if call(&illusion_query, word(&illusion_registration, 11)?, 6)? != 0x483940 {
            return Err("Mirror Image illusion query resolver differs".into());
        }
        shape(&illusion_query, 20, &[0x8b, 0x80])?;
        shape(&illusion_query, 26, &[0xc1, 0xe8])?;
        shape(&illusion_query, 29, &[0x83, 0xe0, 1, 0x5d, 0xc3])?;
        let bit = illusion_query[28];
        let mask = 1u32
            .checked_shl(u32::from(bit))
            .ok_or("Mirror Image invalid illusion bit")?;
        let creation = read(0xc2dc67, 20, true)?;
        shape(&creation, 0, &[0x68])?;
        if call(&creation, 0xc2dc67, 15)? != 0x687ca0 {
            return Err("Mirror Image factory differs".into());
        }
        let mark = read(0x687eb9, 18, true)?;
        shape(&mark, 0, &[0xf7, 0xc3])?;
        shape(&mark, 6, &[0x74, 10, 0x81, 0x8e])?;
        if word(&mark, 2)? != word(&creation, 1)?
            || word(&mark, 10)? != word(&illusion_query, 22)?
            || word(&mark, 14)? != mask
        {
            return Err("Mirror Image factory/query identity differs".into());
        }
        let random = read(0xc2e673, 31, true)?;
        shape(&random, 0, &[0x68])?;
        shape(&random, 5, &[0x6a, 0x00, 0x41, 0x51, 0x6a, 0x01, 0x6a])?;
        shape(&random, 13, &[0x50])?;
        if call(&random, 0xc2e673, 14)? != 0x69c5a0 || call(&random, 0xc2e673, 20)? != 0x9a07b0 {
            return Err("Mirror Image random die/conversion differs".into());
        }
        // The shared real-one object lives in zero-filled storage. Follow its
        // initializer instead of treating an unmapped BSS address as file data.
        let scale_initializer = read(0x401460, 16, true)?;
        shape(&scale_initializer, 0, &[0x6a])?;
        shape(&scale_initializer, 2, &[0x68])?;
        shape(&scale_initializer, 12, &[0x83, 0xc4, 0x08, 0xc3])?;
        if word(&scale_initializer, 3)? != word(&random, 1)?
            || call(&scale_initializer, 0x401460, 7)? != 0x9a1620
            || scale_initializer[1] != 1
        {
            return Err("Mirror Image non-unit die scale unsupported".into());
        }
        let count = read(0xc2e6e6, 19, true)?;
        shape(&count, 0, &[0xb8])?;
        shape(&count, 5, &[0x3b, 0xf0])?;
        shape(&count, 16, &[0x0f, 0x4f, 0xf0])?;
        let image_limit =
            usize::try_from(word(&count, 1)?).map_err(|_| "Mirror Image image limit")?;
        if image_limit == 0 || image_limit > i32::MAX as usize {
            return Err("Mirror Image invalid image limit".into());
        }
        let circle = read(0xc2e266, 43, true)?;
        shape(&circle, 0, &[0x68])?;
        if call(&circle, 0xc2e266, 25)? != 0x99ffe0 || call(&circle, 0xc2e266, 38)? != 0x9a02d0 {
            return Err("Mirror Image circular placement arithmetic differs".into());
        }
        let circle_radians = f32::from_bits(word(&read(word(&circle, 1)?, 4, false)?, 0)?);
        if !circle_radians.is_finite() || circle_radians <= 0.0 {
            return Err("Mirror Image invalid circle operand".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            ability: alias.to_be_bytes(),
            buff: word(&buff, 1)?.to_be_bytes(),
            order: word(&order, 1)? as i32,
            image_limit,
            random_channel: usize::from(random[12]),
            circle_radians,
        })
    }
}

#[cfg(test)]
#[path = "mirror_image_rules_tests.rs"]
mod tests;
