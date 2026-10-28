//! Timer dialogs (`CreateTimerDialog` and the `TimerDialog*` natives): identity and retained
//! state only. Nothing draws the dialog; a presentation consumer would read the title, colors,
//! speed, display flag and the timer it watches from [`TimerDialog`].
//!
//! A dialog watches one live timer. Not established, so refused: a null timer, a destroyed or
//! non-timer handle, and any use of a destroyed dialog (the identity is gone). The color and
//! speed values are stored as executed with no clamping.
use super::{Entry, Error, Failure, ObjectHost, below};
use pcx_script::{HandleId, Value};

const CREATE: &str = "CreateTimerDialog";
const DESTROY: &str = "DestroyTimerDialog";
const FAMILY: &str = "timerdialog";

const NATIVES: [(&str, &[&str], &str); 9] = [
    (CREATE, &["timer"], FAMILY),
    (DESTROY, &[FAMILY], "nothing"),
    ("TimerDialogSetTitle", &[FAMILY, "string"], "nothing"),
    (
        "TimerDialogSetTitleColor",
        &[FAMILY, "integer", "integer", "integer", "integer"],
        "nothing",
    ),
    (
        "TimerDialogSetTimeColor",
        &[FAMILY, "integer", "integer", "integer", "integer"],
        "nothing",
    ),
    ("TimerDialogSetSpeed", &[FAMILY, "real"], "nothing"),
    ("TimerDialogDisplay", &[FAMILY, "boolean"], "nothing"),
    ("IsTimerDialogDisplayed", &[FAMILY], "boolean"),
    (
        "TimerDialogSetRealTimeRemaining",
        &[FAMILY, "real"],
        "nothing",
    ),
];

/// A live dialog as executed. `None` fields were never set.
#[derive(Debug, Clone, PartialEq)]
pub struct TimerDialog {
    pub timer: HandleId,
    pub title: Option<String>,
    pub title_color: Option<[i32; 4]>,
    pub time_color: Option<[i32; 4]>,
    pub speed: Option<f32>,
    pub displayed: bool,
    pub real_time_remaining: Option<f32>,
}

impl ObjectHost {
    /// Requires the timer arena's declarations; every declared native must share `CreateTimer`'s
    /// source. Natives the program does not declare stay unadmitted.
    pub fn with_timer_dialogs(mut self) -> Result<Self, Error> {
        let source = self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .map(|n| n.source.clone())
            .ok_or_else(|| self.inner.error(CREATE, Failure::Signature))?;
        if self.timer_dialogs || !below(&self.inner.program, FAMILY, "agent") {
            return Err(self.inner.error(CREATE, Failure::Signature));
        }
        for (name, args, result) in NATIVES {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, result, false, Some(FAMILY))?;
            if native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.timer_dialogs = true;
        Ok(self)
    }

    /// Live dialogs in creation order.
    pub fn timer_dialogs(&self) -> impl Iterator<Item = (HandleId, &TimerDialog)> {
        self.inner
            .arena
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| match entry {
                Entry::TimerDialog(Some(dialog)) => Some((
                    self.inner.id(i, CREATE).expect("allocated identity"),
                    dialog,
                )),
                _ => None,
            })
    }

    pub(super) fn timer_dialog_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !self.timer_dialogs || !NATIVES.iter().any(|(native, ..)| *native == name) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some(self.apply_timer_dialog(name, args))
    }

    fn apply_timer_dialog(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        if name == CREATE {
            let [Value::Handle(timer)] = args else {
                return Err(self.inner.error(
                    name,
                    Failure::UnitState("a null timer's behavior is not established"),
                ));
            };
            if !matches!(self.inner.entry(*timer)?, Entry::Timer { live: true, .. }) {
                return Err(self.inner.error(name, Failure::WrongFamily));
            }
            let dialog = TimerDialog {
                timer: *timer,
                title: None,
                title_color: None,
                time_color: None,
                speed: None,
                displayed: false,
                real_time_remaining: None,
            };
            return self
                .inner
                .allocate(Entry::TimerDialog(Some(dialog)), name)
                .map(Value::Handle);
        }
        let Some(Value::Handle(id)) = args.first() else {
            return Err(self.inner.error(
                name,
                Failure::UnitState("a null dialog's behavior is not established"),
            ));
        };
        let Entry::TimerDialog(slot) = self.inner.entry_mut(*id)? else {
            return Err(self.inner.error(name, Failure::WrongFamily));
        };
        if name == DESTROY {
            return match slot.take() {
                Some(_) => Ok(Value::Null),
                None => Err(self.inner.error(name, Failure::Identity)),
            };
        }
        let Some(dialog) = slot.as_mut() else {
            return Err(self.inner.error(name, Failure::Identity));
        };
        let color = |tail: &[Value]| -> Option<[i32; 4]> {
            let [Value::Int(r), Value::Int(g), Value::Int(b), Value::Int(a)] = tail else {
                return None;
            };
            Some([*r, *g, *b, *a])
        };
        match (name, &args[1..]) {
            ("TimerDialogSetTitle", [Value::Str(title)]) => dialog.title = Some(title.clone()),
            ("TimerDialogSetTitleColor", tail) if color(tail).is_some() => {
                dialog.title_color = color(tail)
            }
            ("TimerDialogSetTimeColor", tail) if color(tail).is_some() => {
                dialog.time_color = color(tail)
            }
            ("TimerDialogSetSpeed", [speed]) if speed.as_real().is_some_and(f32::is_finite) => {
                dialog.speed = speed.as_real()
            }
            ("TimerDialogDisplay", [Value::Bool(display)]) => dialog.displayed = *display,
            ("IsTimerDialogDisplayed", []) => return Ok(Value::Bool(dialog.displayed)),
            ("TimerDialogSetRealTimeRemaining", [time])
                if time.as_real().is_some_and(f32::is_finite) =>
            {
                dialog.real_time_remaining = time.as_real()
            }
            _ => return Err(self.inner.error(name, Failure::Signature)),
        }
        Ok(Value::Null)
    }
}
