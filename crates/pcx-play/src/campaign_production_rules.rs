//! Bounded selected-client SetProduce quantity and sequential acceptance contract.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: Sha256Digest,
    pub maximum_quantity: i32,
}

impl Rules {
    pub fn read(image: &[u8], budget: RuleInputBudget) -> Result<Self, String> {
        if image.len() > budget.image_bytes || sha256(image) != REVIEWED_IMAGE {
            return Err("SetProduce client identity or image budget differs".into());
        }
        let pe = Pe::parse(image, budget).map_err(|e| e.to_string())?;
        Self::decode(REVIEWED_IMAGE, 512, |address, length, code| {
            pe.read_from(address, code)
                .map_err(|e| e.to_string())?
                .get(..length)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "truncated SetProduce source window".into())
        })
    }

    fn decode(
        image_sha256: Sha256Digest,
        mut remaining: usize,
        mut source: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut read = |address, length, code| {
            remaining = remaining
                .checked_sub(length)
                .ok_or("SetProduce work budget")?;
            let bytes = source(address, length, code)?;
            if bytes.len() != length {
                return Err("truncated SetProduce source window".into());
            }
            Ok::<_, String>(bytes)
        };
        fn word(bytes: &[u8], offset: usize) -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                bytes
                    .get(offset..offset + 4)
                    .ok_or("SetProduce operand bounds")?
                    .try_into()
                    .map_err(|_| "SetProduce operand width")?,
            ))
        }
        fn at(bytes: &[u8], offset: usize, expected: &[u8]) -> Result<(), String> {
            if bytes.get(offset..offset + expected.len()) != Some(expected) {
                return Err("SetProduce instruction relationship differs".into());
            }
            Ok(())
        }
        // The registration is an address identity in the pinned PE, not a game-rule value.
        let registration = read(0xab6352, 20, true)?;
        for offset in [0, 5, 10] {
            at(&registration, offset, &[0x68])?;
        }
        at(&registration, 15, &[0xe8])?;
        if read(word(&registration, 1)?, 7, false)? != b"(III)B\0"
            || read(word(&registration, 6)?, 11, false)? != b"SetProduce\0"
        {
            return Err("SetProduce registration differs".into());
        }
        let handler = read(word(&registration, 11)?, 133, true)?;
        at(&handler, 0, &[0x55, 0x8b, 0xec, 0x8b, 0x0d])?;
        at(
            &handler,
            28,
            &[0x8b, 0x75, 0x08, 0x8d, 0x46, 0xff, 0x83, 0xf8],
        )?;
        at(&handler, 37, &[0x76, 0x0f])?;
        at(&handler, 88, &[0x57, 0x6a, 0x00, 0x53, 0x8b, 0xc8, 0xe8])?;
        at(
            &handler,
            99,
            &[
                0x85, 0xc0, 0x74, 0x0d, 0x83, 0xee, 0x01, 0x74, 0x0f, 0x8b, 0x0d,
            ],
        )?;
        at(
            &handler,
            114,
            &[0xeb, 0xce, 0x5f, 0x5b, 0x33, 0xc0, 0x5e, 0x5d, 0xc3],
        )?;
        at(
            &handler,
            123,
            &[0x5f, 0x5b, 0xb8, 0x01, 0x00, 0x00, 0x00, 0x5e, 0x5d, 0xc3],
        )?;
        if word(&handler, 5)? != word(&handler, 110)? || handler[36] & 0x80 != 0 {
            return Err("SetProduce context or quantity operand differs".into());
        }
        Ok(Self {
            image_sha256,
            maximum_quantity: i32::from(handler[36]) + 1,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture(image_sha256: Sha256Digest, maximum_quantity: i32) -> Self {
        Self {
            image_sha256,
            maximum_quantity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn fixture(handler_address: u32, quantity_minus_one: u8) -> BTreeMap<u32, Vec<u8>> {
        let mut registration = Vec::new();
        for value in [0x1000_u32, 0x2000, handler_address] {
            registration.push(0x68);
            registration.extend(value.to_le_bytes());
        }
        registration.extend([0xe8, 0, 0, 0, 0]);
        let mut handler = vec![0; 133];
        for (offset, bytes) in [
            (0, &b"\x55\x8b\xec\x8b\x0d"[..]),
            (28, &b"\x8b\x75\x08\x8d\x46\xff\x83\xf8"[..]),
            (37, &b"\x76\x0f"[..]),
            (88, &b"\x57\x6a\x00\x53\x8b\xc8\xe8"[..]),
            (99, &b"\x85\xc0\x74\x0d\x83\xee\x01\x74\x0f\x8b\x0d"[..]),
            (114, &b"\xeb\xce\x5f\x5b\x33\xc0\x5e\x5d\xc3"[..]),
            (123, &b"\x5f\x5b\xb8\x01\x00\x00\x00\x5e\x5d\xc3"[..]),
        ] {
            handler[offset..offset + bytes.len()].copy_from_slice(bytes);
        }
        handler[36] = quantity_minus_one;
        BTreeMap::from([
            (0xab6352, registration),
            (0x1000, b"(III)B\0".to_vec()),
            (0x2000, b"SetProduce\0".to_vec()),
            (handler_address, handler),
        ])
    }
    fn decode(data: &BTreeMap<u32, Vec<u8>>, budget: usize) -> Result<Rules, String> {
        Rules::decode([9; 32], budget, |address, length, _| {
            data.get(&address)
                .and_then(|bytes| bytes.get(..length))
                .map(<[u8]>::to_vec)
                .ok_or("missing fixture role".into())
        })
    }
    #[test]
    fn quantity_and_handler_relocation_are_source_derived() {
        for address in [0x3000, 0x9000] {
            let rules = decode(&fixture(address, 6), 512).unwrap();
            assert_eq!(rules.maximum_quantity, 7);
        }
    }
    #[test]
    fn malformed_loop_registration_and_budget_refuse() {
        let mut data = fixture(0x3000, 6);
        assert!(decode(&data, 100).is_err());
        data.get_mut(&0x3000).unwrap()[103] = 0;
        assert!(decode(&data, 512).is_err());
        assert!(decode(&fixture(0x3000, 255), 512).is_err());
        let mut data = fixture(0x3000, 6);
        data.get_mut(&0x2000).unwrap()[0] = b'X';
        assert!(decode(&data, 512).is_err());
    }
    #[test]
    #[ignore = "requires PCX_WAR3_EXE selected original client"]
    fn selected_original_production_contract() {
        let image = std::fs::read(std::env::var("PCX_WAR3_EXE").expect("PCX_WAR3_EXE")).unwrap();
        let rules = Rules::read(
            &image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 0,
                sections: 96,
            },
        )
        .unwrap();
        assert_eq!(rules.image_sha256, sha256(&image));
        assert!(rules.maximum_quantity > 0);
    }
}
