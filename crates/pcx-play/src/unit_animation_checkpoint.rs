//! The independent animation stream joins the same world transaction as gameplay RNG.
use super::{Rules, State};
use pcx_script::checkpoint::{Reader, Writer};
use sha2::{Digest, Sha256};
use std::sync::Arc;

fn number(hash: &mut Sha256, value: usize) -> Result<(), String> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| "animation rule size overflow")?
            .to_le_bytes(),
    );
    Ok(())
}
fn text(hash: &mut Sha256, value: &str) -> Result<(), String> {
    number(hash, value.len())?;
    hash.update(value.as_bytes());
    Ok(())
}
fn binding(rules: &Rules) -> Result<[u8; 32], String> {
    let Rules {
        origin,
        image_sha256,
        rare,
        sequence_limit,
        rare_initial,
        seed,
        mask,
        base,
        one,
        tags,
        input_delimiters,
        model_delimiters,
        fallback,
        token_limits,
        property_token_limit,
        property_capacity,
        props,
    } = rules;
    let mut hash = Sha256::new();
    for value in [&origin.source, &origin.edition, &origin.version] {
        text(&mut hash, value)?;
    }
    hash.update(image_sha256);
    hash.update(rare.to_le_bytes());
    number(&mut hash, *sequence_limit)?;
    for value in [rare_initial.to_bits(), *seed, *mask, *base, one.to_bits()] {
        hash.update(value.to_le_bytes());
    }
    number(&mut hash, tags.len())?;
    for (tag, value) in tags {
        text(&mut hash, tag)?;
        hash.update(value.to_le_bytes());
    }
    for value in [input_delimiters, model_delimiters, props] {
        text(&mut hash, value)?;
    }
    hash.update(fallback.to_le_bytes());
    for value in [
        token_limits[0],
        token_limits[1],
        *property_token_limit,
        *property_capacity,
    ] {
        number(&mut hash, value)?;
    }
    Ok(hash.finalize().into())
}

impl State {
    pub(crate) fn write_checkpoint(&self, out: &mut Writer) -> Result<(), String> {
        out.raw(&binding(&self.rules)?)?;
        self.stream.write_checkpoint(out, &self.policy)
    }
    pub(crate) fn read_checkpoint(&self, input: &mut Reader<'_>) -> Result<Self, String> {
        if input.raw(32)? != binding(&self.rules)? {
            return Err("checkpoint animation rules differ from their candidate sources".into());
        }
        Ok(Self {
            rules: Arc::clone(&self.rules),
            policy: Arc::clone(&self.policy),
            stream: crate::script_random::state::Generator::read_checkpoint(input, &self.policy)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_script::checkpoint::Budget;

    #[test]
    fn independent_random_section_binds_every_nonstock_rule_before_restoring_stream() {
        let policy = Arc::new(crate::script_random::profile::fixture());
        let mut rules = crate::unit_animation_rarity::tests::rules();
        rules.image_sha256 = *policy.image_sha256();
        let mut live = State::new(rules.clone(), Arc::clone(&policy)).unwrap();
        for _ in 0..9 {
            live.choose(&[0.25, 0.5, 0.25]).unwrap();
        }
        let budget = Budget {
            bytes: 4096,
            items: 128,
            depth: 8,
        };
        let mut out = Writer::new(budget).unwrap();
        live.write_checkpoint(&mut out).unwrap();
        let bytes = out.finish();
        let fresh = State::new(rules.clone(), Arc::clone(&policy)).unwrap();
        let mut input = Reader::new(&bytes, budget).unwrap();
        let mut loaded = fresh.read_checkpoint(&mut input).unwrap();
        input.finish().unwrap();
        for _ in 0..29 {
            assert_eq!(
                loaded.choose(&[0.25, 0.5, 0.25]).unwrap(),
                live.choose(&[0.25, 0.5, 0.25]).unwrap()
            );
        }
        rules.seed ^= 1;
        let foreign = State::new(rules, policy).unwrap();
        let mut input = Reader::new(&bytes, budget).unwrap();
        assert!(
            foreign
                .read_checkpoint(&mut input)
                .err()
                .unwrap()
                .contains("rules")
        );
    }
}
