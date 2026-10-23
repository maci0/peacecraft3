use super::*;
use crate::campaign_gathering_policy::GatheringPolicy;

fn source_fields(structure: bool) -> Vec<([u8; 4], ObjectField)> {
    let mut field = geometry_field(Ok(FieldValue::Cell(
        if structure { "1" } else { "0" }.into(),
    )));
    let metadata = field.metadata.as_mut().unwrap();
    metadata.fourcc = *b"F001";
    metadata.field = "isbldg".into();
    metadata.kind = FieldKind::Bool;
    metadata.declared = "bool".into();
    metadata.source = FieldSource::Column {
        slk: SlkId::UnitBalance,
        column: "isbldg".into(),
    };
    let mut class = geometry_field(Ok(FieldValue::Cell("_".into())));
    let metadata = class.metadata.as_mut().unwrap();
    metadata.fourcc = *b"F002";
    metadata.field = "type".into();
    metadata.kind = FieldKind::Token;
    metadata.declared = "unitClass".into();
    metadata.source = FieldSource::Column {
        slk: SlkId::UnitBalance,
        column: "type".into(),
    };
    vec![(*b"F001", field), (*b"F002", class)]
}

fn opened(structure: bool) -> Vm {
    open_with_fields(&source_fields(structure))
}

fn admit(vm: &mut Vm) -> HandleId {
    let a = unit(vm, "A");
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(a).unwrap().owner;
    let context = Arc::clone(host.player_context().unwrap());
    assert_eq!(host.campaign_gathering_state(owner).unwrap(), None);
    host.admit_campaign_gathering(&context, GatheringPolicy::fixture())
        .unwrap();
    assert!(
        host.admit_campaign_gathering(&context, GatheringPolicy::fixture())
            .is_err()
    );
    assert!(host.activate_campaign_ai_player(4).unwrap());
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .enabled,
        None
    );
    owner
}

#[test]
fn stop_gathering_keeps_one_canonical_live_association_and_future_birth() {
    let mut vm = opened(true);
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    let owner = admit(&mut vm);
    let host = &mut vm.host().inner;
    let handles = host.inner().handles();
    host.stop_ai_gathering(4).unwrap();
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .association,
        Some((b, [80.0, 16.0]))
    );
    assert_eq!(host.inner().handles(), handles);
    host.native(
        "SetUnitPosition",
        &[Value::Handle(b), Value::Real(112.0), Value::Real(16.0)],
    )
    .unwrap();
    let observed = host.campaign_gathering_state(owner).unwrap().unwrap();
    assert_eq!(observed.enabled, Some(false));
    assert_eq!(observed.association, Some((b, [112.0, 16.0])));
    host.stop_ai_gathering(4).unwrap();
    assert_eq!(
        host.campaign_gathering_state(owner).unwrap().unwrap(),
        observed
    );
    host.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .association,
        Some((a, [16.0, 16.0]))
    );
    host.stop_ai_gathering(4).unwrap();
    host.native("RemoveUnit", &[Value::Handle(a)]).unwrap();
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .association,
        None
    );
    let Value::Handle(later) = host
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"U001")),
                Value::Real(48.0),
                Value::Real(16.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("later unit")
    };
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .association,
        Some((later, [48.0, 16.0]))
    );
}

#[test]
fn malformed_or_ambiguous_candidates_refuse_before_policy_write() {
    let mut vm = open();
    let owner = admit(&mut vm);
    let host = &mut vm.host().inner;
    let handles = host.inner().handles();
    let error = host.stop_ai_gathering(4).unwrap_err().to_string();
    assert!(error.contains("UnitBalance/type"), "{error}");
    assert_eq!(host.inner().handles(), handles);
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .enabled,
        None
    );

    for malformed in ["wrong kind", "duplicate"] {
        let mut fields = source_fields(false);
        if malformed == "wrong kind" {
            fields[1].1.metadata.as_mut().unwrap().kind = FieldKind::Bool;
        } else {
            let mut duplicate = fields[1].1.clone();
            duplicate.metadata.as_mut().unwrap().fourcc = *b"F003";
            fields.push((*b"F003", duplicate));
        }
        let mut vm = open_with_fields(&fields);
        let owner = admit(&mut vm);
        let host = &mut vm.host().inner;
        let handles = host.inner().handles();
        let error = host.stop_ai_gathering(4).unwrap_err().to_string();
        assert!(error.contains("UnitBalance/type"), "{malformed}: {error}");
        assert_eq!(host.inner().handles(), handles);
        assert_eq!(
            host.campaign_gathering_state(owner)
                .unwrap()
                .unwrap()
                .enabled,
            None
        );
    }

    let mut vm = opened(false);
    let a = unit(&vm, "A");
    let owner = admit(&mut vm);
    let host = &mut vm.host().inner;
    host.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert!(host.stop_ai_gathering(4).is_err());
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .enabled,
        None
    );
}

#[test]
fn owner_transfer_never_exposes_a_foreign_association() {
    let mut vm = opened(true);
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    let owner = admit(&mut vm);
    let host = &mut vm.host().inner;
    let Value::Handle(other) = host.native("Player", &[Value::Int(5)]).unwrap() else {
        panic!("other player")
    };
    host.stop_ai_gathering(4).unwrap();
    host.native(
        "SetUnitOwner",
        &[Value::Handle(b), Value::Handle(other), Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .association,
        Some((a, [16.0, 16.0]))
    );
    host.stop_ai_gathering(4).unwrap();
    host.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert!(host.campaign_gathering_state(owner).is_err());
}

#[test]
fn peon_class_wins_before_or_after_a_structure() {
    let peon = "UNIT_TYPE_PEON"
        .strip_prefix("UNIT_TYPE_")
        .unwrap()
        .to_lowercase();
    for first in [true, false] {
        let mut vm = opened(true);
        let a = unit(&vm, "A");
        let b = unit(&vm, "B");
        let owner = admit(&mut vm);
        let host = &mut vm.host().inner;
        let chosen = if first { a } else { b };
        host.fixture_gathering_class(chosen, &peon);
        host.stop_ai_gathering(4).unwrap();
        assert_eq!(
            host.campaign_gathering_state(owner)
                .unwrap()
                .unwrap()
                .association
                .unwrap()
                .0,
            chosen,
        );
    }
}
