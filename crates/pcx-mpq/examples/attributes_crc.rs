//! Compares each listed member's decoded bytes with the archive's `(attributes)` CRC32.
//! Usage: attributes_crc <archive> <member-suffix>. Prints counts per outcome; no acceptance.
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [archive_path, suffix] = args.as_slice() else {
        return Err("usage: attributes_crc <archive> <member-suffix>".into());
    };
    let mut archive = pcx_mpq::MpqArchive::open(archive_path)?;
    let attributes = archive.extract("(attributes)")?;
    let crcs: Vec<u32> = attributes
        .get(8..)
        .ok_or("attributes header is truncated")?
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| u32::from_le_bytes(*bytes))
        .collect();
    let listfile = archive.extract("(listfile)")?;
    let names = String::from_utf8_lossy(&listfile);
    let mut outcomes: BTreeMap<&str, u32> = BTreeMap::new();
    for name in names
        .lines()
        .filter(|name| name.to_ascii_lowercase().ends_with(suffix))
    {
        let Some(block) = archive.find_block(name) else {
            *outcomes.entry("missing").or_default() += 1;
            continue;
        };
        let index = archive
            .block_entries()
            .iter()
            .position(|entry| std::ptr::eq(entry, block))
            .ok_or("block outside the table")?;
        let outcome = match archive.extract(name) {
            Ok(bytes) => {
                let mut crc = flate2::Crc::new();
                crc.update(&bytes);
                if crcs.get(index) == Some(&crc.sum()) {
                    "crc match"
                } else {
                    "crc mismatch"
                }
            }
            Err(_) => "decode error",
        };
        *outcomes.entry(outcome).or_default() += 1;
    }
    for (outcome, count) in outcomes {
        println!("{outcome}: {count}");
    }
    Ok(())
}
