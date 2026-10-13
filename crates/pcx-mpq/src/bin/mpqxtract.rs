//! `mpqxtract` — list and extract files from MPQ archives.
//!
//! ```text
//! mpqxtract list   <archive…>            one path per line, from (listfile)
//! mpqxtract show   <archive…> <path>      size + FNV-1a of the decoded bytes
//! mpqxtract get    <archive…> <path> <out>
//! mpqxtract dump   <archive…>            header + table statistics
//! ```
//!
//! Archives are searched in the order given, so pass them highest-priority first.

use std::io::Write;
use std::process::ExitCode;

use pcx_mpq::{DEFAULT_PRIORITY, MpqArchive};

fn fnv1a(d: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in d {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn open(paths: &[String]) -> Result<Vec<MpqArchive>, String> {
    let mut v = Vec::new();
    for p in paths {
        match MpqArchive::open(p) {
            Ok(a) => v.push(a),
            Err(e) => return Err(format!("{p}: {e}")),
        }
    }
    if v.is_empty() {
        return Err("no archives given".into());
    }
    Ok(v)
}

/// The archive's own `(listfile)`, if it has one.
fn listfile(arc: &mut MpqArchive) -> Option<Vec<String>> {
    let raw = arc.extract("(listfile)").ok()?;
    Some(
        String::from_utf8_lossy(&raw)
            .split(|c| c == '\r' || c == '\n')
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: mpqxtract <list|show|get|dump> <archive…> [args]");
        return ExitCode::from(2);
    }
    let cmd = args[0].clone();
    // `show`/`get` take trailing operands; everything before them is an archive.
    let extra = match cmd.as_str() {
        "show" => 1,
        "get" => 2,
        _ => 0,
    };
    let (archives, operands) = if extra == 0 {
        (&args[1..], &args[0..0])
    } else {
        let end = args.len().saturating_sub(extra);
        if args.len() <= extra {
            eprintln!("mpqxtract {cmd}: missing operands");
            return ExitCode::from(2);
        }
        (&args[1..end], &args[end..])
    };
    let operand = |i: usize| operands.get(i).cloned().unwrap_or_default();

    if cmd == "verify" {
        let Some(list) = args.get(1) else {
            eprintln!("mpqxtract verify <pathfile> <archive…>");
            return ExitCode::from(2);
        };
        let Ok(text) = std::fs::read_to_string(list) else {
            eprintln!("mpqxtract: cannot read {list}");
            return ExitCode::from(1);
        };
        let mut arcs = match open(&args[2..]) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("mpqxtract: {e}");
                return ExitCode::from(1);
            }
        };
        let out = std::io::stdout();
        let mut w = std::io::BufWriter::new(out.lock());
        for p in text.lines().filter(|l| !l.is_empty()) {
            let key = p.to_lowercase();
            let mut done = false;
            for a in arcs.iter_mut() {
                if !a.contains(p) {
                    continue;
                }
                eprintln!("TRY\t{key}");
                if let Ok(b) = a.extract(p) {
                    let _ = writeln!(w, "{}\t{}\t{:016x}", key, b.len(), fnv1a(&b));
                    let _ = w.flush();
                    done = true;
                    break;
                }
            }
            if !done {
                let _ = writeln!(w, "{}\tERR\t-", key);
            }
        }
        return ExitCode::SUCCESS;
    }

    let mut arcs = match open(archives) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("mpqxtract: {e}");
            return ExitCode::from(1);
        }
    };

    match cmd.as_str() {
        "list" => {
            let mut seen = std::collections::BTreeSet::new();
            for a in arcs.iter_mut() {
                if let Some(names) = listfile(a) {
                    seen.extend(names);
                }
            }
            let out = std::io::stdout();
            let mut w = std::io::BufWriter::new(out.lock());
            for p in &seen {
                let _ = writeln!(w, "{p}");
            }
            eprintln!("{} entries", seen.len());
        }
        "show" => {
            let path = operand(0);
            if path.is_empty() {
                eprintln!("mpqxtract show <archive…> <path>");
                return ExitCode::from(2);
            }
            for a in arcs.iter_mut() {
                if !a.contains(&path) {
                    continue;
                }
                match a.extract(&path) {
                    Ok(b) => {
                        println!("{}\t{}\t{:016x}", a.path().display(), b.len(), fnv1a(&b));
                        return ExitCode::SUCCESS;
                    }
                    Err(e) => {
                        eprintln!("{path}: {e}");
                        return ExitCode::from(1);
                    }
                }
            }
            eprintln!("not found: {path}");
            return ExitCode::from(1);
        }
        "get" => {
            let (path, out) = (operand(0), operand(1));
            if path.is_empty() || out.is_empty() {
                eprintln!("mpqxtract get <archive…> <path> <out>");
                return ExitCode::from(2);
            }
            for a in arcs.iter_mut() {
                if !a.contains(&path) {
                    continue;
                }
                match a
                    .extract(&path)
                    .and_then(|b| std::fs::write(&out, b).map_err(pcx_mpq::Error::Io))
                {
                    Ok(()) => return ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("{path}: {e}");
                        return ExitCode::from(1);
                    }
                }
            }
            eprintln!("not found: {path}");
            return ExitCode::from(1);
        }
        "dump" => {
            for a in arcs.iter() {
                let h = a.header;
                println!(
                    "{}\n  archive_size={} hash_pos={:#x} block_pos={:#x}\n  hash_entries={} block_entries={} sector_size={}\n  words@0x20={:#x} @0x24={:#x}",
                    a.path().display(),
                    h.archive_size,
                    h.hash_table_pos,
                    h.block_table_pos,
                    h.hash_entries,
                    h.block_entries,
                    h.sector_size(),
                    h.w20,
                    h.w24,
                );
            }
        }
        other => {
            eprintln!("unknown command: {other}");
            return ExitCode::from(2);
        }
    }
    let _ = DEFAULT_PRIORITY;
    ExitCode::SUCCESS
}
