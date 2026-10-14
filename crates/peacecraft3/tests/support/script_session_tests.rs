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
