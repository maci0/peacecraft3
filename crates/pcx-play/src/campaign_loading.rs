//! The loading screen shown while a campaign map loads: which screen a map
//! header selects, its text, and the progress bar's easing.
//!
//! The screen table is `[LoadingScreens]` in the selected `UI\WorldEditData.txt`;
//! the header fields are the map's own; the easing steps are read from the
//! reviewed client image. Evidence and open questions are in
//! `docs/re-notes/campaign-loading-screen.md`.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, sha256},
    neutral_rule_input::Pe,
};
use pcx_map::info::LoadingScreenFields;
use pcx_trig::wts::Table as Wts;

/// Selected path of the game table that lists the loading screens.
pub const TABLE_PATH: &str = r"UI\WorldEditData.txt";
/// Section of [`TABLE_PATH`] that lists the loading screens.
const TABLE_SECTION: &str = "LoadingScreens";
const COUNT_KEY: &str = "NumScreens";
/// Fields of one record: first game version, editor name key, sequence, model.
const RECORD_FIELDS: usize = 4;
const TRIGSTR_PREFIX: &str = "TRIGSTR_";
const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_IMAGE_SECTIONS: usize = 96;

/// Virtual addresses in the reviewed image of the three bar constants
/// (`0x549c4a`, `0x549c61` and `0x549c74` load them).
const FAST_STEP_VA: u32 = 0xe7_e968;
const SLOW_STEP_VA: u32 = 0xe8_df60;
const FULL_VA: u32 = 0xe6_9584;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LoadingError {
    #[error("{TABLE_PATH} has no [{TABLE_SECTION}] section")]
    NoSection,
    #[error("{TABLE_PATH} [{TABLE_SECTION}] {key}: {reason}")]
    Record { key: String, reason: &'static str },
    #[error("loading screen {index} is not in {TABLE_PATH} ({count} screens)")]
    UnknownScreen { index: u32, count: usize },
    #[error("{field} {key} is not in the map's string table")]
    MissingString { field: &'static str, key: String },
    #[error("{field} {key} is not UTF-8 in the map's string table")]
    BadString { field: &'static str, key: String },
    #[error("loading progress: {0}")]
    Image(&'static str),
}

/// One `[LoadingScreens]` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenRecord {
    /// Game version in which the screen first appeared.
    pub first_version: u32,
    /// Editor display text key.
    pub name_key: String,
    /// Index of the model sequence to play.
    pub sequence: u32,
    /// Model path as the table names it.
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenTable(Vec<ScreenRecord>);

impl ScreenTable {
    /// Parse the section. A listed record that is missing or malformed refuses.
    pub fn parse(text: &str) -> Result<Self, LoadingError> {
        let mut in_section = false;
        let mut found = false;
        let mut count = None;
        let mut rows = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                in_section = name.eq_ignore_ascii_case(TABLE_SECTION);
                found |= in_section;
                continue;
            }
            if !in_section || line.is_empty() || line.starts_with("//") {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key.eq_ignore_ascii_case(COUNT_KEY) {
                let n = value.trim().parse::<usize>().map_err(|_| {
                    record_error(COUNT_KEY, "count is not a number")
                })?;
                count = Some(n);
            } else {
                rows.push((key.to_owned(), value.trim().to_owned()));
            }
        }
        if !found {
            return Err(LoadingError::NoSection);
        }
        let count = count.ok_or_else(|| record_error(COUNT_KEY, "missing"))?;
        let mut records = Vec::with_capacity(count);
        for index in 0..count {
            let key = format!("{index:02}");
            let (_, value) = rows
                .iter()
                .find(|(k, _)| *k == key)
                .ok_or_else(|| record_error(&key, "missing"))?;
            records.push(parse_record(&key, value)?);
        }
        Ok(Self(records))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, index: u32) -> Option<&ScreenRecord> {
        self.0.get(usize::try_from(index).ok()?)
    }
}

fn record_error(key: &str, reason: &'static str) -> LoadingError {
    LoadingError::Record {
        key: key.to_owned(),
        reason,
    }
}

fn parse_record(key: &str, value: &str) -> Result<ScreenRecord, LoadingError> {
    let fields: Vec<&str> = value.splitn(RECORD_FIELDS, ',').map(str::trim).collect();
    let [first_version, name_key, sequence, model] = fields[..] else {
        return Err(record_error(key, "expected four fields"));
    };
    if name_key.is_empty() || model.is_empty() {
        return Err(record_error(key, "empty name or model"));
    }
    Ok(ScreenRecord {
        first_version: first_version
            .parse()
            .map_err(|_| record_error(key, "first version is not a number"))?,
        name_key: name_key.to_owned(),
        sequence: sequence
            .parse()
            .map_err(|_| record_error(key, "sequence is not a number"))?,
        model: model.to_owned(),
    })
}

/// Where the model of a selected screen came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Record `index` of the game table.
    Table { index: u32, name_key: String },
    /// The map's own model path, played from its first sequence.
    Custom,
}

/// The background model and sequence a map header selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub model: String,
    pub sequence: u32,
    pub origin: Origin,
}

/// What a header asks to be shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// A campaign-style screen with title, subtitle and text.
    Campaign(Screen),
    /// The header names neither a number nor a model: the original shows its
    /// melee panel, which this recreation does not draw.
    Melee,
}

/// A non-negative background number selects a table record; otherwise a custom
/// model path selects that model at sequence 0 (`0x558f8f` to `0x558fb8`).
pub fn select(
    fields: &LoadingScreenFields<'_>,
    table: &ScreenTable,
) -> Result<Selection, LoadingError> {
    if let Some(index) = fields.background {
        let record = table.get(index).ok_or(LoadingError::UnknownScreen {
            index,
            count: table.len(),
        })?;
        return Ok(Selection::Campaign(Screen {
            model: record.model.clone(),
            sequence: record.sequence,
            origin: Origin::Table {
                index,
                name_key: record.name_key.clone(),
            },
        }));
    }
    Ok(match fields.custom_model {
        Some(model) => Selection::Campaign(Screen {
            model: model.to_owned(),
            sequence: 0,
            origin: Origin::Custom,
        }),
        None => Selection::Melee,
    })
}

/// Displayed strings of the campaign panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenText {
    pub text: String,
    pub title: String,
    pub subtitle: String,
}

/// Resolve the header strings. A `TRIGSTR_n` key reads the map's string table; any
/// other value is literal. An absent title or subtitle shows nothing.
pub fn resolve_text(
    fields: &LoadingScreenFields<'_>,
    strings: Option<&Wts>,
) -> Result<ScreenText, LoadingError> {
    let resolve = |field: &'static str, value: &str| -> Result<String, LoadingError> {
        let Some(id) = value
            .get(..TRIGSTR_PREFIX.len())
            .filter(|prefix| prefix.eq_ignore_ascii_case(TRIGSTR_PREFIX))
            .and_then(|_| value[TRIGSTR_PREFIX.len()..].parse::<u32>().ok())
        else {
            return Ok(value.to_owned());
        };
        let key = value.to_owned();
        let strings = strings.ok_or_else(|| LoadingError::MissingString {
            field,
            key: key.clone(),
        })?;
        if !strings.contains(id) {
            return Err(LoadingError::MissingString { field, key });
        }
        strings
            .decode_utf8(id)
            .map(str::to_owned)
            .map_err(|_| LoadingError::BadString { field, key })
    };
    Ok(ScreenText {
        text: resolve("loading text", fields.text)?,
        title: resolve("loading title", fields.title.unwrap_or_default())?,
        subtitle: resolve("loading subtitle", fields.subtitle.unwrap_or_default())?,
    })
}

/// Bar constants read from the reviewed image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressRules {
    /// Displayed progress gain per update while more than this far behind.
    pub fast_step: f32,
    /// Gain per update otherwise.
    pub slow_step: f32,
    /// Largest displayed progress, which is also the end of the bar sequence.
    pub full: f32,
}

impl ProgressRules {
    /// Read the constants from the reviewed Legacy image; any other image refuses.
    pub fn read(image: &[u8]) -> Result<Self, LoadingError> {
        if image.len() > MAX_IMAGE_BYTES || sha256(image) != REVIEWED_IMAGE {
            return Err(LoadingError::Image("unsupported client image"));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: MAX_IMAGE_BYTES,
                profile_bytes: 128,
                sections: MAX_IMAGE_SECTIONS,
            },
        )
        .map_err(|_| LoadingError::Image("invalid PE image"))?;
        Self::decode(|va| {
            let bytes = pe.read_from(va, false).ok()?.get(..4)?.try_into().ok()?;
            Some(f32::from_le_bytes(bytes))
        })
    }

    fn decode(read: impl Fn(u32) -> Option<f32>) -> Result<Self, LoadingError> {
        let value = |va| read(va).ok_or(LoadingError::Image("missing bar constant"));
        let rules = Self {
            fast_step: value(FAST_STEP_VA)?,
            slow_step: value(SLOW_STEP_VA)?,
            full: value(FULL_VA)?,
        };
        let ordered = 0.0 < rules.slow_step
            && rules.slow_step < rules.fast_step
            && rules.fast_step < rules.full;
        if ![rules.fast_step, rules.slow_step, rules.full]
            .iter()
            .all(|v| v.is_finite())
            || !ordered
        {
            return Err(LoadingError::Image("bar constants are not an ordered ramp"));
        }
        Ok(rules)
    }
}

/// Displayed and target progress of the bar, in the original's units where
/// [`ProgressRules::full`] is the end.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Progress {
    displayed: f32,
    target: f32,
}

impl Progress {
    /// A stage report (`0x54fcc0`): a display that lags the old target catches
    /// up to it, then the target rises by `delta`. Zero reports do nothing.
    pub fn report(&mut self, delta: f32) {
        if delta == 0.0 {
            return;
        }
        if self.target > self.displayed {
            self.displayed = self.target;
        }
        self.target += delta;
    }

    /// One bar update (`0x549c10`): the display gains the fast step while it
    /// trails the target by more than that step, the slow step otherwise, so a
    /// level display still creeps toward the end.
    pub fn step(&mut self, rules: &ProgressRules) {
        let behind = self.target - self.displayed;
        self.displayed += if behind > rules.fast_step {
            rules.fast_step
        } else {
            rules.slow_step
        };
    }

    /// The load-complete handler puts the bar at its end (`0x96cd30`).
    pub fn finish(&mut self, rules: &ProgressRules) {
        self.displayed = rules.full;
        self.target = self.target.max(rules.full);
    }

    /// Bar position in `0.0..=1.0`: the displayed value up to [`ProgressRules::full`].
    pub fn fraction(&self, rules: &ProgressRules) -> f32 {
        self.displayed.min(rules.full) / rules.full
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
[Other]
NumScreens=1
00=9,X,9,X.mdl
// comment
[LoadingScreens]
NumScreens=3
00=0,WESTRING_A,0,UI\\Loading\\A.mdl
01=0,WESTRING_B,1, UI\\Loading\\A.mdl
// 02 has a comment above it
02=1,WESTRING_C,4,UI\\Loading\\C.mdl
[PrologueScreens]
NumScreens=1
00=0,WESTRING_UNKNOWN,0,UI\\Loading\\P.mdl
";

    fn fields<'a>(
        background: Option<u32>,
        custom_model: Option<&'a str>,
        text: &'a str,
    ) -> LoadingScreenFields<'a> {
        LoadingScreenFields {
            background,
            custom_model,
            text,
            title: Some("T"),
            subtitle: None,
        }
    }

    #[test]
    fn the_section_is_parsed_by_name_and_count() {
        let table = ScreenTable::parse(TABLE).unwrap();
        assert_eq!(table.len(), 3);
        let record = table.get(1).unwrap();
        assert_eq!(record.name_key, "WESTRING_B");
        assert_eq!(record.sequence, 1);
        assert_eq!(record.model, "UI\\Loading\\A.mdl");
        assert_eq!(table.get(2).unwrap().first_version, 1);
        assert!(table.get(3).is_none());
    }

    #[test]
    fn a_missing_or_malformed_listed_record_refuses() {
        assert_eq!(
            ScreenTable::parse("[Other]\nNumScreens=1\n"),
            Err(LoadingError::NoSection)
        );
        for (text, key) in [
            ("[LoadingScreens]\nNumScreens=2\n00=0,N,0,M.mdl\n", "01"),
            ("[LoadingScreens]\nNumScreens=1\n00=0,N,x,M.mdl\n", "00"),
            ("[LoadingScreens]\nNumScreens=1\n00=0,N,0\n", "00"),
            ("[LoadingScreens]\nNumScreens=1\n00=0,,0,M.mdl\n", "00"),
            ("[LoadingScreens]\n00=0,N,0,M.mdl\n", COUNT_KEY),
        ] {
            assert!(
                matches!(ScreenTable::parse(text), Err(LoadingError::Record { key: k, .. }) if k == key),
                "{text}"
            );
        }
    }

    #[test]
    fn a_number_selects_its_record_and_wins_over_a_custom_model() {
        let table = ScreenTable::parse(TABLE).unwrap();
        let selected = select(&fields(Some(2), Some("Own.mdl"), ""), &table).unwrap();
        assert_eq!(
            selected,
            Selection::Campaign(Screen {
                model: "UI\\Loading\\C.mdl".into(),
                sequence: 4,
                origin: Origin::Table {
                    index: 2,
                    name_key: "WESTRING_C".into()
                }
            })
        );
    }

    #[test]
    fn a_custom_model_plays_its_first_sequence_and_no_header_is_melee() {
        let table = ScreenTable::parse(TABLE).unwrap();
        assert_eq!(
            select(&fields(None, Some("Own.mdl"), ""), &table).unwrap(),
            Selection::Campaign(Screen {
                model: "Own.mdl".into(),
                sequence: 0,
                origin: Origin::Custom
            })
        );
        assert_eq!(
            select(&fields(None, None, ""), &table).unwrap(),
            Selection::Melee
        );
    }

    #[test]
    fn an_unknown_number_refuses_instead_of_falling_back() {
        let table = ScreenTable::parse(TABLE).unwrap();
        assert_eq!(
            select(&fields(Some(3), Some("Own.mdl"), ""), &table),
            Err(LoadingError::UnknownScreen { index: 3, count: 3 })
        );
    }

    fn strings() -> Wts {
        Wts::parse(b"STRING 7\r\n{\r\nThe Defense\r\n}\r\n\r\nSTRING 8\r\n{\r\n\xff\r\n}\r\n").unwrap()
    }

    #[test]
    fn keys_resolve_through_the_map_strings_and_literals_pass_through() {
        let wts = strings();
        let mut f = fields(None, None, "plain text");
        f.title = Some("trigstr_7");
        let text = resolve_text(&f, Some(&wts)).unwrap();
        assert_eq!(text.text, "plain text");
        assert_eq!(text.title, "The Defense");
        assert_eq!(text.subtitle, "");
    }

    #[test]
    fn an_unresolvable_key_refuses() {
        let wts = strings();
        let mut f = fields(None, None, "TRIGSTR_9");
        assert!(matches!(
            resolve_text(&f, Some(&wts)),
            Err(LoadingError::MissingString { field: "loading text", .. })
        ));
        assert!(matches!(
            resolve_text(&f, None),
            Err(LoadingError::MissingString { .. })
        ));
        f.text = "TRIGSTR_8";
        assert!(matches!(
            resolve_text(&f, Some(&wts)),
            Err(LoadingError::BadString { .. })
        ));
    }

    fn rules() -> ProgressRules {
        ProgressRules {
            fast_step: 0.5,
            slow_step: 0.125,
            full: 2.0,
        }
    }

    #[test]
    fn the_display_trails_the_target_and_creeps_when_level() {
        let rules = rules();
        let mut p = Progress::default();
        p.report(1.0);
        // A gap larger than the fast step takes the fast step.
        p.step(&rules);
        assert_eq!(p.displayed, 0.5);
        // A gap equal to the fast step is not larger, so the slow step applies.
        p.step(&rules);
        assert_eq!(p.displayed, 0.625);
        for _ in 0..3 {
            p.step(&rules);
        }
        assert_eq!(p.displayed, 1.0);
        // Level with the target, the display still creeps past it.
        p.step(&rules);
        assert_eq!(p.displayed, 1.125);
        assert_eq!(p.fraction(&rules), 0.5625);
    }

    #[test]
    fn a_report_first_lets_a_lagging_display_catch_up_and_zero_is_ignored() {
        let mut p = Progress::default();
        p.report(1.0);
        p.report(0.0);
        assert_eq!((p.displayed, p.target), (0.0, 1.0));
        p.report(0.5);
        assert_eq!((p.displayed, p.target), (1.0, 1.5));
    }

    #[test]
    fn the_bar_never_shows_past_its_end_and_finish_fills_it() {
        let rules = rules();
        let mut p = Progress::default();
        p.report(10.0);
        for _ in 0..100 {
            p.step(&rules);
        }
        assert_eq!(p.fraction(&rules), 1.0);
        let mut q = Progress::default();
        q.finish(&rules);
        assert_eq!(q.fraction(&rules), 1.0);
    }

    #[test]
    fn constants_must_form_an_ordered_ramp() {
        let ok = |a: f32, b: f32, c: f32| {
            ProgressRules::decode(|va| match va {
                FAST_STEP_VA => Some(a),
                SLOW_STEP_VA => Some(b),
                FULL_VA => Some(c),
                _ => None,
            })
        };
        assert_eq!(
            ok(0.01, 0.001, 1.0),
            Ok(ProgressRules {
                fast_step: 0.01,
                slow_step: 0.001,
                full: 1.0
            })
        );
        for bad in [(0.001, 0.01, 1.0), (0.01, 0.0, 1.0), (f32::NAN, 0.001, 1.0)] {
            assert!(ok(bad.0, bad.1, bad.2).is_err(), "{bad:?}");
        }
        assert_eq!(
            ProgressRules::decode(|_| None),
            Err(LoadingError::Image("missing bar constant"))
        );
    }

    #[test]
    fn another_image_is_refused() {
        assert_eq!(
            ProgressRules::read(b"not the reviewed image"),
            Err(LoadingError::Image("unsupported client image"))
        );
    }

    /// Original-input gate: `PEACECRAFT3_LEGACY_DIR` is the selected 1.29.2.9231 install.
    #[test]
    #[ignore = "requires PEACECRAFT3_LEGACY_DIR pointing at the selected Legacy install"]
    fn the_selected_image_supplies_the_bar_constants() {
        let dir = std::env::var_os("PEACECRAFT3_LEGACY_DIR")
            .expect("set PEACECRAFT3_LEGACY_DIR to the selected 1.29.2.9231 install");
        let image = std::fs::read(std::path::Path::new(&dir).join("Warcraft III.exe")).unwrap();
        let rules = ProgressRules::read(&image).unwrap();
        println!("{rules:?}");
        assert!(rules.slow_step < rules.fast_step && rules.fast_step < rules.full);
    }
}
