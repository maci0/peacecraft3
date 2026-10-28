use pcx_objects::Fourcc;
use pcx_objects::editor::{FieldDef, FieldKind, FieldSource, ObjectKind, SlkId};
use pcx_sim::world::{ColumnError, Definition, FieldProblem, ObjectField};
use std::collections::BTreeMap;

fn field(id: Fourcc, source: Option<FieldSource>) -> ObjectField {
    ObjectField {
        metadata: source.map(|source| FieldDef {
            fourcc: id,
            from: SlkId::UnitMetaData,
            field: "synthetic".into(),
            slk: "UnitBalance".into(),
            index: -1,
            declared: "real".into(),
            kind: FieldKind::Real,
            category: "synthetic".into(),
            source,
            measured: true,
        }),
        value: Err(FieldProblem::Missing),
    }
}

fn column(slk: SlkId, name: &str) -> Option<FieldSource> {
    Some(FieldSource::Column {
        slk,
        column: name.into(),
    })
}

fn definition(fields: Vec<(Fourcc, Option<FieldSource>)>) -> Definition {
    Definition {
        rawcode: *b"u000",
        base_rawcode: "u000".into(),
        kind: ObjectKind::Unit,
        fields: fields
            .into_iter()
            .map(|(id, source)| (id, field(id, source)))
            .collect::<BTreeMap<_, _>>(),
        modifications: Vec::new(),
    }
}

#[test]
fn a_column_selects_its_one_field_in_any_table() {
    let unit = definition(vec![
        (*b"A001", column(SlkId::UnitBalance, "sight")),
        (*b"A002", column(SlkId::UnitUi, "modelScale")),
        (
            *b"A003",
            Some(FieldSource::Profile {
                key: "sight".into(),
                index: -1,
                slots: 1,
            }),
        ),
        (*b"A004", None),
    ]);
    let index = unit.column_index();
    for name in ["sight", "modelScale"] {
        let (id, _) = unit.column_field(name).unwrap();
        assert_eq!(index.field(&unit, name).unwrap().0, id);
    }
    assert_eq!(unit.column_field("sight").unwrap().0, b"A001");
    assert_eq!(unit.column_field("modelScale").unwrap().0, b"A002");
}

#[test]
fn missing_and_ambiguous_columns_are_distinct_errors_in_both_lookups() {
    let unit = definition(vec![
        (*b"A001", column(SlkId::UnitBalance, "dup")),
        (*b"A002", column(SlkId::UnitUi, "dup")),
        (*b"A003", column(SlkId::UnitData, "dup")),
        (*b"A004", column(SlkId::UnitData, "one")),
    ]);
    let index = unit.column_index();
    for (name, expected) in [
        ("dup", ColumnError::Ambiguous),
        ("absent", ColumnError::Missing),
        ("Dup", ColumnError::Missing),
    ] {
        assert_eq!(unit.column_field(name).unwrap_err(), expected, "{name}");
        assert_eq!(index.field(&unit, name).unwrap_err(), expected, "{name}");
    }
    assert!(unit.column_field("one").is_ok());
}

#[test]
fn the_index_matches_the_scan_on_generated_definitions() {
    let columns = ["a", "b", "c", "d", "e"];
    let mut state = 12345u64;
    let mut next = |bound: usize| {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        usize::try_from(state >> 33).unwrap() % bound
    };
    for _ in 0..200 {
        let count = next(12);
        let fields = (0..count)
            .map(|n| {
                let id = [b'F', b'0', u8::try_from(n).unwrap(), 0];
                let source = match next(4) {
                    0 => None,
                    1 => Some(FieldSource::Profile {
                        key: "k".into(),
                        index: -1,
                        slots: 1,
                    }),
                    _ => column(SlkId::UnitBalance, columns[next(columns.len())]),
                };
                (id, source)
            })
            .collect();
        let unit = definition(fields);
        let index = unit.column_index();
        for name in columns.iter().copied().chain(["missing"]) {
            let scan = unit.column_field(name).map(|(id, _)| *id);
            let indexed = index.field(&unit, name).map(|(id, _)| *id);
            assert_eq!(indexed, scan, "{name}");
        }
    }
}

#[test]
fn an_index_of_a_changed_definition_never_returns_a_removed_field() {
    let mut unit = definition(vec![(*b"A001", column(SlkId::UnitBalance, "sight"))]);
    let index = unit.column_index();
    unit.fields.remove(b"A001");
    assert_eq!(
        index.field(&unit, "sight").unwrap_err(),
        ColumnError::Missing
    );
}
