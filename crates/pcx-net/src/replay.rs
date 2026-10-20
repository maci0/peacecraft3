//! Native replay container for a recorded session.
//!
//! A replay holds the seed, the caller's map SHA-1, the tick-0 world snapshot, and
//! for each tick the disconnect stops and the accepted inputs. Playback runs the
//! same sim step from the same seed and checks the final state digest. This is not
//! the original Warcraft III W3G format; see `docs/re-notes/replay-recording.md`.

use pcx_sim::{AdmittedMovement, Input, PlayerInput, Rng, World, step};
use sha1::{Digest, Sha1};
use std::sync::Arc;

use crate::session::{MAX_PLAYERS, commands_in_bounds, stop_owned_moves};
use crate::{
    CodecError, decode_input_payload, decode_world_payload, encode_input_payload,
    encode_world_payload,
};

const MAGIC: &[u8; 8] = b"PCX3REPL";
/// Byte layout version. Playback refuses any other value.
pub const FORMAT_VERSION: u8 = 1;
const DIGEST_BYTES: usize = 20;

pub type MapSha1 = [u8; DIGEST_BYTES];

/// Collects one session's ticks until [`crate::session::Session::stop_recording`].
pub(crate) struct Recorder {
    seed: u64,
    map_sha1: MapSha1,
    start: Vec<u8>,
    ticks: Vec<Tick>,
}

struct Tick {
    stopped: Vec<u8>,
    inputs: Vec<PlayerInput>,
}

impl Recorder {
    /// `world` must be at tick 0; the session checks this.
    pub(crate) fn new(world: &World, seed: u64, map_sha1: MapSha1) -> Self {
        Self {
            seed,
            map_sha1,
            start: encode_world_payload(world),
            ticks: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, stopped: Vec<u8>, input: &Input) {
        self.ticks.push(Tick {
            stopped,
            inputs: input.commands.clone(),
        });
    }

    pub(crate) fn finish(self, final_world: &World) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.push(FORMAT_VERSION);
        out.extend_from_slice(&self.seed.to_le_bytes());
        out.extend_from_slice(&self.map_sha1);
        put_len32(&mut out, self.start.len());
        out.extend_from_slice(&self.start);
        out.extend_from_slice(&(self.ticks.len() as u64).to_le_bytes());
        for tick in &self.ticks {
            // Players are distinct and below MAX_PLAYERS, so each count fits a byte.
            out.push(u8::try_from(tick.stopped.len()).expect("at most MAX_PLAYERS stops"));
            out.extend_from_slice(&tick.stopped);
            out.push(u8::try_from(tick.inputs.len()).expect("at most MAX_PLAYERS inputs"));
            for input in &tick.inputs {
                out.push(input.player);
                let payload = encode_input_payload(&input.commands);
                put_len32(&mut out, payload.len());
                out.extend_from_slice(&payload);
            }
        }
        let mut body = Sha1::new();
        body.update(&out);
        out.extend_from_slice(&<[u8; DIGEST_BYTES]>::from(body.finalize()));
        out.extend_from_slice(&state_digest(final_world));
        out
    }
}

/// A replay that passed every check and reproduced its recorded final state.
#[derive(Debug, PartialEq, Eq)]
pub struct Replay {
    /// The caller's map identity, unverified here; compare it to the loaded map.
    pub map_sha1: MapSha1,
    pub seed: u64,
    pub world: World,
}

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("replay is truncated at byte {at}")]
    Truncated { at: usize },
    #[error("not a native replay: bad magic")]
    BadMagic,
    #[error("unsupported replay format version {found}; this build reads {supported}")]
    UnsupportedVersion { found: u8, supported: u8 },
    #[error("invalid replay: {0}")]
    Invalid(&'static str),
    #[error("invalid replay snapshot: {0}")]
    Snapshot(#[from] CodecError),
    #[error("replay has {remaining} trailing bytes")]
    Trailing { remaining: usize },
    #[error("replay body does not match its checksum")]
    Corrupt,
    #[error("replayed state does not match the recorded final digest")]
    Divergence,
}

/// Parses and replays a native replay. Any byte that changes the result is refused.
pub fn play(bytes: &[u8], movement: Option<Arc<AdmittedMovement>>) -> Result<Replay, ReplayError> {
    let footer_len = 2 * DIGEST_BYTES;
    let split = bytes
        .len()
        .checked_sub(footer_len)
        .ok_or(ReplayError::Truncated { at: bytes.len() })?;
    let (body, footer) = bytes.split_at(split);
    let (body_digest, state) = footer.split_at(DIGEST_BYTES);
    let mut hasher = Sha1::new();
    hasher.update(body);
    if hasher.finalize().as_slice() != body_digest {
        return Err(ReplayError::Corrupt);
    }
    let mut r = Reader { bytes: body, at: 0 };
    if r.take(MAGIC.len())? != MAGIC {
        return Err(ReplayError::BadMagic);
    }
    let found = r.u8()?;
    if found != FORMAT_VERSION {
        return Err(ReplayError::UnsupportedVersion {
            found,
            supported: FORMAT_VERSION,
        });
    }
    let seed = r.u64()?;
    let map_sha1 = r.array::<DIGEST_BYTES>()?;
    let start_len = r.u32()? as usize;
    let mut world = decode_world_payload(0, r.take(start_len)?)?;
    world.movement = movement;
    let mut rng = Rng::new(seed);
    // Each tick costs at least two bytes, so the count cannot force an allocation.
    let ticks = r.u64()?;
    for tick in 0..ticks {
        let stop_count = r.u8()?;
        let stopped = r.take(usize::from(stop_count))?;
        check_players(stopped)?;
        stop_owned_moves(&mut world, stopped);
        let input_count = r.u8()?;
        let mut inputs = Vec::with_capacity(usize::from(input_count));
        for _ in 0..input_count {
            let player = r.u8()?;
            let len = r.u32()? as usize;
            let commands = decode_input_payload(r.take(len)?)?;
            if !commands_in_bounds(&commands) {
                return Err(ReplayError::Invalid(
                    "tick commands exceed the packet limits or coordinate range",
                ));
            }
            inputs.push(PlayerInput { player, commands });
        }
        let players: Vec<u8> = inputs.iter().map(|p| p.player).collect();
        check_players(&players)?;
        step(
            &mut world,
            &Input {
                tick,
                commands: inputs,
            },
            &mut rng,
        );
    }
    if r.at != body.len() {
        return Err(ReplayError::Trailing {
            remaining: body.len() - r.at,
        });
    }
    if state != state_digest(&world) {
        return Err(ReplayError::Divergence);
    }
    Ok(Replay {
        map_sha1,
        seed,
        world,
    })
}

/// Refuses an out-of-range or repeated player index within one tick.
fn check_players(players: &[u8]) -> Result<(), ReplayError> {
    let mut seen = [false; MAX_PLAYERS as usize];
    for &player in players {
        let slot = seen
            .get_mut(usize::from(player))
            .ok_or(ReplayError::Invalid("player index is outside the session"))?;
        if std::mem::replace(slot, true) {
            return Err(ReplayError::Invalid("a player appears twice in one tick"));
        }
    }
    Ok(())
}

/// SHA-1 over the final tick and the world's wire payload.
fn state_digest(world: &World) -> MapSha1 {
    let mut hasher = Sha1::new();
    hasher.update(world.tick.to_le_bytes());
    hasher.update(encode_world_payload(world));
    hasher.finalize().into()
}

fn put_len32(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(
        &u32::try_from(len)
            .expect("replay section fits in u32")
            .to_le_bytes(),
    );
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ReplayError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(ReplayError::Truncated { at: self.at })?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ReplayError> {
        Ok(self
            .take(N)?
            .try_into()
            .expect("take returns exactly N bytes"))
    }

    fn u8(&mut self) -> Result<u8, ReplayError> {
        Ok(self.array::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, ReplayError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ReplayError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_sim::Command;

    const MAP: MapSha1 = [0x5A; DIGEST_BYTES];

    fn world() -> World {
        let mut world = World::empty();
        world.spawn(0, 0, 0);
        world.spawn(1, 0, 0);
        world
    }

    /// One recorded tick holding the given per-player commands, then the replay bytes.
    fn one_tick(inputs: Vec<PlayerInput>) -> Vec<u8> {
        let base = world();
        let mut recorder = Recorder::new(&base, 3, MAP);
        let input = Input {
            tick: 0,
            commands: inputs,
        };
        let mut live = base.clone();
        step(&mut live, &input, &mut Rng::new(3));
        recorder.push(Vec::new(), &input);
        recorder.finish(&live)
    }

    fn player(player: u8, commands: Vec<Command>) -> PlayerInput {
        PlayerInput { player, commands }
    }

    #[test]
    fn recorded_tick_replays_to_the_same_world() {
        let bytes = one_tick(vec![player(1, vec![Command::Move { x: 1 << 16, y: 0 }])]);
        let replay = play(&bytes, None).unwrap();
        assert_eq!(replay.map_sha1, MAP);
        assert_eq!(replay.seed, 3);
        assert_eq!(replay.world.tick, 1);
        assert!(replay.world.units[1].x > 0);
    }

    #[test]
    fn coordinates_outside_the_session_range_are_refused() {
        let bytes = one_tick(vec![player(0, vec![Command::Move { x: i32::MIN, y: 0 }])]);
        assert!(matches!(
            play(&bytes, None),
            Err(ReplayError::Invalid(m)) if m.contains("coordinate")
        ));
    }

    #[test]
    fn a_player_index_outside_the_session_is_refused() {
        let bytes = one_tick(vec![player(MAX_PLAYERS, vec![Command::Nop])]);
        assert!(matches!(play(&bytes, None), Err(ReplayError::Invalid(_))));
    }

    #[test]
    fn a_player_repeated_in_one_tick_is_refused() {
        let bytes = one_tick(vec![
            player(0, vec![Command::Nop]),
            player(0, vec![Command::Nop]),
        ]);
        assert!(matches!(
            play(&bytes, None),
            Err(ReplayError::Invalid("a player appears twice in one tick"))
        ));
    }

    #[test]
    fn a_tampered_final_digest_is_a_divergence() {
        let mut bytes = one_tick(Vec::new());
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(matches!(play(&bytes, None), Err(ReplayError::Divergence)));
    }
}
