const FLAGS: u32 = pcx_mpq::flags::EXISTING | pcx_mpq::flags::SINGLE_UNIT;
/// Matching hash row behind an empty initial probe slot; payload itself is valid.
pub fn unreachable_probe() -> pcx_mpq::MpqArchive {
    use crate::mpq_fixtures::encrypt;
    let mut bytes = b"MPQ\x1a".to_vec();
    for word in [32u32, 82] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(3u16.to_le_bytes());
    for word in [34u32, 66, 2, 1] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend([2, 3]);
    let empty = pcx_mpq::HashEntry {
        name1: u32::MAX,
        name2: u32::MAX,
        locale: 0,
        platform: 0,
        block_index: u32::MAX,
    };
    let mut entries = [empty; 2];
    let (name1, name2) = pcx_mpq::hash::hash_string_both("fixture");
    let initial = pcx_mpq::hash::start_slot("fixture", 2);
    entries[1 - initial] = pcx_mpq::HashEntry {
        name1,
        name2,
        locale: 0,
        platform: 0,
        block_index: 0,
    };
    let mut hashes = Vec::new();
    for e in entries {
        hashes.extend(e.name1.to_le_bytes());
        hashes.extend(e.name2.to_le_bytes());
        hashes.extend(e.locale.to_le_bytes());
        hashes.extend(e.platform.to_le_bytes());
        hashes.extend(e.block_index.to_le_bytes());
    }
    encrypt(&mut hashes, pcx_mpq::hash::hash_table_key());
    bytes.extend(hashes);
    let mut block = Vec::new();
    for word in [32u32, 2, 2, FLAGS] {
        block.extend(word.to_le_bytes());
    }
    encrypt(&mut block, pcx_mpq::hash::block_table_key());
    bytes.extend(block);
    pcx_mpq::MpqArchive::from_bytes(bytes, "unreachable matching row").unwrap()
}
