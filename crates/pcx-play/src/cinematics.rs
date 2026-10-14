//! Checked declaration ingestion for cinematic/dialog FDF inputs.
//!
//! This is an asset dependency boundary, not a renderer or native Host. It retains
//! authored records and inheritance/anchor relationships without choosing property
//! override rules, expanding WITHCHILDREN, measuring text, or inventing rectangles.

use std::{collections::BTreeMap, fmt};

/// Explicit input identity; adjacent executable versions are not archive identity.
#[derive(Debug, Clone, Copy)]
pub struct FdfAsset<'a> {
    pub label: &'a str,
    pub edition: &'a str,
    pub version: &'a str,
    pub source: &'a str,
    pub text: &'a str,
}

/// Caller policy, not original game limits. All limits must be supplied.
#[derive(Debug, Clone, Copy)]
pub struct FdfLimits {
    pub input_bytes: usize,
    pub tokens: usize,
    pub frames: usize,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub label: String,
    pub edition: String,
    pub version: String,
    pub source: String,
    /// Reproducibility checksum over the exact caller-supplied UTF-8 bytes.
    pub fnv1a64: u64,
}

/// A value retains its authored spelling; colors are never padded with alpha.
#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    Quoted(String),
    Identifier(String),
    Number { spelling: String, value: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub key: String,
    pub values: Vec<Atom>,
    pub offset: usize,
    /// Full authored record, including punctuation. Unknown properties stay raw.
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Point {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}
impl Point {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "TOPLEFT" => Self::TopLeft,
            "TOP" => Self::Top,
            "TOPRIGHT" => Self::TopRight,
            "LEFT" => Self::Left,
            "CENTER" => Self::Center,
            "RIGHT" => Self::Right,
            "BOTTOMLEFT" => Self::BottomLeft,
            "BOTTOM" => Self::Bottom,
            "BOTTOMRIGHT" => Self::BottomRight,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Anchor {
    pub point: Point,
    pub relative_name: String,
    pub relative_point: Point,
    pub x: f64,
    pub y: f64,
    pub record: usize,
}

/// Index in this catalog. IDs are metadata indices, never native handle values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inheritance {
    pub template: FrameId,
    pub with_children: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub source: usize,
    pub offset: usize,
    pub kind: String,
    pub name: String,
    pub lexical_parent: Option<FrameId>,
    pub children: Vec<FrameId>,
    pub inherits: Option<Inheritance>,
    pub records: Vec<Record>,
    pub anchors: Vec<Anchor>,
    pub set_all_points: bool,
}
impl Frame {
    /// Only this declaration's record, without inferred inheritance precedence.
    pub fn record(&self, key: &str) -> Option<&Record> {
        self.records.iter().find(|record| record.key == key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyKind {
    Include,
    Template,
    Anchor,
    Font,
    Art,
    FrameReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub source: usize,
    pub frame: Option<FrameId>,
    pub offset: usize,
    pub kind: DependencyKind,
    pub name: String,
}

/// No implicit file loading or global later-definition precedence.
#[derive(Debug, Clone)]
pub struct FdfCatalog {
    sources: Vec<SourceIdentity>,
    frames: Vec<Frame>,
    dependencies: Vec<Dependency>,
    names: BTreeMap<String, FrameId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FdfErrorKind {
    MissingIdentity(&'static str),
    Budget(&'static str),
    Syntax(&'static str),
    Unterminated(&'static str),
    InvalidNumber(String),
    InvalidProperty(String),
    Duplicate(String),
    UnknownTemplate(String),
    InheritanceCycle(String),
    UnknownFrame(FrameId),
    MissingBinding(String),
    MissingParentContext,
    Unsupported(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FdfError {
    pub source: String,
    pub offset: usize,
    pub kind: FdfErrorKind,
}
impl fmt::Display for FdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at byte {}: {:?}",
            self.source, self.offset, self.kind
        )
    }
}
impl std::error::Error for FdfError {}

/// Explicit runtime target identity from the caller. No coordinate box is assumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Frame(FrameId),
    Context(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameBinding {
    pub authored_name: String,
    pub target: Target,
}
#[derive(Debug, Clone, PartialEq)]
pub struct BoundAnchor {
    pub authored: Anchor,
    pub target: Target,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Relationships {
    pub frame: FrameId,
    pub anchors: Vec<BoundAnchor>,
    /// Only present for an authored SetAllPoints. The caller supplies its target.
    pub all_points: Option<Target>,
    /// Direct inheritance edge, retained rather than executed or flattened.
    pub inherits: Option<Inheritance>,
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Quoted(String),
    Word(String),
    Open,
    Close,
    Comma,
    Newline,
}
#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
}

fn error(source: &str, offset: usize, kind: FdfErrorKind) -> FdfError {
    FdfError {
        source: source.into(),
        offset,
        kind,
    }
}
fn digest(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn lex(asset: FdfAsset<'_>, budget: usize) -> Result<Vec<Token>, FdfError> {
    let raw = asset.text.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < raw.len() {
        let start = at;
        let kind = match raw[at] {
            b' ' | b'\t' | b'\r' => {
                at += 1;
                continue;
            }
            b'\n' => {
                at += 1;
                TokenKind::Newline
            }
            b'/' if raw.get(at + 1) == Some(&b'/') => {
                while at < raw.len() && raw[at] != b'\n' {
                    at += 1;
                }
                continue;
            }
            b'/' if raw.get(at + 1) == Some(&b'*') => {
                at += 2;
                while at + 1 < raw.len() && &raw[at..at + 2] != b"*/" {
                    at += 1;
                }
                if at + 1 >= raw.len() {
                    return Err(error(
                        asset.label,
                        start,
                        FdfErrorKind::Unterminated("comment"),
                    ));
                }
                // Keep line boundaries even when comments consume them.
                if raw[start..at].contains(&b'\n') {
                    if tokens.len() >= budget {
                        return Err(error(asset.label, start, FdfErrorKind::Budget("tokens")));
                    }
                    tokens.push(Token {
                        kind: TokenKind::Newline,
                        start,
                        end: at + 2,
                    });
                }
                at += 2;
                continue;
            }
            b'"' => {
                at += 1;
                let content = at;
                while at < raw.len() && raw[at] != b'"' {
                    if raw[at] == b'\\' && raw.get(at + 1) == Some(&b'"') {
                        return Err(error(
                            asset.label,
                            at,
                            FdfErrorKind::Unsupported("quoted-string escape".into()),
                        ));
                    }
                    if matches!(raw[at], b'\n' | b'\r') {
                        return Err(error(
                            asset.label,
                            start,
                            FdfErrorKind::Unterminated("string"),
                        ));
                    }
                    at += 1;
                }
                if at == raw.len() {
                    return Err(error(
                        asset.label,
                        start,
                        FdfErrorKind::Unterminated("string"),
                    ));
                }
                let value = asset.text[content..at].to_string();
                at += 1;
                TokenKind::Quoted(value)
            }
            b'{' => {
                at += 1;
                TokenKind::Open
            }
            b'}' => {
                at += 1;
                TokenKind::Close
            }
            b',' => {
                at += 1;
                TokenKind::Comma
            }
            _ => {
                while at < raw.len()
                    && !raw[at].is_ascii_whitespace()
                    && !matches!(raw[at], b'{' | b'}' | b',' | b'"' | b'/')
                {
                    at += 1;
                }
                if at == start {
                    return Err(error(
                        asset.label,
                        start,
                        FdfErrorKind::Syntax("unexpected byte"),
                    ));
                }
                TokenKind::Word(asset.text[start..at].into())
            }
        };
        if tokens.len() >= budget {
            return Err(error(asset.label, start, FdfErrorKind::Budget("tokens")));
        }
        tokens.push(Token {
            kind,
            start,
            end: at,
        });
    }
    Ok(tokens)
}

fn atom(source: &str, token: &Token) -> Result<Atom, FdfError> {
    match &token.kind {
        TokenKind::Quoted(value) => Ok(Atom::Quoted(value.clone())),
        TokenKind::Word(word) => {
            let number_like = word.as_bytes()[0].is_ascii_digit()
                || matches!(word.as_bytes()[0], b'+' | b'-' | b'.')
                || matches!(
                    word.to_ascii_lowercase().as_str(),
                    "nan" | "inf" | "infinity"
                );
            if !number_like {
                return Ok(Atom::Identifier(word.clone()));
            }
            let value = word
                .strip_suffix('f')
                .unwrap_or(word)
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .ok_or_else(|| {
                    error(
                        source,
                        token.start,
                        FdfErrorKind::InvalidNumber(word.clone()),
                    )
                })?;
            Ok(Atom::Number {
                spelling: word.clone(),
                value,
            })
        }
        _ => Err(error(
            source,
            token.start,
            FdfErrorKind::Syntax("expected value"),
        )),
    }
}

fn number(value: &Atom) -> Option<f64> {
    if let Atom::Number { value, .. } = value {
        Some(*value)
    } else {
        None
    }
}
fn quoted(value: &Atom) -> Option<&str> {
    if let Atom::Quoted(value) = value {
        Some(value)
    } else {
        None
    }
}
fn point(value: &Atom) -> Option<Point> {
    if let Atom::Identifier(value) = value {
        Point::parse(value)
    } else {
        None
    }
}

fn validate_record(source: &str, record: &Record) -> Result<Option<Anchor>, FdfError> {
    let values = &record.values;
    let numeric =
        |count| values.len() == count && values.iter().all(|value| number(value).is_some());
    let valid = match record.key.as_str() {
        "Width" | "Height" | "BackdropCornerSize" | "BackdropBackgroundSize" => numeric(1),
        "BackdropBackgroundInsets" => numeric(4),
        "FontColor" | "FontHighlightColor" | "FontDisabledColor" | "FontShadowColor" => {
            numeric(3) || numeric(4)
        }
        "FrameFont" => {
            values.len() == 3
                && quoted(&values[0]).is_some()
                && number(&values[1]).is_some()
                && quoted(&values[2]).is_some()
        }
        "SetAllPoints"
        | "UseActiveContext"
        | "DecorateFileNames"
        | "BackdropBlendAll"
        | "BackdropTileBackground" => values.is_empty(),
        "BackdropBackground"
        | "BackdropEdgeFile"
        | "HighlightAlphaFile"
        | "ButtonText"
        | "DialogBackdrop"
        | "ControlBackdrop"
        | "ControlPushedBackdrop"
        | "ControlDisabledBackdrop"
        | "ControlDisabledPushedBackdrop"
        | "ControlMouseOverHighlight"
        | "PopupArrowFrame"
        | "PopupMenuFrame"
        | "PopupTitleFrame"
        | "TextAreaScrollBar"
        | "SliderThumbButtonFrame"
        | "ScrollBarDecButtonFrame"
        | "ScrollBarIncButtonFrame"
        | "CheckBoxCheckHighlight"
        | "CheckBoxDisabledCheckHighlight" => values.len() == 1 && quoted(&values[0]).is_some(),
        "SetPoint" => {
            values.len() == 5
                && point(&values[0]).is_some()
                && quoted(&values[1]).is_some()
                && point(&values[2]).is_some()
                && number(&values[3]).is_some()
                && number(&values[4]).is_some()
        }
        _ => true, // Preserved, uninterpreted authored record, never a default.
    };
    if !valid {
        return Err(error(
            source,
            record.offset,
            FdfErrorKind::InvalidProperty(record.key.clone()),
        ));
    }
    if record.key == "SetPoint" {
        return Ok(Some(Anchor {
            point: point(&values[0]).expect("validated"),
            relative_name: quoted(&values[1]).expect("validated").into(),
            relative_point: point(&values[2]).expect("validated"),
            x: number(&values[3]).expect("validated"),
            y: number(&values[4]).expect("validated"),
            record: 0,
        }));
    }
    Ok(None)
}

struct Reader<'a> {
    source: &'a str,
    tokens: &'a [Token],
    at: usize,
}
impl Reader<'_> {
    fn skip_lines(&mut self) {
        while self
            .tokens
            .get(self.at)
            .is_some_and(|token| token.kind == TokenKind::Newline)
        {
            self.at += 1;
        }
    }
    fn next(&mut self) -> Result<&Token, FdfError> {
        let token = self.tokens.get(self.at).ok_or_else(|| {
            error(
                self.source,
                self.tokens.last().map_or(0, |t| t.end),
                FdfErrorKind::Unterminated("declaration"),
            )
        })?;
        self.at += 1;
        Ok(token)
    }
    fn string(&mut self) -> Result<String, FdfError> {
        self.skip_lines();
        let source = self.source;
        let token = self.next()?;
        if let TokenKind::Quoted(value) = &token.kind {
            Ok(value.clone())
        } else {
            Err(error(
                source,
                token.start,
                FdfErrorKind::Syntax("expected quoted name"),
            ))
        }
    }
    fn record(
        &mut self,
        asset: FdfAsset<'_>,
        key: String,
        start: usize,
    ) -> Result<Record, FdfError> {
        let mut values = Vec::new();
        let mut comma = false;
        let mut separators = Vec::new();
        let mut end = start;
        while let Some(token) = self.tokens.get(self.at) {
            if matches!(token.kind, TokenKind::Newline | TokenKind::Close) {
                break;
            }
            self.at += 1;
            end = token.end;
            if token.kind == TokenKind::Comma {
                if comma {
                    return Err(error(
                        asset.label,
                        token.start,
                        FdfErrorKind::Syntax("empty comma-separated value"),
                    ));
                }
                separators.push(values.len());
                comma = true;
            } else {
                if comma && values.is_empty() {
                    return Err(error(
                        asset.label,
                        token.start,
                        FdfErrorKind::Syntax("comma before first value"),
                    ));
                }
                values.push(atom(asset.label, token)?);
                comma = false;
            }
        }
        if !comma {
            return Err(error(
                asset.label,
                start,
                FdfErrorKind::Syntax("record requires trailing comma"),
            ));
        }
        if (key == "SetPoint" && separators != [1, 2, 3, 4, 5])
            || (key == "FrameFont" && separators != [1, 2, 3])
        {
            return Err(error(
                asset.label,
                start,
                FdfErrorKind::InvalidProperty(key),
            ));
        }
        let record = Record {
            key,
            values,
            offset: start,
            raw: asset.text[start..end].into(),
        };
        validate_record(asset.label, &record)?;
        Ok(record)
    }
}

impl FdfCatalog {
    pub fn parse(assets: &[FdfAsset<'_>], limits: FdfLimits) -> Result<Self, FdfError> {
        let total = assets
            .iter()
            .try_fold(0usize, |sum, asset| sum.checked_add(asset.text.len()));
        if total.is_none_or(|size| size > limits.input_bytes) {
            return Err(error("FDF inputs", 0, FdfErrorKind::Budget("input bytes")));
        }
        let mut catalog = Self {
            sources: Vec::new(),
            frames: Vec::new(),
            dependencies: Vec::new(),
            names: BTreeMap::new(),
        };
        let mut pending = Vec::new();
        let mut tokens_used = 0;
        for asset in assets {
            for (field, value) in [
                ("label", asset.label),
                ("edition", asset.edition),
                ("version", asset.version),
                ("source", asset.source),
            ] {
                if value.is_empty() {
                    return Err(error(asset.label, 0, FdfErrorKind::MissingIdentity(field)));
                }
            }
            if catalog
                .sources
                .iter()
                .any(|source| source.label == asset.label)
            {
                return Err(error(
                    asset.label,
                    0,
                    FdfErrorKind::Duplicate(asset.label.into()),
                ));
            }
            let source = catalog.sources.len();
            catalog.sources.push(SourceIdentity {
                label: asset.label.into(),
                edition: asset.edition.into(),
                version: asset.version.into(),
                source: asset.source.into(),
                fnv1a64: digest(asset.text.as_bytes()),
            });
            let remaining_tokens = limits
                .tokens
                .checked_sub(tokens_used)
                .ok_or_else(|| error(asset.label, 0, FdfErrorKind::Budget("tokens")))?;
            let tokens = lex(*asset, remaining_tokens)?;
            tokens_used = tokens_used
                .checked_add(tokens.len())
                .ok_or_else(|| error(asset.label, 0, FdfErrorKind::Budget("tokens")))?;
            let mut reader = Reader {
                source: asset.label,
                tokens: &tokens,
                at: 0,
            };
            let mut stack = Vec::<FrameId>::new();
            while reader.at < tokens.len() {
                reader.skip_lines();
                if reader.at == tokens.len() {
                    break;
                }
                let token = reader.next()?.clone();
                match token.kind {
                    TokenKind::Close => {
                        if stack.pop().is_none() {
                            return Err(error(
                                asset.label,
                                token.start,
                                FdfErrorKind::Syntax("unexpected closing brace"),
                            ));
                        }
                    }
                    TokenKind::Word(key) if key == "Frame" => {
                        if catalog.frames.len() >= limits.frames {
                            return Err(error(
                                asset.label,
                                token.start,
                                FdfErrorKind::Budget("frames"),
                            ));
                        }
                        if stack.len() >= limits.depth {
                            return Err(error(
                                asset.label,
                                token.start,
                                FdfErrorKind::Budget("frame depth"),
                            ));
                        }
                        let kind = reader.string()?;
                        let name = reader.string()?;
                        if kind.is_empty() || name.is_empty() {
                            return Err(error(
                                asset.label,
                                token.start,
                                FdfErrorKind::Syntax("empty frame kind/name"),
                            ));
                        }
                        reader.skip_lines();
                        let mut inherited = None;
                        let mut with_children = false;
                        if reader
                            .tokens
                            .get(reader.at)
                            .is_some_and(|t| t.kind == TokenKind::Word("INHERITS".into()))
                        {
                            reader.at += 1;
                            reader.skip_lines();
                            if reader
                                .tokens
                                .get(reader.at)
                                .is_some_and(|t| t.kind == TokenKind::Word("WITHCHILDREN".into()))
                            {
                                reader.at += 1;
                                with_children = true;
                            }
                            inherited = Some(reader.string()?);
                        }
                        reader.skip_lines();
                        let open = reader.next()?;
                        if open.kind != TokenKind::Open {
                            return Err(error(
                                asset.label,
                                open.start,
                                FdfErrorKind::Syntax("expected frame opening brace"),
                            ));
                        }
                        let id = FrameId(catalog.frames.len());
                        if catalog.names.insert(name.clone(), id).is_some() {
                            return Err(error(
                                asset.label,
                                token.start,
                                FdfErrorKind::Duplicate(name),
                            ));
                        }
                        let parent = stack.last().copied();
                        if let Some(parent) = parent {
                            catalog.frames[parent.0].children.push(id);
                        }
                        catalog.frames.push(Frame {
                            source,
                            offset: token.start,
                            kind,
                            name,
                            lexical_parent: parent,
                            children: Vec::new(),
                            inherits: None,
                            records: Vec::new(),
                            anchors: Vec::new(),
                            set_all_points: false,
                        });
                        pending.push((id, inherited, with_children));
                        stack.push(id);
                    }
                    TokenKind::Word(key) => {
                        let record = reader.record(*asset, key, token.start)?;
                        if let Some(frame) = stack.last().copied() {
                            let declaration = &mut catalog.frames[frame.0];
                            if record.key != "SetPoint" && declaration.record(&record.key).is_some()
                            {
                                return Err(error(
                                    asset.label,
                                    record.offset,
                                    FdfErrorKind::Duplicate(record.key),
                                ));
                            }
                            if let Some(mut anchor) = validate_record(asset.label, &record)? {
                                if declaration.anchors.iter().any(|prior| {
                                    prior.point == anchor.point
                                        && prior.relative_name == anchor.relative_name
                                        && prior.relative_point == anchor.relative_point
                                        && prior.x == anchor.x
                                        && prior.y == anchor.y
                                }) {
                                    return Err(error(
                                        asset.label,
                                        record.offset,
                                        FdfErrorKind::Duplicate("SetPoint".into()),
                                    ));
                                }
                                anchor.record = declaration.records.len();
                                catalog.dependencies.push(Dependency {
                                    source,
                                    frame: Some(frame),
                                    offset: record.offset,
                                    kind: DependencyKind::Anchor,
                                    name: anchor.relative_name.clone(),
                                });
                                declaration.anchors.push(anchor);
                            }
                            if record.key == "SetAllPoints" {
                                declaration.set_all_points = true;
                            }
                            let dependency_kind = match record.key.as_str() {
                                "FrameFont" => Some(DependencyKind::Font),
                                "BackdropBackground" | "BackdropEdgeFile"
                                | "HighlightAlphaFile" => Some(DependencyKind::Art),
                                "ButtonText"
                                | "DialogBackdrop"
                                | "ControlBackdrop"
                                | "ControlPushedBackdrop"
                                | "ControlDisabledBackdrop"
                                | "ControlDisabledPushedBackdrop"
                                | "ControlMouseOverHighlight"
                                | "PopupArrowFrame"
                                | "PopupMenuFrame"
                                | "PopupTitleFrame"
                                | "TextAreaScrollBar"
                                | "SliderThumbButtonFrame"
                                | "ScrollBarDecButtonFrame"
                                | "ScrollBarIncButtonFrame"
                                | "CheckBoxCheckHighlight"
                                | "CheckBoxDisabledCheckHighlight" => {
                                    Some(DependencyKind::FrameReference)
                                }
                                _ => None,
                            };
                            if let Some(kind) = dependency_kind {
                                let name =
                                    record.values.first().and_then(quoted).ok_or_else(|| {
                                        error(
                                            asset.label,
                                            record.offset,
                                            FdfErrorKind::InvalidProperty(record.key.clone()),
                                        )
                                    })?;
                                catalog.dependencies.push(Dependency {
                                    source,
                                    frame: Some(frame),
                                    offset: record.offset,
                                    kind,
                                    name: name.into(),
                                });
                            }
                            declaration.records.push(record);
                        } else if record.key == "IncludeFile" && record.values.len() == 1 {
                            let name = record.values.first().and_then(quoted).ok_or_else(|| {
                                error(
                                    asset.label,
                                    record.offset,
                                    FdfErrorKind::InvalidProperty(record.key.clone()),
                                )
                            })?;
                            catalog.dependencies.push(Dependency {
                                source,
                                frame: None,
                                offset: record.offset,
                                kind: DependencyKind::Include,
                                name: name.into(),
                            });
                        } else {
                            return Err(error(
                                asset.label,
                                record.offset,
                                FdfErrorKind::Unsupported(record.key),
                            ));
                        }
                    }
                    _ => {
                        return Err(error(
                            asset.label,
                            token.start,
                            FdfErrorKind::Syntax("expected declaration"),
                        ));
                    }
                }
            }
            if !stack.is_empty() {
                return Err(error(
                    asset.label,
                    asset.text.len(),
                    FdfErrorKind::Unterminated("frame"),
                ));
            }
        }
        for (id, template, with_children) in pending {
            if let Some(name) = template {
                let frame = &catalog.frames[id.0];
                let target = catalog.names.get(&name).copied().ok_or_else(|| {
                    error(
                        &catalog.sources[frame.source].label,
                        frame.offset,
                        FdfErrorKind::UnknownTemplate(name.clone()),
                    )
                })?;
                catalog.dependencies.push(Dependency {
                    source: frame.source,
                    frame: Some(id),
                    offset: frame.offset,
                    kind: DependencyKind::Template,
                    name,
                });
                catalog.frames[id.0].inherits = Some(Inheritance {
                    template: target,
                    with_children,
                });
            }
        }
        let mut marks = vec![0u8; catalog.frames.len()];
        for start in 0..catalog.frames.len() {
            let mut chain = Vec::new();
            let mut current = Some(FrameId(start));
            while let Some(id) = current {
                if marks[id.0] == 2 {
                    break;
                }
                let frame = &catalog.frames[id.0];
                if marks[id.0] == 1 {
                    return Err(error(
                        &catalog.sources[frame.source].label,
                        frame.offset,
                        FdfErrorKind::InheritanceCycle(frame.name.clone()),
                    ));
                }
                marks[id.0] = 1;
                chain.push(id);
                current = frame.inherits.as_ref().map(|edge| edge.template);
            }
            for id in chain {
                marks[id.0] = 2;
            }
        }
        Ok(catalog)
    }

    pub fn sources(&self) -> &[SourceIdentity] {
        &self.sources
    }
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }
    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    pub fn frame_id(&self, name: &str) -> Option<FrameId> {
        self.names.get(name).copied()
    }
    /// Bounds-checked access by catalog index; invalid FrameId values return None.
    pub fn frame(&self, id: FrameId) -> Option<&Frame> {
        self.frames.get(id.0)
    }

    /// Bind every local authored anchor. Inherited records/children remain graph edges.
    /// The caller supplies parent/active context; this never creates a screen rectangle.
    pub fn relationships(
        &self,
        id: FrameId,
        bindings: &[FrameBinding],
        parent: Option<Target>,
    ) -> Result<Relationships, FdfError> {
        let frame = self
            .frames
            .get(id.0)
            .ok_or_else(|| error("FDF catalog", 0, FdfErrorKind::UnknownFrame(id)))?;
        let source = &self.sources[frame.source].label;
        let mut targets = BTreeMap::new();
        for binding in bindings {
            self.check_target(source, frame.offset, &binding.target)?;
            if targets
                .insert(&binding.authored_name, &binding.target)
                .is_some()
            {
                return Err(error(
                    source,
                    frame.offset,
                    FdfErrorKind::Duplicate(binding.authored_name.clone()),
                ));
            }
        }
        if let Some(target) = &parent {
            self.check_target(source, frame.offset, target)?;
        }
        let mut anchors = Vec::new();
        for anchor in &frame.anchors {
            let target = targets.get(&anchor.relative_name).ok_or_else(|| {
                error(
                    source,
                    frame.records[anchor.record].offset,
                    FdfErrorKind::MissingBinding(anchor.relative_name.clone()),
                )
            })?;
            anchors.push(BoundAnchor {
                authored: anchor.clone(),
                target: (*target).clone(),
            });
        }
        let all_points =
            if frame.set_all_points {
                Some(parent.ok_or_else(|| {
                    error(source, frame.offset, FdfErrorKind::MissingParentContext)
                })?)
            } else {
                None
            };
        Ok(Relationships {
            frame: id,
            anchors,
            all_points,
            inherits: frame.inherits.clone(),
        })
    }

    fn check_target(&self, source: &str, offset: usize, target: &Target) -> Result<(), FdfError> {
        match target {
            Target::Frame(id) if id.0 >= self.frames.len() => {
                Err(error(source, offset, FdfErrorKind::UnknownFrame(*id)))
            }
            Target::Context(name) if name.is_empty() => Err(error(
                source,
                offset,
                FdfErrorKind::MissingIdentity("context"),
            )),
            _ => Ok(()),
        }
    }
}
