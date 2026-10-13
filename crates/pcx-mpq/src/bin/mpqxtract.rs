//! `mpqxtract` — list and extract files from MPQ archives.
//!
//! ```text
//! mpqxtract list   <archive…> [glob]     one path per line, from (listfile)
//! mpqxtract show   <archive…> <path|glob> size + FNV-1a of decoded matches
//! mpqxtract get    <archive…> <path|glob> <out>
//! mpqxtract dump   <archive…>            header + table statistics
//! ```
//!
//! Archives are searched in the order given, so pass them highest-priority first.
//! Quote globs so the shell does not expand them. `*` and `?` stay within a
//! segment; a whole `**` segment spans zero or more segments. Matching folds
//! ASCII case and treats `/` and `\` alike. `get` with a glob writes every
//! matched archive path beneath an output directory; a literal path writes one
//! output file. Globs require each archive's `(listfile)`.

use std::io::Write;
use std::process::ExitCode;

use pcx_mpq::MpqArchive;

mod glob;

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

fn has_glob(path: &str) -> bool {
    path.contains(['*', '?'])
}

/// A glob's matches use the first archive's spelling for each lookup key.
fn select(arcs: &mut [MpqArchive], pattern: &str) -> Result<Vec<String>, String> {
    let mut seen = std::collections::BTreeMap::new();
    for arc in arcs {
        let source = arc.path().display().to_string();
        let raw = arc
            .extract("(listfile)")
            .map_err(|e| format!("{source}: cannot enumerate (listfile): {e}"))?;
        let names = std::str::from_utf8(&raw)
            .map_err(|e| format!("{source}: malformed UTF-8 (listfile): {e}"))?;
        for path in names.split(['\r', '\n']).filter(|path| !path.is_empty()) {
            if glob::matches(pattern, path) {
                let key = path.replace('\\', "/").to_ascii_lowercase();
                seen.entry(key).or_insert_with(|| path.to_owned());
            }
        }
    }
    Ok(seen.into_values().collect())
}

/// Archive names must remain relative beneath the selected output directory.
fn output_path(root: &std::path::Path, name: &str) -> Result<std::path::PathBuf, String> {
    let mut path = root.to_path_buf();
    for part in name.split(['/', '\\']) {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            return Err(format!("unsafe archive output path: {name}"));
        }
        path.push(part);
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!(
                "archive output path contains a symlink: {}",
                path.display()
            ));
        }
    }
    Ok(path)
}

fn trailing_operands(cmd: &str, args: &[String]) -> usize {
    match cmd {
        "show" => 1,
        "get" => 2,
        "list" if args.len() > 2 && args.last().is_some_and(|p| has_glob(p)) => 1,
        _ => 0,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: mpqxtract <list|show|get|dump> <archive…> [args]");
        return ExitCode::from(2);
    }
    let cmd = args[0].clone();
    // A trailing wildcard on `list` is its optional pattern operand.
    let extra = trailing_operands(&cmd, &args);
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
            let pattern = operands.first().map_or("**", String::as_str);
            let paths = match select(&mut arcs, pattern) {
                Ok(paths) => paths,
                Err(e) => {
                    eprintln!("mpqxtract: {e}");
                    return ExitCode::from(1);
                }
            };
            let out = std::io::stdout();
            let mut w = std::io::BufWriter::new(out.lock());
            for path in &paths {
                if let Err(e) = writeln!(w, "{path}") {
                    eprintln!("mpqxtract: {e}");
                    return ExitCode::from(1);
                }
            }
            if let Err(e) = w.flush() {
                eprintln!("mpqxtract: {e}");
                return ExitCode::from(1);
            }
            eprintln!("{} entries", paths.len());
        }
        "show" | "get" => {
            let pattern = operand(0);
            let out = operand(1);
            if pattern.is_empty() || (cmd == "get" && out.is_empty()) {
                eprintln!("mpqxtract {cmd}: missing path/glob or output operand");
                return ExitCode::from(2);
            }
            let wildcard = has_glob(&pattern);
            let paths = if wildcard {
                match select(&mut arcs, &pattern) {
                    Ok(paths) => paths,
                    Err(e) => {
                        eprintln!("mpqxtract: {e}");
                        return ExitCode::from(1);
                    }
                }
            } else {
                vec![pattern.clone()]
            };
            if paths.is_empty() {
                eprintln!("not found: {pattern}");
                return ExitCode::from(1);
            }
            // Validate all selected output paths before extracting any bytes.
            let destinations = if cmd == "get" && wildcard {
                match paths
                    .iter()
                    .map(|path| output_path(std::path::Path::new(&out), path))
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(paths) => paths,
                    Err(e) => {
                        eprintln!("mpqxtract: {e}");
                        return ExitCode::from(1);
                    }
                }
            } else {
                vec![std::path::PathBuf::from(&out); paths.len()]
            };
            for (path, destination) in paths.iter().zip(&destinations) {
                let Some(arc) = arcs.iter_mut().find(|a| a.contains(path)) else {
                    eprintln!("not found: {path}");
                    return ExitCode::from(1);
                };
                let bytes = match arc.extract(path) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        eprintln!("{path}: {e}");
                        return ExitCode::from(1);
                    }
                };
                if cmd == "show" {
                    if wildcard {
                        print!("{path}\t");
                    }
                    println!(
                        "{}\t{}\t{:016x}",
                        arc.path().display(),
                        bytes.len(),
                        fnv1a(&bytes)
                    );
                } else {
                    let write = || -> std::io::Result<()> {
                        if wildcard && let Some(parent) = destination.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(destination, &bytes)
                    };
                    if let Err(e) = write() {
                        eprintln!("{path}: {}: {e}", destination.display());
                        return ExitCode::from(1);
                    }
                }
            }
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
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_pattern_is_not_opened_as_an_archive() {
        let args = ["list", "first.mpq", "second.mpq", "**/*.BLP"].map(str::to_owned);
        assert_eq!(trailing_operands("list", &args), 1);
        assert_eq!(trailing_operands("list", &args[..3]), 0);
        assert_eq!(trailing_operands("show", &args), 1);
        assert_eq!(trailing_operands("get", &args), 2);
    }

    #[test]
    fn archive_output_paths_cannot_escape_the_directory() {
        let root = std::path::Path::new("output");
        assert_eq!(
            output_path(root, "Textures\\a.blp").unwrap(),
            root.join("Textures/a.blp")
        );
        for name in ["../a", "a/../b", "/a", "\\a", "C:\\a", "a//b", "a/./b", ""] {
            assert!(output_path(root, name).is_err(), "{name:?}");
        }
    }
}
