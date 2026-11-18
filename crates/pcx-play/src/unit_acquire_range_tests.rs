//! Default and bounded current acquisition range.
use super::*;
use FieldKind::Real;
use SlkId::UnitWeapons as W;

fn range(vm: &mut Vm, unit: HandleId) -> Result<Value, String> {
    vm.host()
        .inner
        .native("GetUnitDefaultAcquireRange", &[Value::Handle(unit)])
        .map_err(|e| e.to_string())
}

fn current(host: &mut ObjectHost, unit: Value) -> Result<Value, String> {
    host.native("GetUnitAcquireRange", &[unit])
}

#[test]
fn the_default_acquire_range_is_the_unit_rows_acquire_cell() {
    let mut fields = armed("100", "normal");
    fields.retain(|(id, _)| id != b"W005");
    fields.push(column(b"W005", W, "acquire", Real, "733.5"));
    let (mut vm, a, _) = open(&fields);
    assert_eq!(range(&mut vm, a), Ok(Value::Real(733.5)));
    assert_eq!(
        current(&mut vm.host().inner, Value::Handle(a)),
        Ok(Value::Real(733.5))
    );
}

#[test]
fn an_empty_acquire_cell_refuses_instead_of_inventing_a_default() {
    // A weaponless unit may leave the cell empty; creation refuses an armed unit without it.
    let mut fields = armed("100", "normal");
    fields.retain(|(id, _)| id != b"W005" && id != b"W001");
    fields.push(column(b"W001", W, "weapsOn", FieldKind::Int, "0"));
    fields.push(column(b"W005", W, "acquire", Real, "-"));
    let (mut vm, a, _) = open(&fields);
    let error = range(&mut vm, a).unwrap_err();
    assert!(error.contains("no acquire range cell"), "{error}");
    let error = current(&mut vm.host().inner, Value::Handle(a)).unwrap_err();
    assert!(error.contains("no acquire range cell"), "{error}");
}

#[test]
fn current_acquire_refuses_raw_requests_and_unqualified_units_without_mutation() {
    let (mut vm, a, b) = open(&armed("100", "normal"));
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(a).unwrap().owner;
    for request in [0.0, -12.5, 713.0] {
        h.native(
            "SetUnitAcquireRange",
            &[Value::Handle(a), Value::Real(request)],
        )
        .unwrap();
        let before = (h.inner().handles(), h.unit_revision(), h.timer_game_time());
        let error = current(h, Value::Handle(a)).unwrap_err();
        assert!(error.contains("raw acquire override"), "{error}");
        assert!(
            h.native("GetUnitDefaultAcquireRange", &[Value::Handle(a)])
                .is_ok()
        );
        assert_eq!(h.executed_unit(a).unwrap().acquire_range, Some(request));
        assert_eq!(
            (h.inner().handles(), h.unit_revision(), h.timer_game_time()),
            before
        );
    }
    h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    let before = (h.inner().handles(), h.unit_revision());
    for value in [Value::Null, Value::Handle(owner), Value::Handle(b)] {
        assert!(current(h, value).is_err());
    }
    assert_eq!((h.inner().handles(), h.unit_revision()), before);
}

#[test]
fn acquire_getters_require_selected_source_and_signature() {
    for name in ["GetUnitAcquireRange", "GetUnitDefaultAcquireRange"] {
        for (constant, in_map, expected) in [
            ("constant ", false, "incompatible native"),
            ("", true, "multiple declaration sources"),
        ] {
            let native = format!("{constant}native {name} takes unit u returns real\n");
            let (map, common) = if in_map {
                (native.as_str(), "")
            } else {
                ("// no map globals", native.as_str())
            };
            let mut host = position_host_admitting(8, 4, map, common, &[], |h| h);
            let error = host.native(name, &[Value::Null]).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }
}

#[test]
fn current_acquire_requires_armed_finite_source_default() {
    let mut fields = armed("100", "normal");
    fields.retain(|(id, _)| id != b"W005" && id != b"W001");
    fields.push(column(b"W001", W, "weapsOn", FieldKind::Int, "0"));
    for (cell, expected) in [("600", "unqualified"), ("NaN", "not a finite number")] {
        fields.push(column(b"W005", W, "acquire", Real, cell));
        let (mut vm, a, _) = open(&fields);
        let error = current(&mut vm.host().inner, Value::Handle(a)).unwrap_err();
        assert!(error.contains(expected), "{error}");
        fields.pop();
    }
}
