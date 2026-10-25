//! Fresh roster color identity, joined to the selected public getter/setter field.
use crate::{neutral_player::sha256, pe_image::PeSections};

#[derive(Clone, Debug)]
pub struct Rules {
    pub(crate) image_sha256: [u8; 32],
    threshold: u32,
    legacy_first: i32,
    legacy_count: i32,
    legacy_shift: i32,
}

impl Rules {
    pub fn read(image: &[u8]) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("unsupported fresh player-color client".into());
        }
        let pe = PeSections::parse(image)?;
        let read = |va, n| {
            pe.va_bytes(va, n)
                .ok_or("truncated fresh player-color role")
        };
        if !shape(
            read(0x494554, 6)?,
            read(0x4a9565, 6)?,
            read(0x47ce30, 14)?,
            read(0x47ce41, 16)?,
            read(0x47cf49, 16)?,
        ) {
            return Err("fresh player-color getter/setter/roster relationship differs".into());
        }
        let (threshold, legacy_first, legacy_count, legacy_shift) =
            aliases(read(0x4a425f, 18)?).ok_or("unsupported Player neutral-slot normalization")?;
        if read(0x4a4254, 5)? != [0xe8, 0xd7, 0, 0x26, 0] {
            return Err("Player version getter relationship differs".into());
        }
        Ok(Self {
            image_sha256: sha256(image),
            threshold,
            legacy_first,
            legacy_count,
            legacy_shift,
        })
    }

    #[cfg(test)]
    pub(crate) fn fixture(image_sha256: [u8; 32]) -> Self {
        Self {
            image_sha256,
            threshold: 900,
            legacy_first: 2,
            legacy_count: 4,
            legacy_shift: 9,
        }
    }

    pub(crate) fn color(&self, map_word: u32, number: i32) -> Option<i32> {
        if map_word < self.threshold
            && (number.wrapping_sub(self.legacy_first) as u32) < self.legacy_count as u32
        {
            number.checked_add(self.legacy_shift)
        } else {
            Some(number)
        }
    }
}

fn aliases(code: &[u8]) -> Option<(u32, i32, i32, i32)> {
    let [
        0x3d,
        a,
        b,
        c,
        d,
        0x73,
        0x0b,
        0x8d,
        0x41,
        first,
        0x83,
        0xf8,
        last @ 0..=0x7f,
        0x77,
        0x03,
        0x83,
        0xc1,
        shift @ 1..=0x7f,
    ] = code
    else {
        return None;
    };
    let first = -i32::from(*first as i8);
    (first >= 0).then_some((
        u32::from_le_bytes([*a, *b, *c, *d]),
        first,
        i32::from(*last) + 1,
        i32::from(*shift),
    ))
}

fn shape(get: &[u8], set: &[u8], head: &[u8], body: &[u8], tail: &[u8]) -> bool {
    let ([0x8b, 0x80, field @ ..], [0x89, 0x88, same @ ..]) = (get, set) else {
        return false;
    };
    if field.len() != 4
        || field != same
        || head
            != [
                0x33, 0xdb, 0x39, 0x5f, 0x54, 0x0f, 0x86, 0x1e, 1, 0, 0, 0x8d, 0x77, 0x58,
            ]
    {
        return false;
    }
    // EBX starts at zero, is the player-call argument and color value, then advances
    // with the same player-pointer stride and roster bound on each iteration.
    body.len() == 16
        && body[..4] == [0x8b, 0x0e, 0x53, 0xe8]
        && body[4..8] == (0x4c0c00i32 - 0x47ce49).to_le_bytes()
        && body[8..12] == [0x8b, 0x06, 0x89, 0x98]
        && body[12..] == *field
        && tail
            == [
                0x43, 0x83, 0xc6, 4, 0x89, 0x75, 0xf0, 0x3b, 0x5f, 0x54, 0x0f, 0x82, 0xe8, 0xfe,
                0xff, 0xff,
            ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colors_follow_the_slot_index_only_when_the_whole_loop_and_field_join_match() {
        let get = [0x8b, 0x80, 0xa4, 2, 0, 0];
        let set = [0x89, 0x88, 0xa4, 2, 0, 0];
        let head = [
            0x33, 0xdb, 0x39, 0x5f, 0x54, 0x0f, 0x86, 0x1e, 1, 0, 0, 0x8d, 0x77, 0x58,
        ];
        let body = [
            0x8b, 0x0e, 0x53, 0xe8, 0xb7, 0x3d, 4, 0, 0x8b, 6, 0x89, 0x98, 0xa4, 2, 0, 0,
        ];
        let tail = [
            0x43, 0x83, 0xc6, 4, 0x89, 0x75, 0xf0, 0x3b, 0x5f, 0x54, 0x0f, 0x82, 0xe8, 0xfe, 0xff,
            0xff,
        ];
        assert!(shape(&get, &set, &head, &body, &tail));
        for (which, bytes) in [get.as_slice(), set.as_slice(), &head, &body, &tail]
            .into_iter()
            .enumerate()
        {
            for n in 0..bytes.len() {
                let mut inputs = [&get[..], &set, &head, &body, &tail];
                inputs[which] = &bytes[..n];
                assert!(!shape(
                    inputs[0], inputs[1], inputs[2], inputs[3], inputs[4]
                ));
            }
        }
        let mut bad = tail;
        bad[0] = 0x4b;
        assert!(!shape(&get, &set, &head, &body, &bad));
    }
    #[test]
    fn legacy_neutral_aliases_are_operand_driven_and_version_bounded() {
        let code = [
            0x3d, 0x84, 3, 0, 0, 0x73, 0x0b, 0x8d, 0x41, 0xfe, 0x83, 0xf8, 3, 0x77, 3, 0x83, 0xc1,
            9,
        ];
        assert_eq!(aliases(&code), Some((900, 2, 4, 9)));
        let rules = Rules::fixture([7; 32]);
        for (word, slot, expected) in [
            (899, 1, 1),
            (899, 2, 11),
            (899, 5, 14),
            (899, 6, 6),
            (900, 5, 5),
        ] {
            assert_eq!(rules.color(word, slot), Some(expected));
        }
        for n in 0..code.len() {
            assert!(aliases(&code[..n]).is_none());
        }
    }
    #[test]
    #[ignore = "requires PCX_WAR3_EXE from the selected local install"]
    fn selected_original_roster_color_mapping_decodes() {
        let image = std::fs::read(std::env::var("PCX_WAR3_EXE").unwrap()).unwrap();
        Rules::read(&image).unwrap();
    }
}
