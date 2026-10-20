//! Ground item geometry from the resolved ItemData fields and model asset.
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use pcx_sim::world::{Definition, FieldValue};
use std::{collections::BTreeMap, sync::Arc};

fn field<'a>(definition: &'a Definition, column: &str) -> Result<&'a FieldValue, String> {
    if definition.kind != pcx_objects::editor::ObjectKind::Item {
        return Err("item renderer requires an item definition".into());
    }
    let mut fields = definition.fields.iter().filter(|(_, field)| {
        field.metadata.as_ref().is_some_and(|metadata| matches!(
            &metadata.source, FieldSource::Column { slk: SlkId::ItemData, column: name } if name == column
        ))
    });
    let (id, field) = fields
        .next()
        .ok_or_else(|| format!("missing item field {column}"))?;
    if fields.next().is_some() {
        return Err(format!("ambiguous item field {column}"));
    }
    let metadata = field.metadata.as_ref().expect("selected metadata");
    if metadata.from != SlkId::UnitMetaData
        || metadata.fourcc != *id
        || metadata.index != -1
        || if column == "file" {
            metadata.kind != FieldKind::Token
        } else {
            !matches!(
                metadata.kind,
                FieldKind::Int | FieldKind::Real | FieldKind::Unreal
            )
        }
    {
        return Err(format!("unsupported item field {column} metadata"));
    }
    let value = field
        .value
        .as_ref()
        .map_err(|error| format!("item {column}: {error:?}"))?;
    let valid = match value {
        FieldValue::Cell(_) => true,
        FieldValue::Override(value) => matches!(
            (metadata.kind, value),
            (FieldKind::Int, pcx_objects::Value::Int(_))
                | (FieldKind::Real, pcx_objects::Value::Real(_))
                | (FieldKind::Unreal, pcx_objects::Value::Unreal(_))
                | (FieldKind::Token, pcx_objects::Value::Str(_))
        ),
    };
    if !valid {
        return Err(format!("unsupported item field {column} value"));
    }
    Ok(value)
}

fn number(definition: &Definition, column: &str) -> Result<f32, String> {
    let number = match field(definition, column)? {
        FieldValue::Cell(value) => value
            .trim()
            .parse::<f32>()
            .map_err(|e| format!("item {column}: {e}"))?,
        FieldValue::Override(pcx_objects::Value::Int(value)) => *value as f32,
        FieldValue::Override(
            pcx_objects::Value::Real(value) | pcx_objects::Value::Unreal(value),
        ) => *value,
        _ => return Err(format!("item {column} is not numeric")),
    };
    if !number.is_finite() {
        return Err(format!("nonfinite item {column}"));
    }
    Ok(number)
}

pub(super) fn properties(
    definition: &Definition,
    channel_scale: f32,
) -> Result<(f32, [f32; 3]), String> {
    let scale = number(definition, "scale")?;
    let tint = [
        number(definition, "colorR")?,
        number(definition, "colorG")?,
        number(definition, "colorB")?,
    ]
    .map(|value| value * channel_scale);
    if scale <= 0.0 || !channel_scale.is_finite() || tint.iter().any(|value| !value.is_finite()) {
        return Err("invalid item scale or tint".into());
    }
    Ok((scale, tint))
}

fn path(definition: &Definition) -> Result<String, String> {
    let path = match field(definition, "file")? {
        FieldValue::Cell(text) => text.as_str(),
        FieldValue::Override(pcx_objects::Value::Str(text)) => {
            std::str::from_utf8(text).map_err(|e| e.to_string())?
        }
        _ => return Err("item model path is not text".into()),
    }
    .trim();
    model_member(path)
}

/// Binary `.mdx` member for a script or item `.mdl`/`.mdx` reference.
pub(super) fn model_member(path: &str) -> Result<String, String> {
    if matches!(path, "" | "_" | "-")
        || path.len() > 16 << 10
        || path.chars().any(char::is_control)
        || path.contains(',')
    {
        return Err("missing or unsupported model path".into());
    }
    let path = pcx_mdl::binary_model_path(path);
    let name = path.rsplit(['/', '\\']).next().expect("nonempty path");
    if !name.contains('.') {
        Ok(format!("{path}.mdx"))
    } else if name.to_ascii_lowercase().ends_with(".mdx") {
        Ok(path)
    } else {
        Err("unsupported model extension".into())
    }
}

/// Read one source-identified binary model within `budget` bytes.
pub(super) fn read_model(
    reader: &pcx_assets::Resolver,
    edition: &str,
    member: String,
    budget: usize,
) -> Result<Arc<pcx_play::unit_model::Model>, String> {
    let asset = reader
        .resolve_bounded(&member, budget.min(16 << 20) as u64)
        .map_err(|error| error.to_string())?;
    let origin = pcx_play::scripts::AssetIdentity {
        edition: edition.to_owned(),
        version: asset
            .provenance
            .source
            .version
            .clone()
            .unwrap_or_else(|| "unversioned source".into()),
        source: serde_json::to_string(&asset.provenance).map_err(|error| error.to_string())?,
    };
    if [&origin.edition, &origin.version, &origin.source]
        .iter()
        .any(|value| value.len() > 16 << 10)
    {
        return Err("model provenance budget exceeded".into());
    }
    Ok(Arc::new(pcx_play::unit_model::Model::read(
        origin,
        member,
        asset.bytes.into(),
        budget.min(16 << 20),
    )?))
}

pub(super) fn load(
    reader: &pcx_assets::Resolver,
    definition: &Definition,
    edition: &str,
    now: f64,
    models: &mut BTreeMap<String, Arc<pcx_play::unit_model::Model>>,
    source_bytes: &mut usize,
) -> Result<pcx_play::unit_model::Animation, String> {
    let path = path(definition)?;
    let key = path.replace('\\', "/").to_ascii_lowercase();
    if let Some(model) = models.get(&key) {
        return pcx_play::unit_model::Animation::new(Arc::clone(model), now);
    }
    if models.len() >= 512 {
        return Err("item model count budget exceeded".into());
    }
    let remaining = (256usize << 20)
        .checked_sub(*source_bytes)
        .ok_or("item model byte budget exceeded")?;
    let model = read_model(reader, edition, path, remaining)?;
    let size = model.bytes.len();
    let animation = pcx_play::unit_model::Animation::new(Arc::clone(&model), now)?;
    models.insert(key, model);
    *source_bytes += size;
    Ok(animation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_objects::editor::{FieldDef, ObjectKind};
    use pcx_sim::world::ObjectField;
    fn item() -> Definition {
        let mut definition = Definition {
            rawcode: *b"I001",
            base_rawcode: "I001".into(),
            kind: ObjectKind::Item,
            fields: BTreeMap::new(),
            modifications: Vec::new(),
        };
        for (id, column, value, kind) in [
            (*b"f001", "file", "Models/Synthetic.mdl", FieldKind::Token),
            (*b"f002", "scale", "1.75", FieldKind::Real),
            (*b"f003", "colorR", "20", FieldKind::Int),
            (*b"f004", "colorG", "40", FieldKind::Int),
            (*b"f005", "colorB", "60", FieldKind::Int),
        ] {
            definition.fields.insert(
                id,
                ObjectField {
                    metadata: Some(FieldDef {
                        fourcc: id,
                        from: SlkId::UnitMetaData,
                        field: column.into(),
                        slk: "ItemData".into(),
                        index: -1,
                        declared: if kind == FieldKind::Token {
                            "model"
                        } else {
                            "real"
                        }
                        .into(),
                        kind,
                        category: "art".into(),
                        source: FieldSource::Column {
                            slk: SlkId::ItemData,
                            column: column.into(),
                        },
                        measured: true,
                    }),
                    value: Ok(FieldValue::Cell(value.into())),
                },
            );
        }
        definition
    }
    #[test]
    fn source_item_properties_and_map_model_override_are_preserved() {
        let mut item = item();
        assert_eq!(
            properties(&item, 0.01).unwrap(),
            (1.75, [20.0 * 0.01, 40.0 * 0.01, 60.0 * 0.01])
        );
        assert_eq!(path(&item).unwrap(), "Models/Synthetic.mdx");
        item.fields.get_mut(b"f001").unwrap().value = Ok(FieldValue::Override(
            pcx_objects::Value::Str(b"Custom/Changed.mdx".to_vec()),
        ));
        assert_eq!(path(&item).unwrap(), "Custom/Changed.mdx");
    }
    #[test]
    fn missing_malformed_and_wrong_family_fields_refuse() {
        let mut item = item();
        item.fields.get_mut(b"f002").unwrap().value = Ok(FieldValue::Cell("NaN".into()));
        assert!(properties(&item, 1.0).is_err());
        item.fields.get_mut(b"f002").unwrap().value = Ok(FieldValue::Cell("-1".into()));
        assert!(properties(&item, 1.0).is_err());
        item.fields
            .get_mut(b"f001")
            .unwrap()
            .metadata
            .as_mut()
            .unwrap()
            .from = SlkId::AbilityMetaData;
        assert!(path(&item).is_err());
        item.fields.remove(b"f001");
        assert!(path(&item).is_err());
    }

    #[test]
    #[ignore = "requires PCX_ITEM_ORIGINAL_INSTALL explicitly selected local MPQ install"]
    fn original_ledger_metadata_admits_ground_model_properties() {
        use pcx_sim::world::ObjectCatalog;
        let install = std::env::var_os("PCX_ITEM_ORIGINAL_INSTALL").expect("explicit install");
        let editor = pcx_objects::editor::Editor::open(std::path::Path::new(&install)).unwrap();
        let overlay = pcx_objects::editor::Overlay::default();
        let catalog = pcx_sim::world::MetadataCatalog {
            editor: &editor,
            units: &overlay,
            items: &overlay,
        };
        let ledger = catalog.resolve(*b"ledg").unwrap();
        let (scale, tint) = properties(&ledger, 1.0).unwrap();
        assert!(scale > 0.0 && tint.iter().all(|value| value.is_finite()));
        assert!(
            path(&ledger)
                .unwrap()
                .to_ascii_lowercase()
                .ends_with(".mdx")
        );
    }
}
