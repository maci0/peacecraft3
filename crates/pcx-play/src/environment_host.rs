//! Executed sky model and terrain fog for the renderer. Values are the script's
//! own arguments; no fog curve or color conversion is applied here.
use super::{Error, Failure, ObjectHost};
use pcx_script::Value;

const NATIVES: [(&str, &[&str]); 3] = [
    ("SetSkyModel", &["string"]),
    (
        "SetTerrainFogEx",
        &["integer", "real", "real", "real", "real", "real", "real"],
    ),
    ("ResetTerrainFog", &[]),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainFog {
    pub style: i32,
    pub z_start: f32,
    pub z_end: f32,
    pub density: f32,
    pub color: [f32; 3],
}

/// `None` fields have not been set by script: the authored map settings apply.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Environment {
    /// `Some(None)` is an executed `SetSkyModel(null)`: no sky.
    pub sky_model: Option<Option<String>>,
    pub terrain_fog: Option<TerrainFog>,
}

impl ObjectHost {
    /// Admit the declared natives among these three; all share one source.
    pub fn with_environment(mut self) -> Result<Self, Error> {
        if self.scene.is_some() {
            return Err(self.inner.error(
                "SetSkyModel",
                Failure::UnitState("environment already admitted"),
            ));
        }
        let mut source = None;
        for (name, args) in NATIVES {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, "nothing", false, None)?;
            if *source.get_or_insert(&native.source) != &native.source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.scene = Some(Environment::default());
        Ok(self)
    }

    pub fn environment(&self) -> Option<&Environment> {
        self.scene.as_ref()
    }

    pub(super) fn environment_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.scene.as_ref()?;
        if !NATIVES.iter().any(|&(n, _)| n == name)
            || self.inner.program.natives().get(name).is_none()
        {
            return None;
        }
        let real = |v: &Value| v.as_real().filter(|r| r.is_finite());
        let state = self.scene.as_mut().expect("admitted");
        Some(match (name, args) {
            ("SetSkyModel", [Value::Str(path)]) => {
                state.sky_model = Some(Some(path.clone()));
                Ok(Value::Null)
            }
            ("SetSkyModel", [Value::Null]) => {
                state.sky_model = Some(None);
                Ok(Value::Null)
            }
            ("SetTerrainFogEx", [Value::Int(style), rest @ ..]) => {
                match rest.iter().map(real).collect::<Option<Vec<_>>>().as_deref() {
                    Some(&[z_start, z_end, density, r, g, b]) => {
                        state.terrain_fog = Some(TerrainFog {
                            style: *style,
                            z_start,
                            z_end,
                            density,
                            color: [r, g, b],
                        });
                        Ok(Value::Null)
                    }
                    _ => Err(self.inner.error(name, Failure::Signature)),
                }
            }
            ("ResetTerrainFog", []) => {
                state.terrain_fog = None;
                Ok(Value::Null)
            }
            _ => Err(self.inner.error(name, Failure::Signature)),
        })
    }
}
