//! SLK parsing, SLK cell lookup, and object-editor catalog admission.
use std::collections::HashMap;

use pcx_objects::editor::{EDITOR_SLK, EDITOR_TXT, Editor, EditorReadLimits};
use pcx_slq::Table;

use crate::{
    Context, Failure, Outcome, Prepared, Workload,
    digest::Fnv,
    input::{self, extract_all},
};

pub const WORKLOADS: [Workload; 4] = [
    Workload {
        name: "slk-parse-v1",
        summary: "parse eleven fixed SLK tables (units, abilities, items, metadata, terrain)",
        prepare: parse,
    },
    Workload {
        name: "slk-lookup-v1",
        summary: "look up every cell of three tables by row and column name",
        prepare: lookup,
    },
    Workload {
        name: "editor-open-v1",
        summary: "Editor::from_reader over preloaded tables, as headless startup admits them",
        prepare: editor_reader,
    },
    Workload {
        name: "editor-open-install-v1",
        summary: "Editor::open_in over the four archives (listfile union, extraction, decode)",
        prepare: editor_install,
    },
];

const TABLES: [&str; 11] = [
    "Units\\UnitData.slk",
    "Units\\unitUI.slk",
    "Units\\UnitBalance.slk",
    "Units\\UnitWeapons.slk",
    "Units\\AbilityData.slk",
    "Units\\ItemData.slk",
    "Units\\UpgradeData.slk",
    "Units\\UnitMetaData.slk",
    "Units\\AbilityMetaData.slk",
    "Units\\DestructableData.slk",
    "TerrainArt\\Terrain.slk",
];

fn parse(ctx: &Context) -> Result<Prepared, Failure> {
    let mut input_digest = Fnv::default();
    let mut set = input::open_set_with_codec(&ctx.install)?;
    let files = extract_all(&mut set, &TABLES, &mut input_digest)?;
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut tables = Vec::new();
            for (name, bytes) in &files {
                tables.push(Table::parse(bytes).map_err(|e| format!("{name}: {e}"))?);
            }
            let cells: u64 = tables.iter().map(|t| t.cell_count() as u64).sum();
            let bytes: u64 = files.iter().map(|(_, b)| b.len() as u64).sum();
            Ok(Outcome {
                work: vec![
                    ("tables", tables.len() as u64),
                    ("cells", cells),
                    ("source_bytes", bytes),
                ],
                fold: Box::new(move |out| {
                    tables.iter().for_each(|table| fold_table(out, table));
                }),
            })
        }),
    })
}

const LOOKUP_TABLES: [&str; 3] = [
    "Units\\UnitData.slk",
    "Units\\unitUI.slk",
    "Units\\UnitBalance.slk",
];

/// Lookup passes over each table; one pass is every data row by every column name.
const LOOKUP_PASSES: usize = 2;

fn lookup(ctx: &Context) -> Result<Prepared, Failure> {
    let mut input_digest = Fnv::default();
    let mut set = input::open_set_with_codec(&ctx.install)?;
    let files = extract_all(&mut set, &LOOKUP_TABLES, &mut input_digest)?;
    let mut tables = Vec::new();
    for (name, bytes) in &files {
        let table = Table::parse(bytes).map_err(|e| format!("{name}: {e}"))?;
        let names: Vec<String> = table.headers().map(|(_, h)| h.to_string()).collect();
        tables.push((table, names));
    }
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut sum = Fnv::default();
            let mut lookups = 0u64;
            let mut hits = 0u64;
            for _ in 0..LOOKUP_PASSES {
                for (table, names) in &tables {
                    for row in table.data_rows() {
                        for name in names {
                            lookups += 1;
                            if let Some(value) = row.by_name(name) {
                                hits += 1;
                                sum.text(value);
                            }
                        }
                    }
                }
            }
            Ok(Outcome {
                work: vec![("lookups", lookups), ("hits", hits)],
                fold: Box::new(move |out| out.u64(sum.finish())),
            })
        }),
    })
}

const ADMISSION_LIMITS: EditorReadLimits = EditorReadLimits {
    max_listed_paths: usize::MAX,
    max_asset_bytes: 64 << 20,
    max_aggregate_input_bytes: 1 << 30,
};

fn editor_members() -> Vec<&'static str> {
    let mut members: Vec<&'static str> = EDITOR_SLK.iter().map(|id| id.path()).collect();
    for role in EDITOR_TXT {
        members.extend(role.files().iter().copied());
    }
    members
}

fn fold_table(out: &mut Fnv, table: &Table) {
    out.u64(table.fnv1a64());
    out.debug(&table.anomalies());
    out.debug(table.record_counts());
    out.debug(&table.declared());
    out.debug(table.generated_cells());
    out.u32(table.row_count());
    out.u32(table.column_count());
    table.headers().for_each(|(c, h)| {
        out.u32(c);
        out.text(h);
    });
}

/// Only public accessors, so the digest does not depend on private layout.
fn fold_editor(out: &mut Fnv, editor: &Editor) {
    out.text(editor.build());
    out.debug(editor.census());
    out.debug(&editor.absent());
    out.debug(&editor.fields());
    out.debug(editor.profile_keys());
    out.debug(&editor.profile_duplicates());
    out.u64(editor.profiles_len() as u64);
    for id in EDITOR_SLK {
        match editor.table(*id) {
            Some(table) => fold_table(out, table),
            None => out.u32(0),
        }
        let rawcodes = editor.rawcodes(*id);
        out.u64(editor.mixed_case_rawcodes(*id) as u64);
        for rawcode in &rawcodes {
            out.text(rawcode);
            out.debug(&editor.row(*id, rawcode));
            out.debug(&editor.profile(rawcode));
            out.debug(&editor.resolve(rawcode));
        }
    }
    for command in editor.commands() {
        out.debug(&editor.command(&command));
    }
    out.debug(&editor.units());
    out.debug(&editor.items());
    out.debug(&editor.upgrades());
    out.debug(&editor.abilities());
}

fn editor_reader(ctx: &Context) -> Result<Prepared, Failure> {
    let mut input_digest = Fnv::default();
    let mut set = input::open_set_with_codec(&ctx.install)?;
    let mut members: HashMap<String, Vec<u8>> = HashMap::new();
    for member in editor_members() {
        match set.extract(member) {
            Ok(bytes) => {
                input_digest.text(member);
                input_digest.bytes(&bytes);
                members.insert(member.to_string(), bytes);
            }
            Err(pcx_mpq::Error::NotFound(_)) => input_digest.text(member),
            Err(e) => return Err(format!("{member}: {e}")),
        }
    }
    let install = ctx.install.clone();
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut pending = std::mem::take(&mut members);
            let editor = Editor::from_reader(&install, &[], ADMISSION_LIMITS, |member| {
                Ok::<_, std::io::Error>(pending.remove(member))
            })
            .map_err(|e| e.to_string())?;
            let rows: u64 = editor
                .census()
                .rows_by_slk
                .values()
                .map(|r| *r as u64)
                .sum();
            Ok(Outcome {
                work: vec![
                    ("table_rows", rows),
                    ("profiles", editor.profiles_len() as u64),
                ],
                fold: Box::new(move |out| fold_editor(out, &editor)),
            })
        }),
    })
}

fn editor_install(ctx: &Context) -> Result<Prepared, Failure> {
    let mut set = input::open_set_with_codec(&ctx.install)?;
    let mut input_digest = Fnv::default();
    for archive in set.members() {
        input_digest.u64(archive.byte_len());
    }
    let install = ctx.install.clone();
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let editor = Editor::open_in(&mut set, &install).map_err(|e| e.to_string())?;
            let rows: u64 = editor
                .census()
                .rows_by_slk
                .values()
                .map(|r| *r as u64)
                .sum();
            Ok(Outcome {
                work: vec![
                    ("table_rows", rows),
                    ("profiles", editor.profiles_len() as u64),
                ],
                fold: Box::new(move |out| fold_editor(out, &editor)),
            })
        }),
    })
}
