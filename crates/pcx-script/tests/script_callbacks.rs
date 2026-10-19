use pcx_script::{Exec, Host, ScriptCall, Value, Vm};
use std::sync::Arc;

#[derive(Default)]
struct Callbacks {
    depth: usize,
    completed: usize,
    sides: usize,
    aborted: usize,
}

impl Host for Callbacks {
    fn native(&mut self, name: &str, _: &[Value]) -> Result<Value, String> {
        if name == "Side" {
            self.sides += 1;
            Ok(Value::Int(3))
        } else {
            Err(format!("unsupported {name}"))
        }
    }
    fn suspend(&mut self, name: &str, _: &[Value]) -> bool {
        name == "Pause"
    }
    fn begin_script_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, String> {
        if name != "Apply" {
            return Ok(None);
        }
        let [Value::Code(function)] = args else {
            return Err("callback required".into());
        };
        self.depth += 1;
        Ok(Some(ScriptCall::Call(*function)))
    }
    fn continue_script_call(&mut self, value: Value) -> Result<ScriptCall, String> {
        self.depth -= 1;
        self.completed += 1;
        Ok(ScriptCall::Return(value))
    }
    fn abort_script_calls(&mut self) {
        self.depth = 0;
        self.aborted += 1;
    }
}

const SCRIPT: &str = "\
native Apply takes code callback returns integer
native Pause takes nothing returns integer
native Side takes nothing returns integer
globals
integer seen = 0
endglobals
function inner takes nothing returns integer
set seen = seen + 1
return Pause() + Side()
endfunction
function outer takes nothing returns integer
return Apply(function inner) + 10
endfunction
function main takes nothing returns integer
return Side() + Apply(function outer) * 2 + seen
endfunction
";

#[test]
fn nested_native_callbacks_share_globals_and_resume_inside_the_original_expression() {
    let script = pcx_script::parse_script(SCRIPT).unwrap();
    let program = Arc::new(pcx_script::resolve(&script).unwrap());
    let mut vm = Vm::new_owned(program, Callbacks::default());
    assert_eq!(vm.invoke("main", &[]).unwrap(), Exec::Suspended);
    assert_eq!(vm.global("seen"), Value::Int(1));
    assert_eq!(vm.host().depth, 2);
    assert_eq!(vm.host().sides, 1);
    assert!(vm.invoke("main", &[]).is_err());
    assert_eq!(
        vm.host().depth,
        2,
        "a refused second start preserves the parked call"
    );
    assert_eq!(
        vm.resume(Value::Int(5)).unwrap(),
        Exec::Finished(Value::Int(40))
    );
    assert_eq!(vm.host().depth, 0);
    assert_eq!(vm.host().completed, 2);
    assert_eq!(vm.host().sides, 2);
    assert_eq!(vm.host().aborted, 0);
}

#[test]
fn callback_failure_unwinds_host_context_and_keeps_completed_script_effects() {
    let source = SCRIPT.replace("return Pause() + Side()", "return 1 / 0");
    let script = pcx_script::parse_script(&source).unwrap();
    let program = Arc::new(pcx_script::resolve(&script).unwrap());
    let mut vm = Vm::new_owned(program, Callbacks::default());
    assert!(vm.invoke("main", &[]).is_err());
    assert_eq!(vm.global("seen"), Value::Int(1));
    assert_eq!(vm.host().depth, 0);
    assert_eq!(vm.host().completed, 0);
    assert_eq!(vm.host().aborted, 1);
    assert!(vm.resume(Value::Int(5)).is_err());
    assert_eq!(vm.global("seen"), Value::Int(1));
}
