//! Persistent canonical gameplay fixture for test-only interactive order flow.
//!
//! Test-only: behind `#[cfg(all(test,not(target_arch="wasm32"),feature="test-fixtures"))]`
//! in the binary (no production CLI, no second bridge). Reuses the existing
//! file-journal mechanism (`observation-journal.json` in an ignored directory,
//! cf. `campaign_entry_gpu_tests`): Python writes bounded request records, the
//! fixture validates each record fail-closed, applies at most one legal explicit
//! `CampaignInput` order per request via the normal local-player input path
//! (`select_units` + `standard_target`), steps the simulation with
//! `gameplay_observation::step_once`, and writes a bounded response with the
//! next canonical observation.
//!
//! No teleport, hidden state, cinematic/pause/session bypass. All requests
//! are explicit structured canonical player orders (not a vision benchmark).

use std::path::Path;

use bevy::prelude::*;
use pcx_play::map_startup_host::MapStartupHost;
use pcx_script::HandleId;
use peacecraft3::{
    campaign_camera::CampaignCamera,
    gameplay_observation::{self, Observation},
    map_script_session::MapScriptSession,
};
use serde::{Deserialize, Serialize};

/// Canonical protocol version written in every request/response record.
pub const PROTOCOL_VERSION: u32 = 1;
/// Max request file bytes; larger files fail closed.
pub const MAX_REQUEST_BYTES: usize = 8192;
/// Max response file bytes; fixture never writes more.
pub const MAX_RESPONSE_BYTES: usize = 1 << 20;
/// Max seconds of simulation per `step` request.
pub const MAX_STEP_SECONDS: f32 = 30.0;
/// Max units per move order (bounded, fail-closed).
pub const MAX_ORDER_UNITS: usize = 64;
/// Max request records processed per session (bounds runaway loops).
pub const MAX_REQUESTS: usize = 64;
/// Max unit ids per observation returned (observation is already bounded,
/// this caps the on-disk response).
pub const MAX_OBSERVATION_UNITS: usize = 2048;
/// Coordinate bound for move targets (world-space, fail-closed).
pub const MAX_COORDINATE: f32 = 100_000.0;
/// Request file name (JSON, one bounded record).
pub const REQUEST_FILE: &str = "fixture-request.json";
/// Response file name (JSON, one bounded record).
pub const RESPONSE_FILE: &str = "fixture-response.json";
/// Journal file name (append-only list of completed request ids).
pub const JOURNAL_FILE: &str = "fixture-journal.json";

/// A started production-path app (mirrors `native_campaign::tests::started_app`).
pub struct Started {
    pub app: App,
    pub camera: Entity,
}

impl Started {
    pub fn new(mut app: App) -> Self {
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(CampaignCamera::active_for_test(camera));
        Started { app, camera }
    }

    pub fn observe(&mut self) -> Observation {
        let camera = self.camera;
        self.app.world_mut().resource_scope(
            |world, mut session: Mut<MapScriptSession<MapStartupHost>>| {
                let objects = session.host().unwrap().objects();
                let cam = world
                    .get::<Camera>(camera)
                    .map(|c| (c, world.get::<GlobalTransform>(camera).unwrap()));
                gameplay_observation::observe(
                    objects,
                    cam,
                    None::<fn([f32; 2]) -> f32>,
                    MAX_OBSERVATION_UNITS,
                )
            },
        )
    }
}

/// One bounded canonical player-order request. Deserialization is strict:
/// unknown fields are rejected, missing fields fail closed.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FixtureRequest {
    pub protocol: u32,
    pub id: u64,
    pub action: RequestAction,
}

/// Bounded request actions. Only an explicit legal player order exists;
/// there is no teleport/hidden-state/cinematic/pause/session action.
/// Label: structured canonical order, not native input / UI vision.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestAction {
    /// Legal move order + step by `seconds` (0 < s <= MAX_STEP_SECONDS).
    Move {
        units: Vec<u64>,
        target: [f32; 2],
        seconds: f32,
    },
    /// No-order step by `seconds` (0 < s <= MAX_STEP_SECONDS).
    Step { seconds: f32 },
    /// Canonical stop (cancel) order for `units`, then step by `seconds`.
    /// Issued via `activate_standard(Kind::Stop)` -> `issue_form(Immediate)`
    /// on the canonical `MapStartupHost`/`ObjectHost`, not `pcx_sim`.
    Cancel { units: Vec<u64>, seconds: f32 },
    /// Graceful stop request (writes response, returns from run loop).
    Stop,
}

/// Bounded response record referencing the processed request id.
#[derive(Debug, Clone, Serialize)]
pub struct FixtureResponse {
    pub protocol: u32,
    pub id: u64,
    pub ok: bool,
    pub error: Option<String>,
    pub observation: Option<Observation>,
    pub state: Option<String>,
}

fn fail(id: u64, msg: impl Into<String>) -> FixtureResponse {
    FixtureResponse {
        protocol: PROTOCOL_VERSION,
        id,
        ok: false,
        error: Some(msg.into()),
        observation: None,
        state: None,
    }
}

/// Validate a parsed request for fail-closed bounds. Returns error string
/// on any violation; never panics, never applies partial orders.
fn validate(req: &FixtureRequest) -> Result<(), String> {
    if req.protocol != PROTOCOL_VERSION {
        return Err(format!("unsupported protocol {}", req.protocol));
    }
    match &req.action {
        RequestAction::Move {
            units,
            target,
            seconds,
        } => {
            if units.is_empty() || units.len() > MAX_ORDER_UNITS {
                return Err(format!("units length {} out of bounds", units.len()));
            }
            if units.contains(&0) {
                return Err("unit id 0 is invalid".into());
            }
            if !target
                .iter()
                .all(|v| v.is_finite() && v.abs() <= MAX_COORDINATE)
            {
                return Err("target coordinate out of bounds".into());
            }
            if !seconds.is_finite() || *seconds <= 0.0 || *seconds > MAX_STEP_SECONDS {
                return Err(format!("seconds {seconds} out of bounds"));
            }
            Ok(())
        }
        RequestAction::Step { seconds } => {
            if !seconds.is_finite() || *seconds <= 0.0 || *seconds > MAX_STEP_SECONDS {
                return Err(format!("seconds {seconds} out of bounds"));
            }
            Ok(())
        }
        RequestAction::Cancel { units, seconds } => {
            if units.is_empty() || units.len() > MAX_ORDER_UNITS {
                return Err(format!("units length {} out of bounds", units.len()));
            }
            if units.contains(&0) {
                return Err("unit id 0 is invalid".into());
            }
            if !seconds.is_finite() || *seconds <= 0.0 || *seconds > MAX_STEP_SECONDS {
                return Err(format!("seconds {seconds} out of bounds"));
            }
            Ok(())
        }
        RequestAction::Stop => Ok(()),
    }
}

/// Truncate observation units to the response cap (bounded output).
fn bounded(mut obs: Observation) -> Observation {
    obs.units.truncate(MAX_OBSERVATION_UNITS);
    obs
}

fn write_response(dir: &Path, resp: &FixtureResponse) -> Result<(), String> {
    let json = serde_json::to_string_pretty(resp).map_err(|e| e.to_string())?;
    if json.len() > MAX_RESPONSE_BYTES {
        return Err("response exceeds size cap".into());
    }
    let tmp = dir.join("fixture-response.tmp");
    std::fs::write(&tmp, json.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join(RESPONSE_FILE)).map_err(|e| e.to_string())?;
    Ok(())
}

fn append_journal(dir: &Path, id: u64) -> Result<(), String> {
    let path = dir.join(JOURNAL_FILE);
    let mut ids: Vec<u64> = if path.exists() {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    ids.push(id);
    if ids.len() > MAX_REQUESTS {
        return Err("journal exceeds request cap".into());
    }
    let json = serde_json::to_string(&ids).map_err(|e| e.to_string())?;
    std::fs::write(&path, json.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

/// Read + validate one request file (bounded size, strict JSON).
fn read_request(dir: &Path) -> Result<Option<FixtureRequest>, String> {
    let path = dir.join(REQUEST_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_REQUEST_BYTES as u64 {
        return Err(format!("request size {} exceeds cap", meta.len()));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let req: FixtureRequest =
        serde_json::from_str(&content).map_err(|e| format!("invalid request: {e}"))?;
    validate(&req)?;
    Ok(Some(req))
}

fn consume_request(dir: &Path) -> Result<(), String> {
    std::fs::remove_file(dir.join(REQUEST_FILE)).map_err(|e| e.to_string())
}

fn seconds_to_ticks(seconds: f32) -> u32 {
    // Mirrors `gameplay_observation::STEP_DT` (0.25s per tick), bounded by STEP_TICKS.
    let ticks = (seconds / gameplay_observation::STEP_DT as f32).ceil() as u32;
    ticks.clamp(1, gameplay_observation::STEP_TICKS)
}

/// Issue a legal move order through the normal local-player input path.
/// Returns count of issued orders (0 = no eligible units, fail-closed no-op).
pub fn issue_move(started: &mut Started, unit_ids: &[u64], target: [f32; 2]) -> usize {
    let handles: Vec<HandleId> = unit_ids
        .iter()
        .filter_map(|id| std::num::NonZeroU64::new(*id).map(HandleId::new))
        .collect();
    if handles.is_empty() {
        return 0;
    }
    let app = &mut started.app;
    app.world_mut().resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects_mut()
        .select_units(&handles, true);
    let result = app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            crate::campaign_input::standard_target(
                objects,
                &mut input,
                pcx_play::standard_command::Kind::Move,
                target,
                None,
            )
        },
    );
    match result {
        Ok(()) => 1,
        Err(_) => 0,
    }
}

/// Issue a canonical stop (cancel) order through the normal local-player
/// input path (`activate_standard(Kind::Stop)` -> `issue_form(Immediate)`).
/// Returns count of units for which a stop order was admitted (0 = none).
pub fn issue_cancel(started: &mut Started, unit_ids: &[u64]) -> usize {
    let handles: Vec<HandleId> = unit_ids
        .iter()
        .filter_map(|id| std::num::NonZeroU64::new(*id).map(HandleId::new))
        .collect();
    if handles.is_empty() {
        return 0;
    }
    let app = &mut started.app;
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .objects_mut()
        .select_units(&handles, true);
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .expect("live session witness");
    let count = app.world_mut().resource_scope(
        |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let host = session.host().unwrap();
            let first = handles[0];
            crate::campaign_input::activate_standard(
                host,
                &mut input,
                first,
                pcx_play::standard_command::Kind::Stop,
                witness.clone(),
            )
        },
    );
    match count {
        Ok(()) => handles.len(),
        Err(_) => 0,
    }
}

/// One bounded step through the canonical driver, mirroring `step_once` usage
/// in `campaign_entry_gpu_tests`.
fn step_seconds(started: &mut Started, seconds: f32) -> gameplay_observation::StepState {
    let witness = started
        .app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .expect("live session witness");
    let ticks = seconds_to_ticks(seconds);
    let mut session = started
        .app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    gameplay_observation::step_once(&mut session, &witness, ticks)
}

/// Run the persistent fixture loop until `Stop` or MAX_REQUESTS.
/// Each iteration: poll for a bounded request file, validate fail-closed,
/// apply at most one legal order, step, observe, write response + journal.
/// Polls briefly between requests using App::update (no sleep threads).
pub fn run_fixture(
    started: &mut Started,
    dir: &Path,
    request_timeout_polls: u32,
) -> Result<usize, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // Write initial observation response (id 0) so a client can observe
    // before sending the first order.
    let initial = bounded(started.observe());
    write_response(
        dir,
        &FixtureResponse {
            protocol: PROTOCOL_VERSION,
            id: 0,
            ok: true,
            error: None,
            observation: Some(initial),
            state: Some("initial".into()),
        },
    )?;
    let mut processed = 0usize;
    let mut polls = 0u32;
    loop {
        if processed >= MAX_REQUESTS {
            return Err("max requests reached".into());
        }
        let req = match read_request(dir) {
            Ok(Some(r)) => r,
            Ok(None) => {
                // No request yet: pump the app once and poll again.
                started.app.update();
                polls += 1;
                if polls > request_timeout_polls {
                    return Err("request timeout".into());
                }
                continue;
            }
            Err(e) => {
                // Fail-closed: write error response with id 0 sentinel if
                // the id cannot be parsed, consume the bad file, continue.
                let resp = fail(0, e);
                write_response(dir, &resp)?;
                let _ = consume_request(dir);
                polls += 1;
                if polls > request_timeout_polls {
                    return Err("request timeout after invalid request".into());
                }
                continue;
            }
        };
        polls = 0;
        let resp = match &req.action {
            RequestAction::Stop => {
                let obs = bounded(started.observe());
                let r = FixtureResponse {
                    protocol: PROTOCOL_VERSION,
                    id: req.id,
                    ok: true,
                    error: None,
                    observation: Some(obs),
                    state: Some("stopped".into()),
                };
                write_response(dir, &r)?;
                append_journal(dir, req.id)?;
                let _ = consume_request(dir);
                processed += 1;
                return Ok(processed);
            }
            RequestAction::Move {
                units,
                target,
                seconds,
            } => {
                let issued = issue_move(started, units, *target);
                let step = step_seconds(started, *seconds);
                let obs = bounded(started.observe());
                FixtureResponse {
                    protocol: PROTOCOL_VERSION,
                    id: req.id,
                    ok: issued > 0,
                    error: if issued == 0 {
                        Some("no eligible units for order".into())
                    } else {
                        None
                    },
                    observation: Some(obs),
                    state: Some(format!("{step:?}")),
                }
            }
            RequestAction::Step { seconds } => {
                let step = step_seconds(started, *seconds);
                let obs = bounded(started.observe());
                FixtureResponse {
                    protocol: PROTOCOL_VERSION,
                    id: req.id,
                    ok: true,
                    error: None,
                    observation: Some(obs),
                    state: Some(format!("{step:?}")),
                }
            }
            RequestAction::Cancel { units, seconds } => {
                let issued = issue_cancel(started, units);
                let step = step_seconds(started, *seconds);
                let obs = bounded(started.observe());
                FixtureResponse {
                    protocol: PROTOCOL_VERSION,
                    id: req.id,
                    ok: issued > 0,
                    error: if issued == 0 {
                        Some("no eligible units for cancel".into())
                    } else {
                        None
                    },
                    observation: Some(obs),
                    state: Some(format!("{step:?}")),
                }
            }
        };
        write_response(dir, &resp)?;
        append_journal(dir, req.id)?;
        let _ = consume_request(dir);
        processed += 1;
    }
}

#[cfg(test)]
mod unit_fixture_tests {
    use super::*;

    #[test]
    fn validate_rejects_oversized_and_invalid_requests() {
        let bad = FixtureRequest {
            protocol: PROTOCOL_VERSION,
            id: 1,
            action: RequestAction::Move {
                units: vec![],
                target: [0.0, 0.0],
                seconds: 5.0,
            },
        };
        assert!(validate(&bad).is_err());
        let bad = FixtureRequest {
            protocol: 99,
            id: 1,
            action: RequestAction::Step { seconds: 1.0 },
        };
        assert!(validate(&bad).is_err());
        let bad = FixtureRequest {
            protocol: PROTOCOL_VERSION,
            id: 1,
            action: RequestAction::Step {
                seconds: MAX_STEP_SECONDS + 1.0,
            },
        };
        assert!(validate(&bad).is_err());
        let good = FixtureRequest {
            protocol: PROTOCOL_VERSION,
            id: 1,
            action: RequestAction::Move {
                units: vec![1, 2],
                target: [10.0, 20.0],
                seconds: 5.0,
            },
        };
        assert!(validate(&good).is_ok());
    }

    #[test]
    fn protocol_roundtrip_and_cancel() {
        // Serde roundtrip of a move request preserves every field, and the
        // graceful Stop validates; unknown fields are rejected (cancel of an
        // unrecognized extension fails closed).
        let req = FixtureRequest {
            protocol: PROTOCOL_VERSION,
            id: 7,
            action: RequestAction::Move {
                units: vec![1, 2, 3],
                target: [10.0, -20.0],
                seconds: 2.5,
            },
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: FixtureRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
        assert!(validate(&back).is_ok());

        let stop = FixtureRequest {
            protocol: PROTOCOL_VERSION,
            id: 8,
            action: RequestAction::Stop,
        };
        assert!(validate(&stop).is_ok());
        let stop_json = serde_json::to_string(&stop).unwrap();
        let stop_back: FixtureRequest = serde_json::from_str(&stop_json).unwrap();
        assert_eq!(stop_back, stop);

        // Unknown field must be rejected, not silently ignored.
        let with_extra =
            r#"{"protocol":1,"id":9,"action":{"type":"stop"},"teleport":[0,0]}"#;
        assert!(serde_json::from_str::<FixtureRequest>(with_extra).is_err());

        // Canonical cancel roundtrip (serde): Cancel validates and
        // preserves its fields. This is the campaign cancel path; the
        // ignored canonical test proves the live order clears. The
        // pcx_sim Move/Stop unit test below is test-only and does not
        // drive campaign control.
        let cancel = FixtureRequest {
            protocol: PROTOCOL_VERSION,
            id: 10,
            action: RequestAction::Cancel {
                units: vec![1, 2],
                seconds: 1.0,
            },
        };
        assert!(validate(&cancel).is_ok());
        let cancel_json = serde_json::to_string(&cancel).unwrap();
        let cancel_back: FixtureRequest = serde_json::from_str(&cancel_json).unwrap();
        assert_eq!(cancel_back, cancel);
        let bad_cancel = FixtureRequest {
            protocol: PROTOCOL_VERSION,
            id: 11,
            action: RequestAction::Cancel {
                units: vec![],
                seconds: 1.0,
            },
        };
        assert!(validate(&bad_cancel).is_err());

        // pcx-sim protocol roundtrip (test-only serde/step check, not
        // campaign control): move issues, stop cancels the target.
        let mut world = pcx_sim::World::empty();
        let id = world.spawn(0, 0, 0);
        let mut rng = pcx_sim::Rng::new(1);
        let move_input = pcx_sim::Input {
            tick: 0,
            commands: vec![pcx_sim::PlayerInput {
                player: 0,
                commands: vec![pcx_sim::Command::Move {
                    x: pcx_sim::FX_ONE * 4,
                    y: 0,
                }],
            }],
        };
        pcx_sim::step(&mut world, &move_input, &mut rng);
        assert_eq!(world.tick, 1);
        assert!(world.unit(id).unwrap().move_target.is_some());
        let stop_input = pcx_sim::Input {
            tick: 1,
            commands: vec![pcx_sim::PlayerInput {
                player: 0,
                commands: vec![pcx_sim::Command::Stop],
            }],
        };
        pcx_sim::step(&mut world, &stop_input, &mut rng);
        assert_eq!(world.tick, 2);
        assert!(world.unit(id).unwrap().move_target.is_none());
    }

    #[test]
    fn bounded_truncates_observation_units() {
        let mut obs = Observation {
            scope: "structured-state sensors (not a vision benchmark)",
            game_time: Some(0.0),
            local_player: Some(1),
            selection: vec![],
            units: (0..MAX_OBSERVATION_UNITS + 10)
                .map(|i| gameplay_observation::ObservedUnit {
                    id: format!("{i:x}"),
                    rawcode: "hfoo".into(),
                    position: Some([0.0, 0.0]),
                    position_source: "live",
                    life: Some(1.0),
                    max_life: Some(1.0),
                    current_order: Some(0),
                    screen: None,
                    in_viewport: false,
                })
                .collect(),
            camera_bounds: None,
        };
        obs = bounded(obs);
        assert_eq!(obs.units.len(), MAX_OBSERVATION_UNITS);
    }
}
