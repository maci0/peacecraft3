use pcx_mpq::{MpqArchive, crypt, flags, hash, huffman::HuffmanTable};
use std::io::Write;
pub fn encrypt(words: &mut [u8], mut key: u32) {
    let mut seed = 0xeeee_eeeeu32;
    for word in words.as_chunks_mut::<4>().0 {
        seed = seed.wrapping_add(crypt::table()[0x400 + (key & 0xff) as usize]);
        let plain = u32::from_le_bytes(*word);
        *word = (plain ^ key.wrapping_add(seed)).to_le_bytes();
        key = ((!key) << 21).wrapping_add(0x1111_1111) | (key >> 11);
        seed = plain
            .wrapping_add(seed)
            .wrapping_add(seed << 5)
            .wrapping_add(3);
    }
}
pub fn block(position: u32, packed: u32, declared: u32, extra: u32) -> pcx_mpq::BlockEntry {
    pcx_mpq::BlockEntry {
        file_pos: position,
        compressed_size: packed,
        file_size: declared,
        flags: extra,
    }
}
pub fn raw(
    member: &str,
    data: &[u8],
    block: pcx_mpq::BlockEntry,
    locales: &[u16],
    index: u32,
) -> Vec<u8> {
    let hash_at = 40 + data.len();
    let block_at = hash_at + locales.len() * 16;
    let mut bytes = vec![0; block_at + 16];
    bytes[..4].copy_from_slice(b"MPQ\x1a");
    for (at, value) in [
        (4, 32),
        (8, bytes.len() as u32),
        (12, 3 << 16),
        (16, hash_at as u32),
        (20, block_at as u32),
        (24, locales.len() as u32),
        (28, 1),
    ] {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[40..hash_at].copy_from_slice(data);
    let name = hash::hash_string(member);
    for (at, value) in [
        (block_at, block.file_pos),
        (block_at + 4, block.compressed_size),
        (block_at + 8, block.file_size),
        (block_at + 12, flags::EXISTING | block.flags),
    ] {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (i, locale) in locales.iter().enumerate() {
        let at = hash_at + i * 16;
        bytes[at..at + 4].copy_from_slice(&name.name1.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&name.name2.to_le_bytes());
        bytes[at + 8..at + 10].copy_from_slice(&locale.to_le_bytes());
        bytes[at + 12..at + 16].copy_from_slice(&index.to_le_bytes());
    }
    encrypt(&mut bytes[hash_at..block_at], hash::hash_table_key());
    encrypt(&mut bytes[block_at..], hash::block_table_key());
    bytes
}
pub fn archive(member: &str, data: &[u8], declared: u32, extra: u32) -> MpqArchive {
    MpqArchive::from_bytes(
        raw(
            member,
            data,
            block(40, data.len() as u32, declared, extra),
            &[0],
            0,
        ),
        "caller-owned synthetic.mpq",
    )
    .unwrap()
}
pub fn zlib(data: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(data).unwrap();
    let mut packed = vec![2];
    packed.extend(enc.finish().unwrap());
    packed
}
pub fn synthetic_pe() -> Vec<u8> {
    let mut bytes = vec![0; 0x220 + 258];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    bytes[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    bytes[0x86..0x88].copy_from_slice(&2u16.to_le_bytes());
    bytes[0x94..0x96].copy_from_slice(&0xe0u16.to_le_bytes());
    bytes[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    bytes[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
    for (at, rva, offset, size, virtual_size, flags) in [
        (0x178, 0x1000u32, 0x200u32, 0x20u32, 0x20u32, 0x20u32),
        (0x1a0, 0x2000, 0x220, 258, 258 * 256, 0),
    ] {
        for (delta, value) in [
            (8, virtual_size),
            (12, rva),
            (16, size),
            (20, offset),
            (36, flags),
        ] {
            bytes[at + delta..at + delta + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    bytes[0x200..0x202].copy_from_slice(&[0x69, 0xc0]);
    bytes[0x202..0x206].copy_from_slice(&258u32.to_le_bytes());
    bytes[0x206..0x208].copy_from_slice(&[0x8d, 0x88]);
    bytes[0x208..0x20c].copy_from_slice(&0x402000u32.to_le_bytes());
    bytes
}
pub fn table() -> HuffmanTable {
    HuffmanTable::from_pe(&synthetic_pe(), "caller synthetic weights.dll").unwrap()
}
