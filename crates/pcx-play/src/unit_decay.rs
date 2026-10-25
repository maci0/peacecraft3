//! Source-derived corpse eligibility and logical decay deadlines.
use crate::{neutral_player::sha256, pe_image::PeSections};

#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: [u8; 32],
    pub sources: Vec<String>,
    pub(crate) raise_mask: u32,
    pub(crate) decay_mask: u32,
    pub(crate) hero_range: [u8; 2],
    pub(crate) flesh: f64,
    pub(crate) bone: f64,
    pub(crate) structure: f64,
    pub(crate) dissipate: f64,
}
impl Rules {
    pub fn read(
        image: &[u8],
        profiles: &[(&str, &[u8])],
        classifier: &crate::campaign_targeting_policy::TargetingPolicy,
        max_bytes: usize,
    ) -> Result<Self, String> {
        let image_sha256 = sha256(image);
        if image.len() > 32 << 20
            || image_sha256 != crate::menu_lighting::REVIEWED_IMAGE
            || classifier.image_sha256 != image_sha256
        {
            return Err("unit decay client/classifier identity".into());
        }
        let pe = PeSections::parse(image)?;
        let bytes = pe
            .va_bytes(0x66f1bc, 62)
            .ok_or("unit death-type constructor")?;
        let [raise_mask, decay_mask] = Self::masks(bytes)?;
        let ([flesh, bone, structure, dissipate], sources) = Self::durations(profiles, max_bytes)?;
        Ok(Self {
            image_sha256,
            sources,
            raise_mask,
            decay_mask,
            hero_range: [classifier.hero_first, classifier.hero_last],
            flesh,
            bone,
            structure,
            dissipate,
        })
    }
    fn durations(
        profiles: &[(&str, &[u8])],
        max_bytes: usize,
    ) -> Result<([f64; 4], Vec<String>), String> {
        let ([flesh, bone, structure, dissipate], sources) =
            pcx_objects::editor::misc::misc_scalars(
                profiles,
                [
                    "DecayTime",
                    "BoneDecayTime",
                    "StructureDecayTime",
                    "DissipateTime",
                ],
                max_bytes,
            )?;
        let duration = |value: Option<f32>| {
            value
                .filter(|v| *v >= 0.)
                .map(f64::from)
                .ok_or("missing or negative unit decay duration")
        };
        Ok((
            [
                duration(flesh)?,
                duration(bone)?,
                duration(structure)?,
                duration(dissipate)?,
            ],
            sources,
        ))
    }
    fn masks(bytes: &[u8]) -> Result<[u32; 2], String> {
        let b = bytes.get(..62).ok_or("truncated death-type constructor")?;
        // Both TESTs read the same descriptor's deathType; the branches write unit raise/decay flags.
        if b[..6] != [0xf6, 0x86, 0x9c, 1, 0, 0]
            || b[7..20]
                != [
                    0x74, 12, 0x81, 0xa3, 0x64, 1, 0, 0, 0xff, 0xef, 0xff, 0xff, 0xeb,
                ]
            || b[20..31] != [10, 0x81, 0x8b, 0x64, 1, 0, 0, 0, 0x10, 0, 0]
            || b[31..37] != b[..6]
            || b[38..]
                != [
                    0x74, 12, 0x81, 0x8b, 0x64, 1, 0, 0, 0, 2, 0, 0, 0xeb, 10, 0x81, 0xa3, 0x64, 1,
                    0, 0, 0xff, 0xfd, 0xff, 0xff,
                ]
        {
            return Err("unit death-type branch relationship".into());
        }
        let masks = [u32::from(b[6]), u32::from(b[37])];
        if masks.iter().any(|v| !v.is_power_of_two()) || masks[0] & masks[1] != 0 {
            return Err("unit death-type masks".into());
        }
        Ok(masks)
    }
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        Self {
            image_sha256: [0x42; 32],
            sources: vec!["synthetic decay rules".into()],
            raise_mask: 4,
            decay_mask: 8,
            hero_range: *b"HK",
            flesh: 0.75,
            bone: 1.25,
            structure: 2.5,
            dissipate: 0.625,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Alive,
    Dying,
    Flesh,
    Bone,
    Dissipating,
    Remains,
    Hidden,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Unit,
    Structure,
    Hero,
}
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub(crate) kind: Kind,
    pub(crate) death: f64,
    pub(crate) raises: bool,
    pub(crate) decays: bool,
    pub(crate) phase: Phase,
    pub(crate) deadline: Option<f64>,
    pub(crate) remaining: f64,
    pub(crate) suspended: bool,
}
impl State {
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn suspended(&self) -> bool {
        self.suspended
    }
    pub fn deadline(&self) -> Option<f64> {
        self.deadline
    }
    pub(crate) fn begin(&mut self, now: f64) -> Result<(), String> {
        self.phase = Phase::Dying;
        self.suspended = false;
        self.schedule(now, self.death)
    }
    pub(crate) fn schedule(&mut self, now: f64, duration: f64) -> Result<(), String> {
        let deadline = now + duration;
        if !now.is_finite() || !deadline.is_finite() || now < 0. || duration < 0. {
            return Err("unit decay clock overflow".into());
        }
        self.remaining = duration;
        self.deadline = Some(deadline);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn masks_are_operand_driven_and_truncated_or_mutated_branches_refuse() {
        let mut bytes = vec![
            0xf6, 0x86, 0x9c, 1, 0, 0, 4, 0x74, 12, 0x81, 0xa3, 0x64, 1, 0, 0, 0xff, 0xef, 0xff,
            0xff, 0xeb, 10, 0x81, 0x8b, 0x64, 1, 0, 0, 0, 0x10, 0, 0, 0xf6, 0x86, 0x9c, 1, 0, 0, 8,
            0x74, 12, 0x81, 0x8b, 0x64, 1, 0, 0, 0, 2, 0, 0, 0xeb, 10, 0x81, 0xa3, 0x64, 1, 0, 0,
            0xff, 0xfd, 0xff, 0xff,
        ];
        assert_eq!(Rules::masks(&bytes).unwrap(), [4, 8]);
        for i in 0..bytes.len() {
            assert!(Rules::masks(&bytes[..i]).is_err());
        }
        for i in [0, 7, 8, 31, 38, 39, 51] {
            let mut b = bytes.clone();
            b[i] ^= 1;
            assert!(Rules::masks(&b).is_err());
        }
        bytes[37] = 4;
        assert!(Rules::masks(&bytes).is_err());
    }
    #[test]
    fn nonstock_profiles_override_independently_and_bad_or_missing_values_refuse() {
        let base=b"[Misc]\nDecayTime=0.625\nBoneDecayTime=3.75\nStructureDecayTime=4.125\nDissipateTime=0.875\n";
        let edit = b"[Misc]\nBoneDecayTime=7.5\n";
        let (values, sources) = Rules::durations(&[("stock", base), ("map", edit)], 4096).unwrap();
        assert_eq!(values, [0.625, 7.5, 4.125, 0.875]);
        assert_eq!(sources, ["stock", "map"]);
        assert!(Rules::durations(&[("stock", base)], 4).is_err());
        for bad in [
            "",
            "[Misc]\nDecayTime=NaN",
            "[Misc]\nDecayTime=-1",
            "[Misc]\nDecayTime=1\nDecayTime=2",
        ] {
            let profiles = if bad.is_empty() {
                vec![("missing", bad.as_bytes())]
            } else {
                vec![("stock", base.as_slice()), ("map", bad.as_bytes())]
            };
            assert!(Rules::durations(&profiles, 4096).is_err());
        }
    }
    #[test]
    #[ignore = "requires locally supplied PCX_WAR3_EXE and PCX_UNIT_MISC"]
    fn reviewed_source_decodes() {
        let image = std::fs::read(std::env::var("PCX_WAR3_EXE").unwrap()).unwrap();
        let misc = std::fs::read(std::env::var("PCX_UNIT_MISC").unwrap()).unwrap();
        let classifier = crate::campaign_targeting_policy::TargetingPolicy::read(
            &image,
            crate::neutral_player::RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 8 << 20,
                sections: 96,
            },
        )
        .unwrap();
        let rules = Rules::read(
            &image,
            &[("selected Units/MiscData.txt", &misc)],
            &classifier,
            8 << 20,
        )
        .unwrap();
        assert_eq!([rules.raise_mask, rules.decay_mask], [1, 2]);
        assert!(rules.flesh > 0. && rules.bone > rules.flesh);
    }
}
