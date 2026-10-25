use super::*;

const REGISTER: &str = "TriggerRegisterGameEvent";
fn common() -> String {
    format!("{COMMON}native TriggerRegisterGameEvent takes trigger t, gameevent e returns event\n")
        .replace(
            "endglobals",
            "constant gameevent TEST_OTHER = ConvertGameEvent(73)\nendglobals",
        )
}
fn setup(capacity: usize) -> ObjectHost {
    host(&common(), capacity)
        .with_trigger_runtime(input(4))
        .unwrap()
        .with_game_events()
        .unwrap()
}

#[test]
fn game_events_deliver_matching_subscriptions_with_context_and_trigger_lifecycle() {
    let mut h = setup(32);
    let t = h.native("CreateTrigger", &[]).unwrap();
    let e = h.native("ConvertGameEvent", &[Value::Int(73)]).unwrap();
    let first = h.native(REGISTER, &[t.clone(), e.clone()]).unwrap();
    let second = h.native(REGISTER, &[t.clone(), e.clone()]).unwrap();
    assert_ne!(first, second);
    assert!(h.take_fired_event().is_none());
    assert_eq!(h.fire_game_event("TEST_TIMER").unwrap(), 0);
    assert_eq!(h.fire_game_event("TEST_OTHER").unwrap(), 2);
    for _ in 0..2 {
        let fired = h.take_fired_event().unwrap();
        assert!(h.begin_fired_event(&fired).unwrap().is_some());
        assert_eq!(h.native("GetTriggeringTrigger", &[]).unwrap(), t);
        assert_eq!(h.native("GetTriggerEventId", &[]).unwrap(), e);
        h.finish_fired_event();
    }
    h.native("DisableTrigger", std::slice::from_ref(&t))
        .unwrap();
    h.fire_game_event("TEST_OTHER").unwrap();
    while let Some(fired) = h.take_fired_event() {
        assert!(h.begin_fired_event(&fired).unwrap().is_none());
    }
    h.native("DestroyTrigger", &[t]).unwrap();
    assert_eq!(h.fire_game_event("TEST_OTHER").unwrap(), 0);
}

#[test]
fn bad_game_event_signatures_families_and_capacity_refuse_without_partial_events() {
    assert!(
        host(
            &common().replace("gameevent e returns event", "integer e returns event"),
            32
        )
        .with_game_events()
        .is_err()
    );
    let mut h = setup(8);
    let t = h.native("CreateTrigger", &[]).unwrap();
    let e = h.native("ConvertGameEvent", &[Value::Int(73)]).unwrap();
    let count = h.inner().handles();
    for args in [
        vec![t.clone(), t.clone()],
        vec![e.clone(), e.clone()],
        vec![t.clone()],
        vec![Value::Null, e.clone()],
    ] {
        assert!(h.native(REGISTER, &args).is_err());
        assert_eq!(h.inner().handles(), count);
    }
    h.native(REGISTER, &[t.clone(), e.clone()]).unwrap();
    for _ in 0..8 {
        h.fire_game_event("TEST_OTHER").unwrap();
    }
    assert!(h.fire_game_event("TEST_OTHER").is_err());
    assert_eq!(std::iter::from_fn(|| h.take_fired_event()).count(), 8);
    while h.inner().handles() < 8 {
        h.native("CreateTimer", &[]).unwrap();
    }
    assert!(h.native(REGISTER, &[t, e]).is_err());
}

#[test]
fn public_game_event_registration_runs_the_authored_action_in_the_existing_vm() {
    let common = format!(
        "{}type triggeraction extends handle\n{SLEEP_COMMON}",
        common()
    );
    let map = "globals
trigger T=null
integer hits=0
endglobals
function action takes nothing returns nothing
if GetTriggeringTrigger()==T and GetTriggerEventId()==TEST_OTHER then
set hits=hits+1
endif
endfunction
function main takes nothing returns nothing
set T=CreateTrigger()
call TriggerAddAction(T,function action)
call TriggerRegisterGameEvent(T,TEST_OTHER)
endfunction
";
    let h = host_map(&common, map, 32, 4)
        .with_trigger_runtime(input(4))
        .unwrap()
        .with_game_events()
        .unwrap();
    let shared = Arc::clone(&h.inner().program);
    let mut vm = pcx_script::OwnedVm::new_owned(shared, h);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    assert_eq!(vm.host().fire_game_event("TEST_OTHER").unwrap(), 1);
    let event = vm.host().take_fired_event().unwrap();
    let delivery = vm.host().begin_fired_event(&event).unwrap().unwrap();
    vm.run_thread(&delivery.actions).unwrap();
    vm.host().finish_fired_event();
    assert_eq!(vm.global("hits"), Value::Int(1));
    assert_eq!(
        vm.host().native("GetTriggerEventId", &[]).unwrap(),
        Value::Null
    );
}
