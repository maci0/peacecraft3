use super::*;
use std::collections::BTreeSet;

fn table(headers: &[&str], rows: &[&[&str]]) -> pcx_slq::Table {
    let mut text = String::from("ID;PWXL;N;E\n");
    for (y, row) in std::iter::once(headers)
        .chain(rows.iter().copied())
        .enumerate()
    {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;X{};Y{};K\"{}\"\n", x + 1, y + 1, value));
        }
    }
    text.push_str("E\n");
    let text = text.replace("\n", "\r\n");
    pcx_slq::Table::parse(text.as_bytes()).unwrap()
}

fn fixture(tables: BTreeMap<SlkId, pcx_slq::Table>, profiles: &[(&str, &str)]) -> DefaultTables {
    let fields: Vec<_> = METADATA_SLK
        .iter()
        .filter_map(|&id| tables.get(&id).map(|table| (id, table)))
        .flat_map(|(id, table)| FieldDef::read(table, id))
        .collect();
    let fields_by_fourcc = fields
        .iter()
        .enumerate()
        .map(|(i, def)| (def.fourcc, i))
        .collect();
    let mut assets: BTreeMap<_, _> = tables
        .iter()
        .map(|(id, table)| {
            (
                id.path().into(),
                Asset {
                    member: id.path().into(),
                    archive: "synthetic.mpq".into(),
                    fnv1a64: table.fnv1a64(),
                },
            )
        })
        .collect();
    let profiles = profiles
        .iter()
        .map(|(member, text)| {
            assets.insert(
                member.to_string(),
                Asset {
                    member: member.to_string(),
                    archive: "synthetic.mpq".into(),
                    fnv1a64: pcx_mpq::fnv1a64(text.as_bytes()),
                },
            );
            Profile {
                member: member.to_string(),
                families: profile_families(member),
                sections: parse_profile(text.as_bytes(), member).unwrap().sections,
            }
        })
        .collect();
    let editor = Editor {
        build_label: "synthetic".into(),
        census: Default::default(),
        tables,
        rows: BTreeMap::new(),
        spellings: BTreeMap::new(),
        fields,
        fields_by_fourcc,
        profiles: BTreeMap::new(),
        profile_keys: BTreeSet::new(),
        profile_duplicates: Vec::new(),
        commands: BTreeMap::new(),
        absent: Vec::new(),
    };
    DefaultTables::assemble(
        editor,
        Identity {
            install: "synthetic".into(),
            edition_version: "fixture edition/version".into(),
        },
        assets,
        profiles,
        Vec::new(),
    )
    .unwrap()
}

const META: &[&str] = &[
    "ID",
    "field",
    "slk",
    "index",
    "type",
    "category",
    "repeat",
    "data",
    "useSpecific",
];

#[test]
fn aliases_keep_distinct_objects_and_authored_level_columns() {
    let tables = fixture(
        BTreeMap::from([
            (
                SlkId::AbilityData,
                table(
                    &["alias", "code", "levels", "Cool1", "Cool2", "DataA1"],
                    &[
                        &["X001", "Z001", "1", "2.5", "7.25", "8"],
                        &["X002", "Z001", "2", "9.5", "11.25", "13"],
                    ],
                ),
            ),
            (
                SlkId::AbilityMetaData,
                table(
                    META,
                    &[
                        &[
                            "alev",
                            "levels",
                            "AbilityData",
                            "-1",
                            "int",
                            "stats",
                            "0",
                            "0",
                            "",
                        ],
                        &[
                            "acdn",
                            "Cool",
                            "AbilityData",
                            "-1",
                            "unreal",
                            "stats",
                            "4",
                            "0",
                            "",
                        ],
                        &[
                            "D001",
                            "Data",
                            "AbilityData",
                            "-1",
                            "int",
                            "data",
                            "4",
                            "1",
                            "X001",
                        ],
                    ],
                ),
            ),
        ]),
        &[],
    );
    assert_eq!(
        tables.rawcodes(Family::Ability).collect::<Vec<_>>(),
        ["X001", "X002"]
    );
    assert!(tables.object(Family::Ability, "Z001").is_err());
    let first = tables.object(Family::Ability, "x001").unwrap();
    let second = tables.object(Family::Ability, "X002").unwrap();
    assert_eq!(
        first.level_count().unwrap().typed(FieldKind::Int).unwrap(),
        Value::Integer(1)
    );
    // An authored padded column survives; it does not become an effective level.
    assert_eq!(first.level_cell("Cool", 2).unwrap().text, "7.25");
    assert_eq!(second.level_cell("Cool", 2).unwrap().text, "11.25");
    assert!(first.level_cell("Cool", 3).is_err());
    assert_eq!(first.level_columns()[&1], ["Cool1", "DataA1"]);
    let mapped = first.field(*b"alev").unwrap();
    assert_eq!(mapped.value, Value::Integer(1));
    assert_eq!(
        mapped.cell.provenance.asset.member,
        SlkId::AbilityData.path()
    );
    assert_eq!(mapped.cell.provenance.asset.archive, "synthetic.mpq");
    assert_eq!(
        mapped.cell.provenance.identity.edition_version,
        "fixture edition/version"
    );
    assert_eq!(
        mapped
            .metadata
            .attribute("type")
            .unwrap()
            .provenance
            .asset
            .member,
        SlkId::AbilityMetaData.path()
    );
    assert!(matches!(
        first.field(*b"acdn"),
        Err(Refusal::Mapping { .. })
    ));
    assert_eq!(
        tables
            .metadata(*b"D001")
            .unwrap()
            .attribute("data")
            .unwrap()
            .text,
        "1"
    );
    assert_eq!(
        tables
            .metadata(*b"D001")
            .unwrap()
            .attribute("useSpecific")
            .unwrap()
            .text,
        "X001"
    );
}

#[test]
fn profile_quotes_slots_and_conflicts_have_exact_provenance() {
    let tables = fixture(
        BTreeMap::from([
            (
                SlkId::UnitData,
                table(&["unitID", "race"], &[&["Z001", "fixture-race"]]),
            ),
            (
                SlkId::UnitMetaData,
                table(
                    META,
                    &[
                        &[
                            "ubpx",
                            "Buttonpos",
                            "Profile",
                            "0",
                            "int",
                            "art",
                            "0",
                            "0",
                            "",
                        ],
                        &[
                            "ubpy",
                            "Buttonpos",
                            "Profile",
                            "1",
                            "int",
                            "art",
                            "0",
                            "0",
                            "",
                        ],
                        &[
                            "unam", "Name", "Profile", "0", "string", "text", "0", "0", "",
                        ],
                        &[
                            "utip", "Tip", "Profile", "0", "string", "text", "0", "0", "",
                        ],
                    ],
                ),
            ),
        ]),
        &[
            (
                r"Units\FixtureUnitFunc.txt",
                "[Z001]\nButtonpos=3,5\nName=\"name, with comma\"\nTip=first\n",
            ),
            (r"Units\OtherUnitStrings.txt", "[z001]\nTip=second\n"),
        ],
    );
    let unit = tables.object(Family::Unit, "z001").unwrap();
    assert_eq!(unit.field(*b"ubpx").unwrap().value, Value::Integer(3));
    assert_eq!(unit.field(*b"ubpy").unwrap().value, Value::Integer(5));
    let name = unit.field(*b"unam").unwrap();
    assert_eq!(name.value, Value::Text("name, with comma"));
    assert_eq!(
        name.cell.provenance.location,
        Location::Profile {
            section: "z001".into(),
            key: "name".into(),
            line: 3
        }
    );
    assert!(matches!(
        unit.field(*b"utip"),
        Err(Refusal::AmbiguousProfile { .. })
    ));
    assert_eq!(unit.profile("Tip").len(), 2);
    assert_eq!(
        profile_elements(r#""a,b","c",,Doodads\path"#).unwrap(),
        ["a,b", "c", "", r"Doodads\path"]
    );
    let partial = parse_profile(b"[Z001]\nName=\"open\nHP=17", "fixture.txt").unwrap();
    assert_eq!(partial.unresolved.len(), 1);
    assert!(
        matches!(&partial.unresolved[0], Unresolved::ProfileLine { line: 2, text, .. } if text == "Name=\"open")
    );
    assert!(partial.sections["z001"]["name"][0].parse_error.is_some());
    assert_eq!(partial.sections["z001"]["hp"][0].value, "17");
    assert_eq!(
        parse_profile(b"[Z001\nName=value", "fixture.txt")
            .unwrap()
            .unresolved
            .len(),
        2
    );
    assert!(parse_profile(b"[Z001]\nName=\xff", "fixture.txt").is_err());
}

#[test]
fn markers_malformed_values_and_native_defaults_are_not_invented() {
    let tables = fixture(
        BTreeMap::from([
            (
                SlkId::ItemData,
                table(
                    &["itemID", "HP", "usable", "scale"],
                    &[&["Z001", "-", " 2 ", "NaN"]],
                ),
            ),
            (
                SlkId::UnitMetaData,
                table(
                    META,
                    &[
                        &["ihpm", "HP", "ItemData", "-1", "int", "stats", "0", "0", ""],
                        &[
                            "iusa", "usable", "ItemData", "-1", "bool", "stats", "0", "0", "",
                        ],
                        &[
                            "isca", "scale", "ItemData", "-1", "real", "art", "0", "0", "",
                        ],
                    ],
                ),
            ),
        ]),
        &[],
    );
    let item = tables.object(Family::Item, "z001").unwrap();
    assert_eq!(item.field(*b"ihpm").unwrap().value, Value::Marker("-"));
    let malformed = item.field(*b"iusa").unwrap_err();
    let Refusal::MalformedValue {
        member,
        location,
        declared,
        value,
    } = &malformed
    else {
        panic!("unexpected refusal: {malformed}");
    };
    assert_eq!(member, SlkId::ItemData.path());
    assert_eq!(
        location,
        &item
            .cell(SlkId::ItemData, "usable")
            .unwrap()
            .provenance
            .location
    );
    assert_eq!(declared, &FieldKind::Bool.to_string());
    assert_eq!(value.as_ref(), " 2 ");
    assert_eq!(malformed.clone(), malformed);
    assert_eq!(
        malformed.to_string(),
        format!(
            "{member}: {declared} value {:?} is malformed at {location:?}",
            " 2 "
        )
    );
    assert!(matches!(
        item.field(*b"isca"),
        Err(Refusal::MalformedValue { .. })
    ));
    assert!(matches!(
        item.field(*b"xxxx"),
        Err(Refusal::Metadata { .. })
    ));
    assert!(matches!(
        item.cell(SlkId::ItemData, "missing"),
        Err(Refusal::MissingCell { .. })
    ));
    for behavior in [
        Behavior::EngineDefaults,
        Behavior::SelectorApplicability,
        Behavior::TechTreeEvaluation,
        Behavior::AvailabilityEvaluation,
        Behavior::RandomUnitSelection,
        Behavior::RandomItemSelection,
        Behavior::EditorStatParity,
    ] {
        assert_eq!(
            tables.behavior(behavior),
            Refusal::NativeEvidence { behavior }
        );
    }
}

#[test]
fn duplicate_ids_and_missing_row_ids_are_reported() {
    let tables = fixture(
        BTreeMap::from([
            (
                SlkId::AbilityBuffData,
                table(
                    &["alias", "code"],
                    &[&["Z001", "Z002"], &["z001", "Z003"], &["long-id", "Z004"]],
                ),
            ),
            (
                SlkId::DestructableData,
                table(&["DestructableID", "HP"], &[&["Y001", "23"]]),
            ),
        ]),
        &[],
    );
    assert_eq!(tables.unresolved().len(), 2);
    assert!(tables.object(Family::Buff, "Z001").is_err());
    assert_eq!(
        tables
            .object(Family::Destructable, "Y001")
            .unwrap()
            .cell(SlkId::DestructableData, "HP")
            .unwrap()
            .text,
        "23"
    );
    assert!(matches!(
        DefaultTables::open(Path::new("."), ""),
        Err(Unresolved::Identity)
    ));
}

#[test]
fn unsupported_header_and_profile_entry_are_refused_without_losing_valid_data() {
    let tables = fixture(
        BTreeMap::from([
            (
                SlkId::UnitWeapons,
                table(&["unknown", "range"], &[&["Z001", "19"]]),
            ),
            (
                SlkId::ItemData,
                table(&["HP", "itemID", "pickRandom"], &[&["23", "Z001", "1"]]),
            ),
            (
                SlkId::UnitMetaData,
                table(
                    META,
                    &[
                        &[
                            "inam", "Name", "Profile", "0", "string", "text", "0", "0", "",
                        ],
                        &[
                            "ides",
                            "Description",
                            "Profile",
                            "0",
                            "string",
                            "text",
                            "0",
                            "0",
                            "",
                        ],
                        &[
                            "ireq", "Requires", "Profile", "-1", "unitList", "tech", "0", "0", "",
                        ],
                    ],
                ),
            ),
        ]),
        &[(
            r"Units\ItemStrings.txt",
            "[Z001]\nName=\"unterminated\nDescription=valid\nRequires=Y001,Y002",
        )],
    );
    assert_eq!(tables.unresolved().len(), 1);
    assert!(tables.object(Family::Unit, "Z001").is_err());
    let item = tables.object(Family::Item, "Z001").unwrap();
    assert_eq!(item.cell(SlkId::ItemData, "HP").unwrap().text, "23");
    assert_eq!(
        item.cell(SlkId::ItemData, "pickRandom")
            .unwrap()
            .typed(FieldKind::Bool)
            .unwrap(),
        Value::Boolean(true)
    );
    assert!(matches!(
        item.field(*b"inam"),
        Err(Refusal::MalformedValue { .. })
    ));
    assert_eq!(item.field(*b"ides").unwrap().value, Value::Text("valid"));
    let tech = item.fields_in_category("tech");
    assert_eq!(tech.len(), 1);
    assert_eq!(tech[0].1.as_ref().unwrap().value, Value::Text("Y001,Y002"));
}

#[test]
fn malformed_nonassignment_lines_are_retained_verbatim() {
    let parsed = parse_profile(b"[Z001]\n  invalid line  \nName=valid", "fixture.txt").unwrap();
    assert!(
        matches!(&parsed.unresolved[0], Unresolved::ProfileLine { line: 2, text, .. } if text == "  invalid line  ")
    );
    assert_eq!(parsed.sections["z001"]["name"][0].value, "valid");
}
