//! Life restored by a healing item: the authored amount moves life up and never past maximum life.
//! A unit with no life left is refused; whether native healing revives or ignores a dead unit is
//! not qualified here.

/// Life after healing `amount`, clamped to `max_life`.
pub(crate) fn heal_life(life: f32, max_life: f32, amount: f32) -> Result<f32, String> {
    if !life.is_finite() || !max_life.is_finite() || !amount.is_finite() {
        return Err("heal inputs must be finite".into());
    }
    if max_life <= 0.0 || !(0.0..=max_life).contains(&life) {
        return Err("life must satisfy 0 <= life <= max_life with max_life > 0".into());
    }
    if amount < 0.0 {
        return Err("heal amount must be nonnegative".into());
    }
    if life <= 0.0 {
        return Err("dead unit healing is unqualified".into());
    }
    Ok((life + amount).min(max_life))
}

#[cfg(test)]
mod tests {
    use super::heal_life;

    #[test]
    fn heal_adds_the_amount_and_clamps_to_maximum() {
        assert_eq!(heal_life(100.0, 500.0, 250.0).unwrap(), 350.0);
        assert_eq!(heal_life(400.0, 500.0, 250.0).unwrap(), 500.0);
        assert_eq!(heal_life(500.0, 500.0, 250.0).unwrap(), 500.0);
    }

    #[test]
    fn heal_refuses_dead_units_and_invalid_pools_without_a_value() {
        assert!(
            heal_life(0.0, 500.0, 250.0)
                .unwrap_err()
                .contains("dead unit")
        );
        assert!(heal_life(600.0, 500.0, 1.0).is_err());
        assert!(heal_life(10.0, 0.0, 1.0).is_err());
        assert!(heal_life(10.0, 500.0, -1.0).is_err());
        assert!(heal_life(f32::NAN, 500.0, 1.0).is_err());
    }
}
