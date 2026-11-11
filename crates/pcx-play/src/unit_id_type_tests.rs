use super::*;

/// The `unittype` numbers of `common.j`.
const KINDS: [(&str, i32); 13] = [
    ("HERO", 0),
    ("DEAD", 1),
    ("FLYING", 3),
    ("GROUND", 4),
    ("ATTACKS_FLYING", 5),
    ("ATTACKS_GROUND", 6),
    ("MELEE_ATTACKER", 7),
    ("RANGED_ATTACKER", 8),
    ("SUMMONED", 10),
    ("STUNNED", 11),
    ("UNDEAD", 14),
    ("TOWNHALL", 18),
    ("TAUREN", 20),
];

/// The fixture row `U001` (uppercase first letter) with these source columns.
const ROW: [u8; 4] = *b"U001";

/// The fixture catalog resolves every rawcode; this one lacks `ABSENT`.
const ABSENT: [u8; 4] = *b"zzz9";

struct Gapped(Arc<dyn pcx_sim::world::ObjectCatalog + Send + Sync>);

impl pcx_sim::world::ObjectCatalog for Gapped {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, pcx_sim::world::ResolutionProblem> {
        if rawcode == ABSENT {
            return Err(pcx_sim::world::ResolutionProblem::MissingRawcode(
                pcx_objects::fourcc_text(rawcode),
            ));
        }
        self.0.resolve(rawcode)
    }
}

fn declarations() -> String {
    let mut text = String::from(
        "constant native IsUnitIdType takes integer unitId, unittype whichUnitType returns boolean\nglobals\n",
    );
    for (name, value) in KINDS {
        text.push_str(&format!(
            "constant unittype UNIT_TYPE_{name}=ConvertUnitType({value})\n"
        ));
    }
    text.push_str("endglobals\n");
    text
}

fn column_field(
    fourcc: &[u8; 4],
    slk: SlkId,
    column: &str,
    kind: FieldKind,
    value: &str,
) -> ([u8; 4], ObjectField) {
    let mut field = geometry_field(Ok(FieldValue::Cell(value.into())));
    let meta = field.metadata.as_mut().unwrap();
    meta.fourcc = *fourcc;
    meta.field = column.into();
    meta.kind = kind;
    meta.declared = "string".into();
    meta.source = FieldSource::Column {
        slk,
        column: column.into(),
    };
    (*fourcc, field)
}

struct Row<'a> {
    structure: &'a str,
    movetp: &'a str,
    types: &'a str,
    weapons_on: &'a str,
    weapon_types: [&'a str; 2],
    targets: [&'a str; 2],
}

const PLAIN: Row = Row {
    structure: "0",
    movetp: "foot",
    types: "_",
    weapons_on: "0",
    weapon_types: ["_", "_"],
    targets: ["_", "_"],
};

fn open_row(row: &Row) -> Vm {
    let fields = [
        column_field(
            b"F001",
            SlkId::UnitBalance,
            "isbldg",
            FieldKind::Bool,
            row.structure,
        ),
        column_field(
            b"M001",
            SlkId::UnitData,
            "movetp",
            FieldKind::Token,
            row.movetp,
        ),
        column_field(
            b"T001",
            SlkId::UnitBalance,
            "type",
            FieldKind::Token,
            row.types,
        ),
        column_field(
            b"W001",
            SlkId::UnitWeapons,
            "weapsOn",
            FieldKind::Int,
            row.weapons_on,
        ),
        column_field(
            b"W002",
            SlkId::UnitWeapons,
            "weapTp1",
            FieldKind::Token,
            row.weapon_types[0],
        ),
        column_field(
            b"W003",
            SlkId::UnitWeapons,
            "weapTp2",
            FieldKind::Token,
            row.weapon_types[1],
        ),
        column_field(
            b"W004",
            SlkId::UnitWeapons,
            "targs1",
            FieldKind::Token,
            row.targets[0],
        ),
        column_field(
            b"W005",
            SlkId::UnitWeapons,
            "targs2",
            FieldKind::Token,
            row.targets[1],
        ),
    ];
    open_admitting_with(&fields, rules(), &declarations(), |h| {
        let inner = h.unit_catalog().unwrap();
        h.construction_test_catalog(Arc::new(Gapped(inner)));
    })
}

fn ask(vm: &mut Vm, rawcode: [u8; 4], kind: &str) -> Result<Value, String> {
    let kind = vm.global(&format!("UNIT_TYPE_{kind}"));
    vm.host()
        .inner
        .native(
            "IsUnitIdType",
            &[Value::Int(i32::from_be_bytes(rawcode)), kind],
        )
        .map_err(|e| e.to_string())
}

fn is(vm: &mut Vm, rawcode: [u8; 4], kind: &str) -> bool {
    match ask(vm, rawcode, kind).unwrap() {
        Value::Bool(answer) => answer,
        other => panic!("{other:?}"),
    }
}

#[test]
fn hero_reads_the_rawcode_shape_and_structure_the_type_flag() {
    let mut vm = open_row(&Row {
        structure: "1",
        ..PLAIN
    });
    assert!(is(&mut vm, ROW, "STRUCTURE"));
    assert!(is(&mut vm, ROW, "HERO"));
    let mut vm = open_row(&PLAIN);
    assert!(!is(&mut vm, ROW, "STRUCTURE"));
    // Case 0 reads the rawcode alone: a lowercase first character is not a hero.
    assert!(!is(&mut vm, *b"u001", "HERO"));
    assert!(is(&mut vm, *b"Hxyz", "HERO"));
}

#[test]
fn class_tokens_read_the_type_list_including_summoned() {
    let mut vm = open_row(&Row {
        types: "UNDEAD, SUMMONED",
        ..PLAIN
    });
    assert!(is(&mut vm, ROW, "UNDEAD"));
    assert!(is(&mut vm, ROW, "SUMMONED"));
    assert!(!is(&mut vm, ROW, "TOWNHALL"));
    assert!(!is(&mut vm, ROW, "TAUREN"));
}

#[test]
fn live_only_types_read_false_for_a_type() {
    let mut vm = open_row(&Row {
        types: "UNDEAD",
        ..PLAIN
    });
    assert!(!is(&mut vm, ROW, "DEAD"));
    assert!(!is(&mut vm, ROW, "STUNNED"));
}

#[test]
fn flying_is_movement_code_two_and_ground_compares_with_thirteen() {
    let mut vm = open_row(&Row {
        movetp: "fly",
        ..PLAIN
    });
    assert!(is(&mut vm, ROW, "FLYING"));
    assert!(!is(&mut vm, ROW, "GROUND"));
    // No single `movetp` token has code 13, so a walking type is not GROUND either.
    for token in ["foot", "horse", "hover", "float", "amph", "_"] {
        let mut vm = open_row(&Row {
            movetp: token,
            ..PLAIN
        });
        assert!(!is(&mut vm, ROW, "FLYING"), "{token}");
        assert!(!is(&mut vm, ROW, "GROUND"), "{token}");
    }
}

#[test]
fn attack_reach_ors_the_enabled_slots_target_masks() {
    let both = Row {
        weapons_on: "3",
        weapon_types: ["normal", "missile"],
        targets: ["ground,structure", "air"],
        ..PLAIN
    };
    let mut vm = open_row(&both);
    assert!(is(&mut vm, ROW, "ATTACKS_GROUND"));
    assert!(is(&mut vm, ROW, "ATTACKS_FLYING"));
    let first_only = Row {
        weapons_on: "1",
        ..both
    };
    let mut vm = open_row(&first_only);
    assert!(is(&mut vm, ROW, "ATTACKS_GROUND"));
    assert!(!is(&mut vm, ROW, "ATTACKS_FLYING"));
    let second_only = Row {
        weapons_on: "2",
        ..both
    };
    let mut vm = open_row(&second_only);
    assert!(!is(&mut vm, ROW, "ATTACKS_GROUND"));
    assert!(is(&mut vm, ROW, "ATTACKS_FLYING"));
    let mut vm = open_row(&Row {
        weapons_on: "0",
        ..both
    });
    assert!(!is(&mut vm, ROW, "ATTACKS_GROUND"));
    assert!(!is(&mut vm, ROW, "ATTACKS_FLYING"));
}

#[test]
fn melee_and_ranged_follow_the_weapon_number_and_exclude_four_exact_masks() {
    let mut vm = open_row(&Row {
        weapons_on: "1",
        weapon_types: ["normal", "_"],
        targets: ["ground", "_"],
        ..PLAIN
    });
    assert!(is(&mut vm, ROW, "MELEE_ATTACKER"));
    assert!(!is(&mut vm, ROW, "RANGED_ATTACKER"));
    for token in [
        "missile",
        "artillery",
        "instant",
        "msplash",
        "mbounce",
        "mline",
        "aline",
    ] {
        let mut vm = open_row(&Row {
            weapons_on: "1",
            weapon_types: [token, "_"],
            targets: ["ground", "_"],
            ..PLAIN
        });
        assert!(!is(&mut vm, ROW, "MELEE_ATTACKER"), "{token}");
        assert!(is(&mut vm, ROW, "RANGED_ATTACKER"), "{token}");
    }
    // A mask that is exactly none, tree, wall or debris is not an attack; a union is.
    for (targets, counts) in [
        ("none", false),
        ("tree", false),
        ("wall", false),
        ("debris", false),
        ("tree,wall", true),
        ("tree,ground", true),
    ] {
        let mut vm = open_row(&Row {
            weapons_on: "1",
            weapon_types: ["normal", "_"],
            targets: [targets, "_"],
            ..PLAIN
        });
        assert_eq!(is(&mut vm, ROW, "MELEE_ATTACKER"), counts, "{targets}");
    }
    // Slot 2 counts when its bit is enabled, and a disabled slot never does.
    let second = Row {
        weapons_on: "2",
        weapon_types: ["normal", "missile"],
        targets: ["ground", "air"],
        ..PLAIN
    };
    let mut vm = open_row(&second);
    assert!(!is(&mut vm, ROW, "MELEE_ATTACKER"));
    assert!(is(&mut vm, ROW, "RANGED_ATTACKER"));
}

#[test]
fn a_rawcode_without_a_catalog_row_has_no_record() {
    let mut vm = open_row(&PLAIN);
    for kind in [
        "FLYING",
        "ATTACKS_FLYING",
        "MELEE_ATTACKER",
        "UNDEAD",
        "GROUND",
    ] {
        assert!(!is(&mut vm, ABSENT, kind), "{kind}");
    }
    let error = ask(&mut vm, ABSENT, "STRUCTURE").unwrap_err().to_string();
    assert!(error.contains("no catalog row"), "{error}");
}

#[test]
fn a_weapon_type_without_a_decoded_number_refuses() {
    let mut vm = open_row(&Row {
        weapons_on: "1",
        weapon_types: ["bogus", "_"],
        targets: ["ground", "_"],
        ..PLAIN
    });
    let error = ask(&mut vm, ROW, "MELEE_ATTACKER").unwrap_err().to_string();
    assert!(error.contains("weapTp1"), "{error}");
}

#[test]
fn argument_shapes_are_checked() {
    let mut vm = open_row(&PLAIN);
    let unit = unit(&vm, "A");
    let host = &mut vm.host().inner;
    assert!(host.native("IsUnitIdType", &[Value::Int(1)]).is_err());
    assert!(
        host.native("IsUnitIdType", &[Value::Handle(unit), Value::Int(1)])
            .is_err()
    );
}
