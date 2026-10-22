//! Name resolution and declaration checks.
//!
//! # What this builds
//!
//! A [`Program`]: every declaration in one flat environment, every identifier
//! in the *map's own* script resolved to one of them, with these checks:
//!
//! 1. an unknown type in a declaration (`type X extends Y`, `local`,
//!    parameter, return type),
//! 2. a wrong argument count where the declaration gives the arity,
//! 3. `library` `requires` / `uses` ordering.
//! 4. repeated global declarations in the map script.
//!
//! Assignment and return types are not validated here. Resolution success is
//! not proof that the original client compiles a script. Mixed numeric operands,
//! string comparisons and a use anywhere in a function resolve; the value model
//! decides their runtime behavior.
//!
//! # What one resolution unit is
//!
//! A map script is never compiled alone. The game loads the Blizzard scripts
//! beside it (`Scripts\common.j`, `Blizzard.j`, `Cheats.j`) and then the map's
//! `war3map.j`; an AI script is compiled with `common.ai`. A caller therefore
//! hands [`resolve_with`] the sources it loads the script with, and the map's
//! own declarations replace theirs, because it is loaded last.
//!
//! A name declared by two sources with the *same* signature is one
//! declaration: this machine carries two installs and each ships its own
//! `common.j`. A name declared with two *different* signatures is ambiguous,
//! and a call to it is refused with both signatures rather than resolving to
//! whichever script was loaded last. The corpus contains exactly one such name
//! (`TriggerRegisterDestDeathInRegionEvent`, `returns event` on 1.28c and
//! `returns nothing` on 1.29b).
//!
//! # Deliberate refusals
//!
//! `struct` and `library` *lowering* is JASSHelper's rewrite pass, and this
//! crate does not do it: a struct's allocate call `T.create()` reaches here as
//! a call to the type's own name, which is refused naming the rewrite. Methods
//! never reach here at all, because the parser already refuses `method`.

use crate::ast::{
    Expr, FunctionDecl, Ident, Item, LibraryDecl, Param, Script, Stmt, TypeDecl, TypeKind, TypeRef,
    VarDecl,
};
use crate::natives::{AmbiguousNative, NativeTable};
use crate::span::Span;
use crate::text::trigstr_id;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// One source the map's script is loaded with. `label` appears in diagnostics
/// that cite a declaration from this source, where a byte span into it is not a
/// line and column in the map script.
#[derive(Clone, Copy, Debug)]
pub struct Prelude<'a> {
    pub label: &'a str,
    pub script: &'a Script,
}

impl<'a> Prelude<'a> {
    pub fn new(label: &'a str, script: &'a Script) -> Prelude<'a> {
        Prelude { label, script }
    }
}

/// A function in the resolved environment. The body is the map's own AST, so
/// evaluation never needs a second, rewritten form of it.
#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: TypeRef,
    pub is_constant: bool,
    /// `None` for an empty body, which is how the parser spells both an empty
    /// function and a body-less declaration.
    pub body: Option<Vec<Stmt>>,
    pub span: Span,
    pub source: String,
}

/// A global variable, including those declared inside a `globals` block nested
/// in a `library`, a `scope` or a `struct`.
#[derive(Clone, Debug)]
pub struct Global {
    pub name: String,
    pub ty: TypeRef,
    pub array: bool,
    pub is_constant: bool,
    pub init: Option<Expr>,
    pub span: Span,
    pub source: String,
}

/// A `type X extends Y` declaration and where it sits in the handle tree.
#[derive(Clone, Debug)]
pub struct TypeInfo {
    pub name: String,
    pub parent: String,
    pub span: Span,
    pub source: String,
}

/// Where a declaration lives, for a diagnostic that wants to cite it. A span
/// in a Blizzard script is not a line and column in the map script, so it is
/// printed as a source and a byte range.
#[derive(Clone, Debug)]
pub struct DeclaredAt {
    pub source: String,
    pub span: Span,
}

impl fmt::Display for DeclaredAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}..{}", self.source, self.span.start, self.span.end)
    }
}

/// Counts a caller can print without walking the program by hand.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ResolveStats {
    pub functions: usize,
    pub globals: usize,
    pub natives: usize,
    pub types: usize,
    pub libraries: usize,
    pub scopes: usize,
    pub structs: usize,
    /// Identifiers resolved in the map's own script.
    pub uses: usize,
    /// Distinct native names the map's script calls.
    pub natives_called: usize,
    /// Of those, how many this VM can run.
    pub natives_implemented: usize,
}

/// A vJass container name, kept so a call to one is a named refusal.
#[derive(Clone, Debug)]
struct Container {
    kind: &'static str,
    span: Span,
    source_label: Option<String>,
}

/// A resolved script: everything the interpreter needs, and nothing the map did
/// not declare.
#[derive(Clone, Debug)]
pub struct Program {
    functions: Vec<Function>,
    functions_by_name: BTreeMap<String, usize>,
    globals: Vec<Global>,
    globals_by_name: BTreeMap<String, usize>,
    natives: NativeTable,
    types: BTreeMap<String, TypeInfo>,
    /// `library` load order, each library following everything it `requires`.
    library_order: Vec<String>,
    /// Native names the map's script calls, with the number of call sites.
    natives_called: BTreeMap<String, usize>,
    pub stats: ResolveStats,
}

impl Program {
    /// Every function, in declaration order. `code` values index this.
    pub fn functions(&self) -> &[Function] {
        &self.functions
    }

    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions_by_name
            .get(name)
            .map(|&i| &self.functions[i])
    }

    /// The `code` value for `name`, or `None` if it is not a function.
    pub fn function_id(&self, name: &str) -> Option<crate::value::FunctionId> {
        self.functions_by_name
            .get(name)
            .map(|&i| crate::value::FunctionId(i as u32))
    }

    pub fn globals(&self) -> &[Global] {
        &self.globals
    }

    pub fn global(&self, name: &str) -> Option<&Global> {
        self.globals_by_name.get(name).map(|&i| &self.globals[i])
    }

    pub fn natives(&self) -> &NativeTable {
        &self.natives
    }

    pub fn types(&self) -> impl Iterator<Item = &TypeInfo> {
        self.types.values()
    }

    /// `true` if `name` names a declared type.
    pub fn is_type(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    /// The `library` load order, each library after everything it `requires`.
    pub fn library_order(&self) -> &[String] {
        &self.library_order
    }

    /// Every native name the map's script calls, with its number of call sites.
    pub fn natives_called(&self) -> impl Iterator<Item = (&str, usize)> {
        self.natives_called.iter().map(|(n, c)| (n.as_str(), *c))
    }

    /// The names the map calls that no source in the load list declares, as
    /// `(name, call count)`. An unresolved name is already a resolution error,
    /// so this is empty for a program that resolved.
    pub fn unimplemented_natives(&self) -> impl Iterator<Item = (&str, usize)> {
        self.natives_called
            .iter()
            .filter_map(|(n, c)| match self.natives.call(n) {
                Some(crate::natives::NativeCall::Implemented(_, _)) => None,
                _ => Some((n.as_str(), *c)),
            })
    }
}

/// Resolve one script with nothing loaded beside it. A map script resolves
/// against the Blizzard scripts it is loaded with; see [`resolve_with`].
pub fn resolve(script: &Script) -> Result<Program, crate::Error> {
    resolve_with(script, &[])
}

/// Resolve `script` as the last source in a load list.
#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "script.resolve")
)]
pub fn resolve_with(script: &Script, prelude: &[Prelude<'_>]) -> Result<Program, crate::Error> {
    resolve_with_diagnostics(script, prelude).map_err(|diagnostic| diagnostic.error)
}

/// Resolution failure with an explicit source identity.
/// `None` identifies the main script; `Some` retains the caller's prelude label.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{error}")]
pub struct ResolveDiagnostic {
    pub source_label: Option<String>,
    #[source]
    pub error: crate::Error,
}

impl From<crate::Error> for ResolveDiagnostic {
    fn from(error: crate::Error) -> Self {
        Self {
            source_label: None,
            error,
        }
    }
}

/// Resolve as [`resolve_with`], retaining the source of declaration/graph errors.
pub fn resolve_with_diagnostics(
    script: &Script,
    prelude: &[Prelude<'_>],
) -> Result<Program, ResolveDiagnostic> {
    Resolver::new(script, prelude).run()
}

struct Resolver<'a> {
    script: &'a Script,
    prelude: &'a [Prelude<'a>],
    natives: NativeTable,
    functions: Vec<Function>,
    functions_by_name: BTreeMap<String, usize>,
    forward: BTreeMap<String, (usize, Span)>,
    globals: Vec<Global>,
    globals_by_name: BTreeMap<String, usize>,
    main_globals: BTreeSet<String>,
    types: BTreeMap<String, TypeInfo>,
    main_types: BTreeSet<String>,
    containers: BTreeMap<String, Container>,
    libraries: BTreeMap<String, LibrarySite>,
    /// Declaration order of every container, for a deterministic load order.
    container_order: Vec<String>,
    natives_called: BTreeMap<String, usize>,
    uses: usize,
    /// Errors found while collecting declarations, reported before the body
    /// walk so they are the first thing a caller sees.
    pending: Vec<ResolveDiagnostic>,
}

struct LibrarySite {
    requires: Vec<LibRef>,
}

struct LibRef {
    name: String,
    optional: bool,
    span: Span,
    source_label: Option<String>,
}

impl<'a> Resolver<'a> {
    fn new(script: &'a Script, prelude: &'a [Prelude<'a>]) -> Resolver<'a> {
        Resolver {
            script,
            prelude,
            natives: NativeTable::new(),
            functions: Vec::new(),
            functions_by_name: BTreeMap::new(),
            forward: BTreeMap::new(),
            globals: Vec::new(),
            globals_by_name: BTreeMap::new(),
            main_globals: BTreeSet::new(),
            types: BTreeMap::new(),
            main_types: BTreeSet::new(),
            containers: BTreeMap::new(),
            libraries: BTreeMap::new(),
            container_order: Vec::new(),
            natives_called: BTreeMap::new(),
            uses: 0,
            pending: Vec::new(),
        }
    }

    fn run(mut self) -> Result<Program, ResolveDiagnostic> {
        // Phase 1: declarations. The Blizzard scripts come first, the map's
        // script last, which is why a map declaration of the same name wins.
        for p in self.prelude {
            self.collect_declarations(&p.script.items, p.label, false);
        }
        self.collect_declarations(&self.script.items, "the map script", true);
        if let Some(e) = std::mem::take(&mut self.pending).into_iter().next() {
            return Err(e);
        }

        // Phase 2: the type graph and the library order.
        self.check_type_graph()?;
        let library_order = self.order_libraries()?;

        // Phase 3: the map's own script, identifier by identifier. The
        // Blizzard scripts are checked by naming them as the script, which is
        // what the corpus sweep does for each install script in turn.
        for item in &self.script.items {
            self.check_item(item)?;
        }

        let natives_implemented = self
            .natives_called
            .keys()
            .filter(|n| {
                matches!(
                    self.natives.call(n),
                    Some(crate::natives::NativeCall::Implemented(_, _))
                )
            })
            .count();
        let stats = ResolveStats {
            functions: self.functions.len(),
            globals: self.globals.len(),
            natives: self.natives.len(),
            types: self.types.len(),
            libraries: self.count_containers("library"),
            scopes: self.count_containers("scope"),
            structs: self.count_containers("struct"),
            uses: self.uses,
            natives_called: self.natives_called.len(),
            natives_implemented,
        };
        Ok(Program {
            functions: self.functions,
            functions_by_name: self.functions_by_name,
            globals: self.globals,
            globals_by_name: self.globals_by_name,
            natives: self.natives,
            types: self.types,
            library_order,
            natives_called: self.natives_called,
            stats,
        })
    }

    fn count_containers(&self, kind: &str) -> usize {
        self.containers.values().filter(|c| c.kind == kind).count()
    }

    // ----- phase 1: declarations ------------------------------------------

    fn collect_declarations(&mut self, items: &'a [Item], source: &str, main: bool) {
        let source = source.to_string();
        for item in items {
            match item {
                Item::Native(n) => {
                    self.natives.insert(n, &source);
                }
                Item::Function(f) => self.declare_function(f, &source, main),
                Item::Type(t) => self.declare_type(t, &source, main),
                Item::Globals(vars, _) => {
                    for v in vars {
                        self.declare_global(v, &source, main);
                    }
                }
                Item::Library(l) => {
                    self.declare_container(l.name.as_str(), "library", l.span, &source, main);
                    self.declare_library(l, &source, main);
                    self.collect_declarations(&l.body, &source, main);
                }
                Item::Scope(s) => {
                    self.declare_container(s.name.as_str(), "scope", s.span, &source, main);
                    self.collect_declarations(&s.body, &source, main);
                }
                Item::Struct(s) => {
                    self.declare_container(s.name.as_str(), "struct", s.span, &source, main);
                    self.collect_declarations(&s.body, &source, main);
                }
            }
        }
    }

    fn declare_container(
        &mut self,
        name: &str,
        kind: &'static str,
        span: Span,
        source: &str,
        main: bool,
    ) {
        if !self.container_order.iter().any(|n| n == name) {
            self.container_order.push(name.to_string());
        }
        self.containers
            .entry(name.to_string())
            .or_insert(Container {
                kind,
                span,
                source_label: (!main).then(|| source.to_owned()),
            });
    }

    fn declare_library(&mut self, l: &LibraryDecl, source: &str, main: bool) {
        let entry = self
            .libraries
            .entry(l.name.as_str().to_string())
            .or_insert(LibrarySite {
                requires: Vec::new(),
            });
        let mut requires = Vec::new();
        for r in &l.requires {
            requires.push(LibRef {
                name: r.name.as_str().to_string(),
                optional: r.optional,
                span: l.span,
                source_label: (!main).then(|| source.to_owned()),
            });
        }
        entry.requires = requires;
    }

    fn declare_global(&mut self, v: &'a VarDecl, source: &str, main: bool) {
        // The map's own declaration replaces a prelude one, because the map is
        // loaded last. Repeated map declarations must not silently replace one another.
        if main && !self.main_globals.insert(v.name.as_str().to_owned()) {
            self.pending.push(ResolveDiagnostic {
                source_label: None,
                error: crate::Error::resolve(
                    v.span,
                    format!(
                        "global `{}` is declared twice in the map script",
                        v.name.as_str()
                    ),
                ),
            });
            return;
        }
        if !main && self.globals_by_name.contains_key(v.name.as_str()) {
            return;
        }
        let idx = self.globals.len();
        self.globals.push(Global {
            name: v.name.as_str().to_string(),
            ty: v.ty.clone(),
            array: v.ty.array,
            is_constant: v.is_constant,
            init: v.init.clone(),
            span: v.span,
            source: source.to_string(),
        });
        self.globals_by_name
            .insert(v.name.as_str().to_string(), idx);
    }

    fn declare_type(&mut self, t: &'a TypeDecl, source: &str, main: bool) {
        if main {
            self.types.remove(t.name.as_str());
            self.main_types.insert(t.name.as_str().to_owned());
        }
        self.types
            .entry(t.name.as_str().to_string())
            .or_insert(TypeInfo {
                name: t.name.as_str().to_string(),
                parent: t.extends.as_str().to_string(),
                span: t.span,
                source: source.to_string(),
            });
    }

    /// Declare a function, linking a body-less declaration to the body a later
    /// declaration supplies. The parser spells an empty function and a
    /// body-less declaration the same way, so a lone body-less declaration is
    /// simply a function with an empty body.
    fn declare_function(&mut self, f: &'a FunctionDecl, source: &str, main: bool) {
        match f.body {
            Some(_) => {
                if let Some((i, fspan)) = self.forward.remove(f.name.as_str()) {
                    if !same_call_shape(&self.functions[i], f) {
                        self.pending.push(ResolveDiagnostic {
                            source_label: (!main).then(|| source.to_owned()),
                            error: crate::Error::resolve(
                                f.span,
                                format!(
                                    "`{}` is declared twice with different signatures: once at the map's bytes {}..{} and once here",
                                    f.name.as_str(), fspan.start, fspan.end
                                ),
                            ),
                        });
                        return;
                    }
                    self.functions[i] = declared_function(f, source);
                    return;
                }
                // A prelude declaration of the same name is left alone
                // unless it is the map's own, which is loaded last.
                if !main && self.functions_by_name.contains_key(f.name.as_str()) {
                    return;
                }
                if let Some(&i) = self.functions_by_name.get(f.name.as_str()) {
                    self.functions[i] = declared_function(f, source);
                    self.forward.remove(f.name.as_str());
                    return;
                }
                let idx = self.push_function(f, source);
                self.forward.remove(f.name.as_str());
                self.functions_by_name
                    .insert(f.name.as_str().to_string(), idx);
            }
            None => {
                if self.functions_by_name.contains_key(f.name.as_str())
                    || self.forward.contains_key(f.name.as_str())
                {
                    return;
                }
                let idx = self.push_function(f, source);
                self.forward
                    .insert(f.name.as_str().to_string(), (idx, f.span));
            }
        }
    }

    fn push_function(&mut self, f: &FunctionDecl, source: &str) -> usize {
        let idx = self.functions.len();
        self.functions.push(Function {
            name: f.name.as_str().to_string(),
            params: f.params.clone(),
            ret: f.ret.clone(),
            is_constant: f.is_constant,
            body: f.body.clone(),
            span: f.span,
            source: source.to_string(),
        });
        self.functions_by_name
            .insert(f.name.as_str().to_string(), idx);
        idx
    }

    // ----- phase 2: the type graph and the library order ------------------

    fn check_type_graph(&self) -> Result<(), ResolveDiagnostic> {
        for name in self.types.keys() {
            self.type_root(name).map_err(|error| ResolveDiagnostic {
                source_label: (!self.main_types.contains(name))
                    .then(|| self.types[name].source.clone()),
                error,
            })?;
        }
        Ok(())
    }

    /// Walk the `extends` chain from `name` to its root, refusing a chain that
    /// leaves the handle tree, a chain that never ends, and a type declared
    /// nowhere.
    fn type_root(&self, name: &str) -> Result<(), crate::Error> {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut cur = name.to_string();
        loop {
            if !seen.insert(cur.clone()) {
                let info = &self.types[name];
                return Err(crate::Error::resolve(
                    info.span,
                    format!(
                        "the `extends` chain through type `{name}` is cyclic: `{cur}` extends itself"
                    ),
                ));
            }
            // The key borrow is released here, so `cur` can move on.
            let Some(parent) = self.types.get(&cur).map(|t| t.parent.clone()) else {
                return self.type_parent_error(name, &cur);
            };
            cur = parent;
        }
    }

    fn type_parent_error(&self, name: &str, parent: &str) -> Result<(), crate::Error> {
        let span = self.types[name].span;
        match TypeKind::builtin(parent) {
            // `handle` is the root of the handle tree, the only type a
            // declaration is allowed to bottom out at.
            Some(TypeKind::Handle) => Ok(()),
            Some(_) => Err(crate::Error::resolve(
                span,
                format!("type `{name}` extends `{parent}`, which is not a handle type"),
            )),
            None => Err(crate::Error::resolve(
                span,
                format!(
                    "type `{name}` extends `{parent}`, which is not declared in this script or in the Blizzard scripts loaded with it"
                ),
            )),
        }
    }

    fn order_libraries(&self) -> Result<Vec<String>, ResolveDiagnostic> {
        // Every reference must name a declared library. An `optional` one may
        // be absent, which is what the keyword is for.
        for name in &self.container_order {
            let Some(site) = self.libraries.get(name) else {
                continue;
            };
            for r in &site.requires {
                if r.optional && !self.libraries.contains_key(&r.name) {
                    continue;
                }
                if !self.libraries.contains_key(&r.name) && !self.containers.contains_key(&r.name) {
                    return Err(ResolveDiagnostic {
                        source_label: r.source_label.clone(),
                        error: crate::Error::resolve(
                            r.span,
                            format!(
                                "library `{name}` requires `{}`, which no source declares",
                                r.name
                            ),
                        ),
                    });
                }
            }
        }
        // Depth-first, in declaration order, dependencies first. The walk is
        // deterministic: it visits `container_order`, and a cycle is reported
        // rather than resolved by picking an arbitrary entry.
        let mut out: Vec<String> = Vec::new();
        let mut done: BTreeSet<String> = BTreeSet::new();
        let mut stack: BTreeSet<String> = BTreeSet::new();
        for name in &self.container_order {
            if !self.libraries.contains_key(name) || done.contains(name) {
                continue;
            }
            self.visit_library(name, &mut out, &mut done, &mut stack, name)?;
        }
        Ok(out)
    }

    fn visit_library(
        &self,
        start: &str,
        out: &mut Vec<String>,
        done: &mut BTreeSet<String>,
        stack: &mut BTreeSet<String>,
        name: &str,
    ) -> Result<(), ResolveDiagnostic> {
        if done.contains(name) {
            return Ok(());
        }
        let Some(site) = self.libraries.get(name) else {
            // A `scope` or `struct` required by a library is only an ordering
            // edge; it is not itself in the load order.
            done.insert(name.to_string());
            return Ok(());
        };
        if !stack.insert(name.to_string()) {
            let span = self
                .containers
                .get(name)
                .map(|c| c.span)
                .unwrap_or(Span::EMPTY);
            return Err(ResolveDiagnostic {
                source_label: self
                    .containers
                    .get(name)
                    .and_then(|site| site.source_label.clone()),
                error: crate::Error::resolve(
                    span,
                    format!("library `{start}` requires itself through `{name}`"),
                ),
            });
        }
        for r in &site.requires {
            self.visit_library(start, out, done, stack, &r.name)?;
        }
        stack.remove(name);
        done.insert(name.to_string());
        out.push(name.to_string());
        Ok(())
    }

    // ----- phase 3: the map's own script ----------------------------------

    fn check_item(&mut self, item: &Item) -> Result<(), crate::Error> {
        match item {
            Item::Function(f) => {
                let mut scope = Scope::new();
                for p in &f.params {
                    self.check_tyref(&p.ty)?;
                    scope.declare(&p.name, p.ty.array, false);
                }
                if let Some(body) = &f.body {
                    self.check_stmts(body, &mut scope)?;
                }
                Ok(())
            }
            Item::Library(l) => self.check_container(&l.body),
            Item::Scope(s) => self.check_container(&s.body),
            Item::Struct(s) => self.check_container(&s.body),
            Item::Globals(vars, _) => {
                let mut scope = Scope::new();
                for v in vars {
                    self.check_tyref(&v.ty)?;
                    self.check_expr(&mut scope, v.init.as_ref())?;
                }
                Ok(())
            }
            Item::Type(_) | Item::Native(_) => Ok(()),
        }
    }

    fn check_container(&mut self, items: &[Item]) -> Result<(), crate::Error> {
        for item in items {
            self.check_item(item)?;
        }
        Ok(())
    }

    fn check_stmts(&mut self, stmts: &[Stmt], scope: &mut Scope) -> Result<(), crate::Error> {
        for st in stmts {
            match st {
                Stmt::Local(v, _) | Stmt::Constant(v, _) => {
                    self.check_tyref(&v.ty)?;
                    if let Some(init) = &v.init {
                        self.check_expr(scope, Some(init))?;
                    }
                    scope.declare(&v.name, v.ty.array, v.is_constant);
                }
                Stmt::Set { target, value, .. } => {
                    self.check_expr(scope, Some(target))?;
                    self.check_expr(scope, Some(value))?;
                    self.check_assignable(target, scope)?;
                }
                Stmt::Call { callee, args, .. } => {
                    self.check_call(callee, args.len(), callee.span, scope)?;
                    for a in args {
                        self.check_expr(scope, Some(a))?;
                    }
                }
                Stmt::Return(Some(e), _) | Stmt::ExitWhen(e, _) => {
                    self.check_expr(scope, Some(e))?;
                }
                Stmt::Return(None, _) => {}
                Stmt::Debug(inner, _) => {
                    let inner: &Stmt = inner;
                    self.check_stmts(std::slice::from_ref(inner), scope)?;
                }
                Stmt::If {
                    branches,
                    else_body,
                    ..
                } => {
                    for b in branches {
                        self.check_expr(scope, Some(&b.cond))?;
                        self.check_stmts(&b.body, scope)?;
                    }
                    if let Some(body) = else_body {
                        self.check_stmts(body, scope)?;
                    }
                }
                Stmt::Loop { body, .. } => self.check_stmts(body, scope)?,
            }
        }
        Ok(())
    }

    fn check_assignable(&mut self, target: &Expr, scope: &Scope) -> Result<(), crate::Error> {
        let Expr::Ident(name, span) = target else {
            return Ok(());
        };
        let Some((array, is_constant, _, _)) = self.variable(name, scope) else {
            return Ok(());
        };
        if is_constant {
            return Err(crate::Error::resolve(
                *span,
                format!(
                    "`{}` is a constant, and a constant cannot be assigned to",
                    name.as_str()
                ),
            ));
        }
        let _ = array;
        Ok(())
    }

    /// A variable by name: `(is_array, is_constant, index_or_zero, is_local)`.
    fn variable(&self, name: &Ident, scope: &Scope) -> Option<(bool, bool, usize, bool)> {
        if let Some(v) = scope.get(name) {
            return Some((v.array, v.is_constant, 0, true));
        }
        let &i = self.globals_by_name.get(name.as_str())?;
        Some((self.globals[i].array, self.globals[i].is_constant, i, false))
    }

    fn check_expr(&mut self, scope: &mut Scope, e: Option<&Expr>) -> Result<(), crate::Error> {
        let Some(e) = e else {
            return Ok(());
        };
        match e {
            Expr::Ident(name, span) => {
                self.uses += 1;
                self.resolve_name(name, *span, scope)
            }
            Expr::Literal(_, _) => Ok(()),
            Expr::Group(inner, _) => self.check_expr(scope, Some(inner)),
            Expr::Unary { operand, .. } => self.check_expr(scope, Some(operand)),
            Expr::Binary { lhs, rhs, .. } => {
                self.check_expr(scope, Some(lhs))?;
                self.check_expr(scope, Some(rhs))
            }
            Expr::Call { callee, args, span } => {
                let Expr::Ident(name, _) = callee.as_ref() else {
                    return Err(crate::Error::resolve(
                        *span,
                        "a call needs a name: only a function or a native can be called",
                    ));
                };
                self.uses += 1;
                self.check_call(name, args.len(), *span, scope)?;
                for argument in args {
                    self.check_expr(scope, Some(argument))?;
                }
                Ok(())
            }
            Expr::Index { base, index, .. } => {
                if let Expr::Ident(name, span) = base.as_ref() {
                    self.uses += 1;
                    if let Some((array, _, _, _)) = self.variable(name, scope) {
                        if !array {
                            return Err(crate::Error::resolve(
                                *span,
                                format!("`{}` is not an array", name.as_str()),
                            ));
                        }
                    } else if self.functions_by_name.contains_key(name.as_str())
                        || self.natives.get(name.as_str()).is_some()
                    {
                        return Err(crate::Error::resolve(
                            *span,
                            format!(
                                "`{}` is a function, and a function cannot be indexed",
                                name.as_str()
                            ),
                        ));
                    } else {
                        self.resolve_name(name, *span, scope)?;
                        return Err(crate::Error::resolve(
                            *span,
                            format!("`{}` is not an array", name.as_str()),
                        ));
                    }
                    self.check_expr(scope, Some(index))?;
                    return Ok(());
                }
                self.check_expr(scope, Some(base))?;
                self.check_expr(scope, Some(index))
            }
            Expr::FunctionRef { name, span } => {
                self.uses += 1;
                if self.functions_by_name.contains_key(name.as_str())
                    || self.natives.get(name.as_str()).is_some()
                {
                    return Ok(());
                }
                Err(crate::Error::resolve(
                    *span,
                    format!(
                        "`function {}` needs a function, and `{}` is not declared in this script or in the Blizzard scripts loaded with it",
                        name.as_str(),
                        name.as_str()
                    ),
                ))
            }
        }
    }

    /// Resolve `name` in call position, record the call, and check the argument
    /// count against the declaration the load list provides.
    fn check_call(
        &mut self,
        name: &Ident,
        argc: usize,
        span: Span,
        scope: &Scope,
    ) -> Result<(), crate::Error> {
        // A variable of the same spelling is not callable, and silently
        // skipping the call would be a map that runs wrong.
        if scope.get(name).is_some() || self.globals_by_name.contains_key(name.as_str()) {
            return Err(crate::Error::resolve(
                span,
                format!(
                    "`{}` is a variable, and a variable cannot be called",
                    name.as_str()
                ),
            ));
        }
        if let Some(c) = self.containers.get(name.as_str()) {
            return Err(crate::Error::resolve(
                span,
                format!(
                    "`{}(…)` is a {} allocate call, and how a {} becomes a scope or a handle table is JASSHelper's rewrite pass, which this crate refuses rather than guessing at",
                    name.as_str(),
                    c.kind,
                    c.kind
                ),
            ));
        }
        if self.natives.ambiguous(name.as_str()).is_some() {
            let amb = self.natives.ambiguous(name).expect("checked");
            return Err(crate::Error::resolve(
                span,
                format!(
                    "`{}` is declared with two different signatures, so which one this call means cannot be told: {}",
                    name.as_str(),
                    describe(amb)
                ),
            ));
        }
        if let Some(n) = self.natives.get(name.as_str()) {
            *self
                .natives_called
                .entry(name.as_str().to_string())
                .or_insert(0) += 1;
            if n.params.len() != argc {
                return Err(crate::Error::resolve(
                    span,
                    format!(
                        "native `{}` takes {} argument{}, {} given (declared at {})",
                        name.as_str(),
                        n.params.len(),
                        plural(n.params.len()),
                        argc,
                        DeclaredAt {
                            source: n.source.clone(),
                            span: n.span,
                        }
                    ),
                ));
            }
            return Ok(());
        }
        if let Some(&i) = self.functions_by_name.get(name.as_str()) {
            let params = self.functions[i].params.len();
            if params != argc {
                return Err(crate::Error::resolve(
                    span,
                    format!(
                        "`{}` takes {} argument{}, {} given (declared at {})",
                        name.as_str(),
                        params,
                        plural(params),
                        argc,
                        DeclaredAt {
                            source: self.functions[i].source.clone(),
                            span: self.functions[i].span,
                        }
                    ),
                ));
            }
            return Ok(());
        }
        Err(crate::Error::resolve(
            span,
            format!(
                "name `{}` is not declared in this script or in the Blizzard scripts loaded with it",
                name.as_str()
            ),
        ))
    }

    fn resolve_name(&self, name: &Ident, span: Span, scope: &Scope) -> Result<(), crate::Error> {
        if trigstr_id(name.as_str()).is_some() {
            // A trigger-string reference: resolved by the map's `war3map.wts`
            // through the host, never by a declaration.
            return Ok(());
        }
        if scope.get(name).is_some() || self.globals_by_name.contains_key(name.as_str()) {
            return Ok(());
        }
        if self.functions_by_name.contains_key(name.as_str())
            || self.natives.get(name.as_str()).is_some()
        {
            return Ok(());
        }
        let missing = || {
            crate::Error::resolve(
                span,
                format!(
                    "name `{}` is not declared in this script or in the Blizzard scripts loaded with it",
                    name.as_str()
                ),
            )
        };
        if let Some(c) = self.containers.get(name.as_str()) {
            return Err(crate::Error::resolve(
                span,
                format!(
                    "`{}` is a {}, and a {} is not a value",
                    name.as_str(),
                    c.kind,
                    c.kind
                ),
            ));
        }
        if self.types.contains_key(name.as_str()) {
            return Err(crate::Error::resolve(
                span,
                format!("`{}` is a type, and a type is not a value", name.as_str()),
            ));
        }
        Err(missing())
    }

    /// A type reference is valid when it is builtin or declared here. A
    /// `struct` name is a type as far as a declaration goes, which is the one
    /// part of the vJass rewrite that has to be half-accepted for a wrapped
    /// map to type its own variables; how the type *lowers* is still refused.
    fn check_tyref(&self, ty: &TypeRef) -> Result<(), crate::Error> {
        if let TypeKind::Named(name) = &ty.kind {
            let is_struct = self
                .containers
                .get(name)
                .is_some_and(|c| c.kind == "struct");
            if !self.types.contains_key(name) && !is_struct {
                return Err(crate::Error::resolve(
                    ty.span,
                    format!(
                        "type `{name}` is not declared in this script or in the Blizzard scripts loaded with it"
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// Every declaration of a name that resolves to two different signatures.
fn describe(amb: &AmbiguousNative) -> String {
    amb.at
        .iter()
        .map(|(source, span, sig)| format!("{sig} at {source}:{}..{}", span.start, span.end))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The declaration a function adds to the program.
fn declared_function(f: &FunctionDecl, source: &str) -> Function {
    Function {
        name: f.name.as_str().to_string(),
        params: f.params.clone(),
        ret: f.ret.clone(),
        is_constant: f.is_constant,
        body: f.body.clone(),
        span: f.span,
        source: source.to_string(),
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Whether two declarations of a function are the same call shape, so a
/// body-less declaration can be linked to the body that follows it.
fn same_call_shape(a: &Function, b: &FunctionDecl) -> bool {
    a.params.len() == b.params.len()
        && a.params
            .iter()
            .zip(&b.params)
            .all(|(x, y)| x.ty.kind == y.ty.kind && x.ty.array == y.ty.array)
        && a.ret.kind == b.ret.kind
        && a.ret.array == b.ret.array
}

#[derive(Clone, Debug)]
struct Local {
    array: bool,
    is_constant: bool,
}

/// A function's own scope: parameters, locals and constants. JASS scopes them
/// per function, not per block, so every declaration in the function is in
/// scope for every statement in it.
struct Scope {
    locals: BTreeMap<String, Local>,
}

impl Scope {
    fn new() -> Scope {
        Scope {
            locals: BTreeMap::new(),
        }
    }

    fn declare(&mut self, name: &Ident, array: bool, is_constant: bool) {
        self.locals
            .entry(name.as_str().to_string())
            .or_insert(Local { array, is_constant });
    }

    fn get(&self, name: &Ident) -> Option<&Local> {
        self.locals.get(name.as_str())
    }
}
