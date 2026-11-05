//! Custom Game map discovery for the Skirmish `CustomPanel`.
//!
//! Caller supplies a mounted [`pcx_assets::Resolver`] and explicit budgets:
//! `paths()` inventories candidates without reading them, `resolve_bounded`
//! reads each `.w3m`/`.w3x` entry under a per-map byte limit, and discovery
//! stops after `max_maps` candidates. Titles come only from
//! [`pcx_map::Map::info`] (`war3map.w3i` inside the map); an empty authored
//! name stays empty, never a filename-derived substitute.
//!
//! A selected [`SkirmishMap`] is admitted through
//! [`crate::browser_configuration::ConfigurationInbox`] with the caller-
//! pinned [`pcx_play::campaign_data_set::CampaignRecipe`] (the same shape the
//! native campaign chapter launch uses), never through the `.w3n` custom-
//! campaign path. No players, difficulty, teams, rules or seeds are invented
//! here: the map's own `war3map.w3i` players and the pinned recipe's catalog
//! are the only inputs. A refused admission leaves the inbox's current
//! session untouched (the consumer preserves every old active field).

use crate::browser_configuration::{Configuration, ConfigurationInbox, Member};
use crate::map_binding::BindingLimits;
use pcx_play::campaign_data_set::CampaignRecipe;
use pcx_play::install_source::{MAX_AGGREGATE_BYTES, MAX_ASSET_BYTES, MAX_LISTED_PATHS};
use pcx_play::map_admission::MapAdmissionBudget;
use pcx_play::scripts::AssetIdentity;
use std::fmt;
use std::sync::Arc;

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
    /// Candidates skipped because the `max_maps` budget ran out, or that
    /// failed parse/info with a source-context error.
    pub skipped: Vec<DiscoverySkip>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoverySkip {
    pub asset_path: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveryBudget {
    /// Maximum candidates examined; bounds collection work.
    pub max_maps: usize,
    /// Per-map byte limit passed to `Resolver::resolve_bounded`.
    pub max_bytes_per_map: u64,
}

impl DiscoveryBudget {
    /// Refuses a zero collection budget: discovery with no candidates
    /// examined is a caller bug, not an empty result.
    pub fn new(max_maps: usize, max_bytes_per_map: u64) -> Result<Self, Error> {
        if max_maps == 0 {
            return Err(Error::InvalidBudget {
                field: "max_maps",
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

/// Bounded identity for one discovered map, derived only from the selected
/// pinned recipe and the exact selected bytes. The member label is the
/// authored `war3map.w3i` title when present, otherwise the asset path; the
/// identity edition is the pinned recipe's edition label, never a guessed
/// player/difficulty/team set.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedMap {
    pub asset_path: String,
    pub label: String,
    pub identity: AssetIdentity,
}

/// Build the identity for a discovered map from the pinned recipe's own
/// edition. `map_name` is the authored title, possibly empty.
pub fn selected_map(
    recipe: &CampaignRecipe,
    map: &SkirmishMap,
) -> SelectedMap {
    let edition = format!("{:?} custom game map", recipe.mode().addon);
    SelectedMap {
        asset_path: map.asset_path.clone(),
        label: if map.name.is_empty() {
            map.asset_path.clone()
        } else {
            map.name.clone()
        },
        identity: AssetIdentity {
            edition,
            version: recipe.rules().origin.version.clone(),
            source: map.asset_path.clone(),
        },
    }
}

/// Limits for a custom-game admission. These are the same bounded ceilings the
/// native campaign chapter launch uses; they are caller-owned policy, not
/// values read from the map.
pub fn admission_limits() -> BindingLimits {
    let scripts = pcx_play::MapScriptBudget {
        max_script_bytes: MAX_ASSET_BYTES,
        max_wts_bytes: MAX_ASSET_BYTES,
        max_archive_bytes: MAX_ASSET_BYTES,
        max_archive_table_bytes: MAX_ASSET_BYTES,
        max_raw_packed_bytes: MAX_ASSET_BYTES,
        max_decoded_bytes: MAX_ASSET_BYTES,
        max_prelude_bytes: MAX_ASSET_BYTES,
        max_aggregate_input_bytes: MAX_AGGREGATE_BYTES,
    };
    BindingLimits {
        reader: pcx_objects::editor::EditorReadLimits {
            max_listed_paths: MAX_LISTED_PATHS,
            max_asset_bytes: MAX_ASSET_BYTES,
            max_aggregate_input_bytes: MAX_AGGREGATE_BYTES,
        },
        overlay: crate::map_binding::OverlayLimits {
            unit_bytes: MAX_ASSET_BYTES,
            item_bytes: MAX_ASSET_BYTES,
            aggregate_bytes: MAX_AGGREGATE_BYTES,
        },
        admission: MapAdmissionBudget {
            scripts,
            terrain_bytes: MAX_ASSET_BYTES,
            info_bytes: MAX_ASSET_BYTES,
            pathing_bytes: MAX_ASSET_BYTES,
            units_bytes: MAX_ASSET_BYTES,
            aggregate_input_bytes: MAX_AGGREGATE_BYTES,
            required_fields: 0,
        },
        max_map_file_bytes: MAX_ASSET_BYTES,
        max_resolver_input_bytes: MAX_AGGREGATE_BYTES,
    }
}

/// Assemble the canonical [`Configuration`] for one selected discovered map
/// against the pinned campaign recipe. The resolver, catalog sources and
/// common/blizzard members all come from the recipe-selected namespace; the
/// map bytes are re-resolved from that resolver and must match the bytes the
/// recipe opened, so a namespace change cannot silently swap the map.
pub fn configuration(
    recipe: Arc<CampaignRecipe>,
    map: &SkirmishMap,
    revision: u64,
) -> Result<Configuration, String> {
    let base = recipe.base_resolver().map_err(|error| error.to_string())?;
    let bytes = base
        .resolve_bounded(&map.asset_path, MAX_ASSET_BYTES as u64)
        .map_err(|error| format!("{}: {error}", map.asset_path))?;
    let reader = recipe
        .open_map(&bytes.bytes, &map.asset_path, map_read_budget())
        .map_err(|error| error.to_string())?;
    let resolver = reader.resolver;
    let selected = resolver
        .resolve_bounded(&map.asset_path, MAX_ASSET_BYTES as u64)
        .map_err(|error| format!("{}: {error}", map.asset_path))?;
    if selected.bytes != bytes.bytes {
        return Err(
            "selected custom-game namespace changes the map used to select its data".into(),
        );
    }
    let selected_map = selected_map(&recipe, map);
    let edition = selected_map.identity.edition.clone();
    let member = |path: &str, label: &str| -> Result<Member, String> {
        Ok(Member {
            path: path.into(),
            label: label.into(),
            identity: AssetIdentity {
                edition: edition.clone(),
                version: "selected install".into(),
                source: path.into(),
            },
            source: resolver
                .source_for(path)
                .map_err(|error| error.to_string())?
                .clone(),
        })
    };
    let map_member = Member {
        path: map.asset_path.clone(),
        label: selected_map.label.clone(),
        identity: selected_map.identity.clone(),
        source: resolver
            .source_for(&map.asset_path)
            .map_err(|error| error.to_string())?
            .clone(),
    };
    let common = member("Scripts\\common.j", "Scripts\\common.j")?;
    let blizzard = member("Scripts\\Blizzard.j", "Scripts\\Blizzard.j")?;
    let mut catalog_sources = Vec::new();
    for path in resolver.paths() {
        let source = resolver
            .source_for(path.as_str())
            .map_err(|error| error.to_string())?;
        if !catalog_sources.contains(source) {
            catalog_sources.push(source.clone());
        }
    }
    let catalog_identity = AssetIdentity {
        edition,
        version: recipe.rules().origin.version.clone(),
        source: format!(
            "{}::{}",
            recipe.dir().display(),
            reader.selection.prefix
        ),
    };
    Ok(Configuration {
        resolver,
        campaign: Some(recipe),
        map: map_member,
        common,
        blizzard,
        catalog_identity,
        catalog_label: "selected install object data".into(),
        catalog_sources,
        required_catalog_tables: pcx_objects::editor::SlkId::ALL.to_vec(),
        required_fields: Vec::new(),
        revision,
        limits: admission_limits(),
    })
}

fn map_read_budget() -> pcx_mpq::checked::ReadBudget {
    let limits = admission_limits();
    pcx_mpq::checked::ReadBudget {
        max_archive_bytes: limits.max_map_file_bytes,
        max_archive_table_bytes: limits.admission.scripts.max_archive_table_bytes,
        max_raw_packed_bytes: limits.admission.scripts.max_raw_packed_bytes,
        max_decoded_bytes: limits.admission.info_bytes,
    }
}

/// Enqueue one selected discovered map. A refused enqueue (busy, exhausted or
/// an unbuildable request) surfaces as the inbox error and leaves the current
/// session untouched; callers keep the previous selection.
pub fn enqueue_selected(
    inbox: &ConfigurationInbox,
    recipe: Arc<CampaignRecipe>,
    map: &SkirmishMap,
    revision: u64,
) -> Result<u32, String> {
    let request = configuration(recipe, map, revision)?;
    inbox
        .enqueue(request)
        .map_err(|error| error.to_string())
}

/// Inventories `.w3m`/`.w3x` paths from `resolver.paths()`, reads each under
/// `budget.max_bytes_per_map` via `resolve_bounded`, and parses it with
/// `pcx_map::Map::parse` + checked `info()`. Entries in `maps/` are custom
/// candidates; other directories are only inventoried if the resolver has
/// them mounted, never guessed from an install directory.
pub fn discover_custom_maps(
    resolver: &pcx_assets::Resolver,
    budget: &DiscoveryBudget,
) -> Discovery {
    let mut candidates: Vec<String> = resolver
        .paths()
        .map(|p| p.as_str().to_owned())
        .filter(|p| {
            p.to_ascii_lowercase().ends_with(".w3m") || p.to_ascii_lowercase().ends_with(".w3x")
        })
        .collect();
    candidates.sort();
    candidates.dedup();

    let mut maps = Vec::new();
    let mut skipped = Vec::new();
    for (examined, path) in candidates.iter().enumerate() {
        if examined >= budget.max_maps {
            skipped.push(DiscoverySkip {
                asset_path: path.clone(),
                error: format!("{path}: discovery budget exceeded (max_maps={})", budget.max_maps),
            });
            continue;
        }
        match inspect_asset(resolver, path, budget.max_bytes_per_map) {
            Ok(m) => maps.push(m),
            Err(e) => skipped.push(DiscoverySkip {
                asset_path: path.clone(),
                error: e,
            }),
        }
    }
    Discovery { maps, skipped }
}

pub fn inspect_asset(
    resolver: &pcx_assets::Resolver,
    asset_path: &str,
    max_bytes: u64,
) -> Result<SkirmishMap, String> {
    let resolved = resolver
        .resolve_bounded(asset_path, max_bytes)
        .map_err(|e| format!("{asset_path}: {e}"))?;
    inspect_bytes(&resolved.path.as_str().to_owned(), &resolved.bytes)
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
    pub(super) fn w3i_bytes(name: &str) -> Vec<u8> {
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
    pub(super) fn synthetic_w3m(w3i: &[u8]) -> Vec<u8> {
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
        assert_eq!(d.skipped.len(), 1);
        assert!(d.skipped[0].error.contains("max_maps"));
    }

    /// Selected discovered map must enqueue the canonical request from the
    /// pinned recipe. A second enqueue while one is pending refuses (busy)
    /// and leaves the pending request — hence the current session — intact.
    ///
    /// Real-install only: `CampaignRecipe::pin` requires the reviewed
    /// `Warcraft III.exe` image and archives — see `pinned_legacy_recipe`.
    #[test]
    #[ignore = "requires PEACECRAFT3_INSTALL (or PEACECRAFT3_LEGACY_DIR) pointing at the reviewed Legacy install; run with --ignored"]
    fn selected_discovered_map_enqueues_canonical_request_and_refusal_preserves_session() {
        use crate::browser_configuration::{ConfigurationInbox, Poll};
        use crate::map_requests::MapRequests;
        use bevy::prelude::App;

        // A real shipped map from the pinned recipe's own inventory: the
        // synthetic fixture path cannot be resolved by the pinned catalog,
        // so admission of a fixture map would refuse for the wrong reason.
        // The pinned recipe supplies both the discovery resolver and the
        // admission catalog — no invented players/difficulty anywhere.
        let install = std::env::var_os("PEACECRAFT3_INSTALL")
            .or_else(|| std::env::var_os("PEACECRAFT3_LEGACY_DIR"))
            .expect("custom-game admission requires an explicit Legacy install (PEACECRAFT3_INSTALL)");
        let install = PathBuf::from(install);
        let recipe = pinned_legacy_recipe(&install)
            .expect("pinned Legacy recipe must pin against the reviewed image");
        let resolver = recipe.base_resolver().expect("pinned recipe must yield a resolver");
        let budget = DiscoveryBudget::new(256, 1 << 20).unwrap();
        let d = discover_custom_maps(&resolver, &budget);
        assert!(
            !d.maps.is_empty(),
            "real Legacy install must yield a non-zero discovered custom-map count; skipped={:?}",
            d.skipped
        );
        let map = d.maps.into_iter().next().unwrap();
        assert!(
            map.asset_path.ends_with(".w3m") || map.asset_path.ends_with(".w3x"),
            "selected map must be a .w3m/.w3x member"
        );

        // The identity carries no invented players/difficulty: edition is the
        // recipe's own label, label is the authored title, source is the path.
        let selected = selected_map(&recipe, &map);
        assert_eq!(selected.asset_path, map.asset_path);
        assert_eq!(selected.identity.source, map.asset_path);
        assert!(selected.identity.edition.contains("custom game map"));
        assert_eq!(selected.label, map.name);

        // Canonical request: same recipe-selected namespace the chapter launch
        // builds, revision carried through, catalog from the pinned recipe.
        let inbox = ConfigurationInbox::default();
        let id = enqueue_selected(&inbox, Arc::clone(&recipe), &map, 7)
            .expect("selected discovered map must enqueue the canonical request");
        assert!(matches!(
            inbox.poll(id).unwrap(),
            crate::browser_configuration::Poll::Pending
        ));

        // Refusal while pending: a second enqueue is busy; the pending request
        // (and the live session it will configure) is preserved, not replaced.
        assert!(enqueue_selected(&inbox, Arc::clone(&recipe), &map, 8).is_err());
        assert!(matches!(
            inbox.poll(id).unwrap(),
            crate::browser_configuration::Poll::Pending
        ));

        // Driving the consumer against the pending request runs the admission:
        // a shipped map under the pinned catalog either admits (bindings
        // installed for that exact map) or refuses without touching them.
        // Either way the pending request completes with a typed outcome.
        let mut app = App::new();
        crate::map_requests::install(&mut app);
        crate::browser_configuration::install(&mut app, inbox.clone());
        app.update();
        let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
            panic!("pending request must complete after the consumer runs")
        };
        let admitted = outcome.result.is_ok();
        let requests = app.world().resource::<MapRequests>();
        assert_eq!(
            requests.bindings.is_some(),
            admitted,
            "bindings are installed exactly when the admission succeeds"
        );
    }

    /// Pin the reviewed Legacy recipe against the client image at `install`.
    /// The image is read directly from `Warcraft III.exe` under the same
    /// 32 MiB ceiling the native campaign path uses; the recipe's rules,
    /// catalog and edition all derive from that image and the install's own
    /// archives. No recipe is fabricated.
    fn pinned_legacy_recipe(
        install: &std::path::Path,
    ) -> Result<Arc<CampaignRecipe>, String> {
        use std::io::Read;
        let image_path = install.join("Warcraft III.exe");
        let mut file = std::fs::File::open(&image_path)
            .map_err(|e| format!("{}: {e}", image_path.display()))?;
        let mut image = Vec::new();
        file.by_ref()
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut image)
            .map_err(|e| format!("{}: {e}", image_path.display()))?;
        let identity =
            pcx_play::campaign_inputs::client_identity(&image, &image_path.display().to_string());
        let rules = pcx_play::campaign_data_set::ImageRules::read(&image, identity)
            .map_err(|error| error.to_string())?;
        CampaignRecipe::pin(
            install,
            pcx_assets::GameMode::default(),
            rules,
            &image,
            None,
            "VERSION_FROZEN_THRONE",
        )
        .map(Arc::new)
        .map_err(|error| error.to_string())
    }

    /// Real-install gate for discovery itself: against the known Legacy
    /// recipe's resolver inventory, `discover_custom_maps` must find a
    /// non-zero number of real shipped maps. An unrelated `pcx-mdl` geoset
    /// check elsewhere in the tree is NOT discovery proof and is not used
    /// here: this test re-derives the count from the pinned recipe's own
    /// resolver, so a green run proves the pinned catalog actually lists
    /// maps at this install.
    #[test]
    #[ignore = "requires PEACECRAFT3_INSTALL (or PEACECRAFT3_LEGACY_DIR) pointing at the reviewed Legacy install; run with --ignored"]
    fn discover_custom_maps_real_install_gate() {
        let install = std::env::var_os("PEACECRAFT3_INSTALL")
            .or_else(|| std::env::var_os("PEACECRAFT3_LEGACY_DIR"))
            .expect("explicit original install required");
        let install = PathBuf::from(install);
        let recipe = pinned_legacy_recipe(&install)
            .expect("pinned Legacy recipe must pin against the reviewed image");
        let resolver = recipe.base_resolver().expect("pinned recipe must yield a resolver");
        let budget = DiscoveryBudget::new(256, 1 << 20).unwrap();
        let d = discover_custom_maps(&resolver, &budget);
        assert!(
            !d.maps.is_empty(),
            "real Legacy install must yield a non-zero discovered custom-map count; skipped={:?}",
            d.skipped
        );
        assert!(
            d.maps.iter().all(|m| m.asset_path.ends_with(".w3m") || m.asset_path.ends_with(".w3x")),
            "every discovered map must be a .w3m/.w3x member"
        );
    }
}

#[cfg(test)]
mod custom_game_synthetic_sys_fixture {
    //! Synthetic sys-fixture refusal test (nonstock, no real install).
    //! `CampaignRecipe` has no synthetic constructor (`pin` hashes the
    //! reviewed image), so the synthetic path exercises the shared
    //! refusal/preservation contract directly, without fabricating a recipe.
    //! Uses the same opaque `Memory`-backed resolver pattern as
    //! `campaign_audio`'s synthetic source.
    use crate::browser_configuration::{ConfigurationInbox, Configuration, Member, Poll};
    use crate::custom_game::{admission_limits, inspect_asset};
    use super::tests::{synthetic_w3m, w3i_bytes};
    use crate::map_requests::MapRequests;
    use bevy::prelude::App;
    use pcx_assets::{ArchiveLayer, ContentSource, SourceInfo, SourceContent, SourceError};
    use pcx_play::scripts::AssetIdentity;

    struct Memory {
        info: SourceInfo,
        files: Vec<(String, Vec<u8>)>,
    }
    impl ContentSource for Memory {
        fn info(&self) -> &SourceInfo {
            &self.info
        }
        fn paths(&self) -> Result<Vec<String>, SourceError> {
            Ok(self.files.iter().map(|(path, _)| path.clone()).collect())
        }
        fn read(&self, member: &str) -> Result<Option<SourceContent>, SourceError> {
            self.read_bounded(member, u64::MAX)
        }
        fn read_bounded(
            &self,
            member: &str,
            limit: u64,
        ) -> Result<Option<SourceContent>, SourceError> {
            let Some((_, bytes)) = self.files.iter().find(|(path, _)| path == member) else {
                return Ok(None);
            };
            if bytes.len() as u64 > limit {
                return Err(SourceError::Limit {
                    requested: bytes.len() as u64,
                    limit,
                });
            }
            Ok(Some(SourceContent {
                bytes: bytes.clone(),
                declared_length: bytes.len() as u64,
                locator: member.to_owned(),
            }))
        }
    }

    fn synthetic_resolver() -> pcx_assets::Resolver {
        // Opaque nonstock fixture: the map path resolves, but none of the
        // members a real admission requires exist.
        pcx_assets::Resolver::new(
            pcx_assets::GameMode {
                build: pcx_assets::Build::V1_29b,
                addon: pcx_assets::Addon::Tft,
                assets: pcx_assets::AssetMode::Vanilla,
                behaviour: pcx_assets::Behaviour::Vanilla,
            },
            None,
            vec![ArchiveLayer {
                priority: 1,
                source: Box::new(Memory {
                    info: SourceInfo {
                        id: "sys-fixture".into(),
                        kind: pcx_assets::LayerKind::Mpq,
                        origin: "sys fixture".into(),
                        version: Some("synthetic".into()),
                    },
                    files: vec![(
                        "maps/scenario/synthetic.w3m".into(),
                        synthetic_w3m(&w3i_bytes("Synthetic Sys Fixture")),
                    )],
                }),
            }],
        )
        .unwrap()
    }

    fn synthetic_configuration(resolver: pcx_assets::Resolver) -> Configuration {
        let identity = AssetIdentity {
            source: "sys fixture".into(),
            edition: "explicit synthetic edition".into(),
            version: "explicit synthetic version".into(),
        };
        let source = SourceInfo {
            id: "sys-fixture".into(),
            kind: pcx_assets::LayerKind::Mpq,
            origin: "sys fixture".into(),
            version: Some("synthetic".into()),
        };
        let member = |path: &str| Member {
            path: path.into(),
            label: path.into(),
            identity: identity.clone(),
            source: source.clone(),
        };
        Configuration {
            resolver,
            campaign: None,
            map: member("maps/scenario/synthetic.w3m"),
            common: member("Scripts\\common.j"),
            blizzard: member("Scripts\\Blizzard.j"),
            catalog_identity: identity,
            catalog_label: "sys fixture catalog".into(),
            catalog_sources: vec![source],
            required_catalog_tables: vec![],
            required_fields: vec![],
            revision: 1,
            limits: admission_limits(),
        }
    }

    #[test]
    fn synthetic_sys_fixture_refusal_preserves_session() {
        // Refusal #1 — recipe leg: with no pinned recipe the member lookups
        // fail on the synthetic source instead of inventing provenance.
        let resolver = synthetic_resolver();
        let map = inspect_asset(&resolver, "maps/scenario/synthetic.w3m", 1 << 20)
            .expect("synthetic sys-fixture map must be discoverable");
        assert_eq!(map.name, "Synthetic Sys Fixture");
        let missing = resolver
            .source_for("Scripts\\common.j")
            .map_err(|e| e.to_string());
        assert!(missing.is_err(), "sys fixture must have no common.j: {missing:?}");

        // Refusal #2 — inbox busy: a pending synthetic request is not
        // replaced by a second synthetic enqueue.
        let inbox = ConfigurationInbox::default();
        let id = inbox
            .enqueue(synthetic_configuration(synthetic_resolver()))
            .expect("first synthetic request must enqueue");
        assert!(matches!(inbox.poll(id).unwrap(), Poll::Pending));
        assert!(
            inbox
                .enqueue(synthetic_configuration(synthetic_resolver()))
                .is_err(),
            "second enqueue while pending must refuse (busy)"
        );
        assert!(matches!(inbox.poll(id).unwrap(), Poll::Pending));

        // Refusal #3 — consumer outcome: the synthetic request refuses at
        // binding (no real common.j/catalog), and the live session's
        // bindings (initially none) are preserved, not installed.
        let mut app = App::new();
        crate::map_requests::install(&mut app);
        crate::browser_configuration::install(&mut app, inbox.clone());
        app.update();
        let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
            panic!("pending synthetic request must complete after the consumer runs")
        };
        assert!(
            outcome.result.is_err(),
            "sys-fixture admission must refuse (nonstock, no recipe): label={} result_is_err={}",
            outcome.label,
            outcome.result.is_err()
        );
        assert!(
            app.world().resource::<MapRequests>().bindings.is_none(),
            "refused admission must not install bindings"
        );
    }
}
