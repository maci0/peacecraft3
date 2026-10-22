use super::*;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

const PROGRESS: &str = "native SetMissionAvailable takes integer c, integer m, boolean a returns nothing\nnative SetNextLevel takes string map returns nothing\n";
#[derive(Default)]
struct Memory {
    bytes: Mutex<Option<Vec<u8>>>,
    fail: AtomicBool,
}
impl Storage for Memory {
    fn load(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
        let bytes = self.bytes.lock().unwrap();
        if bytes.as_ref().is_some_and(|b| b.len() > limit) {
            return Err("read limit".into());
        }
        Ok(bytes.clone())
    }
    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        if self.fail.load(Ordering::Relaxed) {
            return Err("injected failure".into());
        }
        *self.bytes.lock().unwrap() = Some(bytes.to_vec());
        Ok(())
    }
}

#[test]
fn mission_assignments_are_durable_and_failed_writes_leave_state_unchanged() {
    let memory = Arc::new(Memory::default());
    let p = program(PROGRESS, "", "");
    let mut h = host(p.clone(), 4);
    assert!(
        h.native(MISSION, &[Value::Int(3), Value::Int(7), Value::Bool(true)])
            .is_err()
    );
    h.admit_campaign_progress_storage(memory.clone()).unwrap();
    assert_eq!(h.campaign_progress().unwrap().mission_available(3, 7), None);
    h.native(MISSION, &[Value::Int(3), Value::Int(7), Value::Bool(false)])
        .unwrap();
    let saved = memory.load(1000).unwrap();
    memory.fail.store(true, Ordering::Relaxed);
    assert!(
        h.native(MISSION, &[Value::Int(3), Value::Int(7), Value::Bool(true)])
            .is_err()
    );
    assert_eq!(
        h.campaign_progress().unwrap().mission_available(3, 7),
        Some(false)
    );
    assert_eq!(memory.load(1000).unwrap(), saved);
    memory.fail.store(false, Ordering::Relaxed);
    let mut restored = host(p, 4);
    restored.admit_campaign_progress_storage(memory).unwrap();
    assert_eq!(
        restored
            .campaign_progress()
            .unwrap()
            .mission_available(3, 7),
        Some(false)
    );
    assert_eq!(
        restored
            .campaign_progress()
            .unwrap()
            .mission_available(3, 8),
        None
    );
    for args in [
        vec![],
        vec![Value::Int(-1), Value::Int(2), Value::Bool(true)],
        vec![Value::Int(3), Value::Bool(true), Value::Bool(true)],
    ] {
        assert!(restored.native(MISSION, &args).is_err());
        assert_eq!(
            restored
                .campaign_progress()
                .unwrap()
                .mission_available(3, 7),
            Some(false)
        );
    }
}

#[test]
fn selected_native_contracts_are_required_and_next_map_is_session_local() {
    for declaration in [
        "native SetMissionAvailable takes integer c, real m, boolean a returns nothing\n",
        "constant native SetNextLevel takes string map returns nothing\n",
    ] {
        assert!(
            bare(program(declaration, "", ""))
                .with_game_exit(4)
                .is_err()
        );
    }
    assert!(bare(program("", PROGRESS, "")).with_game_exit(4).is_err());
    let p = program(
        PROGRESS,
        "",
        r#"
globals
integer continued=0
endglobals
function run takes nothing returns nothing
call SetNextLevel("Maps\\Campaign\\Synthetic02.w3m")
set continued=9
endfunction
"#,
    );
    let mut vm = Vm::new(&p, host(p.clone(), 4));
    vm.init_globals().unwrap();
    vm.call("run", &[]).unwrap();
    assert_eq!(vm.global("continued"), Value::Int(9));
    assert_eq!(
        vm.host().next_level(),
        Some("Maps\\Campaign\\Synthetic02.w3m")
    );
    assert!(host(p.clone(), 4).next_level().is_none());
    assert!(
        vm.host()
            .native(NEXT_LEVEL, &[Value::Str("bad\0map".into())])
            .is_err()
    );
    assert_eq!(
        vm.host().next_level(),
        Some("Maps\\Campaign\\Synthetic02.w3m")
    );
    vm.host()
        .native(NEXT_LEVEL, &[Value::Str(String::new())])
        .unwrap();
    assert_eq!(vm.host().next_level(), None);
}
