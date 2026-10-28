use super::*;
use crate::campaign_slow_chopping_policy::SlowChoppingPolicy;
use crate::playercolor_host::{DestructableCatalog, PlayerStateRequestBudget};
use pcx_objects::editor::SlkId;
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_objects::editor::{EditorReadLimits, Overlay};
use std::sync::Arc;

const STATE_EVENTS: &str = r#"
type limitop extends eventid
type playerevent extends eventid
constant native ConvertLimitOp takes integer i returns limitop
constant native ConvertPlayerEvent takes integer i returns playerevent
constant native GetEventPlayerState takes nothing returns playerstate
native TriggerRegisterPlayerStateEvent takes trigger t, player p, playerstate s, limitop op, real limit returns event
globals
constant limitop LESS_THAN=ConvertLimitOp(0)
constant limitop LESS_THAN_OR_EQUAL=ConvertLimitOp(1)
constant limitop EQUAL=ConvertLimitOp(2)
constant limitop GREATER_THAN_OR_EQUAL=ConvertLimitOp(3)
constant limitop GREATER_THAN=ConvertLimitOp(4)
constant limitop NOT_EQUAL=ConvertLimitOp(5)
constant playerevent EVENT_PLAYER_STATE_LIMIT=ConvertPlayerEvent(47)
endglobals
"#;

const DEPOSIT_OBSERVER: &str = r#"
globals
integer Deposits=0
integer DepositOrder=0
integer FirstBank=0
integer FirstGathered=0
integer ObservedBank=0
integer ObservedGathered=0
endglobals
function observe_deposit takes nothing returns nothing
set Deposits=Deposits+1
if GetEventPlayerState()==PLAYER_STATE_RESOURCE_LUMBER then
set DepositOrder=DepositOrder*10+1
else
set DepositOrder=DepositOrder*10+2
endif
set ObservedBank=GetPlayerState(Player(0),PLAYER_STATE_RESOURCE_LUMBER)
set ObservedGathered=GetPlayerState(Player(0),PLAYER_STATE_LUMBER_GATHERED)
if Deposits==1 then
set FirstBank=ObservedBank
set FirstGathered=ObservedGathered
endif
endfunction
function watch_deposit takes nothing returns nothing
local trigger t=CreateTrigger()
call TriggerRegisterPlayerStateEvent(t,Player(0),PLAYER_STATE_RESOURCE_LUMBER,GREATER_THAN,0.0)
call TriggerRegisterPlayerStateEvent(t,Player(0),PLAYER_STATE_LUMBER_GATHERED,GREATER_THAN,0.0)
call TriggerAddAction(t,function observe_deposit)
endfunction
"#;

const GOLD_DEPOSIT_OBSERVER: &str = r#"
globals
integer GoldEvents=0
integer ObservedGold=0
integer ObservedGoldGathered=0
endglobals
function observe_gold takes nothing returns nothing
set GoldEvents=GoldEvents+1
set ObservedGold=GetPlayerState(Player(0),PLAYER_STATE_RESOURCE_GOLD)
set ObservedGoldGathered=GetPlayerState(Player(0),PLAYER_STATE_GOLD_GATHERED)
endfunction
function watch_gold takes nothing returns nothing
local trigger t=CreateTrigger()
call TriggerRegisterPlayerStateEvent(t,Player(0),PLAYER_STATE_RESOURCE_GOLD,GREATER_THAN,0.0)
call TriggerRegisterPlayerStateEvent(t,Player(0),PLAYER_STATE_GOLD_GATHERED,GREATER_THAN,0.0)
call TriggerAddAction(t,function observe_gold)
endfunction
"#;

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
    harvest_tables_for_search_range(damage, capacity, duration, range, "600")
}

fn harvest_tables_for_search_range(
    damage: &'static str,
    capacity: &'static str,
    duration: &'static str,
    range: &'static str,
    search: &'static str,
) -> Arc<DefaultTables> {
    let ability_rows: &[&[&str]] = &[
        &[
            "Agld", "Agld", "1", "0", "0", "500", "2.25", "1", "-", "-", "-",
        ],
        &[
            "Ahar", "Ahar", "1", "0", "0", damage, capacity, "30", range, search, duration,
        ],
        &["Argl", "Artn", "1", "0", "0", "1", "1", "-", "-", "-", "-"],
        &["Arlm", "Artn", "1", "0", "0", "0", "1", "-", "-", "-", "-"],
    ];
    let metadata_rows: &[&[&str]] = &[
        &[
            "Gld2",
            "Data",
            "AbilityData",
            "-1",
            "4",
            "2",
            "0",
            "Agld",
            "-",
            "unreal",
            "stats",
        ],
        &[
            "Gld3",
            "Data",
            "AbilityData",
            "-1",
            "4",
            "3",
            "0",
            "Agld",
            "-",
            "int",
            "stats",
        ],
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
            "Argl,Arlm",
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
            "Argl,Arlm",
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
        (0x760, 967),
        (0x780, 12347),
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
    image[0x760..0x766].copy_from_slice(b"smart\0");
    image[0x780..0x790].copy_from_slice(b"autoharvestgold\0");
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
        crate::order_registry::Registry::read(&image, binding.clone(), budget, 10, 32).unwrap(),
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

fn admit_slow_chopping(vm: &mut Vm, enabled: Option<bool>) {
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    let mut policy = SlowChoppingPolicy::fixture();
    policy.tree_damage = 2.5;
    policy.deposit_lumber = 3;
    host.admit_campaign_slow_chopping(&context, policy).unwrap();
    assert!(host.activate_campaign_ai_player(0).unwrap());
    if let Some(enabled) = enabled {
        host.set_ai_slow_chopping(0, enabled).unwrap();
    }
}

fn open_harvest_with(
    tables: Arc<DefaultTables>,
    catalog: DestructableCatalog,
    tree: crate::playercolor_host::PlacedDestructable,
    fields: Vec<([u8; 4], ObjectField)>,
) -> (Vm, HandleId) {
    open_harvest_with_budget(
        tables,
        catalog,
        tree,
        fields,
        crate::playercolor_host::harvest_runtime::Budget::new(4, 64),
    )
}

fn open_harvest_with_budget(
    tables: Arc<DefaultTables>,
    catalog: DestructableCatalog,
    tree: crate::playercolor_host::PlacedDestructable,
    fields: Vec<([u8; 4], ObjectField)>,
    budget: crate::playercolor_host::harvest_runtime::Budget,
) -> (Vm, HandleId) {
    open_harvest_configured(tables, catalog, tree, fields, budget, ("", MAP), |host| {
        host
    })
}

fn open_harvest_configured(
    tables: Arc<DefaultTables>,
    catalog: DestructableCatalog,
    tree: crate::playercolor_host::PlacedDestructable,
    fields: Vec<([u8; 4], ObjectField)>,
    budget: crate::playercolor_host::harvest_runtime::Budget,
    sources: (&str, &str),
    configure: impl FnOnce(ObjectHost) -> ObjectHost,
) -> (Vm, HandleId) {
    let mut h = host_fields_with_placed_source(
        65536,
        &fields,
        &[tree],
        harvest_rules(),
        &format!("{}\n{DEPOSIT_OBSERVER}\n{GOLD_DEPOSIT_OBSERVER}", sources.1),
        &format!("{STATE_EVENTS}\n{}", sources.0),
    );
    h = h
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 8 })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules())
        .unwrap()
        .with_player_state_events(4)
        .unwrap()
        .with_player_tech_researched(crate::playercolor_host::TechLimitBudget { max_records: 4 })
        .unwrap();
    h.admit_destructable_catalog(catalog).unwrap();
    let mut h = h
        .with_harvest(tables, Overlay::default(), Overlay::default(), budget)
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
    let h = configure(h);
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
    create_unit_for(vm, 0, rawcode, x, y)
}

fn create_unit_for(vm: &mut Vm, owner: i32, rawcode: [u8; 4], x: f32, y: f32) -> HandleId {
    let player = vm
        .host()
        .inner
        .native("Player", &[Value::Int(owner)])
        .unwrap();
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
fn clear_harvest_ai_preserves_a_loaded_explicit_worker_and_its_deposit() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for("9", "18", "4"),
        tree_catalog(),
        placed_tree(),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32., 32.);
    create_unit(&mut vm, *b"htow", 36., 32.);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    host.admit_campaign_gathering(
        &context,
        crate::campaign_gathering_policy::GatheringPolicy::fixture(),
    )
    .unwrap();
    host.activate_campaign_ai_player(0).unwrap();
    host.fixture_ai_harvest_flags(worker, 0x10);
    let mut expected = host.executed_unit(worker).unwrap().clone();
    expected.ai_harvest_flags = Some(0x01);
    host.clear_ai_harvest(0).unwrap();
    assert_eq!(host.executed_unit(worker).unwrap(), &expected);
    assert_eq!(host.harvest_worker_count(worker), 1);
    tick(&mut vm, 3.5);
    assert_eq!(tree_life(&mut vm, tree), 62.);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 18
    );
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
fn hiding_a_loaded_worker_preserves_cargo_for_return_after_reveal() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    assert_eq!(tree_life(&mut vm, tree), 71.0);
    vm.host()
        .inner
        .native("ShowUnit", &[Value::Handle(worker), Value::Bool(false)])
        .unwrap();
    tick(&mut vm, 1.5);
    assert_eq!(vm.host().inner.harvest_worker_count(worker), 0);
    assert_eq!(vm.host().inner.harvest_cargo(worker), Some(9.0));
    assert_eq!(tree_life(&mut vm, tree), 71.0);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"), bank);
    vm.host()
        .inner
        .native("ShowUnit", &[Value::Handle(worker), Value::Bool(true)])
        .unwrap();
    assert_eq!(issue_id(&mut vm, worker, 901), Value::Bool(true));
    tick(&mut vm, 2.0);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 9
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered + 9
    );
    assert_eq!(tree_life(&mut vm, tree), 71.0);
    assert_eq!(current_order(&mut vm, worker), 0);
}

#[test]
fn removal_retires_partial_harvest_cargo_and_worker() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    assert_eq!(tree_life(&mut vm, tree), 62.0);
    assert_eq!(vm.host().inner.harvest_worker_count(worker), 1);

    remove_unit(&mut vm, worker);

    assert_eq!(vm.host().inner.harvest_worker_count(worker), 0);
    assert_eq!(vm.host().inner.harvest_cargo(worker), None);
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

fn gold_role(vm: &mut Vm, unit: HandleId, abilities: &str, building: bool, amount: Option<i32>) {
    let mut definition = vm
        .host()
        .inner
        .executed_unit(unit)
        .unwrap()
        .definition
        .clone();
    let (key, value) = field(*b"uabi", SlkId::UnitAbilities, "abilList", abilities);
    definition.fields.insert(key, value);
    let (key, mut value) = field(
        *b"ubdg",
        SlkId::UnitBalance,
        "isbldg",
        if building { "1" } else { "0" },
    );
    value.metadata.as_mut().unwrap().kind = pcx_objects::editor::FieldKind::Bool;
    value.metadata.as_mut().unwrap().from = SlkId::UnitMetaData;
    value.metadata.as_mut().unwrap().declared = "boolean".into();
    definition.fields.insert(key, value);
    vm.host()
        .inner
        .fixture_harvest_definition(unit, definition, amount);
}

fn gold_fixture() -> (Vm, HandleId, HandleId, HandleId) {
    gold_fixture_budget(4)
}

fn gold_fixture_budget(max_workers: usize) -> (Vm, HandleId, HandleId, HandleId) {
    gold_fixture_search(max_workers, "600")
}

fn gold_fixture_search(
    max_workers: usize,
    search: &'static str,
) -> (Vm, HandleId, HandleId, HandleId) {
    let (mut vm, _) = open_harvest_with_budget(
        harvest_tables_for_search_range("9", "20", "4", "20", search),
        tree_catalog(),
        placed_tree(),
        harvest_fields(),
        crate::playercolor_host::harvest_runtime::Budget::new(max_workers, 64),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let depot = create_unit(&mut vm, *b"htow", 32.0, 96.0);
    let mine = create_unit(&mut vm, *b"ngol", 96.0, 32.0);
    gold_role(&mut vm, worker, "Ahar", false, None);
    gold_role(&mut vm, depot, "Argl", true, None);
    gold_role(&mut vm, mine, "Agld", true, Some(500));
    (vm, worker, depot, mine)
}

fn gold_amount(vm: &mut Vm, mine: HandleId) -> i32 {
    vm.host()
        .inner
        .executed_unit(mine)
        .unwrap()
        .resource_amount
        .unwrap()
}

fn hidden(vm: &mut Vm, unit: HandleId) -> bool {
    vm.host()
        .inner
        .native("IsUnitHidden", &[Value::Handle(unit)])
        .unwrap()
        == Value::Bool(true)
}

fn start_gold(vm: &mut Vm, worker: HandleId, mine: HandleId) {
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(worker),
                    Value::Str("harvest".into()),
                    Value::Handle(mine)
                ]
            )
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn gold_orders_on_units_without_harvest_capability_return_false_without_changes() {
    let (mut vm, worker, depot, mine) = gold_fixture();
    gold_role(&mut vm, worker, "_", false, None);
    for unit in [worker, depot] {
        let before = vm.host().inner.executed_unit(unit).unwrap().clone();
        assert_eq!(
            vm.host()
                .inner
                .native(
                    "IssueTargetOrder",
                    &[
                        Value::Handle(unit),
                        Value::Str("harvest".into()),
                        Value::Handle(mine)
                    ]
                )
                .unwrap(),
            Value::Bool(false)
        );
        let after = vm.host().inner.executed_unit(unit).unwrap();
        assert_eq!(after.activity, before.activity);
        assert_eq!(after.position, before.position);
        assert_eq!(after.shown, before.shown);
        assert_eq!(vm.host().inner.harvest_worker_count(unit), 0);
    }
    assert_eq!(gold_amount(&mut vm, mine), 500);
}

#[test]
fn gold_harvest_enters_the_mine_then_returns_source_cargo_and_repeats() {
    let (mut vm, worker, _depot, mine) = gold_fixture();
    let gold = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_GOLD_GATHERED");
    vm.call("watch_gold", &[]).unwrap();
    start_gold(&mut vm, worker, mine);
    let mut entered = None;
    let mut exited = None;
    for n in 1..=240 {
        let now = n as f64 / 10.0;
        tick(&mut vm, now);
        if entered.is_none() && hidden(&mut vm, worker) {
            entered = Some(now);
        }
        if entered.is_some() && exited.is_none() && !hidden(&mut vm, worker) {
            exited = Some(now);
            assert_eq!(gold_amount(&mut vm, mine), 470);
            assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"), gold);
        }
        if player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD") > gold {
            assert!(entered.is_some() && exited.is_some());
            assert!(exited.unwrap() - entered.unwrap() >= 2.25);
            assert_eq!(
                player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"),
                gold + 30
            );
            assert_eq!(
                player_bank(&mut vm, "PLAYER_STATE_GOLD_GATHERED"),
                gathered + 30
            );
            assert_eq!(gold_amount(&mut vm, mine), 470);
            assert_eq!(current_order(&mut vm, worker), 902);
            while let Some(event) = vm.host().inner.take_fired_event() {
                let delivery = vm.host().inner.begin_fired_event(&event).unwrap().unwrap();
                vm.run_thread(&delivery.actions).unwrap();
                vm.host().inner.finish_fired_event();
            }
            assert_eq!(vm.global("GoldEvents"), Value::Int(2));
            assert_eq!(vm.global("ObservedGold"), Value::Int(gold + 30));
            assert_eq!(vm.global("ObservedGoldGathered"), Value::Int(gathered + 30));
            return;
        }
    }
    panic!("worker did not return its first gold load");
}

#[test]
fn gold_mine_capacity_serializes_workers_and_rejects_orders_while_inside() {
    let (mut vm, worker, _depot, mine) = gold_fixture();
    let other = create_unit(&mut vm, *b"hpea", 64.0, 32.0);
    gold_role(&mut vm, other, "Ahar", false, None);
    start_gold(&mut vm, worker, mine);
    start_gold(&mut vm, other, mine);
    let mut observed = false;
    for n in 1..=60 {
        tick(&mut vm, n as f64 / 10.0);
        assert!(!(hidden(&mut vm, worker) && hidden(&mut vm, other)));
        if hidden(&mut vm, worker) {
            observed = true;
            assert_eq!(
                vm.host()
                    .inner
                    .native(
                        "IssueImmediateOrder",
                        &[Value::Handle(worker), Value::Str("beta".into())]
                    )
                    .unwrap(),
                Value::Bool(false)
            );
        }
    }
    assert!(observed);
}

#[test]
fn carried_gold_survives_stop_and_manual_return_credits_once() {
    let (mut vm, worker, _depot, mine) = gold_fixture();
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
    start_gold(&mut vm, worker, mine);
    let mut stopped_at = None;
    for n in 1..=100 {
        let now = n as f64 / 10.0;
        tick(&mut vm, now);
        if gold_amount(&mut vm, mine) < 500 {
            assert!(!hidden(&mut vm, worker));
            vm.host()
                .inner
                .native(
                    "IssueImmediateOrder",
                    &[Value::Handle(worker), Value::Str("beta".into())],
                )
                .unwrap();
            assert_eq!(
                vm.host()
                    .inner
                    .native(
                        "IssueImmediateOrder",
                        &[Value::Handle(worker), Value::Str("returnresources".into())]
                    )
                    .unwrap(),
                Value::Bool(true)
            );
            stopped_at = Some(now);
            break;
        }
    }
    let stopped_at = stopped_at.expect("first gold cargo");
    for n in 1..=160 {
        tick(&mut vm, stopped_at + n as f64 / 10.0);
    }
    assert_eq!(gold_amount(&mut vm, mine), 470);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"),
        bank + 30
    );
    assert_eq!(current_order(&mut vm, worker), 0);
}

#[test]
fn campaign_wood_keeps_its_town_when_returning_to_a_separate_lumber_mill() {
    let (mut vm, _) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let mill = create_unit(&mut vm, *b"hlum", 32.0, 36.0);
    let hall = create_unit(&mut vm, *b"htow", 100.0, 32.0);
    let spare = create_unit(&mut vm, *b"hpea", 64.0, 32.0);
    for unit in [worker, spare] {
        gold_role(&mut vm, unit, "Ahar", false, None);
    }
    gold_role(&mut vm, mill, "Arlm", true, None);
    gold_role(&mut vm, hall, "Argl", true, None);
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    host.admit_campaign_gathering(
        &context,
        crate::campaign_gathering_policy::GatheringPolicy::fixture(),
    )
    .unwrap();
    host.activate_campaign_ai_player(0).unwrap();
    host.harvest_ai_wood(0, 1, 1).unwrap();
    assert_eq!(
        host.harvest_worker_count(worker),
        0,
        "the mill is not a separate town slot"
    );
    host.harvest_ai_wood(0, 0, 1).unwrap();
    tick(&mut vm, 0.5);
    let host = &mut vm.host().inner;
    let before = host.executed_unit(worker).unwrap().clone();
    host.clear_ai_harvest(0).unwrap();
    host.harvest_ai_wood(0, 0, 1).unwrap();
    assert_eq!(
        host.executed_unit(worker).unwrap().animation,
        before.animation
    );
    assert_eq!(host.harvest_worker_count(worker), 1);
    assert_eq!(host.harvest_worker_count(spare), 0);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    for time in [2.5, 4.5, 5.5] {
        tick(&mut vm, time);
    }
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 20
    );
}

#[test]
fn campaign_wood_assignment_chops_credits_and_reuses_the_original_cycle() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let other = create_unit_for(&mut vm, 1, *b"hpea", 64.0, 32.0);
    gold_role(&mut vm, worker, "Ahar", false, None);
    gold_role(&mut vm, depot, "Argl", true, None);
    gold_role(&mut vm, other, "Ahar", false, None);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    let policy = crate::campaign_gathering_policy::GatheringPolicy::fixture();
    let wood_mask = policy.harvest_current_masks[1];
    host.admit_campaign_gathering(&context, policy).unwrap();
    host.activate_campaign_ai_player(0).unwrap();
    for (town, count) in [(0, 0), (-1, 1), (6, 1), (0, -7)] {
        host.harvest_ai_wood(0, town, count).unwrap();
    }
    assert_eq!(host.harvest_worker_count(worker), 0);
    assert!(host.harvest_ai_wood(1, 0, 1).is_err());
    host.harvest_ai_wood(0, 0, 1).unwrap();
    assert_eq!(host.harvest_worker_count(worker), 1);
    assert_eq!(host.harvest_worker_count(other), 0);
    assert_ne!(
        host.executed_unit(worker)
            .unwrap()
            .ai_harvest_flags
            .unwrap()
            & wood_mask,
        0
    );
    assert_eq!(current_order(&mut vm, worker), 967);
    tick(&mut vm, 0.5);
    assert_eq!(tree_life(&mut vm, tree), 71.0);
    let host = &mut vm.host().inner;
    let before = host.executed_unit(worker).unwrap().clone();
    host.clear_ai_harvest(0).unwrap();
    host.harvest_ai_wood(0, 0, 1).unwrap();
    let after = host.executed_unit(worker).unwrap();
    assert_eq!(after.activity, before.activity);
    assert_eq!(after.animation, before.animation);
    tick(&mut vm, 2.5);
    tick(&mut vm, 4.5);
    assert_eq!(tree_life(&mut vm, tree), 53.0);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"), bank);
    tick(&mut vm, 5.5);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 20
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered + 20
    );
    assert_eq!(current_order(&mut vm, worker), 902);
    assert_eq!(current_order(&mut vm, other), 0);
}

#[test]
fn campaign_wood_does_not_steal_the_gold_assignment() {
    let (mut vm, worker, _, mine) = gold_fixture();
    let wood = create_unit(&mut vm, *b"hpea", 64.0, 32.0);
    gold_role(&mut vm, wood, "Ahar", false, None);
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    host.admit_campaign_gathering(
        &context,
        crate::campaign_gathering_policy::GatheringPolicy::fixture(),
    )
    .unwrap();
    host.activate_campaign_ai_player(0).unwrap();
    host.harvest_ai_gold(0, 0, 1).unwrap();
    let gold = host.executed_unit(worker).unwrap().clone();
    host.harvest_ai_wood(0, 0, 1).unwrap();
    assert_eq!(host.executed_unit(worker).unwrap(), &gold);
    assert_eq!(host.harvest_worker_count(worker), 1);
    assert_eq!(host.harvest_worker_count(wood), 1);
    assert_eq!(gold_amount(&mut vm, mine), 500);
}

#[test]
fn campaign_wood_reserves_all_worker_routes_before_publishing_any() {
    let (mut vm, worker, _, mine) = gold_fixture_budget(1);
    let other = create_unit(&mut vm, *b"hpea", 64.0, 32.0);
    gold_role(&mut vm, other, "Ahar", false, None);
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    host.admit_campaign_gathering(
        &context,
        crate::campaign_gathering_policy::GatheringPolicy::fixture(),
    )
    .unwrap();
    host.activate_campaign_ai_player(0).unwrap();
    let before = [
        host.executed_unit(worker).unwrap().clone(),
        host.executed_unit(other).unwrap().clone(),
    ];
    assert!(host.harvest_ai_wood(0, 0, 2).is_err());
    assert_eq!(host.executed_unit(worker).unwrap(), &before[0]);
    assert_eq!(host.executed_unit(other).unwrap(), &before[1]);
    assert_eq!(host.harvest_worker_count(worker), 0);
    assert_eq!(host.harvest_worker_count(other), 0);
    assert_eq!(gold_amount(&mut vm, mine), 500);
}

#[test]
fn campaign_gold_assignments_reuse_rolled_workers_and_preserve_other_owners() {
    let (mut vm, worker, _depot, mine) = gold_fixture();
    let other = create_unit_for(&mut vm, 1, *b"hpea", 64.0, 32.0);
    gold_role(&mut vm, other, "Ahar", false, None);
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    host.admit_campaign_gathering(
        &context,
        crate::campaign_gathering_policy::GatheringPolicy::fixture(),
    )
    .unwrap();
    host.activate_campaign_ai_player(0).unwrap();
    host.harvest_ai_gold(0, 0, 0).unwrap();
    assert_eq!(host.harvest_worker_count(worker), 0);
    host.harvest_ai_gold(0, -1, 1).unwrap();
    host.harvest_ai_gold(0, 6, 1).unwrap();
    assert_eq!(host.harvest_worker_count(worker), 0);
    host.harvest_ai_gold(0, 0, 1).unwrap();
    assert_eq!(host.harvest_worker_count(worker), 1);
    assert_eq!(host.harvest_worker_count(other), 0);
    assert!(host.harvest_ai_gold(1, 0, 1).is_err());
    let expected = host.executed_unit(worker).unwrap().clone();
    host.clear_ai_harvest(0).unwrap();
    host.harvest_ai_gold(0, 0, 1).unwrap();
    assert_eq!(
        host.executed_unit(worker).unwrap().activity,
        expected.activity
    );
    assert_eq!(host.harvest_worker_count(worker), 1);
    assert_eq!(gold_amount(&mut vm, mine), 500);
}

#[test]
fn campaign_gold_budget_refuses_the_whole_assignment_before_routes_or_flags() {
    let (mut vm, worker, _, mine) = gold_fixture_budget(1);
    let other = create_unit(&mut vm, *b"hpea", 64.0, 32.0);
    gold_role(&mut vm, other, "Ahar", false, None);
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    host.admit_campaign_gathering(
        &context,
        crate::campaign_gathering_policy::GatheringPolicy::fixture(),
    )
    .unwrap();
    host.activate_campaign_ai_player(0).unwrap();
    let before = [
        host.executed_unit(worker).unwrap().clone(),
        host.executed_unit(other).unwrap().clone(),
    ];
    assert!(host.harvest_ai_gold(0, 0, 2).is_err());
    assert_eq!(host.executed_unit(worker).unwrap(), &before[0]);
    assert_eq!(host.executed_unit(other).unwrap(), &before[1]);
    assert_eq!(host.harvest_worker_count(worker), 0);
    assert_eq!(host.harvest_worker_count(other), 0);
    assert_eq!(gold_amount(&mut vm, mine), 500);
}

#[test]
fn accepted_flee_preserves_gold_cargo_without_harvest_overwriting_the_route() {
    let (mut vm, worker, _, mine) = gold_fixture();
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
    start_gold(&mut vm, worker, mine);
    let mut exit_time = None;
    for n in 1..=100 {
        let now = n as f64 / 10.0;
        tick(&mut vm, now);
        if gold_amount(&mut vm, mine) < 500 {
            exit_time = Some(now);
            break;
        }
    }
    let now = exit_time.expect("gold cargo");
    vm.host()
        .inner
        .initiate_campaign_flee(worker, [96.0, 96.0])
        .unwrap();
    assert_eq!(vm.host().inner.harvest_worker_count(worker), 0);
    tick(&mut vm, now + 1.0);
    assert_eq!(gold_amount(&mut vm, mine), 470);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"), bank);
    assert_eq!(issue_id(&mut vm, worker, 901), Value::Bool(true));
    for n in 1..=100 {
        tick(&mut vm, now + 1.0 + n as f64 / 10.0);
    }
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"),
        bank + 30
    );
}

#[test]
fn depleted_gold_mine_delivers_one_death_and_a_partial_final_load() {
    use super::super::combat;
    let mut fields = combat::armed("80", "normal");
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W001")
        .unwrap()
        .1
        .value = Ok(pcx_sim::world::FieldValue::Cell("0".into()));
    fields.extend(harvest_fields().into_iter().filter(|(_, value)| {
        value.metadata.as_ref().is_some_and(|meta| {
            !matches!(
                meta.source,
                pcx_objects::editor::FieldSource::Column {
                    slk: SlkId::UnitWeapons,
                    ..
                }
            )
        })
    }));
    let declarations = combat::DECLARATIONS.replace(
        "native IssueTargetOrder takes unit u, string o, widget t returns boolean\n",
        "",
    );
    let map = "globals\ninteger MineDeaths=0\nunit ObservedMine=null\nendglobals\nfunction mine_death takes nothing returns nothing\nset MineDeaths=MineDeaths+1\nset ObservedMine=GetDyingUnit()\nendfunction\n";
    let map = format!("{}\n{map}", MAP.replace("'U001'", "'u001'"));
    let (mut vm, _) = open_harvest_configured(
        harvest_tables_for_range("9", "20", "4", "20"),
        tree_catalog(),
        placed_tree(),
        fields,
        crate::playercolor_host::harvest_runtime::Budget::new(4, 64),
        (&declarations, &map),
        |host| {
            let mut host = host
                .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
                .unwrap()
                .with_player_unit_events(8)
                .unwrap();
            host.admit_unit_vitals(pcx_objects::editor::misc::HeroVitals {
                life_per_strength: 25.0,
                mana_per_intelligence: 15.0,
                life_regen_per_strength: 0.0,
                mana_regen_per_intelligence: 0.0,
            })
            .unwrap();
            host.admit_combat(combat::rules(), "alpha").unwrap();
            host
        },
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let depot = create_unit(&mut vm, *b"htow", 32.0, 96.0);
    let mine = create_unit(&mut vm, *b"ngol", 96.0, 32.0);
    gold_role(&mut vm, worker, "Ahar", false, None);
    gold_role(&mut vm, depot, "Argl", true, None);
    gold_role(&mut vm, mine, "Agld", true, Some(7));
    let host = &mut vm.host().inner;
    let trigger = host.native("CreateTrigger", &[]).unwrap();
    let event = vm.global("EVENT_UNIT_DEATH");
    vm.host()
        .inner
        .native(
            "TriggerRegisterUnitEvent",
            &[trigger.clone(), Value::Handle(mine), event],
        )
        .unwrap();
    let code = vm.program().function_id("mine_death").unwrap();
    vm.host()
        .inner
        .native("TriggerAddAction", &[trigger, Value::Code(code)])
        .unwrap();
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
    start_gold(&mut vm, worker, mine);
    for n in 1..=240 {
        tick(&mut vm, n as f64 / 10.0);
    }
    while let Some(event) = vm.host().inner.take_fired_event() {
        let delivery = vm.host().inner.begin_fired_event(&event).unwrap().unwrap();
        vm.run_thread(&delivery.actions).unwrap();
        vm.host().inner.finish_fired_event();
    }
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"), bank + 7);
    assert_eq!(gold_amount(&mut vm, mine), 0);
    assert!(
        vm.host()
            .inner
            .executed_unit(mine)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
    assert!(!hidden(&mut vm, worker));
    assert_eq!(current_order(&mut vm, worker), 0);
    assert_eq!(vm.global("MineDeaths"), Value::Int(1));
    assert_eq!(vm.global("ObservedMine"), Value::Handle(mine));
}

#[test]
fn slow_chopping_uses_image_damage_but_source_cargo_and_one_credit() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    admit_slow_chopping(&mut vm, Some(true));
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
    vm.call("watch_deposit", &[]).unwrap();

    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    assert_eq!(tree_life(&mut vm, tree), 77.5);
    tick(&mut vm, 2.5);
    assert_eq!(tree_life(&mut vm, tree), 75.0);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"), bank);
    tick(&mut vm, 4.5);
    assert_eq!(tree_life(&mut vm, tree), 72.5);
    tick(&mut vm, 5.5);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 3
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered + 3
    );
    while let Some(event) = vm.host().inner.take_fired_event() {
        let delivery = vm.host().inner.begin_fired_event(&event).unwrap().unwrap();
        vm.run_thread(&delivery.actions).unwrap();
        vm.host().inner.finish_fired_event();
    }
    assert_eq!(vm.global("Deposits"), Value::Int(2));
    assert_eq!(vm.global("FirstBank"), Value::Int(bank + 3));
    assert_eq!(vm.global("FirstGathered"), Value::Int(gathered + 3));
    tick(&mut vm, 6.5);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 3
    );
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn slow_chopping_rechecks_owner_flag_at_hit_and_deposit() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    admit_slow_chopping(&mut vm, Some(true));
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    assert_eq!(tree_life(&mut vm, tree), 77.5);
    vm.host().inner.set_ai_slow_chopping(0, false).unwrap();
    tick(&mut vm, 2.5);
    assert_eq!(tree_life(&mut vm, tree), 68.5);
    issue_id(&mut vm, worker, 744);
    assert_eq!(vm.host().inner.harvest_cargo(worker), Some(18.0));
    vm.host().inner.set_ai_slow_chopping(0, true).unwrap();
    issue_id(&mut vm, worker, 901);
    tick(&mut vm, 3.5);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 3
    );
}

#[test]
fn slow_chopping_deposit_is_independent_of_carried_lumber() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    let mut policy = SlowChoppingPolicy::fixture();
    policy.tree_damage = 2.5;
    policy.deposit_lumber = 20;
    host.admit_campaign_slow_chopping(&context, policy).unwrap();
    assert!(host.activate_campaign_ai_player(0).unwrap());
    host.set_ai_slow_chopping(0, true).unwrap();

    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    assert_eq!(tree_life(&mut vm, tree), 77.5);
    issue_id(&mut vm, worker, 744);
    assert_eq!(vm.host().inner.harvest_cargo(worker), Some(9.0));
    assert_eq!(issue_id(&mut vm, worker, 901), Value::Bool(true));
    tick(&mut vm, 1.5);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 20
    );
}

#[test]
fn inactive_or_explicitly_false_slow_chopping_keeps_source_harvest() {
    for active in [false, true] {
        let (mut vm, tree) = open_harvest();
        let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
        let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
        if active {
            admit_slow_chopping(&mut vm, Some(false));
        } else {
            let host = &mut vm.host().inner;
            let context = Arc::clone(host.player_context().unwrap());
            host.admit_campaign_slow_chopping(&context, SlowChoppingPolicy::fixture())
                .unwrap();
        }
        let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
        start_harvest(&mut vm, worker);
        tick(&mut vm, 0.5);
        tick(&mut vm, 2.5);
        assert_eq!(tree_life(&mut vm, tree), 62.0);
        issue_id(&mut vm, worker, 744);
        assert_eq!(vm.host().inner.harvest_cargo(worker), Some(18.0));
        issue_id(&mut vm, worker, 901);
        tick(&mut vm, 3.5);
        assert_eq!(
            player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
            bank + 18
        );
    }
}

#[test]
fn another_owners_worker_keeps_source_harvest_while_ai_owner_is_slow() {
    let (mut vm, tree) = open_harvest();
    vm.host().inner.native("Player", &[Value::Int(0)]).unwrap();
    let worker = create_unit_for(&mut vm, 1, *b"hpea", 32.0, 32.0);
    let _depot = create_unit_for(&mut vm, 1, *b"htow", 36.0, 32.0);
    admit_slow_chopping(&mut vm, Some(true));
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    assert_eq!(tree_life(&mut vm, tree), 71.0);
    issue_id(&mut vm, worker, 744);
    assert_eq!(vm.host().inner.harvest_cargo(worker), Some(9.0));
}

#[test]
fn unknown_and_exceptional_slow_chopping_refuse_before_order_or_tree_writes() {
    for exceptional in ["unknown", "summoned", "effect", "ability_off", "ability_on"] {
        let (mut vm, tree) = open_harvest();
        let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
        let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
        admit_slow_chopping(&mut vm, (exceptional != "unknown").then_some(true));
        match exceptional {
            "summoned" => vm.host().inner.test_set_summoned(worker, true),
            "effect" => vm
                .host()
                .inner
                .test_record_effect_history(worker, false, None),
            "ability_off" => vm
                .host()
                .inner
                .test_set_harvest_admitted_order(worker, false),
            "ability_on" => vm
                .host()
                .inner
                .test_set_harvest_admitted_order(worker, true),
            _ => {}
        }
        let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
        let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
        let handles = vm.host().inner.inner().handles();
        let result = vm.host().inner.native(
            "IssueImmediateOrderById",
            &[Value::Handle(worker), Value::Int(900)],
        );
        assert!(result.is_err(), "{exceptional}");
        assert_eq!(current_order(&mut vm, worker), 0, "{exceptional}");
        assert_eq!(vm.host().inner.inner().handles(), handles);
        assert_eq!(vm.host().inner.harvest_worker_count(worker), 0);
        assert_eq!(vm.host().inner.harvest_cargo(worker), None);
        assert_eq!(tree_life(&mut vm, tree), 80.0);
        assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"), bank);
        assert_eq!(
            player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
            gathered
        );
    }
}

#[test]
fn empty_returnresources_ignores_an_unknown_slow_chopping_flag() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    admit_slow_chopping(&mut vm, None);
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
    let handles = vm.host().inner.inner().handles();
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrderById",
                &[Value::Handle(worker), Value::Int(901)],
            )
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(current_order(&mut vm, worker), 0);
    assert_eq!(tree_life(&mut vm, tree), 80.0);
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert_eq!(vm.host().inner.harvest_cargo(worker), None);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"), bank);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered
    );
}

#[test]
fn loaded_returnresources_refuses_an_unknown_slow_chopping_flag_before_credit() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    admit_slow_chopping(&mut vm, Some(true));
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    issue_id(&mut vm, worker, 744);
    assert_eq!(vm.host().inner.harvest_cargo(worker), Some(9.0));
    vm.host().inner.deactivate_campaign_ai_player(0).unwrap();
    assert!(vm.host().inner.activate_campaign_ai_player(0).unwrap());

    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
    let order = current_order(&mut vm, worker);
    let handles = vm.host().inner.inner().handles();
    assert!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrderById",
                &[Value::Handle(worker), Value::Int(901)],
            )
            .is_err()
    );
    assert_eq!(current_order(&mut vm, worker), order);
    assert_eq!(tree_life(&mut vm, tree), 77.5);
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert_eq!(vm.host().inner.harvest_cargo(worker), Some(9.0));
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"), bank);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered
    );
}

#[test]
fn a_new_unknown_ai_context_stops_an_existing_order_before_its_hit() {
    let (mut vm, tree) = open_harvest();
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 36.0, 32.0);
    admit_slow_chopping(&mut vm, Some(true));
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    start_harvest(&mut vm, worker);
    vm.host().inner.deactivate_campaign_ai_player(0).unwrap();
    assert!(vm.host().inner.activate_campaign_ai_player(0).unwrap());
    assert!(advance(&mut vm, 0.5, 64).is_err());
    assert_eq!(tree_life(&mut vm, tree), 80.0);
    assert_eq!(current_order(&mut vm, worker), 902);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"), bank);
}

#[test]
fn slow_chopping_does_not_gate_an_accepted_return_walk() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for_range("9", "18", "4", "8"),
        tree_catalog(),
        placed_tree_at(52.0, 32.0),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 46.0, 32.0);
    let _depot = create_unit(&mut vm, *b"htow", 100.0, 96.0);
    admit_slow_chopping(&mut vm, Some(true));
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED");
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    assert_eq!(current_order(&mut vm, worker), 903);
    assert!(
        !vm.host()
            .inner
            .activity_motion_path(worker)
            .unwrap()
            .is_empty()
    );
    vm.host().inner.deactivate_campaign_ai_player(0).unwrap();
    assert!(vm.host().inner.activate_campaign_ai_player(0).unwrap());

    let before = position(&mut vm, worker);
    tick(&mut vm, 3.5);
    assert_ne!(position(&mut vm, worker), before);
    assert_eq!(tree_life(&mut vm, tree), 75.0);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"), bank);
    assert!(advance(&mut vm, 100.0, 65536).is_err());
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"), bank);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered
    );

    vm.host().inner.set_ai_slow_chopping(0, false).unwrap();
    tick(&mut vm, 100.0);
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_LUMBER"),
        bank + 18
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_LUMBER_GATHERED"),
        gathered + 18
    );
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
fn a_harvest_deposit_delivers_both_state_triggers_with_the_complete_credit() {
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
    vm.call("watch_deposit", &[]).unwrap();
    start_harvest(&mut vm, worker);
    tick(&mut vm, 0.5);
    assert_eq!(tree_life(&mut vm, tree), 0.0);
    assert!(vm.host().inner.take_fired_event().is_none());
    tick(&mut vm, 1.5);
    assert_eq!(vm.global("Deposits"), Value::Int(0));

    while let Some(event) = vm.host().inner.take_fired_event() {
        let delivery = vm.host().inner.begin_fired_event(&event).unwrap().unwrap();
        assert!(delivery.conditions.is_empty());
        vm.run_thread(&delivery.actions).unwrap();
        vm.host().inner.finish_fired_event();
    }
    assert_eq!(vm.global("Deposits"), Value::Int(2));
    assert_eq!(vm.global("DepositOrder"), Value::Int(12));
    assert_eq!(vm.global("FirstBank"), Value::Int(bank + 9));
    assert_eq!(vm.global("FirstGathered"), Value::Int(gathered + 9));
    assert_eq!(vm.global("ObservedBank"), Value::Int(bank + 9));
    assert_eq!(vm.global("ObservedGathered"), Value::Int(gathered + 9));
    tick(&mut vm, 2.5);
    assert!(vm.host().inner.take_fired_event().is_none());
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

fn auto_gold(vm: &mut Vm, worker: HandleId, numeric: bool) -> Result<Value, String> {
    vm.host().inner.native(
        if numeric {
            "IssueImmediateOrderById"
        } else {
            "IssueImmediateOrder"
        },
        &[
            Value::Handle(worker),
            if numeric {
                Value::Int(12347)
            } else {
                Value::Str("autoharvestgold".into())
            },
        ],
    )
}

#[test]
fn autoharvestgold_both_order_forms_enter_the_mine_and_deposit_real_cargo() {
    for numeric in [false, true] {
        let (mut vm, worker, _depot, mine) = gold_fixture();
        let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
        assert_eq!(
            auto_gold(&mut vm, worker, numeric).unwrap(),
            Value::Bool(true)
        );
        let mut entered = false;
        for n in 1..=240 {
            tick(&mut vm, f64::from(n) / 10.0);
            entered |= hidden(&mut vm, worker);
            if player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD") > bank {
                break;
            }
        }
        assert!(entered);
        assert_eq!(gold_amount(&mut vm, mine), 470);
        assert_eq!(
            player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"),
            bank + 30
        );
    }
}

#[test]
fn autoharvestgold_selects_nearest_eligible_mine_with_handle_ties() {
    let (mut vm, worker, _depot, original) = gold_fixture();
    let first = create_unit(&mut vm, *b"ngol", 80.0, 64.0);
    let second = create_unit(&mut vm, *b"ngol", 64.0, 80.0);
    for mine in [first, second] {
        gold_role(&mut vm, mine, "Agld", true, Some(500));
    }
    assert!(first < second);
    let worker_position = vm
        .host()
        .inner
        .executed_unit(worker)
        .unwrap()
        .position
        .unwrap();
    let square = |vm: &mut Vm, mine| {
        let position = vm
            .host()
            .inner
            .executed_unit(mine)
            .unwrap()
            .position
            .unwrap();
        (position[0] - worker_position[0]).powi(2) + (position[1] - worker_position[1]).powi(2)
    };
    assert_eq!(square(&mut vm, first), square(&mut vm, second));
    assert_eq!(
        auto_gold(&mut vm, worker, false).unwrap(),
        Value::Bool(true)
    );
    for n in 1..=240 {
        tick(&mut vm, f64::from(n) / 10.0);
        if gold_amount(&mut vm, first) < 500 {
            break;
        }
    }
    assert_eq!(gold_amount(&mut vm, first), 470);
    assert_eq!(gold_amount(&mut vm, second), 500);
    assert_eq!(gold_amount(&mut vm, original), 500);
}

#[test]
fn autoharvestgold_no_mine_or_failed_admission_preserves_worker_and_resources() {
    for case in 0..4 {
        let (mut vm, worker, depot, mine) = gold_fixture_budget(if case == 2 { 0 } else { 4 });
        if case == 0 {
            gold_role(&mut vm, mine, "Agld", true, Some(0));
        }
        if case == 1 {
            gold_role(&mut vm, depot, "_", true, None);
        }
        if case == 3 {
            gold_role(&mut vm, worker, "_", false, None);
        }
        let before = vm.host().inner.executed_unit(worker).unwrap().clone();
        let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
        assert!(
            vm.host()
                .inner
                .native(
                    "IssueImmediateOrder",
                    &[
                        Value::Handle(worker),
                        Value::Str("unknown-synthetic-order".into())
                    ]
                )
                .is_err()
        );
        let result = auto_gold(&mut vm, worker, false);
        if case == 0 || case == 3 {
            assert_eq!(result.unwrap(), Value::Bool(false));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(vm.host().inner.executed_unit(worker).unwrap(), &before);
        assert_eq!(vm.host().inner.harvest_worker_count(worker), 0);
        assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"), bank);
        assert_eq!(gold_amount(&mut vm, mine), if case == 0 { 0 } else { 500 });
    }
}

#[test]
fn autoharvestgold_ignores_hidden_empty_and_removed_nearer_mines() {
    for case in 0..3 {
        let (mut vm, worker, _depot, mine) = gold_fixture();
        let nearer = create_unit(&mut vm, *b"ngol", 56.0, 32.0);
        gold_role(
            &mut vm,
            nearer,
            "Agld",
            true,
            Some(if case == 1 { 0 } else { 500 }),
        );
        if case == 0 {
            vm.host()
                .inner
                .native("ShowUnit", &[Value::Handle(nearer), Value::Bool(false)])
                .unwrap();
        }
        if case == 2 {
            remove_unit(&mut vm, nearer);
        }
        assert_eq!(
            auto_gold(&mut vm, worker, false).unwrap(),
            Value::Bool(true)
        );
        for n in 1..=240 {
            tick(&mut vm, f64::from(n) / 10.0);
            if gold_amount(&mut vm, mine) < 500 {
                break;
            }
        }
        assert_eq!(gold_amount(&mut vm, mine), 470);
    }
}

#[test]
fn gold_mine_exit_retains_a_usable_admitted_depot_outside_acquisition_radius() {
    let (mut vm, worker, depot, mine) = gold_fixture_search(4, "70");
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
    start_gold(&mut vm, worker, mine);
    let mut outside = false;
    for n in 1..=240 {
        tick(&mut vm, f64::from(n) / 10.0);
        let host = &mut vm.host().inner;
        let from = host.executed_unit(worker).unwrap().position.unwrap();
        let target = host.executed_unit(depot).unwrap().position.unwrap();
        outside |= (from[0] - target[0]).hypot(from[1] - target[1]) > 70.0;
        if player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD") > bank {
            break;
        }
    }
    assert!(
        outside,
        "the accepted mining leg leaves the acquisition radius"
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"),
        bank + 30
    );
    assert_eq!(gold_amount(&mut vm, mine), 470);
}

#[test]
fn gold_mine_exit_reacquires_a_depot_when_the_admitted_one_is_stale() {
    for case in 0..3 {
        let (mut vm, worker, depot, mine) = gold_fixture();
        let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
        start_gold(&mut vm, worker, mine);
        let mut n = 0;
        while !hidden(&mut vm, worker) {
            n += 1;
            tick(&mut vm, f64::from(n) / 10.0);
            assert!(n < 100);
        }
        let replacement = create_unit(&mut vm, *b"htow", 96.0, 96.0);
        gold_role(&mut vm, replacement, "Argl", true, None);
        match case {
            0 => {
                vm.host()
                    .inner
                    .native("ShowUnit", &[Value::Handle(depot), Value::Bool(false)])
                    .unwrap();
            }
            1 => {
                let owner = vm.host().inner.native("Player", &[Value::Int(1)]).unwrap();
                vm.host()
                    .inner
                    .native(
                        "SetUnitOwner",
                        &[Value::Handle(depot), owner, Value::Bool(false)],
                    )
                    .unwrap();
            }
            _ => remove_unit(&mut vm, depot),
        }
        for step in n + 1..=240 {
            tick(&mut vm, f64::from(step) / 10.0);
            if player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD") > bank {
                break;
            }
        }
        assert_eq!(
            player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"),
            bank + 30
        );
        assert_eq!(gold_amount(&mut vm, mine), 470);
    }
}

#[test]
fn gold_mine_exit_without_a_usable_depot_preserves_hidden_worker_and_mine() {
    let (mut vm, worker, depot, mine) = gold_fixture();
    start_gold(&mut vm, worker, mine);
    let mut n = 0;
    while !hidden(&mut vm, worker) {
        n += 1;
        tick(&mut vm, f64::from(n) / 10.0);
        assert!(n < 100);
    }
    remove_unit(&mut vm, depot);
    let before = vm.host().inner.executed_unit(worker).unwrap().clone();
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
    let error = (n + 1..=240)
        .find_map(|step| advance(&mut vm, f64::from(step) / 10.0, 65536).err())
        .expect("missing depot refuses");
    assert!(
        error
            .to_string()
            .contains("no gold-accepting depot at mine exit")
    );
    let after = vm.host().inner.executed_unit(worker).unwrap();
    assert_eq!(after.position, before.position);
    assert_eq!(after.shown, before.shown);
    assert_eq!(after.activity, before.activity);
    assert_eq!(gold_amount(&mut vm, mine), 500);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"), bank);
}

#[test]
fn campaign_targeting_harvesting_is_not_the_spell_channel_band() {
    let mut fields = harvest_fields();
    let mut class = field(*b"clas", SlkId::UnitBalance, "type", "peon");
    class.1.metadata.as_mut().unwrap().from = SlkId::UnitMetaData;
    fields.push(class);
    let (mut vm, _) = open_harvest_with(harvest_tables(), tree_catalog(), placed_tree(), fields);
    let worker = create_unit(&mut vm, *b"hpea", 32., 32.);
    let ordinary = create_unit(&mut vm, *b"hpea", 36., 32.);
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    let policy = crate::campaign_targeting_policy::TargetingPolicy::fixture();
    host.test_set_game_difficulty(i32::from(policy.optional_level_gate));
    host.admit_campaign_targeting(&context, policy).unwrap();
    let choices = [(ordinary, 1.), (worker, 20.)];
    assert_eq!(
        host.test_campaign_acquisition(&choices).unwrap(),
        Some(ordinary)
    );
    assert_eq!(
        host.native(
            "IssueImmediateOrderById",
            &[Value::Handle(worker), Value::Int(900)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    tick(&mut vm, 1.);
    let host = &mut vm.host().inner;
    assert_eq!(host.harvest_worker_count(worker), 1);
    assert_eq!(
        host.test_campaign_acquisition(&choices).unwrap(),
        Some(ordinary)
    );
}

#[test]
fn gold_hidden_world_keeps_accepted_mine_and_depot_across_multiple_cycles() {
    let (mut vm, worker, depot, mine) = gold_fixture();
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
    let gathered = player_bank(&mut vm, "PLAYER_STATE_GOLD_GATHERED");
    start_gold(&mut vm, worker, mine);
    let mut n = 0;
    while !hidden(&mut vm, worker) {
        n += 1;
        tick(&mut vm, f64::from(n) / 10.0);
        assert!(n < 100);
    }
    for unit in [mine, depot, worker] {
        vm.host()
            .inner
            .native("ShowUnit", &[Value::Handle(unit), Value::Bool(false)])
            .unwrap();
    }
    let mut revealed_with_cargo = false;
    for step in n + 1..=300 {
        tick(&mut vm, f64::from(step) / 10.0);
        assert!(hidden(&mut vm, mine) && hidden(&mut vm, depot));
        revealed_with_cargo |= !hidden(&mut vm, worker) && gold_amount(&mut vm, mine) < 500;
        if player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD") >= bank + 60 {
            break;
        }
    }
    assert!(
        revealed_with_cargo,
        "mining exit reveals the explicitly hidden occupant"
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"),
        bank + 60
    );
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_GOLD_GATHERED"),
        gathered + 60
    );
    assert_eq!(gold_amount(&mut vm, mine), 440);
    for unit in [mine, depot] {
        vm.host()
            .inner
            .native("ShowUnit", &[Value::Handle(unit), Value::Bool(true)])
            .unwrap();
    }
    assert!(!hidden(&mut vm, mine) && !hidden(&mut vm, depot));
    assert_eq!(
        player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"),
        bank + 60
    );
}

#[test]
fn gold_hidden_replacement_depot_is_not_newly_acquired_and_exit_is_atomic() {
    for changed in 0..2 {
        let (mut vm, worker, depot, mine) = gold_fixture();
        let replacement = create_unit(&mut vm, *b"htow", 96.0, 96.0);
        gold_role(&mut vm, replacement, "Argl", true, None);
        vm.host()
            .inner
            .native(
                "ShowUnit",
                &[Value::Handle(replacement), Value::Bool(false)],
            )
            .unwrap();
        start_gold(&mut vm, worker, mine);
        let mut n = 0;
        while !hidden(&mut vm, worker) {
            n += 1;
            tick(&mut vm, f64::from(n) / 10.0);
            assert!(n < 100);
        }
        match changed {
            0 => remove_unit(&mut vm, depot),
            1 => {
                let owner = vm.host().inner.native("Player", &[Value::Int(1)]).unwrap();
                vm.host()
                    .inner
                    .native(
                        "SetUnitOwner",
                        &[Value::Handle(depot), owner, Value::Bool(false)],
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let before = vm.host().inner.executed_unit(worker).unwrap().clone();
        let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
        let error = (n + 1..=240)
            .find_map(|step| advance(&mut vm, f64::from(step) / 10.0, 65536).err())
            .expect("hidden replacement is not acquisition eligible");
        assert!(
            error
                .to_string()
                .contains("no gold-accepting depot at mine exit")
        );
        let after = vm.host().inner.executed_unit(worker).unwrap();
        assert_eq!(after.position, before.position);
        assert_eq!(after.shown, before.shown);
        assert_eq!(after.activity, before.activity);
        assert_eq!(gold_amount(&mut vm, mine), 500);
        assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"), bank);
    }
}

#[test]
fn gold_removed_occupied_mine_refuses_with_identity_without_exit_or_credit() {
    let (mut vm, worker, _depot, mine) = gold_fixture();
    start_gold(&mut vm, worker, mine);
    let mut n = 0;
    while !hidden(&mut vm, worker) {
        n += 1;
        tick(&mut vm, f64::from(n) / 10.0);
        assert!(n < 100);
    }
    let before = vm.host().inner.executed_unit(worker).unwrap().clone();
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
    remove_unit(&mut vm, mine);
    let error = (n + 1..=240)
        .find_map(|step| advance(&mut vm, f64::from(step) / 10.0, 65536).err())
        .expect("removed occupied mine remains unsupported")
        .to_string();
    assert!(error.contains("occupied mine removal is not qualified"));
    assert!(error.contains(&format!("worker {worker:?}, mine {mine:?}")));
    let after = vm.host().inner.executed_unit(worker).unwrap();
    assert_eq!(after.position, before.position);
    assert_eq!(after.shown, before.shown);
    assert_eq!(after.activity, before.activity);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"), bank);
}

#[test]
fn gold_dead_occupied_mine_refuses_with_identity_without_debit_or_reveal() {
    use super::super::combat;
    let mut fields = combat::armed("80", "normal");
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W001")
        .unwrap()
        .1
        .value = Ok(pcx_sim::world::FieldValue::Cell("0".into()));
    fields.extend(harvest_fields().into_iter().filter(|(_, value)| {
        value.metadata.as_ref().is_some_and(|meta| {
            !matches!(
                meta.source,
                pcx_objects::editor::FieldSource::Column {
                    slk: SlkId::UnitWeapons,
                    ..
                }
            )
        })
    }));
    let declarations = combat::DECLARATIONS.replace(
        "native IssueTargetOrder takes unit u, string o, widget t returns boolean\n",
        "",
    );
    let map = "globals\ninteger MineDeaths=0\nunit ObservedMine=null\nendglobals\nfunction mine_death takes nothing returns nothing\nset MineDeaths=MineDeaths+1\nset ObservedMine=GetDyingUnit()\nendfunction\n";
    let map = format!("{}\n{map}", MAP.replace("'U001'", "'u001'"));
    let (mut vm, _) = open_harvest_configured(
        harvest_tables_for_range("9", "20", "4", "20"),
        tree_catalog(),
        placed_tree(),
        fields,
        crate::playercolor_host::harvest_runtime::Budget::new(4, 64),
        (&declarations, &map),
        |host| {
            let mut host = host
                .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
                .unwrap()
                .with_player_unit_events(8)
                .unwrap();
            host.admit_unit_vitals(pcx_objects::editor::misc::HeroVitals {
                life_per_strength: 25.0,
                mana_per_intelligence: 15.0,
                life_regen_per_strength: 0.0,
                mana_regen_per_intelligence: 0.0,
            })
            .unwrap();
            host.admit_combat(combat::rules(), "alpha").unwrap();
            host
        },
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let depot = create_unit(&mut vm, *b"htow", 32.0, 96.0);
    let mine = create_unit(&mut vm, *b"ngol", 96.0, 32.0);
    gold_role(&mut vm, worker, "Ahar", false, None);
    gold_role(&mut vm, depot, "Argl", true, None);
    gold_role(&mut vm, mine, "Agld", true, Some(500));
    start_gold(&mut vm, worker, mine);
    let mut n = 0;
    while !hidden(&mut vm, worker) {
        n += 1;
        tick(&mut vm, f64::from(n) / 10.0);
        assert!(n < 100);
    }
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(mine)])
        .unwrap();
    let before = vm.host().inner.executed_unit(worker).unwrap().clone();
    let bank = player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD");
    let error = (n + 1..=240)
        .find_map(|step| advance(&mut vm, f64::from(step) / 10.0, 65536).err())
        .expect("dead occupied mine remains unsupported")
        .to_string();
    assert!(error.contains("occupied mine death is not qualified"));
    assert!(error.contains(&format!("worker {worker:?}, mine {mine:?}")));
    assert_eq!(gold_amount(&mut vm, mine), 500);
    let after = vm.host().inner.executed_unit(worker).unwrap();
    assert_eq!(after.shown, before.shown);
    assert_eq!(after.position, before.position);
    assert_eq!(after.activity, before.activity);
    assert_eq!(player_bank(&mut vm, "PLAYER_STATE_RESOURCE_GOLD"), bank);
}

#[test]
fn idle_workers_exclude_busy_hidden_and_foreign_peons() {
    let mut fields = harvest_fields();
    let mut class = field(*b"clas", SlkId::UnitBalance, "type", "peon");
    class.1.metadata.as_mut().unwrap().from = SlkId::UnitMetaData;
    fields.push(class);
    let (mut vm, _) = open_harvest_with(harvest_tables(), tree_catalog(), placed_tree(), fields);
    let busy = create_unit(&mut vm, *b"hpea", 32., 32.);
    let idle = create_unit(&mut vm, *b"hpea", 36., 32.);
    let hidden_worker = create_unit(&mut vm, *b"hpea", 40., 32.);
    let foreign = create_unit_for(&mut vm, 1, *b"hpea", 48., 32.);
    start_harvest(&mut vm, busy);
    tick(&mut vm, 1.);
    let host = &mut vm.host().inner;
    host.native(
        "ShowUnit",
        &[Value::Handle(hidden_worker), Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(host.idle_workers(0).unwrap(), [idle]);
    assert_eq!(host.idle_workers(1).unwrap(), [foreign]);
    assert!(host.idle_workers(2).unwrap().is_empty());
}

/// A `harvest` target order on a tree starts lumber work on that tree even where the
/// automatic search finds none (search radius 12, tree 20 away).
#[test]
fn an_explicit_harvest_order_on_a_tree_is_not_limited_by_the_search_radius() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for_search_range("9", "20", "4", "200", "12"),
        tree_catalog(),
        placed_tree(),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    create_unit(&mut vm, *b"htow", 36.0, 32.0);
    let error = vm
        .host()
        .inner
        .native(
            "IssueImmediateOrderById",
            &[Value::Handle(worker), Value::Int(900)],
        )
        .unwrap_err();
    assert!(error.contains("no harvestable tree"), "{error}");
    let issued = vm
        .host()
        .inner
        .native(
            "IssueTargetOrder",
            &[
                Value::Handle(worker),
                Value::Str("harvest".into()),
                Value::Handle(tree),
            ],
        )
        .unwrap();
    assert_eq!(issued, Value::Bool(true));
    tick(&mut vm, 0.5);
    tick(&mut vm, 2.5);
    tick(&mut vm, 4.5);
    assert!(tree_life(&mut vm, tree) < 80.0);
}

fn lumber_bank(vm: &mut Vm) -> i32 {
    let Value::Handle(player) = vm.host().inner.native("Player", &[Value::Int(0)]).unwrap() else {
        panic!("player handle")
    };
    vm.host()
        .inner
        .player_resource_value(player, "PLAYER_STATE_RESOURCE_LUMBER")
        .unwrap()
        .unwrap()
}

fn explicit_tree_order(vm: &mut Vm, worker: HandleId, tree: HandleId) -> Result<Value, String> {
    vm.host().inner.native(
        "IssueTargetOrder",
        &[
            Value::Handle(worker),
            Value::Str("harvest".into()),
            Value::Handle(tree),
        ],
    )
}

/// Original-client probes (`.scratch/depot-probe`): a lumber worker 2400 and 4400 from the only
/// town hall walked to it with a full load and deposited. Here the search radius is 12 and the
/// depot 64 away.
#[test]
fn a_loaded_lumber_worker_returns_to_a_depot_beyond_the_search_radius() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for_search_range("9", "20", "4", "200", "12"),
        tree_catalog(),
        placed_tree(),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    create_unit(&mut vm, *b"htow", 96.0, 32.0);
    let before = lumber_bank(&mut vm);
    assert_eq!(
        explicit_tree_order(&mut vm, worker, tree).unwrap(),
        Value::Bool(true)
    );
    let mut deposited = false;
    for step in 1..=120 {
        tick(&mut vm, f64::from(step) * 0.5);
        if lumber_bank(&mut vm) > before {
            deposited = true;
            break;
        }
    }
    assert!(deposited, "the load reached the distant depot");
}

#[test]
fn a_lumber_worker_without_any_owned_depot_refuses_loudly() {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables_for_search_range("9", "20", "4", "200", "12"),
        tree_catalog(),
        placed_tree(),
        harvest_fields(),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let error = explicit_tree_order(&mut vm, worker, tree).unwrap_err();
    assert!(error.contains("no lumber-accepting depot owned"), "{error}");
}

/// The explicit gold order also accepts a depot within the search radius of the mine when the
/// worker itself is outside it (worker 90 from the depot, mine 64, radius 70).
#[test]
fn an_explicit_gold_order_accepts_a_depot_near_the_mine_but_far_from_the_worker() {
    let (mut vm, _) = open_harvest_with_budget(
        harvest_tables_for_search_range("9", "20", "4", "20", "70"),
        tree_catalog(),
        placed_tree(),
        harvest_fields(),
        crate::playercolor_host::harvest_runtime::Budget::new(4, 64),
    );
    let worker = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    let depot = create_unit(&mut vm, *b"htow", 96.0, 96.0);
    let mine = create_unit(&mut vm, *b"ngol", 96.0, 32.0);
    gold_role(&mut vm, worker, "Ahar", false, None);
    gold_role(&mut vm, depot, "Argl", true, None);
    gold_role(&mut vm, mine, "Agld", true, Some(500));
    start_gold(&mut vm, worker, mine);
}
