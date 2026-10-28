//! Which shipped JPEG BLPs carry a non-zero fourth component although
//! `alpha_bits` is 0. Needs a local original install; see the
//! `#[ignore]` reason.

use std::collections::BTreeMap;

use pcx_blp::{Header, MAX_MIPS, jpeg_header, mip_bytes};

/// Per-file result for a four-component, `alpha_bits == 0` JPEG BLP.
struct Cmyk {
    /// Mips whose decoded fourth component has at least one non-zero value.
    nonzero_mips: Vec<usize>,
    mips: usize,
    /// Smallest and largest fourth-component value over all mips.
    range: (u8, u8),
}

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR pointing at a local Legacy install"]
fn fourth_component_census_over_shipped_jpeg_blps() {
    let dir = std::env::var_os("PEACECRAFT3_LEGACY_DIR").expect("set PEACECRAFT3_LEGACY_DIR");
    let mut set = pcx_mpq::MpqSet::open_wc3(dir).expect("open WC3 archive set");
    let names: Vec<String> = set
        .file_list()
        .expect("union listfile")
        .into_iter()
        .filter(|n| n.to_ascii_lowercase().ends_with(".blp"))
        .collect();

    let mut blps = 0usize;
    let mut jpeg_files = 0usize;
    let mut undecodable = Vec::new();
    let mut formats: BTreeMap<String, usize> = BTreeMap::new();
    let mut alpha_cmyk_files = 0usize;
    let mut zero_alpha: Vec<(String, Cmyk)> = Vec::new();
    let mut zero_alpha_cmyk_files = 0usize;
    for rel in &names {
        let Ok(d) = set.extract(rel) else { continue };
        blps += 1;
        let Ok(h) = Header::parse(&d) else { continue };
        if h.content != 0 {
            continue;
        }
        jpeg_files += 1;
        let Ok(head) = jpeg_header(&d, &h) else {
            undecodable.push(format!("{rel}: jpeg header"));
            continue;
        };
        let mut nonzero_mips = Vec::new();
        let mut range = (u8::MAX, u8::MIN);
        let mut is_cmyk = false;
        for i in 0..h.mip_count().min(MAX_MIPS) {
            let Ok(body) = mip_bytes(&d, &h, i) else {
                undecodable.push(format!("{rel}: mip {i} bytes"));
                continue;
            };
            let mut stream = head.to_vec();
            stream.extend_from_slice(body);
            let mut dec = jpeg_decoder::Decoder::new(std::io::Cursor::new(&stream));
            let Ok(px) = dec.decode() else {
                undecodable.push(format!("{rel}: mip {i} decode"));
                continue;
            };
            let format = dec.info().expect("frame info").pixel_format;
            *formats.entry(format!("{format:?}")).or_default() += 1;
            if format == jpeg_decoder::PixelFormat::CMYK32 {
                is_cmyk = true;
                for p in px.as_chunks::<4>().0 {
                    range = (range.0.min(p[3]), range.1.max(p[3]));
                }
                if px.as_chunks::<4>().0.iter().any(|p| p[3] != 0) {
                    nonzero_mips.push(i);
                }
            }
        }
        if is_cmyk {
            if h.alpha_bits > 0 {
                alpha_cmyk_files += 1;
            } else {
                zero_alpha_cmyk_files += 1;
                zero_alpha.push((
                    rel.clone(),
                    Cmyk {
                        nonzero_mips,
                        mips: h.mip_count(),
                        range,
                    },
                ));
            }
        }
    }

    eprintln!("blp files extracted: {blps}, jpeg-content: {jpeg_files}");
    eprintln!("mip pixel formats (mips decoded): {formats:?}");
    eprintln!("undecodable: {} {:?}", undecodable.len(), undecodable);
    eprintln!("four-component, alpha_bits > 0: {alpha_cmyk_files}");
    eprintln!("four-component, alpha_bits == 0: {zero_alpha_cmyk_files}");
    let affected: Vec<&(String, Cmyk)> = zero_alpha
        .iter()
        .filter(|(_, c)| !c.nonzero_mips.is_empty())
        .collect();
    eprintln!(
        "alpha_bits == 0 with a non-zero fourth component in any mip: {}",
        affected.len()
    );
    let mut per_mip: BTreeMap<usize, usize> = BTreeMap::new();
    let mut mip0 = 0usize;
    let mut lower_only = 0usize;
    let mut all_mips = 0usize;
    for (_, c) in &affected {
        for &m in &c.nonzero_mips {
            *per_mip.entry(m).or_default() += 1;
        }
        if c.nonzero_mips.contains(&0) {
            mip0 += 1;
        } else {
            lower_only += 1;
        }
        if c.nonzero_mips.len() == c.mips {
            all_mips += 1;
        }
    }
    eprintln!("per-mip count of affected files: {per_mip:?}");
    eprintln!(
        "affected at mip 0: {mip0}, only at lower mips: {lower_only}, at every mip: {all_mips}"
    );
    let mut ranges: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    for (_, c) in &affected {
        *ranges.entry(c.range).or_default() += 1;
    }
    eprintln!("affected files by (min, max) fourth component: {ranges:?}");
    for (rel, c) in &affected {
        eprintln!(
            "  {rel}: nonzero mips {:?} of {}, fourth range {:?}",
            c.nonzero_mips, c.mips, c.range
        );
    }
    assert!(undecodable.is_empty(), "undecodable mips: {undecodable:?}");
}
