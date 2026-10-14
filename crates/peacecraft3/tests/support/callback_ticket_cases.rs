use crate::consumer_cases::{MAP, StatefulHost, WTS, context, id, initialize};
use pcx_script::{Value, value::FunctionId};
use peacecraft3::map_script_session::{MapScriptSession, ScriptSessionError};
use std::{
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn stale(error: ScriptSessionError, expected: FunctionId) {
    assert!(
        matches!(error,ScriptSessionError::StaleCallback{id} if id==expected),
        "{error:?}"
    );
}

pub fn different_program_ordinals_are_rejected_before_effects() {
    let drops = Arc::new(AtomicUsize::new(0));
    let old = context(MAP, WTS, "ticket-old");
    let old_weak = Arc::downgrade(&old);
    let old_scripts = Arc::downgrade(&old.scripts);
    let old_id = id(&old, "Callback");
    let mut state = initialize(old, &drops);
    let queued = state.callback_ticket(old_id).unwrap();
    let queued_copy = queued.clone();
    let source = MAP
        .replace("Callback", "Different")
        .replace("counter + amount", "counter + amount + 100");
    let next = context(&source, WTS, "ticket-new");
    let next_weak = Arc::downgrade(&next);
    let new_id = id(&next, "Different");
    let entry = id(&next, "SelectedEntry");
    assert_eq!(old_id, new_id);
    state
        .replace(
            next,
            |ctx| Ok::<_, Infallible>(StatefulHost::new(ctx, &drops)),
            &[entry],
        )
        .unwrap();
    assert!(old_weak.upgrade().is_none());
    assert!(old_scripts.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let before = state.global("counter");
    let host = state.host().unwrap() as *const StatefulHost;
    stale(
        state.call_callback(&queued, &[Value::Int(2)]).unwrap_err(),
        old_id,
    );
    stale(state.call_callback(&queued_copy, &[]).unwrap_err(), old_id); // stale before arity checking
    assert_eq!(state.global("counter"), before);
    assert_eq!(state.host().unwrap() as *const StatefulHost, host);
    assert!(state.host().unwrap().text.is_empty());
    assert_eq!(state.host().unwrap().inits, 1);
    let current = state.callback_ticket(new_id).unwrap();
    assert_eq!(
        state.call_callback(&current, &[Value::Int(2)]).unwrap(),
        Value::Int(107)
    );
    drop(state);
    assert!(next_weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    // All tickets still exist here and retain only their weak session witnesses.
    drop((queued, queued_copy, current));
}

pub fn same_source_and_other_live_session_have_distinct_witnesses() {
    let drops = Arc::new(AtomicUsize::new(0));
    let shared = context(MAP, WTS, "same-source");
    let weak = Arc::downgrade(&shared);
    let callback = id(&shared, "Callback");
    let program = shared.scripts.program() as *const pcx_script::Program;
    let mut first = initialize(Arc::clone(&shared), &drops);
    let first_ticket = first.callback_ticket(callback).unwrap();
    assert_eq!(
        first
            .call_callback(&first_ticket, &[Value::Int(3)])
            .unwrap(),
        Value::Int(8)
    );
    let mut other = initialize(Arc::clone(&shared), &drops);
    let other_ticket = other.callback_ticket(callback).unwrap();
    stale(
        other
            .call_callback(&first_ticket, &[Value::Int(2)])
            .unwrap_err(),
        callback,
    );
    stale(
        first
            .call_callback(&other_ticket, &[Value::Int(2)])
            .unwrap_err(),
        callback,
    );
    assert_eq!(other.global("counter"), Some(Value::Int(5)));
    assert_eq!(first.global("counter"), Some(Value::Int(8)));
    assert_eq!(
        other
            .call_callback(&other_ticket, &[Value::Int(1)])
            .unwrap(),
        Value::Int(6)
    );
    first
        .replace(
            Arc::clone(&shared),
            |ctx| Ok::<_, Infallible>(StatefulHost::new(ctx, &drops)),
            &[],
        )
        .unwrap();
    assert_eq!(
        first.context().unwrap().scripts.program() as *const pcx_script::Program,
        program
    );
    stale(
        first
            .call_callback(&first_ticket, &[Value::Int(2)])
            .unwrap_err(),
        callback,
    );
    assert_eq!(first.global("counter"), Some(Value::Int(5)));
    assert_eq!(first.host().unwrap().inits, 1);
    let fresh = first.callback_ticket(callback).unwrap();
    assert_eq!(
        first.call_callback(&fresh, &[Value::Int(2)]).unwrap(),
        Value::Int(7)
    );
    drop(shared);
    drop(first);
    drop(other);
    assert!(weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    drop((first_ticket, other_ticket, fresh));
}

pub fn failed_candidate_keeps_existing_tickets_valid() {
    let drops = Arc::new(AtomicUsize::new(0));
    let old = context(MAP, WTS, "still-active");
    let callback = id(&old, "Callback");
    let old_weak = Arc::downgrade(&old);
    let mut state = initialize(old, &drops);
    let ticket = state.callback_ticket(callback).unwrap();
    let copy = ticket.clone();
    let candidate = context(MAP, WTS, "candidate-failed");
    let candidate_weak = Arc::downgrade(&candidate);
    let bad = id(&candidate, "BadEntry");
    let error = state
        .replace(
            candidate,
            |ctx| Ok::<_, Infallible>(StatefulHost::new(ctx, &drops)),
            &[bad],
        )
        .unwrap_err();
    let ScriptSessionError::Execution(failure) = error else {
        panic!("candidate must preserve WTS failure");
    };
    assert_eq!(failure.identity.source, "candidate-failed");
    assert!(
        failure
            .diagnostic
            .contains("candidate-failed:war3map.wts: WTS entry 9 is not UTF-8")
    );
    assert!(candidate_weak.upgrade().is_none());
    assert_eq!(state.context().unwrap().sources.map.source, "still-active");
    assert_eq!(
        state.call_callback(&ticket, &[Value::Int(2)]).unwrap(),
        Value::Int(7)
    );
    assert_eq!(
        state.call_callback(&copy, &[Value::Int(3)]).unwrap(),
        Value::Int(10)
    );
    let failed = context(MAP, WTS, "factory-failed");
    state
        .replace(
            failed,
            |_| Err::<StatefulHost, _>(std::io::Error::other("factory refusal")),
            &[],
        )
        .unwrap_err();
    assert_eq!(
        state.call_callback(&ticket, &[Value::Int(1)]).unwrap(),
        Value::Int(11)
    );
    assert_eq!(state.host().unwrap().inits, 1);
    drop(state);
    assert!(old_weak.upgrade().is_none());
    drop((ticket, copy));
}

pub fn clear_drop_and_tickets_release_discarded_owners() {
    let drops = Arc::new(AtomicUsize::new(0));
    let ctx = context(MAP, WTS, "clear-map");
    let weak = Arc::downgrade(&ctx);
    let scripts = Arc::downgrade(&ctx.scripts);
    let callback = id(&ctx, "Callback");
    let mut state = initialize(ctx, &drops);
    let held = state.callback_ticket(callback).unwrap();
    state.clear();
    assert!(weak.upgrade().is_none());
    assert!(scripts.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    stale(
        state.call_callback(&held, &[Value::Int(2)]).unwrap_err(),
        callback,
    );
    let ctx = context(MAP, WTS, "drop-map");
    let weak = Arc::downgrade(&ctx);
    let callback = id(&ctx, "Callback");
    let state = initialize(ctx, &drops);
    let after_drop = state.callback_ticket(callback).unwrap();
    drop(state);
    assert!(weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    let mut empty = MapScriptSession::<StatefulHost>::default();
    stale(empty.call_callback(&after_drop, &[]).unwrap_err(), callback);
    drop((held, after_drop));
}

pub fn issuance_bounds_and_no_current_session_are_typed() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut empty = MapScriptSession::<StatefulHost>::default();
    assert!(matches!(
        empty.callback_ticket(FunctionId(0)),
        Err(ScriptSessionError::NoSession)
    ));
    let ctx = context(MAP, WTS, "issuance-map");
    let callback = id(&ctx, "Callback");
    let mut state = initialize(ctx, &drops);
    assert!(matches!(
        state.callback_ticket(FunctionId(u32::MAX)),
        Err(ScriptSessionError::UnknownFunction {
            id: FunctionId(u32::MAX),
            ..
        })
    ));
    let ticket = state.callback_ticket(callback).unwrap();
    stale(
        empty.call_callback(&ticket, &[Value::Int(2)]).unwrap_err(),
        callback,
    );
    assert!(matches!(
        state.call_function(FunctionId(u32::MAX), &[]),
        Err(ScriptSessionError::UnknownFunction { .. })
    ));
    assert_eq!(state.global("counter"), Some(Value::Int(5)));
    assert_eq!(state.host().unwrap().inits, 1);
    assert!(state.host().unwrap().text.is_empty());
    assert_eq!(
        state.call_callback(&ticket, &[Value::Int(2)]).unwrap(),
        Value::Int(7)
    );
}

pub fn current_cloned_tickets_preserve_vm_state_and_diagnostics() {
    let drops = Arc::new(AtomicUsize::new(0));
    let ctx = context(MAP, WTS, "current-map");
    let callback = id(&ctx, "Callback");
    let bad = id(&ctx, "BadEntry");
    let prelude = id(&ctx, "PreludeFail");
    let mut state = initialize(ctx, &drops);
    let ticket = state.callback_ticket(callback).unwrap();
    let cloned = ticket.clone();
    let error = state.call_callback(&ticket, &[]).unwrap_err();
    let ScriptSessionError::Execution(failure) = error else {
        panic!("expected current declaration arity error");
    };
    assert_eq!(failure.function, Some(callback));
    assert_eq!(failure.source_label.as_deref(), Some("the map script"));
    assert!(failure.diagnostic.starts_with("current-map:war3map.j:7:1"));
    let bad_ticket = state.callback_ticket(bad).unwrap();
    let error = state.call_callback(&bad_ticket, &[]).unwrap_err();
    let ScriptSessionError::Execution(failure) = error else {
        panic!("expected actual WTS error");
    };
    assert!(
        failure
            .diagnostic
            .contains("current-map:war3map.wts: WTS entry 9 is not UTF-8")
    );
    let prelude_ticket = state.callback_ticket(prelude).unwrap();
    let error = state.call_callback(&prelude_ticket, &[]).unwrap_err();
    let ScriptSessionError::Execution(failure) = error else {
        panic!("expected actual prelude error");
    };
    assert_eq!(failure.source_label.as_deref(), Some("caller/BJ.j"));
    assert!(failure.diagnostic.starts_with("caller/BJ.j:4:1"));
    for t in [&ticket, &cloned, &ticket] {
        state.call_callback(t, &[Value::Int(2)]).unwrap();
    }
    assert_eq!(state.global("counter"), Some(Value::Int(11)));
    assert_eq!(state.host().unwrap().inits, 1);
    assert_eq!(
        state.host().unwrap().text,
        vec!["|cffff0000Authored|r|nText"; 3]
    );
}
