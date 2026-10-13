//! The shipped skin table, and the authored face → font-file binding.
//!
//! `UI\war3skins.txt` carries a `// font references` block inside its `[Default]`
//! section:
//!
//! ```text
//! // font references
//! MasterFont=Fonts\FRIZQT__.TTF
//! MessageFont=Fonts\FRIZQT__.TTF
//! ...
//! ```
//!
//! A `FrameFont "MasterFont", 0.013, ""` record names a **face**, not a file; the
//! file comes from this table. So the binding is read here rather than copied: the
//! caller passes the face it parsed from the FDF, and this resolves it against the
//! shipped table, reporting where it was found and refusing anything ambiguous.
//!
//! The table is sectioned — `[Main]`, `[Human]`, `[Orc]`, `[NightElf]`,
//! `[Undead]`, `[Default]`. Requested-section-first lookup is caller policy;
//! the native override order is unverified. The supplied tables define their
//! font keys only in `[Default]`.
//!
//! Pure `std`, so it is testable without a window.

use std::fmt;

/// Why a face could not be resolved to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkinError {
    /// No section defines the face.
    UnknownFace(String),
    /// The winning section defines the face more than once, so which value the
    /// engine would use is not established. Reported rather than picked. This
    /// covers both a repeated key inside one block and the same key in a second
    /// block of the same section name.
    Duplicate {
        /// The section that defines it more than once.
        section: String,
        /// The face name.
        face: String,
        /// The values, in file order.
        values: Vec<String>,
    },
    /// The face resolves to an empty value, which cannot name a file. Refused
    /// rather than returned as a binding with no file.
    EmptyValue {
        /// The section the entry came from.
        section: String,
        /// The face name.
        face: String,
    },
    /// A line is neither a section header nor a `key=value` pair, or it has an
    /// empty section name or an empty key.
    Malformed(String),
}

impl fmt::Display for SkinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFace(face) => write!(f, "no skin section defines {face:?}"),
            Self::Duplicate {
                section,
                face,
                values,
            } => write!(f, "[{section}] defines {face:?} {values:?}"),
            Self::EmptyValue { section, face } => {
                write!(f, "[{section}] defines {face:?} with no value")
            }
            Self::Malformed(line) => write!(f, "unreadable skin line {line:?}"),
        }
    }
}

impl std::error::Error for SkinError {}

/// One `[Section]` and its `key=value` entries, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The name inside the brackets.
    pub name: String,
    /// The entries, in file order, keeping duplicates.
    pub entries: Vec<(String, String)>,
}

/// The table's fallback section, consulted after the caller-selected section.
/// This lookup policy is not proof of native engine precedence.
pub const DEFAULT_SECTION: &str = "Default";

/// A parsed `war3skins.txt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinTable {
    sections: Vec<Section>,
}

/// Where a face was found, and what it resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontBinding {
    /// The file the face names, e.g. `Fonts\FRIZQT__.TTF`.
    pub file: String,
    /// The section the winning entry came from, so an override is visible.
    pub section: String,
}

impl SkinTable {
    /// Parses a skin table.
    ///
    /// `//` comments and blank lines are skipped; a line before any section
    /// header, or one that is neither `[Name]` nor `key=value`, is refused rather
    /// than ignored, so a shifted parse cannot silently resolve a face to the
    /// wrong entry.
    ///
    /// # Errors
    ///
    /// [`SkinError::Malformed`] for a line that does not fit either shape.
    pub fn parse(text: &str) -> Result<Self, SkinError> {
        let mut sections: Vec<Section> = Vec::new();
        for raw in text.lines() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Err(SkinError::Malformed(line.to_string()));
                }
                sections.push(Section {
                    name,
                    entries: Vec::new(),
                });
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(SkinError::Malformed(line.to_string()));
            };
            let key = key.trim();
            if key.is_empty() {
                return Err(SkinError::Malformed(line.to_string()));
            }
            let Some(current) = sections.last_mut() else {
                return Err(SkinError::Malformed(line.to_string()));
            };
            current
                .entries
                .push((key.to_string(), value.trim().to_string()));
        }
        Ok(Self { sections })
    }

    /// Every section, in file order.
    #[must_use]
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// The entries a named section defines, or `None` when it is absent.
    #[must_use]
    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    /// Resolves a face to its font file.
    ///
    /// The requested `section` is consulted first, then [`DEFAULT_SECTION`]. That
    /// order is **caller policy, not the verified engine order** — no native
    /// observation of the game's font lookup has been made, so a race section
    /// overriding `[Default]` is an inference from the table's shape, not a proven
    /// behaviour. What is proven is the data: the keys, their sections and the file
    /// they name.
    ///
    /// **Every** block with the consulted section name is aggregated, not just the
    /// first, so a repeated `[Default]` cannot hide a second definition. Within the
    /// section that wins, the face must appear **exactly once**: two entries — in
    /// one block or across two blocks of the same name — make the result ambiguous
    /// and are refused rather than picked.
    ///
    /// # Errors
    ///
    /// [`SkinError::UnknownFace`] when neither section defines it,
    /// [`SkinError::Duplicate`] when the winning section defines it more than once,
    /// and [`SkinError::EmptyValue`] when it resolves to nothing. No default face or
    /// file is substituted.
    pub fn font_file(&self, face: &str, section: &str) -> Result<FontBinding, SkinError> {
        for candidate in [section, DEFAULT_SECTION] {
            let values = self.values_in(candidate, face);
            match values.len() {
                0 => continue,
                1 => {
                    let file = values.into_iter().next().unwrap_or_default();
                    if file.is_empty() {
                        return Err(SkinError::EmptyValue {
                            section: candidate.to_string(),
                            face: face.to_string(),
                        });
                    }
                    return Ok(FontBinding {
                        file,
                        section: candidate.to_string(),
                    });
                }
                _ => {
                    return Err(SkinError::Duplicate {
                        section: candidate.to_string(),
                        face: face.to_string(),
                        values,
                    });
                }
            }
        }
        Err(SkinError::UnknownFace(face.to_string()))
    }

    /// Every value `face` takes across **all** blocks named `section`, in file
    /// order. Aggregating is the point: reading only the first block would let a
    /// repeated section hide a conflicting definition.
    fn values_in(&self, section: &str, face: &str) -> Vec<String> {
        self.sections
            .iter()
            .filter(|s| s.name == section)
            .flat_map(|s| s.entries.iter())
            .filter(|(k, _)| k == face)
            .map(|(_, v)| v.clone())
            .collect()
    }
}

/// Strips a `//` comment.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped shape: the font keys live only in `[Default]`, under a comment,
    /// and the race sections define their own keys.
    const TABLE: &str = r#"
[Human]
HumanUITile01=UI\Widgets\Console\Human\HumanUITile01.blp

[Default]
// font references
MasterFont=Fonts\FRIZQT__.TTF
MessageFont=Fonts\FRIZQT__.TTF

[Undead]
UndeadUITile01=UI\Widgets\Console\Undead\UndeadUITile01.blp
"#;

    #[test]
    fn a_face_resolves_through_the_default_section() {
        let t = SkinTable::parse(TABLE).unwrap();
        // A race section does not define it, so `[Default]` supplies it, and the
        // binding says which section won.
        let b = t.font_file("MasterFont", "Human").unwrap();
        assert_eq!(b.file, r"Fonts\FRIZQT__.TTF");
        assert_eq!(b.section, DEFAULT_SECTION);
        // Asking for `[Default]` directly gives the same entry.
        assert_eq!(t.font_file("MasterFont", DEFAULT_SECTION).unwrap(), b);
    }

    /// A race section that defines the face wins over `[Default]`.
    #[test]
    fn a_section_override_wins_and_keeps_its_identity() {
        let text = "[Default]\nMasterFont=Fonts\\A.TTF\n[Human]\nMasterFont=Fonts\\B.TTF\n";
        let t = SkinTable::parse(text).unwrap();
        let b = t.font_file("MasterFont", "Human").unwrap();
        assert_eq!(b.file, r"Fonts\B.TTF");
        assert_eq!(b.section, "Human", "the winning section is preserved");
        // The other section still resolves to its own value.
        assert_eq!(
            t.font_file("MasterFont", "Orc").unwrap().file,
            r"Fonts\A.TTF"
        );
    }

    /// A face the table does not define is refused, never defaulted.
    #[test]
    fn an_unknown_face_is_refused() {
        let t = SkinTable::parse(TABLE).unwrap();
        assert_eq!(
            t.font_file("NoSuchFont", "Human"),
            Err(SkinError::UnknownFace("NoSuchFont".to_string()))
        );
    }

    /// Two entries for one face in the winning section are ambiguous, not picked.
    #[test]
    fn a_duplicate_face_is_ambiguous() {
        let text = "[Default]\nMasterFont=Fonts\\A.TTF\nMasterFont=Fonts\\B.TTF\n";
        let t = SkinTable::parse(text).unwrap();
        assert_eq!(
            t.font_file("MasterFont", "Human"),
            Err(SkinError::Duplicate {
                section: DEFAULT_SECTION.to_string(),
                face: "MasterFont".to_string(),
                values: vec![r"Fonts\A.TTF".to_string(), r"Fonts\B.TTF".to_string()],
            })
        );
    }

    /// A line that is neither a section nor a pair is refused.
    #[test]
    fn malformed_input_is_refused() {
        assert!(matches!(
            SkinTable::parse("no equals here\n"),
            Err(SkinError::Malformed(_))
        ));
        assert!(matches!(
            SkinTable::parse("[Default]\nok=1\nbroken line\n"),
            Err(SkinError::Malformed(_))
        ));
    }

    /// A repeated section name must not hide a second definition: every block with
    /// the name is aggregated, so a face defined in two `[Default]` blocks is
    /// ambiguous even when the two values agree.
    #[test]
    fn a_repeated_section_does_not_hide_a_duplicate_face() {
        let text = "[Default]\nMasterFont=Fonts\\A.TTF\n[Human]\nHumanX=1\n[Default]\nMasterFont=Fonts\\A.TTF\n";
        let t = SkinTable::parse(text).unwrap();
        assert_eq!(
            t.font_file("MasterFont", "Human"),
            Err(SkinError::Duplicate {
                section: DEFAULT_SECTION.to_string(),
                face: "MasterFont".to_string(),
                values: vec![r"Fonts\A.TTF".to_string(), r"Fonts\A.TTF".to_string()],
            })
        );
    }

    /// Two blocks of one section naming different files is the conflict the
    /// aggregation exists to catch.
    #[test]
    fn conflicting_values_across_blocks_are_ambiguous() {
        let text = "[Default]\nMasterFont=Fonts\\A.TTF\n[Default]\nMasterFont=Fonts\\B.TTF\n";
        let t = SkinTable::parse(text).unwrap();
        assert!(matches!(
            t.font_file("MasterFont", "Default"),
            Err(SkinError::Duplicate { values, .. }) if values.len() == 2
        ));
    }

    /// A face that resolves to nothing is refused, never returned as a binding
    /// with an empty file.
    #[test]
    fn an_empty_binding_is_refused() {
        let t = SkinTable::parse("[Default]\nMasterFont=\n").unwrap();
        assert_eq!(
            t.font_file("MasterFont", "Human"),
            Err(SkinError::EmptyValue {
                section: DEFAULT_SECTION.to_string(),
                face: "MasterFont".to_string(),
            })
        );
    }

    /// An empty section name or an empty key is malformed.
    #[test]
    fn empty_section_or_key_is_refused() {
        assert!(matches!(
            SkinTable::parse("[]\n"),
            Err(SkinError::Malformed(_))
        ));
        assert!(matches!(
            SkinTable::parse("[Default]\n=value\n"),
            Err(SkinError::Malformed(_))
        ));
    }

    /// Sections keep file order and duplicates are kept rather than collapsed, so
    /// an ambiguity is visible.
    #[test]
    fn sections_and_duplicates_are_preserved() {
        let t = SkinTable::parse(TABLE).unwrap();
        assert_eq!(t.sections()[0].name, "Human");
        assert_eq!(t.section("Default").unwrap().entries.len(), 2);
    }
}
