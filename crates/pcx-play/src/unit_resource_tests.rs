use super::super::{COMMON, budget, context, object, slots_rule};
use super::sylk;
use super::{limits, source};
use crate::playercolor_host::{Error, ObjectHost};
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_objects::editor::{FieldDef, FieldKind, FieldSource, ObjectKind, SlkId};
use pcx_script::{Host, Value};
use pcx_sim::world::{Definition, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(super) const RESOURCE_FAMILY_NATIVES: &str = "native SetResourceAmount takes unit whichUnit, integer amount returns nothing\nnative AddResourceAmount takes unit whichUnit, integer amount returns nothing\nnative GetResourceAmount takes unit whichUnit returns integer\n";

const RESOURCE_NATIVES: &str = concat!(
    "native SetResourceAmount takes unit whichUnit, integer amount returns nothing\nnative AddResourceAmount takes unit whichUnit, integer amount returns nothing\nnative GetResourceAmount takes unit whichUnit returns integer\n",
    "native RemoveUnit takes unit u returns nothing\n"
);

const RESOURCE_MAP: &str = "globals\nplayer P = null\nunit U = null\nunit E = null\nunit G = null\nunit F = null\ninteger I = 0\ntimer T = null\nendglobals\nfunction main takes nothing returns nothing\nset P = Player(4)\nset U = CreateUnit(P, 'ngol', 1.5, -2.25, 90.0)\nset E = CreateUnit(P, 'egol', 2.5, -3.25, 90.0)\nset G = CreateUnit(P, 'ugol', 3.5, -4.25, 90.0)\nset F = CreateUnit(P, 'hfoo', 3.0, 4.0, 0.0)\nendfunction\nfunction setMine takes nothing returns nothing\ncall SetResourceAmount(U, 20000)\nendfunction\nfunction setNegative takes nothing returns nothing\ncall SetResourceAmount(U, -7)\nendfunction\nfunction add takes nothing returns nothing\ncall AddResourceAmount(U, 5)\nendfunction\nfunction addUnset takes nothing returns nothing\ncall AddResourceAmount(U, 5)\nendfunction\nfunction addAll takes nothing returns nothing\ncall AddResourceAmount(U, 5)\ncall AddResourceAmount(E, 5)\ncall AddResourceAmount(G, 5)\nendfunction\nfunction addMax takes nothing returns nothing\ncall AddResourceAmount(U, 2147483647)\nendfunction\nfunction read takes nothing returns nothing\nset I = GetResourceAmount(U)\nendfunction\nfunction readAll takes nothing returns nothing\nset I = GetResourceAmount(U)\nendfunction\nfunction readEntangled takes nothing returns nothing\nset I = GetResourceAmount(E)\nendfunction\nfunction setFootman takes nothing returns nothing\ncall SetResourceAmount(F, 9)\nendfunction\nfunction readFootman takes nothing returns nothing\nset I = GetResourceAmount(F)\nendfunction\nfunction nullSet takes nothing returns nothing\ncall SetResourceAmount(null, 3)\nendfunction\nfunction nullGet takes nothing returns nothing\nset I = GetResourceAmount(null)\nendfunction\nfunction removeAndRead takes nothing returns nothing\ncall RemoveUnit(U)\nset I = GetResourceAmount(U)\nendfunction\nfunction removedWriteRead takes nothing returns nothing\ncall SetResourceAmount(U, 41)\ncall AddResourceAmount(U, 1)\nset I = GetResourceAmount(U)\nendfunction\nfunction timerSet takes nothing returns nothing\nset T = CreateTimer()\ncall SetResourceAmount(T, 3)\nendfunction\n";

struct ResourceCatalog;

impl ObjectCatalog for ResourceCatalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        match &rawcode {
            b"ngol" => Ok(resource_definition(
                *b"ngol",
                "ngol",
                "resource",
                "Agld,Avul",
            )),
            b"egol" => Ok(resource_definition(
                *b"egol",
                "egol",
                "resource",
                "Abds,Aenc,Slo2,Adri,Aegm",
            )),
            b"ugol" => Ok(resource_definition(
                *b"ugol",
                "ugol",
                "resource",
                "Abgl,Abgm",
            )),
            b"hfoo" => Ok(resource_definition(*b"hfoo", "hfoo", "_", "Adef,Aihn")),
            _ => Err(ResolutionProblem::MissingRawcode(pcx_objects::fourcc_text(
                rawcode,
            ))),
        }
    }
}

pub(super) fn resource_fields(buff_type: &str, abilities: &str) -> Vec<([u8; 4], ObjectField)> {
    let mut fields = BTreeMap::new();
    fields.insert(
        *b"uabt",
        ObjectField {
            metadata: Some(FieldDef {
                fourcc: *b"uabt",
                from: SlkId::UnitMetaData,
                field: "buffType".into(),
                slk: "UnitData".into(),
                index: -1,
                declared: "string".into(),
                kind: FieldKind::Token,
                category: String::new(),
                source: FieldSource::Column {
                    slk: SlkId::UnitData,
                    column: "buffType".into(),
                },
                measured: true,
            }),
            value: Ok(FieldValue::Cell(buff_type.into())),
        },
    );
    fields.insert(
        *b"uabi",
        ObjectField {
            metadata: Some(FieldDef {
                fourcc: *b"uabi",
                from: SlkId::UnitMetaData,
                field: "abilList".into(),
                slk: "UnitAbilities".into(),
                index: -1,
                declared: "string".into(),
                kind: FieldKind::Token,
                category: String::new(),
                source: FieldSource::Column {
                    slk: SlkId::UnitAbilities,
                    column: "abilList".into(),
                },
                measured: true,
            }),
            value: Ok(FieldValue::Cell(abilities.into())),
        },
    );
    fields.into_iter().collect()
}

fn resource_definition(
    rawcode: [u8; 4],
    base: &str,
    buff_type: &str,
    abilities: &str,
) -> Definition {
    Definition {
        rawcode,
        base_rawcode: base.into(),
        kind: ObjectKind::Unit,
        fields: resource_fields(buff_type, abilities).into_iter().collect(),
        modifications: Vec::new(),
    }
}

fn resource_tables_with(rows: &[&[&str]]) -> Arc<DefaultTables> {
    let data = sylk(&["alias", "code", "levels", "DataA1", "DataB1"], rows);
    Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic resource".into(),
                edition_version: "fixture".into(),
            },
            &[(SlkId::AbilityData.path(), &data)],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 1,
                max_asset_bytes: 65536,
                max_aggregate_input_bytes: 131072,
            },
        )
        .expect("resource ability tables"),
    )
}

pub(super) fn resource_tables() -> Arc<DefaultTables> {
    resource_tables_with(&[
        &["Agld", "Agld", "1", "4242", "1"],
        &["Avul", "Avul", "1", "-", "-"],
        &["Abds", "Abds", "1", "64", "0"],
        &["Aenc", "Aenc", "1", "5", "-"],
        &["Slo2", "Slo2", "1", "-", "-"],
        &["Adri", "Adri", "1", "-", "-"],
        &["Aegm", "Aegm", "1", "10", "1"],
        &["Abgl", "Abgl", "1", "64", "1"],
        &["Abgm", "Abgm", "1", "10", "1"],
    ])
}

fn resource_common(natives: &str) -> String {
    let globals = COMMON.find("\nglobals\n").expect("globals");
    let mut common = COMMON.to_string();
    common.insert_str(
        globals + 1,
        &format!(
            "type widget extends agent\ntype unit extends widget\nnative CreateUnit takes player id, integer unitid, real x, real y, real face returns unit\nnative SetUnitAcquireRange takes unit whichUnit, real newAcquireRange returns nothing\nnative SetHeroLevel takes unit whichHero, integer level, boolean showEyeCandy returns nothing\nnative TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing\nnative TimerGetElapsed takes timer t returns real\nnative TimerGetRemaining takes timer t returns real\nnative TimerGetTimeout takes timer t returns real\nnative GetExpiredTimer takes nothing returns timer\n{natives}"
        ),
    );
    common
}

type ResourceSession = (
    pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>,
    Arc<dyn ObjectCatalog + Send + Sync>,
);

fn resource_host(natives: &str, opt_in: bool) -> Result<ResourceSession, Error> {
    resource_host_with(RESOURCE_MAP, natives, opt_in)
}

fn resource_host_with(map: &str, natives: &str, opt_in: bool) -> Result<ResourceSession, Error> {
    resource_host_tables(map, natives, opt_in.then(resource_tables))
}

fn resource_host_tables(
    map: &str,
    natives: &str,
    tables: Option<Arc<DefaultTables>>,
) -> Result<ResourceSession, Error> {
    let common = resource_common(natives);
    let ctx = context(300, &common, map, "unit-catalog");
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(ResourceCatalog);
    let host = object(&ctx, 64)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(4))?;
    let host = match tables {
        Some(tables) => host.with_resource_amounts(tables)?,
        None => host,
    };
    let host = host.with_timer_runtime(0.0)?;
    Ok((Arc::clone(&ctx.scripts).owned_vm(host), shared))
}

fn amount(host: &ObjectHost, unit: pcx_script::HandleId) -> Option<i32> {
    host.executed_unit(unit).unwrap().resource_amount
}

#[test]
fn resource_amounts_store_exact_integers_for_the_unitdata_resource_family() {
    let (mut vm, _) = resource_host(RESOURCE_NATIVES, true).expect("resource admission");
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let Value::Handle(mine) = vm.global("U") else {
        panic!("mine")
    };
    let Value::Handle(footman) = vm.global("F") else {
        panic!("footman")
    };

    assert_eq!(amount(&vm.host().inner, mine), Some(4242));
    vm.call("read", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(4242));

    vm.call("setMine", &[]).unwrap();
    assert_eq!(amount(&vm.host().inner, mine), Some(20000));
    vm.call("read", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(20000));

    vm.call("addMax", &[]).unwrap();
    assert_eq!(
        amount(&vm.host().inner, mine),
        Some(20000i32.wrapping_add(i32::MAX))
    );

    vm.call("setNegative", &[]).unwrap();
    assert_eq!(amount(&vm.host().inner, mine), Some(-7));
    vm.call("read", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(-7));

    vm.call("add", &[]).unwrap();
    assert_eq!(amount(&vm.host().inner, mine), Some(-2));
    vm.call("read", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(-2));

    vm.call("setFootman", &[]).unwrap();
    assert_eq!(amount(&vm.host().inner, footman), None);
    vm.call("readFootman", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(0));
}

#[test]
fn add_before_set_uses_the_authored_constructor_seed() {
    let (mut vm, _) = resource_host(RESOURCE_NATIVES, true).expect("resource admission");
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let Value::Handle(mine) = vm.global("U") else {
        panic!("mine")
    };
    let Value::Handle(entangled) = vm.global("E") else {
        panic!("entangled")
    };
    let Value::Handle(blighted) = vm.global("G") else {
        panic!("blighted")
    };

    vm.call("addAll", &[]).unwrap();
    assert_eq!(amount(&vm.host().inner, mine), Some(4247));
    assert_eq!(amount(&vm.host().inner, entangled), Some(5));
    assert_eq!(amount(&vm.host().inner, blighted), Some(5));
    vm.call("read", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(4247));
    vm.call("readEntangled", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(5));

    vm.call("setMine", &[]).unwrap();
    vm.call("add", &[]).unwrap();
    assert_eq!(amount(&vm.host().inner, mine), Some(20005));
}

#[test]
fn removed_resource_amount_is_retained_on_the_removal_tick_only() {
    let (mut vm, _) = resource_host(RESOURCE_NATIVES, true).expect("resource admission");
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let Value::Handle(mine) = vm.global("U") else {
        panic!("mine")
    };

    vm.call("nullSet", &[]).unwrap();
    vm.call("nullGet", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(0));
    assert_eq!(amount(&vm.host().inner, mine), Some(4242));

    vm.host().inner.set_timer_game_time(5.0).unwrap();
    vm.call("setMine", &[]).unwrap();
    vm.call("removeAndRead", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(20000));
    vm.call("removedWriteRead", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(42));
    assert!(vm.host().inner.executed_unit(mine).is_err());

    vm.host().inner.set_timer_game_time(5.5).unwrap();
    vm.call("read", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(0));
    vm.call("removedWriteRead", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(0));
}

#[test]
fn unreadable_resource_seeds_refuse_unit_creation_atomically() {
    let missing = resource_tables_with(&[&["Avul", "Avul", "1", "-", "-"]]);
    let malformed = resource_tables_with(&[
        &["Agld", "Agld", "1", "not-an-integer", "1"],
        &["Avul", "Avul", "1", "-", "-"],
    ]);
    for tables in [missing, malformed] {
        let (mut vm, _) = resource_host_tables(RESOURCE_MAP, RESOURCE_NATIVES, Some(tables))
            .expect("resource admission");
        vm.init_globals().unwrap();
        let error = vm.call("main", &[]).unwrap_err().to_string();
        assert!(
            error.contains("resource ability seed is not readable"),
            "{error}"
        );
        assert!(error.contains("ngol"), "{error}");
        assert!(error.contains("Agld"), "{error}");
        assert!(vm.host().inner.executed_units().next().is_none());
        assert_eq!(vm.global("U"), Value::Null);
    }
}

#[test]
fn resource_amounts_refuse_foreign_handles_and_malformed_arguments() {
    let (mut vm, _) = resource_host(RESOURCE_NATIVES, true).expect("resource admission");
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let Value::Handle(mine) = vm.global("U") else {
        panic!("mine")
    };
    let Value::Handle(player) = vm.global("P") else {
        panic!("player")
    };

    let foreign = vm.call("timerSet", &[]).unwrap_err().to_string();
    assert!(foreign.contains("another family"), "{foreign}");
    let player_set = vm
        .host()
        .native("SetResourceAmount", &[Value::Handle(player), Value::Int(5)])
        .unwrap_err()
        .to_string();
    assert!(player_set.contains("another family"), "{player_set}");
    let player_get = vm
        .host()
        .native("GetResourceAmount", &[Value::Handle(player)])
        .unwrap_err()
        .to_string();
    assert!(player_get.contains("another family"), "{player_get}");

    for args in [
        vec![Value::Handle(mine), Value::Real(1.5)],
        vec![Value::Handle(mine)],
        vec![Value::Int(1), Value::Int(2)],
    ] {
        let error = vm
            .host()
            .native("SetResourceAmount", &args)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("arguments do not match the unit native declaration"),
            "{error}"
        );
    }
    let get_malformed = vm
        .host()
        .native("GetResourceAmount", &[])
        .unwrap_err()
        .to_string();
    assert!(
        get_malformed.contains("arguments do not match the unit native declaration"),
        "{get_malformed}"
    );
    assert_eq!(amount(&vm.host().inner, mine), Some(4242));
}

#[test]
fn resource_amount_admission_requires_the_original_setter_contract() {
    let constant = RESOURCE_NATIVES.replace(
        "native SetResourceAmount",
        "constant native SetResourceAmount",
    );
    let wrong = "native SetResourceAmount takes unit whichUnit returns nothing\n";
    for natives in [constant.as_str(), wrong] {
        let error = match resource_host_with(
            "// no globals\nfunction main takes nothing returns nothing\nendfunction\n",
            natives,
            true,
        ) {
            Err(error) => error,
            Ok(_) => panic!("bad resource setter was admitted"),
        };
        assert_eq!(error.failure, crate::playercolor_host::Failure::Signature);
        assert_eq!(error.native, "SetResourceAmount");
    }
    for natives in [
        "",
        "native GetResourceAmount takes unit whichUnit returns integer\n",
    ] {
        let (mut vm, _) = resource_host_with(
            "// no globals\nfunction main takes nothing returns nothing\nendfunction\n",
            natives,
            true,
        )
        .expect("absent setter leaves the family unsupported");
        vm.init_globals().unwrap();
        vm.call("main", &[]).unwrap();
        assert!(vm.host().native("GetResourceAmount", &[]).is_err());
    }

    let (mut vm, _) = resource_host(RESOURCE_NATIVES, false).expect("declined admission");
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert!(vm.host().native("SetResourceAmount", &[]).is_err());
    assert!(vm.host().native("GetResourceAmount", &[]).is_err());
}

#[test]
fn constant_get_resource_amount_is_admitted() {
    let natives = RESOURCE_NATIVES.replace(
        "native GetResourceAmount",
        "constant native GetResourceAmount",
    );
    let (mut vm, _) = resource_host(&natives, true).expect("constant getter");
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    vm.call("setMine", &[]).unwrap();
    vm.call("read", &[]).unwrap();
    assert_eq!(vm.global("I"), Value::Int(20000));
}

#[test]
fn resource_amount_setter_from_another_source_is_refused() {
    use crate::playercolor_host::{EnumHost, Failure, Limits};
    let primary = resource_common("");
    let other = "native SetResourceAmount takes unit whichUnit, integer amount returns nothing\n";
    let map = pcx_script::parse_script("").unwrap();
    let primary = pcx_script::parse_script(&primary).unwrap();
    let other = pcx_script::parse_script(other).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                pcx_script::Prelude::new("synthetic/common.j", &primary),
                pcx_script::Prelude::new("synthetic/other.j", &other),
            ],
        )
        .unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "split-source",
        &[],
        Limits {
            max_handles: 32,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(ResourceCatalog);
    let error = match ObjectHost::checked(inner)
        .unwrap()
        .with_unit_catalog(shared, source(), limits(1))
        .unwrap()
        .with_resource_amounts(resource_tables())
    {
        Err(error) => error,
        Ok(_) => panic!("foreign resource setter source was admitted"),
    };
    assert_eq!(error.failure, Failure::Source);
    assert_eq!(error.native, "SetResourceAmount");
    assert_eq!(error.declaration_source, "synthetic/other.j");
}

#[test]
fn resource_amount_setter_without_add_or_get_is_admitted() {
    let natives = "native SetResourceAmount takes unit whichUnit, integer amount returns nothing\n";
    let (mut vm, _) = resource_host_with(
        "// no globals\nfunction main takes nothing returns nothing\nendfunction\n",
        natives,
        true,
    )
    .expect("setter only");
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
}
