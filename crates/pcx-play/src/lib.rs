//! The play-test harness: scenarios, a deterministic runner, the trace schema and its diff.
//!
//! Unit tests prove a decoder reads a byte correctly. They do not prove the game
//! is faithful. The acceptance test for this project (plan §3.5) is the original
//! campaigns, played start to finish by a script, producing a recorded trace that
//! our build reproduces exactly. This crate is the skeleton of that loop (plan
//! §3.10): a declarative scenario drives [`pcx_sim`] for a fixed number of ticks
//! and a trace records every observable per tick. Two runs of the same scenario
//! must be byte-identical; two traces compare with `pcx-diff`.
//!
//! M1b runs synthetic in-memory scenarios only. A scenario declares its starting
//! state directly; no map container is read. Map decoding wires in at M3
//! (`pcx-map`) and script execution at M7 (`pcx-trig`).
//!
//! # The trace format
//!
//! A trace is canonical text: UTF-8, LF line endings, exactly one trailing
//! newline, fields separated by single spaces. Every record is one line. The
//! record order is fixed and the decoder refuses anything that is not already
//! canonical, so `encode(decode(b)) == b` holds for every accepted input.
//!
//! ```text
//! record        fields
//! pcx-trace     <version>                       magic; version is 1
//! scenario      <name>                          scenario identity
//! map           <name>                          map identity
//! asset-mode    vanilla|hd|reforged             asset mode (plan §3.6)
//! seed          <u64>                           the pcx_sim::Rng seed
//! fix           adopt|reject|defer <id>         fix-ledger rows, sorted by id
//! ticks         <n>                             number of tick records
//! tick          <i>                             step index, 0..n, in order
//! unit          <id> <owner> <x> <y> -|<tx> <ty>   post-step state, sorted by id
//! rng           <index> <0x16-hex>              draw from the shared sim RNG
//! timer         <seq> fire|create|destroy <name>    timer event, sorted by seq
//! end
//! ```
//!
//! Within a tick record the rows appear as `unit`, then `rng`, then `timer`.
//! Integers are decimal except RNG values, which are `0x` plus 16 lowercase hex
//! digits. Names (`scenario`, `map`, timer names, fix row ids) match
//! `[A-Za-z0-9][A-Za-z0-9._-]*` and are at most [`NAME_MAX_LEN`] bytes.
//!
//! # Self-describing
//!
//! The header records the asset mode and the active fix-ledger rows, so a
//! `vanilla` run can never be confused with a `reforged` one: the two hash
//! differently and `pcx-diff` names `asset-mode` first. Scenario name, map name
//! and seed are in the header for the same reason.
//!
//! # Determinism
//!
//! The runner uses fixed-point arithmetic only (`pcx_sim::Fx`), never reads a
//! clock, and takes every random draw from one `pcx_sim::Rng`. Trace rows are
//! emitted in sorted order with no map iteration anywhere near the output.

pub mod camera_margins;
pub mod campaign;
pub mod campaign_assets;
pub mod campaign_cache;
pub mod campaign_client;
pub mod campaign_host;
pub mod campaign_inputs;
pub mod campaign_placement;
pub mod cinematics;
pub mod custom_scripts;
pub mod day_night;
pub mod default_difficulty_image;
pub mod diff;
pub mod editor_triggers;
pub mod file_sounds;
pub mod game_cache;
pub mod generator;
pub mod group_order_rules;
pub mod headless_startup;
pub mod hero_skills;
pub mod install_source;
pub mod label_sounds;
pub mod map_admission;
pub mod map_script_session;
pub mod map_scripts_import;
pub mod map_startup_host;
pub mod menu_edition;
pub mod menu_lighting;
pub mod minimize;
pub mod mp3_frame;
pub mod neutral_player;
mod neutral_rule_discovery;
mod neutral_rule_input;
pub mod options_defaults;
pub mod options_modes;
pub mod order_registry;
pub mod pcm_decoder;
pub mod player_handicap;
pub mod playercolor_host;
pub mod quest_session;
pub mod quest_view;
pub mod rpl;
pub mod runner;
pub mod script_math;
pub mod script_numeric;
pub mod script_random;
pub mod unit_activity;
pub mod unit_model;
pub use rpl::{Expectation, Rpl, RplError};
pub(crate) mod pe_image;
pub mod scenario;
pub mod script_trace;
pub mod scripts;
pub mod sound_environment;
pub mod sound_parameters;
pub mod startup_clock;
pub mod terrain_pathing;
pub mod trace;
pub mod unit_placement;
pub mod upgrade_research;
pub use map_scripts_import::*;

pub use diff::{Divergence, first_divergence};
pub use runner::{RunError, run};
pub use scenario::{
    RunConfig, Scenario, ScheduledAction, ScriptedAction, SpawnSpec, smoke, smoke_config,
};
pub use trace::{
    AssetMode, FixDisposition, FixRow, RngDraw, TickRecord, TimerEvent, TimerEventKind, Trace,
    TraceError, TraceHeader, UnitState, fnv1a64,
};

/// Maximum byte length of any name in a trace or scenario.
pub const NAME_MAX_LEN: usize = 64;

pub(crate) fn is_valid_name(s: &str) -> bool {
    let bytes = s.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= NAME_MAX_LEN
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

pub mod player_resources;

pub mod cheat_rules;
pub mod dialog_layout;

#[path = "inventory_rules.rs"]
pub mod inventory;
pub mod player_command_log;
pub mod player_removal_rules;
pub mod selection_circle;

pub mod inventory_layout;
pub mod inventory_powerup;
pub mod inventory_powerup_host;
pub mod item_class_table;
pub(crate) mod item_heal;
pub mod item_sales;
pub mod item_use_rules;
pub mod script_localization;
pub mod shop_table;
pub mod unit_guard_rules;
