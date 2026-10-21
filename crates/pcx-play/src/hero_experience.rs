//! Source-bound XP counters and required-level thresholds.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub xp: i32,
    pub suspended: bool,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub(crate) initial: State,
    pub(crate) thresholds: pcx_objects::editor::hero_experience::Experience,
}
impl Rules {
    pub(crate) fn read(
        image: &[u8],
        origin: AssetIdentity,
        thresholds: pcx_objects::editor::hero_experience::Experience,
    ) -> Result<Self, String> {
        let digest = sha256(image);
        if image.len() > 32 << 20 || digest != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("unsupported XP client identity".into());
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| e.to_string())?;
        Self::decode(origin, digest, thresholds, |va, n| {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "truncated XP instruction window".into())
        })
    }
    fn decode(
        origin: AssetIdentity,
        digest: Sha256Digest,
        thresholds: pcx_objects::editor::hero_experience::Experience,
        mut read: impl FnMut(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let xp = read(0xb3d829, 10)?;
        let suspended = read(0xb3d924, 10)?;
        let setter = read(0x69f4ad, 9)?;
        let getter = read(0x69378b, 7)?;
        if xp.len() != 10
            || suspended.len() != 10
            || setter.len() != 9
            || getter.len() != 7
            || xp[..6] != [0xc7, 0x86, 0x90, 0, 0, 0]
            || suspended[..6] != [0xc7, 0x86, 0xf0, 0, 0, 0]
            || setter != [0x8b, 0x45, 8, 0x89, 0x81, 0xf0, 0, 0, 0]
            || getter != [0x8b, 0x80, 0x90, 0, 0, 0, 0xc3]
        {
            return Err("unsupported XP field/initializer instructions".into());
        }
        let xp = i32::from_le_bytes(xp[6..].try_into().map_err(|_| "XP initializer")?);
        let suspended = i32::from_le_bytes(
            suspended[6..]
                .try_into()
                .map_err(|_| "XP suspension initializer")?,
        );
        if xp < 0 || !(0..=1).contains(&suspended) {
            return Err("invalid XP initial state".into());
        }
        Ok(Self {
            origin,
            image_sha256: digest,
            initial: State {
                xp,
                suspended: suspended != 0,
            },
            thresholds,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn rules() -> Rules {
        let thresholds=pcx_objects::editor::hero_experience::Experience::read(&[("synthetic",b"[Misc]\nNeedHeroXP=17\nNeedHeroXPFormulaA=1\nNeedHeroXPFormulaB=7\nNeedHeroXPFormulaC=0")],1,11,1024,20).unwrap();
        Rules {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "1".into(),
            },
            image_sha256: sha256(&[]),
            initial: State {
                xp: 0,
                suspended: false,
            },
            thresholds,
        }
    }
    #[test]
    fn initializer_values_follow_image_operands_and_shapes_refuse() {
        let r = rules();
        let windows = |va, _| {
            Ok(match va {
                0xb3d829 => vec![0xc7, 0x86, 0x90, 0, 0, 0, 7, 0, 0, 0],
                0xb3d924 => vec![0xc7, 0x86, 0xf0, 0, 0, 0, 1, 0, 0, 0],
                0x69f4ad => vec![0x8b, 0x45, 8, 0x89, 0x81, 0xf0, 0, 0, 0],
                0x69378b => vec![0x8b, 0x80, 0x90, 0, 0, 0, 0xc3],
                _ => return Err("unknown window".into()),
            })
        };
        let got = Rules::decode(
            r.origin.clone(),
            r.image_sha256,
            r.thresholds.clone(),
            windows,
        )
        .unwrap();
        assert_eq!(
            got.initial,
            State {
                xp: 7,
                suspended: true
            }
        );
        assert!(
            Rules::decode(
                r.origin.clone(),
                r.image_sha256,
                r.thresholds.clone(),
                |_, _| Ok(vec![])
            )
            .is_err()
        );
        assert!(
            Rules::decode(r.origin, r.image_sha256, r.thresholds, |_, n| Ok(vec![
                0;
                n
            ]))
            .is_err()
        );
    }
}
