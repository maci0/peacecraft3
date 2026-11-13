//! Native position-vector heading: software length, reciprocal ratio and table arccosine.
use super::{call, shape, word};
use crate::{script_numeric::NumericPolicy, script_random::real};

#[derive(Clone, Debug)]
pub struct Rules {
    threshold: u32,
    one: u32,
    fixed_scale: u32,
    cap: i32,
    index_shift: u32,
    index_mask: u32,
    fraction_shift: u32,
    regular_scale: u32,
    pi_fixed: u32,
    pi: u32,
    edge_bias: i32,
    edge_scale: u32,
    regular: Vec<u32>,
    edge: Vec<u32>,
    direction_threshold: u32,
    coincident_threshold: u32,
}

impl Rules {
    pub(super) fn read(
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        numeric: &NumericPolicy,
    ) -> Result<Self, String> {
        let code = read(0x9a05f0, 0x1b7, true)?;
        shape(&code, 0x17, &[0x3d])?;
        shape(&code, 0x24, &[0x81, 0xc2])?;
        shape(&code, 0x63, &[0x8d, 0x81])?;
        shape(&code, 0x8c, &[0xc1, 0xe6, 0x0c])?;
        shape(&code, 0x93, &[0xc1, 0xfa, 0x14])?;
        shape(&code, 0x99, &[0x81, 0xe2])?;
        shape(&code, 0xa5, &[0x8b, 0x0c, 0x95])?;
        shape(&code, 0xae, &[0x2b, 0x04, 0x95])?;
        shape(&code, 0xbe, &[0xb8])?;
        shape(&code, 0xc7, &[0xb8])?;
        shape(&code, 0xce, &[0xb9])?;
        shape(&code, 0xfb, &[0x81, 0xe1])?;
        shape(&code, 0x11c, &[0xbf])?;
        shape(&code, 0x139, &[0x8d, 0x0c, 0xc5])?;
        shape(&code, 0x14a, &[0x8b, 0x34, 0x8d])?;
        shape(&code, 0x153, &[0x2b, 0x04, 0x8d])?;
        shape(&code, 0x179, &[0x25])?;
        call(&code, 0x9a05f0, 0xe2, 0x9a1620)?;
        call(&code, 0x9a05f0, 0x125, 0x9a0330)?;
        call(&code, 0x9a05f0, 0x163, 0x9a1620)?;
        call(&code, 0x9a05f0, 0x194, 0x9a00b0)?;
        let table = word(&code, 0xa8)?;
        let index_mask = word(&code, 0x9b)?;
        let count = index_mask
            .checked_add(2)
            .ok_or("heading arccos table count overflow")?;
        if count > 4096
            || word(&code, 0xb1)?
                != table
                    .checked_add(4)
                    .ok_or("heading arccos table address overflow")?
            || word(&code, 0xbf)?
                != table
                    .checked_add((count - 1) * 4)
                    .ok_or("heading arccos table address overflow")?
            || word(&code, 0xc8)?
                != table
                    .checked_add((count - 2) * 4)
                    .ok_or("heading arccos table address overflow")?
        {
            return Err("heading arccos mirrored table relationship differs".into());
        }
        let edge_bias = word(&code, 0x13c)? as i32;
        let edge_count = (u32::BITS as i32 * 8)
            .checked_add(edge_bias)
            .and_then(|n| n.checked_add(2))
            .filter(|n| *n > 1 && *n <= 4096)
            .ok_or("heading arccos edge table count invalid")? as usize;
        let edge_table = word(&code, 0x14d)?;
        if word(&code, 0x156)?
            != edge_table
                .checked_add(4)
                .ok_or("heading arccos edge table overflow")?
        {
            return Err("heading arccos edge table relationship differs".into());
        }
        let regular = read(table, count as usize * 4, false)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect();
        let edge = read(edge_table, edge_count * 4, false)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect();
        let direction = read(0x998640, 0x75, true)?;
        shape(&direction, 0x11, &[0x3d])?;
        shape(&direction, 0x4d, &[0x81, 0xf9])?;
        if word(&direction, 0x12)? != word(&direction, 0x4f)? {
            return Err("heading direction near-zero thresholds differ".into());
        }
        call(&direction, 0x998640, 0x30, 0x9a02d0)?;
        call(&direction, 0x998640, 0x3a, 0x9a05f0)?;
        let divide = read(0x9a02d0, 0x39, true)?;
        call(&divide, 0x9a02d0, 0x20, numeric.multiplication_va())?;
        let coincident = read(0x8bbb2d, 8, true)?;
        shape(&coincident, 0, &[0xf3, 0x0f, 0x10, 0x05])?;
        let result = Self {
            threshold: word(&code, 0x18)?,
            one: word(&code, 0x11d)?,
            fixed_scale: word(&code, 0x26)?,
            cap: word(&code, 0x65)? as i32,
            index_shift: 20,
            index_mask,
            fraction_shift: 12,
            regular_scale: word(&code, 0xfd)?,
            pi_fixed: word(&code, 0xcf)?,
            edge_bias,
            pi: word(&read(word(&code, 0x18f)?, 4, false)?, 0)?,
            edge_scale: word(&code, 0x17a)?,
            regular,
            edge,
            direction_threshold: word(&direction, 0x12)?,
            coincident_threshold: word(&read(word(&coincident, 4)?, 4, false)?, 0)?,
        };
        if result.threshold >= result.one || result.cap <= 0 || result.one != 1_f32.to_bits() {
            return Err("heading arccos source ranges invalid".into());
        }
        Ok(result)
    }

    pub(crate) fn arccosine(&self, value: f32) -> Result<f32, String> {
        let calculate = || {
            let bits = real::from_f32(value)?;
            let abs = bits & !(1 << 31);
            let result = if abs <= self.threshold {
                let fixed =
                    real::truncate(bits.wrapping_add(self.fixed_scale))?.clamp(-self.cap, self.cap);
                let index = ((fixed >> self.index_shift) as u32 & self.index_mask) as usize;
                let fraction = (fixed as u32).wrapping_shl(self.fraction_shift);
                let fraction = fraction | (fraction >> self.index_shift);
                let (start, difference) = if fixed < 0 {
                    let at = self.regular.len() - 1 - index;
                    (
                        self.pi_fixed.wrapping_sub(self.regular[at]),
                        self.regular[at - 1].wrapping_sub(self.regular[at]),
                    )
                } else {
                    (
                        self.regular[index],
                        self.regular[index].wrapping_sub(self.regular[index + 1]),
                    )
                };
                let interpolated = start
                    .wrapping_sub(((u64::from(difference) * u64::from(fraction)) >> 32) as u32);
                let converted = real::from_i32(interpolated as i32)?;
                converted.wrapping_sub(if converted & 0x7f80_0000 != 0 {
                    self.regular_scale
                } else {
                    0
                })
            } else {
                let difference = self.one - abs.min(self.one);
                let zeros = difference.leading_zeros();
                let normalized = if zeros < u32::BITS {
                    (!difference) << zeros
                } else {
                    0
                };
                let index =
                    ((zeros as i32 * 8 + self.edge_bias) as u32 | (normalized >> 28)) as usize;
                let (&start, &next) = self
                    .edge
                    .get(index)
                    .zip(self.edge.get(index + 1))
                    .ok_or(real::MathError::Unsupported)?;
                let delta = ((u64::from(start.wrapping_sub(next)) * u64::from(normalized << 4))
                    >> 32) as u32;
                let converted = real::from_i32(start.wrapping_sub(delta) as i32)?;
                let angle = converted.wrapping_sub(if converted & 0x7f80_0000 != 0 {
                    self.edge_scale
                } else {
                    0
                });
                if value < 0.0 {
                    real::sub(self.pi, angle)?
                } else {
                    angle
                }
            };
            Ok(f32::from_bits(result))
        };
        calculate().map_err(|e: real::MathError| format!("heading arccosine: {e}"))
    }

    pub(crate) fn angle(
        &self,
        numeric: &NumericPolicy,
        vector: [f32; 2],
        full_turn: f32,
        zero: f32,
    ) -> Result<f32, String> {
        let math = numeric.math().ok_or("heading square-root source missing")?;
        let calculate = || {
            let x = real::from_f32(vector[0])?;
            let y = real::from_f32(vector[1])?;
            let length = math.square_root(real::add(real::mul(x, x)?, real::mul(y, y)?)?)?;
            if real::abs(length) <= self.direction_threshold {
                return Ok(zero);
            }
            let angle = self
                .arccosine(f32::from_bits(numeric.division(x, length)?))
                .map_err(|_| real::MathError::Unsupported)?;
            if real::abs(angle.to_bits()) <= self.direction_threshold {
                return Ok(zero);
            }
            if vector[1] < zero {
                real::sub(full_turn.to_bits(), angle.to_bits()).map(f32::from_bits)
            } else {
                Ok(angle)
            }
        };
        calculate().map_err(|e: real::MathError| format!("heading direction: {e}"))
    }

    pub(crate) fn coincident(&self, vector: [f32; 2]) -> Result<bool, String> {
        let calculate = || {
            let x = real::from_f32(vector[0])?;
            let y = real::from_f32(vector[1])?;
            let squared = real::add(real::mul(x, x)?, real::mul(y, y)?)?;
            Ok(f32::from_bits(real::abs(squared)) < f32::from_bits(self.coincident_threshold))
        };
        calculate().map_err(|e: real::MathError| format!("heading coincident point: {e}"))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn nonstock() -> Rules {
        Rules {
            threshold: 0.75_f32.to_bits(),
            one: 1_f32.to_bits(),
            fixed_scale: 23 << 23,
            cap: (1 << 30) - 1,
            index_shift: 20,
            index_mask: 7,
            fraction_shift: 12,
            regular_scale: 0,
            pi_fixed: 4096,
            pi: 8192_f32.to_bits(),
            edge_bias: -120,
            edge_scale: 0,
            regular: (0..9).map(|i| 1000 - i * 64).collect(),
            edge: (0..138).map(|i| 1024 - i * 4).collect(),
            direction_threshold: 0.03125_f32.to_bits(),
            coincident_threshold: 0.0625_f32.to_bits(),
        }
    }

    #[test]
    fn nonstock_arccosine_uses_mirrored_and_edge_tables_with_native_interpolation() {
        let rules = nonstock();
        assert_eq!(rules.arccosine(0.3125).unwrap(), 840.);
        assert_eq!(rules.arccosine(-0.3125).unwrap(), 3256.);
        assert_eq!(rules.arccosine(1.).unwrap(), 480.);
        assert_eq!(rules.arccosine(-1.).unwrap(), 7712.);
        let mut changed = rules.clone();
        changed.regular[2] += 128;
        assert_ne!(changed.arccosine(0.3125).unwrap(), 840.);
        changed.edge[136] += 16;
        assert_ne!(changed.arccosine(1.).unwrap(), 480.);
        for value in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::from_bits(1),
        ] {
            assert!(rules.arccosine(value).is_err());
        }
        assert!(rules.coincident([0.125, 0.125]).unwrap());
        assert!(!rules.coincident([0.25, 0.]).unwrap());
        assert!(rules.coincident([f32::NAN, 0.]).is_err());
    }
    #[test]
    #[ignore = "requires PCX_WAR3_EXE and PCX_HEADING_ACOS_WITNESS from a private original-client probe"]
    fn selected_heading_direction_matches_native_arccosine_witness() {
        use std::io::Read;
        let (image, origin, numeric, budget) = super::super::tests::selected_numeric_image();
        let registry = crate::campaign_client::order_registry(&image, origin.clone()).unwrap();
        let activity = crate::campaign_client::unit_activity(&image, origin, &registry)
            .unwrap()
            .with_heading(&image, &numeric, budget)
            .unwrap();
        let direction = &activity
            .heading_rules()
            .unwrap()
            .profile
            .as_ref()
            .unwrap()
            .direction;
        let mut bytes = Vec::new();
        std::fs::File::open(
            std::env::var_os("PCX_HEADING_ACOS_WITNESS").expect("PCX_HEADING_ACOS_WITNESS"),
        )
        .unwrap()
        .take((64 << 10) + 1)
        .read_to_end(&mut bytes)
        .unwrap();
        assert!(bytes.len() <= 64 << 10);
        let witness: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let hash: String = crate::neutral_player::sha256(&image)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(witness["image_sha256"], hash);
        assert_eq!(witness["roc"], true);
        let cases = witness["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 169);
        for (index, case) in cases.iter().enumerate() {
            assert_eq!(case["index"], index);
            let bits = |field| u32::try_from(case[field].as_u64().unwrap()).unwrap();
            assert_eq!(
                direction
                    .arccosine(f32::from_bits(bits("base")))
                    .unwrap()
                    .to_bits(),
                bits("result"),
                "native arccosine witness {index}"
            );
        }
    }
}
