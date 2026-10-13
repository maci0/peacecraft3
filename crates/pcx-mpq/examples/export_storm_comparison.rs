//! Local comparison stream for tests/native/storm_compare.c; contains original bytes.
//! Write its output outside the repository. The native harness compares actual bytes.
use pcx_mpq::{MpqArchive, crypt, flags, payload};
use std::{fs::File, io::Write};

fn word(out: &mut impl Write, value: usize) -> std::io::Result<()> {
    out.write_all(
        &u32::try_from(value)
            .map_err(|_| std::io::Error::other("comparison record too large"))?
            .to_le_bytes(),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "usage: export_storm_comparison <archive> <candidate-names|-> <output-outside-repo>"
                .into(),
        );
    }
    let mut archive = MpqArchive::open(&args[0])?;
    let names = if args[1] == "-" {
        archive.extract("(listfile)")?
    } else {
        std::fs::read(&args[1])?
    };
    let names: std::collections::BTreeSet<_> = std::str::from_utf8(&names)?
        .split(['\r', '\n'])
        .filter(|name| !name.is_empty())
        .collect();
    let mut output = std::io::BufWriter::new(File::create(&args[2])?);
    output.write_all(b"PXC1")?;
    let mut records = 0;
    let mut refusals = std::collections::BTreeMap::<String, usize>::new();
    for name in names {
        let Some(block) = archive.find_block(name).copied() else {
            continue;
        };
        let plain = match archive.extract(name) {
            Ok(bytes) => bytes,
            Err(error) => {
                *refusals.entry(error.to_string()).or_default() += 1;
                continue;
            }
        };
        let raw = archive.read_block_raw(&block)?;
        let key = payload::file_key(name, block.file_pos, block.file_size, block.flags);
        let size = archive.header.sector_size();
        let count = if block.is_single_unit() {
            1
        } else {
            payload::sector_count(block.file_size, size)
        };
        let compressed = block.flags & (flags::COMPRESS | flags::IMPLODE) != 0;
        let offsets = if compressed && !block.is_single_unit() {
            let table_size = (count + 1) * 4;
            let mut bytes = raw
                .get(..table_size)
                .ok_or("truncated sector table")?
                .to_vec();
            if block.is_encrypted() {
                crypt::decrypt(&mut bytes, key.wrapping_sub(1), crypt::table());
            }
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| u32::from_le_bytes(*bytes) as usize)
                .collect::<Vec<_>>()
        } else if block.is_single_unit() {
            vec![0, raw.len()]
        } else {
            (0..=count)
                .map(|i| (i * size as usize).min(raw.len()))
                .collect()
        };
        word(&mut output, name.len())?;
        output.write_all(name.as_bytes())?;
        word(&mut output, plain.len())?;
        word(&mut output, count)?;
        output.write_all(&plain)?;
        for i in 0..count {
            let mut sector = raw
                .get(offsets[i]..offsets[i + 1])
                .ok_or("invalid sector range")?
                .to_vec();
            if block.is_encrypted() {
                crypt::decrypt(
                    &mut sector,
                    if block.is_single_unit() {
                        key
                    } else {
                        key.wrapping_add(i as u32)
                    },
                    crypt::table(),
                );
            }
            let want = if block.is_single_unit() {
                plain.len()
            } else {
                (plain.len() - i * size as usize).min(size as usize)
            };
            let kind = if !compressed || sector.len() == want {
                0
            } else if block.flags & flags::COMPRESS != 0 {
                1
            } else {
                2
            };
            word(&mut output, kind)?;
            word(&mut output, sector.len())?;
            word(&mut output, want)?;
            output.write_all(&sector)?;
        }
        records += 1;
    }
    output.flush()?;
    println!(
        "archive={} comparison_files={records} rust_refusals={refusals:?}",
        args[0]
    );
    Ok(())
}
