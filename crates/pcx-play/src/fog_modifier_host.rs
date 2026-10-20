//! Executed fog modifiers for a fog-of-war consumer: player, fog state,
//! circle, flags and whether started. No visibility is simulated here.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FogModifier {
    pub player: HandleId,
    pub state: HandleId,
    pub center: [f32; 2],
    pub radius: f32,
    pub shared_vision: bool,
    pub after_units: bool,
    pub started: bool,
}

const CREATE: &str = "CreateFogModifierRadiusLoc";
const NATIVES: [(&str, &[&str], &str); 4] = [
    (
        CREATE,
        &[
            "player", "fogstate", "location", "real", "boolean", "boolean",
        ],
        "fogmodifier",
    ),
    ("FogModifierStart", &["fogmodifier"], "nothing"),
    ("FogModifierStop", &["fogmodifier"], "nothing"),
    ("DestroyFogModifier", &["fogmodifier"], "nothing"),
];

impl ObjectHost {
    pub fn with_fog_modifiers(mut self) -> Result<Self, Error> {
        for (name, args, result) in NATIVES {
            if self.inner.program.natives().get(name).is_some() {
                Self::contract(&self.inner, name, args, result, false, Some("fogmodifier"))?;
            }
        }
        self.fog_modifiers = true;
        Ok(self)
    }

    /// Live modifiers in creation order.
    pub fn fog_modifiers(&self) -> impl Iterator<Item = &FogModifier> {
        self.inner.arena.iter().filter_map(|e| match e {
            Entry::FogModifier(Some(m)) => Some(m),
            _ => None,
        })
    }

    pub(super) fn fog_modifier_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !self.fog_modifiers || !NATIVES.iter().any(|&(n, ..)| n == name) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some((|| {
            if name == CREATE {
                let [
                    Value::Handle(player),
                    Value::Handle(state),
                    location,
                    radius,
                    Value::Bool(shared_vision),
                    Value::Bool(after_units),
                ] = args
                else {
                    return Err(self.inner.error(name, Failure::Signature));
                };
                let center = self.location_point(location, name)?;
                let radius = radius
                    .as_real()
                    .filter(|r| r.is_finite())
                    .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
                let modifier = FogModifier {
                    player: *player,
                    state: *state,
                    center,
                    radius,
                    shared_vision: *shared_vision,
                    after_units: *after_units,
                    started: false,
                };
                return self
                    .inner
                    .allocate(Entry::FogModifier(Some(modifier)), name)
                    .map(Value::Handle);
            }
            let id = match args {
                [Value::Null] => return Ok(Value::Null),
                [Value::Handle(id)] => *id,
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            let Entry::FogModifier(slot) = self.inner.entry_mut(id)? else {
                return Err(self.inner.error(name, Failure::WrongFamily));
            };
            match (name, slot.as_mut()) {
                ("DestroyFogModifier", _) => *slot = None,
                ("FogModifierStart", Some(m)) => m.started = true,
                ("FogModifierStop", Some(m)) => m.started = false,
                _ => {}
            }
            Ok(Value::Null)
        })())
    }
}
