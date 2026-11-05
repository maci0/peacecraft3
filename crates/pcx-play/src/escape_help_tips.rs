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
//!   back to the default font slot. Following `campaign_error_view` /
//!   `pcx_objects::editor::messages`, that slot is `MasterFont` in
//!   `UI\war3skins.txt` `[Default]`, with height from `UI\MiscUI.txt`
//!   `[FontHeights]` — the TEXTAREA inherits the default font face and the
//!   `ChatHistory` height slot (multi-line scrollable text), not a copied
//!   constant. Both values are parsed from assets at runtime.
//! - Input: TEXTAREA is read-only display; Escape/Enter close via the modal
//!   parent. No editor caret is kept.
//!
//! Shared wiring (`campaign_escape_menu`, `glue_menu`, Bevy spawn/input) lives
//! with the integrator; this module owns content, navigation and geometry
//! math only. Evidence: `docs/re-notes/escape-help-tips.md`.

/// Default TEXTAREA font face key in `UI\war3skins.txt`.
pub const DEFAULT_FONT_FACE: &str = "MasterFont";
/// `UI\MiscUI.txt` height slot used by the default TEXTAREA font.
pub const DEFAULT_FONT_HEIGHT_KEY: &str = "ChatHistory";

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
        Ok(Self { body: text.to_string() })
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
            if line.eq_ignore_ascii_case("[General]") {
                if seen_general {
                    return Err(format!("{FILE}: duplicate [General]"));
                }
                seen_general = true;
                continue;
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
            return Err(format!("{FILE}: TipCount={n} but found {} tips", tips.len()));
        }
        for (idx, (i, _)) in tips.iter().enumerate() {
            if *i != idx + 1 {
                return Err(format!("{FILE}: gap: expected Tip{}, found Tip{i}", idx + 1));
            }
        }
        Ok(Self { tips: tips.into_iter().map(|(_, v)| v).collect() })
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

/// TEXTAREA metrics derived from assets, not copied constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextAreaMetrics {
    /// From `TextAreaLineHeight` in the template.
    pub line_height: f32,
    /// From `TextAreaInset` in the template.
    pub inset: f32,
    /// Default font height slot (`ChatHistory` in `MiscUI.txt`).
    pub font_height: f32,
    /// Default font face slot (`MasterFont` in `war3skins.txt`).
    pub font_face: &'static str,
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

/// Reads `key=float` from a `MiscUI.txt`-shaped `[FontHeights]` section.
/// Comments (`//…`) and blank lines are skipped.
pub fn font_height(misc_text: &str, key: &str) -> Result<f32, String> {
    const FILE: &str = "UI\\MiscUI.txt";
    let mut in_section = false;
    let mut found: Option<f32> = None;
    for raw in misc_text.lines() {
        let line = raw.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_section = line.eq_ignore_ascii_case("[FontHeights]");
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim().eq_ignore_ascii_case(key) {
            let h: f32 = v
                .trim()
                .parse()
                .map_err(|_| format!("{FILE}: invalid {key}"))?;
            if !h.is_finite() || h <= 0.0 {
                return Err(format!("{FILE}: invalid {key}"));
            }
            if found.replace(h).is_some() {
                return Err(format!("{FILE}: duplicate {key}"));
            }
        }
    }
    found.ok_or_else(|| format!("{FILE}: missing {key}"))
}

/// Resolves the default TEXTAREA font file via `war3skins.txt` `[Default]`,
/// requiring exactly one definition (mirrors `SkinTable::unique_file`).
pub fn default_font_file(skin_text: &str) -> Result<String, String> {
    const FILE: &str = "UI\\war3skins.txt";
    let mut in_default = false;
    let mut found: Vec<String> = Vec::new();
    for raw in skin_text.lines() {
        let line = raw.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_default = line.eq_ignore_ascii_case("[Default]");
            continue;
        }
        if !in_default {
            continue;
        }
        if let Some((k, v)) = line.split_once('=')
            && k.trim().eq_ignore_ascii_case(DEFAULT_FONT_FACE)
        {
            found.push(v.trim().to_string());
        }
    }
    match found.len() {
        1 if !found[0].is_empty() => Ok(found.into_iter().next().unwrap_or_default()),
        0 => Err(format!("{FILE}: missing {DEFAULT_FONT_FACE}")),
        _ => Err(format!("{FILE}: duplicate {DEFAULT_FONT_FACE}")),
    }
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
        Self { total_lines, offset_lines: 0, visible_lines: visible }
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

    const HELP: &str = "|Cffc5ff26MENU COMMANDS|r\n\n|Cfffed312F9|r\n- Toggle the Quest Log on/off.\n";
    const TIPS: &str = "[General]\nTipCount=3\nTip1=\"First tip.\"\nTip2=\"|Cfffed312SECOND|r tip.\"\nTip3=\"Third tip.\"\n";
    const TMPL: &str = "Frame \"TEXTAREA\" \"EscMenuTextAreaTemplate\" {\n    //FrameFont \"MasterFont\", 0.011, \"\",\n    TextAreaLineHeight 0.015,\n    TextAreaLineGap 0.0,\n    TextAreaInset 0.01,\n    TextAreaScrollBar \"EscMenuTextAreaScrollBarTemplate\",\n    Frame \"SCROLLBAR\" \"EscMenuTextAreaScrollBarTemplate\" {\n    }\n    ControlBackdrop \"EscMenuTextAreaBackdropTemplate\",\n    Frame \"BACKDROP\" \"EscMenuTextAreaBackdropTemplate\" {\n    }\n}\n";
    const MISC: &str = "[FontHeights]\n// in-game ui\nChatHistory=0.011               // chat history area in chat dialog\nQuestDescription=0.011          // quest description text in Quest Dialog\n";
    const SKIN: &str = "[Default]\nMasterFont=Fonts\\FRIZQT__.TTF\nEscMenuTextFont=Fonts\\FRIZQT__.TTF\n";

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
    fn default_font_height_and_file_come_from_assets() {
        let h = font_height(MISC, DEFAULT_FONT_HEIGHT_KEY).unwrap();
        assert!((h - 0.011).abs() < 1e-6);
        assert_eq!(default_font_file(SKIN).unwrap(), "Fonts\\FRIZQT__.TTF");
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
