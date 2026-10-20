//! Shared resolved-field hero growth for current combat and vital consumers.
use super::ObjectHost;
use pcx_sim::world::Definition;
impl ObjectHost {
    pub(crate) fn derive_hero_attribute(
        definition: &Definition,
        progress: Option<(i32, i32)>,
        name: &str,
    ) -> Result<f32, String> {
        let (initial, current) =
            progress.ok_or("hero attributes require admitted initial and current levels")?;
        let gained = current
            .checked_sub(initial)
            .filter(|n| *n >= 0)
            .ok_or("hero attribute level is below the admitted initial level")?;
        let base = super::super::super::unit_activity::balance_real(definition, name)?;
        let growth =
            super::super::super::unit_activity::balance_real(definition, &format!("{name}plus"))?;
        let value = (base + growth * gained as f32).floor();
        if !value.is_finite() || value < 0.0 {
            return Err(format!("invalid derived hero attribute {name}"));
        }
        Ok(value)
    }
}
