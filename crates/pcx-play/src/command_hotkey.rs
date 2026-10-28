//! Command-card hotkeys as the client stores and compares them, and the optional
//! `CustomKeys.txt` overrides. Evidence: `docs/re-notes/input-command-keys.md`.
//!
//! The client reads each `Hotkey`, `Unhotkey` and `Researchhotkey` profile field as a
//! comma list with one value per rank (`0x5c2470`). A value of digits only is a number;
//! anything else contributes the code of its first character. Letters are not case
//! folded: a stored code is compared to the key's Windows virtual-key code (`0x638ed0`),
//! so a letter hotkey is an uppercase letter, as `CustomKeyInfo.txt` states.
use std::collections::BTreeMap;

/// Virtual-key code of Escape.
pub const ESCAPE_KEY: u32 = 0x1B;
/// The stored cancel code of `CustomKeysSample.txt`. The client matches Escape to it
/// (`0x638ed0`: stored `0x200` equals key `0x1b`) as it does to the stored `27` of
/// `CommandStrings.txt`.
const CANCEL_ALIAS: u32 = 0x200;

/// A hotkey code the card compares with a pressed key's virtual-key code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey(u32);

impl Hotkey {
    /// The stored code of a one-letter hotkey, for sources that already hold a letter.
    pub fn letter(letter: char) -> Result<Self, String> {
        if letter.is_ascii_uppercase() {
            Ok(Self(u32::from(letter)))
        } else {
            Err(format!("hotkey {letter:?} is not an uppercase letter"))
        }
    }

    pub fn code(self) -> u32 {
        self.0
    }

    /// Whether the key with virtual-key code `key` triggers this hotkey.
    pub fn matches(self, key: u32) -> bool {
        self.0 == key || (self.0 == CANCEL_ALIAS && key == ESCAPE_KEY)
    }
}

/// Which profile field a hotkey comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    Hotkey,
    Unhotkey,
    Researchhotkey,
}

impl Field {
    const ALL: [Self; 3] = [Self::Hotkey, Self::Unhotkey, Self::Researchhotkey];

    pub fn name(self) -> &'static str {
        match self {
            Self::Hotkey => "Hotkey",
            Self::Unhotkey => "Unhotkey",
            Self::Researchhotkey => "Researchhotkey",
        }
    }

    fn named(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|field| field.name().eq_ignore_ascii_case(key))
    }
}

/// One profile value: a decimal code, or the first character's code. An empty value is no
/// hotkey for that rank.
fn parse_value(value: &str) -> Result<Option<Hotkey>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.bytes().all(|byte| byte.is_ascii_digit()) {
        return value
            .parse::<u32>()
            .map(|code| Some(Hotkey(code)))
            .map_err(|_| format!("hotkey code {value:?} is out of range"));
    }
    let mut chars = value.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), None) if letter.is_ascii_graphic() => Ok(Some(Hotkey(u32::from(letter)))),
        _ => Err(format!(
            "hotkey {value:?} is neither a code nor one character"
        )),
    }
}

/// A `Hotkey`-style profile field: one entry per rank.
pub fn parse_ranked(text: &str) -> Result<Vec<Option<Hotkey>>, String> {
    text.split(',').map(parse_value).collect()
}

/// The hotkey of rank index `rank` (the last entry serves higher ranks).
pub fn at_rank(ranked: &[Option<Hotkey>], rank: usize) -> Option<Hotkey> {
    ranked.get(rank).or(ranked.last()).copied().flatten()
}

/// Entries of `CustomKeys.txt`: section name and field to the per-rank hotkeys.
///
/// Only the three hotkey fields are read. Button positions and tooltips of the file are
/// not applied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomKeys {
    entries: BTreeMap<(String, Field), Vec<Option<Hotkey>>>,
}

impl CustomKeys {
    /// Parses the file. Lowercase letters are refused: the client does not fold case, and
    /// `CustomKeyInfo.txt` requires uppercase entries.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut entries = BTreeMap::new();
        let mut section: Option<String> = None;
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            let fail = |why: String| format!("CustomKeys.txt line {}: {why}", index + 1);
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            if let Some(name) = line.strip_prefix('[') {
                let name = name
                    .strip_suffix(']')
                    .ok_or_else(|| fail("unterminated section header".into()))?;
                section = Some(name.trim().to_ascii_lowercase());
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(fail("expected Key=Value".into()));
            };
            let Some(field) = Field::named(key.trim()) else {
                continue;
            };
            let Some(section) = &section else {
                return Err(fail(format!("{} outside a section", field.name())));
            };
            if value.chars().any(|c| c.is_ascii_lowercase()) {
                return Err(fail(format!(
                    "{}={} must be uppercase",
                    field.name(),
                    value.trim()
                )));
            }
            let ranked = parse_ranked(value).map_err(fail)?;
            if entries.insert((section.clone(), field), ranked).is_some() {
                return Err(fail(format!(
                    "[{section}] {} is set twice",
                    field.name()
                )));
            }
        }
        Ok(Self { entries })
    }

    /// The overriding hotkeys of `section` (an object code or a `Cmd` profile name).
    pub fn get(&self, section: &str, field: Field) -> Option<&[Option<Hotkey>]> {
        self.entries
            .get(&(section.to_ascii_lowercase(), field))
            .map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: u32) -> Option<Hotkey> {
        Some(Hotkey(code))
    }

    #[test]
    fn values_are_a_code_for_digits_and_the_first_character_otherwise() {
        assert_eq!(parse_ranked("Q").unwrap(), [key(0x51)]);
        assert_eq!(parse_ranked("27").unwrap(), [key(27)]);
        assert_eq!(parse_ranked("512").unwrap(), [key(512)]);
        assert_eq!(parse_ranked("X, Y,Z").unwrap(), [key(0x58), key(0x59), key(0x5A)]);
        assert_eq!(parse_ranked("").unwrap(), [None]);
        assert_eq!(parse_ranked("B,,C").unwrap(), [key(0x42), None, key(0x43)]);
        // The client does not fold case: lowercase `s` is the code of F4.
        assert_eq!(parse_ranked("s").unwrap(), [key(0x73)]);
        assert!(parse_ranked("ESC").is_err());
        assert!(parse_ranked("99999999999").is_err());
        assert!(parse_ranked("é").is_err());
    }

    #[test]
    fn the_cancel_codes_both_match_escape_and_letters_match_their_code() {
        for stored in [27, 512] {
            assert!(Hotkey(stored).matches(ESCAPE_KEY), "{stored}");
        }
        assert!(!Hotkey(27).matches(0x51));
        assert!(!Hotkey(512).matches(0x51));
        assert!(Hotkey::letter('Q').unwrap().matches(0x51));
        assert!(!Hotkey::letter('Q').unwrap().matches(0x57));
        assert!(Hotkey::letter('q').is_err());
    }

    #[test]
    fn a_rank_past_the_list_uses_the_last_entry() {
        let ranked = parse_ranked("X,Y,Z").unwrap();
        assert_eq!(at_rank(&ranked, 0), key(0x58));
        assert_eq!(at_rank(&ranked, 2), key(0x5A));
        assert_eq!(at_rank(&ranked, 9), key(0x5A));
        assert_eq!(at_rank(&[], 0), None);
        assert_eq!(at_rank(&parse_ranked("").unwrap(), 0), None);
    }

    #[test]
    fn the_file_overrides_by_section_and_field_ignoring_case_bom_and_other_keys() {
        let text = "\u{feff}// comment\n[ogru]\nHotkey=T\nTip=Train Orc Grunt (T)\n\n[Rhme]\nhotkey = X,Y,Z\n[Adef]\nHotkey=A\nUnhotkey=B\n[AHre]\nResearchhotkey=U\n[CmdCancel]\nHotkey=512\n";
        let keys = CustomKeys::parse(text).unwrap();
        assert_eq!(keys.get("OGRU", Field::Hotkey), Some(&[key(0x54)][..]));
        assert_eq!(
            keys.get("rhme", Field::Hotkey),
            Some(&[key(0x58), key(0x59), key(0x5A)][..])
        );
        assert_eq!(keys.get("Adef", Field::Unhotkey), Some(&[key(0x42)][..]));
        assert_eq!(keys.get("AHre", Field::Researchhotkey), Some(&[key(0x55)][..]));
        assert_eq!(keys.get("CmdCancel", Field::Hotkey), Some(&[key(512)][..]));
        assert_eq!(keys.get("ogru", Field::Unhotkey), None);
        assert_eq!(keys.get("hfoo", Field::Hotkey), None);
    }

    #[test]
    fn a_malformed_file_refuses_with_its_line() {
        for (text, line) in [
            ("[ogru\nHotkey=T\n", "line 1"),
            ("Hotkey=T\n", "line 1"),
            ("[ogru]\nHotkey=t\n", "line 2"),
            ("[ogru]\nHotkey=T\nHotkey=Y\n", "line 3"),
            ("[ogru]\nHotkey=TT\n", "line 2"),
            ("[ogru]\nnonsense\n", "line 2"),
        ] {
            let error = CustomKeys::parse(text).unwrap_err();
            assert!(error.contains(line), "{text:?}: {error}");
        }
        assert_eq!(CustomKeys::parse("").unwrap(), CustomKeys::default());
        assert_eq!(
            CustomKeys::parse("[ogru]\nTip=Train\n").unwrap(),
            CustomKeys::default()
        );
    }
}
