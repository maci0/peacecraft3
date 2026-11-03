//! Focused path tests without the unpublished upstream font-test-data fixtures.
pub extern crate read_fonts as raw;
#[path = "../src/outline/path.rs"]
pub mod path;
#[path = "../src/outline/pen.rs"]
pub mod pen;
