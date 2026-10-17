//! Start one admitted map on the production host after parsed delivery.
//! Browser source selection stays on `SelectedMapSources`. This system reads
//! the admitted context and caller-supplied host inputs only.
use crate::map_requests::{MapRequestSet, MapRequests};
use crate::map_script_session::{MapScriptSession, ScriptSessionError};
use bevy::prelude::*;
use pcx_play::map_admission::ParsedMapContext;
use pcx_play::map_startup_host::{
    self, MapStartupError, MapStartupHost, MapStartupInputs, PostConfigObservations,
};
use pcx_script::value::FunctionId;
use std::sync::{Arc, Weak};

struct AttemptKey {
    context: Weak<ParsedMapContext>,
    revision: u64,
}

#[derive(Resource, Default)]
pub struct CampaignStartup {
    attempted: Option<AttemptKey>,
    pub last_error: Option<StartupError>,
}

#[derive(Resource, Default)]
pub struct CampaignHostSupply {
    pub revision: u64,
    pub inputs: Option<MapStartupInputs>,
}

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("host inputs are for binding revision {supply}; the admitted revision is {binding}")]
    InputRevision { supply: u64, binding: u64 },
    #[error("binding revision {binding} has no campaign host inputs")]
    MissingInputs { binding: u64 },
    #[error("{name} is not declared by the admitted map script")]
    MissingEntrypoint { name: &'static str },
    #[error("{name} is not a map-script function that takes nothing and returns nothing")]
    EntrypointShape { name: &'static str },
    #[error(transparent)]
    Session(#[from] ScriptSessionError),
}

pub fn install(app: &mut App) {
    app.init_resource::<CampaignStartup>()
        .init_resource::<CampaignHostSupply>()
        .init_resource::<MapScriptSession<MapStartupHost>>()
        .add_systems(Update, start_admitted_map.after(MapRequestSet::Deliver));
}

fn entry_id(context: &ParsedMapContext, name: &'static str) -> Result<FunctionId, StartupError> {
    let program = context.scripts.program();
    let Some(function) = program.function(name) else {
        return Err(StartupError::MissingEntrypoint { name });
    };
    if function.source != "the map script"
        || !function.params.is_empty()
        || function.ret.kind.name() != "nothing"
    {
        return Err(StartupError::EntrypointShape { name });
    }
    program
        .function_id(name)
        .ok_or(StartupError::MissingEntrypoint { name })
}

fn start_admitted_map(
    requests: Res<MapRequests>,
    mut supply: ResMut<CampaignHostSupply>,
    mut startup: ResMut<CampaignStartup>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let Some(outcome) = requests.last_outcome.as_ref() else {
        return;
    };
    if outcome.result.is_err() {
        return;
    }
    let Some(bindings) = requests.bindings.as_ref() else {
        return;
    };
    let Some(context) = requests.admission.shared_context() else {
        return;
    };
    // File drops relabel the map source; exact receipts bind its backing source.
    if outcome.binding_revision != Some(bindings.revision)
        || outcome.identity.as_ref() != Some(&context.sources.map)
        || context.sources.map.edition != bindings.map_identity.edition
        || context.sources.map.version != bindings.map_identity.version
        || !requests
            .published_receipts
            .as_ref()
            .is_some_and(|receipts| Arc::ptr_eq(receipts, &bindings.receipts))
    {
        return;
    }
    let key = AttemptKey {
        context: Arc::downgrade(&context),
        revision: bindings.revision,
    };
    if startup.attempted.as_ref().is_some_and(|previous| {
        previous.revision == key.revision && Weak::ptr_eq(&previous.context, &key.context)
    }) {
        return;
    }
    if supply.revision != bindings.revision {
        startup.last_error = Some(StartupError::InputRevision {
            supply: supply.revision,
            binding: bindings.revision,
        });
        return;
    }
    let config = match entry_id(&context, "config") {
        Ok(id) => id,
        Err(error) => {
            startup.attempted = Some(key);
            startup.last_error = Some(error);
            return;
        }
    };
    let main = match entry_id(&context, "main") {
        Ok(id) => id,
        Err(error) => {
            startup.attempted = Some(key);
            startup.last_error = Some(error);
            return;
        }
    };
    let Some(inputs) = supply.inputs.take() else {
        startup.last_error = Some(StartupError::MissingInputs {
            binding: bindings.revision,
        });
        return;
    };
    let post = inputs.post_config.clone();
    let admitted = Arc::clone(&context);
    let result = session.replace_sequenced(
        Arc::clone(&context),
        move |ctx| map_startup_host::assemble(ctx, inputs),
        &[config, main],
        move |vm, completed| apply_after_config(vm, &admitted, completed, &post),
    );
    startup.attempted = Some(key);
    startup.last_error = result.err().map(StartupError::Session);
}

fn apply_after_config(
    vm: &mut pcx_script::OwnedVm<pcx_play::scripts::OwnedLocalizedHost<MapStartupHost>>,
    context: &Arc<ParsedMapContext>,
    completed: usize,
    observations: &PostConfigObservations,
) -> Result<(), MapStartupError> {
    if completed != 1 {
        return Ok(());
    }
    let resolved = {
        let program = context.scripts.program();
        map_startup_host::resolve_observations(program, observations, |name| vm.global(name))?
    };
    vm.host().inner.apply_post_config(context, &resolved)
}
