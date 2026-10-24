use super::*;
use pcx_objects::editor::{FieldDef, FieldKind, FieldSource, SlkId};
use pcx_sim::world::{Definition, FieldProblem, FieldValue, ObjectField};
use std::collections::BTreeMap;

struct Catalog(BTreeMap<[u8; 4], Definition>);
impl ObjectCatalog for Catalog {
    fn resolve(&self, code: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        self.0.get(&code).cloned().ok_or_else(|| {
            ResolutionProblem::MissingRawcode(String::from_utf8_lossy(&code).into_owned())
        })
    }
}

fn field(code: [u8; 4], key: &str, text: &str) -> ([u8; 4], ObjectField) {
    (
        code,
        ObjectField {
            metadata: Some(FieldDef {
                fourcc: code,
                from: SlkId::UnitMetaData,
                field: key.into(),
                slk: "Profile".into(),
                index: -1,
                declared: "unitList".into(),
                kind: FieldKind::Token,
                category: "synthetic renamed category".into(),
                source: FieldSource::Profile {
                    key: key.to_ascii_lowercase(),
                    index: -1,
                    slots: 1,
                },
                measured: true,
            }),
            value: Ok(FieldValue::Cell(text.into())),
        },
    )
}

fn definition(code: &[u8; 4], upgrades: &str, alternatives: &str) -> Definition {
    Definition {
        rawcode: *code,
        base_rawcode: String::from_utf8_lossy(code).into_owned(),
        kind: ObjectKind::Unit,
        fields: BTreeMap::from([
            field(*b"Z001", "Upgrade", upgrades),
            field(*b"Z002", "DependencyOr", alternatives),
        ]),
        modifications: Vec::new(),
    }
}

fn catalog(rows: &[(&[u8; 4], &str, &str)]) -> Catalog {
    Catalog(
        rows.iter()
            .map(|(code, upgrades, alternatives)| {
                (**code, definition(code, upgrades, alternatives))
            })
            .collect(),
    )
}

#[test]
fn source_alternatives_apply_to_specific_counts_and_upgrade_children_only_to_nonspecific_counts() {
    let source = catalog(&[
        (b"a000", "a001", "a003"),
        (b"a001", "a002", ""),
        (b"a002", "", ""),
        (b"a003", "a004", ""),
        (b"a004", "", ""),
    ]);
    let all = provider_types(&source, *b"a000", false, 32)
        .unwrap()
        .unwrap();
    assert_eq!(
        all,
        HashSet::from([*b"a000", *b"a001", *b"a002", *b"a003", *b"a004"])
    );
    assert_eq!(
        provider_types(&source, *b"a000", true, 32)
            .unwrap()
            .unwrap(),
        HashSet::from([*b"a000", *b"a003"])
    );
    let mut changed = source;
    changed
        .0
        .get_mut(b"a000")
        .unwrap()
        .fields
        .get_mut(b"Z002")
        .unwrap()
        .value = Ok(FieldValue::Override(pcx_objects::Value::Str(
        b"a004".to_vec(),
    )));
    assert_eq!(
        provider_types(&changed, *b"a000", true, 32)
            .unwrap()
            .unwrap(),
        HashSet::from([*b"a000", *b"a004"])
    );
}

#[test]
fn overlapping_relationships_form_one_bounded_union() {
    let source = catalog(&[
        (b"a000", "a001,a002", "a001"),
        (b"a001", "a003", ""),
        (b"a002", "a003", ""),
        (b"a003", "", "a000"),
    ]);
    assert_eq!(
        provider_types(&source, *b"a000", false, 32)
            .unwrap()
            .unwrap(),
        HashSet::from([*b"a000", *b"a001", *b"a002", *b"a003"])
    );
    assert!(
        provider_types(&source, *b"a000", false, 3)
            .unwrap_err()
            .contains("budget")
    );
    let source = catalog(&[(b"a000", "a001", ""), (b"a001", "", "")]);
    assert!(provider_types(&source, *b"a000", false, 2).is_err());
    assert_eq!(
        provider_types(&source, *b"a000", false, 3)
            .unwrap()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn missing_malformed_indexed_and_mismatched_relationship_data_refuse() {
    for text in ["bad", "a001,", "a001,,a002", "a00\n"] {
        let source = catalog(&[(b"a000", text, ""), (b"a001", "", "")]);
        assert!(
            provider_types(&source, *b"a000", false, 32).is_err(),
            "{text:?}"
        );
    }
    let mut source = catalog(&[(b"a000", "a001", "")]);
    assert!(provider_types(&source, *b"a000", false, 32).is_err());
    assert!(
        provider_types(&source, *b"a999", false, 32)
            .unwrap()
            .is_none()
    );
    assert!(
        provider_types(&source, [0, 0, 0, 6], false, 32)
            .unwrap()
            .is_none()
    );
    source
        .0
        .get_mut(b"a000")
        .unwrap()
        .fields
        .get_mut(b"Z001")
        .unwrap()
        .value = Err(FieldProblem::Indexed([1, 0]));
    assert!(provider_types(&source, *b"a000", false, 32).is_err());
    source.0.get_mut(b"a000").unwrap().rawcode = *b"a999";
    assert!(
        provider_types(&source, *b"a000", false, 32)
            .unwrap_err()
            .contains("identity")
    );
}
