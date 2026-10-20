//! `mdlx` — inspect MDX models straight out of a WC3 install.
//!
//! The decoder refuses to guess. When a record layout will not commit to
//! itself, this tool is how the bytes get looked at before a decision is made.

use std::process::ExitCode;

use pcx_mdl::{Mdlx, tag};

/// The common leading argument. Every subcommand takes the install directory
/// first, so `rank`, `info` and `dump` all read it the same way.
fn install_dir() -> String {
    std::env::args()
        .nth(2)
        .or_else(|| std::env::var("PEACECRAFT3_INSTALL").ok())
        .or_else(|| std::env::var("PEACECRAFT3_LEGACY_DIR").ok())
        .unwrap_or_else(|| "wc3/game/app".to_string())
}

fn open_set(dir: &str) -> Result<pcx_mpq::MpqSet, pcx_mpq::Error> {
    pcx_mpq::MpqSet::open_wc3(dir)
}

/// Parses a chunk tag, padding short input so "BONE" and "BN" both work.
fn tag4(s: &str) -> [u8; 4] {
    let mut t = [b' '; 4];
    let b = s.as_bytes();
    let n = b.len().min(4);
    t[..n].copy_from_slice(&b[..n]);
    t
}

fn hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn ascii(data: &[u8]) -> String {
    data.iter()
        .map(|b| {
            if (32..127).contains(b) {
                *b as char
            } else {
                '.'
            }
        })
        .collect()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("info");
    let dir = install_dir();

    let mut set = match open_set(&dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{dir}: {e}");
            return ExitCode::from(1);
        }
    };

    match cmd {
        // The workhorse for record-layout work: rank models by the size of one
        // chunk, so the smallest instance of it is available to dissect.
        "rank" => {
            let want = tag4(args.get(3).map(String::as_str).unwrap_or("BONE"));
            let paths = match set.file_list() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("listfile: {e}");
                    return ExitCode::from(1);
                }
            };
            let mut rows: Vec<(usize, String, Vec<String>)> = Vec::new();
            for rel in paths
                .iter()
                .filter(|p| p.to_ascii_lowercase().ends_with(".mdx"))
            {
                let Ok(d) = set.extract(rel) else { continue };
                let Ok(m) = Mdlx::parse(&d) else { continue };
                let Some(size) = m.chunk(&want).map(|c| c.size) else {
                    continue;
                };
                rows.push((size, rel.clone(), m.tags()));
            }
            rows.sort_by_key(|(s, _, _)| *s);
            let show: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(20);
            eprintln!(
                "{} models carry a {} chunk",
                rows.len(),
                String::from_utf8_lossy(&want)
            );
            for (size, rel, _) in rows.iter().take(show) {
                println!("{size:>9}  {rel}");
            }
        }

        "info" => {
            let Some(path) = args.get(3) else {
                eprintln!("usage: mdlx info <install_dir> <model.mdx>");
                return ExitCode::from(2);
            };
            let d = match set.extract(path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            let m = match Mdlx::parse(&d) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            println!("{path}");
            println!("  version {}", m.version);
            if let Ok(model) = m.model(&d) {
                println!(
                    "  name {:?}  extent {:?} .. {:?}",
                    model.name, model.minimum_extent, model.maximum_extent
                );
            }
            for c in &m.chunks {
                println!(
                    "  {:<5} off {:#08x} size {:>9}",
                    String::from_utf8_lossy(&c.tag),
                    c.offset,
                    c.size
                );
            }
            if let Ok(g) = m.geosets(&d) {
                println!("  geosets: {}", g.len());
                for (i, x) in g.iter().enumerate() {
                    println!(
                        "    [{i}] size {:>8} tag {:?}",
                        x.size,
                        String::from_utf8_lossy(&x.data[..4.min(x.data.len())])
                    );
                }
            }
            if let Ok(s) = m.sequences(&d) {
                println!("  sequences: {}", s.len());
                for q in &s {
                    println!(
                        "    {:<24} {:>6} .. {:>6} ({:>5} ms) move {:<8.1} loop {}",
                        q.name,
                        q.interval_start,
                        q.interval_end,
                        q.duration(),
                        q.move_speed,
                        !q.non_looping
                    );
                }
            }
            if let Ok(t) = m.textures(&d) {
                println!("  textures: {}", t.len());
                for x in &t {
                    println!("    slot {:<4} {:?}", x.slot, x.path);
                }
            }
        }

        "dump" => {
            let (Some(path), Some(want)) = (args.get(2), args.get(3)) else {
                eprintln!("usage: mdlx dump <install_dir> <model.mdx> <TAG> [bytes]");
                return ExitCode::from(2);
            };
            let bytes: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(256);
            let t = tag4(want);
            let d = match set.extract(path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            let m = match Mdlx::parse(&d) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            let Some(c) = m.chunk(&t) else {
                eprintln!("{path}: no {want} chunk");
                return ExitCode::from(1);
            };
            let p = c.payload(&d);
            println!("{} {} payload {} bytes", path, want, p.len());
            let n = bytes.min(p.len());
            for off in (0..n).step_by(16) {
                let end = (off + 16).min(n);
                let row = &p[off..end];
                println!("  {off:#08x}  {:<47}  |{}|", hex(row), ascii(row));
            }
            let _ = tag::GEOS;
        }

        // Bone world translations at each sampled sequence time, in model units. The
        // lowest bone's height over a cycle counts footfalls; the root tracks travel.
        "pose" => {
            let (Some(path), Some(name)) = (args.get(3), args.get(4)) else {
                eprintln!("usage: mdlx pose <install_dir> <model.mdx> <sequence> [step_ms]");
                return ExitCode::from(2);
            };
            let step: u32 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(10);
            if step == 0 {
                eprintln!("step must be positive");
                return ExitCode::from(2);
            }
            let d = match set.extract(path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            let m = match Mdlx::parse(&d) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            let sequences = m.sequences(&d).unwrap_or_default();
            let Some(sequence) = sequences.iter().find(|s| s.name.eq_ignore_ascii_case(name))
            else {
                eprintln!("{path}: no sequence named {name}");
                return ExitCode::from(1);
            };
            let bones = m.bones(&d).unwrap_or_default();
            let mut parents = std::collections::HashMap::new();
            for bone in &bones {
                parents.insert(bone.object_id, bone.parent_id);
            }
            for node in m.nodes(&d, tag::HELP).unwrap_or_default() {
                parents.insert(node.object_id, node.parent_id);
            }
            let pivots = m.pivots(&d).unwrap_or_default();
            let parent = |id: u32| parents.get(&id).and_then(|&p| u32::try_from(p).ok());
            println!(
                "# {path} sequence {} interval {}..{} ms step {step} bones {}",
                sequence.name,
                sequence.interval_start,
                sequence.interval_end,
                bones.len()
            );
            println!("elapsed_ms\tbone\tname\tx\ty\tz");
            let mut ms = sequence.interval_start;
            while ms <= sequence.interval_end {
                let world = pcx_mdl::bone_matrices(&bones, &parent, &pivots, ms);
                for (index, (bone, matrix)) in bones.iter().zip(&world).enumerate() {
                    println!(
                        "{}\t{index}\t{}\t{}\t{}\t{}",
                        ms - sequence.interval_start,
                        bone.name,
                        matrix[12],
                        matrix[13],
                        matrix[14]
                    );
                }
                ms += step;
            }
        }

        other => {
            eprintln!("unknown command {other}");
            eprintln!("usage: mdlx <rank|info|dump|pose> ...");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
