//! Layout and labels of the campaign score screen, read from the selected
//! `UI\FrameDef\Glue\ScoreScreen.fdf`. The include list comes from that file's
//! `IncludeFile` lines, and every label is a `GlobalStrings` key. A missing include,
//! key or frame is an error, never a substituted default.
//!
//! This module derives geometry and text only. Score rows are not here: their values
//! come from the simulation counters, which are not yet qualified.
use crate::fdf::{Fdf, FdfError, Rect};
use crate::global_strings::global_string;

/// Screen frame every presentation element hangs from.
pub const ROOT_FRAME: &str = "ScoreScreenFrame";

#[derive(Debug, Clone, PartialEq)]
pub enum LayoutError {
    /// An `IncludeFile` named by the screen was not supplied.
    MissingInclude(String),
    Fdf(FdfError),
    /// The frame's `Text` key has no `GlobalStrings` record.
    MissingString(String),
    /// The frame has no `Text` property.
    NoText(String),
}

impl From<FdfError> for LayoutError {
    fn from(e: FdfError) -> Self {
        Self::Fdf(e)
    }
}

/// The `IncludeFile` paths a screen names, in file order.
pub fn includes(screen: &str) -> Vec<&str> {
    screen
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("IncludeFile")?.trim();
            let rest = rest.strip_prefix('"')?;
            rest.split_once('"').map(|(path, _)| path)
        })
        .collect()
}

/// The parsed screen with its includes applied, and the string table it labels from.
pub struct ScoreScreenLayout {
    fdf: Fdf,
    strings: String,
}

impl ScoreScreenLayout {
    /// `includes` are the texts of each path [`includes`] returns, keyed by that path.
    pub fn parse(
        screen: &str,
        includes_text: &[(&str, &str)],
        strings: &str,
    ) -> Result<Self, LayoutError> {
        let mut fdf = Fdf::default();
        for path in includes(screen) {
            let (_, text) = includes_text
                .iter()
                .find(|(p, _)| *p == path)
                .ok_or_else(|| LayoutError::MissingInclude(path.to_owned()))?;
            fdf.parse_into(text)?;
        }
        fdf.parse_into(screen)?;
        Ok(Self {
            fdf,
            strings: strings.to_owned(),
        })
    }

    /// The parsed screen, for the view's glue helpers.
    pub fn fdf(&self) -> &Fdf {
        &self.fdf
    }

    /// Rectangle of `frame` in the given viewport. Frames without an authored width or
    /// height need their text measured first; that refusal is reported, not guessed.
    pub fn rect(&self, frame: &str, viewport: (f32, f32)) -> Result<Rect, LayoutError> {
        Ok(self.fdf.rect(frame, viewport)?)
    }

    /// The label of a `TEXT` frame, looked up in the string table its key names.
    pub fn label(&self, frame: &str) -> Result<String, LayoutError> {
        let key = self
            .fdf
            .text_value(frame)
            .ok_or_else(|| LayoutError::NoText(frame.to_owned()))?;
        global_string(&self.strings, &key).ok_or(LayoutError::MissingString(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: &str = "\
IncludeFile \"UI\\FrameDef\\Glue\\Templates.fdf\",
Frame \"FRAME\" \"ScoreScreenFrame\" {
\tWidth 0.8,
\tHeight 0.6,
}
Frame\"TEXT\" \"ScoreScreenElapsedTime\" INHERITS \"Label\" {
\tText \"COLON_ELAPSED_TIME\",
}
Frame \"BUTTON\" \"ScoreScreenOk\" {
\tWidth 0.2,
\tHeight 0.03,
\tText \"OK_WHITE\",
}
";
    const TEMPLATES: &str = "\
Frame \"TEXT\" \"Label\" {
\tWidth 0.1,
\tHeight 0.02,
}
";
    const STRINGS: &str = "\
COLON_ELAPSED_TIME \"Elapsed Time:\",
OK_WHITE \"|cffffffffO|rK\",
";

    fn layout() -> ScoreScreenLayout {
        ScoreScreenLayout::parse(SCREEN, &[("UI\\FrameDef\\Glue\\Templates.fdf", TEMPLATES)], STRINGS)
            .unwrap()
    }

    #[test]
    fn includes_are_the_quoted_paths_in_file_order() {
        assert_eq!(includes(SCREEN), vec!["UI\\FrameDef\\Glue\\Templates.fdf"]);
    }

    #[test]
    fn a_named_include_must_be_supplied() {
        assert_eq!(
            ScoreScreenLayout::parse(SCREEN, &[], STRINGS).err(),
            Some(LayoutError::MissingInclude("UI\\FrameDef\\Glue\\Templates.fdf".into()))
        );
    }

    #[test]
    fn inherited_template_geometry_and_the_shorthand_header_resolve() {
        let l = layout();
        assert_eq!(
            l.rect("ScoreScreenElapsedTime", (0.8, 0.6)).unwrap(),
            Rect { left: 0.0, bottom: 0.0, width: 0.1, height: 0.02 }
        );
        assert_eq!(l.rect(ROOT_FRAME, (0.8, 0.6)).unwrap().width, 0.8);
    }

    #[test]
    fn labels_come_from_the_string_table_and_missing_keys_are_refused() {
        let l = layout();
        assert_eq!(l.label("ScoreScreenElapsedTime").unwrap(), "Elapsed Time:");
        assert_eq!(l.label("ScoreScreenOk").unwrap(), "|cffffffffO|rK");
        assert_eq!(l.label(ROOT_FRAME), Err(LayoutError::NoText(ROOT_FRAME.into())));
        let no_strings =
            ScoreScreenLayout::parse(SCREEN, &[("UI\\FrameDef\\Glue\\Templates.fdf", TEMPLATES)], "")
                .unwrap();
        assert_eq!(
            no_strings.label("ScoreScreenOk"),
            Err(LayoutError::MissingString("OK_WHITE".into()))
        );
    }

    /// Original-input gate. `PCX_SCORE_UI_DIR` names a local directory holding the
    /// selected `ScoreScreen.fdf`, its `IncludeFile`s and `GlobalStrings.fdf`, named as
    /// in the archive. Unset, it skips.
    #[test]
    #[ignore = "requires PCX_SCORE_UI_DIR with locally extracted selected score UI files"]
    fn original_score_screen_resolves_its_includes_labels_and_root() {
        let Some(dir) = std::env::var_os("PCX_SCORE_UI_DIR") else {
            return;
        };
        let read = |name: &str| std::fs::read_to_string(std::path::Path::new(&dir).join(name)).unwrap();
        let screen = read("ScoreScreen.fdf");
        let include_names: Vec<String> = includes(&screen)
            .iter()
            .map(|p| p.rsplit('\\').next().unwrap().to_owned())
            .collect();
        let texts: Vec<(String, String)> = include_names
            .iter()
            .map(|n| (n.clone(), read(n)))
            .collect();
        let borrowed: Vec<(&str, &str)> = includes(&screen)
            .into_iter()
            .zip(texts.iter().map(|(_, t)| t.as_str()))
            .collect();
        let strings = read("GlobalStrings.fdf");
        let layout = ScoreScreenLayout::parse(&screen, &borrowed, &strings).unwrap();
        assert!(layout.rect(ROOT_FRAME, (0.8, 0.6)).unwrap().width > 0.0);
        for frame in ["ScoreScreenElapsedTimeLabel", "ScoreScreenOkText", "ScoreScreenTeamsText"] {
            assert!(layout.label(frame).is_ok(), "{frame}");
        }
    }
}
