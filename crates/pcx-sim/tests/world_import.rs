use pcx_map::{Map, MapInfo, Pathing, RandomUnit, UnitPlacement, Units};
use pcx_objects::editor::{Editor, FieldSource, ObjectKind, Overlay, SlkId};
use pcx_objects::{Fourcc, ObjectFile};
use pcx_sim::world::{
    Definition, FieldProblem, FieldValue, ImportError, ImportedWorld, MetadataCatalog,
    ObjectCatalog, ObjectField, RecordProblem, ResolutionProblem, Source,
};
use std::collections::BTreeMap;

fn source() -> Source {
    Source {
        install: "synthetic".into(),
        edition: "synthetic schema".into(),
        map: "scenario".into(),
    }
}
fn info() -> MapInfo {
    // Empty version-18 sections, each authored explicitly in this fixture.
    let mut bytes = Vec::new();
    for word in [18u32, 0, 0] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend([0; 4]); // Four NUL strings.
    bytes.extend([0; 32 + 16 + 8 + 4]); // Camera, bounds and flags.
    bytes.push(0); // Tileset byte.
    bytes.extend(0u32.to_le_bytes()); // Loading word.
    bytes.extend([0; 3]);
    bytes.extend(0u32.to_le_bytes()); // Prologue word.
    bytes.extend([0; 3]);
    bytes.extend([0; 20]); // Five empty trailing counted sections.
    MapInfo::parse(&bytes).expect("synthetic map info")
}
fn pathing() -> Pathing {
    Pathing {
        version: 0,
        width: 2,
        height: 1,
        cells: vec![0x12, 0x84],
    }
}
fn placement(rawcode: Fourcc) -> UnitPlacement {
    UnitPlacement {
        type_id: rawcode,
        variation: 7,
        position: [1.25, -2.5, 3.75],
        angle: 0.125,
        scale: [1.5, 2.5, 3.5],
        flags: 9,
        control_word: u16::from_le_bytes((-2i16).to_le_bytes()),
        state_word: 0x1234,
        life: -1,
        mana: -1,
        item_table: None,
        item_sets: vec![],
        post_drop_word: 0,
        post_drop_float: -1.0,
        progression_word: 1,
        progression_words: None,
        inventory: vec![],
        abilities: vec![],
        random: RandomUnit::None,
        tail_words: [u32::MAX; 2],
        creation_number: 42,
    }
}
struct Catalog(BTreeMap<Fourcc, Definition>, std::cell::Cell<usize>);
impl ObjectCatalog for Catalog {
    fn resolve(&self, rawcode: Fourcc) -> Result<Definition, ResolutionProblem> {
        self.1.set(self.1.get() + 1);
        self.0.get(&rawcode).cloned().ok_or_else(|| {
            ResolutionProblem::MissingRawcode(String::from_utf8_lossy(&rawcode).into_owned())
        })
    }
}
fn definition(rawcode: Fourcc, kind: ObjectKind) -> Definition {
    Definition {
        rawcode,
        base_rawcode: String::from_utf8_lossy(&rawcode).into_owned(),
        kind,
        fields: BTreeMap::from([(
            *b"STAT",
            ObjectField {
                metadata: None,
                value: Ok(FieldValue::Cell("137".into())),
            },
        )]),
        modifications: vec![],
    }
}
#[test]
fn identity_positions_owner_and_supplied_stats_survive_without_tick_conversion() {
    let unit = definition(*b"UNIT", ObjectKind::Unit);
    let item = definition(*b"ITEM", ObjectKind::Item);
    let catalog = Catalog(
        BTreeMap::from([(*b"UNIT", unit), (*b"ITEM", item)]),
        std::cell::Cell::new(0),
    );
    let placements = Units {
        version: 8,
        subversion: 11,
        instances: vec![
            placement(*b"UNIT"),
            placement(*b"ITEM"),
            placement(*b"UNIT"),
        ],
    };
    let world = ImportedWorld::from_authored(
        source(),
        info(),
        pathing(),
        &placements,
        &catalog,
        &[*b"STAT"],
    )
    .unwrap();
    assert_eq!(catalog.1.get(), 2, "resolve each identity once");
    assert_eq!(world.definitions.len(), 2);
    assert_eq!(world.pathing.cells, [0x12, 0x84]);
    for (ordinal, record) in world.records.iter().enumerate() {
        assert_eq!(record.ordinal, ordinal);
        assert_eq!(record.placement, placements.instances[ordinal]);
        assert!(record.problems.is_empty());
    }
    assert_eq!(world.records[0].owner, Some(-2));
    assert_eq!(world.records[1].owner, None);
    assert_eq!(world.records[2].definition, world.records[0].definition);
    assert_eq!(
        world.definitions[0].fields[b"STAT"].value,
        Ok(FieldValue::Cell("137".into()))
    );
}
#[test]
fn unsupported_records_and_required_missing_fields_are_named_and_retained() {
    let mut known = definition(*b"UNIT", ObjectKind::Unit);
    known.fields.insert(
        *b"STAT",
        ObjectField {
            metadata: None,
            value: Err(FieldProblem::Missing),
        },
    );
    let catalog = Catalog(BTreeMap::from([(*b"UNIT", known)]), std::cell::Cell::new(0));
    let mut unclassified = placement(*b"UNIT");
    unclassified.random = RandomUnit::Unclassified(641);
    let placements = Units {
        version: 8,
        subversion: 11,
        instances: vec![placement(*b"MISS"), unclassified],
    };
    let world = ImportedWorld::from_authored(
        source(),
        info(),
        pathing(),
        &placements,
        &catalog,
        &[*b"STAT", *b"NONE"],
    )
    .unwrap();
    assert_eq!(world.records.len(), 2);
    assert_eq!(
        world.records[0].definition,
        Err(ResolutionProblem::MissingRawcode("MISS".into()))
    );
    assert_eq!(
        world.records[1].placement.random,
        RandomUnit::Unclassified(641)
    );
    assert!(
        world.records[1]
            .problems
            .contains(&RecordProblem::RequiredField {
                field: *b"STAT",
                problem: FieldProblem::Missing
            })
    );
    assert!(
        world.records[1]
            .problems
            .contains(&RecordProblem::MissingRequiredField(*b"NONE"))
    );
    assert!(
        world.records[1]
            .problems
            .contains(&RecordProblem::UnverifiedSelection(
                RandomUnit::Unclassified(641)
            ))
    );
}
#[test]
fn invalid_placement_and_absent_provenance_are_explicit() {
    let catalog = Catalog(
        BTreeMap::from([(*b"UNIT", definition(*b"UNIT", ObjectKind::Unit))]),
        std::cell::Cell::new(0),
    );
    let mut record = placement(*b"UNIT");
    record.position[1] = f32::INFINITY;
    let units = Units {
        version: 8,
        subversion: 11,
        instances: vec![record],
    };
    let world =
        ImportedWorld::from_authored(source(), info(), pathing(), &units, &catalog, &[]).unwrap();
    assert!(
        world.records[0]
            .problems
            .contains(&RecordProblem::NonFinitePlacement)
    );
    let mut missing = source();
    missing.edition.clear();
    assert!(matches!(
        ImportedWorld::from_authored(missing, info(), pathing(), &units, &catalog, &[]),
        Err(ImportError::MissingSource)
    ));
}

#[test]
#[ignore = "requires PEACECRAFT3_WORLD_INSTALL, PEACECRAFT3_WORLD_MAP and PEACECRAFT3_WORLD_EDITION"]
fn original_map_imports_with_named_unsupported_records_and_asset_stats() {
    let install =
        std::env::var("PEACECRAFT3_WORLD_INSTALL").expect("set PEACECRAFT3_WORLD_INSTALL");
    let map_path = std::env::var("PEACECRAFT3_WORLD_MAP")
        .expect("set PEACECRAFT3_WORLD_MAP to a local path or original archive member");
    let edition = std::env::var("PEACECRAFT3_WORLD_EDITION")
        .expect("set explicit source identity PEACECRAFT3_WORLD_EDITION");
    let mut archives = pcx_mpq::MpqSet::open_wc3(&install).expect("open original install");
    let editor = Editor::open_in(&mut archives, std::path::Path::new(&install))
        .expect("read original metadata");
    let bytes = if std::path::Path::new(&map_path).is_file() {
        std::fs::read(&map_path).expect("read original map")
    } else {
        archives.extract(&map_path).expect("extract original map")
    };
    let map = Map::parse(&bytes).expect("parse original map");
    let units = read_overlay(&map, "war3map.w3u");
    let items = read_overlay(&map, "war3map.w3t");
    let catalog = MetadataCatalog {
        editor: &editor,
        units: &units,
        items: &items,
    };
    let source = Source {
        install,
        edition,
        map: map_path.clone(),
    };
    let world = ImportedWorld::from_map(&map, source, &catalog, &[])
        .expect("import required authored members");
    let placements = map.units().unwrap();
    assert!(
        !world.records.is_empty(),
        "original fixture has no placements"
    );
    assert_eq!(world.records.len(), placements.instances.len());
    let mut resolved = 0;
    let mut hp_cells = 0;
    let mut kinds = [0usize; 2];
    let mut overrides = 0usize;
    for record in &world.records {
        assert_eq!(record.placement, placements.instances[record.ordinal]);
        match &record.definition {
            Err(problem) => eprintln!(
                "{map_path}: ordinal={} creation={} rawcode={:?} unsupported={problem:?}",
                record.ordinal,
                record.placement.creation_number,
                String::from_utf8_lossy(&record.placement.type_id)
            ),
            Ok(index) => {
                resolved += 1;
                let definition = &world.definitions[*index];
                assert_catalog_values(&editor, definition);
                assert_eq!(definition.rawcode, record.placement.type_id);
                match definition.kind {
                    ObjectKind::Unit => kinds[0] += 1,
                    ObjectKind::Item => kinds[1] += 1,
                    other => panic!("unexpected placed object kind {other:?}"),
                }
                overrides += definition
                    .fields
                    .values()
                    .filter(|field| matches!(field.value, Ok(FieldValue::Override(_))))
                    .count();
                for field in definition.fields.values() {
                    if let Some(metadata) = &field.metadata
                        && let FieldSource::Column { slk, column } = &metadata.source
                        && matches!(slk, SlkId::UnitBalance | SlkId::ItemData)
                        && column == "HP"
                        && field.value.is_ok()
                    {
                        hp_cells += 1;
                    }
                }
            }
        }
        for problem in &record.problems {
            eprintln!(
                "{map_path}: ordinal={} rawcode={:?} problem={problem:?}",
                record.ordinal,
                String::from_utf8_lossy(&record.placement.type_id)
            );
        }
    }
    assert!(
        resolved > 0 && hp_cells > 0,
        "fixture must resolve identities and authored HP cells"
    );
    eprintln!(
        "source={:?} placements={} unique_definitions={} resolved_records={resolved} authored_hp_cells={hp_cells} units={} items={} resolved_override_fields={overrides}",
        world.source,
        world.records.len(),
        world.definitions.len(),
        kinds[0],
        kinds[1]
    );
}

#[test]
#[ignore = "requires PEACECRAFT3_WORLD_INSTALL and PEACECRAFT3_WORLD_EDITION; derives an item-bearing original map"]
fn original_preplaced_items_resolve_from_item_metadata() {
    let install =
        std::env::var("PEACECRAFT3_WORLD_INSTALL").expect("set PEACECRAFT3_WORLD_INSTALL");
    let edition =
        std::env::var("PEACECRAFT3_WORLD_EDITION").expect("set PEACECRAFT3_WORLD_EDITION");
    let mut archives = pcx_mpq::MpqSet::open_wc3(&install).expect("open original install");
    let editor = Editor::open_in(&mut archives, std::path::Path::new(&install))
        .expect("read original metadata");
    let mut names = archives.file_list().expect("read original map names");
    names.sort();
    for name in names {
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(".w3m") && !lower.ends_with(".w3x") {
            continue;
        }
        let bytes = archives
            .extract(&name)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let map = Map::parse(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        let placements = map
            .units()
            .unwrap_or_else(|error| panic!("{name}: placements: {error}"));
        if !placements.instances.iter().any(|record| {
            std::str::from_utf8(&record.type_id)
                .ok()
                .and_then(|rawcode| editor.resolve(rawcode))
                == Some(ObjectKind::Item)
        }) {
            continue;
        }
        let units = read_overlay(&map, "war3map.w3u");
        let items = read_overlay(&map, "war3map.w3t");
        let catalog = MetadataCatalog {
            editor: &editor,
            units: &units,
            items: &items,
        };
        let world = ImportedWorld::from_map(
            &map,
            Source {
                install,
                edition,
                map: name.clone(),
            },
            &catalog,
            &[],
        )
        .expect("import item-bearing original map");
        let mut imported_items = 0;
        let mut hp_cells = 0;
        for record in &world.records {
            let Ok(index) = record.definition else {
                eprintln!(
                    "{name}: ordinal={} rawcode={:?} unsupported={:?}",
                    record.ordinal,
                    String::from_utf8_lossy(&record.placement.type_id),
                    record.definition
                );
                continue;
            };
            let definition = &world.definitions[index];
            assert_catalog_values(&editor, definition);
            if definition.kind != ObjectKind::Item {
                continue;
            }
            imported_items += 1;
            assert_eq!(record.placement, placements.instances[record.ordinal]);
            assert_eq!(record.owner, None);
            for field in definition.fields.values() {
                if let Some(metadata) = &field.metadata
                    && let FieldSource::Column {
                        slk: SlkId::ItemData,
                        column,
                    } = &metadata.source
                    && column == "HP"
                    && field.value.is_ok()
                {
                    hp_cells += 1;
                }
            }
        }
        assert!(
            imported_items > 0 && hp_cells > 0,
            "actual original item identities and stats must resolve"
        );
        eprintln!(
            "source={:?} placements={} imported_items={imported_items} authored_item_hp_cells={hp_cells}",
            world.source,
            world.records.len()
        );
        return;
    }
    panic!("no item-bearing original map found within archive map-list search");
}

fn read_overlay(map: &Map<'_>, name: &str) -> Overlay {
    match map.extract(name) {
        Ok(bytes) => Overlay::of(
            &ObjectFile::parse(name, &bytes).unwrap_or_else(|error| panic!("{name}: {error}")),
        ),
        Err(pcx_mpq::Error::NotFound(_)) => Overlay::default(),
        Err(error) => panic!("{name}: {error}"),
    }
}

fn assert_catalog_values(editor: &Editor, definition: &Definition) {
    for (&fourcc, field) in &definition.fields {
        match (&field.metadata, &field.value) {
            (Some(metadata), Ok(FieldValue::Cell(actual))) => {
                if let FieldSource::Column { slk, column } = &metadata.source {
                    let table = editor.table(*slk).expect("source table for resolved field");
                    let row = editor
                        .row(*slk, &definition.base_rawcode)
                        .expect("source object row");
                    let column = table.column_of(column).expect("metadata column exists");
                    assert_eq!(Some(actual.as_str()), table.get(row, column));
                }
            }
            (_, Ok(FieldValue::Override(actual))) => {
                let authored = definition
                    .modifications
                    .iter()
                    .rev()
                    .find(|entry| entry.field == fourcc && entry.selectors.is_none())
                    .expect("resolved override must have an authored record");
                assert_eq!(actual, &authored.value);
            }
            _ => {}
        }
    }
}
