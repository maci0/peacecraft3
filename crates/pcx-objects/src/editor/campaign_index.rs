//! Campaign screen index from one caller-selected profile member.
//!
//! Comma splitting is `super::defaults::profile_elements`. Section assignment
//! is `parse_profile`. This module does not implement a second lexer.
//!
//! `first_index_entry` is the first non-blank `CampaignList` slot. It is not
//! a playable, unlocked, or `DefaultOpen` campaign. Those gates are absent.

use super::defaults::{self, Unresolved};

use thiserror::Error;

/// Caller policy for one selected member. Not a native screen limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexLimits {
    pub max_bytes: usize,
    pub max_campaign_slots: usize,
    pub max_missions_per_campaign: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mission {
    pub index: u32,
    pub header: String,
    pub name: String,
    pub map_file: String,
    /// Fields after the map filename. Present in authored rows; not unlock or playability.
    pub uninterpreted: Vec<String>,
}

/// One named `CampaignList` section. Presence here is not unlock or playability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub id: String,
    /// The section's first `Header` and `Name` values, as authored.
    pub header: Option<String>,
    pub name: Option<String>,
    pub missions: Vec<Mission>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CampaignSlot {
    Blank,
    Entry(IndexEntry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignIndex {
    /// Caller-supplied member label. It does not select an archive.
    pub member: String,
    pub slots: Vec<CampaignSlot>,
}

impl CampaignIndex {
    /// First non-blank slot in `CampaignList` order.
    ///
    /// This is index position only. `DefaultOpen`, prerequisites, completion
    /// and UI playability are not read.
    pub fn first_index_entry(&self) -> Option<&IndexEntry> {
        self.slots.iter().find_map(|slot| match slot {
            CampaignSlot::Entry(entry) => Some(entry),
            CampaignSlot::Blank => None,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IndexError {
    #[error("{member}: cannot allocate {what}")]
    Allocation { member: String, what: &'static str },
    #[error("{member}: campaign index exceeds {what} limit {limit}")]
    Budget {
        member: String,
        what: &'static str,
        limit: usize,
    },
    #[error("{member}:{line}: {detail}")]
    Profile {
        member: String,
        line: usize,
        detail: String,
    },
    #[error("{member}: no [Index] CampaignList")]
    NoCampaignList { member: String },
    #[error("{member}: CampaignList repeats {id}")]
    DuplicateCampaign { member: String, id: String },
    #[error("{member}: CampaignList names a missing or empty section {id}")]
    UnknownSection { member: String, id: String },
    #[error("{member}: section {id} Mission{index} is missing or not contiguous")]
    MissionGap {
        member: String,
        id: String,
        index: u32,
    },
    #[error("{member}: section {id} repeats Mission{index}")]
    DuplicateMission {
        member: String,
        id: String,
        index: u32,
    },
    #[error("{member}: section {id} Mission{index} has {fields} fields; expected at least 3")]
    MissionShape {
        member: String,
        id: String,
        index: u32,
        fields: usize,
    },
    #[error("{member}: section {id} Mission{index} has an empty map filename")]
    EmptyMap {
        member: String,
        id: String,
        index: u32,
    },
}

/// Parse the selected member's bytes. `member` is recorded and not interpreted.
pub fn parse_campaign_index(
    member: &str,
    bytes: &[u8],
    limits: IndexLimits,
) -> Result<CampaignIndex, IndexError> {
    if member.is_empty() {
        return Err(IndexError::Profile {
            member: member.into(),
            line: 0,
            detail: "selected member name is empty".into(),
        });
    }
    if bytes.len() > limits.max_bytes {
        return Err(IndexError::Budget {
            member: member.into(),
            what: "bytes",
            limit: limits.max_bytes,
        });
    }
    let parsed =
        defaults::parse_profile(bytes, member).map_err(|error| profile_error(member, &error))?;
    if let Some(error) = parsed.unresolved().first() {
        return Err(profile_error(member, error));
    }
    let list = parsed.raw_fields("index", "campaignlist");
    if list.is_empty() {
        return Err(IndexError::NoCampaignList {
            member: member.into(),
        });
    }
    if list.len() != 1 {
        return Err(IndexError::Profile {
            member: member.into(),
            line: list[1].0,
            detail: "duplicate CampaignList".into(),
        });
    }
    let (line, raw) = list[0];
    let mut seen = std::collections::BTreeSet::new();
    let mut slots = Vec::new();
    for (slot, id) in defaults::profile_elements_iter(raw).enumerate() {
        if slot >= limits.max_campaign_slots {
            return Err(IndexError::Budget {
                member: member.into(),
                what: "campaign slots",
                limit: limits.max_campaign_slots,
            });
        }
        let id = id.map_err(|detail| IndexError::Profile {
            member: member.into(),
            line,
            detail,
        })?;
        slots.try_reserve(1).map_err(|_| IndexError::Allocation {
            member: member.into(),
            what: "campaign slots",
        })?;
        if id.is_empty() {
            slots.push(CampaignSlot::Blank);
            continue;
        }
        let key = id.to_ascii_lowercase();
        if !seen.insert(key.clone()) {
            return Err(IndexError::DuplicateCampaign {
                member: member.into(),
                id: id.to_owned(),
            });
        }
        if !parsed.has_section(&key) {
            return Err(IndexError::UnknownSection {
                member: member.into(),
                id: id.to_owned(),
            });
        }
        let missions = missions(member, id, &parsed, limits)?;
        let first = |field: &str| {
            parsed
                .raw_fields(&key, field)
                .first()
                .and_then(|(_, raw)| defaults::profile_elements_iter(raw).next())
                .and_then(Result::ok)
                .map(str::to_owned)
        };
        slots.push(CampaignSlot::Entry(IndexEntry {
            id: id.to_owned(),
            header: first("header"),
            name: first("name"),
            missions,
        }));
    }
    Ok(CampaignIndex {
        member: member.into(),
        slots,
    })
}

fn missions(
    member: &str,
    id: &str,
    parsed: &defaults::ParsedProfile,
    limits: IndexLimits,
) -> Result<Vec<Mission>, IndexError> {
    let section = id.to_ascii_lowercase();
    let keys = parsed.keys(&section).unwrap_or_default();
    let mut numbers = Vec::new();
    for key in keys {
        let Some(suffix) = key.strip_prefix("mission") else {
            continue;
        };
        if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
            let line = parsed
                .raw_fields(&section, key)
                .first()
                .map(|(line, _)| *line)
                .unwrap_or(0);
            return Err(IndexError::Profile {
                member: member.into(),
                line,
                detail: format!("mission key {key} has no decimal index"),
            });
        }
        if suffix.len() > 1 && suffix.starts_with('0') {
            let line = parsed.raw_fields(&section, key)[0].0;
            return Err(IndexError::Profile {
                member: member.into(),
                line,
                detail: format!("mission key {key} is not canonical"),
            });
        }
        let index = suffix.parse::<u32>().map_err(|_| IndexError::Profile {
            member: member.into(),
            line: parsed.raw_fields(&section, key)[0].0,
            detail: format!("mission key {key} does not fit u32"),
        })?;
        let fields = parsed.raw_fields(&section, key);
        if fields.len() != 1 {
            return Err(IndexError::DuplicateMission {
                member: member.into(),
                id: id.to_owned(),
                index,
            });
        }
        if numbers.len() >= limits.max_missions_per_campaign {
            return Err(IndexError::Budget {
                member: member.into(),
                what: "missions",
                limit: limits.max_missions_per_campaign,
            });
        }
        numbers.try_reserve(1).map_err(|_| IndexError::Allocation {
            member: member.into(),
            what: "mission indices",
        })?;
        numbers.push(index);
    }
    numbers.sort_unstable();
    let mut missions = Vec::new();
    missions
        .try_reserve_exact(numbers.len())
        .map_err(|_| IndexError::Allocation {
            member: member.into(),
            what: "missions",
        })?;
    for (position, index) in numbers.iter().copied().enumerate() {
        let expected = u32::try_from(position).map_err(|_| IndexError::Budget {
            member: member.into(),
            what: "mission index",
            limit: u32::MAX as usize,
        })?;
        if index != expected {
            return Err(IndexError::MissionGap {
                member: member.into(),
                id: id.to_owned(),
                index: expected,
            });
        }
        let (line, raw) = parsed.raw_fields(&section, &format!("mission{index}"))[0];
        let fields = defaults::profile_elements(raw).map_err(|detail| IndexError::Profile {
            member: member.into(),
            line,
            detail,
        })?;
        if fields.len() < 3 {
            return Err(IndexError::MissionShape {
                member: member.into(),
                id: id.to_owned(),
                index,
                fields: fields.len(),
            });
        }
        if fields[2].is_empty() {
            return Err(IndexError::EmptyMap {
                member: member.into(),
                id: id.to_owned(),
                index,
            });
        }
        missions.push(Mission {
            index,
            header: fields[0].to_owned(),
            name: fields[1].to_owned(),
            map_file: fields[2].to_owned(),
            uninterpreted: fields[3..]
                .iter()
                .map(|field| (*field).to_owned())
                .collect(),
        });
    }
    if missions.is_empty() {
        return Err(IndexError::MissionGap {
            member: member.into(),
            id: id.to_owned(),
            index: 0,
        });
    }
    Ok(missions)
}

fn profile_error(member: &str, error: &Unresolved) -> IndexError {
    match error {
        Unresolved::ProfileLine { line, detail, .. } => IndexError::Profile {
            member: member.into(),
            line: *line,
            detail: detail.clone(),
        },
        other => IndexError::Profile {
            member: member.into(),
            line: 0,
            detail: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> IndexLimits {
        IndexLimits {
            max_bytes: 4096,
            max_campaign_slots: 8,
            max_missions_per_campaign: 4,
        }
    }

    fn sample() -> &'static str {
        r#"[Index]
// order is the selection screen
CampaignList="Alpha","Beta"
[Alpha]
Header="Alpha Campaign"
Name="Rise, then fall"
DefaultOpen=0
Mission0="H","Name, with comma","Maps\Campaign\Alpha00.w3m"
Mission1="H2","N2","Maps\Campaign\Alpha01.w3m"
[Beta]
DefaultOpen=1
Mission0="H","N","Maps\Campaign\Beta00.w3m"
"#
    }

    #[test]
    fn first_index_entry_ignores_default_open_and_keeps_quoted_commas() {
        let index =
            parse_campaign_index("UI\\CampaignStrings.txt", sample().as_bytes(), limits()).unwrap();
        let first = index.first_index_entry().unwrap();
        assert_eq!(first.id, "Alpha");
        assert_eq!(first.header.as_deref(), Some("Alpha Campaign"));
        assert_eq!(first.name.as_deref(), Some("Rise, then fall"));
        assert_eq!(first.missions[0].name, "Name, with comma");
        assert_eq!(first.missions[0].map_file, "Maps\\Campaign\\Alpha00.w3m");
        assert!(first.missions[0].uninterpreted.is_empty());
        match &index.slots[1] {
            CampaignSlot::Entry(second) => {
                assert_eq!(second.id, "Beta");
                assert_eq!(
                    (second.header.as_deref(), second.name.as_deref()),
                    (None, None)
                );
            }
            CampaignSlot::Blank => panic!("second slot dropped"),
        }
    }

    #[test]
    fn blank_slots_are_index_positions_not_playable_campaigns() {
        let text = "[Index]\nCampaignList=\"\",\"\",\"Human\"\n[Human]\nMission0=\"H\",\"N\",\"Maps\\Human01.w3m\"\n";
        let index =
            parse_campaign_index("UI\\CampaignStrings_exp.txt", text.as_bytes(), limits()).unwrap();
        assert!(matches!(index.slots[0], CampaignSlot::Blank));
        assert_eq!(index.first_index_entry().unwrap().id, "Human");
    }

    #[test]
    fn older_filen_table_has_no_index() {
        let text = "[Human]\nEntries=1\nFile0=\"Human01\"\nMission0=\"Title\"\n";
        let error =
            parse_campaign_index("UI\\CampaignStrings.txt", text.as_bytes(), limits()).unwrap_err();
        assert!(matches!(error, IndexError::NoCampaignList { .. }));
    }

    #[test]
    fn malformed_profile_and_mission_shape_fail() {
        let base = limits();
        let bad_quote = "[Index]\nCampaignList=\"One\n[One]\nMission0=\"H\",\"N\",\"M.w3m\"\n";
        assert!(matches!(
            parse_campaign_index("m", bad_quote.as_bytes(), base).unwrap_err(),
            IndexError::Profile { .. }
        ));
        let two = "[Index]\nCampaignList=\"One\"\n[One]\nMission0=\"H\",\"M.w3m\"\n";
        assert!(matches!(
            parse_campaign_index("m", two.as_bytes(), base).unwrap_err(),
            IndexError::MissionShape { fields: 2, .. }
        ));
        let extra =
            "[Index]\nCampaignList=\"One\"\n[One]\nMission0=\"H\",\"N\",\"Maps\\\\M.w3m\",1\n";
        let index = parse_campaign_index("m", extra.as_bytes(), base).unwrap();
        assert_eq!(
            index.first_index_entry().unwrap().missions[0].uninterpreted,
            vec!["1".to_owned()]
        );
        let gap = "[Index]\nCampaignList=\"One\"\n[One]\nMission1=\"H\",\"N\",\"M.w3m\"\n";
        assert!(matches!(
            parse_campaign_index("m", gap.as_bytes(), base).unwrap_err(),
            IndexError::MissionGap { index: 0, .. }
        ));
        let missing = "[Index]\nCampaignList=\"Gone\"\n";
        assert!(matches!(
            parse_campaign_index("m", missing.as_bytes(), base).unwrap_err(),
            IndexError::UnknownSection { .. }
        ));
        let empty_map = "[Index]\nCampaignList=\"One\"\n[One]\nMission0=\"H\",\"N\",\"\"\n";
        assert!(matches!(
            parse_campaign_index("m", empty_map.as_bytes(), base).unwrap_err(),
            IndexError::EmptyMap { .. }
        ));
    }

    #[test]
    fn caller_bounds_reject_extra_bytes_slots_and_missions() {
        let text = sample();
        assert!(matches!(
            parse_campaign_index(
                "m",
                text.as_bytes(),
                IndexLimits {
                    max_bytes: 8,
                    max_campaign_slots: 8,
                    max_missions_per_campaign: 4,
                },
            )
            .unwrap_err(),
            IndexError::Budget { what: "bytes", .. }
        ));
        assert!(matches!(
            parse_campaign_index(
                "m",
                text.as_bytes(),
                IndexLimits {
                    max_bytes: 4096,
                    max_campaign_slots: 1,
                    max_missions_per_campaign: 4,
                },
            )
            .unwrap_err(),
            IndexError::Budget {
                what: "campaign slots",
                ..
            }
        ));
        assert!(matches!(
            parse_campaign_index(
                "m",
                text.as_bytes(),
                IndexLimits {
                    max_bytes: 4096,
                    max_campaign_slots: 8,
                    max_missions_per_campaign: 1,
                },
            )
            .unwrap_err(),
            IndexError::Budget {
                what: "missions",
                ..
            }
        ));
    }

    #[test]
    fn exact_and_zero_limits_do_not_invent_a_first_campaign() {
        let text = sample();
        let exact = IndexLimits {
            max_bytes: text.len(),
            max_campaign_slots: 2,
            max_missions_per_campaign: 2,
        };
        assert_eq!(
            parse_campaign_index("selected:member", text.as_bytes(), exact)
                .unwrap()
                .slots
                .len(),
            2
        );
        for (policy, what) in [
            (
                IndexLimits {
                    max_campaign_slots: 0,
                    ..exact
                },
                "campaign slots",
            ),
            (
                IndexLimits {
                    max_missions_per_campaign: 0,
                    ..exact
                },
                "missions",
            ),
        ] {
            assert!(
                matches!(parse_campaign_index("selected:member",text.as_bytes(),policy),Err(IndexError::Budget {what:actual,..}) if actual==what)
            );
        }
        let empty = parse_campaign_index(
            "selected:empty",
            b"[Index]\nCampaignList=\"\",\"\"\n",
            limits(),
        )
        .unwrap();
        assert_eq!(empty.slots, vec![CampaignSlot::Blank, CampaignSlot::Blank]);
        assert!(empty.first_index_entry().is_none());
    }

    #[test]
    fn duplicates_sparse_indices_and_overflow_keep_the_selected_member_context() {
        let text = "[Index]\nCampaignList=One\n[One]\nMission0=H,N,Maps\\One.w3m\n";
        for (bytes, expected) in [
            (
                text.replace("CampaignList=One", "CampaignList=One,ONE"),
                "repeats ONE",
            ),
            (
                text.replace("[One]", "[One]\nMission0=Other,Name,Other.w3m"),
                "repeats Mission0",
            ),
            (
                text.replace("Mission0", "Mission4294967295"),
                "Mission0 is missing",
            ),
            (
                text.replace("Mission0", "Mission4294967296"),
                "does not fit u32",
            ),
            (text.replace("Mission0", "Mission00"), "not canonical"),
        ] {
            let error =
                parse_campaign_index("archive::selected-member", bytes.as_bytes(), limits())
                    .unwrap_err()
                    .to_string();
            assert!(error.starts_with("archive::selected-member"), "{error}");
            assert!(error.contains(expected), "{error}");
        }
        let repeated = text.replace("[Index]", "[Index]\nCampaignList=One");
        assert!(
            matches!(parse_campaign_index("selected",repeated.as_bytes(),limits()),Err(IndexError::Profile {detail,..}) if detail=="duplicate CampaignList")
        );
    }

    #[test]
    fn mission_rows_keep_raw_quote_boundaries_and_uninterpreted_fields() {
        let text = "[Index]\nCampaignList=One\n[One]\nMission0=\"Header, with comma\",\"Chapter β\",\"Maps\\A,B.w3m\",7,\"\",unknown\n";
        let result = parse_campaign_index("selected", text.as_bytes(), limits()).unwrap();
        let mission = &result.first_index_entry().unwrap().missions[0];
        assert_eq!(mission.header, "Header, with comma");
        assert_eq!(mission.name, "Chapter β");
        assert_eq!(mission.map_file, r"Maps\A,B.w3m");
        assert_eq!(mission.uninterpreted, ["7", "", "unknown"]);
        let one_field = "[Index]\nCampaignList=One\n[One]\nMission0=\"H,N,Maps\\One.w3m\"\n";
        assert!(matches!(
            parse_campaign_index("selected", one_field.as_bytes(), limits()),
            Err(IndexError::MissionShape { fields: 1, .. })
        ));
    }
}
