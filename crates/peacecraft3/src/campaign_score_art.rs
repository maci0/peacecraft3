//! Skin selection of the score screen's race art. `ScoreScreenRaceArt` is a `MODEL` frame
//! with no `BackgroundArt`, so the model it shows is not in the FDF. `war3skins.txt` names it:
//! `ScoreScreenVictory` and `ScoreScreenDefeat` in `[Default]`, and `ScoreScreenVictory_V0`
//! and `_V1` per race section. The executable carries the `ScoreScreenVictory`,
//! `ScoreScreenDefeat`, `ScoreScreenBannerWin` and `ScoreScreenBannerLoss` key strings.
//!
//! Which section wins and which edition suffix applies are recreation choices, shared with
//! the HUD and menu lookups: the local player's race section first, then `[Default]`, and
//! `_V1` for the Frozen Throne edition, `_V0` otherwise. A result with no victory or defeat
//! key, or a missing or ambiguous skin entry, selects nothing.
use crate::menu_skin::SkinTable;
use peacecraft3::campaign_runtime::LocalGameResult;

pub const VICTORY_KEY: &str = "ScoreScreenVictory";
pub const DEFEAT_KEY: &str = "ScoreScreenDefeat";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtError {
    /// The local result is absent, or is a tie or neutral result with no authored key.
    NoKeyForResult(Option<LocalGameResult>),
    /// A section defines the key more than once, or defines it empty.
    Ambiguous { section: String, key: String },
    /// No consulted section defines the key.
    Missing(String),
}

/// The skin key the result selects, before an edition suffix.
fn result_key(result: Option<LocalGameResult>) -> Result<&'static str, ArtError> {
    match result {
        Some(LocalGameResult::Victory) => Ok(VICTORY_KEY),
        Some(LocalGameResult::Defeat) => Ok(DEFEAT_KEY),
        other => Err(ArtError::NoKeyForResult(other)),
    }
}

/// The one value `section` defines for `key`, `None` when it defines none.
fn section_value(skin: &SkinTable, section: &str, key: &str) -> Result<Option<String>, ArtError> {
    let values: Vec<&String> = skin
        .sections()
        .iter()
        .filter(|candidate| candidate.name == section)
        .flat_map(|candidate| &candidate.entries)
        .filter(|(entry, _)| entry == key)
        .map(|(_, value)| value)
        .collect();
    match values.as_slice() {
        [] => Ok(None),
        [value] if !value.is_empty() => Ok(Some((*value).clone())),
        _ => Err(ArtError::Ambiguous {
            section: section.to_owned(),
            key: key.to_owned(),
        }),
    }
}

/// The `.mdl` path the skin names for the local result: per section in `sections` order, the
/// edition-suffixed key, then the plain key.
pub fn race_art_model(
    skin: &SkinTable,
    sections: &[String],
    expansion: bool,
    result: Option<LocalGameResult>,
) -> Result<String, ArtError> {
    let key = result_key(result)?;
    let suffixed = format!("{key}_V{}", u32::from(expansion));
    for section in sections {
        for candidate in [suffixed.as_str(), key] {
            if let Some(value) = section_value(skin, section, candidate)? {
                return Ok(value);
            }
        }
    }
    Err(ArtError::Missing(suffixed))
}

/// Upper bound on the skin table read through the selected resolver.
const SKIN_BYTES: u64 = 4 << 20;

/// The race art model the selected source's skin names for the exiting session's local
/// player and result, as the `.mdl` path the skin states.
pub fn resolve_race_art(
    resolver: &pcx_assets::Resolver,
    local_race: &Result<Option<String>, String>,
    result: Option<LocalGameResult>,
) -> Result<String, String> {
    let table = resolver
        .resolve_bounded(crate::mainmenu::SKIN_TABLE, SKIN_BYTES)
        .map_err(|e| format!("{}: {e}", crate::mainmenu::SKIN_TABLE))?;
    let table = String::from_utf8(table.bytes)
        .map_err(|_| format!("{}: not UTF-8 text", crate::mainmenu::SKIN_TABLE))?;
    let skin =
        SkinTable::parse(&table).map_err(|e| format!("{}: {e:?}", crate::mainmenu::SKIN_TABLE))?;
    let sections = crate::campaign_hud::race_sections(&skin, local_race.clone()?)?;
    let expansion = matches!(resolver.mode().addon, pcx_assets::Addon::Tft);
    race_art_model(&skin, &sections, expansion, result).map_err(|e| format!("{e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKIN: &str = "\
[Human]
ScoreScreenVictory_V0=Human\\Roc.mdl
ScoreScreenVictory_V1=Human\\Tft.mdl
[Orc]
ScoreScreenVictory_V0=Orc\\Roc.mdl
[Default]
ScoreScreenVictory=Default\\Victory.mdl
ScoreScreenDefeat=Default\\Defeat.mdl
";

    fn skin() -> SkinTable {
        SkinTable::parse(SKIN).unwrap()
    }

    fn sections(race: &str) -> Vec<String> {
        vec![race.to_owned(), "Default".to_owned()]
    }

    #[test]
    fn victory_takes_the_race_section_entry_of_the_edition() {
        let victory = Some(LocalGameResult::Victory);
        assert_eq!(
            race_art_model(&skin(), &sections("Human"), false, victory).unwrap(),
            "Human\\Roc.mdl"
        );
        assert_eq!(
            race_art_model(&skin(), &sections("Human"), true, victory).unwrap(),
            "Human\\Tft.mdl"
        );
    }

    #[test]
    fn a_race_without_the_edition_key_falls_to_default_not_to_another_edition() {
        assert_eq!(
            race_art_model(
                &skin(),
                &sections("Orc"),
                true,
                Some(LocalGameResult::Victory)
            )
            .unwrap(),
            "Default\\Victory.mdl"
        );
    }

    #[test]
    fn defeat_reads_the_default_entry() {
        assert_eq!(
            race_art_model(
                &skin(),
                &sections("Human"),
                true,
                Some(LocalGameResult::Defeat)
            )
            .unwrap(),
            "Default\\Defeat.mdl"
        );
    }

    #[test]
    fn tie_neutral_and_absent_results_select_no_art() {
        for result in [
            Some(LocalGameResult::Tie),
            Some(LocalGameResult::Neutral),
            None,
        ] {
            assert_eq!(
                race_art_model(&skin(), &sections("Human"), false, result),
                Err(ArtError::NoKeyForResult(result))
            );
        }
    }

    #[test]
    fn missing_and_duplicate_entries_refuse() {
        let empty = SkinTable::parse("[Default]\nOther=x\n").unwrap();
        assert_eq!(
            race_art_model(
                &empty,
                &sections("Human"),
                false,
                Some(LocalGameResult::Defeat)
            ),
            Err(ArtError::Missing("ScoreScreenDefeat_V0".into()))
        );
        let twice =
            SkinTable::parse("[Default]\nScoreScreenDefeat=a.mdl\nScoreScreenDefeat=b.mdl\n")
                .unwrap();
        assert_eq!(
            race_art_model(
                &twice,
                &sections("Human"),
                false,
                Some(LocalGameResult::Defeat)
            ),
            Err(ArtError::Ambiguous {
                section: "Default".into(),
                key: "ScoreScreenDefeat".into()
            })
        );
    }

    /// Original-input gate. `PCX_SCORE_ART_DIR` names a local directory holding the selected
    /// `UI\war3skins.txt` and the victory and defeat models with their textures, laid out as
    /// in the archives. Every race and edition resolves to a model that prepares for drawing.
    #[test]
    #[ignore = "requires PCX_SCORE_ART_DIR with locally extracted selected score art"]
    fn original_race_art_resolves_and_prepares_for_every_race_and_edition() {
        use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode, OverrideDirectory};
        let Some(dir) = std::env::var_os("PCX_SCORE_ART_DIR") else {
            return;
        };
        for addon in [Addon::NoAddon, Addon::Tft] {
            let mode = GameMode {
                build: Build::V1_29b,
                addon,
                assets: AssetMode::Vanilla,
                behaviour: Behaviour::Vanilla,
            };
            let resolver = pcx_assets::Resolver::new(
                mode,
                Some(
                    OverrideDirectory::open(std::path::Path::new(&dir), "score-art", None).unwrap(),
                ),
                Vec::new(),
            )
            .unwrap();
            for race in ["HUMAN", "ORC", "NIGHTELF", "UNDEAD"] {
                let local_race = Ok(Some(format!("RACE_PREF_{race}")));
                for result in [LocalGameResult::Victory, LocalGameResult::Defeat] {
                    let model = resolve_race_art(&resolver, &local_race, Some(result))
                        .unwrap_or_else(|e| panic!("{addon:?} {race} {result:?}: {e}"));
                    let background = crate::campaign_score_model::AuthoredBackground {
                        frame: "ScoreScreenRaceArt".into(),
                        model: model.clone(),
                        rect: crate::fdf::Rect {
                            left: 0.0,
                            bottom: 0.0,
                            width: 0.8,
                            height: 0.6,
                        },
                        unlit: true,
                    };
                    crate::campaign_score_model::prepare(&resolver, &background)
                        .unwrap_or_else(|e| panic!("{addon:?} {race} {result:?} {model}: {e}"));
                }
            }
        }
    }
}
