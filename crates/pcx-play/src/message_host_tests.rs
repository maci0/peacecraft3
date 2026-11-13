use super::{COMMON, budget, context, object, player, slots_rule};
use crate::playercolor_host::{MessageBudget, ObjectHost};
use pcx_script::{Host, Value};
const DECLARATIONS: &str = "
constant native GetLocalPlayer takes nothing returns player
native DisplayTimedTextToPlayer takes player p, real x, real y, real duration, string text returns nothing
native ClearTextMessages takes nothing returns nothing
native TimerStart takes timer t, real timeout, boolean periodic, code handler returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
";
fn host(map: &str, records: usize, bytes: usize) -> ObjectHost {
    let ctx = context(
        301,
        &format!("{COMMON}{DECLARATIONS}"),
        map,
        "synthetic:messages",
    );
    let mut host = object(&ctx, 32)
        .with_players(ctx.clone(), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_timer_runtime(0.)
        .unwrap();
    host.admit_local_player(&ctx, 2).unwrap();
    host.admit_messages(MessageBudget {
        records,
        text_bytes: bytes,
    })
    .unwrap();
    host
}
fn show(host: &mut ObjectHost, to: i32, text: &str, duration: f32) -> Result<Value, String> {
    let p = player(host, to);
    host.native(
        "DisplayTimedTextToPlayer",
        &[
            p,
            Value::Real(-0.2),
            Value::Real(0.3),
            Value::Real(duration),
            Value::Str(text.into()),
        ],
    )
}

#[test]
fn campaign_debug_text_is_source_admitted_and_preserves_live_messages() {
    let mut host = host("// silent AI debug", 1, 5);
    assert!(host.ai_debug_text().is_err());
    show(&mut host, 2, "local", 4.).unwrap();
    let before: Vec<_> = host.messages().unwrap().visible(0.).cloned().collect();
    let revision = host.messages().unwrap().revision();
    let handles = host.inner().handles();
    let context = host.player_context().unwrap().clone();
    let mut policy = crate::campaign_mode_policy::CampaignModePolicy::fixture();
    policy.campaign_mode = false;
    host.admit_campaign_mode(&context, policy).unwrap();
    for _ in 0..4 {
        host.ai_debug_text().unwrap();
    }
    assert_eq!(host.messages().unwrap().revision(), revision);
    assert_eq!(
        host.messages()
            .unwrap()
            .visible(0.)
            .cloned()
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(host.inner().handles(), handles);
    assert!(show(&mut host, 2, "extra", 1.).is_err());
}
#[test]
fn executed_messages_are_local_and_clear_releases_the_live_budget() {
    let mut vm = host("function run takes nothing returns nothing\ncall DisplayTimedTextToPlayer(Player(2),0,0,4,\"local\")\ncall DisplayTimedTextToPlayer(Player(1),0,0,4,\"other\")\nendfunction", 1, 5).into_vm();
    vm.init_globals().unwrap();
    vm.call("run", &[]).unwrap();
    let entries: Vec<_> = vm.host().messages().unwrap().visible(0.).cloned().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].text, "local");
    let handles = vm.host().inner().handles();
    assert!(show(vm.host(), 2, "full", 1.).is_err());
    assert_eq!(vm.host().messages().unwrap().revision(), 1);
    vm.host().native("ClearTextMessages", &[]).unwrap();
    assert_eq!(vm.host().messages().unwrap().visible(0.).count(), 0);
    show(vm.host(), 2, "again", 1.).unwrap();
    assert_eq!(vm.host().messages().unwrap().revision(), 3);
    assert_eq!(vm.host().inner().handles(), handles);
}
#[test]
fn logical_time_chunking_and_expiration_do_not_depend_on_rendering() {
    for stops in [&[4.0][..], &[0.5, 1.0, 2.0, 4.0], &[3.99, 4.0]] {
        let mut host = host("// map", 1, 4);
        show(&mut host, 2, "old", 4.).unwrap();
        assert_eq!(host.messages().unwrap().visible(3.999).count(), 1);
        for &until in stops {
            assert!(host.next_timer_expiry(until, &mut 8).unwrap().is_none());
        }
        assert_eq!(
            host.messages()
                .unwrap()
                .visible(host.timer_game_time().unwrap())
                .count(),
            0
        );
        show(&mut host, 2, "new", 1.).unwrap();
        let current = host.messages().unwrap().visible(4.).next().unwrap();
        assert_eq!(
            (
                current.text.as_utf8().unwrap(),
                current.started_at,
                current.expires_at
            ),
            ("new", 4., 5.)
        );
        assert_eq!(current.position, [0., 0.3]);
        assert!(!current.visible_at(f64::NAN));
    }
}
#[test]
fn bad_arguments_and_capacity_leave_the_previous_message_intact() {
    let mut host = host("// map", 2, 4);
    show(&mut host, 2, "old", 4.).unwrap();
    let player = player(&mut host, 2);
    for duration in [f32::NAN, f32::INFINITY, -1.] {
        assert!(show(&mut host, 2, "x", duration).is_err());
    }
    assert!(show(&mut host, 2, "long", 1.).is_err());
    assert!(show(&mut host, 2, "\0", 1.).is_err());
    assert!(host.native("ClearTextMessages", &[Value::Null]).is_err());
    assert!(
        host.native(
            "DisplayTimedTextToPlayer",
            &[
                Value::Null,
                Value::Real(0.),
                Value::Real(0.),
                Value::Real(1.),
                Value::Str("x".into())
            ]
        )
        .is_err()
    );
    assert!(
        host.native(
            "DisplayTimedTextToPlayer",
            &[
                player,
                Value::Real(f32::NAN),
                Value::Real(0.),
                Value::Real(1.),
                Value::Str("x".into())
            ]
        )
        .is_err()
    );
    assert_eq!(host.messages().unwrap().revision(), 1);
    assert_eq!(
        host.messages().unwrap().visible(0.).next().unwrap().text,
        "old"
    );
}
#[test]
fn optional_message_declarations_must_be_same_source_and_exact() {
    for declarations in [
        "",
        "constant native ClearTextMessages takes nothing returns nothing",
        "native ClearTextMessages takes integer n returns nothing",
    ] {
        let ctx = context(
            301,
            &format!("{COMMON}\n{declarations}"),
            "// map",
            "synthetic:messages",
        );
        let mut host = object(&ctx, 32);
        let admitted = host.admit_messages(MessageBudget {
            records: 1,
            text_bytes: 8,
        });
        if declarations.is_empty() {
            admitted.unwrap();
            assert!(host.messages().is_none());
        } else {
            assert!(admitted.is_err());
        }
        assert!(host.native("ClearTextMessages", &[]).is_err());
    }
    let ctx = context(
        301,
        COMMON,
        "native ClearTextMessages takes nothing returns nothing",
        "synthetic:foreign",
    );
    assert!(
        object(&ctx, 32)
            .admit_messages(MessageBudget {
                records: 1,
                text_bytes: 8
            })
            .is_err()
    );
}

#[test]
fn display_requires_a_real_local_player_and_the_logical_clock() {
    let ctx = context(
        301,
        &format!("{COMMON}{DECLARATIONS}"),
        "// map",
        "synthetic:missing-message-inputs",
    );
    let mut host = object(&ctx, 32)
        .with_players(ctx.clone(), slots_rule(301, -3, 9), budget())
        .unwrap();
    host.admit_messages(MessageBudget {
        records: 1,
        text_bytes: 8,
    })
    .unwrap();
    assert!(
        show(&mut host, 2, "x", 1.)
            .unwrap_err()
            .contains("local client")
    );
    host.admit_local_player(&ctx, 2).unwrap();
    assert!(
        show(&mut host, 2, "x", 1.)
            .unwrap_err()
            .contains("logical clock")
    );
    assert_eq!(host.messages().unwrap().revision(), 0);
    host.native("ClearTextMessages", &[]).unwrap();
    assert_eq!(host.messages().unwrap().revision(), 1);
    let mut host = host.with_timer_runtime(0.).unwrap();
    let timer = host.native("CreateTimer", &[]).unwrap();
    assert!(
        host.native(
            "DisplayTimedTextToPlayer",
            &[
                timer,
                Value::Real(0.),
                Value::Real(0.),
                Value::Real(1.),
                Value::Str("x".into())
            ]
        )
        .is_err()
    );
    assert_eq!(host.messages().unwrap().revision(), 1);
}

#[test]
fn each_host_keeps_the_message_layout_admitted_with_its_session() {
    use crate::message_layout::tests::rules;
    let first = host("// map", 1, 4)
        .with_message_layout(rules(0.5))
        .unwrap();
    let second = host("// map", 1, 4)
        .with_message_layout(rules(0.25))
        .unwrap();
    assert_eq!(
        first
            .message_layout()
            .unwrap()
            .placement(0., 0.)
            .unwrap()
            .width,
        0.5
    );
    assert_eq!(
        second
            .message_layout()
            .unwrap()
            .placement(0., 0.)
            .unwrap()
            .width,
        0.25
    );
    assert_ne!(
        first.message_layout().unwrap().placement(0., 0.).unwrap(),
        second.message_layout().unwrap().placement(0., 0.).unwrap()
    );
}

#[test]
fn a_published_message_layout_cannot_be_rebound() {
    use crate::message_layout::tests::rules;
    let host = host("// map", 1, 4)
        .with_message_layout(rules(0.5))
        .unwrap();
    let Err(error) = host.with_message_layout(rules(0.25)) else {
        panic!("a second layout must be refused");
    };
    assert!(error.to_string().contains("layout already admitted"));
}

#[test]
fn native_string_messages_filter_remote_before_lookup_and_budget_decoded_bytes() {
    use std::cell::RefCell;
    let mut h = host("// map", 2, 4);
    let remote = player(&mut h, 1);
    let local = player(&mut h, 2);
    let calls = RefCell::new(Vec::new());
    let source = |id| {
        calls.borrow_mut().push(id);
        if id == 7 {
            Ok(Some("é".into()))
        } else {
            Err("fixture.wts invalid record".into())
        }
    };
    let args = |p| {
        [
            p,
            Value::Real(0.),
            Value::Real(0.),
            Value::Real(4.),
            Value::Str("TRIGSTR_7".into()),
        ]
    };
    h.native_with_strings("DisplayTimedTextToPlayer", &args(remote), &source)
        .unwrap();
    assert!(calls.borrow().is_empty());
    h.native_with_strings("DisplayTimedTextToPlayer", &args(local.clone()), &source)
        .unwrap();
    assert_eq!(*calls.borrow(), [7]);
    assert_eq!(h.messages().unwrap().visible(0.).next().unwrap().text, "é");
    let revision = h.messages().unwrap().revision();
    let mut broken = args(local);
    broken[4] = Value::Str("TRIGSTR_8".into());
    assert!(
        h.native_with_strings("DisplayTimedTextToPlayer", &broken, &source)
            .unwrap_err()
            .contains("fixture.wts invalid record")
    );
    assert_eq!(h.messages().unwrap().revision(), revision);
}

#[test]
fn native_string_player_names_resolve_once_after_player_identity() {
    use std::cell::Cell;
    let mut h = host("// names", 1, 32);
    let p = player(&mut h, 2);
    let calls = Cell::new(0);
    let source = |_| {
        calls.set(calls.get() + 1);
        Ok(Some("é".into()))
    };
    assert!(
        h.native_with_strings(
            "SetPlayerName",
            &[Value::Null, Value::Str("TRIGSTR_7".into())],
            &source
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
    h.native_with_strings(
        "SetPlayerName",
        &[p.clone(), Value::Str("TRIGSTR_7".into())],
        &source,
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        h.native("GetPlayerName", &[p]).unwrap(),
        Value::Str("é".into())
    );
}

#[test]
fn message_bytes_survive_retention_and_local_budget_failures_are_atomic() {
    let mut h = host("// raw payload", 1, 2);
    let local = player(&mut h, 2);
    let foreign = player(&mut h, 1);
    let args = |player, bytes: Vec<u8>| {
        vec![
            player,
            Value::Real(0.),
            Value::Real(0.),
            Value::Real(1.),
            Value::Str(bytes.into()),
        ]
    };
    h.native("DisplayTimedTextToPlayer", &args(foreign, vec![0xff; 3]))
        .unwrap();
    assert_eq!(h.messages().unwrap().visible(0.).count(), 0);
    h.native(
        "DisplayTimedTextToPlayer",
        &args(local.clone(), vec![0xc3, 0xff]),
    )
    .unwrap();
    assert_eq!(
        h.messages()
            .unwrap()
            .visible(0.)
            .next()
            .unwrap()
            .text
            .as_bytes(),
        &[0xc3, 0xff]
    );
    let revision = h.messages().unwrap().revision();
    assert!(
        h.native(
            "DisplayTimedTextToPlayer",
            &args(local.clone(), vec![0xff; 3])
        )
        .is_err()
    );
    assert_eq!(h.messages().unwrap().revision(), revision);
    assert_eq!(
        h.messages()
            .unwrap()
            .visible(0.)
            .next()
            .unwrap()
            .text
            .as_bytes(),
        &[0xc3, 0xff]
    );
    h.native("ClearTextMessages", &[]).unwrap();
    h.native("DisplayTimedTextToPlayer", &args(local, vec![0xff]))
        .unwrap();
    assert_eq!(
        h.messages()
            .unwrap()
            .visible(0.)
            .next()
            .unwrap()
            .text
            .as_bytes(),
        &[0xff]
    );
}
