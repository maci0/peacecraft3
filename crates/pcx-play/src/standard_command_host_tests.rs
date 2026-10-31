use super::*;
use crate::standard_command::Kind;

#[test]
fn following_ignores_enemy_acquisition_until_a_stop_releases_the_order() {
    let mut fields = armed("64", "normal");
    fields.retain(|(id, _)| id != b"W005");
    fields.push(column(
        b"W005",
        SlkId::UnitWeapons,
        "acquire",
        FieldKind::Real,
        "200",
    ));
    fields.push(column(
        b"F001",
        SlkId::UnitBalance,
        "isbldg",
        FieldKind::Bool,
        "0",
    ));
    let (mut vm, a, b) = open(&fields);
    let life = |vm: &mut Vm, unit| vm.host().inner.unit_vitals(unit).unwrap().unwrap().life;
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(b), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(a),
                    Value::Str("gamma".into()),
                    Value::Handle(b)
                ]
            )
            .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 10.0);
    assert_eq!(life(&mut vm, b), 20.0);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .followed_unit(),
        Some(b)
    );
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        None
    );
    let stop = vm
        .host()
        .inner
        .standard_command_order(a, Kind::Stop)
        .unwrap()
        .unwrap();
    vm.host()
        .inner
        .native(
            "IssueImmediateOrderById",
            &[Value::Handle(a), Value::Int(stop)],
        )
        .unwrap();
    step(&mut vm, 10.5);
    assert!(life(&mut vm, b) < 20.0);
}

#[test]
fn source_bound_standard_identifiers_reach_motion_hold_stop_patrol_and_attack() {
    let (mut vm, [unit, _, enemy, _]) = holy_open("", LEARNED, NOTHING);
    let host = &mut vm.host().inner;
    let order = |host: &ObjectHost, kind| host.standard_command_order(unit, kind).unwrap().unwrap();
    let movement = order(host, Kind::Move);
    assert_eq!(
        host.native(
            "IssuePointOrderById",
            &[
                Value::Handle(unit),
                Value::Int(movement),
                Value::Real(32.),
                Value::Real(32.)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    let hold = order(host, Kind::Hold);
    assert_eq!(
        host.native(
            "IssueImmediateOrderById",
            &[Value::Handle(unit), Value::Int(hold)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .held()
    );
    let stop = order(host, Kind::Stop);
    assert_eq!(
        host.native(
            "IssueImmediateOrderById",
            &[Value::Handle(unit), Value::Int(stop)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert!(
        !host
            .executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .held()
    );
    let patrol = order(host, Kind::Patrol);
    assert_eq!(
        host.native(
            "IssuePointOrderById",
            &[
                Value::Handle(unit),
                Value::Int(patrol),
                Value::Real(32.),
                Value::Real(32.)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        host.native("GetUnitCurrentOrder", &[Value::Handle(unit)])
            .unwrap(),
        Value::Int(patrol)
    );
    let attack = order(host, Kind::Attack);
    assert_eq!(
        host.native(
            "IssueTargetOrderById",
            &[
                Value::Handle(unit),
                Value::Int(attack),
                Value::Handle(enemy)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(enemy)
    );
    host.native("KillUnit", &[Value::Handle(unit)]).unwrap();
    for kind in Kind::ALL {
        assert_eq!(host.standard_command_order(unit, kind).unwrap(), None);
    }
}

fn registry_with_smart() -> (
    std::sync::Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    use crate::neutral_player::sha256;
    let (mut image, mut binding, budget) = crate::order_registry::tests::fixture([431, 744]);
    let third = 0x220 + 9 + 15 + 17;
    image[third + 1..third + 5].copy_from_slice(&0x301480u32.to_le_bytes());
    image[third + 6..third + 10].copy_from_slice(&2122i32.to_le_bytes());
    image[0x680..0x686].copy_from_slice(b"delta\0");
    let start = 0x220usize;
    let old_len = binding.initializer.len as usize;
    let insert_at = start + old_len - 2;
    let name_va = 0x3014c0u32;
    let smart_id = 9001i32;
    let mut reg = vec![0x68];
    reg.extend_from_slice(&name_va.to_le_bytes());
    reg.push(0x68);
    reg.extend_from_slice(&smart_id.to_le_bytes());
    reg.extend_from_slice(&[0x8b, 0xce, 0xe8]);
    let va_next = 0x301020 + (insert_at + reg.len() + 4 - start) as u32;
    reg.extend_from_slice(&(0x301100i32 - va_next as i32).to_le_bytes());
    image.copy_within(insert_at..insert_at + 2, insert_at + reg.len());
    image[insert_at..insert_at + reg.len()].copy_from_slice(&reg);
    image[0x6c0..0x6c6].copy_from_slice(b"smart\0");
    binding.initializer.len = (old_len + reg.len()) as u32;
    let mut put = |at: usize, prefix: &[u8], id: i32| {
        let mut bytes = prefix.to_vec();
        bytes.extend_from_slice(&id.to_le_bytes());
        image[at..at + bytes.len()].copy_from_slice(&bytes);
        crate::camera_margins::SourceWindow {
            va: 0x301000 + at as u32 - 0x200,
            len: bytes.len() as u32,
            digest: sha256(&bytes),
        }
    };
    let hold = put(0x320, &[0x3d], 431);
    let stop = put(0x330, &[0x81, 0xfb], 744);
    let movement = put(0x340, &[0x3d], 1631);
    let patrol = put(0x350, &[0x3d], 2122);
    binding.initializer.digest = sha256(&image[start..start + binding.initializer.len as usize]);
    binding.image_sha256 = sha256(&image);
    let registry = std::sync::Arc::new(
        crate::order_registry::Registry::read(&image, binding.clone(), budget, 5, 32).unwrap(),
    );
    let rules = crate::unit_activity::Rules::read(
        &image,
        crate::unit_activity::Binding {
            origin: binding.origin,
            image_sha256: binding.image_sha256,
            hold,
            stop,
            movement,
            patrol,
        },
        &registry,
        budget,
    )
    .unwrap();
    (registry, rules)
}

fn smart_vm() -> (Vm, pcx_script::HandleId) {
    const MAP: &str = r#"
globals
unit A=null
endglobals
function setup takes nothing returns nothing
set A=CreateUnit(Player(4),'U001',16.0,16.0,0.0)
endfunction
"#;
    let (registry, rules) = registry_with_smart();
    let extra = format!("{}\n{}", enumeration::TRIGGERS, activity::ORDERS);
    let mut host = position_host_admitting(1000, 16, MAP, &extra, &[], |host| host)
        .with_locations()
        .unwrap()
        .with_order_registry(registry)
        .unwrap();
    host.admit_unit_activity(rules, 65536).unwrap();
    host.admit_order_queue(crate::playercolor_host::QueueBudget {
        units: 2,
        orders_per_unit: 3,
    })
    .unwrap();
    let context = std::sync::Arc::clone(host.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let pcx_script::Value::Handle(unit) = vm.global("A") else {
        panic!("unit");
    };
    (vm, unit)
}

fn project(vm: &mut Vm, now: f64) {
    let host = &mut vm.host().inner;
    assert!(host.next_timer_expiry(now, &mut 1000).unwrap().is_none());
    host.advance_unit_activity(now, &mut 65536).unwrap();
}

#[test]
fn a_point_smart_order_resolves_to_move_and_queues_behind_that_motion() {
    use crate::playercolor_host::{OrderForm, QueueOutcome};
    let (mut vm, unit) = smart_vm();
    let host = &mut vm.host().inner;
    let smart = host
        .native("OrderId", &[Value::Str("SMART".into())])
        .unwrap();
    let movement = host
        .native("OrderId", &[Value::Str("gamma".into())])
        .unwrap();
    assert_eq!(smart, Value::Int(9001));
    assert_ne!(smart, movement);
    assert!(
        host.native(
            "IssuePointOrder",
            &[
                Value::Handle(unit),
                Value::Str("missing".into()),
                Value::Real(72.),
                Value::Real(16.)
            ]
        )
        .is_err()
    );
    assert!(
        host.native(
            "IssuePointOrder",
            &[
                Value::Handle(unit),
                Value::Str("smart".into()),
                Value::Real(f32::NAN),
                Value::Real(16.)
            ]
        )
        .unwrap_err()
        .contains("finite")
    );
    let far = OrderForm::Point {
        order: 9001,
        at: [72., 16.],
    };
    let home = OrderForm::Point {
        order: 9001,
        at: [16., 16.],
    };
    assert_eq!(host.queue_order(unit, far), Ok(QueueOutcome::Issued(true)));
    assert!(host.queued_orders(unit).is_empty());
    assert!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    assert_eq!(
        host.native("GetUnitCurrentOrder", &[Value::Handle(unit)])
            .unwrap(),
        movement
    );
    assert_eq!(
        host.native(
            "IssuePointOrderById",
            &[
                Value::Handle(unit),
                Value::Int(9001),
                Value::Real(72.),
                Value::Real(16.)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(host.queue_order(unit, home), Ok(QueueOutcome::Queued(1)));
    assert_eq!(host.queued_orders(unit), vec![home]);
    project(&mut vm, 9.);
    let host = &mut vm.host().inner;
    assert!(host.queued_orders(unit).is_empty());
    assert_eq!(
        host.executed_unit(unit).unwrap().position.unwrap(),
        [72., 16.]
    );
    assert_eq!(
        host.native("GetUnitCurrentOrder", &[Value::Handle(unit)])
            .unwrap(),
        movement
    );
    project(&mut vm, 20.);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(unit)
            .unwrap()
            .position
            .unwrap(),
        [16., 16.]
    );
}
