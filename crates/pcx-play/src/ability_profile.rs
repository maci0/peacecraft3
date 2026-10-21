//! Ability profiles resolved against the same map overlay at startup and during play.
use pcx_objects::editor::{
    FieldKind, FieldSource, Overlay, SlkId,
    defaults::{DefaultTables, Family},
};

/// Authored presentation of one learned, available ability. This is not a cast-readiness check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Button {
    pub ability: [u8; 4],
    pub rank: i32,
    pub art: String,
    /// Source column and row; row zero is the top of the command grid.
    pub position: [usize; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tooltip {
    pub title: String,
    pub description: String,
}

pub(crate) fn ranked(
    tables: &DefaultTables,
    overlay: &Overlay,
    alias: [u8; 4],
    key: &str,
    rank: i32,
    levels: i32,
) -> Result<String, String> {
    if rank < 1 || rank > levels {
        return Err(format!("{alias:?}: {key} rank {rank} outside {levels}"));
    }
    let (base, mods) = base(tables, overlay, alias)?;
    if let Some(entry) = mods.iter().find(|entry| {
        tables
            .editor()
            .field(entry.field)
            .is_none_or(|field| field.from != SlkId::AbilityMetaData)
    }) {
        return Err(format!(
            "{alias:?}: unknown tooltip override {:?}",
            entry.field
        ));
    }
    for entry in mods.iter().rev() {
        let field = tables
            .editor()
            .field(entry.field)
            .filter(|field| field.from == SlkId::AbilityMetaData)
            .ok_or_else(|| format!("{alias:?}: unknown tooltip override {:?}", entry.field))?;
        let FieldSource::Profile {
            key: mapped,
            index,
            slots,
        } = &field.source
        else {
            continue;
        };
        if !mapped.eq_ignore_ascii_case(key) {
            continue;
        }
        let meta = tables
            .metadata(field.fourcc)
            .ok_or("missing tooltip metadata")?;
        let repeated = match meta.attribute("repeat").map(|v| v.text.trim()) {
            None | Some("" | "_" | "-") => false,
            Some(value) => {
                value
                    .parse::<u32>()
                    .map_err(|_| format!("{alias:?}: invalid {key} repeat metadata"))?
                    != 0
            }
        };
        if !repeated || *slots > 1 || !matches!(index, -1 | 0) {
            return Err(format!("{alias:?}: unsupported {key} profile mapping"));
        }
        for selector in ["data", "appendIndex"] {
            if meta
                .attribute(selector)
                .is_some_and(|value| !matches!(value.text.trim(), "" | "0" | "_" | "-"))
            {
                return Err(format!("{alias:?}: unsupported {key} {selector} mapping"));
            }
        }
        let Some([level, data]) = entry.selectors else {
            return Err(format!("{alias:?}: {key} override has no rank"));
        };
        if data != 0 || level == 0 {
            return Err(format!("{alias:?}: unsupported {key} selector"));
        }
        if level != rank as u32 {
            continue;
        }
        for (selector, excluded) in [("useSpecific", false), ("notSpecific", true)] {
            if let Some(value) = meta.attribute(selector)
                && !matches!(value.text.trim(), "" | "_" | "-")
                && value.text.split(',').any(|id| id.trim() == base) == excluded
            {
                return Err(format!(
                    "{alias:?}: {key} override does not apply to {base}"
                ));
            }
        }
        if entry.value_type != pcx_objects::ValueType::Str
            || !matches!(field.kind, FieldKind::Token | FieldKind::Char)
        {
            return Err(format!("{alias:?}: nonstring {key} override"));
        }
        let pcx_objects::Value::Str(raw) = &entry.value else {
            return Err(format!("{alias:?}: nonstring {key} override"));
        };
        return std::str::from_utf8(raw)
            .map(str::to_owned)
            .map_err(|_| format!("{alias:?}: non-UTF8 {key} override"));
    }
    let cell = tables
        .object(Family::Ability, base)
        .map_err(|e| e.to_string())?
        .profile_element(key, rank - 1, levels as u32)
        .map_err(|e| e.to_string())?;
    if cell.parse_error.is_some() {
        return Err(format!("{alias:?}: malformed {key} profile"));
    }
    Ok(cell.text.to_owned())
}

pub(crate) fn button_position(
    tables: &DefaultTables,
    overlay: &Overlay,
    alias: [u8; 4],
) -> Result<[usize; 2], String> {
    let (base, mods) = base(tables, overlay, alias)?;
    let object = tables
        .object(Family::Ability, base)
        .map_err(|error| error.to_string())?;
    let mut result = [0; 2];
    for (index, coordinate) in result.iter_mut().enumerate() {
        let mut fields = tables.editor().fields().iter().filter(|field| {
            field.from == SlkId::AbilityMetaData
                && matches!(&field.source,
                FieldSource::Profile { key, index: slot, slots: 2 }
                if key.eq_ignore_ascii_case("Buttonpos") && *slot == index as i32)
        });
        let field = fields
            .next()
            .ok_or_else(|| format!("{alias:?}: missing Buttonpos/{index} metadata"))?;
        if fields.next().is_some() || field.kind != FieldKind::Int {
            return Err(format!(
                "{alias:?}: ambiguous or noninteger Buttonpos/{index} metadata"
            ));
        }
        let metadata = tables
            .metadata(field.fourcc)
            .ok_or_else(|| format!("{alias:?}: missing Buttonpos/{index} metadata record"))?;
        for selector in [
            "repeat",
            "data",
            "appendIndex",
            "useSpecific",
            "notSpecific",
        ] {
            if metadata
                .attribute(selector)
                .is_some_and(|value| !matches!(value.text.trim(), "" | "0" | "_" | "-"))
            {
                return Err(format!(
                    "{alias:?}: unsupported Buttonpos/{index} {selector}"
                ));
            }
        }
        let value = if let Some(modification) =
            mods.iter().rev().find(|entry| entry.field == field.fourcc)
        {
            if modification.value_type != pcx_objects::ValueType::Int
                || modification
                    .selectors
                    .is_some_and(|selectors| selectors != [0, 0])
            {
                return Err(format!("{alias:?}: unsupported Buttonpos/{index} override"));
            }
            let pcx_objects::Value::Int(value) = modification.value else {
                return Err(format!("{alias:?}: noninteger Buttonpos/{index} override"));
            };
            i64::from(value as i32)
        } else {
            let stock = object
                .field(field.fourcc)
                .map_err(|error| error.to_string())?;
            let pcx_objects::editor::defaults::Value::Integer(value) = stock.value else {
                return Err(format!("{alias:?}: noninteger Buttonpos/{index}"));
            };
            value
        };
        *coordinate = usize::try_from(value)
            .map_err(|_| format!("{alias:?}: negative or oversized Buttonpos/{index}"))?;
    }
    Ok(result)
}

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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL and PCX_WAR3_EXE from the selected local install"]
    fn original_button_positions_agree_with_authored_profiles_and_client_grid() {
        let install = std::path::PathBuf::from(
            std::env::var_os("PCX_NATIVE_CHAPTER_INSTALL").expect("set PCX_NATIVE_CHAPTER_INSTALL"),
        );
        let client_path =
            std::path::PathBuf::from(std::env::var_os("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE"));
        let image = std::fs::read(&client_path).expect("selected executable");
        let identity =
            crate::campaign_inputs::client_identity(&image, &client_path.display().to_string());
        let grid = crate::command_layout::Rules::reviewed_legacy(&image, identity.clone()).unwrap();
        let tables =
            DefaultTables::open(&install, identity.version).expect("selected object tables");
        for alias in [*b"AHhb", *b"AHwe"] {
            let row = pcx_objects::fourcc_text(alias);
            let object = tables.object(Family::Ability, &row).unwrap();
            let authored = object.profile("Buttonpos");
            let [authored] = authored.as_slice() else {
                panic!("{row}: ambiguous Buttonpos")
            };
            let raw: Vec<usize> = authored
                .text
                .split(',')
                .map(|value| value.trim().parse().unwrap())
                .collect();
            let position = button_position(&tables, &Overlay::default(), alias).unwrap();
            assert_eq!(position.as_slice(), raw, "{row}: metadata slot mapping");
            let rect = grid
                .slot(position[0], position[1])
                .expect("source button fits source grid");
            let art = profiles(&tables, &Overlay::default(), alias, "Art").unwrap();
            assert_eq!(art.len(), 1, "{row}: one authored icon");
            assert!(!art[0].is_empty());
            let levels: i32 = object
                .cell(SlkId::AbilityData, "levels")
                .unwrap()
                .text
                .parse()
                .unwrap();
            assert!(levels > 1, "source hero spell must supply ranked profiles");
            let rank = (levels + 1) / 2;
            for key in ["Tip", "UberTip"] {
                let fields: Vec<_> = tables
                    .editor()
                    .fields()
                    .iter()
                    .filter(|field| {
                        field.from == SlkId::AbilityMetaData
                            && matches!(&field.source,
                        FieldSource::Profile {key: mapped, ..} if mapped.eq_ignore_ascii_case(key))
                    })
                    .collect();
                let [field] = fields.as_slice() else {
                    panic!("{row}: ambiguous {key} metadata")
                };
                let mut overlay = Overlay::default();
                overlay.mods.insert(
                    row.clone(),
                    vec![pcx_objects::Mod {
                        field: field.fourcc,
                        value_type: pcx_objects::ValueType::Str,
                        selectors: Some([rank as u32, 0]),
                        value: pcx_objects::Value::Str(b"Synthetic ranked map text".to_vec()),
                        end_word: 0,
                    }],
                );
                assert_eq!(
                    ranked(&tables, &overlay, alias, key, rank, levels).unwrap(),
                    "Synthetic ranked map text"
                );
                let other = if rank > 1 { rank - 1 } else { rank + 1 };
                assert_eq!(
                    ranked(&tables, &overlay, alias, key, other, levels).unwrap(),
                    object
                        .profile_element(key, other - 1, levels as u32)
                        .unwrap()
                        .text
                );
            }
            println!("ability={row} position={position:?} rect={rect:?}");
        }
    }
}
