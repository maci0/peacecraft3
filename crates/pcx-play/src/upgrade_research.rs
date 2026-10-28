//! Source-bound counters for upgrade IDs, excluding unit/item technology aliases.
use crate::{
    neutral_player::{RuleInputBudget, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};
use pcx_objects::editor::{
    FieldKind, FieldSource, Overlay, SlkId,
    defaults::{self, DefaultTables, Family},
};

#[derive(Clone, Copy)]
pub struct Budget {
    pub upgrades: usize,
    pub modifications_per_upgrade: usize,
}

/// Research cost columns of one upgrade. Each pair is `(base, modifier)`: the value for level
/// `n` is `base + modifier * (n - 1)`, the arithmetic of the executable's cost routines
/// 0x6df7e0 (gold) and 0x6e0a30 (lumber). The same shape for the time columns is an inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Prices {
    gold: (i32, i32),
    lumber: (i32, i32),
    seconds: (i32, i32),
}

impl Prices {
    fn at(pair: (i32, i32), level: i32) -> Option<i32> {
        pair.1
            .checked_mul(level.checked_sub(1)?)?
            .checked_add(pair.0)
    }
    pub(crate) fn gold(&self, level: i32) -> Option<i32> {
        Self::at(self.gold, level)
    }
    pub(crate) fn lumber(&self, level: i32) -> Option<i32> {
        Self::at(self.lumber, level)
    }
    pub(crate) fn seconds(&self, level: i32) -> Option<i32> {
        Self::at(self.seconds, level)
    }
}

/// What researching an upgrade costs and needs, by upgrade id.
#[derive(Clone, Debug)]
pub(crate) struct Research {
    prices: Prices,
    /// Required technologies for level `n + 1` at index `n`: the profile key `Requires` for
    /// level 1 and `Requires<n>` for the next levels. That key-to-level mapping is a reading of
    /// the stock rows (Unholy Strength needs `unp1` and `unp2` only at levels 2 and 3), not
    /// decoded from the executable.
    requires: Vec<Vec<[u8; 4]>>,
}

impl Research {
    pub(crate) fn prices(&self) -> &Prices {
        &self.prices
    }
    pub(crate) fn requires(&self, level: i32) -> &[[u8; 4]] {
        usize::try_from(level.saturating_sub(1))
            .ok()
            .and_then(|at| self.requires.get(at))
            .map_or(&[], Vec::as_slice)
    }
}

/// Research data by upgrade id; one unreadable row fails that upgrade only.
type ResearchTable = Result<Vec<(i32, Result<Research, String>)>, String>;

#[derive(Clone, Debug)]
pub struct Rules {
    /// Failure to read the prices does not invalidate the counters; a research order refuses
    /// with this reason.
    research: ResearchTable,
    pub(crate) catalog: defaults::Identity,
    origin: AssetIdentity,
    maximum: Vec<(i32, i32)>,
    classes: Result<Vec<(i32, String)>, String>,
    /// Failure to read the effects does not invalidate the counters; a level change that
    /// needs them refuses with this reason.
    effects: Result<crate::upgrade_effects::Table, String>,
    pub(crate) initial: i32,
    threshold: u32,
}

impl Rules {
    /// Fresh counters begin with the decoded empty sum; map/config setters run
    /// afterward. This is not an observation provider for an already running game.
    pub fn read(
        image: &[u8],
        origin: AssetIdentity,
        tables: &DefaultTables,
        overlay: &Overlay,
        catalog: &dyn pcx_sim::world::ObjectCatalog,
        budget: Budget,
    ) -> Result<Self, String> {
        let read = || -> Result<Self, String> {
            let input_budget = RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 128 << 10,
                sections: 96,
            };
            if [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
            {
                return Err("missing upgrade-counter source identity".into());
            }
            if image.len() > input_budget.image_bytes
                || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
            {
                return Err("unsupported upgrade-counter client".into());
            }
            let pe = Pe::parse(image, input_budget).map_err(|e| e.to_string())?;
            let getter = pe.read_from(0x494c20, true).map_err(|e| e.to_string())?;
            let count = pe.read_from(0x513620, true).map_err(|e| e.to_string())?;
            // The reviewed getter pushes the minimum, and the count helper clears
            // its accumulator before looking up a record or related technology.
            if getter.get(25) != Some(&0x6a) || count.get(20..22) != Some(&[0x33, 0xf6][..]) {
                return Err("unsupported upgrade-counter profile".into());
            }
            let threshold = u32::from(*getter.get(26).ok_or("truncated research threshold")?);
            if threshold == 0 {
                return Err("unsupported zero research threshold".into());
            }
            let maximum = read_maximum(tables, overlay, catalog, budget)?;
            let ids: Vec<i32> = maximum.iter().map(|entry| entry.0).collect();
            let effects = crate::upgrade_effects::verify_profile(&pe)
                .and_then(|()| crate::upgrade_effects::Table::read(tables, overlay, &ids));
            Ok(Self {
                catalog: tables.identity().clone(),
                origin: origin.clone(),
                classes: read_classes(tables, overlay, &maximum),
                research: read_research(tables, overlay, &maximum),
                effects,
                maximum,
                initial: 0,
                threshold,
            })
        };
        read().map_err(|e| format!("{}: upgrade research: {e}", origin.source))
    }

    pub fn origin(&self) -> &AssetIdentity {
        &self.origin
    }
    pub fn maximum(&self, techid: i32) -> Option<i32> {
        self.maximum
            .binary_search_by_key(&techid, |entry| entry.0)
            .ok()
            .map(|at| self.maximum[at].1)
    }
    /// Prices and requirements of an upgrade known to the counters.
    pub(crate) fn research(&self, techid: i32) -> Result<&Research, String> {
        let table = self.research.as_ref().map_err(Clone::clone)?;
        let at = table
            .binary_search_by_key(&techid, |entry| entry.0)
            .map_err(|_| format!("research: unknown upgrade identifier {techid}"))?;
        table[at].1.as_ref().map_err(Clone::clone)
    }
    pub(crate) fn class(&self, techid: i32) -> Result<&str, String> {
        let classes = self.classes.as_ref().map_err(Clone::clone)?;
        let at = classes
            .binary_search_by_key(&techid, |entry| entry.0)
            .map_err(|_| format!("upgrade presentation: unknown identifier {techid}"))?;
        Ok(&classes[at].1)
    }
    pub(crate) fn effects(&self) -> Result<&crate::upgrade_effects::Table, String> {
        self.effects
            .as_ref()
            .map_err(|reason| format!("upgrade effects: {reason}"))
    }
    pub(crate) fn researched(&self, level: i32) -> bool {
        level as u32 >= self.threshold
    }
    pub(crate) fn target(&self, techid: i32, current: i32, requested: i32) -> Option<i32> {
        let maximum = self.maximum(techid)?;
        let delta = requested.wrapping_sub(current);
        Some(if delta >= 0 {
            if current < maximum {
                current.saturating_add(delta).min(maximum)
            } else {
                current
            }
        } else {
            let remove = delta.wrapping_neg();
            if remove > 0 {
                current.saturating_sub(remove).max(self.initial)
            } else {
                current
            }
        })
    }
}

const PRICE_COLUMNS: [&str; 6] = [
    "goldbase",
    "goldmod",
    "lumberbase",
    "lumbermod",
    "timebase",
    "timemod",
];

fn read_research(
    tables: &DefaultTables,
    overlay: &Overlay,
    maximum: &[(i32, i32)],
) -> ResearchTable {
    let column_fourcc = |name: &str| -> Result<[u8; 4], String> {
        let mut fields = tables.editor().fields().iter().filter(|f| {
            f.from == SlkId::UpgradeMetaData
                && matches!(&f.source,
                FieldSource::Column { slk: SlkId::UpgradeData, column } if column == name)
        });
        let field = fields
            .next()
            .ok_or_else(|| format!("research: missing {name} metadata"))?;
        if fields.next().is_some() || field.kind != FieldKind::Int || field.index != -1 {
            return Err(format!(
                "research: ambiguous or unsupported {name} metadata"
            ));
        }
        Ok(field.fourcc)
    };
    let fourccs = PRICE_COLUMNS.map(column_fourcc);
    let mut price_fields = [[0u8; 4]; 6];
    for (slot, found) in price_fields.iter_mut().zip(fourccs) {
        *slot = found?;
    }
    let requirement_fields: Vec<[u8; 4]> = tables
        .editor()
        .fields()
        .iter()
        .filter(|f| {
            f.from == SlkId::UpgradeMetaData
                && matches!(&f.source,
                FieldSource::Profile { key, .. } if key.to_ascii_lowercase().starts_with("requires"))
        })
        .map(|f| f.fourcc)
        .collect();
    // A row that cannot be read fails its own upgrade only.
    let one = |id: i32, levels: i32| -> Result<Research, String> {
        let bytes = id.to_be_bytes();
        let alias = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        let (base, mods) = overlay
            .new_object(alias)
            .unwrap_or((alias, overlay.on(alias)));
        let object = tables
            .object(Family::Upgrade, base)
            .map_err(|e| format!("{alias}: {e}"))?;
        let mut values = [0i32; 6];
        for ((value, name), fourcc) in values.iter_mut().zip(PRICE_COLUMNS).zip(price_fields) {
            *value = if let Some(edit) = mods.iter().rev().find(|m| m.field == fourcc) {
                match edit.value {
                    pcx_objects::Value::Int(v)
                        if edit.value_type == pcx_objects::ValueType::Int
                            && edit.selectors.is_none_or(|s| s == [0, 0]) =>
                    {
                        v as i32
                    }
                    _ => return Err(format!("{alias}: unsupported {name} edit")),
                }
            } else {
                let cell = object
                    .cell(SlkId::UpgradeData, name)
                    .map_err(|e| format!("{alias}: {e}"))?;
                cell.text
                    .trim()
                    .parse::<i32>()
                    .map_err(|_| format!("{alias}: {name} is not an integer"))?
            };
        }
        let [gold, gold_mod, lumber, lumber_mod, time, time_mod] = values;
        if mods.iter().any(|m| requirement_fields.contains(&m.field)) {
            return Err(format!(
                "{alias}: map edits to research requirements are not read"
            ));
        }
        let mut requires = Vec::new();
        for level in 1..=levels.max(0) {
            let key = if level == 1 {
                "Requires".to_owned()
            } else {
                format!("Requires{}", level - 1)
            };
            let cells = object.profile(&key);
            let text = match cells.as_slice() {
                [] => "",
                [one] if one.parse_error.is_none() => one.text,
                _ => return Err(format!("{alias}: {key} is malformed or assigned twice")),
            };
            let amount_key = format!("Requiresamount{}", key.trim_start_matches("Requires"));
            if object
                .profile(&amount_key)
                .iter()
                .any(|cell| !cell.text.trim().is_empty() || cell.parse_error.is_some())
            {
                return Err(format!("{alias}: {amount_key} amounts are not read"));
            }
            let mut codes = Vec::new();
            for token in text.split(',').map(str::trim) {
                if matches!(token, "" | "_" | "-") {
                    continue;
                }
                let code: [u8; 4] = token
                    .as_bytes()
                    .try_into()
                    .ok()
                    .filter(|code: &[u8; 4]| code.iter().all(u8::is_ascii_graphic))
                    .ok_or_else(|| format!("{alias}: {key} has a malformed rawcode"))?;
                codes.push(code);
            }
            requires.push(codes);
        }
        Ok(Research {
            prices: Prices {
                gold: (gold, gold_mod),
                lumber: (lumber, lumber_mod),
                seconds: (time, time_mod),
            },
            requires,
        })
    };
    Ok(maximum
        .iter()
        .map(|&(id, levels)| (id, one(id, levels)))
        .collect())
}

// Presentation failures must not invalidate otherwise valid research counters.
fn read_classes(
    tables: &DefaultTables,
    overlay: &Overlay,
    maximum: &[(i32, i32)],
) -> Result<Vec<(i32, String)>, String> {
    let mut fields = tables.editor().fields().iter().filter(|f| {
        f.from == SlkId::UpgradeMetaData
            && matches!(&f.source,
            FieldSource::Column { slk: SlkId::UpgradeData, column } if column == "class")
    });
    let field = fields
        .next()
        .ok_or("upgrade presentation: missing class metadata")?;
    if fields.next().is_some() || field.kind != FieldKind::Token || field.index != -1 {
        return Err("upgrade presentation: ambiguous or unsupported class metadata".into());
    }
    maximum
        .iter()
        .map(|&(id, _)| {
            let bytes = id.to_be_bytes();
            let alias = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
            let (base, mods) = overlay
                .new_object(alias)
                .unwrap_or((alias, overlay.on(alias)));
            let value = if let Some(edit) = mods.iter().rev().find(|m| m.field == field.fourcc) {
                if edit.selectors.is_some_and(|s| s != [0, 0])
                    || edit.value_type != pcx_objects::ValueType::Str
                {
                    return Err(format!("{alias}: unsupported upgrade class edit"));
                }
                let pcx_objects::Value::Str(bytes) = &edit.value else {
                    return Err(format!("{alias}: nonstring upgrade class"));
                };
                std::str::from_utf8(bytes)
                    .map_err(|e| e.to_string())?
                    .to_string()
            } else {
                let object = tables
                    .object(Family::Upgrade, base)
                    .map_err(|e| e.to_string())?;
                let cell = object
                    .cell(SlkId::UpgradeData, "class")
                    .map_err(|e| e.to_string())?;
                if cell.parse_error.is_some() {
                    return Err(format!("{alias}: malformed upgrade class"));
                }
                cell.text.to_string()
            };
            if value.len() > 4096 || value.contains('\0') {
                return Err(format!("{alias}: invalid upgrade class"));
            }
            Ok((id, value))
        })
        .collect()
}

fn read_maximum(
    tables: &DefaultTables,
    overlay: &Overlay,
    catalog: &dyn pcx_sim::world::ObjectCatalog,
    budget: Budget,
) -> Result<Vec<(i32, i32)>, String> {
    if overlay.mods.len().saturating_add(overlay.custom.len()) > budget.upgrades
        || overlay
            .mods
            .values()
            .chain(overlay.custom.values().map(|(_, mods)| mods))
            .any(|mods| mods.len() > budget.modifications_per_upgrade)
    {
        return Err("upgrade overlay budget".into());
    }
    let mut fields = tables.editor().fields().iter().filter(|f| f.from == SlkId::UpgradeMetaData
        && matches!(&f.source, FieldSource::Column { slk: SlkId::UpgradeData, column } if column == "maxlevel"));
    let field = fields
        .next()
        .ok_or("missing upgrade level-count metadata")?;
    if fields.next().is_some() || field.kind != FieldKind::Int || field.index != -1 {
        return Err("ambiguous or unsupported upgrade level-count metadata".into());
    }
    for alias in overlay.mods.keys() {
        tables
            .object(Family::Upgrade, alias)
            .map_err(|e| e.to_string())?;
    }
    let mut maximum = Vec::new();
    for alias in tables
        .rawcodes(Family::Upgrade)
        .chain(overlay.custom.keys().map(String::as_str))
    {
        if maximum.len() >= budget.upgrades {
            return Err("upgrade catalog budget".into());
        }
        let code: [u8; 4] = alias
            .as_bytes()
            .try_into()
            .map_err(|_| format!("invalid upgrade id {alias:?}"))?;
        if !code.iter().all(u8::is_ascii_graphic)
            || tables.object(Family::Unit, alias).is_ok()
            || tables.object(Family::Item, alias).is_ok()
        {
            return Err(format!("{alias}: upgrade/unit/item technology collision"));
        }
        match catalog.resolve(code) {
            Ok(_) => return Err(format!("{alias}: upgrade/unit/item technology collision")),
            Err(
                pcx_sim::world::ResolutionProblem::MissingRawcode(_)
                | pcx_sim::world::ResolutionProblem::UnsupportedKind(
                    pcx_objects::editor::ObjectKind::Upgrade,
                ),
            ) => {}
            Err(error) => {
                return Err(format!(
                    "{alias}: cannot qualify technology category: {error:?}"
                ));
            }
        }
        let (base, mods) = overlay
            .new_object(alias)
            .unwrap_or((alias, overlay.on(alias)));
        if overlay.new_object(base).is_some() {
            return Err(format!("{alias}: custom upgrade base chain"));
        }
        let object = tables
            .object(Family::Upgrade, base)
            .map_err(|e| e.to_string())?;
        let cap = if let Some(edit) = mods.iter().rev().find(|m| m.field == field.fourcc) {
            if edit.selectors.is_some_and(|s| s != [0, 0])
                || edit.value_type != pcx_objects::ValueType::Int
            {
                return Err(format!("{alias}: unsupported level-count edit"));
            }
            let pcx_objects::Value::Int(value) = edit.value else {
                return Err(format!("{alias}: noninteger level count"));
            };
            value as i32
        } else {
            match object
                .level_count()
                .map_err(|e| e.to_string())?
                .typed(FieldKind::Int)
                .map_err(|e| e.to_string())?
            {
                defaults::Value::Integer(value) => {
                    i32::try_from(value).map_err(|_| format!("{alias}: level count range"))?
                }
                _ => return Err(format!("{alias}: unsupported level count")),
            }
        };
        if cap < 0 {
            return Err(format!("{alias}: negative level count"));
        }
        maximum
            .try_reserve(1)
            .map_err(|_| "upgrade catalog allocation")?;
        maximum.push((i32::from_be_bytes(code), cap));
    }
    maximum.sort_unstable_by_key(|entry| entry.0);
    if maximum.is_empty() || maximum.windows(2).any(|p| p[0].0 == p[1].0) {
        return Err("empty or duplicate upgrade catalog".into());
    }
    Ok(maximum)
}

#[cfg(any(test, feature = "test-fixtures"))]
const FIXTURE_EFFECT_LEVELS: i32 = 4;

#[cfg(any(test, feature = "test-fixtures"))]
pub fn fixture(catalog: &AssetIdentity) -> Rules {
    fixture_with_effects(catalog, crate::upgrade_effects::Table::default())
}

/// One synthetic researchable upgrade: id, level count, `[gold base, gold mod, lumber base,
/// lumber mod, seconds base, seconds mod]` and the required technologies per level.
#[cfg(any(test, feature = "test-fixtures"))]
pub type FixtureResearch = (i32, i32, [i32; 6], Vec<Vec<[u8; 4]>>);

#[cfg(any(test, feature = "test-fixtures"))]
pub fn fixture_with_research(catalog: &AssetIdentity, upgrades: Vec<FixtureResearch>) -> Rules {
    let mut rules = fixture(catalog);
    let mut research = Vec::new();
    for (id, levels, [gold, gold_mod, lumber, lumber_mod, time, time_mod], requires) in upgrades {
        rules.maximum.retain(|entry| entry.0 != id);
        rules.maximum.push((id, levels));
        research.push((
            id,
            Ok(Research {
                prices: Prices {
                    gold: (gold, gold_mod),
                    lumber: (lumber, lumber_mod),
                    seconds: (time, time_mod),
                },
                requires,
            }),
        ));
    }
    research.sort_unstable_by_key(|entry| entry.0);
    rules.maximum.sort_unstable_by_key(|entry| entry.0);
    rules.research = Ok(research);
    rules
}

#[cfg(any(test, feature = "test-fixtures"))]
pub fn fixture_with_effects(
    catalog: &AssetIdentity,
    effects: crate::upgrade_effects::Table,
) -> Rules {
    // Every upgrade with effects is researchable up to a synthetic level cap.
    let mut maximum = vec![(-37, 2), (5, 4), (i32::from_be_bytes(*b"Qtec"), 1)];
    maximum.extend(
        effects
            .upgrade_ids()
            .filter(|id| maximum.iter().all(|entry| entry.0 != *id))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|id| (id, FIXTURE_EFFECT_LEVELS)),
    );
    maximum.sort_unstable_by_key(|entry| entry.0);
    Rules {
        catalog: defaults::Identity {
            install: catalog.source.clone(),
            edition_version: format!("{}; {}", catalog.edition, catalog.version),
        },
        origin: AssetIdentity {
            source: "synthetic upgrade counter profile".into(),
            edition: "synthetic".into(),
            version: "synthetic".into(),
        },
        classes: Err("synthetic research fixture has no presentation classes".into()),
        research: Ok(Vec::new()),
        effects: Ok(effects),
        maximum,
        initial: 0,
        threshold: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoUnits;
    impl pcx_sim::world::ObjectCatalog for NoUnits {
        fn resolve(
            &self,
            id: pcx_objects::Fourcc,
        ) -> Result<pcx_sim::world::Definition, pcx_sim::world::ResolutionProblem> {
            Err(pcx_sim::world::ResolutionProblem::MissingRawcode(
                pcx_objects::fourcc_text(id),
            ))
        }
    }
    fn table(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
        let mut text = String::from("ID;PWXL;N;E\r\n");
        for (y, row) in std::iter::once(headers)
            .chain(rows.iter().copied())
            .enumerate()
        {
            for (x, value) in row.iter().enumerate() {
                text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
            }
        }
        text.push_str("E\r\n");
        text.into_bytes()
    }
    #[test]
    fn presentation_classes_keep_stock_custom_and_last_map_override() {
        let tables = tables(false);
        let mut overlay = Overlay::default();
        let edit = |value: &str| pcx_objects::Mod {
            field: *b"QCLS",
            value_type: pcx_objects::ValueType::Str,
            selectors: None,
            value: pcx_objects::Value::Str(value.as_bytes().to_vec()),
            end_word: 0,
        };
        overlay
            .mods
            .insert("R001".into(), vec![edit("old"), edit("new")]);
        overlay
            .custom
            .insert("Z903".into(), ("R001".into(), vec![edit("CustomGuard")]));
        let ids = [
            (i32::from_be_bytes(*b"R001"), 2),
            (i32::from_be_bytes(*b"R002"), 0),
            (i32::from_be_bytes(*b"Z903"), 2),
        ];
        let classes = read_classes(&tables, &overlay, &ids).unwrap();
        assert_eq!(
            classes
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            ["new", "_", "CustomGuard"]
        );
        overlay.mods.get_mut("R001").unwrap()[1].selectors = Some([9, 1]);
        assert!(read_classes(&tables, &overlay, &ids).is_err());
        // Class presentation is independent of the counter's valid maximum data.
        assert!(
            read_maximum(
                &tables,
                &overlay,
                &NoUnits,
                Budget {
                    upgrades: 8,
                    modifications_per_upgrade: 8
                }
            )
            .is_ok()
        );
    }
    fn tables(unit_collision: bool) -> DefaultTables {
        let data = table(
            &["upgradeid", "maxlevel", "class"],
            &[&["R001", "2", "SyntheticProtection"], &["R002", "0", "_"]],
        );
        let meta = table(
            &["ID", "field", "slk", "type", "index", "category"],
            &[
                &["QMAX", "maxlevel", "UpgradeData", "int", "-1", "tech"],
                &["QCLS", "class", "UpgradeData", "token", "-1", "tech"],
            ],
        );
        let units = table(&["unitID"], if unit_collision { &[&["R001"]] } else { &[] });
        DefaultTables::from_bytes(
            defaults::Identity {
                install: "synthetic catalog".into(),
                edition_version: "synthetic version".into(),
            },
            &[
                (SlkId::UpgradeData.path(), &data),
                (SlkId::UpgradeMetaData.path(), &meta),
                (SlkId::UnitData.path(), &units),
            ],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 16,
                max_asset_bytes: 4096,
                max_aggregate_input_bytes: 16384,
            },
        )
        .unwrap()
    }
    #[test]
    fn caps_follow_stock_and_map_metadata_and_refuse_unsupported_inputs() {
        let tables = tables(false);
        let mut overlay = Overlay::default();
        let edit = |value| pcx_objects::Mod {
            field: *b"QMAX",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([0, 0]),
            value: pcx_objects::Value::Int(value),
            end_word: 0,
        };
        overlay.mods.insert("R001".into(), vec![edit(5)]);
        overlay
            .custom
            .insert("R003".into(), ("R001".into(), vec![edit(9)]));
        let budget = Budget {
            upgrades: 4,
            modifications_per_upgrade: 2,
        };
        assert_eq!(
            read_maximum(&tables, &overlay, &NoUnits, budget).unwrap(),
            vec![
                (i32::from_be_bytes(*b"R001"), 5),
                (i32::from_be_bytes(*b"R002"), 0),
                (i32::from_be_bytes(*b"R003"), 9)
            ]
        );
        assert!(
            read_maximum(
                &tables,
                &overlay,
                &NoUnits,
                Budget {
                    upgrades: 1,
                    ..budget
                }
            )
            .is_err()
        );
        overlay.mods.get_mut("R001").unwrap()[0].selectors = Some([1, 0]);
        assert!(read_maximum(&tables, &overlay, &NoUnits, budget).is_err());
        overlay.mods.clear();
        overlay.custom.get_mut("R003").unwrap().0 = "R003".into();
        assert!(read_maximum(&tables, &overlay, &NoUnits, budget).is_err());
        assert!(read_maximum(&self::tables(true), &Overlay::default(), &NoUnits, budget).is_err());
    }
    fn research_tables(profile: &str) -> DefaultTables {
        let data = table(
            &[
                "upgradeid",
                "maxlevel",
                "goldbase",
                "goldmod",
                "lumberbase",
                "lumbermod",
                "timebase",
                "timemod",
            ],
            &[
                &["R001", "3", "100", "25", "40", "10", "60", "15"],
                &["R002", "1", "7", "0", "0", "0", "5", "0"],
            ],
        );
        let meta = table(
            &["ID", "field", "slk", "type", "index", "category"],
            &[
                &["QMAX", "maxlevel", "UpgradeData", "int", "-1", "tech"],
                &["QGLB", "goldbase", "UpgradeData", "int", "-1", "stats"],
                &["QGLM", "goldmod", "UpgradeData", "int", "-1", "stats"],
                &["QLMB", "lumberbase", "UpgradeData", "int", "-1", "stats"],
                &["QLMM", "lumbermod", "UpgradeData", "int", "-1", "stats"],
                &["QTMB", "timebase", "UpgradeData", "int", "-1", "stats"],
                &["QTMM", "timemod", "UpgradeData", "int", "-1", "stats"],
                &["QREQ", "Requires", "Profile", "techList", "-1", "tech"],
            ],
        );
        DefaultTables::from_bytes(
            defaults::Identity {
                install: "synthetic catalog".into(),
                edition_version: "synthetic version".into(),
            },
            &[
                (SlkId::UpgradeData.path(), &data),
                (SlkId::UpgradeMetaData.path(), &meta),
            ],
            &[("Units\\SyntheticUpgradeFunc.txt", profile.as_bytes())],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 16,
                max_asset_bytes: 4096,
                max_aggregate_input_bytes: 16384,
            },
        )
        .unwrap()
    }
    fn int_edit(field: &[u8; 4], value: u32) -> pcx_objects::Mod {
        pcx_objects::Mod {
            field: *field,
            value_type: pcx_objects::ValueType::Int,
            selectors: None,
            value: pcx_objects::Value::Int(value),
            end_word: 0,
        }
    }
    const PROFILE: &str = "[R001]\r\nRequires=\r\nRequires1=a001,b001\r\nRequires2=c001\r\n[R002]\r\nRequires=d001\r\n";
    const MAXIMUM: [(i32, i32); 2] = [
        (i32::from_be_bytes(*b"R001"), 3),
        (i32::from_be_bytes(*b"R002"), 1),
    ];
    fn refused(read: ResearchTable) -> bool {
        match read {
            Ok(rows) => rows.iter().any(|row| row.1.is_err()),
            Err(_) => true,
        }
    }
    #[test]
    fn research_prices_are_base_plus_modifier_per_level_with_map_edits() {
        let tables = research_tables(PROFILE);
        let mut overlay = Overlay::default();
        let first = read_research(&tables, &overlay, &MAXIMUM).unwrap();
        let prices = first[0].1.as_ref().unwrap().prices();
        assert_eq!(
            [prices.gold(1), prices.gold(2), prices.gold(3)],
            [Some(100), Some(125), Some(150)]
        );
        assert_eq!([prices.lumber(1), prices.lumber(3)], [Some(40), Some(60)]);
        assert_eq!([prices.seconds(1), prices.seconds(2)], [Some(60), Some(75)]);
        assert_eq!(prices.gold(0), Some(75), "the formula, not a level rule");
        assert_eq!(prices.gold(i32::MIN), None);
        overlay.mods.insert(
            "R001".into(),
            vec![
                int_edit(b"QGLB", 1),
                int_edit(b"QGLB", 300),
                int_edit(b"QTMM", 1),
            ],
        );
        overlay
            .custom
            .insert("R003".into(), ("R002".into(), vec![int_edit(b"QGLM", 9)]));
        let ids = [MAXIMUM[0], MAXIMUM[1], (i32::from_be_bytes(*b"R003"), 2)];
        let edited = read_research(&tables, &overlay, &ids).unwrap();
        assert_eq!(
            edited[0].1.as_ref().unwrap().prices().gold(2),
            Some(325),
            "last edit wins"
        );
        assert_eq!(edited[0].1.as_ref().unwrap().prices().seconds(3), Some(62));
        assert_eq!(
            edited[2].1.as_ref().unwrap().prices().gold(2),
            Some(16),
            "custom upgrade"
        );
        overlay.mods.get_mut("R001").unwrap()[2].selectors = Some([1, 0]);
        assert!(refused(read_research(&tables, &overlay, &ids)));
    }
    #[test]
    fn research_requirements_follow_the_level_keys_and_refuse_unread_forms() {
        let tables = research_tables(PROFILE);
        let overlay = Overlay::default();
        let research = read_research(&tables, &overlay, &MAXIMUM).unwrap();
        let need = |at: usize, level| research[at].1.as_ref().unwrap().requires(level).to_vec();
        assert!(need(0, 1).is_empty());
        assert_eq!(need(0, 2), [*b"a001", *b"b001"]);
        assert_eq!(need(0, 3), [*b"c001"]);
        assert!(need(0, 4).is_empty() && need(0, 0).is_empty());
        assert_eq!(need(1, 1), [*b"d001"]);
        let mut overlay = Overlay::default();
        overlay
            .mods
            .insert("R002".into(), vec![int_edit(b"QREQ", 1)]);
        assert!(refused(read_research(&tables, &overlay, &MAXIMUM)));
        for bad in [
            "[R001]\r\nRequires=abc\r\n",
            "[R001]\r\nRequires=a001\r\nRequires=b001\r\n",
            "[R001]\r\nRequires1=a001\r\nRequiresamount1=2\r\n",
        ] {
            assert!(
                refused(read_research(
                    &research_tables(bad),
                    &Overlay::default(),
                    &MAXIMUM
                )),
                "{bad}"
            );
        }
        assert!(refused(read_research(
            &self::tables(false),
            &Overlay::default(),
            &MAXIMUM
        )));
    }
    #[test]
    fn target_follows_signed_native_delta_and_cap_without_iterating_level_requests() {
        let rules = super::fixture(&crate::scripts::AssetIdentity {
            source: "catalog".into(),
            edition: "synthetic".into(),
            version: "test".into(),
        });
        assert_eq!(rules.target(5, 0, 3), Some(3));
        assert_eq!(rules.target(5, 3, 100), Some(4));
        assert_eq!(rules.target(5, 3, -3), Some(0));
        assert_eq!(rules.target(5, 0, i32::MIN), Some(0));
        assert_eq!(rules.target(5, 1, i32::MIN), Some(4));
        assert_eq!(rules.target(6, 0, 1), None);
        assert!(!rules.researched(rules.initial));
    }
}
