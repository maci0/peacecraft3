//! Ability profiles resolved against the same map overlay at startup and during play.
use pcx_objects::editor::{
    FieldKind, FieldSource, Overlay, SlkId,
    defaults::{DefaultTables, Family},
};

pub(crate) fn base<'a>(
    tables: &'a DefaultTables,
    overlay: &'a Overlay,
    alias: [u8; 4],
) -> Result<(&'a str, &'a [pcx_objects::Mod]), String> {
    let alias = std::str::from_utf8(&alias).map_err(|_| "non-UTF8 ability alias")?;
    let (base, mods) = overlay
        .new_object(alias)
        .unwrap_or((alias, overlay.on(alias)));
    if overlay.new_object(base).is_some() {
        return Err("custom ability base chain unsupported".into());
    }
    // Returned base borrows the overlay for custom rows, or must borrow the table's spelling.
    let object = tables
        .object(Family::Ability, base)
        .map_err(|e| e.to_string())?;
    if object.rawcode() != base {
        return Err("ability rawcode spelling differs from the selected row".into());
    }
    Ok((object.rawcode(), mods))
}

pub(crate) fn profiles(
    tables: &DefaultTables,
    overlay: &Overlay,
    alias: [u8; 4],
    key: &str,
) -> Result<Vec<String>, String> {
    let (base, mods) = base(tables, overlay, alias)?;
    if let Some(entry) = mods.iter().find(|entry| {
        tables
            .editor()
            .field(entry.field)
            .is_none_or(|field| field.from != SlkId::AbilityMetaData)
    }) {
        return Err(format!(
            "{alias:?}: unknown ability profile override {:?}",
            entry.field
        ));
    }

    for entry in mods.iter().rev() {
        let field = tables
            .editor()
            .field(entry.field)
            .filter(|field| field.from == SlkId::AbilityMetaData)
            .ok_or_else(|| {
                format!(
                    "{alias:?}: unknown ability profile override {:?}",
                    entry.field
                )
            })?;
        let FieldSource::Profile {
            key: changed,
            index,
            slots,
        } = &field.source
        else {
            continue;
        };
        if !changed.eq_ignore_ascii_case(key) {
            continue;
        }
        let metadata = tables
            .metadata(field.fourcc)
            .ok_or_else(|| format!("{alias:?}: missing {key} profile metadata"))?;
        if metadata
            .attribute("repeat")
            .is_some_and(|value| !matches!(value.text.trim(), "" | "0" | "_" | "-"))
        {
            return Err(format!(
                "{alias:?}: ranked {key} profile override unsupported"
            ));
        }
        for (selector, excluded) in [("useSpecific", false), ("notSpecific", true)] {
            if let Some(value) = metadata.attribute(selector)
                && !matches!(value.text.trim(), "" | "_" | "-")
                && value.text.split(',').any(|id| id.trim() == base) == excluded
            {
                return Err(format!(
                    "{alias:?}: {key} override does not apply to {base}"
                ));
            }
        }
        if *slots > 1
            || !matches!(index, -1 | 0)
            || entry.selectors.is_some_and(|selectors| selectors != [0, 0])
        {
            return Err(format!(
                "{alias:?}: indexed or ranked {key} profile override unsupported"
            ));
        }
        if entry.value_type != pcx_objects::ValueType::Str
            || !matches!(field.kind, FieldKind::Token | FieldKind::Char)
        {
            return Err(format!("{alias:?}: wrong type for {key} profile override"));
        }
        let pcx_objects::Value::Str(bytes) = &entry.value else {
            return Err(format!("{alias:?}: nonstring {key} profile override"));
        };
        let text = std::str::from_utf8(bytes)
            .map_err(|_| format!("{alias:?}: non-UTF8 {key} profile override"))?
            .trim();
        return Ok(if matches!(text, "" | "_" | "-") {
            Vec::new()
        } else {
            vec![text.to_owned()]
        });
    }
    let cells = tables
        .object(Family::Ability, base)
        .map_err(|e| e.to_string())?
        .profile(key);
    if let Some(bad) = cells.iter().find(|c| c.parse_error.is_some()) {
        return Err(format!("{alias:?}: malformed {key} profile {:?}", bad.text));
    }
    Ok(cells
        .iter()
        .map(|c| c.text.trim())
        .filter(|t| !matches!(*t, "" | "_" | "-"))
        .map(str::to_owned)
        .collect())
}
