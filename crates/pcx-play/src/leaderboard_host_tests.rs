use super::{COMMON, budget as player_budget, context, context_with_wts, id, player, slots_rule};
use crate::playercolor_host::{
    EnumHost, Failure, LeaderboardBudget, Limits, MapConfigBudget, ObjectHost,
};
use pcx_script::{Host, Value};

const DECLARATIONS: &str = r#"
type leaderboard extends agent
native CreateLeaderboard takes nothing returns leaderboard
native DestroyLeaderboard takes leaderboard b returns nothing
native LeaderboardDisplay takes leaderboard b, boolean shown returns nothing
native IsLeaderboardDisplayed takes leaderboard b returns boolean
native LeaderboardGetItemCount takes leaderboard b returns integer
native LeaderboardSetSizeByItemCount takes leaderboard b, integer n returns nothing
native LeaderboardAddItem takes leaderboard b, string label, integer value, player p returns nothing
native LeaderboardRemovePlayerItem takes leaderboard b, player p returns nothing
native LeaderboardHasPlayerItem takes leaderboard b, player p returns boolean
native LeaderboardGetPlayerIndex takes leaderboard b, player p returns integer
native LeaderboardSetLabel takes leaderboard b, string label returns nothing
native LeaderboardGetLabelText takes leaderboard b returns string
native PlayerSetLeaderboard takes player p, leaderboard b returns nothing
native PlayerGetLeaderboard takes player p returns leaderboard
native LeaderboardSetLabelColor takes leaderboard b, integer r, integer g, integer blue, integer a returns nothing
native LeaderboardSetValueColor takes leaderboard b, integer r, integer g, integer blue, integer a returns nothing
native LeaderboardSetStyle takes leaderboard b, boolean title, boolean names, boolean values, boolean icons returns nothing
native LeaderboardSetItemLabel takes leaderboard b, integer i, string label returns nothing
native LeaderboardSetItemValue takes leaderboard b, integer i, integer v returns nothing
native LeaderboardSetItemLabelColor takes leaderboard b, integer i, integer r, integer g, integer blue, integer a returns nothing
native LeaderboardSetItemValueColor takes leaderboard b, integer i, integer r, integer g, integer blue, integer a returns nothing
"#;
fn budget() -> LeaderboardBudget {
    LeaderboardBudget {
        max_boards: 2,
        max_items: 3,
        max_assignments: 2,
        text_bytes: 64,
    }
}
fn from_context(
    ctx: std::sync::Arc<crate::map_admission::ParsedMapContext>,
    budget: LeaderboardBudget,
) -> Result<ObjectHost, crate::playercolor_host::Error> {
    let enums = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
        ],
        Limits {
            max_handles: 64,
            admission_steps: 4096,
            expression_depth: 32,
            expression_work: 4096,
        },
    )
    .unwrap();
    ObjectHost::checked(enums)
        .unwrap()
        .with_map_config(MapConfigBudget {
            text_bytes: 128,
            start_locations: 2,
        })
        .unwrap()
        .with_players(ctx.clone(), slots_rule(301, -3, 9), player_budget())
        .unwrap()
        .with_leaderboards(budget)
}
fn try_host(
    declarations: &str,
    budget: LeaderboardBudget,
) -> Result<ObjectHost, crate::playercolor_host::Error> {
    from_context(
        context(
            301,
            &format!("{COMMON}{declarations}"),
            "// synthetic map",
            "synthetic:leaderboard",
        ),
        budget,
    )
}
fn host() -> ObjectHost {
    try_host(DECLARATIONS, budget()).unwrap()
}
fn text(s: &str) -> Value {
    Value::Str(s.into())
}
fn create(h: &mut ObjectHost) -> Value {
    h.native("CreateLeaderboard", &[]).unwrap()
}
fn add(h: &mut ObjectHost, board: &Value, p: &Value, label: &str, value: i32) {
    h.native(
        "LeaderboardAddItem",
        &[board.clone(), text(label), Value::Int(value), p.clone()],
    )
    .unwrap();
}
fn snapshot(h: &ObjectHost) -> String {
    format!(
        "{:?};{}",
        h.leaderboard_state().unwrap(),
        h.inner().handles()
    )
}

#[test]
fn leaderboard_source_missing_rows_and_invalid_value_updates_preserve_state() {
    let mut h = host()
        .with_leaderboard_layout(crate::leaderboard_layout::tests::synthetic_rules())
        .unwrap();
    let present = player(&mut h, 1);
    let missing = player(&mut h, 2);
    let board = create(&mut h);
    add(&mut h, &board, &present, "kept", 29);
    let before = snapshot(&h);
    for (b, p, expected) in [
        (board.clone(), missing.clone(), -7),
        (board.clone(), Value::Null, -7),
        (Value::Null, present.clone(), -7),
        (board.clone(), present.clone(), 0),
    ] {
        assert_eq!(
            h.native("LeaderboardGetPlayerIndex", &[b, p]).unwrap(),
            Value::Int(expected)
        );
        assert_eq!(snapshot(&h), before);
    }
    for row in [-7, -1, 1, i32::MAX] {
        h.native(
            "LeaderboardSetItemValue",
            &[board.clone(), Value::Int(row), Value::Int(31)],
        )
        .unwrap();
        assert_eq!(snapshot(&h), before);
    }
    h.native(
        "LeaderboardSetItemValue",
        &[Value::Null, Value::Int(0), Value::Int(31)],
    )
    .unwrap();
    assert_eq!(snapshot(&h), before);
    for args in [
        vec![present.clone(), missing.clone()],
        vec![board.clone(), board.clone()],
        vec![
            Value::Handle(pcx_script::HandleId::new(
                std::num::NonZeroU64::new(999_999).unwrap(),
            )),
            present.clone(),
        ],
        vec![board.clone(), Value::Int(1)],
    ] {
        assert!(h.native("LeaderboardGetPlayerIndex", &args).is_err());
        assert_eq!(snapshot(&h), before);
    }
    h.native(
        "LeaderboardSetItemValue",
        &[board.clone(), Value::Int(0), Value::Int(-37)],
    )
    .unwrap();
    assert_eq!(
        h.leaderboard_state()
            .unwrap()
            .get(id(&board))
            .unwrap()
            .items[0]
            .value,
        -37
    );
    h.native("DestroyLeaderboard", std::slice::from_ref(&board))
        .unwrap();
    let retired = snapshot(&h);
    assert_eq!(
        h.native("LeaderboardGetPlayerIndex", &[board.clone(), present])
            .unwrap(),
        Value::Int(-7)
    );
    h.native(
        "LeaderboardSetItemValue",
        &[board, Value::Int(0), Value::Int(31)],
    )
    .unwrap();
    assert_eq!(snapshot(&h), retired);
}

#[test]
fn leaderboard_compiled_player_value_helper_ignores_missing_rows_and_updates_present_rows() {
    let ctx = context(
        301,
        &format!("{COMMON}{DECLARATIONS}"),
        r#"
        globals
            leaderboard Board = null
            integer Missing = 0
        endglobals
        function LeaderboardSetPlayerItemValueBJ takes player whichPlayer, leaderboard lb, integer val returns nothing
            call LeaderboardSetItemValue(lb, LeaderboardGetPlayerIndex(lb, whichPlayer), val)
        endfunction
        function main takes nothing returns nothing
            set Board = CreateLeaderboard()
            call LeaderboardAddItem(Board, "retained", 11, Player(1))
            set Missing = LeaderboardGetPlayerIndex(Board, Player(2))
            call LeaderboardSetPlayerItemValueBJ(Player(2), Board, 71)
            call LeaderboardSetPlayerItemValueBJ(Player(1), Board, 43)
        endfunction
    "#,
        "synthetic:leaderboard-player-value",
    );
    let object = from_context(ctx.clone(), budget())
        .unwrap()
        .with_leaderboard_layout(crate::leaderboard_layout::tests::synthetic_rules())
        .unwrap();
    let mut vm = ctx.scripts.clone().owned_vm(object);
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("Missing"), Value::Int(-7));
    let board_id = id(&vm.global("Board"));
    let state = vm.host().inner.leaderboard_state().unwrap();
    let board = state.get(board_id).unwrap();
    assert_eq!(board.items.len(), 1);
    assert_eq!(board.items[0].label, "retained");
    assert_eq!(board.items[0].value, 43);
    assert_eq!(state.revision(), 3);
}

#[test]
#[ignore = "requires PCX_WAR3_EXE pointing at the reviewed local client"]
fn leaderboard_selected_source_missing_player_value_path_uses_public_host() {
    use std::io::Read;
    let mut image = Vec::new();
    std::fs::File::open(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE"))
        .unwrap()
        .take((32 << 20) + 1)
        .read_to_end(&mut image)
        .unwrap();
    assert!(image.len() <= 32 << 20);
    let rules = crate::leaderboard_layout::Rules::reviewed_legacy(
        &image,
        crate::scripts::AssetIdentity {
            source: "explicit local client qualification".into(),
            edition: "legacy".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    let missing_index = rules.missing_player_index;
    let mut h = host().with_leaderboard_layout(rules).unwrap();
    let p = player(&mut h, 1);
    let board = create(&mut h);
    let before = snapshot(&h);
    let index = h
        .native("LeaderboardGetPlayerIndex", &[board.clone(), p])
        .unwrap();
    assert_eq!(index, Value::Int(missing_index));
    h.native("LeaderboardSetItemValue", &[board, index, Value::Int(53)])
        .unwrap();
    assert_eq!(snapshot(&h), before);
}

#[test]
fn leaderboard_lifecycle_rows_associations_and_display_are_independent() {
    let mut h = host();
    let a = player(&mut h, 1);
    let b = player(&mut h, 2);
    let board = create(&mut h);
    let other = create(&mut h);
    assert_eq!(h.inner().kind(id(&board)).unwrap(), "leaderboard");
    assert_eq!(
        h.native("PlayerGetLeaderboard", std::slice::from_ref(&a))
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        h.native("LeaderboardGetItemCount", std::slice::from_ref(&board))
            .unwrap(),
        Value::Int(0)
    );
    h.native("LeaderboardSetLabel", &[board.clone(), text("title")])
        .unwrap();
    h.native("PlayerSetLeaderboard", &[a.clone(), board.clone()])
        .unwrap();
    h.native("PlayerSetLeaderboard", &[b.clone(), board.clone()])
        .unwrap();
    h.native("LeaderboardDisplay", &[board.clone(), Value::Bool(true)])
        .unwrap();
    add(&mut h, &board, &b, "second player first row", -17);
    add(&mut h, &board, &a, "first player second row", 0);
    assert_eq!(
        h.native("LeaderboardGetPlayerIndex", &[board.clone(), b.clone()])
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        h.native("LeaderboardGetPlayerIndex", &[board.clone(), a.clone()])
            .unwrap(),
        Value::Int(1)
    );
    h.native(
        "LeaderboardSetItemLabel",
        &[board.clone(), Value::Int(1), text("")],
    )
    .unwrap();
    h.native(
        "LeaderboardSetSizeByItemCount",
        &[board.clone(), Value::Int(7)],
    )
    .unwrap();
    h.native(
        "LeaderboardSetStyle",
        &[
            board.clone(),
            Value::Bool(false),
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(false),
        ],
    )
    .unwrap();
    h.native(
        "LeaderboardSetLabelColor",
        &[
            board.clone(),
            Value::Int(3),
            Value::Int(7),
            Value::Int(11),
            Value::Int(255),
        ],
    )
    .unwrap();
    h.native(
        "LeaderboardSetValueColor",
        &[
            board.clone(),
            Value::Int(29),
            Value::Int(31),
            Value::Int(37),
            Value::Int(0),
        ],
    )
    .unwrap();
    let state = h.leaderboard_state().unwrap();
    let record = state.get(id(&board)).unwrap();
    assert_eq!(record.items[0].value, -17);
    assert_eq!(record.items[1].label, "");
    assert_eq!(record.size_by_item_count, Some(7));
    assert_eq!(record.style, Some([false, true, false, false]));
    assert_eq!(record.label_color, Some([3, 7, 11, 255]));
    assert_eq!(record.value_color, Some([29, 31, 37, 0]));
    let revision = state.revision();
    assert_eq!(
        h.native("IsLeaderboardDisplayed", std::slice::from_ref(&board))
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        h.native("LeaderboardGetLabelText", std::slice::from_ref(&board))
            .unwrap(),
        text("title")
    );
    assert_eq!(h.leaderboard_state().unwrap().revision(), revision);
    h.native("PlayerSetLeaderboard", &[a.clone(), other.clone()])
        .unwrap();
    h.native("LeaderboardDisplay", &[board.clone(), Value::Bool(false)])
        .unwrap();
    assert_eq!(
        h.native("PlayerGetLeaderboard", std::slice::from_ref(&a))
            .unwrap(),
        other
    );
    assert_eq!(
        h.native("PlayerGetLeaderboard", std::slice::from_ref(&b))
            .unwrap(),
        board
    );
    h.native("LeaderboardRemovePlayerItem", &[board.clone(), b.clone()])
        .unwrap();
    assert_eq!(
        h.native("LeaderboardGetPlayerIndex", &[board.clone(), a.clone()])
            .unwrap(),
        Value::Int(0)
    );
    h.native("DestroyLeaderboard", std::slice::from_ref(&board))
        .unwrap();
    assert_eq!(
        h.native("PlayerGetLeaderboard", std::slice::from_ref(&b))
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        h.native("PlayerGetLeaderboard", std::slice::from_ref(&a))
            .unwrap(),
        other
    );
    assert!(h.inner().kind(id(&board)).is_err());
    let before = snapshot(&h);
    assert_eq!(
        h.native("LeaderboardDisplay", &[board.clone(), Value::Bool(true)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        h.native("DestroyLeaderboard", &[board]).unwrap(),
        Value::Null
    );
    assert_eq!(snapshot(&h), before);
    h.native("PlayerSetLeaderboard", &[a.clone(), Value::Null])
        .unwrap();
    assert_eq!(h.native("PlayerGetLeaderboard", &[a]).unwrap(), Value::Null);
    assert!(host().leaderboard_state().unwrap().boards().is_empty());
}

#[test]
fn leaderboard_invalid_arguments_and_resource_limits_preserve_all_accepted_state() {
    let mut h = host();
    let p = player(&mut h, 1);
    let other_player = player(&mut h, 2);
    let third_player = player(&mut h, 3);
    let board = create(&mut h);
    let other = create(&mut h);
    add(&mut h, &board, &p, "row", 0);
    h.native("PlayerSetLeaderboard", &[p.clone(), board.clone()])
        .unwrap();
    h.native(
        "PlayerSetLeaderboard",
        &[other_player.clone(), board.clone()],
    )
    .unwrap();
    let before = snapshot(&h);
    for (native, args) in [
        ("CreateLeaderboard", vec![]), // live-board bound
        ("CreateLeaderboard", vec![p.clone()]),
        ("LeaderboardDisplay", vec![p.clone(), Value::Bool(true)]),
        ("LeaderboardDisplay", vec![board.clone(), Value::Int(1)]),
        ("LeaderboardDisplay", vec![board.clone()]),
        ("IsLeaderboardDisplayed", vec![board.clone()]), // unqualified constructor state
        ("LeaderboardGetLabelText", vec![board.clone()]),
        (
            "LeaderboardSetItemLabel",
            vec![board.clone(), Value::Int(-1), text("new")],
        ),
        (
            "LeaderboardSetItemLabel",
            vec![board.clone(), Value::Int(1), text("new")],
        ),
        (
            "LeaderboardSetSizeByItemCount",
            vec![board.clone(), Value::Int(-1)],
        ),
        (
            "LeaderboardSetLabelColor",
            vec![
                board.clone(),
                Value::Int(0),
                Value::Int(256),
                Value::Int(0),
                Value::Int(255),
            ],
        ),
        (
            "LeaderboardSetValueColor",
            vec![
                board.clone(),
                Value::Int(0),
                Value::Int(0),
                Value::Int(-1),
                Value::Int(255),
            ],
        ),
        (
            "LeaderboardAddItem",
            vec![board.clone(), text("duplicate"), Value::Int(1), p.clone()],
        ),
        (
            "LeaderboardAddItem",
            vec![
                board.clone(),
                text("wrong family"),
                Value::Int(1),
                board.clone(),
            ],
        ),
        (
            "LeaderboardSetLabel",
            vec![board.clone(), text(&"x".repeat(65))],
        ),
        (
            "LeaderboardGetPlayerIndex",
            vec![board.clone(), other_player.clone()],
        ),
        (
            "LeaderboardRemovePlayerItem",
            vec![board.clone(), other_player.clone()],
        ),
        ("PlayerSetLeaderboard", vec![third_player, other]),
        ("LeaderboardClear", vec![board]), // outside the admitted API
    ] {
        let error = h.native(native, &args).unwrap_err();
        assert!(error.contains(native), "{error}");
        assert_eq!(snapshot(&h), before, "{native}");
    }
}

#[test]
fn leaderboard_remove_destroy_and_text_replacement_reclaim_budgets() {
    let mut limited = budget();
    limited.max_boards = 1;
    limited.max_items = 1;
    limited.text_bytes = 5;
    let mut h = try_host(DECLARATIONS, limited).unwrap();
    let p = player(&mut h, 1);
    let q = player(&mut h, 2);
    let board = create(&mut h);
    h.native("LeaderboardSetLabel", &[board.clone(), text("ab")])
        .unwrap();
    add(&mut h, &board, &p, "cde", 12);
    let before = snapshot(&h);
    assert!(
        h.native(
            "LeaderboardSetItemLabel",
            &[board.clone(), Value::Int(0), text("four")]
        )
        .is_err()
    );
    assert!(
        h.native(
            "LeaderboardAddItem",
            &[board.clone(), text(""), Value::Int(0), q.clone()]
        )
        .is_err()
    );
    assert_eq!(snapshot(&h), before);
    h.native("LeaderboardSetLabel", &[board.clone(), text("")])
        .unwrap();
    h.native(
        "LeaderboardSetItemLabel",
        &[board.clone(), Value::Int(0), text("12345")],
    )
    .unwrap();
    h.native("LeaderboardRemovePlayerItem", &[board.clone(), p])
        .unwrap();
    add(&mut h, &board, &q, "12345", 13);
    h.native("DestroyLeaderboard", &[board]).unwrap();
    let new_board = create(&mut h);
    add(&mut h, &new_board, &q, "12345", 14);
}

#[test]
fn leaderboard_abi_ancestry_overrides_and_duplicate_admission_refuse() {
    assert!(
        try_host("", budget())
            .unwrap()
            .leaderboard_state()
            .is_none()
    );
    for declarations in [
        DECLARATIONS.replace(
            "type leaderboard extends agent",
            "type leaderboard extends handle",
        ),
        DECLARATIONS.replace(
            "native CreateLeaderboard",
            "constant native CreateLeaderboard",
        ),
        DECLARATIONS.replace(
            "string label returns nothing",
            "integer label returns nothing",
        ),
        DECLARATIONS.replace("returns leaderboard", "returns integer"),
    ] {
        assert!(try_host(&declarations, budget()).is_err());
    }
    assert!(host().with_leaderboards(budget()).is_err());
    let ctx = context(
        301,
        &format!("{COMMON}{DECLARATIONS}"),
        "native CreateLeaderboard takes nothing returns leaderboard",
        "foreign:leaderboard",
    );
    assert!(from_context(ctx, budget()).is_err());
    assert!(
        matches!(try_host(DECLARATIONS, LeaderboardBudget { max_items: usize::MAX, ..budget() }), Err(e) if matches!(e.failure, Failure::Budget(_)))
    );
}

#[test]
fn leaderboard_compiled_authored_shape_keeps_localized_row_labels_in_canonical_state() {
    let map = r#"
    globals
        leaderboard LastBoard = null
        leaderboard Board = null
    endglobals
    function Resize takes leaderboard b returns nothing
        local integer size = LeaderboardGetItemCount(b)
        if LeaderboardGetLabelText(b) == "" then
            set size = size - 1
        endif
        call LeaderboardSetSizeByItemCount(b, size)
    endfunction
    function ReplaceRow takes player p, leaderboard b, string label returns nothing
        if LeaderboardHasPlayerItem(b, p) then
            call LeaderboardRemovePlayerItem(b, p)
        endif
        call LeaderboardAddItem(b, label, 0, p)
        call Resize(b)
    endfunction
    function main takes nothing returns nothing
        set LastBoard = CreateLeaderboard()
        call LeaderboardSetLabel(LastBoard, "TRIGSTR_1")
        call PlayerSetLeaderboard(Player(1), LastBoard)
        call LeaderboardDisplay(LastBoard, true)
        set Board = LastBoard
        call ReplaceRow(Player(1), Board, "TRIGSTR_2")
        call ReplaceRow(Player(1), Board, "TRIGSTR_3")
        call LeaderboardSetItemLabel(Board, LeaderboardGetPlayerIndex(Board, Player(1)), "TRIGSTR_4")
        call LeaderboardSetStyle(Board, false, true, false, false)
    endfunction
    "#;
    let ctx = context_with_wts(301, &format!("{COMMON}{DECLARATIONS}"), map, "synthetic:leaderboard-wts",
        Some(b"STRING 1\r\n{\r\nSynthetic title\r\n}\r\nSTRING 2\r\n{\r\nBefore\r\n}\r\nSTRING 3\r\n{\r\nReplacement\r\n}\r\nSTRING 4\r\n{\r\nAuthored count label\r\n}\r\n"));
    let mut vm = ctx
        .scripts
        .clone()
        .owned_vm(from_context(ctx.clone(), budget()).unwrap());
    vm.call("main", &[]).unwrap();
    let board = vm.global("Board");
    let h = &vm.host().inner;
    let state = h.leaderboard_state().unwrap().get(id(&board)).unwrap();
    assert_eq!(state.label.as_deref(), Some("Synthetic title"));
    assert_eq!(state.items.len(), 1);
    assert_eq!(state.items[0].label, "Authored count label");
    assert_eq!(state.items[0].value, 0);
    assert_eq!(state.size_by_item_count, Some(1));
}

#[test]
fn leaderboard_rows_preserve_color_and_style_history() {
    let mut h = host();
    let first = player(&mut h, 1);
    let later = player(&mut h, 2);
    let b = create(&mut h);
    add(&mut h, &b, &first, "first", 3);
    h.native(
        "LeaderboardSetStyle",
        &[
            b.clone(),
            Value::Bool(false),
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(true),
        ],
    )
    .unwrap();
    for (name, color) in [
        ("LeaderboardSetLabelColor", [13, 29, 57, 101]),
        ("LeaderboardSetValueColor", [7, 19, 37, 89]),
    ] {
        let mut args = vec![b.clone()];
        args.extend(color.map(Value::Int));
        h.native(name, &args).unwrap();
    }
    add(&mut h, &b, &later, "later", 4);
    let board = &h.leaderboard_state().unwrap().boards()[0];
    assert_eq!(board.items[0].style, [Some(true), Some(false), Some(true)]);
    assert_eq!(board.items[1].style, [Some(true), Some(false), None]);
    assert_eq!(board.items[0].label_color, Some([13, 29, 57, 101]));
    assert_eq!(board.items[0].value_color, Some([7, 19, 37, 89]));
    assert_eq!(board.items[1].label_color, None);
    assert_eq!(board.items[1].value_color, None);
    h.native(
        "LeaderboardSetItemValueColor",
        &[
            b.clone(),
            Value::Int(1),
            Value::Int(255),
            Value::Int(255),
            Value::Int(255),
            Value::Int(255),
        ],
    )
    .unwrap();
    let board = &h.leaderboard_state().unwrap().boards()[0];
    assert_eq!(board.items[0].value_color, Some([7, 19, 37, 89]));
    assert_eq!(board.value_color, Some([7, 19, 37, 89]));
    assert_eq!(board.items[1].value_color, Some([255, 255, 255, 255]));
    h.native(
        "LeaderboardSetItemLabelColor",
        &[
            b.clone(),
            Value::Int(1),
            Value::Int(9),
            Value::Int(8),
            Value::Int(7),
            Value::Int(6),
        ],
    )
    .unwrap();
    let board = &h.leaderboard_state().unwrap().boards()[0];
    assert_eq!(board.items[0].label_color, Some([13, 29, 57, 101]));
    assert_eq!(board.label_color, Some([13, 29, 57, 101]));
    assert_eq!(board.items[1].label_color, Some([9, 8, 7, 6]));
    assert_eq!(board.items[1].value_color, Some([255, 255, 255, 255]));
    h.native(
        "LeaderboardSetItemValue",
        &[b.clone(), Value::Int(0), Value::Int(11)],
    )
    .unwrap();
    let board = &h.leaderboard_state().unwrap().boards()[0];
    assert_eq!(board.items[0].value, 11);
    assert_eq!(board.items[1].value, 4);
    assert!(
        h.native(
            "LeaderboardSetItemValue",
            &[b.clone(), Value::Int(2), Value::Int(9)],
        )
        .is_err()
    );
    assert_eq!(
        h.leaderboard_state().unwrap().boards()[0].items[0].value,
        11
    );
    let kept = snapshot(&h);
    for args in [
        vec![
            b.clone(),
            Value::Int(2),
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
        ],
        vec![
            b.clone(),
            Value::Int(-1),
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
        ],
        vec![
            b.clone(),
            Value::Int(1),
            Value::Int(256),
            Value::Int(0),
            Value::Int(0),
            Value::Int(0),
        ],
    ] {
        assert!(h.native("LeaderboardSetItemValueColor", &args).is_err());
        assert_eq!(snapshot(&h), kept);
    }
    h.native(
        "LeaderboardSetStyle",
        &[
            b.clone(),
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(true),
            Value::Bool(false),
        ],
    )
    .unwrap();
    h.native(
        "LeaderboardSetLabelColor",
        &[
            b,
            Value::Int(31),
            Value::Int(61),
            Value::Int(127),
            Value::Int(251),
        ],
    )
    .unwrap();
    for item in &h.leaderboard_state().unwrap().boards()[0].items {
        assert_eq!(item.style, [Some(false), Some(true), Some(false)]);
        assert_eq!(item.label_color, Some([31, 61, 127, 251]));
    }
}

#[test]
fn item_label_color_packs_low_bytes_and_ignores_absent_boards_and_rows() {
    let mut h = host();
    let p = player(&mut h, 1);
    let b = create(&mut h);
    add(&mut h, &b, &p, "kept label", 37);
    let name = "LeaderboardSetItemLabelColor";
    for (channels, expected) in [
        ([-1, 256, 513, -258], [255, 0, 1, 254]),
        ([i32::MIN, i32::MAX, 0x12345678, 0], [0, 255, 120, 0]),
    ] {
        let before = h.leaderboard_state().unwrap().revision();
        let mut args = vec![b.clone(), Value::Int(0)];
        args.extend(channels.map(Value::Int));
        assert_eq!(h.native(name, &args).unwrap(), Value::Null);
        let state = h.leaderboard_state().unwrap();
        assert_eq!(state.revision(), before + 1);
        let board = state.get(id(&b)).unwrap();
        assert_eq!(board.items[0].label_color, Some(expected));
        assert_eq!(board.items[0].value_color, None);
        assert_eq!(board.items[0].label, "kept label");
        assert_eq!(board.items[0].value, 37);
        assert_eq!(board.label_color, None);
    }
    let args = |board: Value, row: i32| {
        vec![
            board,
            Value::Int(row),
            Value::Int(-1),
            Value::Int(256),
            Value::Int(513),
            Value::Int(-258),
        ]
    };
    let kept = snapshot(&h);
    for row in [i32::MIN, -1, 1, i32::MAX] {
        assert_eq!(h.native(name, &args(b.clone(), row)).unwrap(), Value::Null);
        assert_eq!(snapshot(&h), kept);
    }
    let unknown = Value::Handle(pcx_script::HandleId::new(
        std::num::NonZeroU64::new(u64::MAX).unwrap(),
    ));
    for invalid in [p, unknown] {
        assert!(h.native(name, &args(invalid, -1)).is_err());
        assert_eq!(snapshot(&h), kept);
    }
    h.native("DestroyLeaderboard", std::slice::from_ref(&b))
        .unwrap();
    let kept = snapshot(&h);
    for absent in [Value::Null, b] {
        assert_eq!(
            h.native(name, &args(absent.clone(), 0)).unwrap(),
            Value::Null
        );
        assert_eq!(snapshot(&h), kept);
        let mut malformed = args(absent.clone(), 0);
        malformed[2] = Value::Bool(true);
        assert!(h.native(name, &malformed).is_err());
        assert!(h.native(name, &[absent]).is_err());
        assert_eq!(snapshot(&h), kept);
    }
}

#[test]
fn leaderboard_display_and_destroy_resolve_only_null_and_typed_tombstones_as_absent() {
    let mut h = host();
    let p = player(&mut h, 1);
    let retired = create(&mut h);
    let other = create(&mut h);
    add(&mut h, &retired, &p, "retired row", 7);
    h.native("PlayerSetLeaderboard", &[p.clone(), retired.clone()])
        .unwrap();
    h.native("LeaderboardDisplay", &[retired.clone(), Value::Bool(true)])
        .unwrap();
    h.native("LeaderboardDisplay", &[other.clone(), Value::Bool(true)])
        .unwrap();
    h.native("DestroyLeaderboard", std::slice::from_ref(&retired))
        .unwrap();
    assert_eq!(h.leaderboard_state().unwrap().boards().len(), 1);
    assert_eq!(h.leaderboard_state().unwrap().player_board(id(&p)), None);
    assert!(h.leaderboard_state().unwrap().get(id(&retired)).is_none());
    let destroyed = snapshot(&h);
    for absent in [&retired, &Value::Null] {
        for shown in [false, true] {
            assert_eq!(
                h.native("LeaderboardDisplay", &[absent.clone(), Value::Bool(shown)])
                    .unwrap(),
                Value::Null
            );
            assert_eq!(snapshot(&h), destroyed);
        }
        assert_eq!(
            h.native("DestroyLeaderboard", std::slice::from_ref(absent))
                .unwrap(),
            Value::Null
        );
        assert_eq!(snapshot(&h), destroyed);
    }
    let unknown = Value::Handle(pcx_script::HandleId::new(
        std::num::NonZeroU64::new(u64::MAX).unwrap(),
    ));
    for invalid in [&p, &unknown] {
        assert!(
            h.native("LeaderboardDisplay", &[invalid.clone(), Value::Bool(false)])
                .is_err()
        );
        assert_eq!(snapshot(&h), destroyed);
        assert!(
            h.native("DestroyLeaderboard", std::slice::from_ref(invalid))
                .is_err()
        );
        assert_eq!(snapshot(&h), destroyed);
    }
    assert!(
        h.native("LeaderboardDisplay", &[retired.clone(), Value::Int(1)])
            .is_err()
    );
    assert!(
        h.native("LeaderboardDisplay", std::slice::from_ref(&retired))
            .is_err()
    );
    assert!(
        h.native("LeaderboardGetItemCount", std::slice::from_ref(&retired))
            .is_err()
    );
    assert!(
        h.native("LeaderboardSetLabel", &[retired, text("no resurrection")])
            .is_err()
    );
    assert_eq!(snapshot(&h), destroyed);
    assert_eq!(
        h.native("IsLeaderboardDisplayed", std::slice::from_ref(&other))
            .unwrap(),
        Value::Bool(true)
    );
    h.native("LeaderboardDisplay", &[other.clone(), Value::Bool(false)])
        .unwrap();
    assert_eq!(
        h.native("IsLeaderboardDisplayed", std::slice::from_ref(&other))
            .unwrap(),
        Value::Bool(false)
    );
    h.native("DestroyLeaderboard", std::slice::from_ref(&other))
        .unwrap();
    assert!(h.leaderboard_state().unwrap().boards().is_empty());
    // Destroy still reclaims canonical budgets; a later identity is distinct.
    let fresh = create(&mut h);
    assert_ne!(id(&fresh), id(&other));
    add(&mut h, &fresh, &p, "fresh row", 11);
}

#[test]
fn native_string_leaderboard_labels_resolve_at_each_native_boundary() {
    use std::cell::RefCell;
    let mut h = host();
    let b = create(&mut h);
    let p = player(&mut h, 0);
    let calls = RefCell::new(Vec::new());
    let source = |id| {
        calls.borrow_mut().push(id);
        Ok(Some("TRIGSTR_23 ^^x^^".into()))
    };
    assert!(
        h.native_with_strings(
            "LeaderboardSetLabel",
            &[Value::Null, text("TRIGSTR_7")],
            &source
        )
        .is_err()
    );
    assert!(calls.borrow().is_empty());
    h.native_with_strings(
        "LeaderboardSetLabel",
        &[b.clone(), text("TRIGSTR_7")],
        &source,
    )
    .unwrap();
    h.native_with_strings(
        "LeaderboardAddItem",
        &[b.clone(), text("TRIGSTR_8"), Value::Int(4), p],
        &source,
    )
    .unwrap();
    h.native_with_strings(
        "LeaderboardSetItemLabel",
        &[b.clone(), Value::Int(0), text("TRIGSTR_9")],
        &source,
    )
    .unwrap();
    assert_eq!(*calls.borrow(), [7, 8, 9]);
    assert_eq!(
        h.native("LeaderboardGetLabelText", &[b]).unwrap(),
        text("TRIGSTR_23 ^^x^^")
    );
}

#[test]
fn native_string_leaderboard_c_text_terminates_before_budgets_and_retention() {
    let mut h = host();
    let board = create(&mut h);
    h.native(
        "LeaderboardSetLabel",
        &[board.clone(), text(&format!("é\0{}", "x".repeat(100)))],
    )
    .unwrap();
    assert_eq!(
        h.native("LeaderboardGetLabelText", &[board]).unwrap(),
        text("é")
    );
}
