use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use std::{collections::VecDeque, sync::Arc};

#[test]
fn ai_callback_boundary_refuses_underflow_and_preserves_foreign_frames_before_restore() {
    let syntax = pcx_script::parse_script("").unwrap();
    let program = Arc::new(pcx_script::resolve(&syntax).unwrap());
    let inner = EnumHost::checked_selected(
        program,
        "synthetic callback boundary",
        &[],
        Limits {
            max_handles: 16,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    let mut host = ObjectHost::checked(inner).unwrap();
    let frame = || crate::playercolor_host::objects::SynchronousFrame {
        result: Value::Int(7),
        events: VecDeque::new(),
        active: false,
        spawn_depth: 0,
        callback_depth: 0,
        conditions: None,
    };
    host.synchronous_frames.push(frame());
    host.events.subject.spell_ability = Some(7);
    let baseline = host.capture_ai_callback_boundary();
    host.events.subject.spell_ability = Some(9);
    host.threads.expired_timer = Some(None);
    for kind in 0..5 {
        let mut invalid = baseline;
        match kind {
            0 => invalid.synchronous = 3,
            1 => invalid.callbacks = 1,
            2 => invalid.threads = 1,
            3 => invalid.previous = 1,
            _ => {
                invalid.synchronous = 3;
                invalid.previous = 1;
            }
        }
        assert!(host.abort_ai_callback_boundary(invalid).is_err());
        assert_eq!(host.synchronous_frames.len(), 1);
        assert_eq!(host.events.subject.spell_ability, Some(9));
        assert_eq!(host.threads.expired_timer, Some(None));
    }
    host.synchronous_frames.push(frame());
    host.abort_ai_callback_boundary(baseline).unwrap();
    assert_eq!(host.synchronous_frames.len(), 1);
    assert_eq!(host.synchronous_frames[0].result, Value::Int(7));
    assert_eq!(host.events.subject.spell_ability, Some(7));
    assert_eq!(host.threads.expired_timer, None);
    host.threads.expired_timer = Some(None);
    let explicit_null = host.capture_ai_callback_boundary();
    host.threads.expired_timer = None;
    host.abort_ai_callback_boundary(explicit_null).unwrap();
    assert_eq!(host.threads.expired_timer, Some(None));
}
