use pcx_play::scripts::{AssetIdentity, MapScripts, OwnedLocalizedHost, ScriptAsset};
use pcx_script::{Error, Host, NullHost, OwnedVm, Program, Value, Vm, value::FunctionId};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Capture {
    inits: usize,
    callbacks: Vec<FunctionId>,
    text: Vec<String>,
    drops: Arc<AtomicUsize>,
    refuse_init: bool,
}
impl Capture {
    fn new(drops: &Arc<AtomicUsize>) -> Self {
        Self {
            inits: 0,
            callbacks: Vec::new(),
            text: Vec::new(),
            drops: Arc::clone(drops),
            refuse_init: false,
        }
    }
}
impl Host for Capture {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        match (name, args) {
            ("Init", []) => {
                self.inits += 1;
                if self.refuse_init {
                    Err("explicit fixture initialization refusal".into())
                } else {
                    Ok(Value::Int(10))
                }
            }
            ("Register", [Value::Code(id)]) => {
                self.callbacks.push(*id);
                Ok(Value::Null)
            }
            ("Capture", [Value::Str(text)]) => {
                self.text.push(text.clone());
                Ok(Value::Null)
            }
            _ => Err(format!("explicit fixture native refusal {name}")),
        }
    }
    fn code_tag(&self, tag: &str) -> Option<String> {
        (tag == "fixture").then(|| "kept".into())
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
const COMMON: &str = "native Init takes nothing returns integer\nnative Register takes code callback returns nothing\nnative Capture takes string text returns nothing\nnative Refuse takes nothing returns nothing\n";
const BJ: &str = "// deliberate diagnostic padding\n\nfunction PreludeFail takes nothing returns nothing\ncall Refuse()\nendfunction\n";
const MAP: &str = "globals\ninteger counter = Init()\nendglobals\nfunction config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall Register(function Callback)\nendfunction\nfunction Callback takes nothing returns nothing\nset counter = counter + 1\ncall Capture(\"TRIGSTR_7\")\nendfunction\nfunction Add takes integer amount returns integer\nset counter = counter + amount\nreturn counter\nendfunction\nfunction Read takes nothing returns integer\nreturn counter\nendfunction\nfunction BadText takes nothing returns string\nreturn \"TRIGSTR_9\"\nendfunction\nfunction MissingText takes nothing returns string\nreturn \"TRIGSTR_8\"\nendfunction\nfunction Tagged takes nothing returns string\nreturn \"^^fixture^^\"\nendfunction\n";
const WTS: &[u8] =
    b"STRING 7\r\n{\r\n|cffff0000Authored|r|nText\r\n}\r\nSTRING 9\r\n{\r\n\xff\r\n}\r\n";
fn compile(map: &str, wts: Option<&[u8]>, source: &str) -> MapScripts {
    // All compilation input owners leave this function before a VM is created.
    let common = COMMON.to_owned();
    let bj = BJ.to_owned();
    let map = map.to_owned();
    let wts = wts.map(<[u8]>::to_vec);
    MapScripts::compile(
        AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture-owned-1".into(),
            source: source.into(),
        },
        ScriptAsset {
            label: "owned/common.j",
            bytes: common.as_bytes(),
        },
        ScriptAsset {
            label: "owned/Blizzard.j",
            bytes: bj.as_bytes(),
        },
        ScriptAsset {
            label: "owned/war3map.j",
            bytes: map.as_bytes(),
        },
        wts.as_deref().map(|bytes| ScriptAsset {
            label: "owned/war3map.wts",
            bytes,
        }),
    )
    .unwrap()
}
fn diagnostic<H: Host>(vm: &mut OwnedVm<OwnedLocalizedHost<H>>, error: &Error) -> String {
    let source = vm.error_source().map(str::to_owned);
    vm.host().diagnostic(source.as_deref(), error)
}
fn borrowed_program_lifetime(program: &Program) -> &Program {
    // The existing borrowed API can return the program beyond this temporary VM.
    Vm::new(program, NullHost).program()
}
fn app_resource_bounds<T: Send + Sync + 'static>(_: &T) {}

pub fn shared_program_survives_owners_and_releases_on_drop() {
    let source = "globals\ninteger counter = 0\nendglobals\nfunction Add takes integer amount returns integer\nset counter = counter + amount\nreturn counter\nendfunction\n".to_owned();
    let program = Arc::new(pcx_script::resolve(&pcx_script::parse(&source).unwrap()).unwrap());
    assert!(std::ptr::eq(borrowed_program_lifetime(&program), &*program));
    let weak = Arc::downgrade(&program);
    let mut vm = OwnedVm::new_owned(Arc::clone(&program), NullHost);
    drop(program);
    drop(source);
    assert_eq!(weak.strong_count(), 1);
    assert_eq!(vm.call("Add", &[Value::Int(3)]).unwrap(), Value::Int(3));
    assert_eq!(vm.call("Add", &[Value::Int(4)]).unwrap(), Value::Int(7));
    assert_eq!(vm.global("counter"), Value::Int(7));
    assert!(vm.program().function("Add").is_some());
    drop(vm);
    assert!(weak.upgrade().is_none());
}

pub fn map_session_retains_globals_callbacks_wts_and_replaces_without_leaks() {
    let drops = Arc::new(AtomicUsize::new(0));
    let scripts = Arc::new(compile(MAP, Some(WTS), "first-map"));
    let weak = Arc::downgrade(&scripts);
    let digest = scripts.digests.clone();
    let program_address = scripts.program() as *const Program;
    assert!(std::ptr::eq(&*scripts.shared_program(), scripts.program()));
    let mut vm = Arc::clone(&scripts).owned_vm(Capture::new(&drops));
    drop(scripts);
    app_resource_bounds(&vm);
    assert_eq!(weak.strong_count(), 1);
    assert_eq!(vm.program() as *const Program, program_address);
    assert_eq!(vm.host().context().identity.source, "first-map");
    assert_eq!(vm.host().context().digests, digest);
    vm.call("config", &[]).unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.host().inner.inits, 1);
    let callback = vm.host().inner.callbacks[0];
    let name = vm.program().functions()[callback.index()].name.clone();
    assert_eq!(name, "Callback");
    for _ in 0..3 {
        vm.call(&name, &[]).unwrap();
    }
    assert_eq!(vm.call("Add", &[Value::Int(7)]).unwrap(), Value::Int(20));
    assert_eq!(vm.call("Read", &[]).unwrap(), Value::Int(20));
    assert_eq!(vm.call("Tagged", &[]).unwrap(), Value::Str("kept".into()));
    assert_eq!(vm.host().inner.inits, 1);
    assert_eq!(vm.host().inner.text, vec!["|cffff0000Authored|r|nText"; 3]);
    let replacement = Arc::new(compile(
        MAP,
        Some(b"STRING 7\r\n{\r\nReplacement text\r\n}\r\n"),
        "second-map",
    ));
    let replacement_weak = Arc::downgrade(&replacement);
    vm = replacement.owned_vm(Capture::new(&drops));
    assert!(weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(vm.host().context().identity.source, "second-map");
    vm.call("Callback", &[]).unwrap();
    assert_eq!(vm.call("Read", &[]).unwrap(), Value::Int(11));
    assert_eq!(vm.host().inner.text, ["Replacement text"]);
    drop(vm);
    assert!(replacement_weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

pub fn retained_diagnostics_distinguish_sources_wts_and_missing() {
    let drops = Arc::new(AtomicUsize::new(0));
    let scripts = Arc::new(compile(MAP, Some(WTS), "diagnostic-map"));
    let mut vm = scripts.owned_vm(Capture::new(&drops));
    let e = vm.call("PreludeFail", &[]).unwrap_err();
    assert_eq!(vm.error_source(), Some("owned/Blizzard.j"));
    let text = diagnostic(&mut vm, &e);
    assert!(text.starts_with("owned/Blizzard.j:4:1"), "{text}");
    assert!(text.contains("explicit fixture native refusal Refuse"));
    let e = vm.call("BadText", &[]).unwrap_err();
    assert_eq!(vm.error_source(), Some("the map script"));
    assert_eq!(
        &MAP[e.span.start as usize..e.span.end as usize],
        "\"TRIGSTR_9\""
    );
    let text = diagnostic(&mut vm, &e);
    assert!(text.starts_with("owned/war3map.j:21:8"), "{text}");
    assert!(text.contains("owned/war3map.wts: WTS entry 9 is not UTF-8 at body byte 0"));
    let e = vm.call("MissingText", &[]).unwrap_err();
    assert!(
        e.message
            .contains("TRIGSTR_8 is not in this map's war3map.wts")
    );
    assert!(!e.message.contains("not UTF-8"));
    let e = vm.call("Add", &[]).unwrap_err();
    assert_eq!(vm.error_source(), Some("the map script"));
    assert!(e.message.contains("1 argument, 0 given"));
    assert!(diagnostic(&mut vm, &e).starts_with("owned/war3map.j:13:1"));
    // A function failure does not poison a successfully initialized session.
    assert_eq!(vm.call("Add", &[Value::Int(2)]).unwrap(), Value::Int(12));
    assert_eq!(vm.error_source(), None);
    let scripts = Arc::new(compile(MAP, None, "no-table-map"));
    let mut vm = scripts.owned_vm(Capture::new(&drops));
    let e = vm.call("Callback", &[]).unwrap_err();
    assert!(
        e.message
            .contains("TRIGSTR_7 is not in this map's war3map.wts")
    );
}

pub fn initialization_refusal_is_sticky_with_retained_context() {
    let drops = Arc::new(AtomicUsize::new(0));
    let scripts = Arc::new(compile(MAP, Some(WTS), "sticky-map"));
    let mut host = Capture::new(&drops);
    host.refuse_init = true;
    let mut vm = scripts.owned_vm(host);
    let first = vm.call("main", &[]).unwrap_err();
    let source = vm.error_source().map(str::to_owned);
    let text = diagnostic(&mut vm, &first);
    assert!(text.starts_with("owned/war3map.j:2:19"), "{text}");
    assert!(text.contains("explicit fixture initialization refusal"));
    for name in ["config", "main", "Callback", "Read"] {
        assert_eq!(vm.call(name, &[]).unwrap_err(), first);
        assert_eq!(vm.error_source(), source.as_deref());
        assert_eq!(diagnostic(&mut vm, &first), text);
    }
    assert_eq!(vm.host().inner.inits, 1);
    assert!(vm.host().inner.callbacks.is_empty());
}

pub fn shared_context_has_independent_vm_state_and_matches_borrowed_execution() {
    let drops = Arc::new(AtomicUsize::new(0));
    let scripts = Arc::new(compile(MAP, Some(WTS), "shared-map"));
    let mut borrowed = scripts.vm(Capture::new(&drops));
    let mut owned = Arc::clone(&scripts).owned_vm(Capture::new(&drops));
    for name in ["config", "main", "Callback", "Callback", "Read"] {
        assert_eq!(owned.call(name, &[]), borrowed.call(name, &[]));
    }
    assert_eq!(owned.global("counter"), borrowed.global("counter"));
    assert_eq!(owned.host().inner.inits, borrowed.host().inner.inits);
    assert_eq!(owned.host().inner.text, borrowed.host().inner.text);
    assert_eq!(
        owned.host().inner.callbacks,
        borrowed.host().inner.callbacks
    );
    let mut second = Arc::clone(&scripts).owned_vm(Capture::new(&drops));
    assert_eq!(second.call("Read", &[]).unwrap(), Value::Int(10));
    assert_eq!(owned.call("Read", &[]).unwrap(), Value::Int(12));
    drop(borrowed);
    drop(scripts);
    let weak = Arc::downgrade(owned.host().context());
    drop(owned);
    assert_eq!(weak.strong_count(), 1);
    assert_eq!(second.call("Read", &[]).unwrap(), Value::Int(10));
    drop(second);
    assert!(weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 3);
}

#[test]
fn owned_program_lifetime() {
    shared_program_survives_owners_and_releases_on_drop();
}
#[test]
fn persistent_session_replacement() {
    map_session_retains_globals_callbacks_wts_and_replaces_without_leaks();
}
#[test]
fn owned_source_and_localization_diagnostics() {
    retained_diagnostics_distinguish_sources_wts_and_missing();
}
#[test]
fn owned_sticky_initialization() {
    initialization_refusal_is_sticky_with_retained_context();
}
#[test]
fn borrowed_equivalence_and_independent_sessions() {
    shared_context_has_independent_vm_state_and_matches_borrowed_execution();
}
