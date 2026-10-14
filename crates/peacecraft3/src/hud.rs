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
        rect: fdf.rect(name, viewport)?,
        width: width_or_absent(fdf, name)?,
        height: height_or_absent(fdf, name)?,
        records: frame.props.records.clone(),
        dependencies,
    };
    out.push(derived);

    for child in frame.children.clone() {
        visit(fdf, &child, viewport, out, seen)?;
    }
    if let Some(sp) = fdf.set_point(name) {
        // Resolve the anchor target when it is a defined frame. This is needed
        // for layouts that are connected through anchors rather than nesting.
        if fdf.frame(&sp.relative).is_some() {
            visit(fdf, &sp.relative, viewport, out, seen)?;
        }
    }
    if let Some(label) = fdf.button_text(name)
        && fdf.frame(&label).is_some()
    {
        visit(fdf, &label, viewport, out, seen)?;
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
            Fdf::parse("Frame \"FRAME\" \"ROOT\" {\n    Width 1.0,\n    Height 1.0,\n}\n")
                .unwrap();
        assert!(matches!(
            derive(&fdf, &["NOPE"], (0.8, 0.6)),
            Err(FdfError::UnknownFrame(_))
        ));
    }
}
