//! First-captain assault operands and ordinary-member readiness.
//! Special target-category flags and exact native target ranking remain unqualified.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    script_numeric::NumericPolicy,
};
use pcx_script::{RealOps, Value, ast::BinOp};

#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: Sha256Digest,
    pub full_mask: u32,
    pub initial_member_count: i32,
    pub casualty_weight: i32,
    pub readiness_bias: f32,
    pub percentage: f32,
    pub hero_first: u8,
    pub hero_last: u8,
    pub gold_mine: [u8; 4],
    /// Ordinary nonstructure/structure priority tiers, requested owner first.
    pub owner_tiers: [[i32; 2]; 2],
    /// Native special-category/explicit mine tiers; no invented canonical flag mapping.
    pub special_tiers: [i32; 2],
}

fn word(bytes: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or("assault operand bounds")?
            .try_into()
            .map_err(|_| "assault operand width")?,
    ))
}
fn shape(bytes: &[u8], at: usize, expected: &[u8]) -> Result<(), String> {
    if bytes.get(at..at + expected.len()) == Some(expected) {
        Ok(())
    } else {
        Err(format!(
            "assault instruction relationship differs at window offset {at}: expected {expected:02x?}, found {:02x?}",
            bytes.get(at..at + expected.len())
        ))
    }
}
fn call(bytes: &[u8], base: u32, at: usize) -> Result<u32, String> {
    shape(bytes, at, &[0xe8])?;
    Ok(base
        .wrapping_add(at as u32 + 5)
        .wrapping_add(word(bytes, at + 1)?))
}

impl Rules {
    pub fn read(
        image: &[u8],
        numeric: &NumericPolicy,
        budget: RuleInputBudget,
    ) -> Result<Self, String> {
        if image.len() > budget.image_bytes
            || sha256(image) != REVIEWED_IMAGE
            || numeric.image_sha256() != &REVIEWED_IMAGE
        {
            return Err("assault client/numeric identity or image budget differs".into());
        }
        let pe = Pe::parse(image, budget).map_err(|error| error.to_string())?;
        Self::decode(REVIEWED_IMAGE, numeric, 4096, |address, length, code| {
            pe.read_from(address, code)
                .map_err(|e| e.to_string())?
                .get(..length)
                .map(<[u8]>::to_vec)
                .ok_or("truncated assault role".into())
        })
    }

    fn decode(
        image_sha256: Sha256Digest,
        numeric: &NumericPolicy,
        mut remaining: usize,
        mut source: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut read = |address, length, code| {
            remaining = remaining.checked_sub(length).ok_or("assault work budget")?;
            let bytes = source(address, length, code)?;
            if bytes.len() != length {
                return Err("truncated assault role".into());
            }
            Ok::<_, String>(bytes)
        };
        Self::validate_strategic_fog_bypass(&mut read)?;
        // Instruction-window addresses identify roles in the reviewed PE, never rule values.
        let first = read(0xabe270, 27, true)?;
        shape(&first, 0, &[0x8b, 0x81])?;
        shape(&first, 6, &[0x81, 0xc1])?;
        let first_slot = word(&first, 8)?;
        if word(&first, 2)? != first_slot + 4 {
            return Err("assault first-reference pair differs".into());
        }
        let init = read(0xabf930, 36, true)?;
        if word(&init, 2)? != first_slot + 4 || word(&init, 8)? != first_slot {
            return Err("InitAssault first captain differs".into());
        }
        shape(&init, 28, &[0x83, 0x88])?;
        let full_field = word(&init, 30)?;
        let full_mask = u32::from(init[34]);
        if full_mask.count_ones() != 1 || init[35] != 0xc3 {
            return Err("assault full flag shape".into());
        }
        let failure = read(0xabb238, 18, true)?;
        shape(&failure, 0, &[0x83, 0xa3])?;
        if word(&failure, 2)? != full_field
            || u32::from(failure[6] as i8 as u8) != (!full_mask & 0xff)
        {
            return Err("recruitment failure/full flag differ".into());
        }
        let query = read(0xab3fc0, 15, true)?;
        shape(&query, 5, &[0x8b, 0x80])?;
        if word(&query, 7)? != full_field
            || query.get(11..15) != Some(&[0x83, 0xe0, full_mask as u8, 0xc3])
        {
            return Err("CaptainIsFull flag differs".into());
        }
        let counters = read(0xac9b30, 22, true)?;
        shape(&counters, 0, &[0x01, 0xb3])?;
        shape(&counters, 6, &[0x85, 0xf6, 0x7e, 0x0c, 0x8b, 0x83])?;
        shape(&counters, 16, &[0x89, 0x83])?;
        let current = word(&counters, 2)?;
        let baseline = word(&counters, 18)?;
        if word(&counters, 12)? != current {
            return Err("assault member counter differs".into());
        }
        let size = read(0xab3e1f, 7, true)?;
        shape(&size, 0, &[0x8b, 0x80])?;
        if word(&size, 2)? != current {
            return Err("assault membership/query count differs".into());
        }
        let zero = read(0xad5a4c, 20, true)?;
        for at in [0, 10] {
            shape(&zero, at, &[0xc7, 0x83])?;
        }
        if word(&zero, 2)? != baseline
            || word(&zero, 12)? != current
            || word(&zero, 6)? != word(&zero, 16)?
        {
            return Err("assault initial counts differ".into());
        }
        let deficit = read(0xad2831, 42, true)?;
        shape(&deficit, 0, &[0x8b, 0x86])?;
        shape(&deficit, 6, &[0x2b, 0x86])?;
        shape(&deficit, 12, &[0x69, 0xc0])?;
        if word(&deficit, 2)? != baseline || word(&deficit, 8)? != current {
            return Err("readiness casualty counters differ".into());
        }
        let casualty_weight = word(&deficit, 14)? as i32;
        if casualty_weight < 0 {
            return Err("negative readiness casualty weight".into());
        }
        let one = read(0x401460, 16, true)?;
        let percent = read(0x401350, 16, true)?;
        let (one_va, readiness_bias) = numeric
            .integer_initializer(&one, 0x401460)
            .ok_or("readiness unity initializer")?;
        let (percent_va, percentage) = numeric
            .integer_initializer(&percent, 0x401350)
            .ok_or("readiness percentage initializer")?;
        let initial = read(0xad281a, 23, true)?;
        shape(&initial, 0, &[0xa1])?;
        if word(&initial, 1)? != one_va || readiness_bias <= 0. || percentage <= 0. {
            return Err("readiness initial scalar differs".into());
        }
        let hero = read(0xad28d0, 18, true)?;
        shape(&hero, 0, &[0x8b, 0x4f, 0x34, 0xc1, 0xe9, 0x18, 0x83, 0xe9])?;
        shape(&hero, 9, &[0x83, 0xf9])?;
        let hero_first = hero[8];
        let hero_last = hero_first
            .checked_add(hero[11])
            .ok_or("readiness hero range overflow")?;
        let ratio = read(0xad2a6f, 112, true)?;
        for at in [4, 42, 80] {
            shape(&ratio, at, &[0x68])?;
            if word(&ratio, at + 1)? != percent_va {
                return Err("readiness percentage references differ".into());
            }
        }
        let ordinary = read(0xad2987, 68, true)?;
        shape(&ordinary, 8, &[0xff, 0x90])?;
        shape(&ordinary, 44, &[0xff, 0x90])?;
        if word(&ordinary, 10)? != 0x12c || word(&ordinary, 46)? != 0x130 {
            return Err("readiness current/max life ABI differs".into());
        }
        let finish = read(0xad2ae1, 30, true)?;
        shape(&finish, 0, &[0x83, 0x7d, 0x08, 0x00, 0x74, 0x4c])?;
        shape(&finish, 18, &[0x0f, 0x47, 0x45, 0xd0])?;
        let requested_owner = read(0xad0857, 112, true)?;
        shape(&requested_owner, 4, &[0x8b, 0xbf])?;
        shape(
            &requested_owner,
            10,
            &[0xff, 0x90, 0xec, 0x00, 0x00, 0x00, 0x3b, 0xc7],
        )?;
        let gold_mine = word(&requested_owner, 32)?.to_be_bytes();
        if word(&requested_owner, 78)? != u32::from_be_bytes(gold_mine) {
            return Err("assault special alias differs".into());
        }
        shape(&requested_owner, 47, &[0xf7, 0xd8, 0x1b, 0xc0, 0x83, 0xc0])?;
        shape(&requested_owner, 93, &[0xf7, 0xd8, 0x1b, 0xc0, 0x83, 0xc0])?;
        shape(&requested_owner, 59, &[0x33, 0xc0])?;
        shape(&requested_owner, 105, &[0xc7, 0x45, 0xec])?;
        let tiers = [
            i32::from(requested_owner[53] as i8),
            i32::from(requested_owner[99] as i8),
        ];
        let special_tiers = [0, word(&requested_owner, 108)? as i32];
        let launch = read(0xac40f0, 77, true)?;
        if word(&launch, 5)? != first_slot + 4 || word(&launch, 11)? != first_slot {
            return Err("launch first captain differs".into());
        }
        shape(&launch, 37, &[0xf6, 0x80])?;
        if word(&launch, 39)? != full_field || u32::from(launch[43]) != full_mask {
            return Err("launch fullness gate differs".into());
        }
        shape(&launch, 62, &[0xb8, 1, 0, 0, 0, 0x5d, 0xc2, 0x10, 0])?;
        // Both metadata predicates are reused by canonical structure admission, not reinterpreted flags.
        if call(&requested_owner, 0xad0857, 42)? != 0x696b50
            || call(&requested_owner, 0xad0857, 88)? != 0x696b50
        {
            return Err("assault structure classification helper differs".into());
        }
        Ok(Self {
            image_sha256,
            full_mask,
            initial_member_count: word(&zero, 6)? as i32,
            casualty_weight,
            readiness_bias,
            percentage,
            hero_first,
            hero_last,
            gold_mine,
            owner_tiers: [[tiers[0], tiers[0] - 1], [tiers[1], tiers[1] - 1]],
            special_tiers,
        })
    }

    fn validate_strategic_fog_bypass(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<(), String> {
        let consumer = read(0xad07ce, 20, true)?;
        shape(&consumer, 0, &[0x8b, 0x06, 0x8b, 0xce, 0x6a])?;
        shape(&consumer, 6, &[0x6a])?;
        shape(&consumer, 8, &[0xff, 0xb7, 0x70, 0x01, 0x00, 0x00])?;
        // Independently reviewed CUnit virtual ABI, shared with the vital getters.
        shape(&consumer, 14, &[0xff, 0x90, 0xfc, 0x00, 0x00, 0x00])?;
        let mode = consumer[7] as i8 as i32 as u32;
        let slot = read(0xe9c600, 4, false)?;
        let wrapper_address = word(&slot, 0)?;
        let wrapper = read(wrapper_address, 83, true)?;
        // The forwarded mode may OR additional source bits, never clear its input.
        shape(
            &wrapper,
            63,
            &[
                0x0b, 0x45, 0x0c, 0x8b, 0xce, 0x6a, 0x00, 0xff, 0x75, 0x10, 0x50, 0x57, 0xff, 0x75,
                0x08,
            ],
        )?;
        let query_address = call(&wrapper, wrapper_address, 78)?;
        let query = read(query_address, 228, true)?;
        shape(&query, 0x93, &[0xb8])?;
        let bypass_mask = word(&query, 0x94)?;
        shape(&query, 0x98, &[0x84, 0xd8, 0x75])?;
        let skip = (0x9c_i32)
            .checked_add(i32::from(query[0x9b] as i8))
            .and_then(|at| usize::try_from(at).ok())
            .ok_or("assault fog branch bounds")?;
        // Non-bypass path obtains the unit position then consults its fog cell.
        shape(
            &query,
            0x9c,
            &[0x8d, 0x45, 0xf8, 0xc7, 0x45, 0xf8, 0, 0, 0, 0, 0x50],
        )?;
        shape(&query, 0xb2, &[0xff, 0x90, 0xb8, 0, 0, 0])?;
        shape(&query, 0xd6, &[0xe8])?;
        shape(
            &query,
            skip,
            &[0x5b, 0x5f, 0x5e, 0x8b, 0xe5, 0x5d, 0xc2, 0x14, 0],
        )?;
        if bypass_mask.count_ones() != 1
            || bypass_mask > u32::from(u8::MAX)
            || mode & bypass_mask == 0
        {
            return Err("ordinary assault geometric-fog bypass mode is unsupported".into());
        }
        Ok(())
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture(image_sha256: Sha256Digest) -> Self {
        Self {
            image_sha256,
            full_mask: 4,
            initial_member_count: 0,
            casualty_weight: 37,
            readiness_bias: 1.,
            percentage: 80.,
            hero_first: b'X',
            hero_last: b'Z',
            gold_mine: *b"q123",
            owner_tiers: [[3, 2], [9, 8]],
            special_tiers: [0, 7],
        }
    }

    /// Source ordinary path only. Hero members require the separate weighted aggregate.
    pub fn ordinary_readiness(
        &self,
        numeric: &NumericPolicy,
        baseline: i32,
        members: &[([u8; 4], f32, f32)],
    ) -> Result<i32, String> {
        if numeric.image_sha256() != &self.image_sha256 {
            return Err("readiness numeric identity differs".into());
        }
        let count = i32::try_from(members.len()).map_err(|_| "readiness member count width")?;
        if baseline < count || baseline < 0 {
            return Err("readiness baseline/current invariant".into());
        }
        let math = |op, a, b| -> Result<f32, String> {
            match numeric.binary(op, a, b)? {
                Value::Real(value) if value.is_finite() => Ok(value),
                _ => Err("readiness arithmetic is not finite real".into()),
            }
        };
        let penalty = baseline
            .wrapping_sub(count)
            .wrapping_mul(self.casualty_weight);
        let mut maximum = math(BinOp::Add, self.readiness_bias, numeric.integer(penalty)?)?;
        let mut life = self.readiness_bias;
        for &(code, current, max) in members {
            if (self.hero_first..=self.hero_last).contains(&code[0]) {
                return Err("hero captain readiness is not admitted".into());
            }
            if !current.is_finite() || !max.is_finite() || current < 0. || max <= 0. {
                return Err("readiness member vitals".into());
            }
            life = math(BinOp::Add, life, current)?;
            maximum = math(BinOp::Add, maximum, max)?;
        }
        let percentage = math(
            BinOp::Div,
            math(BinOp::Mul, life, self.percentage)?,
            maximum,
        )?
        .min(self.percentage);
        if percentage < 0. || f64::from(percentage) > f64::from(i32::MAX) {
            return Err("readiness integer class".into());
        }
        Ok(percentage.trunc() as i32)
    }
}

#[cfg(test)]
#[path = "campaign_assault_policy_tests.rs"]
mod tests;
