//! The shipped base object data — the object editor's own tables, decoded.
//!
//! `war3map.w3u` / `war3map.w3t` are a map's *edits*; this module is the data
//! those edits sit on top of. Every number the in-game editor shows for a
//! stock unit is a cell in one of the `.slk` tables under `Units\`, and every
//! field fourcc a map override can name is a row of `Units\UnitMetaData.slk`
//! that says which table and which column it lands in.
//!
//! # What actually ships (measured, both builds)
//!
//! Counted from [`pcx_mpq::MpqSet::file_list`], the union of every shipped
//! archive's own listfile — never a single `(listfile)` read. `.slk` in that
//! union is **59 on 1.28c and 65 on 1.29b**, matching what `pcx-slq` measures
//! on the same union; `.txt` is **159** and **192**.
//!
//! The object-editor data is the top-level `Units\` directory plus `Doodads\`,
//! plus the World Editor's own working copies. **Measured per build:**
//!
//! | group | 1.28c slk | 1.28c txt | 1.29b slk | 1.29b txt |
//! |-------|:---------:|:---------:|:---------:|:---------:|
//! | `Units\` (top level — what the engine reads)  | 15 | 48 | 17 | 49 |
//! | `Doodads\`                                     |  2 |  0 |  2 |  0 |
//! | `Melee_V0\Units\` (editor working copy)       |  7 | 23 |  9 | 23 |
//! | `Custom_V0\Units\` (editor working copy)      |  7 | 23 |  9 | 23 |
//! | `Custom_V1\Units\` (editor working copy)      |  5 | 27 |  6 | 27 |
//!
//! Several names remembered from a package listing are wrong, and the bytes
//! say so:
//!
//! * **There is no `Abilities\*.slk` and no `Misc\` directory at all.** The
//!   ability data is `Units\AbilityData.slk`; the misc constants are
//!   `Units\MiscData.txt` and `UI\MiscData.txt`.
//! * The UI table is spelled **`Units\unitUI.slk`** in the listfile — lower
//!   `unit`. MPQ name hashing is case-insensitive so both spellings reach the
//!   same bytes, but a directory listing built from memory would miss it.
//! * The unit localisation tables are **`.txt`, not `.fdf`**: six
//!   `*UnitStrings.txt` (Human, Orc, Undead, NightElf, Neutral, Campaign) and
//!   six `*UnitFunc.txt`. All 86 `.fdf` are `UI\FrameDef\*` UI frames.
//! * `Melee_V0`, `Custom_V0`, `Custom_V1` are the **World Editor's working
//!   copies** and are deliberately not decoded: the engine's tables are the
//!   top-level ones, and a working copy would answer for the same rawcode with
//!   different bytes (`Melee_V0\Units\UnitBalance.slk` is 345 341 bytes on
//!   1.28c against 616 820 for `Units\UnitBalance.slk`).
//!
//! # The listfile is not the file set
//!
//! A census from the union listfile **under-counts**. 1.28c's union names 59
//! `.slk`, but `Units\AbilityBuffData.slk` and `Units\AbilityBuffMetaData.slk`
//! also exist in that build — in `War3Patch.mpq`, whose own `(listfile)`
//! extract fails outright, and **no listfile on 1.28c names either file**.
//! They were extracted and decoded here (238 and 28 rows), so
//! [`Editor::absent`] is empty on both builds while a listfile census would
//! report them missing. The two builds differ by where the engine's tables
//! live, too: `Units\UnitData.slk` resolves out of `War3Patch.mpq` on 1.28c
//! and out of `War3x.mpq` on 1.29b.
//!
//! # The fourcc join, and what it proves
//!
//! `Units\UnitMetaData.slk` is the machine-readable object-editor schema: one
//! row per field fourcc carrying the SLK column name (`field`), which table
//! (`slk`), and the editor's own declared type (`type`). Measured on both
//! builds:
//!
//! * **267/267 rows resolve.** Every `slk`+`field` pair names a row-1 column
//!   that exists in the shipped table, through `pcx_slq::Table::column_of`
//!   (`UnitBalance` 51, `UnitUI` 43, `UnitData` 24, `ItemData` 30,
//!   `UnitWeapons` 59, `UnitAbilities` 3, `Profile` 57). **0 duplicate
//!   fourccs.** Of the 57 `Profile` rows, **49** name a key that a shipped
//!   `.txt` section carries and **8** do not (see the refusal list below).
//!   The same check is run at load time by [`Editor::open_in`] for every field
//!   of every metadata table, and the ones that fail are counted in
//!   [`EditorCensus`] rather than answered with a blank.
//! * **836 canonical unit rawcodes, in all five unit tables at once.** `UnitData`,
//!   `unitUI`, `UnitBalance`, `UnitWeapons` and `UnitAbilities` carry those
//!   shared identities. A selected UnitWeapons table can have a different
//!   identity header and additional authored rows. Its identity column is then
//!   derived from a unique complete join to the supplied UnitData/UnitBalance
//!   keys, with every weapon key validated; header aliases and column order
//!   are not inferred from a known install.
//! * **All 135 field fourccs used by any shipped `w3u`/`w3t` modification**
//!   (3 401 entries on 1.28c, 5 975 on both builds) come from
//!   `UnitMetaData.slk`. The **eight** metadata tables this crate reads hold
//!   **1 443 field rows and share 0 fourccs** between them, so nothing is
//!   ambiguous about which family a field belongs to: `UnitMetaData` 267,
//!   `AbilityMetaData` 747, `MiscMetaData` 223, `UpgradeEffectMetaData` 55,
//!   `DestructableMetaData` 52, `UpgradeMetaData` 37,
//!   `AbilityBuffMetaData` 27, `DoodadMetaData` 35.
//! * `UpgradeEffectMetaData.slk` *has* the `field`/`slk`/`index`/`category`
//!   columns in its header row — at columns 15..21 — but **not one of its 55
//!   data rows fills any of them**. Its rows parse to an empty field name and
//!   an empty `slk`, so they land in [`FieldSource::Unknown`] and are refused;
//!   nothing there is guessed.
//!
//! So `fourcc -> (table, column)` is measured, not folklore, and it pins the
//! wire type: the editor's own `type` column predicts the value word of
//! **3 393 of the 3 401** shipped modifications on 1.28c. The single exception
//! is `uhot`, declared `char` but shipped on the string wire 8 times; see
//! [`FieldKind::wire_word`] and [`Refusal::CharOnStringWire`].
//!
//! # Raw codes are matched case-insensitively, and that is measured too
//!
//! `Units\UnitBalance.slk` **row 664 spells its raw code `Nman`** while
//! `Maps\Campaign\Orc02Interlude.w3m`'s `war3map.w3u` writes `nman`; the same
//! column mixes `Hamg` (row 2) and `nmsc` (row 379), with **91 mixed-case raw
//! codes in all**. A case-sensitive join silently drops those records, so
//! [`Editor::row`] folds case and [`Editor::mixed_case_rawcodes`] counts how
//! many needed it.
//!
//! # The gate's answer, over the whole shipped corpus
//!
//! `#[ignore]`d sweeps in this module measure the M4 second half over every
//! shipped map, on both builds, and write
//! `/tmp/opencode/objects-editor-sweep.txt`:
//!
//! ```text
//! 1.28c: 48 object files, 888 base records (887 resolved), 282 custom
//!        records (282 new), 3 401 modifications
//! 1.29b: 44 object files, 757 base records (756 resolved), 252 custom
//!        records (252 new), 2 574 modifications
//! both:  836 units, 273 items, 89 upgrades, 20 command buttons
//! ```
//!
//! **1 643 of 1 645 first-table records resolve, and all 534 custom records
//! create a raw code the base data does not already carry (zero
//! collisions).** The single record that does not resolve is
//! `Maps\Campaign\Orc02Interlude.w3m`'s `war3map.w3u` base record for
//! **`Nwar`**, on **both** builds: no shipped `.slk` or `.txt` on either
//! install carries that raw code, at any case. It is reported by name rather
//! than dropped, because a map whose rawcodes do not resolve is the failure
//! this project refuses. (The other 8 field-level wire mismatches are all
//! `uhot`.)
//!
//! # What is still refused
//!
//! * **A word's width.** An int word is a word. The largest shipped value is
//!   99 999, which refutes a `u8` *slot*; nothing in the bytes says whether
//!   the game reads it `u8`, `u16`, `u32` or `i32`.
//! * **Scales.** `real`/`unreal` are IEEE-754 on the wire; nothing here says
//!   which is divided, rounded or clamped afterwards.
//! * **Eight profile fourccs.** `urq3..urq8` (`Requires3..8`), `urqa`
//!   (`Requiresamount`) and `urva` (`Reviveat`) are `Profile` rows, but **no
//!   shipped unit section carries `requires3`..`requires8`, `requiresamount`
//!   or `reviveat`**, so their base value is unmeasured. [`Unit::stat`]
//!   answers for the map's override, and [`Refusal::NoShippedBase`] reports
//!   that the base one cannot be read off the shipped tables.
//! * **The rest of the schema.** 1 443 field fourccs are read off the eight
//!   metadata tables and **1 001 of them are unmeasured**, by source:
//!   `AbilityMetaData` 724, `MiscMetaData` 192, `UpgradeEffectMetaData` 55,
//!   `AbilityBuffMetaData` 18, `UnitMetaData` 8, `DoodadMetaData` 3,
//!   `UpgradeMetaData` 1. The cause is one thing: this crate decodes the
//!   *unit*, *item* and *command* `.txt` layer only, so the ability, upgrade,
//!   misc and doodad `Profile` fields — whose keys ship in
//!   `HumanAbilityFunc.txt`, `*UpgradeFunc.txt`, `MiscData.txt` and
//!   `DoodadData.slk` — have no base value here. They are counted, reported and
//!   refused. **No shipped map sets any of them**, so nothing is lost, and the
//!   count is the honest statement of this crate's scope.
//! * **The end word of a modification.** Still raw, still unexplained.
//! * **`DoodadData`, `MiscGame` and the replay report format.** Not
//!   object-editor unit/item data; out of scope.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pcx_mpq::MpqSet;

use crate::Error;

pub mod camera;
pub mod campaign_index;
pub mod defaults;
pub mod fog_boundary;
pub mod hero;
pub mod hero_experience;
mod keys;
pub mod messages;
pub mod meta;
pub mod misc;
pub mod object;
pub mod path;
pub mod report;
pub mod selection;

#[cfg(test)]
mod tests;

pub use meta::{FieldDef, FieldKind, FieldSource};
pub use object::{Item, ObjectKind, StatValue, Unit, Upgrade};
pub use path::{SlkId, TxtRole};
pub use report::{
    Applied, ApplyReport, BuildCoverage, Coverage, Overlay, Refusal, Resolution, Unresolved,
    WireMismatch,
};

/// What one `.slk` carries, and where it lives in the archive.
pub const EDITOR_SLK: &[SlkId] = SlkId::ALL;

/// What one `.txt` carries, and where it lives in the archive.
pub const EDITOR_TXT: &[TxtRole] = TxtRole::ALL;

/// The measured file counts of one install, before anything is decoded.
#[derive(Debug, Clone, Default)]
pub struct EditorCensus {
    /// Every `.slk` in the union listfile of this install.
    pub slk_files: usize,
    /// Every `.txt` in the union listfile of this install.
    pub txt_files: usize,
    /// Every `.fdf` in the union listfile of this install.
    pub fdf_files: usize,
    /// `.slk` under the top-level `Units\` directory.
    pub units_slk: usize,
    /// `.txt` under the top-level `Units\` directory.
    pub units_txt: usize,
    /// `.slk` under `Doodads\`.
    pub doodads_slk: usize,
    /// Editor working copies (`Melee_V0`, `Custom_V0`, `Custom_V1`): `.slk`.
    pub working_copy_slk: usize,
    /// Editor working copies: `.txt`.
    pub working_copy_txt: usize,
    /// Raw codes per table, for every table this build ships.
    pub rows_by_slk: BTreeMap<SlkId, usize>,
    /// `.txt` profile tables this build ships that were decoded, by role.
    pub txt_loaded: BTreeMap<TxtRole, usize>,
    /// Editor working-copy members this build ships, as archive paths.
    pub working_copy_paths: Vec<String>,
}

/// Supplied byte limits apply before table/profile parsing.
/// The reader must bound its own extraction and allocation.
#[derive(Clone, Copy, Debug)]
pub struct EditorReadLimits {
    pub max_listed_paths: usize,
    pub max_asset_bytes: usize,
    pub max_aggregate_input_bytes: usize,
}
/// The base object data of one install, decoded.
#[derive(Debug)]
pub struct Editor {
    build_label: String,
    census: EditorCensus,
    tables: BTreeMap<SlkId, pcx_slq::Table>,
    /// Lower-cased raw code -> row, per table.
    rows: BTreeMap<SlkId, BTreeMap<String, u32>>,
    /// Each table's own raw-code spellings, in row order.
    spellings: BTreeMap<SlkId, Vec<String>>,
    fields: Vec<FieldDef>,
    fields_by_fourcc: BTreeMap<crate::Fourcc, usize>,
    profiles: BTreeMap<String, ProfileSection>,
    profile_keys: BTreeSet<String>,
    profile_duplicates: Vec<(String, String)>,
    /// `Units\CommandFunc.txt`'s sections, keyed by the command raw code.
    commands: BTreeMap<String, ProfileSection>,
    absent: Vec<SlkId>,
}

impl Editor {
    /// Open an install and decode every object-editor table it carries.
    ///
    /// A table this build does not ship is **absent, not defaulted**, and the
    /// load order says which archive it came from rather than assuming one:
    /// `Units\UnitData.slk` resolves out of `War3Patch.mpq` on 1.28c and out
    /// of `War3x.mpq` on 1.29b. [`Editor::absent`] reports what did not load;
    /// the measured rows of what did are in
    /// [`EditorCensus::rows_by_slk`].
    pub fn open(install: &Path) -> Result<Self, Error> {
        let mut set = MpqSet::open_wc3(install)?;
        Self::open_in(&mut set, install)
    }

    /// Open from an archive set already opened against `install`.
    pub fn open_in(set: &mut MpqSet, install: &Path) -> Result<Self, Error> {
        let listed = set.file_list()?;
        // Missing optional assets remain absent; present extraction failures propagate.
        Self::from_reader(
            install,
            &listed,
            EditorReadLimits {
                max_listed_paths: usize::MAX,
                max_asset_bytes: usize::MAX,
                max_aggregate_input_bytes: usize::MAX,
            },
            |member| match set.extract(member) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(pcx_mpq::Error::NotFound(_)) => Ok(None),
                Err(source) => Err(source),
            },
        )
    }

    /// Decode caller-selected supplied assets. None means absent; errors stop decoding.
    /// The reader owns extraction budgets and source/priority policy.
    pub fn from_reader<E: std::error::Error + Send + Sync + 'static>(
        install: &Path,
        listed: &[String],
        limits: EditorReadLimits,
        mut reader: impl FnMut(&str) -> Result<Option<Vec<u8>>, E>,
    ) -> Result<Self, Error> {
        if listed.len() > limits.max_listed_paths {
            return Err(Error::Unsupported {
                what: "editor listed path budget",
                detail: Some(format!(
                    "{}: {} > {}",
                    install.display(),
                    listed.len(),
                    limits.max_listed_paths
                )),
            });
        }
        let mut total = 0usize;
        let mut read = |member: &str| -> Result<Option<Vec<u8>>, Error> {
            let result = reader(member).map_err(|source| Error::EditorAsset {
                source_label: install.display().to_string(),
                member: member.to_owned(),
                source: Box::new(source),
            })?;
            if let Some(bytes) = &result {
                let sum = total.checked_add(bytes.len());
                if bytes.len() > limits.max_asset_bytes
                    || sum.is_none_or(|sum| sum > limits.max_aggregate_input_bytes)
                {
                    return Err(Error::Unsupported {
                        what: "editor supplied input budget",
                        detail: Some(format!(
                            "{}: {member}: bytes={}, aggregate={sum:?}",
                            install.display(),
                            bytes.len()
                        )),
                    });
                }
                total = sum.expect("validated aggregate");
            }
            Ok(result)
        };
        let mut census = EditorCensus::default();
        for p in listed {
            let low = p.to_ascii_lowercase();
            let is_slk = low.ends_with(".slk");
            let is_txt = low.ends_with(".txt");
            let is_fdf = low.ends_with(".fdf");
            census.slk_files += usize::from(is_slk);
            census.txt_files += usize::from(is_txt);
            census.fdf_files += usize::from(is_fdf);
            let root = low.split(['\\', '/']).next().unwrap_or("").to_string();
            match root.as_str() {
                "units" => {
                    census.units_slk += usize::from(is_slk);
                    census.units_txt += usize::from(is_txt);
                }
                "doodads" => census.doodads_slk += usize::from(is_slk),
                "melee_v0" | "custom_v0" | "custom_v1" => {
                    census.working_copy_slk += usize::from(is_slk);
                    census.working_copy_txt += usize::from(is_txt);
                    census.working_copy_paths.push(p.clone());
                }
                _ => {}
            }
        }

        let mut tables = BTreeMap::new();
        let mut absent = Vec::new();
        for id in SlkId::ALL {
            let Some(bytes) = read(id.path())? else {
                absent.push(*id);
                continue;
            };
            let table = pcx_slq::Table::parse(&bytes).map_err(|source| Error::EditorAsset {
                source_label: install.display().to_string(),
                member: id.path().to_owned(),
                source: Box::new(source),
            })?;
            census.rows_by_slk.insert(*id, table.row_count() as usize);
            tables.insert(*id, table);
        }

        // The fourcc index, straight out of the metadata tables.
        let mut fields: Vec<FieldDef> = Vec::new();
        for id in meta::METADATA_SLK {
            let Some(t) = tables.get(id) else { continue };
            fields.extend(FieldDef::read(t, *id));
        }
        let mut fields_by_fourcc = BTreeMap::new();
        for (i, f) in fields.iter().enumerate() {
            fields_by_fourcc.insert(f.fourcc, i);
        }

        // The raw-code index, one per row-bearing table, **case-folded**.
        //
        // This is measured, not defensive: the shipped tables do not agree
        // with the shipped maps on raw-code case. `Units\UnitBalance.slk`
        // row 664 spells the raw code `Nman` while
        // `Maps\Campaign\Orc04Interlude.w3m`'s `war3map.w3u` writes `nman`,
        // and the same column mixes `Hamg` (row 2) and `nmsc` (row 379). A
        // case-sensitive join therefore drops records that the map really
        // declares.
        let mut rows = BTreeMap::new();
        let mut spellings = BTreeMap::new();
        for id in SlkId::ALL.iter().filter(|id| id.id_column().is_some()) {
            let Some(t) = tables.get(id) else { continue };
            let mut m = BTreeMap::new();
            let mut spelled = Vec::new();
            let column = if *id == SlkId::UnitWeapons {
                Some(keys::weapon_identity_column(&tables).map_err(|detail| {
                    Error::Unsupported {
                        what: "editor weapon identity schema",
                        detail: Some(format!("{}: {detail}", install.display())),
                    }
                })?)
            } else {
                t.column_of(id.id_column().expect("row-bearing table identity"))
            };
            if let Some(c) = column {
                for r in t.data_rows() {
                    if let Some(v) = r.get(c) {
                        m.entry(v.to_lowercase()).or_insert(r.index());
                        spelled.push(v.to_string());
                    }
                }
            }
            rows.insert(*id, m);
            spellings.insert(*id, spelled);
        }

        // The `.txt` profile layer.
        let mut profiles: BTreeMap<String, ProfileSection> = BTreeMap::new();
        let mut commands: BTreeMap<String, ProfileSection> = BTreeMap::new();
        let mut profile_keys: BTreeSet<String> = BTreeSet::new();
        let mut profile_duplicates: Vec<(String, String)> = Vec::new();
        let mut txt_loaded = BTreeMap::new();
        for role in TxtRole::ALL {
            let mut loaded = 0usize;
            for file in role.files() {
                let Some(bytes) = read(file)? else {
                    continue;
                };
                let text = String::from_utf8_lossy(&bytes).into_owned();
                loaded += 1;
                // The command tables own their namespace: `CmdMove` is not a
                // unit and must not be found by a unit lookup.
                let (sink, key_sink) = match role {
                    TxtRole::CommandFunc | TxtRole::CommandStrings => {
                        (&mut commands, &mut profile_keys)
                    }
                    _ => (&mut profiles, &mut profile_keys),
                };
                parse_txt(&text, file, sink, key_sink, &mut profile_duplicates);
            }
            txt_loaded.insert(*role, loaded);
        }
        census.txt_loaded = txt_loaded;

        // Which fields this build really carries a base value for.
        //
        // A `Profile` field is measured when some shipped `.txt` section uses
        // its key; a `Column` field when the table really has that row-1
        // column. This is the join, measured at load rather than assumed, and
        // it is what [`Refusal::NoShippedBase`] reports on.
        for def in &mut fields {
            def.measured = match &def.source {
                FieldSource::Column { slk, column } => {
                    tables.get(slk).and_then(|t| t.column_of(column)).is_some()
                }
                FieldSource::Profile { key, .. } => profile_keys.contains(key),
                FieldSource::Unknown { .. } => false,
            };
        }

        Ok(Self {
            build_label: install.display().to_string(),
            census,
            tables,
            rows,
            spellings,
            fields,
            fields_by_fourcc,
            profiles,
            profile_keys,
            profile_duplicates,
            commands,
            absent,
        })
    }

    /// Tables this install does not ship, as archive paths.
    ///
    /// Measured on both builds this crate is tested against, the answer is
    /// `[]`: 1.28c *does* carry `Units\AbilityBuffData.slk` and
    /// `Units\AbilityBuffMetaData.slk`, in `War3Patch.mpq`, even though no
    /// listfile on that build names them. So the listfile undercounts and the
    /// [`Editor::census`] slk count is not the number of tables that load —
    /// both are reported, and neither is smoothed over.
    pub fn absent(&self) -> &[SlkId] {
        &self.absent
    }

    /// The install this came from, as a displayable label.
    pub fn build(&self) -> &str {
        &self.build_label
    }

    /// The measured file census, before decoding.
    pub fn census(&self) -> &EditorCensus {
        &self.census
    }

    /// One decoded table.
    pub fn table(&self, id: SlkId) -> Option<&pcx_slq::Table> {
        self.tables.get(&id)
    }

    /// Every field fourcc this install's metadata defines.
    pub fn fields(&self) -> &[FieldDef] {
        &self.fields
    }

    /// One field fourcc's definition.
    pub fn field(&self, fourcc: crate::Fourcc) -> Option<&FieldDef> {
        self.fields_by_fourcc.get(&fourcc).map(|&i| &self.fields[i])
    }

    /// The row of `id` whose id column is `rawcode`, if it exists.
    ///
    /// Looked up case-insensitively, because the shipped tables and the
    /// shipped maps spell raw codes with different case — the measured example
    /// is in the load code, and [`Editor::mixed_case_rawcodes`] counts the
    /// 91 that need it in `UnitBalance.slk` alone.
    pub fn row(&self, id: SlkId, rawcode: &str) -> Option<u32> {
        self.rows.get(&id)?.get(&rawcode.to_lowercase()).copied()
    }

    /// The `.txt` profile section for a raw code, merged across every role.
    ///
    /// Looked up case-insensitively: the files spell `Hamg` and `nmsc`, and a
    /// map overrides either case.
    pub fn profile(&self, rawcode: &str) -> Option<&ProfileSection> {
        self.profiles.get(&rawcode.to_lowercase())
    }

    /// How many distinct raw codes a shipped `.txt` section is keyed by.
    pub fn profiles_len(&self) -> usize {
        self.profiles.len()
    }

    /// Every key any shipped profile section carries, lower-cased.
    pub fn profile_keys(&self) -> &BTreeSet<String> {
        &self.profile_keys
    }

    /// `(rawcode, key)` pairs a shipped `.txt` wrote twice with different
    /// values. Two ship, both `Buttonpos` in `NeutralUnitFunc.txt`.
    pub fn profile_duplicates(&self) -> &[(String, String)] {
        &self.profile_duplicates
    }

    /// Whether `rawcode` names something in the base data, and which kind.
    pub fn resolve(&self, rawcode: &str) -> Option<ObjectKind> {
        if self.row(SlkId::UnitBalance, rawcode).is_some()
            || self.row(SlkId::UnitData, rawcode).is_some()
        {
            return Some(ObjectKind::Unit);
        }
        if self.row(SlkId::ItemData, rawcode).is_some() {
            return Some(ObjectKind::Item);
        }
        if self.row(SlkId::UpgradeData, rawcode).is_some() {
            return Some(ObjectKind::Upgrade);
        }
        if self.row(SlkId::AbilityData, rawcode).is_some() {
            return Some(ObjectKind::Ability);
        }
        if self.command(rawcode).is_some() {
            return Some(ObjectKind::Command);
        }
        None
    }

    /// Every unit raw code, in `UnitBalance.slk` row order and spelling.
    pub fn units(&self) -> Vec<String> {
        self.rawcodes(SlkId::UnitBalance)
    }

    /// Every item raw code, in `ItemData.slk` row order and spelling.
    pub fn items(&self) -> Vec<String> {
        self.rawcodes(SlkId::ItemData)
    }

    /// Every upgrade raw code, in `UpgradeData.slk` row order and spelling.
    pub fn upgrades(&self) -> Vec<String> {
        self.rawcodes(SlkId::UpgradeData)
    }

    /// Every ability raw code, in `AbilityData.slk` `code` column order.
    pub fn abilities(&self) -> Vec<String> {
        self.rawcodes(SlkId::AbilityData)
    }

    /// How many raw codes of a table differ in case from their lower-cased
    /// form — the reason [`Editor::row`] folds case.
    pub fn mixed_case_rawcodes(&self, id: SlkId) -> usize {
        self.rawcodes(id)
            .iter()
            .filter(|s| *s != &s.to_lowercase())
            .count()
    }

    /// The `[rawcode]` section of `Units\CommandFunc.txt`, the table that binds
    /// a command button (`CmdMove`, `CmdAttackGround`, …) to its icon.
    pub fn command(&self, rawcode: &str) -> Option<&ProfileSection> {
        self.commands.get(&rawcode.to_lowercase())
    }

    /// Every command-button raw code, in `CommandFunc.txt` order.
    pub fn commands(&self) -> Vec<String> {
        self.commands.keys().cloned().collect()
    }

    /// Every raw code of a table, in the spelling the table itself uses.
    pub fn rawcodes(&self, id: SlkId) -> Vec<String> {
        self.spellings.get(&id).cloned().unwrap_or_default()
    }

    /// One unit by raw code.
    pub fn unit(&self, rawcode: &str) -> Option<Unit<'_>> {
        self.row(SlkId::UnitBalance, rawcode)
            .map(|_| Unit::new(self, rawcode))
    }

    /// One item by raw code.
    pub fn item(&self, rawcode: &str) -> Option<Item<'_>> {
        self.row(SlkId::ItemData, rawcode)
            .map(|_| Item::new(self, rawcode))
    }

    /// One upgrade by raw code.
    pub fn upgrade(&self, rawcode: &str) -> Option<Upgrade<'_>> {
        self.row(SlkId::UpgradeData, rawcode)
            .map(|_| Upgrade::new(self, rawcode))
    }
}

/// One `[rawcode]` section of the `.txt` layer, merged across every file that
/// writes it.
#[derive(Debug, Clone, Default)]
pub struct ProfileSection {
    /// Values in file order; the first occurrence wins.
    pub keys: BTreeMap<String, Vec<String>>,
    /// Which archive file each key came from, for the inspector.
    pub origin: BTreeMap<String, String>,
}

impl ProfileSection {
    fn put(&mut self, key: &str, value: &str, file: &str) {
        let k = key.to_lowercase();
        match self.keys.get_mut(&k) {
            Some(v) => {
                v.push(value.to_string());
            }
            None => {
                self.keys.insert(k.clone(), vec![value.to_string()]);
                self.origin.insert(k, file.to_string());
            }
        }
    }

    /// Every value a shipped file gave for a key, lower-cased.
    pub fn values(&self, key: &str) -> &[String] {
        self.keys
            .get(&key.to_lowercase())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// The first value shipped for `key`, if any.
    pub fn value(&self, key: &str) -> Option<&str> {
        self.values(key).first().map(String::as_str)
    }

    /// The `index`-th element of `key`'s first value.
    ///
    /// A key the metadata gives **one** slot (`Name`, `Tip`, `Ubertip`,
    /// `Hotkey`, `Art`, …) has one value and a comma in it is text: the whole
    /// value is the answer at every index. A key the metadata gives **more**
    /// than one — only `Buttonpos`, `Missilearc`, `Missileart`, `MissileHoming`
    /// and `Missilespeed`, one row per weapon slot — is a comma list, and
    /// `Buttonpos=0,0` puts `ubpx` at 0 and `ubpy` at 1.
    ///
    /// Callers pass the slot count in because it comes from the metadata, not
    /// from the section. A slot past the value's elements answers `None`,
    /// which is the measured "this weapon slot was never shipped" answer.
    pub fn element(&self, key: &str, index: i32, slots: u32) -> Option<&str> {
        let v = self.value(key)?;
        if index < 0 || slots <= 1 {
            return Some(v);
        }
        v.split(',')
            .nth(index as usize)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// The file the first value came from.
    pub fn origin(&self, key: &str) -> Option<&str> {
        self.origin.get(&key.to_lowercase()).map(String::as_str)
    }
}

/// Read one `[section]` / `key=value` file into the profile layer.
///
/// Lines starting `//` and blank lines are skipped; a `[section]` line starts
/// a new raw code, lower-cased. Keys are compared lower-cased too, since the
/// files ship `Art=`, `Hotkey=` and `hotkey=` in different sections.
///
/// A value the file writes inside `"…"` — every `Ubertip`, every `Description`
/// — is stored **without** the quotes, exactly as the `.slk` parser removes
/// them. `Name=Archmage` is stored verbatim because nothing quotes it.
fn parse_txt(
    text: &str,
    file: &str,
    profiles: &mut BTreeMap<String, ProfileSection>,
    keys_seen: &mut BTreeSet<String>,
    duplicates: &mut Vec<(String, String)>,
) {
    let mut current = String::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        if let Some(rest) = t.strip_prefix('[') {
            match rest.find(']') {
                Some(end) => {
                    current = rest[..end].trim().to_lowercase();
                    profiles.entry(current.clone()).or_default();
                }
                None => continue,
            }
            continue;
        }
        let Some(eq) = t.find('=') else { continue };
        let key = t[..eq].trim();
        if key.is_empty() || current.is_empty() {
            continue;
        }
        let raw = t[eq + 1..].trim();
        // One pair of surrounding double quotes, if the file wrote them.
        let value = raw
            .strip_prefix('"')
            .and_then(|r| r.strip_suffix('"'))
            .unwrap_or(raw);
        let k = key.to_lowercase();
        keys_seen.insert(k.clone());
        let section = profiles.get_mut(&current).expect("section exists");
        let before = section.values(&k).first().cloned();
        section.put(&k, value, file);
        if let Some(b) = before
            && b != value
        {
            duplicates.push((current.clone(), k));
        }
    }
}
