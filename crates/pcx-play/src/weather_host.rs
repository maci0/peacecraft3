//! Session-owned weather areas. The renderer reads this same state, not a second world.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

const NATIVES: [(&str, &[&str], &str); 3] = [
    ("AddWeatherEffect", &["rect", "integer"], "weathereffect"),
    (
        "EnableWeatherEffect",
        &["weathereffect", "boolean"],
        "nothing",
    ),
    ("RemoveWeatherEffect", &["weathereffect"], "nothing"),
];

pub(super) struct State(pub crate::weather::Input);

#[derive(Clone, Copy, Debug)]
pub struct WeatherEffect<'a> {
    pub definition: &'a crate::weather::Definition,
    pub bounds: [f32; 4],
    pub enabled: bool,
}

impl ObjectHost {
    pub fn with_weather(mut self, input: crate::weather::Input) -> Result<Self, Error> {
        let fail = |why| self.inner.error("AddWeatherEffect", Failure::Weather(why));
        let [x0, y0, x1, y1] = input.world_bounds;
        if self
            .world_bounds
            .is_some_and(|bounds| bounds != input.world_bounds)
            || self.weather.is_some()
            || !self.rectangles
            || ![x0, y0, x1, y1].iter().all(|v| v.is_finite())
            || x0 >= x1
            || y0 >= y1
        {
            return Err(fail(
                "weather needs finite world bounds and admitted rectangles",
            ));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("Rect")
            .expect("admitted")
            .source;
        for (name, args, result) in NATIVES {
            // common.j declares weathereffect directly under handle, not agent.
            Self::contract_in(
                &self.inner,
                name,
                args,
                result,
                false,
                Some("weathereffect"),
                false,
            )?;
            if self.inner.program.natives().get(name).map(|n| &n.source) != Some(source) {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.weather = Some(State(input));
        Ok(self)
    }

    pub fn weather_effects(&self) -> impl Iterator<Item = (HandleId, WeatherEffect<'_>)> {
        self.inner
            .arena
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                let Entry::Weather {
                    id,
                    bounds,
                    enabled,
                    live: true,
                } = entry
                else {
                    return None;
                };
                let definition = self.weather.as_ref()?.0.catalog.get(*id)?;
                Some((
                    self.inner
                        .id(i, "AddWeatherEffect")
                        .expect("allocated identity"),
                    WeatherEffect {
                        definition,
                        bounds: *bounds,
                        enabled: *enabled,
                    },
                ))
            })
    }

    pub(super) fn weather_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.weather.as_ref()?;
        if !NATIVES.iter().any(|(n, _, _)| *n == name) {
            return None;
        }
        Some((|| match (name, args) {
            ("AddWeatherEffect", [Value::Handle(rect), Value::Int(kind)]) => {
                let state = self.weather.as_ref().expect("admitted");
                let id = kind.to_be_bytes();
                if state.0.catalog.get(id).is_none() {
                    return Err(self.inner.error(
                        name,
                        Failure::Weather("weather effectID is absent from selected table"),
                    ));
                }
                let world_bounds = state.0.world_bounds;
                let enabled = state.0.initial_enabled;
                let mut bounds = self.rectangle_bounds(*rect)?;
                for (i, value) in bounds.iter_mut().enumerate() {
                    *value = value.clamp(world_bounds[i % 2], world_bounds[i % 2 + 2]);
                }
                self.inner
                    .allocate(
                        Entry::Weather {
                            id,
                            bounds,
                            enabled,
                            live: true,
                        },
                        name,
                    )
                    .map(Value::Handle)
            }
            ("EnableWeatherEffect", [Value::Handle(id), Value::Bool(value)]) => {
                match self.inner.entry_mut(*id)? {
                    Entry::Weather {
                        enabled,
                        live: true,
                        ..
                    } => {
                        *enabled = *value;
                        Ok(Value::Null)
                    }
                    // The original warns for an absent weather area and returns normally.
                    Entry::Weather { live: false, .. } => Ok(Value::Null),
                    _ => Err(self.inner.error(name, Failure::WrongFamily)),
                }
            }
            ("RemoveWeatherEffect", [Value::Handle(id)]) => match self.inner.entry_mut(*id)? {
                Entry::Weather { live, .. } => {
                    *live = false;
                    Ok(Value::Null)
                }
                _ => Err(self.inner.error(name, Failure::WrongFamily)),
            },
            ("RemoveWeatherEffect", [Value::Null])
            | ("EnableWeatherEffect", [Value::Null, Value::Bool(_)]) => Ok(Value::Null),
            _ => Err(self.inner.error(name, Failure::Signature)),
        })())
    }
}
