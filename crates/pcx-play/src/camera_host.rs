use super::{Error, Failure, ObjectHost};
use crate::camera_margins::CameraMargins;
use pcx_script::Value;

/// Source-derived margin inputs and bounds actually assigned by script execution.
/// No camera movement, projection or default bounds are inferred.
#[derive(Debug)]
pub struct CameraState {
    pub margins: CameraMargins,
    pub bounds: Option<[f32; 8]>,
    bound_getters: bool,
}
const BOUND_GETTERS: [&str; 4] = [
    "GetCameraBoundMinX",
    "GetCameraBoundMinY",
    "GetCameraBoundMaxX",
    "GetCameraBoundMaxY",
];
impl ObjectHost {
    pub fn with_camera(mut self, margins: CameraMargins) -> Result<Self, Error> {
        if self.camera.is_some() {
            return Err(self
                .inner
                .error("camera", Failure::CameraState("already admitted")));
        }
        Self::contract(
            &self.inner,
            "GetCameraMargin",
            &["integer"],
            "real",
            false,
            None,
        )?;
        Self::contract(
            &self.inner,
            "SetCameraBounds",
            &["real"; 8],
            "nothing",
            false,
            None,
        )?;
        self.camera = Some(CameraState {
            margins,
            bounds: None,
            bound_getters: false,
        });
        Ok(self)
    }
    /// Admit coordinate extrema of executed bounds for an unrotated camera.
    ///
    /// The caller qualifies this coordinate profile; no initial bounds or
    /// external camera rotation is inferred. Camera rotation/movement natives
    /// remain unsupported. Margin/config values are not applied a second time.
    pub fn with_camera_bound_getters(mut self) -> Result<Self, Error> {
        let fail = |reason| {
            self.inner
                .error("camera bounds", Failure::CameraState(reason))
        };
        let state = self
            .camera
            .as_ref()
            .ok_or_else(|| fail("camera not admitted"))?;
        if state.bound_getters {
            return Err(fail("already admitted"));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("SetCameraBounds")
            .expect("admitted bounds declaration")
            .source;
        for name in BOUND_GETTERS {
            Self::contract(&self.inner, name, &[], "real", true, None)?;
        }
        for name in BOUND_GETTERS
            .into_iter()
            .chain(["GetCameraMargin", "CreateTimer"])
        {
            if &self
                .inner
                .program
                .natives()
                .get(name)
                .expect("admitted camera declaration")
                .source
                != source
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.camera.as_mut().expect("admitted camera").bound_getters = true;
        Ok(self)
    }
    pub fn camera_state(&self) -> Option<&CameraState> {
        self.camera.as_ref()
    }
    pub(super) fn camera_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let state = self.camera.as_mut()?;
        if state.bound_getters
            && let Some(index) = BOUND_GETTERS.iter().position(|getter| *getter == name)
        {
            let fail = |reason| self.inner.error(name, Failure::CameraState(reason));
            return Some((|| {
                if !args.is_empty() {
                    return Err(fail("arguments do not match the camera declaration"));
                }
                let bounds = state
                    .bounds
                    .ok_or_else(|| fail("camera bounds are unset"))?;
                let values = bounds
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|corner| corner[index % 2]);
                let value = if index < 2 {
                    values.reduce(f32::min)
                } else {
                    values.reduce(f32::max)
                };
                Ok(Value::Real(value.expect("four retained corners")))
            })());
        }
        if !matches!(name, "GetCameraMargin" | "SetCameraBounds") {
            return None;
        }
        let fail = |reason| self.inner.error(name, Failure::CameraState(reason));
        Some(match (name, args) {
            ("GetCameraMargin", [Value::Int(index)]) => usize::try_from(*index)
                .ok()
                .and_then(|i| state.margins.values().get(i))
                .copied()
                .map(Value::Real)
                .ok_or_else(|| fail("unsupported margin index")),
            ("SetCameraBounds", args) if args.len() == 8 => {
                let mut bounds = [0.0; 8];
                for (out, value) in bounds.iter_mut().zip(args) {
                    let Some(value) = value.as_real().filter(|v| v.is_finite()) else {
                        return Some(Err(fail("finite bounds required")));
                    };
                    *out = value;
                }
                state.bounds = Some(bounds);
                Ok(Value::Null)
            }
            _ => Err(fail("arguments do not match the camera declaration")),
        })
    }
}
