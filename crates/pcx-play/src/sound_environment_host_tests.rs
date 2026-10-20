use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use crate::sound_environment::tests::{sheet, synthetic_tables};
use crate::sound_environment::{EnvironmentTables, SoundEnvironment, SoundEnvironmentInput};
use pcx_script::{Host, Prelude};
use std::sync::Arc;

const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative NewSoundEnvironment takes string environmentName returns nothing\nnative TimerStart takes timer t, real timeout, boolean periodic, code handler returns nothing\nnative TimerGetElapsed takes timer t returns real\nnative TimerGetRemaining takes timer t returns real\nnative TimerGetTimeout takes timer t returns real\nnative GetExpiredTimer takes nothing returns timer\nnative PauseTimer takes timer t returns nothing\nnative ResumeTimer takes timer t returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";
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
    }
}

/// Host with the game clock admitted at `now`, as campaign startup does.
fn host(common: &str, now: Option<f64>) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    let host = ObjectHost::checked(
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
    .unwrap();
    match now {
        Some(now) => host.with_timer_runtime(now).unwrap(),
        None => host,
    }
}

fn select(h: &mut ObjectHost, name: &str) -> Result<Value, String> {
    h.native("NewSoundEnvironment", &[Value::Str(name.into())])
}

/// Pure state over the synthetic records, for timing tests. Window comes from the tables.
fn state(rows: &[[&str; 17]]) -> State {
    let tables = synthetic_tables();
    let entries = records(rows)
        .into_iter()
        .map(|environment| {
            let request = crate::sound_environment::apply(&environment, &tables).unwrap();
            Entry {
                environment,
                request,
            }
        })
        .collect();
    State {
        entries,
        window: tables.window,
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
    let mut h = host(COMMON, Some(0.0));
    assert!(h.sound_environment_delivery().is_none());
    assert!(select(&mut h, "Default").is_err());
    assert!(h.sound_environment_delivery().is_none());
}

#[test]
fn a_call_without_a_game_clock_is_refused_rather_than_timed_from_zero() {
    let mut h = host(COMMON, None)
        .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
        .unwrap();
    assert!(
        select(&mut h, "Default")
            .unwrap_err()
            .contains("game clock")
    );
    assert!(h.sound_environment_delivery().is_none());
}

#[test]
fn admission_computes_every_record_and_refuses_one_that_cannot_be_written() {
    let mut bad = DEFAULT;
    bad[0] = "\"Bad\"";
    bad[7] = "0.5";
    assert!(
        host(COMMON, Some(0.0))
            .with_sound_environments(input(&[DEFAULT, bad], synthetic_tables()))
            .is_err()
    );
    let mut short = synthetic_tables();
    short.properties.truncate(3);
    assert!(
        host(COMMON, Some(0.0))
            .with_sound_environments(input(&[DEFAULT], short))
            .is_err()
    );
    assert!(
        host(COMMON, Some(0.0))
            .with_sound_environments(SoundEnvironmentInput {
                records: Vec::new(),
                tables: synthetic_tables(),
            })
            .is_err()
    );
    let admitted = host(COMMON, Some(0.0))
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
        host(&wrong, Some(0.0))
            .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
            .is_err()
    );
}

#[test]
fn without_a_provider_report_the_request_is_validated_and_nothing_is_sent() {
    let mut h = host(COMMON, Some(0.0))
        .with_sound_environments(input(&[DEFAULT, dungeon()], synthetic_tables()))
        .unwrap();
    assert_eq!(select(&mut h, "Default").unwrap(), Value::Null);
    assert_eq!(h.sound_environment().unwrap().name, "Default");
    assert!(matches!(
        h.sound_environment_delivery(),
        Some(Delivery::Validated(request)) if request.writes.len() == 13
    ));
}

#[test]
fn a_closed_provider_skips_the_native_calls() {
    let mut h = host(COMMON, Some(0.0))
        .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
        .unwrap();
    h.set_sound_environment_provider(ProviderState::Closed);
    select(&mut h, "Default").unwrap();
    assert!(matches!(
        h.sound_environment_delivery(),
        Some(Delivery::ProviderClosed(_))
    ));
}

#[test]
fn an_open_provider_prepares_the_request_with_its_room_gate_unresolved_and_sends_nothing() {
    let mut h = host(COMMON, Some(0.0))
        .with_sound_environments(input(&[DEFAULT], synthetic_tables()))
        .unwrap();
    h.set_sound_environment_provider(ProviderState::Open(PROVIDER));
    select(&mut h, "Default").unwrap();
    match h.sound_environment_delivery() {
        Some(Delivery::Prepared {
            provider,
            request,
            room_gate,
        }) => {
            assert_eq!(*provider, PROVIDER);
            assert_eq!(request.effect_volume, 1.0);
            assert_eq!(request.room, Some(17));
            assert_eq!(*room_gate, RoomGate::Unresolved);
        }
        other => panic!("expected Prepared, got {other:?}"),
    }
}

#[test]
fn a_record_whose_room_is_not_in_the_table_has_no_room_gate_to_resolve() {
    let mut unknown = DEFAULT;
    unknown[1] = "\"NOWHERE\"";
    let mut h = host(COMMON, Some(0.0))
        .with_sound_environments(input(&[unknown], synthetic_tables()))
        .unwrap();
    h.set_sound_environment_provider(ProviderState::Open(PROVIDER));
    select(&mut h, "Default").unwrap();
    assert!(matches!(
        h.sound_environment_delivery(),
        Some(Delivery::Prepared { request, room_gate: RoomGate::NotApplicable, .. }) if request.room.is_none()
    ));
}

#[test]
fn a_call_inside_the_window_is_scheduled_not_computed_at_once() {
    let mut h = host(COMMON, Some(0.0))
        .with_sound_environments(input(&[DEFAULT, dungeon()], synthetic_tables()))
        .unwrap();
    h.set_sound_environment_provider(ProviderState::Open(PROVIDER));
    select(&mut h, "Default").unwrap();
    select(&mut h, "Dungeon").unwrap();
    // The window in the synthetic tables is 5 s, so the second call at time zero waits.
    assert!(matches!(
        h.sound_environment_delivery(),
        Some(Delivery::Scheduled { fire_at }) if *fire_at == 5.0
    ));
    assert_eq!(h.sound_environment().unwrap().name, "Default");
}

#[test]
fn an_unknown_name_selects_no_record_and_refuses_nothing() {
    let mut h = host(COMMON, Some(0.0))
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
    let mut h = host(COMMON, Some(0.0))
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
fn the_first_call_computes_at_once_and_a_later_call_in_the_window_waits_for_its_callback() {
    let mut s = state(&[DEFAULT, dungeon()]);
    s.call("Default", 0.0);
    assert_eq!(s.last_change, Some(0.0));
    assert_eq!(named(&s), "Default");
    assert!(matches!(s.delivery, Some(Delivery::Prepared { .. })));

    s.call("Dungeon", 1.0);
    assert!(s.pending);
    assert_eq!(s.fires, vec![6.0]);
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
fn a_callback_computes_the_latest_buffered_name_with_the_provider_at_its_own_time() {
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
    assert!(matches!(s.delivery, Some(Delivery::Prepared { .. })));
    assert_eq!(named(&s), "Default");
}

#[test]
fn a_call_after_the_window_computes_at_once_while_an_earlier_callback_is_still_queued() {
    let mut s = state(&[DEFAULT, dungeon()]);
    s.call("Default", 0.0);
    s.call("Dungeon", 1.0);
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
