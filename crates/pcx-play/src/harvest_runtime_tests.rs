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
    harvest_tables_for("9", "20", "4")
}

fn harvest_tables_for(
    damage: &'static str,
    capacity: &'static str,
    duration: &'static str,
) -> Arc<DefaultTables> {
    harvest_tables_for_range(damage, capacity, duration, "200")
}

fn harvest_tables_for_range(
    damage: &'static str,
    capacity: &'static str,
    duration: &'static str,
    range: &'static str,
) -> Arc<DefaultTables> {
    let ability_rows: &[&[&str]] = &[
        &[
            "Ahar", "Ahar", "1", "0", "0", damage, capacity, "30", range, "600", duration,
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
        (
            SlkId::UpgradeData.path().to_owned(),
            sylk(
                &["upgradeid", "maxlevel", "effect1", "base1", "mod1"],
                &[&["Rhlh", "2", "rlum", "10", "10"]],
            ),
        ),
    ]))
}

fn tree_catalog() -> DestructableCatalog {
    tree_catalog_for("80")
}

fn tree_catalog_for(hp: &'static str) -> DestructableCatalog {
    let tables = read_tables(vec![
        (
            SlkId::DestructableData.path().to_owned(),
            sylk(
                &["DestructableID", "HP", "targType"],
                &[&["LTlt", hp, "tree,ground"]],
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
    let bootstrap_va = 0x301500u32;
    let mut bootstrap = vec![0x68];
    bootstrap.extend_from_slice(&0x301400u32.to_le_bytes());
    bootstrap.extend_from_slice(&[0xff, 0x35]);
    bootstrap.extend_from_slice(&0x301350u32.to_le_bytes());
    for target in [0x301100u32, 0x301020u32] {
        bootstrap.push(0xe8);
        let next = bootstrap_va + bootstrap.len() as u32 + 4;
        bootstrap.extend_from_slice(&(target as i32 - next as i32).to_le_bytes());
    }
    image[0x700..0x700 + bootstrap.len()].copy_from_slice(&bootstrap);
    binding.bootstrap.va = bootstrap_va;
    binding.bootstrap.len = bootstrap.len() as u32;
    binding.bootstrap.digest = sha256(&bootstrap);
    let mut initializer = image[0x220..0x220 + binding.initializer.len as usize].to_vec();
    assert_eq!(&initializer[initializer.len() - 2..], &[0x5e, 0xc3]);
    initializer.truncate(initializer.len() - 2);
    for (at, id) in [
        (0x680usize, 900i32),
        (0x6A0, 901),
        (0x6C0, 902),
        (0x6E0, 903),
    ] {
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
    image[0x6C0..0x6C8].copy_from_slice(b"harvest\0");
    image[0x6E0..0x6F1].copy_from_slice(b"resumeharvesting\0");
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
        field(*b"upgr", SlkId::UnitBalance, "upgradeList", "Rhlh"),
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

fn harvest_fields_without_tree_weapon() -> Vec<([u8; 4], ObjectField)> {
    let mut fields = harvest_fields();
    fields.retain(|(fourcc, _)| !fourcc.starts_with(b"ua2"));
    let (_, weapon_mask) = fields
        .iter_mut()
        .find(|(fourcc, _)| fourcc == b"ua1w")
        .expect("weapon mask");
    weapon_mask.value = Ok(pcx_sim::world::FieldValue::Cell("0".into()));
    fields
}

fn placed_tree() -> crate::playercolor_host::PlacedDestructable {
    placed_tree_with_life(80.0)
}

fn placed_tree_with_life(life: f32) -> crate::playercolor_host::PlacedDestructable {
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
            life: Some(life),
            max_life: Some(life),
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

fn placed_tree_at(x: f32, y: f32) -> crate::playercolor_host::PlacedDestructable {
    let mut tree = placed_tree();
    tree.creation.x = x;
    tree.creation.y = y;
    tree.source.position = [x, y, 0.0];
    tree
}

fn open_harvest() -> (Vm, HandleId) {
    open_harvest_with(
        harvest_tables(),
        tree_catalog(),
        placed_tree(),
        harvest_fields(),
    )
}

fn open_harvest_with(
    tables: Arc<DefaultTables>,
    catalog: DestructableCatalog,
    tree: crate::playercolor_host::PlacedDestructable,
    fields: Vec<([u8; 4], ObjectField)>,
) -> (Vm, HandleId) {
    let mut h = host_fields_with_placed(65536, &fields, &[tree], harvest_rules());
    h = h
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 8 })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules())
        .unwrap()
        .with_player_tech_researched(crate::playercolor_host::TechLimitBudget { max_records: 4 })
        .unwrap();
    h.admit_destructable_catalog(catalog).unwrap();
    let mut h = h
        .with_harvest(
            tables,
            Overlay::default(),
            Overlay::default(),
            crate::playercolor_host::harvest_runtime::Budget::new(4, 64),
        )
        .unwrap();
    let model = crate::unit_model::tests::model();
    let mut bytes = model.bytes.to_vec();
    let at = bytes
        .windows(12)
        .position(|b| b == b"Stand Defend")
        .expect("fixture sequence");
    bytes[at..at + 80].fill(0);
    bytes[at..at + 6].copy_from_slice(b"Attack");
    let model = Arc::new(
        crate::unit_model::Model::read(
            model.origin.clone(),
            "synthetic-attack.mdx".into(),
            bytes.into(),
            4096,
        )
        .unwrap(),
    );
    h.admit_unit_models(move |_| Ok(Some(Arc::clone(&model))))
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
        Value::Int(902)
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
        Value::Int(903)
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
        Value::Int(902)
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
fn active_lumber_upgrade_raises_the_deposited_load() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let player = match vm.host().inner.native("Player", &[Value::Int(0)]).unwrap() {
        Value::Handle(player) => player,
        _ => panic!("player handle"),
    };
    let lumber = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
    vm.host()
        .inner
        .native(
            "SetPlayerTechResearched",
            &[
                Value::Handle(player),
                Value::Int(i32::from_be_bytes(*b"Rhlh")),
                Value::Int(1),
            ],
        )
        .unwrap();
    start_harvest(&mut vm, worker);

    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    tick(&mut vm, 4.5);
    assert_eq!(tree_life(&mut vm, tree), 53.0);
    assert_eq!(current_order(&mut vm, worker), 902);

    tick(&mut vm, 6.5);
    assert_eq!(tree_life(&mut vm, tree), 44.0);
    assert_eq!(current_order(&mut vm, worker), 903);

    tick(&mut vm, 7.5);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        lumber + 30
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered + 30
    );
}

#[test]
fn harvest_orders_are_four_distinct_registry_identities() {
    let (registry, _) = harvest_rules();
    let orders = [
        ("autoharvestlumber", 900),
        ("returnresources", 901),
        ("harvest", 902),
        ("resumeharvesting", 903),
    ];
    for (name, id) in orders {
        assert_eq!(registry.id(name), Some(id), "{name}");
        assert_eq!(registry.name(id), Some(name), "{id}");
    }
    let mut ids: Vec<_> = orders.iter().map(|(_, id)| *id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 4);
}

#[test]
fn stop_preserves_partial_cargo_across_reissue_and_manual_return() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for("9", "18", "4"),
        tree_catalog(),
        placed_tree(),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    assert_eq!(tree_life(&mut vm, tree), 62.0);

    issue_id(&mut vm, worker, 744);
    assert_eq!(current_order(&mut vm, worker), 0);
    start_harvest(&mut vm, worker);
    assert_eq!(current_order(&mut vm, worker), 902);
    issue_id(&mut vm, worker, 901);
    assert_eq!(current_order(&mut vm, worker), 901);

    tick(&mut vm, 3.5);
    assert_eq!(current_order(&mut vm, worker), 0);
    assert_eq!(tree_life(&mut vm, tree), 62.0);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 18
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered + 18
    );
}

#[test]
fn a_move_order_preserves_partial_cargo_across_reissue_and_manual_return() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for("9", "18", "4"),
        tree_catalog(),
        placed_tree(),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    assert_eq!(tree_life(&mut vm, tree), 62.0);

    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssuePointOrderById",
                &[
                    Value::Handle(worker),
                    Value::Int(1631),
                    Value::Real(10.0),
                    Value::Real(10.0),
                ],
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(current_order(&mut vm, worker), 1631);
    start_harvest(&mut vm, worker);
    assert_eq!(current_order(&mut vm, worker), 902);
    issue_id(&mut vm, worker, 901);

    tick(&mut vm, 3.5);
    assert_eq!(current_order(&mut vm, worker), 0);
    assert_eq!(tree_life(&mut vm, tree), 62.0);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 18
    );
}

#[test]
fn an_active_upgrade_applied_mid_cycle_raises_the_load_before_deposit() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let player = match vm.host().inner.native("Player", &[Value::Int(0)]).unwrap() {
        Value::Handle(player) => player,
        _ => panic!("player handle"),
    };
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    assert_eq!(tree_life(&mut vm, tree), 62.0);

    vm.host()
        .inner
        .native(
            "SetPlayerTechResearched",
            &[
                Value::Handle(player),
                Value::Int(i32::from_be_bytes(*b"Rhlh")),
                Value::Int(1),
            ],
        )
        .unwrap();
    tick(&mut vm, 4.5);
    assert_eq!(tree_life(&mut vm, tree), 53.0);
    assert_eq!(current_order(&mut vm, worker), 902);
    tick(&mut vm, 6.5);
    assert_eq!(tree_life(&mut vm, tree), 44.0);
    assert_eq!(current_order(&mut vm, worker), 903);
    tick(&mut vm, 7.5);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 30
    );
}

#[test]
fn tree_reach_is_the_ahar_range_without_a_plus_one_heuristic() {
    for (range, expected) in [("20", 71.0), ("19", 80.0)] {
        let (mut vm, tree) = open_harvest_with(
            harvest_tables_for_range("9", "20", "4", range),
            tree_catalog(),
            placed_tree(),
            harvest_fields(),
        );
        let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
        let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
        start_harvest(&mut vm, worker);
        tick(&mut vm, 0.5);
        assert_eq!(tree_life(&mut vm, tree), expected, "Ahar range {range}");
    }
}

#[test]
fn a_missing_tree_weapon_refuses_harvest_without_changing_state() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables(),
        tree_catalog(),
        placed_tree(),
        harvest_fields_without_tree_weapon(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");

    let error = vm
        .host()
        .inner
        .native(
            "IssueImmediateOrderById",
            &[Value::Handle(worker), Value::Int(900)],
        )
        .unwrap_err();
    assert!(error.contains("tree-targeting weapon"), "{error}");
    assert_eq!(current_order(&mut vm, worker), 0);
    assert_eq!(tree_life(&mut vm, tree), 80.0);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"), bank);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered
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

fn player_bank(vm: &mut Vm, state: &str) -> i32 {
    let player = match vm.host().inner.native("Player", &[Value::Int(0)]).unwrap() {
        Value::Handle(player) => player,
        _ => panic!("player handle"),
    };
    vm.host()
        .inner
        .player_resource_value(player, state)
        .unwrap()
        .unwrap()
}

fn tree_life(vm: &mut Vm, tree: HandleId) -> f32 {
    vm.host()
        .inner
        .destructable_state(tree)
        .unwrap()
        .life
        .unwrap()
}

fn current_order(vm: &mut Vm, unit: HandleId) -> i32 {
    match vm
        .host()
        .inner
        .native("GetUnitCurrentOrder", &[Value::Handle(unit)])
        .unwrap()
    {
        Value::Int(order) => order,
        other => panic!("unexpected order value: {other:?}"),
    }
}

fn start_harvest(vm: &mut Vm, worker: HandleId) {
    assert_eq!(issue_id(vm, worker, 900), Value::Bool(true));
}

fn issue_id(vm: &mut Vm, worker: HandleId, order: i32) -> Value {
    vm.host()
        .inner
        .native(
            "IssueImmediateOrderById",
            &[Value::Handle(worker), Value::Int(order)],
        )
        .unwrap()
}

fn move_to(vm: &mut Vm, unit: HandleId, x: f32, y: f32) -> Value {
    vm.host()
        .inner
        .native(
            "IssuePointOrderById",
            &[
                Value::Handle(unit),
                Value::Int(1631),
                Value::Real(x),
                Value::Real(y),
            ],
        )
        .unwrap()
}

fn animation_name(vm: &mut Vm, unit: HandleId) -> String {
    let animation = vm
        .host()
        .inner
        .executed_unit(unit)
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    animation.model.sequences[animation.sequence.unwrap()]
        .name
        .clone()
}

#[derive(Debug, PartialEq)]
struct HarvestState {
    route: Option<Vec<[f32; 2]>>,
    animation: String,
    order: i32,
    cargo: Option<f32>,
}

fn harvest_state(vm: &mut Vm, unit: HandleId) -> HarvestState {
    let route = vm.host().inner.activity_motion_path(unit);
    let animation = animation_name(vm, unit);
    let order = current_order(vm, unit);
    let cargo = vm.host().inner.harvest_cargo(unit);
    HarvestState {
        route,
        animation,
        order,
        cargo,
    }
}

fn advance(
    vm: &mut Vm,
    now: f64,
    mut work: usize,
) -> Result<usize, crate::playercolor_host::Error> {
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(now, &mut 1000).unwrap().is_none());
    h.advance_unit_activity(now, &mut work)
}

fn remove_unit(vm: &mut Vm, unit: HandleId) {
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(unit)])
        .unwrap();
}

#[test]
fn a_work_budget_refusal_defers_the_due_hit_and_keeps_progress_coherent() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    start_harvest(&mut vm, worker);

    assert!(advance(&mut vm, 0.5, 1).is_err());
    assert_eq!(tree_life(&mut vm, tree), 80.0);

    advance(&mut vm, 0.5, 64).unwrap();
    assert_eq!(tree_life(&mut vm, tree), 71.0);

    advance(&mut vm, 0.5, 64).unwrap();
    assert_eq!(tree_life(&mut vm, tree), 71.0);

    advance(&mut vm, 2.5, 64).unwrap();
    assert_eq!(tree_life(&mut vm, tree), 62.0);
}

#[test]
fn a_capacity_hit_retries_the_depot_route_without_chopping_again() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for("9", "18", "4"),
        tree_catalog(),
        placed_tree(),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
    start_harvest(&mut vm, worker);
    remove_unit(&mut vm, depot);

    tick(&mut vm, 0.5);
    assert_eq!(tree_life(&mut vm, tree), 71.0);
    assert!(advance(&mut vm, 2.5, 64).is_err());
    assert_eq!(tree_life(&mut vm, tree), 62.0);
    assert_eq!(current_order(&mut vm, worker), 903);

    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    tick(&mut vm, 3.5);
    tick(&mut vm, 4.5);
    assert_eq!(tree_life(&mut vm, tree), 62.0);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 18
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered + 18
    );
}

#[test]
fn a_capacity_hit_keeps_the_depot_walk_animation_on_a_loaded_departure() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for_range("9", "18", "4", "8"),
        tree_catalog(),
        placed_tree_at(52.0, 32.0),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 46.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 100.0, 96.0);
    start_harvest(&mut vm, worker);

    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);

    assert_eq!(tree_life(&mut vm, tree), 62.0);
    assert_eq!(current_order(&mut vm, worker), 903);
    assert_eq!(animation_name(&mut vm, worker), "Walk");
    let route = vm.host().inner.activity_motion_path(worker).unwrap();
    assert!(!route.is_empty());
}

#[test]
fn an_unreachable_tree_refusal_keeps_an_existing_move_route() {
    let (mut vm, _) = open_harvest_with(
        harvest_tables_for_range("9", "20", "4", "8"),
        tree_catalog(),
        placed_tree_at(52.0, 32.0),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    assert_eq!(move_to(&mut vm, worker, 10.0, 10.0), Value::Bool(true));
    {
        let h = &mut vm.host().inner;
        for at in [[26.0, 32.0], [42.0, 32.0], [32.0, 26.0], [32.0, 42.0]] {
            position_create(h, at[0], at[1]).unwrap();
        }
    }
    let before = harvest_state(&mut vm, worker);

    assert!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrderById",
                &[Value::Handle(worker), Value::Int(900)]
            )
            .is_err()
    );
    assert_eq!(harvest_state(&mut vm, worker), before);
}

#[test]
fn a_refused_manual_return_keeps_partial_cargo_and_the_existing_move_route() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for_range("9", "18", "4", "8"),
        tree_catalog(),
        placed_tree_at(36.0, 32.0),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 100.0, 96.0);
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    assert_eq!(tree_life(&mut vm, tree), 62.0);

    assert_eq!(move_to(&mut vm, worker, 10.0, 10.0), Value::Bool(true));
    {
        let h = &mut vm.host().inner;
        for at in [[26.0, 32.0], [42.0, 32.0], [32.0, 26.0], [32.0, 42.0]] {
            position_create(h, at[0], at[1]).unwrap();
        }
    }
    let before = harvest_state(&mut vm, worker);
    assert_eq!(before.cargo, Some(18.0));

    assert!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrderById",
                &[Value::Handle(worker), Value::Int(901)]
            )
            .is_err()
    );
    assert_eq!(harvest_state(&mut vm, worker), before);
}

#[test]
fn a_lethal_hit_retries_the_depot_route_and_credits_every_carried_unit_once() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for("9", "100", "4"),
        tree_catalog_for("20"),
        placed_tree_with_life(20.0),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    start_harvest(&mut vm, worker);
    remove_unit(&mut vm, depot);

    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    assert!(advance(&mut vm, 4.5, 64).is_err());
    assert_eq!(tree_life(&mut vm, tree), 0.0);
    assert_eq!(current_order(&mut vm, worker), 903);

    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    tick(&mut vm, 5.5);
    tick(&mut vm, 6.5);
    assert_eq!(tree_life(&mut vm, tree), 0.0);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 27
    );
    tick(&mut vm, 7.5);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 27
    );
}

#[test]
fn deposit_credit_is_not_duplicated_when_no_tree_remains() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for("9", "20", "4"),
        tree_catalog_for("9"),
        placed_tree_with_life(9.0),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
    start_harvest(&mut vm, worker);

    tick(&mut vm, 0.5);
    assert_eq!(tree_life(&mut vm, tree), 0.0);
    tick(&mut vm, 1.5);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 9
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered + 9
    );

    tick(&mut vm, 2.5);
    tick(&mut vm, 3.5);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 9
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered + 9
    );
}

#[test]
fn work_catch_up_is_independent_of_the_tick_partition() {
    let (mut whole, whole_tree) = open_harvest();
    let whole_worker = create_unit(&mut whole, *b"hpea", 32.0, 32.0);
    let _whole_depot = create_unit(&mut whole, *b"htow", 36.0, 32.0);
    start_harvest(&mut whole, whole_worker);
    tick(&mut whole, 4.5);

    let (mut split, split_tree) = open_harvest();
    let split_worker = create_unit(&mut split, *b"hpea", 32.0, 32.0);
    let _split_depot = create_unit(&mut split, *b"htow", 36.0, 32.0);
    start_harvest(&mut split, split_worker);
    tick(&mut split, 1.5);
    tick(&mut split, 2.5);
    tick(&mut split, 3.5);
    tick(&mut split, 4.5);

    assert_eq!(
        tree_life(&mut whole, whole_tree),
        tree_life(&mut split, split_tree)
    );
    assert_eq!(tree_life(&mut whole, whole_tree), 53.0);
    assert_eq!(
        current_order(&mut whole, whole_worker),
        current_order(&mut split, split_worker)
    );
    assert_eq!(
        player_bank(&mut whole, "PLAYER_STATE_RESOURCE_LUMBER"),
        player_bank(&mut split, "PLAYER_STATE_RESOURCE_LUMBER")
    );
}
