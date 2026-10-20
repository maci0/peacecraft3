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

#[derive(Clone, Debug)]
pub struct Rules {
    pub(crate) catalog: defaults::Identity,
    origin: AssetIdentity,
    maximum: Vec<(i32, i32)>,
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
            Ok(Self {
                catalog: tables.identity().clone(),
                origin: origin.clone(),
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

#[cfg(test)]
pub(crate) fn fixture(catalog: &AssetIdentity) -> Rules {
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
        maximum: vec![(-37, 2), (5, 4), (i32::from_be_bytes(*b"Qtec"), 1)],
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
    fn tables(unit_collision: bool) -> DefaultTables {
        let data = table(
            &["upgradeid", "maxlevel"],
            &[&["R001", "2"], &["R002", "0"]],
        );
        let meta = table(
            &["ID", "field", "slk", "type", "index", "category"],
            &[&["QMAX", "maxlevel", "UpgradeData", "int", "-1", "tech"]],
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
