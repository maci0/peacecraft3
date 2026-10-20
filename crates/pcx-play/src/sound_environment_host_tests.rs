use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use crate::sound_environment::tests::{sheet, synthetic_tables};
use crate::sound_environment::{EnvironmentTables, SoundEnvironment, SoundEnvironmentInput};
use pcx_script::{Host, Prelude};
use std::sync::Arc;

const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative NewSoundEnvironment takes string environmentName returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";
const WINDOW_MS: u32 = 5000;
const PROVIDER: ProviderId = ProviderId(7);

const DEFAULT: [&str; 17] = [
    "\"Default\"",
    "\"MOUNTAINS\"",
    "1",
    "2.5",
    "1",
    "10",
    "1",
    "0",
    "0",
    "1",
    "-2250",
    "0.07",
    "-2500",
    "0.03",
    "0",
    "-2",
    "\"WESTRING_SNDENV_DEFAULT\"",
];

fn records(rows: &[[&str; 17]]) -> Vec<SoundEnvironment> {
    let table = pcx_slq::Table::parse(&sheet(rows)).unwrap();
    crate::sound_environment::from_table(&table).unwrap()
}

fn input(rows: &[[&str; 17]], tables: EnvironmentTables) -> SoundEnvironmentInput {
    SoundEnvironmentInput {
        records: records(rows),
        tables,
        window_ms: WINDOW_MS,
    }
}

fn host(common: &str) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    ObjectHost::checked(
        EnumHost::checked_selected(
            program,
            "synthetic:environment",
            &["ConvertPlayerColor"],
            Limits {
                max_handles: 8,
                admission_steps: 100,
                expression_depth: 16,
                expression_work: 100,
            },
        )
        .unwrap(),
    )
    .unwrap()
}

fn select(h: &mut ObjectHost, name: &str) -> Result<Value, String> {
    h.native("NewSoundEnvironment", &[Value::Str(name.into())])
}

/// Pure state with the window and records above, for timing tests.
fn state(rows: &[[&str; 17]]) -> State {
    let tables = synthetic_tables();
    let entries = records(rows)
        .into_iter()
        .map(|environment| {
            let values = crate::sound_environment::apply(&environment, &tables).unwrap();
            Entry {
                environment,
                values,
            }
        })
        .collect();
    State {
        entries,
        window: f64::from(WINDOW_MS) / MS_PER_SECOND,
        provider: ProviderState::Open(PROVIDER),
        last_change: None,
        buffered: String::new(),
        pending: false,
        fires: Vec::new(),
        selected: None,
        delivery: None,
    }
}

fn dungeon() -> [&'static str; 17] {
    let mut row = DEFAULT;
    row[0] = "\"Dungeon\"";
    row
}

fn named(state: &State) -> &str {
    &state.entries[state.selected.unwrap()].environment.name
}

#[test]
fn an_unadmitted_call_is_refused_and_leaves_nothing_behind() {
    let mut h = host(COMMON);
    assert!(h.sound_environment_delivery().is_none());
    assert!(select(&mut h, "Default").is_err());
    assert!(h.sound_environment_delivery().is_none());
}

#[test]
fn admission_applies_every_record_and_refuses_one_that_cannot_be_written() {
    let mut bad = DEFAULT;
    bad[0] = "\"Bad\"";
    bad[7] = "0.5";
    assert!(
        host(COMMON)
            .with_sound_environments(input(&[DEFAULT, bad], synthetic_tables()))
            .is_err()
    );
    let mut short = synthetic_tables();
    short.properties.truncate(3);
    assert!(
        host(COMMON)
            .with_sound_environments(input(&[DEFAULT], short))
            .is_err()
    );
    assert!(
        host(COMMON)
            .with_sound_environments(SoundEnvironmentInput {
                records: Vec::new(),
                tables: synthetic_tables(),
                window_ms: WINDOW_MS,
            })
            .is_err()
    );
    let admitted = host(COMMON)
        .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
        .unwrap();
    assert!(
        admitted
            .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
            .is_err()
    );
}

#[test]
fn a_wrong_signature_refuses_admission() {
    let wrong = COMMON.replace(
        "takes string environmentName returns nothing",
        "takes integer environmentName returns nothing",
    );
    assert!(
        host(&wrong)
            .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
            .is_err()
    );
}

#[test]
fn without_a_provider_report_values_are_validated_and_nothing_is_claimed_applied() {
    let mut h = host(COMMON)
        .with_sound_environments(input(&[DEFAULT, dungeon()], synthetic_tables()))
        .unwrap();
    assert_eq!(select(&mut h, "Default").unwrap(), Value::Null);
    assert_eq!(h.sound_environment().unwrap().name, "Default");
    assert!(matches!(
        h.sound_environment_delivery(),
        Some(Delivery::Validated(values)) if values.writes.len() == 13
    ));
    assert!(h.sound_environment_applied().is_none());
}

#[test]
fn a_closed_provider_skips_the_native_calls_and_an_open_one_delivers() {
    let mut h = host(COMMON)
        .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
        .unwrap();
    h.set_sound_environment_provider(ProviderState::Closed);
    select(&mut h, "Default").unwrap();
    assert!(matches!(
        h.sound_environment_delivery(),
        Some(Delivery::ProviderClosed(_))
    ));
    assert!(h.sound_environment_applied().is_none());

    let mut open = host(COMMON)
        .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
        .unwrap();
    open.set_sound_environment_provider(ProviderState::Open(PROVIDER));
    select(&mut open, "Default").unwrap();
    let applied = open.sound_environment_applied().unwrap();
    assert_eq!(applied.effect_volume, 1.0);
    assert_eq!(applied.room, Some(17));
    assert!(matches!(
        open.sound_environment_delivery(),
        Some(Delivery::Delivered { provider, .. }) if *provider == PROVIDER
    ));
}

#[test]
fn a_call_inside_the_window_is_scheduled_not_applied_at_once() {
    let mut h = host(COMMON)
        .with_sound_environments(input(&[DEFAULT, dungeon()], synthetic_tables()))
        .unwrap();
    h.set_sound_environment_provider(ProviderState::Open(PROVIDER));
    select(&mut h, "Default").unwrap();
    select(&mut h, "Dungeon").unwrap();
    // Game time is zero for both calls, so the second falls inside the window.
    assert!(matches!(
        h.sound_environment_delivery(),
        Some(Delivery::Scheduled { fire_at }) if *fire_at == 5.0
    ));
    assert!(h.sound_environment_applied().is_none());
    assert_eq!(h.sound_environment().unwrap().name, "Default");
}

#[test]
fn an_unknown_name_changes_no_record_and_is_refused_nothing() {
    let mut h = host(COMMON)
        .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
        .unwrap();
    h.set_sound_environment_provider(ProviderState::Open(PROVIDER));
    assert_eq!(select(&mut h, "Nowhere").unwrap(), Value::Null);
    assert!(matches!(
        h.sound_environment_delivery(),
        Some(Delivery::NoRecord)
    ));
    assert!(h.sound_environment().is_none());
}

#[test]
fn a_wrong_argument_refuses_without_touching_the_previous_outcome() {
    let mut h = host(COMMON)
        .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
        .unwrap();
    h.set_sound_environment_provider(ProviderState::Open(PROVIDER));
    select(&mut h, "Default").unwrap();
    let before = h.sound_environment_delivery().cloned();
    assert!(h.native("NewSoundEnvironment", &[]).is_err());
    assert!(h.native("NewSoundEnvironment", &[Value::Int(1)]).is_err());
    assert_eq!(h.sound_environment_delivery().cloned(), before);
}

#[test]
fn the_first_call_applies_at_once_and_a_later_call_in_the_window_waits_for_its_callback() {
    let mut s = state(&[DEFAULT, dungeon()]);
    s.call("Default", 0.0);
    assert_eq!(s.last_change, Some(0.0));
    assert_eq!(named(&s), "Default");
    assert!(matches!(s.delivery, Some(Delivery::Delivered { .. })));

    s.call("Dungeon", 1.0);
    assert!(s.pending);
    assert_eq!(s.fires, vec![6.0]);
    // The scheduled call changes neither the last change nor the selection yet.
    assert_eq!(s.last_change, Some(0.0));
    assert_eq!(named(&s), "Default");

    s.fire_due(5.9);
    assert_eq!(named(&s), "Default");
    s.fire_due(6.0);
    assert_eq!(named(&s), "Dungeon");
    assert_eq!(s.last_change, Some(6.0));
    assert!(!s.pending);
    assert!(s.fires.is_empty());
}

#[test]
fn the_callback_applies_the_latest_buffered_name_and_the_provider_at_its_own_time() {
    let mut s = state(&[DEFAULT, dungeon()]);
    s.call("Default", 0.0);
    s.call("Dungeon", 1.0);
    s.call("Default", 2.0);
    assert_eq!(s.fires, vec![6.0, 7.0]);
    s.provider = ProviderState::Closed;
    s.fire_due(6.0);
    // The first callback clears the flag even though a second one is still queued.
    assert!(!s.pending);
    assert!(matches!(s.delivery, Some(Delivery::ProviderClosed(_))));
    assert_eq!(s.last_change, Some(6.0));
    s.provider = ProviderState::Open(PROVIDER);
    s.fire_due(7.0);
    assert!(matches!(s.delivery, Some(Delivery::Delivered { .. })));
    assert_eq!(named(&s), "Default");
}

#[test]
fn a_call_after_the_window_applies_at_once_while_an_earlier_callback_is_still_queued() {
    let mut s = state(&[DEFAULT, dungeon()]);
    s.call("Default", 0.0);
    s.call("Dungeon", 1.0);
    // Past the window: applied at once, and the flag stays set for the queued callback.
    s.call("Default", 10.0);
    assert_eq!(named(&s), "Default");
    assert_eq!(s.last_change, Some(10.0));
    assert!(s.pending);
    assert_eq!(s.buffered, "Default");
    s.fire_due(6.0);
    assert_eq!(s.last_change, Some(6.0));
}

#[test]
fn an_unknown_call_still_starts_the_window_because_the_native_records_its_time() {
    let mut s = state(&[DEFAULT]);
    s.call("Nowhere", 0.0);
    assert!(matches!(s.delivery, Some(Delivery::NoRecord)));
    assert_eq!(s.last_change, Some(0.0));
    s.call("Default", 3.0);
    assert!(matches!(s.delivery, Some(Delivery::Scheduled { fire_at }) if fire_at == 8.0));
}
