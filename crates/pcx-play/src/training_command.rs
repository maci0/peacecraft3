//! Training presentation from resolved unit metadata, including map overrides.
use crate::ability_profile::Tooltip;
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use pcx_sim::world::{Definition, FieldValue};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub kind: i32,
    pub art: String,
    pub position: [usize; 2],
    pub hotkey: Option<char>,
    pub tooltip: Result<Tooltip, String>,
}

pub fn types(definition: &Definition) -> Result<Vec<[u8; 4]>, String> {
    let mut fields = definition.fields.values().filter(|field| {
        field.metadata.as_ref().is_some_and(|m| {
        matches!(&m.source, FieldSource::Profile { key, .. } if key.eq_ignore_ascii_case("Trains"))
    })
    });
    let field = fields.next().ok_or("Trains metadata is not admitted")?;
    let metadata = field.metadata.as_ref().expect("matched");
    if fields.next().is_some()
        || metadata.from != SlkId::UnitMetaData
        || metadata.declared != "unitList"
        || metadata.kind != FieldKind::Token
        || metadata.index != -1
        || !metadata.field.eq_ignore_ascii_case("Trains")
        || !matches!(
            &metadata.source,
            FieldSource::Profile {
                index: -1,
                slots: 1,
                ..
            }
        )
    {
        return Err("Trains metadata is ambiguous or indexed".into());
    }
    let text = match &field.value {
        Err(pcx_sim::world::FieldProblem::Missing) => return Ok(Vec::new()),
        Ok(FieldValue::Override(pcx_objects::Value::Int(_))) => {
            return Err("Trains requires a string value".into());
        }
        _ => value(&field.value)?,
    };
    if matches!(text.trim(), "" | "_" | "-") {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for token in text.split(',').filter(|token| !token.trim().is_empty()) {
        let code: [u8; 4] = token
            .trim()
            .as_bytes()
            .try_into()
            .map_err(|_| "invalid Trains rawcode")?;
        if !code.iter().all(u8::is_ascii_graphic) {
            return Err("invalid Trains rawcode".into());
        }
        if seen.insert(code) {
            result.push(code);
        }
    }
    Ok(result)
}

fn value(field: &Result<FieldValue, pcx_sim::world::FieldProblem>) -> Result<String, String> {
    let text = match field {
        Ok(FieldValue::Cell(text)) => text.as_str(),
        Ok(FieldValue::Override(pcx_objects::Value::Str(text))) => {
            std::str::from_utf8(text).map_err(|_| "profile is not UTF-8")?
        }
        Ok(FieldValue::Override(pcx_objects::Value::Int(value))) => {
            return Ok((*value as i32).to_string());
        }
        _ => return Err("missing or unsupported unit profile value".into()),
    };
    if text.len() > 64 << 10 {
        return Err("unit profile text budget exceeded".into());
    }
    Ok(text.to_owned())
}

fn cell(
    definition: &Definition,
    id: [u8; 4],
    key: &str,
    index: i32,
    slots: u32,
    kind: FieldKind,
) -> Result<String, String> {
    let field = definition
        .fields
        .get(&id)
        .ok_or_else(|| format!("{key} metadata missing"))?;
    let m = field
        .metadata
        .as_ref()
        .ok_or_else(|| format!("{key} metadata missing"))?;
    if m.fourcc != id
        || !m.field.eq_ignore_ascii_case(key)
        || m.from != SlkId::UnitMetaData
        || m.kind != kind
        || m.index != index
        || !matches!(&m.source, FieldSource::Profile {key: k, index: i, slots: s} if k.eq_ignore_ascii_case(key) && *i == index && *s >= slots)
    {
        return Err(format!("{key} metadata mapping differs"));
    }
    if let Ok(FieldValue::Override(value)) = &field.value {
        let valid = matches!(
            (kind, value),
            (FieldKind::Int, pcx_objects::Value::Int(_))
                | (
                    FieldKind::Token | FieldKind::Char,
                    pcx_objects::Value::Str(_)
                )
        );
        if !valid {
            return Err(format!("{key}: override wire type differs"));
        }
    }
    value(&field.value).map_err(|why| format!("{key}: {why}"))
}

impl Command {
    pub fn read(definition: &Definition) -> Result<Self, String> {
        if definition.kind != pcx_objects::editor::ObjectKind::Unit {
            return Err("training presentation requires a unit definition".into());
        }
        let read = |id, key| cell(definition, id, key, 0, 1, FieldKind::Token);
        let art = read(*b"uico", "Art")?;
        pcx_assets::AssetPath::new(&art).map_err(|e| e.to_string())?;
        let position = [*b"ubpx", *b"ubpy"]
            .into_iter()
            .enumerate()
            .map(|(i, id)| {
                cell(definition, id, "Buttonpos", i as i32, 2, FieldKind::Int)?
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| "invalid Buttonpos coordinate".into())
            })
            .collect::<Result<Vec<_>, String>>()?
            .try_into()
            .expect("two coordinates");
        let key = cell(definition, *b"uhot", "Hotkey", 0, 1, FieldKind::Char)?;
        let hotkey = if matches!(key.trim(), "" | "_" | "-") {
            None
        } else {
            let mut chars = key.trim().chars();
            let key = chars.next().expect("nonempty");
            if chars.next().is_some() || !key.is_ascii_alphabetic() {
                return Err("unsupported training hotkey".into());
            }
            Some(key)
        };
        let tooltip = (|| {
            let title = read(*b"utip", "Tip")?;
            let description = read(*b"utub", "Ubertip")?;
            if [&title, &description]
                .iter()
                .any(|text| text.starts_with("TRIGSTR_") || text.contains(['<', '>']))
            {
                return Err("training tooltip reference is not resolved".into());
            }
            Ok(Tooltip { title, description })
        })();
        Ok(Self {
            kind: i32::from_be_bytes(definition.rawcode),
            art,
            position,
            hotkey,
            tooltip,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_objects::editor::{FieldDef, ObjectKind};
    use pcx_sim::world::ObjectField;

    fn fixture() -> Definition {
        let mut fields = std::collections::BTreeMap::new();
        for (id, key, index, slots, kind, text) in [
            (*b"uico", "Art", 0, 1, FieldKind::Token, "fixture/icon.blp"),
            (*b"ubpx", "Buttonpos", 0, 2, FieldKind::Int, "2"),
            (*b"ubpy", "Buttonpos", 1, 2, FieldKind::Int, "1"),
            (*b"uhot", "Hotkey", 0, 1, FieldKind::Char, "Q"),
            (
                *b"utip",
                "Tip",
                0,
                1,
                FieldKind::Token,
                "Train |cffaabbccQ|r",
            ),
            (
                *b"utub",
                "Ubertip",
                0,
                1,
                FieldKind::Token,
                "Source text.|nSecond line.",
            ),
            (
                *b"T001",
                "Trains",
                -1,
                1,
                FieldKind::Token,
                "u001,u002,u001",
            ),
        ] {
            fields.insert(
                id,
                ObjectField {
                    metadata: Some(FieldDef {
                        fourcc: id,
                        from: SlkId::UnitMetaData,
                        field: key.into(),
                        slk: "Profile".into(),
                        index,
                        declared: if key == "Trains" {
                            "unitList"
                        } else {
                            "synthetic"
                        }
                        .into(),
                        kind,
                        category: "fixture".into(),
                        source: FieldSource::Profile {
                            key: key.to_ascii_lowercase(),
                            index,
                            slots,
                        },
                        measured: true,
                    }),
                    value: Ok(FieldValue::Cell(text.into())),
                },
            );
        }
        Definition {
            rawcode: *b"uNEW",
            base_rawcode: "uNEW".into(),
            kind: ObjectKind::Unit,
            fields,
            modifications: Vec::new(),
        }
    }

    #[test]
    fn duplicated_profile_keys_keep_the_unit_field_binding() {
        let mut definition = fixture();
        let mut item = definition.fields.get(b"uico").unwrap().clone();
        item.metadata.as_mut().unwrap().fourcc = *b"iico";
        item.value = Ok(FieldValue::Cell("item/wrong.blp".into()));
        definition.fields.insert(*b"iico", item);
        if let FieldSource::Profile { slots, .. } = &mut definition
            .fields
            .get_mut(b"uico")
            .unwrap()
            .metadata
            .as_mut()
            .unwrap()
            .source
        {
            *slots = 2;
        }
        assert_eq!(Command::read(&definition).unwrap().art, "fixture/icon.blp");
        definition.fields.remove(b"uico");
        assert!(Command::read(&definition).is_err());
    }

    #[test]
    #[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL and PCX_WAR3_EXE original inputs"]
    fn selected_original_training_profiles_decode() {
        use pcx_objects::editor::{
            Overlay,
            defaults::{DefaultTables, Family},
        };
        use pcx_sim::world::{MetadataCatalog, ObjectCatalog};
        let install =
            std::path::PathBuf::from(std::env::var_os("PCX_NATIVE_CHAPTER_INSTALL").unwrap());
        let exe = std::env::var("PCX_WAR3_EXE").unwrap();
        let image = std::fs::read(&exe).unwrap();
        let identity = crate::campaign_inputs::client_identity(&image, &exe);
        let tables = DefaultTables::open(&install, identity.version).unwrap();
        let units = Overlay::default();
        let items = Overlay::default();
        let catalog = MetadataCatalog {
            editor: tables.editor(),
            units: &units,
            items: &items,
        };
        let producer = catalog.resolve(*b"hbar").unwrap();
        let trained = types(&producer).unwrap();
        assert!(!trained.is_empty());
        for kind in trained {
            let definition = catalog.resolve(kind).unwrap();
            let command = Command::read(&definition).unwrap();
            let row = pcx_objects::fourcc_text(kind);
            let object = tables.object(Family::Unit, &row).unwrap();
            assert_eq!(object.profile("Art")[0].text, command.art);
            let raw = object.profile("Buttonpos");
            let authored: Vec<usize> = raw[0]
                .text
                .split(',')
                .map(|v| v.trim().parse().unwrap())
                .collect();
            assert_eq!(authored, command.position);
            assert!(command.tooltip.is_ok(), "{kind:?}: {:?}", command.tooltip);
        }
    }

    #[test]
    fn metadata_and_map_values_drive_icon_position_binding_and_text() {
        let mut definition = fixture();
        assert_eq!(types(&definition).unwrap(), vec![*b"u001", *b"u002"]);
        definition.fields.get_mut(b"uico").unwrap().value = Ok(FieldValue::Override(
            pcx_objects::Value::Str(b"map/new.blp".to_vec()),
        ));
        definition.fields.get_mut(b"ubpx").unwrap().value =
            Ok(FieldValue::Override(pcx_objects::Value::Int(4)));
        let command = Command::read(&definition).unwrap();
        assert_eq!(command.kind, i32::from_be_bytes(*b"uNEW"));
        assert_eq!(command.art, "map/new.blp");
        assert_eq!(command.position, [4, 1]);
        assert_eq!(command.hotkey, Some('Q'));
        assert_eq!(
            command.tooltip.unwrap().description,
            "Source text.|nSecond line."
        );
    }

    #[test]
    fn missing_malformed_or_wrong_family_fields_do_not_fall_back() {
        for id in [*b"uico", *b"ubpx", *b"ubpy", *b"uhot"] {
            let mut definition = fixture();
            definition.fields.remove(&id);
            assert!(Command::read(&definition).is_err());
        }
        let mut definition = fixture();
        definition.fields.get_mut(b"ubpx").unwrap().value =
            Ok(FieldValue::Override(pcx_objects::Value::Int(u32::MAX)));
        assert!(Command::read(&definition).is_err());
        definition = fixture();
        definition.fields.get_mut(b"uico").unwrap().value =
            Ok(FieldValue::Override(pcx_objects::Value::Str(vec![255])));
        assert!(Command::read(&definition).is_err());
        definition = fixture();
        definition.kind = ObjectKind::Item;
        assert!(Command::read(&definition).is_err());
        definition = fixture();
        definition.fields.get_mut(b"utip").unwrap().value =
            Ok(FieldValue::Cell("TRIGSTR_9".into()));
        assert!(Command::read(&definition).unwrap().tooltip.is_err());
        definition.fields.get_mut(b"T001").unwrap().value = Ok(FieldValue::Cell("u001,bad".into()));
        assert!(types(&definition).is_err());
    }
}
