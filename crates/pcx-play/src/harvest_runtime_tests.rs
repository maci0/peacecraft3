use super::*;
use crate::playercolor_host::{DestructableCatalog, PlayerStateRequestBudget};
use pcx_objects::editor::SlkId;
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_objects::editor::{EditorReadLimits, Overlay};
use std::sync::Arc;

const ABILITY_HEADERS: &[&str] = &[
    "alias",
    "code",
    "levels",
    "reqLevel",
    "levelSkip",
    "DataA1",
    "DataB1",
    "DataC1",
    "Rng1",
    "Area1",
    "Dur1",
];

fn sylk(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
    let mut s = String::from("ID;PWXL;N;E\r\n");
    for (col, name) in headers.iter().enumerate() {
        s.push_str(&format!("C;X{};Y1;K\"{name}\"\r\n", col + 1));
    }
    for (r, row) in rows.iter().enumerate() {
        for (col, value) in row.iter().enumerate() {
            let value = if value.parse::<f64>().is_ok() {
                (*value).to_owned()
            } else {
                format!("\"{value}\"")
            };
            s.push_str(&format!("C;X{};Y{};K{value}\r\n", col + 1, r + 2));
        }
    }
    s.push_str("E\r\n");
    s.into_bytes()
}

fn read_tables(members: Vec<(String, Vec<u8>)>) -> DefaultTables {
    let owned: Vec<(&str, &[u8])> = members
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    DefaultTables::from_bytes(
        Identity {
            install: "synthetic-install".into(),
            edition_version: "synthetic-edition".into(),
        },
        &owned,
        &[],
        EditorReadLimits {
            max_listed_paths: 8,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 128 << 10,
        },
    )
    .unwrap()
}

fn harvest_tables() -> Arc<DefaultTables> {
    let ability_rows: &[&[&str]] = &[
        &[
            "Ahar", "Ahar", "1", "0", "0", "9", "20", "30", "200", "600", "4",
        ],
        &["Argl", "Artn", "1", "0", "0", "1", "1", "-", "-", "-", "-"],
    ];
    let metadata_rows: &[&[&str]] = &[
        &[
            "Har1",
            "Data",
            "AbilityData",
            "-1",
            "4",
            "1",
            "0",
            "Ahar",
            "-",
            "int",
            "stats",
        ],
        &[
            "Har2",
            "Data",
            "AbilityData",
            "-1",
            "4",
            "2",
            "0",
            "Ahar",
            "-",
            "int",
            "stats",
        ],
        &[
            "Har3",
            "Data",
            "AbilityData",
            "-1",
            "4",
            "3",
            "0",
            "Ahar",
            "-",
            "int",
            "stats",
        ],
        &[
            "adur",
            "Dur",
            "AbilityData",
            "-1",
            "4",
            "0",
            "0",
            "Ahar",
            "-",
            "unreal",
            "stats",
        ],
        &[
            "aran",
            "Rng",
            "AbilityData",
            "-1",
            "4",
            "0",
            "0",
            "Ahar",
            "-",
            "unreal",
            "stats",
        ],
        &[
            "aare",
            "Area",
            "AbilityData",
            "-1",
            "4",
            "0",
            "0",
            "Ahar",
            "-",
            "unreal",
            "stats",
        ],
        &[
            "Arg1",
            "Data",
            "AbilityData",
            "-1",
            "4",
            "1",
            "0",
            "Argl",
            "-",
            "bool",
            "stats",
        ],
        &[
            "Arg2",
            "Data",
            "AbilityData",
            "-1",
            "4",
            "2",
            "0",
            "Argl",
            "-",
            "bool",
            "stats",
        ],
    ];
    Arc::new(read_tables(vec![
        (
            SlkId::AbilityData.path().to_owned(),
            sylk(ABILITY_HEADERS, ability_rows),
        ),
        (
            SlkId::AbilityMetaData.path().to_owned(),
            sylk(
                &[
                    "ID",
                    "field",
                    "slk",
                    "index",
                    "repeat",
                    "data",
                    "appendIndex",
                    "useSpecific",
                    "notSpecific",
                    "type",
                    "category",
                ],
                metadata_rows,
            ),
        ),
    ]))
}

fn tree_catalog() -> DestructableCatalog {
    let tables = read_tables(vec![
        (
            SlkId::DestructableData.path().to_owned(),
            sylk(
                &["DestructableID", "HP", "targType"],
                &[&["LTlt", "80", "tree,ground"]],
            ),
        ),
        (
            SlkId::DestructableMetaData.path().to_owned(),
            sylk(
                &["ID", "field", "slk", "type", "index", "category"],
                &[
                    &["Ahpv", "HP", "DestructableData", "int", "0", "stats"],
                    &[
                        "Atgt",
                        "targType",
                        "DestructableData",
                        "string",
                        "0",
                        "combat",
                    ],
                ],
            ),
        ),
    ]);
    DestructableCatalog::new(Arc::new(tables), Overlay::default(), Overlay::default())
}

fn harvest_rules() -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    use crate::camera_margins::SourceWindow;
    use crate::neutral_player::sha256;
    let (mut image, mut binding, budget) = crate::order_registry::tests::fixture([431, 744]);
    let mut initializer = image[0x220..0x220 + binding.initializer.len as usize].to_vec();
    assert_eq!(&initializer[initializer.len() - 2..], &[0x5e, 0xc3]);
    initializer.truncate(initializer.len() - 2);
    for (at, id) in [(0x680usize, 900i32), (0x6A0, 901)] {
        let name_va = 0x301000 + (at as u32 - 0x200);
        let mut record = vec![0x68];
        record.extend_from_slice(&name_va.to_le_bytes());
        record.push(0x68);
        record.extend_from_slice(&id.to_le_bytes());
        record.extend_from_slice(&[0x8b, 0xce, 0xe8]);
        let next = binding.initializer.va + initializer.len() as u32 + record.len() as u32 + 4;
        record.extend_from_slice(&(binding.register as i32 - next as i32).to_le_bytes());
        initializer.extend_from_slice(&record);
    }
    initializer.extend_from_slice(&[0x5e, 0xc3]);
    image[0x680..0x692].copy_from_slice(b"autoharvestlumber\0");
    image[0x6A0..0x6B0].copy_from_slice(b"returnresources\0");
    image[0x220..0x220 + initializer.len()].copy_from_slice(&initializer);
    binding.initializer.len = initializer.len() as u32;
    let mut put = |at: usize, prefix: &[u8], id: i32| {
        let mut b = prefix.to_vec();
        b.extend_from_slice(&id.to_le_bytes());
        image[at..at + b.len()].copy_from_slice(&b);
        SourceWindow {
            va: 0x301000 + at as u32 - 0x200,
            len: b.len() as u32,
            digest: sha256(&b),
        }
    };
    let hold = put(0x320, &[0x3d], 431);
    let stop = put(0x330, &[0x81, 0xfb], 744);
    let movement = put(0x340, &[0x3d], 1631);
    let patrol = put(0x350, &[0x3d], 900);
    binding.initializer.digest = sha256(&image[0x220..0x220 + binding.initializer.len as usize]);
    binding.image_sha256 = sha256(&image);
    let registry = Arc::new(
        crate::order_registry::Registry::read(&image, binding.clone(), budget, 8, 32).unwrap(),
    );
    let rules = crate::unit_activity::Rules::read(
        &image,
        crate::unit_activity::Binding {
            origin: binding.origin,
            image_sha256: binding.image_sha256,
            hold,
            stop,
            movement,
            patrol,
        },
        &registry,
        budget,
    )
    .unwrap();
    (registry, rules)
}

fn field(fourcc: [u8; 4], slk: SlkId, column: &str, text: &str) -> ([u8; 4], ObjectField) {
    (
        fourcc,
        ObjectField {
            metadata: Some(pcx_objects::editor::FieldDef {
                fourcc,
                from: slk,
                field: column.into(),
                slk: format!("{slk:?}"),
                index: -1,
                declared: "token".into(),
                kind: pcx_objects::editor::FieldKind::Token,
                category: String::new(),
                source: pcx_objects::editor::FieldSource::Column {
                    slk,
                    column: column.into(),
                },
                measured: true,
            }),
            value: Ok(pcx_sim::world::FieldValue::Cell(text.into())),
        },
    )
}

fn harvest_fields() -> Vec<([u8; 4], ObjectField)> {
    vec![
        field(*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar,Argl"),
        field(*b"ua1w", SlkId::UnitWeapons, "weapsOn", "2"),
        field(*b"ua1t", SlkId::UnitWeapons, "targs1", "ground"),
        field(*b"ua1c", SlkId::UnitWeapons, "cool1", "2"),
        field(*b"ua1d", SlkId::UnitWeapons, "dmgpt1", "0.5"),
        field(*b"ua1b", SlkId::UnitWeapons, "backSw1", "0.5"),
        field(*b"ua1r", SlkId::UnitWeapons, "rangeN1", "90"),
        field(*b"ua2t", SlkId::UnitWeapons, "targs2", "tree"),
        field(*b"ua2c", SlkId::UnitWeapons, "cool2", "2"),
        field(*b"ua2d", SlkId::UnitWeapons, "dmgpt2", "0.5"),
        field(*b"ua2b", SlkId::UnitWeapons, "backSw2", "0.5"),
        field(*b"ua2r", SlkId::UnitWeapons, "rangeN2", "60"),
    ]
}

fn placed_tree() -> crate::playercolor_host::PlacedDestructable {
    use crate::playercolor_host::{
        DestructableCreation, DestructableOrigin, DestructableState, PlacedDestructable,
        PlacedSource,
    };
    PlacedDestructable {
        creation: DestructableCreation {
            origin: DestructableOrigin::Placed { creation_number: 1 },
            objectid: i32::from_be_bytes(*b"LTlt"),
            x: 52.0,
            y: 32.0,
            z: None,
            face: 0.0,
            scale: 1.0,
            variation: 0,
            dead: false,
        },
        state: DestructableState {
            life: Some(80.0),
            max_life: Some(80.0),
            ..DestructableState::default()
        },
        source: PlacedSource {
            creation_number: 1,
            type_id: *b"LTlt",
            variation: 0,
            position: [52.0, 32.0, 0.0],
            angle: 0.0,
            scale: [1.0, 1.0, 1.0],
            flags: 0,
            life: 100,
        },
    }
}

fn open_harvest() -> (Vm, HandleId) {
    let mut h =
        host_fields_with_placed(65536, &harvest_fields(), &[placed_tree()], harvest_rules());
    h = h
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 8 })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules())
        .unwrap();
    h.admit_destructable_catalog(tree_catalog()).unwrap();
    let h = h
        .with_harvest(
            harvest_tables(),
            Overlay::default(),
            crate::playercolor_host::harvest_runtime::Budget::new(4, 64),
        )
        .unwrap();
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    for name in ["A", "B"] {
        let id = unit(&vm, name);
        vm.host()
            .inner
            .native("RemoveUnit", &[Value::Handle(id)])
            .unwrap();
    }
    let tree = vm
        .host()
        .inner
        .destructables_with_handles()
        .next()
        .expect("placed tree")
        .0;
    (vm, tree)
}

fn create_unit(vm: &mut Vm, rawcode: [u8; 4], x: f32, y: f32) -> HandleId {
    let player = vm.host().inner.native("Player", &[Value::Int(0)]).unwrap();
    let Value::Handle(id) = vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                player,
                Value::Int(i32::from_be_bytes(rawcode)),
                Value::Real(x),
                Value::Real(y),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit handle")
    };
    id
}

#[test]
fn autoharvest_claims_a_tree_deposits_source_capacity_and_repeats() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let player = vm.host().inner.native("Player", &[Value::Int(0)]).unwrap();
    let Value::Handle(player) = player else {
        panic!("player handle")
    };
    let lumber = vm
        .host()
        .inner
        .player_resource_value(player, "PLAYER_STATE_RESOURCE_LUMBER")
        .unwrap()
        .unwrap();
    let gathered = vm
        .host()
        .inner
        .player_resource_value(player, "PLAYER_STATE_LUMBER_GATHERED")
        .unwrap()
        .unwrap();

    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrderById",
                &[Value::Handle(worker), Value::Int(900)]
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(worker)])
            .unwrap(),
        Value::Int(900)
    );
    assert_eq!(
        vm.host()
            .inner
            .player_resource_value(player, "PLAYER_STATE_RESOURCE_LUMBER")
            .unwrap(),
        Some(lumber)
    );

    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    assert_eq!(
        vm.host()
            .inner
            .player_resource_value(player, "PLAYER_STATE_RESOURCE_LUMBER")
            .unwrap(),
        Some(lumber)
    );
    assert_eq!(
        vm.host()
            .inner
            .destructable_state(tree)
            .unwrap()
            .life
            .unwrap(),
        62.0
    );

    tick(&mut vm, 4.5);
    assert_eq!(
        vm.host()
            .inner
            .destructable_state(tree)
            .unwrap()
            .life
            .unwrap(),
        53.0
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(worker)])
            .unwrap(),
        Value::Int(901)
    );
    assert_eq!(
        vm.host()
            .inner
            .player_resource_value(player, "PLAYER_STATE_RESOURCE_LUMBER")
            .unwrap(),
        Some(lumber)
    );

    tick(&mut vm, 5.5);
    assert_eq!(
        vm.host()
            .inner
            .player_resource_value(player, "PLAYER_STATE_RESOURCE_LUMBER")
            .unwrap(),
        Some(lumber + 20)
    );
    assert_eq!(
        vm.host()
            .inner
            .player_resource_value(player, "PLAYER_STATE_LUMBER_GATHERED")
            .unwrap(),
        Some(gathered + 20)
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(worker)])
            .unwrap(),
        Value::Int(900)
    );
    let depot_record = vm.host().inner.executed_unit(depot).unwrap();
    assert_eq!(depot_record.owner, player);

    tick(&mut vm, 6.5);
    tick(&mut vm, 7.5);
    assert_eq!(
        vm.host()
            .inner
            .destructable_state(tree)
            .unwrap()
            .life
            .unwrap(),
        44.0
    );
}

#[test]
fn returnresources_routes_a_loaded_worker_to_an_authored_lumber_depot() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let player = vm.host().inner.native("Player", &[Value::Int(0)]).unwrap();
    let Value::Handle(player) = player else {
        panic!("player handle")
    };
    let lumber = vm
        .host()
        .inner
        .player_resource_value(player, "PLAYER_STATE_RESOURCE_LUMBER")
        .unwrap()
        .unwrap();
    let _gathered = vm
        .host()
        .inner
        .player_resource_value(player, "PLAYER_STATE_LUMBER_GATHERED")
        .unwrap()
        .unwrap();
    vm.host()
        .inner
        .native(
            "IssueImmediateOrderById",
            &[Value::Handle(worker), Value::Int(900)],
        )
        .unwrap();
    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    tick(&mut vm, 4.5);
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrderById",
                &[Value::Handle(worker), Value::Int(901)]
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        vm.host()
            .inner
            .player_resource_value(player, "PLAYER_STATE_RESOURCE_LUMBER")
            .unwrap(),
        Some(lumber)
    );
    tick(&mut vm, 5.5);
    assert_eq!(
        vm.host()
            .inner
            .player_resource_value(player, "PLAYER_STATE_RESOURCE_LUMBER")
            .unwrap(),
        Some(lumber + 20)
    );
    assert_eq!(
        vm.host()
            .inner
            .destructable_state(tree)
            .unwrap()
            .life
            .unwrap(),
        53.0
    );
}
