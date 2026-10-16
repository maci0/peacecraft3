//! A narrow parser for Warcraft III FDF frame definitions.
//!
//! The launcher needs a few numbers that belong to the original's own UI
//! definitions — a button's plate size, its inner button's bounds, the backdrop's
//! insets, and the anchor chain between them. Those are asset records, so they are
//! **read from the FDF text** rather than copied into constants.
//!
//! This parses only what those derivations need, and refuses rather than
//! defaulting: an unknown frame, a cycle in the anchor chain, a missing property
//! or a malformed number all become an [`FdfError`].
//!
//! Scope: `Frame "TYPE" "NAME" [INHERITS [WITHCHILDREN] "TEMPLATE"] { … }` blocks,
//! nested `Texture { … }` blocks, and the handful of properties listed in
//! [`Props`]. Comments (`//`) are stripped. A `Texture` block keeps its own
//! records on the enclosing frame; those records are not frame layout. Any
//! other `{` block is refused. It is not closed as a frame and its lines are
//! not written onto a neighboring frame.
//!
//! **One line, one record.** A frame's `{` and `}` are expected on their own
//! lines with each property on its own line, which is how both shipped files are
//! written (265 frames parse from them). A property sharing the header's line, or
//! a `}` sharing a property's line, is not read — so the parser would silently
//! drop it. `{}` on one line therefore leaves the block open and is reported as
//! [`FdfError::Unterminated`] rather than accepted.

use std::collections::HashMap;

/// A parse or resolution failure. Every variant names what was wrong.
#[derive(Debug, Clone, PartialEq)]
pub enum FdfError {
    /// A `Frame` header could not be read.
    Header(String),
    /// A `{` was never closed.
    Unterminated(String),
    /// A frame name that is not defined in the parsed text.
    UnknownFrame(String),
    /// An anchor chain that loops.
    Cycle(String),
    /// A property the caller needs is absent.
    Missing {
        /// The frame that lacks it.
        frame: String,
        /// The property, e.g. `Width`.
        property: &'static str,
    },
    /// A property is present but not a number.
    BadNumber {
        /// The frame carrying it.
        frame: String,
        /// The property, e.g. `Width`.
        property: &'static str,
        /// The text that would not parse.
        text: String,
    },
    /// An authored value has no supported rendering interpretation.
    Unsupported {
        frame: String,
        property: &'static str,
        value: String,
    },
}

impl std::fmt::Display for FdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported {
                frame,
                property,
                value,
            } => write!(f, "{frame:?}'s {property} value {value:?} is unsupported"),
            Self::Header(s) => write!(f, "unreadable Frame header: {s}"),
            Self::Unterminated(s) => write!(f, "frame {s:?} is never closed"),
            Self::UnknownFrame(s) => write!(f, "no frame named {s:?}"),
            Self::Cycle(s) => write!(f, "the anchor chain through {s:?} loops"),
            Self::Missing { frame, property } => {
                write!(f, "{frame:?} has no {property}")
            }
            Self::BadNumber {
                frame,
                property,
                text,
            } => write!(f, "{frame:?}'s {property} is not a number: {text:?}"),
        }
    }
}

impl std::error::Error for FdfError {}

/// One of the nine anchor points an FDF `SetPoint` can name.
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
    fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_uppercase().as_str() {
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

/// One `SetPoint <mine>, "<frame>", <theirs>, <x>, <y>` record.
///
/// The engine's Y is **upward**, so a `TOPRIGHT … -0.110625` anchor places the
/// frame `0.110625` *below* the top. [`Fdf::rect`] keeps that convention and
/// callers flip for a Y-down UI.
#[derive(Debug, Clone, PartialEq)]
pub struct SetPoint {
    /// This frame's own point.
    pub point: Point,
    /// The frame it is anchored to.
    pub relative: String,
    /// The point on that frame.
    pub relative_point: Point,
    /// Horizontal offset, frame units.
    pub x: f32,
    /// Vertical offset, frame units (positive is up).
    pub y: f32,
}

/// The properties this parser keeps. Anything else is ignored.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Props {
    /// `Width`, frame units.
    pub width: Option<f32>,
    /// `Height`, frame units.
    pub height: Option<f32>,
    /// `SetPoint`, in file order.
    pub set_points: Vec<SetPoint>,
    /// `ButtonText`, naming the frame that draws a button's label.
    pub button_text: Option<String>,
    /// `Text`, an authored label or the `GlobalStrings` key for one.
    pub text: Option<String>,
    /// Every record line as `(key, value)`, value trimmed of its trailing comma,
    /// in file order. The typed fields above are parsed views of some of these.
    pub records: Vec<(String, String)>,
}

/// Records inside one `Texture { … }` block, in file order.
///
/// The block has no authored name. Its enclosing frame is the `Frame` whose
/// [`Frame::textures`] list holds it, and sibling blocks are the other entries
/// of that list. Nothing here is copied onto the frame's layout properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureBlock {
    /// `(key, value)` as written, with the trailing comma removed from the value.
    pub records: Vec<(String, String)>,
}

/// One parsed `Frame` block.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// `FRAME`, `BACKDROP`, `TEXTBUTTON`, `GLUEBUTTON`, `SPRITE`, `TEXT`, …
    pub kind: String,
    /// The frame's name.
    pub name: String,
    /// `INHERITS "TEMPLATE"`, if any.
    pub inherits: Option<String>,
    /// `INHERITS WITHCHILDREN "TEMPLATE"` — the template's children are adopted.
    pub with_children: bool,
    /// The enclosing frame, if any.
    pub parent: Option<String>,
    /// The frame's own properties.
    pub props: Props,
    /// Child frame names, in file order.
    pub children: Vec<String>,
    /// Nested `Texture` blocks, in file order.
    pub textures: Vec<TextureBlock>,
}

/// Every frame parsed from one or more FDF texts, indexed by name.
#[derive(Debug, Clone, Default)]
pub struct Fdf {
    frames: Vec<Frame>,
    index: HashMap<String, usize>,
    /// Text extents measured by the renderer; explicit authored sizes win.
    intrinsic_sizes: HashMap<String, (f32, f32)>,
}

/// A resolved rectangle in the engine's Y-**up** frame coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub left: f32,
    /// Bottom edge.
    pub bottom: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// The named point on the rectangle, in Y-up coordinates.
    #[must_use]
    pub fn point(&self, p: Point) -> (f32, f32) {
        let (l, b, w, h) = (self.left, self.bottom, self.width, self.height);
        match p {
            Point::TopLeft => (l, b + h),
            Point::Top => (l + w / 2.0, b + h),
            Point::TopRight => (l + w, b + h),
            Point::Left => (l, b + h / 2.0),
            Point::Center => (l + w / 2.0, b + h / 2.0),
            Point::Right => (l + w, b + h / 2.0),
            Point::BottomLeft => (l, b),
            Point::Bottom => (l + w / 2.0, b),
            Point::BottomRight => (l + w, b),
        }
    }

    /// The rectangle's top edge, in Y-up coordinates.
    #[must_use]
    pub fn top(&self) -> f32 {
        self.bottom + self.height
    }
}

impl Fdf {
    /// Parses one FDF text into a fresh set, or adds to an existing one.
    ///
    /// Later definitions of a name replace earlier ones, which is what an
    /// expansion archive overriding a base file does.
    ///
    /// # Errors
    ///
    /// Returns [`FdfError`] on a malformed header or an unclosed block.
    pub fn parse_into(&mut self, text: &str) -> Result<(), FdfError> {
        let mut stack: Vec<Open> = Vec::new();
        for raw in text.lines() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('}') {
                if stack.pop().is_none() {
                    return Err(FdfError::Unterminated("<extra closing brace>".to_string()));
                }
                continue;
            }
            // A header is `Frame` followed by whitespace. `FrameFont` (and any
            // other `Frame…` property) also starts with `Frame`, and treating it
            // as a header would invent a frame.
            if let Some(rest) = line
                .strip_prefix("Frame")
                .filter(|r| r.is_empty() || r.starts_with(' ') || r.starts_with('\t'))
            {
                if let Some(Open::Texture { frame, index }) = stack.last().copied() {
                    return Err(FdfError::Unsupported {
                        frame: self.frames[frame].name.clone(),
                        property: "block",
                        value: format!("Frame inside Texture#{index}"),
                    });
                }
                let (frame, opens) = parse_header(rest)?;
                let parent = innermost_frame(&stack).map(|i| self.frames[i].name.clone());
                let idx = self.frames.len();
                self.index.insert(frame.name.clone(), idx);
                if let Some(p) = &parent
                    && let Some(pi) = self.index.get(p).copied()
                {
                    self.frames[pi].children.push(frame.name.clone());
                }
                self.frames.push(Frame { parent, ..frame });
                if opens {
                    stack.push(Open::Frame(idx));
                }
                continue;
            }
            if let Some(keyword) = block_keyword(line) {
                self.open_block(&mut stack, keyword)?;
                continue;
            }
            match stack.last().copied() {
                Some(Open::Texture { frame, index }) => {
                    self.frames[frame].textures[index]
                        .records
                        .push(split_record(line));
                }
                Some(Open::Frame(cur)) => apply_property(&mut self.frames[cur], line)?,
                None => {}
            }
        }
        if let Some(open) = stack.last() {
            let name = match *open {
                Open::Frame(index) => self.frames[index].name.clone(),
                Open::Texture { frame, index } => {
                    format!("{} Texture#{index}", self.frames[frame].name)
                }
            };
            return Err(FdfError::Unterminated(name));
        }
        Ok(())
    }

    fn open_block(&mut self, stack: &mut Vec<Open>, keyword: &str) -> Result<(), FdfError> {
        if keyword != "Texture" {
            let frame = innermost_frame(stack)
                .map(|index| self.frames[index].name.clone())
                .unwrap_or_else(|| "<root>".to_string());
            return Err(FdfError::Unsupported {
                frame,
                property: "block",
                value: keyword.to_string(),
            });
        }
        if let Some(Open::Texture { frame, index }) = stack.last().copied() {
            return Err(FdfError::Unsupported {
                frame: self.frames[frame].name.clone(),
                property: "block",
                value: format!("Texture inside Texture#{index}"),
            });
        }
        let Some(frame) = innermost_frame(stack) else {
            return Err(FdfError::Unsupported {
                frame: "<root>".to_string(),
                property: "block",
                value: "Texture".to_string(),
            });
        };
        let index = self.frames[frame].textures.len();
        self.frames[frame].textures.push(TextureBlock {
            records: Vec::new(),
        });
        stack.push(Open::Texture { frame, index });
        Ok(())
    }

    /// Parses a single FDF text.
    ///
    /// # Errors
    ///
    /// As [`Fdf::parse_into`].
    pub fn parse(text: &str) -> Result<Self, FdfError> {
        let mut f = Self::default();
        f.parse_into(text)?;
        Ok(f)
    }

    /// The frame with this name, if the parsed text defines it.
    #[must_use]
    pub fn frame(&self, name: &str) -> Option<&Frame> {
        self.index.get(name).map(|i| &self.frames[*i])
    }

    /// Every frame, in definition order.
    ///
    /// Used by screen layout and by browser-local asset hydration.
    #[must_use]
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// Looks a property up along the `INHERITS` chain, nearest definition first.
    ///
    /// The frame's own value wins; otherwise its template's, recursively. A
    /// `WITHCHILDREN` inherit is the same lookup for the properties here.
    fn inherited<'a, T>(
        &'a self,
        name: &str,
        get: impl Fn(&'a Props) -> Option<T> + Copy,
    ) -> Option<T> {
        let mut cur = name.to_string();
        let mut seen: Vec<String> = Vec::new();
        loop {
            if seen.contains(&cur) {
                return None; // inheritance loop; treated as absent, not as a value
            }
            seen.push(cur.clone());
            let frame = self.frame(&cur)?;
            if let Some(v) = get(&frame.props) {
                return Some(v);
            }
            cur = frame.inherits.clone()?;
        }
    }

    /// A frame's `Width`, along the inheritance chain.
    ///
    /// # Errors
    ///
    /// [`FdfError::Missing`] when neither the frame nor its templates set it.
    pub fn width(&self, name: &str) -> Result<f32, FdfError> {
        self.inherited(name, |p| p.width)
            .or_else(|| self.intrinsic_sizes.get(name).map(|s| s.0))
            .ok_or_else(|| FdfError::Missing {
                frame: name.to_string(),
                property: "Width",
            })
    }

    /// A frame's `Height`, along the inheritance chain.
    ///
    /// # Errors
    ///
    /// As [`Fdf::width`].
    pub fn height(&self, name: &str) -> Result<f32, FdfError> {
        self.inherited(name, |p| p.height)
            .or_else(|| self.intrinsic_sizes.get(name).map(|s| s.1))
            .ok_or_else(|| FdfError::Missing {
                frame: name.to_string(),
                property: "Height",
            })
    }

    /// A frame's `ButtonText`, naming the frame that draws its label.
    #[must_use]
    pub fn button_text(&self, name: &str) -> Option<String> {
        self.inherited(name, |p| p.button_text.clone())
    }

    /// A frame's `Text`, which is an authored label or its `GlobalStrings` key.
    #[must_use]
    pub fn text_value(&self, name: &str) -> Option<String> {
        self.inherited(name, |p| p.text.clone())
    }

    /// Supplies shaped text extents in frame units, without replacing authored
    /// dimensions. Layout may be repeated after the font finishes loading.
    pub fn measure_text(&mut self, name: &str, size: (f32, f32)) {
        if self.frame(name).is_some_and(|f| f.kind == "TEXT")
            && size.0.is_finite()
            && size.1.is_finite()
            && size.0 >= 0.0
            && size.1 >= 0.0
        {
            self.intrinsic_sizes.insert(name.to_string(), size);
        }
    }

    /// The frame's first `SetPoint`, if it has one.
    #[must_use]
    pub fn set_point(&self, name: &str) -> Option<SetPoint> {
        self.inherited(name, |p| p.set_points.first().cloned())
    }

    /// A record's raw value along the `INHERITS` chain, nearest definition first.
    #[must_use]
    pub fn record(&self, name: &str, key: &str) -> Option<String> {
        self.inherited(name, |p| {
            p.records
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
        })
    }

    /// A record's value with its quotes removed.
    #[must_use]
    pub fn string(&self, name: &str, key: &str) -> Option<String> {
        self.record(name, key).map(|v| unquote(&v))
    }

    /// Whether a flag record (`SetAllPoints`, `BackdropTileBackground`, …) is
    /// present along the `INHERITS` chain.
    #[must_use]
    pub fn flag(&self, name: &str, key: &str) -> bool {
        self.record(name, key).is_some()
    }

    /// A record's whitespace-separated numbers, or `None` when it is absent.
    ///
    /// # Errors
    ///
    /// [`FdfError::BadNumber`] when a field is not a number.
    pub fn numbers(&self, name: &str, key: &'static str) -> Result<Option<Vec<f32>>, FdfError> {
        let Some(value) = self.record(name, key) else {
            return Ok(None);
        };
        value
            .split_whitespace()
            .map(|t| {
                num(t).ok_or_else(|| FdfError::BadNumber {
                    frame: name.to_string(),
                    property: key,
                    text: value.clone(),
                })
            })
            .collect::<Result<Vec<f32>, _>>()
            .map(Some)
    }

    /// Resolves a frame's rectangle in the engine's Y-up frame coordinates.
    ///
    /// A frame with no `SetPoint` is a **root**: it is placed at the origin and
    /// sized by its own `Width`/`Height`, or, when it declares neither, by the
    /// authored viewport the caller passes — the FDF does not size `MainMenuFrame`
    /// at all, the engine gives it the screen. Everything else is placed from its
    /// anchor, so the chain must terminate: a cycle or a missing ancestor is an
    /// error, never a default.
    ///
    /// # Errors
    ///
    /// [`FdfError::UnknownFrame`], [`FdfError::Missing`] or [`FdfError::Cycle`].
    pub fn rect(&self, name: &str, viewport: (f32, f32)) -> Result<Rect, FdfError> {
        self.rect_inner(name, viewport, &mut Vec::new())
    }

    fn rect_inner(
        &self,
        name: &str,
        viewport: (f32, f32),
        stack: &mut Vec<String>,
    ) -> Result<Rect, FdfError> {
        if stack.iter().any(|s| s == name) {
            return Err(FdfError::Cycle(name.to_string()));
        }
        let Some(frame) = self.frame(name) else {
            return Err(FdfError::UnknownFrame(name.to_string()));
        };
        // `SetAllPoints` takes the enclosing frame's rectangle; a root that sets
        // it takes the viewport the engine gives it.
        if self.flag(name, "SetAllPoints") {
            let Some(parent) = frame.parent.clone() else {
                return Ok(Rect {
                    left: 0.0,
                    bottom: 0.0,
                    width: viewport.0,
                    height: viewport.1,
                });
            };
            stack.push(name.to_string());
            let rect = self.rect_inner(&parent, viewport, stack);
            stack.pop();
            return rect;
        }
        let Some(sp) = self.set_point(name) else {
            // No anchor: a root. Sized by its own properties, else the viewport.
            return Ok(Rect {
                left: 0.0,
                bottom: 0.0,
                width: self.width(name).unwrap_or(viewport.0),
                height: self.height(name).unwrap_or(viewport.1),
            });
        };
        let width = self.width(name)?;
        let height = self.height(name)?;
        stack.push(name.to_string());
        let rel = self.rect_inner(&sp.relative, viewport, stack)?;
        stack.pop();
        let (px, py) = rel.point(sp.relative_point);
        let (ax, ay) = (px + sp.x, py + sp.y);
        // The anchor fixes one of our points; derive the corner from it.
        let (left, bottom) = match sp.point {
            Point::TopLeft => (ax, ay - height),
            Point::Top => (ax - width / 2.0, ay - height),
            Point::TopRight => (ax - width, ay - height),
            Point::Left => (ax, ay - height / 2.0),
            Point::Center => (ax - width / 2.0, ay - height / 2.0),
            Point::Right => (ax - width, ay - height / 2.0),
            Point::BottomLeft => (ax, ay),
            Point::Bottom => (ax - width / 2.0, ay),
            Point::BottomRight => (ax - width, ay),
        };
        Ok(Rect {
            left,
            bottom,
            width,
            height,
        })
    }
}

/// Strips a `//` comment. FDF has no string literals containing `//` in the
/// records this parser keeps.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Reads `"TYPE" "NAME" [INHERITS [WITHCHILDREN] "TEMPLATE"] [{]`.
fn parse_header(rest: &str) -> Result<(Frame, bool), FdfError> {
    let mut quoted = Vec::new();
    let mut i = 0;
    let bytes = rest.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            let Some(end) = rest[start..].find('"').map(|e| start + e) else {
                return Err(FdfError::Header(rest.to_string()));
            };
            quoted.push(rest[start..end].to_string());
            i = end + 1;
        } else {
            i += 1;
        }
    }
    let (Some(kind), Some(name)) = (quoted.first().cloned(), quoted.get(1).cloned()) else {
        return Err(FdfError::Header(rest.to_string()));
    };
    let inherits = quoted.get(2).cloned();
    Ok((
        Frame {
            kind,
            name,
            inherits,
            with_children: rest.contains("WITHCHILDREN"),
            parent: None,
            props: Props::default(),
            children: Vec::new(),
            textures: Vec::new(),
        },
        rest.contains('{'),
    ))
}

/// What `{` … `}` is currently open. A texture sits inside one frame and
/// does not replace that frame on the stack.
#[derive(Clone, Copy)]
enum Open {
    Frame(usize),
    Texture { frame: usize, index: usize },
}

fn innermost_frame(stack: &[Open]) -> Option<usize> {
    stack.last().map(|open| match *open {
        Open::Frame(index) => index,
        Open::Texture { frame, .. } => frame,
    })
}

/// `Keyword {` opens a nested block. A bare `{`, or a line that also carries
/// a value, is not a block keyword.
fn block_keyword(line: &str) -> Option<&str> {
    let line = line.trim().trim_end_matches(',').trim();
    let keyword = line.strip_suffix('{')?.trim();
    if keyword.is_empty() {
        return None;
    }
    keyword
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
        .then_some(keyword)
}

/// Splits one `Key value` record. A flag keeps an empty value: `SetAllPoints,`.
fn split_record(line: &str) -> (String, String) {
    let (key, value) = match line.split_once(char::is_whitespace) {
        Some((key, value)) => (key, value.trim()),
        None => (line.trim_end_matches(','), ""),
    };
    (
        key.to_string(),
        value.trim_end_matches(',').trim().to_string(),
    )
}

/// Applies one `Key value` line to a frame.
fn apply_property(frame: &mut Frame, line: &str) -> Result<(), FdfError> {
    let (key, value) = split_record(line);
    let bad = |property: &'static str| FdfError::BadNumber {
        frame: frame.name.clone(),
        property,
        text: value.clone(),
    };
    frame.props.records.push((key.clone(), value.clone()));
    match key.as_str() {
        "Width" => frame.props.width = Some(num(&value).ok_or_else(|| bad("Width"))?),
        "Height" => frame.props.height = Some(num(&value).ok_or_else(|| bad("Height"))?),
        "ButtonText" => frame.props.button_text = Some(unquote(&value)),
        "Text" => frame.props.text = Some(unquote(&value)),
        "SetPoint" => frame.props.set_points.push(parse_set_point(&value)?),
        _ => {}
    }
    Ok(())
}

/// Reads `POINT, "frame", POINT, x, y,`.
fn parse_set_point(value: &str) -> Result<SetPoint, FdfError> {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    if parts.len() < 5 {
        return Err(FdfError::Header(format!("SetPoint {value}")));
    }
    let point = Point::parse(parts[0]).ok_or_else(|| FdfError::Header(parts[0].to_string()))?;
    let relative = unquote(parts[1]);
    let relative_point =
        Point::parse(parts[2]).ok_or_else(|| FdfError::Header(parts[2].to_string()))?;
    let bad = |t: &str| FdfError::BadNumber {
        frame: format!("SetPoint {relative}"),
        property: "SetPoint offset",
        text: t.to_string(),
    };
    let x = num(parts[3]).ok_or_else(|| bad(parts[3]))?;
    let y = num(parts[4]).ok_or_else(|| bad(parts[4]))?;
    Ok(SetPoint {
        point,
        relative,
        relative_point,
        x,
        y,
    })
}

/// A trimmed, unquoted string value.
fn unquote(v: &str) -> String {
    v.trim()
        .trim_end_matches(',')
        .trim()
        .trim_matches('"')
        .to_string()
}

/// A frame-unit number, with the FDF's optional `f` suffix.
fn num(t: &str) -> Option<f32> {
    t.trim()
        .trim_end_matches(',')
        .trim()
        .trim_end_matches('f')
        .parse::<f32>()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATES: &str = r#"
Frame "BACKDROP" "StandardMenuButtonBaseBackdrop" {
    Width 0.256,
    Height 0.064,
    BackdropBackground  "UI\\Widgets\\Glues\\GlueScreen-Button1-Border.blp",
}
Frame "BACKDROP" "StandardButtonBackdropTemplate" {
    BackdropBackground  "UI\\Widgets\\Glues\\GlueScreen-Button1-BackdropBackground.blp",
    BackdropCornerSize  0.016,
    BackdropBackgroundInsets 0.004 0.004 0.004 0.004,
    BackdropEdgeFile  "UI\\Widgets\\Glues\\GlueScreen-Button1-BackdropBorder.blp",
}
Frame "GLUEBUTTON" "StandardIconicButtonTemplate" {
    Height 0.031,
    Width 0.031,
}
Frame "TEXTBUTTON" "StandardButtonTemplate" {
    Height 0.031,
    Width 0.179,
    ControlBackdrop "ButtonBackdropTemplate",
    Frame "BACKDROP" "ButtonBackdropTemplate" INHERITS "StandardButtonBackdropTemplate" {
    }
}
"#;

    const MENU: &str = r#"
Frame "FRAME" "MainMenuFrame" {
    Frame "BACKDROP" "SinglePlayerBackdrop" INHERITS "StandardMenuButtonBaseBackdrop" {
        SetPoint TOPRIGHT,"MainMenuFrame",TOPRIGHT, -0.015, -0.110625,
        Frame "GLUEBUTTON" "EditionButton" INHERITS "StandardIconicButtonTemplate" {
            SetPoint TOPRIGHT, "SinglePlayerBackdrop", TOPRIGHT, -0.012, -0.0165,
        }
        Frame "GLUETEXTBUTTON" "SinglePlayerButton" INHERITS WITHCHILDREN "StandardButtonTemplate" {
            Width 0.149,
            SetPoint TOPRIGHT, "EditionButton", TOPLEFT, 0.001, 0.0,
            ButtonText "SinglePlayerButtonText",
            Frame "TEXT" "SinglePlayerButtonText" INHERITS "StandardButtonTextTemplate" {
                Text "KEY_SINGLE_PLAYER",
            }
        }
    }
}
"#;

    fn fdf() -> Fdf {
        let mut f = Fdf::default();
        f.parse_into(TEMPLATES).unwrap();
        f.parse_into(MENU).unwrap();
        f
    }

    /// A nested frame's parent is recorded and the child list built.
    #[test]
    fn nesting_records_parent_and_children() {
        let f = fdf();
        let b = f.frame("SinglePlayerButton").unwrap();
        assert_eq!(b.parent.as_deref(), Some("SinglePlayerBackdrop"));
        assert_eq!(b.kind, "GLUETEXTBUTTON");
        assert!(b.with_children);
        let p = f.frame("SinglePlayerBackdrop").unwrap();
        assert_eq!(
            p.children,
            vec![
                "EditionButton".to_string(),
                "SinglePlayerButton".to_string()
            ]
        );
        assert_eq!(
            f.frame("SinglePlayerButton").unwrap().parent.as_deref(),
            Some("SinglePlayerBackdrop")
        );
    }

    /// Width is inherited from the template, and the frame's own value wins.
    #[test]
    fn width_follows_the_inheritance_chain() {
        let f = fdf();
        // Own override.
        assert_eq!(f.width("SinglePlayerButton").unwrap(), 0.149);
        // Inherited from StandardMenuButtonBaseBackdrop.
        assert_eq!(f.width("SinglePlayerBackdrop").unwrap(), 0.256);
        // The inner backdrop inherits a *backdrop* template, which sizes nothing:
        // a backdrop takes its button's box, so there is no width to inherit.
        assert!(matches!(
            f.width("ButtonBackdropTemplate"),
            Err(FdfError::Missing { .. })
        ));
        // Insets do come from that template.
        assert_eq!(
            f.numbers("ButtonBackdropTemplate", "BackdropBackgroundInsets"),
            Ok(Some(vec![0.004; 4]))
        );
    }

    /// The anchor chain resolves to the plate-relative box the FDF describes.
    #[test]
    fn the_anchor_chain_places_the_button() {
        let f = fdf();
        let plate = f.rect("SinglePlayerBackdrop", (0.8, 0.6)).unwrap();
        // Menu frame is 0.8 x 0.6, anchored TOPRIGHT at (-0.015, -0.110625).
        assert!(
            (plate.left - (0.8 - 0.015 - 0.256)).abs() < 1e-6,
            "{plate:?}"
        );
        assert!((plate.top() - (0.6 - 0.110625)).abs() < 1e-6, "{plate:?}");
        let inner = f.rect("SinglePlayerButton", (0.8, 0.6)).unwrap();
        // Inner left is 0.065 into the plate, top is 0.0165 below its top.
        assert!((inner.left - plate.left - 0.065).abs() < 1e-6, "{inner:?}");
        assert!(
            (plate.top() - inner.top() - 0.0165).abs() < 1e-6,
            "{inner:?}"
        );
    }

    /// A `Frame…` property is not a frame header.
    ///
    /// `StandardFrameTemplate` carries `FrameFont "MasterFont", 0.015, "",`, and
    /// a prefix match on `Frame` alone would read it as a frame named `""`.
    #[test]
    fn framefont_is_not_a_frame_header() {
        let text = "Frame \"FRAME\" \"A\" {\n    FrameFont \"MasterFont\", 0.015, \"\",\n}\n";
        let f = Fdf::parse(text).unwrap();
        assert!(f.frame("A").is_some());
        assert!(f.frame("").is_none());
        assert!(f.frames().iter().all(|fr| !fr.name.is_empty()));
        assert_eq!(f.frame("A").unwrap().children.len(), 0);
    }

    /// The label key is reached through the button's `ButtonText`, which names
    /// the text frame whose `Text` record holds it.
    #[test]
    fn the_label_key_is_reached_through_button_text() {
        let f = fdf();
        let text_frame = f.button_text("SinglePlayerButton").expect("ButtonText");
        assert_eq!(text_frame, "SinglePlayerButtonText");
        assert_eq!(
            f.text_value(&text_frame).as_deref(),
            Some("KEY_SINGLE_PLAYER")
        );
        // A button that declares neither is reported, not defaulted.
        assert!(f.button_text("SinglePlayerBackdrop").is_none());
    }

    /// A missing property is reported, not defaulted.
    #[test]
    fn a_missing_property_is_reported() {
        let f = fdf();
        assert_eq!(
            f.numbers("SinglePlayerButton", "BackdropBackgroundInsets"),
            Ok(None)
        );
        assert!(matches!(
            f.rect("NoSuchFrame", (0.8, 0.6)),
            Err(FdfError::UnknownFrame(_))
        ));
    }

    /// An anchor chain that loops is refused.
    #[test]
    fn a_cycle_is_refused() {
        let text = r#"
Frame "FRAME" "A" {
    Width 0.1,
    Height 0.1,
    SetPoint TOPLEFT, "B", TOPLEFT, 0.0, 0.0,
}
Frame "FRAME" "B" {
    Width 0.1,
    Height 0.1,
    SetPoint TOPLEFT, "A", TOPLEFT, 0.0, 0.0,
}
"#;
        let f = Fdf::parse(text).unwrap();
        assert!(matches!(f.rect("A", (0.8, 0.6)), Err(FdfError::Cycle(_))));
    }

    /// Shaped text changes anchor chains without overriding authored dimensions.
    #[test]
    fn measured_text_moves_dependents_and_never_overrides_authored_dimensions() {
        let mut f = Fdf::parse(
            r#"
Frame "FRAME" "Root" {
    SetAllPoints,
}
Frame "TEXT" "Title" {
    SetPoint TOPLEFT, "Root", TOPLEFT, 0.0, 0.0,
}
Frame "FRAME" "Follower" {
    Width 0.13,
    Height 0.07,
    SetPoint TOPLEFT, "Title", BOTTOMLEFT, 0.0, -0.01,
}
Frame "TEXT" "Fixed" {
    Width 0.24,
    Height 0.04,
}
"#,
        )
        .unwrap();
        f.measure_text("Title", (0.31, 0.06));
        assert_eq!(f.width("Title").unwrap(), 0.31);
        assert!((f.rect("Follower", (0.8, 0.6)).unwrap().top() - 0.53).abs() < 1e-6);
        f.measure_text("Title", (0.31, 0.09));
        assert!((f.rect("Follower", (0.8, 0.6)).unwrap().top() - 0.50).abs() < 1e-6);
        f.measure_text("Title", (f32::NAN, 0.5));
        assert_eq!(f.height("Title").unwrap(), 0.09);
        f.measure_text("Fixed", (0.7, 0.3));
        assert_eq!(f.width("Fixed").unwrap(), 0.24);
        assert_eq!(f.height("Fixed").unwrap(), 0.04);
    }

    /// An unclosed block and a malformed number are both refused.
    #[test]
    fn malformed_input_is_refused() {
        assert!(matches!(
            Fdf::parse("Frame \"FRAME\" \"A\" {"),
            Err(FdfError::Unterminated(_))
        ));
        let bad = "Frame \"FRAME\" \"A\" {\n Width wide,\n}\n";
        assert!(matches!(Fdf::parse(bad), Err(FdfError::BadNumber { .. })));
        assert!(matches!(Fdf::parse("Frame\n"), Err(FdfError::Header(_))));
    }

    #[test]
    fn menu_style_frames_do_not_gain_texture_blocks() {
        let fdf = fdf();
        assert!(fdf.frames().iter().all(|frame| frame.textures.is_empty()));
        let nested = Fdf::parse(
            r#"
Frame "FRAME" "Root" {
Frame "FRAME" "A" {
}
Frame "FRAME" "A" {
}
Frame "FRAME" "A" {
Frame "FRAME" "B" {
}
}
}
"#,
        )
        .unwrap();
        assert_eq!(nested.frame("B").unwrap().parent.as_deref(), Some("A"));
        assert!(
            nested
                .frames()
                .iter()
                .all(|frame| frame.textures.is_empty())
        );
    }

    #[test]
    fn texture_blocks_stay_on_their_frame_and_do_not_bleed() {
        let fdf = Fdf::parse(
            r#"
Frame "FRAME" "Plate" {
    Width 0.2,
    Height 0.1,
    Texture {
        Width 0.9,
        Height 0.8,
        File "left.blp",
    }
    Texture {
        File "right.blp",
        Width 0.05,
    }
    SetPoint TOPLEFT, "Plate", TOPLEFT, 0.01, -0.02,
    Frame "BACKDROP" "Child" {
        Width 0.3,
        Height 0.3,
    }
}
"#,
        )
        .unwrap();
        let plate = fdf.frame("Plate").unwrap();
        assert_eq!(plate.props.width, Some(0.2));
        assert_eq!(plate.props.height, Some(0.1));
        assert!(plate.props.set_points.len() == 1);
        assert!(plate.props.records.iter().all(|(key, _)| key != "File"));
        assert_eq!(plate.children, vec!["Child".to_string()]);
        assert_eq!(plate.textures.len(), 2);
        assert_eq!(
            plate.textures[0].records,
            vec![
                ("Width".into(), "0.9".into()),
                ("Height".into(), "0.8".into()),
                ("File".into(), "\"left.blp\"".into()),
            ]
        );
        assert_eq!(
            plate.textures[1].records[0],
            ("File".into(), "\"right.blp\"".into())
        );
        let child = fdf.frame("Child").unwrap();
        assert_eq!(child.parent.as_deref(), Some("Plate"));
        assert!(child.textures.is_empty());
        assert_eq!(child.props.width, Some(0.3));
    }

    #[test]
    fn closing_a_texture_does_not_drop_the_following_frame_record() {
        let fdf = Fdf::parse(
            r#"
Frame "FRAME" "Plate" {
    Width 0.2,
    Height 0.1,
    Texture {
        File "only.blp",
    }
    SetPoint BOTTOMLEFT, "Plate", BOTTOMLEFT, 0.0, 0.0,
}
"#,
        )
        .unwrap();
        let plate = fdf.frame("Plate").unwrap();
        assert_eq!(plate.textures.len(), 1);
        assert_eq!(plate.textures[0].records.len(), 1);
        assert_eq!(plate.props.set_points.len(), 1);
        assert_eq!(plate.props.width, Some(0.2));
    }

    #[test]
    fn unknown_blocks_are_refused_without_becoming_the_parent_frame() {
        let err = Fdf::parse(
            r#"
Frame "FRAME" "Plate" {
    Width 0.2,
    Height 0.1,
    Layer {
        Width 9,
    }
}
"#,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            FdfError::Unsupported {
                frame,
                property: "block",
                value,
            } if frame == "Plate" && value == "Layer"
        ));
        assert!(matches!(
            Fdf::parse("Texture {\n File \"orphan.blp\",\n}\n"),
            Err(FdfError::Unsupported {
                frame: root,
                property: "block",
                value,
            }) if root == "<root>" && value == "Texture"
        ));
        assert!(matches!(
            Fdf::parse(
                "Frame \"FRAME\" \"Plate\" {\n Texture {\n Frame \"FRAME\" \"Inner\" {\n Width 1,\n Height 1,\n }\n }\n}\n"
            ),
            Err(FdfError::Unsupported {
                property: "block",
                value,
                ..
            }) if value.contains("Frame inside Texture")
        ));
    }
}
