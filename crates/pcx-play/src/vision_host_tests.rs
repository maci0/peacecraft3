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
    assert_eq!(
        day[0].shape,
        crate::playercolor_host::VisionShape::Circle {
            center: [16., 16.],
            radius: 37.5
        }
    );
    assert_eq!(
        h.vision_sources(owner, true, 16).unwrap()[0].shape,
        crate::playercolor_host::VisionShape::Circle {
            center: [16., 16.],
            radius: 11.25
        }
    );
    assert!(h.vision_sources(owner, false, 1).is_err());
    assert_eq!(h.inner().handles(), handles);
    h.native(
        "SetUnitPosition",
        &[Value::Handle(a), Value::Real(24.), Value::Real(24.)],
    )
    .unwrap();
    assert_eq!(
        h.vision_sources(owner, false, 16).unwrap()[0].shape,
        crate::playercolor_host::VisionShape::Circle {
            center: [24., 24.],
            radius: 37.5
        }
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
            h.vision_sources(before.owner, true, 16).unwrap()[0].shape,
            crate::playercolor_host::VisionShape::Circle {
                center: [16., 16.],
                radius: 3.
            }
        );
        assert_eq!(h.executed_unit(a).unwrap(), &before);
    }
}

#[test]
fn modifiers_use_source_states_and_stable_before_units_after_order() {
    use crate::playercolor_host::VisionState;

    let mut h = fog_host();
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
    assert!(matches!(
        sources[0].origin,
        crate::playercolor_host::VisionOrigin::Modifier { .. }
    ));
    assert!(matches!(
        sources[1].origin,
        crate::playercolor_host::VisionOrigin::Unit { .. }
    ));
    assert!(matches!(
        sources[2].origin,
        crate::playercolor_host::VisionOrigin::Modifier { .. }
    ));
    assert!(sources.iter().all(|source| source.source_height.is_none()));
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

const FOG_DECLARATIONS: &str = r#"
native UnitShareVision takes unit u, player p, boolean share returns nothing
native ShowUnit takes unit u, boolean show returns nothing
native RemoveUnit takes unit u returns nothing
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
native CreateFogModifierRect takes player p, fogstate s, rect r, boolean shared, boolean after returns fogmodifier
native CreateFogModifierRadius takes player p, fogstate s, real x, real y, real r, boolean shared, boolean after returns fogmodifier
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

fn fog_host() -> ObjectHost {
    position_host_admitting(
        1000,
        16,
        "// no map globals",
        FOG_DECLARATIONS,
        &fields("13", "7"),
        |h| h,
    )
    .with_locations()
    .unwrap()
    .with_fog_modifiers()
    .unwrap()
    .with_player_alliances(crate::playercolor_host::PlayerAllianceBudget { max_records: 4 })
    .unwrap()
}

#[test]
fn rectangular_and_point_radius_modifiers_share_live_vision_and_snapshot_their_inputs() {
    use crate::playercolor_host::VisionShape;
    let mut h = fog_host();
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let Value::Handle(viewer) = owner.clone() else {
        panic!("player")
    };
    let state = h.native("ConvertFogState", &[Value::Int(59)]).unwrap();
    let rect = h
        .native("Rect", &[-7., -5., 61., 29.].map(Value::Real))
        .unwrap();
    let modifier = h
        .native(
            "CreateFogModifierRect",
            &[
                owner.clone(),
                state.clone(),
                rect.clone(),
                Value::Bool(true),
                Value::Bool(true),
            ],
        )
        .unwrap();
    assert!(h.vision_sources(viewer, false, 16).unwrap().is_empty());
    h.native(
        "MoveRectTo",
        &[rect.clone(), Value::Real(1000.), Value::Real(2000.)],
    )
    .unwrap();
    h.native("RemoveRect", std::slice::from_ref(&rect)).unwrap();
    h.native("FogModifierStart", std::slice::from_ref(&modifier))
        .unwrap();
    let source = h.vision_sources(viewer, false, 16).unwrap()[0];
    assert_eq!(
        source.shape,
        VisionShape::Rectangle {
            bounds: [-7., -5., 61., 29.]
        }
    );
    assert!(source.after_units);
    assert!(source.shape.contains([60., 28.]));
    assert!(!source.shape.contains([62., 28.]));
    let radius = h
        .native(
            "CreateFogModifierRadius",
            &[
                owner.clone(),
                state.clone(),
                Value::Real(103.),
                Value::Real(179.),
                Value::Real(37.),
                Value::Bool(false),
                Value::Bool(false),
            ],
        )
        .unwrap();
    h.native("FogModifierStart", std::slice::from_ref(&radius))
        .unwrap();
    let sources = h.vision_sources(viewer, false, 16).unwrap();
    assert_eq!(
        sources[0].shape,
        VisionShape::Circle {
            center: [103., 179.],
            radius: 37.
        }
    );
    assert_eq!(sources[1], source);
    let count = h.inner().handles();
    for args in [
        vec![
            owner.clone(),
            state.clone(),
            rect,
            Value::Bool(false),
            Value::Bool(false),
        ],
        vec![
            state.clone(),
            state.clone(),
            modifier.clone(),
            Value::Bool(false),
            Value::Bool(false),
        ],
    ] {
        assert!(h.native("CreateFogModifierRect", &args).is_err());
    }
    for invalid in [-1., f32::NAN, f32::INFINITY] {
        assert!(
            h.native(
                "CreateFogModifierRadius",
                &[
                    owner.clone(),
                    state.clone(),
                    Value::Real(103.),
                    Value::Real(179.),
                    Value::Real(invalid),
                    Value::Bool(false),
                    Value::Bool(false)
                ]
            )
            .is_err()
        );
    }
    assert_eq!(h.inner().handles(), count);
    h.native("FogModifierStop", std::slice::from_ref(&modifier))
        .unwrap();
    assert_eq!(h.vision_sources(viewer, false, 16).unwrap().len(), 1);
    h.native("DestroyFogModifier", &[modifier]).unwrap();
    h.native("DestroyFogModifier", &[radius]).unwrap();
    assert!(h.vision_sources(viewer, false, 16).unwrap().is_empty());
}

#[test]
fn shared_vision_stays_unadmitted_without_day_night_classification() {
    let mut h = fog_host();
    assert!(matches!(
        h.vision_capability(),
        crate::playercolor_host::VisionCapability::Unadmitted
    ));
    assert!(!h.admit_shared_vision().unwrap());
    assert!(!h.shared_vision_admitted());
    assert!(matches!(
        h.vision_capability(),
        crate::playercolor_host::VisionCapability::Unavailable(_)
    ));
    assert_eq!(
        h.vision_readiness(),
        Err("shared vision needs admitted day/night sight")
    );
    assert!(h.sample_shared_vision(1).is_err());
    assert!(h.vision_occlusion_applied().is_none());
}

#[test]
fn unit_share_vision_grants_are_per_unit_and_do_not_change_owner_or_alliances() {
    let mut h = fog_host();
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let viewer = h.native("Player", &[Value::Int(5)]).unwrap();
    let Value::Handle(viewer_id) = viewer else {
        panic!("player")
    };
    let create = |h: &mut ObjectHost, x| {
        h.native(
            "CreateUnit",
            &[
                owner.clone(),
                Value::Int(i32::from_be_bytes(*b"U001")),
                Value::Real(x),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    };
    let a = create(&mut h, 16.);
    let b = create(&mut h, 80.);
    let Value::Handle(a_id) = a else {
        panic!("unit")
    };
    let share = |h: &mut ObjectHost, enabled| {
        h.native(
            "UnitShareVision",
            &[a.clone(), viewer.clone(), Value::Bool(enabled)],
        )
        .unwrap()
    };
    assert!(h.vision_sources(viewer_id, false, 16).unwrap().is_empty());
    share(&mut h, true);
    share(&mut h, true);
    assert_eq!(h.executed_unit(a_id).unwrap().vision_viewers, [viewer_id]);
    assert_eq!(Value::Handle(h.executed_unit(a_id).unwrap().owner), owner);
    assert_eq!(h.vision_sources(viewer_id, false, 16).unwrap().len(), 1);
    let before = h.executed_unit(a_id).unwrap().clone();
    let handles = h.inner().handles();
    for args in [
        vec![],
        vec![a.clone(), viewer.clone(), Value::Int(1)],
        vec![a.clone(), a.clone(), Value::Bool(false)],
        vec![viewer.clone(), viewer.clone(), Value::Bool(false)],
        vec![Value::Null, viewer.clone(), Value::Bool(true)],
    ] {
        assert!(h.native("UnitShareVision", &args).is_err());
        assert_eq!(h.executed_unit(a_id).unwrap(), &before);
        assert_eq!(h.inner().handles(), handles);
    }
    let alliance = h.native("ConvertAllianceType", &[Value::Int(73)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            owner.clone(),
            viewer.clone(),
            alliance.clone(),
            Value::Bool(true),
        ],
    )
    .unwrap();
    share(&mut h, false);
    assert_eq!(h.vision_sources(viewer_id, false, 16).unwrap().len(), 2);
    h.native(
        "SetPlayerAlliance",
        &[owner, viewer.clone(), alliance, Value::Bool(false)],
    )
    .unwrap();
    assert!(h.vision_sources(viewer_id, false, 16).unwrap().is_empty());
    share(&mut h, true);
    h.native("ShowUnit", &[a.clone(), Value::Bool(false)])
        .unwrap();
    assert!(h.vision_sources(viewer_id, false, 16).unwrap().is_empty());
    h.native("ShowUnit", &[a.clone(), Value::Bool(true)])
        .unwrap();
    assert_eq!(h.vision_sources(viewer_id, false, 16).unwrap().len(), 1);
    h.native("RemoveUnit", std::slice::from_ref(&a)).unwrap();
    assert!(h.vision_sources(viewer_id, false, 16).unwrap().is_empty());
    assert!(
        h.native("UnitShareVision", &[a, viewer, Value::Bool(true)])
            .is_err()
    );
    assert!(h.executed_units().any(|(_, u)| u.vision_viewers.is_empty()));
    h.native("RemoveUnit", &[b]).unwrap();
}

#[test]
fn unit_share_vision_stamps_and_revokes_the_canonical_exploration_ledger() {
    use crate::playercolor_host::VisionState;
    let extra = format!(
        r#"{FOG_DECLARATIONS}
        type fgamestate extends handle
        constant native ConvertFGameState takes integer i returns fgamestate
        constant native GetFloatGameState takes fgamestate s returns real
        native SetFloatGameState takes fgamestate s, real v returns nothing
        native SuspendTimeOfDay takes boolean b returns nothing
        native SetTimeOfDayScale takes real r returns nothing
        native GetTimeOfDayScale takes nothing returns real
        native TimerStart takes timer t, real timeout, boolean periodic, code handler returns nothing
        native TimerGetElapsed takes timer t returns real
        native TimerGetRemaining takes timer t returns real
        native TimerGetTimeout takes timer t returns real
        native GetExpiredTimer takes nothing returns timer
        globals
        constant fgamestate CLOCK = ConvertFGameState(2)
        constant real bj_TOD_DAWN = 6.0
        constant real bj_TOD_DUSK = 18.0
        endglobals
    "#
    );
    let mut h = position_host_admitting(
        1000,
        16,
        "// synthetic map",
        &extra,
        &fields("13", "7"),
        |h| h,
    );
    h = h
        .with_startup_clock(crate::startup_clock::tests::input(9), "CLOCK", 64, 16, 1000)
        .unwrap()
        .with_timer_runtime(0.)
        .unwrap();
    h.admit_time_of_day(&pcx_objects::editor::misc::DayCycle {
        day_length: 480.,
        day_hours: 24.,
        sources: vec!["synthetic".into()],
    })
    .unwrap();
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let viewer = h.native("Player", &[Value::Int(5)]).unwrap();
    let Value::Handle(viewer_id) = viewer else {
        panic!("viewer")
    };
    let unit = h
        .native(
            "CreateUnit",
            &[
                owner,
                Value::Int(i32::from_be_bytes(*b"U001")),
                Value::Real(16.),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap();
    assert!(h.admit_shared_vision().unwrap());
    let sample = |h: &mut ObjectHost| {
        let budget = h.shared_vision_work_limit().unwrap();
        h.sample_shared_vision(budget).unwrap();
        h.vision_cell(viewer_id, [16., 16.]).unwrap().unwrap()
    };
    assert!(!sample(&mut h).explored);
    h.native(
        "UnitShareVision",
        &[unit.clone(), viewer.clone(), Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(sample(&mut h).raw, VisionState::Visible);
    h.native("UnitShareVision", &[unit, viewer, Value::Bool(false)])
        .unwrap();
    let cell = sample(&mut h);
    assert!(cell.explored);
    assert_eq!(cell.raw, VisionState::Fogged);
}
