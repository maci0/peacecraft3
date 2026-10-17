//! Tooltip and HUD markup. Menu access-key colours stay in `menu_text::Label`.
//!
//! `|c` / `|C` plus eight hex digits is an authored `AARRGGBB` colour. `|r` /
//! `|R` returns to the caller's base colour. `|n` / `|N` is the newline
//! `menu_text` already treats as a line break. A closed `^^…^^` is the code
//! tag the script host refuses to substitute: the bytes stay in the run and
//! the tag is reported.
//!
//! Any other `|` form, including `||`, refuses the whole string. No
//! shipped-byte note defines `||` as an escape, so this parser does not
//! collapse it or keep it as a special form.

use std::fmt;

/// One run. `colour` is `None` when the run uses the caller's base colour.
/// A colour or reset ends the run. `|n` / `|N` stays inside the current run.
#[derive(Debug, Clone, PartialEq)]
pub struct RichSpan {
    /// Visible text, including a reported `^^` tag.
    pub text: String,
    /// Authored `AARRGGBB` channels, or inherited.
    pub colour: Option<[f32; 4]>,
}

/// A closed `^^` tag. The bytes are also in [`RichSpan::text`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedMarkup {
    /// Byte index of `form`.
    pub at: usize,
    /// The exact `^^…^^` slice.
    pub form: String,
}

/// Parsed markup. Empty `spans` means the input was empty.
#[derive(Debug, Clone, PartialEq)]
pub struct RichText {
    /// Runs in source order. Empty runs are omitted.
    pub spans: Vec<RichSpan>,
    /// Closed code tags, in source order.
    pub unsupported: Vec<UnsupportedMarkup>,
}

/// The whole string was refused. `source` is that string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RichTextError {
    /// `|` or `|c` / `|C` ended before a complete code, or `^^` never closed.
    Truncated {
        /// Byte index of the code.
        at: usize,
        /// The refused string.
        source: String,
    },
    /// `|` was not a complete colour, reset, or newline code.
    Malformed {
        /// Byte index of the code.
        at: usize,
        /// The refused string.
        source: String,
    },
}

impl fmt::Display for RichTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { at, source } => {
                write!(f, "truncated tooltip markup at {at} in {source:?}")
            }
            Self::Malformed { at, source } => {
                write!(f, "malformed tooltip markup at {at} in {source:?}")
            }
        }
    }
}

impl std::error::Error for RichTextError {}

/// Parses tooltip markup.
///
/// # Errors
///
/// [`RichTextError`] is atomic: the error carries the source and no spans.
pub fn parse(raw: &str) -> Result<RichText, RichTextError> {
    let mut spans = Vec::new();
    let mut unsupported = Vec::new();
    let mut current = String::new();
    let mut colour: Option<[f32; 4]> = None;
    let bytes = raw.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'^' && index + 1 < bytes.len() && bytes[index + 1] == b'^' {
            let Some(end) = code_tag_end(bytes, index + 2) else {
                return Err(RichTextError::Truncated {
                    at: index,
                    source: raw.to_string(),
                });
            };
            let form = raw[index..end].to_string();
            unsupported.push(UnsupportedMarkup {
                at: index,
                form: form.clone(),
            });
            current.push_str(&form);
            index = end;
            continue;
        }
        if bytes[index] == b'|' {
            if index + 1 >= bytes.len() {
                return Err(RichTextError::Truncated {
                    at: index,
                    source: raw.to_string(),
                });
            }
            match bytes[index + 1] {
                b'c' | b'C' => {
                    let start = index + 2;
                    let end = start + 8;
                    if end > raw.len() {
                        return Err(RichTextError::Truncated {
                            at: index,
                            source: raw.to_string(),
                        });
                    }
                    let Some(hex) = raw.get(start..end) else {
                        return Err(RichTextError::Malformed {
                            at: index,
                            source: raw.to_string(),
                        });
                    };
                    if hex.len() != 8
                        || hex.chars().count() != 8
                        || !hex.bytes().all(|byte| byte.is_ascii_hexdigit())
                    {
                        return Err(RichTextError::Malformed {
                            at: index,
                            source: raw.to_string(),
                        });
                    }
                    flush(&mut spans, &mut current, colour);
                    colour = Some(argb(hex));
                    index = end;
                }
                b'r' | b'R' => {
                    flush(&mut spans, &mut current, colour);
                    colour = None;
                    index += 2;
                }
                b'n' | b'N' => {
                    current.push('\n');
                    index += 2;
                }
                _ => {
                    return Err(RichTextError::Malformed {
                        at: index,
                        source: raw.to_string(),
                    });
                }
            }
            continue;
        }
        let Some(ch) = raw[index..].chars().next() else {
            return Err(RichTextError::Malformed {
                at: index,
                source: raw.to_string(),
            });
        };
        current.push(ch);
        index += ch.len_utf8();
    }
    flush(&mut spans, &mut current, colour);
    Ok(RichText { spans, unsupported })
}

fn code_tag_end(bytes: &[u8], mut index: usize) -> Option<usize> {
    while index + 1 < bytes.len() {
        if bytes[index] == b'^' && bytes[index + 1] == b'^' {
            return Some(index + 2);
        }
        index += 1;
    }
    None
}

fn flush(spans: &mut Vec<RichSpan>, current: &mut String, colour: Option<[f32; 4]>) {
    if current.is_empty() {
        return;
    }
    spans.push(RichSpan {
        text: std::mem::take(current),
        colour,
    });
}

/// Eight-bit `AARRGGBB`. 255 is that form's channel width.
fn argb(hex: &str) -> [f32; 4] {
    let packed = u32::from_str_radix(hex, 16).expect("eight hex digits");
    let channel = |shift: u32| {
        let byte = u8::try_from((packed >> shift) & 0xff).expect("masked to one byte");
        f32::from(byte) / 255.0
    };
    [channel(16), channel(8), channel(0), channel(24)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_colour_reset_and_newline_keep_authored_channels() {
        let parsed = parse("|cffff0000赤|r|n|cff00ff00AB|r").unwrap();
        assert!(parsed.unsupported.is_empty());
        assert_eq!(
            parsed.spans,
            vec![
                RichSpan {
                    text: "赤".into(),
                    colour: Some([1.0, 0.0, 0.0, 1.0]),
                },
                RichSpan {
                    text: "\n".into(),
                    colour: None,
                },
                RichSpan {
                    text: "AB".into(),
                    colour: Some([0.0, 1.0, 0.0, 1.0]),
                },
            ]
        );
    }

    #[test]
    fn tooltip_uppercase_colour_is_not_a_hotkey_and_blank_input_has_no_runs() {
        let parsed = parse("|Cffff0000A|NB|Rtail").unwrap();
        assert_eq!(parsed.spans[0].text, "A\nB");
        assert_eq!(parsed.spans[0].colour, Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(parsed.spans[1].text, "tail");
        assert_eq!(parsed.spans[1].colour, None);
        assert!(parse("").unwrap().spans.is_empty());
    }

    #[test]
    fn tooltip_closed_code_tag_is_kept_and_not_recoloured() {
        let parsed = parse("pre^^code|cffff0000^^post").unwrap();
        let visible: String = parsed.spans.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(visible, "pre^^code|cffff0000^^post");
        assert_eq!(parsed.unsupported[0].form, "^^code|cffff0000^^");
        assert!(parsed.spans.iter().all(|span| span.colour.is_none()));
    }

    #[test]
    fn tooltip_malformed_bar_and_truncated_colour_refuse_the_whole_string() {
        for sample in [
            "A||B",
            "|Tpath",
            "|",
            "|cffff",
            "|cffff0000Hi|c12",
            "|czzzzzzzz",
        ] {
            let error = parse(sample).expect_err(sample);
            let source = match &error {
                RichTextError::Truncated { source, .. }
                | RichTextError::Malformed { source, .. } => source.as_str(),
            };
            assert_eq!(source, sample);
        }
        assert!(matches!(
            parse("A||B"),
            Err(RichTextError::Malformed { .. })
        ));
        assert!(matches!(
            parse("|cffff"),
            Err(RichTextError::Truncated { .. })
        ));
        assert!(matches!(
            parse("before^^open"),
            Err(RichTextError::Truncated { .. })
        ));
    }

    #[test]
    fn tooltip_zero_alpha_is_kept() {
        let parsed = parse("|c00ff0000X|r").unwrap();
        assert_eq!(parsed.spans[0].colour, Some([1.0, 0.0, 0.0, 0.0]));
    }
}
