#[path = "support/map_admission_cases.rs"]
mod admission;
#[path = "support/map_binding_cases.rs"]
mod binding;
#[path = "support/map_fixtures.rs"]
mod fixtures;
#[path = "support/ground_fixtures.rs"]
mod ground_fixtures;

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

#[path = "support/browser_bounded_cases.rs"]
mod browser_bounded_cases;
#[path = "support/browser_mpq_fixtures.rs"]
mod browser_mpq_fixtures;
#[path = "support/browser_read_cases.rs"]
mod browser_read_cases;

#[path = "support/browser_archive_source_cases.rs"]
mod browser_archive_source_cases;

#[path = "support/browser_configuration_cases.rs"]
mod browser_configuration_cases;
#[path = "support/browser_configuration_json_cases.rs"]
mod browser_configuration_json_cases;

#[path = "support/campaign_startup_cases.rs"]
mod campaign_startup_cases;

#[path = "support/map_ground_cases.rs"]
mod map_ground_cases;
