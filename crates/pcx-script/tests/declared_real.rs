//! Integers that cross a declared `real` use the host's `RealOps::integer`.
//! `marked` is not a game conversion. It shifts the cast so a test can tell
//! that call from the unbound `as f32` cast used by `I2R`.

use pcx_script::{BinOp, Exec, OwnedVm, RealOps, Value, parse, resolve};
use std::sync::Arc;

const WIDE: i32 = 33_554_435;

const SRC: &str = "\
native Seen takes real x returns nothing
native Give takes nothing returns real
native Hold takes nothing returns integer
native HoldReal takes nothing returns real
native R2I takes real r returns integer
native I2R takes integer i returns real
globals
    real stored = 3
    real array cells
    integer plain = 3
    integer array ints
    integer bucket = 0
endglobals
function direct takes nothing returns real
    return 3
endfunction
function wide takes nothing returns real
    return 33554435
endfunction
function write_local takes nothing returns integer
    local real value = 3
    set bucket = value
    return 0
endfunction
function write_cells takes nothing returns integer
    local real array shadow
    set shadow[2] = 3
    set cells[4] = 3
    set ints[1] = 4
    set bucket = shadow[2]
    return 0
endfunction
function shadow takes nothing returns integer
    local real plain = 3
    set bucket = plain
    return 0
endfunction
function touch_plain takes nothing returns integer
    set plain = 4
    return plain
endfunction
function take takes real x returns integer
    set bucket = x
    return 0
endfunction
function id takes integer x returns integer
    return x
endfunction
function see takes nothing returns integer
    call Seen(3)
    return 0
endfunction
function see_real takes nothing returns integer
    call Seen(1.5)
    return 0
endfunction
function see_bool takes nothing returns integer
    call Seen(true)
    return 0
endfunction
function give takes nothing returns real
    return Give()
endfunction
function give_bucket takes nothing returns integer
    set bucket = Give()
    return 0
endfunction
function conv takes nothing returns real
    return I2R(3)
endfunction
function trunc takes nothing returns integer
    return R2I(3)
endfunction
function bad_i2r takes nothing returns real
    return I2R(1.5)
endfunction
function keep_real takes nothing returns integer
    local integer value = 1.5
    return value
endfunction
function refuse takes nothing returns real
    return -8
endfunction
function save_hold takes nothing returns integer
    set stored = Hold()
    set bucket = stored
    return 0
endfunction
function save_index takes nothing returns integer
    set cells[Hold()] = 4
    set bucket = cells[4]
    return 0
endfunction
function bind_hold takes nothing returns integer
    local real value = Hold()
    set bucket = value
    return 0
endfunction
function pass_hold takes nothing returns integer
    return take(Hold())
endfunction
function see_hold takes nothing returns integer
    call Seen(Hold())
    return 0
endfunction
function parked_return takes nothing returns real
    return Hold()
endfunction
function parked_give takes nothing returns integer
    set bucket = HoldReal()
    return 0
endfunction
";

fn marked(value: i32) -> f32 {
    (value as f32) + 100.0
}

struct Recorder {
    policy: bool,
    events: Vec<String>,
    args: Vec<Vec<Value>>,
}

impl Recorder {
    fn new(policy: bool) -> Self {
        Self {
            policy,
            events: Vec::new(),
            args: Vec::new(),
        }
    }
}

impl RealOps for Recorder {
    fn literal(&self, spelling: &str) -> Result<f32, String> {
        match spelling {
            "1.5" => Ok(1.5),
            other => Err(format!("uncovered literal {other}")),
        }
    }

    fn integer(&self, value: i32) -> Result<f32, String> {
        if value < 0 {
            return Err(format!("declared real refused {value}"));
        }
        Ok(marked(value))
    }

    fn binary(&self, op: BinOp, left: f32, right: f32) -> Result<Value, String> {
        let _ = (op, left, right);
        Err("binary is not used here".into())
    }
}

impl pcx_script::Host for Recorder {
    fn real_ops(&self) -> Option<&dyn RealOps> {
        self.policy.then_some(self as &dyn RealOps)
    }

    fn suspend(&mut self, name: &str, _args: &[Value]) -> bool {
        if name == "Hold" || name == "HoldReal" {
            self.events.push(format!("suspend {name}"));
            true
        } else {
            false
        }
    }

    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        self.events.push(format!("native {name}"));
        self.args.push(args.to_vec());
        match name {
            "Seen" => Ok(Value::Null),
            "Give" => Ok(Value::Int(3)),
            "Hold" | "HoldReal" => Err("parked native must not run".into()),
            other => Err(format!("unexpected native {other}")),
        }
    }
}

fn open(policy: bool) -> OwnedVm<Recorder> {
    let program = Arc::new(resolve(&parse(SRC).unwrap()).unwrap());
    OwnedVm::new_owned(program, Recorder::new(policy))
}

fn check(
    policy: bool,
    name: &str,
    args: &[Value],
    look: impl FnOnce(&mut OwnedVm<Recorder>, Value),
) {
    let mut vm = open(policy);
    let value = vm.call(name, args).unwrap();
    look(&mut vm, value);
}

fn check_resume(
    policy: bool,
    name: &str,
    incoming: Value,
    look: impl FnOnce(&mut OwnedVm<Recorder>, Exec),
) {
    let mut vm = open(policy);
    assert_eq!(vm.invoke(name, &[]).unwrap(), Exec::Suspended);
    let exec = vm.resume(incoming).unwrap();
    look(&mut vm, exec);
}

#[test]
fn declared_reals_use_real_ops_integer_at_each_boundary() {
    let real3 = Value::Real(marked(3));

    check(true, "direct", &[], |vm, value| {
        assert_eq!(value, real3);
        assert_eq!(vm.global("stored"), real3);
        assert_eq!(vm.global("plain"), Value::Int(3));
        assert_eq!(vm.global("bucket"), Value::Int(0));
    });
    check(true, "wide", &[], |_, value| {
        assert_eq!(value, Value::Real(marked(WIDE)));
        assert_ne!(value, Value::Real(WIDE as f32));
    });
    check(true, "write_local", &[], |vm, value| {
        assert_eq!(value, Value::Int(0));
        assert_eq!(vm.global("bucket"), real3);
    });
    check(true, "write_cells", &[], |vm, value| {
        assert_eq!(value, Value::Int(0));
        assert_eq!(vm.global("bucket"), real3);
        assert_eq!(vm.global_at("cells", 4), real3);
        assert_eq!(vm.global_at("cells", 2), Value::Null);
        assert_eq!(vm.global_at("ints", 1), Value::Int(4));
    });
    check(true, "shadow", &[], |vm, _| {
        assert_eq!(vm.global("bucket"), real3);
        assert_eq!(vm.global("plain"), Value::Int(3));
    });
    check(true, "touch_plain", &[], |vm, value| {
        assert_eq!(value, Value::Int(4));
        assert_eq!(vm.global("plain"), Value::Int(4));
    });
    check(true, "take", &[Value::Int(3)], |vm, value| {
        assert_eq!(value, Value::Int(0));
        assert_eq!(vm.global("bucket"), real3);
    });
    check(true, "take", &[Value::Real(1.5)], |vm, _| {
        assert_eq!(vm.global("bucket"), Value::Real(1.5));
    });
    check(true, "take", &[Value::Null], |vm, _| {
        assert_eq!(vm.global("bucket"), Value::Null);
    });
    check(true, "take", &[Value::Bool(true)], |vm, _| {
        assert_eq!(vm.global("bucket"), Value::Bool(true));
    });
    check(true, "id", &[Value::Real(1.5)], |_, value| {
        assert_eq!(value, Value::Real(1.5));
    });
    check(true, "see", &[], |vm, value| {
        assert_eq!(value, Value::Int(0));
        assert_eq!(vm.host().events, vec!["native Seen".to_string()]);
        assert_eq!(vm.host().args, vec![vec![real3.clone()]]);
    });
    check(true, "see_real", &[], |vm, _| {
        assert_eq!(vm.host().args, vec![vec![Value::Real(1.5)]]);
    });
    check(true, "see_bool", &[], |vm, _| {
        assert_eq!(vm.host().args, vec![vec![Value::Bool(true)]]);
    });
    check(true, "give", &[], |vm, value| {
        assert_eq!(value, real3);
        assert_eq!(vm.host().events, vec!["native Give".to_string()]);
        assert!(vm.host().args[0].is_empty());
    });
    check(true, "give_bucket", &[], |vm, value| {
        assert_eq!(value, Value::Int(0));
        assert_eq!(vm.global("bucket"), real3);
    });
    check(true, "conv", &[], |_, value| assert_eq!(value, real3));
    check(true, "trunc", &[], |_, value| {
        assert_eq!(value, Value::Int(marked(3).trunc() as i32));
    });
    check(true, "keep_real", &[], |_, value| {
        assert_eq!(value, Value::Real(1.5));
    });
}

#[test]
fn resumed_declared_reals_convert_the_incoming_integer() {
    let real3 = Value::Real(marked(3));
    let real4 = Value::Real(marked(4));

    check_resume(true, "save_hold", Value::Int(4), |vm, exec| {
        assert_eq!(exec, Exec::Finished(Value::Int(0)));
        assert_eq!(vm.host().events, vec!["suspend Hold".to_string()]);
        assert_eq!(vm.global("stored"), real4);
        assert_eq!(vm.global("bucket"), real4);
    });
    check_resume(true, "save_index", Value::Int(4), |vm, exec| {
        assert_eq!(exec, Exec::Finished(Value::Int(0)));
        assert_eq!(vm.global_at("cells", 4), real4);
        assert_eq!(vm.global("bucket"), real4);
        assert_eq!(vm.global_at("cells", 0), Value::Null);
    });
    check_resume(true, "bind_hold", Value::Int(3), |vm, exec| {
        assert_eq!(exec, Exec::Finished(Value::Int(0)));
        assert_eq!(vm.global("bucket"), real3);
    });
    check_resume(true, "pass_hold", Value::Int(3), |vm, exec| {
        assert_eq!(exec, Exec::Finished(Value::Int(0)));
        assert_eq!(vm.global("bucket"), real3);
    });
    check_resume(true, "see_hold", Value::Int(3), |vm, exec| {
        assert_eq!(exec, Exec::Finished(Value::Int(0)));
        assert_eq!(
            vm.host().events,
            vec!["suspend Hold".to_string(), "native Seen".to_string()]
        );
        assert_eq!(vm.host().args, vec![vec![real3.clone()]]);
    });
    check_resume(true, "parked_return", Value::Int(3), |_, exec| {
        assert_eq!(exec, Exec::Finished(real3.clone()));
    });
    check_resume(true, "parked_give", Value::Int(3), |vm, exec| {
        assert_eq!(exec, Exec::Finished(Value::Int(0)));
        assert_eq!(vm.host().events, vec!["suspend HoldReal".to_string()]);
        assert_eq!(vm.global("bucket"), real3);
    });
    check_resume(true, "parked_give", Value::Real(1.5), |vm, _| {
        assert_eq!(vm.global("bucket"), Value::Real(1.5));
    });
}

#[test]
fn unbound_real_declarations_use_the_same_cast_as_unbound_i2r() {
    let cast3 = Value::Real(3.0);
    check(false, "direct", &[], |vm, value| {
        assert_eq!(value, cast3);
        assert_eq!(vm.global("stored"), cast3);
        assert_eq!(value, Value::Real(3_i32 as f32));
    });
    check(false, "wide", &[], |_, value| {
        assert_eq!(value, Value::Real(WIDE as f32));
        assert_ne!(value, Value::Real(marked(WIDE)));
    });
    check(false, "see", &[], |vm, _| {
        assert_eq!(vm.host().args, vec![vec![cast3.clone()]]);
    });
    check(false, "give", &[], |_, value| assert_eq!(value, cast3));
    check(false, "give_bucket", &[], |vm, _| {
        assert_eq!(vm.global("bucket"), cast3);
    });
    check(false, "conv", &[], |_, value| assert_eq!(value, cast3));
    check(false, "trunc", &[], |_, value| {
        assert_eq!(value, Value::Int(3));
    });
    check(false, "take", &[Value::Int(3)], |vm, _| {
        assert_eq!(vm.global("bucket"), cast3);
    });
    check(false, "touch_plain", &[], |vm, value| {
        assert_eq!(value, Value::Int(4));
        assert_eq!(vm.global("plain"), Value::Int(4));
    });
    check(false, "keep_real", &[], |_, value| {
        assert_eq!(value, Value::Real(1.5));
    });
    check_resume(false, "save_hold", Value::Int(4), |vm, exec| {
        assert_eq!(exec, Exec::Finished(Value::Int(0)));
        assert_eq!(vm.global("bucket"), Value::Real(4.0));
    });
    check_resume(false, "parked_return", Value::Int(3), |_, exec| {
        assert_eq!(exec, Exec::Finished(cast3));
    });
}

#[test]
fn real_ops_refusal_is_the_run_error_and_global_init_caches_it() {
    let mut vm = open(true);
    let error = vm.call("refuse", &[]).unwrap_err();
    assert_eq!(error.stage, pcx_script::Stage::Run);
    assert!(
        error.message.contains("declared real refused -8"),
        "{}",
        error.message
    );
    assert!(
        SRC[error.span.start as usize..error.span.end as usize].contains("-8"),
        "{}",
        error.message
    );
    let bad_i2r = vm.call("bad_i2r", &[]).unwrap_err();
    assert!(bad_i2r.message.contains("integer"), "{}", bad_i2r.message);

    let mut external = open(true);
    let error = external.call("take", &[Value::Int(-8)]).unwrap_err();
    assert!(
        error.message.contains("declared real refused -8"),
        "{}",
        error.message
    );
    assert_eq!(external.error_source(), Some("the map script"));
    assert_eq!(external.global("bucket"), Value::Int(0));

    let bad = "globals\nreal bad = -8\nendglobals\nfunction main takes nothing returns nothing\nendfunction\n";
    let program = Arc::new(resolve(&parse(bad).unwrap()).unwrap());
    let mut vm = OwnedVm::new_owned(program, Recorder::new(true));
    let first = vm.call("main", &[]).unwrap_err();
    let second = vm.call("main", &[]).unwrap_err();
    assert_eq!(first, second);
    assert!(
        first.message.contains("declared real refused -8"),
        "{}",
        first.message
    );
    assert_eq!(
        &bad[first.span.start as usize..first.span.end as usize],
        "-8"
    );
}

#[test]
fn resumed_refusal_reports_the_same_conversion_error() {
    let mut vm = open(true);
    assert_eq!(vm.invoke("parked_return", &[]).unwrap(), Exec::Suspended);
    let error = vm.resume(Value::Int(-8)).unwrap_err();
    assert!(
        error.message.contains("declared real refused -8"),
        "{}",
        error.message
    );
    assert!(vm.call("direct", &[]).is_ok());
}
