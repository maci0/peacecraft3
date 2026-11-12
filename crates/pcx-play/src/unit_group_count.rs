//! Source-bound insertion quota shared by the native group enumerators.
use crate::neutral_rule_input::Pe;

pub(super) const INPUT_BYTES: usize = 5 * 2 + 7 + 12 + 7;

#[derive(Clone, Debug)]
pub(crate) struct Rules {
    initial: u32,
    default: i32,
}

#[derive(Clone, Copy)]
pub(crate) struct Cursor {
    inserted: u32,
    limit: u32,
}

impl Rules {
    pub(super) fn read(pe: &Pe<'_>, budget: usize) -> Result<Self, String> {
        if budget < INPUT_BYTES {
            return Err("group count source budget exceeded".into());
        }
        Self::decode(|va, len| {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("group count truncated at {va:#x}"))
        })
    }

    fn decode(read: impl Fn(u32, usize) -> Result<Vec<u8>, String>) -> Result<Self, String> {
        let mut default = None;
        for va in [0x4982d1, 0x498184, 0x498022, 0x4980c0, 0x498269] {
            let code = read(va, 2)?;
            let [0x6a, operand] = code.as_slice() else {
                return Err(format!(
                    "group count default instruction changed at {va:#x}"
                ));
            };
            let operand = i32::from(*operand as i8);
            if default.is_some_and(|value| value != operand) {
                return Err("group count defaults differ".into());
            }
            default = Some(operand);
        }
        let reset = read(0x50337b, 7)?;
        if reset.len() != 7 || reset[..3] != [0xc7, 0x46, 0x4c] {
            return Err("group count reset instruction changed".into());
        }
        // TEST/JE bypasses zero; CMP/JAE compares the insertion count as unsigned.
        if read(0x50292a, 12)?
            != [
                0x8b, 0x46, 0x48, 0x85, 0xc0, 0x74, 0x05, 0x39, 0x46, 0x4c, 0x73, 0xe0,
            ]
            || read(0x5029ac, 7)? != [0x85, 0xc0, 0x75, 0x03, 0xff, 0x46, 0x4c]
        {
            return Err("group count comparison/insertion instructions changed".into());
        }
        Ok(Self {
            initial: u32::from_le_bytes(reset[3..].try_into().unwrap()),
            default: default.ok_or("group count default missing")?,
        })
    }

    pub(crate) fn cursor(&self, limit: Option<i32>) -> Cursor {
        Cursor {
            inserted: self.initial,
            limit: limit.unwrap_or(self.default) as u32,
        }
    }
}

impl Cursor {
    pub(crate) fn full(self) -> bool {
        self.limit != 0 && self.inserted >= self.limit
    }
    pub(crate) fn insert(&mut self) {
        self.inserted = self.inserted.wrapping_add(1);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn rules() -> Rules {
        fixture(0, -1).unwrap()
    }
    fn fixture(initial: u32, default: i8) -> Result<Rules, String> {
        Rules::decode(|va, _| {
            Ok(match va {
                0x50337b => [vec![0xc7, 0x46, 0x4c], initial.to_le_bytes().to_vec()].concat(),
                0x50292a => vec![
                    0x8b, 0x46, 0x48, 0x85, 0xc0, 0x74, 0x05, 0x39, 0x46, 0x4c, 0x73, 0xe0,
                ],
                0x5029ac => vec![0x85, 0xc0, 0x75, 0x03, 0xff, 0x46, 0x4c],
                _ => vec![0x6a, default as u8],
            })
        })
    }
    #[test]
    fn nonstock_group_count_operands_unsigned_zero_and_decode_refusals() {
        let rules = fixture(2, 4).unwrap();
        let mut count = rules.cursor(None);
        assert!(!count.full());
        count.insert();
        assert!(!count.full());
        count.insert();
        assert!(count.full());
        assert!(rules.cursor(Some(1)).full());
        assert!(!rules.cursor(Some(0)).full());
        assert!(!rules.cursor(Some(-1)).full());
        assert!(!rules.cursor(Some(i32::MIN)).full());
        assert!(fixture(u32::MAX, -1).unwrap().cursor(None).full());
        assert!(Rules::decode(|_, n| Ok(vec![0; n])).is_err());
        assert!(Rules::decode(|_, _| Ok(Vec::new())).is_err());
    }
}
