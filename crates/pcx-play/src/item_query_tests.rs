use super::super::ItemBudget;
use crate::item_class_table::ItemClassTable;
use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_objects::editor::ObjectKind;
use pcx_objects::Value as FieldOverride;
use crate::scripts::AssetIdentity;
use pcx_script::{HandleId, Host, Prelude};
use pcx_sim::world::{Definition, FieldProblem, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem, Source};
use std::collections::BTreeMap;

const DECLARATION: &str = "\
native GetItemLevel takes item whichItem returns integer
native GetItemType takes item whichItem returns itemtype
constant native ConvertItemType takes integer i returns itemtype
globals
constant itemtype ITEM_TYPE_PERMANENT = ConvertItemType(0)
constant itemtype ITEM_TYPE_CHARGED = ConvertItemType(1)
constant itemtype ITEM_TYPE_POWERUP = ConvertItemType(2)
constant itemtype ITEM_TYPE_ARTIFACT = ConvertItemType(3)
constant itemtype ITEM_TYPE_PURCHASABLE = ConvertItemType(4)
constant itemtype ITEM_TYPE_CAMPAIGN = ConvertItemType(5)
constant itemtype ITEM_TYPE_MISCELLANEOUS = ConvertItemType(6)
constant itemtype ITEM_TYPE_UNKNOWN = ConvertItemType(7)
endglobals
";
const BASE: &str = "\
type agent extends handle
type itemtype extends handle
type timer extends agent
type widget extends agent
type item extends widget
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native CreateItem takes integer typeId, real x, real y returns item
";

/// Level and class cells by rawcode. `NOCL` has no class; `MISS` has a level that is absent.
struct Catalog;
impl ObjectCatalog for Catalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let cell = |text: &str| Ok(FieldValue::Cell(text.into()));
        let mut fields = BTreeMap::new();
        let mut put = |fourcc: &[u8; 4], value| {
            fields.insert(*fourcc, ObjectField { metadata: None, value });
        };
        match &rawcode {
            b"LVL3" => {
                put(b"ilev", cell("3"));
                put(b"icla", cell("Permanent"));
            }
            b"TEXT" => put(b"ilev", cell("three")),
            b"MISS" => put(b"ilev", Err(FieldProblem::Missing)),
            b"OVRD" => {
                put(b"ilev", Ok(FieldValue::Override(FieldOverride::Int(9))));
                put(b"icla", cell("PowerUp"));
            }
            b"MISC" => {
                put(b"ilev", cell("1"));
                put(b"icla", cell("Miscellaneous"));
            }
            b"UPPR" => {
                put(b"ilev", cell("1"));
                put(b"icla", cell("CHARGED"));
            }
            b"NOCL" => put(b"ilev", cell("1")),
            _ => {}
        }
        Ok(Definition {
            rawcode,
            base_rawcode: "synthetic base".into(),
            kind: ObjectKind::Item,
            fields,
            modifications: Vec::new(),
        })
    }
}

fn source() -> Source {
    Source {
        install: "synthetic install".into(),
        edition: "synthetic edition/revision".into(),
        map: "synthetic map".into(),
    }
}

fn enums(declaration: &str) -> EnumHost {
    let common = pcx_script::parse_script(&format!("{BASE}{declaration}")).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = std::sync::Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    EnumHost::checked_selected(
        program,
        "synthetic:map",
        &["ConvertItemType"],
        Limits {
            max_handles: 16,
            admission_steps: 1024,
            expression_depth: 16,
            expression_work: 1024,
        },
    )
    .unwrap()
}

/// Same ordinals as the 1.29b chain: each name maps to its `ITEM_TYPE` value.
fn classes() -> ItemClassTable {
    ItemClassTable {
        origin: AssetIdentity {
            edition: "synthetic".into(),
            version: "1.29.2.9231".into(),
            source: "synthetic".into(),
        },
        image_sha256: [0; 32],
        ordinals: BTreeMap::from([
            ("artifact".to_owned(), 3),
            ("campaign".to_owned(), 5),
            ("charged".to_owned(), 1),
            ("miscellanous".to_owned(), 6),
            ("permanent".to_owned(), 0),
            ("powerup".to_owned(), 2),
            ("purchasable".to_owned(), 4),
        ]),
        default: 7,
    }
}

fn host() -> ObjectHost {
    ObjectHost::checked(enums(DECLARATION))
        .unwrap()
        .with_item_catalog(
            std::sync::Arc::new(Catalog),
            source(),
            ItemBudget {
                max_records: 8,
                max_retained_fields: 8,
                max_modifications: 8,
            },
        )
        .unwrap()
        .with_item_queries(classes())
        .unwrap()
}

fn item(h: &mut ObjectHost, rawcode: &[u8; 4]) -> HandleId {
    let Value::Handle(id) = h
        .native(
            "CreateItem",
            &[Value::Int(i32::from_be_bytes(*rawcode)), Value::Real(0.0), Value::Real(0.0)],
        )
        .unwrap()
    else {
        panic!("CreateItem returns a handle");
    };
    id
}

/// Creates an item of `rawcode` and queries it with the named native.
fn query(h: &mut ObjectHost, name: &str, rawcode: &[u8; 4]) -> Result<Value, String> {
    let id = item(h, rawcode);
    h.native(name, &[Value::Handle(id)])
}

/// The program's own `ConvertItemType(n)` identity, to compare against query results.
fn itemtype(h: &mut ObjectHost, n: i32) -> Value {
    h.native("ConvertItemType", &[Value::Int(n)]).unwrap()
}

#[test]
fn admission_checks_declarations_and_ordinals_against_the_program_once() {
    let convert = "constant native ConvertItemType takes integer i returns itemtype\nglobals\nconstant itemtype ITEM_TYPE_PERMANENT = ConvertItemType(0)\nendglobals\n";
    let wrong_level = enums(&format!("native GetItemLevel takes item whichItem returns real\nnative GetItemType takes item whichItem returns itemtype\n{convert}"));
    assert!(ObjectHost::checked(wrong_level).unwrap().with_item_queries(classes()).is_err());
    let missing = enums(convert);
    assert!(ObjectHost::checked(missing).unwrap().with_item_queries(classes()).is_err());
    let mut outside = classes();
    outside.ordinals.insert("powerup".to_owned(), 99);
    assert!(ObjectHost::checked(enums(DECLARATION)).unwrap().with_item_queries(outside).is_err());
    assert!(host().with_item_queries(classes()).is_err());
}

#[test]
fn level_is_the_authored_cell_or_typed_override_and_refuses_otherwise() {
    let mut h = host();
    assert_eq!(query(&mut h, "GetItemLevel", b"LVL3"), Ok(Value::Int(3)));
    assert_eq!(query(&mut h, "GetItemLevel", b"OVRD"), Ok(Value::Int(9)));
    assert!(query(&mut h, "GetItemLevel", b"TEXT").is_err());
    assert!(query(&mut h, "GetItemLevel", b"MISS").is_err());
    assert!(query(&mut h, "GetItemLevel", b"NOCL").is_ok());
}

#[test]
fn type_is_the_program_identity_of_the_class_ordinal() {
    let mut h = host();
    let permanent = item(&mut h, b"LVL3");
    assert_eq!(h.native("GetItemType", &[Value::Handle(permanent)]), Ok(itemtype(&mut h, 0)));
    let powerup = item(&mut h, b"OVRD");
    assert_eq!(h.native("GetItemType", &[Value::Handle(powerup)]), Ok(itemtype(&mut h, 2)));
    let upper = item(&mut h, b"UPPR");
    assert_eq!(h.native("GetItemType", &[Value::Handle(upper)]), Ok(itemtype(&mut h, 1)));
}

#[test]
fn unmatched_and_missing_classes_take_the_chain_default() {
    let mut h = host();
    // The 1.29b chain compares against "miscellanous", so the correct spelling is unmatched.
    let misc = item(&mut h, b"MISC");
    assert_eq!(h.native("GetItemType", &[Value::Handle(misc)]), Ok(itemtype(&mut h, 7)));
    let classless = item(&mut h, b"NOCL");
    assert_eq!(h.native("GetItemType", &[Value::Handle(classless)]), Ok(itemtype(&mut h, 7)));
}

#[test]
fn queries_refuse_a_non_item_handle_and_a_wrong_argument() {
    let mut h = host();
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(h.native("GetItemLevel", std::slice::from_ref(&timer)).is_err());
    assert!(h.native("GetItemType", &[timer]).is_err());
    assert!(h.native("GetItemLevel", &[Value::Real(1.0)]).is_err());
}

#[test]
fn unadmitted_host_does_not_answer_the_queries() {
    let mut h = ObjectHost::checked(enums(DECLARATION)).unwrap();
    assert!(h.native("GetItemType", &[Value::Null]).is_err());
}
