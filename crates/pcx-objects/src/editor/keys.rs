//! Resolve UnitWeapons identity from authored row relationships, never a header alias.
use std::collections::{BTreeMap, BTreeSet};

use super::SlkId;

pub(super) fn weapon_identity_column(
    tables: &BTreeMap<SlkId, pcx_slq::Table>,
) -> Result<u32, String> {
    let id = SlkId::UnitWeapons;
    let table = tables
        .get(&id)
        .ok_or_else(|| format!("{}: missing table", id.path()))?;
    let expected = id.id_column().expect("UnitWeapons identity invariant");
    if let Some(column) = named_column(table, expected, id.path())? {
        row_keys(table, column, id.path())?;
        return Ok(column);
    }
    let mut anchors = Vec::new();
    for canonical in [SlkId::UnitData, SlkId::UnitBalance] {
        let Some(table) = tables.get(&canonical) else {
            continue;
        };
        let header = canonical
            .id_column()
            .expect("canonical unit identity invariant");
        let column = named_column(table, header, canonical.path())?.ok_or_else(|| {
            format!(
                "{}: missing canonical identity header {header:?}",
                canonical.path()
            )
        })?;
        let keys = row_keys(table, column, canonical.path())?;
        if keys.is_empty() {
            return Err(format!(
                "{}: no canonical unit keys for identity derivation",
                canonical.path()
            ));
        }
        anchors.push(keys);
    }
    if anchors.is_empty() {
        return Err(format!(
            "{}: missing identity header {expected:?}; cannot derive row keys without UnitData or UnitBalance",
            id.path()
        ));
    }
    let mut candidates = Vec::new();
    for (column, header) in table.headers() {
        let Ok(keys) = row_keys(table, column, id.path()) else {
            continue;
        };
        if !keys.is_empty() && anchors.iter().all(|anchor| anchor.is_subset(&keys)) {
            candidates.push((column, header));
        }
    }
    match candidates.as_slice() {
        [(column, _)] => Ok(*column),
        [] => Err(format!(
            "{}: missing identity header {expected:?}; no column has a complete unique fourcc join to the supplied canonical unit tables",
            id.path()
        )),
        _ => Err(format!(
            "{}: ambiguous identity columns {candidates:?} match the supplied canonical unit tables",
            id.path()
        )),
    }
}

fn named_column(table: &pcx_slq::Table, name: &str, member: &str) -> Result<Option<u32>, String> {
    let mut columns = table.headers().filter(|(_, header)| *header == name);
    let first = columns.next().map(|(column, _)| column);
    if columns.next().is_some() {
        return Err(format!("{member}: duplicate identity header {name:?}"));
    }
    Ok(first)
}

fn row_keys(table: &pcx_slq::Table, column: u32, member: &str) -> Result<BTreeSet<String>, String> {
    if table
        .anomalies()
        .iter()
        .any(|anomaly| anomaly.severity == pcx_slq::Severity::Error)
    {
        return Err(format!("{member}: SYLK has unrecoverable anomalies"));
    }
    let mut keys = BTreeSet::new();
    for row in table.data_rows().filter(|row| !row.is_empty()) {
        let value = row.get(column).ok_or_else(|| {
            format!(
                "{member}: row {} missing identity cell in column {column}",
                row.index()
            )
        })?;
        if value.len() != 4 || !value.is_ascii() {
            return Err(format!(
                "{member}: row {} column {column} identity is not an ASCII fourcc",
                row.index()
            ));
        }
        if !keys.insert(value.to_ascii_lowercase()) {
            return Err(format!(
                "{member}: duplicate identity {value:?} in column {column}"
            ));
        }
    }
    Ok(keys)
}
