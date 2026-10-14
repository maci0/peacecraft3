use pcx_play::campaign::{CampaignHost, ChapterPlan, Gate, OutcomeCheck, run_chapter};
use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{Host, SourceMap, Value, value::FunctionId};
use std::{cell::RefCell, rc::Rc};

const COMMON: &[u8] = b"native FixtureRegister takes code cb, integer marker, real amount, string label returns nothing\nnative FixtureSeen takes integer value returns nothing\n";
const PRELUDE: &[u8] = b"// fixture declaration padding\n\nfunction Parameterized takes integer supplied returns nothing\ncall FixtureSeen(supplied)\nendfunction\nfunction Zero takes nothing returns nothing\ncall FixtureSeen(41)\nendfunction\n";
const MAP: &[u8] = b"function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall FixtureRegister(function Parameterized, -17, -0.0, \"payload\")\nendfunction\nfunction Ready takes nothing returns boolean\nreturn true\nendfunction\nfunction MapParameterized takes integer supplied returns nothing\ncall FixtureSeen(supplied)\nendfunction\n";

fn scripts(map: &[u8]) -> MapScripts {
    scripts_with_common(COMMON, map)
}
fn scripts_with_common(common: &[u8], map: &[u8]) -> MapScripts {
    MapScripts::compile(
        AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture-v1".into(),
            source: "memory-fixture".into(),
        },
        ScriptAsset {
            label: "fixture/common.j",
            bytes: common,
        },
        ScriptAsset {
            label: "fixture/Blizzard.j",
            bytes: PRELUDE,
        },
        ScriptAsset {
            label: "fixture/war3map.j",
            bytes: map,
        },
        None,
    )
    .unwrap()
}
fn plan() -> ChapterPlan {
    ChapterPlan {
        name: "registration-fixture".into(),
        ticks: 1,
        entrypoints: vec!["config".into(), "main".into()],
        calls: vec![],
        outcomes: [Gate::Dialogue, Gate::Quest, Gate::Victory, Gate::EndGame]
            .into_iter()
            .map(|gate| OutcomeCheck {
                tick: 0,
                gate,
                predicate: "Ready".into(),
            })
            .collect(),
    }
}
#[derive(Default)]
struct Observations {
    registration: Vec<Value>,
    seen: Vec<Value>,
}
struct Capture {
    shared: Rc<RefCell<Observations>>,
    callback: Option<FunctionId>,
    deliver: bool,
    refuse: bool,
    replace_callback: Option<FunctionId>,
}
impl Capture {
    fn new(shared: Rc<RefCell<Observations>>, deliver: bool) -> Self {
        Self {
            shared,
            callback: None,
            deliver,
            refuse: false,
            replace_callback: None,
        }
    }
}
impl Host for Capture {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        match name {
            "FixtureRegister" => {
                self.shared.borrow_mut().registration = args.to_vec();
                if self.refuse {
                    return Err("fixture registration unsupported".into());
                }
                let Some(Value::Code(id)) = args.first() else {
                    return Err("fixture callback missing".into());
                };
                self.callback = Some(*id);
                Ok(Value::Null)
            }
            "FixtureSeen" => {
                self.shared.borrow_mut().seen.extend_from_slice(args);
                Ok(Value::Null)
            }
            _ => Err(format!("fixture native unsupported: {name}")),
        }
    }
}
impl CampaignHost for Capture {
    fn advance_tick(&mut self, _: u64) -> Result<Vec<FunctionId>, String> {
        Ok(if self.deliver {
            self.replace_callback
                .or(self.callback.take())
                .into_iter()
                .collect()
        } else {
            vec![]
        })
    }
}
fn assert_registration(scripts: &MapScripts, observations: &Observations, callback: &str) {
    assert_eq!(observations.registration.len(), 4);
    assert_eq!(
        observations.registration[0],
        Value::Code(scripts.program().function_id(callback).unwrap())
    );
    assert_eq!(observations.registration[1], Value::Int(-17));
    let Value::Real(value) = observations.registration[2] else {
        panic!("lost real argument")
    };
    assert_eq!(value.to_bits(), (-0.0f32).to_bits());
    assert_eq!(observations.registration[3], Value::Str("payload".into()));
}
#[test]
fn registered_parameterized_callback_refusal_names_actual_declaration() {
    let scripts = scripts(MAP);
    let shared = Rc::new(RefCell::new(Observations::default()));
    let error = run_chapter(&scripts, &plan(), Capture::new(shared.clone(), true)).unwrap_err();
    assert_registration(&scripts, &shared.borrow(), "Parameterized");
    assert!(
        shared.borrow().seen.is_empty(),
        "unsupported callback body ran"
    );
    let f = scripts.program().function("Parameterized").unwrap();
    let (line, column) = f
        .span
        .line_col(&SourceMap::new(std::str::from_utf8(PRELUDE).unwrap()));
    assert!(
        error.contains(&format!("tick 0: fixture/Blizzard.j:{line}:{column} ")),
        "{error}"
    );
    assert!(
        error.contains("`Parameterized` takes 1 argument, 0 given"),
        "{error}"
    );
}
#[test]
fn external_arity_refusal_uses_declared_source_span_and_can_recover() {
    let scripts = scripts(MAP);
    let shared = Rc::new(RefCell::new(Observations::default()));
    let mut vm = scripts.vm(Capture::new(shared.clone(), false));
    let f = scripts.program().function("Parameterized").unwrap();
    let error = vm.call("Parameterized", &[]).unwrap_err();
    assert_eq!(vm.error_source(), Some(f.source.as_str()));
    assert_eq!(error.span, f.span);
    assert!(shared.borrow().seen.is_empty());
    assert_eq!(
        vm.call("Parameterized", &[Value::Int(73)]).unwrap(),
        Value::Null
    );
    assert_eq!(shared.borrow().seen, [Value::Int(73)]);
    assert_eq!(vm.error_source(), None);
}
#[test]
fn map_function_external_refusal_uses_map_declaration_position() {
    let scripts = scripts(MAP);
    let mut vm = scripts.vm(Capture::new(Rc::default(), false));
    let f = scripts.program().function("MapParameterized").unwrap();
    let error = vm.call("MapParameterized", &[]).unwrap_err();
    assert_eq!(vm.error_source(), Some("the map script"));
    assert_eq!(error.span, f.span);
    let (line, column) = f
        .span
        .line_col(&SourceMap::new(std::str::from_utf8(MAP).unwrap()));
    assert!(
        scripts
            .diagnostic(&vm, &error)
            .starts_with(&format!("fixture/war3map.j:{line}:{column} "))
    );
}
#[test]
fn zero_argument_registration_preserves_code_arguments_and_explicit_delivery() {
    let map = String::from_utf8(MAP.to_vec())
        .unwrap()
        .replace("function Parameterized,", "function Zero,");
    let scripts = scripts(map.as_bytes());
    let shared = Rc::new(RefCell::new(Observations::default()));
    let trace = run_chapter(&scripts, &plan(), Capture::new(shared.clone(), true)).unwrap();
    assert_registration(&scripts, &shared.borrow(), "Zero");
    assert_eq!(shared.borrow().seen, [Value::Int(41)]);
    assert_eq!(
        trace
            .calls
            .iter()
            .filter(|call| call.phase == "callback")
            .map(|call| call.function.as_str())
            .collect::<Vec<_>>(),
        ["Zero"]
    );
    assert_eq!(trace.natives[0].name, "FixtureRegister");
    assert_eq!(
        trace.natives[0].args[0],
        Value::Code(scripts.program().function_id("Zero").unwrap())
    );
    let no_delivery = Rc::new(RefCell::new(Observations::default()));
    let trace = run_chapter(&scripts, &plan(), Capture::new(no_delivery.clone(), false)).unwrap();
    assert_registration(&scripts, &no_delivery.borrow(), "Zero");
    assert!(no_delivery.borrow().seen.is_empty());
    assert!(trace.calls.iter().all(|call| call.phase != "callback"));
}
#[test]
fn unknown_callback_id_and_unsupported_native_remain_contextual_refusals() {
    let scripts = scripts(MAP);
    let shared = Rc::new(RefCell::new(Observations::default()));
    let mut host = Capture::new(shared.clone(), true);
    host.replace_callback = Some(FunctionId(u32::MAX));
    let error = run_chapter(&scripts, &plan(), host).unwrap_err();
    assert!(error.contains("tick 0: callback function id 4294967295 is not declared"));
    let mut host = Capture::new(shared.clone(), false);
    host.refuse = true;
    let error = run_chapter(&scripts, &plan(), host).unwrap_err();
    assert_registration(&scripts, &shared.borrow(), "Parameterized");
    assert!(error.contains("fixture/war3map.j:4:"), "{error}");
    assert!(
        error.contains("fixture registration unsupported") && error.contains("fixture/common.j:"),
        "{error}"
    );
    assert!(shared.borrow().seen.is_empty());
}

#[test]
fn script_owned_arity_error_retains_caller_span_and_source() {
    let common = String::from_utf8(COMMON.to_vec()).unwrap()
        + "function CallerBad takes nothing returns nothing\ncall Parameterized(1, 2)\nendfunction\n";
    let scripts = scripts_with_common(common.as_bytes(), MAP);
    let mut vm = scripts.vm(Capture::new(Rc::default(), false));
    let caller = scripts.program().function("CallerBad").unwrap();
    let callee = scripts.program().function("Parameterized").unwrap();
    let error = vm.call("CallerBad", &[]).unwrap_err();
    assert_eq!(vm.error_source(), Some(caller.source.as_str()));
    assert!(error.span.start >= caller.span.start && error.span.end <= caller.span.end);
    assert_ne!(error.span, callee.span);
    assert!(error.span.text(&common).contains("Parameterized"));
    assert!(error.message.contains("takes 1 argument, 2 given"));
    assert!(
        scripts
            .diagnostic(&vm, &error)
            .starts_with("fixture/common.j:")
    );
}
