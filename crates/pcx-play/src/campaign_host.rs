//! Campaign map-host creation and post-configuration. The application's startup system and
//! the headless startup both call these functions with the same inputs.
use crate::map_admission::ParsedMapContext;
use crate::map_startup_host::{
    self, MapStartupError, MapStartupHost, MapStartupInputs, PostConfigObservations,
};
use pcx_assets::Resolver;
use pcx_script::value::FunctionId;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum EntrypointError {
    #[error("{name} is not declared by the admitted map script")]
    Missing { name: &'static str },
    #[error("{name} is not a map-script function that takes nothing and returns nothing")]
    Shape { name: &'static str },
}

pub fn entry_id(
    context: &ParsedMapContext,
    name: &'static str,
) -> Result<FunctionId, EntrypointError> {
    let program = context.scripts.program();
    let Some(function) = program.function(name) else {
        return Err(EntrypointError::Missing { name });
    };
    if function.source != "the map script"
        || !function.params.is_empty()
        || function.ret.kind.name() != "nothing"
    {
        return Err(EntrypointError::Shape { name });
    }
    program
        .function_id(name)
        .ok_or(EntrypointError::Missing { name })
}

/// Creates the campaign host: the assembled startup host with the cache, placement, sound,
/// unit-model and day/night loaders attached.
pub fn create_host(
    ctx: Arc<ParsedMapContext>,
    inputs: MapStartupInputs,
    reader: Option<Arc<Resolver>>,
    placement_map: Arc<[u8]>,
    no_occlusion: bool,
) -> Result<MapStartupHost, MapStartupError> {
    let edition = ctx.sources.map.edition.clone();
    let mut host = map_startup_host::assemble(ctx, inputs)?;
    crate::campaign_cache::attach(&mut host)?;
    if no_occlusion {
        host.objects_mut().admit_no_occlusion_pass()?;
    }
    host.objects_mut().admit_presentation(16 << 10)?;
    host.objects_mut()
        .admit_messages(crate::playercolor_host::MessageBudget {
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
                let mut models =
                    crate::campaign_assets::UnitModels::new(Arc::clone(&reader), edition.clone());
                host.objects_mut()
                    .admit_unit_models(move |definition| models.load(definition))?;
            }
            let parameter_reader = Arc::clone(&reader);
            let parameter_edition = edition.clone();
            let parameter_budget = crate::sound_parameters::BanksBudget {
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
            host.objects_mut()
                .admit_sound_parameter_loader(parameter_budget, move |label| {
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
                })?;
            let label_reader = Arc::clone(&reader);
            let label_edition = edition.clone();
            let mut remaining_labels = 128usize;
            host.objects_mut().admit_label_sound_loader(
                crate::label_sounds::LoadBudget {
                    banks: 130,
                    total_table_bytes: 32 << 20,
                    inputs: crate::label_sounds::Budget {
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
                crate::file_sounds::Budget {
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
                        crate::file_sounds::Mp3Budget {
                            max_input_allocation_bytes: (16 << 20)
                                + crate::mp3_frame::DECODER_RING_BYTES,
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
                    crate::day_night::Budget {
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
}

/// Applies the post-configuration observations once `config` has returned.
pub fn apply_after_config(
    vm: &mut pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<MapStartupHost>>,
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

/// Advances the admitted session's clock to `until`: timers and expiries first, then unit
/// ranges unless the script paused the game. The application's frame loop and the headless
/// startup both step through this function.
pub fn advance_tick(
    session: &mut crate::map_script_session::MapScriptSession<MapStartupHost>,
    until: f64,
    max_expiries: usize,
) -> Result<usize, crate::map_script_session::TimerSessionError> {
    let count = session.advance_timers(until, max_expiries)?;
    if session
        .host()
        .is_some_and(|host| host.objects().game_paused() == Some(true))
    {
        Ok(count)
    } else {
        session.advance_unit_ranges(65536).map(|_| count)
    }
}
