//! objx — inspect object data straight out of a WC3 install.
//!
//! ```text
//! objx census <install_dir>
//! objx editor <install_dir>
//! objx unit   <install_dir> <rawcode>
//! objx diff   <install_dir> [map.w3m]
//! objx info   <install_dir> <map.w3m> <war3map.w3u>
//! objx dump   <install_dir> <map.w3m> <war3map.w3u>
//! objx hash   <install_dir> <map.w3m> <war3map.w3u>
//! ```

use std::process::ExitCode;

use pcx_objects::editor::{Editor, FieldSource, ObjectKind, Overlay};
use pcx_objects::{
    EXTENSIONS, Fourcc, ObjectFile, Value, ValueType, fourcc_text, map_object_files, shipped_maps,
};

enum Install {
    Given(String),
    Default,
}

fn install_dir() -> Install {
    if let Some(a) = std::env::args().nth(2) {
        return Install::Given(a);
    }
    if let Ok(v) = std::env::var("PEACECRAFT3_INSTALL") {
        return Install::Given(v);
    }
    if let Ok(v) = std::env::var("PEACECRAFT3_LEGACY_DIR") {
        return Install::Given(v);
    }
    Install::Default
}

impl Install {
    fn path(&self) -> &str {
        match self {
            Self::Given(s) => s,
            Self::Default => "wc3/game/app",
        }
    }
}

fn fourcc(f: Fourcc) -> String {
    if f.iter().all(|b| (0x20..0x7f).contains(b)) {
        fourcc_text(f)
    } else {
        f.iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join("")
    }
}

/// Print the measured inventory of the object-editor data of this install.
fn cmd_editor(dir: &Install) -> Result<(), String> {
    let install = std::path::Path::new(dir.path());
    let editor = Editor::open(install).map_err(|e| format!("{e}"))?;
    let c = editor.census();
    println!("{}: object-editor data", editor.build());
    println!(
        "  union listfile  {} .slk, {} .txt, {} .fdf",
        c.slk_files, c.txt_files, c.fdf_files
    );
    println!(
        "  Units\\          {} .slk, {} .txt",
        c.units_slk, c.units_txt
    );
    println!("  Doodads\\        {} .slk", c.doodads_slk);
    println!(
        "  working copies  {} .slk, {} .txt (not decoded)",
        c.working_copy_slk, c.working_copy_txt
    );
    println!("  decoded tables  {}", c.rows_by_slk.len());
    let mut rows: Vec<_> = c.rows_by_slk.iter().collect();
    rows.sort_by_key(|(id, _)| **id);
    for (id, n) in &rows {
        println!("    {:24} {:>5} rows   {}", id.name(), n, id.path());
    }
    println!("  absent on this build:");
    for id in editor.absent() {
        println!("    {}  {}", id.name(), id.path());
    }
    println!(
        "  units {}, items {}, upgrades {}, abilities {}",
        editor.units().len(),
        editor.items().len(),
        editor.upgrades().len(),
        editor.abilities().len(),
    );
    println!(
        "  commands {}, profile sections {}, profile keys {}",
        editor.commands().len(),
        editor.profiles_len(),
        editor.profile_keys().len(),
    );
    println!(
        "  field fourccs {}, unmeasured {}",
        editor.fields().len(),
        editor
            .fields()
            .iter()
            .filter(|f| matches!(f.source, FieldSource::Profile { .. }) && !f.measured)
            .count()
    );
    Ok(())
}

fn cmd_unit(dir: &Install, rawcode: &str) -> Result<(), String> {
    let install = std::path::Path::new(dir.path());
    let editor = Editor::open(install).map_err(|e| format!("{e}"))?;
    let kind = editor
        .resolve(rawcode)
        .ok_or_else(|| format!("no object named {rawcode}"))?;
    println!("{rawcode} ({kind}) in {}", editor.build());
    match kind {
        ObjectKind::Unit => {
            let u = editor.unit(rawcode).expect("unit resolved");
            print_unit(&editor, &u);
        }
        ObjectKind::Item => {
            let i = editor.item(rawcode).expect("item resolved");
            print_item(&i);
        }
        ObjectKind::Upgrade => {
            let u = editor.upgrade(rawcode).expect("upgrade resolved");
            println!("  comment   {}", u.comment().unwrap_or("-"));
            println!("  maxlevel  {:?}", u.max_level());
        }
        ObjectKind::Ability => {
            let Some(row) = editor.row(pcx_objects::editor::SlkId::AbilityData, rawcode) else {
                return Err("no row".into());
            };
            let t = editor
                .table(pcx_objects::editor::SlkId::AbilityData)
                .expect("table");
            let r = t.row(row);
            println!("  comment   {}", r.by_name("comment").unwrap_or("-"));
            for name in [
                "levels",
                "reqLevel",
                "levelSkip",
                "targs1",
                "Cast1",
                "Dur1",
                "Cool1",
                "Cost1",
                "Area1",
                "Rng1",
            ] {
                println!("  {name:10} {}", r.by_name(name).unwrap_or("-"));
            }
        }
        ObjectKind::Command => {
            let Some(s) = editor.command(rawcode) else {
                return Err("no command section".into());
            };
            for k in ["art", "buttonpos", "placementmodel"] {
                if let Some(v) = s.value(k) {
                    println!("  {k:14} {v}");
                }
            }
        }
    }
    Ok(())
}

fn print_unit(editor: &Editor, u: &pcx_objects::editor::Unit<'_>) {
    let named: &[(&str, Fourcc)] = &[
        ("comment", *b"cmnt"),
        ("uhpm", *b"uhpm"),
        ("umpm", *b"umpm"),
        ("umvs", *b"umvs"),
        ("udef", *b"udef"),
        ("ua1d", *b"ua1d"),
        ("ua1s", *b"ua1s"),
        ("ua1b", *b"ua1b"),
        ("ua1r", *b"ua1r"),
        ("ua1c", *b"ua1c"),
        ("ugol", *b"ugol"),
        ("ulum", *b"ulum"),
        ("ubld", *b"ubld"),
        ("usid", *b"usid"),
        ("ulev", *b"ulev"),
        ("urac", *b"urac"),
        ("ubdi", *b"ubdi"),
        ("umpr", *b"umpr"),
    ];
    if let Some(c) = u.comment() {
        println!("  comment   {c}");
    }
    if let Some(n) = u.name() {
        println!("  name      {n}");
    }
    for (label, f) in named {
        if *label == "comment" {
            continue;
        }
        if let Some(def) = u.field(*f) {
            let v = u.stat(*f).map(|v| v.text().unwrap_or("-").to_string());
            println!(
                "  {label:10} {:<22}  <- {}.{} ({} {})",
                v.as_deref().unwrap_or("-"),
                def.slk,
                def.column_name(),
                def.kind,
                def.field
            );
        }
    }
    let _ = editor;
}

fn print_item(i: &pcx_objects::editor::Item<'_>) {
    if let Some(c) = i.comment() {
        println!("  comment   {c}");
    }
    if let Some(n) = i.name() {
        println!("  name      {n}");
    }
    for (label, f) in [
        ("igol", *b"igol"),
        ("ilum", *b"ilum"),
        ("ihtp", *b"ihtp"),
        ("icla", *b"icla"),
        ("ilev", *b"ilev"),
        ("iabi", *b"iabi"),
    ] {
        if let Some(def) = i.field(f) {
            println!(
                "  {label:10} {:<22}  <- {}.{} ({})",
                i.base_cell(f).unwrap_or("-"),
                def.slk,
                def.column_name(),
                def.kind
            );
        }
    }
}

fn cmd_diff(dir: &Install, map: &str, only: Option<&str>) -> Result<(), String> {
    let install = std::path::Path::new(dir.path());
    let mut set = pcx_mpq::MpqSet::open_wc3(install).map_err(|e| format!("{install:?}: {e}"))?;
    let editor = Editor::open_in(&mut set, install).map_err(|e| format!("{e}"))?;
    let mut total = 0usize;
    let mut unresolved = Vec::new();
    let names = match map_object_files(&mut set, map) {
        Ok(n) => n,
        Err(e) => return Err(format!("{map}: {e}")),
    };
    for name in names {
        let low = name.to_ascii_lowercase();
        if !(low.ends_with(".w3u") || low.ends_with(".w3t")) {
            continue;
        }
        if let Some(o) = only {
            if !name.eq_ignore_ascii_case(o) {
                continue;
            }
        }
        let bytes = pcx_objects::extract_object_file(&mut set, map, &name)
            .map_err(|e| format!("{map} {name}: {e}"))?;
        let file = ObjectFile::parse(&name, &bytes).map_err(|e| e.to_string())?;
        let overlay = Overlay::of(&file);
        println!(
            "{map} {name}: {} base records, {} custom",
            file.base().len(),
            file.custom().len()
        );
        for (rawcode, _) in overlay.mods.iter() {
            let Some(base) = editor.unit(rawcode) else {
                unresolved.push(rawcode.clone());
                println!("  {rawcode}: UNRESOLVED (no row in any editor table)");
                continue;
            };
            let overlay_u = overlay.unit(&editor, rawcode);
            println!(
                "  {rawcode}{}:",
                if overlay_u.custom() { " (custom)" } else { "" }
            );
            for m in overlay.on(rawcode) {
                let before = match base.stat(m.field) {
                    Some(v) => v.text().unwrap_or("<not text>").to_string(),
                    None => "<no base cell>".to_string(),
                };
                let def_txt = match editor.field(m.field) {
                    Some(d) => format!("[{}.{} {}]", d.slk, d.column_name(), d.kind),
                    None => "[no field def]".to_string(),
                };
                println!(
                    "    {} {before} -> {}  {def_txt}",
                    fourcc(m.field),
                    show_value(&m.value)
                );
            }
            total += 1;
        }
        for rawcode in overlay.custom.keys() {
            println!(
                "  {rawcode}: new object derived from {:?}",
                overlay.new_object(rawcode).map(|(b, _)| b)
            );
            total += 1;
        }
    }
    println!("{total} objects diffed, {} unresolved", unresolved.len());
    Ok(())
}

/// One modification value, in the shape its type word says.
fn show_value(v: &Value) -> String {
    match v {
        Value::Int(w) => format!("int {w}"),
        Value::Real(f) => format!("real {f}"),
        Value::Unreal(f) => format!("unreal {f}"),
        Value::Str(s) => format!("str {:?}", String::from_utf8_lossy(s)),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = install_dir();
    let result = match args.first().map(String::as_str).unwrap_or("census") {
        "census" => cmd_census(&dir),
        "editor" => cmd_editor(&dir),
        "unit" => match args.get(3).or_else(|| args.get(2)) {
            Some(rc) => cmd_unit(&dir, rc),
            None => Err("usage: objx unit <install_dir> <rawcode>".into()),
        },
        "diff" => {
            let map = args.get(2);
            let only = args.get(3).map(String::as_str);
            match map {
                Some(m) => cmd_diff(&dir, m, only),
                None => Err("usage: objx diff <install_dir> [map.w3m] [war3map.w3u]".into()),
            }
        }
        "info" | "dump" | "hash" => {
            let (Some(map), Some(name)) = (args.get(2), args.get(3)) else {
                eprintln!(
                    "usage: objx {cmd} <install_dir> <map.w3m> <war3map.w3u>",
                    cmd = args[0]
                );
                return ExitCode::from(2);
            };
            cmd_file(&dir, map, name)
        }
        other => {
            eprintln!("unknown command {other:?}; try census/editor/unit/diff/info/dump/hash");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_census(dir: &Install) -> Result<(), String> {
    let install = std::path::Path::new(dir.path());
    let mut set = pcx_mpq::MpqSet::open_wc3(install).map_err(|e| format!("{install:?}: {e}"))?;
    let maps = shipped_maps(&mut set).map_err(|e| format!("listfile: {e}"))?;
    let mut per_ext = [0usize; 7];
    let mut parsed = 0usize;
    let mut records = 0usize;
    let mut mods = 0usize;
    for map in &maps {
        let names = match map_object_files(&mut set, map) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("  BAD  {map}: {e}");
                continue;
            }
        };
        for name in names {
            let low = name.to_ascii_lowercase();
            let Some(i) = EXTENSIONS
                .iter()
                .position(|e| low.ends_with(&format!(".{e}")))
            else {
                continue;
            };
            per_ext[i] += 1;
            let bytes = match pcx_objects::extract_object_file(&mut set, map, &name) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("  MISS {map} {name}: {e}");
                    continue;
                }
            };
            match ObjectFile::parse(&name, &bytes) {
                Ok(f) => {
                    let fr = f.framing();
                    for v in fr.violations() {
                        eprintln!("  BAD  {map} {name}: {v}");
                    }
                    if f.canonical_bytes() != bytes {
                        eprintln!("  BAD  {map} {name}: round trip");
                    }
                    parsed += 1;
                    records += fr.objects;
                    mods += fr.mods;
                }
                Err(e) => eprintln!("  BAD  {map} {name}: {e}"),
            }
        }
    }
    println!(
        "{}: {} object files (w3u {} w3t {} w3a {} w3b {} w3d {} w3h {} w3q {})",
        dir.path(),
        per_ext.iter().sum::<usize>(),
        per_ext[0],
        per_ext[1],
        per_ext[2],
        per_ext[3],
        per_ext[4],
        per_ext[5],
        per_ext[6],
    );
    println!("  {parsed} parsed, {records} records, {mods} modifications");
    Ok(())
}

fn cmd_file(dir: &Install, map: &str, name: &str) -> Result<(), String> {
    let install = std::path::Path::new(dir.path());
    let mut set = pcx_mpq::MpqSet::open_wc3(install).map_err(|e| format!("{install:?}: {e}"))?;
    let bytes = pcx_objects::extract_object_file(&mut set, map, name)
        .map_err(|e| format!("{map} {name}: {e}"))?;
    let f = ObjectFile::parse(name, &bytes).map_err(|e| format!("{map} {name}: {e}"))?;
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "info" => {
            let fr = f.framing();
            let mut types = [0usize; 4];
            let mut end_zero = 0usize;
            let mut end_other = 0usize;
            for m in f.records().flat_map(|r| r.mods.iter()) {
                types[match m.value_type {
                    ValueType::Int => 0,
                    ValueType::Real => 1,
                    ValueType::Unreal => 2,
                    ValueType::Str => 3,
                }] += 1;
                if m.end_word == 0 {
                    end_zero += 1;
                } else {
                    end_other += 1;
                }
            }
            println!("{map} {name}");
            println!("  file        {} bytes", bytes.len());
            println!("  version     {}", f.version());
            println!(
                "  objects     {} base, {} custom",
                f.base().len(),
                f.custom().len()
            );
            println!(
                "  values      {} int, {} real, {} unreal, {} string",
                types[0], types[1], types[2], types[3]
            );
            println!("  end words   {end_zero} zero, {end_other} non-zero");
            println!(
                "  framing     {} (base {} + custom {} + 12 bytes of counts)",
                if fr.sums() { "sums" } else { "FAILS" },
                fr.base_bytes,
                fr.custom_bytes
            );
            for v in fr.violations() {
                println!("  violation   {v}");
            }
        }
        "dump" => {
            for (label, table) in [("base", f.base()), ("custom", f.custom())] {
                for (i, r) in table.iter().enumerate() {
                    println!(
                        "{label}[{i}] {} -> {} ({} mods)",
                        fourcc(r.original),
                        fourcc(r.modified),
                        r.mods.len()
                    );
                    for m in &r.mods {
                        let val = match &m.value {
                            Value::Int(w) => format!("int {w} ({w:#x})"),
                            Value::Real(v) => format!("real {v}"),
                            Value::Unreal(v) => format!("unreal {v}"),
                            Value::Str(s) => format!("str {:?}", String::from_utf8_lossy(s)),
                        };
                        println!(
                            "    {} type={} {val} end={}",
                            fourcc(m.field),
                            m.value_type.word(),
                            fourcc(m.end_word.to_le_bytes().try_into().unwrap())
                        );
                    }
                }
            }
        }
        "hash" => {
            let h_file = pcx_mpq::fnv1a64(&bytes);
            let h_canon = f.canonical_hash();
            println!("file     fnv1a64 {h_file:016x}");
            println!("canonical fnv1a64 {h_canon:016x}");
        }
        _ => {}
    }
    Ok(())
}
