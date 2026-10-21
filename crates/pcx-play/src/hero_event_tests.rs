//! Hero event callbacks execute in the same VM/host as the changed unit.
use super::*;

fn deliver(vm: &mut Vm) -> usize {
    let mut count = 0;
    while let Some(event) = vm.host().inner.take_fired_event() {
        let Some(delivery) = vm.host().inner.begin_fired_event(&event).unwrap() else {
            continue;
        };
        let mut accepted = true;
        for id in event.filter.into_iter().chain(delivery.conditions) {
            let name = vm.program().functions()[id.index()].name.clone();
            if vm.call(&name, &[]).unwrap() != Value::Bool(true) {
                accepted = false;
                break;
            }
        }
        if accepted {
            vm.run_thread(&delivery.actions).unwrap();
            count += 1;
        }
        vm.host().inner.finish_fired_event();
    }
    count
}

#[test]
fn hero_level_events_match_the_unit_and_player_and_callbacks_see_updated_state() {
    let (mut vm, [hero, ordinary, _, _]) = holy_open("", "", "AHhb");
    let event = vm.global("EVENT_UNIT_HERO_LEVEL");
    vm.call("hero_listen_unit", &[Value::Handle(hero), event.clone()])
        .unwrap();
    vm.call("hero_listen_unit", &[Value::Handle(ordinary), event])
        .unwrap();
    let owner = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let event = vm.global("EVENT_PLAYER_HERO_LEVEL");
    vm.call(
        "hero_listen_player",
        &[owner.clone(), event.clone(), Value::Null],
    )
    .unwrap();
    vm.host()
        .inner
        .native(
            "SetHeroLevel",
            &[Value::Handle(hero), Value::Int(5), Value::Bool(false)],
        )
        .unwrap();
    assert_eq!(deliver(&mut vm), 2);
    assert_eq!(vm.global("HeroSeen"), Value::Int(2));
    assert_eq!(vm.global("HeroUnit"), Value::Handle(hero));
    assert_eq!(vm.global("HeroLevel"), Value::Int(5));
    assert_eq!(vm.global("HeroPoints"), Value::Int(3));
    assert_eq!(vm.global("HeroAbility"), Value::Int(0));
    for level in [5, 4] {
        vm.host()
            .inner
            .native(
                "SetHeroLevel",
                &[Value::Handle(hero), Value::Int(level), Value::Bool(true)],
            )
            .unwrap();
    }
    assert_eq!(deliver(&mut vm), 0);
    let h = &mut vm.host().inner;
    assert_eq!(h.native("GetLevelingUnit", &[]).unwrap(), Value::Null);
    assert_eq!(h.native("GetLearningUnit", &[]).unwrap(), Value::Null);
    assert_eq!(h.native("GetLearnedSkill", &[]).unwrap(), Value::Int(0));
}

#[test]
fn hero_skill_events_keep_each_learned_rank_and_finish_clears_the_response() {
    let (mut vm, [hero, _, _, _]) = holy_open("", "", "AHhb");
    let event = vm.global("EVENT_UNIT_HERO_SKILL");
    vm.call("hero_listen_unit", &[Value::Handle(hero), event.clone()])
        .unwrap();
    let alias = i32::from_be_bytes(*b"AHhb");
    holy_learn(&mut vm, alias);
    vm.host()
        .inner
        .native(
            "SetHeroLevel",
            &[Value::Handle(hero), Value::Int(5), Value::Bool(false)],
        )
        .unwrap();
    holy_learn(&mut vm, alias);
    for rank in [1, 2] {
        let fired = vm.host().inner.take_fired_event().unwrap();
        let delivery = vm.host().inner.begin_fired_event(&fired).unwrap().unwrap();
        assert_eq!(
            vm.host().inner.native("GetTriggerEventId", &[]).unwrap(),
            event
        );
        assert_eq!(
            vm.host().inner.native("GetLearnedSkill", &[]).unwrap(),
            Value::Int(alias)
        );
        assert_eq!(
            vm.host().inner.native("GetLearnedSkillLevel", &[]).unwrap(),
            Value::Int(rank)
        );
        assert_eq!(
            vm.host().inner.native("GetLevelingUnit", &[]).unwrap(),
            Value::Null
        );
        vm.run_thread(&delivery.actions).unwrap();
        assert_eq!(vm.global("HeroRank"), Value::Int(rank));
        assert_eq!(vm.global("HeroUnit"), Value::Handle(hero));
        vm.host().inner.finish_fired_event();
    }
    assert!(vm.host().inner.take_fired_event().is_none());
    assert_eq!(
        vm.host().inner.native("GetLearnedSkillLevel", &[]).unwrap(),
        Value::Int(0)
    );
}

#[test]
fn hero_player_events_use_current_owner_filters_and_live_trigger_state() {
    let (mut vm, [hero, _, _, _]) = holy_open("", "", "AHhb");
    let old = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let current = vm.host().inner.native("Player", &[Value::Int(5)]).unwrap();
    vm.host()
        .inner
        .native(
            "SetUnitOwner",
            &[Value::Handle(hero), current.clone(), Value::Bool(false)],
        )
        .unwrap();
    let event = vm.global("EVENT_PLAYER_HERO_SKILL");
    vm.call("hero_listen_player", &[old, event.clone(), Value::Null])
        .unwrap();
    let filter = vm.call("hero_filter_handle", &[]).unwrap();
    let trigger = vm
        .call(
            "hero_listen_player",
            &[current.clone(), event.clone(), filter],
        )
        .unwrap();
    holy_learn(&mut vm, i32::from_be_bytes(*b"AHhb"));
    let fired = vm.host().inner.take_fired_event().unwrap();
    assert_eq!(Value::Handle(fired.trigger), trigger);
    vm.host().inner.begin_fired_event(&fired).unwrap().unwrap();
    assert_eq!(
        vm.host().inner.native("GetTriggerPlayer", &[]).unwrap(),
        current
    );
    assert_eq!(
        vm.host().inner.native("GetTriggerEventId", &[]).unwrap(),
        event
    );
    assert_eq!(vm.call("hero_filter", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.global("HeroFiltered"), Value::Handle(hero));
    vm.host().inner.finish_fired_event();
    vm.call("hero_refuse_filter", &[]).unwrap();
    assert!(vm.host().inner.take_fired_event().is_none());
    vm.host()
        .inner
        .native(
            "SetHeroLevel",
            &[Value::Handle(hero), Value::Int(5), Value::Bool(false)],
        )
        .unwrap();
    holy_learn(&mut vm, i32::from_be_bytes(*b"AHhb"));
    assert_eq!(deliver(&mut vm), 0);
    assert_eq!(vm.global("HeroFiltered"), Value::Handle(hero));
    assert_eq!(vm.global("HeroSeen"), Value::Int(0));
    holy_learn(&mut vm, i32::from_be_bytes(*b"AHhb"));
    let pending = vm
        .host()
        .inner
        .take_fired_event()
        .expect("third learned rank event");
    assert_eq!(Value::Handle(pending.trigger), trigger);
    vm.host()
        .inner
        .native("DisableTrigger", &[trigger])
        .unwrap();
    assert!(
        vm.host()
            .inner
            .begin_fired_event(&pending)
            .unwrap()
            .is_none()
    );
    assert_eq!(deliver(&mut vm), 0);
    assert_eq!(
        vm.host().inner.native("GetLearningUnit", &[]).unwrap(),
        Value::Null
    );
}
