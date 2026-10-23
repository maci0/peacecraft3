use super::*;

fn fields(day: &str, night: &str) -> Vec<([u8; 4], ObjectField)> {
    [(b"V001", "sight", day), (b"V002", "nsight", night)]
        .into_iter()
        .map(|(id, column, value)| {
            let mut field = geometry_field(Ok(FieldValue::Cell(value.into())));
            let metadata = field.metadata.as_mut().unwrap();
            metadata.fourcc = *id;
            metadata.field = column.into();
            metadata.kind = FieldKind::Real;
            metadata.declared = "real".into();
            metadata.source = FieldSource::Column {
                slk: SlkId::UnitBalance,
                column: column.into(),
            };
            (*id, field)
        })
        .collect()
}

#[test]
fn vision_reads_authored_day_night_and_live_position_visibility_and_owner() {
    let mut vm = open_with_fields(&fields("37.5", "11.25"));
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(a).unwrap().owner;
    let handles = h.inner().handles();
    let day = h.vision_sources(owner, false, 16).unwrap();
    assert_eq!(day.len(), 2);
    assert_eq!(day[0].center, [16., 16.]);
    assert_eq!(day[0].radius, 37.5);
    assert_eq!(h.vision_sources(owner, true, 16).unwrap()[0].radius, 11.25);
    assert!(h.vision_sources(owner, false, 1).is_err());
    assert_eq!(h.inner().handles(), handles);
    h.native(
        "SetUnitPosition",
        &[Value::Handle(a), Value::Real(24.), Value::Real(24.)],
    )
    .unwrap();
    assert_eq!(
        h.vision_sources(owner, false, 16).unwrap()[0].center,
        [24., 24.]
    );
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert_eq!(h.vision_sources(owner, false, 16).unwrap().len(), 1);
    h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    assert!(h.vision_sources(owner, false, 16).unwrap().is_empty());
}

#[test]
fn malformed_sight_has_no_guessed_radius_or_world_mutation() {
    for value in ["-1", "NaN", "bad"] {
        let mut vm = open_with_fields(&fields(value, "3"));
        let a = unit(&vm, "A");
        let h = &mut vm.host().inner;
        let before = h.executed_unit(a).unwrap().clone();
        assert!(h.vision_sources(before.owner, false, 16).is_err());
        assert_eq!(
            h.vision_sources(before.owner, true, 16).unwrap()[0].radius,
            3.
        );
        assert_eq!(h.executed_unit(a).unwrap(), &before);
    }
}

#[test]
fn modifiers_use_source_states_and_stable_before_units_after_order() {
    use crate::playercolor_host::VisionState;
    let declarations = r#"
type location extends agent
type fogstate extends handle
type fogmodifier extends agent
type alliancetype extends handle
native Location takes real x, real y returns location
native MoveLocation takes location l, real x, real y returns nothing
native RemoveLocation takes location l returns nothing
native GetLocationX takes location l returns real
native GetLocationY takes location l returns real
constant native ConvertFogState takes integer n returns fogstate
constant native ConvertAllianceType takes integer n returns alliancetype
native SetPlayerAlliance takes player a, player b, alliancetype setting, boolean enabled returns nothing
native CreateFogModifierRadiusLoc takes player p, fogstate s, location l, real r, boolean shared, boolean after returns fogmodifier
native FogModifierStart takes fogmodifier f returns nothing
native FogModifierStop takes fogmodifier f returns nothing
native DestroyFogModifier takes fogmodifier f returns nothing
globals
constant fogstate FOG_OF_WAR_MASKED=ConvertFogState(31)
constant fogstate FOG_OF_WAR_FOGGED=ConvertFogState(47)
constant fogstate FOG_OF_WAR_VISIBLE=ConvertFogState(59)
constant alliancetype ALLIANCE_SHARED_VISION=ConvertAllianceType(73)
endglobals
"#;
    let mut h = position_host_admitting(
        1000,
        16,
        "// no map globals",
        declarations,
        &fields("13", "7"),
        |h| h,
    )
    .with_locations()
    .unwrap()
    .with_fog_modifiers()
    .unwrap()
    .with_player_alliances(crate::playercolor_host::PlayerAllianceBudget { max_records: 4 })
    .unwrap();
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let other = h.native("Player", &[Value::Int(5)]).unwrap();
    let Value::Handle(viewer) = owner.clone() else {
        panic!("player");
    };
    h.native(
        "CreateUnit",
        &[
            owner.clone(),
            Value::Int(i32::from_be_bytes(*b"U001")),
            Value::Real(16.),
            Value::Real(16.),
            Value::Real(0.),
        ],
    )
    .unwrap();
    let location = h
        .native("Location", &[Value::Real(24.), Value::Real(24.)])
        .unwrap();
    let mut make = |who: Value, state: i32, after: bool| {
        let state = h.native("ConvertFogState", &[Value::Int(state)]).unwrap();
        let modifier = h
            .native(
                "CreateFogModifierRadiusLoc",
                &[
                    who,
                    state,
                    location.clone(),
                    Value::Real(19.),
                    Value::Bool(false),
                    Value::Bool(after),
                ],
            )
            .unwrap();
        h.native("FogModifierStart", std::slice::from_ref(&modifier))
            .unwrap();
        modifier
    };
    let after = make(owner.clone(), 31, true);
    let before = make(owner, 47, false);
    make(other.clone(), 59, false);
    let sources = h.vision_sources(viewer, false, 16).unwrap();
    assert_eq!(
        sources.iter().map(|s| s.state).collect::<Vec<_>>(),
        [
            VisionState::Fogged,
            VisionState::Visible,
            VisionState::Masked
        ]
    );
    h.native("FogModifierStop", &[before]).unwrap();
    h.native("DestroyFogModifier", &[after]).unwrap();
    assert_eq!(h.vision_sources(viewer, false, 16).unwrap().len(), 1);
    h.native(
        "CreateUnit",
        &[
            other.clone(),
            Value::Int(i32::from_be_bytes(*b"U001")),
            Value::Real(80.),
            Value::Real(16.),
            Value::Real(0.),
        ],
    )
    .unwrap();
    assert_eq!(h.vision_sources(viewer, false, 16).unwrap().len(), 1);
    let setting = h.native("ConvertAllianceType", &[Value::Int(73)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            other.clone(),
            Value::Handle(viewer),
            setting.clone(),
            Value::Bool(true),
        ],
    )
    .unwrap();
    assert_eq!(h.vision_sources(viewer, false, 16).unwrap().len(), 2);
    h.native(
        "SetPlayerAlliance",
        &[other, Value::Handle(viewer), setting, Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(h.vision_sources(viewer, false, 16).unwrap().len(), 1);
}
