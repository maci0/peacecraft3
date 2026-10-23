//! Inventory policy from selected ability metadata, map overrides and profiles.
use crate::order_registry::Registry;
use pcx_objects::editor::defaults::{DefaultTables, Family, Value};
use pcx_objects::editor::{FieldKind, Overlay, SlkId};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct InventoryBudget {
    pub max_slots_per_unit: usize,
    pub max_pending_orders: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub slots: usize,
    pub drop_on_death: bool,
    pub use_items: bool,
    pub get_items: bool,
    pub drop_items: bool,
}
#[derive(Clone, Debug)]
pub struct Rules {
    pub tables: Arc<DefaultTables>,
    pub profile_sources: Vec<String>,
    pub order_source: crate::order_registry::Binding,
    overlay: Overlay,
    class: String,
    fields: [[u8; 4]; 5],
    data: [u32; 5],
    maximum: usize,
    pub pickup_range: f32,
    pub drop_range: f32,
    pub give_range: f32,
    pub(crate) smart: i32,
    pub(crate) get: i32,
}
impl Rules {
    pub fn read(
        tables: Arc<DefaultTables>,
        overlay: Overlay,
        registry: Arc<Registry>,
        profiles: &[(&str, &[u8])],
        profile_budget: usize,
    ) -> Result<Self, String> {
        let mut fields = [[0; 4]; 5];
        let mut data = [0; 5];
        let mut classes = Vec::new();
        let mut maximum = None;
        for (index, id) in [*b"inv1", *b"inv2", *b"inv3", *b"inv4", *b"inv5"]
            .into_iter()
            .enumerate()
        {
            let metadata = tables.metadata(id).ok_or("inventory metadata missing")?;
            let text = |key| {
                metadata
                    .attribute(key)
                    .map(|c| c.text.trim().to_owned())
                    .ok_or_else(|| format!("inventory metadata {key} missing"))
            };
            if text("field")? != "Data"
                || text("slk")? != "AbilityData"
                || text("type")? != if index == 0 { "int" } else { "bool" }
            {
                return Err("unsupported inventory metadata mapping".into());
            }
            fields[index] = id;
            data[index] = text("data")?
                .parse()
                .map_err(|_| "invalid inventory data index")?;
            if !(1..=26).contains(&data[index]) {
                return Err("inventory data index outside column alphabet".into());
            }
            if index == 0 {
                maximum = Some(
                    text("maxVal")?
                        .parse::<usize>()
                        .map_err(|_| "invalid inventory capacity bound")?,
                );
                for alias in text("useSpecific")?.split(',') {
                    let class = tables
                        .object(Family::Ability, alias.trim())
                        .map_err(|e| e.to_string())?
                        .cell(SlkId::AbilityData, "code")
                        .map_err(|e| e.to_string())?
                        .text
                        .trim()
                        .to_owned();
                    if !classes.contains(&class) {
                        classes.push(class);
                    }
                }
            }
        }
        let [class] = classes.as_slice() else {
            return Err("inventory ability class missing or ambiguous".into());
        };
        let ([pickup, drop, give], profile_sources) = pcx_objects::editor::misc::misc_scalars(
            profiles,
            ["PickupItemRange", "DropItemRange", "GiveItemRange"],
            profile_budget,
        )?;
        let range = |v: Option<f32>| {
            v.filter(|v| v.is_finite() && *v >= 0.)
                .ok_or_else(|| "missing finite nonnegative inventory range".to_string())
        };
        Ok(Self {
            order_source: registry.binding().clone(),
            tables,
            overlay,
            class: class.clone(),
            fields,
            data,
            maximum: maximum.ok_or("inventory capacity bound missing")?,
            profile_sources,
            pickup_range: range(pickup)?,
            drop_range: range(drop)?,
            give_range: range(give)?,
            smart: registry.id("smart").ok_or("selected smart order missing")?,
            get: registry
                .id("getitem")
                .ok_or("selected getitem order missing")?,
        })
    }
    pub(crate) fn policy(
        &self,
        abilities: impl Iterator<Item = ([u8; 4], i32)>,
        budget: usize,
    ) -> Result<Policy, String> {
        let mut selected = None;
        for (alias, level) in abilities {
            if level <= 0 {
                continue;
            }
            let name =
                std::str::from_utf8(&alias).map_err(|_| "invalid inventory ability alias")?;
            let base = self.overlay.new_object(name).map_or(name, |(base, _)| base);
            let object = self
                .tables
                .object(Family::Ability, base)
                .map_err(|e| e.to_string())?;
            if object
                .cell(SlkId::AbilityData, "code")
                .map_err(|e| e.to_string())?
                .text
                .trim()
                != self.class
            {
                continue;
            }
            if selected.is_some() {
                return Err("multiple active inventory abilities".into());
            }
            let mods = self
                .overlay
                .new_object(name)
                .map(|(_, m)| m)
                .unwrap_or_else(|| self.overlay.on(name));
            let mut values = [0; 5];
            for (index, value) in values.iter_mut().enumerate() {
                if let Some(entry) = mods.iter().rev().find(|m| {
                    m.field == self.fields[index]
                        && m.selectors.is_none_or(|s| s[0] == level as u32)
                }) {
                    if let Some([_, pointer]) = entry.selectors
                        && pointer != self.data[index]
                    {
                        return Err(format!(
                            "inventory override data pointer {pointer} does not match metadata data column {}",
                            self.data[index]
                        ));
                    }
                    let pcx_objects::Value::Int(n) = entry.value else {
                        return Err("noninteger inventory override".into());
                    };
                    *value = i32::try_from(n).map_err(|_| "inventory override exceeds integer")?;
                } else {
                    let column = format!(
                        "Data{}{}",
                        char::from_u32(u32::from(b'A') + self.data[index] - 1)
                            .ok_or("inventory column")?,
                        level
                    );
                    let cell = object
                        .cell(SlkId::AbilityData, &column)
                        .map_err(|e| e.to_string())?;
                    *value = match cell.typed(FieldKind::Int).map_err(|e| e.to_string())? {
                        Value::Integer(n) => {
                            i32::try_from(n).map_err(|_| "inventory value exceeds integer")?
                        }
                        _ => return Err("inventory field is not integral".into()),
                    };
                }
            }
            let slots = usize::try_from(values[0]).map_err(|_| "negative inventory capacity")?;
            if slots > self.maximum || slots > budget {
                return Err("inventory capacity exceeds source bound or caller budget".into());
            }
            if values[1..].iter().any(|v| !matches!(v, 0 | 1)) {
                return Err("inventory policy is not boolean".into());
            }
            selected = Some(Policy {
                slots,
                drop_on_death: values[1] != 0,
                use_items: values[2] != 0,
                get_items: values[3] != 0,
                drop_items: values[4] != 0,
            });
        }
        Ok(selected.unwrap_or(Policy {
            slots: 0,
            drop_on_death: false,
            use_items: false,
            get_items: false,
            drop_items: false,
        }))
    }
}
