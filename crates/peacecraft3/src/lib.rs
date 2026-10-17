//! Explicit selected-map admission for the application.
//! Callers supply source identities, archive policy, field bindings and input limits.
//! Parsed publication and script startup stay separate: startup runs only after a
//! successful admission and caller-supplied host inputs.

pub mod campaign_startup;
pub mod map_admission;
pub mod map_binding;
pub mod map_ground;
pub mod map_requests;
pub mod map_script_session;
pub mod script_audio;

/// Explicit browser memory/archive reads with caller budgets and codec input.
pub mod browser_read;

pub mod browser_archive_source;

pub mod browser_configuration;

pub mod browser_configuration_json;
