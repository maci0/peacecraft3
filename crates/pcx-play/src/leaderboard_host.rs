//! Script leaderboards share the session arena and player identities; no UI geometry lives here.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

#[derive(Clone, Copy, Debug)]
pub struct LeaderboardBudget {
    pub max_boards: usize,
    pub max_items: usize,
    pub max_assignments: usize,
    pub text_bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaderboardItem {
    pub player: HandleId,
    pub label: String,
    pub value: i32,
    /// Effective name/value/icon flags; creation defaults await admitted native rules.
    pub style: [Option<bool>; 3],
    pub label_color: Option<[u8; 4]>,
    pub value_color: Option<[u8; 4]>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptLeaderboard {
    pub id: HandleId,
    /// Unset constructor presentation stays unqualified until explicitly assigned.
    pub label: Option<String>,
    pub displayed: Option<bool>,
    pub size_by_item_count: Option<i32>,
    /// showLabel, showNames, showValues, showIcons, in native argument order.
    pub style: Option<[bool; 4]>,
    pub label_color: Option<[u8; 4]>,
    pub value_color: Option<[u8; 4]>,
    pub items: Vec<LeaderboardItem>,
}
#[derive(Debug)]
pub struct LeaderboardState {
    budget: LeaderboardBudget,
    boards: Vec<ScriptLeaderboard>,
    assignments: Vec<(HandleId, HandleId)>,
    items: usize,
    text_bytes: usize,
    revision: u64,
}
impl LeaderboardState {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn boards(&self) -> &[ScriptLeaderboard] {
        &self.boards
    }
    pub fn get(&self, id: HandleId) -> Option<&ScriptLeaderboard> {
        self.boards.iter().find(|board| board.id == id)
    }
    pub fn player_board(&self, player: HandleId) -> Option<HandleId> {
        self.assignments
            .iter()
            .find_map(|&(p, board)| (p == player).then_some(board))
    }
}
const CREATE: &str = "CreateLeaderboard";
const NATIVES: &[(&str, &[&str], &str)] = &[
    (CREATE, &[], "leaderboard"),
    ("DestroyLeaderboard", &["leaderboard"], "nothing"),
    ("LeaderboardDisplay", &["leaderboard", "boolean"], "nothing"),
    ("IsLeaderboardDisplayed", &["leaderboard"], "boolean"),
    ("LeaderboardGetItemCount", &["leaderboard"], "integer"),
    (
        "LeaderboardSetSizeByItemCount",
        &["leaderboard", "integer"],
        "nothing",
    ),
    (
        "LeaderboardAddItem",
        &["leaderboard", "string", "integer", "player"],
        "nothing",
    ),
    (
        "LeaderboardRemovePlayerItem",
        &["leaderboard", "player"],
        "nothing",
    ),
    (
        "LeaderboardHasPlayerItem",
        &["leaderboard", "player"],
        "boolean",
    ),
    (
        "LeaderboardGetPlayerIndex",
        &["leaderboard", "player"],
        "integer",
    ),
    ("LeaderboardSetLabel", &["leaderboard", "string"], "nothing"),
    ("LeaderboardGetLabelText", &["leaderboard"], "string"),
    (
        "PlayerSetLeaderboard",
        &["player", "leaderboard"],
        "nothing",
    ),
    ("PlayerGetLeaderboard", &["player"], "leaderboard"),
    (
        "LeaderboardSetLabelColor",
        &["leaderboard", "integer", "integer", "integer", "integer"],
        "nothing",
    ),
    (
        "LeaderboardSetValueColor",
        &["leaderboard", "integer", "integer", "integer", "integer"],
        "nothing",
    ),
    (
        "LeaderboardSetStyle",
        &["leaderboard", "boolean", "boolean", "boolean", "boolean"],
        "nothing",
    ),
    (
        "LeaderboardSetItemLabel",
        &["leaderboard", "integer", "string"],
        "nothing",
    ),
];
impl ObjectHost {
    pub fn with_leaderboards(mut self, budget: LeaderboardBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(CREATE, reason);
        if self.leaderboards.is_some() {
            return Err(fail(Failure::LeaderboardState("already admitted")));
        }
        if !NATIVES.iter().any(|(name, ..)| {
            self.inner.program.natives().get(name).is_some()
                || self.inner.program.natives().ambiguous(name).is_some()
        }) {
            return Ok(self);
        }
        if self.players.is_none() {
            return Err(fail(Failure::LeaderboardState("requires admitted players")));
        }
        if budget.max_items > i32::MAX as usize {
            return Err(fail(Failure::Budget("leaderboard integer item count")));
        }
        std::alloc::Layout::array::<ScriptLeaderboard>(budget.max_boards)
            .and_then(|_| std::alloc::Layout::array::<LeaderboardItem>(budget.max_items))
            .and_then(|_| std::alloc::Layout::array::<(HandleId, HandleId)>(budget.max_assignments))
            .map_err(|_| fail(Failure::Capacity))?;
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        for (name, args, result) in NATIVES {
            if self.inner.program.natives().get(name).is_none()
                && self.inner.program.natives().ambiguous(name).is_none()
            {
                continue;
            }
            Self::contract(&self.inner, name, args, result, false, Some("leaderboard"))?;
            if self
                .inner
                .program
                .natives()
                .get(name)
                .is_none_or(|n| &n.source != source)
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        for family in ["leaderboard", "player"] {
            if !super::below(&self.inner.program, family, "agent") {
                return Err(fail(Failure::Signature));
            }
            let mut ancestor = family;
            for _ in 0..=self.inner.program.types().count() {
                if ancestor == "handle" {
                    break;
                }
                let ty = self
                    .inner
                    .program
                    .types()
                    .find(|t| t.name == ancestor)
                    .ok_or_else(|| fail(Failure::Signature))?;
                if &ty.source != source {
                    return Err(fail(Failure::Source));
                }
                ancestor = &ty.parent;
            }
            if ancestor != "handle" {
                return Err(fail(Failure::Signature));
            }
        }
        self.leaderboards = Some(LeaderboardState {
            budget,
            boards: Vec::new(),
            assignments: Vec::new(),
            items: 0,
            text_bytes: 0,
            revision: 0,
        });
        Ok(self)
    }
    pub fn leaderboard_state(&self) -> Option<&LeaderboardState> {
        self.leaderboards.as_ref()
    }

    fn leaderboard_index(&self, name: &str, id: HandleId) -> Result<usize, Error> {
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(name, e.failure))?
        {
            Entry::Leaderboard { live: true } => {}
            Entry::Leaderboard { live: false } => {
                return Err(self.inner.error(name, Failure::Identity));
            }
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        }
        self.leaderboards
            .as_ref()
            .expect("admitted")
            .boards
            .iter()
            .position(|b| b.id == id)
            .ok_or_else(|| self.inner.error(name, Failure::Identity))
    }
    fn leaderboard_text(
        &self,
        name: &str,
        text: &str,
        old: usize,
    ) -> Result<(String, usize), Error> {
        let state = self.leaderboards.as_ref().expect("admitted");
        let total = state
            .text_bytes
            .checked_sub(old)
            .and_then(|v| v.checked_add(text.len()))
            .filter(|&n| n <= state.budget.text_bytes)
            .ok_or_else(|| {
                self.inner
                    .error(name, Failure::Budget("leaderboard text bytes"))
            })?;
        if text.contains('\0') {
            return Err(self
                .inner
                .error(name, Failure::LeaderboardState("text contains NUL")));
        }
        let mut value = String::new();
        value
            .try_reserve_exact(text.len())
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        value.push_str(text);
        Ok((value, total))
    }
    pub(super) fn leaderboard_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.leaderboards.as_ref()?;
        let (_, parameters, result) = NATIVES.iter().find(|(n, ..)| *n == name)?;
        self.inner.program.natives().get(name)?;
        Some((|| {
            if args.len() != parameters.len()
                || args.iter().zip(*parameters).any(|(v, ty)| {
                    !matches!(
                        (*ty, v),
                        ("leaderboard" | "player", Value::Handle(_) | Value::Null)
                            | ("string", Value::Str(_))
                            | ("integer", Value::Int(_))
                            | ("boolean", Value::Bool(_))
                    )
                })
            {
                return Err(self.inner.error(name, Failure::Signature));
            }
            // These native wrappers return before touching presentation when
            // typed resolution finds no live board. Keep unknown identities strict.
            if matches!(name, "LeaderboardDisplay" | "DestroyLeaderboard") {
                match args[0] {
                    Value::Null => return Ok(Value::Null),
                    Value::Handle(id) => match self
                        .inner
                        .entry(id)
                        .map_err(|e| self.inner.error(name, e.failure))?
                    {
                        Entry::Leaderboard { live: false } => return Ok(Value::Null),
                        Entry::Leaderboard { live: true } => {}
                        _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                    },
                    _ => unreachable!("validated leaderboard signature"),
                }
            }
            let mutation = *result == "nothing" || name == CREATE;
            let revision = self
                .leaderboards
                .as_ref()
                .expect("admitted")
                .revision
                .checked_add(u64::from(mutation))
                .ok_or_else(|| {
                    self.inner
                        .error(name, Failure::Budget("leaderboard revision"))
                })?;
            let value = self.execute_leaderboard(name, args)?;
            self.leaderboards.as_mut().expect("admitted").revision = revision;
            Ok(value)
        })())
    }
    fn execute_leaderboard(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(name, reason);
        if name == CREATE {
            let state = self.leaderboards.as_mut().expect("admitted");
            if state.boards.len() >= state.budget.max_boards {
                return Err(fail(Failure::Budget("live leaderboards")));
            }
            state
                .boards
                .try_reserve(1)
                .map_err(|_| fail(Failure::Capacity))?;
            let id = self
                .inner
                .allocate(Entry::Leaderboard { live: true }, name)?;
            self.leaderboards
                .as_mut()
                .expect("admitted")
                .boards
                .push(ScriptLeaderboard {
                    id,
                    label: None,
                    displayed: None,
                    size_by_item_count: None,
                    style: None,
                    label_color: None,
                    value_color: None,
                    items: Vec::new(),
                });
            return Ok(Value::Handle(id));
        }
        if name == "PlayerGetLeaderboard" || name == "PlayerSetLeaderboard" {
            let Value::Handle(player) = args[0] else {
                return Err(fail(Failure::Identity));
            };
            self.player_settings(player).map_err(|e| fail(e.failure))?;
            if name == "PlayerGetLeaderboard" {
                return Ok(self
                    .leaderboards
                    .as_ref()
                    .expect("admitted")
                    .player_board(player)
                    .map_or(Value::Null, Value::Handle));
            }
            let board = match args[1] {
                Value::Null => None,
                Value::Handle(id) => {
                    self.leaderboard_index(name, id)?;
                    Some(id)
                }
                _ => unreachable!("signature"),
            };
            let state = self.leaderboards.as_mut().expect("admitted");
            let existing = state.assignments.iter().position(|&(p, _)| p == player);
            match (existing, board) {
                (Some(index), Some(id)) => state.assignments[index].1 = id,
                (Some(index), None) => {
                    state.assignments.remove(index);
                }
                (None, Some(id)) => {
                    if state.assignments.len() >= state.budget.max_assignments {
                        return Err(fail(Failure::Budget("leaderboard player assignments")));
                    }
                    state
                        .assignments
                        .try_reserve(1)
                        .map_err(|_| fail(Failure::Capacity))?;
                    state.assignments.push((player, id));
                }
                (None, None) => {}
            }
            return Ok(Value::Null);
        }
        let Value::Handle(id) = args[0] else {
            return Err(fail(Failure::Identity));
        };
        let index = self.leaderboard_index(name, id)?;
        let state = self.leaderboards.as_ref().expect("admitted");
        let board = &state.boards[index];
        match (name, args) {
            ("IsLeaderboardDisplayed", _) => {
                return board.displayed.map(Value::Bool).ok_or_else(|| {
                    fail(Failure::LeaderboardState(
                        "constructor display is unqualified",
                    ))
                });
            }
            ("LeaderboardGetItemCount", _) => {
                return Ok(Value::Int(
                    i32::try_from(board.items.len()).expect("budget"),
                ));
            }
            ("LeaderboardGetLabelText", _) => {
                return board
                    .label
                    .as_ref()
                    .map(|s| Value::Str(s.clone()))
                    .ok_or_else(|| {
                        fail(Failure::LeaderboardState(
                            "constructor label is unqualified",
                        ))
                    });
            }
            (
                "LeaderboardHasPlayerItem"
                | "LeaderboardGetPlayerIndex"
                | "LeaderboardRemovePlayerItem",
                [_, Value::Handle(player)],
            ) => {
                self.player_settings(*player).map_err(|e| fail(e.failure))?;
                let row = board.items.iter().position(|item| item.player == *player);
                if name == "LeaderboardHasPlayerItem" {
                    return Ok(Value::Bool(row.is_some()));
                }
                let row = row.ok_or_else(|| {
                    fail(Failure::LeaderboardState("player has no leaderboard row"))
                })?;
                if name == "LeaderboardGetPlayerIndex" {
                    return Ok(Value::Int(i32::try_from(row).expect("budget")));
                }
                let state = self.leaderboards.as_mut().expect("admitted");
                let removed = state.boards[index].items.remove(row);
                state.text_bytes -= removed.label.len();
                state.items -= 1;
            }
            (
                "LeaderboardAddItem",
                [
                    _,
                    Value::Str(label),
                    Value::Int(value),
                    Value::Handle(player),
                ],
            ) => {
                self.player_settings(*player).map_err(|e| fail(e.failure))?;
                if board.items.iter().any(|item| item.player == *player) {
                    return Err(fail(Failure::LeaderboardState(
                        "duplicate player row requires removal",
                    )));
                }
                if state.items >= state.budget.max_items {
                    return Err(fail(Failure::Budget("leaderboard items")));
                }
                let (label, total) = self.leaderboard_text(name, label, 0)?;
                let state = self.leaderboards.as_mut().expect("admitted");
                state.boards[index]
                    .items
                    .try_reserve(1)
                    .map_err(|_| fail(Failure::Capacity))?;
                let style = state.boards[index].style;
                state.boards[index].items.push(LeaderboardItem {
                    player: *player,
                    label,
                    value: *value,
                    style: [style.map(|s| s[1]), style.map(|s| s[2]), None],
                    label_color: None,
                    value_color: None,
                });
                state.text_bytes = total;
                state.items += 1;
            }
            ("LeaderboardSetLabel", [_, Value::Str(label)]) => {
                let old = board.label.as_ref().map_or(0, String::len);
                let (label, total) = self.leaderboard_text(name, label, old)?;
                let state = self.leaderboards.as_mut().expect("admitted");
                state.boards[index].label = Some(label);
                state.text_bytes = total;
            }
            ("LeaderboardSetItemLabel", [_, Value::Int(row), Value::Str(label)]) => {
                let row = usize::try_from(*row)
                    .ok()
                    .filter(|&r| r < board.items.len())
                    .ok_or_else(|| fail(Failure::LeaderboardState("row index out of range")))?;
                let (label, total) =
                    self.leaderboard_text(name, label, board.items[row].label.len())?;
                let state = self.leaderboards.as_mut().expect("admitted");
                state.boards[index].items[row].label = label;
                state.text_bytes = total;
            }
            ("LeaderboardSetSizeByItemCount", [_, Value::Int(count)]) => {
                if *count < 0 {
                    return Err(fail(Failure::LeaderboardState(
                        "negative display item count",
                    )));
                }
                self.leaderboards.as_mut().expect("admitted").boards[index].size_by_item_count =
                    Some(*count);
            }
            ("LeaderboardDisplay", [_, Value::Bool(displayed)]) => {
                self.leaderboards.as_mut().expect("admitted").boards[index].displayed =
                    Some(*displayed);
            }
            (
                "LeaderboardSetStyle",
                [
                    _,
                    Value::Bool(a),
                    Value::Bool(b),
                    Value::Bool(c),
                    Value::Bool(d),
                ],
            ) => {
                let board = &mut self.leaderboards.as_mut().expect("admitted").boards[index];
                board.style = Some([*a, *b, *c, *d]);
                for item in &mut board.items {
                    item.style = [Some(*b), Some(*c), Some(*d)];
                }
            }
            (
                "LeaderboardSetLabelColor" | "LeaderboardSetValueColor",
                [
                    _,
                    Value::Int(r),
                    Value::Int(g),
                    Value::Int(b),
                    Value::Int(a),
                ],
            ) => {
                let color = [*r, *g, *b, *a].map(u8::try_from);
                let [Ok(r), Ok(g), Ok(b), Ok(a)] = color else {
                    return Err(fail(Failure::LeaderboardState(
                        "RGBA channels must fit bytes",
                    )));
                };
                let board = &mut self.leaderboards.as_mut().expect("admitted").boards[index];
                if name == "LeaderboardSetLabelColor" {
                    board.label_color = Some([r, g, b, a]);
                    for item in &mut board.items {
                        item.label_color = Some([r, g, b, a]);
                    }
                } else {
                    board.value_color = Some([r, g, b, a]);
                    for item in &mut board.items {
                        item.value_color = Some([r, g, b, a]);
                    }
                }
            }
            ("DestroyLeaderboard", _) => {
                let text = board.label.as_ref().map_or(0, String::len)
                    + board.items.iter().map(|i| i.label.len()).sum::<usize>();
                let items = board.items.len();
                *self.inner.entry_mut(id)? = Entry::Leaderboard { live: false };
                let state = self.leaderboards.as_mut().expect("admitted");
                state.boards.remove(index);
                state.assignments.retain(|&(_, b)| b != id);
                state.items -= items;
                state.text_bytes -= text;
            }
            _ => return Err(fail(Failure::Identity)),
        }
        Ok(Value::Null)
    }
}
