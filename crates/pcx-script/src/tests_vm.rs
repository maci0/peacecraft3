//! Tests for the VM stage: resolution errors, the value model, the native
//! registry, the text seam, and an end-to-end run of a real shipped function.
//!
//! The end-to-end proof drives the shipped `Blizzard.j` off the install and
//! calls functions from it, so the value model is checked against code the game
//! compiles rather than against a hand-written oracle.

use crate::natives::{NativeTable, signature_of};
use crate::resolve::{Prelude, resolve, resolve_with};
use crate::value::{Class, OpError, Value, binary, unary};
use crate::{BinOp, NullHost, Script, UnOp, Vm, parse};
use std::collections::BTreeMap;

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
            .map(Vec::len),
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
fn i2r_and_r2i_are_the_only_natives_the_table_runs() {
    assert_eq!(
        crate::natives::implemented("I2R"),
        Some(crate::natives::NativeImpl::IntToReal)
    );
    assert_eq!(
        crate::natives::implemented("R2I"),
        Some(crate::natives::NativeImpl::RealToInt)
    );
    assert_eq!(crate::natives::implemented("i2r"), None);
    assert_eq!(crate::natives::implemented("I2S"), None);
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

// ----- the text seam -----------------------------------------------------

#[test]
fn a_trigger_string_resolves_through_the_host_table() {
    struct Table(BTreeMap<u32, &'static str>);
    impl crate::Host for Table {
        fn native(&mut self, _name: &str, _args: &[Value]) -> Result<Value, String> {
            Err("no natives in this test".into())
        }
        fn table_string(&self, id: u32) -> Option<String> {
            self.0.get(&id).map(|s| (*s).to_string())
        }
    }
    let table = Table(BTreeMap::from([(374, "First blood!")]));

    // The literal spelling Footman Frenzy ships.
    let src = "function f takes nothing returns string\n    return \"TRIGSTR_374\"\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, Table(table.0.clone()));
    assert_eq!(
        vm.call("f", &[]).expect("call"),
        Value::Str("First blood!".into())
    );

    // The identifier spelling a desugared trigger graph produces.
    let src = "globals\n    string s = TRIGSTR_374\nendglobals\nfunction f takes nothing returns string\n    return s\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, Table(table.0));
    assert_eq!(
        vm.call("f", &[]).expect("call"),
        Value::Str("First blood!".into())
    );
}

#[test]
fn a_trigger_string_no_table_carries_is_a_loud_error() {
    let src = "function f takes nothing returns string\n    return \"TRIGSTR_999\"\nendfunction\n";
    let e = call(src, "f", &[]).expect_err("no table");
    assert!(e.message.contains("TRIGSTR_999"), "{}", e.message);
    assert!(e.message.contains("war3map.wts"), "{}", e.message);
    // An identifier with no table fails the same way, not as an unresolved
    // name: resolution deliberately does not require a declaration for it.
    let e = call(
        "function f takes nothing returns string\n    return TRIGSTR_999\nendfunction\n",
        "f",
        &[],
    )
    .expect_err("no table");
    assert!(e.message.contains("TRIGSTR_999"), "{}", e.message);
}

#[test]
fn a_code_tag_needs_a_substitution_or_it_is_a_named_error() {
    struct Tags;
    impl crate::Host for Tags {
        fn native(&mut self, _name: &str, _args: &[Value]) -> Result<Value, String> {
            Err("no natives".into())
        }
        fn code_tag(&self, tag: &str) -> Option<String> {
            Some(format!("<{tag}>"))
        }
    }
    let src = "function f takes nothing returns string\n    return \"a ^^b^^ c\"\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, Tags);
    assert_eq!(
        vm.call("f", &[]).expect("call"),
        Value::Str("a <b> c".into())
    );

    let src = "function f takes nothing returns string\n    return \"a ^^b^^ c\"\nendfunction\n";
    let program = program_of(src);
    let mut vm = Vm::new(&program, NullHost);
    let e = vm.call("f", &[]).expect_err("no substitution");
    assert!(e.message.contains("`^b^`"), "{}", e.message);
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
