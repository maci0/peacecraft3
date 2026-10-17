//! Caller-qualified script random parameters and explicitly seeded state.
//! The selected image is separate from archive provenance and initial engine state.
pub mod profile;
pub mod real;
pub mod state;

#[cfg(test)]
mod state_tests;
