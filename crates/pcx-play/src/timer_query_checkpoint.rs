//! Exact current-entry fields; decoding never projects a future expiry.
use super::{Epoch, NumericPolicy, Rules, real};
use pcx_script::checkpoint::{Reader, Writer};
use sha2::{Digest, Sha256};

impl Rules {
    pub(crate) fn checkpoint_binding(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(self.image);
        for value in [self.segment, self.minimum, self.zero] {
            hash.update(value.to_le_bytes());
        }
        hash.finalize().into()
    }

    fn validate_checkpoint_epoch(&self, epoch: Epoch) -> Result<(), String> {
        for value in [
            epoch.period,
            epoch.remainder,
            epoch.first,
            epoch.expiry.to_bits(),
        ] {
            real::from_f32(f32::from_bits(value)).map_err(|e| e.to_string())?;
        }
        if !self.check_epoch(epoch)
            || epoch.remaining_count > epoch.whole
            || f32::from_bits(epoch.remainder) < 0.
            || f32::from_bits(epoch.remainder) >= f32::from_bits(self.segment)
            || epoch.period
                != if epoch.whole > 0 {
                    self.segment
                } else {
                    self.clamp(epoch.remainder)
                }
        {
            return Err("checkpoint timer epoch differs from its source policy".into());
        }
        Ok(())
    }

    pub(crate) fn write_checkpoint_epoch(
        &self,
        epoch: Epoch,
        out: &mut Writer,
    ) -> Result<(), String> {
        self.validate_checkpoint_epoch(epoch)?;
        out.raw(&self.checkpoint_binding())?;
        out.raw(&epoch.image)?;
        out.u32(epoch.period)?;
        out.u32(u32::from(epoch.whole))?;
        out.u32(u32::from(epoch.remaining_count))?;
        out.u32(epoch.remainder)?;
        out.u32(epoch.first)?;
        out.real(epoch.expiry)
    }

    pub(crate) fn read_checkpoint_epoch(&self, input: &mut Reader<'_>) -> Result<Epoch, String> {
        if input.raw(32)? != self.checkpoint_binding() {
            return Err("checkpoint timer query source differs".into());
        }
        let epoch = Epoch {
            image: input
                .raw(32)?
                .try_into()
                .map_err(|_| "checkpoint timer image")?,
            period: input.u32()?,
            whole: u16::try_from(input.u32()?).map_err(|_| "checkpoint timer whole count")?,
            remaining_count: u16::try_from(input.u32()?)
                .map_err(|_| "checkpoint timer remaining count")?,
            remainder: input.u32()?,
            first: input.u32()?,
            expiry: input.real()?,
        };
        self.validate_checkpoint_epoch(epoch)?;
        Ok(epoch)
    }

    pub(crate) fn validate_checkpoint_sleep(
        &self,
        numeric: &NumericPolicy,
        epoch: Epoch,
        origin: u32,
        delay: u32,
    ) -> Result<(), String> {
        self.validate_checkpoint_epoch(epoch)?;
        let initial =
            self.begin_sleep(numeric, f32::from_bits(origin), f32::from_bits(delay), 1)?;
        if epoch.image != initial.image
            || epoch.period != initial.period
            || epoch.whole != initial.whole
            || epoch.remainder != initial.remainder
            || epoch.expiry.to_bits() != initial.expiry.to_bits()
        {
            return Err("checkpoint sleep epoch differs from its original request".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_script::checkpoint::Budget;

    fn budget() -> Budget {
        Budget {
            bytes: 4096,
            items: 64,
            depth: 16,
        }
    }

    #[test]
    fn epoch_checkpoint_preserves_current_count_and_rebased_entry_without_reprojecting_shadow() {
        let numeric = super::super::tests::policy(8, 0.125);
        let rules = numeric.timer_queries().unwrap();
        let mut epoch = rules.begin_sleep(&numeric, 0.75, 20.25, 1).unwrap();
        assert!(!rules.advance_entry(&numeric, &mut epoch).unwrap());
        rules
            .rebase_epoch(&numeric, &mut epoch, 16f32.to_bits())
            .unwrap();
        assert_eq!(epoch.whole, 2);
        assert_eq!(epoch.remaining_count, 1);
        assert_eq!(epoch.first, 0.75f32.to_bits());
        assert_eq!(epoch.expiry.to_bits(), 8.75f32.to_bits());
        let mut out = Writer::new(budget()).unwrap();
        rules.write_checkpoint_epoch(epoch, &mut out).unwrap();
        let bytes = out.finish();
        let mut input = Reader::new(&bytes, budget()).unwrap();
        let mut restored = rules.read_checkpoint_epoch(&mut input).unwrap();
        input.finish().unwrap();
        assert_eq!(restored, epoch);
        assert_eq!(
            rules
                .remaining_current(&numeric, restored, 0.5)
                .unwrap()
                .to_bits(),
            rules
                .remaining_current(&numeric, epoch, 0.5)
                .unwrap()
                .to_bits()
        );
        for _ in 0..2 {
            assert_eq!(
                rules.advance_entry(&numeric, &mut restored).unwrap(),
                rules.advance_entry(&numeric, &mut epoch).unwrap()
            );
            assert_eq!(restored, epoch);
        }
        let legacy = rules.begin(&numeric, -2., 0.5, 1).unwrap();
        let mut out = Writer::new(budget()).unwrap();
        rules.write_checkpoint_epoch(legacy, &mut out).unwrap();
        assert_eq!(legacy.first, (-1.5f32).to_bits());
    }

    #[test]
    fn epoch_checkpoint_rejects_truncation_invalid_counts_and_same_image_foreign_policy() {
        let numeric = super::super::tests::policy(8, 0.125);
        let rules = numeric.timer_queries().unwrap();
        let epoch = rules.begin_sleep(&numeric, 0.75, 20.25, 1).unwrap();
        let mut out = Writer::new(budget()).unwrap();
        rules.write_checkpoint_epoch(epoch, &mut out).unwrap();
        let bytes = out.finish();
        for cut in 0..bytes.len() {
            assert!(
                rules
                    .read_checkpoint_epoch(&mut Reader::new(&bytes[..cut], budget()).unwrap())
                    .is_err()
            );
        }
        for (at, value) in [(64, 7f32.to_bits()), (68, 1u32 << 16), (72, 3)] {
            let mut corrupt = bytes.clone();
            corrupt[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(
                rules
                    .read_checkpoint_epoch(&mut Reader::new(&corrupt, budget()).unwrap())
                    .is_err()
            );
        }
        let foreign = Rules {
            image: rules.image,
            segment: 16f32.to_bits(),
            minimum: rules.minimum,
            zero: rules.zero,
        };
        assert!(
            foreign
                .read_checkpoint_epoch(&mut Reader::new(&bytes, budget()).unwrap())
                .is_err()
        );
    }
}
