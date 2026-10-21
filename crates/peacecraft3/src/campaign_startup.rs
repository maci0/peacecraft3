//! Start one admitted map on the production host after parsed delivery.
//! Browser source selection stays on `SelectedMapSources`. This system reads
//! the admitted context and caller-supplied host inputs only.
use crate::browser_configuration::SelectedMapSources;
use crate::map_requests::{MapRequestSet, MapRequests};
use crate::map_script_session::{MapScriptSession, ScriptSessionError};
use bevy::prelude::*;
use pcx_play::map_admission::ParsedMapContext;
use pcx_play::map_startup_host::{MapStartupHost, MapStartupInputs};
use pcx_script::value::FunctionId;
use std::sync::{Arc, Weak};

struct AttemptKey {
    context: Weak<ParsedMapContext>,
    revision: u64,
    retryable: bool,
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

/// Explicit replay override for subsequently prepared games; existing sessions stay unchanged.
#[derive(Resource, Default)]
pub struct CampaignSeed {
    pub requested: Option<i32>,
    pub prepared: Option<i32>,
}

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("host inputs are for binding revision {supply}; the admitted revision is {binding}")]
    InputRevision { supply: u64, binding: u64 },
    #[error("binding revision {binding} has no campaign host inputs")]
    MissingInputs { binding: u64 },
    #[error("preparing campaign binding {binding}: {reason}")]
    Preparation { binding: u64, reason: String },
    #[error("{name} is not declared by the admitted map script")]
    MissingEntrypoint { name: &'static str },
    #[error("{name} is not a map-script function that takes nothing and returns nothing")]
    EntrypointShape { name: &'static str },
    #[error(transparent)]
    Session(#[from] ScriptSessionError),
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampaignStartupSet;

pub fn install(app: &mut App) {
    app.init_resource::<CampaignStartup>()
        .init_resource::<CampaignHostSupply>()
        .init_resource::<CampaignSeed>()
        .init_resource::<MapScriptSession<MapStartupHost>>()
        .add_systems(
            Update,
            start_admitted_map
                .in_set(CampaignStartupSet)
                .after(MapRequestSet::Deliver),
        );
}

/// Entrypoint lookup, mapped to this startup's error kinds.
fn entry_id(context: &ParsedMapContext, name: &'static str) -> Result<FunctionId, StartupError> {
    pcx_play::campaign_host::entry_id(context, name).map_err(|error| match error {
        pcx_play::campaign_host::EntrypointError::Missing { name } => {
            StartupError::MissingEntrypoint { name }
        }
        pcx_play::campaign_host::EntrypointError::Shape { name } => {
            StartupError::EntrypointShape { name }
        }
    })
}

fn start_admitted_map(
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    mut supply: ResMut<CampaignHostSupply>,
    no_occlusion: Option<Res<crate::map_ground::NoOcclusionPass>>,
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
    let mut key = AttemptKey {
        context: Arc::downgrade(&context),
        revision: bindings.revision,
        retryable: false,
    };
    if startup.attempted.as_ref().is_some_and(|previous| {
        previous.revision == key.revision
            && Weak::ptr_eq(&previous.context, &key.context)
            && (!previous.retryable || supply.inputs.is_none())
    }) {
        return;
    }
    if (supply.inputs.is_none() || supply.revision != bindings.revision)
        && matches!(&startup.last_error, Some(StartupError::Preparation { binding, .. }) if *binding == bindings.revision)
    {
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
    let no_occlusion = no_occlusion.is_some();
    let reader = sources.for_receipts(&bindings.receipts);
    let admitted = Arc::clone(&context);
    let placement_map = Arc::clone(&bindings.catalog_map_bytes);
    let result = session.replace_sequenced(
        Arc::clone(&context),
        move |ctx| {
            pcx_play::campaign_host::create_host(
                ctx,
                inputs,
                reader,
                placement_map,
                no_occlusion,
                pcx_play::campaign_cache::local_storage(),
            )
        },
        &[config, main],
        move |vm, completed| {
            pcx_play::campaign_host::apply_after_config(vm, &admitted, completed, &post)
        },
    );
    key.retryable = result.is_err();
    startup.attempted = Some(key);
    startup.last_error = result.err().map(StartupError::Session);
}
