//! Reviewed Legacy client bindings for normal campaign preparation.
//! Only instruction locations and image identity are fixed here. All game values
//! and names are decoded by the existing readers from the selected source bytes.
use crate::{
    camera_margins::{CameraMarginBinding, CameraMarginBudget, CameraMargins, SourceWindow},
    hero_skills,
    neutral_player::{RuleInputBudget, sha256},
    neutral_rule_input::Pe,
    player_handicap, script_numeric,
    script_random::profile,
    scripts::AssetIdentity,
    startup_clock,
};
use pcx_objects::editor::hero::Progression;

#[path = "campaign_fresh_settings.rs"]
mod fresh;
pub use fresh::{FreshSettings, FreshTriggers, fresh_settings, fresh_triggers};

pub struct Inputs {
    pub camera: CameraMargins,
    pub numeric: script_numeric::NumericPolicy,
    pub random: profile::Policy,
    pub clock: startup_clock::StartupClock,
    pub experience: player_handicap::Rules,
    pub hero: hero_skills::Rules,
}

const BUDGET: RuleInputBudget = RuleInputBudget {
    image_bytes: 32 << 20,
    profile_bytes: 64 << 10,
    sections: 96,
};

pub fn unit_activity(
    image: &[u8],
    origin: AssetIdentity,
    registry: &crate::order_registry::Registry,
) -> Result<crate::unit_activity::Rules, String> {
    if image.len() > BUDGET.image_bytes || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
        return Err(format!("{}: unsupported activity client", origin.source));
    }
    let pe = Pe::parse(image, BUDGET).map_err(|e| e.to_string())?;
    let window = |va, len| -> Result<SourceWindow, String> {
        let bytes = pe
            .read_from(va, true)
            .map_err(|e| e.to_string())?
            .get(..len as usize)
            .ok_or("truncated activity comparison")?;
        Ok(SourceWindow {
            va,
            len,
            digest: sha256(bytes),
        })
    };
    crate::unit_activity::Rules::read(
        image,
        crate::unit_activity::Binding {
            origin,
            image_sha256: crate::menu_lighting::REVIEWED_IMAGE,
            hold: window(0xb64316, 5)?,
            stop: window(0x6a16c6, 6)?,
            movement: window(0xb64586, 5)?,
            patrol: window(0xb64594, 5)?,
        },
        registry,
        BUDGET,
    )
}

pub fn order_registry(
    image: &[u8],
    origin: AssetIdentity,
) -> Result<crate::order_registry::Registry, String> {
    if image.len() > BUDGET.image_bytes || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
        return Err(format!(
            "{}: unsupported order-registry client",
            origin.source
        ));
    }
    let pe = Pe::parse(image, BUDGET).map_err(|e| e.to_string())?;
    let va = 0xb11550;
    let len = 0xb12c75 - va;
    let code = pe
        .read_from(va, true)
        .map_err(|e| e.to_string())?
        .get(..len as usize)
        .ok_or("truncated order registry")?;
    crate::order_registry::Registry::read(
        image,
        crate::order_registry::Binding {
            origin,
            image_sha256: crate::menu_lighting::REVIEWED_IMAGE,
            initializer: SourceWindow {
                va,
                len,
                digest: sha256(code),
            },
            bootstrap: SourceWindow {
                va: 0x47abbf,
                len: 21,
                digest: sha256(
                    pe.read_from(0x47abbf, true)
                        .map_err(|e| e.to_string())?
                        .get(..21)
                        .ok_or("truncated order bootstrap")?,
                ),
            },
            direct_register: 0x47c4a0,
            register: 0x47c550,
        },
        BUDGET,
        4096,
        1024,
    )
}

pub fn placement_rules(
    image: &[u8],
    origin: AssetIdentity,
) -> Result<crate::unit_placement::Rules, String> {
    if image.len() > BUDGET.image_bytes || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
        return Err(format!("{}: unsupported placement client", origin.source));
    }
    let pe = Pe::parse(image, BUDGET).map_err(|e| e.to_string())?;
    let window = |va, len| -> Result<SourceWindow, String> {
        let bytes = pe
            .read_from(va, true)
            .map_err(|e| e.to_string())?
            .get(..len as usize)
            .ok_or("truncated placement role")?;
        Ok(SourceWindow {
            va,
            len,
            digest: sha256(bytes),
        })
    };
    crate::unit_placement::Rules::read(
        image,
        crate::unit_placement::Binding {
            origin,
            image_sha256: crate::menu_lighting::REVIEWED_IMAGE,
            normalization: window(0x8002d1, 5)?,
            initial_position: window(0x8baf2d, 5)?,
            small_radius: window(0x8bc929, 8)?,
            large_radius: window(0x8bc966, 8)?,
            one_initializer: window(0x401460, 16)?,
            two_initializer: window(0x401560, 16)?,
            ground_mask: window(0x67cebc, 5)?,
            fly_mask: window(0x67cec3, 5)?,
        },
        BUDGET,
    )
    .map_err(|e| e.to_string())
}

pub fn read(
    image: &[u8],
    origin: AssetIdentity,
    camera_config: &[u8],
    camera_origin: AssetIdentity,
    progression: Progression,
) -> Result<Inputs, String> {
    if image.len() > BUDGET.image_bytes || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
        return Err(format!(
            "{}: unsupported campaign client; no source rules substituted",
            origin.source
        ));
    }
    let pe = Pe::parse(image, BUDGET).map_err(|e| format!("{}: {e}", origin.source))?;
    let window = |va: u32, len: u32, code: bool| -> Result<SourceWindow, String> {
        let bytes = pe
            .read_from(va, code)
            .map_err(|e| e.to_string())?
            .get(..len as usize)
            .ok_or_else(|| format!("{}: truncated reviewed window {va:#x}", origin.source))?;
        Ok(SourceWindow {
            va,
            len,
            digest: sha256(bytes),
        })
    };
    // The complete-image fingerprint qualifies these reviewed role locations.
    let image_sha256 = crate::menu_lighting::REVIEWED_IMAGE;
    let math_sub = window(0x9a00b0, 264, true)?;
    let math_add = window(0x9a01c0, 262, true)?;
    let math_mul = window(0x99ffe0, 195, true)?;
    let math_lzcnt = window(0x9a0330, 29, true)?;
    let math_i32_to_real = window(0x9a1620, 92, true)?;
    let math = crate::script_math::Binding {
        image_sha256,
        square_root_native: window(0x4ac1a0, 94, true)?,
        arctangent2_native: window(0x489fd0, 126, true)?,
        square_root: window(0x9a20a0, 164, true)?,
        integer_root: window(0x9a2150, 61, true)?,
        arctangent2: window(0x9a0cf0, 161, true)?,
        arctangent: window(0x9a0da0, 423, true)?,
        zero_initializer: window(0x401300, 16, true)?,
        one_initializer: window(0x401460, 16, true)?,
    };
    let numeric = script_numeric::NumericPolicy::read(
        image,
        script_numeric::Binding {
            origin: origin.clone(),
            image_sha256,
            literal: window(0x8df660, 238, true)?,
            division: window(0x9a02d0, 57, true)?,
            reciprocal: window(0x9a1d00, 103, true)?,
            reciprocal_table: window(0xed6cf0, 4100, false)?,
            equality: window(0x8d8d10, 53, true)?,
            equality_constant: window(0x10db054, 4, false)?,
            inequality: window(0x8d7e32, 162, true)?,
            math_sub,
            math_add,
            math_mul,
            math_lzcnt,
            math_i32_to_real,
        },
        BUDGET,
        4096,
    )
    .and_then(|numeric| numeric.with_math(image, &math, BUDGET))
    .map_err(|e| e.to_string())?;
    let random = profile::Policy::read(
        image,
        profile::Binding {
            origin: origin.clone(),
            image_sha256,
            seed: window(0x9892a0, 119, true)?,
            draw: window(0x480dd0, 169, true)?,
            integer: window(0x494e40, 60, true)?,
            real: window(0x494e80, 177, true)?,
            set_seed: window(0x4a99a0, 39, true)?,
            secondary: window(0x69a0d0, 251, true)?,
            real_constant_init: window(0x4017c0, 16, true)?,
            math_sub,
            math_add,
            math_mul,
            math_lzcnt,
            math_i32_to_real,
        },
        BUDGET,
    )
    .map_err(|e| e.to_string())?;
    let camera = CameraMargins::read(
        image,
        CameraMarginBinding {
            origin: origin.clone(),
            image_sha256,
            section: window(0xe6eb40, 14, false)?,
            keys: [
                window(0xe6eb38, 5, false)?,
                window(0xe6eb50, 6, false)?,
                window(0xe6eb58, 4, false)?,
                window(0xe6eb5c, 7, false)?,
            ],
            scale: window(0xe78348, 4, false)?,
        },
        camera_config,
        camera_origin,
        CameraMarginBudget {
            image: BUDGET,
            config_bytes: 2 << 20,
        },
    )
    .map_err(|e| e.to_string())?;
    let clock = startup_clock::StartupClock::read(
        image,
        startup_clock::Binding {
            origin: origin.clone(),
            image_sha256,
            initializer: window(0x401300, 16, true)?,
            scale_initializer: window(0x401460, 16, true)?,
        },
        BUDGET,
    )
    .map_err(|e| e.to_string())?;
    let experience = player_handicap::Rules::read(
        image,
        player_handicap::Binding {
            origin: origin.clone(),
            image_sha256,
            setter: window(0x4a9660, 97, true)?,
            getter: window(0x4945c0, 33, true)?,
            maximum_initializer: window(0x401350, 16, true)?,
            null_initializer: window(0x401460, 16, true)?,
        },
        &numeric,
        BUDGET,
    )
    .map_err(|e| e.to_string())?;
    let hero = hero_skills::Rules::read(
        image,
        hero_skills::Binding {
            origin: origin.clone(),
            image_sha256,
            level_initializer: window(0xb3d7e5, 7, true)?,
            level_query: window(0x691920, 20, true)?,
            points_initializer: window(0xb3f1b8, 21, true)?,
            slot_limit: window(0xb3f135, 5, true)?,
            first_rank: window(0xb3f5b1, 16, true)?,
            hero_comparison: window(0x6d659d, 8, true)?,
            hero_handler: window(0x6d65b8, 11, true)?,
            other_implicit: [
                0x6d6577, 0x6d6581, 0x6d658b, 0x6d6593, 0x6d65a5, 0x6d65c3, 0x6d65cd, 0x6d65d5,
                0x6d65f3,
            ]
            .into_iter()
            .map(|va| window(va, 6, true))
            .collect::<Result<_, _>>()?,
        },
        progression,
        BUDGET,
    )?;
    Ok(Inputs {
        camera,
        numeric,
        random,
        clock,
        experience,
        hero,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic client".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        }
    }
    #[test]
    fn unsupported_client_cannot_substitute_source_rules() {
        let progression = Progression::read(
            &[(
                "fixture",
                b"[Misc]\nMaxHeroLevel=21\nHeroAbilityLevelSkip=4\n",
            )],
            1024,
        )
        .unwrap();
        let error = read(
            b"MZ unsupported",
            identity(),
            b"[Camera]\n",
            identity(),
            progression,
        )
        .err()
        .expect("unsupported client");
        assert!(error.contains("unsupported campaign client"));
    }
}
