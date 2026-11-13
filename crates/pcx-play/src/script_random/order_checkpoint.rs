use super::{CallSite, DrawLog, DrawRecord};
use pcx_script::checkpoint::{Reader, Writer};

fn word_out(out: &mut Writer, value: Option<u32>) -> Result<(), String> {
    out.boolean(value.is_some())?;
    if let Some(word) = value {
        out.u32(word)?;
    }
    Ok(())
}

fn word_in(input: &mut Reader<'_>) -> Result<Option<u32>, String> {
    if input.boolean()? {
        input.u32().map(Some)
    } else {
        Ok(None)
    }
}

fn validate(record: &DrawRecord, secondary_count: usize) -> Result<(), String> {
    let secondary = match record.site {
        CallSite::SecondaryDie { channel } | CallSite::SecondaryReal { channel } => {
            if channel >= secondary_count || record.primary_word.is_none() {
                return Err("checkpoint random caller has a foreign secondary stream".into());
            }
            true
        }
        _ => false,
    };
    if secondary != record.secondary_word.is_some() {
        return Err("checkpoint random caller and secondary word differ".into());
    }
    Ok(())
}

impl DrawLog {
    pub fn write_checkpoint(&self, out: &mut Writer, secondary_count: usize) -> Result<(), String> {
        out.list(self.records.len())?;
        out.owned(std::mem::size_of_val(self.records.as_slice()))?;
        for record in &self.records {
            validate(record, secondary_count)?;
            match &record.site {
                CallSite::RaceResolution { slot } => {
                    out.byte(0)?;
                    out.u32(*slot)?;
                }
                CallSite::Wander { unit } => {
                    out.byte(1)?;
                    out.u32(*unit)?;
                }
                CallSite::ScriptInteger => out.byte(2)?,
                CallSite::ScriptReal => out.byte(3)?,
                CallSite::SecondaryDie { channel } | CallSite::SecondaryReal { channel } => {
                    out.byte(if matches!(record.site, CallSite::SecondaryDie { .. }) {
                        4
                    } else {
                        5
                    })?;
                    out.u64(u64::try_from(*channel).map_err(|_| "random channel overflow")?)?;
                }
                CallSite::Gameplay { caller } => {
                    out.byte(6)?;
                    out.text(caller)?;
                }
            }
            word_out(out, record.primary_word)?;
            word_out(out, record.secondary_word)?;
        }
        Ok(())
    }

    pub fn read_checkpoint(input: &mut Reader<'_>, secondary_count: usize) -> Result<Self, String> {
        let count = input.list()?;
        input.owned(
            count
                .checked_mul(std::mem::size_of::<DrawRecord>())
                .ok_or("checkpoint random log size overflow")?,
        )?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(count)
            .map_err(|_| "checkpoint random log allocation failed")?;
        for _ in 0..count {
            let site = match input.byte()? {
                0 => CallSite::RaceResolution { slot: input.u32()? },
                1 => CallSite::Wander { unit: input.u32()? },
                2 => CallSite::ScriptInteger,
                3 => CallSite::ScriptReal,
                tag @ (4 | 5) => {
                    let channel = usize::try_from(input.u64()?)
                        .map_err(|_| "checkpoint random channel overflow")?;
                    if tag == 4 {
                        CallSite::SecondaryDie { channel }
                    } else {
                        CallSite::SecondaryReal { channel }
                    }
                }
                6 => CallSite::Gameplay {
                    caller: input.text()?.into_boxed_str(),
                },
                _ => return Err("unknown checkpoint random caller".into()),
            };
            let record = DrawRecord {
                site,
                primary_word: word_in(input)?,
                secondary_word: word_in(input)?,
            };
            validate(&record, secondary_count)?;
            records.push(record);
        }
        Ok(Self { records })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_script::checkpoint::Budget;

    const BUDGET: Budget = Budget {
        bytes: 8192,
        items: 128,
        depth: 8,
    };

    #[test]
    fn random_order_section_retains_each_caller_and_refuses_foreign_channel_or_phase() {
        let records = vec![
            DrawRecord {
                site: CallSite::RaceResolution { slot: 7 },
                primary_word: Some(19),
                secondary_word: None,
            },
            DrawRecord {
                site: CallSite::Wander { unit: 29 },
                primary_word: Some(23),
                secondary_word: None,
            },
            DrawRecord {
                site: CallSite::ScriptInteger,
                primary_word: None,
                secondary_word: None,
            },
            DrawRecord {
                site: CallSite::ScriptReal,
                primary_word: Some(31),
                secondary_word: None,
            },
            DrawRecord {
                site: CallSite::SecondaryDie { channel: 4 },
                primary_word: Some(31),
                secondary_word: Some(37),
            },
            DrawRecord {
                site: CallSite::SecondaryReal { channel: 2 },
                primary_word: Some(31),
                secondary_word: Some(41),
            },
            DrawRecord {
                site: CallSite::gameplay("synthetic\0é").unwrap(),
                primary_word: Some(43),
                secondary_word: None,
            },
        ];
        let log = DrawLog { records };
        let mut out = Writer::new(BUDGET).unwrap();
        log.write_checkpoint(&mut out, 5).unwrap();
        let bytes = out.finish();
        let mut input = Reader::new(&bytes, BUDGET).unwrap();
        let loaded = DrawLog::read_checkpoint(&mut input, 5).unwrap();
        input.finish().unwrap();
        assert_eq!(loaded, log);
        let mut input = Reader::new(&bytes, BUDGET).unwrap();
        assert!(
            DrawLog::read_checkpoint(&mut input, 4)
                .unwrap_err()
                .contains("secondary")
        );
        let mut bad = bytes;
        bad[4] = 255;
        let mut input = Reader::new(&bad, BUDGET).unwrap();
        assert!(
            DrawLog::read_checkpoint(&mut input, 5)
                .unwrap_err()
                .contains("caller")
        );
        let invalid = DrawLog {
            records: vec![DrawRecord {
                site: CallSite::SecondaryDie { channel: 1 },
                primary_word: Some(1),
                secondary_word: None,
            }],
        };
        assert!(
            invalid
                .write_checkpoint(&mut Writer::new(BUDGET).unwrap(), 5)
                .is_err()
        );
        let invalid = DrawLog {
            records: vec![DrawRecord {
                site: CallSite::ScriptInteger,
                primary_word: Some(1),
                secondary_word: Some(2),
            }],
        };
        assert!(
            invalid
                .write_checkpoint(&mut Writer::new(BUDGET).unwrap(), 5)
                .is_err()
        );
    }
}
