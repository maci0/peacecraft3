//! App-owned parsed context publication; no game execution or filepicker policy.
use pcx_play::map_admission::{
    MapAdmissionBudget, MapAdmissionError, MapAdmissionInputs, ParsedMapContext,
    prepare_map_context,
};
use pcx_sim::world::ObjectCatalog;
use std::sync::Arc;
#[derive(Default)]
pub struct MapAdmission {
    context: Option<Arc<ParsedMapContext>>,
}
impl MapAdmission {
    pub fn context(&self) -> Option<&ParsedMapContext> {
        self.context.as_deref()
    }
    pub fn shared_context(&self) -> Option<Arc<ParsedMapContext>> {
        self.context.as_ref().map(Arc::clone)
    }
    pub fn replace<C: ObjectCatalog>(
        &mut self,
        input: MapAdmissionInputs<'_, C>,
        budget: MapAdmissionBudget,
    ) -> Result<(), MapAdmissionError> {
        let context = prepare_map_context(input, budget)?;
        self.context = Some(Arc::new(context));
        Ok(())
    }
}
