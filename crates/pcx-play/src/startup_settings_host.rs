//! Explicit startup settings on the current map Host; no inferred defaults.
use super::super::{Error, Failure};
use super::ObjectHost;
use crate::map_admission::ParsedMapContext;
use pcx_script::ast::Expr;
use pcx_script::{HandleId, Value};
use std::sync::Arc;
/// Caller-qualified current state, not simulation rates or rendered visibility.
#[derive(Clone, Copy, Debug)]
pub struct StartupSettings {
    pub speed: HandleId,
    pub fog_enabled: bool,
    pub fog_mask_enabled: bool,
}
impl ObjectHost {
    /// Install or replace current settings on the exact admitted player context.
    ///
    /// Context identity does not authenticate the input's runtime provenance.
    /// The caller must refresh or invalidate on external setting changes. Failed
    /// admission preserves the prior state. Only requests for the already-current
    /// speed are supported; a different speed requires a qualified clock rate.
    pub fn admit_startup_settings(
        &mut self,
        context: &Arc<ParsedMapContext>,
        settings: StartupSettings,
    ) -> Result<(), Error> {
        let fail = |reason| self.inner.error("GetGameSpeed", reason);
        if self
            .player_context()
            .is_none_or(|selected| !Arc::ptr_eq(selected, context))
        {
            return Err(fail(Failure::SettingsState("map context is not admitted")));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("Player")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        if &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?
            .source
            != source
        {
            return Err(fail(Failure::Source));
        }
        for (name, params, result, constant) in [
            ("ConvertGameSpeed", &["integer"][..], "gamespeed", true),
            ("GetGameSpeed", &[][..], "gamespeed", true),
            ("IsFogEnabled", &[][..], "boolean", false),
            ("IsFogMaskEnabled", &[][..], "boolean", false),
            ("FogEnable", &["boolean"][..], "nothing", false),
            ("FogMaskEnable", &["boolean"][..], "nothing", false),
        ] {
            Self::contract(&self.inner, name, params, result, constant, None)?;
            if &self
                .inner
                .program
                .natives()
                .get(name)
                .ok_or_else(|| fail(Failure::Signature))?
                .source
                != source
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        let name = "SetGameSpeed";
        if self.inner.program.natives().get(name).is_some()
            || self.inner.program.natives().ambiguous(name).is_some()
        {
            Self::contract(&self.inner, name, &["gamespeed"], "nothing", false, None)?;
            if &self
                .inner
                .program
                .natives()
                .get(name)
                .expect("checked setter")
                .source
                != source
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        let conversion = self
            .inner
            .conversions
            .get("ConvertGameSpeed")
            .ok_or_else(|| fail(Failure::Signature))?;
        if &conversion.source != source {
            return Err(fail(Failure::Source));
        }
        if conversion.kind != "gamespeed" {
            return Err(fail(Failure::Signature));
        }
        let mut kind = "gamespeed";
        for _ in 0..=self.inner.program.types().count() {
            if kind == "handle" {
                break;
            }
            let ty = self
                .inner
                .program
                .types()
                .find(|ty| ty.name == kind)
                .ok_or_else(|| fail(Failure::Signature))?;
            if &ty.source != source {
                return Err(fail(Failure::Source));
            }
            kind = &ty.parent;
        }
        if kind != "handle" {
            return Err(fail(Failure::Signature));
        }
        let value = self
            .inner
            .symbolic_value(settings.speed, "gamespeed")
            .map_err(|e| fail(e.failure))?;
        if !conversion.domain.contains(&value) {
            return Err(fail(Failure::OutsideDomain(value)));
        }
        self.startup_settings = Some(settings);
        Ok(())
    }
    /// Supply a qualified runtime version identity for this exact map context.
    /// No archive name, display skin or numeric executable version selects it.
    /// Failed admission preserves current input; queries mint no identities.
    pub fn admit_startup_version(
        &mut self,
        context: &Arc<ParsedMapContext>,
        version: HandleId,
    ) -> Result<(), Error> {
        let fail = |reason| self.inner.error("VersionGet", reason);
        if self
            .player_context()
            .is_none_or(|selected| !Arc::ptr_eq(selected, context))
        {
            return Err(fail(Failure::SettingsState("map context is not admitted")));
        }
        Self::contract(&self.inner, "VersionGet", &[], "version", false, None)?;
        Self::contract(
            &self.inner,
            "ConvertVersion",
            &["integer"],
            "version",
            true,
            None,
        )?;
        let program = &self.inner.program;
        let source = &program
            .natives()
            .get("Player")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        for name in ["CreateTimer", "ConvertVersion", "VersionGet"] {
            if program
                .natives()
                .get(name)
                .is_none_or(|n| &n.source != source)
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        let conversion = self
            .inner
            .conversions
            .get("ConvertVersion")
            .ok_or_else(|| fail(Failure::Signature))?;
        if &conversion.source != source {
            return Err(fail(Failure::Source));
        }
        if conversion.kind != "version" {
            return Err(fail(Failure::Signature));
        }
        let mut kind = "version";
        for _ in 0..=program.types().count() {
            if kind == "handle" {
                break;
            }
            let ty = program
                .types()
                .find(|ty| ty.name == kind)
                .ok_or_else(|| fail(Failure::Signature))?;
            if &ty.source != source {
                return Err(fail(Failure::Source));
            }
            kind = &ty.parent;
        }
        if kind != "handle" {
            return Err(fail(Failure::Signature));
        }
        let value = self
            .inner
            .symbolic_value(version, "version")
            .map_err(|e| fail(e.failure))?;
        if !conversion.domain.contains(&value) {
            return Err(fail(Failure::OutsideDomain(value)));
        }
        self.startup_version = Some(version);
        Ok(())
    }
    /// Withdraw version input without changing independently supplied settings.
    pub fn invalidate_startup_version(&mut self) {
        self.startup_version = None;
    }
    /// Call before further script queries when the retained input is no longer current.
    pub fn invalidate_startup_settings(&mut self) {
        self.startup_settings = None;
    }
    fn legacy_campaign_menu_native(&self, args: &[Value]) -> Result<Value, Error> {
        let name = "SetCampaignMenuRace";
        let fail = |reason| self.inner.error(name, reason);
        Self::contract(&self.inner, name, &["race"], "nothing", false, None)?;
        Self::contract(&self.inner, "ConvertRace", &["integer"], "race", true, None)?;
        let source = &self
            .inner
            .program
            .natives()
            .get("VersionGet")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        if [name, "ConvertRace"].iter().any(|n| {
            self.inner
                .program
                .natives()
                .get(n)
                .is_none_or(|n| &n.source != source)
        }) {
            return Err(fail(Failure::Source));
        }
        let global = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == "VERSION_FROZEN_THRONE")
            .ok_or_else(|| {
                fail(Failure::SettingsState(
                    "missing Frozen Throne version constant",
                ))
            })?;
        if global.source != *source {
            return Err(fail(Failure::Source));
        }
        if !global.is_constant
            || global.array
            || global.ty.array
            || global.ty.kind.name() != "version"
        {
            return Err(fail(Failure::Initializer(global.name.clone())));
        }
        let Some(Expr::Call {
            callee, args: init, ..
        }) = &global.init
        else {
            return Err(fail(Failure::Initializer(global.name.clone())));
        };
        let (Expr::Ident(conversion, _), [value]) = (callee.as_ref(), init.as_slice()) else {
            return Err(fail(Failure::Initializer(global.name.clone())));
        };
        if conversion.as_str() != "ConvertVersion" {
            return Err(fail(Failure::Initializer(global.name.clone())));
        }
        let mut work = 128;
        let expected = super::super::integer(value, 16, &mut work).map_err(fail)?;
        let version = self
            .startup_version
            .ok_or_else(|| fail(Failure::SettingsState("missing explicit runtime version")))?;
        if self.inner.symbolic_value(version, "version")? != expected {
            return Err(fail(Failure::SettingsState(
                "campaign menu selection is unsupported for this runtime version",
            )));
        }
        match args {
            [Value::Null] => {}
            [Value::Handle(race)] => {
                self.inner
                    .symbolic_value(*race, "race")
                    .map_err(|e| fail(e.failure))?;
            }
            _ => {
                return Err(fail(Failure::SettingsState(
                    "expected race identity or null",
                )));
            }
        }
        // The selected legacy setter returns before reading race in Frozen Throne mode.
        Ok(Value::Null)
    }
    pub(super) fn startup_settings_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name == "SetCampaignMenuRace" && self.startup_version.is_some() {
            return Some(self.legacy_campaign_menu_native(args));
        }
        if name == "VersionGet" {
            let version = self.startup_version?;
            return Some(if args.is_empty() {
                Ok(Value::Handle(version))
            } else {
                Err(self.inner.error(
                    name,
                    Failure::SettingsState("arguments do not match the native declaration"),
                ))
            });
        }
        if name == "SetGameSpeed" {
            let settings = self.startup_settings?;
            return Some((|| {
                Self::contract(&self.inner, name, &["gamespeed"], "nothing", false, None)?;
                match args {
                    [Value::Handle(speed)] if *speed == settings.speed => Ok(Value::Null),
                    // shortcut: rate changes refuse until the clock consumes qualified speed rates.
                    [Value::Handle(_)] => Err(self.inner.error(
                        name,
                        Failure::SettingsState("changing game speed is unsupported"),
                    )),
                    _ => Err(self.inner.error(
                        name,
                        Failure::SettingsState("expected the current gamespeed identity"),
                    )),
                }
            })());
        }
        let settings = self.startup_settings.as_mut()?;
        match (name, args) {
            ("GetGameSpeed", []) => Some(Ok(Value::Handle(settings.speed))),
            ("IsFogEnabled", []) => Some(Ok(Value::Bool(settings.fog_enabled))),
            ("IsFogMaskEnabled", []) => Some(Ok(Value::Bool(settings.fog_mask_enabled))),
            ("FogEnable", [Value::Bool(value)]) => {
                settings.fog_enabled = *value;
                Some(Ok(Value::Null))
            }
            ("FogMaskEnable", [Value::Bool(value)]) => {
                settings.fog_mask_enabled = *value;
                Some(Ok(Value::Null))
            }
            (
                "GetGameSpeed" | "IsFogEnabled" | "IsFogMaskEnabled" | "FogEnable"
                | "FogMaskEnable",
                _,
            ) => Some(Err(self.inner.error(
                name,
                Failure::SettingsState("arguments do not match the native declaration"),
            ))),
            _ => None,
        }
    }
}
