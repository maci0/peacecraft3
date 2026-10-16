//! `blpx` — inspect a BLP file's header and raw mip payloads.
//!
//! The decoder deliberately refuses encodings it cannot prove, so this tool is
//! how we look at bytes before deciding what a new encoding is.
//!
//! A texture can be named two ways. A path to a `.blp` on disk is read
//! directly. A directory holding the WC3 archives is treated as an install: with
//! no second argument `info` tallies the whole texture set into the
//! `(magic, content)` census the decoder is measured against; with one, the
//! named file is pulled out of the archive set.

use std::process::ExitCode;

use pcx_blp::{
    Census, Framing, Header, MAX_MIPS, bytes_per_pixel, decode_jpeg, decode_mip, mip_bytes,
};

fn hex(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 3);
    for (i, b) in data.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// A quick tally of byte values, to tell an index plane from an alpha plane.
fn histogram(data: &[u8]) -> Vec<(u8, usize)> {
    let mut counts = [0usize; 256];
    for b in data {
        counts[*b as usize] += 1;
    }
    let mut v: Vec<_> = counts
        .iter()
        .enumerate()
        .filter(|(_, c)| **c > 0)
        .map(|(b, c)| (b as u8, *c))
        .collect();
    v.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    v
}

/// Where the bytes came from, so every command accepts either form.
enum Source<'a> {
    File(std::path::PathBuf),
    Install(&'a std::path::Path),
}

fn is_install(p: &std::path::Path) -> bool {
    p.is_dir()
}

/// Reads the named texture, whichever form the caller used.
fn read_texture(src: &Source<'_>, rel: Option<&str>) -> Result<Vec<u8>, String> {
    match src {
        Source::File(p) => std::fs::read(p).map_err(|e| format!("{}: {e}", p.display())),
        Source::Install(dir) => {
            let Some(rel) = rel else {
                return Err("an install needs a relative path inside the archives".into());
            };
            let mut set = pcx_mpq::MpqSet::open_wc3(dir).map_err(|e| format!("{dir:?}: {e}"))?;
            set.extract(rel).map_err(|e| format!("{rel}: {e}"))
        }
    }
}

/// Which decode path a header asks for, so no command has to guess.
fn decode(h: &Header, d: &[u8], mip: usize) -> Result<Vec<u8>, pcx_blp::Error> {
    match h.encoding() {
        pcx_blp::Encoding::Jpeg => decode_jpeg(d, h, mip),
        _ => decode_mip(d, h, mip),
    }
}

fn usage(cmd: &str) -> ExitCode {
    eprintln!("usage: blpx {cmd} <file.blp | install_dir> [relpath] [mip] [n]");
    ExitCode::from(2)
}

/// The `(magic, content)` census over every `.blp` an install ships.
fn census(dir: &std::path::Path) -> Result<(), String> {
    let mut set = pcx_mpq::MpqSet::open_wc3(dir).map_err(|e| format!("{dir:?}: {e}"))?;
    let blps: Vec<String> = set
        .file_list()
        .map_err(|e| format!("(listfile): {e}"))?
        .into_iter()
        .filter(|l| l.to_ascii_lowercase().ends_with(".blp"))
        .collect();
    let mut census = Census::new();
    for rel in &blps {
        if let Ok(d) = set.extract(rel) {
            census.record(&d);
        }
    }
    println!("{}", dir.display());
    println!("  .blp entries in (listfile) union   {}", blps.len());
    println!(
        "  {:<6} {:<8} {:>7} {:>9} {:>9}  bytes/pixel",
        "magic", "content", "files", "handled", "refused"
    );
    for row in census.buckets() {
        let refused: usize = row.refused.values().sum();
        let bpp = if row.bytes_per_pixel.is_empty() {
            "-".to_string()
        } else {
            row.bytes_per_pixel
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!(
            "  {:<6} {:<8} {:>7} {:>9} {:>9}  {bpp}",
            String::from_utf8_lossy(&row.magic),
            row.content,
            row.files,
            row.handled,
            refused,
        );
        for (kind, n) in &row.refused {
            println!("      refused x{n:<5} {kind}");
        }
    }
    println!(
        "  total {} handled {} refused {}",
        census.len(),
        census.handled(),
        census.refused()
    );
    Ok(())
}

fn info(label: &str, d: &[u8]) -> Result<(), pcx_blp::Error> {
    let h = Header::parse(d)?;
    println!("{label}");
    println!("  magic           {}", String::from_utf8_lossy(&h.magic));
    println!("  content         {}", h.content);
    println!(
        "  alpha bits      {} (0x08 on BLP1, 0x09 on BLP2)",
        h.alpha_bits
    );
    println!(
        "  format byte     {} (0x08 on BLP2)",
        if h.format == 0 { "-" } else { "set" }
    );
    println!("  width x height  {} x {}", h.width, h.height);
    println!("  picture type    {}", h.picture_type);
    println!("  has mipmaps     {}", h.has_mipmaps);
    println!("  mip count       {}", h.mip_count());
    println!("  encoding        {:?}", h.encoding());
    println!(
        "  palette base    {:#x}   entries {}",
        h.palette_base(),
        pcx_blp::palette_entry_count(&h)
    );
    for i in 0..MAX_MIPS {
        let (sz, off) = (h.mip_sizes[i] as usize, h.mip_offsets[i] as usize);
        if sz == 0 {
            continue;
        }
        let (w, hh) = h.mip_size(i);
        println!(
            "  mip{i:<2} {:>6}x{:<6} off {off:#08x} size {sz:>7}  {:.3} bpp",
            w,
            hh,
            sz as f64 / (w as f64 * hh as f64)
        );
    }
    let f = Framing::of(d, &h);
    println!(
        "  framing         payload {} + mips {} = {} (file {})",
        f.payload_start,
        f.mip_bytes,
        f.payload_start + f.mip_bytes,
        f.file_len
    );
    println!(
        "  framing         bpp {}   jpeg header {}   jpeg gap {}",
        bytes_per_pixel(d, &h, 0)
            .unwrap_or(None)
            .map(|b| b.to_string())
            .unwrap_or_else(|| "-".to_string()),
        f.jpeg_header_len
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string()),
        f.jpeg_gap_len
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string()),
    );
    let violations = f.violations(d, &h);
    println!(
        "  framing         {}",
        if violations.is_empty() {
            "all relations hold".to_string()
        } else {
            violations.join("; ")
        }
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(String::as_str) else {
        return usage("<command>");
    };
    if args.len() < 2 {
        return usage(cmd);
    }
    let first = std::path::PathBuf::from(&args[1]);
    // `leak` is what lets `Source` borrow the path for the whole process, which
    // is the only lifetime a CLI needs.
    let source = if is_install(&first) {
        Source::Install(first.leak())
    } else {
        Source::File(first)
    };
    // With an install, the second argument names a file inside the archives;
    // with a plain file, it is the first positional of the command itself.
    let rest: &[String] = args.get(2..).unwrap_or(&[]);
    if cmd == "info" && matches!(source, Source::Install(_)) && rest.is_empty() {
        let Source::Install(dir) = source else {
            unreachable!()
        };
        if let Err(e) = census(dir) {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }

    let label = match &source {
        Source::File(p) => p.display().to_string(),
        Source::Install(_) => rest.first().cloned().unwrap_or_else(|| args[1].clone()),
    };
    let data = match read_texture(&source, rest.first().map(String::as_str)) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let tail = if matches!(source, Source::Install(_)) {
        &rest[1..]
    } else {
        rest
    };
    let num = |i: usize, dflt: usize| -> usize {
        tail.get(i).and_then(|s| s.parse().ok()).unwrap_or(dflt)
    };

    if cmd == "info" {
        if let Err(e) = info(&label, &data) {
            eprintln!("{label}: {e}");
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }

    let h = match Header::parse(&data) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{label}: {e}");
            return ExitCode::from(1);
        }
    };

    if cmd == "dump" {
        let mip = num(0, 0);
        let want = num(1, 64);
        let raw = match mip_bytes(&data, &h, mip) {
            Ok(raw) => raw,
            Err(error) => {
                eprintln!("{label}: {error}");
                return ExitCode::from(1);
            }
        };
        let off = h.mip_offsets[mip] as usize;
        let size = raw.len();
        let (w, hh) = h.mip_size(mip);
        let px = u64::from(w) * u64::from(hh);
        println!(
            "mip{mip} {w}x{hh} off {off:#x} size {size} px {px} bpp {:.3}",
            size as f64 / px as f64
        );
        let n = want.min(raw.len());
        println!("  head[0..{n}]   {}", hex(&raw[..n]));
        println!("  head histogram {:?}", histogram(&raw[..n]));
        if size > n {
            println!("  tail[-16..]    {}", hex(&raw[size.saturating_sub(16)..]));
        }
        let px = usize::try_from(px).unwrap_or(usize::MAX);
        for (label, base) in [("half A", 0usize), ("half B", px.min(raw.len() / 2))] {
            let len = px.min(raw.len());
            if base + len <= raw.len() && len > 0 {
                println!(
                    "  {label} histogram {:?}",
                    histogram(&raw[base..base + len.min(4096)])
                );
            }
        }
        return ExitCode::SUCCESS;
    }

    let mip = match cmd {
        "pixels" | "hash" | "export" => num(0, 0),
        _ => {
            eprintln!("unknown command {cmd}");
            return ExitCode::from(2);
        }
    };

    if cmd == "pixels" {
        let n = num(1, 8);
        let px = decode(&h, &data, mip).unwrap_or_else(|e| {
            eprintln!("decode: {e}");
            std::process::exit(1);
        });
        println!("{} pixels ({} rgba8 bytes)", px.len() / 4, px.len());
        for i in 0..n.min(px.len() / 4) {
            let p = &px[i * 4..i * 4 + 4];
            println!("  px{i:<6} rgba({}, {}, {}, {})", p[0], p[1], p[2], p[3]);
        }
        return ExitCode::SUCCESS;
    }

    if cmd == "export" {
        let out = tail
            .get(1)
            .cloned()
            .unwrap_or_else(|| "/tmp/opencode/blps2/out.rgba".to_string());
        let rgb_only = tail.get(2).map(String::as_str) == Some("rgb");
        let px = decode(&h, &data, mip).unwrap_or_else(|e| {
            eprintln!("decode: {e}");
            std::process::exit(1);
        });
        let bytes: Vec<u8> = if rgb_only {
            px.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
        } else {
            px
        };
        std::fs::write(&out, &bytes).unwrap_or_else(|e| {
            eprintln!("{out}: {e}");
            std::process::exit(1);
        });
        println!("{out}: {} bytes", bytes.len());
        return ExitCode::SUCCESS;
    }

    // hash
    let px = decode(&h, &data, mip).unwrap_or_else(|e| {
        eprintln!("decode: {e}");
        std::process::exit(1);
    });
    // FNV-1a, so an external decoder can be compared byte for byte.
    // `hash <file> <mip> rgb` drops the alpha byte so the digest matches a
    // decoder that emits packed RGB.
    let rgb_only = tail.get(1).map(String::as_str) == Some("rgb");
    let mut h64: u64 = 0xcbf2_9ce4_8422_2325;
    let mut bytes = 0usize;
    for (i, b) in px.iter().enumerate() {
        if rgb_only && i % 4 == 3 {
            continue;
        }
        h64 ^= u64::from(*b);
        h64 = h64.wrapping_mul(0x100_0000_01b3);
        bytes += 1;
    }
    println!(
        "mip{mip} {bytes} bytes{} fnv1a64 {h64:016x}",
        if rgb_only { " rgb" } else { " rgba8" }
    );
    ExitCode::SUCCESS
}
