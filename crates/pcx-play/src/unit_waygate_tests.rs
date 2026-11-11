use super::*;

pub(super) const WAYGATE_NATIVES: &str =
    "native WaygateGetDestinationX takes unit waygate returns real
native WaygateGetDestinationY takes unit waygate returns real
native WaygateSetDestination takes unit waygate, real x, real y returns nothing
native WaygateActivate takes unit waygate, boolean activate returns nothing
native WaygateIsActive takes unit waygate returns boolean
";

const MAP: &str = r#"
globals
unit W = null
unit P = null
endglobals
function setup takes nothing returns nothing
set W = CreateUnit(Player(4), 'nwgt', 0.0, 0.0, 0.0)
set P = CreateUnit(Player(4), 'hfoo', 0.0, 0.0, 0.0)
endfunction
"#;

/// `nwgt` names `Awrp` in its `abilList`, as `Units\UnitAbilities.slk` does.
/// `hfoo` names no waygate ability.
struct WaygateCatalog;

impl ObjectCatalog for WaygateCatalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let abilities = match &rawcode {
            b"nwgt" => "Awrp,Avul",
            b"hfoo" => "Avul",
            _ => {
                return Err(ResolutionProblem::MissingRawcode(pcx_objects::fourcc_text(
                    rawcode,
                )));
            }
        };
        let mut fields = retained_field();
        fields.insert(*b"uabi", ability_list_field(abilities));
        Ok(definition(
            rawcode,
            pcx_objects::fourcc_text(rawcode),
            ObjectKind::Unit,
            fields,
        ))
    }
}

pub(super) fn ability_list_field(text: &str) -> ObjectField {
    ObjectField {
        metadata: Some(pcx_objects::editor::FieldDef {
            fourcc: *b"uabi",
            from: SlkId::UnitMetaData,
            field: "abilList".into(),
            slk: "UnitAbilities".into(),
            index: -1,
            declared: "token".into(),
            kind: pcx_objects::editor::FieldKind::Token,
            category: String::new(),
            source: pcx_objects::editor::FieldSource::Column {
                slk: SlkId::UnitAbilities,
                column: "abilList".into(),
            },
            measured: true,
        }),
        value: Ok(FieldValue::Cell(text.into())),
    }
}

pub(super) fn open(extra: &str, map: &str) -> VmHost {
    let common = script_common_with("widget", true, extra);
    let ctx = context(300, &common, map, "unit-catalog");
    let (_, hits, seen) = counting();
    let catalog: Arc<dyn ObjectCatalog + Send + Sync> = Arc::new(WaygateCatalog);
    let host = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&catalog), source(), limits(2))
        .unwrap();
    VmHost {
        vm: Arc::clone(&ctx.scripts).owned_vm(host),
        hits,
        seen,
        catalog,
    }
}

fn setup() -> (VmHost, Value, Value) {
    let mut s = open(WAYGATE_NATIVES, MAP);
    s.vm.init_globals().unwrap();
    s.vm.call("setup", &[]).unwrap();
    let waygate = s.vm.global("W");
    let plain = s.vm.global("P");
    (s, waygate, plain)
}

#[test]
fn destination_and_flag_round_trip_on_an_awrp_unit() {
    let (mut s, w, _) = setup();
    let host = &mut s.vm.host().inner;
    host.native(
        "WaygateSetDestination",
        &[w.clone(), Value::Real(3.5), Value::Real(-2.)],
    )
    .unwrap();
    assert_eq!(
        host.native("WaygateGetDestinationX", std::slice::from_ref(&w))
            .unwrap(),
        Value::Real(3.5)
    );
    assert_eq!(
        host.native("WaygateGetDestinationY", std::slice::from_ref(&w))
            .unwrap(),
        Value::Real(-2.)
    );
    host.native("WaygateActivate", &[w.clone(), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        host.native("WaygateIsActive", std::slice::from_ref(&w))
            .unwrap(),
        Value::Bool(true)
    );
    host.native("WaygateActivate", &[w.clone(), Value::Bool(false)])
        .unwrap();
    assert_eq!(
        host.native("WaygateIsActive", std::slice::from_ref(&w))
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn unwritten_destination_and_flag_refuse_instead_of_defaulting() {
    let (mut s, w, _) = setup();
    let host = &mut s.vm.host().inner;
    let x = host
        .native("WaygateGetDestinationX", std::slice::from_ref(&w))
        .unwrap_err();
    assert!(x.contains("waygate destination was never written"), "{x}");
    let active = host
        .native("WaygateIsActive", std::slice::from_ref(&w))
        .unwrap_err();
    assert!(
        active.contains("waygate active flag was never written"),
        "{active}"
    );
    // A destination write does not stand in for the flag.
    host.native(
        "WaygateSetDestination",
        &[w.clone(), Value::Real(1.), Value::Real(1.)],
    )
    .unwrap();
    assert!(
        host.native("WaygateIsActive", std::slice::from_ref(&w))
            .is_err()
    );
}

#[test]
fn unit_without_the_waygate_ability_takes_the_missing_ability_path() {
    let (mut s, _, plain) = setup();
    let host = &mut s.vm.host().inner;
    assert_eq!(
        host.native(
            "WaygateSetDestination",
            &[plain.clone(), Value::Real(9.), Value::Real(9.)],
        )
        .unwrap(),
        Value::Null
    );
    host.native("WaygateActivate", &[plain.clone(), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        host.native("WaygateGetDestinationX", std::slice::from_ref(&plain))
            .unwrap(),
        Value::Real(0.)
    );
    assert_eq!(
        host.native("WaygateIsActive", std::slice::from_ref(&plain))
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn null_unit_takes_the_missing_handle_path() {
    let (mut s, _, _) = setup();
    let host = &mut s.vm.host().inner;
    assert_eq!(
        host.native(
            "WaygateSetDestination",
            &[Value::Null, Value::Real(1.), Value::Real(2.)],
        )
        .unwrap(),
        Value::Null
    );
    host.native("WaygateActivate", &[Value::Null, Value::Bool(true)])
        .unwrap();
    assert_eq!(
        host.native("WaygateGetDestinationY", &[Value::Null])
            .unwrap(),
        Value::Real(0.)
    );
    assert_eq!(
        host.native("WaygateIsActive", &[Value::Null]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn non_finite_destination_is_refused_and_leaves_the_state_unwritten() {
    let (mut s, w, _) = setup();
    let host = &mut s.vm.host().inner;
    let error = host
        .native(
            "WaygateSetDestination",
            &[w.clone(), Value::Real(f32::NAN), Value::Real(0.)],
        )
        .unwrap_err();
    assert!(error.contains("finite destination required"), "{error}");
    assert!(
        host.native("WaygateGetDestinationX", std::slice::from_ref(&w))
            .is_err()
    );
}
