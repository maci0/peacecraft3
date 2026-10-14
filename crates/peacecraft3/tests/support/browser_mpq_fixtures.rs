//! Synthetic encrypted-table archive. Framing follows retained fixtures.rs:53-160.
pub fn archive(
    payload: &[u8],
    size: u32,
    flags: u32,
    block_index: u32,
    name: &str,
) -> pcx_mpq::MpqArchive {
    archive_member(payload, size, flags, block_index, name, "fixture")
}
pub fn archive_member(
    payload: &[u8],
    size: u32,
    flags: u32,
    block_index: u32,
    name: &str,
    member: &str,
) -> pcx_mpq::MpqArchive {
    fn encrypt(data: &mut [u8], key: u32) {
        let table = pcx_mpq::crypt::table();
        let mut seed1 = key;
        let mut seed2 = 0xeeee_eeeeu32;
        for chunk in data.as_chunks_mut::<4>().0 {
            seed2 = seed2.wrapping_add(table[0x400 + (seed1 & 0xff) as usize]);
            let word = u32::from_le_bytes(*chunk);
            chunk.copy_from_slice(&(word ^ seed1.wrapping_add(seed2)).to_le_bytes());
            seed1 = ((!seed1) << 0x15).wrapping_add(0x1111_1111) | (seed1 >> 0x0b);
            seed2 = word
                .wrapping_add(seed2)
                .wrapping_add(seed2 << 5)
                .wrapping_add(3);
        }
    }
    let hash_pos = 32 + payload.len() as u32;
    let block_pos = hash_pos + 16;
    let mut bytes = b"MPQ\x1a".to_vec();
    bytes.extend(32u32.to_le_bytes());
    bytes.extend((block_pos + 16).to_le_bytes());
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(3u16.to_le_bytes());
    for word in [hash_pos, block_pos, 1, 1] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(payload);
    let (name1, name2) = pcx_mpq::hash::hash_string_both(member);
    let mut hash = Vec::new();
    for word in [name1, name2, 0, block_index] {
        hash.extend(word.to_le_bytes());
    }
    encrypt(&mut hash, pcx_mpq::hash::hash_table_key());
    bytes.extend(hash);
    let mut block = Vec::new();
    for word in [32, payload.len() as u32, size, flags] {
        block.extend(word.to_le_bytes());
    }
    encrypt(&mut block, pcx_mpq::hash::block_table_key());
    bytes.extend(block);
    pcx_mpq::MpqArchive::from_bytes(bytes, name).unwrap()
}
pub const FLAGS: u32 = pcx_mpq::flags::EXISTING | pcx_mpq::flags::SINGLE_UNIT;
