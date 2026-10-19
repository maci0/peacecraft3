//! Native real parameters share the admitted I2R conversion, including parked calls.
use pcx_script::{BinOp, Exec, Host, OwnedVm, RealOps, ScriptCall, Value, parse, resolve};
use std::{cell::Cell, sync::Arc};
const SOURCE: &str = "
constant native I2R takes integer n returns real
constant native R2I takes real n returns integer
native Capture takes real r, integer i returns integer
native Hold takes real r returns integer
native Callback takes real r returns integer
function capture takes nothing returns integer
return Capture(3,4)
endfunction
function real_value takes nothing returns integer
return Capture(1.5,4)
endfunction
function converted takes nothing returns integer
return Capture(I2R(3),4)
endfunction
function wide takes nothing returns integer
return Capture(33554435,4)
endfunction
function rejected takes nothing returns integer
return Capture(-3,4)
endfunction
function parked takes nothing returns integer
return Hold(3)
endfunction
function callback takes nothing returns integer
return Callback(3)
endfunction
function trunc takes nothing returns integer
return R2I(3)
endfunction
";
#[derive(Default)]
struct Recorder {
    policy: bool,
    conversions: Cell<usize>,
    arguments: Vec<Vec<Value>>,
    suspended: Vec<Vec<Value>>,
    callbacks: Vec<Vec<Value>>,
}
impl RealOps for Recorder {
    fn integer(&self, n: i32) -> Result<f32, String> {
        self.conversions.set(self.conversions.get() + 1);
        if n < 0 {
            Err("integer conversion refused".into())
        } else {
            Ok(n as f32 + 100.)
        }
    }
    fn literal(&self, s: &str) -> Result<f32, String> {
        s.parse::<f32>().map_err(|e| e.to_string())
    }
    fn binary(&self, _: BinOp, _: f32, _: f32) -> Result<Value, String> {
        Err("unused".into())
    }
}
impl Host for Recorder {
    fn real_ops(&self) -> Option<&dyn RealOps> {
        self.policy.then_some(self as &dyn RealOps)
    }
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        assert_eq!(name, "Capture");
        self.arguments.push(args.to_vec());
        Ok(Value::Int(7))
    }
    fn suspend(&mut self, name: &str, args: &[Value]) -> bool {
        if name == "Hold" {
            self.suspended.push(args.to_vec());
            true
        } else {
            false
        }
    }
    fn begin_script_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, String> {
        if name == "Callback" {
            self.callbacks.push(args.to_vec());
            Ok(Some(ScriptCall::Return(Value::Int(9))))
        } else {
            Ok(None)
        }
    }
}
fn vm(policy: bool) -> OwnedVm<Recorder> {
    OwnedVm::new_owned(
        Arc::new(resolve(&parse(SOURCE).unwrap()).unwrap()),
        Recorder {
            policy,
            ..Default::default()
        },
    )
}
#[test]
fn selected_conversion_applies_once_and_preserves_integer_and_real_arguments() {
    let mut vm = vm(true);
    assert_eq!(vm.call("capture", &[]).unwrap(), Value::Int(7));
    assert_eq!(
        vm.host().arguments,
        [vec![Value::Real(103.), Value::Int(4)]]
    );
    assert_eq!(vm.host().conversions.get(), 1);
    vm.call("real_value", &[]).unwrap();
    assert_eq!(vm.host().arguments[1], [Value::Real(1.5), Value::Int(4)]);
    assert_eq!(vm.host().conversions.get(), 1);
    vm.call("converted", &[]).unwrap();
    assert_eq!(vm.host().arguments[2], [Value::Real(103.), Value::Int(4)]);
    assert_eq!(vm.host().conversions.get(), 2);
    assert_eq!(vm.call("trunc", &[]).unwrap(), Value::Int(103));
}
#[test]
fn unbound_native_arguments_use_the_existing_i2r_cast() {
    let mut vm = vm(false);
    vm.call("wide", &[]).unwrap();
    assert_eq!(
        vm.host().arguments,
        [vec![Value::Real(33_554_435i32 as f32), Value::Int(4)]]
    );
    assert_eq!(vm.host().conversions.get(), 0);
}
#[test]
fn failed_conversion_never_reaches_the_host() {
    let mut vm = vm(true);
    assert!(
        vm.call("rejected", &[])
            .unwrap_err()
            .to_string()
            .contains("integer conversion refused")
    );
    assert!(vm.host().arguments.is_empty());
    assert!(vm.host().suspended.is_empty());
    assert!(vm.host().callbacks.is_empty());
}
#[test]
fn suspension_and_callback_dispatch_receive_the_same_converted_arguments() {
    let mut vm = vm(true);
    assert_eq!(vm.invoke("parked", &[]).unwrap(), Exec::Suspended);
    assert_eq!(vm.host().suspended, [vec![Value::Real(103.)]]);
    assert_eq!(
        vm.resume(Value::Int(17)).unwrap(),
        Exec::Finished(Value::Int(17))
    );
    assert_eq!(vm.host().conversions.get(), 1);
    assert_eq!(vm.call("callback", &[]).unwrap(), Value::Int(9));
    assert_eq!(vm.host().callbacks, [vec![Value::Real(103.)]]);
    assert_eq!(vm.host().conversions.get(), 2);
    assert!(vm.host().arguments.is_empty());
}
