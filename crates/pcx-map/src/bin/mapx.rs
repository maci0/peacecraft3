//! `mapx` — inspect W3M/W3N maps and their terrain straight out of a WC3 install.
//!
//! ```text
//! mapx census  <install_dir>
//! mapx info    <install_dir> <map.w3m>
//! mapx dump    <install_dir> <map.w3m> header|terrain|doo|shd|map|ter [bytes]
//! mapx hash    <install_dir> <map.w3m>
//! mapx hashall <install_dir>
//! mapx doo     <install_dir> <map.w3m> [info|dump [bytes]|hash]
//! mapx shd     <install_dir> <map.w3m> [info|dump [bytes]|hash]
//! mapx map     <install_dir> <map.w3m> [info|dump [bytes]|hash]
//! mapx ter     <install_dir> <map.w3m> [info|dump [bytes]|hash]
//! mapx w3i     <install_dir> <map.w3m> [info|dump [bytes]|hash]
//! ```
//!
//! `doo` is `war3map.doo` (doodad placements), `shd` is `war3map.shd` (the
//! shadow grid), `map` is the minimap image, `ter` the terrain minimap and
//! `w3i` is `war3map.w3i` (the map info file: player slots, forces and the
//! forced/suggested object-table lists).
//! The last two are tracker *role* names: the files that actually ship are
//! `war3mapMap.blp` and `war3map.mmp`, and literal `war3map.map` /
//! `war3map.ter` occur in zero shipped maps. The commands print that fact
//! rather than inventing a layout for names that do not exist.
//!
//! `hash` prints FNV-1a 64 digests over canonical forms: the whole map file,
//! the terrain re-serialised field by field, and each content file's own
//! canonical form. Those are what another decoder has to reproduce byte for
//! byte before its output can be called equivalent — and they deliberately
//! contain no world height and no invented scale, because this crate decodes
//! neither.

use std::process::ExitCode;

use pcx_map::{Map, shipped_maps};

fn install_dir() -> String {
    std::env::args()
        .nth(2)
        .or_else(|| std::env::var("PEACECRAFT3_INSTALL").ok())
        .or_else(|| std::env::var("PEACECRAFT3_LEGACY_DIR").ok())
        .unwrap_or_else(|| "wc3/game/app".to_string())
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

fn slot(slot: [u8; 4]) -> String {
    String::from_utf8_lossy(&slot).into_owned()
}

fn hexdump(bytes: &[u8], n: usize) {
    for off in (0..n.min(bytes.len())).step_by(16) {
        let end = (off + 16).min(bytes.len());
        let row = &bytes[off..end];
        println!("  {off:#08x}  {:<47}  |{}|", hex(row), ascii(row));
    }
}

/// One of the four content formats the format subcommands cover.
enum Format {
    Doo,
    Shd,
    Map,
    Ter,
    W3i,
}

impl Format {
    fn label(&self) -> &'static str {
        match self {
            Format::Doo => "war3map.doo",
            Format::Shd => "war3map.shd",
            Format::Map => "war3mapMap.blp",
            Format::Ter => "war3map.mmp",
            Format::W3i => "war3map.w3i",
        }
    }

    fn role_note(&self) -> Option<&'static str> {
        match self {
            Format::Map => Some(
                "note      war3map.map: absent in every shipped map; the minimap image is war3mapMap.blp (or .tga)",
            ),
            Format::Ter => Some(
                "note      war3map.ter: absent in every shipped map; the terrain minimap is war3map.mmp",
            ),
            _ => None,
        }
    }
}

fn format_info(m: &Map<'_>, what: &Format) {
    let raw = match what {
        Format::Doo => m.extract("war3map.doo"),
        Format::Shd => m.extract("war3map.shd"),
        Format::Map => match m.minimap_image() {
            Ok(img) => Ok(img.raw().to_vec()),
            Err(e) => {
                println!("  unavailable: {e}");
                return;
            }
        },
        Format::Ter => m.extract("war3map.mmp"),
        Format::W3i => m.extract(pcx_map::info::INFO_NAME),
    };
    let raw = match raw {
        Ok(b) => b,
        Err(e) => {
            println!("  {}: unavailable: {e}", what.label());
            return;
        }
    };
    println!("{}: {} bytes", what.label(), raw.len());
    match what {
        Format::Doo => match m.doodads() {
            Ok(d) => {
                println!(
                    "  version   {} (subversion {})",
                    d.version(),
                    d.subversion()
                );
                match d.creation_number_range() {
                    Some((lo, hi)) => println!(
                        "  instances {} (creation numbers {lo}..={hi}, {})",
                        d.instances().len(),
                        if d.creation_numbers_are_unique() {
                            "unique"
                        } else {
                            "NOT unique"
                        }
                    ),
                    None => println!("  instances 0"),
                }
                println!(
                    "  terrain   {} doodads (section version {})",
                    d.terrain_doodads().len(),
                    d.terrain_section_version()
                );
                let want = 16 + 42 * d.instances().len() + 8 + 16 * d.terrain_doodads().len();
                println!(
                    "  framing   16 + 42*{} + 8 + 16*{} == {want} vs {}",
                    d.instances().len(),
                    d.terrain_doodads().len(),
                    raw.len()
                );
                println!("  hash      {:016x}", d.canonical_hash());
            }
            Err(e) => println!("  decode    refused: {e}"),
        },
        Format::Shd => match m.shadows() {
            Ok(s) => {
                let t = m.terrain().unwrap();
                println!(
                    "  grid      {} x {} cells (4 per tile edge of {} x {})",
                    s.columns(),
                    s.rows(),
                    s.width(),
                    s.height()
                );
                let vals: Vec<String> = s
                    .values_used()
                    .into_iter()
                    .map(|v| format!("{v:#04x}"))
                    .collect();
                println!("  values    {}", vals.join(" "));
                println!(
                    "  framing   16 * {} * {} == {} vs {}",
                    t.width(),
                    t.height(),
                    16 * t.width() as usize * t.height() as usize,
                    raw.len()
                );
                println!("  hash      {:016x}", s.canonical_hash());
            }
            Err(e) => println!("  decode    refused: {e}"),
        },
        Format::Map => match m.minimap_image() {
            Ok(img) => {
                match img.kind() {
                    pcx_map::MinimapImageKind::Blp { header, framing } => {
                        println!(
                            "  container BLP {:?} {:?} {}x{}, {} mip(s), jpeg header {:?}",
                            header.variant(),
                            header.encoding(),
                            header.width,
                            header.height,
                            header.mip_count(),
                            framing.jpeg_header_len
                        );
                        println!(
                            "  framing   sums={} jpeg gap padding={}",
                            framing.sums(),
                            framing.jpeg_gap_is_padding(img.raw())
                        );
                    }
                    pcx_map::MinimapImageKind::Tga { header } => {
                        println!(
                            "  container TGA {:?} {}x{}",
                            header.image_type, header.width, header.height
                        );
                    }
                }
                println!("  hash      {:016x}", img.canonical_hash());
            }
            Err(e) => println!("  decode    refused: {e}"),
        },
        Format::W3i => match m.info() {
            Ok(i) => {
                println!(
                    "  version   {} (the only version either build writes)",
                    i.version()
                );
                println!("  consts    {} / {:?}", i.constant_a, i.constant_b);
                println!(
                    "  name      {} (w3s reference; {})",
                    i.name,
                    if i.name.starts_with("TRIGSTR_") {
                        "a TRIGSTR table entry"
                    } else {
                        "literal text"
                    }
                );
                println!("  desc      {}", i.description);
                println!("  camera    {:?} / {:?}", i.camera.first, i.camera.second);
                println!("  bounds    {:?}", i.bounds);
                println!(
                    "  players   {} slots, {} forces",
                    i.players().len(),
                    i.forces().len()
                );
                for (n, p) in i.players().iter().enumerate() {
                    println!(
                        "    {:<2} ctl={:<2} race={} start={} colour={} {} at ({}, {})",
                        n,
                        p.controller,
                        p.race,
                        p.start_point,
                        p.colour,
                        p.name,
                        p.start_x,
                        p.start_y
                    );
                }
                for f in i.forces() {
                    println!("    force {:<#010x} {}", f.mask, f.name);
                }
                let lists = i.table_lists();
                println!("  tables    {} forced/suggested lists", lists.len());
                for l in lists {
                    println!(
                        "    {:#04x} {:<22} M={} K={}",
                        l.id,
                        l.name,
                        l.first.len(),
                        l.second.len()
                    );
                }
                println!(
                    "  xrefs     {} start locations, {} pairs",
                    i.start_locations.len(),
                    i.pairs.len()
                );
                let fr = i.framing();
                println!(
                    "  framing   head {} + {} players + {} forces + {} xrefs + {} table lists + 8 == {}",
                    fr.head_bytes,
                    fr.player_bytes,
                    fr.force_bytes,
                    fr.xref_bytes,
                    fr.table_list_bytes,
                    fr.file_len
                );
                println!("  hash      {:016x}", i.canonical_hash());
            }
            Err(e) => println!("  decode    refused: {e}"),
        },
        Format::Ter => match m.minimap_markers() {
            Ok(mm) => {
                println!("  version   {}", mm.version());
                println!("  markers   {}", mm.markers().len());
                let kinds: Vec<String> = mm
                    .kinds_used()
                    .into_iter()
                    .map(|k| format!("{k}"))
                    .collect();
                println!("  kinds     {}", kinds.join(","));
                if let Some((x0, x1, y0, y1)) = mm.coordinate_range() {
                    println!("  coords    x {x0}..={x1}, y {y0}..={y1}");
                }
                println!(
                    "  framing   8 + 16*{} == {} vs {}",
                    mm.markers().len(),
                    8 + 16 * mm.markers().len(),
                    raw.len()
                );
                println!("  hash      {:016x}", mm.canonical_hash());
            }
            Err(e) => println!("  decode    refused: {e}"),
        },
    }
    if let Some(note) = what.role_note() {
        println!("  {note}");
    }
}

fn format_hash(m: &Map<'_>, what: &Format) {
    match what {
        Format::Doo => match m.doodads() {
            Ok(d) => println!("{:016x}  doo {}", d.canonical_hash(), what.label()),
            Err(e) => println!("doo unavailable: {e}"),
        },
        Format::Shd => match m.shadows() {
            Ok(s) => println!("{:016x}  shd {}", s.canonical_hash(), what.label()),
            Err(e) => println!("shd unavailable: {e}"),
        },
        Format::Map => match m.minimap_image() {
            Ok(i) => println!("{:016x}  map {}", i.canonical_hash(), i.name),
            Err(e) => println!("map unavailable: {e}"),
        },
        Format::Ter => match m.minimap_markers() {
            Ok(mm) => println!("{:016x}  ter {}", mm.canonical_hash(), what.label()),
            Err(e) => println!("ter unavailable: {e}"),
        },
        Format::W3i => match m.info() {
            Ok(i) => println!("{:016x}  w3i {}", i.canonical_hash(), what.label()),
            Err(e) => println!("w3i unavailable: {e}"),
        },
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

    let format_cmd = match cmd {
        "doo" => Some(Format::Doo),
        "shd" => Some(Format::Shd),
        "map" => Some(Format::Map),
        "ter" => Some(Format::Ter),
        "w3i" => Some(Format::W3i),
        _ => None,
    };

    if let Some(what) = format_cmd {
        let Some(path) = args.get(2) else {
            eprintln!("usage: mapx {cmd} <install_dir> <map.w3m> [info|dump [bytes]|hash]");
            return ExitCode::from(2);
        };
        let raw = match set.extract(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::from(1);
            }
        };
        let m = match Map::parse(&raw) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::from(1);
            }
        };
        println!("{path}");
        match args.get(3).map(String::as_str).unwrap_or("info") {
            "info" => format_info(&m, &what),
            "dump" => {
                let n: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(256);
                let bytes = match &what {
                    Format::Doo => m.extract("war3map.doo"),
                    Format::Shd => m.extract("war3map.shd"),
                    Format::Map => m
                        .minimap_image()
                        .map(|i| i.raw().to_vec())
                        .map_err(|e| pcx_mpq::Error::NotFound(e.to_string())),
                    Format::Ter => m.extract("war3map.mmp"),
                    Format::W3i => m.extract(pcx_map::info::INFO_NAME),
                };
                match bytes {
                    Ok(b) => {
                        println!("{} {} bytes", what.label(), b.len());
                        hexdump(&b, n);
                        if let Some(note) = what.role_note() {
                            println!("  {note}");
                        }
                    }
                    Err(e) => {
                        eprintln!("{path}: {e}");
                        return ExitCode::from(1);
                    }
                }
            }
            "hash" => format_hash(&m, &what),
            other => {
                eprintln!("{cmd} takes `info`, `dump` or `hash`, got {other}");
                return ExitCode::from(2);
            }
        }
        return ExitCode::SUCCESS;
    }

    match cmd {
        "census" => {
            let maps = match shipped_maps(&mut set) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("listfile: {e}");
                    return ExitCode::from(1);
                }
            };
            let n_m = maps.iter().filter(|p| p.ends_with(".w3m")).count();
            let n_n = maps.iter().filter(|p| p.ends_with(".w3n")).count();
            println!("{dir}: {} maps ({} .w3m, {} .w3n)", maps.len(), n_m, n_n);
            let mut ok = 0usize;
            let mut terr = 0usize;
            let mut content = [0usize; 4];
            let mut unproven = 0usize;
            for path in &maps {
                let Ok(raw) = set.extract(path) else {
                    eprintln!("  MISS {path}");
                    continue;
                };
                match Map::parse(&raw) {
                    Ok(m) => {
                        ok += 1;
                        if m.terrain().is_ok() {
                            terr += 1;
                        }
                        let c = m.content_files();
                        content[0] += c.doodads as usize;
                        content[1] += c.shadows as usize;
                        content[2] += c.minimap_markers as usize;
                        content[3] += c.minimap_image.is_some() as usize;
                        unproven += c.unproven_minimap.is_some() as usize;
                    }
                    Err(e) => eprintln!("  BAD  {path}: {e}"),
                }
            }
            println!("  {ok} headers parsed, {terr} terrains decoded");
            println!(
                "  content: doo {}, shd {}, mmp {}, minimap image {}",
                content[0], content[1], content[2], content[3]
            );
            println!("  war3map.map / war3map.ter present: {unproven} (expected 0)");
        }

        "info" => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: mapx info <install_dir> <map.w3m>");
                return ExitCode::from(2);
            };
            let raw = match set.extract(path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            let m = match Map::parse(&raw) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            let h = &m.header;
            println!("{path}");
            println!("  file      {} bytes", m.len());
            println!("  version   {}", h.version);
            println!("  name      {:?}", h.name);
            println!("  field_a   {}  (unattributed)", h.field_a);
            println!("  field_b   {}  (players)", h.field_b);
            println!(
                "  archive   +{:#x} .. +{:#x}  ({})",
                h.archive_offset,
                h.archive_offset + h.archive_size,
                h.archive_size
            );
            match &h.trailer {
                Some(t) => println!("  trailer   {} bytes, tag {:02x?}", t.len(), &t[..4]),
                None => println!("  trailer   absent"),
            }
            println!("  hash      {:016x}", pcx_map::map_hash(m.raw()));
            match m.terrain() {
                Ok(t) => {
                    println!("  terrain   war3map.w3e, version {}", t.version());
                    println!(
                        "  size      {} x {} tiles, {} corners",
                        t.width(),
                        t.height(),
                        t.corner_count()
                    );
                    println!(
                        "  origin    {:?} / {:?}",
                        f32::from_bits(t.origin().0.to_bits()),
                        f32::from_bits(t.origin().1.to_bits())
                    );
                    println!("  default   {:#04x}", t.default_slot());
                    print!("  slots     ");
                    for s in t.slots() {
                        print!("{} ", slot(*s));
                    }
                    print!("| ");
                    for s in t.secondary_slots() {
                        print!("{} ", slot(*s));
                    }
                    println!();
                    if let Some((lo, hi)) = t.height_index_range() {
                        println!("  heights   14-bit index {lo}..={hi} of 0..=16383 (raw)");
                    }
                    println!("  nibbles   {:?}", t.slot_nibbles_used());
                    println!("  hash      {:016x}", t.canonical_hash());
                }
                Err(e) => println!("  terrain   unavailable: {e}"),
            }
            let c = m.content_files();
            println!(
                "  content   doo={} shd={} mmp={} image={:?} unproven={:?}",
                c.doodads, c.shadows, c.minimap_markers, c.minimap_image, c.unproven_minimap
            );
            if c.doodads
                && let Ok(d) = m.doodads()
            {
                println!(
                    "  doo       {} instances, {} terrain doodads, hash {:016x}",
                    d.instances().len(),
                    d.terrain_doodads().len(),
                    d.canonical_hash()
                );
            }
            if c.shadows
                && let Ok(s) = m.shadows()
            {
                println!(
                    "  shd       {} x {} cells, hash {:016x}",
                    s.columns(),
                    s.rows(),
                    s.canonical_hash()
                );
            }
            if c.minimap_markers
                && let Ok(mm) = m.minimap_markers()
            {
                println!(
                    "  mmp       {} markers, hash {:016x}",
                    mm.markers().len(),
                    mm.canonical_hash()
                );
            }
            if c.minimap_image.is_some()
                && let Ok(img) = m.minimap_image()
            {
                println!(
                    "  minimap   {} {}x{}, framed={}, hash {:016x}",
                    img.name,
                    img.dimensions().0,
                    img.dimensions().1,
                    img.framing_holds(),
                    img.canonical_hash()
                );
            }
        }

        "dump" => {
            let (Some(path), Some(what)) = (args.get(2), args.get(3)) else {
                eprintln!(
                    "usage: mapx dump <install_dir> <map.w3m> header|terrain|doo|shd|map|ter [bytes]"
                );
                return ExitCode::from(2);
            };
            let n: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(256);
            let raw = match set.extract(path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            let bytes: Vec<u8> = match what.as_str() {
                "header" => raw[..raw.len().min(0x200)].to_vec(),
                "terrain" => {
                    let Ok(m) = Map::parse(&raw) else {
                        eprintln!("{path}: bad header");
                        return ExitCode::from(1);
                    };
                    match m.extract("war3map.w3e") {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{path}: {e}");
                            return ExitCode::from(1);
                        }
                    }
                }
                "doo" | "shd" | "map" | "ter" => {
                    let Ok(m) = Map::parse(&raw) else {
                        eprintln!("{path}: bad header");
                        return ExitCode::from(1);
                    };
                    let got = match what.as_str() {
                        "doo" => m.extract("war3map.doo"),
                        "shd" => m.extract("war3map.shd"),
                        "map" => m
                            .minimap_image()
                            .map(|i| i.raw().to_vec())
                            .map_err(|e| pcx_mpq::Error::NotFound(e.to_string())),
                        _ => m.extract("war3map.mmp"),
                    };
                    match got {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("{path}: {e}");
                            return ExitCode::from(1);
                        }
                    }
                }
                _ => {
                    eprintln!("dump takes `header`, `terrain`, `doo`, `shd`, `map` or `ter`");
                    return ExitCode::from(2);
                }
            };
            println!("{path} {what}: {} bytes", bytes.len());
            hexdump(&bytes, n);
        }

        "hash" => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: mapx hash <install_dir> <map.w3m>");
                return ExitCode::from(2);
            };
            let raw = match set.extract(path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            let m = match Map::parse(&raw) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            println!("{path} file {:016x}", pcx_map::map_hash(m.raw()));
            match m.terrain() {
                Ok(t) => println!("{path} terrain {:016x}", t.canonical_hash()),
                Err(e) => println!("{path} terrain unavailable: {e}"),
            }
            for what in [Format::Doo, Format::Shd, Format::Map, Format::Ter] {
                print!("{path} ");
                format_hash(&m, &what);
            }
        }

        "hashall" => {
            let maps = match shipped_maps(&mut set) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("listfile: {e}");
                    return ExitCode::from(1);
                }
            };
            for path in &maps {
                let Ok(raw) = set.extract(path) else { continue };
                let Ok(m) = Map::parse(&raw) else { continue };
                match m.terrain() {
                    Ok(t) => println!("{:016x}  {}", t.canonical_hash(), path),
                    Err(_) => println!(
                        "{:016x}  {}  (no terrain)",
                        pcx_map::map_hash(m.raw()),
                        path
                    ),
                }
            }
        }

        other => {
            eprintln!("unknown command {other}");
            eprintln!("usage: mapx <census|info|dump|hash|hashall|doo|shd|map|ter> ...");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
