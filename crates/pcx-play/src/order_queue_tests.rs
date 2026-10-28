use super::*;
use crate::playercolor_host::{OrderForm, QueueBudget, QueueOutcome, QueueRefusal};

const MOVE: i32 = 1631;
const FAR: [f32; 2] = [72., 16.];
const HOME: [f32; 2] = [16., 16.];

fn queued_vm(orders_per_unit: usize) -> (Vm, HandleId) {
    let mut vm = open();
    let a = unit(&vm, "A");
    vm.host()
        .inner
        .admit_order_queue(QueueBudget {
            units: 2,
            orders_per_unit,
        })
        .unwrap();
    (vm, a)
}

fn go(at: [f32; 2]) -> OrderForm {
    OrderForm::Point { order: MOVE, at }
}

#[test]
fn a_queued_order_starts_when_the_current_one_finishes() {
    let (mut vm, a) = queued_vm(3);
    let host = &mut vm.host().inner;
    assert_eq!(host.queue_order(a, go(FAR)), Ok(QueueOutcome::Issued(true)));
    assert_eq!(host.queue_order(a, go(HOME)), Ok(QueueOutcome::Queued(1)));
    assert_eq!(host.queued_orders(a), vec![go(HOME)]);
    tick(&mut vm, 4.);
    assert_eq!(position(&mut vm, a), [48., 16.]);
    assert_eq!(vm.host().inner.queued_orders(a), vec![go(HOME)]);
    tick(&mut vm, 9.);
    assert_eq!(position(&mut vm, a), FAR);
    assert!(vm.host().inner.queued_orders(a).is_empty());
    tick(&mut vm, 12.);
    assert_eq!(position(&mut vm, a), [48., 16.]);
    tick(&mut vm, 20.);
    assert_eq!(position(&mut vm, a), HOME);
    assert!(vm.host().inner.take_queue_refusals().is_empty());
}

#[test]
fn an_order_issued_without_the_queue_replaces_it() {
    let (mut vm, a) = queued_vm(3);
    let host = &mut vm.host().inner;
    host.queue_order(a, go(FAR)).unwrap();
    host.queue_order(a, go(HOME)).unwrap();
    assert_eq!(vm.call("hold", &[]).unwrap(), Value::Bool(true));
    assert!(vm.host().inner.queued_orders(a).is_empty());
    tick(&mut vm, 20.);
    assert_eq!(position(&mut vm, a), HOME);
}

#[test]
fn a_unit_holding_position_keeps_its_queue_waiting() {
    let (mut vm, a) = queued_vm(3);
    assert_eq!(vm.call("hold", &[]).unwrap(), Value::Bool(true));
    let host = &mut vm.host().inner;
    assert_eq!(host.queue_order(a, go(FAR)), Ok(QueueOutcome::Queued(1)));
    tick(&mut vm, 9.);
    assert_eq!(position(&mut vm, a), HOME);
    assert_eq!(vm.host().inner.queued_orders(a), vec![go(FAR)]);
}

#[test]
fn bounds_refuse_loudly_and_leave_the_queue_as_it_was() {
    let (mut vm, a) = queued_vm(1);
    let b = unit(&vm, "B");
    let host = &mut vm.host().inner;
    assert_eq!(host.queue_order(a, go(FAR)), Ok(QueueOutcome::Issued(true)));
    assert_eq!(host.queue_order(a, go(HOME)), Ok(QueueOutcome::Queued(1)));
    assert_eq!(
        host.queue_order(a, go(FAR)),
        Err(QueueRefusal::UnitFull { limit: 1 })
    );
    assert_eq!(host.queued_orders(a), vec![go(HOME)]);
    assert_eq!(
        host.queue_order(b, go(HOME)),
        Ok(QueueOutcome::Issued(true))
    );
}

#[test]
fn a_queued_order_the_unit_refuses_is_dropped_with_a_reason() {
    let (mut vm, a) = queued_vm(3);
    let host = &mut vm.host().inner;
    host.queue_order(a, go(FAR)).unwrap();
    host.queue_order(a, go([f32::NAN, 16.])).unwrap();
    tick(&mut vm, 9.);
    let host = &mut vm.host().inner;
    assert!(host.queued_orders(a).is_empty());
    assert_eq!(host.take_queue_refusals().len(), 1);
    assert!(host.take_queue_refusals().is_empty());
    assert_eq!(position(&mut vm, a), FAR);
}

#[test]
fn queueing_is_refused_until_admitted() {
    let mut vm = open();
    let a = unit(&vm, "A");
    assert_eq!(
        vm.host().inner.queue_order(a, go(FAR)),
        Err(QueueRefusal::NotAdmitted)
    );
}

/// Two units whose `isbldg` is authored, so target classes can be read.
fn classed_vm() -> (Vm, HandleId, HandleId) {
    let mut field = geometry_field(Ok(FieldValue::Cell("0".into())));
    let meta = field.metadata.as_mut().unwrap();
    meta.fourcc = *b"F001";
    meta.field = "isbldg".into();
    meta.kind = FieldKind::Bool;
    meta.declared = "bool".into();
    meta.source = FieldSource::Column {
        slk: SlkId::UnitBalance,
        column: "isbldg".into(),
    };
    let mut vm = open_with_fields(&[(*b"F001", field)]);
    let (a, b) = (unit(&vm, "A"), unit(&vm, "B"));
    for id in [a, b] {
        vm.host()
            .inner
            .native("SetUnitPathing", &[Value::Handle(id), Value::Bool(false)])
            .unwrap();
    }
    (vm, a, b)
}

#[test]
fn smart_order_resolves_ground_and_units_through_the_hosts_abilities() {
    use crate::playercolor_host::{SmartRefusal, SmartTarget};
    let (mut vm, a, b) = classed_vm();
    let host = &mut vm.host().inner;
    assert_eq!(
        host.resolve_smart_order(a, SmartTarget::Ground(FAR)),
        Ok(go(FAR))
    );
    let follow = OrderForm::Target {
        order: MOVE,
        target: b,
    };
    assert_eq!(
        host.resolve_smart_order(a, SmartTarget::Unit(b)),
        Ok(follow)
    );
    assert_eq!(
        host.resolve_smart_order(a, SmartTarget::Unit(a)),
        Err(SmartRefusal::SelfTarget)
    );
    assert_eq!(host.issue_order_form(a, follow), Ok(true));
    assert_eq!(
        host.native("GetUnitCurrentOrder", &[Value::Handle(a)]),
        Ok(Value::Int(MOVE))
    );
}
