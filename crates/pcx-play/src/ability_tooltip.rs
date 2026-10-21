//! Learned tooltip text, with bounded expansion of source row/column references.
use super::*;
use crate::ability_profile::{self, Tooltip};

const MAX_TEXT: usize = 64 << 10;
const MAX_TOKENS: usize = 256;

fn expand(
    raw: &str,
    mut read: impl FnMut([u8; 4], &str) -> Result<String, String>,
) -> Result<String, String> {
    if raw.len() > MAX_TEXT {
        return Err("tooltip text budget exceeded".into());
    }
    if raw.starts_with("TRIGSTR_") {
        return Err("tooltip map string is not resolved".into());
    }
    let mut result = String::with_capacity(raw.len());
    let mut tail = raw;
    let mut tokens = 0;
    while let Some(start) = tail.find('<') {
        if result
            .len()
            .checked_add(start)
            .is_none_or(|length| length > MAX_TEXT)
        {
            return Err("expanded tooltip text budget exceeded".into());
        }
        result.push_str(&tail[..start]);
        let end = tail[start..]
            .find('>')
            .map(|end| start + end)
            .ok_or("unterminated tooltip reference")?;
        let token = &tail[start + 1..end];
        let (row, field) = token
            .split_once(',')
            .ok_or_else(|| format!("unsupported tooltip reference <{token}>"))?;
        let row: [u8; 4] = row
            .as_bytes()
            .try_into()
            .map_err(|_| format!("invalid tooltip row <{token}>"))?;
        if field.is_empty()
            || !field
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err(format!("invalid tooltip field <{token}>"));
        }
        tokens += 1;
        if tokens > MAX_TOKENS {
            return Err("tooltip token budget exceeded".into());
        }
        let value = read(row, field).map_err(|e| format!("tooltip <{token}>: {e}"))?;
        if value.contains(['<', '>']) {
            return Err(format!("nested tooltip reference <{token}>"));
        }
        if result
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > MAX_TEXT)
        {
            return Err("expanded tooltip text budget exceeded".into());
        }
        result.push_str(&value);
        tail = &tail[end + 1..];
    }
    if result
        .len()
        .checked_add(tail.len())
        .is_none_or(|length| length > MAX_TEXT)
    {
        return Err("expanded tooltip text budget exceeded".into());
    }
    result.push_str(tail);
    Ok(result)
}

impl ObjectHost {
    pub fn ability_tooltip(
        &self,
        unit: HandleId,
        ability: [u8; 4],
    ) -> Result<Option<Tooltip>, Error> {
        let record = self.executed_unit(unit)?;
        let rank = record.abilities.as_ref().map_or(0, |a| a.level(ability));
        if rank <= 0 || !self.unit_ability_available(unit, ability)? {
            return Ok(None);
        }
        let read = |key| {
            let input = self.heroes_admission()?;
            let raw = ability_profile::ranked(
                &input.tables,
                &input.overlay,
                ability,
                key,
                rank,
                self.hero_ability_number(ability, "levels")?,
            )?;
            expand(&raw, |row, field| self.tooltip_value(row, field))
                .map_err(|e| format!("{ability:?} {key} rank {rank}: {e}"))
        };
        let tooltip = (|| {
            Ok::<_, String>(Tooltip {
                title: read("Tip")?,
                description: read("UberTip")?,
            })
        })()
        .map_err(|detail| self.hero_error(ABILITY, detail))?;
        Ok(Some(tooltip))
    }

    fn tooltip_value(&self, row: [u8; 4], field: &str) -> Result<String, String> {
        let input = self.heroes_admission()?;
        if input.base(row).is_ok() {
            let stem = field.trim_end_matches(|c: char| c.is_ascii_digit());
            let rank: i32 = field[stem.len()..]
                .parse()
                .map_err(|_| "ability reference has no rank")?;
            return self
                .hero_ability_level_cell(row, stem, rank, FieldKind::Int)
                .or_else(|_| self.hero_ability_level_cell(row, stem, rank, FieldKind::Real));
        }
        let definition = self
            .admitted_units()
            .map_err(|e| e.to_string())?
            .catalog
            .resolve(row)
            .map_err(|e| format!("{e:?}"))?;
        if !definition.modifications.is_empty() {
            return Err(
                "referenced unit tooltip columns with map modifications are not derived".into(),
            );
        }
        let object = input
            .tables
            .object(Family::Unit, &definition.base_rawcode)
            .map_err(|e| e.to_string())?;
        let cells = object.cells();
        let mut matching = cells.iter().filter(|cell| {
            matches!(&cell.provenance.location,
            defaults::Location::Slk { column, .. } if column == field)
        });
        let cell = matching
            .next()
            .ok_or_else(|| format!("missing unit column {field}"))?;
        if matching.next().is_some() {
            return Err(format!("ambiguous unit column {field}"));
        }
        if !matches!(
            cell.typed(FieldKind::Real).map_err(|e| e.to_string())?,
            defaults::Value::Real(_)
        ) {
            return Err(format!("missing numeric unit column {field}"));
        }
        Ok(cell.text.trim().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn references_preserve_markup_and_refuse_unbounded_or_unsupported_forms() {
        assert_eq!(
            expand("|cffaabbccValue|r <TEST,DataA2>|n", |row, field| {
                assert_eq!(row, *b"TEST");
                assert_eq!(field, "DataA2");
                Ok("17".into())
            })
            .unwrap(),
            "|cffaabbccValue|r 17|n"
        );
        for raw in [
            "<TEST,DataA1,%>",
            "<TEST,>",
            "<ABC,DataA1>",
            "<TEST,DataA1",
            "TRIGSTR_7",
        ] {
            assert!(expand(raw, |_, _| Ok("1".into())).is_err(), "{raw}");
        }
        assert!(
            expand("<TEST,DataA1>", |_, _| Err("missing field".into()))
                .unwrap_err()
                .contains("<TEST,DataA1>")
        );
        assert!(expand(&"x".repeat(MAX_TEXT + 1), |_, _| unreachable!()).is_err());
        assert!(expand(&"<TEST,A1>".repeat(MAX_TOKENS + 1), |_, _| Ok("1".into())).is_err());
        assert!(expand("<TEST,A1>", |_, _| Ok("x".repeat(MAX_TEXT + 1))).is_err());
    }
}
