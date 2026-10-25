//! Source-bound SetUnitBlendTime clamp and disable threshold.
use crate::{neutral_player::sha256, pe_image::PeSections};
#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: [u8; 32],
    maximum: f32,
    epsilon: f32,
}
impl Rules {
    pub fn read(image: &[u8]) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("unsupported unit blend client".into());
        }
        let pe = PeSections::parse(image)?;
        Self::decode(sha256(image), |va, n| {
            pe.va_bytes(va, n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("unit blend window {va:#x}"))
        })
    }
    fn decode(
        image_sha256: [u8; 32],
        read: impl Fn(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let exact = |va, n| -> Result<Vec<u8>, String> {
            let b = read(va, n)?;
            if b.len() != n {
                return Err("truncated unit blend window".into());
            }
            Ok(b)
        };
        let clamp = exact(0x6c05b2, 29)?;
        let near = exact(0x6c05dd, 13)?;
        if clamp[..6] != [0x0f, 0x57, 0xc9, 0x0f, 0x2f, 0xc1]
            || clamp[6] != 0x76
            || 8 + i32::from(clamp[7] as i8) != 21
            || clamp[8..12] != [0xf3, 0x0f, 0x10, 0x0d]
            || clamp[16..20] != [0x0f, 0x2f, 0xc8, 0x77]
            || 21 + i32::from(clamp[20] as i8) != 29
            || clamp[21..] != [0x0f, 0x28, 0xc1, 0xf3, 0x0f, 0x11, 0x45, 8]
            || near[..4] != [0xf3, 0x0f, 0x10, 5]
            || near[8..12] != [0x0f, 0x2f, 0xc1, 0x77]
            || i64::from(0x6c05dd) + 13 + i64::from(near[12] as i8) != 0x6c060f
        {
            return Err("unit blend clamp/disable branch".into());
        }
        let maximum = f32::from_le_bytes(
            exact(u32::from_le_bytes(clamp[12..16].try_into().unwrap()), 4)?
                .try_into()
                .unwrap(),
        );
        let epsilon = f32::from_le_bytes(
            exact(u32::from_le_bytes(near[4..8].try_into().unwrap()), 4)?
                .try_into()
                .unwrap(),
        );
        if !maximum.is_finite() || !epsilon.is_finite() || epsilon <= 0. || maximum < epsilon {
            return Err("invalid unit blend bounds".into());
        }
        Ok(Self {
            image_sha256,
            maximum,
            epsilon,
        })
    }
    pub(crate) fn bound(&self, value: f32) -> Result<f32, String> {
        if !value.is_finite() {
            return Err("finite blend time required".into());
        }
        let value = value.max(0.).min(self.maximum);
        Ok(if value < self.epsilon { 0. } else { value })
    }
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        Self {
            image_sha256: [0x42; 32],
            maximum: 1.75,
            epsilon: 0.0001,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonstock_operands_and_branch_mutants() {
        let windows = std::collections::BTreeMap::from([
            (
                0x6c05b2,
                [
                    vec![
                        0x0f, 0x57, 0xc9, 0x0f, 0x2f, 0xc1, 0x76, 13, 0xf3, 0x0f, 0x10, 0x0d,
                    ],
                    0x100u32.to_le_bytes().to_vec(),
                    vec![
                        0x0f, 0x2f, 0xc8, 0x77, 8, 0x0f, 0x28, 0xc1, 0xf3, 0x0f, 0x11, 0x45, 8,
                    ],
                ]
                .concat(),
            ),
            (
                0x6c05dd,
                [
                    vec![0xf3, 0x0f, 0x10, 5],
                    0x200u32.to_le_bytes().to_vec(),
                    vec![0x0f, 0x2f, 0xc1, 0x77, 0x25],
                ]
                .concat(),
            ),
            (0x100, 2.75f32.to_le_bytes().to_vec()),
            (0x200, 0.01f32.to_le_bytes().to_vec()),
        ]);
        let decode = |w: &std::collections::BTreeMap<u32, Vec<u8>>| {
            Rules::decode([7; 32], |va, _| w.get(&va).cloned().ok_or("missing".into()))
        };
        let r = decode(&windows).unwrap();
        for (v, want) in [(-1., 0.), (0.009, 0.), (0.01, 0.01), (1.5, 1.5), (4., 2.75)] {
            assert_eq!(r.bound(v).unwrap(), want);
        }
        assert!(r.bound(f32::NAN).is_err());
        for (va, offset) in [(0x6c05b2, 7), (0x6c05b2, 20), (0x6c05dd, 12)] {
            let mut bad = windows.clone();
            bad.get_mut(&va).unwrap()[offset] ^= 1;
            assert!(decode(&bad).is_err());
        }
        for va in windows.keys() {
            for len in 0..windows[va].len() {
                let mut bad = windows.clone();
                bad.get_mut(va).unwrap().truncate(len);
                assert!(decode(&bad).is_err());
            }
        }
        assert!(Rules::read(&[]).is_err());
    }
    #[test]
    #[ignore = "requires locally supplied PCX_WAR3_EXE"]
    fn reviewed_original_image_decodes() {
        let image = std::fs::read(std::env::var("PCX_WAR3_EXE").unwrap()).unwrap();
        let r = Rules::read(&image).unwrap();
        assert_eq!(r.bound(2.).unwrap(), 1.);
        assert_eq!(r.bound(0.).unwrap(), 0.);
    }
}
