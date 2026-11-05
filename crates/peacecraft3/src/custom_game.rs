//! Custom Game map discovery for the Skirmish `CustomPanel`.
//!
//! Caller supplies a mounted [`pcx_assets::Resolver`] and explicit budgets:
//! `paths()` inventories candidates without reading them, `resolve_bounded`
//! reads each `.w3m`/`.w3x` entry under a per-map byte limit, and discovery
//! stops after `max_maps` candidates. Titles come only from
//! [`pcx_map::Map::info`] (`war3map.w3i` inside the map); an empty authored
//! name stays empty, never a filename-derived substitute.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct SkirmishMap {
    /// Asset path as inventoried by the resolver (lowercase, slash-separated).
    pub asset_path: String,
    /// Authored `war3map.w3i` name; may be empty if the map ships it empty.
    pub name: String,
    pub slots: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Discovery {
    pub maps: Vec<SkirmishMap>,
    /// Candidates examined that failed parse/info with a source-context
    /// error. Bounded by `examined`; budget truncation never adds records.
    pub skipped: Vec<DiscoverySkip>,
    /// `.w3m`/`.w3x` candidates actually visited (<= budget.max_maps).
    pub examined: usize,
    /// Candidates passed to `resolve_bounded` (<= examined).
    pub read: usize,
    /// Maps stored in `maps` (<= read).
    pub stored: usize,
    /// More candidates remained, or traversal stopped at the inventory
    /// budget before the inventory ended.
    pub truncated: bool,
    /// Candidates left unexamined because the budget ran out. Exact only
    /// when the whole inventory was visited; otherwise `None` (unknown
    /// remainder — counting it would require an O(N) full scan).
    pub unexamined: Option<usize>,
    /// Inventory entries visited, including non-map filler skipped before
    /// the candidate filter. This is the traversal work actually done,
    /// bounded by `inventory_budget` (see [`DiscoveryBudget`]).
    pub inventory_visited: usize,
    /// True when traversal stopped at the inventory budget with entries
    /// whose candidate-ness was never determined.
    pub inventory_truncated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoverySkip {
    pub asset_path: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveryBudget {
    /// Maximum candidates examined; bounds read/parse work.
    pub max_maps: usize,
    /// Maximum inventory entries *visited* (including non-map filler
    /// skipped by the extension filter); bounds traversal work. Discovery
    /// stops with truncated/unknown remainder rather than scanning the
    /// whole inventory to produce an exact leftover count.
    pub max_inventory: usize,
    /// Per-map byte limit passed to `Resolver::resolve_bounded`.
    pub max_bytes_per_map: u64,
}

impl DiscoveryBudget {
    /// Refuses a zero collection budget: discovery with no candidates
    /// examined is a caller bug, not an empty result.
    pub fn new(max_maps: usize, max_bytes_per_map: u64) -> Result<Self, Error> {
        Self::with_inventory(max_maps, usize::MAX, max_bytes_per_map)
    }

    /// Like [`Self::new`] but also bounds inventory traversal: at most
    /// `max_inventory` entries are visited. [`usize::MAX`] means no
    /// traversal cap beyond the candidate budget.
    pub fn with_inventory(
        max_maps: usize,
        max_inventory: usize,
        max_bytes_per_map: u64,
    ) -> Result<Self, Error> {
        if max_maps == 0 {
            return Err(Error::InvalidBudget {
                field: "max_maps",
                value: 0,
            });
        }
        if max_inventory == 0 {
            return Err(Error::InvalidBudget {
                field: "max_inventory",
                value: 0,
            });
        }
        if max_bytes_per_map == 0 {
            return Err(Error::InvalidBudget {
                field: "max_bytes_per_map",
                value: 0,
            });
        }
        Ok(Self {
            max_maps,
            max_inventory,
            max_bytes_per_map,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidBudget { field: &'static str, value: u64 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBudget { field, value } => {
                write!(f, "discovery budget {field} must be > 0, got {value}")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Inventories `.w3m`/`.w3x` paths lazily from `resolver.paths()` (already
/// sorted/deduped in the resolver's `BTreeSet`), reads each candidate under
/// `budget.max_bytes_per_map` via `resolve_bounded`, and parses it with
/// `pcx_map::Map::parse` + checked `info()`. Work is strictly bounded:
/// at most `budget.max_inventory` inventory entries are *visited*
/// (`inventory_visited`, including non-map filler skipped before the
/// extension filter) and at most `budget.max_maps` candidates are
/// examined/read/stored. Traversal stops at the inventory budget without
/// scanning the rest: the leftover is then truncated with
/// `unexamined = None` (unknown remainder) rather than an exact count
/// that would require an O(N) full scan. When the candidate budget runs
/// out first, traversal continues only to count the leftover candidates
/// when no inventory cap forces an early stop; in that case
/// `unexamined` is the exact remaining-candidate count. Titles come only
/// from the map's `war3map.w3i`. Entries in `maps/` are custom
/// candidates; other directories are only inventoried if the resolver
/// has them mounted, never guessed from an install directory.
pub fn discover_custom_maps(
    resolver: &pcx_assets::Resolver,
    budget: &DiscoveryBudget,
) -> Discovery {
    let mut maps = Vec::new();
    let mut skipped = Vec::new();
    let mut examined = 0usize;
    let mut read = 0usize;
    let mut truncated = false;
    let mut unexamined: Option<usize> = Some(0);
    let mut inventory_visited = 0usize;
    let mut inventory_truncated = false;
    let mut paths = resolver.paths();

    for path in &mut paths {
        if inventory_visited >= budget.max_inventory {
            inventory_truncated = true;
            truncated = true;
            unexamined = None;
            break;
        }
        inventory_visited += 1;
        let asset_path = path.as_str();
        if !(asset_path.ends_with(".w3m") || asset_path.ends_with(".w3x")) {
            continue;
        }
        if examined >= budget.max_maps {
            truncated = true;
            if let Some(n) = unexamined.as_mut() {
                *n += 1;
            }
            continue;
        }
        examined += 1;
        read += 1;
        match inspect_asset(resolver, asset_path, budget.max_bytes_per_map) {
            Ok(m) => maps.push(m),
            Err(e) => skipped.push(DiscoverySkip {
                asset_path: asset_path.to_owned(),
                error: e,
            }),
        }
    }
    let stored = maps.len();
    Discovery {
        maps,
        skipped,
        examined,
        read,
        stored,
        truncated,
        unexamined,
        inventory_visited,
        inventory_truncated,
    }
}

pub fn inspect_asset(
    resolver: &pcx_assets::Resolver,
    asset_path: &str,
    max_bytes: u64,
) -> Result<SkirmishMap, String> {
    let resolved = resolver
        .resolve_bounded(asset_path, max_bytes)
        .map_err(|e| format!("{asset_path}: {e}"))?;
    inspect_bytes(resolved.path.as_str(), &resolved.bytes)
}

pub fn inspect_bytes(asset_path: &str, bytes: &[u8]) -> Result<SkirmishMap, String> {
    let map = pcx_map::Map::parse(bytes)
        .map_err(|e| format!("{asset_path}: map parse: {e:?}"))?;
    let info = map
        .info()
        .map_err(|e| format!("{asset_path}: war3map.w3i: {e:?}"))?;
    Ok(SkirmishMap {
        asset_path: asset_path.to_owned(),
        name: info.name.clone(),
        slots: info.players().len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode, OverrideDirectory, Resolver};
    use std::path::PathBuf;

    /// Synthetic `war3map.w3i` version 18 with empty player/force/table lists,
    /// built from the field order in `pcx-map/src/info.rs`. Str fields carry
    /// `name` / `""`; no title is fabricated when `name` is empty.
    fn w3i_bytes(name: &str) -> Vec<u8> {
        fn u32(v: &mut Vec<u8>, x: u32) {
            v.extend_from_slice(&x.to_le_bytes());
        }
        fn s(v: &mut Vec<u8>, t: &str) {
            v.extend_from_slice(t.as_bytes());
            v.push(0);
        }
        let mut v = Vec::new();
        u32(&mut v, 18); // version
        u32(&mut v, 0); // constant_a
        u32(&mut v, 0); // constant_b (version >= 0x10)
        s(&mut v, name);
        s(&mut v, "");
        s(&mut v, "");
        s(&mut v, ""); // head_string_4 (version >= 8)
        for _ in 0..8 {
            u32(&mut v, 0); // camera bounds
        }
        for _ in 0..4 {
            u32(&mut v, 0); // tileset constants
        }
        u32(&mut v, 0); // bounds[0]
        u32(&mut v, 0); // bounds[1]
        u32(&mut v, 0); // flags
        v.push(0); // byte_field
        u32(&mut v, 0); // extra_u32
        // first_string gated at 0x14: absent at 18
        s(&mut v, ""); // fog_string
        s(&mut v, ""); // second_string (version >= 0x0b)
        s(&mut v, ""); // third_string
        u32(&mut v, 0); // loading_id (version >= 0x11); loading_path gated at 0x14: absent
        s(&mut v, ""); // loading_strings[0] (version >= 0x0d)
        s(&mut v, ""); // [1]
        s(&mut v, ""); // [2]
        // fog_style / float_triple gated at 0x13=19: absent at 18
        // extra_u32_16 gated at 0x15=21, extra_string_17 at 0x16=22,
        // extra_byte_18 at 0x17=23, extra_u32_19 at 0x19=25: all absent.
        u32(&mut v, 0); // player count
        u32(&mut v, 0); // force count (version >= 3)
        u32(&mut v, 0); // start locations (version >= 6)
        u32(&mut v, 0); // pairs (version >= 7)
        u32(&mut v, 0); // table lists (version >= 0x0c)
        // item tables gated at version >= 0x18 (24): absent at 18 (note: 0x18=24,
        // not the version word 18) — parser checks `version >= 0x18`, so a
        // version-18 file carries no item-table count.
        v
    }

    fn encrypt_mpq_block(data: &mut [u8], key: u32) {
        let table = pcx_mpq::crypt::table();
        let mut seed1 = key;
        let mut seed2 = 0xEEEE_EEEEu32;
        for chunk in data.as_chunks_mut::<4>().0 {
            seed2 = seed2.wrapping_add(table[0x400 + (seed1 & 0xFF) as usize]);
            let plaintext = u32::from_le_bytes(*chunk);
            let ciphertext = plaintext ^ seed1.wrapping_add(seed2);
            chunk.copy_from_slice(&ciphertext.to_le_bytes());
            seed1 = ((!seed1) << 0x15).wrapping_add(0x1111_1111) | (seed1 >> 0x0B);
            seed2 = plaintext
                .wrapping_add(seed2)
                .wrapping_add(seed2 << 5)
                .wrapping_add(3);
        }
    }

    /// Minimal synthetic HM3W map wrapping a one-member MPQ holding `war3map.w3i`.
    fn synthetic_w3m(w3i: &[u8]) -> Vec<u8> {
        const HASH_ENTRIES: usize = 16;
        let files: &[(&str, &[u8])] = &[("war3map.w3i", w3i)];
        let mut payloads = Vec::new();
        let mut block_entries = Vec::new();
        let payload_base_offset = 0x40usize;
        for (_name, data) in files {
            let file_pos = payload_base_offset + payloads.len();
            let file_size = data.len() as u32;
            let flags: u32 = 0x8000_0000 | 0x0100_0000;
            block_entries.push((file_pos as u32, file_size, file_size, flags));
            payloads.extend_from_slice(data);
            while payloads.len() % 4 != 0 {
                payloads.push(0);
            }
        }
        let hash_table_pos = (payload_base_offset + payloads.len()) as u32;
        let mut hash_entries = vec![
            pcx_mpq::HashEntry {
                name1: 0xFFFF_FFFF,
                name2: 0xFFFF_FFFF,
                locale: 0,
                platform: 0,
                block_index: 0xFFFF_FFFF,
            };
            HASH_ENTRIES
        ];
        for (i, (name, _)) in files.iter().enumerate() {
            let (name1, name2) = pcx_mpq::hash::hash_string_both(name);
            let mut slot = pcx_mpq::hash::start_slot(name, HASH_ENTRIES);
            while hash_entries[slot].block_index != 0xFFFF_FFFF {
                slot = (slot + 1) % HASH_ENTRIES;
            }
            hash_entries[slot].name1 = name1;
            hash_entries[slot].name2 = name2;
            hash_entries[slot].block_index = i as u32;
        }
        let mut hash_table_raw = Vec::with_capacity(HASH_ENTRIES * 16);
        for entry in &hash_entries {
            hash_table_raw.extend_from_slice(&entry.name1.to_le_bytes());
            hash_table_raw.extend_from_slice(&entry.name2.to_le_bytes());
            hash_table_raw.extend_from_slice(&entry.locale.to_le_bytes());
            hash_table_raw.extend_from_slice(&entry.platform.to_le_bytes());
            hash_table_raw.extend_from_slice(&entry.block_index.to_le_bytes());
        }
        encrypt_mpq_block(&mut hash_table_raw, pcx_mpq::hash::hash_table_key());
        let block_table_pos = hash_table_pos + (HASH_ENTRIES as u32 * 16);
        let mut block_table_raw = Vec::with_capacity(block_entries.len() * 16);
        for (pos, comp, size, flags) in &block_entries {
            block_table_raw.extend_from_slice(&pos.to_le_bytes());
            block_table_raw.extend_from_slice(&comp.to_le_bytes());
            block_table_raw.extend_from_slice(&size.to_le_bytes());
            block_table_raw.extend_from_slice(&(*flags).to_le_bytes());
        }
        encrypt_mpq_block(&mut block_table_raw, pcx_mpq::hash::block_table_key());
        let archive_size = block_table_pos + (block_entries.len() as u32 * 16);
        let mut mpq = Vec::new();
        mpq.extend_from_slice(b"MPQ\x1a");
        mpq.extend_from_slice(&0x20u32.to_le_bytes());
        mpq.extend_from_slice(&archive_size.to_le_bytes());
        mpq.extend_from_slice(&0u16.to_le_bytes());
        mpq.push(3u8);
        mpq.push(0u8);
        mpq.extend_from_slice(&hash_table_pos.to_le_bytes());
        mpq.extend_from_slice(&block_table_pos.to_le_bytes());
        mpq.extend_from_slice(&(HASH_ENTRIES as u32).to_le_bytes());
        mpq.extend_from_slice(&(files.len() as u32).to_le_bytes());
        mpq.extend_from_slice(&0u32.to_le_bytes());
        mpq.extend_from_slice(&0u32.to_le_bytes());
        while mpq.len() < payload_base_offset {
            mpq.push(0);
        }
        mpq.extend_from_slice(&payloads);
        mpq.extend_from_slice(&hash_table_raw);
        mpq.extend_from_slice(&block_table_raw);
        assert_eq!(mpq.len(), archive_size as usize);
        let mut map = Vec::new();
        map.extend_from_slice(b"HM3W");
        map.extend_from_slice(&0u32.to_le_bytes());
        map.extend_from_slice(b"Synthetic\0");
        map.extend_from_slice(&1u32.to_le_bytes());
        map.extend_from_slice(&1u32.to_le_bytes());
        while map.len() < 0x200 {
            map.push(0);
        }
        map.extend_from_slice(&mpq);
        map
    }

    struct Fixture(PathBuf);
    impl Fixture {
        fn new(name: &str, files: &[(&str, Vec<u8>)]) -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../.scratch/custom-game-fixtures")
                .join(format!("{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            for (path, bytes) in files {
                let at = root.join(path.replace('\\', "/"));
                std::fs::create_dir_all(at.parent().expect("file has a parent")).unwrap();
                std::fs::write(&at, bytes).unwrap();
            }
            Self(root)
        }
        fn resolver(&self) -> Resolver {
            let mode = GameMode {
                build: Build::V1_29b,
                addon: Addon::Tft,
                assets: AssetMode::Vanilla,
                behaviour: Behaviour::Vanilla,
            };
            Resolver::new(
                mode,
                Some(OverrideDirectory::open(&self.0, "custom-game-test", None).unwrap()),
                Vec::new(),
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn discovers_title_from_w3i_and_refuses_non_maps() {
        let good = synthetic_w3m(&w3i_bytes("Synthetic Title"));
        let fixture = Fixture::new(
            "titles",
            &[
                (r"Maps\Scenario\(2)Synthetic.w3m", good),
                (r"Maps\Scenario\not-a-map.w3m", b"XXXXnot a map".to_vec()),
                (r"Maps\Scenario\notes.txt", b"hello".to_vec()),
            ],
        );
        let resolver = fixture.resolver();
        let budget = DiscoveryBudget::new(10, 1 << 20).unwrap();
        let d = discover_custom_maps(&resolver, &budget);
        assert_eq!(d.maps.len(), 1, "maps={:?} skipped={:?}", d.maps, d.skipped);
        assert_eq!(d.maps[0].name, "Synthetic Title");
        assert_eq!(d.maps[0].asset_path, "maps/scenario/(2)synthetic.w3m");
        assert_eq!(d.maps[0].slots, 0);
        assert!(
            d.skipped.iter().any(|s| s.asset_path.contains("not-a-map")),
            "non-map must be a typed skip, not silently dropped: {d:?}"
        );
        // Empty authored title stays empty — no filename fabrication.
        let empty = inspect_bytes("maps/scenario/x.w3m", &synthetic_w3m(&w3i_bytes(""))).unwrap();
        assert_eq!(empty.name, "");
    }

    #[test]
    fn budget_refuses_zero_limits() {
        assert!(DiscoveryBudget::new(0, 1).is_err());
        assert!(DiscoveryBudget::new(1, 0).is_err());
    }

    #[test]
    fn over_budget_candidates_are_skipped_not_read() {
        let m = synthetic_w3m(&w3i_bytes("T"));
        let fixture = Fixture::new(
            "budget",
            &[
                (r"Maps\a.w3m", m.clone()),
                (r"Maps\b.w3m", m),
            ],
        );
        let resolver = fixture.resolver();
        let budget = DiscoveryBudget::new(1, 1 << 20).unwrap();
        let d = discover_custom_maps(&resolver, &budget);
        assert_eq!(d.maps.len(), 1);
        assert!(d.truncated, "leftover inventory must be explicit truncation");
        assert_eq!(d.unexamined, Some(1));
        assert_eq!(d.examined, 1);
        assert_eq!(d.inventory_visited, 2, "every entry visited is counted: {d:?}");
        assert!(!d.inventory_truncated);
        assert_eq!(d.skipped.len(), 0, "truncation must not emit per-entry skips");
    }

    #[test]
    fn large_inventory_work_is_bounded_by_deterministic_counts() {
        // 5000 non-map entries + 3 maps, budget 2: bounded work must be
        // counted, never elapsed-time dependent. Resolver paths are a
        // BTreeSet (sorted), so the first 2 examined are deterministic.
        let m = synthetic_w3m(&w3i_bytes("T"));
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for i in 0..5000 {
            files.push((format!(r"Maps\filler\z{i:05}.txt"), b"x".to_vec()));
        }
        files.push((r"Maps\a.w3m".into(), m.clone()));
        files.push((r"Maps\b.w3m".into(), m.clone()));
        files.push((r"Maps\c.w3m".into(), m));
        let refs: Vec<(&str, Vec<u8>)> = files.iter().map(|(p, b)| (p.as_str(), b.clone())).collect();
        let fixture = Fixture::new("large", &refs);
        let resolver = fixture.resolver();
        let budget = DiscoveryBudget::new(2, 1 << 20).unwrap();
        let d = discover_custom_maps(&resolver, &budget);
        // Deterministic: sorted inventory — maps/a.w3m, maps/b.w3m examined;
        // maps/c.w3m never examined (truncated, not per-entry skipped).
        assert_eq!(d.examined, 2, "examined must be bounded by max_maps: {d:?}");
        assert_eq!(d.read, 2, "read must be bounded by max_maps: {d:?}");
        assert_eq!(d.stored, 2, "stored must be bounded by max_maps: {d:?}");
        assert_eq!(d.maps.len(), 2);
        assert_eq!(d.skipped.len(), 0, "truncation must not emit per-entry skip records: {d:?}");
        assert!(d.truncated, "remaining inventory beyond budget must be an explicit outcome");
        assert_eq!(d.unexamined, Some(1), "maps/c.w3m unexamined: {d:?}");
        // The 5000 filler entries were *visited* (counted as inventory
        // work), never read — traversal work is visible, not hidden.
        assert_eq!(
            d.inventory_visited, 5003,
            "inventory_visited must count non-map filler too: {d:?}"
        );
        // Parse-failure skips stay bounded by examined count too.
        let budget2 = DiscoveryBudget::new(10, 1 << 20).unwrap();
        let d2 = discover_custom_maps(&resolver, &budget2);
        assert_eq!(d2.examined, 3);
        assert_eq!(d2.read, 3);
        assert_eq!(d2.stored, 3);
        assert!(!d2.truncated);
        assert_eq!(d2.unexamined, Some(0));
        assert_eq!(d2.inventory_visited, 5003);
    }

    #[test]
    fn inventory_budget_stops_traversal_with_unknown_remainder() {
        // Sorted order: maps/a.w3m, maps/b.w3m, then 5000 filler .txt
        // entries. An inventory cap of 3 must stop traversal before the
        // filler, report inventory_visited == 3 (not a faked "read"
        // counter), and leave the remainder unknown instead of scanning
        // all 5003 entries for an exact count.
        let m = synthetic_w3m(&w3i_bytes("T"));
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for i in 0..5000 {
            files.push((format!(r"Maps\filler\z{i:05}.txt"), b"x".to_vec()));
        }
        files.push((r"Maps\a.w3m".into(), m.clone()));
        files.push((r"Maps\b.w3m".into(), m));
        let refs: Vec<(&str, Vec<u8>)> = files.iter().map(|(p, b)| (p.as_str(), b.clone())).collect();
        let fixture = Fixture::new("invbudget", &refs);
        let resolver = fixture.resolver();
        let budget = DiscoveryBudget::with_inventory(10, 3, 1 << 20).unwrap();
        let d = discover_custom_maps(&resolver, &budget);
        assert_eq!(d.inventory_visited, 3, "traversal must stop at the inventory budget: {d:?}");
        assert_eq!(d.examined, 2, "only maps/a.w3m and maps/b.w3m examined: {d:?}");
        assert!(d.truncated);
        assert!(d.inventory_truncated);
        assert_eq!(
            d.unexamined, None,
            "remainder must be unknown, not an exact count requiring a full scan: {d:?}"
        );
        assert_eq!(d.maps.len(), 2);
    }
}
