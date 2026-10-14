const MPQ: usize = 0x200;
fn encrypt(bytes: &mut [u8], mut key: u32) {
    let mut seed = 0xEEEE_EEEEu32;
    for word in bytes.as_chunks_mut::<4>().0 {
        seed = seed.wrapping_add(pcx_mpq::crypt::table()[0x400 + (key & 0xff) as usize]);
        let plain = u32::from_le_bytes(*word);
        *word = (plain ^ key.wrapping_add(seed)).to_le_bytes();
        key = ((!key) << 21).wrapping_add(0x1111_1111) | (key >> 11);
        seed = plain
            .wrapping_add(seed)
            .wrapping_add(seed << 5)
            .wrapping_add(3);
    }
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn fixture(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = vec![0; 0x40];
    let mut blocks = Vec::new();
    let mut hashes = vec![[u32::MAX, u32::MAX, 0, u32::MAX]; 16];
    for (index, (name, bytes)) in files.iter().enumerate() {
        blocks.push([
            archive.len() as u32,
            bytes.len() as u32,
            bytes.len() as u32,
            pcx_mpq::flags::EXISTING | pcx_mpq::flags::SINGLE_UNIT,
        ]);
        archive.extend_from_slice(bytes);
        let mut slot = pcx_mpq::hash::start_slot(name, hashes.len());
        while hashes[slot][3] != u32::MAX {
            slot = (slot + 1) % hashes.len();
        }
        let (name1, name2) = pcx_mpq::hash::hash_string_both(name);
        hashes[slot] = [name1, name2, 0, index as u32];
    }
    let hash_pos = archive.len() as u32;
    let mut hash_bytes: Vec<u8> = hashes
        .iter()
        .flat_map(|row| row.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    encrypt(&mut hash_bytes, pcx_mpq::hash::hash_table_key());
    archive.extend(hash_bytes);
    let block_pos = archive.len() as u32;
    let mut block_bytes: Vec<u8> = blocks
        .iter()
        .flat_map(|row| row.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    encrypt(&mut block_bytes, pcx_mpq::hash::block_table_key());
    archive.extend(block_bytes);
    archive[..4].copy_from_slice(b"MPQ\x1a");
    for (at, value) in [
        (4, 0x20),
        (8, archive.len() as u32),
        (0x10, hash_pos),
        (0x14, block_pos),
        (0x18, hashes.len() as u32),
        (0x1c, blocks.len() as u32),
    ] {
        archive[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    archive[0x0e] = 3;
    let mut map = b"HM3W\0\0\0\0Synthetic\0".to_vec();
    map.extend_from_slice(&1u32.to_le_bytes());
    map.extend_from_slice(&1u32.to_le_bytes());
    map.resize(MPQ, 0);
    map.extend(archive);
    map
}

fn table_mut(bytes: &mut [u8], hash: bool, mutate: impl FnOnce(&mut [u8])) {
    let (pos_field, count_field, key) = if hash {
        (0x10, 0x18, pcx_mpq::hash::hash_table_key())
    } else {
        (0x14, 0x1c, pcx_mpq::hash::block_table_key())
    };
    let at = MPQ + word(bytes, MPQ + pos_field) as usize;
    let len = word(bytes, MPQ + count_field) as usize * 16;
    let table = &mut bytes[at..at + len];
    pcx_mpq::crypt::decrypt(table, key, pcx_mpq::crypt::table());
    mutate(table);
    encrypt(table, key);
}

