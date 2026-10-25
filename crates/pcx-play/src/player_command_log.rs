//! Accepted player commands of one campaign session, with the identity it started from.
//!
//! The application records a command once, after the authoritative native accepts
//! it, so script-issued natives stay out of the log. The first record that cannot
//! be kept faults the log for good: later commands still run, nothing more is
//! recorded, and [`PlayerCommandLog::save_readiness`] refuses. Clock values are
//! whatever the caller reads from the session's timer clock; this log does not
//! convert or reinterpret them.

use pcx_script::HandleId;

use crate::map_admission::{MapInputSources, ParsedMapContext};

/// Most entries one session keeps. A longer session faults the log instead of growing it.
pub const MAX_LOG_ENTRIES: usize = 1 << 20;

/// A player command as the application accepted it.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerCommand {
    /// A registered immediate order issued to a unit.
    Immediate { unit: HandleId, order: i32 },
    /// Replace the selection with `units`, or add them when `replace` is false.
    Select { units: Vec<HandleId>, replace: bool },
    /// A ground order such as `move` to `(x, y)`.
    Point {
        unit: HandleId,
        order: String,
        x: f32,
        y: f32,
    },
    /// An order aimed at a unit or item handle, such as `attack` or `smart`.
    Target {
        unit: HandleId,
        order: String,
        target: HandleId,
    },
    /// A local-player event fired by the application, such as end-of-cinematic.
    LocalEvent { name: String },
    /// `SelectHeroSkill` for one admitted ability rawcode.
    Learn { unit: HandleId, ability: i32 },
}

/// What a session started from. The map is identified by its SHA-256 and by the
/// same asset identities admission already records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartIdentity {
    /// The seed the session was prepared with.
    pub seed: i32,
    /// The admitted map's label, as its digest records it.
    pub map_label: String,
    /// SHA-256 of the admitted map bytes.
    pub map_sha256: [u8; 32],
    /// Map, common, Blizzard and catalog identities of the admitted context.
    pub sources: MapInputSources,
}

impl StartIdentity {
    /// Reads the map identity from an admitted context, so no path is restated here.
    pub fn from_context(seed: i32, context: &ParsedMapContext) -> Self {
        Self {
            seed,
            map_label: context.map_digest.label.clone(),
            map_sha256: context.map_sha256,
            sources: context.sources.clone(),
        }
    }
}

/// One command and the clock value it was accepted at.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub clock: f64,
    pub command: PlayerCommand,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum LogError {
    #[error("no session start identity is recorded")]
    NoStart,
    #[error("the session clock is not running, so the command has no time")]
    NoClock,
    #[error("clock {clock} is not finite")]
    NonFiniteClock { clock: f64 },
    #[error("clock {clock} precedes the last recorded clock {last}")]
    OutOfOrder { clock: f64, last: f64 },
    #[error("the log already holds its {limit} entries")]
    Full { limit: usize },
}

/// Why a session's commands cannot be saved as a replay.
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum SaveRefusal {
    #[error("the command log faulted: {0}")]
    Faulted(LogError),
    #[error("no session has been started")]
    NoStart,
    /// The host does not yet produce the initial-state snapshot a replay needs:
    /// campaign VM, globals, objects and timers at map start.
    #[error("the initial-state snapshot of the session is not captured")]
    NoInitialSnapshot,
}

#[derive(Debug, Default, PartialEq)]
pub struct PlayerCommandLog {
    start: Option<StartIdentity>,
    entries: Vec<Entry>,
    fault: Option<LogError>,
}

impl PlayerCommandLog {
    /// Begins a log for a newly prepared session, discarding any earlier log and fault.
    pub fn begin(&mut self, start: StartIdentity) {
        *self = Self {
            start: Some(start),
            entries: Vec::new(),
            fault: None,
        };
    }

    pub fn start(&self) -> Option<&StartIdentity> {
        self.start.as_ref()
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Appends a command at `clock`. The first refusal faults the log; later records repeat it.
    pub fn record(&mut self, clock: Option<f64>, command: PlayerCommand) -> Result<(), LogError> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        let result = self.append(clock, command);
        if let Err(error) = &result {
            self.fault = Some(error.clone());
        }
        result
    }

    fn append(&mut self, clock: Option<f64>, command: PlayerCommand) -> Result<(), LogError> {
        if self.start.is_none() {
            return Err(LogError::NoStart);
        }
        let clock = clock.ok_or(LogError::NoClock)?;
        if !clock.is_finite() {
            return Err(LogError::NonFiniteClock { clock });
        }
        if let Some(last) = self.entries.last().map(|e| e.clock)
            && clock < last
        {
            return Err(LogError::OutOfOrder { clock, last });
        }
        if self.entries.len() == MAX_LOG_ENTRIES {
            return Err(LogError::Full {
                limit: MAX_LOG_ENTRIES,
            });
        }
        self.entries.push(Entry { clock, command });
        Ok(())
    }

    /// Whether the recorded commands can be saved as a replay. Always refused until
    /// the initial-state snapshot exists.
    pub fn save_readiness(&self) -> Result<(), SaveRefusal> {
        if let Some(fault) = &self.fault {
            return Err(SaveRefusal::Faulted(fault.clone()));
        }
        if self.start.is_none() {
            return Err(SaveRefusal::NoStart);
        }
        Err(SaveRefusal::NoInitialSnapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripts::AssetIdentity;

    fn handle(n: u64) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(n).expect("non-zero fixture handle"))
    }

    fn identity(source: &str) -> AssetIdentity {
        AssetIdentity {
            edition: "fixture".into(),
            version: "fixture".into(),
            source: source.into(),
        }
    }

    fn start() -> StartIdentity {
        StartIdentity {
            seed: 7,
            map_label: "fixture.w3x".into(),
            map_sha256: [0xA5; 32],
            sources: MapInputSources {
                map: identity("map"),
                common: identity("common"),
                blizzard: identity("blizzard"),
                catalog: identity("catalog"),
            },
        }
    }

    fn select() -> PlayerCommand {
        PlayerCommand::Select {
            units: vec![handle(1)],
            replace: true,
        }
    }

    #[test]
    fn a_missing_start_or_clock_faults_the_log_and_later_records_repeat_the_fault() {
        let mut log = PlayerCommandLog::default();
        assert_eq!(log.record(Some(1.0), select()), Err(LogError::NoStart));
        log.begin(start());
        assert_eq!(log.record(None, select()), Err(LogError::NoClock));
        assert_eq!(log.record(Some(2.0), select()), Err(LogError::NoClock));
        assert!(log.entries().is_empty());
    }

    #[test]
    fn non_finite_and_out_of_order_clocks_fault_the_log() {
        let mut log = PlayerCommandLog::default();
        log.begin(start());
        assert!(matches!(
            log.record(Some(f64::NAN), select()),
            Err(LogError::NonFiniteClock { .. })
        ));
        log.begin(start());
        log.record(Some(2.0), select()).unwrap();
        assert_eq!(
            log.record(Some(1.5), select()),
            Err(LogError::OutOfOrder {
                clock: 1.5,
                last: 2.0
            })
        );
        assert!(matches!(
            log.save_readiness(),
            Err(SaveRefusal::Faulted(LogError::OutOfOrder { .. }))
        ));
        assert_eq!(log.entries().len(), 1);
    }

    #[test]
    fn the_log_is_bounded_and_a_full_log_faults() {
        let mut log = PlayerCommandLog::default();
        log.begin(start());
        for n in 0..MAX_LOG_ENTRIES {
            log.record(
                Some(n as f64),
                PlayerCommand::LocalEvent {
                    name: String::new(),
                },
            )
            .unwrap();
        }
        assert_eq!(
            log.record(Some(MAX_LOG_ENTRIES as f64), select()),
            Err(LogError::Full {
                limit: MAX_LOG_ENTRIES
            })
        );
        assert_eq!(log.entries().len(), MAX_LOG_ENTRIES);
    }

    #[test]
    fn save_is_refused_until_the_initial_state_snapshot_exists() {
        let mut log = PlayerCommandLog::default();
        assert_eq!(log.save_readiness(), Err(SaveRefusal::NoStart));
        log.begin(start());
        log.record(Some(1.0), select()).unwrap();
        assert_eq!(log.save_readiness(), Err(SaveRefusal::NoInitialSnapshot));
    }

    #[test]
    fn begin_replaces_the_previous_session_log_and_its_fault() {
        let mut log = PlayerCommandLog::default();
        log.begin(start());
        assert!(log.record(None, select()).is_err());
        log.begin(StartIdentity { seed: 8, ..start() });
        assert!(log.entries().is_empty());
        assert_eq!(log.start().map(|s| s.seed), Some(8));
        assert_eq!(log.save_readiness(), Err(SaveRefusal::NoInitialSnapshot));
    }
}
