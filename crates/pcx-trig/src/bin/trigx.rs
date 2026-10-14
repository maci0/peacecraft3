//! `trigx` — inspect the scripting-side map files straight out of a WC3
//! install.
//!
//! ```text
//! trigx census <install_dir>
//! trigx info   <install_dir> <map.w3m> <war3map.wts|war3map.wtg|war3map.wct>
//! trigx dump   <install_dir> <map.w3m> <war3map.wts|war3map.wtg|war3map.wct>
//! trigx hash   <install_dir> <map.w3m> <war3map.wts|war3map.wtg|war3map.wct>
//! ```
//!
//! `hash` prints two FNV-1a 64 digests: the file's own bytes, and the
//! decoded tree re-serialised field by field. The second is what another
//! decoder must reproduce byte for byte before its decode can be called
//! equivalent.
//!
//! A WTG read needs the function arities of `UI\TriggerData.txt`, which
//! `trigx` pulls from the same install.

use std::process::ExitCode;

use pcx_trig::{
    EXTENSIONS, EcaKind, ParamKind, SubKind, Wct, Wtg, Wts, extract_trig_file, map_trig_files,
    shipped_maps, wct_wtg_violations,
};

fn install_dir() -> String {
    std::env::args()
        .nth(2)
        .or_else(|| std::env::var("PEACECRAFT3_INSTALL").ok())
        .or_else(|| std::env::var("PEACECRAFT3_LEGACY_DIR").ok())
        .unwrap_or_else(|| "wc3/game/app".to_string())
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn kind_word(k: EcaKind) -> &'static str {
    match k {
        EcaKind::Event => "event",
        EcaKind::Condition => "condition",
        EcaKind::Action => "action",
    }
}

fn param_kind_word(k: ParamKind) -> &'static str {
    match k {
        ParamKind::Preset => "preset",
        ParamKind::Variable => "variable",
        ParamKind::Function => "function",
        ParamKind::Str => "string",
    }
}

fn sub_kind_word(k: SubKind) -> &'static str {
    match k {
        SubKind::WrappedCondition => "wrapped-condition",
        SubKind::WrappedAction => "wrapped-action",
        SubKind::NestedCall => "nested-call",
    }
}

fn dump_param(p: &pcx_trig::Param, ind: usize) {
    let pad = "  ".repeat(ind);
    let mut line = format!(
        "{pad}{} {:?}{}",
        param_kind_word(p.kind),
        text(&p.value),
        if p.array_index.is_some() {
            " [indexed]"
        } else {
            ""
        }
    );
    if let Some(s) = &p.sub {
        line.push_str(&format!(
            " -> {} {}({})",
            sub_kind_word(s.kind),
            text(&s.name),
            s.params.len()
        ));
    }
    println!("{line}");
    if let Some(s) = &p.sub {
        for q in &s.params {
            dump_param(q, ind + 1);
        }
    }
    if let Some(i) = &p.array_index {
        println!("{pad}  index:");
        dump_param(i, ind + 1);
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("census");
    let dir = install_dir();

    let mut set = match pcx_mpq::MpqSet::open_wc3(&dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{dir}: {e}");
            return ExitCode::from(1);
        }
    };

    match cmd {
        "census" => {
            let maps = match shipped_maps(&mut set) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("listfile: {e}");
                    return ExitCode::from(1);
                }
            };
            let arity = match pcx_trig::install_trigger_arity(std::path::Path::new(&dir)) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("TriggerData: {e}");
                    return ExitCode::from(1);
                }
            };
            let mut per_type = [0usize; 3];
            let mut parsed = 0usize;
            let mut entries = 0usize;
            let mut variables = 0usize;
            let mut triggers = 0usize;
            let mut ecas = 0usize;
            let mut params = 0usize;
            let mut blocks = 0usize;
            let mut lacking = 0usize;
            for map in &maps {
                let names = match map_trig_files(&mut set, map) {
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("  BAD  {map}: {e}");
                        continue;
                    }
                };
                let mut have = [false; 3];
                for name in &names {
                    let low = name.to_ascii_lowercase();
                    let Some(i) = EXTENSIONS
                        .iter()
                        .position(|e| low.ends_with(&format!(".{e}")))
                    else {
                        continue;
                    };
                    have[i] = true;
                    per_type[i] += 1;
                    let bytes = match extract_trig_file(&mut set, map, name) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("  MISS {map} {name}: {e}");
                            continue;
                        }
                    };
                    match i {
                        0 => match Wts::parse(&bytes) {
                            Ok(f) => {
                                for v in f.framing().violations() {
                                    eprintln!("  BAD  {map} {name}: {v}");
                                }
                                if f.canonical_bytes() != bytes {
                                    eprintln!("  BAD  {map} {name}: round trip");
                                }
                                entries += f.entries().len();
                                parsed += 1;
                            }
                            Err(e) => eprintln!("  BAD  {map} {name}: {e}"),
                        },
                        1 => match Wtg::parse(&bytes, &arity) {
                            Ok(f) => {
                                for v in f.framing().violations() {
                                    eprintln!("  BAD  {map} {name}: {v}");
                                }
                                if f.canonical_bytes() != bytes {
                                    eprintln!("  BAD  {map} {name}: round trip");
                                }
                                let fr = f.framing();
                                variables += fr.variables;
                                triggers += fr.triggers;
                                ecas += fr.ecas;
                                params += fr.params;
                                parsed += 1;
                            }
                            Err(e) => eprintln!("  BAD  {map} {name}: {e}"),
                        },
                        2 => match Wct::parse(&bytes) {
                            Ok(f) => {
                                for v in f.framing().violations() {
                                    eprintln!("  BAD  {map} {name}: {v}");
                                }
                                if f.canonical_bytes() != bytes {
                                    eprintln!("  BAD  {map} {name}: round trip");
                                }
                                blocks += f.blocks().len();
                                parsed += 1;
                            }
                            Err(e) => eprintln!("  BAD  {map} {name}: {e}"),
                        },
                        _ => unreachable!(),
                    }
                }
                if !have.iter().all(|h| *h) {
                    lacking += 1;
                    let mut s = String::new();
                    for (i, h) in have.iter().enumerate() {
                        s.push_str(if *h { ["wts", "wtg", "wct"][i] } else { "---" });
                        if i < 2 {
                            s.push('.');
                        }
                    }
                    println!("  lacks {s}  {map}");
                }
            }
            println!(
                "{dir}: {} maps ({} lacking at least one file), \
                 files wts {} wtg {} wct {}",
                maps.len(),
                lacking,
                per_type[0],
                per_type[1],
                per_type[2],
            );
            println!(
                "  {parsed} parsed, {entries} string entries, {variables} variables, \
                 {triggers} triggers, {ecas} ECAs, {params} parameters, {blocks} wct blocks"
            );
        }
        "info" | "dump" | "hash" => {
            let map = match args.get(2) {
                Some(m) => m,
                None => {
                    eprintln!("usage: trigx {cmd} <install_dir> <map> <file>");
                    return ExitCode::from(2);
                }
            };
            let name = match args.get(3) {
                Some(n) => n,
                None => {
                    eprintln!("usage: trigx {cmd} <install_dir> <map> <file>");
                    return ExitCode::from(2);
                }
            };
            let bytes = match extract_trig_file(&mut set, map, name) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("{map} {name}: {e}");
                    return ExitCode::from(1);
                }
            };
            let low = name.to_ascii_lowercase();
            let ext = low.rsplit('.').next().unwrap_or("");
            match ext {
                "wts" => {
                    let f = match Wts::parse(&bytes) {
                        Ok(f) => f,
                        Err(e) => {
                            eprintln!("{name}: {e}");
                            return ExitCode::from(1);
                        }
                    };
                    match cmd {
                        "info" => {
                            let fr = f.framing();
                            println!("{name}: {} bytes, bom {}", fr.file_len, f.bom());
                            println!(
                                "  {} entries, {} comment lines, entry bytes {}",
                                fr.entries, fr.comments, fr.entry_bytes
                            );
                            println!(
                                "  framing: {} (ids strictly increasing: {})",
                                if fr.sums() { "holds" } else { "FAILS" },
                                fr.ids_strictly_increasing
                            );
                            for v in fr.violations() {
                                println!("  violation: {v}");
                            }
                        }
                        "dump" => {
                            for e in f.entries() {
                                for c in &e.comments {
                                    println!("// {}", text(c));
                                }
                                println!(
                                    "STRING {} ({} bytes){}",
                                    e.id,
                                    e.body.len(),
                                    if e.body.is_empty() { " [empty]" } else { "" }
                                );
                                if !e.body.is_empty() {
                                    for line in text(&e.body).lines() {
                                        println!("  {line}");
                                    }
                                }
                            }
                        }
                        _ => {
                            println!("raw      {:016x}", pcx_mpq::fnv1a64(&bytes));
                            println!("canonical {:016x}", f.canonical_hash());
                        }
                    }
                }
                "wct" => {
                    let f = match Wct::parse(&bytes) {
                        Ok(f) => f,
                        Err(e) => {
                            eprintln!("{name}: {e}");
                            return ExitCode::from(1);
                        }
                    };
                    match cmd {
                        "info" => {
                            let fr = f.framing();
                            println!(
                                "{name}: {} bytes, unknown0 {}, {} text blocks",
                                fr.file_len,
                                f.unknown0(),
                                fr.blocks
                            );
                            println!(
                                "  {} non-empty, {} text bytes, framing: {}",
                                fr.nonempty,
                                fr.text_bytes,
                                if fr.sums() { "holds" } else { "FAILS" }
                            );
                            for v in fr.violations() {
                                println!("  violation: {v}");
                            }
                        }
                        "dump" => {
                            for (i, b) in f.blocks().iter().enumerate() {
                                if b.text.is_empty() {
                                    println!("block {i}: [empty]");
                                    continue;
                                }
                                let s = text(&b.text);
                                let s = s.trim_end_matches('\0');
                                println!("block {i}: {} bytes", b.text.len());
                                for line in s.lines() {
                                    println!("  {line}");
                                }
                            }
                        }
                        _ => {
                            println!("raw      {:016x}", pcx_mpq::fnv1a64(&bytes));
                            println!("canonical {:016x}", f.canonical_hash());
                        }
                    }
                }
                "wtg" => {
                    let arity = match pcx_trig::install_trigger_arity(std::path::Path::new(&dir)) {
                        Ok(a) => a,
                        Err(e) => {
                            eprintln!("TriggerData: {e}");
                            return ExitCode::from(1);
                        }
                    };
                    let f = match Wtg::parse(&bytes, &arity) {
                        Ok(f) => f,
                        Err(e) => {
                            eprintln!("{name}: {e}");
                            return ExitCode::from(1);
                        }
                    };
                    match cmd {
                        "info" => {
                            let fr = f.framing();
                            println!(
                                "{name}: {} bytes, version {}, root word {}",
                                fr.file_len,
                                f.version(),
                                f.unknown_root()
                            );
                            println!(
                                "  {} categories, {} variables, {} triggers, {} ECAs, \
                                 {} parameters, {} sub records",
                                fr.categories,
                                fr.variables,
                                fr.triggers,
                                fr.ecas,
                                fr.params,
                                fr.subs
                            );
                            println!(
                                "  framing: {} (category ids resolve: {})",
                                if fr.sums() { "holds" } else { "FAILS" },
                                fr.trigger_category_ids_resolve
                            );
                            for v in fr.violations() {
                                println!("  violation: {v}");
                            }
                        }
                        "dump" => {
                            for c in f.categories() {
                                println!("category {}: {}", c.id, text(&c.name));
                            }
                            for v in f.variables() {
                                println!(
                                    "variable {} {} array={} init={} = {:?}",
                                    text(&v.type_name),
                                    text(&v.name),
                                    v.is_array,
                                    v.is_initialized,
                                    text(&v.initial_value)
                                );
                            }
                            for t in f.triggers() {
                                println!(
                                    "trigger {:?} cat={} enabled={} custom={} off={} \
                                     init={} ecas={}",
                                    text(&t.name),
                                    t.category_id,
                                    t.is_enabled,
                                    t.is_custom,
                                    t.is_initially_off,
                                    t.run_on_init,
                                    t.ecas.len()
                                );
                                if !t.description.is_empty() {
                                    println!("  desc {:?}", text(&t.description));
                                }
                                for e in &t.ecas {
                                    println!(
                                        "  {} {:?} enabled={} params={}",
                                        kind_word(e.kind),
                                        text(&e.name),
                                        e.is_enabled,
                                        e.params.len()
                                    );
                                    for p in &e.params {
                                        dump_param(p, 2);
                                    }
                                }
                            }
                        }
                        _ => {
                            println!("raw      {:016x}", pcx_mpq::fnv1a64(&bytes));
                            println!("canonical {:016x}", f.canonical_hash());
                        }
                    }
                    if ext == "wtg"
                        && let Ok(wct_b) = extract_trig_file(&mut set, map, "war3map.wct")
                        && let Ok(wct) = Wct::parse(&wct_b)
                    {
                        let v = wct_wtg_violations(&wct, &f);
                        if cmd == "info" {
                            if v.is_empty() {
                                println!("  wct/wtg relation: holds");
                            } else {
                                for x in v {
                                    println!("  wct/wtg violation: {x}");
                                }
                            }
                        }
                    }
                }
                other => {
                    eprintln!("{name}: extension {other:?} is not wts/wtg/wct");
                    return ExitCode::from(2);
                }
            }
        }
        other => {
            eprintln!("unknown command {other:?}; census | info | dump | hash");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
