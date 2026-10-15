//! One session's host-owned typed identities; no delegate mints handles.
use pcx_script::ast::{Expr, Literal, TypeKind};
use pcx_script::resolve::Program;
use pcx_script::span::Span;
use pcx_script::{HandleId, Host, Value};
use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::sync::Arc;

// Native/type spellings are interface identifiers, not color contents or numeric IDs.
pub const NATIVE: &str = "ConvertPlayerColor";
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    #[error("missing or incompatible native/type declaration")]
    Signature,
    #[error("unsupported authored color initializer {0}")]
    Initializer(String),
    #[error("no source-authored color enumerands")]
    EmptyDomain,
    #[error("expected exactly one integer argument")]
    Arguments,
    #[error("color enumerand {0} is outside the admitted source domain")]
    OutsideDomain(i32),
    #[error("invalid or reserved handle type {0}")]
    Type(String),
    #[error("unknown handle identity")]
    Identity,
    #[error("handle does not identify a playercolor")]
    WrongFamily,
    #[error("host identity capacity or allocation exceeded")]
    Capacity,
}
#[derive(Debug, thiserror::Error)]
#[error("{source_context}: {declaration_source}:{declaration_span:?}: {failure}")]
pub struct Error {
    pub source_context: String,
    pub declaration_source: String,
    pub declaration_span: Span,
    pub failure: Failure,
}
#[derive(Debug)]
enum Entry {
    PlayerColor(i32),
    Object(String),
}
pub struct ColorHost {
    program: Arc<Program>,
    source_context: String,
    declaration_source: String,
    declaration_span: Span,
    color_type: String,
    domain: BTreeSet<i32>,
    arena: Vec<Entry>,
    max_handles: usize,
}
fn integer(expr: &Expr) -> Option<i32> {
    match expr {
        Expr::Literal(Literal::Int(value), _) => Some(Value::wrap32(value.value)),
        Expr::Group(inner, _) => integer(inner),
        Expr::Unary { op, operand, .. } => {
            pcx_script::value::unary(*op, &Value::Int(integer(operand)?))
                .ok()?
                .as_int()
        }
        _ => None,
    }
}
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
impl ColorHost {
    /// Source context must qualify the caller's exact common/Program choice.
    /// Only immutable constant conversion globals from that native's declaring source admit colors.
    pub fn checked(
        program: Arc<Program>,
        source_context: &str,
        max_handles: usize,
    ) -> Result<Self, Error> {
        let native = program.natives().get(NATIVE);
        let fail = |failure| Error {
            source_context: source_context.into(),
            declaration_source: native.map_or_else(String::new, |n| n.source.clone()),
            declaration_span: native.map_or(Span::EMPTY, |n| n.span),
            failure,
        };
        let native = native.ok_or_else(|| fail(Failure::Signature))?;
        let [parameter] = native.params.as_slice() else {
            return Err(fail(Failure::Signature));
        };
        let color_type = native.ret.kind.name();
        if program.function(NATIVE).is_some()
            || parameter.ty.kind != TypeKind::Integer
            || parameter.ty.array
            || native.ret.array
            || color_type != "playercolor"
            || !below(&program, color_type, "handle")
        {
            return Err(fail(Failure::Signature));
        }
        let mut domain = BTreeSet::new();
        for global in program
            .globals()
            .iter()
            .filter(|g| g.source == native.source && g.ty.kind.name() == color_type)
        {
            let invalid = || fail(Failure::Initializer(global.name.clone()));
            if !global.is_constant || global.array || global.ty.array {
                return Err(invalid());
            }
            let Some(Expr::Call { callee, args, .. }) = &global.init else {
                return Err(invalid());
            };
            let Expr::Ident(name, _) = callee.as_ref() else {
                return Err(invalid());
            };
            let [arg] = args.as_slice() else {
                return Err(invalid());
            };
            if name.as_str() != native.name {
                return Err(invalid());
            }
            let enumerand = integer(arg).ok_or_else(invalid)?;
            if !domain.contains(&enumerand) && domain.len() >= max_handles {
                return Err(fail(Failure::Capacity));
            }
            domain.insert(enumerand);
        }
        if domain.is_empty() {
            return Err(fail(Failure::EmptyDomain));
        }
        if domain.len() > max_handles {
            return Err(fail(Failure::Capacity));
        }
        Ok(Self {
            source_context: source_context.into(),
            declaration_source: native.source.clone(),
            declaration_span: native.span,
            color_type: color_type.into(),
            program,
            domain,
            arena: Vec::new(),
            max_handles,
        })
    }
    fn error(&self, failure: Failure) -> Error {
        Error {
            source_context: self.source_context.clone(),
            declaration_source: self.declaration_source.clone(),
            declaration_span: self.declaration_span,
            failure,
        }
    }
    pub fn into_vm(self) -> pcx_script::OwnedVm<Self> {
        pcx_script::OwnedVm::new_owned(Arc::clone(&self.program), self)
    }
    pub fn domain(&self) -> impl Iterator<Item = i32> + '_ {
        self.domain.iter().copied()
    }
    pub fn handles(&self) -> usize {
        self.arena.len()
    }
    fn allocate(&mut self, entry: Entry) -> Result<HandleId, Error> {
        if self.arena.len() >= self.max_handles {
            return Err(self.error(Failure::Capacity));
        }
        let token = u64::try_from(self.arena.len())
            .ok()
            .and_then(|v| v.checked_add(1))
            .and_then(NonZeroU64::new)
            .ok_or_else(|| self.error(Failure::Capacity))?;
        self.arena
            .try_reserve(1)
            .map_err(|_| self.error(Failure::Capacity))?;
        self.arena.push(entry);
        Ok(HandleId::new(token))
    }
    /// Reserves only typed identity, not a unit/object payload or any game behavior.
    /// Every future handle-producing native on this host must use this same arena.
    pub fn reserve_identity(&mut self, kind: &str) -> Result<HandleId, Error> {
        if !below(&self.program, kind, "handle") || below(&self.program, kind, &self.color_type) {
            return Err(self.error(Failure::Type(kind.into())));
        }
        self.allocate(Entry::Object(kind.into()))
    }
    fn entry(&self, id: HandleId) -> Result<&Entry, Error> {
        usize::try_from(id.token() - 1)
            .ok()
            .and_then(|i| self.arena.get(i))
            .ok_or_else(|| self.error(Failure::Identity))
    }
    pub fn kind(&self, id: HandleId) -> Result<&str, Error> {
        Ok(match self.entry(id)? {
            Entry::PlayerColor(_) => &self.color_type,
            Entry::Object(kind) => kind,
        })
    }
    pub fn player_color(&self, id: HandleId) -> Result<i32, Error> {
        match self.entry(id)? {
            Entry::PlayerColor(value) => Ok(*value),
            Entry::Object(_) => Err(self.error(Failure::WrongFamily)),
        }
    }
    pub fn convert(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [Value::Int(value)] = args else {
            return Err(self.error(Failure::Arguments));
        };
        if !self.domain.contains(value) {
            return Err(self.error(Failure::OutsideDomain(*value)));
        }
        if let Some(index) = self
            .arena
            .iter()
            .position(|entry| matches!(entry,Entry::PlayerColor(old) if old == value))
        {
            let token = u64::try_from(index)
                .ok()
                .and_then(|v| v.checked_add(1))
                .and_then(NonZeroU64::new)
                .ok_or_else(|| self.error(Failure::Capacity))?;
            return Ok(Value::Handle(HandleId::new(token)));
        }
        self.allocate(Entry::PlayerColor(*value)).map(Value::Handle)
    }
}
impl Host for ColorHost {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        if name == NATIVE {
            self.convert(args).map_err(|e| e.to_string())
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
