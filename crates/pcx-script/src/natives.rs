//! The native table: exact declared signatures, and a dispatch that refuses
//! what it cannot prove.
//!
//! A map never declares the natives it calls. They come from the Blizzard
//! scripts the game loads beside it (`Scripts\common.j` carries 965 of them on
//! this machine, `common.ai` another 123), so the table is filled from
//! declarations rather than from a hand-copied list: a mistyped entry would be
//! the "plausible but wrong" layer this project refuses.
//!
//! # What is implemented
//!
//! Two natives, both proved from the shipped scripts rather than from their
//! names:
//!
//! - `I2R`: integer to real. `Blizzard.j`'s `ModuloReal` computes
//!   `dividend - I2R(R2I(dividend / divisor)) * divisor`, which only
//!   reproduces the ring remainder it documents when `I2R` is the exact
//!   int-to-real conversion.
//! - `R2I`: real to integer truncating toward zero. That same expression with
//!   its comment ("the result is negative, but within (-divisor..0]" when the
//!   dividend is negative) rules out a floor.
//!
//! Everything else is refused with its name and the span of the call, which
//! is what `PLAN.md` requires of a missing native. The formatting natives
//! (`I2S`, `R2S`, `R2SW`) and the string natives (`SubString`, `StringLength`,
//! `StringCase`, `StringHash`) are refused because their exact output is not
//! derivable from anything on this machine, and a slightly-wrong substring or
//! number format is exactly the failure a loud error prevents.

use crate::ast::{NativeDecl, Param, TypeRef};
use crate::span::Span;
use std::collections::BTreeMap;

/// A native's declared signature, kept exactly as the declaring script spells
/// it so an arity mismatch is reported against the map's own call.
#[derive(Clone, Debug, PartialEq)]
pub struct Native {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: TypeRef,
    pub is_constant: bool,
    pub span: Span,
    /// The source that declared it, used only in diagnostics.
    pub source: String,
    /// Other sources that declared the same name the same way, in load order.
    /// The two installs on this machine each ship a `common.j`, so a resolved
    /// name routinely has more than one of these.
    pub also_declared_in: Vec<(String, Span)>,
}

/// What the table can run. The variant is the decision; there is no
/// free-form implementation slot, so adding a native is a compile-visible
/// change to this enum and not a silent runtime guess.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NativeImpl {
    /// `I2R`, integer to real.
    IntToReal,
    /// `R2I`, real to integer, truncating toward zero.
    RealToInt,
}

/// How a call to a declared native name resolves.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NativeCall<'a> {
    /// One declaration the table cannot run.
    Unimplemented(&'a Native),
    /// One declaration the table can run.
    Implemented(&'a Native, NativeImpl),
}

/// A name declared with two different signatures.
#[derive(Clone, Debug, PartialEq)]
pub struct AmbiguousNative {
    /// Every declaration, as `(source, span, "takes … returns …")`. The span
    /// is a byte offset in that source, not a line/column in the map script.
    pub at: Vec<(String, Span, String)>,
}

/// The signatures a resolved program can dispatch to.
#[derive(Clone, Debug, Default)]
pub struct NativeTable {
    by_name: BTreeMap<String, Native>,
    ambiguous: BTreeMap<String, AmbiguousNative>,
}

/// `takes a, b returns c`, the spelling a diagnostic shows for a signature.
pub fn signature_of(name: &str, params: &[Param], ret: &TypeRef) -> String {
    use std::fmt::Write as _;
    let mut s = String::from(name);
    s.push_str(" takes ");
    if params.is_empty() {
        s.push_str("nothing");
    }
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        let _ = write!(s, "{} {}", p.ty.kind.name(), p.name.as_str());
    }
    s.push_str(" returns ");
    s.push_str(ret.kind.name());
    s
}

impl NativeTable {
    pub fn new() -> NativeTable {
        NativeTable::default()
    }

    /// Record a declaration from `source`. A name declared twice with the same
    /// signature stays one entry; a name declared with two *different*
    /// signatures becomes ambiguous rather than silently resolving to whichever
    /// declaration was loaded last.
    pub fn insert(&mut self, decl: &NativeDecl, source: &str) {
        if let Some(existing) = self.by_name.get_mut(decl.name.as_str()) {
            if same_signature(existing, decl) {
                existing
                    .also_declared_in
                    .push((source.to_string(), decl.span));
                return;
            }
            let removed = self
                .by_name
                .remove(decl.name.as_str())
                .expect("a differing declaration means the name was present");
            self.push_ambiguous(&removed, decl, source);
            return;
        }
        if let Some(amb) = self.ambiguous.get_mut(decl.name.as_str()) {
            amb.at.push((
                source.to_string(),
                decl.span,
                signature_of(&decl.name, &decl.params, &decl.ret),
            ));
            return;
        }
        self.by_name.insert(
            decl.name.as_str().to_string(),
            Native {
                name: decl.name.as_str().to_string(),
                params: decl.params.clone(),
                ret: decl.ret.clone(),
                is_constant: decl.is_constant,
                span: decl.span,
                source: source.to_string(),
                also_declared_in: Vec::new(),
            },
        );
    }

    fn push_ambiguous(&mut self, first: &Native, decl: &NativeDecl, source: &str) {
        self.ambiguous
            .entry(decl.name.as_str().to_string())
            .or_insert_with(|| AmbiguousNative { at: Vec::new() })
            .at
            .push((
                first.source.clone(),
                first.span,
                signature_of(&first.name, &first.params, &first.ret),
            ));
        let entry = self
            .ambiguous
            .get_mut(decl.name.as_str())
            .expect("just inserted");
        entry.at.push((
            source.to_string(),
            decl.span,
            signature_of(&decl.name, &decl.params, &decl.ret),
        ));
    }

    /// The entry for `name`, or `None` if nothing declared it as a native.
    pub fn get(&self, name: &str) -> Option<&Native> {
        self.by_name.get(name)
    }

    pub fn ambiguous(&self, name: &str) -> Option<&AmbiguousNative> {
        self.ambiguous.get(name)
    }

    /// How a call to `name` resolves. `None` means nothing declared the name.
    pub fn call(&self, name: &str) -> Option<NativeCall<'_>> {
        let Some(decl) = self.by_name.get(name) else {
            return None;
        };
        Some(match implemented(name) {
            Some(impl_) => NativeCall::Implemented(decl, impl_),
            None => NativeCall::Unimplemented(decl),
        })
    }

    /// Number of distinct declared native names.
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/// The natives this table can run, keyed by the exact spelled name. Vanilla is
/// case-sensitive, so this is an exact match and nothing else.
pub fn implemented(name: &str) -> Option<NativeImpl> {
    match name {
        "I2R" => Some(NativeImpl::IntToReal),
        "R2I" => Some(NativeImpl::RealToInt),
        _ => None,
    }
}

fn same_signature(a: &Native, b: &NativeDecl) -> bool {
    a.is_constant == b.is_constant
        && a.ret.kind == b.ret.kind
        && a.ret.array == b.ret.array
        && a.params.len() == b.params.len()
        && a.params
            .iter()
            .zip(&b.params)
            .all(|(x, y)| x.ty.kind == y.ty.kind && x.ty.array == y.ty.array)
}
