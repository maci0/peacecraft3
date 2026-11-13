//! Program-bound VM section of a whole-world checkpoint; active native-save calls are omitted.
use super::*;
use crate::HandleId;
use crate::checkpoint::{Budget, Reader, Writer};

const MAGIC: &[u8] = b"PC3VM\0\x02";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    Idle,
    NativeSave,
}

pub enum Reference<'a> {
    Handle {
        identity: HandleId,
        declared: Option<&'a TypeKind>,
    },
    Thread(u64),
}

#[derive(Clone, Copy)]
struct Decl<'a> {
    kind: &'a TypeKind,
    array: bool,
}
type Declarations<'a> = BTreeMap<&'a str, Vec<Decl<'a>>>;
struct Catalog<'a> {
    declarations: Declarations<'a>,
    expressions: BTreeMap<Span, &'a Expr>,
    calls: BTreeMap<Span, (&'a str, &'a [Expr])>,
}
impl<'a> Catalog<'a> {
    fn new(function: &'a crate::resolve::Function) -> Result<Self, String> {
        let mut result = Self {
            declarations: BTreeMap::new(),
            expressions: BTreeMap::new(),
            calls: BTreeMap::new(),
        };
        for p in &function.params {
            result
                .declarations
                .entry(p.name.as_str())
                .or_default()
                .push(Decl {
                    kind: &p.ty.kind,
                    array: p.ty.array,
                });
        }
        let mut statements: Vec<_> = function.body.as_deref().unwrap_or(&[]).iter().collect();
        let mut expressions = Vec::new();
        while let Some(stmt) = statements.pop() {
            match stmt {
                Stmt::Set { target, value, .. } => {
                    expressions.push(target);
                    expressions.push(value);
                }
                Stmt::Call { callee, args, span } => {
                    result.calls.insert(*span, (callee.as_str(), args));
                    expressions.extend(args);
                }
                Stmt::Return(value, _) => expressions.extend(value),
                Stmt::If {
                    branches,
                    else_body,
                    ..
                } => {
                    for branch in branches {
                        expressions.push(&branch.cond);
                        statements.extend(&branch.body);
                    }
                    if let Some(body) = else_body {
                        statements.extend(body);
                    }
                }
                Stmt::Loop { body, .. } => statements.extend(body),
                Stmt::ExitWhen(value, _) => expressions.push(value),
                Stmt::Local(var, _) | Stmt::Constant(var, _) => {
                    result
                        .declarations
                        .entry(var.name.as_str())
                        .or_default()
                        .push(Decl {
                            kind: &var.ty.kind,
                            array: var.ty.array,
                        });
                    expressions.extend(&var.init);
                }
                Stmt::Debug(stmt, _) => statements.push(stmt),
            }
        }
        while let Some(expr) = expressions.pop() {
            if let Some(previous) = result.expressions.insert(expr.span(), expr)
                && previous != expr
            {
                return Err("checkpoint source expression spans are ambiguous".into());
            }
            match expr {
                Expr::Group(expr, _) | Expr::Unary { operand: expr, .. } => expressions.push(expr),
                Expr::Call { callee, args, span } => {
                    let Expr::Ident(name, _) = callee.as_ref() else {
                        return Err("checkpoint source call has no identifier".into());
                    };
                    result.calls.insert(*span, (name.as_str(), args));
                    expressions.push(callee);
                    expressions.extend(args);
                }
                Expr::Index { base, index, .. } => {
                    expressions.push(base);
                    expressions.push(index);
                }
                Expr::Binary { lhs, rhs, .. } => {
                    expressions.push(lhs);
                    expressions.push(rhs);
                }
                Expr::Ident(..) | Expr::Literal(..) | Expr::FunctionRef { .. } => {}
            }
        }
        Ok(result)
    }
    fn expression(&self, span: Span) -> Result<&'a Expr, String> {
        self.expressions
            .get(&span)
            .copied()
            .ok_or("checkpoint expression is not in its source function".into())
    }
    fn call(&self, span: Span) -> Result<(&'a str, &'a [Expr]), String> {
        self.calls
            .get(&span)
            .copied()
            .ok_or("checkpoint call is not in its source function".into())
    }
}

fn global_declarations(program: &Program) -> Declarations<'_> {
    program
        .globals()
        .iter()
        .map(|g| {
            (
                g.name.as_str(),
                vec![Decl {
                    kind: &g.ty.kind,
                    array: g.array,
                }],
            )
        })
        .collect()
}
fn number(value: usize) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| "checkpoint index exceeds its format".into())
}
fn span_out(out: &mut Writer, span: Span) -> Result<(), String> {
    out.u32(span.start)?;
    out.u32(span.end)
}
fn span_in(input: &mut Reader<'_>) -> Result<Span, String> {
    let span = Span {
        start: input.u32()?,
        end: input.u32()?,
    };
    if span.start > span.end {
        return Err("reversed checkpoint span".into());
    }
    Ok(span)
}
fn optional_value_out(out: &mut Writer, value: Option<&Value>) -> Result<(), String> {
    out.boolean(value.is_some())?;
    if let Some(value) = value {
        out.value(value)?;
    }
    Ok(())
}
fn kind_copy(input: &mut Reader<'_>, kind: &TypeKind) -> Result<TypeKind, String> {
    if let TypeKind::Named(name) = kind {
        input.owned(name.len())?;
    }
    Ok(kind.clone())
}
fn expr_cost(expr: &Expr) -> Result<(usize, usize), String> {
    let mut pending = vec![expr];
    let mut nodes = 0usize;
    while let Some(expr) = pending.pop() {
        nodes = nodes
            .checked_add(1)
            .ok_or("checkpoint source node count overflow")?;
        match expr {
            Expr::Group(expr, _) | Expr::Unary { operand: expr, .. } => pending.push(expr),
            Expr::Call { callee, args, .. } => {
                pending.push(callee);
                pending.extend(args);
            }
            Expr::Index { base, index, .. } => {
                pending.push(base);
                pending.push(index);
            }
            Expr::Binary { lhs, rhs, .. } => {
                pending.push(lhs);
                pending.push(rhs);
            }
            _ => {}
        }
    }
    // Source references expand into cloned AST nodes and retained literal spellings.
    let bytes = nodes
        .checked_mul(std::mem::size_of::<Expr>())
        .and_then(|n| n.checked_add((expr.span().len() as usize).checked_mul(4)?))
        .ok_or("checkpoint source expansion overflow")?;
    Ok((nodes, bytes))
}
fn expr_copy(input: &mut Reader<'_>, expr: &Expr) -> Result<Expr, String> {
    let (nodes, bytes) = expr_cost(expr)?;
    input.items(nodes)?;
    input.owned(bytes)?;
    Ok(expr.clone())
}
fn expr_out_cost(out: &mut Writer, expr: &Expr) -> Result<(), String> {
    let (nodes, bytes) = expr_cost(expr)?;
    out.items(nodes)?;
    out.owned(bytes)
}
fn children(stmt: &Stmt, kind: Nest) -> Result<&[Stmt], String> {
    match (stmt, kind) {
        (Stmt::If { branches, .. }, Nest::IfBranch(index)) => branches
            .get(index)
            .map(|b| b.body.as_slice())
            .ok_or("checkpoint if branch is absent".into()),
        (
            Stmt::If {
                else_body: Some(body),
                ..
            },
            Nest::IfElse,
        )
        | (Stmt::Loop { body, .. }, Nest::Loop) => Ok(body),
        (Stmt::Debug(stmt, _), Nest::Debug) => Ok(std::slice::from_ref(stmt.as_ref())),
        _ => Err("checkpoint nested cursor differs from its source statement".into()),
    }
}
fn leaf<'a>(mut body: &'a [Stmt], mut cont: &Cont) -> Result<&'a Stmt, String> {
    loop {
        let stmt = body
            .get(cont.at)
            .ok_or("checkpoint statement cursor is outside its body")?;
        if let Some(nested) = &cont.nested {
            body = children(stmt, nested.kind)?;
            cont = &nested.cont;
        } else {
            return Ok(stmt);
        }
    }
}
fn slot_parts(slot: &Slot) -> (&TypeKind, bool) {
    match slot {
        Slot::Scalar(_, kind) => (kind, false),
        Slot::Array(_, kind) => (kind, true),
    }
}
fn frame_out(
    out: &mut Writer,
    frame: &Frame,
    declarations: &Declarations<'_>,
) -> Result<(), String> {
    out.list(frame.len())?;
    for (name, slot) in frame {
        let (kind, array) = slot_parts(slot);
        let selector = declarations
            .get(name.as_str())
            .and_then(|decls| {
                decls
                    .iter()
                    .position(|d| d.kind == kind && d.array == array)
            })
            .ok_or("checkpoint variable declaration differs from its program")?;
        out.text(name)?;
        out.u32(number(selector)?)?;
        if let TypeKind::Named(name) = kind {
            out.owned(name.len())?;
        }
        match slot {
            Slot::Scalar(value, _) => out.value(value)?,
            Slot::Array(values, _) => {
                out.list(values.len())?;
                for (index, value) in values {
                    out.i32(*index)?;
                    out.value(value)?;
                }
            }
        }
    }
    Ok(())
}

fn paused_out(
    out: &mut Writer,
    paused: &PausedExpr,
    stmt: &Stmt,
    catalog: &Catalog<'_>,
    depth: usize,
) -> Result<(), String> {
    out.depth(depth)?;
    match (&paused.dest, stmt) {
        (ExprDest::Bind { name, ty }, Stmt::Local(var, _) | Stmt::Constant(var, _))
            if var.name.as_str() == name && var.ty.kind == *ty && !var.ty.array =>
        {
            out.owned(name.len())?;
            if let TypeKind::Named(name) = ty {
                out.owned(name.len())?;
            }
            out.byte(0)?
        }
        (
            ExprDest::Set {
                target,
                rhs,
                index_span,
            },
            Stmt::Set { target: source, .. },
        ) if target == source => {
            out.byte(1)?;
            expr_out_cost(out, target)?;
            optional_value_out(out, rhs.as_ref())?;
            out.boolean(index_span.is_some())?;
            if let Some(span) = index_span {
                span_out(out, *span)?;
            }
        }
        (ExprDest::Discard, Stmt::Call { .. }) => out.byte(2)?,
        (ExprDest::Return, Stmt::Return(Some(_), _)) => out.byte(3)?,
        (ExprDest::ExitWhen { span }, Stmt::ExitWhen(source, _)) if *span == source.span() => {
            out.byte(4)?;
            span_out(out, *span)?;
        }
        (ExprDest::IfCond { branch, span }, Stmt::If { branches, .. })
            if branches
                .get(*branch)
                .is_some_and(|b| b.cond.span() == *span) =>
        {
            out.byte(5)?;
            out.u32(number(*branch)?)?;
            span_out(out, *span)?;
        }
        _ => return Err("checkpoint expression destination differs from its statement".into()),
    }
    out.list(paused.zip.len())?;
    for zip in &paused.zip {
        match zip {
            Zip::ScriptCall { span } => {
                catalog.call(*span)?;
                out.byte(0)?;
                span_out(out, *span)?;
            }
            Zip::ScriptSpawn { span } => {
                catalog.call(*span)?;
                out.byte(1)?;
                span_out(out, *span)?;
            }
            Zip::Unary { op, span } => {
                if !matches!(catalog.expression(*span)?,Expr::Unary {op:source,..} if source==op) {
                    return Err("checkpoint unary source differs".into());
                }
                out.byte(2)?;
                span_out(out, *span)?;
            }
            Zip::Left {
                op,
                span,
                left_span,
                rhs,
            } => {
                if !matches!(catalog.expression(*span)?,Expr::Binary {op:source,lhs,rhs:source_rhs,..} if source==op&&lhs.span()==*left_span&&source_rhs.as_ref()==rhs)
                {
                    return Err("checkpoint binary source differs".into());
                }
                out.byte(3)?;
                span_out(out, *span)?;
                expr_out_cost(out, rhs)?;
            }
            Zip::Right {
                op,
                span,
                left,
                rhs_span,
            } => {
                if !matches!(catalog.expression(*span)?,Expr::Binary {op:source,rhs,..} if source==op&&rhs.span()==*rhs_span)
                {
                    return Err("checkpoint binary source differs".into());
                }
                out.byte(4)?;
                span_out(out, *span)?;
                out.value(left)?;
            }
            Zip::Arg {
                callee,
                span,
                got,
                rest,
                next,
            } => {
                let (name, args) = catalog.call(*span)?;
                let after = got
                    .len()
                    .checked_add(1)
                    .ok_or("checkpoint argument index overflow")?;
                if name != callee || rest.get(*next..) != args.get(after..) {
                    return Err("checkpoint argument source differs".into());
                }
                out.byte(5)?;
                out.owned(callee.len())?;
                span_out(out, *span)?;
                out.list(got.len())?;
                for value in got {
                    out.value(value)?;
                }
                for expr in &rest[*next..] {
                    expr_out_cost(out, expr)?;
                }
            }
            Zip::Index {
                name,
                span,
                index_span,
            } => {
                if !matches!(catalog.expression(*span)?,Expr::Index {base,index,..} if matches!(base.as_ref(),Expr::Ident(source,_) if source.as_str()==name)&&index.span()==*index_span)
                {
                    return Err("checkpoint index source differs".into());
                }
                out.byte(6)?;
                out.owned(name.len())?;
                span_out(out, *span)?;
            }
            Zip::DeclaredReal { span } => {
                catalog.call(*span)?;
                out.byte(7)?;
                span_out(out, *span)?;
            }
        }
    }
    Ok(())
}
fn cont_out(
    out: &mut Writer,
    cont: &Cont,
    body: &[Stmt],
    catalog: &Catalog<'_>,
    depth: usize,
) -> Result<(), String> {
    out.depth(depth)?;
    if cont.at > body.len() {
        return Err("checkpoint statement cursor is outside its body".into());
    }
    out.u32(number(cont.at)?)?;
    match (&cont.nested, &cont.paused) {
        (None, None) => out.byte(0),
        (Some(nested), None) => {
            let stmt = body
                .get(cont.at)
                .ok_or("checkpoint nested statement is absent")?;
            let body = children(stmt, nested.kind)?;
            out.byte(1)?;
            match nested.kind {
                Nest::IfBranch(index) => {
                    out.byte(0)?;
                    out.u32(number(index)?)?;
                }
                Nest::IfElse => out.byte(1)?,
                Nest::Loop => out.byte(2)?,
                Nest::Debug => out.byte(3)?,
            }
            cont_out(out, &nested.cont, body, catalog, depth + 1)
        }
        (None, Some(paused)) => {
            out.byte(2)?;
            paused_out(
                out,
                paused,
                body.get(cont.at)
                    .ok_or("checkpoint paused statement is absent")?,
                catalog,
                depth + 1,
            )
        }
        _ => Err("checkpoint cursor has two active continuations".into()),
    }
}

struct Decode<'r, 'p, H, F> {
    input: Reader<'r>,
    program: &'p Program,
    host: &'p H,
    reference: F,
}
impl<H, F> Decode<'_, '_, H, F>
where
    F: for<'a> FnMut(&H, Reference<'a>) -> Result<(), String>,
{
    fn value(&mut self, declared: Option<&TypeKind>) -> Result<Value, String> {
        let Self {
            input,
            program,
            host,
            reference,
        } = self;
        input.value(program.functions().len(), |identity| {
            reference(host, Reference::Handle { identity, declared })
        })
    }
    fn optional_value(&mut self) -> Result<Option<Value>, String> {
        if self.input.boolean()? {
            self.value(None).map(Some)
        } else {
            Ok(None)
        }
    }
    fn frame(&mut self, declarations: &Declarations<'_>) -> Result<Frame, String> {
        let count = self.input.list()?;
        let mut frame = Frame::new();
        for _ in 0..count {
            let name = self.input.text()?;
            if frame.contains_key(&name) {
                return Err("duplicate checkpoint variable".into());
            }
            let selector = self.input.u32()? as usize;
            let decl = *declarations
                .get(name.as_str())
                .and_then(|d| d.get(selector))
                .ok_or("checkpoint variable is foreign to its program")?;
            let kind = kind_copy(&mut self.input, decl.kind)?;
            let slot = if decl.array {
                let count = self.input.list()?;
                let mut values = BTreeMap::new();
                for _ in 0..count {
                    let index = self.input.i32()?;
                    if values.contains_key(&index) {
                        return Err("duplicate checkpoint array index".into());
                    }
                    values.insert(index, self.value(Some(&kind))?);
                }
                Slot::Array(values, kind)
            } else {
                Slot::Scalar(self.value(Some(&kind))?, kind)
            };
            frame.insert(name, slot);
        }
        Ok(frame)
    }
    fn paused(
        &mut self,
        stmt: &Stmt,
        catalog: &Catalog<'_>,
        depth: usize,
    ) -> Result<PausedExpr, String> {
        self.input.depth(depth)?;
        let dest = match (self.input.byte()?, stmt) {
            (0, Stmt::Local(var, _) | Stmt::Constant(var, _)) if !var.ty.array => {
                self.input.owned(var.name.as_str().len())?;
                ExprDest::Bind {
                    name: var.name.as_str().to_owned(),
                    ty: kind_copy(&mut self.input, &var.ty.kind)?,
                }
            }
            (1, Stmt::Set { target, .. }) => {
                let rhs = self.optional_value()?;
                let index_span = if self.input.boolean()? {
                    Some(span_in(&mut self.input)?)
                } else {
                    None
                };
                if rhs.is_some() != index_span.is_some()
                    || index_span.is_some_and(
                        |span| !matches!(target,Expr::Index {index,..} if index.span()==span),
                    )
                {
                    return Err("checkpoint assignment phase differs from its target".into());
                }
                ExprDest::Set {
                    target: expr_copy(&mut self.input, target)?,
                    rhs,
                    index_span,
                }
            }
            (2, Stmt::Call { .. }) => ExprDest::Discard,
            (3, Stmt::Return(Some(_), _)) => ExprDest::Return,
            (4, Stmt::ExitWhen(source, _)) => {
                let span = span_in(&mut self.input)?;
                if span != source.span() {
                    return Err("checkpoint exit span differs".into());
                }
                ExprDest::ExitWhen { span }
            }
            (5, Stmt::If { branches, .. }) => {
                let branch = self.input.u32()? as usize;
                let span = span_in(&mut self.input)?;
                if !branches.get(branch).is_some_and(|b| b.cond.span() == span) {
                    return Err("checkpoint conditional branch differs".into());
                }
                ExprDest::IfCond { branch, span }
            }
            _ => return Err("checkpoint destination is foreign to its statement".into()),
        };
        let count = self.input.list()?;
        let mut zip = Vec::new();
        zip.try_reserve_exact(count)
            .map_err(|_| "checkpoint expression allocation failed")?;
        for _ in 0..count {
            let tag = self.input.byte()?;
            let span = span_in(&mut self.input)?;
            zip.push(match tag {
                0 | 1 => {
                    let (name, _) = catalog.call(span)?;
                    if !matches!(
                        self.program.natives().call(name),
                        Some(NativeCall::Unimplemented(_))
                    ) {
                        return Err("checkpoint callback is not a host native".into());
                    }
                    if tag == 0 {
                        Zip::ScriptCall { span }
                    } else {
                        Zip::ScriptSpawn { span }
                    }
                }
                2 => {
                    let Expr::Unary { op, .. } = catalog.expression(span)? else {
                        return Err("checkpoint unary source differs".into());
                    };
                    Zip::Unary { op: *op, span }
                }
                3 => {
                    let Expr::Binary { op, lhs, rhs, .. } = catalog.expression(span)? else {
                        return Err("checkpoint binary source differs".into());
                    };
                    Zip::Left {
                        op: *op,
                        span,
                        left_span: lhs.span(),
                        rhs: expr_copy(&mut self.input, rhs)?,
                    }
                }
                4 => {
                    let Expr::Binary { op, rhs, .. } = catalog.expression(span)? else {
                        return Err("checkpoint binary source differs".into());
                    };
                    Zip::Right {
                        op: *op,
                        span,
                        left: self.value(None)?,
                        rhs_span: rhs.span(),
                    }
                }
                5 => {
                    let (name, args) = catalog.call(span)?;
                    let count = self.input.list()?;
                    let rest = args
                        .get(
                            count
                                .checked_add(1)
                                .ok_or("checkpoint argument index overflow")?..,
                        )
                        .ok_or("checkpoint argument cursor differs from source")?;
                    let mut got = Vec::new();
                    got.try_reserve_exact(count)
                        .map_err(|_| "checkpoint argument allocation failed")?;
                    for _ in 0..count {
                        got.push(self.value(None)?);
                    }
                    let mut cloned = Vec::new();
                    cloned
                        .try_reserve_exact(rest.len())
                        .map_err(|_| "checkpoint source argument allocation failed")?;
                    for expr in rest {
                        cloned.push(expr_copy(&mut self.input, expr)?);
                    }
                    self.input.owned(name.len())?;
                    Zip::Arg {
                        callee: name.to_owned(),
                        span,
                        got,
                        rest: cloned.into(),
                        next: 0,
                    }
                }
                6 => {
                    let Expr::Index { base, index, .. } = catalog.expression(span)? else {
                        return Err("checkpoint index source differs".into());
                    };
                    let Expr::Ident(name, _) = base.as_ref() else {
                        return Err("checkpoint index name differs".into());
                    };
                    self.input.owned(name.as_str().len())?;
                    Zip::Index {
                        name: name.as_str().to_owned(),
                        span,
                        index_span: index.span(),
                    }
                }
                7 => {
                    let (name, _) = catalog.call(span)?;
                    if self
                        .program
                        .natives()
                        .get(name)
                        .is_none_or(|n| n.ret.kind != TypeKind::Real)
                    {
                        return Err("checkpoint real-return native differs".into());
                    }
                    Zip::DeclaredReal { span }
                }
                _ => return Err("unknown checkpoint expression continuation".into()),
            });
        }
        Ok(PausedExpr { dest, zip })
    }
    fn cont(&mut self, body: &[Stmt], catalog: &Catalog<'_>, depth: usize) -> Result<Cont, String> {
        self.input.depth(depth)?;
        let at = self.input.u32()? as usize;
        if at > body.len() {
            return Err("checkpoint statement cursor is outside its body".into());
        }
        let mut cont = Cont {
            at,
            nested: None,
            paused: None,
        };
        match self.input.byte()? {
            0 => {}
            1 => {
                let kind = match self.input.byte()? {
                    0 => Nest::IfBranch(self.input.u32()? as usize),
                    1 => Nest::IfElse,
                    2 => Nest::Loop,
                    3 => Nest::Debug,
                    _ => return Err("unknown checkpoint nested continuation".into()),
                };
                let child = children(
                    body.get(at)
                        .ok_or("checkpoint nested statement is absent")?,
                    kind,
                )?;
                cont.nested = Some(Box::new(Nested {
                    kind,
                    cont: self.cont(child, catalog, depth + 1)?,
                }));
            }
            2 => {
                cont.paused = Some(Box::new(
                    self.paused(
                        body.get(at)
                            .ok_or("checkpoint paused statement is absent")?,
                        catalog,
                        depth + 1,
                    )?,
                ))
            }
            _ => return Err("unknown checkpoint statement continuation".into()),
        }
        Ok(cont)
    }
    fn activation(&mut self) -> Result<Activation, String> {
        let id = self.input.u32()? as usize;
        let function = self
            .program
            .functions()
            .get(id)
            .ok_or("checkpoint activation belongs to another program")?;
        let catalog = Catalog::new(function)?;
        self.input.items(catalog_cost(&catalog)?)?;
        let body = function.body.clone().unwrap_or_default();
        let frame = self.frame(&catalog.declarations)?;
        let cont = self.cont(&body, &catalog, 0)?;
        let after_call = if self.input.boolean()? {
            Some(Box::new(self.paused(leaf(&body, &cont)?, &catalog, 0)?))
        } else {
            None
        };
        let incoming = self.optional_value()?;
        self.input.owned(
            function
                .name
                .len()
                .checked_add(function.source.len())
                .ok_or("checkpoint function name overflow")?,
        )?;
        if let TypeKind::Named(name) = &function.ret.kind {
            self.input.owned(name.len())?;
        }
        Ok(Activation {
            name: function.name.clone(),
            source: function.source.clone(),
            ret: function.ret.clone(),
            span: function.span,
            body,
            frame,
            cont,
            after_call,
            incoming,
        })
    }
}

fn catalog_cost(catalog: &Catalog<'_>) -> Result<usize, String> {
    catalog
        .expressions
        .len()
        .checked_add(catalog.calls.len())
        .and_then(|n| n.checked_add(catalog.declarations.len()))
        .ok_or("checkpoint source index overflow".into())
}
fn activation_out(
    out: &mut Writer,
    activation: &Activation,
    program: &Program,
) -> Result<(), String> {
    let id = program
        .function_id(&activation.name)
        .ok_or("checkpoint activation is foreign to its program")?;
    let function = &program.functions()[id.index()];
    if function.source != activation.source
        || function.span != activation.span
        || function.ret != activation.ret
        || function.body.as_deref().unwrap_or(&[]) != activation.body.as_ref()
    {
        return Err("checkpoint activation source differs".into());
    }
    let catalog = Catalog::new(function)?;
    out.items(catalog_cost(&catalog)?)?;
    out.owned(
        function
            .name
            .len()
            .checked_add(function.source.len())
            .ok_or("checkpoint function name overflow")?,
    )?;
    if let TypeKind::Named(name) = &function.ret.kind {
        out.owned(name.len())?;
    }
    out.u32(id.0)?;
    frame_out(out, &activation.frame, &catalog.declarations)?;
    cont_out(out, &activation.cont, &activation.body, &catalog, 0)?;
    out.boolean(activation.after_call.is_some())?;
    if let Some(paused) = &activation.after_call {
        paused_out(
            out,
            paused,
            leaf(&activation.body, &activation.cont)?,
            &catalog,
            0,
        )?;
    }
    optional_value_out(out, activation.incoming.as_ref())
}
fn parked_boundary(mut body: &[Stmt], mut cont: &Cont, program: &Program) -> Result<bool, String> {
    loop {
        if let Some(nested) = &cont.nested {
            body = children(
                body.get(cont.at)
                    .ok_or("checkpoint nested cursor is absent")?,
                nested.kind,
            )?;
            cont = &nested.cont;
            continue;
        }
        if cont.paused.is_some() {
            return Ok(true);
        }
        let Some(mut previous) = cont.at.checked_sub(1).and_then(|at| body.get(at)) else {
            return Ok(false);
        };
        while let Stmt::Debug(inner, _) = previous {
            previous = inner;
        }
        return Ok(
            matches!(previous,Stmt::Call {callee,..} if matches!(program.natives().call(callee.as_str()),Some(NativeCall::Unimplemented(_)))),
        );
    }
}
fn stack_valid(stack: &[Activation], depth: u32, program: &Program) -> Result<(), String> {
    if stack.is_empty() || stack.len() != depth as usize || depth > MAX_CALL_DEPTH {
        return Err("checkpoint parked frame depth differs".into());
    }
    for (index, frame) in stack.iter().enumerate() {
        if frame.after_call.is_some() != (index + 1 < stack.len()) {
            return Err("checkpoint parked call chain is incomplete".into());
        }
    }
    let top = stack.last().expect("nonempty above");
    if !parked_boundary(&top.body, &top.cont, program)? {
        return Err("checkpoint parked return hole is absent".into());
    }
    Ok(())
}
fn error_out(out: &mut Writer, error: Option<&crate::Error>) -> Result<(), String> {
    out.boolean(error.is_some())?;
    if let Some(error) = error {
        out.byte(match error.stage {
            crate::Stage::Lex => 0,
            crate::Stage::Parse => 1,
            crate::Stage::Resolve => 2,
            crate::Stage::Run => 3,
        })?;
        span_out(out, error.span)?;
        out.text(&error.message)?;
    }
    Ok(())
}
fn error_in(input: &mut Reader<'_>) -> Result<Option<crate::Error>, String> {
    if !input.boolean()? {
        return Ok(None);
    }
    let stage = match input.byte()? {
        0 => crate::Stage::Lex,
        1 => crate::Stage::Parse,
        2 => crate::Stage::Resolve,
        3 => crate::Stage::Run,
        _ => return Err("unknown checkpoint error stage".into()),
    };
    Ok(Some(crate::Error {
        stage,
        span: span_in(input)?,
        message: input.text()?,
    }))
}

impl<H: Host, P: Deref<Target = Program>> Vm<'_, H, P> {
    /// Encode this VM's section, never a complete game save. The world owner captures
    /// all hosts, AI contexts, scheduling and presentation in the same transaction.
    pub fn checkpoint_state_bytes(
        &self,
        source: [u8; 32],
        projection: Projection,
        budget: Budget,
    ) -> Result<Vec<u8>, String> {
        if self.busy {
            return Err("checkpoint owner has not reached a stable VM boundary".into());
        }
        match projection {
            Projection::Idle
                if !self.stack.is_empty()
                    || !self.spawns.is_empty()
                    || self.thread_entry.is_some() =>
            {
                return Err("idle checkpoint would omit active execution".into());
            }
            Projection::NativeSave if self.checkpoint_span.is_none() || self.stack.is_empty() => {
                return Err("native-save checkpoint requires its active return hole".into());
            }
            _ => {}
        }
        if self.ready == self.init_error.is_some() {
            return Err("checkpoint VM initialization state is incomplete".into());
        }
        let mut out = Writer::new(budget)?;
        out.raw(MAGIC)?;
        out.raw(&source)?;
        out.byte(if projection == Projection::Idle { 0 } else { 1 })?;
        out.boolean(self.ready)?;
        out.boolean(self.run_debug)?;
        out.u64(self.steps)?;
        error_out(&mut out, self.init_error.as_ref())?;
        out.boolean(self.error_source.is_some())?;
        if let Some(source) = &self.error_source {
            out.text(source)?;
        }
        let declarations = global_declarations(&self.program);
        if self.ready && self.globals.len() != declarations.len() {
            return Err("checkpoint globals are incomplete".into());
        }
        frame_out(&mut out, &self.globals, &declarations)?;
        out.list(self.parked.len())?;
        for (ticket, thread) in &self.parked {
            stack_valid(&thread.stack, thread.depth, &self.program)?;
            out.u64(*ticket)?;
            out.u32(thread.depth)?;
            out.list(thread.stack.len())?;
            for activation in &thread.stack {
                activation_out(&mut out, activation, &self.program)?;
            }
            out.list(thread.remaining.len())?;
            for id in &thread.remaining {
                let function = self
                    .program
                    .functions()
                    .get(id.index())
                    .ok_or("checkpoint callback is foreign to its program")?;
                if !function.params.is_empty() {
                    return Err("checkpoint remaining callback requires arguments".into());
                }
                out.u32(id.0)?;
            }
        }
        Ok(out.finish())
    }

    /// Restore only into a fresh candidate VM. Validate references against its already
    /// decoded host graph before the application publishes any part of that candidate.
    pub fn restore_checkpoint_state<F>(
        &mut self,
        bytes: &[u8],
        source: [u8; 32],
        budget: Budget,
        reference: F,
    ) -> Result<Projection, String>
    where
        F: for<'a> FnMut(&H, Reference<'a>) -> Result<(), String>,
    {
        if self.busy
            || self.ready
            || self.init_error.is_some()
            || !self.globals.is_empty()
            || !self.parked.is_empty()
            || !self.stack.is_empty()
            || !self.spawns.is_empty()
            || self.thread_entry.is_some()
            || self.checkpoint_span.is_some()
        {
            return Err("checkpoint restoration requires a fresh candidate VM".into());
        }
        let mut decode = Decode {
            input: Reader::new(bytes, budget)?,
            program: &self.program,
            host: &self.host,
            reference,
        };
        if decode.input.raw(MAGIC.len())? != MAGIC {
            return Err("unsupported VM checkpoint header".into());
        }
        if decode.input.raw(source.len())? != source {
            return Err("VM checkpoint source identity differs".into());
        }
        let projection = match decode.input.byte()? {
            0 => Projection::Idle,
            1 => Projection::NativeSave,
            _ => return Err("unknown VM checkpoint projection".into()),
        };
        let ready = decode.input.boolean()?;
        let run_debug = decode.input.boolean()?;
        let steps = decode.input.u64()?;
        let init_error = error_in(&mut decode.input)?;
        if ready == init_error.is_some() {
            return Err("checkpoint VM initialization state is incomplete".into());
        }
        let error_source = if decode.input.boolean()? {
            Some(decode.input.text()?)
        } else {
            None
        };
        let declarations = global_declarations(&self.program);
        let globals = decode.frame(&declarations)?;
        if ready && globals.len() != declarations.len() {
            return Err("checkpoint globals are incomplete".into());
        }
        let count = decode.input.list()?;
        let mut parked = BTreeMap::new();
        for _ in 0..count {
            let ticket = decode.input.u64()?;
            if parked.contains_key(&ticket) {
                return Err("duplicate checkpoint thread ticket".into());
            }
            (decode.reference)(decode.host, Reference::Thread(ticket))?;
            let depth = decode.input.u32()?;
            let frames = decode.input.list()?;
            if frames == 0 || frames != depth as usize || depth > MAX_CALL_DEPTH {
                return Err("checkpoint parked frame depth differs".into());
            }
            let mut stack = Vec::new();
            stack
                .try_reserve_exact(frames)
                .map_err(|_| "checkpoint stack allocation failed")?;
            for _ in 0..frames {
                stack.push(decode.activation()?);
            }
            stack_valid(&stack, depth, &self.program)?;
            let count = decode.input.list()?;
            let mut remaining = std::collections::VecDeque::new();
            remaining
                .try_reserve(count)
                .map_err(|_| "checkpoint callback allocation failed")?;
            for _ in 0..count {
                let id = crate::value::FunctionId(decode.input.u32()?);
                let function = self
                    .program
                    .functions()
                    .get(id.index())
                    .ok_or("checkpoint callback is foreign to its program")?;
                if !function.params.is_empty() {
                    return Err("checkpoint remaining callback requires arguments".into());
                }
                remaining.push_back(id);
            }
            parked.insert(
                ticket,
                Thread {
                    stack,
                    depth,
                    remaining,
                },
            );
        }
        decode.input.finish()?;
        self.globals = globals;
        self.parked = parked;
        self.ready = ready;
        self.run_debug = run_debug;
        self.steps = steps;
        self.init_error = init_error;
        self.error_source = error_source;
        Ok(projection)
    }
}
