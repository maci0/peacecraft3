//! Source model and logical animation state shared by unit scripts and rendering.
use crate::scripts::AssetIdentity;
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use pcx_sim::world::{Definition, FieldValue};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub origin: AssetIdentity,
    pub member: String,
    pub bytes: Arc<[u8]>,
    pub sequences: Vec<pcx_mdl::Sequence>,
}

impl Model {
    pub fn read(
        origin: AssetIdentity,
        member: String,
        bytes: Arc<[u8]>,
        maximum_bytes: usize,
    ) -> Result<Self, String> {
        if bytes.len() > maximum_bytes
            || member.trim().is_empty()
            || [&origin.source, &origin.edition, &origin.version]
                .into_iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(format!(
                "{member}: unit model source identity or byte budget"
            ));
        }
        let model = pcx_mdl::Mdlx::parse(&bytes).map_err(|e| format!("{member}: {e}"))?;
        let sequences = model
            .sequences(&bytes)
            .map_err(|e| format!("{member}: SEQS: {e}"))?;
        if sequences
            .iter()
            .any(|s| s.interval_end < s.interval_start || s.name.chars().any(char::is_control))
        {
            return Err(format!("{member}: invalid animation interval/name"));
        }
        Ok(Self {
            origin,
            member,
            bytes,
            sequences,
        })
    }

    fn select(&self, name: &str) -> Option<usize> {
        let wanted: Vec<_> = name.split_ascii_whitespace().collect();
        if wanted.is_empty() {
            return None;
        }
        self.sequences
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name.trim()))
            .or_else(|| {
                self.sequences.iter().position(|s| {
                    let tokens: Vec<_> = s
                        .name
                        .split(|c: char| c.is_ascii_whitespace() || c == '-')
                        .filter(|s| !s.is_empty())
                        .collect();
                    tokens.len() >= wanted.len()
                        && wanted
                            .iter()
                            .zip(&tokens)
                            .all(|(a, b)| a.eq_ignore_ascii_case(b))
                })
            })
    }
}

/// Resolve the model through the selected metadata, including scalar map edits.
pub fn model_path(definition: &Definition) -> Result<Option<String>, String> {
    let mut fields = definition.fields.iter().filter(|(_, f)| {
        f.metadata.as_ref().is_some_and(|m|
        matches!(&m.source, FieldSource::Column {slk: SlkId::UnitUi,column} if column == "file"))
    });
    let (id, field) = fields.next().ok_or("missing UnitUI/file metadata")?;
    if fields.next().is_some() {
        return Err("ambiguous UnitUI/file metadata".into());
    }
    let m = field.metadata.as_ref().expect("selected metadata");
    if m.fourcc != *id
        || m.from != SlkId::UnitMetaData
        || m.index != -1
        || m.kind != FieldKind::Token
    {
        return Err("unsupported UnitUI/file metadata".into());
    }
    let path = match &field.value {
        Ok(FieldValue::Cell(text)) => text.as_str(),
        Ok(FieldValue::Override(pcx_objects::Value::Str(text))) => {
            std::str::from_utf8(text).map_err(|_| "model path is not UTF8")?
        }
        _ => return Err("unsupported UnitUI/file value".into()),
    }
    .trim();
    if matches!(path, "" | "_" | "-") {
        return Ok(None);
    }
    if path.len() > 16 << 10 || path.chars().any(char::is_control) || path.contains(',') {
        return Err("unsupported model path or model variation list".into());
    }
    let path = pcx_mdl::binary_model_path(path);
    let name = path.rsplit(['\\', '/']).next().expect("nonempty path");
    if !name.contains('.') {
        Ok(Some(format!("{path}.mdx")))
    } else if name.to_ascii_lowercase().ends_with(".mdx") {
        Ok(Some(path))
    } else {
        Err("unsupported unit model extension".into())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    pub model: Arc<Model>,
    pub sequence: Option<usize>,
    pub created: f64,
    pub started: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub interval: [u32; 2],
    pub frame: u32,
    pub elapsed_ms: u64,
}

impl Animation {
    pub fn new(model: Arc<Model>, now: f64) -> Result<Self, String> {
        if !now.is_finite() || now < 0. {
            return Err("invalid unit model game time".into());
        }
        // shortcut: animation variants use source order; qualify original rarity/RNG selection before claiming visual lockstep.
        let sequence = model
            .select("stand")
            .or_else(|| (!model.sequences.is_empty()).then_some(0));
        Ok(Self {
            model,
            sequence,
            created: now,
            started: now,
        })
    }
    /// Whether the model declares a sequence matching `name`. `select` ignores misses.
    pub fn has_sequence(&self, name: &str) -> bool {
        self.model.select(name).is_some()
    }
    pub fn select(&mut self, name: &str, now: f64) -> Result<(), String> {
        if !now.is_finite() || now < self.started {
            return Err("invalid unit animation game time".into());
        }
        if let Some(index) = self.model.select(name) {
            self.sequence = Some(index);
            self.started = now;
        }
        Ok(())
    }
    pub fn sample(&self, now: f64) -> Result<Option<Sample>, String> {
        if !now.is_finite() || now < self.started || now < self.created {
            return Err("invalid unit animation game time".into());
        }
        let Some(index) = self.sequence else {
            return Ok(None);
        };
        let sequence = self
            .model
            .sequences
            .get(index)
            .ok_or("unit animation sequence missing")?;
        let millis = |seconds: f64| -> Result<u64, String> {
            let ms = (seconds * 1000.).floor();
            if ms >= u64::MAX as f64 {
                return Err("unit animation clock overflow".into());
            }
            Ok(ms as u64)
        };
        let elapsed_ms = millis(now - self.created)?;
        let local = millis(now - self.started)?;
        let duration = u64::from(sequence.duration());
        let offset = if duration == 0 {
            0
        } else if sequence.non_looping {
            local.min(duration)
        } else {
            local % duration
        };
        Ok(Some(Sample {
            interval: [sequence.interval_start, sequence.interval_end],
            frame: sequence.interval_start + offset as u32,
            elapsed_ms,
        }))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn model() -> Arc<Model> {
        let mut bytes = b"MDLX".to_vec();
        bytes.extend(b"VERS");
        bytes.extend(4u32.to_le_bytes());
        bytes.extend(800u32.to_le_bytes());
        let mut records = Vec::new();
        for (name, start, end, once) in [
            ("Stand - 1", 100u32, 900u32, false),
            ("Stand Defend", 1200, 1800, false),
            ("Death", 2000, 2400, true),
        ] {
            let mut record = vec![0; pcx_mdl::SEQUENCE_SIZE];
            record[..name.len()].copy_from_slice(name.as_bytes());
            record[80..84].copy_from_slice(&start.to_le_bytes());
            record[84..88].copy_from_slice(&end.to_le_bytes());
            record[92..96].copy_from_slice(&u32::from(once).to_le_bytes());
            records.extend(record);
        }
        bytes.extend(b"SEQS");
        bytes.extend((records.len() as u32).to_le_bytes());
        bytes.extend(records);
        Arc::new(
            Model::read(
                AssetIdentity {
                    source: "synthetic".into(),
                    edition: "test".into(),
                    version: "fixture".into(),
                },
                "fixture.mdx".into(),
                bytes.into(),
                4096,
            )
            .unwrap(),
        )
    }
    #[test]
    fn model_paths_follow_declared_metadata_and_scalar_map_overrides() {
        use pcx_objects::editor::{FieldDef, ObjectKind};
        use pcx_sim::world::ObjectField;
        let field = FieldDef {
            fourcc: *b"M001",
            from: SlkId::UnitMetaData,
            field: "file".into(),
            slk: "UnitUI".into(),
            index: -1,
            declared: "model".into(),
            kind: FieldKind::Token,
            category: "art".into(),
            source: FieldSource::Column {
                slk: SlkId::UnitUi,
                column: "file".into(),
            },
            measured: true,
        };
        let mut definition = Definition {
            rawcode: *b"U001",
            base_rawcode: "U001".into(),
            kind: ObjectKind::Unit,
            fields: std::collections::BTreeMap::from([(
                *b"M001",
                ObjectField {
                    metadata: Some(field),
                    value: Ok(FieldValue::Cell("Units\\Synthetic\\Actor".into())),
                },
            )]),
            modifications: Vec::new(),
        };
        assert_eq!(
            model_path(&definition).unwrap().as_deref(),
            Some("Units\\Synthetic\\Actor.mdx")
        );
        definition.fields.get_mut(b"M001").unwrap().value = Ok(FieldValue::Override(
            pcx_objects::Value::Str(b"Map\\Edited.mdl".to_vec()),
        ));
        assert_eq!(
            model_path(&definition).unwrap().as_deref(),
            Some("Map\\Edited.mdx")
        );
        definition.fields.get_mut(b"M001").unwrap().value = Ok(FieldValue::Cell("_".into()));
        assert_eq!(model_path(&definition).unwrap(), None);
        definition.fields.get_mut(b"M001").unwrap().value =
            Ok(FieldValue::Cell("Map\\A,Map\\B".into()));
        assert!(model_path(&definition).is_err());
        definition
            .fields
            .get_mut(b"M001")
            .unwrap()
            .metadata
            .as_mut()
            .unwrap()
            .index = 0;
        assert!(model_path(&definition).is_err());
    }
    #[test]
    fn selected_sequence_drives_frames_without_wall_time_or_frame_partition_dependence() {
        let mut animation = Animation::new(model(), 2.).unwrap();
        animation.select("sTaNd DeFeNd", 3.).unwrap();
        assert_eq!(animation.sequence, Some(1));
        assert_eq!(
            animation.sample(3.25).unwrap(),
            Some(Sample {
                interval: [1200, 1800],
                frame: 1450,
                elapsed_ms: 1250
            })
        );
        let mut other = animation.clone();
        for time in [3.1, 3.3, 3.9, 4.25] {
            other.sample(time).unwrap();
        }
        assert_eq!(animation.sample(4.25).unwrap(), other.sample(4.25).unwrap());
        other.select("absent", 4.25).unwrap();
        assert_eq!(other, animation);
        animation.select("Death", 4.).unwrap();
        assert_eq!(animation.sample(9.).unwrap().unwrap().frame, 2400);
        assert!(animation.sample(3.).is_err());
    }
}
