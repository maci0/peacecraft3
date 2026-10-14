//! HUD frame derivation from authored FDF data.
//!
//! The in-game console is described by its FDF frame tree. Given a parsed
//! [`super::fdf::Fdf`] and caller-supplied authored root frame names, this
//! derives the reachable frame graph, resolving inherited anchors, authored
//! records ("styles") and frame-to-frame dependencies without inlining any of
//! those values as constants.
//!
//! No production integration exists yet: this module is a source-leaf
//! derivation utility and does not spawn Bevy entities or choose frame roles.

use std::collections::HashSet;

use super::fdf::{Fdf, FdfError, Rect, SetPoint};

/// A frame-to-frame dependency discovered while deriving a HUD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HudDependency {
    /// `INHERITS "TEMPLATE"`.
    Template(String),
    /// Enclosing FDF parent frame.
    Parent(String),
    /// `SetPoint` relative frame.
    Anchor(String),
    /// `ButtonText` label frame.
    Label(String),
    /// Nested FDF child frame.
    Child(String),
}

/// One derived HUD frame.
#[derive(Debug, Clone, PartialEq)]
pub struct HudFrame {
    /// Frame name.
    pub name: String,
    /// Frame kind (`FRAME`, `BACKDROP`, ...).
    pub kind: String,
    /// Template name from `INHERITS`, if any.
    pub inherits: Option<String>,
    /// `INHERITS WITHCHILDREN`.
    pub with_children: bool,
    /// Enclosing frame, if any.
    pub parent: Option<String>,
    /// Nested frame names, in FDF order.
    pub children: Vec<String>,
    /// `ButtonText` target, if any.
    pub button_text: Option<String>,
    /// Authored `Text`, if any.
    pub text: Option<String>,
    /// `SetAllPoints`.
    pub set_all_points: bool,
    /// First resolved `SetPoint`, including inherited values.
    pub set_point: Option<SetPoint>,
    /// Resolved frame rectangle in Y-up frame units.
    pub rect: Rect,
    /// Width, when authored or inherited.
    pub width: Option<f32>,
    /// Height, when authored or inherited.
    pub height: Option<f32>,
    /// Authored records on this frame.
    pub records: Vec<(String, String)>,
    /// Derived frame dependencies.
    pub dependencies: Vec<HudDependency>,
}

/// Derived HUD frame graph.
#[derive(Debug, Clone, PartialEq)]
pub struct HudLayout {
    /// Authored root frame names supplied by the caller.
    pub roots: Vec<String>,
    /// Viewport used for resolution, frame units.
    pub viewport: (f32, f32),
    /// Derived frames in visitation order.
    pub frames: Vec<HudFrame>,
}

/// Derives all frames reachable from `roots` through FDF children, anchors and
/// label frames.
///
/// Roots are caller-supplied authored names. Rectangles resolve through the FDF's
/// existing anchor inheritance. An unknown root, cycle, or bad authored value
/// propagates as [`FdfError`].
pub fn derive(fdf: &Fdf, roots: &[&str], viewport: (f32, f32)) -> Result<HudLayout, FdfError> {
    for (property, value) in [("Width", viewport.0), ("Height", viewport.1)] {
        if !value.is_finite() || value <= 0.0 {
            return Err(FdfError::Unsupported {
                frame: "<viewport>".into(),
                property,
                value: value.to_string(),
            });
        }
    }
    let mut layout = HudLayout {
        roots: roots.iter().map(|s| (*s).to_string()).collect(),
        viewport,
        frames: Vec::new(),
    };
    let mut seen = Vec::new();
    for root in roots {
        visit(fdf, root, viewport, &mut layout.frames, &mut seen)?;
    }
    Ok(layout)
}

fn visit(
    fdf: &Fdf,
    name: &str,
    viewport: (f32, f32),
    out: &mut Vec<HudFrame>,
    seen: &mut Vec<String>,
) -> Result<(), FdfError> {
    if seen.iter().any(|s| s == name) {
        return Ok(());
    }
    let Some(frame) = fdf.frame(name) else {
        return Err(FdfError::UnknownFrame(name.to_string()));
    };
    validate_inheritance(fdf, name)?;
    if fdf.flag(name, "SetAllPoints") && fdf.set_point(name).is_some() {
        return Err(FdfError::Unsupported {
            frame: name.into(),
            property: "SetAllPoints",
            value: "resolved SetPoint is also present".into(),
        });
    }
    seen.push(name.to_string());

    let mut dependencies = Vec::new();
    if let Some(template) = &frame.inherits {
        dependencies.push(HudDependency::Template(template.clone()));
    }
    if let Some(parent) = &frame.parent {
        dependencies.push(HudDependency::Parent(parent.clone()));
    }
    if let Some(sp) = fdf.set_point(name) {
        dependencies.push(HudDependency::Anchor(sp.relative.clone()));
    }
    if let Some(label) = fdf.button_text(name) {
        dependencies.push(HudDependency::Label(label.clone()));
    }
    for child in &frame.children {
        dependencies.push(HudDependency::Child(child.clone()));
    }

    let width = width_or_absent(fdf, name)?;
    let height = height_or_absent(fdf, name)?;
    if frame.parent.is_some() && !fdf.flag(name, "SetAllPoints") {
        for (property, value) in [("Width", width), ("Height", height)] {
            if value.is_none() {
                return Err(FdfError::Missing {
                    frame: name.into(),
                    property,
                });
            }
        }
    }
    let rect = fdf.rect(name, viewport)?;
    if [
        rect.left,
        rect.bottom,
        rect.width,
        rect.height,
        rect.left + rect.width,
        rect.top(),
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return Err(FdfError::Unsupported {
            frame: name.into(),
            property: "rectangle",
            value: format!("{rect:?}"),
        });
    }
    let derived = HudFrame {
        name: name.to_string(),
        kind: frame.kind.clone(),
        inherits: frame.inherits.clone(),
        with_children: frame.with_children,
        parent: frame.parent.clone(),
        children: frame.children.clone(),
        button_text: fdf.button_text(name),
        text: fdf.text_value(name),
        set_all_points: fdf.flag(name, "SetAllPoints"),
        set_point: fdf.set_point(name),
        rect,
        width,
        height,
        records: frame.props.records.clone(),
        dependencies,
    };
    out.push(derived);

    for child in &frame.children {
        visit(fdf, child, viewport, out, seen)?;
    }
    if let Some(parent) = &frame.parent {
        visit(fdf, parent, viewport, out, seen)?;
    }
    if let Some(sp) = fdf.set_point(name) {
        visit(fdf, &sp.relative, viewport, out, seen)?;
    }
    if let Some(label) = fdf.button_text(name) {
        visit(fdf, &label, viewport, out, seen)?;
    }
    Ok(())
}

// Baseline lookup treats a broken template chain as absence. Validate it before
// asking that lookup to supply dimensions or a viewport-backed rectangle.
fn validate_inheritance(fdf: &Fdf, name: &str) -> Result<(), FdfError> {
    let mut current = Some(name);
    let mut seen = HashSet::new();
    while let Some(name) = current {
        if !seen.insert(name) {
            return Err(FdfError::Cycle(name.into()));
        }
        let frame = fdf
            .frame(name)
            .ok_or_else(|| FdfError::UnknownFrame(name.into()))?;
        let unsupported = |property, value| FdfError::Unsupported {
            frame: name.into(),
            property,
            value,
        };
        if frame.with_children {
            return Err(unsupported(
                "INHERITS WITHCHILDREN",
                frame.inherits.clone().unwrap_or_default(),
            ));
        }
        for property in [
            "Width",
            "Height",
            "ButtonText",
            "Text",
            "SetAllPoints",
            "SetPoint",
        ] {
            if frame
                .props
                .records
                .iter()
                .filter(|(key, _)| key == property)
                .count()
                > 1
            {
                return Err(unsupported(property, "multiple authored records".into()));
            }
        }
        if frame
            .props
            .records
            .iter()
            .any(|(key, value)| key == "SetAllPoints" && !value.is_empty())
        {
            return Err(unsupported(
                "SetAllPoints",
                "flag has an authored value".into(),
            ));
        }
        for (property, value) in [("Width", frame.props.width), ("Height", frame.props.height)] {
            if let Some(value) = value
                && (!value.is_finite() || value < 0.0)
            {
                return Err(unsupported(property, value.to_string()));
            }
        }
        for point in &frame.props.set_points {
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(unsupported("SetPoint", format!("{point:?}")));
            }
        }
        current = frame.inherits.as_deref();
    }
    Ok(())
}

fn width_or_absent(fdf: &Fdf, name: &str) -> Result<Option<f32>, FdfError> {
    match fdf.width(name) {
        Ok(value) => Ok(Some(value)),
        Err(FdfError::Missing { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

fn height_or_absent(fdf: &Fdf, name: &str) -> Result<Option<f32>, FdfError> {
    match fdf.height(name) {
        Ok(value) => Ok(Some(value)),
        Err(FdfError::Missing { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

impl HudLayout {
    /// Frame by name, if derived.
    #[must_use]
    pub fn frame(&self, name: &str) -> Option<&HudFrame> {
        self.frames.iter().find(|f| f.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_resolves_anchor_inheritance_and_children() {
        let text = r#"
Frame "FRAME" "TEMPLATE" {
    Width 0.25,
    Height 0.125,
}
Frame "FRAME" "ROOT" {
    Width 0.8,
    Height 0.6,
    Frame "BACKDROP" "CHILD" INHERITS "TEMPLATE" {
        SetPoint TOPLEFT, "ROOT", TOPLEFT, 0.05, -0.04,
    }
}
"#;
        let fdf = Fdf::parse(text).unwrap();
        let layout = derive(&fdf, &["ROOT"], (0.8, 0.6)).unwrap();
        assert_eq!(layout.roots, vec!["ROOT".to_string()]);
        assert_eq!(layout.frames.len(), 2);
        let root = layout.frame("ROOT").unwrap();
        assert_eq!(root.rect.width, 0.8);
        assert_eq!(root.rect.height, 0.6);
        let child = layout.frame("CHILD").unwrap();
        assert_eq!(child.inherits.as_deref(), Some("TEMPLATE"));
        assert_eq!(child.width, Some(0.25));
        assert_eq!(child.height, Some(0.125));
        assert!(
            root.dependencies
                .iter()
                .any(|d| *d == HudDependency::Child("CHILD".to_string()))
        );
        assert_eq!(child.rect.left, 0.05);
        assert_eq!(child.rect.top(), 0.6 - 0.04);
    }

    #[test]
    fn derives_label_and_anchor_dependencies() {
        let text = r#"
Frame "TEXT" "LABEL" {
    Width 0.1,
    Height 0.02,
}
Frame "FRAME" "ROOT" {
    Width 0.8,
    Height 0.6,
    Frame "GLUEBUTTON" "BUTTON" {
        Width 0.2,
        Height 0.08,
        SetPoint TOPLEFT, "ROOT", TOPLEFT, 0.0, 0.0,
        ButtonText "LABEL",
    }
}
"#;
        let fdf = Fdf::parse(text).unwrap();
        let layout = derive(&fdf, &["ROOT"], (0.8, 0.6)).unwrap();
        let button = layout.frame("BUTTON").unwrap();
        assert!(
            button
                .dependencies
                .iter()
                .any(|d| *d == HudDependency::Label("LABEL".to_string()))
        );
        assert!(
            button
                .dependencies
                .iter()
                .any(|d| *d == HudDependency::Anchor("ROOT".to_string()))
        );
        assert!(layout.frame("LABEL").is_some());
    }

    #[test]
    fn rejects_unknown_root() {
        let fdf =
            Fdf::parse("Frame \"FRAME\" \"ROOT\" {\n    Width 1.0,\n    Height 1.0,\n}\n").unwrap();
        assert!(matches!(
            derive(&fdf, &["NOPE"], (0.8, 0.6)),
            Err(FdfError::UnknownFrame(_))
        ));
    }

    #[test]
    fn broken_inheritance_cannot_become_viewport_dimensions() {
        for text in [
            "Frame \"FRAME\" \"A\" INHERITS \"MISSING\" {\n}\n",
            "Frame \"FRAME\" \"A\" INHERITS \"B\" {\n Width 1,\n Height 1,\n}\nFrame \"FRAME\" \"B\" INHERITS \"A\" {\n}\n",
        ] {
            let fdf = Fdf::parse(text).unwrap();
            assert!(matches!(
                derive(&fdf, &["A"], (2.0, 3.0)),
                Err(FdfError::UnknownFrame(_) | FdfError::Cycle(_))
            ));
        }
    }

    #[test]
    fn missing_label_dependency_is_reported() {
        let fdf = Fdf::parse("Frame \"FRAME\" \"A\" {\n ButtonText \"MISSING\",\n}\n").unwrap();
        assert_eq!(
            derive(&fdf, &["A"], (2.0, 3.0)),
            Err(FdfError::UnknownFrame("MISSING".into()))
        );
    }

    #[test]
    fn inherited_children_are_explicitly_unsupported() {
        let fdf = Fdf::parse("Frame \"FRAME\" \"T\" {\n Frame \"FRAME\" \"CHILD\" {\n Width 1,\n Height 1,\n }\n}\nFrame \"FRAME\" \"A\" INHERITS WITHCHILDREN \"T\" {\n}\n").unwrap();
        assert!(matches!(
            derive(&fdf, &["A"], (2.0, 3.0)),
            Err(FdfError::Unsupported {
                property: "INHERITS WITHCHILDREN",
                ..
            })
        ));
    }

    #[test]
    fn ambiguous_or_nonfinite_layout_records_are_refused() {
        for body in [
            "Width NaN,",
            "Height 1e40,",
            "Width -1,",
            "Width 1,\n Width 2,",
            "SetPoint TOPLEFT, \"A\", TOPLEFT, NaN, 0,",
            "SetAllPoints 0,",
            "SetAllPoints,\n SetPoint TOPLEFT, \"A\", TOPLEFT, 0, 0,",
        ] {
            let fdf = Fdf::parse(&format!("Frame \"FRAME\" \"A\" {{\n {body}\n}}\n")).unwrap();
            assert!(
                matches!(
                    derive(&fdf, &["A"], (2.0, 3.0)),
                    Err(FdfError::Unsupported { .. })
                ),
                "{body}"
            );
        }
    }

    #[test]
    fn absent_root_dimensions_use_only_the_supplied_viewport() {
        let fdf = Fdf::parse("Frame \"FRAME\" \"A\" {\n}\n").unwrap();
        let layout = derive(&fdf, &["A"], (2.0, 3.0)).unwrap();
        let frame = layout.frame("A").unwrap();
        assert_eq!((frame.width, frame.height), (None, None));
        assert_eq!((frame.rect.width, frame.rect.height), (2.0, 3.0));
        for viewport in [(f32::NAN, 3.0), (2.0, f32::INFINITY), (0.0, 3.0)] {
            assert!(derive(&fdf, &["A"], viewport).is_err());
        }
    }

    #[test]
    fn a_child_cannot_invent_missing_extents_from_viewport() {
        let fdf =
            Fdf::parse("Frame \"FRAME\" \"A\" {\n Frame \"FRAME\" \"CHILD\" {\n }\n}\n").unwrap();
        assert!(matches!(derive(&fdf, &["A"], (2.0, 3.0)),
            Err(FdfError::Missing { frame, property: "Width" }) if frame == "CHILD"));
    }

    #[test]
    fn set_all_points_requires_a_valid_parent_layout() {
        let fdf = Fdf::parse("Frame \"FRAME\" \"A\" {\n Width 1,\n Width 2,\n Frame \"FRAME\" \"CHILD\" {\n SetAllPoints,\n }\n}\n").unwrap();
        assert!(matches!(derive(&fdf, &["CHILD"], (2.0, 3.0)),
            Err(FdfError::Unsupported { frame, property: "Width", .. }) if frame == "A"));
    }
}
