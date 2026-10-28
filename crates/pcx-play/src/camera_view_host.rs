//! Live camera on the owned game clock: target and field ramps from script
//! pans, setup applications and resets to the selected game camera.
//! Linear interpolation over the requested duration is a recreation choice.
use super::{Entry, Error, Failure, ObjectHost};
use crate::camera_input::Pose;
use pcx_objects::editor::camera::GameCamera;
use pcx_script::{HandleId, Value};
use std::collections::BTreeMap;

const NATIVES: [(&str, &[&str], &str, bool); 8] = [
    (
        "PanCameraToTimed",
        &["real", "real", "real"],
        "nothing",
        false,
    ),
    ("ResetToGameCamera", &["real"], "nothing", false),
    ("CameraSetSmoothingFactor", &["real"], "nothing", false),
    (
        "SetCameraQuickPosition",
        &["real", "real"],
        "nothing",
        false,
    ),
    ("GetCameraTargetPositionLoc", &[], "location", true),
    ("GetCameraTargetPositionX", &[], "real", true),
    ("GetCameraTargetPositionY", &[], "real", true),
    ("GetCameraField", &["camerafield"], "real", true),
];

/// Fields whose `GetCameraField` result is in radians. The selected `Blizzard.j`
/// `GetCurrentCameraSetup` scales exactly these by `bj_RADTODEG` before
/// `CameraSetupSetField`, which takes degrees. Stored values stay in degrees.
const ANGLE_FIELDS: [&str; 4] = [
    "CAMERA_FIELD_ANGLE_OF_ATTACK",
    "CAMERA_FIELD_FIELD_OF_VIEW",
    "CAMERA_FIELD_ROLL",
    "CAMERA_FIELD_ROTATION",
];
/// The selected script's own default game-camera roll; `[Camera]` has no roll key.
const DEFAULT_ROLL: &str = "bj_CAMERA_DEFAULT_ROLL";

#[derive(Clone, Copy, Debug)]
struct Ramp<const N: usize> {
    from: [f32; N],
    to: [f32; N],
    start: f64,
    duration: f64,
}

impl<const N: usize> Ramp<N> {
    fn fixed(value: [f32; N], now: f64) -> Self {
        Self {
            from: value,
            to: value,
            start: now,
            duration: 0.0,
        }
    }
    fn at(&self, now: f64) -> [f32; N] {
        let t = if self.duration > 0.0 {
            ((now - self.start) / self.duration).clamp(0.0, 1.0) as f32
        } else {
            1.0
        };
        std::array::from_fn(|i| self.from[i] + (self.to[i] - self.from[i]) * t)
    }
    fn retarget(&mut self, to: [f32; N], now: f64, duration: f32) {
        *self = Self {
            from: self.at(now),
            to,
            start: now,
            duration: f64::from(duration.max(0.0)),
        };
    }
}

pub(super) struct CameraView {
    target: Option<Ramp<2>>,
    fields: BTreeMap<i32, Ramp<1>>,
    game: Vec<(i32, f32)>,
    smoothing: Option<f32>,
    quick: Option<[f32; 2]>,
    scroll_rates: [f32; 2],
}

/// Script camera fields named by the selected common, paired with the game camera.
fn game_fields(camera: &GameCamera) -> [(&'static str, f32); 6] {
    [
        ("CAMERA_FIELD_ANGLE_OF_ATTACK", camera.angle_of_attack),
        ("CAMERA_FIELD_FIELD_OF_VIEW", camera.field_of_view),
        ("CAMERA_FIELD_ROTATION", camera.rotation),
        ("CAMERA_FIELD_TARGET_DISTANCE", camera.distance),
        ("CAMERA_FIELD_FARZ", camera.far_z),
        ("CAMERA_FIELD_ZOFFSET", camera.height),
    ]
}

impl ObjectHost {
    /// Requires camera setups and the game clock. Game-camera fields resolve
    /// through the selected `CAMERA_FIELD_*` constants, not built-in numbers.
    pub fn admit_camera_view(&mut self, camera: &GameCamera) -> Result<(), Error> {
        let name = "ResetToGameCamera";
        let fail = |reason| self.inner.error(name, Failure::CameraSetupObject(reason));
        if self.camera_view.is_some() || self.camera_setups.is_none() {
            return Err(fail("requires camera setups, once"));
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| fail("game clock required"))?;
        for (native, args, result, constant) in NATIVES {
            if self.inner.program.natives().get(native).is_none() {
                continue;
            }
            Self::contract_in(&self.inner, native, args, result, constant, None, false)?;
        }
        let mut game = Vec::new();
        for (global, value) in game_fields(camera) {
            game.push((self.camera_field_constant(global)?, value));
        }
        if let Some(roll) = self.default_roll()? {
            game.push((self.camera_field_constant("CAMERA_FIELD_ROLL")?, roll));
        }
        let fields = game
            .iter()
            .map(|&(field, value)| (field, Ramp::fixed([value], now)))
            .collect();
        self.camera_view = Some(CameraView {
            target: None,
            fields,
            game,
            smoothing: None,
            quick: None,
            scroll_rates: [camera.forward_rate, camera.strafe_rate],
        });
        Ok(())
    }

    /// `bj_CAMERA_DEFAULT_ROLL` when the selected script declares it.
    fn default_roll(&self) -> Result<Option<f32>, Error> {
        let Some(declared) = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == DEFAULT_ROLL && g.is_constant && !g.array)
        else {
            return Ok(None);
        };
        let fail = |reason| self.inner.error(DEFAULT_ROLL, Failure::UnitState(reason));
        let init = declared
            .init
            .as_ref()
            .ok_or_else(|| fail("missing initializer"))?;
        let mut work = 4096;
        let roll = crate::playercolor_host::integer(init, 64, &mut work)
            .map_err(|failure| self.inner.error(DEFAULT_ROLL, failure))?;
        Ok(Some(roll as f32))
    }

    fn camera_field_constant(&self, global: &str) -> Result<i32, Error> {
        self.source_constant_enumerand(global, "camerafield")
    }

    /// Current target, if any script call set one.
    pub fn camera_target(&self) -> Option<[f32; 2]> {
        let now = self.timer_game_time()?;
        Some(self.camera_view.as_ref()?.target?.at(now))
    }

    /// Current value of a loaded `camerafield` handle.
    pub fn camera_field(&self, field: HandleId) -> Result<Option<f32>, Error> {
        let name = "SetCameraField";
        let Entry::Symbolic { kind, enumerand } = self.inner.entry(field)? else {
            return Err(self.inner.error(name, Failure::WrongFamily));
        };
        if kind != "camerafield" {
            return Err(self.inner.error(name, Failure::WrongFamily));
        }
        let (Some(view), Some(now)) = (&self.camera_view, self.timer_game_time()) else {
            return Ok(None);
        };
        Ok(view.fields.get(enumerand).map(|ramp| ramp.at(now)[0]))
    }

    /// Selected `[CameraRates]` forward and strafe scroll rates.
    pub fn camera_scroll_rates(&self) -> Option<[f32; 2]> {
        Some(self.camera_view.as_ref()?.scroll_rates)
    }

    /// Player scrolling: an immediate target change, cancelling a running pan.
    pub fn scroll_camera(&mut self, to: [f32; 2]) -> Result<(), Error> {
        let fail = |reason| {
            self.inner
                .error("camera scroll", Failure::CameraSetupObject(reason))
        };
        let now = self.timer_game_time().ok_or_else(|| fail("game clock"))?;
        if !to.iter().all(|v| v.is_finite()) {
            return Err(fail("finite target required"));
        }
        let view = self.camera_view.as_mut().ok_or_else(|| fail("no camera"))?;
        view.target = Some(Ramp::fixed(to, now));
        Ok(())
    }

    /// Player zoom or rotation: the game camera takes `pose`, so `ResetToGameCamera`
    /// returns to it, and the live distance, angle of attack and rotation ease there
    /// over `duration`. Linear easing and the shared game clock are recreation choices.
    pub fn ease_player_camera(&mut self, pose: Pose, duration: f32) -> Result<(), Error> {
        let fail = |reason| {
            self.inner
                .error("camera zoom", Failure::CameraSetupObject(reason))
        };
        let values = [pose.distance, pose.angle_of_attack, pose.rotation];
        if !duration.is_finite() || !values.iter().all(|v| v.is_finite()) {
            return Err(fail("finite pose and duration required"));
        }
        let now = self.timer_game_time().ok_or_else(|| fail("game clock"))?;
        let fields = [
            ("CAMERA_FIELD_TARGET_DISTANCE", pose.distance),
            ("CAMERA_FIELD_ANGLE_OF_ATTACK", pose.angle_of_attack),
            ("CAMERA_FIELD_ROTATION", pose.rotation),
        ]
        .map(|(global, value)| self.camera_field_constant(global).map(|field| (field, value)))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
        let view = self.camera_view.as_mut().ok_or_else(|| fail("no camera"))?;
        for (field, value) in fields {
            match view.game.iter_mut().find(|(known, _)| *known == field) {
                Some(game) => game.1 = value,
                None => view.game.push((field, value)),
            }
            view.fields
                .entry(field)
                .or_insert(Ramp::fixed([value], now))
                .retarget([value], now, duration);
        }
        Ok(())
    }

    /// Setup application: fields and optional target ramp over `duration`.
    pub(super) fn apply_camera_view(
        &mut self,
        fields: &[(i32, f32)],
        target: Option<[f32; 2]>,
        duration: f32,
    ) {
        let (Some(now), Some(view)) = (self.timer_game_time(), self.camera_view.as_mut()) else {
            return;
        };
        for &(field, value) in fields {
            view.fields
                .entry(field)
                .or_insert(Ramp::fixed([value], now))
                .retarget([value], now, duration);
        }
        if let Some(to) = target {
            match &mut view.target {
                Some(ramp) => ramp.retarget(to, now, duration),
                None => view.target = Some(Ramp::fixed(to, now)),
            }
        }
    }

    pub(super) fn camera_view_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.camera_view.as_ref()?;
        if !NATIVES.iter().any(|&(n, ..)| n == name) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some((|| {
            let fail = |reason| self.inner.error(name, Failure::CameraSetupObject(reason));
            let now = self.timer_game_time().ok_or_else(|| fail("game clock"))?;
            if name == "GetCameraField" {
                let [Value::Handle(field)] = args else {
                    return Err(self.inner.error(name, Failure::Signature));
                };
                let value = self
                    .camera_field(*field)?
                    .ok_or_else(|| fail("camera field has no value"))?;
                let Entry::Symbolic { enumerand, .. } = self.inner.entry(*field)? else {
                    return Err(self.inner.error(name, Failure::WrongFamily));
                };
                let enumerand = *enumerand;
                for angle in ANGLE_FIELDS {
                    if self.camera_field_constant(angle).ok() == Some(enumerand) {
                        return Ok(Value::Real(value.to_radians()));
                    }
                }
                return Ok(Value::Real(value));
            }
            let reals = args
                .iter()
                .map(|v| v.as_real().filter(|r| r.is_finite()))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| fail("finite reals required"))?;
            let view = self.camera_view.as_mut().expect("admitted");
            match (name, reals.as_slice()) {
                ("PanCameraToTimed", &[x, y, duration]) => match &mut view.target {
                    Some(ramp) => ramp.retarget([x, y], now, duration),
                    None => view.target = Some(Ramp::fixed([x, y], now)),
                },
                ("ResetToGameCamera", &[duration]) => {
                    for &(field, value) in &view.game {
                        view.fields
                            .entry(field)
                            .or_insert(Ramp::fixed([value], now))
                            .retarget([value], now, duration);
                    }
                }
                ("CameraSetSmoothingFactor", &[factor]) => view.smoothing = Some(factor),
                ("SetCameraQuickPosition", &[x, y]) => view.quick = Some([x, y]),
                ("GetCameraTargetPositionX" | "GetCameraTargetPositionY", []) => {
                    let point = view
                        .target
                        .map(|ramp| ramp.at(now))
                        .ok_or_else(|| fail("camera target is unset"))?;
                    return Ok(Value::Real(point[usize::from(name.ends_with('Y'))]));
                }
                ("GetCameraTargetPositionLoc", []) => {
                    let point = view
                        .target
                        .map(|ramp| ramp.at(now))
                        .ok_or_else(|| fail("camera target is unset"))?;
                    return self
                        .inner
                        .allocate(Entry::Location { point: Some(point) }, name)
                        .map(Value::Handle);
                }
                _ => return Err(self.inner.error(name, Failure::Signature)),
            }
            Ok(Value::Null)
        })())
    }
}
