use super::{Error, Failure, ObjectHost};
use crate::camera_margins::CameraMargins;
use pcx_script::Value;

/// Source-derived margin inputs and bounds actually assigned by script execution.
/// No camera movement, projection or default bounds are inferred.
#[derive(Debug)]
pub struct CameraState {
    pub margins: CameraMargins,
    pub bounds: Option<[f32; 8]>,
}
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
        });
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
