//! Hero strength changes that move life pools by the current/maximum ratio.
//! Native path: STR `b3dd30` -> `b3fd00` -> mode 1 `6be8c0`, which keeps the current/max ratio.
//! Unlike `SetHeroLevel` mode 2, the current pool is not set to the new absolute missing amount.
use super::inventory_powerup::StrengthEffect;

/// One hero's retained permanent strength plus its life pools after the grant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grant {
    pub strength: i32,
    pub life: f32,
    pub max_life: f32,
}

/// Life pools after a signed strength change, keeping the current/max ratio.
/// `life_per_strength` is the selected `StrHitPointBonus` value.
pub fn reprice_life(
    life: f32,
    max_life: f32,
    life_per_strength: f32,
    strength_delta: i32,
) -> Result<(f32, f32), String> {
    let fail = |why: &str| format!("strength life change: {why}");
    if !life.is_finite() || !max_life.is_finite() || !life_per_strength.is_finite() {
        return Err(fail("non-finite life value"));
    }
    if max_life <= 0.0 || !(0.0..=max_life).contains(&life) {
        return Err(fail(
            "life pools must satisfy 0 <= life <= max_life with max_life > 0",
        ));
    }
    let new_max = max_life + strength_delta as f32 * life_per_strength;
    if !new_max.is_finite() || new_max <= 0.0 {
        return Err(fail("new maximum life is not positive"));
    }
    Ok((new_max * (life / max_life), new_max))
}

/// Applies `effect` to a hero's retained strength and life pools, or refuses without changes.
pub fn grant_strength(
    retained: i32,
    life: f32,
    max_life: f32,
    life_per_strength: f32,
    effect: StrengthEffect,
) -> Result<Grant, String> {
    let strength = retained
        .checked_add(effect.strength())
        .ok_or_else(|| "powerup strength grant: retained strength overflows".to_string())?;
    let (life, max_life) = reprice_life(life, max_life, life_per_strength, effect.strength())
        .map_err(|e| format!("powerup strength grant: {e}"))?;
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

    #[test]
    fn percentage_of_life_is_kept_when_maximum_grows() {
        let grant = grant_strength(0, 50.0, 100.0, 25.0, effect(2)).unwrap();
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
        let first = grant_strength(0, 100.0, 100.0, 25.0, effect(1)).unwrap();
        let second =
            grant_strength(first.strength, first.life, first.max_life, 25.0, effect(1)).unwrap();
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
            grant_strength(3, 40.0, 80.0, 25.0, effect(0)).unwrap(),
            Grant {
                strength: 3,
                life: 40.0,
                max_life: 80.0
            }
        );
        assert!(grant_strength(0, 90.0, 80.0, 25.0, effect(1)).is_err());
        assert!(grant_strength(0, 10.0, 0.0, 25.0, effect(1)).is_err());
        assert!(grant_strength(0, f32::NAN, 80.0, 25.0, effect(1)).is_err());
        assert!(grant_strength(i32::MAX, 10.0, 80.0, 25.0, effect(1)).is_err());
        assert!(grant_strength(0, 10.0, 80.0, -100.0, effect(1)).is_err());
    }

    #[test]
    fn removing_strength_scales_life_back_and_refuses_a_nonpositive_maximum() {
        assert_eq!(reprice_life(75.0, 150.0, 25.0, -2).unwrap(), (50.0, 100.0));
        assert!(reprice_life(10.0, 100.0, 25.0, -4).is_err());
    }
}
