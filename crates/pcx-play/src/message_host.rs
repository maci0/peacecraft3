//! Local text messages owned by the published script session.
use super::{Error, Failure, ObjectHost};
use pcx_script::Value;

/// Retained live messages and their aggregate UTF-8 bytes, independent of game limits.
#[derive(Clone, Copy, Debug)]
pub struct MessageBudget {
    pub records: usize,
    pub text_bytes: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TimedMessage {
    pub sequence: u64,
    pub position: [f32; 2],
    pub text: String,
    pub started_at: f64,
    pub expires_at: f64,
}
impl TimedMessage {
    pub fn visible_at(&self, now: f64) -> bool {
        now.is_finite() && now >= self.started_at && now < self.expires_at
    }
}
pub struct MessageState {
    budget: MessageBudget,
    revision: u64,
    entries: Vec<TimedMessage>,
}
impl MessageState {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Expiration is sampled from game time; rendering never advances it.
    pub fn visible(&self, now: f64) -> impl DoubleEndedIterator<Item = &TimedMessage> {
        self.entries
            .iter()
            .filter(move |entry| entry.visible_at(now))
    }
}
const TIMED: &str = "DisplayTimedTextToPlayer";
const CLEAR: &str = "ClearTextMessages";
impl ObjectHost {
    /// Optional declarations must belong to the same source as the timer/player APIs.
    /// Automatic-duration text and sender-prefixed text remain unsupported.
    pub fn admit_messages(&mut self, budget: MessageBudget) -> Result<(), Error> {
        let fail = |why| self.inner.error(CLEAR, why);
        if self.messages.is_some() {
            return Err(fail(Failure::Message("already admitted")));
        }
        std::alloc::Layout::array::<TimedMessage>(budget.records)
            .map_err(|_| fail(Failure::Capacity))?;
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        let mut found = false;
        for (name, params) in [
            (CLEAR, &[][..]),
            (TIMED, &["player", "real", "real", "real", "string"][..]),
        ] {
            if self.inner.program.natives().get(name).is_none()
                && self.inner.program.natives().ambiguous(name).is_none()
            {
                continue;
            }
            Self::contract(&self.inner, name, params, "nothing", false, None)?;
            let native = self
                .inner
                .program
                .natives()
                .get(name)
                .expect("checked declaration");
            if &native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
            if name == TIMED {
                Self::contract(
                    &self.inner,
                    "Player",
                    &["integer"],
                    "player",
                    true,
                    Some("player"),
                )?;
                if &self
                    .inner
                    .program
                    .natives()
                    .get("Player")
                    .expect("checked player")
                    .source
                    != source
                {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
            found = true;
        }
        if found {
            self.messages = Some(MessageState {
                budget,
                revision: 0,
                entries: Vec::new(),
            });
        }
        Ok(())
    }
    pub fn messages(&self) -> Option<&MessageState> {
        self.messages.as_ref()
    }
    pub(super) fn message_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.messages.as_ref()?;
        if !matches!(name, TIMED | CLEAR) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some(self.message_call(name, args))
    }
    fn message_call(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |why| self.inner.error(name, Failure::Message(why));
        let state = self.messages.as_ref().expect("admitted messages");
        let revision = state
            .revision
            .checked_add(1)
            .ok_or_else(|| fail("sequence exhausted"))?;
        if name == CLEAR {
            if !args.is_empty() {
                return Err(fail("expected no arguments"));
            }
            let state = self.messages.as_mut().expect("admitted messages");
            state.entries.clear();
            state.revision = revision;
            return Ok(Value::Null);
        }
        let [
            Value::Handle(player),
            Value::Real(x),
            Value::Real(y),
            Value::Real(duration),
            Value::Str(text),
        ] = args
        else {
            return Err(fail("arguments do not match the declaration"));
        };
        if !x.is_finite()
            || !y.is_finite()
            || !duration.is_finite()
            || *duration < 0.0
            || text.contains('\0')
        {
            return Err(fail("invalid position, duration or text"));
        }
        let player = self
            .player_settings(*player)
            .map_err(|error| self.inner.error(name, error.failure))?
            .number;
        let local = self
            .local_player_number()
            .ok_or_else(|| fail("local client is not admitted"))?;
        if player != local {
            return Ok(Value::Null);
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| fail("logical clock is not admitted"))?;
        let expires_at = now + f64::from(*duration);
        if !expires_at.is_finite() {
            return Err(fail("unrepresentable deadline"));
        }
        let mut count = 0usize;
        let mut bytes = text.len();
        for entry in state.visible(now) {
            count += 1;
            bytes = bytes
                .checked_add(entry.text.len())
                .ok_or_else(|| fail("text budget exceeded"))?;
        }
        if bytes > state.budget.text_bytes || count >= state.budget.records {
            return Err(fail("live message budget exceeded"));
        }
        let mut copy = String::new();
        copy.try_reserve_exact(text.len())
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        copy.push_str(text);
        let state = self.messages.as_mut().expect("admitted messages");
        state
            .entries
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        state.entries.retain(|entry| entry.visible_at(now));
        state.entries.push(TimedMessage {
            sequence: revision,
            position: [x.max(0.0), y.max(0.0)],
            text: copy,
            started_at: now,
            expires_at,
        });
        state.revision = revision;
        Ok(Value::Null)
    }
}
