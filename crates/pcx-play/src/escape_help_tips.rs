//! In-game Help/Tips content and TEXTAREA model. Headless, no Bevy.
//!
//! Asset provenance (read at runtime by the integrator; tests use synthetic
//! fixtures only):
//! - `UI\FrameDef\UI\EscMenuMainPanel.fdf`: `HelpPanel` 0.432x0.384,
//!   `HelpTextArea` 0.362x0.264 under `HelpTitleText`; `TipsPanel` 0.432x0.284,
//!   `TipsTextArea` 0.362x0.147; Tips buttons Back/Next/OK with `TabFocusPush`.
//! - `UI\FrameDef\UI\EscMenuTemplates.fdf`: `EscMenuTextAreaTemplate` has
//!   `TextAreaLineHeight 0.015`, `TextAreaInset 0.01`, a `SCROLLBAR` child and
//!   a `BACKDROP` child; `FrameFont` is commented out.
//! - `UI\HelpStrings.txt`: single HELP body with `|C…|R` markup.
//! - `UI\TipStrings.txt`: `[General] TipCount=N` then `Tip1..TipN` quoted.
//! - Font for TEXTAREA: the template names no `FrameFont`; the engine falls
//!   back through the parent chain to the default font slot (live-qualified
//!   for the selected RoC run, see `docs/re-notes/escape-help-tips.md`).
//!   That slot is overridden at startup from `UI\MiscUI.txt` `[Misc]`
//!   `FontTexture`, resolved through the image's path-prefix/format operands
//!   (`Fonts` + `%s\\%s`), with native height `table[slot] * axis` from the
//!   image's default-height table and axis factor — see `DefaultFont`.
//!   No `MasterFont`/`ChatHistory` binding is used.
//! - Input: TEXTAREA is read-only display; Escape/Enter close via the modal
//!   parent. No editor caret is kept.
//!
//! Shared wiring (`campaign_escape_menu`, `glue_menu`, Bevy spawn/input) lives
//! with the integrator; this module owns content, navigation and geometry
//! math only. Evidence: `docs/re-notes/escape-help-tips.md`.

use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

/// Read bound for the small text assets.
pub const MAX_TEXT_BYTES: usize = 1 << 20;

/// Parsed `UI\HelpStrings.txt`: the whole file is the HELP body, stored verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpStrings {
    /// Raw body, including `|C…|R` markup and newlines, exactly as authored.
    pub body: String,
}

impl HelpStrings {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.is_empty() {
            return Err("UI\\HelpStrings.txt: empty HELP body".into());
        }
        if text.contains('\0') {
            return Err("UI\\HelpStrings.txt: NUL byte".into());
        }
        Ok(Self {
            body: text.to_string(),
        })
    }
}

/// Parsed `UI\TipStrings.txt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TipStrings {
    /// Tips in authored order, `Tip1` first. Markup kept verbatim.
    pub tips: Vec<String>,
}

impl TipStrings {
    pub fn parse(text: &str) -> Result<Self, String> {
        const FILE: &str = "UI\\TipStrings.txt";
        let mut count: Option<usize> = None;
        let mut tips: Vec<(usize, String)> = Vec::new();
        let mut seen_general = false;
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                if line.eq_ignore_ascii_case("[General]") {
                    if seen_general {
                        return Err(format!("{FILE}: duplicate [General]"));
                    }
                    seen_general = true;
                    continue;
                }
                // Unsupported tip sections are refused explicitly; the native
                // section-table policy is not established by a single-section file.
                return Err(format!("{FILE}: unsupported section {line}"));
            }
            if !seen_general {
                return Err(format!("{FILE}: key before [General]: {line}"));
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("{FILE}: malformed line: {line}"));
            };
            let key = key.trim();
            let value = value.trim();
            if key.eq_ignore_ascii_case("TipCount") {
                if count.is_some() {
                    return Err(format!("{FILE}: duplicate TipCount"));
                }
                let n: usize = value
                    .parse()
                    .map_err(|_| format!("{FILE}: invalid TipCount={value}"))?;
                if n == 0 || n > 10_000 {
                    return Err(format!("{FILE}: TipCount out of range: {value}"));
                }
                count = Some(n);
                continue;
            }
            let Some(digits) = key.strip_prefix("Tip").or_else(|| key.strip_prefix("tip")) else {
                return Err(format!("{FILE}: unknown key: {key}"));
            };
            let n: usize = digits
                .parse()
                .map_err(|_| format!("{FILE}: invalid tip key: {key}"))?;
            if n == 0 {
                return Err(format!("{FILE}: Tip0 is invalid"));
            }
            let v = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .ok_or_else(|| format!("{FILE}: Tip{n} not quoted"))?;
            if tips.iter().any(|(i, _)| *i == n) {
                return Err(format!("{FILE}: duplicate Tip{n}"));
            }
            tips.push((n, v.to_string()));
        }
        let n = count.ok_or_else(|| format!("{FILE}: missing TipCount"))?;
        tips.sort_by_key(|(i, _)| *i);
        if tips.len() != n {
            return Err(format!(
                "{FILE}: TipCount={n} but found {} tips",
                tips.len()
            ));
        }
        for (idx, (i, _)) in tips.iter().enumerate() {
            if *i != idx + 1 {
                return Err(format!(
                    "{FILE}: gap: expected Tip{}, found Tip{i}",
                    idx + 1
                ));
            }
        }
        Ok(Self {
            tips: tips.into_iter().map(|(_, v)| v).collect(),
        })
    }
}

/// Retained selected tip across panel show/hide. Wraps both directions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TipSelection {
    /// Zero-based index into `TipStrings::tips`. `0` until first navigation.
    index: usize,
}

impl TipSelection {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn current<'a>(&self, tips: &'a TipStrings) -> Option<&'a str> {
        tips.tips.get(self.index).map(String::as_str)
    }

    /// Next tip, wrapping past the end back to the first.
    pub fn next(&mut self, tips: &TipStrings) {
        if tips.tips.is_empty() {
            self.index = 0;
            return;
        }
        self.index = (self.index + 1) % tips.tips.len();
    }

    /// Previous tip, wrapping before the first back to the last.
    pub fn previous(&mut self, tips: &TipStrings) {
        if tips.tips.is_empty() {
            self.index = 0;
            return;
        }
        self.index = (self.index + tips.tips.len() - 1) % tips.tips.len();
    }

    /// Keep the selection valid after the tip list changes.
    pub fn clamp(&mut self, tips: &TipStrings) {
        if tips.tips.is_empty() || self.index >= tips.tips.len() {
            self.index = 0;
        }
    }
}

/// TEXTAREA row metrics derived from assets, not copied constants.
/// Row height (`line_height`) is the FDF `TextAreaLineHeight`; it is distinct
/// from the wrapping font height in `DefaultFont`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextAreaMetrics {
    /// From `TextAreaLineHeight` in the template.
    pub line_height: f32,
    /// From `TextAreaInset` in the template.
    pub inset: f32,
}

/// Source-derived default font binding (Headless, no Bevy).
///
/// Decoded at runtime from the reviewed client image; every authored value
/// (`.02`-style table entry, `.6`-style axis factor, `Fonts` prefix, format
/// string, `FRIZQT__`-style texture name) comes from image operands or the
/// selected assets, never from constants here.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultFont {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// `MiscUI.txt` section name from image push operand (`Misc`).
    pub section: String,
    /// `MiscUI.txt` key from image push operand (`FontTexture`).
    pub key: String,
    /// Path prefix string from image (`Fonts`).
    pub path_prefix: String,
    /// Path format string from image (`%s\\%s`).
    pub path_format: String,
    /// Value of `[Misc] FontTexture` in the selected `MiscUI.txt`.
    pub font_texture: String,
    /// `path_prefix` + separator from `path_format` + `font_texture`.
    pub font_path: String,
    /// Default slot used by TEXTAREA wrapping, from the image's
    /// `push imm8; call 0x959b90` / `push 0; call 0x959bc0` operands.
    pub wrapping_slot: u32,
    /// Native default-height table entry for `wrapping_slot`.
    pub table_height: f32,
    /// Axis factor operand read by the height conversion (`0xab2ad0`).
    pub axis_factor: f32,
}

impl DefaultFont {
    /// Wrapping font height: `table_height * axis_factor`.
    /// This is NOT the FDF row height (`TextAreaMetrics::line_height`).
    pub fn wrapping_height(&self) -> f32 {
        self.table_height * self.axis_factor
    }

    /// Read from the reviewed client image + selected `MiscUI.txt` bytes.
    /// See `decode` for exact instruction checks. Any mismatch is an error;
    /// no fallback face, slot, or height is substituted.
    pub fn read(image: &[u8], origin: AssetIdentity, misc_text: &str) -> Result<Self, String> {
        let fail = |e: &str| format!("default font {}: {e}", origin.source);
        if image.len() > 32 << 20 {
            return Err(fail("client image exceeds byte budget"));
        }
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|s| s.trim().is_empty())
        {
            return Err(fail("missing source/edition/version provenance"));
        }
        let digest = sha256(image);
        if digest != REVIEWED_IMAGE {
            return Err(fail("unsupported client image; no font values substituted"));
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
        Self::decode(origin.clone(), digest, misc_text, |va, n, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated record at {va:#x}"))
        })
        .map_err(|e| fail(&e))
    }

    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        misc_text: &str,
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        fn operand(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
            size: usize,
        ) -> Result<Vec<u8>, String> {
            let b = read(va, prefix.len() + size, true)?;
            if b.get(..prefix.len()) != Some(prefix) || b.len() != prefix.len() + size {
                return Err(format!("unsupported instruction at {va:#x}"));
            }
            Ok(b[prefix.len()..].to_vec())
        }
        fn word(b: Vec<u8>) -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                b.try_into().map_err(|_| "truncated operand")?,
            ))
        }
        fn string(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            pointer: u32,
        ) -> Result<String, String> {
            let mut text = Vec::new();
            for offset in 0..1024 {
                let b = read(
                    pointer
                        .checked_add(offset)
                        .ok_or("default font string address overflow")?,
                    1,
                    false,
                )?;
                if b[0] == 0 {
                    return String::from_utf8(text).map_err(|e| e.to_string());
                }
                text.push(b[0]);
            }
            Err("default font string budget".into())
        }
        fn pushed_string(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
        ) -> Result<String, String> {
            let p = word(operand(read, va, &[0x68], 4)?)?;
            string(read, p)
        }
        fn imm8(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
        ) -> Result<u8, String> {
            Ok(operand(read, va, &[0x6a], 1)?[0])
        }
        fn call_target(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
        ) -> Result<u32, String> {
            let disp = word(operand(read, va, &[0xe8], 4)?)? as i32;
            Ok(va.wrapping_add(5).wrapping_add_signed(disp))
        }

        // --- Misc/FontTexture override caller: push FontTexture, push Misc ---
        // 0x42eeda push FontTexture ; 0x42eedf push Misc ; 0x42eee4 call 0x430570
        let key = pushed_string(&mut read, 0x42eeda)?;
        let section = pushed_string(&mut read, 0x42eedf)?;
        if call_target(&mut read, 0x42eee4)? != 0x430570 {
            return Err("unsupported FontTexture lookup call".into());
        }
        // 0x42ef01 push 0x104 ; 0x42ef14 call 0x7af9e0 (path resolver)
        if imm8(&mut read, 0x42ef20)? != 0 {
            return Err("unsupported default-font slot override (slot 0)".into());
        }
        if call_target(&mut read, 0x42ef14)? != 0x7af9e0 {
            return Err("unsupported font path resolver call".into());
        }
        if call_target(&mut read, 0x42ef22)? != 0x959bf0 {
            return Err("unsupported default-font override slot 0 call".into());
        }
        if imm8(&mut read, 0x42ef2e)? != 1 {
            return Err("unsupported default-font slot override (slot 1)".into());
        }
        // 0x42ef30 call 0x959bf0 (second slot, same path)
        if call_target(&mut read, 0x42ef30)? != 0x959bf0 {
            return Err("unsupported default-font override slot 1 call".into());
        }
        // --- Path resolver 0x7af9e0: push Fonts ; push "%s\\%s" ---
        let path_prefix = pushed_string(&mut read, 0x7af9e6)?;
        let path_format = pushed_string(&mut read, 0x7af9eb)?;
        // Format must be exactly two %s with a single backslash separator.
        let sep = if path_format == "%s\\%s" {
            "\\"
        } else {
            return Err("unsupported font path format".into());
        };
        // --- Default-height table + axis conversion ---
        // 0x959bf0: movss xmm0,[esi*4+height_table] ; call 0xab2ad0
        let height_table = word(operand(
            &mut read,
            0x959c2a,
            &[0xf3, 0x0f, 0x10, 0x04, 0xb5],
            4,
        )?)?;
        if call_target(&mut read, 0x959c39)? != 0xab2ad0 {
            return Err("unsupported default-height conversion call".into());
        }
        // 0xab2ad0: fld [ebp+8] ; fmul dword ptr [axis] ; pop ebp ; ret
        let axis_addr = word(operand(&mut read, 0xab2ad6, &[0xd8, 0x0d], 4)?)?;
        let axis_bytes = read(axis_addr, 4, false)?;
        let axis_factor = f32::from_le_bytes(
            axis_bytes
                .try_into()
                .map_err(|_| "truncated axis operand")?,
        );
        if !axis_factor.is_finite() || axis_factor <= 0.0 {
            return Err("invalid axis factor".into());
        }
        // 0x959bc0: fld dword ptr [eax*4+height_table] must agree with 0x959bf0 table.
        let height_table2 = word(operand(&mut read, 0x959bd9, &[0xd9, 0x04, 0x85], 4)?)?;
        if height_table2 != height_table {
            return Err("default-height table relation".into());
        }
        // 0x959b90: mov eax,[eax*4+font_ptr_table] (structure check only)
        let _font_ptr_table = word(operand(&mut read, 0x959ba9, &[0x8b, 0x04, 0x85], 4)?)?;
        // --- Wrapping slot: 0x86cf97 push 0 ; call 0x959b90 ; 0x86cff5 push 0 ; call 0x959bc0 ---
        if imm8(&mut read, 0x86cf97)? != 0 {
            return Err("unqualified wrapping font slot".into());
        }
        if call_target(&mut read, 0x86cf99)? != 0x959b90 {
            return Err("unsupported wrapping font getter".into());
        }
        if imm8(&mut read, 0x86cff5)? != 0 {
            return Err("unqualified wrapping height slot".into());
        }
        if call_target(&mut read, 0x86cff9)? != 0x959bc0 {
            return Err("unsupported wrapping height getter".into());
        }
        let wrapping_slot = 0u32;
        // --- Native table heights for both override slots (finite, positive) ---
        let mut table_entry = |slot: u32| -> Result<f32, String> {
            let addr = height_table
                .checked_add(slot * 4)
                .ok_or("height table overflow")?;
            let v = f32::from_le_bytes(
                read(addr, 4, false)?
                    .try_into()
                    .map_err(|_| "truncated height table")?,
            );
            if !v.is_finite() || v <= 0.0 {
                return Err("invalid default-height table entry".into());
            }
            Ok(v)
        };
        let table_height = table_entry(wrapping_slot)?;
        let _other = table_entry(1)?;
        // --- MiscUI.txt [Misc] FontTexture value (exactly one) ---
        let font_texture = misc_font_texture(misc_text, &section, &key)?;
        let font_path = format!("{path_prefix}{sep}{font_texture}");
        Ok(Self {
            origin,
            image_sha256,
            section,
            key,
            path_prefix,
            path_format,
            font_texture,
            font_path,
            wrapping_slot,
            table_height,
            axis_factor,
        })
    }
}

/// Reads exactly one `key=value` from the named `[section]` in a
/// `MiscUI.txt`-shaped profile. Comments (`//…`) and blank lines are skipped.
/// Any other section is refused by the caller via the decoded names.
fn misc_font_texture(misc_text: &str, section: &str, key: &str) -> Result<String, String> {
    const FILE: &str = "UI\\MiscUI.txt";
    let mut in_section = false;
    let mut found: Vec<String> = Vec::new();
    for raw in misc_text.lines() {
        let line = raw.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_section = line[1..line.len() - 1].trim().eq_ignore_ascii_case(section);
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((k, v)) = line.split_once('=')
            && k.trim().eq_ignore_ascii_case(key)
        {
            found.push(v.trim().trim_matches('"').to_string());
        }
    }
    match found.len() {
        1 if !found[0].is_empty() && !found[0].contains(['\\', '/', '\0']) => {
            Ok(found.into_iter().next().unwrap_or_default())
        }
        0 => Err(format!("{FILE}: missing [{section}] {key}")),
        _ => Err(format!("{FILE}: duplicate or invalid [{section}] {key}")),
    }
}

/// Parses the three template numbers out of an FDF text. Only the named
/// records are read; a missing or non-finite value is an error.
pub fn text_area_numbers(fdf_text: &str, frame: &str) -> Result<(f32, f32), String> {
    let need = |key: &str| -> Result<f32, String> {
        // Find the frame block, then the record line inside it.
        let mut in_frame = false;
        let mut depth = 0i32;
        for raw in fdf_text.lines() {
            let line = raw.trim();
            if !in_frame {
                if line.contains(&format!("\"{frame}\"")) && line.starts_with("Frame") {
                    in_frame = true;
                    depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
                    if depth == 0 {
                        depth = 1;
                    }
                }
                continue;
            }
            depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
            if depth <= 0 {
                break;
            }
            if let Some(rest) = line.strip_prefix(key) {
                let rest = rest.trim().trim_end_matches(',');
                return rest
                    .parse::<f32>()
                    .ok()
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .ok_or_else(|| format!("{frame}: invalid {key}={rest}"));
            }
        }
        Err(format!("{frame}: missing {key}"))
    };
    Ok((need("TextAreaLineHeight")?, need("TextAreaInset")?))
}

/// One line of laid-out TEXTAREA text (visible text only, markup stripped by
/// the caller via the rich-text parser).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLine {
    pub text: String,
}

/// Scroll/clip state for a TEXTAREA. `offset_lines` is the first visible line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextAreaView {
    /// Total wrapped lines.
    pub total_lines: usize,
    /// First visible line index, clamped to `[0, max_offset]`.
    pub offset_lines: usize,
    /// Lines that fit in the painted area.
    pub visible_lines: usize,
}

impl TextAreaView {
    pub fn new(total_lines: usize, area_height: f32, line_height: f32) -> Self {
        let visible = if line_height > 0.0 && area_height > 0.0 {
            ((area_height / line_height).floor() as usize).max(1)
        } else {
            1
        };
        Self {
            total_lines,
            offset_lines: 0,
            visible_lines: visible,
        }
    }

    pub fn max_offset(&self) -> usize {
        self.total_lines.saturating_sub(self.visible_lines)
    }

    pub fn scroll_by(&mut self, delta: i32) {
        let max = self.max_offset() as i32;
        let next = (self.offset_lines as i32 + delta).clamp(0, max);
        self.offset_lines = next as usize;
    }

    pub fn page_by(&mut self, pages: i32) {
        let step = (self.visible_lines as i32).saturating_mul(pages);
        self.scroll_by(step);
    }

    /// The `[start, end)` line range to paint (clipped to the area).
    pub fn visible_range(&self) -> (usize, usize) {
        let start = self.offset_lines.min(self.total_lines);
        let end = (start + self.visible_lines).min(self.total_lines);
        (start, end)
    }

    pub fn needs_scrollbar(&self) -> bool {
        self.total_lines > self.visible_lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELP: &str =
        "|Cffc5ff26MENU COMMANDS|r\n\n|Cfffed312F9|r\n- Toggle the Quest Log on/off.\n";
    const TIPS: &str = "[General]\nTipCount=3\nTip1=\"First tip.\"\nTip2=\"|Cfffed312SECOND|r tip.\"\nTip3=\"Third tip.\"\n";
    const TMPL: &str = "Frame \"TEXTAREA\" \"EscMenuTextAreaTemplate\" {\n    //FrameFont \"MasterFont\", 0.011, \"\",\n    TextAreaLineHeight 0.015,\n    TextAreaLineGap 0.0,\n    TextAreaInset 0.01,\n    TextAreaScrollBar \"EscMenuTextAreaScrollBarTemplate\",\n    Frame \"SCROLLBAR\" \"EscMenuTextAreaScrollBarTemplate\" {\n    }\n    ControlBackdrop \"EscMenuTextAreaBackdropTemplate\",\n    Frame \"BACKDROP\" \"EscMenuTextAreaBackdropTemplate\" {\n    }\n}\n";
    const MISC: &str = "[Misc]\nFontTexture=SYNTHETIC__.TTF // synthetic fixture\n";

    #[test]
    fn help_body_is_kept_verbatim() {
        let h = HelpStrings::parse(HELP).unwrap();
        assert_eq!(h.body, HELP);
    }

    #[test]
    fn help_rejects_empty() {
        assert!(HelpStrings::parse("").is_err());
    }

    #[test]
    fn tips_parse_in_order() {
        let t = TipStrings::parse(TIPS).unwrap();
        assert_eq!(t.tips.len(), 3);
        assert_eq!(t.tips[0], "First tip.");
        assert_eq!(t.tips[1], "|Cfffed312SECOND|r tip.");
    }

    #[test]
    fn tips_reject_count_mismatch() {
        let bad = "[General]\nTipCount=2\nTip1=\"a\"\nTip3=\"c\"\n";
        assert!(TipStrings::parse(bad).is_err());
        let bad2 = "[General]\nTipCount=1\nTip1=\"a\"\nTip2=\"b\"\n";
        assert!(TipStrings::parse(bad2).is_err());
    }

    #[test]
    fn tip_selection_wraps_and_is_retained() {
        let tips = TipStrings::parse(TIPS).unwrap();
        let mut sel = TipSelection::default();
        assert_eq!(sel.current(&tips), Some("First tip."));
        sel.next(&tips);
        assert_eq!(sel.current(&tips), Some("|Cfffed312SECOND|r tip."));
        sel.next(&tips);
        sel.next(&tips);
        assert_eq!(sel.index(), 0, "wraps past the end");
        sel.previous(&tips);
        assert_eq!(sel.index(), 2, "wraps before the start");
        // Retained across a hide/show: selection survives without reset.
        let kept = sel.index();
        assert_eq!(kept, 2);
        let tips2 = TipStrings::parse(TIPS).unwrap();
        let mut sel2 = TipSelection { index: kept };
        assert_eq!(sel2.current(&tips2), Some("Third tip."));
        sel2.clamp(&tips2);
        assert_eq!(sel2.index(), 2);
    }

    #[test]
    fn textarea_numbers_come_from_template() {
        let (line, inset) = text_area_numbers(TMPL, "EscMenuTextAreaTemplate").unwrap();
        assert!((line - 0.015).abs() < 1e-6);
        assert!((inset - 0.01).abs() < 1e-6);
    }

    #[test]
    fn tips_refuse_unsupported_sections() {
        let bad = "[General]\nTipCount=1\nTip1=\"a\"\n[OrcTips]\nTipCount=1\nTip1=\"b\"\n";
        let e = TipStrings::parse(bad).unwrap_err();
        assert!(e.contains("unsupported section"), "{e}");
    }

    // Synthetic image fixture: all authored values are non-stock placeholders.
    fn synthetic_image() -> (Vec<u8>, std::collections::HashMap<u32, Vec<u8>>) {
        use std::collections::HashMap;
        let mut m: HashMap<u32, Vec<u8>> = HashMap::new();
        let put = |m: &mut HashMap<u32, Vec<u8>>, va: u32, bytes: &[u8]| {
            m.insert(va, bytes.to_vec());
        };
        // Strings (non-stock)
        put(&mut m, 0xE10000, b"Misc\0");
        put(&mut m, 0xE10010, b"FontTexture\0");
        put(&mut m, 0xE10020, b"Fonts\0");
        put(&mut m, 0xE10030, b"%s\\%s\0");
        // String addresses are read via push operands; store fake pointers
        // by encoding the string VA directly in the push operand bytes.
        let sec: u32 = 0xE10000;
        let key: u32 = 0xE10010;
        let prefix: u32 = 0xE10020;
        let fmt: u32 = 0xE10030;
        // FontTexture lookup caller
        put(
            &mut m,
            0x42eeda,
            &[
                0x68,
                sec.to_le_bytes()[0],
                sec.to_le_bytes()[1],
                sec.to_le_bytes()[2],
                sec.to_le_bytes()[3],
            ],
        );
        // NOTE: swapped order below is corrected by dedicated puts:
        // 0x42eeda must be FontTexture, 0x42eedf Misc. Rebuild correctly:
        m.remove(&0x42eeda);
        put(
            &mut m,
            0x42eeda,
            &[
                0x68,
                key.to_le_bytes()[0],
                key.to_le_bytes()[1],
                key.to_le_bytes()[2],
                key.to_le_bytes()[3],
            ],
        );
        put(
            &mut m,
            0x42eedf,
            &[
                0x68,
                sec.to_le_bytes()[0],
                sec.to_le_bytes()[1],
                sec.to_le_bytes()[2],
                sec.to_le_bytes()[3],
            ],
        );
        put(&mut m, 0x42eee4, &[0xe8, 0x00, 0x00, 0x00, 0x00]); // call target checked separately; patch below
        m.remove(&0x42eee4);
        let disp = (0x430570u32).wrapping_sub(0x42eee4 + 5);
        put(
            &mut m,
            0x42eee4,
            &[
                0xe8,
                disp.to_le_bytes()[0],
                disp.to_le_bytes()[1],
                disp.to_le_bytes()[2],
                disp.to_le_bytes()[3],
            ],
        );
        // slot 0 override
        put(&mut m, 0x42ef20, &[0x6a, 0]);
        let disp = (0x7af9e0u32).wrapping_sub(0x42ef14 + 5);
        put(
            &mut m,
            0x42ef14,
            &[
                0xe8,
                disp.to_le_bytes()[0],
                disp.to_le_bytes()[1],
                disp.to_le_bytes()[2],
                disp.to_le_bytes()[3],
            ],
        );
        let disp = (0x959bf0u32).wrapping_sub(0x42ef22 + 5);
        put(
            &mut m,
            0x42ef22,
            &[
                0xe8,
                disp.to_le_bytes()[0],
                disp.to_le_bytes()[1],
                disp.to_le_bytes()[2],
                disp.to_le_bytes()[3],
            ],
        );
        put(&mut m, 0x42ef2e, &[0x6a, 1]);
        let disp = (0x959bf0u32).wrapping_sub(0x42ef30 + 5);
        put(
            &mut m,
            0x42ef30,
            &[
                0xe8,
                disp.to_le_bytes()[0],
                disp.to_le_bytes()[1],
                disp.to_le_bytes()[2],
                disp.to_le_bytes()[3],
            ],
        );
        // path resolver
        put(
            &mut m,
            0x7af9e6,
            &[
                0x68,
                prefix.to_le_bytes()[0],
                prefix.to_le_bytes()[1],
                prefix.to_le_bytes()[2],
                prefix.to_le_bytes()[3],
            ],
        );
        put(
            &mut m,
            0x7af9eb,
            &[
                0x68,
                fmt.to_le_bytes()[0],
                fmt.to_le_bytes()[1],
                fmt.to_le_bytes()[2],
                fmt.to_le_bytes()[3],
            ],
        );
        // height table at 0xED2000, axis at 0xE20000
        put(&mut m, 0xED2000, &0.03125f32.to_le_bytes());
        put(&mut m, 0xED2004, &0.03125f32.to_le_bytes());
        put(&mut m, 0xE20000, &0.5f32.to_le_bytes());
        let ht: u32 = 0xED2000;
        let ax: u32 = 0xE20000;
        put(
            &mut m,
            0x959c2a,
            &[
                0xf3,
                0x0f,
                0x10,
                0x04,
                0xb5,
                ht.to_le_bytes()[0],
                ht.to_le_bytes()[1],
                ht.to_le_bytes()[2],
                ht.to_le_bytes()[3],
            ],
        );
        let disp = (0xab2ad0u32).wrapping_sub(0x959c39 + 5);
        put(
            &mut m,
            0x959c39,
            &[
                0xe8,
                disp.to_le_bytes()[0],
                disp.to_le_bytes()[1],
                disp.to_le_bytes()[2],
                disp.to_le_bytes()[3],
            ],
        );
        put(
            &mut m,
            0xab2ad6,
            &[
                0xd8,
                0x0d,
                ax.to_le_bytes()[0],
                ax.to_le_bytes()[1],
                ax.to_le_bytes()[2],
                ax.to_le_bytes()[3],
            ],
        );
        put(
            &mut m,
            0x959bd9,
            &[
                0xd9,
                0x04,
                0x85,
                ht.to_le_bytes()[0],
                ht.to_le_bytes()[1],
                ht.to_le_bytes()[2],
                ht.to_le_bytes()[3],
            ],
        );
        let fpt: u32 = 0xED3000;
        put(
            &mut m,
            0x959ba9,
            &[
                0x8b,
                0x04,
                0x85,
                fpt.to_le_bytes()[0],
                fpt.to_le_bytes()[1],
                fpt.to_le_bytes()[2],
                fpt.to_le_bytes()[3],
            ],
        );
        put(&mut m, 0x86cf97, &[0x6a, 0]);
        let disp = (0x959b90u32).wrapping_sub(0x86cf99 + 5);
        put(
            &mut m,
            0x86cf99,
            &[
                0xe8,
                disp.to_le_bytes()[0],
                disp.to_le_bytes()[1],
                disp.to_le_bytes()[2],
                disp.to_le_bytes()[3],
            ],
        );
        put(&mut m, 0x86cff5, &[0x6a, 0]);
        let disp = (0x959bc0u32).wrapping_sub(0x86cff9 + 5);
        put(
            &mut m,
            0x86cff9,
            &[
                0xe8,
                disp.to_le_bytes()[0],
                disp.to_le_bytes()[1],
                disp.to_le_bytes()[2],
                disp.to_le_bytes()[3],
            ],
        );
        (vec![], m)
    }

    fn decode_synthetic(
        map: &std::collections::HashMap<u32, Vec<u8>>,
        misc: &str,
    ) -> Result<DefaultFont, String> {
        let read = |va: u32, n: usize, _code: bool| -> Result<Vec<u8>, String> {
            // Exact VA match for instruction bytes; string/table bytes via map too.
            if let Some(b) = map.get(&va) {
                if b.len() >= n {
                    return Ok(b[..n].to_vec());
                }
                return Err(format!("short at {va:#x}"));
            }
            // String byte-by-byte reads land inside a stored string range.
            for (base, b) in map.iter() {
                if va >= *base && (va as usize) < *base as usize + b.len() {
                    let off = (va - base) as usize;
                    if off + n <= b.len() {
                        return Ok(b[off..off + n].to_vec());
                    }
                }
            }
            Err(format!("missing synthetic bytes at {va:#x}"))
        };
        DefaultFont::decode(
            AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            [0x42; 32],
            misc,
            read,
        )
    }

    #[test]
    fn default_font_decodes_source_operands_not_constants() {
        let (_, map) = synthetic_image();
        let font = decode_synthetic(&map, MISC).unwrap();
        assert_eq!(font.section, "Misc");
        assert_eq!(font.key, "FontTexture");
        assert_eq!(font.path_prefix, "Fonts");
        assert_eq!(font.font_texture, "SYNTHETIC__.TTF");
        assert_eq!(font.font_path, "Fonts\\SYNTHETIC__.TTF");
        assert_eq!(font.wrapping_slot, 0);
        assert!((font.table_height - 0.03125).abs() < 1e-9);
        assert!((font.axis_factor - 0.5).abs() < 1e-9);
        assert!((font.wrapping_height() - 0.015625).abs() < 1e-9);
        // Row height is a separate FDF input; wrapping height must not equal it by construction.
        let (line, _) = text_area_numbers(TMPL, "EscMenuTextAreaTemplate").unwrap();
        assert!((line - 0.015).abs() < 1e-6);
        assert!((font.wrapping_height() - line).abs() > 1e-6);
    }

    #[test]
    fn default_font_refuses_wrong_slot_and_missing_misc() {
        let (_, mut map) = synthetic_image();
        // Change wrapping slot push to 1 -> unqualified
        map.insert(0x86cf97, vec![0x6a, 1]);
        assert!(decode_synthetic(&map, MISC).is_err());
        let (_, map) = synthetic_image();
        assert!(decode_synthetic(&map, "[Other]\nFontTexture=X\n").is_err());
        assert!(decode_synthetic(&map, "[Misc]\nOther=1\n").is_err());
        let dup = "[Misc]\nFontTexture=A\nFontTexture=B\n";
        assert!(decode_synthetic(&map, dup).is_err());
    }

    #[test]
    fn scroll_view_clamps_and_clips() {
        let mut v = TextAreaView::new(50, 0.264, 0.015);
        assert_eq!(v.visible_lines, 17);
        assert!(v.needs_scrollbar());
        v.scroll_by(1000);
        assert_eq!(v.offset_lines, 33);
        assert_eq!(v.visible_range(), (33, 50));
        v.scroll_by(-1000);
        assert_eq!(v.offset_lines, 0);
        v.page_by(1);
        assert_eq!(v.offset_lines, 17);
        let flat = TextAreaView::new(3, 0.264, 0.015);
        assert!(!flat.needs_scrollbar());
        assert_eq!(flat.visible_range(), (0, 3));
    }
}
