//! Control groups driven through the real key system on a synthetic published session.
//! No original asset is read: the rules and texts are synthetic values with the
//! shapes the loader produces, and the keys are injected into `ButtonInput`.
use super::unit_range_cases::{Geometry, placement};
use super::*;
use pcx_play::control_group_rules::{ControlGroupRules, ErrorKeys};
use pcx_play::control_groups::Group;
use pcx_script::{HandleId, Host};
use peacecraft3::campaign_control_groups::{
    CampaignControlGroups, ControlGroupCentred, ControlGroupSelected, ErrorTexts, Source,
};
use peacecraft3::game_input::{Closer, GameInput};
use std::time::Duration;

const EXTRA: &str = "constant native GetLocalPlayer takes nothing returns player\nnative RemoveUnit takes unit u returns nothing\n";
const SCENE: &str = "function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n";
const CAP: usize = 4;
const WINDOW: f32 = 0.75;
/// Digit `d` binds group `(d + 3) % 10`, unlike the keyboard order, so a test that
/// passes proves the binding is read from the rules.
const BOUND: fn(usize) -> u8 = |digit| ((digit + 3) % 10) as u8;

fn session() -> MapScriptSession<MapStartupHost> {
    let mut map = fixture(SCENE);
    map.common
        .extend_from_slice(super::script_timer_cases::TIMER_COMMON.as_bytes());
    map.common
        .extend_from_slice(super::timed_trigger_cases::COMMON.as_bytes());
    map.common
        .extend_from_slice(super::unit_range_cases::EXTRA.as_bytes());
    map.common.extend_from_slice(EXTRA.as_bytes());
    map.rebuild();
    let reader = map.resolver();
    let bindings = bind_map(&reader, map.input(1), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::new(Geometry));
    inputs.timer_time = Some(10.);
    inputs.enumeration.limits.max_handles = 256;
    inputs.budgets.units.max_records = 32;
    inputs.budgets.units.max_retained_fields = 256;
    inputs.enumeration.names.push("ConvertGameEvent".into());
    inputs.budgets.conditions =
        Some(pcx_play::playercolor_host::ConditionBudget { max_records: 8 });
    inputs.budgets.trigger_conditions = Some(8);
    inputs.budgets.unit_in_range = Some(8);
    inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
        initial_enabled: true,
        timer_event_global: "TEST_TIMER_EVENT".into(),
        max_timed_events: 8,
        max_parked_threads: 8,
    });
    let payload = Arc::clone(&bindings.catalog_map_bytes);
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    app.insert_resource(MapRequests {
        bindings: Some(bindings),
        ..Default::default()
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic control group scene".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id,
        binding_revision: 1,
        payload: MapPayload::Shared(payload),
    });
    app.update();
    let ctx = app
        .world()
        .resource::<MapRequests>()
        .admission
        .shared_context()
        .unwrap();
    let entries = [
        ctx.scripts.program().function_id("config").unwrap(),
        ctx.scripts.program().function_id("main").unwrap(),
    ];
    let mut session = MapScriptSession::default();
    session
        .replace(
            Arc::clone(&ctx),
            move |ctx| {
                let source = ctx.world.source.clone();
                let mut host = pcx_play::map_startup_host::assemble(Arc::clone(&ctx), inputs)?;
                host.objects_mut().admit_unit_placement(placement(source))?;
                host.objects_mut()
                    .admit_unit_range_runtime("TEST_RANGE", 8)?;
                host.objects_mut().admit_local_player(&ctx, 0)?;
                Ok::<_, pcx_play::map_startup_host::MapStartupError>(host)
            },
            &entries,
        )
        .unwrap();
    session
}

fn rules() -> ControlGroupRules {
    ControlGroupRules {
        origin: AssetIdentity {
            source: "synthetic client".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        },
        image_sha256: [0; 32],
        digit_groups: std::array::from_fn(BOUND),
        selection_cap: CAP,
        double_press_seconds: WINDOW,
        error_keys: ErrorKeys {
            invalid: "Invalid".into(),
            empty: "Empty".into(),
            non_unit: "NonUnit".into(),
        },
    }
}

fn texts() -> ErrorTexts {
    ErrorTexts {
        invalid: "synthetic: not yours".into(),
        empty: "synthetic: nothing selected".into(),
        non_unit: "synthetic: not units".into(),
    }
}

struct Rig {
    app: App,
    /// Units of the local player, in creation order.
    own: Vec<HandleId>,
    foreign: HandleId,
}

fn create(session: &mut MapScriptSession<MapStartupHost>, player: i32, x: f32) -> HandleId {
    let objects = session.host().unwrap().objects_mut();
    let Value::Handle(owner) = objects.native("Player", &[Value::Int(player)]).unwrap() else {
        panic!("player handle")
    };
    let Value::Handle(unit) = objects
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"UNIT")),
                Value::Real(x),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    else {
        panic!("unit handle")
    };
    unit
}

fn rig() -> Rig {
    let mut session = session();
    let own = (0..6)
        .map(|n| create(&mut session, 0, 16. + 8. * n as f32))
        .collect();
    let foreign = create(&mut session, 1, 80.);
    let mut app = App::new();
    app.insert_resource(session)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Time<Real>>();
    peacecraft3::campaign_control_groups::install(&mut app);
    // The first frame binds the groups to the session.
    app.update();
    app.world_mut()
        .resource_mut::<CampaignControlGroups>()
        .admit(Ok(Source {
            rules: rules(),
            texts: texts(),
        }));
    Rig { app, own, foreign }
}

impl Rig {
    fn objects<R>(
        &mut self,
        read: impl FnOnce(&mut pcx_play::playercolor_host::ObjectHost) -> R,
    ) -> R {
        let mut session = self
            .app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        read(session.host().unwrap().objects_mut())
    }

    fn select(&mut self, units: &[HandleId]) {
        let units = units.to_vec();
        self.objects(|objects| objects.select_units(&units, true));
    }

    fn selection(&mut self) -> Vec<HandleId> {
        self.objects(|objects| objects.selection().to_vec())
    }

    fn key(&mut self, key: KeyCode) {
        self.app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
    }

    fn release(&mut self, key: KeyCode) {
        self.app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
    }

    /// One rendered frame `seconds` after the previous one, as the input plugin
    /// would leave it: presses are seen once, then cleared.
    fn frame(&mut self, seconds: f32) {
        self.app
            .world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs_f32(seconds));
        self.app.update();
        self.app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    /// Presses and releases `digit` over one frame with `modifiers` held.
    fn digit(&mut self, digit: usize, modifiers: &[KeyCode], seconds: f32) {
        let key = [
            KeyCode::Digit0,
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ][digit];
        for modifier in modifiers {
            self.key(*modifier);
        }
        self.key(key);
        self.frame(seconds);
        self.release(key);
        for modifier in modifiers {
            self.release(*modifier);
        }
    }

    fn members(&self, digit: usize) -> Vec<HandleId> {
        self.app
            .world()
            .resource::<CampaignControlGroups>()
            .groups(0)
            .map(|groups| groups.members(Group::new(BOUND(digit)).unwrap()).to_vec())
            .unwrap_or_default()
    }

    fn notice(&self) -> Option<String> {
        self.app
            .world()
            .resource::<CampaignControlGroups>()
            .notice
            .clone()
    }

    fn selected(&mut self) -> Vec<Vec<HandleId>> {
        self.app
            .world_mut()
            .resource_mut::<Messages<ControlGroupSelected>>()
            .drain()
            .map(|message| message.units)
            .collect()
    }

    fn centred(&mut self) -> Vec<[f32; 2]> {
        self.app
            .world_mut()
            .resource_mut::<Messages<ControlGroupCentred>>()
            .drain()
            .map(|message| message.position)
            .collect()
    }
}

const CTRL: &[KeyCode] = &[KeyCode::ControlLeft];
const SHIFT: &[KeyCode] = &[KeyCode::ShiftRight];

#[test]
fn ctrl_digit_defines_the_group_the_rules_bind_and_the_digit_selects_it() {
    let mut rig = rig();
    let own = rig.own.clone();
    rig.select(&own[..3]);
    rig.digit(1, CTRL, 0.1);
    assert_eq!(rig.members(1), own[..3]);
    assert_eq!(rig.notice(), None);
    assert!(
        rig.members(2).is_empty(),
        "another digit's group is untouched"
    );

    rig.select(&[own[5]]);
    rig.digit(2, &[], 0.1);
    assert_eq!(rig.selection(), [own[5]], "an empty group changes nothing");
    assert!(rig.selected().is_empty());

    rig.digit(1, &[], 0.1);
    assert_eq!(rig.selection(), own[..3]);
    assert_eq!(rig.selected(), [own[..3].to_vec()]);
}

#[test]
fn the_cap_limits_what_a_define_keeps() {
    let mut rig = rig();
    let own = rig.own.clone();
    rig.select(&own);
    assert_eq!(rig.selection().len(), 6);
    rig.digit(0, CTRL, 0.1);
    assert_eq!(rig.members(0), own[..CAP]);
    rig.digit(0, &[], 0.1);
    assert_eq!(rig.selection(), own[..CAP]);
}

#[test]
fn shift_digit_adds_the_selection_without_duplicates_and_ctrl_wins_over_shift() {
    let mut rig = rig();
    let own = rig.own.clone();
    rig.select(&own[..2]);
    rig.digit(4, CTRL, 0.1);
    rig.select(&[own[1], own[3]]);
    rig.digit(4, SHIFT, 0.1);
    assert_eq!(rig.members(4), [own[0], own[1], own[3]]);
    rig.select(&[own[4]]);
    rig.digit(4, &[KeyCode::ShiftLeft, KeyCode::ControlRight], 0.1);
    assert_eq!(
        rig.members(4),
        [own[4]],
        "Ctrl defines even with Shift held"
    );
}

#[test]
fn refusals_post_the_string_table_texts_and_keep_the_group() {
    let mut rig = rig();
    let (own, foreign) = (rig.own.clone(), rig.foreign);
    rig.digit(5, CTRL, 0.1);
    assert_eq!(rig.notice().as_deref(), Some("synthetic: nothing selected"));
    rig.select(&own[..2]);
    rig.digit(5, CTRL, 0.1);
    assert_eq!(rig.notice(), None, "an accepted key clears the text");
    rig.select(&[foreign]);
    rig.digit(5, CTRL, 0.1);
    assert_eq!(rig.notice().as_deref(), Some("synthetic: not yours"));
    assert_eq!(rig.members(5), own[..2]);
    rig.digit(5, SHIFT, 0.1);
    assert_eq!(rig.members(5), own[..2], "the merge keeps the old members");
    assert_eq!(rig.notice(), None);
}

#[test]
fn removed_units_leave_the_group_and_hidden_ones_wait() {
    let mut rig = rig();
    let own = rig.own.clone();
    rig.select(&own[..4]);
    rig.digit(6, CTRL, 0.1);
    rig.objects(|objects| {
        objects
            .native("RemoveUnit", &[Value::Handle(own[1])])
            .unwrap();
        objects
            .native("ShowUnit", &[Value::Handle(own[2]), Value::Bool(false)])
            .unwrap();
    });
    rig.digit(6, &[], 0.1);
    assert_eq!(rig.selection(), [own[0], own[3]]);
    assert_eq!(rig.members(6), [own[0], own[2], own[3]]);
    rig.objects(|objects| {
        objects
            .native("ShowUnit", &[Value::Handle(own[2]), Value::Bool(true)])
            .unwrap();
    });
    rig.digit(6, &[], 2.0);
    assert_eq!(rig.selection(), [own[0], own[2], own[3]]);
}

#[test]
fn a_second_press_inside_the_window_centres_on_the_primary_unit_once() {
    let mut rig = rig();
    let own = rig.own.clone();
    rig.select(&[own[2], own[0]]);
    rig.digit(3, CTRL, 0.1);
    let primary = rig.objects(|objects| objects.executed_unit(own[2]).unwrap().position.unwrap());
    // First press selects only.
    rig.digit(3, &[], 5.0);
    assert!(rig.centred().is_empty());
    // Within the window: centre, and the memory is spent.
    rig.digit(3, &[], WINDOW - 0.25);
    assert_eq!(rig.centred(), [primary]);
    rig.digit(3, &[], 0.1);
    assert!(rig.centred().is_empty(), "the pair was consumed");
    // A press after the window starts a new pair.
    rig.digit(3, &[], WINDOW + 0.1);
    assert!(rig.centred().is_empty());
    // A press of another non-empty group between two presses breaks the pair.
    rig.select(&[own[4]]);
    rig.digit(2, CTRL, WINDOW + 0.1);
    rig.digit(3, &[], WINDOW + 0.1);
    rig.digit(2, &[], 0.1);
    rig.digit(3, &[], 0.1);
    assert!(rig.centred().is_empty());
}

#[test]
fn closed_input_ignores_digits_and_other_keys_do_not_form_groups() {
    let mut rig = rig();
    let own = rig.own.clone();
    rig.select(&own[..2]);
    rig.app
        .world_mut()
        .resource_mut::<GameInput>()
        .set_closer(Closer::ScriptDialog, true);
    rig.digit(7, CTRL, 0.1);
    assert!(rig.members(7).is_empty());
    rig.app
        .world_mut()
        .resource_mut::<GameInput>()
        .set_closer(Closer::ScriptDialog, false);
    rig.key(KeyCode::Numpad7);
    rig.key(KeyCode::KeyA);
    rig.frame(0.1);
    assert!(rig.members(7).is_empty());
    // Alt is not read by the digit handler.
    rig.digit(7, &[KeyCode::ControlLeft, KeyCode::AltLeft], 0.1);
    assert_eq!(rig.members(7), own[..2]);
}

#[test]
fn a_new_session_starts_without_groups_and_without_the_old_source() {
    let mut rig = rig();
    let own = rig.own.clone();
    rig.select(&own[..2]);
    rig.digit(8, CTRL, 0.1);
    assert_eq!(rig.members(8), own[..2]);
    rig.app.insert_resource(session());
    rig.frame(0.1);
    let state = rig.app.world().resource::<CampaignControlGroups>();
    assert!(state.groups(0).is_none());
    assert!(state.needs_source());
}
