//! Tests: grammar corners, the round-trip proof, and the real-map sweep.
//!
//! The round-trip discipline is the same one the asset decoders use: for
//! every sample `s`, `parse(s).to_source()` must re-lex to the same token
//! kinds and re-parse to the same tree, compared on the span-free canonical
//! encoding (and therefore the same FNV-1a [`Script::hash`]). That is the
//! strongest correctness check available at the syntax layer: the printed
//! form can only differ in trivia, and the tree must not.

use crate::lexer::{CommentKind, IntBase, StrPart};
use crate::resolve::{Prelude, resolve_with};
use crate::span::SourceMap;
use crate::{BinOp, Expr, Item, Literal, Script, Stmt, UnOp, fnv1a64, lex, parse};

// ----- helpers -----------------------------------------------------------

/// The full round-trip proof for one source: re-lex equality of token kinds,
/// re-parse equality of the canonical encoding, print idempotence, and hash
/// stability.
#[track_caller]
fn roundtrip(src: &str) -> Script {
    let first = parse(src).unwrap_or_else(|e| panic!("parse failed: {}\n{src}", e.render(src)));
    let printed = first.to_source();
    let lexed1 = lex(src).unwrap();
    let lexed2 = lex(&printed).unwrap_or_else(|e| panic!("re-lex failed: {}", e.render(&printed)));
    assert_eq!(
        kinds(&lexed1.tokens),
        kinds(&lexed2.tokens),
        "token kinds changed by printing:\n--- src ---\n{src}\n--- printed ---\n{printed}"
    );
    let second = parse(&printed)
        .unwrap_or_else(|e| panic!("re-parse failed: {}\n{printed}", e.render(&printed)));
    assert_eq!(
        first.canonical_bytes(),
        second.canonical_bytes(),
        "canonical form changed by printing:\n--- src ---\n{src}\n--- printed ---\n{printed}"
    );
    assert_eq!(first.hash(), second.hash(), "hash changed by printing");
    let printed2 = second.to_source();
    assert_eq!(printed, printed2, "printing is not idempotent");
    first
}

fn kinds(tokens: &[crate::Token]) -> Vec<String> {
    tokens.iter().map(|t| t.kind.label()).collect()
}

#[track_caller]
fn parse_ok(src: &str) -> Script {
    parse(src).unwrap_or_else(|e| panic!("expected parse to succeed: {}\n{src}", e.render(src)))
}

#[track_caller]
fn parse_err(src: &str) -> String {
    match parse(src) {
        Ok(_) => panic!("expected parse to fail:\n{src}"),
        Err(e) => e.message,
    }
}

#[track_caller]
fn lex_err(src: &str) -> String {
    match lex(src) {
        Ok(_) => panic!("expected lex to fail:\n{src}"),
        Err(e) => e.message,
    }
}

/// The one function body of the script, as statements.
#[track_caller]
fn body(src: &str) -> Vec<Stmt> {
    let s = parse_ok(src);
    match &s.items[0] {
        Item::Function(f) => f.body.clone().unwrap_or_default(),
        other => panic!("expected a function, got {other:?}"),
    }
}

/// The single expression of `function f takes nothing returns nothing / return <expr>`.
#[track_caller]
fn expr_of(src: &str) -> Expr {
    let b = body(src);
    match &b[0] {
        Stmt::Return(Some(e), _) => e.clone(),
        other => panic!("expected a return, got {other:?}"),
    }
}

// ----- lexer: keywords, names, rawcodes ----------------------------------

#[test]
fn keywords_are_case_sensitive() {
    // Vanilla keywords are lowercase-only: the game's own common.j declares
    // `constant boolean TRUE = true`, which needs `TRUE` to be an ordinary
    // identifier and uppercase `TRUE`/`FALSE` not to be keywords.
    roundtrip(
        "globals\n    constant boolean TRUE = true\n    constant boolean FALSE = false\nendglobals\n",
    );
    assert!(parse("FUNCTION Foo TAKES NOTHING RETURNS NOTHING\nENDFUNCTION\n").is_err());
    assert!(
        parse("function Foo takes nothing returns nothing\nCALL DoNothing()\nendfunction\n")
            .is_err()
    );
    // …and a name may be spelled like a keyword's uppercase form.
    roundtrip("function f takes nothing returns nothing\n    local integer If = 1\nendfunction\n");
}

#[test]
fn identifiers_keep_their_spelling() {
    let s = parse_ok(
        "function Foo takes nothing returns nothing\n    local integer MyVar = 0\nendfunction\n",
    );
    match &s.items[0] {
        Item::Function(f) => match &f.body.as_ref().unwrap()[0] {
            Stmt::Local(v, _) => assert_eq!(v.name.as_str(), "MyVar"),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn alias_is_reserved() {
    assert!(
        parse_err("function alias takes nothing returns nothing\nendfunction\n")
            .contains("identifier")
    );
}

#[test]
fn rawcodes_one_or_four_characters() {
    // 4cc and the 1-character ASCII spelling both appear in vanilla.
    roundtrip(
        "globals\n    constant integer A = 'DTep'\n    constant integer B = 'A'\nendglobals\n",
    );
    assert!(lex_err("globals\n    constant integer A = 'abc'\nendglobals\n").contains("1 or 4"));
    assert!(lex_err("globals\n    constant integer A = 'abcde'\nendglobals\n").contains("1 or 4"));
}

#[test]
fn rawcode_single_escape_form() {
    let s = parse_ok("globals\n    constant integer NL = '\\n'\nendglobals\n");
    match &s.globals()[0].init {
        Some(Expr::Literal(Literal::Raw(r), _)) => assert_eq!(r.text, "\\n"),
        other => panic!("{other:?}"),
    }
    roundtrip("globals\n    constant integer NL = '\\n'\nendglobals\n");
}

#[test]
fn rawcodes_are_case_sensitive_values() {
    let a = parse_ok("globals\n    constant integer A = 'A000'\nendglobals\n");
    let b = parse_ok("globals\n    constant integer A = 'a000'\nendglobals\n");
    assert_ne!(
        a.hash(),
        b.hash(),
        "'A000' and 'a000' are different object ids"
    );
}

// ----- lexer: numbers ----------------------------------------------------

#[test]
fn integer_bases() {
    let s = parse_ok(
        "globals\n    constant integer A = 255\n    constant integer B = $FF\n    constant integer C = 0xFF\n    constant integer D = 012\n    constant integer E = %1010\nendglobals\n",
    );
    let vals: Vec<(i64, IntBase)> = s
        .globals()
        .iter()
        .map(|v| match &v.init {
            Some(Expr::Literal(Literal::Int(i), _)) => (i.value, i.base),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        vals,
        vec![
            (255, IntBase::Dec),
            (255, IntBase::Hex),
            (255, IntBase::Hex),
            (10, IntBase::Octal),
            (10, IntBase::Bin),
        ]
    );
    roundtrip(
        "globals\n    constant integer A = 255\n    constant integer B = $FF\n    constant integer C = 0xFF\n    constant integer D = 012\n    constant integer E = %1010\nendglobals\n",
    );
}

#[test]
fn hex_and_binary_prefixes_are_distinct() {
    // The named spellings: `$AB12` is hex, and `$1010` is hex 0x1010 — never
    // binary. Only `%` spells binary, so `%1010` is 10 while `$1010` is 4112
    // and bare `1010` is decimal. A lexer that read `$1010` as binary would
    // silently compute 10 where the map wrote 4112.
    let s = parse_ok(
        "globals\n    constant integer A = $AB12\n    constant integer B = $1010\n    constant integer C = %1010\n    constant integer D = 1010\nendglobals\n",
    );
    let vals: Vec<(i64, IntBase)> = s
        .globals()
        .iter()
        .map(|v| match &v.init {
            Some(Expr::Literal(Literal::Int(i), _)) => (i.value, i.base),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        vals,
        vec![
            (0xAB12, IntBase::Hex),
            (0x1010, IntBase::Hex),
            (10, IntBase::Bin),
            (1010, IntBase::Dec),
        ]
    );
    roundtrip(
        "globals\n    constant integer A = $AB12\n    constant integer B = $1010\n    constant integer C = %1010\n    constant integer D = 1010\nendglobals\n",
    );
}

#[test]
fn octal_rejects_8_and_9() {
    assert!(lex_err("globals\n    constant integer A = 08\nendglobals\n").contains("octal"));
    assert!(lex_err("globals\n    constant integer A = 0999\nendglobals\n").contains("octal"));
    // A real may carry leading zeroes; only integer notation is octal.
    roundtrip("globals\n    constant real R = 08.5\nendglobals\n");
}

#[test]
fn percent_is_binary_literal_or_modulus() {
    // Operand position: a binary literal.
    let e = expr_of("function f takes nothing returns nothing\n    return %1010\nendfunction\n");
    assert!(
        matches!(&e, Expr::Literal(Literal::Int(i), _) if i.value == 10 && i.base == IntBase::Bin)
    );
    // Operator position: integer modulus (the engine's `NodeMod`).
    let e = expr_of("function f takes integer a returns integer\n    return a % 10\nendfunction\n");
    assert!(
        matches!(
            &e,
            Expr::Binary {
                op: BinOp::Modulo,
                ..
            }
        ),
        "expected modulus, got {e:?}"
    );
    roundtrip("function f takes integer a returns integer\n    return a % 10\nendfunction\n");
}

#[test]
fn real_spellings() {
    roundtrip(
        "globals\n    constant real A = 1.50\n    constant real B = 1.\n    constant real C = .5\n    constant real D = 1e3\n    constant real E = 1.5e-2\nendglobals\n",
    );
    let s = parse_ok("globals\n    constant real D = 1e3\nendglobals\n");
    match &s.globals()[0].init {
        Some(Expr::Literal(Literal::Real(r), _)) => {
            assert_eq!(r.text, "1e3");
            assert_eq!(r.value, 1000.0);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn no_underscores_in_numbers() {
    // `1_000` is `1` followed by a stray `_`, which vanilla rejects outright.
    assert!(
        lex_err("globals\n    constant integer A = 1_000\nendglobals\n")
            .contains("unexpected byte")
    );
}

// ----- lexer: strings ----------------------------------------------------

#[test]
fn string_escapes_are_strict() {
    roundtrip(
        r#"globals
    constant string A = "q\"q"
    constant string B = "back\\slash"
    constant string C = "\b\t\r\n\f"
endglobals
"#,
    );
    // Any other escape is a lex error, as pjass enforces; no shipped script
    // needs one.
    assert!(
        lex_err("globals\n    constant string A = \"C:\\dir\"\nendglobals\n")
            .contains("invalid escape")
    );
}

#[test]
fn string_newlines_are_legal() {
    roundtrip("globals\n    constant string A = \"line one\nline two\"\nendglobals\n");
}

#[test]
fn control_codes_stay_byte_for_byte() {
    let s = parse_ok("globals\n    constant string A = \"|cff0000ffTip|r|nnext\"\nendglobals\n");
    match &s.globals()[0].init {
        Some(Expr::Literal(Literal::Str(l), _)) => {
            assert_eq!(l.decoded(), "|cff0000ffTip|r|nnext");
        }
        other => panic!("{other:?}"),
    }
    roundtrip("globals\n    constant string A = \"|cff0000ffTip|r|nnext\"\nendglobals\n");
}

#[test]
fn code_tags_only_when_paired() {
    // Paired: recorded structurally.
    let s = parse_ok("globals\n    constant string A = \"a ^^code^^ b\"\nendglobals\n");
    match &s.globals()[0].init {
        Some(Expr::Literal(Literal::Str(l), _)) => {
            assert_eq!(
                l.parts,
                vec![
                    StrPart::Text("a ".into()),
                    StrPart::CodeTag {
                        text: "code".into()
                    },
                    StrPart::Text(" b".into()),
                ]
            );
        }
        other => panic!("{other:?}"),
    }
    roundtrip("globals\n    constant string A = \"a ^^code^^ b\"\nendglobals\n");

    // Unpaired: literal text. Shipped maps end strings with smiley `^^`;
    // swallowing the rest of the string there would silently corrupt them.
    let s = parse_ok("globals\n    constant string A = \"Play often ^^|r\"\nendglobals\n");
    match &s.globals()[0].init {
        Some(Expr::Literal(Literal::Str(l), _)) => {
            assert_eq!(
                l.parts,
                vec![StrPart::Text("Play often ^^|r".into())],
                "an unpaired ^^ must stay literal"
            );
        }
        other => panic!("{other:?}"),
    }
    roundtrip("globals\n    constant string A = \"Play often ^^|r\"\nendglobals\n");
    roundtrip("globals\n    constant string A = \"i hope you winn^^\"\nendglobals\n");
}

#[test]
fn string_nul_and_ff_rejected() {
    assert!(lex_err("globals\n    constant string A = \"a\0b\"\nendglobals\n").contains("NUL"));
}

// ----- lexer: comments and directives ------------------------------------

#[test]
fn comments_both_forms_kept() {
    let s =
        parse_ok("// line\n/* block */\nfunction f takes nothing returns nothing\nendfunction\n");
    assert_eq!(s.comments.len(), 2);
    assert_eq!(s.comments[0].kind, CommentKind::Line);
    assert_eq!(s.comments[1].kind, CommentKind::Block);
    let kept = s.to_source_with(crate::PrintOptions {
        indent: "    ",
        comments: crate::CommentPolicy::Keep,
    });
    assert!(kept.contains("// line"));
    assert!(kept.contains("/* block */"));
}

#[test]
fn directive_forms_are_recorded_and_reemitted() {
    let s = parse_ok(
        "#define FOO 1\n//! textmacro T\n//#strict\nfunction f takes nothing returns nothing\nendfunction\n",
    );
    let names: Vec<&str> = s
        .directives
        .iter()
        .map(|d| d.directive.name.as_str())
        .collect();
    assert_eq!(names, vec!["define", "textmacro", "strict"]);
    assert_eq!(s.directives[1].directive.args, "T");
    // Trivia: they shape no declarations.
    assert_eq!(s.items.len(), 1);
    roundtrip(
        "#define FOO 1\n//! textmacro T\n//#strict\nfunction f takes nothing returns nothing\nendfunction\n",
    );
}

#[test]
fn directive_text_is_verbatim_on_the_print() {
    let s =
        parse_ok("#define   FOO   bar  \nfunction f takes nothing returns nothing\nendfunction\n");
    let printed = s.to_source();
    assert!(printed.contains("#define   FOO   bar"), "got: {printed}");
}

// ----- parser: declarations ----------------------------------------------

#[test]
fn globals_shapes() {
    let s = parse_ok(
        "globals\n    constant integer MAX = 10\n    integer array scores\n    player user = Player(1)\n    string name\nendglobals\n",
    );
    assert_eq!(s.globals().len(), 4);
    assert!(s.globals()[0].is_constant);
    assert!(s.globals()[1].ty.array);
    assert!(s.globals()[2].init.is_some());
    assert!(s.globals()[3].init.is_none());
    roundtrip(
        "globals\n    constant integer MAX = 10\n    integer array scores\n    player user = Player(1)\n    string name\nendglobals\n",
    );
}

#[test]
fn constant_array_is_rejected() {
    assert!(parse_err("globals\n    constant integer array a\nendglobals\n").contains("array"));
    assert!(parse_err("function f takes nothing returns nothing\n    local constant integer array a\nendfunction\n")
        .contains("array"));
}

#[test]
fn type_requires_extends() {
    roundtrip("type unit extends widget\ntype foo extends handle\n");
    assert!(parse_err("type foo\n").contains("extends"));
}

#[test]
fn native_shapes() {
    roundtrip(
        "native Foo takes integer a, real b returns nothing\nconstant native GetBJMaxPlayers takes nothing returns integer\n",
    );
}

#[test]
fn function_shapes() {
    roundtrip(
        "function A takes nothing returns nothing\nendfunction\nfunction B takes integer a, player p returns real\n    return 1.0\nendfunction\nconstant function C takes nothing returns integer\n    return 2\nendfunction\n",
    );
    // `returns` is required, even when the value is `nothing`.
    assert!(parse_err("function A takes nothing\nendfunction\n").contains("returns"));
}

#[test]
fn library_scope_struct_shapes() {
    roundtrip(
        "library Foo requires Bar, optional Baz uses Qux initializer initFoo\n    function inLib takes nothing returns nothing\n    endfunction\nendlibrary\nscope S initializer initS\n    function inScope takes nothing returns nothing\n    endfunction\nendscope\nstruct T\n    function inStruct takes nothing returns nothing\n    endfunction\nendstruct\n",
    );
}

#[test]
fn vjass_methods_refused_loudly() {
    assert!(
        !parse("struct T\n    method m takes nothing returns nothing\n    endmethod\nendstruct\n")
            .is_ok()
    );
    assert!(!parse("struct T\n    private method m takes nothing returns nothing\n    endmethod\nendstruct\n").is_ok());
}

// ----- parser: statements ------------------------------------------------

#[test]
fn statements_round_trip() {
    roundtrip(
        "function f takes integer a returns integer\n    local integer i = 0\n    local integer array buf\n    set i = a\n    set buf[i] = i + 1\n    call DoNothing()\n    if (a > 0) then\n        set i = 1\n    elseif (a == 0) then\n        set i = 2\n    else\n        set i = 3\n    endif\n    loop\n        exitwhen i > 10\n        set i = i + 1\n    endloop\n    debug call DoNothing()\n    debug set i = 0\n    return i\nendfunction\n",
    );
}

#[test]
fn debug_wraps_whole_statements() {
    // Blizzard's own common.ai writes `debug call …`.
    let b =
        body("function f takes nothing returns nothing\n    debug call DoNothing()\nendfunction\n");
    match &b[0] {
        Stmt::Debug(inner, _) => assert!(matches!(&**inner, Stmt::Call { .. })),
        other => panic!("{other:?}"),
    }
    // Including block statements: `debug if … then` with its own lines.
    let b = body(
        "function f takes nothing returns nothing\n    debug if (true) then\n        call DoNothing()\n    endif\nendfunction\n",
    );
    match &b[0] {
        Stmt::Debug(inner, _) => assert!(matches!(&**inner, Stmt::If { .. })),
        other => panic!("{other:?}"),
    }
    roundtrip(
        "function f takes nothing returns nothing\n    debug if (true) then\n        call DoNothing()\n    endif\nendfunction\n",
    );
}

#[test]
fn constant_statements() {
    let b = body(
        "function f takes nothing returns nothing\n    constant integer X = 5\n    local constant integer Y = 6\nendfunction\n",
    );
    assert!(matches!(&b[0], Stmt::Constant(v, _) if v.is_constant));
    assert!(matches!(&b[1], Stmt::Local(v, _) if v.is_constant));
    roundtrip(
        "function f takes nothing returns nothing\n    constant integer X = 5\n    local constant integer Y = 6\nendfunction\n",
    );
}

#[test]
fn comma_local_lists_are_rejected() {
    // Vanilla declares one name per `local`; the game reports trailing data
    // on the line for `local integer a, b`.
    assert!(
        parse_err(
            "function f takes nothing returns nothing\n    local integer a, b\nendfunction\n"
        )
        .contains("end of the line")
    );
}

#[test]
fn set_targets_are_names_or_indexes() {
    roundtrip("function f takes nothing returns nothing\n    set udg_A[3] = 1\nendfunction\n");
    // `set f() = …` stops at the missing `=`; vanilla only assigns names.
    assert!(
        parse_err("function f takes nothing returns nothing\n    set GetX() = 1\nendfunction\n")
            .contains("expected")
    );
}

#[test]
fn exit_statement_does_not_exist() {
    assert!(
        parse_err("function f takes nothing returns nothing\n    exit\nendfunction\n")
            .contains("expected a statement")
    );
}

#[test]
fn bang_is_not_an_operator() {
    assert!(
        lex_err(
            "function f takes nothing returns nothing\n    local boolean b = !true\nendfunction\n"
        )
        .contains("`!`")
    );
}

#[test]
fn dot_and_dotdot_do_not_exist() {
    // `..` is not concatenation (that is `+`) and `.` is not member access;
    // vanilla rejects both.
    assert!(
        lex_err(
            "function f takes nothing returns nothing\n    local integer a = 1 .. 2\nendfunction\n"
        )
        .contains("`.`")
    );
    assert!(
        lex_err("function f takes nothing returns nothing\n    call obj.method()\nendfunction\n")
            .contains("`.`")
    );
}

// ----- parser: precedence ------------------------------------------------

#[test]
fn precedence_and_or() {
    // Vanilla: `and` binds looser than `or`.
    let e = expr_of(
        "function f takes boolean a returns boolean\n    return a and a or a\nendfunction\n",
    );
    match e {
        Expr::Binary {
            op: BinOp::And,
            lhs,
            rhs,
            ..
        } => {
            assert!(matches!(lhs.as_ref(), Expr::Ident(_, _)));
            assert!(matches!(rhs.as_ref(), Expr::Binary { op: BinOp::Or, .. }));
        }
        other => panic!("expected and(…, or(…)): {other:?}"),
    }
    roundtrip("function f takes boolean a returns boolean\n    return a and a or a\nendfunction\n");
}

#[test]
fn precedence_not_reaches_over_arithmetic() {
    // `not a + b` is `not (a + b)`; `not a == b` is `(not a) == b`.
    let e =
        expr_of("function f takes integer a returns boolean\n    return not a + a\nendfunction\n");
    assert!(
        matches!(&e, Expr::Unary { op: UnOp::Not, operand, .. } if matches!(operand.as_ref(), Expr::Binary { op: BinOp::Add, .. })),
        "{e:?}"
    );
    let e =
        expr_of("function f takes integer a returns boolean\n    return not a == a\nendfunction\n");
    assert!(
        matches!(&e, Expr::Binary { op: BinOp::Eq, lhs, .. } if matches!(lhs.as_ref(), Expr::Unary { op: UnOp::Not, .. })),
        "{e:?}"
    );
    roundtrip(
        "function f takes integer a returns boolean\n    return not a + a and not a == a\nendfunction\n",
    );
}

#[test]
fn precedence_arithmetic() {
    // `-a * b` is `(-a) * b`; `a + b * c` is `a + (b * c)`.
    let e = expr_of("function f takes integer a returns integer\n    return -a * a\nendfunction\n");
    assert!(
        matches!(&e, Expr::Binary { op: BinOp::Mul, lhs, .. } if matches!(lhs.as_ref(), Expr::Unary { op: UnOp::Neg, .. })),
        "{e:?}"
    );
    let e =
        expr_of("function f takes integer a returns integer\n    return a + a * a\nendfunction\n");
    assert!(
        matches!(&e, Expr::Binary { op: BinOp::Add, rhs, .. } if matches!(rhs.as_ref(), Expr::Binary { op: BinOp::Mul, .. })),
        "{e:?}"
    );
    // Ordering binds tighter than equality: `a == b < c` is `a == (b < c)`.
    let e =
        expr_of("function f takes integer a returns boolean\n    return a == a < a\nendfunction\n");
    assert!(
        matches!(&e, Expr::Binary { op: BinOp::Eq, rhs, .. } if matches!(rhs.as_ref(), Expr::Binary { op: BinOp::Lt, .. })),
        "{e:?}"
    );
    roundtrip(
        "function f takes integer a returns integer\n    return -a * a + a * a\nendfunction\n",
    );
}

#[test]
fn parentheses_are_preserved_as_nodes() {
    let e = expr_of(
        "function f takes boolean a returns boolean\n    return (a or a) and a\nendfunction\n",
    );
    // The author's grouping is a `Group` node, so printing restores it and
    // the tree never re-associates against them.
    assert!(
        matches!(&e, Expr::Binary { op: BinOp::And, lhs, .. } if matches!(lhs.as_ref(), Expr::Group(..))),
        "{e:?}"
    );
    roundtrip(
        "function f takes boolean a returns boolean\n    return (a or a) and a\nendfunction\n",
    );
    roundtrip(
        "function f takes integer a returns integer\n    return (a + a) * (a - a)\nendfunction\n",
    );
}

#[test]
fn unary_plus_and_function_refs() {
    roundtrip(
        "function g takes nothing returns nothing\nendfunction\nfunction f takes nothing returns nothing\n    call ForGroup(null, function g)\n    local integer a = +1\nendfunction\n",
    );
    // `function g()` is not a code reference: the parentheses form exists in
    // the checker only to reject it with a better message.
    assert!(parse("function g takes nothing returns nothing\nendfunction\nfunction f takes nothing returns nothing\n    call ForGroup(null, function g())\nendfunction\n").is_err());
}

// ----- parser: line orientation -----------------------------------------

#[test]
fn statements_are_line_oriented() {
    // Two statements on one line: the game reports "unexpected data at the
    // end of line", and so does this parser.
    let m = parse_err(
        "function f takes nothing returns nothing\n    call DoNothing() call DoNothing()\nendfunction\n",
    );
    assert!(m.contains("end of the line"), "{m}");
    // A statement must start its own line.
    let m = parse_err(
        "function f takes nothing returns nothing\n    local integer a = 1 set a = 2\nendfunction\n",
    );
    assert!(m.contains("end of the line"), "{m}");
    // Headers too.
    let m = parse_err(
        "function f takes nothing returns nothing\nendfunction function g takes nothing returns nothing\nendfunction\n",
    );
    assert!(m.contains("end of the line"), "{m}");
}

#[test]
fn bare_cr_line_endings_are_real_line_breaks() {
    // A shipped protected map (`Maul/Anime Maul Beta v0.952(Protect).w3x`'s
    // war3map.j) writes its script with bare-CR line endings and the game
    // accepts it, so `\r` ends a statement like `\n`, and `\r\n` is one
    // break — not two and not zero.
    let s = parse_ok("globals\r    integer a = 1\rendglobals\r");
    assert_eq!(s.items.len(), 1);
    roundtrip("globals\r    integer a = 1\rendglobals\r");
    roundtrip("globals\r\n    integer a = 1\r\nendglobals\r\n");
    // Line discipline holds per CR line…
    let m = parse_err(
        "function f takes nothing returns nothing\r    set a = 1 set a = 2\rendfunction\r",
    );
    assert!(m.contains("end of the line"), "{m}");
    // …and diagnostics report the map's own line/column, not "everything is
    // line 1".
    let src = "function f takes nothing returns nothing\r    set a = 1 set a = 2\rendfunction\r";
    let e = parse(src).unwrap_err();
    let map = SourceMap::new(src);
    assert_eq!(e.locate(&map), (2, 15), "{}", e.render(src));
    let src = "globals\r    integer a = 1\r    integer b = @\rendglobals\r";
    let e = parse(src).unwrap_err();
    let map = SourceMap::new(src);
    assert_eq!(e.locate(&map), (3, 17), "{}", e.render(src));
}

// ----- real-map corner cases ---------------------------------------------

#[test]
fn corpus_string_and_debug_shapes() {
    // common.ai: `debug call Trace("SleepInCombat\n")`
    roundtrip(
        "function SleepInCombat takes nothing returns nothing\n    debug call Trace(\"SleepInCombat\\n\")\nendfunction\n",
    );
    // Blizzard.j: `call Preloader( "scripts\\HumanMelee.pld" )`
    roundtrip(
        "function f takes nothing returns nothing\n    call Preloader(\"scripts\\\\HumanMelee.pld\")\nendfunction\n",
    );
    // GUI trigger code shape.
    roundtrip(
        "function Trig_Actions takes nothing returns nothing\n    if ( not ( udg_I == 1 ) ) then\n        call DoNothing()\n    endif\nendfunction\n",
    );
    // Shipped map tips: control codes and smiley `^^` in one string.
    roundtrip(
        "function f takes nothing returns nothing\n    set udg_Tip[24] = \"|c0000ff00Tip 24 of 30|r - |c0000cdf9Play Wintermaul TD often ^^|r\"\nendfunction\n",
    );
}

#[test]
fn error_reports_the_maps_own_position() {
    let src = "function f takes nothing returns nothing\n    local integer a = 1\n    set a = @\nendfunction\n";
    let e = parse(src).unwrap_err();
    let map = SourceMap::new(src);
    let (l, c) = e.locate(&map);
    assert_eq!((l, c), (3, 13), "unexpected location in {}", e.render(src));
    assert!(e.render(src).contains("3:13"));
}

#[test]
fn token_spans_carry_the_maps_line_and_column() {
    // PLAN.md: a missing native must fail loudly *with the map's identifier*,
    // so a token's span must resolve to the exact line/column of its spelling.
    let src =
        "function f takes nothing returns nothing\n    call BJDebugMsg(\"hi\")\nendfunction\n";
    let lexed = lex(src).unwrap();
    let map = SourceMap::new(src);
    let tok = lexed
        .tokens
        .iter()
        .find(|t| t.span.text(src) == "BJDebugMsg")
        .expect("identifier token");
    assert_eq!(tok.kind.label(), "ident:BJDebugMsg");
    assert_eq!(map.line_col(tok.span.start), (2, 10), "line/column");
    assert_eq!(tok.span.len(), 10);
    assert_eq!(map.render(tok.span), "2:10-20");
}

#[test]
fn hash_is_fnv1a64_of_canonical_bytes() {
    let s = parse_ok("globals\n    integer a = 1\nendglobals\n");
    assert_eq!(s.hash(), fnv1a64(&s.canonical_bytes()));
    // Formatting does not move the hash; meaning does.
    let t = parse_ok("globals\n\n    integer   a = 1\n\nendglobals\n");
    assert_eq!(s.hash(), t.hash());
    let u = parse_ok("globals\n    integer a = 2\nendglobals\n");
    assert_ne!(s.hash(), u.hash());
}

// ----- the real-map sweep ------------------------------------------------

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn wc3_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("wc3")
}

/// The 1.29b install the sweep also covers. Absent machines skip it; this
/// one has it, and its maps differ from the 1.28c set.
fn legacy_root() -> Option<PathBuf> {
    let p = PathBuf::from(
        "/home/maci/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)",
    );
    p.is_dir().then_some(p)
}

/// UTF-8 scripts, or else the raw bytes kept as an encoding reject. A source
/// that is not UTF-8 never reaches `parse` — the API takes `&str` — so it is
/// reported rather than silently dropped.
fn record_script(
    name: String,
    bytes: Vec<u8>,
    out: &mut Vec<(String, String)>,
    rejects: &mut Vec<(String, Vec<u8>)>,
) {
    match String::from_utf8(bytes) {
        Ok(text) => out.push((name, text)),
        Err(e) => rejects.push((name, e.into_bytes())),
    }
}

/// What kind of real source a corpus name refers to, for the sweep report.
fn source_kind(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("democampaign") {
        "demo-campaign map"
    } else if lower.ends_with(":war3map.j") {
        if lower.contains(".w3x:") {
            "community .w3x map"
        } else {
            "shipped .w3m map"
        }
    } else if lower.contains(".mpq:") {
        "install script"
    } else {
        "loose .j"
    }
}

/// Pull every real script this machine has: `Scripts\*.j` and `*.ai` out of
/// the install archives of both installs (1.28c `wc3/` and the 1.29b Legacy
/// install), `war3map.j` out of every shipped `.w3m`/`.w3x` map, and any
/// loose `.j` under either root. Returns UTF-8 scripts plus the sources whose
/// bytes are not UTF-8 (kept, reported, never parsed).
type ScriptCorpus = (Vec<(String, String)>, Vec<(String, Vec<u8>)>);

pub(super) fn corpus() -> ScriptCorpus {
    use pcx_mpq::MpqArchive;
    let mut out: Vec<(String, String)> = Vec::new();
    let mut rejects: Vec<(String, Vec<u8>)> = Vec::new();
    let mut installs: Vec<(String, PathBuf)> = vec![("1.28c".to_string(), wc3_root())];
    if let Some(legacy) = legacy_root() {
        installs.push(("1.29b".to_string(), legacy));
    }

    // 1. The install archives' own scripts.
    for (inst, root) in &installs {
        let app = if *inst == "1.28c" {
            root.join("game/app")
        } else {
            root.clone()
        };
        for arc_name in [
            "war3.mpq",
            "War3x.mpq",
            "War3Patch.mpq",
            "War3xlocal.mpq",
            "War3Local.mpq",
            "Deprecated.mpq",
        ] {
            let path = app.join(arc_name);
            let Ok(mut arc) = MpqArchive::open(&path) else {
                continue;
            };
            let Ok(list) = arc.extract("(listfile)") else {
                continue;
            };
            let names: Vec<String> = String::from_utf8_lossy(&list)
                .split(['\r', '\n'])
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect();
            for name in names {
                let lower = name.to_ascii_lowercase();
                if !(lower.ends_with(".j") || lower.ends_with(".ai")) {
                    continue;
                }
                if let Ok(bytes) = arc.extract(&name) {
                    record_script(
                        format!("{inst}:{arc_name}:{name}"),
                        bytes,
                        &mut out,
                        &mut rejects,
                    );
                }
            }
        }
    }

    // 2. Every shipped map's war3map.j (the embedded MPQ sits at 0x200).
    //    `.w3x` maps are as real as `.w3m`; both carry `war3map.j`.
    for (inst, root) in &installs {
        let maps = if *inst == "1.28c" {
            root.join("game/app/Maps")
        } else {
            root.join("Maps")
        };
        let mut w3ms: Vec<PathBuf> = Vec::new();
        let mut stack = vec![maps];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "w3m" || x == "w3x") {
                    w3ms.push(p);
                }
            }
        }
        w3ms.sort();
        for map in &w3ms {
            let Ok(data) = std::fs::read(map) else {
                continue;
            };
            let name = format!(
                "{inst}:{}",
                map.strip_prefix(root).unwrap_or(map).to_string_lossy()
            );
            if data.len() <= 0x200 {
                continue;
            }
            let Ok(mut arc) = MpqArchive::from_bytes(data[0x200..].to_vec(), &name) else {
                continue;
            };
            if let Ok(bytes) = arc.extract("war3map.j") {
                record_script(format!("{name}:war3map.j"), bytes, &mut out, &mut rejects);
            }
        }
    }

    // 3. The demo campaign's embedded maps (1.28c).
    let camp = wc3_root().join("game/app/Campaigns/DemoCampaign.w3n");
    if let Ok(data) = std::fs::read(&camp)
        && data.len() > 0x200
        && let Ok(mut arc) = MpqArchive::from_bytes(data[0x200..].to_vec(), "DemoCampaign.w3n")
    {
        for entry in ["Demo03.w3m", "Demo04.w3m", "Demo05.w3m"] {
            if let Ok(inner) = arc.extract(entry)
                && inner.len() > 0x200
                && let Ok(mut m) = MpqArchive::from_bytes(inner[0x200..].to_vec(), entry)
                && let Ok(bytes) = m.extract("war3map.j")
            {
                record_script(
                    format!("DemoCampaign.w3n:{entry}:war3map.j"),
                    bytes,
                    &mut out,
                    &mut rejects,
                );
            }
        }
    }

    // 4. Loose .j files anywhere under either install (the engine scripts
    //    land here if someone extracts them).
    for (inst, root) in &installs {
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "j")
                    && let Ok(bytes) = std::fs::read(&p)
                {
                    let name = format!(
                        "{inst}:{}",
                        p.strip_prefix(root).unwrap_or(&p).to_string_lossy()
                    );
                    record_script(name, bytes, &mut out, &mut rejects);
                }
            }
        }
    }

    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    rejects.sort_by(|a, b| a.0.cmp(&b.0));
    rejects.dedup_by(|a, b| a.0 == b.0);
    (out, rejects)
}

/// The load order of the Blizzard scripts. A map never declares the natives it
/// calls; the game loads these beside it, so they are the prelude a map script
/// resolves against.
const BLIZZARD_ORDER: [&str; 4] = ["common.j", "Blizzard.j", "Cheats.j", "InitCheats.j"];

/// The archive whose copy of a Blizzard script the game loads. This machine
/// carries two installs and each ships `Scripts\Blizzard.j` in both
/// `war3.mpq` (the Reign of Chaos original, 611 functions) and `War3x.mpq` (the
/// Frozen Throne expansion's, 760). The expansion copy is the one maps are
/// compiled against, and it is measured rather than assumed: corpus maps call
/// `AddItemToStockBJ`, `CinematicModeExBJ` and `ClearSelectionForPlayer`, which
/// only `War3x.mpq`'s copy declares, and never the two functions only
/// `war3.mpq`'s copy carries (`AllowPlayerToRescueBJ`, `DoTransmissionBasicsBJ`).
const BLIZZARD_ARCHIVE: &str = "War3x.mpq";

/// One copy of `Scripts\<base>`: the expansion archive's when the install has
/// it, otherwise any copy. Two copies of a script are two versions of it, and
/// a name the two disagree on is not a resolution error but a mixed load.
const INSTALLS: [&str; 2] = ["1.28c", "1.29b"];

/// Parsed install scripts, so a prelude is built once per corpus member
/// instead of re-parsing common.j a thousand times.
fn install_scripts(scripts: &[(String, String)]) -> Vec<(String, Script)> {
    scripts
        .iter()
        .filter(|(n, _)| n.contains(".mpq:Scripts\\") && (n.ends_with(".j") || n.ends_with(".ai")))
        .map(|(n, src)| (n.clone(), parse(src).expect("install scripts parse")))
        .collect()
}

/// Which sources a corpus script is loaded with. Both installs' Blizzard
/// scripts, one copy of each per install: a script resolved against the install
/// it came from fails on names only the other install's `common.j` declares,
/// and that is version skew, not a resolution defect. The number of such names
/// is measured separately, from `old_install_declares`, so the union is never
/// mistaken for a measurement.
///
/// An AI script also gets `common.ai`, which declares the AI native set that
/// `common.j` does not (`DisplayText`, `GetHeroId`, …). A script is never its
/// own prelude.
fn prelude_for<'a>(name: &str, installs: &'a [(String, Script)]) -> Vec<Prelude<'a>> {
    let lower = name.to_ascii_lowercase();
    let mut out: Vec<Prelude<'a>> = Vec::new();
    let mut load = |base: &str| {
        let suffix = format!(":Scripts\\{base}");
        for inst in INSTALLS {
            let from = installs
                .iter()
                .filter(|(n, _)| n.starts_with(&format!("{inst}:")) && n.ends_with(&suffix));
            let chosen = from
                .clone()
                .find(|(n, _)| n.starts_with(&format!("{inst}:{BLIZZARD_ARCHIVE}:")))
                .or_else(|| from.into_iter().next());
            if let Some((n, s)) = chosen
                && n != name
            {
                out.push(Prelude::new(n, s));
            }
        }
    };
    if lower.ends_with(".ai") {
        load("common.ai");
    }
    for b in BLIZZARD_ORDER {
        load(b);
    }
    out
}

/// The unresolved names the corpus actually contains, each with the construct
/// and the reason it is genuinely there. Nothing here is a resolver defect, and
/// the sweep fails if a new one appears or one of these disappears, so the
/// table cannot silently grow into a way of hiding findings.
const KNOWN_UNRESOLVED: [(&str, &str, &str); 4] = [
    (
        "Scripts\\orc03",
        "`CampaignAI` takes 2 arguments, 1 given",
        "all five orc3 campaign scripts call `CampaignAI(function heroes)` with one argument while common.ai declares `(integer farms, code heroes)`; the AI compiler's own view of CampaignAI is not the JASS wrapper's, and this check is the JASS one",
    ),
    (
        "Scripts\\h04_orange.ai",
        "`debug_fast_attacks` is not declared",
        "the script assigns to a global no install declares",
    ),
    (
        "Scripts\\demo.ai",
        "`GetUpgradeStoneCost` is not declared",
        "a sample AI script calling a native no shipped script declares",
    ),
    (
        "Diablo III Beta v1.17.w3x:war3map.j",
        "`CamLock` is not declared",
        "the map reads a global it never declares (it does declare `gg_trg_CamLock`), a genuine script defect",
    ),
];

/// Whether the Blizzard scripts of `name`'s own install declare `called`. The
/// demo campaign has no install of its own and counts as the older one.
fn old_install_declares(name: &str, called: &str, installs: &[(String, Script)]) -> bool {
    let inst = install_of(name).unwrap_or("1.28c");
    installs
        .iter()
        .filter(|(n, _)| n.starts_with(&format!("{inst}:")))
        .flat_map(|(_, s)| {
            let mut names: Vec<String> = Vec::new();
            names.extend(s.natives().map(|n| n.name.as_str().to_string()));
            names.extend(
                s.all_functions()
                    .iter()
                    .map(|f| f.name.as_str().to_string()),
            );
            names.extend(s.globals().iter().map(|g| g.name.as_str().to_string()));
            names.extend(s.items.iter().filter_map(|i| match i {
                Item::Type(t) => Some(t.name.as_str().to_string()),
                _ => None,
            }));
            names
        })
        .any(|n| n == called)
}

/// The install a corpus name came from, or `None` for the demo campaign.
fn install_of(name: &str) -> Option<&str> {
    INSTALLS
        .iter()
        .copied()
        .find(|i| name.starts_with(&format!("{i}:")))
}

/// The corpus sweep. Every real script on this machine must parse, every one
/// must round-trip, and failures are written to `/tmp/opencode/script-sweep.txt`.
#[test]
#[ignore = "sweeps every shipped map and install script; run with --ignored"]
fn sweep_every_real_script() {
    let (scripts, encoding_rejects) = corpus();
    assert!(!scripts.is_empty(), "no corpus found on this machine");

    let installs = install_scripts(&scripts);
    let mut failures: Vec<String> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut resolved = 0usize;
    let mut resolved_uses = 0usize;
    let mut natives_called = 0usize;
    // Natives a script calls that the install it came from does not declare:
    // the M7 number for which Blizzard scripts a map actually needs.
    let mut version_skew_names: BTreeMap<String, usize> = BTreeMap::new();
    let mut version_skew_scripts = 0usize;
    let mut every_native: BTreeSet<String> = BTreeSet::new();
    let mut every_native_implemented: BTreeSet<String> = BTreeSet::new();
    // Every real literal, checked for lossless narrowing to the value model's
    // single-precision real.
    let mut reals = 0usize;
    let mut reals_lossy = 0usize;
    let mut parsed = 0usize;
    let mut tokens = 0usize;
    let mut items = 0usize;
    let mut functions = 0usize;
    let mut statements = 0usize;
    let mut rawcodes = 0usize;
    let mut strings = 0usize;

    for (name, src) in &scripts {
        match parse(src) {
            Ok(script) => {
                // Round-trip: the same proof the unit tests use, on real maps.
                let printed = script.to_source();
                match parse(&printed) {
                    Ok(second) => {
                        if second.canonical_bytes() != script.canonical_bytes() {
                            failures.push(format!("{name}: round-trip changed the tree"));
                        }
                    }
                    Err(e) => {
                        failures.push(format!(
                            "{name}: printed form failed to re-parse: {}",
                            e.render(&printed)
                        ));
                    }
                }
                let lexed = lex(src).expect("lex after parse");
                let st = lexed.stats();
                let ts = script.stats();
                parsed += 1;
                tokens += st.tokens;
                items += ts.items;
                functions += ts.functions;
                statements += ts.statements;
                rawcodes += st.rawcodes;
                strings += st.string_literals;
            }
            Err(e) => {
                failures.push(format!("{name}: {}", e.render(src)));
            }
        }
    }

    // Name resolution. Each script resolves against the sources the game loads
    // it with, so an unresolved name is either a defect in the resolver or a
    // name no Blizzard script declares, and never an artifact of the corpus.
    for (name, src) in &scripts {
        let Ok(script) = parse(src) else {
            continue;
        };
        let prelude = prelude_for(name, &installs);
        match resolve_with(&script, &prelude) {
            Ok(program) => {
                resolved += 1;
                let st = program.stats;
                resolved_uses += st.uses;
                natives_called += st.natives_called;
                // What the map calls that the install it came from does not
                // declare: the M7 number for "which Blizzard scripts does this
                // map actually need".
                for (called, n) in program.natives_called() {
                    every_native.insert(called.to_string());
                    if program
                        .natives()
                        .call(called)
                        .is_some_and(|c| matches!(c, crate::natives::NativeCall::Implemented(_, _)))
                    {
                        every_native_implemented.insert(called.to_string());
                    }
                    if !old_install_declares(name, called, &installs) {
                        *version_skew_names.entry(called.to_string()).or_insert(0) += n;
                        version_skew_scripts += 1;
                    }
                }
                for g in program.globals() {
                    if let Some(e) = &g.init {
                        reals_in(e, &mut reals, &mut reals_lossy);
                    }
                }
                for f in program.functions() {
                    if let Some(body) = &f.body {
                        for st in body {
                            reals_in_stmt(st, &mut reals, &mut reals_lossy);
                        }
                    }
                }
            }
            Err(e) => {
                unresolved.push(format!("{name}: {}", e.render(src)));
            }
        }
    }

    // Every unresolved name has to be a construct the engine accepts and this
    // resolver refuses, or a genuine defect in the script. Each entry below is
    // proved against the corpus and any *new* one fails the sweep.
    let mut explained = 0usize;
    let mut surprises: Vec<String> = Vec::new();
    for u in &unresolved {
        let hit = KNOWN_UNRESOLVED
            .iter()
            .find(|(file, detail, _why)| u.contains(file) && u.contains(detail));
        match hit {
            Some(_) => explained += 1,
            None => surprises.push(u.clone()),
        }
    }
    for (file, detail, why) in KNOWN_UNRESOLVED {
        if !unresolved
            .iter()
            .any(|u| u.contains(file) && u.contains(detail))
        {
            surprises.push(format!(
                "{file}: the documented finding `{detail}` is gone, so the table needs updating: {why}"
            ));
        }
    }

    // Sources whose bytes are not UTF-8 never reach `parse` (the API takes
    // `&str`) — the file encoding is a load-stage question. They are reported
    // by path, never dropped, and their lossy-decoded form must still parse,
    // so nothing hides behind the encoding: the gap is bytes, not grammar.
    let mut encoding: Vec<String> = Vec::new();
    for (name, bytes) in &encoding_rejects {
        let lossy = String::from_utf8_lossy(bytes);
        match parse(&lossy) {
            Ok(_) => encoding.push(name.clone()),
            Err(e) => failures.push(format!(
                "{name}: not UTF-8, and the lossy-decoded form fails to parse: {}",
                e.render(&lossy)
            )),
        }
    }

    let mut report = String::new();
    let _ = writeln!(
        report,
        "pcx-script sweep: {} scripts, {} parsed, {} failed, {} non-UTF-8 (listed below)",
        scripts.len(),
        parsed,
        failures.len(),
        encoding.len()
    );
    let _ = writeln!(
        report,
        "resolve: {} scripts resolved, {} unresolved ({} explained below), {} identifier uses, \
         {} native calls over {} distinct natives, {} of them implemented",
        resolved,
        unresolved.len(),
        explained,
        resolved_uses,
        natives_called,
        every_native.len(),
        every_native_implemented.len()
    );
    let _ = writeln!(
        report,
        "real literals: {reals} seen, {reals_lossy} of them lose a bit narrowing f64 to f32"
    );
    for (file, detail, why) in KNOWN_UNRESOLVED {
        let _ = writeln!(report, "KNOWN {file}: {detail} -- {why}");
    }
    let _ = writeln!(
        report,
        "other install's natives needed: {version_skew_scripts} call sites over {} distinct names",
        version_skew_names.len()
    );
    for (n, c) in &version_skew_names {
        let _ = writeln!(report, "  NEEDS-NEWER {n} (x{c})");
    }
    for u in &unresolved {
        let _ = writeln!(report, "UNRESOLVED {u}");
    }
    let mut kinds: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for name in scripts
        .iter()
        .map(|(n, _)| n.as_str())
        .chain(encoding_rejects.iter().map(|(n, _)| n.as_str()))
    {
        *kinds.entry(source_kind(name)).or_insert(0) += 1;
    }
    for (k, n) in &kinds {
        let _ = writeln!(report, "  {k}: {n}");
    }
    let _ = writeln!(
        report,
        "totals: {tokens} tokens, {items} items, {functions} functions, {statements} statements, {rawcodes} rawcodes, {strings} strings"
    );
    for n in &encoding {
        let _ = writeln!(
            report,
            "ENCODING {n}: not UTF-8; lossy-decoded form parses (file encoding is out of scope for &str parsing)"
        );
    }
    for f in &failures {
        let _ = writeln!(report, "FAIL {f}");
    }
    std::fs::create_dir_all("/tmp/opencode").expect("mkdir /tmp/opencode");
    std::fs::write("/tmp/opencode/script-sweep.txt", &report).expect("write sweep report");
    print!("{report}");

    assert_eq!(
        failures,
        Vec::<String>::new(),
        "sweep failures are listed above and in /tmp/opencode/script-sweep.txt"
    );
    assert_eq!(
        surprises,
        Vec::<String>::new(),
        "an unexplained name-resolution failure is listed above and in /tmp/opencode/script-sweep.txt"
    );
}

/// Every real literal under `e`, counting the ones whose f64 parse is not
/// exactly the f32 the value model uses. A non-zero count would make the
/// narrowing a real behaviour change rather than a formality.
fn reals_in(e: &Expr, total: &mut usize, lossy: &mut usize) {
    match e {
        Expr::Literal(crate::Literal::Real(r), _) => {
            *total += 1;
            if r.value as f32 != r.text.parse::<f32>().unwrap_or(f32::NAN) {
                *lossy += 1;
            }
        }
        Expr::Group(inner, _) | Expr::Unary { operand: inner, .. } => reals_in(inner, total, lossy),
        Expr::Binary { lhs, rhs, .. } => {
            reals_in(lhs, total, lossy);
            reals_in(rhs, total, lossy);
        }
        Expr::Call { callee, args, .. } => {
            reals_in(callee, total, lossy);
            for a in args {
                reals_in(a, total, lossy);
            }
        }
        Expr::Index { base, index, .. } => {
            reals_in(base, total, lossy);
            reals_in(index, total, lossy);
        }
        _ => {}
    }
}

fn reals_in_stmt(st: &Stmt, total: &mut usize, lossy: &mut usize) {
    match st {
        Stmt::Set { target, value, .. } => {
            reals_in(target, total, lossy);
            reals_in(value, total, lossy);
        }
        Stmt::Call { args, .. } => {
            for a in args {
                reals_in(a, total, lossy);
            }
        }
        Stmt::Return(Some(e), _) | Stmt::ExitWhen(e, _) => reals_in(e, total, lossy),
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for b in branches {
                reals_in(&b.cond, total, lossy);
                for st in &b.body {
                    reals_in_stmt(st, total, lossy);
                }
            }
            for st in else_body.iter().flatten() {
                reals_in_stmt(st, total, lossy);
            }
        }
        Stmt::Loop { body, .. } => {
            for st in body {
                reals_in_stmt(st, total, lossy);
            }
        }
        Stmt::Local(v, _) | Stmt::Constant(v, _) => {
            if let Some(e) = &v.init {
                reals_in(e, total, lossy);
            }
        }
        Stmt::Debug(inner, _) => reals_in_stmt(inner, total, lossy),
        _ => {}
    }
}

use std::fmt::Write as _;
