//! Explicit selected-map admission for the application.
//! Callers supply source identities, archive policy, field bindings and input limits.
//! Admission publishes parsed data; it does not initialize a gameplay Host.

pub mod map_admission;
pub mod map_binding;
pub mod map_requests;
pub mod map_script_session;
