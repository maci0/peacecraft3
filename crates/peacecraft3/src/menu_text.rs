//! Authored menu text: the label markup and the FDF font records.
//!
//! Two asset records decide what a menu label looks like, and both are read here
//! rather than copied into constants:
//!
//! * the **string** comes from `UI\FrameDef\GlobalStrings.fdf`, whose values carry
//!   the original's `|C…|R` colour markup — `KEY_SINGLE_PLAYER` is
//!   `"|CffffffffS|Ringle Player"`, and `KEY_EXIT` is `"E|Cffffffffx|Rit"`, so the
//!   hotkey is **not** always the first character;
//! * the **style** comes from the text frame's own `FrameFont`, `FontColor`,
//!   `FontHighlightColor`, `FontDisabledColor`, `FontShadowColor`,
//!   `FontShadowOffset`, `FontJustificationH/V` and `FontFlags` records.
//!
//! Nothing here is Bevy-aware, so it can be tested without a window, and nothing
//! hard-codes an authored value: a missing record is an error, not a default.

use std::fmt;

/// Why a label or a text record could not be read.
#[derive(Debug, Clone, PartialEq)]
pub enum TextError {
    /// A `|C` run was opened and never closed by `|R`.
    UnterminatedColour(String),
    /// `|C` was not followed by eight hex digits.
    BadColourRun(String),
    /// The named frame is not defined in the text.
    UnknownFrame(String),
    /// A frame's `{` was never closed.
    UnterminatedFrame(String),
    /// A record the style needs is absent.
    Missing {
        /// The frame that lacks it.
        frame: String,
        /// The record, e.g. `FontColor`.
        record: &'static str,
    },
    /// A frame's own block defines the same record twice, so which one the engine
    /// would use is not established. Reported rather than picked.
    Duplicate {
        /// The frame whose block repeats it.
        frame: String,
        /// The record, e.g. `FontColor`.
        record: &'static str,
    },
    /// A record is present but will not parse.
    Malformed {
        /// The frame carrying it.
        frame: String,
        /// The record, e.g. `FontColor`.
        record: &'static str,
        /// The text that would not parse.
        text: String,
    },
    /// A record parsed as a number, but the value cannot drive a UI — a
    /// non-finite colour or offset, or a font height that is not positive.
    ///
    /// A malformed asset must be refused rather than handed to the renderer: a
    /// `NaN` colour or a zero height produces invisible or degenerate text that
    /// looks like a rendering bug rather than a bad asset.
    Unusable {
        /// The frame carrying it.
        frame: String,
        /// The record, e.g. `FontColor`.
        record: &'static str,
        /// The value that was rejected.
        value: f32,
    },
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnterminatedColour(s) => write!(f, "colour run never closed in {s:?}"),
            Self::BadColourRun(s) => write!(f, "malformed colour run in {s:?}"),
            Self::UnknownFrame(s) => write!(f, "no frame named {s:?}"),
            Self::UnterminatedFrame(s) => write!(f, "frame {s:?} is never closed"),
            Self::Missing { frame, record } => write!(f, "{frame:?} has no {record}"),
            Self::Duplicate { frame, record } => {
                write!(f, "{frame:?} defines {record} more than once")
            }
            Self::Malformed {
                frame,
                record,
                text,
            } => write!(f, "{frame:?}'s {record} will not parse: {text:?}"),
            Self::Unusable {
                frame,
                record,
                value,
            } => write!(f, "{frame:?}'s {record} is unusable: {value}"),
        }
    }
}

impl std::error::Error for TextError {}

/// One run of a label, in authored order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// The literal characters of the run.
    pub text: String,
    /// True when the run sits inside a `|C…|R` pair, i.e. is the hotkey and is
    /// drawn in the template's highlight colour.
    pub hotkey: bool,
}

/// A label parsed from the original's `|C…|R` markup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// The runs, in authored order. Empty runs are not emitted.
    pub spans: Vec<Span>,
}

impl Label {
    /// Splits the markup into runs.
    ///
    /// The hotkey is the run **inside** the colour pair, which may be preceded by
    /// literal text: `E|Cffffffffx|Rit` is `E` + `x` + `it`. Treating the first
    /// character as the hotkey, or dropping everything before `|R`, loses that
    /// `E`.
    ///
    /// # Errors
    ///
    /// [`TextError::BadColourRun`] when `|C` is not followed by eight hex digits,
    /// and [`TextError::UnterminatedColour`] when a run is never closed.
    pub fn parse(raw: &str) -> Result<Self, TextError> {
        let mut spans: Vec<Span> = Vec::new();
        let mut plain = String::new();
        let mut highlighted: Option<String> = None;
        let bytes = raw.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'|' && i + 1 < bytes.len() {
                match bytes[i + 1] {
                    b'C' | b'c' => {
                        if highlighted.is_some() {
                            return Err(TextError::UnterminatedColour(raw.to_string()));
                        }
                        let hex = raw
                            .get(i + 2..i + 10)
                            .ok_or_else(|| TextError::BadColourRun(raw.to_string()))?;
                        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                            return Err(TextError::BadColourRun(raw.to_string()));
                        }
                        if !plain.is_empty() {
                            spans.push(Span {
                                text: std::mem::take(&mut plain),
                                hotkey: false,
                            });
                        }
                        highlighted = Some(String::new());
                        i += 10;
                        continue;
                    }
                    b'R' | b'r' => {
                        let run = highlighted
                            .take()
                            .ok_or_else(|| TextError::UnterminatedColour(raw.to_string()))?;
                        spans.push(Span {
                            text: run,
                            hotkey: true,
                        });
                        i += 2;
                        continue;
                    }
                    b'n' | b'N' => {
                        // The original's line break inside a label.
                        match highlighted.as_mut() {
                            Some(run) => run.push('\n'),
                            None => plain.push('\n'),
                        }
                        i += 2;
                        continue;
                    }
                    _ => {}
                }
            }
            // A multi-byte character must be pushed whole.
            let ch = raw[i..].chars().next().unwrap_or('\u{fffd}');
            match highlighted.as_mut() {
                Some(run) => run.push(ch),
                None => plain.push(ch),
            }
            i += ch.len_utf8();
        }
        if highlighted.is_some() {
            return Err(TextError::UnterminatedColour(raw.to_string()));
        }
        if !plain.is_empty() {
            spans.push(Span {
                text: plain,
                hotkey: false,
            });
        }
        Ok(Self { spans })
    }

    /// The characters the original draws, with the markup removed.
    #[must_use]
    pub fn visible(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    /// The hotkey letter, if the label marks one.
    #[must_use]
    pub fn hotkey(&self) -> Option<char> {
        self.spans
            .iter()
            .find(|s| s.hotkey)
            .and_then(|s| s.text.chars().next())
    }
}

/// Horizontal justification, from `FontJustificationH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyH {
    /// `JUSTIFYLEFT`.
    Left,
    /// `JUSTIFYCENTER`.
    Center,
    /// `JUSTIFYRIGHT`.
    Right,
}

/// Vertical justification, from `FontJustificationV`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyV {
    /// `JUSTIFYTOP`.
    Top,
    /// `JUSTIFYMIDDLE`.
    Middle,
    /// `JUSTIFYBOTTOM`.
    Bottom,
}

/// The font record a text frame declares.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// The face name the FDF names, e.g. `MasterFont`. The FDF does not name a
    /// file; the skin/engine binds the name.
    pub font: String,
    /// Height in frame units, as written.
    pub height: f32,
    /// Normal colour.
    pub colour: [f32; 4],
    /// Colour while highlighted (the hotkey run).
    pub highlight: [f32; 4],
    /// Colour when disabled.
    pub disabled: [f32; 4],
    /// Shadow colour.
    pub shadow_colour: [f32; 4],
    /// Shadow offset in frame units.
    pub shadow_offset: [f32; 2],
    /// Horizontal justification.
    pub justify_h: JustifyH,
    /// Vertical justification.
    pub justify_v: JustifyV,
    /// True when `FontFlags` carries `FIXEDSIZE`.
    pub fixed_size: bool,
}

/// Renderer style for ordinary labels as well as buttons. Optional state colours
/// inherit the foreground; an omitted shadow is absent. These are renderer
/// conventions, while every present record is validated and kept.
pub fn render_style(fdf: &crate::fdf::Fdf, frame: &str) -> Result<TextStyle, String> {
    render_style_with_parent(fdf, frame, frame)
}

/// Preserve child records while resolving an omitted font through the actual
/// instance's title, before a shared template child's enclosing definition.
pub fn render_style_with_parent(
    fdf: &crate::fdf::Fdf,
    frame: &str,
    parent: &str,
) -> Result<TextStyle, String> {
    let record = |key: &str| {
        if let Some(value) = fdf.record(frame, key).or_else(|| fdf.record(parent, key)) {
            return Some(value);
        }
        let mut current = fdf.frame(frame).and_then(|f| f.parent.clone());
        let mut seen = std::collections::HashSet::new();
        while let Some(name) = current {
            if !seen.insert(name.clone()) {
                return None;
            }
            if let Some(value) = fdf.record(&name, key) {
                return Some(value);
            }
            current = fdf.frame(&name).and_then(|f| f.parent.clone());
        }
        None
    };
    let font_record = record("FrameFont").ok_or_else(|| format!("{frame}: missing FrameFont"))?;
    let (font, height) = parse_frame_font(&font_record)
        .filter(|(_, h)| h.is_finite() && *h > 0.0)
        .ok_or_else(|| format!("{frame}: invalid FrameFont"))?;
    let colour = |key: &'static str, fallback: Option<[f32; 4]>| -> Result<[f32; 4], String> {
        let value = match fdf.numbers(frame, key).map_err(|e| e.to_string())? {
            Some(v) => match v.as_slice() {
                [r, g, b] => [*r, *g, *b, 1.0],
                [r, g, b, a] => [*r, *g, *b, *a],
                _ => return Err(format!("{frame}: invalid {key}")),
            },
            None => return fallback.ok_or_else(|| format!("{frame}: missing {key}")),
        };
        if value
            .iter()
            .all(|c| c.is_finite() && (0.0..=1.0).contains(c))
        {
            Ok(value)
        } else {
            Err(format!("{frame}: invalid {key}"))
        }
    };
    let foreground = colour("FontColor", Some([1.0; 4]))?;
    let shadow_offset = match fdf
        .numbers(frame, "FontShadowOffset")
        .map_err(|e| e.to_string())?
    {
        Some(v) => {
            <[f32; 2]>::try_from(v).map_err(|_| format!("{frame}: invalid FontShadowOffset"))?
        }
        None => [0.0; 2],
    };
    if !shadow_offset.iter().all(|c| c.is_finite()) {
        return Err(format!("{frame}: invalid shadow offset"));
    }
    let justify_h = match fdf.record(frame, "FontJustificationH").as_deref() {
        Some("JUSTIFYLEFT") | None => JustifyH::Left,
        Some("JUSTIFYCENTER") => JustifyH::Center,
        Some("JUSTIFYRIGHT") => JustifyH::Right,
        Some(v) => return Err(format!("{frame}: invalid horizontal justification {v}")),
    };
    let justify_v = match fdf.record(frame, "FontJustificationV").as_deref() {
        Some("JUSTIFYTOP") | None => JustifyV::Top,
        Some("JUSTIFYMIDDLE") => JustifyV::Middle,
        Some("JUSTIFYBOTTOM") => JustifyV::Bottom,
        Some(v) => return Err(format!("{frame}: invalid vertical justification {v}")),
    };
    Ok(TextStyle {
        font,
        height,
        colour: foreground,
        highlight: colour("FontHighlightColor", Some(foreground))?,
        disabled: colour("FontDisabledColor", Some(foreground))?,
        shadow_colour: colour("FontShadowColor", Some([0.0; 4]))?,
        shadow_offset,
        justify_h,
        justify_v,
        fixed_size: fdf
            .record(frame, "FontFlags")
            .is_some_and(|s| s.contains("FIXEDSIZE")),
    })
}

/// Reads the value of `key` from a `GlobalStrings.fdf` text.
///
/// The records are `KEY  "value",` lines. A key that is absent returns `None`;
/// the caller reports it rather than substituting the key name.
#[must_use]
pub fn global_string(fdf: &str, key: &str) -> Option<String> {
    for line in fdf.lines() {
        let line = line.trim();
        let Some((name, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if name != key {
            continue;
        }
        let rest = rest.trim();
        let rest = rest.strip_prefix('"')?;
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    None
}

/// Reads the style of the `Frame "TEXT" "<frame>"` block in `fdf`.
///
/// # Errors
///
/// [`TextError::UnknownFrame`] when the frame is absent,
/// [`TextError::UnterminatedFrame`] when its block never closes, and
/// [`TextError::Missing`]/[`TextError::Malformed`] for a record that is absent or
/// unreadable. Nothing is defaulted.
/// The style of `frame` in a single text. A convenience for the tests and for a
/// caller with one file; the menu resolves across two, so it uses
/// [`text_style_in`].
#[cfg(test)]
pub fn text_style(fdf: &str, frame: &str) -> Result<TextStyle, TextError> {
    text_style_in(&[fdf], frame)
}

/// The style of `frame`, resolving `INHERITS` across `texts`.
///
/// The frame's own block wins; a record it does not carry comes from the nearest
/// ancestor that does, which is how a per-entry text frame overrides one record of
/// its template while inheriting the rest. A record defined **twice in the same
/// block** is ambiguous and refused rather than picked.
///
/// # Errors
///
/// [`TextError`] for malformed or missing style records, including
/// [`TextError::Duplicate`] for ambiguous records.
pub fn text_style_in(texts: &[&str], frame: &str) -> Result<TextStyle, TextError> {
    let chain = frame_chain(texts, frame)?;
    let record = |name: &'static str| -> Result<&str, TextError> {
        for (owner, body) in &chain {
            let hits: Vec<&str> = body.lines().filter_map(|l| record_value(l, name)).collect();
            match hits.len() {
                0 => continue,
                1 => return Ok(hits[0]),
                _ => {
                    return Err(TextError::Duplicate {
                        frame: owner.clone(),
                        record: name,
                    });
                }
            }
        }
        Err(TextError::Missing {
            frame: frame.to_string(),
            record: name,
        })
    };
    let bad = |record: &'static str, text: &str| TextError::Malformed {
        frame: frame.to_string(),
        record,
        text: text.to_string(),
    };

    let font_line = record("FrameFont")?;
    let (font, height) = parse_frame_font(font_line).ok_or_else(|| bad("FrameFont", font_line))?;
    let unusable = |record: &'static str, value: f32| TextError::Unusable {
        frame: frame.to_string(),
        record,
        value,
    };
    // A height that is not finite or not positive cannot drive text; refusing it
    // is what stops a bad asset from rendering as an invisible label.
    if !height.is_finite() || height <= 0.0 {
        return Err(unusable("FrameFont", height));
    }

    let colour = |name: &'static str| -> Result<[f32; 4], TextError> {
        let line = record(name)?;
        let v: [f32; 4] = floats(line, 4)
            .and_then(|v| <[f32; 4]>::try_from(v).ok())
            .ok_or_else(|| bad(name, line))?;
        match v.iter().copied().find(|c| !c.is_finite()) {
            Some(bad_value) => Err(unusable(name, bad_value)),
            None => Ok(v),
        }
    };

    let justify_h = match record("FontJustificationH")?.trim() {
        "JUSTIFYLEFT" => JustifyH::Left,
        "JUSTIFYCENTER" => JustifyH::Center,
        "JUSTIFYRIGHT" => JustifyH::Right,
        other => return Err(bad("FontJustificationH", other)),
    };
    let justify_v = match record("FontJustificationV")?.trim() {
        "JUSTIFYTOP" => JustifyV::Top,
        "JUSTIFYMIDDLE" => JustifyV::Middle,
        "JUSTIFYBOTTOM" => JustifyV::Bottom,
        other => return Err(bad("FontJustificationV", other)),
    };
    let offset_line = record("FontShadowOffset")?;
    let offset: [f32; 2] = floats(offset_line, 2)
        .and_then(|v| <[f32; 2]>::try_from(v).ok())
        .ok_or_else(|| bad("FontShadowOffset", offset_line))?;
    if let Some(bad_value) = offset.iter().copied().find(|c| !c.is_finite()) {
        return Err(unusable("FontShadowOffset", bad_value));
    }
    let flags = record("FontFlags")?;

    Ok(TextStyle {
        font,
        height,
        colour: colour("FontColor")?,
        highlight: colour("FontHighlightColor")?,
        disabled: colour("FontDisabledColor")?,
        shadow_colour: colour("FontShadowColor")?,
        shadow_offset: offset,
        justify_h,
        justify_v,
        fixed_size: flags.contains("FIXEDSIZE"),
    })
}

/// The `INHERITS` parent and `{ … }` body of the block naming `frame`, searched
/// across every supplied text.
fn frame_block(texts: &[&str], frame: &str) -> Result<(Option<String>, String), TextError> {
    for text in texts {
        let mut depth = 0usize;
        let mut collecting = false;
        let mut inherits = None;
        let mut body = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if !collecting {
                // The frame's **own name** decides the match: a header whose
                // `INHERITS` names this frame must not be mistaken for it.
                if trimmed.starts_with("Frame") && header_name(trimmed).as_deref() == Some(frame) {
                    inherits = inherit_of(trimmed);
                    collecting = true;
                    depth = usize::from(trimmed.ends_with('{'));
                }
                continue;
            }
            for ch in trimmed.chars() {
                if ch == '{' {
                    depth += 1;
                }
            }
            if trimmed == "}" {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok((inherits, body));
                }
            }
            body.push_str(trimmed);
            body.push('\n');
        }
        if collecting {
            return Err(TextError::UnterminatedFrame(frame.to_string()));
        }
    }
    Err(TextError::UnknownFrame(frame.to_string()))
}

/// A frame header's own name: the **second** quoted token of
/// `Frame "TYPE" "NAME" …`. The first is the kind, and any later one is an
/// `INHERITS` template, which must not be mistaken for the name.
fn header_name(header: &str) -> Option<String> {
    let mut quotes: Vec<&str> = Vec::new();
    let mut rest = header;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let end = after.find('"')?;
        quotes.push(&after[..end]);
        rest = &after[end + 1..];
    }
    quotes.get(1).map(|s| (*s).to_string())
}

/// The template a frame header `INHERITS`, if it names one.
fn inherit_of(header: &str) -> Option<String> {
    let after = header.split_once("INHERITS")?.1;
    let start = after.find('"')? + 1;
    let end = after[start..].find('"')? + start;
    Some(after[start..end].to_string())
}

/// The record-resolution chain for `frame`: itself, then each `INHERITS` ancestor.
///
/// A loop in the chain is refused rather than followed.
fn frame_chain(texts: &[&str], frame: &str) -> Result<Vec<(String, String)>, TextError> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut current = frame.to_string();
    loop {
        if out.iter().any(|(n, _)| n == &current) {
            return Err(TextError::UnterminatedFrame(current));
        }
        let (inherits, body) = frame_block(texts, &current)?;
        out.push((current.clone(), body));
        match inherits {
            Some(parent) => current = parent,
            None => break,
        }
    }
    Ok(out)
}

/// The value of a `<name> <value>` record on one line, if that is the line's
/// record.
fn record_value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let line = line.trim();
    let (k, v) = line.split_once(char::is_whitespace)?;
    (k == name).then(|| v.trim().trim_end_matches(',').trim())
}

/// `"MasterFont", 0.013, ""` -> `("MasterFont", 0.013)`.
fn parse_frame_font(line: &str) -> Option<(String, f32)> {
    let rest = line.trim().strip_prefix('"')?;
    let end = rest.find('"')?;
    let name = rest[..end].to_string();
    if name.is_empty() {
        return None;
    }
    let height = rest[end + 1..]
        .split(',')
        .map(str::trim)
        .find_map(|t| t.trim_end_matches('f').parse::<f32>().ok())?;
    Some((name, height))
}

/// `n` whitespace-separated floats, ignoring a trailing comma.
///
/// The FDF's optional `f` suffix is stripped only when the text does not parse
/// as written, so `inf` stays `inf` rather than becoming `in` and failing for the
/// wrong reason.
fn floats(line: &str, n: usize) -> Option<Vec<f32>> {
    let v: Vec<f32> = line
        .trim_end_matches(',')
        .split_whitespace()
        .map(|t| {
            t.parse::<f32>()
                .or_else(|_| t.trim_end_matches('f').parse::<f32>())
        })
        .collect::<Result<_, _>>()
        .ok()?;
    (v.len() == n).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A beginning hotkey: `|CffffffffS|Ringle Player` is "Single Player".
    #[test]
    fn a_leading_hotkey_run_reads_as_the_whole_word() {
        let l = Label::parse("|CffffffffS|Ringle Player").unwrap();
        assert_eq!(l.visible(), "Single Player");
        assert_eq!(l.hotkey(), Some('S'));
        // The empty prefix is not emitted, so the runs are the hotkey then the
        // remainder.
        assert_eq!(l.spans.len(), 2);
        assert!(l.spans[0].hotkey);
        assert_eq!(l.spans[0].text, "S");
        assert_eq!(l.spans[1].text, "ingle Player");
    }

    /// An **interior** hotkey, which is the shape `KEY_EXIT` ships:
    /// `E|Cffffffffx|Rit` is "Exit" with `x` highlighted, and the leading `E`
    /// belongs to the label rather than being dropped.
    #[test]
    fn an_interior_hotkey_keeps_the_leading_text() {
        let l = Label::parse("E|Cffffffffx|Rit").unwrap();
        assert_eq!(l.visible(), "Exit");
        assert_eq!(l.hotkey(), Some('x'));
        assert_eq!(
            l.spans[0],
            Span {
                text: "E".to_string(),
                hotkey: false
            }
        );
        assert_eq!(l.spans[1].text, "x");
        assert!(l.spans[1].hotkey);
        assert_eq!(l.spans[2].text, "it");
    }

    /// Markup that is malformed is refused, not silently flattened.
    #[test]
    fn malformed_markup_is_refused() {
        assert!(matches!(
            Label::parse("|CffS|Ringle"),
            Err(TextError::BadColourRun(_))
        ));
        assert!(matches!(
            Label::parse("|CffffffffSingle"),
            Err(TextError::UnterminatedColour(_))
        ));
        assert!(matches!(
            Label::parse("plain|R"),
            Err(TextError::UnterminatedColour(_))
        ));
    }

    /// A `GlobalStrings.fdf` value is found by its key.
    #[test]
    fn a_global_string_resolves_by_key() {
        let text = "\n    KEY_SINGLE_PLAYER               \"|CffffffffS|Ringle Player\",\n";
        let raw = global_string(text, "KEY_SINGLE_PLAYER").unwrap();
        assert_eq!(Label::parse(&raw).unwrap().visible(), "Single Player");
        assert!(global_string(text, "KEY_NOPE").is_none());
    }

    /// The style comes from the frame's own records.
    #[test]
    fn the_style_is_read_from_the_frame_records() {
        let text = r#"
Frame "TEXT" "StandardButtonTextTemplate" {
    FrameFont "MasterFont", 0.013, "",
    FontJustificationH JUSTIFYCENTER,
    FontJustificationV JUSTIFYMIDDLE,
    FontFlags "FIXEDSIZE",
    FontColor 0.99 0.827 0.0705 1.0,
    FontHighlightColor 1.0 1.0 1.0 1.0,
    FontDisabledColor 0.5 0.5 0.5 1.0,
    FontShadowColor 0.0 0.0 0.0 0.9,
    FontShadowOffset 0.001 -0.001,
}
"#;
        let s = text_style(text, "StandardButtonTextTemplate").unwrap();
        assert_eq!(s.font, "MasterFont");
        assert_eq!(s.height, 0.013);
        assert_eq!(s.justify_h, JustifyH::Center);
        assert_eq!(s.justify_v, JustifyV::Middle);
        assert!(s.fixed_size);
        assert_eq!(s.colour[0], 0.99);
        assert_eq!(s.shadow_offset, [0.001, -0.001]);
    }

    /// A malformed numeric record is refused rather than handed to the renderer:
    /// a `NaN` colour or a non-positive height renders as invisible or degenerate
    /// text, which reads as a rendering bug instead of a bad asset.
    #[test]
    fn a_malformed_numeric_record_is_refused() {
        let style_text = |font: &str, colour: &str, offset: &str| {
            format!(
                "Frame \"TEXT\" \"T\" {{\n    FrameFont \"MasterFont\", {font}, \"\",\n    FontJustificationH JUSTIFYCENTER,\n    FontJustificationV JUSTIFYMIDDLE,\n    FontFlags \"FIXEDSIZE\",\n    FontColor {colour},\n    FontHighlightColor 1.0 1.0 1.0 1.0,\n    FontDisabledColor 0.5 0.5 0.5 1.0,\n    FontShadowColor 0.0 0.0 0.0 0.9,\n    FontShadowOffset {offset},\n}}\n"
            )
        };
        // The well-formed case parses, so the refusals below are not vacuous.
        assert!(
            text_style(
                &style_text("0.013", "0.99 0.827 0.0705 1.0", "0.001 -0.001"),
                "T"
            )
            .is_ok()
        );
        for (font, colour, offset) in [
            ("0.0", "0.99 0.827 0.0705 1.0", "0.001 -0.001"),
            ("-0.013", "0.99 0.827 0.0705 1.0", "0.001 -0.001"),
            ("NaN", "0.99 0.827 0.0705 1.0", "0.001 -0.001"),
            ("0.013", "NaN 0.827 0.0705 1.0", "0.001 -0.001"),
            ("0.013", "0.99 0.827 0.0705 1.0", "inf -0.001"),
        ] {
            assert!(
                matches!(
                    text_style(&style_text(font, colour, offset), "T"),
                    Err(TextError::Unusable { .. })
                ),
                "accepted font={font} colour={colour} offset={offset}"
            );
        }
    }

    /// A text frame inherits its style from a **renamed** template, and a
    /// per-entry frame overrides one record while inheriting the rest — the shape
    /// the menu actually uses, where the template's name is not assumed.
    #[test]
    fn the_style_follows_inherits_and_a_per_entry_override_wins() {
        let templates = r#"
Frame "TEXT" "MenuButtonTextTemplate" {
    FrameFont "MasterFont", 0.013, "",
    FontJustificationH JUSTIFYCENTER,
    FontJustificationV JUSTIFYMIDDLE,
    FontFlags "FIXEDSIZE",
    FontColor 0.99 0.827 0.0705 1.0,
    FontHighlightColor 1.0 1.0 1.0 1.0,
    FontDisabledColor 0.5 0.5 0.5 1.0,
    FontShadowColor 0.0 0.0 0.0 0.9,
    FontShadowOffset 0.001 -0.001,
}
"#;
        let menu = r#"
Frame "TEXT" "SinglePlayerButtonText" INHERITS "MenuButtonTextTemplate" {
    Text "KEY_SINGLE_PLAYER",
}
Frame "TEXT" "ExitButtonText" INHERITS "MenuButtonTextTemplate" {
    FontColor 1.0 0.0 0.0 1.0,
    Text "KEY_EXIT",
}
"#;
        let texts = [menu, templates];
        // The frame's own block carries no font records: they come from whatever
        // template it names, so nothing is tied to one template's name.
        let s = text_style_in(&texts, "SinglePlayerButtonText").unwrap();
        assert_eq!(s.font, "MasterFont");
        assert_eq!(s.colour, [0.99, 0.827, 0.0705, 1.0]);
        // The override wins for the record it names; the rest are still inherited.
        let e = text_style_in(&texts, "ExitButtonText").unwrap();
        assert_eq!(e.colour, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(e.height, 0.013);
        assert_eq!(e.justify_h, JustifyH::Center);
    }

    /// A record repeated inside one block is ambiguous, not picked.
    #[test]
    fn a_duplicate_record_in_a_block_is_refused() {
        let text = "Frame \"TEXT\" \"T\" {\n    FrameFont \"MasterFont\", 0.013, \"\",\n    FontColor 1.0 1.0 1.0 1.0,\n    FontColor 0.0 0.0 0.0 1.0,\n    FontJustificationH JUSTIFYCENTER,\n    FontJustificationV JUSTIFYMIDDLE,\n    FontFlags \"FIXEDSIZE\",\n    FontHighlightColor 1.0 1.0 1.0 1.0,\n    FontDisabledColor 0.5 0.5 0.5 1.0,\n    FontShadowColor 0.0 0.0 0.0 0.9,\n    FontShadowOffset 0.001 -0.001,\n}\n";
        assert!(matches!(
            text_style(text, "T"),
            Err(TextError::Duplicate {
                record: "FontColor",
                ..
            })
        ));
    }

    /// A missing record is an error, never a default.
    #[test]
    fn a_missing_record_is_reported() {
        let text = r#"
Frame "TEXT" "T" {
    FrameFont "MasterFont", 0.013, "",
    FontJustificationH JUSTIFYCENTER,
    FontJustificationV JUSTIFYMIDDLE,
    FontFlags "FIXEDSIZE",
    FontHighlightColor 1.0 1.0 1.0 1.0,
    FontDisabledColor 0.5 0.5 0.5 1.0,
    FontShadowColor 0.0 0.0 0.0 0.9,
    FontShadowOffset 0.001 -0.001,
}
"#;
        assert_eq!(
            text_style(text, "T"),
            Err(TextError::Missing {
                frame: "T".to_string(),
                record: "FontColor",
            })
        );
        assert!(matches!(
            text_style(text, "Nope"),
            Err(TextError::UnknownFrame(_))
        ));
    }
}

#[cfg(test)]
mod parent_tests {
    #[test]
    fn renderer_accepts_rgb_and_rgba_records_but_not_partial_or_nonfinite_color() {
        for (record, expected) in [
            ("0.12 0.34 0.56", Some([0.12, 0.34, 0.56, 1.0])),
            ("0.12 0.34 0.56 0.78", Some([0.12, 0.34, 0.56, 0.78])),
            ("0.12 0.34", None),
            ("0.12 NaN 0.56", None),
        ] {
            let f = crate::fdf::Fdf::parse(&format!(
                "Frame \"TEXT\" \"Label\" {{\nFrameFont \"Face\",0.02,\"\",\nFontColor {record},\n}}\n"
            )).unwrap();
            let result = super::render_style(&f, "Label");
            match expected {
                Some(expected) => assert_eq!(result.unwrap().colour, expected),
                None => assert!(result.is_err()),
            }
        }
    }

    #[test]
    fn actual_title_font_preserves_child_style() {
        let f = crate::fdf::Fdf::parse(
            r#"Frame "TEXTBUTTON" "IndexedTemplateTitle" {
FrameFont "TemplateFont",0.01,"",
Frame "TEXT" "SharedTitleChild" {
FontColor 0.2 0.3 0.4 1.0,
FontFlags "FIXEDSIZE",
FontJustificationH JUSTIFYRIGHT,
}
}
Frame "TEXTBUTTON" "ActualPopupTitle" {
FrameFont "InstanceFont",0.023,"",
ButtonText "SharedTitleChild",
}
"#,
        )
        .unwrap();
        let baseline = crate::menu_text::render_style(&f, "SharedTitleChild").unwrap();
        assert_eq!(baseline.font, "TemplateFont");
        let child =
            crate::menu_text::render_style_with_parent(&f, "SharedTitleChild", "ActualPopupTitle")
                .unwrap();
        assert_eq!(child.font, "InstanceFont");
        assert_eq!(child.height, 0.023);
        assert_eq!(child.colour, [0.2, 0.3, 0.4, 1.0]);
        assert!(child.fixed_size);
        assert_eq!(child.justify_h, crate::menu_text::JustifyH::Right);
    }
    #[test]
    fn child_font_wins_and_missing_font_is_not_invented() {
        let f = crate::fdf::Fdf::parse(
            r#"Frame "TEXT" "Child" {
FrameFont "ChildFont",0.017,"",
}
Frame "TEXTBUTTON" "Parent" {
FrameFont "ParentFont",0.025,"",
}
Frame "TEXT" "Missing" {
}
"#,
        )
        .unwrap();
        assert_eq!(
            crate::menu_text::render_style_with_parent(&f, "Child", "Parent")
                .unwrap()
                .font,
            "ChildFont"
        );
        assert!(crate::menu_text::render_style_with_parent(&f, "Missing", "Absent").is_err());
    }
}
