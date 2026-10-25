//! Local ally-color display rules. Values come from the selected client and UI profile.
use crate::{neutral_player::sha256, pe_image::PeSections};

pub use pcx_objects::editor::ally_color::{Palette, Role};

#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: [u8; 32],
    pub initial: i32,
    minimap_only: i32,
    all: i32,
    pub(crate) creep_first: i32,
    pub(crate) unfiltered_owner: i32,
}
impl Rules {
    pub fn read(image: &[u8]) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("unsupported ally-color client".into());
        }
        let pe = PeSections::parse(image)?;
        Self::decode(sha256(image), |va, n| {
            pe.va_bytes(va, n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated ally-color role {va:#x}"))
        })
    }
    fn decode(
        image_sha256: [u8; 32],
        read: impl Fn(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let setter = read(0x4a6e70, 38)?;
        let getter = read(0x490fd0, 29)?;
        if setter.len() != 38
            || getter.len() != 29
            || setter[..4] != [0x55, 0x8b, 0xec, 0x57]
            || setter[9..14] != [0xff, 0x75, 8, 0x8b, 0xf8]
            || setter[14] != 0x6a
            || setter[23..30] != [0xff, 0x75, 8, 0x8b, 0xcf, 0x6a, setter[15]]
            || setter[35..] != [0x5f, 0x5d, 0xc3]
            || getter[..10]
                != [
                    0x55, 0x8b, 0xec, 0x51, 0x8d, 0x45, 0xfc, 0x50, 0x6a, setter[15],
                ]
            || getter[22..] != [0x8b, 0x45, 0xfc, 0x8b, 0xe5, 0x5d, 0xc3]
        {
            return Err("ally-color setter/getter relationship differs".into());
        }
        if setter[15] & 0x80 != 0 {
            return Err("negative ally-color option slot".into());
        }
        let call = |code: &[u8], at: usize, va: u32, target: u32| {
            code.get(at) == Some(&0xe8)
                && code.get(at + 1..at + 5).is_some_and(|d| {
                    (i64::from(va)
                        + at as i64
                        + 5
                        + i64::from(i32::from_le_bytes(d.try_into().unwrap())))
                        == i64::from(target)
                })
        };
        if !call(&setter, 4, 0x4a6e70, 0x42b550)
            || !call(&setter, 18, 0x4a6e70, 0x42ca10)
            || !call(&setter, 30, 0x4a6e70, 0x42ab70)
            || !call(&getter, 10, 0x490fd0, 0x42b550)
            || !call(&getter, 17, 0x490fd0, 0x42b5f0)
        {
            return Err("ally-color option chain differs".into());
        }
        let mode = read(0x5ea74f, 17)?;
        if mode.len() != 17
            || mode[..2] != [0x83, 0xe8]
            || mode[3..7] != [0x74, 0x26, 0x83, 0xe8]
            || mode[8..17] != [0x74, 4, 0x6a, 1, 0xeb, 0x1f, 0x6a, 0, 0xe8]
        {
            return Err("ally-color mode branches differ".into());
        }
        let minimap_only = i32::from(mode[2] as i8);
        let all = minimap_only
            .checked_add(i32::from(mode[7] as i8))
            .ok_or("ally-color mode overflow")?;
        if all == minimap_only {
            return Err("indistinguishable ally-color modes".into());
        }
        let table = read(0x42b630, 9)?;
        if table.len() != 9 || table[..5] != [0xc1, 0xe0, 4, 0xff, 0xb0] {
            return Err("ally-color option table differs".into());
        }
        let address = u32::from_le_bytes(table[5..9].try_into().unwrap())
            .checked_add(u32::from(setter[15]) * 16)
            .ok_or("ally-color option table overflow")?;
        let pointer = read(address, 4)?;
        let pointer = u32::from_le_bytes(
            pointer
                .as_slice()
                .try_into()
                .map_err(|_| "ally-color default pointer")?,
        );
        let text = read(pointer, 16)?;
        let end = text
            .iter()
            .position(|v| *v == 0)
            .ok_or("unterminated ally-color default")?;
        let initial = std::str::from_utf8(&text[..end])
            .map_err(|_| "ally-color default encoding")?
            .parse()
            .map_err(|_| "ally-color integer default")?;
        let keep = read(0x5bd912, 3)?;
        let creep = read(0x5bd9b2, 3)?;
        if keep.len() != 3
            || creep.len() != 3
            || keep[..2] != [0x83, 0xff]
            || creep[..2] != [0x83, 0xff]
            || keep[2] & 0x80 != 0
            || creep[2] & 0x80 != 0
            || keep[2] < creep[2]
        {
            return Err("ally-color owner classes differ".into());
        }
        Ok(Self {
            image_sha256,
            initial,
            minimap_only,
            all,
            creep_first: i32::from(creep[2]),
            unfiltered_owner: i32::from(keep[2]),
        })
    }
    pub fn filters_units(&self, state: i32) -> bool {
        state == self.all
    }
    pub fn filters_minimap(&self, state: i32) -> bool {
        state == self.minimap_only || state == self.all
    }
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        Self {
            image_sha256: [0x42; 32],
            initial: 7,
            minimap_only: 7,
            all: 11,
            creep_first: 24,
            unfiltered_owner: 27,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Input {
    pub rules: Rules,
    pub palette: Palette,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn fixture() -> BTreeMap<u32, Vec<u8>> {
        let mut setter = vec![
            0x55, 0x8b, 0xec, 0x57, 0xe8, 0, 0, 0, 0, 0xff, 0x75, 8, 0x8b, 0xf8, 0x6a, 5, 0x8b,
            0xcf, 0xe8, 0, 0, 0, 0, 0xff, 0x75, 8, 0x8b, 0xcf, 0x6a, 5, 0xe8, 0, 0, 0, 0, 0x5f,
            0x5d, 0xc3,
        ];
        let mut getter = vec![
            0x55, 0x8b, 0xec, 0x51, 0x8d, 0x45, 0xfc, 0x50, 0x6a, 5, 0xe8, 0, 0, 0, 0, 0x8b, 0xc8,
            0xe8, 0, 0, 0, 0, 0x8b, 0x45, 0xfc, 0x8b, 0xe5, 0x5d, 0xc3,
        ];
        fn call(b: &mut [u8], at: usize, va: u32, to: u32) {
            b[at + 1..at + 5].copy_from_slice(
                &((i64::from(to) - i64::from(va) - at as i64 - 5) as i32).to_le_bytes(),
            );
        }
        for (at, to) in [(4, 0x42b550), (18, 0x42ca10), (30, 0x42ab70)] {
            call(&mut setter, at, 0x4a6e70, to);
        }
        for (at, to) in [(10, 0x42b550), (17, 0x42b5f0)] {
            call(&mut getter, at, 0x490fd0, to);
        }
        BTreeMap::from([
            (0x4a6e70, setter),
            (0x490fd0, getter),
            (
                0x5ea74f,
                vec![
                    0x83, 0xe8, 7, 0x74, 0x26, 0x83, 0xe8, 4, 0x74, 4, 0x6a, 1, 0xeb, 0x1f, 0x6a,
                    0, 0xe8,
                ],
            ),
            (
                0x42b630,
                [&[0xc1, 0xe0, 4, 0xff, 0xb0][..], &0x100000u32.to_le_bytes()].concat(),
            ),
            (0x100050, 0x200000u32.to_le_bytes().to_vec()),
            (0x200000, [b"9\0".as_slice(), &[0; 14]].concat()),
            (0x5bd912, vec![0x83, 0xff, 15]),
            (0x5bd9b2, vec![0x83, 0xff, 10]),
        ])
    }
    fn decode(windows: &BTreeMap<u32, Vec<u8>>) -> Result<Rules, String> {
        Rules::decode([0x42; 32], |va, n| {
            windows
                .get(&va)
                .filter(|b| b.len() == n)
                .cloned()
                .ok_or_else(|| format!("missing synthetic window {va:#x}"))
        })
    }
    #[test]
    fn nonstock_state_default_modes_and_owner_roles_come_from_operands() {
        let r = decode(&fixture()).unwrap();
        assert_eq!(r.initial, 9);
        assert_eq!(r.creep_first, 10);
        assert_eq!(r.unfiltered_owner, 15);
        for state in [-1, 0, 1, 2, 9, 12, i32::MAX] {
            assert!(!r.filters_units(state));
            assert!(!r.filters_minimap(state));
        }
        assert!(!r.filters_units(7));
        assert!(r.filters_minimap(7));
        assert!(r.filters_units(11));
        assert!(r.filters_minimap(11));
    }
    #[test]
    fn changed_calls_fields_branches_defaults_and_all_short_windows_refuse() {
        for (va, at) in [
            (0x4a6e70, 15),
            (0x4a6e70, 31),
            (0x490fd0, 9),
            (0x5ea74f, 4),
            (0x42b630, 3),
            (0x200000, 0),
            (0x5bd912, 1),
        ] {
            let mut data = fixture();
            data.get_mut(&va).unwrap()[at] ^= 0xff;
            assert!(decode(&data).is_err(), "{va:#x}/{at}");
        }
        for (va, full) in fixture() {
            for n in 0..full.len() {
                let mut data = fixture();
                data.insert(va, full[..n].to_vec());
                assert!(decode(&data).is_err(), "{va:#x}/{n}");
            }
        }
    }
    #[test]
    #[ignore = "requires PCX_WAR3_EXE from the selected local install"]
    fn selected_original_ally_color_rules_decode() {
        let bytes =
            std::fs::read(std::env::var("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE")).unwrap();
        let rules = Rules::read(&bytes).unwrap();
        assert_eq!(rules.initial, 1);
        assert!(rules.filters_minimap(1));
        assert!(!rules.filters_units(1));
        assert!(rules.filters_units(2));
        assert_eq!((rules.creep_first, rules.unfiltered_owner), (24, 27));
    }
}
