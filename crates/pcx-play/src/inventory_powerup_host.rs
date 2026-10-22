//! Hero strength changes that move life pools by the current/maximum ratio.
//! Native path: STR `b3dd30` -> `b3fd00` -> mode 1 `6be8c0`, which keeps the current/max ratio.
//! Unlike `SetHeroLevel` mode 2, the current pool is not set to the new absolute missing amount.
use super::inventory_powerup::StrengthEffect;
use crate::player_handicap::LifeRounding;

/// One hero's retained permanent strength plus its life pools after the grant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grant {
    pub strength: i32,
    pub life: f32,
    pub max_life: f32,
}

/// Legacy half-up rounding for paths that have not yet admitted the selected
/// unit-pool rounder. New health and strength paths pass `LifeRounding` instead.
pub(super) fn round_life(value: f32) -> f32 {
    (value + 0.5).floor()
}

/// One health component after the owner's handicap factor. The native unit-pool
/// callback rounds the scaled component before it computes the pool delta.
pub(super) fn scale_life_component(value: f32, factor: f32, rounding: Option<LifeRounding>) -> f32 {
    if factor == 1.0 {
        value
    } else {
        round_scaled_life_delta(value, factor, rounding)
    }
}

/// Strength-item and powerup deltas round even at the identity factor. Creation's
/// unscaled base HP uses `scale_life_component` and keeps its distinct contract.
pub(super) fn round_scaled_life_delta(
    value: f32,
    factor: f32,
    rounding: Option<LifeRounding>,
) -> f32 {
    match rounding {
        Some(rounding) => rounding.round(value * factor),
        None => round_life(value * factor),
    }
}

/// Life pools after a signed strength change, keeping the current/max ratio.
/// `life_per_strength` is the selected `StrHitPointBonus` value. The pool change and the retained
/// ratio are rounded separately, as the original does.
pub fn reprice_life(
    life: f32,
    max_life: f32,
    life_per_strength: f32,
    strength_delta: i32,
) -> Result<(f32, f32), String> {
    reprice_life_scaled(life, max_life, life_per_strength, strength_delta, 1.0, None)
}

/// Strength life change after the owner's health handicap. `factor` scales the
/// strength hit-point component, not flat held maximum-life bonuses.
pub(crate) fn reprice_life_scaled(
    life: f32,
    max_life: f32,
    life_per_strength: f32,
    strength_delta: i32,
    factor: f32,
    rounding: Option<LifeRounding>,
) -> Result<(f32, f32), String> {
    let fail = |why: &str| format!("strength life change: {why}");
    if !life.is_finite()
        || !max_life.is_finite()
        || !life_per_strength.is_finite()
        || !factor.is_finite()
    {
        return Err(fail("non-finite life value"));
    }
    let change =
        round_scaled_life_delta(strength_delta as f32 * life_per_strength, factor, rounding);
    change_life_pools(life, max_life, change).map_err(|e| fail(&e))
}

/// Life pools after a signed maximum-life change. The new current life is the retained ratio
/// rounded with half upward.
pub(crate) fn reprice_max_life(
    life: f32,
    max_life: f32,
    max_life_delta: i32,
) -> Result<(f32, f32), String> {
    change_life_pools(life, max_life, max_life_delta as f32)
        .map_err(|e| format!("max life change: {e}"))
}

/// Mana pools after a signed intelligence change, keeping the current/max ratio rounded with
/// half upward. `mana_per_intelligence` is the selected intelligence-to-mana coefficient.
pub(crate) fn reprice_mana(
    mana: f32,
    max_mana: f32,
    mana_per_intelligence: f32,
    intelligence_delta: i32,
) -> Result<(f32, f32), String> {
    if !mana_per_intelligence.is_finite() {
        return Err("intelligence mana change: non-finite mana per intelligence".into());
    }
    let change = round_life(intelligence_delta as f32 * mana_per_intelligence);
    change_mana_pools(mana, max_mana, change).map_err(|e| format!("intelligence mana change: {e}"))
}

/// Final pools prepared by a projection: a living holder keeps at least one hit point, while a
/// dead holder stays at zero. The write stores the prepared projection as-is, so a pool whose
/// delta was zero is not re-clamped.
pub(crate) fn clamp_life_pools(life: f32, max_life: f32, dead: bool) -> (f32, f32) {
    let life = if dead {
        0.0
    } else {
        life.max(1.0).min(max_life)
    };
    (life, max_life)
}

/// Mana has no living floor: a holder may keep exactly zero current mana, including after a pool
/// change. Death does not clear mana, so this clamp is independent of the combat state.
pub(crate) fn clamp_mana_pools(mana: f32, max_mana: f32) -> (f32, f32) {
    (mana.max(0.0).min(max_mana), max_mana)
}

fn change_life_pools(life: f32, max_life: f32, change: f32) -> Result<(f32, f32), String> {
    if !life.is_finite() || !max_life.is_finite() || !change.is_finite() {
        return Err("non-finite life value".into());
    }
    if max_life <= 0.0 || !(0.0..=max_life).contains(&life) {
        return Err("life pools must satisfy 0 <= life <= max_life with max_life > 0".into());
    }
    let new_max = max_life + change;
    if !new_max.is_finite() || new_max <= 0.0 {
        return Err("new maximum life is not positive".into());
    }
    Ok((round_life(new_max * (life / max_life)), new_max))
}

fn change_mana_pools(mana: f32, max_mana: f32, change: f32) -> Result<(f32, f32), String> {
    if !mana.is_finite() || !max_mana.is_finite() || !change.is_finite() {
        return Err("non-finite mana value".into());
    }
    if max_mana < 0.0 || !(0.0..=max_mana).contains(&mana) {
        return Err("mana pools must satisfy 0 <= mana <= max_mana".into());
    }
    let new_max = max_mana + change;
    if !new_max.is_finite() || new_max < 0.0 {
        return Err("new maximum mana is negative".into());
    }
    let ratio = if max_mana > 0.0 { mana / max_mana } else { 0.0 };
    Ok((round_life(new_max * ratio).min(new_max), new_max))
}

#[derive(Clone, Copy)]
pub struct HealthScale {
    pub handicap: f32,
    pub rounding: Option<LifeRounding>,
}

/// Applies `effect` to a hero's retained strength and life pools, or refuses without changes.
pub fn grant_strength(
    retained: i32,
    life: f32,
    max_life: f32,
    life_per_strength: f32,
    effect: StrengthEffect,
    dead: bool,
    scale: HealthScale,
) -> Result<Grant, String> {
    let strength = retained
        .checked_add(effect.strength())
        .ok_or_else(|| "powerup strength grant: retained strength overflows".to_string())?;
    let (life, max_life) = reprice_life_scaled(
        life,
        max_life,
        life_per_strength,
        effect.strength(),
        scale.handicap,
        scale.rounding,
    )
    .map_err(|e| format!("powerup strength grant: {e}"))?;
    let (life, max_life) = clamp_life_pools(life, max_life, dead);
    Ok(Grant {
        strength,
        life,
        max_life,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(strength: i32) -> StrengthEffect {
        StrengthEffect { strength }
    }

    fn scale() -> HealthScale {
        HealthScale {
            handicap: 1.0,
            rounding: None,
        }
    }

    #[test]
    fn percentage_of_life_is_kept_when_maximum_grows() {
        let grant = grant_strength(0, 50.0, 100.0, 25.0, effect(2), false, scale()).unwrap();
        assert_eq!(
            grant,
            Grant {
                strength: 2,
                life: 75.0,
                max_life: 150.0
            }
        );
    }

    #[test]
    fn retained_strength_accumulates_across_grants() {
        let first = grant_strength(0, 100.0, 100.0, 25.0, effect(1), false, scale()).unwrap();
        let second = grant_strength(
            first.strength,
            first.life,
            first.max_life,
            25.0,
            effect(1),
            false,
            scale(),
        )
        .unwrap();
        assert_eq!(
            second,
            Grant {
                strength: 2,
                life: 150.0,
                max_life: 150.0
            }
        );
    }

    #[test]
    fn zero_strength_keeps_pools_and_refuses_bad_pools_atomically() {
        assert_eq!(
            grant_strength(3, 40.0, 80.0, 25.0, effect(0), false, scale()).unwrap(),
            Grant {
                strength: 3,
                life: 40.0,
                max_life: 80.0
            }
        );
        assert!(grant_strength(0, 90.0, 80.0, 25.0, effect(1), false, scale()).is_err());
        assert!(grant_strength(0, 10.0, 0.0, 25.0, effect(1), false, scale()).is_err());
        assert!(grant_strength(0, f32::NAN, 80.0, 25.0, effect(1), false, scale()).is_err());
        assert!(grant_strength(i32::MAX, 10.0, 80.0, 25.0, effect(1), false, scale()).is_err());
        assert!(grant_strength(0, 10.0, 80.0, -100.0, effect(1), false, scale()).is_err());
    }

    #[test]
    fn removing_strength_scales_life_back_and_refuses_a_nonpositive_maximum() {
        assert_eq!(reprice_life(75.0, 150.0, 25.0, -2).unwrap(), (50.0, 100.0));
        assert!(reprice_life(10.0, 100.0, 25.0, -4).is_err());
    }

    #[test]
    fn intelligence_reprices_mana_with_half_up_rounding_and_a_zero_floor() {
        assert_eq!(reprice_mana(1.0, 4.0, 2.0, 1).unwrap(), (2.0, 6.0));
        assert_eq!(reprice_mana(0.0, 4.0, 2.0, 1).unwrap(), (0.0, 6.0));
        assert_eq!(reprice_mana(6.0, 6.0, 2.0, -1).unwrap(), (4.0, 4.0));
        assert!(reprice_mana(1.0, 4.0, f32::NAN, 1).is_err());
        assert!(reprice_mana(1.0, 4.0, 2.0, i32::MIN).is_err());
    }

    #[test]
    fn mana_clamp_keeps_zero_and_never_touches_dead_state() {
        assert_eq!(clamp_mana_pools(-1.0, 100.0), (0.0, 100.0));
        assert_eq!(clamp_mana_pools(25.0, 100.0), (25.0, 100.0));
        assert_eq!(clamp_mana_pools(125.0, 100.0), (100.0, 100.0));
        assert_eq!(clamp_mana_pools(0.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn grants_clamp_tiny_living_life_to_one_and_dead_life_to_zero() {
        let living = grant_strength(0, 0.1, 100.0, 25.0, effect(1), false, scale()).unwrap();
        assert_eq!(living.life, 1.0);
        let dead = grant_strength(0, 0.1, 100.0, 25.0, effect(1), true, scale()).unwrap();
        assert_eq!(dead.life, 0.0);
    }
}
