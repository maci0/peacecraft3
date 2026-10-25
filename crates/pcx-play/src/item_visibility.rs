//! Item visibility defaults and flags derived from the selected client.
use crate::{neutral_player::sha256, pe_image::PeSections};

#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: [u8; 32],
    pub(crate) initially_visible: bool,
}
impl Rules {
    pub fn read(image: &[u8]) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("unsupported item visibility client".into());
        }
        let pe = PeSections::parse(image)?;
        Self::decode(sha256(image), |va, n| {
            pe.va_bytes(va, n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("item visibility window {va:#x}"))
        })
    }
    fn decode(
        image_sha256: [u8; 32],
        read: impl Fn(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let exact = |va, n| -> Result<Vec<u8>, String> {
            let bytes = read(va, n)?;
            if bytes.len() != n {
                return Err("truncated item visibility window".into());
            }
            Ok(bytes)
        };
        let getter = exact(0x4991b4, 10)?;
        let initial = exact(0x676998, 7)?;
        let hide = exact(0x6c2bc7, 4)?;
        let show = exact(0x6a2600, 4)?;
        // The item constructor calls the widget constructor whose flag word is read above.
        let constructor = exact(0x6c3349, 11)?;
        let item_table = exact(0x6c337e, 6)?;
        let table = exact(0xea1558 + 0x80, 8)?;
        if getter[..5] != [0x8b, 0x40, 0x20, 0xf7, 0xd0]
            || getter[5..7] != [0x83, 0xe0]
            || getter[8..] != [0x5d, 0xc3]
            || initial[..3] != [0xc7, 0x47, getter[2]]
            || hide[..3] != [0x83, 0x4e, getter[2]]
            || show[..3] != [0x83, 0x61, getter[2]]
            || getter[7].count_ones() != 1
            || getter[7] & 0x80 != 0
            || hide[3] != getter[7]
            || show[3] != !getter[7]
            || constructor[0] != 0xe8
            || i64::from(0x6c334e)
                + i64::from(i32::from_le_bytes(constructor[1..5].try_into().unwrap()))
                != 0x676940
            || constructor[5..] != [0xc7, 0x06, 0xcc, 0xcd, 0xe9, 0x00]
            || item_table != [0xc7, 0x06, 0x58, 0x15, 0xea, 0]
            || table != [0x20, 0x2c, 0x6c, 0, 0xc0, 0x2b, 0x6c, 0]
        {
            return Err("item visibility flag/constructor relationship".into());
        }
        let flags = u32::from_le_bytes(initial[3..].try_into().unwrap());
        Ok(Self {
            image_sha256,
            initially_visible: flags & u32::from(getter[7]) == 0,
        })
    }
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        Self {
            image_sha256: [0x42; 32],
            initially_visible: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn windows() -> std::collections::BTreeMap<u32, Vec<u8>> {
        std::collections::BTreeMap::from([
            (
                0x4991b4,
                vec![0x8b, 0x40, 0x20, 0xf7, 0xd0, 0x83, 0xe0, 4, 0x5d, 0xc3],
            ),
            (0x676998, vec![0xc7, 0x47, 0x20, 2, 0, 0, 0]),
            (0x6c2bc7, vec![0x83, 0x4e, 0x20, 4]),
            (0x6a2600, vec![0x83, 0x61, 0x20, 0xfb]),
            (
                0x6c3349,
                [
                    vec![0xe8],
                    (0x676940i32 - 0x6c334e).to_le_bytes().to_vec(),
                    vec![0xc7, 0x06, 0xcc, 0xcd, 0xe9, 0],
                ]
                .concat(),
            ),
            (0x6c337e, vec![0xc7, 0x06, 0x58, 0x15, 0xea, 0]),
            (0xea15d8, vec![0x20, 0x2c, 0x6c, 0, 0xc0, 0x2b, 0x6c, 0]),
        ])
    }
    #[test]
    fn nonstock_flags_and_malformed_relationships() {
        let mut w = windows();
        let decode = |w: &std::collections::BTreeMap<u32, Vec<u8>>| {
            Rules::decode([7; 32], |va, _| w.get(&va).cloned().ok_or("missing".into()))
        };
        assert!(decode(&w).unwrap().initially_visible);
        w.get_mut(&0x676998).unwrap()[3] = 4;
        assert!(!decode(&w).unwrap().initially_visible);
        for va in [
            0x4991b4, 0x676998, 0x6c2bc7, 0x6a2600, 0x6c3349, 0x6c337e, 0xea15d8,
        ] {
            let mut bad = w.clone();
            bad.get_mut(&va).unwrap()[0] ^= 1;
            assert!(decode(&bad).is_err());
            for len in 0..w[&va].len() {
                let mut bad = w.clone();
                bad.get_mut(&va).unwrap().truncate(len);
                assert!(decode(&bad).is_err());
            }
        }
        assert!(Rules::read(&[]).is_err());
    }
    #[test]
    #[ignore = "requires locally supplied selected Warcraft III.exe"]
    fn reviewed_original_image_decodes() {
        let image = std::fs::read(std::env::var("PCX_WAR3_EXE").expect("PCX_WAR3_EXE")).unwrap();
        assert!(Rules::read(&image).unwrap().initially_visible);
    }
}
