//! Executed fog modifiers for a fog-of-war consumer: player, fog state,
//! geometry, flags and whether started. The vision consumer reads this same state.
use super::{Entry, Error, Failure, ObjectHost, VisionShape};
use pcx_script::{HandleId, Value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FogModifier {
    pub player: HandleId,
    pub state: HandleId,
    pub shape: VisionShape,
    pub shared_vision: bool,
    pub after_units: bool,
    pub started: bool,
}

const CREATE: &str = "CreateFogModifierRadiusLoc";
const NATIVES: [(&str, &[&str], &str); 6] = [
    (
        CREATE,
        &[
            "player", "fogstate", "location", "real", "boolean", "boolean",
        ],
        "fogmodifier",
    ),
    (
        "CreateFogModifierRadius",
        &[
            "player", "fogstate", "real", "real", "real", "boolean", "boolean",
        ],
        "fogmodifier",
    ),
    (
        "CreateFogModifierRect",
        &["player", "fogstate", "rect", "boolean", "boolean"],
        "fogmodifier",
    ),
    ("FogModifierStart", &["fogmodifier"], "nothing"),
    ("FogModifierStop", &["fogmodifier"], "nothing"),
    ("DestroyFogModifier", &["fogmodifier"], "nothing"),
];

impl ObjectHost {
    pub fn with_fog_modifiers(mut self) -> Result<Self, Error> {
        if self.fog_modifiers {
            return Err(self
                .inner
                .error(CREATE, Failure::FogModifier("already admitted".into())));
        }
        let mut source = None;
        for (name, args, result) in NATIVES {
            if let Some(native) = self.inner.program.natives().get(name) {
                if source
                    .replace(&native.source)
                    .is_some_and(|previous| previous != &native.source)
                {
                    return Err(self.inner.error(name, Failure::Source));
                }
                Self::contract(&self.inner, name, args, result, false, Some("fogmodifier"))?;
            }
        }
        self.fog_modifiers = true;
        Ok(self)
    }

    /// Live modifiers in creation order.
    pub fn fog_modifiers(&self) -> impl Iterator<Item = &FogModifier> {
        self.inner
            .fog_slots
            .iter()
            .filter_map(|&i| match &self.inner.arena[i] {
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
            if matches!(
                name,
                CREATE | "CreateFogModifierRadius" | "CreateFogModifierRect"
            ) {
                let (player, state, shape, shared_vision, after_units) = match (name, args) {
                    (
                        CREATE,
                        [
                            Value::Handle(player),
                            Value::Handle(state),
                            location,
                            radius,
                            Value::Bool(shared),
                            Value::Bool(after),
                        ],
                    ) => {
                        let center = self.location_point(location, name)?;
                        let radius = radius
                            .as_real()
                            .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
                        (
                            *player,
                            *state,
                            VisionShape::Circle { center, radius },
                            *shared,
                            *after,
                        )
                    }
                    (
                        "CreateFogModifierRadius",
                        [
                            Value::Handle(player),
                            Value::Handle(state),
                            x,
                            y,
                            radius,
                            Value::Bool(shared),
                            Value::Bool(after),
                        ],
                    ) => {
                        let [Some(x), Some(y), Some(radius)] = [x, y, radius].map(|v| v.as_real())
                        else {
                            return Err(self.inner.error(name, Failure::Signature));
                        };
                        (
                            *player,
                            *state,
                            VisionShape::Circle {
                                center: [x, y],
                                radius,
                            },
                            *shared,
                            *after,
                        )
                    }
                    (
                        "CreateFogModifierRect",
                        [
                            Value::Handle(player),
                            Value::Handle(state),
                            Value::Handle(rect),
                            Value::Bool(shared),
                            Value::Bool(after),
                        ],
                    ) => (
                        *player,
                        *state,
                        VisionShape::Rectangle {
                            bounds: self.rectangle_bounds(*rect)?,
                        },
                        *shared,
                        *after,
                    ),
                    _ => return Err(self.inner.error(name, Failure::Signature)),
                };
                self.player_settings(player)?;
                self.inner.symbolic_value(state, "fogstate")?;
                if shape.bounds().is_none() {
                    return Err(self.inner.error(
                        name,
                        Failure::FogModifier(
                            "finite nonnegative radius or ordered rectangle required".into(),
                        ),
                    ));
                }
                return self
                    .inner
                    .allocate(
                        Entry::FogModifier(Some(FogModifier {
                            player,
                            state,
                            shape,
                            shared_vision,
                            after_units,
                            started: false,
                        })),
                        name,
                    )
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
