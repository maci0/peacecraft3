//! Seeded state for the reviewed primary and retained secondary generators.
use super::{profile::Policy, real};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Primary {
    word: u32,
    lanes: [u8; 4],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    policy_key: [u8; 32],
    primary: Primary,
    secondary: Vec<Primary>,
}

/// A narrow copy of the primary generator state. Restoring it undoes consumed
/// primary draws without cloning the retained secondary block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PrimaryCheckpoint(Primary);

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StateError {
    #[error("no qualified random state; an explicit seed is required")]
    Uninitialized,
    #[error("random state and selected profile do not match")]
    Profile,
    #[error("random secondary allocation exceeds the caller budget")]
    Budget,
    #[error(transparent)]
    Arithmetic(#[from] real::MathError),
}

impl Primary {
    fn pack(policy: &Policy, seed: u32) -> Self {
        Self {
            word: seed,
            lanes: std::array::from_fn(|lane| {
                ((seed % policy.moduli[lane]) << policy.packing_shift) as u8
            }),
        }
    }

    fn preview(self, policy: &Policy) -> Self {
        let lanes = std::array::from_fn(|lane| {
            let next = i32::from(self.lanes[lane]) - policy.decrements[lane] as i32;
            (if next < 0 {
                next + policy.periods[lane] as i32
            } else {
                next
            }) as u8
        });
        let mixed = lanes.iter().enumerate().fold(0, |mixed, (lane, index)| {
            mixed
                ^ policy.table[usize::from(*index) / std::mem::size_of::<u32>()]
                    .rotate_left(policy.rotations[lane])
        });
        Self {
            word: self.word.wrapping_add(mixed),
            lanes,
        }
    }
}

impl State {
    /// Inspection of the recreation's retained state, not native address values.
    pub fn primary_word(&self) -> u32 {
        self.primary.word
    }
    pub fn secondary_words(&self) -> impl ExactSizeIterator<Item = u32> + '_ {
        self.secondary.iter().map(|state| state.word)
    }
    pub(crate) fn primary_checkpoint(&self) -> PrimaryCheckpoint {
        PrimaryCheckpoint(self.primary)
    }
    pub(crate) fn restore_primary(&mut self, checkpoint: PrimaryCheckpoint) {
        self.primary = checkpoint.0;
    }
    fn check(&self, policy: &Policy) -> Result<(), StateError> {
        if self.policy_key == policy.key {
            Ok(())
        } else {
            Err(StateError::Profile)
        }
    }
}

/// A seed call discards one primary draw and seeds the source-sized secondary
/// block from a separate local stream. No engine initial seed is supplied here.
pub fn seeded(policy: &Policy, seed: i32, max_secondary: usize) -> Result<State, StateError> {
    if policy.secondary_count > max_secondary {
        return Err(StateError::Budget);
    }
    let mut secondary = Vec::new();
    secondary
        .try_reserve_exact(policy.secondary_count)
        .map_err(|_| StateError::Budget)?;
    let primary = Primary::pack(policy, seed as u32).preview(policy);
    let mut local = Primary::pack(policy, primary.word);
    for _ in 0..policy.secondary_count {
        local = local.preview(policy);
        secondary.push(Primary::pack(policy, local.word));
    }
    Ok(State {
        policy_key: policy.key,
        primary,
        secondary,
    })
}

pub fn integer(
    policy: &Policy,
    state: &mut Option<State>,
    low: i32,
    high: i32,
) -> Result<i32, StateError> {
    if low == high {
        return Ok(low);
    }
    let current = state.as_mut().ok_or(StateError::Uninitialized)?;
    current.check(policy)?;
    let next = current.primary.preview(policy);
    let distance = u32::try_from((i64::from(high) - i64::from(low)).unsigned_abs()).unwrap();
    let width = distance.wrapping_add(1);
    let reduced = ((u64::from(next.word) * u64::from(width)) >> 32) as u32;
    let result = low.wrapping_add(reduced as i32);
    current.primary = next;
    Ok(result)
}

pub fn real(
    policy: &Policy,
    state: &mut Option<State>,
    low: f32,
    high: f32,
) -> Result<f32, StateError> {
    let left = real::from_f32(low)?;
    let right = real::from_f32(high)?;
    let difference = real::sub(left, right)?;
    if real::abs(difference) < policy.real_tolerance {
        return Ok(low);
    }
    let current = state.as_mut().ok_or(StateError::Uninitialized)?;
    current.check(policy)?;
    let span = if real::below_or_equal(left, right) {
        real::sub(right, left)?
    } else {
        real::sub(left, right)?
    };
    let next = current.primary.preview(policy);
    let fraction = (next.word & policy.real_mask) | policy.real_base;
    let offset = real::from_i32(policy.real_offset)?;
    let fraction = real::add(fraction, offset)?;
    let scaled = real::mul(span, fraction)?;
    let result = real::add(left, scaled)?;
    current.primary = next;
    Ok(f32::from_bits(result))
}
