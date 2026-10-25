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
    /// admission preserves the prior state. Locked speed requests are ignored.
    /// Unlocked requests can only retain the current speed until rates are qualified.
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
    /// Explicit game input from the selected `ConvertGameDifficulty` domain.
    pub fn admit_game_difficulty(
        &mut self,
        context: &Arc<ParsedMapContext>,
        difficulty: HandleId,
    ) -> Result<(), Error> {
        let name = "GetGameDifficulty";
        let fail = |reason| self.inner.error(name, reason);
        if self
            .player_context()
            .is_none_or(|selected| !Arc::ptr_eq(selected, context))
        {
            return Err(fail(Failure::SettingsState("map context is not admitted")));
        }
        Self::contract(&self.inner, name, &[], "gamedifficulty", true, None)?;
        if self
            .inner
            .program
            .natives()
            .get("SetGameDifficulty")
            .is_some()
        {
            Self::contract(
                &self.inner,
                "SetGameDifficulty",
                &["gamedifficulty"],
                "nothing",
                false,
                None,
            )?;
        }
        let conversion = self
            .inner
            .conversions
            .get("ConvertGameDifficulty")
            .filter(|c| c.kind == "gamedifficulty")
            .ok_or_else(|| fail(Failure::Signature))?;
        let value = self
            .inner
            .symbolic_value(difficulty, "gamedifficulty")
            .map_err(|e| fail(e.failure))?;
        if !conversion.domain.contains(&value) {
            return Err(fail(Failure::OutsideDomain(value)));
        }
        self.game_difficulty = Some(difficulty);
        Ok(())
    }
    /// Admits `GetDefaultDifficulty`/`SetDefaultDifficulty` with a fallback ordinal from the
    /// selected image. The ordinal must lie in the selected `ConvertGameDifficulty` domain.
    /// Current difficulty stays separate; admission is all-or-nothing.
    pub fn admit_default_difficulty(
        &mut self,
        context: &Arc<ParsedMapContext>,
        fallback: i32,
    ) -> Result<(), Error> {
        let name = "GetDefaultDifficulty";
        let fail = |reason| self.inner.error(name, reason);
        if self
            .player_context()
            .is_none_or(|selected| !Arc::ptr_eq(selected, context))
        {
            return Err(fail(Failure::SettingsState("map context is not admitted")));
        }
        Self::contract(&self.inner, name, &[], "gamedifficulty", false, None)?;
        Self::contract(
            &self.inner,
            "SetDefaultDifficulty",
            &["gamedifficulty"],
            "nothing",
            false,
            None,
        )?;
        let anchor = self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?
            .source
            .clone();
        for native in [name, "SetDefaultDifficulty"] {
            if self
                .inner
                .program
                .natives()
                .get(native)
                .is_none_or(|n| n.source != anchor)
            {
                return Err(self.inner.error(native, Failure::Source));
            }
        }
        let conversion = self
            .inner
            .conversions
            .get("ConvertGameDifficulty")
            .filter(|c| c.kind == "gamedifficulty")
            .ok_or_else(|| fail(Failure::Signature))?;
        if !conversion.domain.contains(&fallback) {
            return Err(fail(Failure::OutsideDomain(fallback)));
        }
        let converted = self
            .inner
            .convert_for("ConvertGameDifficulty", &[Value::Int(fallback)]);
        let Value::Handle(handle) = converted.map_err(|e| self.inner.error(name, e.failure))?
        else {
            return Err(self.inner.error(name, Failure::Signature));
        };
        self.default_difficulty = Some(handle);
        Ok(())
    }

    pub fn startup_settings(&self) -> Option<StartupSettings> {
        self.startup_settings
    }
    /// Withdraw version input without changing independently supplied settings.
    pub fn invalidate_startup_version(&mut self) {
        self.startup_version = None;
    }
    /// Call before further script queries when the retained input is no longer current.
    pub fn invalidate_startup_settings(&mut self) {
        self.startup_settings = None;
    }
    /// Enumerand of one selected `version` constant initialized by `ConvertVersion`.
    fn version_constant(&self, name: &str, global: &str, source: &str) -> Result<i32, Error> {
        let fail = |reason| self.inner.error(name, reason);
        let declared = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == global)
            .ok_or_else(|| fail(Failure::SettingsState("missing version constant")))?;
        if declared.source != *source {
            return Err(fail(Failure::Source));
        }
        if !declared.is_constant
            || declared.array
            || declared.ty.array
            || declared.ty.kind.name() != "version"
        {
            return Err(fail(Failure::Initializer(declared.name.clone())));
        }
        let Some(Expr::Call {
            callee, args: init, ..
        }) = &declared.init
        else {
            return Err(fail(Failure::Initializer(declared.name.clone())));
        };
        let (Expr::Ident(conversion, _), [value]) = (callee.as_ref(), init.as_slice()) else {
            return Err(fail(Failure::Initializer(declared.name.clone())));
        };
        if conversion.as_str() != "ConvertVersion" {
            return Err(fail(Failure::Initializer(declared.name.clone())));
        }
        let mut work = 128;
        super::super::integer(value, 16, &mut work).map_err(fail)
    }

    /// Frozen Throne: the client returns before reading the race. Reign of Chaos:
    /// the executed race is retained for the campaign glue screen.
    fn legacy_campaign_menu_native(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "SetCampaignMenuRace";
        let fail = |reason| self.inner.error(name, reason);
        Self::contract(&self.inner, name, &["race"], "nothing", false, None)?;
        Self::contract(&self.inner, "ConvertRace", &["integer"], "race", true, None)?;
        let source = self
            .inner
            .program
            .natives()
            .get("VersionGet")
            .ok_or_else(|| fail(Failure::Signature))?
            .source
            .clone();
        if [name, "ConvertRace"].iter().any(|n| {
            self.inner
                .program
                .natives()
                .get(n)
                .is_none_or(|n| n.source != source)
        }) {
            return Err(fail(Failure::Source));
        }
        let version = self
            .startup_version
            .ok_or_else(|| fail(Failure::SettingsState("missing explicit runtime version")))?;
        let version = self.inner.symbolic_value(version, "version")?;
        let race = match args {
            [Value::Null] => None,
            [Value::Handle(race)] => {
                self.inner
                    .symbolic_value(*race, "race")
                    .map_err(|e| fail(e.failure))?;
                Some(*race)
            }
            _ => {
                return Err(fail(Failure::SettingsState(
                    "expected race identity or null",
                )));
            }
        };
        if version == self.version_constant(name, "VERSION_FROZEN_THRONE", &source)? {
            return Ok(Value::Null);
        }
        if version == self.version_constant(name, "VERSION_REIGN_OF_CHAOS", &source)? {
            self.campaign_menu_race = Some(race);
            return Ok(Value::Null);
        }
        Err(fail(Failure::SettingsState(
            "campaign menu selection is unsupported for this runtime version",
        )))
    }

    fn extended_campaign_menu_native(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "SetCampaignMenuRaceEx";
        Self::contract(&self.inner, name, &["integer"], "nothing", false, None)?;
        let source = &self
            .inner
            .program
            .natives()
            .get(name)
            .expect("checked declaration")
            .source;
        if self
            .inner
            .program
            .natives()
            .get("VersionGet")
            .is_none_or(|n| &n.source != source)
        {
            return Err(self.inner.error(name, Failure::Source));
        }
        let [Value::Int(index)] = args else {
            return Err(self
                .inner
                .error(name, Failure::SettingsState("expected campaign index")));
        };
        self.campaign_menu_index = Some(*index);
        Ok(Value::Null)
    }

    /// Pending source-indexed glue selection. The menu retains its resolved selection.
    pub fn take_campaign_menu_index(&mut self) -> Option<i32> {
        self.startup_version.and(self.campaign_menu_index.take())
    }

    /// Executed Reign of Chaos `SetCampaignMenuRace`; `Some(None)` is a null race.
    pub fn campaign_menu_race(&self) -> Option<Option<HandleId>> {
        self.campaign_menu_race
    }
    pub(super) fn startup_settings_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if self.startup_version.is_some() {
            match name {
                "SetCampaignMenuRace" => return Some(self.legacy_campaign_menu_native(args)),
                "SetCampaignMenuRaceEx" => return Some(self.extended_campaign_menu_native(args)),
                _ => {}
            }
        }
        if matches!(name, "GetGameDifficulty" | "SetGameDifficulty") {
            self.game_difficulty?;
            return Some(match (name, args) {
                ("GetGameDifficulty", []) => {
                    Ok(Value::Handle(self.game_difficulty.expect("admitted")))
                }
                ("SetGameDifficulty", [Value::Handle(id)])
                    if self.inner.symbolic_value(*id, "gamedifficulty").is_ok() =>
                {
                    self.game_difficulty = Some(*id);
                    Ok(Value::Null)
                }
                _ => Err(self.inner.error(name, Failure::WrongFamily)),
            });
        }
        if matches!(name, "GetDefaultDifficulty" | "SetDefaultDifficulty") {
            self.default_difficulty?;
            return Some(match args {
                [] if name == "GetDefaultDifficulty" => {
                    Ok(Value::Handle(self.default_difficulty.expect("admitted")))
                }
                [Value::Handle(id)]
                    if name == "SetDefaultDifficulty"
                        && self.inner.symbolic_value(*id, "gamedifficulty").is_ok() =>
                {
                    self.default_difficulty = Some(*id);
                    Ok(Value::Null)
                }
                _ => Err(self.inner.error(name, Failure::WrongFamily)),
            });
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
            let settings = self.startup_settings;
            let locked = self
                .config
                .as_ref()
                .is_some_and(|c| c.speed_locked == Some(true));
            if settings.is_none() && !locked {
                return None;
            }
            return Some((|| {
                Self::contract(&self.inner, name, &["gamespeed"], "nothing", false, None)?;
                let source = &self
                    .inner
                    .program
                    .natives()
                    .get("CreateTimer")
                    .expect("checked Host")
                    .source;
                let conversion = self
                    .inner
                    .conversions
                    .get("ConvertGameSpeed")
                    .filter(|c| c.kind == "gamespeed" && c.source == *source)
                    .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
                if self
                    .inner
                    .program
                    .natives()
                    .get(name)
                    .expect("checked setter")
                    .source
                    != *source
                {
                    return Err(self.inner.error(name, Failure::Source));
                }
                match args {
                    [Value::Handle(speed)] => {
                        let value = self
                            .inner
                            .symbolic_value(*speed, "gamespeed")
                            .map_err(|e| self.inner.error(name, e.failure))?;
                        if !conversion.domain.contains(&value) {
                            return Err(self.inner.error(name, Failure::OutsideDomain(value)));
                        }
                        if locked || settings.is_some_and(|settings| *speed == settings.speed) {
                            Ok(Value::Null)
                        } else {
                            // shortcut: rate changes refuse until the clock consumes qualified speed rates.
                            Err(self.inner.error(
                                name,
                                Failure::SettingsState("changing game speed is unsupported"),
                            ))
                        }
                    }
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
