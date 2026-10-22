//! Source-checked campaign cost queries over the admitted unit catalog.
use crate::neutral_player::{RuleInputBudget, sha256};
use crate::neutral_rule_input::Pe;
use pcx_objects::editor::{FieldKind, FieldSource, ObjectKind, SlkId};
use pcx_sim::world::{Definition, FieldValue};

#[derive(Clone, Debug)]
pub(crate) struct Policy {
    pub(crate) hero_state: i32,
    upper_first: u8,
    upper_last: u8,
    zero_from: i32,
}

fn call(bytes: &[u8], va: u32, at: usize) -> Option<u32> {
    if bytes.get(at) != Some(&0xe8) {
        return None;
    }
    let delta = i32::from_le_bytes(bytes.get(at + 1..at + 5)?.try_into().ok()?);
    Some(va.wrapping_add(at as u32 + 5).wrapping_add_signed(delta))
}

impl Policy {
    /// These addresses identify four functions in the checked selected image;
    /// the branch operands and state ordinal are read from its bytes.
    pub(crate) fn selected(image: &[u8]) -> Result<Self, String> {
        if sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("cost image differs from reviewed client".into());
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 0,
                sections: 96,
            },
        )
        .map_err(|e| e.to_string())?;
        let window = |va, len| -> Result<&[u8], String> {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..len)
                .ok_or_else(|| "cost image window is short".into())
        };
        let get = window(0x494a80, 96)?;
        let set = window(0x4a97a0, 171)?;
        let gold = window(0xab5430, 143)?;
        let wood = window(0xab54c0, 143)?;
        let reader = call(get, 0x494a80, 0x2b).ok_or("cost state reader missing")?;
        if get.get(0x22..0x28) != Some(&[0x8d, 0x4e, 4, 0xc1, 0xe1, 4])
            || call(set, 0x4a97a0, 0xa3) != Some(0x7fb380)
            || reader != 0x7fb2b0
        {
            return Err("cost state accessor differs".into());
        }
        let mut parts = [gold, wood].into_iter().zip([0xab5430, 0xab54c0]);
        let (first, last, slot, threshold) = Self::branch(parts.next().unwrap())?;
        if Self::branch(parts.next().unwrap())? != (first, last, slot, threshold) {
            return Err("gold and wood cost policy differ".into());
        }
        let hero_state = i32::from(slot >> get[0x27]) - i32::from(get[0x24]);
        if slot != ((hero_state + i32::from(get[0x24])) << get[0x27]) as u8
            || hero_state < 0
            || hero_state >= i32::from(get[0x1a])
        {
            return Err("cost hero state is outside selected counter domain".into());
        }
        Ok(Self {
            hero_state,
            upper_first: first,
            upper_last: last,
            zero_from: threshold,
        })
    }

    fn branch((body, va): (&[u8], u32)) -> Result<(u8, u8, u8, i32), String> {
        if body.get(..12)
            != Some(&[
                0x55, 0x8b, 0xec, 0x8b, 0x45, 8, 0xc1, 0xe8, 24, 0x56, 0x83, 0xf8,
            ])
            || body.get(13) != Some(&0x72)
            || body.get(15..17) != Some(&[0x83, 0xf8])
            || body.get(18) != Some(&0x77)
            || body.get(0x7b..0x7d) != Some(&[0x8d, 0x4e])
            || body.get(0x83..0x85) != Some(&[0x83, 0xf8])
            || body.get(0x86) != Some(&0x7d)
            || call(body, va, 0x7e) != Some(0x7fb2b0)
            || body.get(0x8a) != Some(&0xe9)
        {
            return Err("cost native branch differs".into());
        }
        let first = body[12];
        let last = body[17];
        if first > last {
            return Err("cost upper range is invalid".into());
        }
        Ok((first, last, body[0x7d], i32::from(body[0x85])))
    }

    pub(crate) fn zero_cost(&self, rawcode: i32, tokens: i32) -> bool {
        (self.upper_first..=self.upper_last).contains(&rawcode.to_be_bytes()[0])
            && tokens >= self.zero_from
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub(crate) fn fixture(
        hero_state: i32,
        upper_first: u8,
        upper_last: u8,
        zero_from: i32,
    ) -> Self {
        Self {
            hero_state,
            upper_first,
            upper_last,
            zero_from,
        }
    }
}

pub(crate) fn amount(definition: &Definition, column: &str) -> Result<i32, String> {
    if definition.kind != ObjectKind::Unit {
        return Err("cost rawcode is not a unit".into());
    }
    let mut fields = definition.fields.iter().filter(|(id, field)| {
        field.metadata.as_ref().is_some_and(|meta| {
            meta.fourcc == **id
                && meta.from == SlkId::UnitMetaData
                && matches!(meta.index, -1 | 0)
                && meta.kind == FieldKind::Int
                && matches!(&meta.source, FieldSource::Column { slk: SlkId::UnitBalance, column: c } if c == column)
        })
    });
    let (_, field) = fields.next().ok_or("unit cost metadata missing")?;
    if fields.next().is_some() {
        return Err("unit cost metadata ambiguous".into());
    }
    let value = match field
        .value
        .as_ref()
        .map_err(|e| format!("unit cost: {e:?}"))?
    {
        FieldValue::Cell(cell) => cell
            .trim()
            .parse::<i32>()
            .map_err(|_| "unit cost cell is not an integer")?,
        FieldValue::Override(pcx_objects::Value::Int(bits)) => *bits as i32,
        _ => return Err("unit cost value kind unsupported".into()),
    };
    if value < 0 {
        return Err("unit cost is negative".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_objects::editor::FieldDef;
    use pcx_sim::world::ObjectField;

    fn definition(gold: FieldValue, wood: FieldValue) -> Definition {
        let fields = [(*b"ugol", "goldcost", gold), (*b"ulum", "lumbercost", wood)].map(
            |(fourcc, column, value)| {
                (
                    fourcc,
                    ObjectField {
                        metadata: Some(FieldDef {
                            fourcc,
                            from: SlkId::UnitMetaData,
                            field: column.into(),
                            slk: "UnitBalance".into(),
                            index: -1,
                            declared: "int".into(),
                            kind: FieldKind::Int,
                            category: "synthetic".into(),
                            source: FieldSource::Column {
                                slk: SlkId::UnitBalance,
                                column: column.into(),
                            },
                            measured: true,
                        }),
                        value: Ok(value),
                    },
                )
            },
        );
        Definition {
            rawcode: *b"Hxyz",
            base_rawcode: "Hxyz".into(),
            kind: ObjectKind::Unit,
            fields: fields.into(),
            modifications: Vec::new(),
        }
    }

    #[test]
    fn integer_cost_cells_and_map_overrides_stay_exact() {
        let source = definition(
            FieldValue::Cell("16777217".into()),
            FieldValue::Override(pcx_objects::Value::Int(37)),
        );
        assert_eq!(amount(&source, "goldcost"), Ok(16_777_217));
        assert_eq!(amount(&source, "lumbercost"), Ok(37));
        let invalid = definition(
            FieldValue::Cell("-1".into()),
            FieldValue::Cell("bad".into()),
        );
        assert!(amount(&invalid, "goldcost").is_err());
        assert!(amount(&invalid, "lumbercost").is_err());
    }

    #[test]
    fn policy_uses_derived_byte_range_and_signed_threshold() {
        let va = 0x700000;
        let mut body = [0u8; 143];
        body[..12].copy_from_slice(&[
            0x55, 0x8b, 0xec, 0x8b, 0x45, 8, 0xc1, 0xe8, 24, 0x56, 0x83, 0xf8,
        ]);
        body[12..19].copy_from_slice(&[b'B', 0x72, 1, 0x83, 0xf8, b'Y', 0x77]);
        body[0x7b..0x7e].copy_from_slice(&[0x8d, 0x4e, 0x70]);
        body[0x7e] = 0xe8;
        body[0x7f..0x83].copy_from_slice(&((0x7fb2b0u32 - (va + 0x83)) as i32).to_le_bytes());
        body[0x83..0x87].copy_from_slice(&[0x83, 0xf8, 3, 0x7d]);
        body[0x8a] = 0xe9;
        let (first, last, slot, threshold) = Policy::branch((&body, va)).unwrap();
        assert_eq!((first, last, slot, threshold), (b'B', b'Y', 0x70, 3));
        let policy = Policy::fixture(3, first, last, threshold);
        assert!(!policy.zero_cost(i32::from_be_bytes(*b"Hxyz"), 2));
        assert!(policy.zero_cost(i32::from_be_bytes(*b"Hxyz"), 3));
        assert!(!policy.zero_cost(i32::from_be_bytes(*b"Axyz"), 3));
        body[0x7d] = 0x71;
        assert_ne!(Policy::branch((&body, va)).unwrap().2, slot);
        body[0x86] = 0x73;
        assert!(Policy::branch((&body, va)).is_err());
    }

    #[test]
    #[ignore = "requires PCX_CAMPAIGN_IMAGE with selected legacy executable"]
    fn selected_image_cost_reader_shares_hero_token_slot() {
        let path = std::env::var("PCX_CAMPAIGN_IMAGE").unwrap();
        let image = std::fs::read(path).unwrap();
        let policy = Policy::selected(&image).unwrap();
        assert_eq!(policy.hero_state, 3);
        assert!(policy.zero_cost(i32::from_be_bytes(*b"Huth"), 1));
        assert!(!policy.zero_cost(i32::from_be_bytes(*b"hfoo"), 1));
    }
}
