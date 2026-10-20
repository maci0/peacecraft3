//! Start one admitted map on the production host after parsed delivery.
//! Browser source selection stays on `SelectedMapSources`. This system reads
//! the admitted context and caller-supplied host inputs only.
use crate::browser_configuration::SelectedMapSources;
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
    let edition = context.sources.map.edition.clone();
    let admitted = Arc::clone(&context);
    let placement_map = Arc::clone(&bindings.catalog_map_bytes);
    let result = session.replace_sequenced(
        Arc::clone(&context),
        move |ctx| {
            let mut host = map_startup_host::assemble(ctx, inputs)?;
            crate::campaign_cache::attach(&mut host)?;
            if no_occlusion {
                host.objects_mut().admit_no_occlusion_pass()?;
            }
            host.objects_mut().admit_presentation(16 << 10)?;
            host.objects_mut()
                .admit_messages(pcx_play::playercolor_host::MessageBudget {
                    records: 256,
                    text_bytes: 1 << 20,
                })?;
            Ok::<_, MapStartupError>(match reader {
                Some(reader) => {
                    let placement_reader = Arc::clone(&reader);
                    let placement_context = Arc::clone(host.context());
                    host = host.with_placement_loader(move |objects, rules, tables| {
                        crate::campaign_placement::assemble(
                            &placement_context,
                            &placement_map,
                            Arc::clone(&placement_reader),
                            tables,
                            objects,
                            rules,
                        )
                    });
                    if host.objects().timer_game_time().is_some() {
                        let mut models = crate::campaign_assets::UnitModels::new(
                            Arc::clone(&reader),
                            edition.clone(),
                        );
                        host.objects_mut()
                            .admit_unit_models(move |definition| models.load(definition))?;
                    }
                    let parameter_reader = Arc::clone(&reader);
                    let parameter_edition = edition.clone();
                    let parameter_budget = pcx_play::sound_parameters::BanksBudget {
                        banks: 32,
                        table_bytes: 4 << 20,
                        total_table_bytes: 32 << 20,
                        coordinates: pcx_slq::CoordinateLimits {
                            max_row: 10000,
                            max_column: 256,
                        },
                        definitions: 1024,
                        metadata_bytes: 1 << 20,
                    };
                    let sound_sources = Arc::new(std::sync::OnceLock::new());
                    let parameter_sources = Arc::clone(&sound_sources);
                    let mut remaining_parameters = 128usize;
                    host.objects_mut().admit_sound_parameter_loader(
                        parameter_budget,
                        move |label| {
                            remaining_parameters = remaining_parameters
                                .checked_sub(1)
                                .ok_or("sound parameter source work budget exhausted")?;
                            if label.len() > 16 << 10 {
                                return Err("sound parameter label byte budget exceeded".into());
                            }
                            parameter_sources
                                .get_or_init(|| {
                                    crate::campaign_assets::SoundSources::read(
                                        &parameter_reader,
                                        &parameter_edition,
                                        parameter_budget,
                                    )
                                })
                                .as_ref()
                                .map_err(Clone::clone)?
                                .bind(label, parameter_budget)
                        },
                    )?;
                    let label_reader = Arc::clone(&reader);
                    let label_edition = edition.clone();
                    let mut remaining_labels = 128usize;
                    host.objects_mut().admit_label_sound_loader(
                        pcx_play::label_sounds::LoadBudget {
                            banks: 130,
                            total_table_bytes: 32 << 20,
                            inputs: pcx_play::label_sounds::Budget {
                                table_bytes: 4 << 20,
                                coordinates: parameter_budget.coordinates,
                                sounds: 128,
                                resource_bytes: 64 << 20,
                                total_resource_bytes: 128 << 20,
                                metadata_bytes: 1 << 20,
                            },
                        },
                        move |label, budget| {
                            remaining_labels = remaining_labels
                                .checked_sub(1)
                                .ok_or("label sound source work budget exhausted")?;
                            if label.len() > 16 << 10 {
                                return Err("label sound label byte budget exceeded".into());
                            }
                            sound_sources
                                .get_or_init(|| {
                                    crate::campaign_assets::SoundSources::read(
                                        &label_reader,
                                        &label_edition,
                                        parameter_budget,
                                    )
                                })
                                .as_ref()
                                .map_err(Clone::clone)?
                                .label_sound(&label_reader, &label_edition, label, budget)
                        },
                    )?;
                    let sound_reader = Arc::clone(&reader);
                    let sound_edition = edition.clone();
                    let mut remaining_sounds = 128usize;
                    // Application source/decoder limits, not authored sound settings.
                    host.objects_mut().admit_file_sound_loader(
                        pcx_play::file_sounds::Budget {
                            resources: 128,
                            resource_bytes: 16 << 20,
                            total_bytes: 128 << 20,
                            metadata_bytes: 1 << 20,
                        },
                        move |member, remaining| {
                            remaining_sounds = remaining_sounds
                                .checked_sub(1)
                                .ok_or("CreateSound: session source-load budget exhausted")?;
                            crate::campaign_assets::file_sound(
                                &sound_reader,
                                &sound_edition,
                                member,
                                remaining,
                                pcx_play::file_sounds::Mp3Budget {
                                    max_input_allocation_bytes: (16 << 20)
                                        + pcx_play::mp3_frame::DECODER_RING_BYTES,
                                    max_decoder_steps: 1 << 20,
                                    max_packets: 1 << 16,
                                    max_frames: 16 << 20,
                                    max_samples: 32 << 20,
                                },
                            )
                        },
                    )?;
                    // Bound cumulative source work too; selecting an already loaded pair reads nothing.
                    let mut remaining_loads = 64usize;
                    host.with_day_night_loader(move |terrain, unit| {
                        remaining_loads = remaining_loads
                            .checked_sub(1)
                            .ok_or("SetDayNightModels: session source-load budget exhausted")?;
                        // Application input limits, not lighting values or native engine limits.
                        crate::campaign_assets::day_night(
                            &reader,
                            &edition,
                            [terrain, unit],
                            pcx_play::day_night::Budget {
                                models: 2,
                                model_bytes: 2 << 20,
                                total_bytes: 4 << 20,
                                path_bytes: 16 << 10,
                            },
                        )
                    })
                }
                None => host,
            })
        },
        &[config, main],
        move |vm, completed| apply_after_config(vm, &admitted, completed, &post),
    );
    key.retryable = result.is_err();
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
