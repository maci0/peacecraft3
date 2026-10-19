//! Executed local UI input and explicitly configured cinematic filters.
use super::{Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct CineFilter {
    pub sequence: u64,
    pub texture: String,
    pub start_uv: [f32; 4],
    pub end_uv: [f32; 4],
    pub start_color: [u8; 4],
    pub end_color: [u8; 4],
    pub duration: f32,
    pub started_at: Option<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilterSample {
    pub uv: [f32; 4],
    /// Numeric source channels normalized from RGBA8; not a color-space conversion.
    pub color: [f32; 4],
}
impl CineFilter {
    /// Reconstruction timing uses the admitted logical clock, never wall time.
    pub fn sample(&self, now: Option<f64>) -> Result<FilterSample, &'static str> {
        if !self.duration.is_finite() || self.duration < 0.0 {
            return Err("invalid filter duration");
        }
        let t = if self.duration == 0.0 {
            1.0
        } else {
            let (Some(start), Some(now)) = (self.started_at, now) else {
                return Err("filter requires an admitted logical clock");
            };
            if !start.is_finite() || !now.is_finite() || now < start {
                return Err("invalid filter clock");
            }
            ((now - start) / f64::from(self.duration)).clamp(0.0, 1.0) as f32
        };
        let uv =
            std::array::from_fn(|i| self.start_uv[i] + (self.end_uv[i] - self.start_uv[i]) * t);
        let color = std::array::from_fn(|i| {
            (f32::from(self.start_color[i])
                + (f32::from(self.end_color[i]) - f32::from(self.start_color[i])) * t)
                / f32::from(u8::MAX)
        });
        if uv.iter().chain(color.iter()).any(|v| !v.is_finite()) {
            return Err("unrepresentable filter interpolation");
        }
        Ok(FilterSample { uv, color })
    }
}

#[derive(Default)]
pub struct PresentationState {
    pub ui_enabled: Option<bool>,
    pub control_enabled: Option<bool>,
    pub interface_visible: Option<bool>,
    pub world_fog_boundary: Option<bool>,
    pub filter: Option<CineFilter>,
    texture: Option<String>,
    blend: Option<HandleId>,
    flags: Option<HandleId>,
    start_uv: Option<[f32; 4]>,
    end_uv: Option<[f32; 4]>,
    start_color: Option<[u8; 4]>,
    end_color: Option<[u8; 4]>,
    duration: Option<f32>,
    sequence: u64,
    path_bytes: usize,
}
const CONTRACTS: &[(&str, &[&str])] = &[
    ("EnableUserUI", &["boolean"]),
    ("EnableWorldFogBoundary", &["boolean"]),
    ("EnableUserControl", &["boolean"]),
    ("ShowInterface", &["boolean", "real"]),
    ("SetCineFilterTexture", &["string"]),
    ("SetCineFilterBlendMode", &["blendmode"]),
    ("SetCineFilterTexMapFlags", &["texmapflags"]),
    ("SetCineFilterStartUV", &["real", "real", "real", "real"]),
    ("SetCineFilterEndUV", &["real", "real", "real", "real"]),
    (
        "SetCineFilterStartColor",
        &["integer", "integer", "integer", "integer"],
    ),
    (
        "SetCineFilterEndColor",
        &["integer", "integer", "integer", "integer"],
    ),
    ("SetCineFilterDuration", &["real"]),
    ("DisplayCineFilter", &["boolean"]),
];
impl ObjectHost {
    pub fn admit_presentation(&mut self, path_bytes: usize) -> Result<(), Error> {
        if self.presentation.is_some() {
            return Err(self
                .inner
                .error("EnableUserUI", Failure::Presentation("already admitted")));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| self.inner.error("EnableUserUI", Failure::Signature))?
            .source;
        let mut found = false;
        for &(name, parameters) in CONTRACTS {
            if self.inner.program.natives().get(name).is_some()
                || self.inner.program.natives().ambiguous(name).is_some()
            {
                Self::contract(&self.inner, name, parameters, "nothing", false, None)?;
                let native = self
                    .inner
                    .program
                    .natives()
                    .get(name)
                    .expect("checked presentation contract");
                if &native.source != source {
                    return Err(self.inner.error(name, Failure::Source));
                }
                found = true;
            }
        }
        if found {
            self.presentation = Some(PresentationState {
                path_bytes,
                ..Default::default()
            });
        }
        Ok(())
    }
    /// Only a renderer without gameplay occlusion fading may grant this capability.
    /// Enabling fading remains unsupported; no original initial option is inferred.
    pub fn admit_no_occlusion_pass(&mut self) -> Result<(), Error> {
        let name = "EnableOcclusion";
        if self.inner.program.natives().get(name).is_none()
            && self.inner.program.natives().ambiguous(name).is_none()
        {
            return Ok(());
        }
        Self::contract(&self.inner, name, &["boolean"], "nothing", false, None)?;
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .expect("checked Host")
            .source;
        if self
            .inner
            .program
            .natives()
            .get(name)
            .expect("checked occlusion contract")
            .source
            != *source
        {
            return Err(self.inner.error(name, Failure::Source));
        }
        self.no_occlusion_pass = true;
        Ok(())
    }
    pub fn presentation(&self) -> Option<&PresentationState> {
        self.presentation.as_ref()
    }

    pub(super) fn presentation_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name == "EnableOcclusion" && self.no_occlusion_pass {
            return Some(match args {
                [Value::Bool(false)] => Ok(Value::Null),
                [Value::Bool(true)] => Err(self.inner.error(
                    name,
                    Failure::Presentation(
                        "the selected renderer has no active occlusion-fade pass",
                    ),
                )),
                _ => Err(self
                    .inner
                    .error(name, Failure::Presentation("expected occlusion boolean"))),
            });
        }
        self.presentation.as_ref()?;
        let &(_, parameters) = CONTRACTS.iter().find(|(n, _)| *n == name)?;
        self.inner.program.natives().get(name)?;
        Some(self.presentation_call(name, args, parameters.len()))
    }
    fn presentation_call(
        &mut self,
        name: &str,
        args: &[Value],
        arity: usize,
    ) -> Result<Value, Error> {
        let fail = |why| self.inner.error(name, Failure::Presentation(why));
        if args.len() != arity {
            return Err(fail("arguments do not match the declaration"));
        }
        if name == "ShowInterface" {
            let [Value::Bool(show), Value::Real(duration)] = args else {
                return Err(fail("expected interface boolean and real duration"));
            };
            if !duration.is_finite() || *duration != 0.0 {
                return Err(fail(
                    "interface fades require a supported transition consumer; only immediate changes are admitted",
                ));
            }
            let state = self.presentation.as_mut().expect("admitted");
            state.interface_visible = Some(*show);
            state.control_enabled = Some(*show);
            return Ok(Value::Null);
        }
        if name == "EnableWorldFogBoundary" {
            let [Value::Bool(enabled)] = args else {
                return Err(fail("expected world fog boundary boolean"));
            };
            self.presentation
                .as_mut()
                .expect("admitted")
                .world_fog_boundary = Some(*enabled);
            return Ok(Value::Null);
        }
        if name == "EnableUserControl" {
            let [Value::Bool(enabled)] = args else {
                return Err(fail("expected control boolean"));
            };
            self.presentation
                .as_mut()
                .expect("admitted")
                .control_enabled = Some(*enabled);
            return Ok(Value::Null);
        }
        if name == "DisplayCineFilter" {
            let [Value::Bool(show)] = args else {
                return Err(fail("expected display boolean"));
            };
            if !show {
                self.presentation.as_mut().expect("admitted").filter = None;
                return Ok(Value::Null);
            }
            let state = self.presentation.as_ref().expect("admitted");
            let texture = state
                .texture
                .as_ref()
                .ok_or_else(|| fail("missing filter texture"))?;
            let start_uv = state.start_uv.ok_or_else(|| fail("missing start UV"))?;
            let end_uv = state.end_uv.ok_or_else(|| fail("missing end UV"))?;
            let start_color = state
                .start_color
                .ok_or_else(|| fail("missing start color"))?;
            let end_color = state.end_color.ok_or_else(|| fail("missing end color"))?;
            let duration = state.duration.ok_or_else(|| fail("missing duration"))?;
            let blend = state.blend.ok_or_else(|| fail("missing blend mode"))?;
            let flags = state
                .flags
                .ok_or_else(|| fail("missing texture mapping flags"))?;
            if self.inner.symbolic_value(blend, "blendmode")?
                != self.source_enumerand("DisplayCineFilter", "BLEND_MODE_BLEND", "blendmode")?
                || self.inner.symbolic_value(flags, "texmapflags")?
                    != self.source_enumerand(
                        "DisplayCineFilter",
                        "TEXMAP_FLAG_NONE",
                        "texmapflags",
                    )?
            {
                return Err(fail("unsupported filter blend or texture mapping"));
            }
            let started_at = self.timer_game_time();
            if duration > 0.0 && started_at.is_none() {
                return Err(fail("timed filter requires an admitted logical clock"));
            }
            let sequence = state
                .sequence
                .checked_add(1)
                .ok_or_else(|| fail("filter sequence exhausted"))?;
            let snapshot = CineFilter {
                sequence,
                texture: texture.clone(),
                start_uv,
                end_uv,
                start_color,
                end_color,
                duration,
                started_at,
            };
            snapshot.sample(started_at).map_err(fail)?;
            let state = self.presentation.as_mut().expect("admitted");
            state.sequence = sequence;
            state.filter = Some(snapshot);
            return Ok(Value::Null);
        }
        let enum_value = match (name, args) {
            ("SetCineFilterBlendMode", [Value::Handle(id)]) => {
                self.inner.symbolic_value(*id, "blendmode")?;
                Some(*id)
            }
            ("SetCineFilterTexMapFlags", [Value::Handle(id)]) => {
                self.inner.symbolic_value(*id, "texmapflags")?;
                Some(*id)
            }
            _ => None,
        };
        let state = self.presentation.as_mut().expect("admitted");
        match (name, args) {
            ("EnableUserUI", [Value::Bool(enabled)]) => state.ui_enabled = Some(*enabled),
            ("SetCineFilterTexture", [Value::Str(path)])
                if !path.is_empty() && path.len() <= state.path_bytes && !path.contains('\0') =>
            {
                state.texture = Some(path.clone())
            }
            ("SetCineFilterBlendMode", _) if enum_value.is_some() => state.blend = enum_value,
            ("SetCineFilterTexMapFlags", _) if enum_value.is_some() => state.flags = enum_value,
            ("SetCineFilterDuration", [Value::Real(duration)])
                if duration.is_finite() && *duration >= 0.0 =>
            {
                state.duration = Some(*duration)
            }
            (
                "SetCineFilterStartUV" | "SetCineFilterEndUV",
                [
                    Value::Real(a),
                    Value::Real(b),
                    Value::Real(c),
                    Value::Real(d),
                ],
            ) if [a, b, c, d].iter().all(|v| v.is_finite()) => {
                let uv = [a.min(*c), b.min(*d), a.max(*c), b.max(*d)];
                if name == "SetCineFilterStartUV" {
                    state.start_uv = Some(uv);
                } else {
                    state.end_uv = Some(uv);
                }
            }
            (
                "SetCineFilterStartColor" | "SetCineFilterEndColor",
                [Value::Int(r), Value::Int(g), Value::Int(b), Value::Int(a)],
            ) => {
                let rgba = [*r as u8, *g as u8, *b as u8, *a as u8];
                if name == "SetCineFilterStartColor" {
                    state.start_color = Some(rgba);
                } else {
                    state.end_color = Some(rgba);
                }
            }
            _ => {
                return Err(self.inner.error(
                    name,
                    Failure::Presentation("invalid or unsupported arguments"),
                ));
            }
        }
        Ok(Value::Null)
    }
}

#[cfg(test)]
#[path = "presentation_host_tests.rs"]
mod tests;
