#[path = "support/map_admission_cases.rs"]
mod admission;
#[path = "support/map_binding_cases.rs"]
mod binding;
#[path = "support/map_fixtures.rs"]
mod fixtures;

pub use peacecraft3::map_requests;
#[path = "../src/button_actions.rs"]
mod button_actions;
#[path = "../src/map_source_controls.rs"]
mod source_controls;

#[path = "support/script_session_cases.rs"]
mod consumer_cases;
#[path = "support/script_session_tests.rs"]
mod script_session_tests;
#[path = "support/callback_ticket_cases.rs"]
mod ticket_cases;

#[path = "support/mpq_extraction_cases.rs"]
mod mpq_extraction_cases;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/mpq_file_cases.rs"]
mod mpq_file_cases;
#[path = "support/mpq_extraction_fixtures.rs"]
mod mpq_fixtures;
#[path = "support/source_budget_cases.rs"]
mod source_budget_cases;
