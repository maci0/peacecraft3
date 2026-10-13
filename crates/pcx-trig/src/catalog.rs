//! Edition-specific editor definitions and metadata from actual TriggerData.txt.
//! This is a binding input for execution/lowering, not a trigger interpreter.
use crate::TriggerArity;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Event,
    Condition,
    Action,
    Call,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub kind: Kind,
    pub name: String,
    pub line: usize,
    /// All authored comma-separated fields, including empty/nothing fields.
    pub fields: Vec<String>,
    pub parameter_types: Vec<String>,
    pub return_type: Option<String>,
    /// An explicit alias from this file; absence is retained, never guessed.
    pub script_name: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataRecord {
    pub line: usize,
    pub value: String,
}
#[derive(Debug, Clone)]
pub struct Catalog {
    pub source: String,
    pub edition: String,
    pub version: String,
    definitions: BTreeMap<(Kind, String), Definition>,
    /// Exact section/key/value metadata, including unknown editor options.
    metadata: BTreeMap<(Kind, String), Vec<MetadataRecord>>,
    unsupported_metadata: Vec<CatalogError>,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{source_name}:line {line}: {message}")]
pub struct CatalogError {
    pub source_name: String,
    pub line: usize,
    pub message: String,
}
impl Catalog {
    pub fn parse(
        source: &str,
        edition: &str,
        version: &str,
        text: &str,
    ) -> Result<Self, CatalogError> {
        let error = |line, message| CatalogError {
            source_name: source.into(),
            line,
            message,
        };
        if source.is_empty() || edition.is_empty() || version.is_empty() {
            return Err(error(
                0,
                "source, edition and version must be explicit".into(),
            ));
        }
        let mut catalog = Self {
            source: source.into(),
            edition: edition.into(),
            version: version.into(),
            definitions: BTreeMap::new(),
            metadata: BTreeMap::new(),
            unsupported_metadata: Vec::new(),
        };
        let mut section = None;
        for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            let t = line.trim();
            if t.is_empty() || t.starts_with("//") {
                continue;
            }
            if t.starts_with('[') && t.ends_with(']') {
                section = match &t[1..t.len() - 1] {
                    "TriggerEvents" => Some(Kind::Event),
                    "TriggerConditions" => Some(Kind::Condition),
                    "TriggerActions" => Some(Kind::Action),
                    "TriggerCalls" => Some(Kind::Call),
                    _ => None,
                };
                continue;
            }
            let Some(kind) = section else {
                continue;
            };
            let Some((key, value)) = t.split_once('=') else {
                let issue = error(index + 1, format!("missing '=' in {t:?}"));
                if t.starts_with('_') {
                    catalog.unsupported_metadata.push(issue);
                    continue;
                }
                return Err(issue);
            };
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() {
                return Err(error(index + 1, "empty definition key".into()));
            }
            if key.starts_with('_') {
                catalog
                    .metadata
                    .entry((kind, key.to_string()))
                    .or_default()
                    .push(MetadataRecord {
                        line: index + 1,
                        value: value.to_string(),
                    });
                continue;
            }
            let fields: Vec<_> = value.split(',').map(|f| f.trim().to_string()).collect();
            let prefix = if kind == Kind::Call { 3 } else { 1 };
            if fields.len() < prefix || fields[0].parse::<u32>().is_err() {
                return Err(error(
                    index + 1,
                    format!("malformed {kind:?} definition {key:?}"),
                ));
            }
            if kind == Kind::Call && (fields[1].parse::<u32>().is_err() || fields[2].is_empty()) {
                return Err(error(
                    index + 1,
                    format!("missing call flag or return type for {key:?}"),
                ));
            }
            let parameter_types = fields[prefix..]
                .iter()
                .filter(|f| !f.is_empty() && *f != "nothing")
                .cloned()
                .collect();
            let return_type = if kind == Kind::Call {
                Some(fields[2].clone())
            } else {
                None
            };
            let entry = (kind, key.to_string());
            if let Some(previous) = catalog.definitions.get(&entry) {
                if previous.fields != fields {
                    return Err(error(index + 1, format!("conflicting definition {key:?}")));
                }
                continue;
            }
            catalog.definitions.insert(
                entry,
                Definition {
                    kind,
                    name: key.to_string(),
                    line: index + 1,
                    fields,
                    parameter_types,
                    return_type,
                    script_name: None,
                },
            );
        }
        if catalog.definitions.is_empty() {
            return Err(error(0, "no trigger definitions found".into()));
        }
        for ((kind, name), definition) in &mut catalog.definitions {
            if let Some(records) = catalog
                .metadata
                .get(&(*kind, format!("_{name}_ScriptName")))
            {
                let alias = &records[0].value;
                if alias.is_empty() || records.iter().any(|r| r.value != *alias) {
                    return Err(error(
                        definition.line,
                        format!("empty or conflicting ScriptName for {name:?}"),
                    ));
                }
                definition.script_name = Some(alias.clone());
            }
        }
        Ok(catalog)
    }
    pub fn get(&self, kind: Kind, name: &str) -> Option<&Definition> {
        self.definitions.get(&(kind, name.into()))
    }
    pub fn definitions(&self) -> impl Iterator<Item = &Definition> {
        self.definitions.values()
    }
    /// Malformed metadata remains an explicit unsupported state. No corrected
    /// defaults or guessed separators are synthesized from editor typos.
    pub fn unsupported_metadata(&self) -> &[CatalogError] {
        &self.unsupported_metadata
    }
    pub fn require_complete_metadata(&self) -> Result<(), CatalogError> {
        match self.unsupported_metadata.first() {
            Some(issue) => Err(issue.clone()),
            None => Ok(()),
        }
    }
    /// Preserve repeated metadata records in source order; no first/last-wins
    /// behavior is assumed for conflicting shipped editor defaults.
    pub fn metadata_records(&self, kind: Kind, key: &str) -> Option<&[MetadataRecord]> {
        self.metadata.get(&(kind, key.into())).map(Vec::as_slice)
    }
    pub fn metadata(&self, kind: Kind, key: &str) -> Result<Option<&str>, CatalogError> {
        let Some(records) = self.metadata_records(kind, key) else {
            return Ok(None);
        };
        let value = &records[0].value;
        if let Some(conflict) = records.iter().find(|r| r.value != *value) {
            return Err(CatalogError {
                source_name: self.source.clone(),
                line: conflict.line,
                message: format!("ambiguous metadata {key:?}; editor precedence is not measured"),
            });
        }
        Ok(Some(value))
    }
    /// Use this table directly with the existing WTG decoder. All counts derive
    /// from the supplied definitions; conflicting cross-kind arities are refused.
    pub fn arity(&self) -> Result<TriggerArity, crate::Error> {
        let mut arity = TriggerArity::new();
        for definition in self.definitions.values() {
            arity.insert(definition.name.as_bytes(), definition.parameter_types.len())?;
        }
        Ok(arity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = "[TriggerActions]\n_Custom_ScriptName=ActualFunction\nCustom=7,unit,real\n_Custom_Defaults=_,4\n[TriggerCalls]\nQuery=2,0,boolean,unit\n[TriggerEvents]\nStart=1,nothing\n";
    #[test]
    fn authored_aliases_types_versions_and_metadata_are_retained() {
        let catalog = Catalog::parse("fixture.txt", "synthetic", "test-7", FIXTURE).unwrap();
        let action = catalog.get(Kind::Action, "Custom").unwrap();
        assert_eq!(action.fields, ["7", "unit", "real"]);
        assert_eq!(action.parameter_types, ["unit", "real"]);
        assert_eq!(action.script_name.as_deref(), Some("ActualFunction"));
        assert_eq!(
            catalog
                .get(Kind::Call, "Query")
                .unwrap()
                .return_type
                .as_deref(),
            Some("boolean")
        );
        assert_eq!(catalog.get(Kind::Event, "Start").unwrap().script_name, None);
        assert_eq!(
            catalog.metadata(Kind::Action, "_Custom_Defaults").unwrap(),
            Some("_,4")
        );
        assert_eq!(
            catalog.arity().unwrap(),
            TriggerArity::from_trigger_data(FIXTURE).unwrap()
        );
    }
    #[test]
    fn missing_identity_and_conflicting_or_malformed_data_are_refused() {
        assert!(Catalog::parse("fixture", "", "test", FIXTURE).is_err());
        for text in [
            FIXTURE.replace("Query=2,0,boolean,unit", "Query=2"),
            FIXTURE.replace("Custom=7,unit,real", "Custom=7,unit,real\nCustom=8,integer"),
            FIXTURE.replace("_Custom_ScriptName=ActualFunction", "_Custom_ScriptName="),
            "[TriggerCalls]\nBroken\n".into(),
        ] {
            let error = Catalog::parse("fixture.txt", "synthetic", "test", &text).unwrap_err();
            assert_eq!(error.source_name, "fixture.txt");
            assert!(error.line > 0);
        }
    }
}

#[cfg(test)]
mod original_tests {
    use super::*;
    #[test]
    #[ignore = "requires explicit local original TriggerData path, edition and version in PEACECRAFT3_TRIGGER_* environment variables"]
    fn original_catalog_derives_the_same_wtg_arity_and_preserves_script_aliases() {
        let env = |name: &str| {
            std::env::var(format!("PEACECRAFT3_TRIGGER_{name}"))
                .unwrap_or_else(|_| panic!("set PEACECRAFT3_TRIGGER_{name}"))
        };
        let path = env("FILE");
        let text = std::fs::read_to_string(&path).unwrap();
        let catalog = Catalog::parse(&path, &env("EDITION"), &env("VERSION"), &text).unwrap();
        assert_eq!(
            catalog.arity().unwrap(),
            TriggerArity::from_trigger_data(&text).unwrap()
        );
        let aliases = catalog
            .definitions()
            .filter(|d| d.script_name.is_some())
            .count();
        assert!(
            aliases > 0,
            "explicitly supplied original should exercise ScriptName metadata"
        );
        println!(
            "original catalog: {} definitions, {aliases} authored aliases, {} unsupported metadata records",
            catalog.definitions().count(),
            catalog.unsupported_metadata().len()
        );
    }
}

#[cfg(test)]
mod repeated_metadata_tests {
    use super::*;
    #[test]
    fn conflicting_authored_defaults_are_preserved_without_assumed_precedence() {
        let catalog = Catalog::parse(
            "fixture",
            "synthetic",
            "test",
            "[TriggerActions]\nAction=1,real\n_Action_Defaults=3\n_Action_Defaults=7\n",
        )
        .unwrap();
        let records = catalog
            .metadata_records(Kind::Action, "_Action_Defaults")
            .unwrap();
        assert_eq!(
            records.iter().map(|r| r.value.as_str()).collect::<Vec<_>>(),
            ["3", "7"]
        );
        assert_eq!(
            catalog
                .metadata(Kind::Action, "_Action_Defaults")
                .unwrap_err()
                .line,
            4
        );
        assert_eq!(catalog.arity().unwrap().get(b"Action"), Some(1));
    }
}

#[cfg(test)]
mod malformed_metadata_tests {
    use super::*;
    #[test]
    fn malformed_editor_metadata_is_reported_without_inventing_a_definition() {
        let catalog = Catalog::parse(
            "fixture",
            "synthetic",
            "test",
            "[TriggerActions]\nAction=1,real\n_Action_Limits-_,_\n",
        )
        .unwrap();
        assert_eq!(catalog.unsupported_metadata().len(), 1);
        assert_eq!(catalog.require_complete_metadata().unwrap_err().line, 3);
        assert_eq!(
            catalog.metadata(Kind::Action, "_Action_Limits").unwrap(),
            None
        );
        assert_eq!(catalog.arity().unwrap().get(b"Action"), Some(1));
    }
}
