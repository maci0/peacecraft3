//! The shared audio generator; initialization requires a recorded presentation word.
use super::rules::Rules;
use crate::script_random::state::Generator;
use std::{collections::BTreeMap, sync::Arc};

/// A recreation memory/work bound, independent of source sound-table contents.
const MAX_CHOICE_ROWS: usize = 65536;

#[derive(Clone, Debug)]
pub struct AudioRandom {
    rules: Arc<Rules>,
    stream: Generator,
    initialized_at: u32,
    last: Arc<BTreeMap<String, usize>>,
}

fn charge(work: &mut usize) -> Result<(), String> {
    *work = work
        .checked_sub(1)
        .ok_or("audio random work budget exhausted")?;
    Ok(())
}
fn draw(stream: &mut Generator, rules: &Rules, work: &mut usize) -> Result<u32, String> {
    charge(work)?;
    stream.next(&rules.random).map_err(|e| e.to_string())
}
fn count(count: usize) -> Result<u32, String> {
    let count =
        u32::try_from(count).map_err(|_| "audio random member count exceeds native range")?;
    if count == 0 {
        return Err("audio random list has no members".into());
    }
    Ok(count)
}
fn reduce(word: u32, count: u32) -> usize {
    ((u64::from(word) * u64::from(count)) >> u32::BITS) as usize
}

impl AudioRandom {
    pub fn new(rules: Arc<Rules>, initialized_at: u32) -> Self {
        let stream = Generator::new(&rules.random, initialized_at);
        Self {
            rules,
            stream,
            initialized_at,
            last: Arc::new(BTreeMap::new()),
        }
    }
    pub fn initialized_at(&self) -> u32 {
        self.initialized_at
    }

    /// Music and simple labels draw even when the list has only one member.
    pub fn choose(&mut self, members: usize, work: &mut usize) -> Result<usize, String> {
        let members = count(members)?;
        let mut stream = self.stream.clone();
        let chosen = reduce(draw(&mut stream, &self.rules, work)?, members);
        self.stream = stream;
        Ok(chosen)
    }

    /// Generic sound-record history is independent of the music playlist index.
    pub fn choose_non_repeating(
        &mut self,
        record: &str,
        members: usize,
        work: &mut usize,
    ) -> Result<usize, String> {
        let members = count(members)?;
        if record.is_empty() || record.len() >= self.rules.name_bytes {
            return Err("audio random record identity exceeds the source name bound".into());
        }
        if !self.last.contains_key(record) && self.last.len() >= MAX_CHOICE_ROWS {
            return Err("audio random sound-record history budget exhausted".into());
        }
        charge(work)?;
        let previous = self.last.get(record).copied();
        let mut stream = self.stream.clone();
        let mut chosen = 0;
        if members > 1 {
            for _ in 0..self.rules.repeat_attempts {
                chosen = reduce(draw(&mut stream, &self.rules, work)?, members);
                if Some(chosen) != previous {
                    break;
                }
            }
        }
        let mut key = String::new();
        key.try_reserve_exact(record.len())
            .map_err(|_| "audio random record allocation")?;
        key.push_str(record);
        Arc::make_mut(&mut self.last).insert(key, chosen);
        self.stream = stream;
        Ok(chosen)
    }

    pub fn pitch(
        &mut self,
        base: f32,
        variance: f32,
        random: bool,
        work: &mut usize,
    ) -> Result<f32, String> {
        if !base.is_finite()
            || !variance.is_finite()
            || base <= 0.0
            || variance < 0.0
            || variance >= base
        {
            return Err("unsupported audio pitch or variance".into());
        }
        if !random {
            return Ok(base);
        }
        let mut stream = self.stream.clone();
        let word = draw(&mut stream, &self.rules, work)?;
        let packed = f32::from_bits((word & self.rules.pitch_mask) | self.rules.pitch_base);
        let delta = if word.cast_signed() < 0 {
            self.rules.pitch_one - packed
        } else {
            packed - self.rules.pitch_one
        };
        // Source x87 evaluates the product and sum before the caller stores binary32.
        let pitch = (f64::from(base) + f64::from(variance) * f64::from(delta)) as f32;
        if !pitch.is_finite() || pitch <= 0.0 {
            return Err("source audio pitch is not playable".into());
        }
        self.stream = stream;
        Ok(pitch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rules() -> Arc<Rules> {
        Arc::new(Rules::fixture())
    }
    #[test]
    fn audio_random_single_music_member_draws_but_single_generic_member_does_not() {
        let rules = rules();
        let seed = 0xfedc_0123;
        let mut music = AudioRandom::new(Arc::clone(&rules), seed);
        let mut sound = music.clone();
        let mut expected = Generator::new(&rules.random, seed);
        expected.next(&rules.random).unwrap();
        assert_eq!(music.choose(1, &mut 100).unwrap(), 0);
        assert_eq!(music.stream, expected);
        assert_eq!(sound.choose_non_repeating("row", 1, &mut 100).unwrap(), 0);
        assert_eq!(sound.stream, Generator::new(&rules.random, seed));
    }
    #[test]
    fn audio_random_non_repeat_exhaustion_accepts_duplicate_and_refusal_preserves_state() {
        let mut rules = Rules::fixture();
        let mut policy = crate::script_random::profile::fixture();
        policy.table.fill(0);
        rules.random = Arc::new(policy);
        let mut rng = AudioRandom::new(Arc::new(rules), 0);
        let mut work = 100;
        assert_eq!(rng.choose_non_repeating("row", 2, &mut work).unwrap(), 0);
        assert_eq!(work, 98);
        let expected = rng.clone();
        assert!(rng.choose_non_repeating("row", 2, &mut 3).is_err());
        assert_eq!(rng.stream, expected.stream);
        assert_eq!(rng.last, expected.last);
        let mut work = 100;
        assert_eq!(rng.choose_non_repeating("row", 2, &mut work).unwrap(), 0);
        assert_eq!(work, 96);
    }
    #[test]
    fn audio_random_unflagged_pitch_and_invalid_pitch_leave_stream_unchanged() {
        let mut rng = AudioRandom::new(rules(), 0x2391);
        let old = rng.stream.clone();
        assert_eq!(rng.pitch(1.25, 0.125, false, &mut 0).unwrap(), 1.25);
        assert!(rng.pitch(0.0, 0.0, true, &mut 100).is_err());
        assert_eq!(rng.stream, old);
        let value = rng.pitch(1.25, 0.125, true, &mut 100).unwrap();
        assert!((1.125..=1.375).contains(&value));
    }
}
