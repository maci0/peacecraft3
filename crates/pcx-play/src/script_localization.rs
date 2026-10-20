//! Selected UI StringList and Hotkeys queries; map WTS remains a separate source.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::{AssetIdentity, ScriptAsset},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub bytes: usize,
    pub entries: usize,
    pub string_bytes: usize,
}
#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    section: String,
    missing_hotkey: i32,
    output_bytes: usize,
}
impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |why: &str| format!("script localization {}: {why}", origin.source);
        if image.len() > 32 << 20
            || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
            || !identity(&origin)
        {
            return Err(fail("unsupported client identity"));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| fail(&e.to_string()))?;
        let read = |va, len, code| -> Result<&[u8], String> {
            pe.read_from(va, code)
                .map_err(|e| fail(&e.to_string()))?
                .get(..len)
                .ok_or_else(|| fail("truncated native window"))
        };
        if read(0x493967, 4, true)? != [0x33, 0xc0, 0x5d, 0xc3] {
            return Err(fail("unsupported null hotkey result"));
        }
        let hotkey = read(0x493954, 9, true)?;
        let output = read(0x4939cd, 5, true)?;
        let (missing_hotkey, pointer, output_bytes) = operands(hotkey, output).map_err(fail)?;
        let pointer = u32::from_le_bytes(read(pointer, 4, false)?.try_into().unwrap());
        let section = pe
            .read_from(pointer, false)
            .map_err(|e| fail(&e.to_string()))?;
        let end = section
            .iter()
            .take(64)
            .position(|b| *b == 0)
            .ok_or_else(|| fail("unterminated hotkey section"))?;
        let section =
            std::str::from_utf8(&section[..end]).map_err(|_| fail("invalid hotkey section"))?;
        if section.is_empty() || !section.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(fail("unsupported hotkey section"));
        }
        Ok(Self {
            origin,
            image_sha256: sha256(image),
            section: section.into(),
            missing_hotkey,
            output_bytes,
        })
    }
}
fn identity(origin: &AssetIdentity) -> bool {
    [&origin.source, &origin.edition, &origin.version]
        .iter()
        .all(|s| !s.trim().is_empty())
}
fn operands(hotkey: &[u8], output: &[u8]) -> Result<(i32, u32, usize), &'static str> {
    if hotkey.len() != 9
        || hotkey[..1] != [0x6a]
        || hotkey[2] != 0x50
        || hotkey[3..5] != [0xff, 0x35]
        || output.len() != 5
        || output[0] != 0x68
    {
        return Err("unsupported localization getter operands");
    }
    let capacity = u32::from_le_bytes(output[1..].try_into().unwrap()) as usize;
    if capacity < 2 {
        return Err("invalid localization output bound");
    }
    Ok((
        i32::from(hotkey[1] as i8),
        u32::from_le_bytes(hotkey[5..].try_into().unwrap()),
        capacity,
    ))
}
#[derive(Clone, Debug)]
pub struct Receipt {
    pub label: String,
    pub origin: AssetIdentity,
    pub sha256: Sha256Digest,
}
#[derive(Clone, Debug)]
pub struct Bank {
    pub strings: Receipt,
    pub hotkeys: Receipt,
    pub rules: Rules,
    labels: BTreeMap<String, String>,
    keys: BTreeMap<String, i32>,
}
impl Bank {
    pub fn read(
        strings: ScriptAsset<'_>,
        strings_origin: AssetIdentity,
        hotkeys: ScriptAsset<'_>,
        hotkeys_origin: AssetIdentity,
        rules: Rules,
        budget: Budget,
    ) -> Result<Self, String> {
        let fail = |why: &str| {
            format!(
                "script localization {} / {}: {why}",
                strings.label, hotkeys.label
            )
        };
        if !identity(&strings_origin)
            || !identity(&hotkeys_origin)
            || strings.label.trim().is_empty()
            || hotkeys.label.trim().is_empty()
        {
            return Err(fail("missing asset identity"));
        }
        if strings
            .bytes
            .len()
            .checked_add(hotkeys.bytes.len())
            .is_none_or(|n| n > budget.bytes)
        {
            return Err(fail("input byte budget"));
        }
        let labels = string_list(input_text(strings)?, budget)
            .map_err(|e| format!("{}: {e}", strings.label))?;
        let keys = hotkey_list(input_text(hotkeys)?, &rules.section, budget)
            .map_err(|e| format!("{}: {e}", hotkeys.label))?;
        if labels
            .len()
            .checked_add(keys.len())
            .is_none_or(|n| n > budget.entries)
        {
            return Err(fail("combined entry budget"));
        }
        let receipt = |asset: ScriptAsset<'_>, origin| Receipt {
            label: asset.label.into(),
            origin,
            sha256: sha256(asset.bytes),
        };
        Ok(Self {
            strings: receipt(strings, strings_origin),
            hotkeys: receipt(hotkeys, hotkeys_origin),
            rules,
            labels,
            keys,
        })
    }
    pub fn localized_string(&self, key: &str) -> Result<Option<String>, String> {
        if key.contains('\0') {
            return Err(format!("{}: embedded NUL in lookup", self.strings.label));
        }
        if key.is_empty() {
            return Ok(None);
        }
        if key
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("TRIGSTR_"))
        {
            return Err(format!(
                "{}: native dynamic TRIGSTR lookup requires the selected map WTS context",
                self.strings.label
            ));
        }
        let value = self
            .labels
            .get(key)
            .filter(|v| !v.is_empty())
            .map_or(key, String::as_str);
        // Native copies at most capacity-1 encoded bytes. Split UTF-8 would require a byte-string VM.
        let max = self.rules.output_bytes - 1;
        let value = if self.labels.get(key).is_some_and(|v| !v.is_empty()) && value.len() > max {
            if !value.is_char_boundary(max) {
                return Err(format!(
                    "{}: native output bound splits UTF-8",
                    self.strings.label
                ));
            }
            &value[..max]
        } else {
            value
        };
        Ok(Some(value.into()))
    }
    pub fn localized_hotkey(&self, key: &str) -> Result<i32, String> {
        if key.is_empty() {
            return Ok(0);
        }
        if !key.is_ascii() || key.contains('\0') {
            return Err(format!(
                "{}: non-ASCII hotkey comparison is not qualified",
                self.hotkeys.label
            ));
        }
        Ok(self
            .keys
            .get(&key.to_ascii_lowercase())
            .copied()
            .unwrap_or(self.rules.missing_hotkey))
    }
}
fn input_text<'a>(asset: ScriptAsset<'a>) -> Result<&'a str, String> {
    std::str::from_utf8(asset.bytes).map_err(|e| format!("{}: {e}", asset.label))
}
struct Tokens<'a> {
    text: &'a str,
    at: usize,
    max: usize,
}
impl<'a> Tokens<'a> {
    fn space(&mut self) -> Result<(), String> {
        loop {
            while self
                .text
                .as_bytes()
                .get(self.at)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.at += 1
            }
            let rest = &self.text[self.at..];
            if rest.starts_with("//") {
                self.at += rest.find('\n').unwrap_or(rest.len());
            } else if rest.starts_with("/*") {
                self.at += rest.find("*/").ok_or("unterminated block comment")? + 2;
            } else {
                return Ok(());
            }
        }
    }
    fn symbol(&mut self, byte: u8) -> Result<(), String> {
        self.space()?;
        if self.text.as_bytes().get(self.at) != Some(&byte) {
            return Err(format!("expected {} at byte {}", char::from(byte), self.at));
        }
        self.at += 1;
        Ok(())
    }
    fn word(&mut self) -> Result<String, String> {
        self.space()?;
        let start = self.at;
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            self.at += 1
        }
        if self.at == start || self.at - start > self.max {
            return Err("missing/oversized StringList identifier".into());
        }
        Ok(self.text[start..self.at].into())
    }
    fn quoted(&mut self) -> Result<String, String> {
        self.symbol(b'"')?;
        let mut value = String::new();
        loop {
            let c = self.text[self.at..]
                .chars()
                .next()
                .ok_or("unterminated StringList string")?;
            self.at += c.len_utf8();
            match c {
                '"' => return Ok(value),
                '\\' => {
                    let c = self.text[self.at..]
                        .chars()
                        .next()
                        .ok_or("unterminated escape")?;
                    self.at += c.len_utf8();
                    match c {
                        '"' | '\\' => value.push(c),
                        _ => return Err(format!("unsupported StringList escape {c:?}")),
                    }
                }
                '\0' => return Err("embedded NUL".into()),
                _ => value.push(c),
            }
            if value.len() > self.max {
                return Err("string byte budget".into());
            }
        }
    }
}
fn string_list(text: &str, budget: Budget) -> Result<BTreeMap<String, String>, String> {
    let mut t = Tokens {
        text: text.strip_prefix('\u{feff}').unwrap_or(text),
        at: 0,
        max: budget.string_bytes,
    };
    let mut out = BTreeMap::new();
    if t.word()? != "StringList" {
        return Err("expected StringList".into());
    }
    t.symbol(b'{')?;
    loop {
        t.space()?;
        if t.text.as_bytes().get(t.at) == Some(&b'}') {
            t.at += 1;
            break;
        }
        if out.len() >= budget.entries {
            return Err("entry budget".into());
        }
        let key = t.word()?;
        let value = t.quoted()?;
        t.symbol(b',')?;
        if out.insert(key, value).is_some() {
            return Err("duplicate StringList identifier".into());
        }
    }
    t.space()?;
    if t.at != t.text.len() {
        return Err("unsupported trailing StringList content".into());
    }
    Ok(out)
}
fn hotkey_list(text: &str, section: &str, budget: Budget) -> Result<BTreeMap<String, i32>, String> {
    let mut active = false;
    let mut seen = false;
    let mut out = BTreeMap::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let mut quoted = false;
        let mut end = line.len();
        let bytes = line.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at] == b'\'' {
                quoted = !quoted
            }
            if !quoted && bytes[at..].starts_with(b"//") {
                end = at;
                break;
            }
            at += 1
        }
        let line = line[..end].trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[') {
            let name = name.strip_suffix(']').ok_or("malformed profile section")?;
            active = name.eq_ignore_ascii_case(section);
            if active && std::mem::replace(&mut seen, true) {
                return Err("duplicate hotkey section".into());
            }
            continue;
        }
        if !active {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or("malformed hotkey entry")?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || key.len() > budget.string_bytes
        {
            return Err("invalid hotkey identifier".into());
        }
        if out.len() >= budget.entries || value.len() > budget.string_bytes {
            return Err("hotkey budget".into());
        }
        let number = if let Some(c) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
            if c.len() != 1 || !c.is_ascii() {
                return Err("unsupported hotkey character literal".into());
            }
            i32::from(c.as_bytes()[0])
        } else if let Some(hex) = value.strip_prefix("0x") {
            i32::from_str_radix(hex, 16).map_err(|_| "invalid hotkey integer")?
        } else {
            value.parse::<i32>().map_err(|_| "invalid hotkey integer")?
        };
        if out.insert(key.to_ascii_lowercase(), number).is_some() {
            return Err("duplicate hotkey identifier".into());
        }
    }
    if !seen {
        return Err(format!("missing profile section {section}"));
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic:UI".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        }
    }
    pub(crate) fn budget() -> Budget {
        Budget {
            bytes: 8192,
            entries: 64,
            string_bytes: 512,
        }
    }
    pub(crate) fn rules() -> Rules {
        Rules {
            origin: origin(),
            image_sha256: [0; 32],
            section: "HotKeys".into(),
            missing_hotkey: 17,
            output_bytes: 64,
        }
    }
    pub(crate) fn bank() -> Bank {
        Bank::read(
            ScriptAsset {
                label: "fixture:StringList",
                bytes: br#"StringList{GAMEOVER_RESTART "|cffffffffR|restart",EMPTY "",}"#,
            },
            origin(),
            ScriptAsset {
                label: "fixture:keys",
                bytes: b"[hOtKeYs]\nGAMEOVER_RESTART='R'\n",
            },
            origin(),
            rules(),
            budget(),
        )
        .unwrap()
    }
    #[test]
    fn localization_preserves_source_text_case_and_native_fallbacks() {
        let bank = bank();
        assert_eq!(
            bank.localized_string("GAMEOVER_RESTART")
                .unwrap()
                .as_deref(),
            Some("|cffffffffR|restart")
        );
        assert_eq!(
            bank.localized_string("gameover_restart")
                .unwrap()
                .as_deref(),
            Some("gameover_restart")
        );
        assert_eq!(
            bank.localized_string("EMPTY").unwrap().as_deref(),
            Some("EMPTY")
        );
        assert_eq!(bank.localized_string("").unwrap(), None);
        assert_eq!(
            bank.localized_hotkey("gameover_RESTART").unwrap(),
            i32::from(b'R')
        );
        assert_eq!(bank.localized_hotkey("MISSING").unwrap(), 17);
        assert!(
            bank.localized_string("TRIGSTR_18")
                .unwrap_err()
                .contains("WTS")
        );
    }
    #[test]
    fn string_list_handles_comments_bom_escaped_quotes_and_backslashes() {
        let source = "\u{feff}/* header */ StringList { // line\n A \"say \\\"hello\\\" \\\\ path\", B \" // literal /* value */\", } /* end */";
        let values = string_list(source, budget()).unwrap();
        assert_eq!(values["A"], "say \"hello\" \\ path");
        assert_eq!(values["B"], " // literal /* value */");
        for source in [
            "StringList{A \"one\", A \"two\",}",
            "StringList{A \"unterminated}",
            "StringList{A \"x\"}",
            "StringList{} garbage",
            "/* open",
            r#"StringList{A "bad\q",}"#,
        ] {
            assert!(string_list(source, budget()).is_err(), "{source}")
        }
    }
    #[test]
    fn localization_budgets_duplicates_and_hotkey_literals_are_checked() {
        assert!(
            string_list(
                "StringList{A \"abc\",}",
                Budget {
                    string_bytes: 2,
                    ..budget()
                }
            )
            .is_err()
        );
        assert!(
            string_list(
                "StringList{A \"a\",}",
                Budget {
                    entries: 0,
                    ..budget()
                }
            )
            .is_err()
        );
        for profile in [
            "[HotKeys]\nA='R'\na='Q'",
            "[Hotkeys]\nA='R'\n[HOTKEYS]\nB='Q'",
            "[Other]\nA='R'",
            "[Hotkeys]\nA='RR'",
            "[Hotkeys]\nA=oops",
        ] {
            assert!(
                hotkey_list(profile, "HotKeys", budget()).is_err(),
                "{profile}"
            )
        }
        let keys = hotkey_list(
            "[Other]\nIgnored=invalid\n[HOTKEYS]\nA='/' // comment\nB=71\nC=0x48\n",
            "HotKeys",
            budget(),
        )
        .unwrap();
        assert_eq!((keys["a"], keys["b"], keys["c"]), (47, 71, 72));
        let b = bank();
        assert_eq!(b.strings.origin, origin());
        assert_ne!(b.strings.sha256, b.hotkeys.sha256);
        assert!(
            Bank::read(
                ScriptAsset {
                    label: "s",
                    bytes: b"StringList{}"
                },
                origin(),
                ScriptAsset {
                    label: "h",
                    bytes: b"[Hotkeys]"
                },
                origin(),
                rules(),
                Budget {
                    bytes: 1,
                    ..budget()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn localization_output_bound_applies_to_found_text_and_preserves_input_fallback() {
        let mut bank = bank();
        bank.rules.output_bytes = 5;
        bank.labels.insert("LONG".into(), "abcdef".into());
        assert_eq!(
            bank.localized_string("LONG").unwrap().as_deref(),
            Some("abcd")
        );
        assert_eq!(
            bank.localized_string("MISSING_LONG").unwrap().as_deref(),
            Some("MISSING_LONG")
        );
        bank.labels.insert("UTF".into(), "xx☃".into());
        assert!(bank.localized_string("UTF").unwrap_err().contains("UTF-8"));
    }
    #[test]
    fn localization_native_operands_follow_synthetic_instructions() {
        let mut hotkey = vec![0x6a, 9, 0x50, 0xff, 0x35];
        hotkey.extend(0x12345678u32.to_le_bytes());
        let mut output = vec![0x68];
        output.extend(31u32.to_le_bytes());
        assert_eq!(operands(&hotkey, &output), Ok((9, 0x12345678, 31)));
        for n in 0..hotkey.len() {
            assert!(operands(&hotkey[..n], &output).is_err());
        }
        for n in 0..output.len() {
            assert!(operands(&hotkey, &output[..n]).is_err());
        }
        hotkey[0] = 0x90;
        assert!(operands(&hotkey, &output).is_err());
        assert!(
            Rules::reviewed_legacy(&[], origin())
                .unwrap_err()
                .contains("synthetic:UI")
        );
    }
}
