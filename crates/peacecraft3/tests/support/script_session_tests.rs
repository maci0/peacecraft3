use crate::{consumer_cases, ticket_cases};

#[test]
fn persistent_ecs_session() {
    consumer_cases::admitted_resource_preserves_vm_host_and_callbacks_across_updates();
}
#[test]
fn candidate_refusal_keeps_old_vm() {
    consumer_cases::failed_candidate_keeps_old_context_and_retains_candidate_diagnostics();
}
#[test]
fn unsupported_neutral_boundary() {
    consumer_cases::neutral_global_refusal_preserves_an_existing_session();
}
#[test]
fn caller_factory_and_ids() {
    consumer_cases::explicit_factory_and_id_refusals_preserve_active_state();
}
#[test]
fn successful_replacement_and_release() {
    consumer_cases::replacement_releases_old_context_and_switches_world_wts_and_state_together();
}
#[test]
fn source_and_external_effect_seams() {
    consumer_cases::source_errors_and_external_effects_keep_their_explicit_boundaries();
}

#[test]
fn different_program() {
    ticket_cases::different_program_ordinals_are_rejected_before_effects();
}
#[test]
fn same_source_and_wrong_live_session() {
    ticket_cases::same_source_and_other_live_session_have_distinct_witnesses();
}
#[test]
fn failed_candidate_keeps_tickets() {
    ticket_cases::failed_candidate_keeps_existing_tickets_valid();
}
#[test]
fn tickets_do_not_retain_owners() {
    ticket_cases::clear_drop_and_tickets_release_discarded_owners();
}
#[test]
fn issuance_and_empty_state() {
    ticket_cases::issuance_bounds_and_no_current_session_are_typed();
}
#[test]
fn real_callbacks_and_diagnostics() {
    ticket_cases::current_cloned_tickets_preserve_vm_state_and_diagnostics();
}

#[test]
fn suspended_candidate_keeps_the_synchronous_session_and_its_ticket() {
    use pcx_script::{Host, Value};
    use peacecraft3::map_script_session::{MapScriptSession, ScriptSessionError};
    use std::{
        convert::Infallible,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    struct Suspends(consumer_cases::StatefulHost);
    impl Host for Suspends {
        fn suspend(&mut self, name: &str, _: &[Value]) -> bool {
            name == "Refuse"
        }
        fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
            self.0.native(name, args)
        }
    }
    let drops = Arc::new(AtomicUsize::new(0));
    let safe = consumer_cases::context(
        consumer_cases::MAP,
        consumer_cases::WTS,
        "safe synchronous session",
    );
    let mut session = MapScriptSession::<Suspends>::default();
    session
        .replace(
            Arc::clone(&safe),
            |ctx| Ok::<_, Infallible>(Suspends(consumer_cases::StatefulHost::new(ctx, &drops))),
            &[consumer_cases::id(&safe, "SelectedEntry")],
        )
        .unwrap();
    let ticket = session
        .callback_ticket(consumer_cases::id(&safe, "Read"))
        .unwrap();
    let text = format!(
        "{}\nfunction main takes nothing returns nothing\ncall PreludeFail()\nendfunction\n",
        consumer_cases::MAP
    );
    let candidate = consumer_cases::context(&text, consumer_cases::WTS, "suspending candidate");
    let entry = consumer_cases::id(&candidate, "main");
    let error = session
        .replace(
            candidate,
            |ctx| Ok::<_, Infallible>(Suspends(consumer_cases::StatefulHost::new(ctx, &drops))),
            &[entry],
        )
        .unwrap_err();
    let ScriptSessionError::Execution(failure) = error else {
        panic!("expected a suspension refusal")
    };
    assert_eq!(failure.source_label.as_deref(), Some("caller/BJ.j"));
    assert!(failure.diagnostic.contains("suspended"));
    let span = failure.error.span;
    assert_eq!(
        consumer_cases::BJ[span.start as usize..span.end as usize].trim(),
        "call Refuse()"
    );
    assert!(Arc::ptr_eq(session.context().unwrap(), &safe));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(session.call_callback(&ticket, &[]).unwrap(), Value::Int(5));
}
