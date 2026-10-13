//! Exports packed sectors for comparison with the original Storm decompressor.
use pcx_mpq::{MpqArchive, crypt, payload};
use std::{fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: export_audio_sectors <archive> <member> <directory>".into());
    }
    let mut archive = MpqArchive::open(&args[0])?;
    let block = *archive.find_block(&args[1]).ok_or("member missing")?;
    let raw = archive.read_block_raw(&block)?;
    if block.is_single_unit() || !block.is_compressed() {
        return Err("expected a sector-compressed audio member".into());
    }
    let size = archive.header.sector_size();
    let count = payload::sector_count(block.file_size, size);
    let table_size = (count + 1) * 4;
    let mut table = raw
        .get(..table_size)
        .ok_or("truncated offset table")?
        .to_vec();
    let key = payload::file_key(&args[1], block.file_pos, block.file_size, block.flags);
    if block.is_encrypted() {
        crypt::decrypt(&mut table, key.wrapping_sub(1), crypt::table());
    }
    let offsets: Vec<_> = table
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b) as usize)
        .collect();
    let directory = PathBuf::from(&args[2]);
    fs::create_dir_all(&directory)?;
    let mut manifest = format!("{} {}\n", block.file_size, count);
    for i in 0..count {
        if offsets[i] < table_size || offsets[i] > offsets[i + 1] {
            return Err("invalid sector offsets".into());
        }
        let mut sector = raw
            .get(offsets[i]..offsets[i + 1])
            .ok_or("sector outside member")?
            .to_vec();
        if block.is_encrypted() {
            crypt::decrypt(&mut sector, key.wrapping_add(i as u32), crypt::table());
        }
        let expected = (block.file_size - (i as u32) * size).min(size);
        fs::write(directory.join(format!("{i}.bin")), sector)?;
        manifest.push_str(&format!("{i} {expected}\n"));
    }
    fs::write(directory.join("manifest.txt"), manifest)?;
    println!(
        "{}: {} decoded bytes, {count} sectors, flags {:#x}",
        args[1], block.file_size, block.flags
    );
    Ok(())
}
