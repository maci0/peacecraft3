use super::ObjectHost;
use pcx_script::checkpoint::{Reader, Writer};

pub(crate) struct PreparedUnitRandom(Option<crate::unit_animation_rarity::State>);

impl ObjectHost {
    pub(crate) fn write_unit_random_checkpoint(&self, out: &mut Writer) -> Result<(), String> {
        match self.units.as_ref().map(|units| &units.rarity) {
            None => out.byte(0),
            Some(None) => out.byte(1),
            Some(Some(Err(error))) => {
                out.byte(2)?;
                out.text(error)
            }
            Some(Some(Ok(state))) => {
                out.byte(3)?;
                state.write_checkpoint(out)
            }
        }
    }
    pub(crate) fn prepare_unit_random_checkpoint(
        &self,
        input: &mut Reader<'_>,
    ) -> Result<PreparedUnitRandom, String> {
        let state = match (
            input.byte()?,
            self.units.as_ref().map(|units| &units.rarity),
        ) {
            (0, None) | (1, Some(None)) => None,
            (2, Some(Some(Err(error)))) if input.text()? == *error => None,
            (3, Some(Some(Ok(state)))) => Some(state.read_checkpoint(input)?),
            _ => return Err("checkpoint unit random admission differs from its candidate".into()),
        };
        Ok(PreparedUnitRandom(state))
    }
    pub(crate) fn commit_unit_random_checkpoint(&mut self, prepared: PreparedUnitRandom) {
        if let Some(state) = prepared.0 {
            self.units
                .as_mut()
                .expect("prepared unit random admission")
                .rarity = Some(Ok(state));
        }
    }
}
