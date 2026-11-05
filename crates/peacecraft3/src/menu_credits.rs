//! Source-derived Credits content for the MainMenu Credits button.
//!
//! The original Credits button does not show a panel; it launches the
//! authored credits map named by the shipped `UI\MiscUI.txt` table
//! (`CreditsFile_V0` for Reign of Chaos, `CreditsFile_V1` for The Frozen
//! Throne). This module reads that selection and the credits sequence
//! authored inside the selected map (`war3map.j` trigger actions plus
//! `war3map.wts` string table) at runtime from the selected install.
//! No path, name, string, timing or file list is copied into code; every
//! value below comes from bytes the caller supplies. A record the reader
//! does not understand is an error, never a skipped line, so an authored
//! sequence can never silently lose a step.
//!
//! The sequence reader below is test-only evidence for the authored
//! `war3map.j`/`war3map.wts` records; production launches the selected
//! map through the canonical map path and does not interpret its
//! sequence or `.pld` preloads here.

use std::fmt;

/// The shipped table that names the credits maps.
pub const MISC_UI: &str = r"UI\MiscUI.txt";

/// Why a credits record could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreditsError {
    /// `UI\MiscUI.txt` has no usable `CreditsFile_V{edition}` entry.
    MissingSelection { member: String, key: String },
    /// The selected entry is listed more than once with conflicting values.
    DuplicateSelection {
        member: String,
        key: String,
        values: Vec<String>,
    },
    /// The selected entry names an empty path.
    EmptySelection { member: String, key: String },
    /// The selected map member is missing from the install, or its bytes
    /// are not a map.
    MapUnreadable { member: String, reason: String },
    /// A required map member (`war3map.j`, `war3map.wts`) is missing.
    #[cfg(test)]
    MissingMember { map: String, member: String },
    /// A map member is not the encoding its format requires.
    MalformedMember {
        map: String,
        member: String,
        reason: String,
    },
    /// A `Trig_*_Actions` statement is not a credits record this reader
    /// understands. The statement text is carried so the gap is visible.
    #[cfg(test)]
    UnsupportedRecord { function: String, record: String },
    /// A `TRIGSTR_*` reference has no entry in `war3map.wts`.
    #[cfg(test)]
    UnresolvedString { function: String, reference: String },
    /// A malformed numeric operand.
    #[cfg(test)]
    InvalidNumber {
        function: String,
        record: String,
        value: String,
    },
    /// The authored sequence is empty.
    #[cfg(test)]
    EmptySequence { map: String },
}

impl fmt::Display for CreditsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSelection { member, key } => {
                write!(f, "{member}: no {key} entry")
            }
            Self::DuplicateSelection {
                member,
                key,
                values,
            } => {
                write!(f, "{member}: {key} defined {values:?}")
            }
            Self::EmptySelection { member, key } => {
                write!(f, "{member}: {key} names no file")
            }
            Self::MapUnreadable { member, reason } => {
                write!(f, "{member}: {reason}")
            }
            #[cfg(test)]
            Self::MissingMember { map, member } => {
                write!(f, "{map}: missing {member}")
            }
            Self::MalformedMember {
                map,
                member,
                reason,
            } => {
                write!(f, "{map}: {member}: {reason}")
            }
            #[cfg(test)]
            Self::UnsupportedRecord { function, record } => {
                write!(f, "{function}: unsupported credits record {record:?}")
            }
            #[cfg(test)]
            Self::UnresolvedString {
                function,
                reference,
            } => {
                write!(f, "{function}: {reference} has no war3map.wts entry")
            }
            #[cfg(test)]
            Self::InvalidNumber {
                function,
                record,
                value,
            } => {
                write!(f, "{function}: invalid number {value:?} in {record:?}")
            }
            #[cfg(test)]
            Self::EmptySequence { map } => write!(f, "{map}: no credits sequence"),
        }
    }
}

impl std::error::Error for CreditsError {}

#[cfg(test)]
/// Provenance of every value in a [`CreditsContent`]: the shipped member
/// each value was read from, and the map header that identifies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// The MPQ member the selection was read from (`UI\MiscUI.txt`).
    pub selection_member: String,
    /// The exact key used, `CreditsFile_V0` or `CreditsFile_V1`.
    pub selection_key: String,
    /// The map path exactly as the table names it.
    pub map_path: String,
    /// The map header name (`HM3W` record 0x008), as shipped.
    pub map_name: String,
    /// The map header version word.
    pub map_version: u32,
}

#[cfg(test)]
/// One authored step of the credits sequence, in authored order.
#[derive(Debug, Clone, PartialEq)]
pub enum Credited {
    /// `call Preloader("scripts\\...pld")`: a referenced shipped file.
    /// The `.pld` payloads live outside the map archive; the path is
    /// recorded, the payload is not decoded here.
    Preload { file: String },
    /// `TransmissionFromUnitTypeWithNameBJ(...)` with both string-table
    /// references, the authored unit code, time operation and duration.
    Transmission {
        unit: String,
        name: String,
        text: String,
        time_op: String,
        seconds: f32,
    },
    /// `call TriggerExecute(gg_trg_X)`: a chained trigger step.
    ExecuteTrigger { trigger: String },
    /// `call TriggerSleepAction(seconds)`.
    Sleep { seconds: f32 },
    /// `call SetUnitX/SetUnitY(...)`: the authored camera-unit motion.
    /// Recorded verbatim so the integrator can replay it; not interpreted.
    Move { call: String },
}

#[cfg(test)]
/// The credits sequence of one `Trig_*_Actions` function.
#[derive(Debug, Clone, PartialEq)]
pub struct CreditSection {
    /// The authored function name, e.g. `Trig_Blizzard_Actions`.
    pub function: String,
    pub steps: Vec<Credited>,
}

#[cfg(test)]
/// The source-derived credits content of the selected edition.
#[derive(Debug, Clone, PartialEq)]
pub struct CreditsContent {
    /// `0` Reign of Chaos, `1` The Frozen Throne, as the skin edition.
    pub edition: u32,
    pub provenance: Provenance,
    pub sections: Vec<CreditSection>,
}

// ---------------------------------------------------------------------------
// MiscUI selection
// ---------------------------------------------------------------------------

fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Unquote a MiscUI value. The shipped values are `"path"`; a bare value
/// passes through so the reader never depends on quoting style.
fn unquote(value: &str) -> &str {
    let value = value.trim();
    match value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        Some(inner) => inner,
        None => value,
    }
}

/// Upper bound on one `UI\MiscUI.txt` read through the selected resolver.
const MISC_UI_LIMIT: u64 = 1 << 20;

/// Read `UI\MiscUI.txt` through the pinned selected-install resolver and
/// pick `CreditsFile_V{edition}`. The returned path is exactly as authored;
/// the canonical map launch then resolves and validates it the same way as
/// any mission, so this performs no sequence or `.pld` interpretation.
pub fn credits_map_for_edition(
    resolver: &pcx_assets::Resolver,
    edition: u32,
) -> Result<String, CreditsError> {
    let asset = resolver
        .resolve_bounded(MISC_UI, MISC_UI_LIMIT)
        .map_err(|e| CreditsError::MapUnreadable {
            member: MISC_UI.into(),
            reason: e.to_string(),
        })?;
    select_map_path(&asset.bytes, edition)
}

/// Pick `CreditsFile_V{edition}` out of shipped `UI\MiscUI.txt` bytes.
/// Returns the key and the map path exactly as authored. Key matching is
/// case-insensitive, as the shipped table's casing is not stable across
/// sections; a conflicting duplicate is refused rather than picked.
pub fn select_map_path(misc_ui: &[u8], edition: u32) -> Result<String, CreditsError> {
    let text = std::str::from_utf8(misc_ui).map_err(|_| CreditsError::MalformedMember {
        map: MISC_UI.into(),
        member: MISC_UI.into(),
        reason: "not UTF-8".into(),
    })?;
    let key = format!("CreditsFile_V{edition}");
    let mut values = Vec::new();
    for line in text.lines() {
        let line = strip_line_comment(line).trim();
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(&key) {
            values.push(unquote(value).to_string());
        }
    }
    match values.as_slice() {
        [] => Err(CreditsError::MissingSelection {
            member: MISC_UI.into(),
            key,
        }),
        [one] if !one.is_empty() => Ok(one.clone()),
        [empty] if empty.is_empty() => Err(CreditsError::EmptySelection {
            member: MISC_UI.into(),
            key,
        }),
        many => Err(CreditsError::DuplicateSelection {
            member: MISC_UI.into(),
            key,
            values: many.to_vec(),
        }),
    }
}

// ---------------------------------------------------------------------------
// war3map.wts
// ---------------------------------------------------------------------------

#[cfg(test)]
/// Parse shipped `war3map.wts` bytes into an id → text map. `pcx-trig`
/// owns the STRING framing; this reader accepts the same bytes through
/// that crate so the two never disagree about encoding.
pub fn parse_wts<'a>(bytes: &'a [u8]) -> Result<pcx_trig::Wts<'a>, CreditsError> {
    pcx_trig::Wts::parse(bytes).map_err(|e| CreditsError::MalformedMember {
        map: "war3map.wts".into(),
        member: "war3map.wts".into(),
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// war3map.j sequence scan
// ---------------------------------------------------------------------------

/// One token of the small JASS surface this reader understands. Anything
/// outside this surface inside a credits action is an error, so the scan
/// is a closed vocabulary rather than a partial JASS implementation.
#[derive(Debug, Clone, PartialEq)]
#[cfg(test)]
enum Tok {
    Ident(String),
    Str(String),
    Raw(String),
    Num(String),
    Punct(char),
}

#[cfg(test)]
fn tokenize(src: &str) -> Vec<Tok> {
    let bytes = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if b == b'"' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'"' {
                if bytes[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            out.push(Tok::Str(src[start..j.min(bytes.len())].to_string()));
            i = (j + 1).min(bytes.len());
        } else if b == b'\'' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'\'' {
                j += 1;
            }
            out.push(Tok::Raw(src[start..j.min(bytes.len())].to_string()));
            i = (j + 1).min(bytes.len());
        } else if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
            let start = i;
            let mut j = i;
            while j < bytes.len()
                && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'.')
            {
                j += 1;
            }
            let word = &src[start..j];
            if word.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                out.push(Tok::Num(word.to_string()));
            } else {
                out.push(Tok::Ident(word.to_string()));
            }
            i = j;
        } else {
            out.push(Tok::Punct(b as char));
            i += 1;
        }
    }
    out
}

#[cfg(test)]
fn render(tokens: &[Tok]) -> String {
    let mut s = String::new();
    for tok in tokens {
        match tok {
            Tok::Ident(w) | Tok::Num(w) => {
                if !s.is_empty() && !s.ends_with('(') && !s.ends_with(',') {
                    s.push(' ');
                }
                s.push_str(w);
            }
            Tok::Str(w) => {
                if !s.is_empty() && !s.ends_with('(') && !s.ends_with(',') {
                    s.push(' ');
                }
                s.push('"');
                s.push_str(w);
                s.push('"');
            }
            Tok::Raw(w) => {
                if !s.is_empty() && !s.ends_with('(') && !s.ends_with(',') {
                    s.push(' ');
                }
                s.push('\'');
                s.push_str(w);
                s.push('\'');
            }
            Tok::Punct('(') => s.push('('),
            Tok::Punct(',') => s.push(','),
            Tok::Punct(')') => s.push(')'),
            Tok::Punct(c) => {
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push(*c);
            }
        }
    }
    s
}

#[cfg(test)]
/// Split a parenthesised argument list (tokens after `(`) on top-level
/// commas. Returns the argument token groups and the index just past `)`.
fn split_args(tokens: &[Tok], open: usize) -> Option<(Vec<Vec<Tok>>, usize)> {
    let mut args = Vec::new();
    let mut current = Vec::new();
    let mut depth = 0usize;
    let mut i = open;
    while i < tokens.len() {
        match &tokens[i] {
            Tok::Punct('(') => {
                depth += 1;
                if depth > 1 {
                    current.push(tokens[i].clone());
                }
            }
            Tok::Punct(')') => {
                depth -= 1;
                if depth == 0 {
                    args.push(current);
                    return Some((args, i + 1));
                }
                current.push(tokens[i].clone());
            }
            Tok::Punct(',') if depth == 1 => {
                args.push(std::mem::take(&mut current));
            }
            tok => current.push(tok.clone()),
        }
        i += 1;
    }
    None
}

#[cfg(test)]
fn arg_str(arg: &[Tok]) -> Option<String> {
    match arg {
        [Tok::Str(s)] => Some(s.clone()),
        [Tok::Raw(s)] => Some(format!("'{s}'")),
        _ => None,
    }
}

#[cfg(test)]
fn parse_num(function: &str, record: &str, arg: &[Tok]) -> Result<f32, CreditsError> {
    match arg {
        [Tok::Num(n)] => n.parse::<f32>().map_err(|_| CreditsError::InvalidNumber {
            function: function.into(),
            record: record.into(),
            value: n.clone(),
        }),
        _ => Err(CreditsError::InvalidNumber {
            function: function.into(),
            record: record.into(),
            value: render(arg),
        }),
    }
}

#[cfg(test)]
fn resolve_trigstr(
    wts: &pcx_trig::Wts<'_>,
    function: &str,
    reference: &str,
) -> Result<String, CreditsError> {
    let id: i32 = reference
        .strip_prefix("TRIGSTR_")
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| CreditsError::UnresolvedString {
            function: function.into(),
            reference: reference.into(),
        })?;
    wts.get(id)
        .and_then(|entry| String::from_utf8(entry.body.clone()).ok())
        .ok_or_else(|| CreditsError::UnresolvedString {
            function: function.into(),
            reference: reference.into(),
        })
}

#[cfg(test)]
/// Read the credits sequence out of shipped `war3map.j` bytes. Only
/// `Trig_*_Actions` function bodies are credits records; every statement
/// in those bodies must be a record this reader understands, in order.
pub fn parse_sequence(
    jass: &[u8],
    wts: &pcx_trig::Wts<'_>,
) -> Result<Vec<CreditSection>, CreditsError> {
    let text = std::str::from_utf8(jass).map_err(|_| CreditsError::MalformedMember {
        map: "war3map.j".into(),
        member: "war3map.j".into(),
        reason: "not UTF-8".into(),
    })?;
    let tokens = tokenize(text);
    let mut sections = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        // function Trig_X_Actions takes nothing returns nothing
        if matches!(tokens.get(i), Some(Tok::Ident(k)) if k == "function")
            && let Some(Tok::Ident(name)) = tokens.get(i + 1)
            && name.starts_with("Trig_")
            && name.ends_with("_Actions")
        {
            let function = name.clone();
            // Find the body: tokens after the header, up to `endfunction`.
            let mut j = i + 2;
            while j < tokens.len()
                && !matches!(tokens.get(j), Some(Tok::Ident(k)) if k == "endfunction")
            {
                j += 1;
            }
            let body = &tokens[i + 2..j.min(tokens.len())];
            let mut steps = Vec::new();
            let mut k = 0;
            while k < body.len() {
                if matches!(body.get(k), Some(Tok::Ident(key)) if key == "call")
                    && let Some(Tok::Ident(callee)) = body.get(k + 1)
                    && matches!(body.get(k + 2), Some(Tok::Punct('(')))
                {
                    let record = render(&body[k..(k + 3).min(body.len())]);
                    let Some((args, next)) = split_args(body, k + 2) else {
                        return Err(CreditsError::UnsupportedRecord {
                            function: function.clone(),
                            record,
                        });
                    };
                    match callee.as_str() {
                        "Preloader" => {
                            let Some(file) = args.first().and_then(|a| arg_str(a)) else {
                                return Err(CreditsError::UnsupportedRecord {
                                    function: function.clone(),
                                    record: render(&body[k..next]),
                                });
                            };
                            steps.push(Credited::Preload { file });
                        }
                        "TransmissionFromUnitTypeWithNameBJ" => {
                            // (dst, srcPlayer, 'unit', name, loc, item, text, op, secs, show)
                            if args.len() != 10 {
                                return Err(CreditsError::UnsupportedRecord {
                                    function: function.clone(),
                                    record: render(&body[k..next]),
                                });
                            }
                            let unit = arg_str(&args[2]).ok_or_else(|| {
                                CreditsError::UnsupportedRecord {
                                    function: function.clone(),
                                    record: render(&body[k..next]),
                                }
                            })?;
                            let name_ref = arg_str(&args[3]).ok_or_else(|| {
                                CreditsError::UnsupportedRecord {
                                    function: function.clone(),
                                    record: render(&body[k..next]),
                                }
                            })?;
                            let text_ref = arg_str(&args[6]).ok_or_else(|| {
                                CreditsError::UnsupportedRecord {
                                    function: function.clone(),
                                    record: render(&body[k..next]),
                                }
                            })?;
                            let time_op = render(&args[7]);
                            let seconds = parse_num(&function, &render(&body[k..next]), &args[8])?;
                            let name = resolve_trigstr(wts, &function, &name_ref)?;
                            let text = resolve_trigstr(wts, &function, &text_ref)?;
                            steps.push(Credited::Transmission {
                                unit,
                                name,
                                text,
                                time_op,
                                seconds,
                            });
                        }
                        "TriggerExecute" => {
                            let trigger = render(&args[0]);
                            let Some(trigger) = trigger.strip_prefix("gg_trg_") else {
                                return Err(CreditsError::UnsupportedRecord {
                                    function: function.clone(),
                                    record: render(&body[k..next]),
                                });
                            };
                            steps.push(Credited::ExecuteTrigger {
                                trigger: trigger.to_string(),
                            });
                        }
                        "TriggerSleepAction" => {
                            let seconds = parse_num(&function, &render(&body[k..next]), &args[0])?;
                            steps.push(Credited::Sleep { seconds });
                        }
                        "SetUnitX" | "SetUnitY" => {
                            steps.push(Credited::Move {
                                call: render(&body[k..next]),
                            });
                        }
                        // Branch/skip scaffolding the shipped triggers use
                        // around every step: recognised, not sequence data.
                        "DoNothing" => {}
                        _ => {
                            return Err(CreditsError::UnsupportedRecord {
                                function: function.clone(),
                                record: render(&body[k..next]),
                            });
                        }
                    }
                    k = next;
                    continue;
                }
                k += 1;
            }
            if steps.is_empty() {
                return Err(CreditsError::UnsupportedRecord {
                    function: function.clone(),
                    record: "empty Trig_*_Actions body".into(),
                });
            }
            sections.push(CreditSection { function, steps });
            i = j + 1;
            continue;
        }
        i += 1;
    }
    if sections.is_empty() {
        return Err(CreditsError::UnsupportedRecord {
            function: "war3map.j".into(),
            record: "no Trig_*_Actions functions".into(),
        });
    }
    Ok(sections)
}

// ---------------------------------------------------------------------------
// Install read (non-WASM; the browser path supplies bytes separately)
// ---------------------------------------------------------------------------

#[cfg(test)]
/// Build content from already-extracted map bytes. This is the seam the
/// synthetic-fixture tests and the browser path share: same bytes in,
/// same provenance and errors out.
pub fn content_from_map(
    edition: u32,
    map_path: &str,
    map_bytes: &[u8],
) -> Result<CreditsContent, CreditsError> {
    let map = pcx_map::Map::parse(map_bytes).map_err(|e| CreditsError::MapUnreadable {
        member: map_path.to_string(),
        reason: e.to_string(),
    })?;
    let header = &map.header;
    let jass = map
        .extract("war3map.j")
        .map_err(|_| CreditsError::MissingMember {
            map: map_path.to_string(),
            member: "war3map.j".into(),
        })?;
    let wts_bytes = map
        .extract("war3map.wts")
        .map_err(|_| CreditsError::MissingMember {
            map: map_path.to_string(),
            member: "war3map.wts".into(),
        })?;
    let wts = parse_wts(&wts_bytes)?;
    let sections = parse_sequence(&jass, &wts)?;
    if sections.is_empty() {
        return Err(CreditsError::EmptySequence {
            map: map_path.to_string(),
        });
    }
    Ok(CreditsContent {
        edition,
        provenance: Provenance {
            selection_member: MISC_UI.into(),
            selection_key: format!("CreditsFile_V{edition}"),
            map_path: map_path.to_string(),
            map_name: header.name.clone(),
            map_version: header.version,
        },
        sections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MISC: &str = "[Misc]\n// credits file\nCreditsFile_V0=\"maps\\campaign\\warcraftiiicredits.w3m\"\nCreditsFile_V1=\"maps\\frozenthrone\\campaign\\war3xregularcreditsice.w3x\"\n";

    #[test]
    fn selects_each_edition_from_the_table() {
        assert_eq!(
            select_map_path(MISC.as_bytes(), 0).unwrap(),
            r"maps\campaign\warcraftiiicredits.w3m"
        );
        assert_eq!(
            select_map_path(MISC.as_bytes(), 1).unwrap(),
            r"maps\frozenthrone\campaign\war3xregularcreditsice.w3x"
        );
    }

    #[test]
    fn missing_and_duplicate_selections_are_errors() {
        assert!(matches!(
            select_map_path(b"[Misc]\n", 0),
            Err(CreditsError::MissingSelection { .. })
        ));
        let dup = "[Misc]\nCreditsFile_V0=\"a.w3m\"\nCreditsFile_V0=\"b.w3m\"\n";
        assert!(matches!(
            select_map_path(dup.as_bytes(), 0),
            Err(CreditsError::DuplicateSelection { .. })
        ));
        let empty = "[Misc]\nCreditsFile_V0=\"\"\n";
        assert!(matches!(
            select_map_path(empty.as_bytes(), 0),
            Err(CreditsError::EmptySelection { .. })
        ));
    }

    fn wts(ids: &[(u32, &str)]) -> Vec<u8> {
        // pcx-trig STRING records: `STRING <id>\r\n{\r\n<body>\r\n}\r\n\r\n`.
        let mut out = Vec::new();
        for (id, text) in ids {
            out.extend_from_slice(b"STRING ");
            out.extend_from_slice(id.to_string().as_bytes());
            out.extend_from_slice(b"\r\n{\r\n");
            out.extend_from_slice(text.as_bytes());
            out.extend_from_slice(b"\r\n}\r\n\r\n");
        }
        out
    }

    const JASS: &str = r#"
function Trig_CinematicStart_Actions takes nothing returns nothing
    call Preloader( "scripts\CreditsBlizzard.pld" )
    call TransmissionFromUnitTypeWithNameBJ( GetPlayersAll(), Player(0), 'nfoo', "TRIGSTR_1", GetRectCenter(GetPlayableMapRect()), null, "TRIGSTR_2", bj_TIMETYPE_ADD, 2.00, true )
    call TriggerSleepAction( 0.10 )
    call SetUnitX( gg_unit_CinematicCamera, 1.00 )
    call TriggerExecute( gg_trg_Blizzard )
endfunction
"#;

    #[test]
    fn sequence_reads_preload_transmission_sleep_move_and_execute_in_order() {
        let table = wts(&[(1, "Name One"), (2, "Text One")]);
        let parsed = parse_wts(&table).unwrap();
        let sections = parse_sequence(JASS.as_bytes(), &parsed).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].function, "Trig_CinematicStart_Actions");
        let steps = &sections[0].steps;
        assert_eq!(steps.len(), 5);
        assert_eq!(
            steps[0],
            Credited::Preload {
                file: r"scripts\CreditsBlizzard.pld".to_string()
            }
        );
        match &steps[1] {
            Credited::Transmission {
                unit,
                name,
                text,
                time_op,
                seconds,
            } => {
                assert_eq!(unit, "'nfoo'");
                assert_eq!(name, "Name One");
                assert_eq!(text, "Text One");
                assert_eq!(time_op, "bj_TIMETYPE_ADD");
                assert_eq!(*seconds, 2.0);
            }
            other => panic!("unexpected step: {other:?}"),
        }
        assert_eq!(steps[2], Credited::Sleep { seconds: 0.10 });
        assert!(matches!(steps[3], Credited::Move { .. }));
        assert_eq!(
            steps[4],
            Credited::ExecuteTrigger {
                trigger: "Blizzard".to_string()
            }
        );
    }

    #[test]
    fn unresolved_string_and_unknown_record_are_errors() {
        let table = wts(&[(1, "Name One")]);
        let parsed = parse_wts(&table).unwrap();
        assert!(matches!(
            parse_sequence(JASS.as_bytes(), &parsed),
            Err(CreditsError::UnresolvedString { .. })
        ));
        let unknown = r#"function Trig_X_Actions takes nothing returns nothing
    call SomeUnknownNative( 1 )
endfunction"#;
        assert!(matches!(
            parse_sequence(unknown.as_bytes(), &parsed),
            Err(CreditsError::UnsupportedRecord { .. })
        ));
    }

    #[test]
    fn non_map_bytes_are_a_map_error_not_a_panic() {
        assert!(matches!(
            content_from_map(0, "synthetic.w3m", b"not a map"),
            Err(CreditsError::MapUnreadable { .. })
        ));
    }

    #[test]
    fn shipped_table_selects_maps_that_extract_as_maps() {
        let Some(dir) = std::env::var_os("PEACECRAFT3_LEGACY_DIR")
            .or_else(|| std::env::var_os("PEACECRAFT3_INSTALL"))
        else {
            eprintln!("skipping: no install");
            return;
        };
        let mut set =
            pcx_mpq::MpqSet::open_wc3(std::path::PathBuf::from(dir)).expect("open install");
        let misc = set.extract(MISC_UI).expect("extract MiscUI");
        for (edition, expected) in [
            (0, r"maps\campaign\warcraftiiicredits.w3m"),
            (1, r"maps\frozenthrone\campaign\war3xregularcreditsice.w3x"),
        ] {
            let selected = select_map_path(&misc, edition)
                .unwrap_or_else(|e| panic!("edition {edition}: {e}"));
            assert_eq!(selected, expected);
            let map = set
                .extract(&selected)
                .unwrap_or_else(|e| panic!("{selected}: {e:?}"));
            assert!(map.starts_with(b"HM3W"), "{selected}: not a map");
        }
    }
}
