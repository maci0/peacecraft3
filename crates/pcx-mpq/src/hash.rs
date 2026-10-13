//! MPQ filename hashing.
//!
//! One routine, four seeds, exactly as Storm.dll implements it
//! (`FUN_150175E0`, evidence `ev_ddc2808ad6bdeac104d96f2c1c2520a975ac38f0d36e884cb777f7b1ffcc4324`):
//!
//! ```text
//! seed1 = 0x7FED7FED
//! seed2 = 0xEEEEEEEE
//! for each byte of the uppercased name:
//!     seed1 = TABLE[seed * 256 + byte] ^ (seed1 + seed2)
//!     seed2 = byte + seed1 + seed2 + (seed2 << 5) + 3
//! ```
//!
//! The seed selects which 256-entry slice of the encryption table is used:
//! `0` for the initial hash-table index, `1` for `name1`, `2` for `name2`, and
//! `3` for the table keys. Confirmed by the shipped archives: `hash_bytes(b"(hash table)", 3)`
//! and `hash_bytes(b"(block table)", 3)` produce exactly the keys that decrypt their
//! tables into self-consistent contents.

use crate::crypt::{TABLE_ENTRIES, table};

/// Seed selecting the slice used for the initial hash-table index.
pub const SEED_INDEX: u32 = 0;
/// Seed selecting the slice used for `name1`.
pub const SEED_NAME1: u32 = 1;
/// Seed selecting the slice used for `name2`.
pub const SEED_NAME2: u32 = 2;
/// Seed selecting the slice used for the table keys.
pub const SEED_TABLE_KEY: u32 = 3;

/// Hash `bytes` with the given table slice.
pub fn hash_bytes(bytes: &[u8], seed_slice: u32) -> u32 {
    let t = table();
    let base = ((seed_slice << 8) as usize) % TABLE_ENTRIES;
    let mut seed1: u32 = 0x7FED_7FED;
    let mut seed2: u32 = 0xEEEE_EEEE;
    for &b in bytes {
        let c = b.to_ascii_uppercase() as usize;
        seed1 = t[base + c] ^ seed1.wrapping_add(seed2);
        seed2 = (c as u32)
            .wrapping_add(seed1)
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
    seed1
}

/// The two name hashes a hash-table entry is keyed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HashResult {
    pub name1: u32,
    pub name2: u32,
}

/// Hash a path. MPQ names are ASCII and case-insensitive; the path separator is
/// normalised to `\` before hashing because that is what WC3 stores internally.
pub fn hash_string(name: &str) -> HashResult {
    let normalized = name.replace('/', "\\");
    let bytes = normalized.as_bytes();
    HashResult {
        name1: hash_bytes(bytes, SEED_NAME1),
        name2: hash_bytes(bytes, SEED_NAME2),
    }
}

/// The `(name1, name2)` a table entry must match for `name`.
pub fn hash_string_both(name: &str) -> (u32, u32) {
    let h = hash_string(name);
    (h.name1, h.name2)
}

/// The slot a lookup for `name` starts at, in a table of `entries` slots.
pub fn start_slot(name: &str, entries: usize) -> usize {
    let normalized = name.replace('/', "\\");
    (hash_bytes(normalized.as_bytes(), SEED_INDEX) as usize) % entries
}

/// Key used to decrypt the hash table.
pub fn hash_table_key() -> u32 {
    hash_bytes(b"(hash table)", SEED_TABLE_KEY)
}

/// Key used to decrypt the block table.
pub fn block_table_key() -> u32 {
    hash_bytes(b"(block table)", SEED_TABLE_KEY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_keys_are_stable() {
        // These are the keys that actually decrypt the four shipped archives'
        // tables into self-consistent contents, under the fixed table.
        assert_eq!(hash_table_key(), 0xC3AF_3770);
        assert_eq!(block_table_key(), 0xEC83_B3A3);
    }

    #[test]
    fn hashing_is_case_insensitive() {
        assert_eq!(hash_string("FOOTMAN.MDL"), hash_string("footman.mdl"));
    }

    #[test]
    fn separators_are_normalised() {
        assert_eq!(hash_string("a/b.mdl"), hash_string("a\\b.mdl"));
    }

    #[test]
    fn distinct_names_differ() {
        let a = hash_string("units\\human\\footman\\footman.mdl");
        let b = hash_string("units\\human\\herofootman\\herofootman.mdl");
        assert_ne!(a, b);
    }

    #[test]
    fn start_slot_is_in_range() {
        for n in [1usize, 0x4000, 0x8000] {
            let s = start_slot("units\\human\\footman\\footman.mdl", n);
            assert!(s < n);
        }
    }
}
