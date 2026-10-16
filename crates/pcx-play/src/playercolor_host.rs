//! Explicit symbolic contracts and object identities share one session-owned arena.
use pcx_script::ast::{Expr, Literal, TypeKind};
use pcx_script::natives::Native;
use pcx_script::resolve::Program;
use pcx_script::span::Span;
use pcx_script::{HandleId, Host, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;
use std::sync::Arc;
pub const NATIVE: &str = "ConvertPlayerColor";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    #[error("missing or incompatible native/type declaration")]
    Signature,
    #[error("multiple declaration sources require qualification")]
    Source,
    #[error("unsupported authored initializer {0}")]
    Initializer(String),
    #[error("no source-authored enumerands")]
    EmptyDomain,
    #[error("expected exactly one integer argument")]
    Arguments,
    #[error("arguments do not match the timer native declaration")]
    TimerArguments,
    #[error("arguments do not match the group native declaration")]
    GroupArguments,
    #[error("arguments do not match the map configuration native declaration")]
    ConfigArguments,
    #[error("map configuration: {0}")]
    ConfigState(&'static str),
    #[error("arguments do not match the player native declaration")]
    PlayerArguments,
    #[error("player setup: {0}")]
    PlayerState(&'static str),
    #[error("camera setup: {0}")]
    CameraState(&'static str),
    #[error("startup clock: {0}")]
    ClockState(&'static str),
    #[error("map music selection: {0}")]
    MusicState(&'static str),
    #[error("day/night setup: {0}")]
    DayNightState(&'static str),
    #[error("day/night model is not admitted: {0}")]
    MissingDayNightModel(String),
    #[error("sound setup: {0}")]
    SoundState(&'static str),
    #[error("sound parameter profile {0} is unsupported by the full numeric query")]
    UnsupportedSoundParameterProfile(crate::sound_parameters::Profile),
    #[error("sound label is not admitted: {0}")]
    MissingSoundLabel(String),
    #[error("sound file is not admitted: {0}")]
    MissingSoundFile(String),
    #[error("enumerand {0} is outside this native's admitted source domain")]
    OutsideDomain(i32),
    #[error("unsupported closed integer expression: {0}")]
    Expression(String),
    #[error("{0} policy exceeded")]
    Budget(&'static str),
    #[error("invalid or reserved handle type {0}")]
    Type(String),
    #[error("unknown handle identity")]
    Identity,
    #[error("handle identifies another family")]
    WrongFamily,
    #[error("arguments do not match the quest native declaration")]
    QuestArguments,
    #[error("quest state: {0}")]
    QuestState(&'static str),
    #[error("host identity capacity or allocation exceeded")]
    Capacity,
}
#[derive(Debug, thiserror::Error)]
#[error(
    "{source_context}: {native} at {declaration_source}:{declaration_span:?}, initializer {initializer:?}: {failure}"
)]
pub struct Error {
    pub source_context: String,
    pub native: String,
    pub declaration_source: String,
    pub declaration_span: Span,
    pub initializer: Option<Box<(String, Span)>>,
    pub failure: Failure,
}
fn error(context: &str, name: &str, native: Option<&Native>, failure: Failure) -> Error {
    Error {
        source_context: context.into(),
        native: name.into(),
        declaration_source: native.map_or_else(String::new, |n| n.source.clone()),
        declaration_span: native.map_or(Span::EMPTY, |n| n.span),
        initializer: None,
        failure,
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_handles: usize,
    /// Supplied-name visits (including duplicates) plus per-contract global visits.
    pub admission_steps: usize,
    pub expression_depth: usize,
    /// Aggregate evaluated AST nodes across all selected source declarations.
    pub expression_work: usize,
}
#[derive(Debug)]
pub struct Conversion {
    pub name: String,
    pub kind: String,
    pub source: String,
    pub span: Span,
    pub domain: BTreeSet<i32>,
}
#[derive(Debug)]
enum Entry {
    Symbolic {
        kind: String,
        enumerand: i32,
    },
    Object(String),
    Timer {
        live: bool,
    },
    Group {
        live: bool,
    },
    Player {
        number: i32,
    },
    Sound {
        resource: usize,
        parameters: Option<crate::sound_parameters::ParameterKey>,
        duration: Option<i32>,
        fade_in_rate: i32,
        fade_out_rate: i32,
    },
    FileSound {
        resource: usize,
        parameters: Option<crate::sound_parameters::ParameterKey>,
        duration: Option<i32>,
        looping: bool,
        is_3d: bool,
        stop_when_out_of_range: bool,
        fade_in_rate: i32,
        fade_out_rate: i32,
        eax_setting: String,
    },
    Quest {
        live: bool,
    },
    QuestItem {
        live: bool,
    },
}
pub struct EnumHost {
    program: Arc<Program>,
    source_context: String,
    conversions: BTreeMap<String, Conversion>,
    arena: Vec<Entry>,
    max_handles: usize,
}
pub type ColorHost = EnumHost;
fn below<'a>(program: &'a Program, mut name: &'a str, ancestor: &str) -> bool {
    for _ in 0..=program.types().count() {
        if name == ancestor {
            return true;
        }
        let Some(ty) = program.types().find(|ty| ty.name == name) else {
            return false;
        };
        name = &ty.parent;
    }
    false
}
fn integer(expr: &Expr, depth: usize, work: &mut usize) -> Result<i32, Failure> {
    let child_depth = depth
        .checked_sub(1)
        .ok_or(Failure::Budget("expression depth"))?;
    *work = work
        .checked_sub(1)
        .ok_or(Failure::Budget("expression work"))?;
    let value = match expr {
        Expr::Literal(Literal::Int(value), _) => return Ok(Value::wrap32(value.value)),
        Expr::Group(inner, _) => return integer(inner, child_depth, work),
        Expr::Unary { op, operand, .. } => {
            pcx_script::value::unary(*op, &Value::Int(integer(operand, child_depth, work)?))
                .map_err(|e| Failure::Expression(e.message()))?
        }
        Expr::Binary { op, lhs, rhs, .. } => pcx_script::value::binary(
            *op,
            &Value::Int(integer(lhs, child_depth, work)?),
            &Value::Int(integer(rhs, child_depth, work)?),
        )
        .map_err(|e| Failure::Expression(e.message()))?,
        _ => {
            return Err(Failure::Expression(
                "literal arithmetic only; identifiers and calls are not evaluated".into(),
            ));
        }
    };
    value
        .as_int()
        .ok_or(Failure::Expression("integer result required".into()))
}
impl EnumHost {
    /// Color-only compatibility: depth64 is a project safety policy, never game data.
    /// Source spans bound the work available to this compatibility entry point.
    pub fn checked(
        program: Arc<Program>,
        context: &str,
        max_handles: usize,
    ) -> Result<Self, Error> {
        let native = program.natives().get(NATIVE);
        if native.is_none_or(|n| n.ret.kind.name() != "playercolor") {
            return Err(error(context, NATIVE, native, Failure::Signature));
        }
        let admission_steps = program
            .globals()
            .len()
            .checked_add(1)
            .ok_or_else(|| error(context, NATIVE, native, Failure::Capacity))?;
        let work = program
            .globals()
            .iter()
            .filter_map(|g| g.init.as_ref())
            .try_fold(0usize, |sum, e| {
                sum.checked_add(usize::try_from(e.span().end.checked_sub(e.span().start)?).ok()?)
            })
            .ok_or_else(|| {
                error(
                    context,
                    NATIVE,
                    program.natives().get(NATIVE),
                    Failure::Capacity,
                )
            })?;
        Self::checked_selected(
            program,
            context,
            &[NATIVE],
            Limits {
                max_handles,
                admission_steps,
                expression_depth: 64,
                expression_work: work,
            },
        )
    }
    /// Native names are explicit caller-selected symbolic contracts, never inferred from signature shape.
    /// The Program producer must enforce MapScripts-equivalent unique, non-reserved
    /// source labels before resolution; collapsed labels cannot be authenticated here.
    pub fn checked_selected(
        program: Arc<Program>,
        context: &str,
        names: &[&str],
        limits: Limits,
    ) -> Result<Self, Error> {
        let mut conversions = BTreeMap::new();
        let mut admission_steps = limits.admission_steps;
        for name in names {
            admission_steps = admission_steps.checked_sub(1).ok_or_else(|| {
                error(
                    context,
                    name,
                    program.natives().get(name),
                    Failure::Budget("admission steps"),
                )
            })?;
            if conversions.contains_key(*name) {
                continue;
            }
            let native = program.natives().get(name);
            let fail = |failure| error(context, name, native, failure);
            let n = native.ok_or_else(|| fail(Failure::Signature))?;
            let [parameter] = n.params.as_slice() else {
                return Err(fail(Failure::Signature));
            };
            if !n.is_constant
                || program.function(name).is_some()
                || parameter.ty.kind != TypeKind::Integer
                || parameter.ty.array
                || n.ret.array
                || !matches!(n.ret.kind, TypeKind::Named(_))
                || !below(&program, n.ret.kind.name(), "handle")
            {
                return Err(fail(Failure::Signature));
            }
            if n.also_declared_in
                .iter()
                .any(|(source, _)| source != &n.source)
            {
                return Err(fail(Failure::Source));
            }
            conversions.insert(
                (*name).into(),
                Conversion {
                    name: (*name).into(),
                    kind: n.ret.kind.name().into(),
                    source: n.source.clone(),
                    span: n.span,
                    domain: BTreeSet::new(),
                },
            );
        }
        let mut work = limits.expression_work;
        for conversion in conversions.values_mut() {
            let native = program.natives().get(&conversion.name);
            let fail = |failure| error(context, &conversion.name, native, failure);
            for global in program.globals() {
                admission_steps = admission_steps
                    .checked_sub(1)
                    .ok_or_else(|| fail(Failure::Budget("admission steps")))?;
                if global.source != conversion.source || global.ty.kind.name() != conversion.kind {
                    continue;
                }
                let invalid = |failure| {
                    let mut e = fail(failure);
                    e.initializer = Some(Box::new((global.name.clone(), global.span)));
                    e
                };
                if !global.is_constant || global.array || global.ty.array {
                    return Err(invalid(Failure::Initializer(global.name.clone())));
                }
                let Some(Expr::Call { callee, args, .. }) = &global.init else {
                    return Err(invalid(Failure::Initializer(global.name.clone())));
                };
                let Expr::Ident(name, _) = callee.as_ref() else {
                    return Err(invalid(Failure::Initializer(global.name.clone())));
                };
                // Other named contracts do not widen this native's domain or install themselves.
                if name.as_str() != conversion.name {
                    continue;
                }
                let [arg] = args.as_slice() else {
                    return Err(invalid(Failure::Initializer(global.name.clone())));
                };
                let value = integer(arg, limits.expression_depth, &mut work).map_err(invalid)?;
                if !conversion.domain.contains(&value)
                    && conversion.domain.len() >= limits.max_handles
                {
                    return Err(fail(Failure::Capacity));
                }
                conversion.domain.insert(value);
            }
            if conversion.domain.is_empty() {
                return Err(fail(Failure::EmptyDomain));
            }
        }
        let mut identities = BTreeSet::new();
        for c in conversions.values() {
            for value in &c.domain {
                let key = (&c.kind, *value);
                if !identities.contains(&key) && identities.len() >= limits.max_handles {
                    return Err(error(
                        context,
                        &c.name,
                        program.natives().get(&c.name),
                        Failure::Capacity,
                    ));
                }
                identities.insert(key);
            }
        }
        Ok(Self {
            program,
            source_context: context.into(),
            conversions,
            arena: Vec::new(),
            max_handles: limits.max_handles,
        })
    }
    fn error(&self, name: &str, failure: Failure) -> Error {
        error(
            &self.source_context,
            name,
            self.program.natives().get(name),
            failure,
        )
    }
    pub fn into_vm(self) -> pcx_script::OwnedVm<Self> {
        pcx_script::OwnedVm::new_owned(Arc::clone(&self.program), self)
    }
    pub fn conversions(&self) -> impl Iterator<Item = &Conversion> {
        self.conversions.values()
    }
    pub fn domain(&self) -> impl Iterator<Item = i32> + '_ {
        self.conversions
            .get(NATIVE)
            .filter(|c| c.kind == "playercolor")
            .into_iter()
            .flat_map(|c| c.domain.iter().copied())
    }
    /// Total identities issued, including destroyed objects whose tokens cannot be reused.
    pub fn handles(&self) -> usize {
        self.arena.len()
    }
    fn id(&self, index: usize, name: &str) -> Result<HandleId, Error> {
        u64::try_from(index)
            .ok()
            .and_then(|v| v.checked_add(1))
            .and_then(NonZeroU64::new)
            .map(HandleId::new)
            .ok_or_else(|| self.error(name, Failure::Capacity))
    }
    fn allocate(&mut self, entry: Entry, name: &str) -> Result<HandleId, Error> {
        if self.arena.len() >= self.max_handles {
            return Err(self.error(name, Failure::Capacity));
        }
        let id = self.id(self.arena.len(), name)?;
        self.arena
            .try_reserve(1)
            .map_err(|_| self.error(name, Failure::Capacity))?;
        self.arena.push(entry);
        Ok(id)
    }
    pub fn reserve_identity(&mut self, kind: &str) -> Result<HandleId, Error> {
        if !below(&self.program, kind, "handle")
            || self
                .conversions
                .values()
                .any(|c| below(&self.program, kind, &c.kind))
        {
            return Err(self.error("arena", Failure::Type(kind.into())));
        }
        self.allocate(Entry::Object(kind.into()), "arena")
    }
    fn entry(&self, id: HandleId) -> Result<&Entry, Error> {
        usize::try_from(id.token() - 1)
            .ok()
            .and_then(|i| self.arena.get(i))
            .ok_or_else(|| self.error("arena", Failure::Identity))
    }
    pub fn kind(&self, id: HandleId) -> Result<&str, Error> {
        Ok(match self.entry(id)? {
            Entry::Symbolic { kind, .. } | Entry::Object(kind) => kind,
            Entry::Timer { live: true } => "timer",
            Entry::Group { live: true } => "group",
            Entry::Player { .. } => "player",
            Entry::Sound { .. } | Entry::FileSound { .. } => "sound",
            Entry::Timer { live: false } | Entry::Group { live: false } => {
                return Err(self.error("arena", Failure::Identity));
            }
            Entry::Quest { live: true } => "quest",
            Entry::QuestItem { live: true } => "questitem",
            Entry::Quest { live: false } | Entry::QuestItem { live: false } => {
                return Err(self.error("arena", Failure::Identity));
            }
        })
    }
    pub fn symbolic_value(&self, id: HandleId, kind: &str) -> Result<i32, Error> {
        match self.entry(id)? {
            Entry::Symbolic {
                kind: actual,
                enumerand,
            } if actual == kind => Ok(*enumerand),
            _ => Err(self.error("arena", Failure::WrongFamily)),
        }
    }
    pub fn player_color(&self, id: HandleId) -> Result<i32, Error> {
        let c = self
            .conversions
            .get(NATIVE)
            .filter(|c| c.kind == "playercolor")
            .ok_or_else(|| self.error(NATIVE, Failure::Signature))?;
        self.symbolic_value(id, &c.kind)
    }
    pub fn convert(&mut self, args: &[Value]) -> Result<Value, Error> {
        if self
            .conversions
            .get(NATIVE)
            .is_none_or(|c| c.kind != "playercolor")
        {
            return Err(self.error(NATIVE, Failure::Signature));
        }
        self.convert_for(NATIVE, args)
    }
    pub fn convert_for(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let c = self
            .conversions
            .get(name)
            .ok_or_else(|| self.error(name, Failure::Signature))?;
        let [Value::Int(value)] = args else {
            return Err(self.error(name, Failure::Arguments));
        };
        if !c.domain.contains(value) {
            return Err(self.error(name, Failure::OutsideDomain(*value)));
        }
        if let Some(index) = self.arena.iter().position(
            |e| matches!(e,Entry::Symbolic {kind,enumerand} if kind==&c.kind && enumerand==value),
        ) {
            return self.id(index, name).map(Value::Handle);
        }
        let kind = c.kind.clone();
        self.allocate(
            Entry::Symbolic {
                kind,
                enumerand: *value,
            },
            name,
        )
        .map(Value::Handle)
    }
}
impl Host for EnumHost {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        if self.conversions.contains_key(name) {
            self.convert_for(name, args).map_err(|e| e.to_string())
        } else {
            Err(format!(
                "{}: native `{name}` is unsupported by this host",
                self.source_context
            ))
        }
    }
}

#[cfg(test)]
#[path = "playercolor_host_tests.rs"]
mod tests;

#[path = "object_host.rs"]
mod objects;
pub use objects::{
    CameraState, DayNightState, FileSoundState, MapConfig, MapConfigBudget, MapMusicSelection,
    ObjectHost, PlayerAdmissionError, PlayerBudget, PlayerSettings, QuestBudget, QuestEvent,
    QuestFlag, SoundState, StartLocation, StartPriorities, StartPriority, StartupClockState,
    TimerHost,
};
