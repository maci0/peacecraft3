//! MPQ encryption table and the table-decryption primitive.
//!
//! The archives store their hash and block tables as `entries * 16` bytes of
//! MPQ block cipher (dwords, little-endian), keyed by the filename hash of the
//! literal strings `(hash table)` and `(block table)`.
//!
//! Three independent sources agree on this:
//!
//! * Storm.dll `FUN_15017570` (`ev_703ee25fa60119017f501186a4e3477602692be9755a29492ee9964202b5d16e`)
//!   — the cipher, recovered from assembly.
//! * `FUN_150175E0` (`ev_ddc2808ad6bdeac104d96f2c1c2520a975ac38f0d36e884cb777f7b1ffcc4324`)
//!   — the filename hash that produces the two table keys.
//! * The archives themselves: applying this to the shipped files yields block
//!   tables whose entries are self-consistent for 98.9% (`war3.mpq`) to 99.3%
//!   (`War3x.mpq`) of slots, and hash tables whose 10681 of 32768 populated slots
//!   point inside the block table.
//!
//! Note on the Storm.dll dump: the 1280-dword table read out of that process at
//! `Storm.dll+0x4C2E4` does *not* equal the table these archives use — all 1280
//! words differ. The archives use the fixed table built here. See
//! `docs/re-notes/000-mpq.md` §"Runtime table discrepancy" for the measurement.

/// Number of 256-entry slices in the MPQ encryption table.
pub const TABLE_SLICES: usize = 5;

/// Total dwords in the MPQ encryption table: 5 slices of 256.
pub const TABLE_ENTRIES: usize = TABLE_SLICES * 256;

/// First index of the slice used by the block cipher.
const CIPHER_SLICE: usize = 0x400;

/// Build the fixed MPQ encryption table.
///
/// ```text
/// seed = 0x00100001
/// for slice in 0..5, for byte in 0..256:
///     seed = (seed * 125 + 3) % 0x2AAAAB
///     hi   = (seed & 0xFFFF) << 16
///     seed = (seed * 125 + 3) % 0x2AAAAB
///     table[slice * 256 + byte] = hi | (seed & 0xFFFF)
/// ```
pub fn build_table() -> [u32; TABLE_ENTRIES] {
    let mut table = [0u32; TABLE_ENTRIES];
    let mut seed: u32 = 0x0010_0001;
    for byte in 0..256usize {
        let mut index = byte;
        for _ in 0..TABLE_SLICES {
            seed = (seed * 125 + 3) % 0x002A_AAAB;
            let hi = (seed & 0xFFFF) << 16;
            seed = (seed * 125 + 3) % 0x002A_AAAB;
            table[index] = hi | (seed & 0xFFFF);
            index += 0x100;
        }
    }
    table
}

/// The fixed table, built once.
pub fn table() -> &'static [u32; TABLE_ENTRIES] {
    use std::sync::OnceLock;
    static T: OnceLock<[u32; TABLE_ENTRIES]> = OnceLock::new();
    T.get_or_init(build_table)
}

/// Decrypt `data` in place as an MPQ block.
///
/// `data` is processed in 4-byte units; a trailing partial unit is left alone.
///
/// ```text
/// seed1 = key
/// seed2 = 0xEEEEEEEE
/// per dword:
///     seed2 += TABLE[0x400 + (seed1 & 0xFF)]
///     value  = word ^ (seed1 + seed2)
///     seed1  = ((~seed1 << 21) + 0x11111111) | (seed1 >> 11)
///     seed2  = value + seed2 + (seed2 << 5) + 3
/// ```
pub fn decrypt(data: &mut [u8], key: u32, table: &[u32; TABLE_ENTRIES]) {
    let mut seed1 = key;
    let mut seed2 = 0xEEEE_EEEEu32;
    for chunk in data.chunks_exact_mut(4) {
        seed2 = seed2.wrapping_add(table[CIPHER_SLICE + (seed1 & 0xFF) as usize]);
        let word = u32::from_le_bytes(chunk.try_into().unwrap());
        let value = word ^ seed1.wrapping_add(seed2);
        chunk.copy_from_slice(&value.to_le_bytes());
        seed1 = ((!seed1) << 0x15).wrapping_add(0x1111_1111) | (seed1 >> 0x0B);
        seed2 = value
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
}

/// This primitive is **not** self-inverse. The `seed2` schedule advances using
/// the *decrypted* word, so running `decrypt` twice does not restore the input.
/// Writing an archive needs the separate inverse schedule; reading one does not.
/// The `fn decrypt_is_an_involution` test below pins that measured fact rather
/// than asserting the tempting-but-false symmetry.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_documented_head() {
        let t = build_table();
        assert_eq!(
            t[0], 0x55c6_36e2,
            "slice 0 word 0 must match the shipped-archive table"
        );
        assert_eq!(t[1], 0x02be_0170);
        assert_eq!(t[CIPHER_SLICE], 0x193a_a698);
        assert_eq!(t[CIPHER_SLICE + 1], 0x5496_f7d5);
    }

    #[test]
    fn slices_are_contiguous_and_distinct() {
        let t = build_table();
        let mut seen = std::collections::HashSet::new();
        for slice in 0..TABLE_SLICES {
            for byte in 0..256 {
                assert!(seen.insert(t[slice * 256 + byte]), "duplicate table word");
            }
        }
        assert_eq!(seen.len(), TABLE_ENTRIES);
    }

    #[test]
    fn decrypt_is_not_self_inverse() {
        // Measured against the shipped archives: the seed2 schedule folds in the
        // decrypted word, so two passes do not cancel. Guards against anyone
        // "simplifying" the loop back into an involution.
        let t = build_table();
        let key = 0xC3AF_3770u32;
        let buf: Vec<u8> = (0..64u32)
            .map(|i| (i.wrapping_mul(0x0100_0101)) as u8)
            .collect();
        let mut once = buf.clone();
        decrypt(&mut once, key, &t);
        let mut twice = once.clone();
        decrypt(&mut twice, key, &t);
        assert_ne!(buf, once, "first pass must transform the buffer");
        assert_ne!(once, twice, "the primitive must not be self-inverse");
    }

    #[test]
    fn decrypts_known_war3_block_table_head() {
        // Ground truth from the shipped archive: war3.mpq block entry 0 is
        // pos=0x20, compSize=0xC5B, size=0xAF46, flags=0x80000200.
        let Some(dir) = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("wc3/game/app/war3.mpq"))
            .filter(|p| p.is_file())
        else {
            eprintln!("skipping: no extracted install");
            return;
        };
        let t = build_table();
        let bytes = std::fs::read(dir).unwrap();
        let u32_at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let mut raw = bytes[u32_at(0x14) as usize..u32_at(0x14) as usize + 16].to_vec();
        decrypt(&mut raw, crate::hash::block_table_key(), &t);
        assert_eq!(
            u32::from_le_bytes(raw[0..4].try_into().unwrap()),
            0x20,
            "block 0 file position"
        );
        assert_eq!(
            u32::from_le_bytes(raw[4..8].try_into().unwrap()),
            0xC5B,
            "block 0 compressed size"
        );
        assert_eq!(
            u32::from_le_bytes(raw[8..12].try_into().unwrap()),
            0xAF46,
            "block 0 uncompressed size"
        );
        assert_eq!(
            u32::from_le_bytes(raw[12..16].try_into().unwrap()),
            0x8000_0200,
            "block 0 flags (EXISTS | COMPRESS)"
        );
    }

    #[test]
    fn trailing_partial_dword_is_untouched() {
        let t = build_table();
        let mut buf = vec![0u8; 9];
        buf[8] = 0xAB;
        decrypt(&mut buf, 1, &t);
        assert_eq!(buf[8], 0xAB);
    }
}
