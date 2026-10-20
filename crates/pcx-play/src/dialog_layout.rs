//! Reviewed client ScriptDialog dynamic geometry; every numeric operand is read.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};
#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    caption_extra: f32,
    height_gap: f32,
    padding: f32,
    minimum_height: f32,
    round_step: f32,
    round_period: f64,
    half: f32,
    minimum_caption_top: f32,
    caption_button_gap: f32,
    empty_button_top: f32,
    button_gap: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub height: f32,
    pub caption_top: f32,
    pub first_button_top: f32,
    pub button_step: f32,
}
impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |why: &str| format!("script dialog layout {}: {why}", origin.source);
        if image.len() > 32 << 20
            || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail("unsupported client identity"));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| fail(&e.to_string()))?;
        Self::decode(origin.clone(), sha256(image), |va, len, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..len)
                .ok_or_else(|| "truncated dialog instruction/data record".into())
                .map(<[u8]>::to_vec)
        })
        .map_err(|e| fail(&e))
    }
    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        fn reference(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            opcode: &[u8],
            bytes: usize,
        ) -> Result<Vec<u8>, String> {
            let instruction = read(va, opcode.len() + 4, true)?;
            if instruction.get(..opcode.len()) != Some(opcode) {
                return Err(format!("unsupported dialog instruction at {va:#x}"));
            }
            let pointer = u32::from_le_bytes(
                instruction[opcode.len()..]
                    .try_into()
                    .map_err(|_| "truncated dialog operand")?,
            );
            read(pointer, bytes, false)
        }
        let mut float = |va, opcode: &[u8]| -> Result<f32, String> {
            let bytes = reference(&mut read, va, opcode, 4)?;
            Ok(f32::from_le_bytes(
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| "truncated dialog float")?,
            ))
        };
        let caption_extra = float(0x649bc5, &[0xf3, 0x0f, 0x58, 0x05])?;
        let height_gap = float(0x649bcd, &[0xf3, 0x0f, 0x58, 0x05])?;
        let padding = float(0x649bd5, &[0xf3, 0x0f, 0x58, 0x05])?;
        let minimum_height = float(0x649c5c, &[0xf3, 0x0f, 0x10, 0x05])?;
        let round_step = float(0x649cc3, &[0xf3, 0x0f, 0x58, 0x05])?;
        let half = float(0x649cf0, &[0xf3, 0x0f, 0x59, 0x05])?;
        let bytes = reference(&mut read, 0x649c98, &[0xdd, 0x05], 8)?;
        let round_period = f64::from_le_bytes(
            bytes
                .as_slice()
                .try_into()
                .map_err(|_| "truncated dialog rounding period")?,
        );
        let mut negative_immediate = |va| -> Result<f32, String> {
            let instruction = read(va, 8, true)?;
            if instruction.get(..4) != Some(&[0xc7, 0x44, 0x24, 0x04]) {
                return Err(format!("unsupported dialog anchor instruction at {va:#x}"));
            }
            let value = f32::from_le_bytes(
                instruction[4..]
                    .try_into()
                    .map_err(|_| "truncated dialog anchor operand")?,
            );
            if !value.is_finite() || value > 0. {
                return Err("invalid negative dialog anchor operand".into());
            }
            Ok(-value)
        };
        let minimum_caption_top = negative_immediate(0x649c78)?;
        let caption_button_gap = negative_immediate(0x629277)?;
        let empty_button_top = negative_immediate(0x62929a)?;
        let button_gap = negative_immediate(0x6292ec)?;
        if [
            caption_extra,
            height_gap,
            padding,
            minimum_height,
            round_step,
            half,
            minimum_caption_top,
            caption_button_gap,
            empty_button_top,
            button_gap,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.)
            || minimum_height <= 0.
            || round_step <= 0.
            || half <= 0.
            || !round_period.is_finite()
            || round_period <= 0.
        {
            return Err("invalid dialog geometry operand".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            caption_extra,
            height_gap,
            padding,
            minimum_height,
            round_step,
            round_period,
            half,
            minimum_caption_top,
            caption_button_gap,
            empty_button_top,
            button_gap,
        })
    }
    /// Height/rounding and title/button anchors follow reviewed client arithmetic.
    /// Caption and button extents are supplied by their source-backed text/control layout.
    pub fn geometry(
        &self,
        caption_height: Option<f32>,
        button_height: f32,
        count: usize,
    ) -> Result<Geometry, String> {
        if !button_height.is_finite()
            || button_height <= 0.
            || caption_height.is_some_and(|h| !h.is_finite() || h < 0.)
        {
            return Err("invalid script dialog content extent".into());
        }
        let count = u32::try_from(count)
            .map_err(|_| "script dialog button count exceeds native record range")?;
        let caption = caption_height.map_or(self.padding, |h| {
            ((h + self.caption_extra) + self.height_gap) + self.padding
        });
        let buttons = if count == 0 {
            self.padding
        } else {
            ((button_height + self.height_gap) * count as f32 - self.height_gap) + self.padding
        };
        let total = caption + buttons;
        if !total.is_finite() {
            return Err("script dialog content height overflow".into());
        }
        let (height, caption_top) = if self.minimum_height > total {
            (self.minimum_height, self.minimum_caption_top)
        } else {
            let remainder = (f64::from(total) % self.round_period) as f32;
            let height = if remainder != 0. {
                (total - remainder) + self.round_step
            } else {
                total
            };
            let top = (height - ((total - self.padding) - self.padding)) * self.half;
            (height, top)
        };
        let first_button_top = caption_height.map_or(self.empty_button_top, |h| {
            (caption_top + h) + self.caption_button_gap
        });
        let button_step = button_height + self.button_gap;
        if [height, caption_top, first_button_top, button_step]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
            || height <= 0.
        {
            return Err("script dialog geometry is unrepresentable".into());
        }
        Ok(Geometry {
            height,
            caption_top,
            first_button_top,
            button_step,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic:dialog-layout".into(),
            edition: "synthetic".into(),
            version: "fixture".into(),
        }
    }
    fn fixture(gap: f32, min: f32) -> std::collections::BTreeMap<u32, Vec<u8>> {
        let mut map = std::collections::BTreeMap::new();
        for (index, (va, opcode, value)) in [
            (0x649bc5, [0xf3, 0x0f, 0x58, 0x05], 0.006),
            (0x649bcd, [0xf3, 0x0f, 0x58, 0x05], gap),
            (0x649bd5, [0xf3, 0x0f, 0x58, 0x05], 0.02),
            (0x649c5c, [0xf3, 0x0f, 0x10, 0x05], min),
            (0x649cc3, [0xf3, 0x0f, 0x58, 0x05], 0.04),
            (0x649cf0, [0xf3, 0x0f, 0x59, 0x05], 0.5),
        ]
        .into_iter()
        .enumerate()
        {
            let address = 0x1000 + index as u32 * 16;
            let mut instruction = opcode.to_vec();
            instruction.extend(address.to_le_bytes());
            map.insert(va, instruction);
            map.insert(address, value.to_le_bytes().to_vec());
        }
        let mut instruction = vec![0xdd, 0x05];
        instruction.extend(0x2000u32.to_le_bytes());
        map.insert(0x649c98, instruction);
        map.insert(0x2000, 0.04f64.to_le_bytes().to_vec());
        for (va, value) in [
            (0x649c78, -0.02f32),
            (0x629277, -0.009),
            (0x62929a, -0.024),
            (0x6292ec, -gap),
        ] {
            let mut instruction = vec![0xc7, 0x44, 0x24, 0x04];
            instruction.extend(value.to_le_bytes());
            map.insert(va, instruction);
        }
        map
    }
    fn rules(map: &std::collections::BTreeMap<u32, Vec<u8>>) -> Result<Rules, String> {
        Rules::decode(origin(), sha256(b"synthetic"), |va, len, _| {
            map.get(&va)
                .and_then(|v| v.get(..len))
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "missing synthetic window".into())
        })
    }
    #[test]
    fn geometry_tracks_mutated_instruction_operands_and_rounding() {
        let a = rules(&fixture(0.002, 0.08)).unwrap();
        let b = rules(&fixture(0.007, 0.12)).unwrap();
        assert_eq!(a.geometry(None, 0.03, 0).unwrap().height, 0.08);
        assert_eq!(b.geometry(None, 0.03, 0).unwrap().height, 0.12);
        let ga = a.geometry(Some(0.015), 0.03, 3).unwrap();
        let gb = b.geometry(Some(0.015), 0.03, 3).unwrap();
        assert_ne!(ga.button_step, gb.button_step);
        assert_ne!(ga.height, gb.height);
        assert_eq!(ga.first_button_top, ga.caption_top + 0.015 + 0.009);
        assert!(a.geometry(Some(f32::NAN), 0.03, 1).is_err());
        assert!(a.geometry(None, 0., 1).is_err());
    }
    #[test]
    fn present_zero_caption_keeps_caption_relations_distinct_from_absent() {
        let rules = rules(&fixture(0.002, 0.08)).unwrap();
        let present = rules.geometry(Some(0.), 0.03, 1).unwrap();
        let absent = rules.geometry(None, 0.03, 1).unwrap();
        assert_eq!(
            present.first_button_top,
            present.caption_top + rules.caption_button_gap
        );
        assert_eq!(absent.first_button_top, rules.empty_button_top);
        assert_ne!(present.first_button_top, absent.first_button_top);
        assert!(rules.geometry(Some(-0.001), 0.03, 1).is_err());
    }
    #[test]
    fn malformed_instruction_shapes_and_source_values_refuse() {
        let mut map = fixture(0.002, 0.08);
        map.get_mut(&0x649bc5).unwrap()[0] = 0x90;
        assert!(rules(&map).is_err());
        let mut map = fixture(0.002, 0.08);
        map.insert(0x1000, f32::NAN.to_le_bytes().to_vec());
        assert!(rules(&map).is_err());
        let mut map = fixture(0.002, 0.08);
        map.insert(0x2000, 0f64.to_le_bytes().to_vec());
        assert!(rules(&map).is_err());
        let mut map = fixture(0.002, 0.08);
        map.remove(&0x629277);
        assert!(rules(&map).is_err());
        assert!(
            Rules::reviewed_legacy(&[], origin())
                .unwrap_err()
                .contains("identity")
        );
    }
    #[test]
    #[ignore = "requires explicit PEACECRAFT3_DIALOG_LAYOUT_IMAGE containing the qualified local client"]
    fn local_original_dialog_layout_reads_qualified_operands() {
        let path =
            std::env::var_os("PEACECRAFT3_DIALOG_LAYOUT_IMAGE").expect("explicit local client");
        let image = std::fs::read(&path).unwrap();
        let rules = Rules::reviewed_legacy(
            &image,
            AssetIdentity {
                source: std::path::PathBuf::from(path).display().to_string(),
                edition: "selected local legacy".into(),
                version: "PE VERSIONINFO 1.29.2.9231".into(),
            },
        )
        .unwrap();
        assert_eq!(rules.image_sha256, sha256(&image));
        let g = rules.geometry(Some(0.015), 0.035, 2).unwrap();
        assert!(g.height > 0. && g.first_button_top > g.caption_top);
    }
}
