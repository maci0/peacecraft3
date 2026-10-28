//! `GetUnitDefaultAcquireRange` on a nonstock acquire column.
use super::*;
use FieldKind::Real;
use SlkId::UnitWeapons as W;

fn range(vm: &mut Vm, unit: HandleId) -> Result<Value, String> {
    vm.host()
        .inner
        .native("GetUnitDefaultAcquireRange", &[Value::Handle(unit)])
        .map_err(|e| e.to_string())
}

#[test]
fn the_default_acquire_range_is_the_unit_rows_acquire_cell() {
    let mut fields = armed("100", "normal");
    fields.retain(|(id, _)| id != b"W005");
    fields.push(column(b"W005", W, "acquire", Real, "733.5"));
    let (mut vm, a, _) = open(&fields);
    assert_eq!(range(&mut vm, a), Ok(Value::Real(733.5)));
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
}
