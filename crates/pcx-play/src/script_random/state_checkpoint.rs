//! Dynamic generator state; the selected policy and its table remain local source bindings.
use super::{Policy, Primary, State};
use pcx_script::checkpoint::{Reader, Writer};

fn primary_out(out: &mut Writer, state: Primary, policy: &Policy) -> Result<(), String> {
    for (lane, period) in state.lanes.iter().zip(policy.periods) {
        if u32::from(*lane) >= period {
            return Err("checkpoint random lane is outside the selected period".into());
        }
    }
    out.u32(state.word)?;
    out.raw(&state.lanes)
}

fn primary_in(input: &mut Reader<'_>, policy: &Policy) -> Result<Primary, String> {
    let state = Primary {
        word: input.u32()?,
        lanes: input.raw(4)?.try_into().expect("four random lanes"),
    };
    if state
        .lanes
        .iter()
        .zip(policy.periods)
        .any(|(lane, period)| u32::from(*lane) >= period)
    {
        return Err("checkpoint random lane is outside the selected period".into());
    }
    Ok(state)
}

impl State {
    /// One world section, including every retained secondary stream. This does not
    /// replace the primary-only checkpoint used to roll back a refused native.
    pub fn write_checkpoint(&self, out: &mut Writer, policy: &Policy) -> Result<(), String> {
        self.check(policy).map_err(|e| e.to_string())?;
        if self.secondary.len() != policy.secondary_count {
            return Err("checkpoint random secondary count differs from its source".into());
        }
        out.raw(&self.policy_key)?;
        primary_out(out, self.primary, policy)?;
        out.list(self.secondary.len())?;
        out.owned(std::mem::size_of_val(self.secondary.as_slice()))?;
        for state in &self.secondary {
            primary_out(out, *state, policy)?;
        }
        Ok(())
    }

    /// Decode a candidate without reseeding, discarding a draw or advancing any stream.
    pub fn read_checkpoint(input: &mut Reader<'_>, policy: &Policy) -> Result<Self, String> {
        if input.raw(policy.key.len())? != policy.key {
            return Err("checkpoint random policy differs from its local source".into());
        }
        let primary = primary_in(input, policy)?;
        let count = input.list()?;
        if count != policy.secondary_count {
            return Err("checkpoint random secondary count differs from its source".into());
        }
        input.owned(
            count
                .checked_mul(std::mem::size_of::<Primary>())
                .ok_or("checkpoint random allocation size overflow")?,
        )?;
        let mut secondary = Vec::new();
        secondary
            .try_reserve_exact(count)
            .map_err(|_| "checkpoint random allocation failed")?;
        for _ in 0..count {
            secondary.push(primary_in(input, policy)?);
        }
        Ok(Self {
            policy_key: policy.key,
            primary,
            secondary,
        })
    }
}

impl super::Generator {
    pub(crate) fn write_checkpoint(&self, out: &mut Writer, policy: &Policy) -> Result<(), String> {
        if self.key != policy.key {
            return Err(
                "checkpoint independent random policy differs from its local source".into(),
            );
        }
        out.raw(&self.key)?;
        primary_out(out, self.primary, policy)
    }

    pub(crate) fn read_checkpoint(input: &mut Reader<'_>, policy: &Policy) -> Result<Self, String> {
        if input.raw(policy.key.len())? != policy.key {
            return Err(
                "checkpoint independent random policy differs from its local source".into(),
            );
        }
        Ok(Self {
            key: policy.key,
            primary: primary_in(input, policy)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script_random::{profile, state};
    use pcx_script::checkpoint::Budget;

    const BUDGET: Budget = Budget {
        bytes: 4096,
        items: 128,
        depth: 8,
    };

    fn encode(state: &State, policy: &Policy) -> Vec<u8> {
        let mut out = Writer::new(BUDGET).unwrap();
        state.write_checkpoint(&mut out, policy).unwrap();
        out.finish()
    }

    fn decode(bytes: &[u8], policy: &Policy) -> Result<State, String> {
        let mut input = Reader::new(bytes, BUDGET)?;
        let state = State::read_checkpoint(&mut input, policy)?;
        input.finish()?;
        Ok(state)
    }

    fn draw(policy: &Policy, state: &mut Option<State>) -> Vec<u32> {
        let mut result = Vec::new();
        for channel in 0..policy.secondary_count {
            result.push(state::secondary_die(policy, state, channel, 97).unwrap());
            result.push(
                state::secondary_real(policy, state, channel)
                    .unwrap()
                    .to_bits(),
            );
        }
        result.push(state::integer(policy, state, -91, 701).unwrap() as u32);
        result.push(state::real(policy, state, -7., 21.).unwrap().to_bits());
        result
    }

    #[test]
    fn complete_random_section_preserves_next_draws_in_all_nonstock_streams() {
        let mut policy = profile::fixture();
        policy.secondary_count = 5;
        let mut live = Some(state::seeded(&policy, -773, 5).unwrap());
        for channel in 0..5 {
            for _ in 0..channel + 1 {
                state::secondary_die(&policy, &mut live, channel, 13).unwrap();
            }
        }
        state::integer(&policy, &mut live, -3, 19).unwrap();
        let bytes = encode(live.as_ref().unwrap(), &policy);
        let mut loaded = Some(decode(&bytes, &policy).unwrap());
        assert_eq!(loaded, live);
        assert_eq!(encode(loaded.as_ref().unwrap(), &policy), bytes);
        for _ in 0..17 {
            assert_eq!(draw(&policy, &mut live), draw(&policy, &mut loaded));
        }
        assert_eq!(loaded, live);
    }

    #[test]
    fn random_section_refuses_foreign_policy_counts_lanes_and_truncation() {
        let policy = profile::fixture();
        let state = state::seeded(&policy, 33, 3).unwrap();
        let bytes = encode(&state, &policy);
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end], &policy).is_err(), "length {end}");
        }
        let mut foreign = policy.clone();
        foreign.key[0] ^= 1;
        assert!(decode(&bytes, &foreign).unwrap_err().contains("policy"));
        foreign = policy.clone();
        foreign.secondary_count += 1;
        assert!(decode(&bytes, &foreign).unwrap_err().contains("count"));
        let mut bad = bytes.clone();
        bad[36] = u8::try_from(policy.periods[0]).unwrap();
        assert!(decode(&bad, &policy).unwrap_err().contains("lane"));
        let mut bad = bytes.clone();
        bad[48] = u8::try_from(policy.periods[0]).unwrap();
        assert!(decode(&bad, &policy).unwrap_err().contains("lane"));
        let mut bad = bytes;
        bad.extend_from_slice(&[0]);
        assert!(decode(&bad, &policy).unwrap_err().contains("trailing"));
        assert_eq!(state, state::seeded(&policy, 33, 3).unwrap());
    }
}
