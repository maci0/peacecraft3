//! `scriptx` — inspect a JASS script with the pcx-script syntax layer.
//!
//! ```text
//! scriptx lex <file.j>      token stream with spans, then comments/directives
//! scriptx parse <file.j>    parse and print the tree as indented text
//! scriptx stats <file.j>    token/tree counts
//! scriptx hash <file.j>     FNV-1a 64 of the canonical, span-free AST form
//! scriptx resolve <map.j> [prelude.j ...]
//!                           resolve names in the first file, with the rest
//!                           loaded as its Blizzard scripts
//! ```
//!
//! `hash` is the cross-implementation comparison value: two parsers that
//! agree byte for byte on what they parsed produce the same number for the
//! same script, regardless of formatting. Use `-` to read the script from
//! stdin.

use std::io::Read;
use std::process::ExitCode;

use pcx_script as ps;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    let paths: Vec<String> = args.collect();
    if paths.is_empty() || matches!(cmd.as_str(), "-h" | "--help" | "help") {
        eprintln!("usage: scriptx <lex|parse|stats|hash|resolve> <file.j | -> [prelude.j ...]");
        return ExitCode::from(2);
    }
    let (main, prelude_paths) = paths.split_first().expect("at least one path");
    let src = match read_src(main) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("scriptx: {e}");
            return ExitCode::from(2);
        }
    };
    match cmd.as_str() {
        "lex" => lex(&src),
        "parse" => parse(&src),
        "stats" => stats(&src),
        "hash" => hash(&src),
        "resolve" => resolve(
            &src,
            &prelude_paths.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        ),
        other => {
            eprintln!("scriptx: unknown command `{other}`");
            ExitCode::from(2)
        }
    }
}

fn read_src(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| format!("stdin: {e}"))?;
        Ok(s)
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
    }
}

fn lex(src: &str) -> ExitCode {
    let lexed = match ps::lex(src) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}", e.render(src));
            return ExitCode::FAILURE;
        }
    };
    let map = ps::SourceMap::new(src);
    for t in &lexed.tokens {
        let (l, c) = map.line_col(t.span.start);
        let text = t.span.text(src);
        let show: String = if text.len() > 48 {
            format!("{}…", &text[..48.min(text.len())])
        } else {
            text.to_string()
        };
        println!(
            "{:>6}:{:<3} {:>6}..{:<6} {:<24} {:?}",
            l,
            c,
            t.span.start,
            t.span.end,
            t.kind.label(),
            show
        );
    }
    for d in &lexed.directives {
        let (l, c) = map.line_col(d.span.start);
        println!("directive {l}:{c} {}", d.directive.text);
    }
    for cm in &lexed.comments {
        let (l, c) = map.line_col(cm.span.start);
        println!("comment {l}:{c} {:?}", cm.text);
    }
    ExitCode::SUCCESS
}

fn parse(src: &str) -> ExitCode {
    let script = match ps::parse(src) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e.render(src));
            return ExitCode::FAILURE;
        }
    };
    let mut out = String::new();
    dump_script(&script, &mut out);
    print!("{out}");
    ExitCode::SUCCESS
}

fn stats(src: &str) -> ExitCode {
    let script = match ps::parse(src) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e.render(src));
            return ExitCode::FAILURE;
        }
    };
    let lexed = ps::lex(src).expect("lex after parse");
    let mut s = lexed.stats();
    let tree = script.stats();
    s.bytes = src.len();
    s.lines = ps::SourceMap::new(src).line_count();
    s.tokens = lexed.tokens.len();
    s.comments = lexed.comments.len();
    s.directives = lexed.directives.len();
    s.items = tree.items;
    s.globals = tree.globals;
    s.functions = tree.functions;
    s.natives = tree.natives;
    s.type_decls = tree.type_decls;
    s.libraries = tree.libraries;
    s.scopes = tree.scopes;
    s.structs = tree.structs;
    s.statements = tree.statements;
    s.expressions = tree.expressions;
    s.int_literals += tree.int_literals;
    s.real_literals += tree.real_literals;
    s.string_literals += tree.string_literals;
    s.rawcodes += tree.rawcodes;
    s.identifiers += tree.identifiers;
    println!("bytes            {}", s.bytes);
    println!("lines            {}", s.lines);
    println!("tokens           {}", s.tokens);
    println!("comments         {}", s.comments);
    println!("directives       {}", s.directives);
    println!("items            {}", s.items);
    println!("globals          {}", s.globals);
    println!("functions        {}", s.functions);
    println!("natives          {}", s.natives);
    println!("type_decls       {}", s.type_decls);
    println!("libraries        {}", s.libraries);
    println!("scopes           {}", s.scopes);
    println!("structs          {}", s.structs);
    println!("statements       {}", s.statements);
    println!("expressions      {}", s.expressions);
    println!("int_literals     {}", s.int_literals);
    println!("real_literals    {}", s.real_literals);
    println!("string_literals  {}", s.string_literals);
    println!("rawcodes         {}", s.rawcodes);
    println!("identifiers      {}", s.identifiers);
    ExitCode::SUCCESS
}

/// Resolve names in the last file, with the earlier files loaded as its
/// Blizzard scripts. Every path is parsed first, so a syntax error in a
/// prelude is a failure and not a silent empty table.
fn resolve(main_src: &str, prelude_paths: &[&str]) -> ExitCode {
    let mut prelude: Vec<ps::Script> = Vec::new();
    for p in prelude_paths {
        let text = match read_src(p) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("scriptx: {e}");
                return ExitCode::from(2);
            }
        };
        match ps::parse(&text) {
            Ok(s) => prelude.push(s),
            Err(e) => {
                eprintln!("{}", e.render(&text));
                return ExitCode::FAILURE;
            }
        }
    }
    let refs: Vec<ps::Prelude<'_>> = prelude
        .iter()
        .zip(prelude_paths.iter())
        .map(|(s, p)| ps::Prelude::new(p, s))
        .collect();
    let script = match ps::parse(main_src) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e.render(main_src));
            return ExitCode::FAILURE;
        }
    };
    match ps::resolve_with(&script, &refs) {
        Ok(p) => {
            let s = p.stats;
            println!("resolved        ok");
            println!("functions       {}", s.functions);
            println!("globals         {}", s.globals);
            println!("natives         {}", s.natives);
            println!("types           {}", s.types);
            println!("libraries       {}", s.libraries);
            println!("scopes          {}", s.scopes);
            println!("structs         {}", s.structs);
            println!("uses            {}", s.uses);
            println!("natives_called  {}", s.natives_called);
            println!("native_impls    {}", s.natives_implemented);
            let refused: Vec<(&str, usize)> = p.unimplemented_natives().collect();
            println!("native_refused  {}", refused.len());
            for (name, n) in refused.iter().take(10) {
                println!("  {name} (x{n})");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}", e.render(main_src));
            ExitCode::FAILURE
        }
    }
}

fn hash(src: &str) -> ExitCode {
    let script = match ps::parse(src) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e.render(src));
            return ExitCode::FAILURE;
        }
    };
    println!("{:016x}", script.hash());
    ExitCode::SUCCESS
}

// ----- deterministic tree dump ------------------------------------------

fn dump_script(s: &ps::Script, out: &mut String) {
    for item in &s.items {
        dump_item(item, out, 0);
    }
    for d in &s.directives {
        out.push_str(&format!("directive {:?}\n", d.directive.text));
    }
}

fn ind(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn dump_item(item: &ps::Item, out: &mut String, depth: usize) {
    match item {
        ps::Item::Globals(vars, sp) => {
            ind(out, depth);
            out.push_str(&format!("globals {}\n", sp));
            for v in vars {
                dump_var(v, out, depth + 1);
            }
        }
        ps::Item::Type(t) => {
            ind(out, depth);
            out.push_str(&format!(
                "type {} extends {} {}\n",
                t.name.as_str(),
                t.extends.name(),
                t.span
            ));
        }
        ps::Item::Native(n) => {
            ind(out, depth);
            out.push_str(&format!(
                "native{} {} takes {} returns {} {}\n",
                if n.is_constant { " constant" } else { "" },
                n.name.as_str(),
                params_text(&n.params),
                n.ret.kind.name(),
                n.span
            ));
        }
        ps::Item::Function(f) => {
            ind(out, depth);
            out.push_str(&format!(
                "function {} takes {} returns {} {}\n",
                f.name.as_str(),
                params_text(&f.params),
                f.ret.kind.name(),
                f.span
            ));
            match &f.body {
                None => {
                    ind(out, depth + 1);
                    out.push_str("<forward declaration>\n");
                }
                Some(body) => {
                    for st in body {
                        dump_stmt(st, out, depth + 1);
                    }
                }
            }
        }
        ps::Item::Library(l) => {
            ind(out, depth);
            out.push_str(&format!("library {} {}\n", l.name.as_str(), l.span));
            if !l.requires.is_empty() {
                ind(out, depth + 1);
                out.push_str(&format!("requires {}\n", lib_text(&l.requires)));
            }
            if let Some(u) = &l.uses {
                ind(out, depth + 1);
                out.push_str(&format!("uses {}\n", lib_text(u)));
            }
            for it in &l.body {
                dump_item(it, out, depth + 1);
            }
        }
        ps::Item::Scope(sc) => {
            ind(out, depth);
            out.push_str(&format!("scope {} {}\n", sc.name.as_str(), sc.span));
            for it in &sc.body {
                dump_item(it, out, depth + 1);
            }
        }
        ps::Item::Struct(st) => {
            ind(out, depth);
            out.push_str(&format!("struct {} {}\n", st.name.as_str(), st.span));
            for it in &st.body {
                dump_item(it, out, depth + 1);
            }
        }
    }
}

fn params_text(params: &[ps::Param]) -> String {
    if params.is_empty() {
        "nothing".into()
    } else {
        params
            .iter()
            .map(|p| format!("{} {}", p.ty.kind.name(), p.name.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn lib_text(v: &[ps::LibRef]) -> String {
    v.iter()
        .map(|r| {
            let mut s = String::new();
            if r.optional {
                s.push_str("optional ");
            }
            if r.public {
                s.push_str("public ");
            }
            s.push_str(r.name.as_str());
            s
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn dump_var(v: &ps::VarDecl, out: &mut String, depth: usize) {
    ind(out, depth);
    out.push_str(&format!(
        "var{}{} {} {} {}\n",
        if v.is_constant { " constant" } else { "" },
        if v.ty.array { " array" } else { "" },
        v.ty.kind.name(),
        v.name.as_str(),
        v.span
    ));
    if let Some(e) = &v.init {
        dump_expr(e, out, depth + 1);
    }
}

fn dump_stmt(st: &ps::Stmt, out: &mut String, depth: usize) {
    match st {
        ps::Stmt::Set {
            target,
            value,
            span,
        } => {
            ind(out, depth);
            out.push_str(&format!("set {span}\n"));
            dump_expr(target, out, depth + 1);
            dump_expr(value, out, depth + 1);
        }
        ps::Stmt::Call { callee, args, span } => {
            ind(out, depth);
            out.push_str(&format!("call {} {span}\n", callee.as_str()));
            for a in args {
                dump_expr(a, out, depth + 1);
            }
        }
        ps::Stmt::Return(v, span) => {
            ind(out, depth);
            out.push_str(&format!("return {span}\n"));
            if let Some(e) = v {
                dump_expr(e, out, depth + 1);
            }
        }
        ps::Stmt::If {
            branches,
            else_body,
            span,
        } => {
            ind(out, depth);
            out.push_str(&format!("if {span}\n"));
            for b in branches {
                ind(out, depth + 1);
                out.push_str(&format!("branch {}\n", b.span));
                dump_expr(&b.cond, out, depth + 2);
                for st in &b.body {
                    dump_stmt(st, out, depth + 2);
                }
            }
            if let Some(body) = else_body {
                ind(out, depth + 1);
                out.push_str("else\n");
                for st in body {
                    dump_stmt(st, out, depth + 2);
                }
            }
        }
        ps::Stmt::Loop { body, span } => {
            ind(out, depth);
            out.push_str(&format!("loop {span}\n"));
            for st in body {
                dump_stmt(st, out, depth + 1);
            }
        }
        ps::Stmt::ExitWhen(e, span) => {
            ind(out, depth);
            out.push_str(&format!("exitwhen {span}\n"));
            dump_expr(e, out, depth + 1);
        }
        ps::Stmt::Local(v, span) => {
            ind(out, depth);
            out.push_str(&format!("local {span}\n"));
            dump_var(v, out, depth + 1);
        }
        ps::Stmt::Constant(v, span) => {
            ind(out, depth);
            out.push_str(&format!("constant {span}\n"));
            dump_var(v, out, depth + 1);
        }
        ps::Stmt::Debug(inner, span) => {
            ind(out, depth);
            out.push_str(&format!("debug {span}\n"));
            dump_stmt(inner, out, depth + 1);
        }
    }
}

fn dump_expr(e: &ps::Expr, out: &mut String, depth: usize) {
    ind(out, depth);
    match e {
        ps::Expr::Ident(i, span) => out.push_str(&format!("ident {} {span}\n", i.as_str())),
        ps::Expr::Literal(l, span) => {
            out.push_str(&format!("literal {} {span}\n", ps::print::literal_text(l)))
        }
        ps::Expr::Group(inner, span) => {
            out.push_str(&format!("group {span}\n"));
            dump_expr(inner, out, depth + 1);
        }
        ps::Expr::Call { callee, args, span } => {
            out.push_str(&format!("call {span}\n"));
            dump_expr(callee, out, depth + 1);
            for a in args {
                dump_expr(a, out, depth + 1);
            }
        }
        ps::Expr::Index { base, index, span } => {
            out.push_str(&format!("index {span}\n"));
            dump_expr(base, out, depth + 1);
            dump_expr(index, out, depth + 1);
        }
        ps::Expr::Binary { op, lhs, rhs, span } => {
            out.push_str(&format!("binary {} {span}\n", op.spelling()));
            dump_expr(lhs, out, depth + 1);
            dump_expr(rhs, out, depth + 1);
        }
        ps::Expr::Unary { op, operand, span } => {
            out.push_str(&format!("unary {} {span}\n", op.spelling()));
            dump_expr(operand, out, depth + 1);
        }
        ps::Expr::FunctionRef { name, span } => {
            out.push_str(&format!("function {} {span}\n", name.as_str()))
        }
    }
}
