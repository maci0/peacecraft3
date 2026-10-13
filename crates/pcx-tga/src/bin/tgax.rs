//! `tgax` — inspect a TGA file's header, pixels and decoded digest.
//!
//! Mirrors `blpx`. TGA has no magic number, so `info` reports the header's
//! self-describing fields and the measured variant rather than a format name.
//! The decoder refuses variants it cannot prove, so this is also how a new
//! encoding shows up before anyone guesses at it.

use std::process::ExitCode;

use pcx_tga::{Header, fnv1a64};

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

/// A quick tally of byte values, to eyeball a payload without decoding it.
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

const USAGE: &str = "usage: tgax info|dump|pixels|hash|export <file.tga> [arg] [arg]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(String::as_str) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if args.len() < 2 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let path = &args[1];
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::from(1);
        }
    };
    let h = match Header::parse(&data) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::from(1);
        }
    };

    if cmd == "info" {
        println!("{path}");
        println!("  id length       {}", h.id_length);
        println!(
            "  colour map      type {} ({} entries of {} bits)",
            h.colour_map_type, h.colour_map_entries, h.colour_map_entry_bits
        );
        println!(
            "  image type      {} ({:?})",
            h.image_type.raw(),
            h.image_type
        );
        println!(
            "  encoding        {}",
            if h.image_type.is_run_length_encoded() {
                "RLE"
            } else {
                "uncompressed"
            }
        );
        println!("  dimensions      {} x {}", h.width, h.height);
        println!("  origin field    {} x {}", h.x_origin, h.y_origin);
        println!("  pixel depth     {} bits", h.pixel_depth);
        println!(
            "  descriptor      {:#04x} (alpha bits {}, origin {:?}, right-to-left {})",
            h.descriptor,
            h.descriptor & 0x0F,
            h.origin(),
            h.is_right_to_left()
        );
        println!(
            "  image data at   {}",
            h.image_data_offset()
                .map(|o| format!("{o} (#{o:x})"))
                .unwrap_or_else(|| "unknown (bad colour map type)".into())
        );
        if let Some(raw) = h.raw_pixel_bytes() {
            println!(
                "  uncompressed    {raw} bytes ({:.3} of file)",
                raw as f64 / data.len() as f64
            );
        }
        match pcx_tga::footer(&data) {
            Some(f) => println!(
                "  footer          TRUEVISION-XFILE. (extension {:?}, developer {:?})",
                f.extension_area_offset, f.developer_directory_offset
            ),
            None => println!("  footer          absent{}", trailing_note(&data, &h)),
        }
        println!("  file length     {}", data.len());
        return ExitCode::SUCCESS;
    }

    if cmd == "dump" {
        let want: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(64);
        let Some(at) = h.image_data_offset() else {
            eprintln!("{path}: cannot locate image data");
            return ExitCode::from(1);
        };
        let raw = &data[at..];
        let n = want.min(raw.len());
        println!(
            "image data at {at} (#{at:x}), {} bytes for a {}-pixel raster",
            raw.len(),
            h.pixel_count()
        );
        println!("  head[0..{n}]   {}", hex(&raw[..n]));
        println!("  head histogram {:?}", histogram(&raw[..n]));
        if raw.len() > n {
            println!("  tail[-16..]    {}", hex(&raw[raw.len() - 16..]));
        }
        return ExitCode::SUCCESS;
    }

    let decoded = |path: &str, h: &Header, data: &[u8]| match pcx_tga::decode_with(data, h) {
        Ok(px) => px,
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    };

    if cmd == "pixels" {
        let n: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);
        let px = decoded(path, &h, &data);
        println!("{} pixels ({} rgba8 bytes)", px.len() / 4, px.len());
        for i in 0..n.min(px.len() / 4) {
            let p = &px[i * 4..i * 4 + 4];
            println!("  px{i:<6} rgba({}, {}, {}, {})", p[0], p[1], p[2], p[3]);
        }
        return ExitCode::SUCCESS;
    }

    if cmd == "export" {
        let out = args
            .get(2)
            .map(String::as_str)
            .unwrap_or("/tmp/opencode/tgax/out.rgba");
        let rgb_only = args.get(3).map(String::as_str) == Some("rgb");
        let px = decoded(path, &h, &data);
        let bytes: Vec<u8> = if rgb_only {
            px.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
        } else {
            px
        };
        if let Some(parent) = std::path::Path::new(out).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(out, &bytes).unwrap_or_else(|e| {
            eprintln!("{out}: {e}");
            std::process::exit(1);
        });
        println!("{out}: {} bytes", bytes.len());
        return ExitCode::SUCCESS;
    }

    if cmd == "hash" {
        // FNV-1a over the decoded RGBA8, so an external decoder can be compared
        // byte for byte. `hash <file> rgb` drops the alpha byte so the digest
        // matches a decoder that emits packed RGB.
        let rgb_only = args.get(2).map(String::as_str) == Some("rgb");
        let px = decoded(path, &h, &data);
        let bytes: Vec<u8> = if rgb_only {
            px.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
        } else {
            px.clone()
        };
        println!(
            "{}x{} {} bytes{} fnv1a64 {:016x}",
            h.width,
            h.height,
            bytes.len(),
            if rgb_only { " rgb" } else { " rgba8" },
            fnv1a64(&bytes)
        );
        return ExitCode::SUCCESS;
    }

    eprintln!("unknown command {cmd}");
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

/// Describes what sits between the image data and EOF, where a Truevision
/// footer would be. Only an uncompressed image can be measured from the header
/// alone: a run-length stream's end has to be found by walking it.
fn trailing_note(data: &[u8], h: &Header) -> String {
    match (h.image_data_offset(), h.raw_pixel_bytes()) {
        (Some(at), Some(raw)) if !h.image_type.is_run_length_encoded() => format!(
            " ({} trailing bytes after the image)",
            data.len().saturating_sub(at + raw)
        ),
        _ if h.image_type.is_run_length_encoded() => {
            " (run length: walk the stream to find where it ends)".to_string()
        }
        _ => String::new(),
    }
}
