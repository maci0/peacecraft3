#![cfg_attr(all(test, target_arch = "wasm32"), no_main)]
//! Explicit selected-map admission for the application.
//! Callers supply source identities, archive policy, field bindings and input limits.
//! Parsed publication and script startup stay separate: startup runs only after a
//! successful admission and caller-supplied host inputs.

mod campaign_assets;
pub mod campaign_boundary;
mod campaign_cache;
pub mod campaign_camera;
pub mod campaign_inputs;
pub mod campaign_placement;
pub mod campaign_presentation;
pub mod campaign_runtime;
pub mod campaign_score;
pub mod campaign_score_assets;
pub mod campaign_score_layout;
pub mod campaign_startup;
pub mod fdf;
pub mod global_strings;
pub mod map_admission;
pub mod map_binding;
pub mod map_ground;
pub mod map_requests;
pub mod map_script_session;
pub mod script_audio;
#[path = "menu_panel_volume.rs"]
pub mod sound_volume;

/// Explicit browser memory/archive reads with caller budgets and codec input.
pub mod browser_read;

pub mod browser_archive_source;

pub mod browser_configuration;

pub mod browser_configuration_json;
