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
//! Scope: `Frame "TYPE" "NAME" [INHERITS [WITHCHILDREN] "TEMPLATE"] { … }` blocks
//! and the handful of properties listed in [`Props`]. Comments (`//`) are
//! stripped; everything else is ignored, not guessed at.

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
}

impl std::fmt::Display for FdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
    /// `BackdropBackground`.
    pub backdrop_background: Option<String>,
    /// `BackdropEdgeFile`.
    pub backdrop_edge: Option<String>,
    /// `BackdropBackgroundInsets`, left/bottom/right/top as written.
    pub insets: Option<[f32; 4]>,
    /// `BackdropCornerSize`.
    pub corner_size: Option<f32>,
    /// `BackdropBackgroundSize`.
    pub background_size: Option<f32>,
    /// `ControlBackdrop`, naming the frame drawn for the normal state.
    pub control_backdrop: Option<String>,
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
}

/// Every frame parsed from one or more FDF texts, indexed by name.
#[derive(Debug, Clone, Default)]
pub struct Fdf {
    frames: Vec<Frame>,
    index: HashMap<String, usize>,
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
        let mut stack: Vec<usize> = Vec::new();
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
                let (frame, opens) = parse_header(rest)?;
                let parent = stack.last().map(|i| self.frames[*i].name.clone());
                let idx = self.frames.len();
                self.index.insert(frame.name.clone(), idx);
                if let Some(p) = &parent {
                    if let Some(pi) = self.index.get(p).copied() {
                        self.frames[pi].children.push(frame.name.clone());
                    }
                }
                self.frames.push(Frame { parent, ..frame });
                if opens {
                    stack.push(idx);
                }
                continue;
            }
            // A property line belongs to the innermost open frame.
            if let Some(&cur) = stack.last() {
                apply_property(&mut self.frames[cur], line)?;
            }
        }
        if let Some(&open) = stack.last() {
            return Err(FdfError::Unterminated(self.frames[open].name.clone()));
        }
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
            .ok_or_else(|| FdfError::Missing {
                frame: name.to_string(),
                property: "Height",
            })
    }

    /// A frame's `BackdropBackgroundInsets`, along the inheritance chain.
    ///
    /// # Errors
    ///
    /// [`FdfError::Missing`] when neither the frame nor its templates set it.
    pub fn insets(&self, name: &str) -> Result<[f32; 4], FdfError> {
        self.inherited(name, |p| p.insets)
            .ok_or_else(|| FdfError::Missing {
                frame: name.to_string(),
                property: "BackdropBackgroundInsets",
            })
    }

    /// A frame's `ControlBackdrop`, along the inheritance chain.
    ///
    /// The normal-state backdrop is usually declared on the *template* — every
    /// button inherits `StandardButtonTemplate`'s `ControlBackdrop
    /// "ButtonBackdropTemplate"` — so a direct read of the frame would find
    /// nothing.
    #[must_use]
    pub fn control_backdrop(&self, name: &str) -> Option<String> {
        self.inherited(name, |p| p.control_backdrop.clone())
    }

    /// The frame's first `SetPoint`, if it has one.
    #[must_use]
    pub fn set_point(&self, name: &str) -> Option<SetPoint> {
        self.inherited(name, |p| p.set_points.first().cloned())
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
        if self.frame(name).is_none() {
            return Err(FdfError::UnknownFrame(name.to_string()));
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
        },
        rest.contains('{'),
    ))
}

/// Applies one `Key value` line to a frame.
fn apply_property(frame: &mut Frame, line: &str) -> Result<(), FdfError> {
    let (key, value) = match line.split_once(char::is_whitespace) {
        Some((k, v)) => (k, v.trim()),
        None => (line, ""),
    };
    let bad = |property: &'static str| FdfError::BadNumber {
        frame: frame.name.clone(),
        property,
        text: value.to_string(),
    };
    match key {
        "Width" => frame.props.width = Some(num(value).ok_or_else(|| bad("Width"))?),
        "Height" => frame.props.height = Some(num(value).ok_or_else(|| bad("Height"))?),
        "BackdropCornerSize" => {
            frame.props.corner_size = Some(num(value).ok_or_else(|| bad("BackdropCornerSize"))?)
        }
        "BackdropBackgroundSize" => {
            frame.props.background_size =
                Some(num(value).ok_or_else(|| bad("BackdropBackgroundSize"))?)
        }
        "BackdropBackgroundInsets" => {
            let parts: Vec<f32> = value
                .trim_end_matches(',')
                .split_whitespace()
                .filter_map(|t| num(t))
                .collect();
            if parts.len() != 4 {
                return Err(bad("BackdropBackgroundInsets"));
            }
            frame.props.insets = Some([parts[0], parts[1], parts[2], parts[3]]);
        }
        "BackdropBackground" => frame.props.backdrop_background = Some(unquote(value)),
        "BackdropEdgeFile" => frame.props.backdrop_edge = Some(unquote(value)),
        "ControlBackdrop" => frame.props.control_backdrop = Some(unquote(value)),
        "SetPoint" => frame.props.set_points.push(parse_set_point(value)?),
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
        assert_eq!(f.insets("ButtonBackdropTemplate").unwrap(), [0.004; 4]);
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

    /// A missing property is reported, not defaulted.
    #[test]
    fn a_missing_property_is_reported() {
        let f = fdf();
        assert_eq!(
            f.insets("SinglePlayerButton"),
            Err(FdfError::Missing {
                frame: "SinglePlayerButton".to_string(),
                property: "BackdropBackgroundInsets",
            })
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
}
