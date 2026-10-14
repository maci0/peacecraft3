use pcx_script::{NullHost, Prelude, SourceMap, Stage, Value, Vm, parse, resolve, resolve_with};

#[test]
fn bare_local_array_cannot_fall_through_to_global_scalar() {
    let source = "globals\ninteger shared = 47\nendglobals\nfunction Broken takes nothing returns integer\nlocal integer array shared\nreturn shared\nendfunction\n";
    let script = parse(source).unwrap();
    for prelude in [false, true] {
        let empty = parse("").unwrap();
        let program = if prelude {
            resolve_with(&empty, &[Prelude::new("synthetic/array-body.j", &script)]).unwrap()
        } else {
            resolve(&script).unwrap()
        };
        let mut vm = Vm::new(&program, NullHost);
        let error = vm
            .call("Broken", &[])
            .expect_err("local array must require its own index, not yield the global's 47");
        assert_eq!(error.stage, Stage::Run);
        assert!(
            error
                .message
                .contains("`shared` is an array and needs an index"),
            "{}",
            error.message
        );
        assert_eq!(error.locate(&SourceMap::new(source)), (6, 8));
        assert_eq!(
            &source[error.span.start as usize..error.span.end as usize],
            "shared"
        );
        assert_eq!(
            vm.error_source(),
            Some(if prelude {
                "synthetic/array-body.j"
            } else {
                "the map script"
            })
        );
        assert_eq!(vm.global("shared"), Value::Int(47));
        assert_eq!(vm.call("Broken", &[]).unwrap_err(), error);
    }
}

#[test]
fn indexed_local_array_and_scalar_shadowing_still_use_the_frame() {
    let source = "globals\ninteger shared = 47\ninteger array entries\nendglobals\nfunction LocalArray takes nothing returns integer\nlocal integer array shared\nset shared[-1] = 13\nreturn shared[-1]\nendfunction\nfunction LocalScalar takes nothing returns integer\nlocal integer entries = 29\nreturn entries\nendfunction\n";
    let program = resolve(&parse(source).unwrap()).unwrap();
    let mut vm = Vm::new(&program, NullHost);
    assert_eq!(vm.call("LocalArray", &[]).unwrap(), Value::Int(13));
    assert_eq!(vm.call("LocalScalar", &[]).unwrap(), Value::Int(29));
    assert_eq!(vm.global("shared"), Value::Int(47));
    assert_eq!(vm.global_at("entries", -1), Value::Null);
    assert_eq!(vm.error_source(), None);
}

#[test]
fn sparse_signed_indices_and_finite_casts_keep_existing_values() {
    let source = "native R2I takes real r returns integer\nnative I2R takes integer i returns real\nglobals\ninteger array values\nendglobals\nfunction Fill takes nothing returns integer\nset values[$80000000] = 41\nset values[$7FFFFFFF] = 43\nset values[-1] = 47\nset values[R2I(3.75)] = 53\nset values[R2I(-3.75)] = 59\nset values[$7FFFFFFF + 1] = 61\nreturn values[$80000000] + values[$7FFFFFFF] + values[-1] + values[3] + values[-3]\nendfunction\n";
    let program = resolve(&parse(source).unwrap()).unwrap();
    let mut vm = Vm::new(&program, NullHost);
    assert_eq!(vm.call("Fill", &[]).unwrap(), Value::Int(263));
    for (index, value) in [(i32::MIN, 61), (i32::MAX, 43), (-1, 47), (3, 53), (-3, 59)] {
        assert_eq!(vm.global_at("values", index), Value::Int(value));
    }
    assert_eq!(vm.global_at("values", 0), Value::Null);
}

#[test]
fn noninteger_indices_keep_authored_error_spans_and_do_not_store() {
    let source = "native I2R takes integer i returns real\nglobals\ninteger array values\nendglobals\nfunction BadRead takes nothing returns integer\nreturn values[I2R(1)]\nendfunction\nfunction BadWrite takes nothing returns nothing\nset values[false] = 7\nendfunction\n";
    let program = resolve(&parse(source).unwrap()).unwrap();
    let mut vm = Vm::new(&program, NullHost);
    for (name, index_text, kind) in [
        ("BadRead", "I2R(1)", "real"),
        ("BadWrite", "false", "boolean"),
    ] {
        let error = vm.call(name, &[]).unwrap_err();
        assert_eq!(error.stage, Stage::Run);
        assert!(
            error
                .message
                .contains(&format!("an array index needs an integer, got {kind}")),
            "{}",
            error.message
        );
        assert_eq!(
            &source[error.span.start as usize..error.span.end as usize],
            index_text
        );
        assert_eq!(vm.error_source(), Some("the map script"));
        assert_eq!(vm.global_at("values", 0), Value::Null);
        assert_eq!(vm.global_at("values", 1), Value::Null);
    }
}
