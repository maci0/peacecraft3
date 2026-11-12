//! Tests for the VM stage: resolution errors, the value model, the native
//! registry, the text seam, and an end-to-end run of a real shipped function.
//!
//! The end-to-end proof drives the shipped `Blizzard.j` off the install and
//! calls functions from it, so the value model is checked against code the game
//! compiles rather than against a hand-written oracle.

use crate::eval::MAX_STEPS;
use crate::natives::{NativeTable, signature_of};
use crate::resolve::{Prelude, resolve, resolve_with};
use crate::span::Span;
use crate::value::{Class, OpError, Value, binary, unary};
use crate::{BinOp, Exec, NullHost, OwnedVm, Script, UnOp, Vm, parse};
use std::sync::Arc;

// ----- harness -----------------------------------------------------------

/// Parse and resolve one script. Panics on either failure, so a test that is
/// about a *value* is not testing error plumbing.
#[track_caller]
fn program_of(src: &str) -> crate::resolve::Program {
    let script = parse(src).unwrap_or_else(|e| panic!("parse failed: {}\n{src}", e.render(src)));
    resolve(&script).unwrap_or_else(|e| panic!("resolve failed: {}\n{src}", e.render(src)))
}

/// Resolve, expecting the caller to name the error it wants to see.
#[track_caller]
fn resolve_of(src: &str) -> Result<crate::resolve::Program, crate::Error> {
    let script = parse(src).unwrap_or_else(|e| panic!("parse failed: {}\n{src}", e.render(src)));
    resolve(&script)
}

/// Run `name(args)` in a program with no host, so every unimplemented native is
/// the loud error rather than a zero.
#[track_caller]
fn call(src: &str, name: &str, args: &[Value]) -> Result<Value, crate::Error> {
    let program = program_of(src);
    let mut vm = Vm::new(&program, NullHost);
    vm.init_globals()?;
    vm.call(name, args)
}

/// The line and column a resolution error reports, 1-based.
#[track_caller]
fn where_is(src: &str) -> (u32, u32, String) {
    let e = resolve_of(src).expect_err("expected a resolution error");
    let map = crate::span::SourceMap::new(src);
    let (l, c) = e.locate(&map);
    (l, c, e.message)
}

/// Evaluate one expression out of `return <expr>` and take the integer.
#[track_caller]
fn int_of(expr: &str) -> i32 {
    let src = format!("function f takes nothing returns integer\n    return {expr}\nendfunction\n");
    match call(&src, "f", &[]).expect("call failed") {
        Value::Int(i) => i,
        other => panic!("expected an integer, got {other}"),
    }
}

// ----- resolution: a name error carries the map's own position ------------

#[test]
fn duplicate_map_globals_refuse_instead_of_replacing_the_first() {
    for src in [
        "globals\n integer repeated = 1\n integer repeated = 2\nendglobals\n",
        "globals\n integer repeated = 1\nendglobals\nglobals\n integer repeated = 2\nendglobals\n",
    ] {
        let error = resolve_of(src).expect_err("duplicate globals must refuse");
        assert!(error.message.contains("repeated"), "{}", error.message);
        assert!(
            error.message.contains("declared twice"),
            "{}",
            error.message
        );
    }
}

#[test]
fn a_map_global_still_replaces_a_prelude_global() {
    let prelude = parse("globals\n integer shared = 2\nendglobals\n").unwrap();
    let map = parse("globals\n integer shared = 7\nendglobals\n").unwrap();
    let program = resolve_with(&map, &[Prelude::new("common.j", &prelude)]).unwrap();
    let mut vm = Vm::new(&program, NullHost);
    vm.init_globals().unwrap();
    assert_eq!(vm.global("shared"), Value::Int(7));
}

#[test]
fn an_unresolved_name_reports_the_maps_own_line_and_column() {
    let src = "globals\n    integer a = 0\nendglobals\nfunction f takes nothing returns nothing\n    call Missing(a)\nendfunction\n";
    let (line, col, message) = where_is(src);
    assert_eq!((line, col), (5, 10), "{message}");
    assert!(
        message.contains("`Missing`") && message.contains("is not declared"),
        "{message}"
    );
    // The span covers the spelling, not the whole line.
    let e = resolve_of(src).expect_err("error");
    let map = crate::span::SourceMap::new(src);
    assert_eq!(map.render(e.span), "5:10-17", "{}", e.render(src));
}

#[test]
fn a_name_used_in_a_set_and_in_an_argument_reports_the_same_span_rule() {
    let src = "function f takes nothing returns nothing\n    local integer i = 0\n    set i = UndeclaredGlobal\nendfunction\n";
    let (line, col, _) = where_is(src);
    assert_eq!((line, col), (3, 13));
}

#[test]
fn a_forward_declaration_links_to_a_later_body() {
    // The parser spells an empty body and a body-less declaration the same
    // way, so a lone declaration is an empty function and a declared pair
    // links.
    let src = "function Later takes nothing returns integer\nendfunction\nfunction Later takes nothing returns integer\n    return 7\nendfunction\n";
    let program = program_of(src);
    assert_eq!(
        program
            .function("Later")
            .and_then(|f| f.body.as_ref())
            .map(|body| body.len()),
        Some(1)
    );
}

#[test]
fn a_forward_declaration_with_a_different_shape_is_refused() {
    let src = "function Later takes integer a returns integer\nendfunction\nfunction Later takes real a returns integer\n    return 7\nendfunction\n";
    let (line, _, message) = where_is(src);
    assert_eq!(line, 3, "{message}");
    assert!(
        message.contains("declared twice with different signatures"),
        "{message}"
    );
}

// ----- resolution: the checks vanilla does -------------------------------

#[test]
fn an_unknown_type_declaration_is_reported() {
    let src = "globals\n    widget array things\nendglobals\n";
    let (line, _, message) = where_is(src);
    assert_eq!(line, 2);
    assert!(
        message.contains("type `widget` is not declared"),
        "{message}"
    );
}

#[test]
fn a_type_declared_later_in_the_same_script_is_fine() {
    // Vanilla's flat declaration pass has no ordering requirement.
    program_of("type thing extends handle\nglobals\n    thing t = null\nendglobals\n");
}

#[test]
fn a_type_extends_an_undeclared_type_is_reported() {
    let src = "type noodle extends soup\n";
    let (_, _, message) = where_is(src);
    assert!(
        message.contains("`soup`, which is not declared"),
        "{message}"
    );
}

#[test]
fn a_type_extends_a_value_type_is_reported() {
    let src = "type noodle extends integer\n";
    let (_, _, message) = where_is(src);
    assert!(message.contains("is not a handle type"), "{message}");
}

#[test]
fn a_cyclic_type_chain_is_reported() {
    let src = "type a extends b\ntype b extends a\n";
    let (_, _, message) = where_is(src);
    assert!(message.contains("cyclic"), "{message}");
}

#[test]
fn a_wrong_argument_count_is_reported_with_the_declaration() {
    let src = "function One takes integer a returns nothing\nendfunction\nfunction caller takes nothing returns nothing\n    call One(1, 2)\nendfunction\n";
    let (line, col, message) = where_is(src);
    assert_eq!((line, col), (4, 10), "{message}");
    assert!(
        message.contains("takes 1 argument") && message.contains("2 given"),
        "{message}"
    );
}

#[test]
fn a_native_call_arity_is_checked_against_its_declaration() {
    let src = "native Two takes integer a, real b returns nothing\nfunction caller takes nothing returns nothing\n    call Two(1)\nendfunction\n";
    let (line, _, message) = where_is(src);
    assert_eq!(line, 3, "{message}");
    assert!(
        message.contains("native `Two` takes 2 arguments, 1 given"),
        "{message}"
    );
}

#[test]
fn nested_call_arguments_are_resolved_in_initializers_returns_and_statements() {
    let declarations = "native Wrap takes integer value returns integer\nnative Leaf takes integer value returns integer\nnative Sink takes integer value returns nothing\n";
    for (expression, message) in [
        ("Wrap(Missing())", "name `Missing` is not declared"),
        ("Wrap(Leaf())", "native `Leaf` takes 1 argument, 0 given"),
        (
            "Wrap(Leaf(MissingValue))",
            "name `MissingValue` is not declared",
        ),
    ] {
        for body in [
            format!("globals\n integer value = {expression}\nendglobals\n"),
            format!(
                "function f takes nothing returns integer\n return {expression}\nendfunction\n"
            ),
            format!(
                "function f takes nothing returns nothing\n call Sink({expression})\nendfunction\n"
            ),
        ] {
            let source = format!("{declarations}{body}");
            let error = resolve_of(&source).expect_err(&source);
            assert!(error.message.contains(message), "{}", error.render(&source));
        }
    }
}

#[test]
fn nested_native_calls_are_included_in_admission_statistics() {
    let program = program_of(
        "native Wrap takes integer value returns integer\nnative Leaf takes integer value returns integer\nfunction f takes nothing returns integer\n return Wrap(Leaf(3) + Leaf(4))\nendfunction\n",
    );
    assert_eq!(
        program.natives_called().collect::<Vec<_>>(),
        [("Leaf", 2), ("Wrap", 1)]
    );
    assert_eq!(program.stats.natives_called, 2);
    assert_eq!(program.unimplemented_natives().count(), 2);
}

#[test]
fn a_constant_cannot_be_assigned_to() {
    let src = "globals\n    constant integer FIXED = 3\nendglobals\nfunction f takes nothing returns nothing\n    set FIXED = 4\nendfunction\n";
    let (line, _, message) = where_is(src);
    assert_eq!(line, 5, "{message}");
    assert!(message.contains("is a constant"), "{message}");
}

#[test]
fn a_scalar_used_with_an_index_is_reported() {
    let src = "globals\n    integer one = 1\nendglobals\nfunction f takes nothing returns nothing\n    set one[2] = 1\nendfunction\n";
    let (line, _, message) = where_is(src);
    assert_eq!(line, 5, "{message}");
    assert!(message.contains("`one` is not an array"), "{message}");
}

#[test]
fn undeclared_array_bases_are_refused_for_reads_and_writes() {
    for statement in ["return Missing[0]", "set Missing[0] = 1"] {
        let source =
            format!("function f takes nothing returns integer\n {statement}\nendfunction\n");
        let (line, _, message) = where_is(&source);
        assert_eq!(line, 2);
        assert!(
            message.contains("name `Missing` is not declared"),
            "{message}"
        );
    }
}

#[test]
fn native_functions_cannot_be_used_as_array_bases() {
    let source = "native Leaf takes integer value returns integer\nfunction f takes nothing returns integer\n return Leaf[0]\nendfunction\n";
    let (_, _, message) = where_is(source);
    assert!(message.contains("`Leaf` is a function"), "{message}");
}

#[test]
fn a_trigger_string_is_not_an_unresolved_name() {
    // Footman Frenzy ships 862 of the literal form and the corpus 34,634
    // between both spellings.
    let src = "function Show takes string s returns nothing\nendfunction\nfunction f takes nothing returns nothing\n    call Show(TRIGSTR_374)\nendfunction\n";
    resolve_of(src).unwrap_or_else(|e| panic!("resolve failed: {}\n{src}", e.render(src)));
}

#[test]
fn a_library_requires_a_declared_library() {
    let src = "library A requires MissingLib\n    function inLib takes nothing returns nothing\n    endfunction\nendlibrary\n";
    let (line, _, message) = where_is(src);
    assert_eq!(line, 1, "{message}");
    assert!(message.contains("requires `MissingLib`"), "{message}");
}

#[test]
fn an_optional_declaration_may_be_absent() {
    let src = "library A requires optional NoSuchLib\n    function inLib takes nothing returns nothing\n    endfunction\nendlibrary\n";
    resolve_of(src).unwrap_or_else(|e| panic!("{}\n{src}", e.render(src)));
}

#[test]
fn a_library_loads_after_what_it_requires() {
    let src = "library B requires A\n    function inB takes nothing returns nothing\n    endfunction\nendlibrary\nlibrary A\n    function inA takes nothing returns nothing\n    endfunction\nendlibrary\n";
    let program = program_of(src);
    assert_eq!(program.library_order(), ["A", "B"]);
}

#[test]
fn a_cyclic_library_chain_is_reported() {
    let src = "library A requires B\n    function inA takes nothing returns nothing\n    endfunction\nendlibrary\nlibrary B requires A\n    function inB takes nothing returns nothing\n    endfunction\nendlibrary\n";
    let (_, _, message) = where_is(src);
    assert!(message.contains("requires itself"), "{message}");
}

#[test]
fn a_struct_allocate_call_is_refused_naming_the_rewrite() {
    let src = "struct Foo\nendstruct\nfunction f takes nothing returns nothing\n    local Foo x = Foo(1)\nendfunction\n";
    let (line, _, message) = where_is(src);
    assert_eq!(line, 4, "{message}");
    assert!(
        message.contains("allocate call") && message.contains("JASSHelper"),
        "{message}"
    );
}

#[test]
fn a_container_name_is_not_a_value() {
    let src = "library Wrap\nendlibrary\nfunction f takes nothing returns nothing\n    set Wrap = Wrap\nendfunction\n";
    let (line, _, message) = where_is(src);
    assert_eq!(line, 4, "{message}");
    assert!(message.contains("a library is not a value"), "{message}");
}

#[test]
fn a_type_name_is_not_a_value() {
    let src = "type thing extends handle\nfunction f takes nothing returns nothing\n    set thing = thing\nendfunction\n";
    let (line, _, message) = where_is(src);
    assert_eq!(line, 3, "{message}");
    assert!(message.contains("a type is not a value"), "{message}");
}

#[test]
fn the_maps_own_declaration_replaces_the_preludes() {
    // The map is loaded last, so its declarations win over the Blizzard
    // scripts', and a name they disagree on is not silently the prelude's.
    let blizzard =
        parse("function Shared takes nothing returns integer\n    return 1\nendfunction\n")
            .expect("parse");
    let map = parse(
        "function Shared takes nothing returns integer\n    return 2\nendfunction\nfunction f takes nothing returns integer\n    return Shared()\nendfunction\n",
    )
    .expect("parse");
    let prelude = [Prelude::new("common.j", &blizzard)];
    let program = resolve_with(&map, &prelude).expect("resolve");
    assert_eq!(program.stats.functions, 2);
    let mut vm = Vm::new(&program, NullHost);
    assert_eq!(vm.call("Shared", &[]).expect("call"), Value::Int(2));
}

#[test]
fn two_install_scripts_agreeing_are_one_declaration() {
    let one = parse("native Same takes real r returns integer\n").expect("parse");
    let two = parse("native Same takes real r returns integer\n").expect("parse");
    let mut table = NativeTable::new();
    table.insert(one.natives().next().expect("decl"), "war3.mpq");
    table.insert(two.natives().next().expect("decl"), "War3x.mpq");
    let decl = table.get("Same").expect("one declaration");
    assert_eq!(decl.also_declared_in.len(), 1);
}

#[test]
fn two_install_scripts_disagreeing_make_the_name_ambiguous() {
    let one = parse("native Same takes real r returns integer\n").expect("parse");
    let two = parse("native Same takes real r returns nothing\n").expect("parse");
    let mut table = NativeTable::new();
    table.insert(one.natives().next().expect("decl"), "war3.mpq");
    table.insert(two.natives().next().expect("decl"), "War3x.mpq");
    let amb = table.ambiguous("Same").expect("ambiguous");
    assert_eq!(amb.at.len(), 2);
    assert_eq!(amb.at[0].0, "war3.mpq");
    assert_eq!(amb.at[0].2, "Same takes real r returns integer");
    assert_eq!(amb.at[1].2, "Same takes real r returns nothing");
    assert_eq!(table.get("Same"), None);
    assert_eq!(
        signature_of("Two", &[], &one.natives().next().expect("d").ret),
        "Two takes nothing returns integer"
    );
    assert_eq!(table.len(), 0);
}

#[test]
fn a_call_to_an_ambiguous_native_is_refused() {
    // The corpus's one real case: TriggerRegisterDestDeathInRegionEvent
    // returns `event` on one install and `nothing` on the other.
    let war = parse("native Same takes real r returns integer\n").expect("parse");
    let tft = parse("native Same takes real r returns nothing\n").expect("parse");
    let map =
        parse("function f takes nothing returns integer\n    return Same(1.0)\nendfunction\n")
            .expect("parse");
    let prelude = [
        Prelude::new("war3.mpq", &war),
        Prelude::new("War3x.mpq", &tft),
    ];
    let e = resolve_with(&map, &prelude).expect_err("ambiguous");
    assert!(
        e.message.contains("two different signatures"),
        "{}",
        e.message
    );
    assert!(e.message.contains("war3.mpq"), "{}", e.message);
    assert!(e.message.contains("War3x.mpq"), "{}", e.message);
}

// ----- the value model ---------------------------------------------------

#[test]
fn integer_arithmetic_wraps_at_32_bits() {
    // The literal `$FFFFFFFF` is the map's own spelling of the 32-bit id
    // space, not an out-of-range value.
    assert_eq!(Value::wrap32(0xFFFF_FFFF), -1);
    assert_eq!(Value::wrap32(0x7FFF_FFFF), i32::MAX);
    assert_eq!(int_of("2147483647 + 1"), i32::MIN);
    assert_eq!(int_of("$FFFFFFFF + 1"), 0);
    assert_eq!(int_of("$7FFFFFFF * 2"), -2);
    assert_eq!(int_of("-2147483648 - 1"), i32::MAX);
    // A rawcode is an integer, so the same wrapping applies to it.
    assert_eq!(int_of("'A000' + 1"), int_of("'A001'"));
}

#[test]
fn modulo_follows_the_sign_of_the_dividend() {
    // The identity `(a / b) * b + a % b == a`, which is the invariant
    // Blizzard.j's ModuloInteger documents, holds for both signs.
    assert_eq!(int_of("-7 % 3"), -1);
    assert_eq!(int_of("7 % -3"), 1);
    assert_eq!(int_of("-7 % -3"), -1);
    assert_eq!(int_of("7 % 3"), 1);
    assert_eq!(int_of("-7 / 3"), -2);
    let a = -9;
    let b = 4;
    assert_eq!(a / b * b + a % b, a);
}

#[test]
fn division_by_zero_is_refused_rather_than_given_a_value() {
    let src = "function f takes nothing returns integer\n    return 1 / 0\nendfunction\n";
    let e = call(src, "f", &[]).expect_err("expected a refusal");
    assert!(e.message.contains("division by zero"), "{}", e.message);
}

#[test]
fn a_plus_concatenates_only_two_strings() {
    assert_eq!(
        call(
            "function f takes nothing returns string\n    return \"a\" + \"b\"\nendfunction\n",
            "f",
            &[]
        )
        .expect("call"),
        Value::Str("ab".into())
    );
    // Vanilla has no implicit conversion to string, which is why every
    // concatenation in the corpus spells the conversion (`"Win = " + I2S(x)`).
    for rhs in ["1", "1.0", "true", "null"] {
        let src = format!(
            "function f takes nothing returns string\n    return \"a\" + {rhs}\nendfunction\n"
        );
        let e = call(&src, "f", &[]).expect_err("expected a refusal");
        assert!(
            e.message.contains("concatenates two strings") && e.message.contains("I2S/R2S"),
            "for `{rhs}`: {}",
            e.message
        );
    }
}

#[test]
fn a_mixed_real_and_integer_operand_promotes_the_integer() {
    assert_eq!(
        call(
            "function f takes nothing returns real\n    return 1000.0 * 2\nendfunction\n",
            "f",
            &[]
        )
        .expect("call"),
        Value::Real(2000.0)
    );
    assert_eq!(
        binary(BinOp::Add, &Value::Real(1.5), &Value::Int(1)).expect("add"),
        Value::Real(2.5)
    );
    // Two integers stay integers: an f32 promotion would silently lose the
    // low bit of every odd value.
    assert_eq!(
        binary(BinOp::Add, &Value::Int(1), &Value::Int(2)).expect("add"),
        Value::Int(3)
    );
}

#[test]
fn equality_needs_the_same_class() {
    assert_eq!(
        binary(BinOp::Eq, &Value::Int(1), &Value::Int(1)).expect("eq"),
        Value::Bool(true)
    );
    assert_eq!(
        binary(BinOp::Eq, &Value::Str("a".into()), &Value::Str("a".into())).expect("eq"),
        Value::Bool(true)
    );
    let e = binary(BinOp::Eq, &Value::Int(1), &Value::Bool(true)).expect_err("mismatch");
    assert!(
        matches!(e, OpError::EqualityMismatch(Class::Int, Class::Bool)),
        "{e:?}"
    );
    let e =
        binary(BinOp::Gt, &Value::Str("a".into()), &Value::Str("b".into())).expect_err("ordering");
    assert!(e.message().contains("`<`"), "{}", e.message());
    use crate::value::Class;
}

#[test]
fn boolean_operators_need_booleans() {
    let e = unary(UnOp::Not, &Value::Int(1)).expect_err("not an integer");
    assert!(matches!(e, OpError::Logic(Class::Int)), "{e:?}");
    assert_eq!(
        unary(UnOp::Not, &Value::Bool(true)).expect("not"),
        Value::Bool(false)
    );
    assert_eq!(
        unary(UnOp::Neg, &Value::Int(3)).expect("neg"),
        Value::Int(-3)
    );
    assert_eq!(
        unary(UnOp::Neg, &Value::Real(2.5)).expect("neg"),
        Value::Real(-2.5)
    );
}

#[test]
fn a_string_is_interned_by_its_contents() {
    // `udg_RedVote == "expert"` and 4,108 more map comparisons only work
    // because string equality compares the bytes.
    let program = program_of(
        "globals\n    string s = \"expert\"\nendglobals\nfunction f takes nothing returns boolean\n    return s == \"expert\"\nendfunction\n",
    );
    let mut vm = Vm::new(&program, NullHost);
    assert_eq!(vm.call("f", &[]).expect("call"), Value::Bool(true));
}

// ----- evaluation: statements --------------------------------------------

#[test]
fn a_local_shadows_a_global_inside_a_function() {
    let src = "globals\n    integer i = 1\nendglobals\nfunction f takes nothing returns integer\n    local integer i = 2\n    return i\nendfunction\nfunction g takes nothing returns integer\n    return i\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, NullHost);
    assert_eq!(vm.call("f", &[]).expect("f"), Value::Int(2));
    assert_eq!(vm.call("g", &[]).expect("g"), Value::Int(1));
}

#[test]
fn exitwhen_breaks_the_innermost_loop_and_return_escapes_it() {
    let src = "function f takes nothing returns integer\n    local integer i = 0\n    loop\n        set i = i + 1\n        exitwhen i >= 3\n    endloop\n    return i\nendfunction\nfunction g takes nothing returns integer\n    local integer i = 0\n    loop\n        loop\n            set i = i + 1\n            exitwhen i == 1\n        endloop\n        return i\n    endloop\n    return -1\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, NullHost);
    assert_eq!(vm.call("f", &[]).expect("f"), Value::Int(3));
    assert_eq!(vm.call("g", &[]).expect("g"), Value::Int(1));
}

#[test]
fn an_array_global_indexes_by_integer_and_defaults_to_null() {
    let src = "globals\n    integer array buf\nendglobals\nfunction f takes nothing returns nothing\n    set buf[2] = 5\nendfunction\nfunction g takes nothing returns integer\n    set buf[0] = 0\n    set buf[2] = 5\n    return buf[2] + buf[9]\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, NullHost);
    vm.call("f", &[]).expect("f");
    assert_eq!(vm.global_at("buf", 2), Value::Int(5));
    assert_eq!(vm.global_at("buf", 9), Value::Null);
    // Reading an unset slot is null, and null in arithmetic is a refusal.
    let e = vm.call("g", &[]).expect_err("null in arithmetic");
    assert!(e.message.contains("numbers"), "{}", e.message);
}

#[test]
fn a_function_that_reaches_endfunction_without_a_value_is_refused() {
    let src = "function f takes integer a returns integer\n    if (a < 0) then\n        return a\n    endif\nendfunction\n";
    assert_eq!(
        call(src, "f", &[Value::Int(-1)]).expect("negative path"),
        Value::Int(-1)
    );
    let e = call(src, "f", &[Value::Int(1)]).expect_err("no value on the positive path");
    assert!(e.message.contains("without a value"), "{}", e.message);
}

#[test]
fn a_recursive_function_is_bounded_by_the_depth_guard() {
    let src = "function down takes integer i returns integer\n    if (i <= 0) then\n        return 0\n    endif\n    return down(i - 1)\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, NullHost);
    assert_eq!(
        vm.call("down", &[Value::Int(5)]).expect("call"),
        Value::Int(0)
    );
    let e = vm
        .call("down", &[Value::Int(100_000)])
        .expect_err("depth guard");
    assert!(e.message.contains("recursed"), "{}", e.message);
}

#[test]
fn a_variable_of_the_same_name_as_a_function_is_not_callable() {
    let src = "function Shared takes nothing returns nothing\nendfunction\nfunction f takes nothing returns nothing\n    local integer Shared = 1\n    call Shared()\nendfunction\n";
    let (line, _, message) = where_is(src);
    assert_eq!(line, 5, "{message}");
    assert!(message.contains("a variable cannot be called"), "{message}");
}

// ----- natives -----------------------------------------------------------

#[test]
fn an_unimplemented_native_carries_its_name_the_declaration_and_the_call_span() {
    // `GetWidgetLife` is declared in common.j and nothing implements it: the
    // map must fail with the identifier, never a zero.
    let src = "native GetWidgetLife takes widget whichWidget returns real\nfunction f takes nothing returns real\n    return GetWidgetLife(null)\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, NullHost);
    let e = vm.call("f", &[]).expect_err("unimplemented native");
    assert!(e.message.contains("GetWidgetLife"), "{}", e.message);
    assert!(
        e.message.contains("takes widget whichWidget returns real"),
        "{}",
        e.message
    );
    let map = crate::span::SourceMap::new(src);
    assert_eq!(map.line_col(e.span.start), (3, 12), "{}", e.render(src));
}

#[test]
fn numeric_conversions_and_string_length_are_the_natives_the_table_runs() {
    assert_eq!(
        crate::natives::implemented("I2S"),
        Some(crate::natives::NativeImpl::IntToString)
    );
    assert_eq!(
        crate::natives::implemented("I2R"),
        Some(crate::natives::NativeImpl::IntToReal)
    );
    assert_eq!(
        crate::natives::implemented("R2I"),
        Some(crate::natives::NativeImpl::RealToInt)
    );
    assert_eq!(
        crate::natives::implemented("StringLength"),
        Some(crate::natives::NativeImpl::StringLength)
    );
    assert_eq!(crate::natives::implemented("i2r"), None);
    assert_eq!(
        crate::natives::implemented("R2S"),
        Some(crate::natives::NativeImpl::RealToString)
    );
    assert_eq!(
        crate::natives::implemented("R2SW"),
        Some(crate::natives::NativeImpl::RealToStringWidth)
    );
    assert_eq!(crate::natives::implemented("SubString"), None);
}

#[test]
fn the_host_takes_the_natives_the_table_refuses() {
    struct Stub;
    impl crate::Host for Stub {
        fn native(&mut self, name: &str, _args: &[Value]) -> Result<Value, String> {
            assert_eq!(name, "GetWidgetLife");
            Ok(Value::Real(42.5))
        }
    }
    let src = "native GetWidgetLife takes widget whichWidget returns real\nfunction f takes nothing returns real\n    return GetWidgetLife(null)\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, Stub);
    assert_eq!(vm.call("f", &[]).expect("call"), Value::Real(42.5));
}

#[test]
fn i2r_and_r2i_run_from_a_real_declaration() {
    let src = "native I2R takes integer i returns real\nnative R2I takes real r returns integer\nfunction f takes nothing returns integer\n    return R2I(I2R(3))\nendfunction\n";
    assert_eq!(call(src, "f", &[]).expect("call"), Value::Int(3));
}

#[test]
fn i2s_prints_signed_decimal_without_padding() {
    let src = "native I2S takes integer i returns string\nfunction f takes integer i returns string\n    return \"ZoneSave\" + I2S(i)\nendfunction\n";
    for (value, text) in [
        (0, "ZoneSave0"),
        (7, "ZoneSave7"),
        (-5, "ZoneSave-5"),
        (i32::MAX, "ZoneSave2147483647"),
        (i32::MIN, "ZoneSave-2147483648"),
    ] {
        assert_eq!(
            call(src, "f", &[Value::Int(value)]).expect("call"),
            Value::Str(text.into())
        );
    }
}

#[test]
fn i2s_with_a_foreign_signature_is_refused_not_run() {
    let src = "native I2S takes real r returns string\nfunction f takes nothing returns string\n    return I2S(1.5)\nendfunction\n";
    let error = call(src, "f", &[]).expect_err("a real parameter is not the shipped I2S");
    assert!(error.message.contains("I2S"), "{}", error.message);
}

// ----- the text seam -----------------------------------------------------

#[test]
fn trigger_tokens_remain_raw_until_a_native_consumes_them() {
    for (source, expected) in [
        (
            "function f takes nothing returns string\nreturn \"TRIGSTR_00374\"\nendfunction\n",
            "TRIGSTR_00374",
        ),
        (
            "globals\nstring s = TRIGSTR_00374\nendglobals\nfunction f takes nothing returns string\nreturn s\nendfunction\n",
            "TRIGSTR_00374",
        ),
    ] {
        assert_eq!(call(source, "f", &[]).unwrap(), Value::Str(expected.into()));
    }
}

#[test]
fn literal_tags_are_data_without_demanding_a_ui_formatter() {
    struct NoFormatting;
    impl crate::Host for NoFormatting {
        fn native(&mut self, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("no native")
        }
        fn code_tag(&self, _: &str) -> Option<String> {
            panic!("literal evaluation cannot format UI text")
        }
        fn table_string(&self, _: u32) -> Option<String> {
            panic!("literal evaluation cannot read WTS")
        }
    }
    let source = "function f takes nothing returns string\nreturn \"a ^^b^^ c\\n\"\nendfunction\n";
    let program = program_of(source);
    let mut vm = Vm::new(&program, NoFormatting);
    assert_eq!(vm.call("f", &[]).unwrap(), Value::Str("a ^^b^^ c\n".into()));
}

// ----- debug -------------------------------------------------------------

#[test]
fn debug_statements_run_by_default_and_can_be_dropped() {
    #[derive(Default)]
    struct Calls(usize);
    impl crate::Host for Calls {
        fn native(&mut self, _name: &str, _args: &[Value]) -> Result<Value, String> {
            self.0 += 1;
            Ok(Value::Null)
        }
    }
    let src = "native DebugOnly takes nothing returns nothing\nfunction f takes nothing returns nothing\n    debug call DebugOnly()\nendfunction\n";
    let program = program_of(src);
    let mut on = Vm::new(&program, Calls::default());
    on.call("f", &[]).expect("call");
    assert_eq!(on.host().0, 1, "a debug statement must run by default");
    let mut off = Vm::new(&program, Calls::default());
    off.set_debug(false);
    off.call("f", &[]).expect("call");
    assert_eq!(off.host().0, 0, "the gate must drop the statement");
}

// ----- the value model's own arithmetic ---------------------------------

#[test]
fn a_code_value_names_the_same_function() {
    let src = "function g takes nothing returns nothing\nendfunction\nfunction f takes nothing returns boolean\n    return function g == function g\nendfunction\nfunction h takes nothing returns boolean\n    return function g == function h\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, NullHost);
    assert_eq!(vm.call("f", &[]).expect("f"), Value::Bool(true));
    assert_eq!(vm.call("h", &[]).expect("h"), Value::Bool(false));
}

#[test]
fn a_code_value_names_a_function_and_not_something_else() {
    // A native is callable from JASS but is not a function body, so a
    // reference to one is a refusal rather than a code value that calls
    // nothing.
    let src = "native Nat takes nothing returns nothing\nfunction f takes nothing returns code\n    return function Nat\nendfunction\n";
    let e = call(src, "f", &[]).expect_err("a native is not a function");
    assert!(e.message.contains("needs a function"), "{}", e.message);

    // A name no declaration has is a resolution error, not a run error.
    let src = "function f takes nothing returns code\n    return function Nowhere\nendfunction\n";
    let (_, _, message) = where_is(src);
    assert!(message.contains("`Nowhere`"), "{message}");
}

// ----- the shipped Blizzard scripts, run --------------------------------

/// One install script by its corpus name's tail, from the install `inst` is in
/// (or any install when `None`).
#[track_caller]
fn install_script<'a>(scripts: &'a [(String, String)], inst: Option<&str>, tail: &str) -> &'a str {
    let found = scripts
        .iter()
        .find(|(n, _)| n.ends_with(tail) && inst.is_none_or(|i| n.starts_with(&format!("{i}:"))))
        .unwrap_or_else(|| panic!("no `{tail}` in the corpus"));
    &found.1
}

/// Resolve one map script against both installs' Blizzard scripts, which is
/// what the sweep does: the corpus mixes maps generated for either, and a name
/// a map needs from one of them is version skew, not a resolution error.
fn resolve_map(_name: &str, src: &str, scripts: &[(String, String)]) -> Result<(), crate::Error> {
    let mut sources: Vec<Script> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    for tail in [
        "Scripts\\common.j",
        "Scripts\\Blizzard.j",
        "Scripts\\Cheats.j",
    ] {
        for (n, src) in scripts.iter().filter(|(n, _)| n.ends_with(tail)) {
            sources.push(parse(src).expect("the install scripts parse"));
            labels.push(n.clone());
        }
    }
    let refs: Vec<Prelude<'_>> = sources
        .iter()
        .zip(&labels)
        .map(|(s, n)| Prelude::new(n.as_str(), s))
        .collect();
    let script = parse(src)?;
    resolve_with(&script, &refs).map(|_| ())
}

/// The end-to-end proof: parse the shipped `Blizzard.j`, resolve it against the
/// shipped `common.j`, and call functions out of it. The oracles are the
/// functions themselves, so the value model is checked against code the game
/// compiles rather than a hand-written expectation.
#[test]
#[ignore = "reads the install's Blizzard scripts off this machine"]
fn the_shipped_blizzard_j_functions_run() {
    let (scripts, _) = crate::tests::corpus();
    let blizzard =
        parse(install_script(&scripts, None, ":Scripts\\Blizzard.j")).expect("Blizzard.j parses");
    let common =
        parse(install_script(&scripts, None, ":Scripts\\common.j")).expect("common.j parses");
    let program = resolve_with(&blizzard, &[Prelude::new("common.j", &common)])
        .unwrap_or_else(|e| panic!("Blizzard.j failed to resolve: {}", e.render("")));

    // The shipped constants are initialised from native conversions, so the
    // run needs a host for those. The arithmetic under test calls no native at
    // all, which the count at the end of the test asserts, so nothing here
    // answers for a value the functions under test produce.
    struct Constants {
        called: Vec<String>,
    }
    impl crate::Host for Constants {
        fn native(&mut self, name: &str, _args: &[Value]) -> Result<Value, String> {
            self.called.push(name.to_string());
            if name.starts_with("Convert")
                || matches!(
                    name,
                    "IsUnitType" | "ChooseRandomItemEx" | "CreateGroup" | "CreateTimer"
                )
            {
                return Ok(Value::Null);
            }
            Err(format!("no host implements `{name}`"))
        }
    }
    let mut vm = Vm::new(&program, Constants { called: Vec::new() });
    vm.init_globals().expect("globals initialise");
    let before_constants = vm.host().called.len();
    assert!(before_constants > 0, "the constants should be initialised");
    assert_eq!(
        vm.host().called.len(),
        before_constants,
        "a function under test called a native: {:?}",
        &vm.host().called[before_constants..]
    );
    let int = |vm: &mut Vm<'_, Constants>, name: &str, args: &[Value]| -> i32 {
        match vm
            .call(name, args)
            .unwrap_or_else(|e| panic!("{name}: {}", e.message))
        {
            Value::Int(i) => i,
            other => panic!("{name} returned {other}"),
        }
    };
    let real = |vm: &mut Vm<'_, Constants>, name: &str, args: &[Value]| -> f32 {
        match vm
            .call(name, args)
            .unwrap_or_else(|e| panic!("{name}: {}", e.message))
        {
            Value::Real(r) => r,
            other => panic!("{name} returned {other}"),
        }
    };
    let boolean = |vm: &mut Vm<'_, Constants>, name: &str, args: &[Value]| -> bool {
        match vm
            .call(name, args)
            .unwrap_or_else(|e| panic!("{name}: {}", e.message))
        {
            Value::Bool(b) => b,
            other => panic!("{name} returned {other}"),
        }
    };

    assert_eq!(int(&mut vm, "IMaxBJ", &[Value::Int(3), Value::Int(7)]), 7);
    assert_eq!(int(&mut vm, "IMinBJ", &[Value::Int(3), Value::Int(7)]), 3);
    assert_eq!(int(&mut vm, "IAbsBJ", &[Value::Int(-5)]), 5);
    // The 32-bit wrap, through shipped code: `-(-2147483648)` has no other
    // representable answer.
    assert_eq!(int(&mut vm, "IAbsBJ", &[Value::Int(i32::MIN)]), i32::MIN);
    assert_eq!(
        real(&mut vm, "RMaxBJ", &[Value::Real(1.5), Value::Real(-2.5)]),
        1.5
    );
    assert!(!boolean(
        &mut vm,
        "GetBooleanAnd",
        &[Value::Bool(true), Value::Bool(false)]
    ));
    assert!(boolean(
        &mut vm,
        "GetBooleanAnd",
        &[Value::Bool(true), Value::Bool(true)]
    ));
    assert!(boolean(
        &mut vm,
        "GetBooleanOr",
        &[Value::Bool(false), Value::Bool(true)]
    ));
    assert_eq!(
        int(
            &mut vm,
            "IntegerTertiaryOp",
            &[Value::Bool(true), Value::Int(1), Value::Int(2)]
        ),
        1
    );

    // Blizzard.j's own remainder, whose comment documents the range this
    // value model implements: `(dividend / divisor) * divisor` with a
    // truncated division, shifted into `(0..divisor]`.
    assert_eq!(
        int(&mut vm, "ModuloInteger", &[Value::Int(-7), Value::Int(2)]),
        1
    );
    assert_eq!(
        int(&mut vm, "ModuloInteger", &[Value::Int(7), Value::Int(-2)]),
        1
    );
    // The same arithmetic on reals goes through I2R and R2I, the only two
    // natives this table can run.
    assert_eq!(
        real(
            &mut vm,
            "ModuloReal",
            &[Value::Real(-7.0), Value::Real(2.0)]
        ),
        1.0
    );
    assert_eq!(
        real(&mut vm, "ModuloReal", &[Value::Real(7.0), Value::Real(2.0)]),
        1.0
    );
}

/// The headline acceptance criterion, per map: DotA, a Footman Frenzy and the
/// whole map family resolve with no unresolved name beyond the four constructs
/// the sweep documents.
#[test]
#[ignore = "reads every shipped map's war3map.j off both installs"]
fn every_map_script_resolves_against_its_install() {
    let (scripts, _) = crate::tests::corpus();
    let mut failures: Vec<String> = Vec::new();
    let mut maps = 0usize;
    let mut dota = 0usize;
    let mut footman = 0usize;
    for (name, src) in &scripts {
        if !name.contains("war3map.j") {
            continue;
        }
        maps += 1;
        let lower = name.to_ascii_lowercase();
        if lower.contains("dota") {
            dota += 1;
        }
        if lower.contains("footman frenzy") || lower.contains("footmen frenzy") {
            footman += 1;
        }
        if let Err(e) = parse(src) {
            failures.push(format!("{name}: {}", e.render(src)));
            continue;
        }
        if let Err(e) = resolve_map(name, src, &scripts) {
            let rendered = e.render(src);
            let allowed = rendered.contains("`CamLock`")
                || rendered.contains("`CampaignAI`")
                || rendered.contains("`debug_fast_attacks`")
                || rendered.contains("`GetUpgradeStoneCost`");
            if !allowed {
                failures.push(format!("{name}: {rendered}"));
            }
        }
    }
    assert!(maps > 100, "the corpus must carry the maps: found {maps}");
    assert!(dota >= 1, "the corpus must carry a DotA map: found {dota}");
    assert!(
        footman >= 1,
        "the corpus must carry a Footman Frenzy: found {footman}"
    );
    assert_eq!(
        failures,
        Vec::<String>::new(),
        "map scripts that failed to resolve are listed above"
    );
}

#[test]
fn failed_global_initialization_remains_failed_without_repeating_side_effects() {
    struct Failing {
        calls: usize,
    }
    impl crate::Host for Failing {
        fn native(&mut self, _: &str, _: &[Value]) -> Result<Value, String> {
            self.calls += 1;
            Err("unsupported".into())
        }
    }
    let source = "native Missing takes nothing returns integer\nglobals\ninteger partial = 4\ninteger bad = Missing()\nendglobals\nfunction main takes nothing returns nothing\nset partial = 99\nendfunction\n";
    let program = program_of(source);
    let mut vm = Vm::new(&program, Failing { calls: 0 });
    let first = vm.call("main", &[]).unwrap_err();
    assert_eq!(vm.call("main", &[]).unwrap_err(), first);
    assert_eq!(vm.host().calls, 1);
    assert_eq!(vm.global("partial"), Value::Int(4));
    assert_eq!(vm.error_source(), Some("the map script"));
}

#[test]
fn runtime_error_retains_deepest_prelude_source() {
    let prelude = parse("native Missing takes nothing returns nothing\nfunction Inner takes nothing returns nothing\ncall Missing()\nendfunction\n").unwrap();
    let map =
        parse("function main takes nothing returns nothing\ncall Inner()\nendfunction\n").unwrap();
    let program = resolve_with(&map, &[Prelude::new("shipped/Blizzard.j", &prelude)]).unwrap();
    let mut vm = Vm::new(&program, NullHost);
    let error = vm.call("main", &[]).unwrap_err();
    assert!(error.message.contains("Missing"));
    assert_eq!(vm.error_source(), Some("shipped/Blizzard.j"));
}

#[test]
fn undeclared_prelude_calls_do_not_bypass_asset_defined_native_signatures() {
    struct Permissive {
        calls: usize,
    }
    impl crate::Host for Permissive {
        fn native(&mut self, _: &str, _: &[Value]) -> Result<Value, String> {
            self.calls += 1;
            Ok(Value::Null)
        }
    }
    let prelude =
        parse("function BJ takes nothing returns nothing\ncall NotDeclared()\nendfunction\n")
            .unwrap();
    let map =
        parse("function main takes nothing returns nothing\ncall BJ()\nendfunction\n").unwrap();
    let program = resolve_with(&map, &[Prelude::new("actual/Blizzard.j", &prelude)]).unwrap();
    let mut vm = Vm::new(&program, Permissive { calls: 0 });
    let error = vm.call("main", &[]).unwrap_err();
    assert!(error.message.contains("NotDeclared") && error.message.contains("not declared"));
    assert_eq!(vm.error_source(), Some("actual/Blizzard.j"));
    assert_eq!(vm.host().calls, 0);
}

// ----- suspension --------------------------------------------------------

const SUSPEND_SRC: &str = "\
native Mark takes nothing returns integer
native Later takes nothing returns integer
native Flag takes nothing returns boolean
native Park takes nothing returns integer
native ParkBool takes nothing returns boolean
native Sleep takes real timeout returns nothing
native Boom takes nothing returns nothing
globals
    integer g = 0
    integer array a
endglobals
function side takes nothing returns boolean
    call Mark()
    return true
endfunction
function plain takes nothing returns integer
    return 4
endfunction
function skip_and takes nothing returns boolean
    return false and ParkBool()
endfunction
function skip_or takes nothing returns boolean
    return true or ParkBool()
endfunction
function skip_user takes nothing returns boolean
    return false and side()
endfunction
function or_user takes nothing returns boolean
    return side() or Later()
endfunction
function left takes nothing returns integer
    return Mark() + Park()
endfunction
function right takes nothing returns integer
    return Park() + Mark()
endfunction
function neg takes nothing returns integer
    return -(Mark() + Park())
endfunction
function prec takes nothing returns integer
    return Mark() + Park() * Later()
endfunction
function args takes nothing returns integer
    return tri(Mark(), Park(), Later())
endfunction
function tri takes integer a, integer b, integer c returns integer
    return a + b + c
endfunction
function sc takes nothing returns boolean
    return false or (Flag() and ParkBool())
endfunction
function slot takes nothing returns integer
    set a[1] = 4
    set a[2] = 7
    return a[Mark() + Park()]
endfunction
function put takes nothing returns nothing
    set a[Park()] = Mark()
endfunction
function keep takes nothing returns integer
    local integer k = Mark() + Park()
    set g = k
    return k
endfunction
function leaf takes nothing returns integer
    call Sleep(1.0)
    return 10
endfunction
function mid takes nothing returns integer
    return leaf()
endfunction
function nest takes nothing returns integer
    set g = mid()
    return g
endfunction
function wrap takes nothing returns integer
    return leaf() + Later()
endfunction
function inner takes nothing returns nothing
    set g = 1
    call Sleep(1.0)
    set g = g + 10
endfunction
function outer takes nothing returns nothing
    local integer keep = 7
    call inner()
    set g = g + keep
endfunction
function boom takes nothing returns nothing
    set g = 1
    call Sleep(1.0)
    call Boom()
endfunction
function branch takes nothing returns nothing
    if ParkBool() then
        call Mark()
    else
        call Later()
    endif
endfunction
function spin takes nothing returns nothing
    loop
        exitwhen ParkBool()
        call Mark()
    endloop
endfunction
function budget takes nothing returns integer
    set g = 1
    return Park() + Mark()
endfunction
";

#[derive(Default)]
struct Probe {
    log: Vec<String>,
    sleep_args: Vec<Value>,
}

impl crate::Host for Probe {
    fn suspend(&mut self, name: &str, args: &[Value]) -> bool {
        match name {
            "Park" | "ParkBool" | "Sleep" => {
                if name == "Sleep" {
                    self.sleep_args.extend(args.iter().cloned());
                }
                self.log.push(format!("suspend {name}"));
                true
            }
            _ => false,
        }
    }

    fn native(&mut self, name: &str, _args: &[Value]) -> Result<Value, String> {
        self.log.push(format!("native {name}"));
        match name {
            "Mark" => Ok(Value::Int(2)),
            "Later" => Ok(Value::Int(5)),
            "Flag" => Ok(Value::Bool(true)),
            "Boom" => Err("boom".into()),
            "Park" | "ParkBool" | "Sleep" => Err("parked native must not run".into()),
            _ => Err(format!("unexpected native {name}")),
        }
    }
}

fn vm_of(program: &crate::resolve::Program) -> Vm<'_, Probe> {
    Vm::new(program, Probe::default())
}

#[test]
fn expression_parks_keep_earlier_operands_and_resume_once() {
    let program = program_of(SUSPEND_SRC);
    let mut vm = vm_of(&program);
    assert_eq!(vm.call("skip_and", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.call("skip_or", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.call("skip_user", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.call("or_user", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.host().log, vec!["native Mark".to_string()]);
    assert_eq!(vm.call("plain", &[]).unwrap(), Value::Int(4));

    let mut vm = vm_of(&program);
    let err = vm.call("left", &[]).unwrap_err();
    assert!(err.message.contains("suspended"), "{}", err.message);
    assert_eq!(
        vm.host().log,
        vec!["native Mark".to_string(), "suspend Park".to_string()]
    );
    assert!(vm.invoke("plain", &[]).is_err());
    assert_eq!(
        vm.resume(Value::Int(3)).unwrap(),
        Exec::Finished(Value::Int(5))
    );
    assert_eq!(
        vm.host().log,
        vec!["native Mark".to_string(), "suspend Park".to_string()]
    );
    assert!(vm.resume(Value::Null).is_err());

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("right", &[]).unwrap(), Exec::Suspended);
    assert!(vm.host().log.iter().all(|e| e.as_str() != "native Mark"));
    assert_eq!(
        vm.resume(Value::Int(3)).unwrap(),
        Exec::Finished(Value::Int(5))
    );
    assert_eq!(
        vm.host().log,
        vec!["suspend Park".to_string(), "native Mark".to_string()]
    );

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("neg", &[]).unwrap(), Exec::Suspended);
    assert_eq!(
        vm.host().log,
        vec!["native Mark".to_string(), "suspend Park".to_string()]
    );
    assert_eq!(
        vm.resume(Value::Int(3)).unwrap(),
        Exec::Finished(Value::Int(-5))
    );
    assert_eq!(vm.host().log.len(), 2);

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("prec", &[]).unwrap(), Exec::Suspended);
    assert_eq!(
        vm.host().log,
        vec!["native Mark".to_string(), "suspend Park".to_string()]
    );
    assert_eq!(
        vm.resume(Value::Int(4)).unwrap(),
        Exec::Finished(Value::Int(22))
    );
    assert_eq!(
        vm.host().log,
        vec![
            "native Mark".to_string(),
            "suspend Park".to_string(),
            "native Later".to_string()
        ]
    );

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("args", &[]).unwrap(), Exec::Suspended);
    assert_eq!(
        vm.host().log,
        vec!["native Mark".to_string(), "suspend Park".to_string()]
    );
    assert_eq!(vm.depth(), 1);
    assert_eq!(
        vm.resume(Value::Int(10)).unwrap(),
        Exec::Finished(Value::Int(17))
    );
    assert_eq!(
        vm.host().log,
        vec![
            "native Mark".to_string(),
            "suspend Park".to_string(),
            "native Later".to_string()
        ]
    );
    assert_eq!(vm.depth(), 0);

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("sc", &[]).unwrap(), Exec::Suspended);
    assert_eq!(
        vm.host().log,
        vec!["native Flag".to_string(), "suspend ParkBool".to_string()]
    );
    assert_eq!(
        vm.resume(Value::Bool(false)).unwrap(),
        Exec::Finished(Value::Bool(false))
    );
    assert_eq!(vm.host().log.len(), 2);

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("slot", &[]).unwrap(), Exec::Suspended);
    assert_eq!(vm.global_at("a", 2), Value::Int(7));
    assert_eq!(
        vm.resume(Value::Int(0)).unwrap(),
        Exec::Finished(Value::Int(7))
    );
    assert_eq!(
        vm.host()
            .log
            .iter()
            .filter(|e| e.as_str() == "native Mark")
            .count(),
        1
    );

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("put", &[]).unwrap(), Exec::Suspended);
    assert_eq!(vm.global_at("a", 1), Value::Null);
    assert_eq!(
        vm.host().log,
        vec!["native Mark".to_string(), "suspend Park".to_string()]
    );
    assert_eq!(
        vm.resume(Value::Int(1)).unwrap(),
        Exec::Finished(Value::Null)
    );
    assert_eq!(vm.global_at("a", 1), Value::Int(2));
    assert_eq!(vm.host().log.len(), 2);

    let mut vm = vm_of(&program);
    let before = {
        assert_eq!(vm.invoke("keep", &[]).unwrap(), Exec::Suspended);
        assert_eq!(vm.global("g"), Value::Int(0));
        vm.steps()
    };
    assert!(before > 0);
    assert_eq!(
        vm.resume(Value::Int(3)).unwrap(),
        Exec::Finished(Value::Int(5))
    );
    assert_eq!(vm.global("g"), Value::Int(5));
    assert!(vm.steps() > before);

    let owned = Arc::new(program_of(SUSPEND_SRC));
    let mut vm = OwnedVm::new_owned(Arc::clone(&owned), Probe::default());
    assert_eq!(
        vm.invoke("plain", &[]).unwrap(),
        Exec::Finished(Value::Int(4))
    );
}

#[test]
fn nested_calls_if_and_loop_resume_without_replaying() {
    let program = program_of(SUSPEND_SRC);
    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("nest", &[]).unwrap(), Exec::Suspended);
    assert_eq!(vm.depth(), 3);
    assert_eq!(vm.global("g"), Value::Int(0));
    assert_eq!(vm.host().sleep_args, vec![Value::Real(1.0)]);
    assert!(vm.host().log.iter().all(|e| e.starts_with("suspend")));
    let steps = vm.steps();
    assert_eq!(
        vm.resume(Value::Int(99)).unwrap(),
        Exec::Finished(Value::Int(10))
    );
    assert_eq!(vm.global("g"), Value::Int(10));
    assert_eq!(vm.depth(), 0);
    assert!(vm.steps() > steps);

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("wrap", &[]).unwrap(), Exec::Suspended);
    assert!(vm.host().log.iter().all(|e| e.as_str() != "native Later"));
    assert_eq!(
        vm.resume(Value::Null).unwrap(),
        Exec::Finished(Value::Int(15))
    );
    assert_eq!(
        vm.host().log,
        vec!["suspend Sleep".to_string(), "native Later".to_string()]
    );

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("outer", &[]).unwrap(), Exec::Suspended);
    assert_eq!(vm.global("g"), Value::Int(1));
    let blocked = vm.call("plain", &[]).unwrap_err();
    assert!(blocked.message.contains("suspended"), "{}", blocked.message);
    assert_eq!(vm.global("g"), Value::Int(1));
    assert_eq!(
        vm.resume(Value::Int(99)).unwrap(),
        Exec::Finished(Value::Null)
    );
    assert_eq!(vm.global("g"), Value::Int(18));
    assert_eq!(vm.call("plain", &[]).unwrap(), Value::Int(4));

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("boom", &[]).unwrap(), Exec::Suspended);
    let error = vm.resume(Value::Null).unwrap_err();
    assert!(error.message.contains("Boom"), "{}", error.message);
    assert_ne!(error.span, Span::EMPTY);
    assert_eq!(vm.global("g"), Value::Int(1));
    assert_eq!(vm.error_source(), Some("the map script"));
    assert_eq!(vm.depth(), 0);
    assert!(vm.resume(Value::Null).is_err());

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("branch", &[]).unwrap(), Exec::Suspended);
    assert!(vm.host().log.iter().all(|e| !e.starts_with("native")));
    assert_eq!(
        vm.resume(Value::Bool(true)).unwrap(),
        Exec::Finished(Value::Null)
    );
    assert_eq!(
        vm.host().log,
        vec!["suspend ParkBool".to_string(), "native Mark".to_string()]
    );

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("branch", &[]).unwrap(), Exec::Suspended);
    assert_eq!(
        vm.resume(Value::Bool(false)).unwrap(),
        Exec::Finished(Value::Null)
    );
    assert_eq!(
        vm.host().log,
        vec!["suspend ParkBool".to_string(), "native Later".to_string()]
    );

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("spin", &[]).unwrap(), Exec::Suspended);
    assert_eq!(vm.resume(Value::Bool(false)).unwrap(), Exec::Suspended);
    assert_eq!(
        vm.host().log,
        vec![
            "suspend ParkBool".to_string(),
            "native Mark".to_string(),
            "suspend ParkBool".to_string()
        ]
    );
    assert_eq!(
        vm.resume(Value::Bool(true)).unwrap(),
        Exec::Finished(Value::Null)
    );
    assert_eq!(
        vm.host().log.iter().filter(|e| *e == "native Mark").count(),
        1
    );
}

#[test]
fn invalid_resume_reentry_budget_and_init_do_not_pretend_success() {
    let program = program_of(SUSPEND_SRC);
    let mut idle = vm_of(&program);
    assert!(
        idle.resume(Value::Null)
            .unwrap_err()
            .message
            .contains("not suspended")
    );

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("budget", &[]).unwrap(), Exec::Suspended);
    assert_eq!(vm.global("g"), Value::Int(1));
    assert!(vm.host().log.iter().all(|e| e.as_str() != "native Mark"));
    let parked = vm.steps();
    vm.set_steps_for_test(MAX_STEPS);
    let error = vm.resume(Value::Int(1)).unwrap_err();
    assert!(error.message.contains("steps"), "{}", error.message);
    assert_ne!(error.span, Span::EMPTY);
    assert_eq!(vm.global("g"), Value::Int(1));
    assert_eq!(vm.depth(), 0);
    assert!(vm.host().log.iter().all(|e| e.as_str() != "native Mark"));
    assert!(vm.resume(Value::Null).is_err());
    assert!(parked > 0);

    let mut vm = vm_of(&program);
    assert_eq!(vm.invoke("left", &[]).unwrap(), Exec::Suspended);
    vm.set_busy_for_test(true);
    assert!(
        vm.resume(Value::Int(1))
            .unwrap_err()
            .message
            .contains("reentrant")
    );
    assert!(
        vm.invoke("plain", &[])
            .unwrap_err()
            .message
            .contains("reentrant")
    );
    vm.set_busy_for_test(false);
    assert_eq!(vm.global("g"), Value::Int(0));
    assert_eq!(
        vm.resume(Value::Int(3)).unwrap(),
        Exec::Finished(Value::Int(5))
    );

    let bad = "\
native Mark takes nothing returns integer
native Park takes nothing returns integer
globals
    integer partial = 4
    integer bad = Mark() + Park()
endglobals
function main takes nothing returns nothing
endfunction
";
    let program = program_of(bad);
    let mut vm = vm_of(&program);
    let first = vm.init_globals().unwrap_err();
    assert!(first.message.contains("suspended"), "{}", first.message);
    assert_eq!(vm.init_globals().unwrap_err(), first);
    assert_eq!(
        vm.host().log,
        vec!["native Mark".to_string(), "suspend Park".to_string()]
    );
    assert_eq!(vm.global("partial"), Value::Int(4));
    assert_eq!(vm.global("bad"), Value::Null);
    assert_eq!(vm.depth(), 0);
    assert!(vm.resume(Value::Int(1)).is_err());
    assert!(
        vm.call("main", &[])
            .unwrap_err()
            .message
            .contains("suspended")
    );
}

#[test]
fn returning_calls_share_one_function_body() {
    for calls in [32, 128] {
        let mut source = "globals\ninteger total = 0\nendglobals\nfunction child takes nothing returns integer\nreturn 1\nendfunction\nfunction main takes nothing returns nothing\n".to_owned();
        for _ in 0..calls {
            source.push_str("set total = total + child()\n");
        }
        source.push_str("endfunction\n");
        let program = program_of(&source);
        let mut vm = Vm::new(&program, NullHost);
        vm.call("main", &[]).unwrap();
        assert_eq!(vm.global("total"), Value::Int(calls));
        let body = program
            .function("child")
            .and_then(|f| f.body.as_ref())
            .unwrap();
        assert_eq!(
            Arc::strong_count(body),
            1,
            "activations released the shared body"
        );
    }
}

fn run_wide_argument_list() {
    const ARGS: usize = 4096;
    let params = (0..ARGS)
        .map(|i| format!("integer a{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let args = vec!["1"; ARGS].join(", ");
    let source = format!(
        "native Wide takes {params} returns integer\nfunction main takes nothing returns integer\nreturn Wide({args})\nendfunction\n"
    );
    struct Wide;
    impl crate::Host for Wide {
        fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
            assert_eq!(name, "Wide");
            assert!(args.iter().all(|v| *v == Value::Int(1)));
            Ok(Value::Int(args.len() as i32))
        }
    }
    let program = program_of(&source);
    assert_eq!(
        Vm::new(&program, Wide).call("main", &[]).unwrap(),
        Value::Int(ARGS as i32)
    );
    println!("flat-arguments:{ARGS}");
}

#[test]
fn flat_argument_lists_keep_the_native_stack_bounded() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests_vm::wide_argument_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("PCX_WIDE_ARGUMENT_CHILD", "1")
            .env("RUST_MIN_STACK", "262144")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "status={:?}\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("flat-arguments:4096"));
    }
    #[cfg(target_arch = "wasm32")]
    run_wide_argument_list();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn wide_argument_child() {
    if std::env::var_os("PCX_WIDE_ARGUMENT_CHILD").is_some() {
        run_wide_argument_list();
    }
}

#[test]
fn returning_arguments_share_the_unevaluated_tail() {
    for count in [32, 128] {
        let params = (0..count)
            .map(|i| format!("integer a{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let args = vec!["child()"; count].join(",");
        let source = format!(
            "native Count takes {params} returns integer\nfunction child takes nothing returns integer\nreturn 1\nendfunction\nfunction main takes nothing returns integer\nreturn Count({args})\nendfunction\n"
        );
        struct Count;
        impl crate::Host for Count {
            fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
                assert_eq!(name, "Count");
                Ok(Value::Int(args.len() as i32))
            }
        }
        let program = program_of(&source);
        let mut vm = Vm::new(&program, Count);
        assert_eq!(vm.call("main", &[]).unwrap(), Value::Int(count as i32));
        assert!(
            vm.copied_argument_expressions() <= count,
            "{count} arguments copied {} expressions",
            vm.copied_argument_expressions()
        );
    }
}

#[test]
fn parked_control_flow_matches_synchronous_effects_and_step_work() {
    #[derive(Default)]
    struct Effects {
        park: bool,
        pending: Option<Value>,
        calls: Vec<(String, Vec<Value>)>,
    }
    impl crate::Host for Effects {
        fn suspend(&mut self, name: &str, args: &[Value]) -> bool {
            if !self.park || name != "Hop" {
                return false;
            }
            let [Value::Int(value)] = args else {
                panic!("synthetic Hop args")
            };
            assert!(self.pending.is_none());
            self.calls.push((name.into(), args.to_vec()));
            self.pending = Some(Value::Int(value + 1));
            true
        }
        fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
            let [Value::Int(value)] = args else {
                panic!("synthetic value args")
            };
            assert!(!(self.park && name == "Hop"));
            self.calls.push((name.into(), args.to_vec()));
            match name {
                "Hop" => Ok(Value::Int(value + 1)),
                "Tag" => Ok(Value::Int(value + 2)),
                _ => Err(name.into()),
            }
        }
    }
    let prefix = "native Hop takes integer n returns integer\nnative Tag takes integer n returns integer\nglobals\ninteger g=0\ninteger array a\nendglobals\nfunction inner takes integer n returns integer\nreturn Hop(n)+Tag(n)\nendfunction\nfunction choose takes integer n returns integer\nif Hop(n)>0 then\nreturn inner(n)\nelse\nreturn Hop(n)\nendif\nendfunction\n";
    let bodies = [
        "set g=Tag(1)+inner(Tag(2))+Hop(Tag(3))\nreturn g\n",
        "set a[Hop(Tag(1))]=inner(Tag(5))\nreturn a[Hop(Tag(1))]\n",
        "local integer k=inner(Hop(2))\nset g=Hop(k)+choose(Hop(0))\nreturn g\n",
        "if Hop(0)>1 then\nset g=100\nelseif Hop(1)==2 then\nset g=choose(2)\nelse\nset g=200\nendif\nreturn g\n",
        "loop\nexitwhen Hop(g)>2\nif Hop(g)>0 then\nset g=g+1\nendif\nendloop\nreturn inner(g)\n",
        "debug if Hop(1)>0 then\ndebug set g=inner(1)\nendif\nreturn Hop(g)\n",
        "loop\nif Hop(g)>0 then\nreturn inner(Hop(g))\nendif\nendloop\n",
        "if (Hop(0)==1 and Hop(1)==2) or Hop(99)==0 then\nset g=Hop(2)\nendif\nreturn g\n",
        "if false and Hop(99)==0 then\nset g=1\nendif\nif true or Hop(98)==0 then\nset g=Hop(0)\nendif\nreturn g\n",
    ];
    for body in bodies {
        for debug in [false, true] {
            let program = program_of(&format!(
                "{prefix}function main takes nothing returns integer\n{body}endfunction\n"
            ));
            let mut sync = Vm::new(&program, Effects::default());
            sync.set_debug(debug);
            let expected = sync.invoke("main", &[]).unwrap();
            assert!(matches!(expected, crate::Exec::Finished(_)));
            let mut parked = Vm::new(
                &program,
                Effects {
                    park: true,
                    ..Default::default()
                },
            );
            parked.set_debug(debug);
            let mut result = parked.invoke("main", &[]).unwrap();
            let mut parks = 0;
            while result == crate::Exec::Suspended {
                parks += 1;
                assert!(parks < 128);
                let value = parked.host().pending.take().unwrap();
                result = parked.resume(value).unwrap();
            }
            assert_eq!(result, expected, "{body}");
            assert!(parks > 0);
            assert_eq!(
                parked.host().calls,
                sync.host().calls,
                "effect order: {body}"
            );
            assert_eq!(parked.steps(), sync.steps(), "step work: {body}");
            assert_eq!(parked.depth(), 0);
            assert_eq!(parked.global("g"), sync.global("g"));
            for index in 0..8 {
                assert_eq!(parked.global_at("a", index), sync.global_at("a", index));
            }
        }
    }
}

#[test]
fn legacy_park_errors_keep_the_site_and_resume_relabels_later_errors() {
    let prelude = "function Inner takes nothing returns integer\nreturn Park()+1\nendfunction\n";
    let map = "native Park takes nothing returns integer\nnative Boom takes nothing returns integer\nfunction main takes nothing returns integer\nreturn Inner()+Boom()\nendfunction\n";
    let prelude_ast = parse(prelude).unwrap();
    let map_ast = parse(map).unwrap();
    let program =
        resolve_with(&map_ast, &[Prelude::new("synthetic/park.j", &prelude_ast)]).unwrap();
    let mut vm = Vm::new(&program, Probe::default());
    let parked = vm.call("main", &[]).unwrap_err();
    assert!(parked.message.contains("suspended"));
    let at = prelude.find("Park()").unwrap();
    assert_eq!(parked.span, crate::Span::new(at, at + 6));
    assert_eq!(vm.error_source(), Some("synthetic/park.j"));
    let resumed = vm.resume(Value::Int(4)).unwrap_err();
    let at = map.find("Boom()").unwrap();
    assert_eq!(resumed.span, crate::Span::new(at, at + 6));
    assert_eq!(vm.error_source(), Some("the map script"));
    assert!(resumed.message.contains("Boom"));
}

// ----- threads ------------------------------------------------------------

const THREAD_SRC: &str = "native Sleep takes real r returns nothing
native Exec takes nothing returns nothing
native Mark takes integer i returns nothing
globals
integer g = 0
endglobals
function a takes nothing returns nothing
    call Mark(1)
    call Sleep(2.0)
    call Mark(2)
endfunction
function b takes nothing returns nothing
    call Mark(3)
endfunction
function outer takes nothing returns nothing
    call Mark(0)
    call Exec()
    call Mark(4)
endfunction
";

#[derive(Default)]
struct Threads {
    log: Vec<String>,
    next: u64,
    spawn: Vec<crate::value::FunctionId>,
}

impl crate::Host for Threads {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        self.log.push(format!("{name} {args:?}"));
        Ok(Value::Null)
    }
    fn suspend(&mut self, name: &str, _: &[Value]) -> bool {
        name == "Sleep"
    }
    fn begin_script_call(
        &mut self,
        name: &str,
        _: &[Value],
    ) -> Result<Option<crate::ScriptCall>, String> {
        Ok((name == "Exec").then(|| crate::ScriptCall::Spawn(self.spawn.clone())))
    }
    fn park_thread(&mut self) -> Result<u64, String> {
        self.next += 1;
        self.log.push(format!("park {}", self.next));
        Ok(self.next)
    }
    fn resume_thread(&mut self, ticket: u64) -> Result<(), String> {
        self.log.push(format!("resume {ticket}"));
        Ok(())
    }
    fn leave_thread(&mut self) -> Result<(), String> {
        self.log.push("leave".into());
        Ok(())
    }
}

#[test]
fn sequential_spawn_followups_spend_fuel_without_growing_the_rust_stack() {
    #[derive(Default)]
    struct Followups {
        function: Option<crate::value::FunctionId>,
        followups: usize,
        leaves: usize,
        aborts: usize,
    }
    impl crate::Host for Followups {
        fn native(&mut self, name: &str, _: &[Value]) -> Result<Value, String> {
            Err(format!("unexpected native {name}"))
        }
        fn begin_script_call(
            &mut self,
            name: &str,
            args: &[Value],
        ) -> Result<Option<crate::ScriptCall>, String> {
            let [Value::Code(function)] = args else {
                return Err("callback required".into());
            };
            self.function = Some(*function);
            Ok((name == "Drive").then(|| crate::ScriptCall::Spawn(vec![*function])))
        }
        fn after_script_spawn(&mut self) -> Result<Option<crate::ScriptCall>, String> {
            self.followups += 1;
            Ok(Some(crate::ScriptCall::Spawn(vec![
                self.function.expect("script call seeded the function"),
            ])))
        }
        fn leave_thread(&mut self) -> Result<(), String> {
            self.leaves += 1;
            Ok(())
        }
        fn abort_script_calls(&mut self) {
            self.aborts += 1;
        }
    }
    let program = program_of(
        "native Drive takes code callback returns nothing
function empty takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call Drive(function empty)
endfunction
",
    );
    let mut vm = Vm::new(&program, Followups::default());
    let error = vm.call("main", &[]).unwrap_err();
    assert!(error.to_string().contains("steps"), "{error}");
    assert!(vm.host().followups > 0);
    let host = vm.host();
    assert_eq!(host.followups, host.leaves);
    assert_eq!(vm.host().aborts, 1);
    assert_eq!(vm.call("empty", &[]).unwrap(), Value::Null);
    assert!(
        vm.host().followups as u64 <= MAX_STEPS,
        "the fuel check bounds sequential follow-ups: {}",
        vm.host().followups
    );
}

#[test]
fn spawned_threads_park_alone_and_resume_with_their_remaining_callbacks() {
    let program = program_of(THREAD_SRC);
    let id = |name| program.function_id(name).unwrap();
    let mut vm = Vm::new(
        &program,
        Threads {
            spawn: vec![id("a"), id("b")],
            ..Threads::default()
        },
    );
    vm.init_globals().unwrap();
    assert_eq!(vm.call("outer", &[]).unwrap(), Value::Null);
    assert_eq!(
        vm.host().log,
        [
            "Mark [Int(0)]",
            "Mark [Int(1)]",
            "park 1",
            "leave",
            "Mark [Int(4)]"
        ]
    );
    assert_eq!(vm.parked_threads().collect::<Vec<_>>(), [1]);
    vm.host().log.clear();
    vm.resume_thread(1).unwrap();
    assert_eq!(
        vm.host().log,
        ["resume 1", "Mark [Int(2)]", "Mark [Int(3)]", "leave"]
    );
    assert_eq!(vm.parked_threads().count(), 0);
    assert!(vm.resume_thread(1).is_err());

    // A top-level thread parks without a leave; its tail runs on resume.
    vm.host().log.clear();
    vm.run_thread(&[id("a"), id("b")]).unwrap();
    assert_eq!(vm.host().log, ["Mark [Int(1)]", "park 2"]);
    vm.resume_thread(2).unwrap();
    assert_eq!(vm.parked_threads().count(), 0);

    // Hosts without thread support refuse the park loudly.
    let mut plain = Vm::new(&program, Probe::default());
    plain.init_globals().unwrap();
    assert!(plain.run_thread(&[id("b")]).is_ok());
}

#[derive(Default)]
struct SpawnProbe {
    observed: Vec<i32>,
    next: u64,
    starts: usize,
    leaves: usize,
    followups: usize,
    aborts: usize,
    fail_leave: bool,
    fail_followup: bool,
    reuse_ticket: bool,
}

impl crate::Host for SpawnProbe {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        match (name, args) {
            ("Observe", [Value::Int(value)]) => {
                self.observed.push(*value);
                Ok(Value::Null)
            }
            ("Boom", []) => Err("synthetic child failure".into()),
            _ => Err(format!("unexpected native {name}")),
        }
    }
    fn begin_script_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<crate::ScriptCall>, String> {
        if name != "Spawn" {
            return Ok(None);
        }
        self.starts += 1;
        let functions = args
            .iter()
            .filter_map(|v| match v {
                Value::Code(id) => Some(*id),
                _ => None,
            })
            .collect();
        Ok(Some(crate::ScriptCall::Spawn(functions)))
    }
    fn suspend(&mut self, name: &str, _: &[Value]) -> bool {
        name == "Sleep"
    }
    fn park_thread(&mut self) -> Result<u64, String> {
        self.next += 1;
        Ok(if self.reuse_ticket { 1 } else { self.next })
    }
    fn resume_thread(&mut self, _: u64) -> Result<(), String> {
        Ok(())
    }
    fn leave_thread(&mut self) -> Result<(), String> {
        self.leaves += 1;
        if self.fail_leave {
            Err("synthetic leave failure".into())
        } else {
            Ok(())
        }
    }
    fn after_script_spawn(&mut self) -> Result<Option<crate::ScriptCall>, String> {
        self.followups += 1;
        if self.fail_followup {
            Err("synthetic followup failure".into())
        } else {
            Ok(Some(crate::ScriptCall::Return(Value::Int(5))))
        }
    }
    fn abort_script_calls(&mut self) {
        self.aborts += 1;
    }
}

const SPAWN_PROBE: &str = "\
native Spawn takes code first, code tail returns integer
native Sleep takes real seconds returns nothing
native Observe takes integer value returns nothing
native Boom takes nothing returns nothing
function empty takes nothing returns nothing
endfunction
function tail takes nothing returns nothing
call Observe(4)
endfunction
function safe takes nothing returns integer
return 71
endfunction
";

#[test]
fn nested_spawn_boundaries_keep_parent_expression_and_reparked_child_tails() {
    let source = format!(
        "{SPAWN_PROBE}\
globals
integer result = 0
endglobals
function grand_tail takes nothing returns nothing
call Observe(14)
endfunction
function grand takes nothing returns nothing
local integer retained = 477
call Observe(11)
call Sleep(1.0)
call Observe(retained)
call Sleep(2.0)
call Observe(retained+1)
endfunction
function child takes nothing returns nothing
local integer retained = 701
call Observe(1)
call Spawn(function grand,function grand_tail)
call Observe(retained)
call Sleep(3.0)
call Observe(retained+1)
endfunction
function main takes nothing returns integer
local integer retained = 701
local integer array values
set values[2] = 13
set result = 11 + Spawn(function child,function tail)*3
return retained + values[2] + result
endfunction
"
    );
    let program = program_of(&source);
    let mut vm = Vm::new(&program, SpawnProbe::default());
    assert_eq!(vm.call("main", &[]).unwrap(), Value::Int(740));
    assert_eq!(vm.global("result"), Value::Int(26));
    assert_eq!(vm.host().observed, [1, 11, 701]);
    assert_eq!(vm.parked_threads().collect::<Vec<_>>(), [1, 2]);
    assert_eq!((vm.host().leaves, vm.host().followups), (2, 2));
    vm.resume_thread(1).unwrap();
    assert_eq!(vm.parked_threads().collect::<Vec<_>>(), [2, 3]);
    vm.resume_thread(2).unwrap();
    vm.resume_thread(3).unwrap();
    assert_eq!(vm.host().observed, [1, 11, 701, 477, 702, 4, 478, 14]);
    assert_eq!(
        (vm.host().leaves, vm.host().followups, vm.host().aborts),
        (5, 2, 0)
    );
    assert_eq!(vm.parked_threads().count(), 0);
    assert_eq!(vm.call("safe", &[]).unwrap(), Value::Int(71));
}

#[test]
fn spawn_boundary_child_error_keeps_source_and_skips_parent_followup() {
    let prelude =
        "function child takes nothing returns nothing\ncall Observe(1)\ncall Boom()\nendfunction\n";
    let source = format!(
        "{SPAWN_PROBE}function main takes nothing returns nothing\ncall Spawn(function child,function tail)\ncall Observe(9)\nendfunction\n"
    );
    let prelude_ast = parse(prelude).unwrap();
    let map_ast = parse(&source).unwrap();
    let program =
        resolve_with(&map_ast, &[Prelude::new("synthetic/child.j", &prelude_ast)]).unwrap();
    let mut vm = Vm::new(&program, SpawnProbe::default());
    let error = vm.call("main", &[]).unwrap_err();
    let at = prelude.find("call Boom()").unwrap();
    assert_eq!(error.span, crate::Span::new(at, at + 11));
    assert_eq!(vm.error_source(), Some("synthetic/child.j"));
    assert_eq!(vm.host().observed, [1]);
    assert_eq!(
        (vm.host().leaves, vm.host().followups, vm.host().aborts),
        (0, 0, 1)
    );
    assert_eq!(vm.call("safe", &[]).unwrap(), Value::Int(71));
}

#[test]
fn spawn_boundary_invalid_tail_keeps_native_caller_span() {
    let source = format!(
        "{SPAWN_PROBE}function bad takes integer value returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall Spawn(function tail,function bad)\nendfunction\n"
    );
    let program = program_of(&source);
    let mut vm = Vm::new(&program, SpawnProbe::default());
    let error = vm.call("main", &[]).unwrap_err();
    let at = source
        .find("call Spawn(function tail,function bad)")
        .unwrap();
    assert_eq!(
        error.span,
        crate::Span::new(at, at + "call Spawn(function tail,function bad)".len())
    );
    assert_eq!(vm.error_source(), Some("the map script"));
    assert!(error.message.contains("callback must take nothing"));
    assert_eq!(vm.host().observed, [4]);
    assert_eq!(
        (vm.host().leaves, vm.host().followups, vm.host().aborts),
        (0, 0, 1)
    );
    assert_eq!(vm.call("safe", &[]).unwrap(), Value::Int(71));
}

#[test]
fn spawn_boundary_nesting_limit_unwinds_and_vm_can_run_again() {
    let source = format!(
        "{SPAWN_PROBE}function recurse takes nothing returns nothing\ncall Spawn(function recurse,function empty)\nendfunction\n"
    );
    let program = program_of(&source);
    let mut vm = Vm::new(&program, SpawnProbe::default());
    let error = vm.call("recurse", &[]).unwrap_err();
    assert!(error.message.contains("spawns"), "{error}");
    assert_eq!(vm.host().starts, crate::eval::MAX_SPAWN_DEPTH as usize + 1);
    assert_eq!(
        (vm.host().leaves, vm.host().followups, vm.host().aborts),
        (0, 0, 1)
    );
    assert_eq!(vm.call("safe", &[]).unwrap(), Value::Int(71));
}

#[test]
fn spawn_boundary_leave_and_followup_failures_do_not_repeat_child_effects() {
    let source = format!(
        "{SPAWN_PROBE}function main takes nothing returns nothing\ncall Spawn(function tail,function empty)\ncall Observe(9)\nendfunction\n"
    );
    let program = program_of(&source);
    for fail_leave in [false, true] {
        let mut vm = Vm::new(
            &program,
            SpawnProbe {
                fail_leave,
                fail_followup: !fail_leave,
                ..SpawnProbe::default()
            },
        );
        let error = vm.call("main", &[]).unwrap_err();
        let at = source
            .find("call Spawn(function tail,function empty)")
            .unwrap();
        assert_eq!(
            error.span,
            crate::Span::new(at, at + "call Spawn(function tail,function empty)".len())
        );
        assert_eq!(vm.host().observed, [4]);
        assert_eq!(
            (vm.host().leaves, vm.host().followups, vm.host().aborts),
            (1, usize::from(!fail_leave), 1)
        );
        assert_eq!(vm.call("safe", &[]).unwrap(), Value::Int(71));
        assert_eq!(vm.host().observed, [4]);
    }
}

#[test]
fn spawn_boundary_reused_ticket_preserves_the_previously_parked_thread() {
    let source = format!(
        "{SPAWN_PROBE}function child takes nothing returns nothing\nlocal integer retained=477\ncall Sleep(1.0)\ncall Observe(retained)\nendfunction\nfunction main takes nothing returns nothing\ncall Spawn(function child,function tail)\ncall Spawn(function child,function tail)\nendfunction\n"
    );
    let program = program_of(&source);
    let mut vm = Vm::new(
        &program,
        SpawnProbe {
            reuse_ticket: true,
            ..SpawnProbe::default()
        },
    );
    let error = vm.call("main", &[]).unwrap_err();
    assert!(error.message.contains("reused a parked thread ticket"));
    assert_eq!(vm.parked_threads().collect::<Vec<_>>(), [1]);
    assert_eq!(
        (vm.host().leaves, vm.host().followups, vm.host().aborts),
        (1, 1, 1)
    );
    vm.resume_thread(1).unwrap();
    assert_eq!(vm.host().observed, [477, 4]);
    assert_eq!(vm.parked_threads().count(), 0);
    assert_eq!(vm.call("safe", &[]).unwrap(), Value::Int(71));
}

#[test]
fn root_thread_entry_keeps_callback_tail_across_repark_and_nested_spawn() {
    let source = format!(
        "{SPAWN_PROBE}function first takes nothing returns nothing\nlocal integer retained=901\ncall Sleep(1.0)\ncall Observe(retained)\ncall Sleep(2.0)\ncall Observe(retained+1)\nendfunction\nfunction last takes nothing returns nothing\ncall Spawn(function empty,function tail)\ncall Observe(9)\nendfunction\n"
    );
    let program = program_of(&source);
    let mut vm = Vm::new(&program, SpawnProbe::default());
    vm.run_thread(&[
        program.function_id("first").unwrap(),
        program.function_id("last").unwrap(),
    ])
    .unwrap();
    assert_eq!(vm.parked_threads().collect::<Vec<_>>(), [1]);
    assert_eq!((vm.host().leaves, vm.host().followups), (0, 0));
    vm.resume_thread(1).unwrap();
    assert_eq!(vm.host().observed, [901]);
    assert_eq!(vm.parked_threads().collect::<Vec<_>>(), [2]);
    assert_eq!((vm.host().leaves, vm.host().followups), (1, 0));
    vm.resume_thread(2).unwrap();
    assert_eq!(vm.host().observed, [901, 902, 4, 9]);
    assert_eq!(
        (vm.host().leaves, vm.host().followups, vm.host().aborts),
        (3, 1, 0)
    );
    assert_eq!(vm.parked_threads().count(), 0);
    vm.run_thread(&[]).unwrap();
    assert_eq!(vm.host().leaves, 3);
}

#[test]
fn root_thread_entry_tail_failure_aborts_without_running_leave_or_followup() {
    for resumes in [false, true] {
        let source = format!(
            "{SPAWN_PROBE}function first takes nothing returns nothing\n{}call Observe(1)\nendfunction\nfunction bad takes integer value returns nothing\nendfunction\n",
            if resumes { "call Sleep(1.0)\n" } else { "" }
        );
        let program = program_of(&source);
        let mut vm = Vm::new(&program, SpawnProbe::default());
        let start = vm.run_thread(&[
            program.function_id("first").unwrap(),
            program.function_id("bad").unwrap(),
        ]);
        let error = if resumes {
            start.unwrap();
            vm.resume_thread(1).unwrap_err()
        } else {
            start.unwrap_err()
        };
        assert!(error.message.contains("callback must take nothing"));
        assert_eq!(error.span, crate::Span::EMPTY);
        assert_eq!(vm.host().observed, [1]);
        assert_eq!(
            (vm.host().leaves, vm.host().followups, vm.host().aborts),
            (0, 0, 1)
        );
        assert_eq!(vm.call("safe", &[]).unwrap(), Value::Int(71));
        vm.run_thread(&[program.function_id("tail").unwrap()])
            .unwrap();
        assert_eq!(vm.host().observed, [1, 4]);
    }
}

#[test]
fn empty_spawn_boundary_returns_through_parent_expression_once() {
    let source = format!(
        "{SPAWN_PROBE}function main takes nothing returns integer\nreturn 11+Spawn(null,null)*3\nendfunction\n"
    );
    let program = program_of(&source);
    let mut vm = Vm::new(&program, SpawnProbe::default());
    assert_eq!(vm.call("main", &[]).unwrap(), Value::Int(26));
    assert_eq!(
        (vm.host().starts, vm.host().leaves, vm.host().followups),
        (1, 1, 1)
    );
}

#[test]
fn global_initializer_spawn_preserves_return_expression_and_parked_tail() {
    let source = format!(
        "{SPAWN_PROBE}globals\ninteger result=3+Spawn(function child,function tail)*2\nendglobals\nfunction child takes nothing returns nothing\nlocal integer retained=477\ncall Sleep(1.0)\ncall Observe(retained)\nendfunction\n"
    );
    let program = program_of(&source);
    let mut vm = Vm::new(&program, SpawnProbe::default());
    vm.init_globals().unwrap();
    assert_eq!(vm.global("result"), Value::Int(13));
    assert_eq!(vm.parked_threads().collect::<Vec<_>>(), [1]);
    assert_eq!((vm.host().leaves, vm.host().followups), (1, 1));
    vm.resume_thread(1).unwrap();
    assert_eq!(vm.host().observed, [477, 4]);
    assert_eq!((vm.host().leaves, vm.host().followups), (2, 1));
}

#[test]
fn string_length_counts_bytes_and_stops_at_nul_through_vm_calls() {
    let src = "native StringLength takes string s returns integer\nfunction f takes string s returns integer\nreturn StringLength(s)\nendfunction\n";
    for (input, expected) in [
        (Value::Null, 0),
        (Value::Str(String::new()), 0),
        (Value::Str("é界".into()), 5),
        (Value::Str("é\0ignored".into()), 2),
        (Value::Str("\0TRIGSTR_12".into()), 0),
        (Value::Str("trigstr_12".into()), 10),
        (Value::Str("TRIGSTR_-1".into()), 10),
        (Value::Str("TRIGSTR_4294967295".into()), 18),
    ] {
        assert_eq!(call(src, "f", &[input]).unwrap(), Value::Int(expected));
    }
}

#[test]
fn string_length_resolves_computed_trigger_references_once_without_expanding_tags() {
    struct Table;
    impl crate::Host for Table {
        fn native(&mut self, name: &str, _: &[Value]) -> Result<Value, String> {
            panic!("implemented native reached host: {name}");
        }
        fn table_string(&self, id: u32) -> Option<String> {
            match id {
                0 => Some("zero".into()),
                12 => Some("é^^unknown^^\0tail".into()),
                23 => Some("TRIGSTR_12".into()),
                41 => Some(String::new()),
                _ => None,
            }
        }
    }
    let src = "native StringLength takes string s returns integer\nfunction f takes string suffix returns integer\nreturn StringLength(\"TRIGSTR_\" + suffix)\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, Table);
    for (suffix, expected) in [
        ("12", 13),
        ("12suffix", 13),
        ("00012", 13),
        ("4294967308", 13),
        ("", 4),
        ("nondigit", 4),
        ("+12", 4),
        (" 12", 4),
        ("23", 10),
        ("41", 0),
    ] {
        assert_eq!(
            vm.call("f", &[Value::Str(suffix.into())]).unwrap(),
            Value::Int(expected)
        );
    }
    let error = vm.call("f", &[Value::Str("91".into())]).unwrap_err();
    assert!(error.message.contains("TRIGSTR_91"));
    assert!(error.message.contains("war3map.wts"));
    assert_eq!(
        vm.call("f", &[Value::Str("12".into())]).unwrap(),
        Value::Int(13)
    );
}

#[test]
fn string_length_preserves_table_source_errors_and_rejects_wrong_declarations() {
    struct FailedTable;
    impl crate::Host for FailedTable {
        fn native(&mut self, name: &str, _: &[Value]) -> Result<Value, String> {
            panic!("implemented native reached host: {name}");
        }
        fn resolve_table_string(&self, _: u32) -> Result<Option<String>, String> {
            Err("synthetic map.wts: invalid record".into())
        }
    }
    let src = "native StringLength takes string s returns integer\nfunction f takes string s returns integer\nreturn StringLength(s)\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, FailedTable);
    let error = vm
        .call("f", &[Value::Str("TRIGSTR_77".into())])
        .unwrap_err();
    assert!(
        error
            .message
            .contains("TRIGSTR_77: synthetic map.wts: invalid record")
    );
    for declaration in [
        "native StringLength takes integer s returns integer\n",
        "native StringLength takes string s returns string\n",
        "native StringLength takes string s, integer extra returns integer\n",
    ] {
        let script = parse(declaration).unwrap();
        let mut table = NativeTable::new();
        for item in &script.items {
            if let crate::Item::Native(native) = item {
                table.insert(native, "synthetic.j");
            }
        }
        assert!(matches!(
            table.call("StringLength"),
            Some(crate::natives::NativeCall::SignatureMismatch(..))
        ));
    }
}

#[test]
fn string_length_literals_and_computed_values_resolve_once_with_identical_results() {
    use std::cell::RefCell;
    #[derive(Default)]
    struct Source {
        calls: RefCell<Vec<u32>>,
    }
    impl crate::Host for Source {
        fn native(&mut self, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("builtin")
        }
        fn native_table_string(&self, id: u32) -> Result<Option<String>, String> {
            self.calls.borrow_mut().push(id);
            Ok(match id {
                12 => Some("TRIGSTR_23".into()),
                23 => Some("é".into()),
                _ => None,
            })
        }
    }
    let source = "native StringLength takes string s returns integer\nfunction literal takes nothing returns integer\nreturn StringLength(\"TRIGSTR_12\")\nendfunction\nfunction argument takes string s returns integer\nreturn StringLength(s)\nendfunction\n";
    let program = program_of(source);
    let mut vm = Vm::new(&program, Source::default());
    assert_eq!(vm.call("literal", &[]).unwrap(), Value::Int(10));
    assert_eq!(*vm.host().calls.borrow(), [12]);
    vm.host().calls.borrow_mut().clear();
    assert_eq!(
        vm.call("argument", &[Value::Str("TRIGSTR_12".into())])
            .unwrap(),
        Value::Int(10)
    );
    assert_eq!(*vm.host().calls.borrow(), [12]);
    assert_eq!(
        vm.call("argument", &[Value::Str("TRIGSTR_91".into())])
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        vm.call("argument", &[Value::Str("^^raw^^".into())])
            .unwrap(),
        Value::Int(7)
    );
}

#[test]
fn raw_literal_tag_payload_decodes_escapes_and_rejects_unknown_ones() {
    let source = r#"function f takes nothing returns string
return "a ^^b\n\t^^ c"
endfunction
"#;
    assert_eq!(
        call(source, "f", &[]).unwrap(),
        Value::Str("a ^^b\n\t^^ c".into())
    );
    let invalid = r#"function f takes nothing returns string
return "a ^^b\q^^ c"
endfunction
"#;
    assert!(
        parse(invalid)
            .unwrap_err()
            .message
            .contains("invalid escape")
    );
}

#[test]
fn real_formatter_builtins_preserve_selected_conversion_and_argument_read_point() {
    use std::cell::RefCell;
    #[derive(Debug, PartialEq)]
    struct Call {
        bits: u32,
        dimensions: Option<(i32, i32)>,
    }
    #[derive(Default)]
    struct Runtime {
        calls: RefCell<Vec<Call>>,
    }
    impl crate::RealOps for Runtime {
        fn literal(&self, _: &str) -> Result<f32, String> {
            Err("no fixture literals".into())
        }
        fn integer(&self, value: i32) -> Result<f32, String> {
            assert_eq!(value, 7);
            Ok(0.5)
        }
        fn binary(&self, _: crate::ast::BinOp, _: f32, _: f32) -> Result<Value, String> {
            Err("no fixture operators".into())
        }
        fn real_to_string(&self, value: f32, args: Option<(i32, i32)>) -> Result<String, String> {
            self.calls.borrow_mut().push(Call {
                bits: value.to_bits(),
                dimensions: args,
            });
            Ok("source result".into())
        }
    }
    impl crate::Host for Runtime {
        fn native(&mut self, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("formatter is builtin")
        }
        fn real_ops(&self) -> Option<&dyn crate::RealOps> {
            Some(self)
        }
    }
    let source = "native R2S takes real value returns string\nnative R2SW takes real value, integer width, integer precision returns string\nfunction Default takes integer value returns string\nreturn R2S(value)\nendfunction\nfunction Width takes real value, integer width, integer precision returns string\nreturn R2SW(value,width,precision)\nendfunction\n";
    let program = program_of(source);
    let mut vm = Vm::new(&program, Runtime::default());
    assert_eq!(
        vm.call("Default", &[Value::Int(7)]).unwrap(),
        Value::Str("source result".into())
    );
    assert_eq!(
        vm.call(
            "Width",
            &[Value::Real(-0.0), Value::Int(-8), Value::Int(19)]
        )
        .unwrap(),
        Value::Str("source result".into())
    );
    assert_eq!(
        *vm.host().calls.borrow(),
        [
            Call {
                bits: 0.5f32.to_bits(),
                dimensions: None
            },
            Call {
                bits: (-0.0f32).to_bits(),
                dimensions: Some((-8, 19))
            }
        ]
    );
}

#[test]
fn real_formatter_refuses_unbound_runtime_and_mismatched_native_signatures() {
    let program = program_of(
        "native R2S takes real value returns string\nfunction f takes real value returns string\nreturn R2S(value)\nendfunction\n",
    );
    let mut vm = Vm::new(&program, crate::NullHost);
    assert!(
        vm.call("f", &[Value::Real(0.5)])
            .unwrap_err()
            .message
            .contains("selected runtime")
    );
    for declaration in [
        "native R2S takes integer value returns string\n",
        "native R2S takes real value returns real\n",
        "native R2SW takes real value, integer width returns string\n",
        "native R2SW takes real value, integer width, real precision returns string\n",
    ] {
        let program = program_of(declaration);
        let name = if declaration.contains("R2SW") {
            "R2SW"
        } else {
            "R2S"
        };
        assert!(matches!(
            program.natives().call(name),
            Some(crate::natives::NativeCall::SignatureMismatch(..))
        ));
    }
}

#[test]
fn string_hash_vm_preserves_raw_bytes_and_never_demands_wts_or_tags() {
    use crate::Host;
    use std::cell::RefCell;
    #[derive(Default)]
    struct Raw(RefCell<Vec<Vec<u8>>>);
    impl Host for Raw {
        fn string_hash(&self, bytes: &[u8]) -> Result<i32, String> {
            self.0.borrow_mut().push(bytes.to_vec());
            Ok(-73)
        }
        fn native_table_string(&self, _: u32) -> Result<Option<String>, String> {
            panic!("StringHash must not resolve WTS")
        }
        fn code_tag(&self, _: &str) -> Option<String> {
            panic!("StringHash must not expand tags")
        }
        fn native(&mut self, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("StringHash must dispatch the builtin")
        }
    }
    let program = program_of(
        "native StringHash takes string s returns integer\nfunction f takes string s returns integer\nreturn StringHash(s)\nendfunction\nfunction computed takes nothing returns integer\nreturn StringHash(\"TRIGSTR_\" + \"7\")\nendfunction\n",
    );
    let mut vm = Vm::new(&program, Raw::default());
    for s in [
        Value::Null,
        Value::Str("".into()),
        Value::Str("\0ignored".into()),
    ] {
        assert_eq!(vm.call("f", &[s]).unwrap(), Value::Int(0));
    }
    for text in ["TRIGSTR_7", "^^tag^^", "é\0tail"] {
        assert_eq!(
            vm.call("f", &[Value::Str(text.into())]).unwrap(),
            Value::Int(-73)
        );
    }
    assert_eq!(vm.call("computed", &[]).unwrap(), Value::Int(-73));
    assert_eq!(
        *vm.host().0.borrow(),
        [
            b"TRIGSTR_7".to_vec(),
            b"^^tag^^".to_vec(),
            "é".as_bytes().to_vec(),
            b"TRIGSTR_7".to_vec()
        ]
    );
    let mut plain = Vm::new(&program, NullHost);
    assert_eq!(plain.call("f", &[Value::Null]).unwrap(), Value::Int(0));
    assert!(
        plain
            .call("computed", &[])
            .unwrap_err()
            .message
            .contains("StringHash source policy is not admitted")
    );
}

#[test]
fn string_hash_rejects_wrong_declared_signatures() {
    for declaration in [
        "native StringHash takes integer s returns integer\n",
        "native StringHash takes string s returns string\n",
        "native StringHash takes string s, integer extra returns integer\n",
    ] {
        let script = parse(declaration).unwrap();
        let mut table = NativeTable::new();
        for item in &script.items {
            if let crate::Item::Native(native) = item {
                table.insert(native, "synthetic.j");
            }
        }
        assert!(matches!(
            table.call("StringHash"),
            Some(crate::natives::NativeCall::SignatureMismatch(..))
        ));
    }
}
