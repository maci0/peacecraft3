//! Headless script chapter execution with explicit, observed outcome gates.
//! Timer delivery is a host contract; this module does not invent game timing.
use crate::scripts::{AssetIdentity, MapScripts, SourceDigest};
use pcx_script::{Host, Value, value::FunctionId};
use std::collections::BTreeMap;

/// A game host advances its state and supplies due callbacks in its evidenced order.
/// There is no default implementation that silently drops native timers.
pub trait CampaignHost: Host {
    fn advance_tick(&mut self, tick: u64) -> Result<Vec<FunctionId>, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Gate {
    Dialogue,
    Quest,
    Victory,
    EndGame,
}
#[derive(Debug, Clone)]
pub struct OutcomeCheck {
    pub tick: u64,
    pub gate: Gate,
    /// Declared, zero-argument boolean function querying real script/host state.
    pub predicate: String,
}
#[derive(Debug, Clone)]
pub struct ScriptCall {
    pub tick: u64,
    pub function: String,
    pub args: Vec<Value>,
}
#[derive(Debug, Clone)]
pub struct ChapterPlan {
    pub name: String,
    pub ticks: u64,
    /// Explicit load entrypoints, usually config/main, in declaration order.
    pub entrypoints: Vec<String>,
    pub calls: Vec<ScriptCall>,
    pub outcomes: Vec<OutcomeCheck>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CallRecord {
    pub sequence: u64,
    pub tick: Option<u64>,
    pub phase: String,
    pub function: String,
    pub result: Value,
}
#[derive(Debug, Clone, PartialEq)]
pub struct NativeRecord {
    pub sequence: u64,
    pub tick: Option<u64>,
    pub name: String,
    pub args: Vec<Value>,
    pub result: Value,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ChapterTrace {
    pub chapter: String,
    pub identity: AssetIdentity,
    pub sources: Vec<SourceDigest>,
    pub ticks: u64,
    pub calls: Vec<CallRecord>,
    pub natives: Vec<NativeRecord>,
    pub gates: Vec<(u64, Gate, String)>,
}
impl ChapterTrace {
    /// Canonical script trace. Text is hex UTF-8 and reals are raw f32 bits.
    /// This is separate from the existing unit-state trace: neither loses fields.
    pub fn encode(&self) -> Vec<u8> {
        use std::fmt::Write;
        let mut out = format!(
            "pcx-script-trace 1\nchapter {}\nedition {}\nversion {}\nasset-source {}\nticks {}\n",
            hex(&self.chapter),
            hex(&self.identity.edition),
            hex(&self.identity.version),
            hex(&self.identity.source),
            self.ticks
        );
        for source in &self.sources {
            let _ = writeln!(out, "source {} {:016x}", hex(&source.label), source.hash);
        }
        let mut events = BTreeMap::new();
        for call in &self.calls {
            events.insert(
                call.sequence,
                format!(
                    "call {} {} {} {}",
                    tick_text(call.tick),
                    hex(&call.phase),
                    hex(&call.function),
                    value_text(&call.result)
                ),
            );
        }
        for native in &self.natives {
            let mut row = format!(
                "native {} {} {}",
                tick_text(native.tick),
                hex(&native.name),
                native.args.len()
            );
            for arg in &native.args {
                let _ = write!(row, " {}", value_text(arg));
            }
            let _ = write!(row, " result {}", value_text(&native.result));
            events.insert(native.sequence, row);
        }
        for (sequence, event) in events {
            let _ = writeln!(out, "event {sequence} {event}");
        }
        for (tick, gate, predicate) in &self.gates {
            let _ = writeln!(out, "gate {tick} {gate:?} {}", hex(predicate));
        }
        out.into_bytes()
    }
}
fn hex(text: &str) -> String {
    use std::fmt::Write;
    if text.is_empty() {
        return "-".into();
    }
    let mut out = String::new();
    for byte in text.bytes() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
fn value_text(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => format!("bool:{b}"),
        Value::Int(i) => format!("int:{i}"),
        Value::Real(r) => format!("real:{:08x}", r.to_bits()),
        Value::Str(s) => format!("string:{}", hex(s)),
        Value::Code(id) => format!("code:{}", id.0),
        Value::Handle(id) => format!("host-handle:{}", id.token()),
    }
}
fn tick_text(tick: Option<u64>) -> String {
    tick.map(|t| t.to_string()).unwrap_or_else(|| "init".into())
}
struct RecordingHost<H> {
    inner: H,
    tick: Option<u64>,
    natives: Vec<NativeRecord>,
    sequence: u64,
}
impl<H> RecordingHost<H> {
    fn next_sequence(&mut self) -> u64 {
        let sequence = self.sequence;
        self.sequence += 1;
        sequence
    }
}
impl<H: Host> Host for RecordingHost<H> {
    fn real_ops(&self) -> Option<&dyn pcx_script::RealOps> {
        self.inner.real_ops()
    }

    fn suspend(&mut self, name: &str, args: &[Value]) -> bool {
        self.inner.suspend(name, args)
    }
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        let result = self.inner.native(name, args)?;
        let sequence = self.next_sequence();
        self.natives.push(NativeRecord {
            sequence,
            tick: self.tick,
            name: name.into(),
            args: args.to_vec(),
            result: result.clone(),
        });
        Ok(result)
    }
    fn resolve_table_string(&self, id: u32) -> Result<Option<String>, String> {
        self.inner.resolve_table_string(id)
    }
    fn code_tag(&self, tag: &str) -> Option<String> {
        self.inner.code_tag(tag)
    }
}

/// Run all authored entrypoints, tick callbacks and scenario calls, then evaluate
/// outcome predicates at their scheduled ticks. False/non-boolean is failure.
/// This harness orders host callbacks before player/script calls, then predicates;
/// that ordering is not a claim about the original engine's unmeasured event loop.
pub fn run_chapter<H: CampaignHost>(
    scripts: &MapScripts,
    plan: &ChapterPlan,
    host: H,
) -> Result<ChapterTrace, String> {
    if plan.name.is_empty() || plan.entrypoints.is_empty() || plan.ticks == 0 {
        return Err("chapter name, entrypoints and nonzero tick count are required".into());
    }
    let mut required = [Gate::Dialogue, Gate::Quest, Gate::Victory, Gate::EndGame]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let mut checks = BTreeMap::<u64, Vec<&OutcomeCheck>>::new();
    for check in &plan.outcomes {
        if check.tick >= plan.ticks {
            return Err(format!(
                "outcome {:?} tick {} outside chapter",
                check.gate, check.tick
            ));
        }
        let function = scripts
            .program()
            .function(&check.predicate)
            .ok_or_else(|| format!("outcome predicate {:?} is not declared", check.predicate))?;
        if !function.params.is_empty() || function.ret.kind != pcx_script::TypeKind::Boolean {
            return Err(format!(
                "outcome predicate {:?} must take nothing and return boolean",
                check.predicate
            ));
        }
        required.remove(&check.gate);
        checks.entry(check.tick).or_default().push(check);
    }
    if !required.is_empty() {
        return Err(format!(
            "chapter is missing explicit outcome gates {required:?}"
        ));
    }
    let mut calls = BTreeMap::<u64, Vec<&ScriptCall>>::new();
    for call in &plan.calls {
        if call.tick >= plan.ticks {
            return Err(format!(
                "call {:?} tick {} outside chapter",
                call.function, call.tick
            ));
        }
        calls.entry(call.tick).or_default().push(call);
    }
    // Validate all entrypoints before globals or other native effects run.
    for name in &plan.entrypoints {
        let function = scripts
            .program()
            .function(name)
            .ok_or_else(|| format!("entrypoint {name:?} is not declared"))?;
        if !function.params.is_empty() || function.ret.kind != pcx_script::TypeKind::Nothing {
            return Err(format!(
                "entrypoint {name:?} must take nothing and return nothing"
            ));
        }
    }
    let mut trace = ChapterTrace {
        chapter: plan.name.clone(),
        identity: scripts.identity.clone(),
        sources: scripts.digests.clone(),
        ticks: plan.ticks,
        calls: Vec::new(),
        natives: Vec::new(),
        gates: Vec::new(),
    };
    let mut vm = scripts.vm(RecordingHost {
        inner: host,
        tick: None,
        natives: Vec::new(),
        sequence: 0,
    });
    for name in &plan.entrypoints {
        let result = vm
            .call(name, &[])
            .map_err(|e| scripts.diagnostic(&vm, &e))?;
        trace.calls.push(CallRecord {
            sequence: vm.host().inner.next_sequence(),
            tick: None,
            phase: "entry".into(),
            function: name.clone(),
            result,
        });
    }
    for tick in 0..plan.ticks {
        vm.host().inner.tick = Some(tick);
        let callbacks = vm
            .host()
            .inner
            .inner
            .advance_tick(tick)
            .map_err(|e| format!("chapter {:?} tick {tick}: {e}", plan.name))?;
        for id in callbacks {
            let function = scripts
                .program()
                .functions()
                .get(id.index())
                .ok_or_else(|| {
                    format!("tick {tick}: callback function id {} is not declared", id.0)
                })?;
            let name = function.name.clone();
            let result = vm
                .call(&name, &[])
                .map_err(|e| format!("tick {tick}: {}", scripts.diagnostic(&vm, &e)))?;
            trace.calls.push(CallRecord {
                sequence: vm.host().inner.next_sequence(),
                tick: Some(tick),
                phase: "callback".into(),
                function: name,
                result,
            });
        }
        for call in calls.get(&tick).into_iter().flatten() {
            let result = vm
                .call(&call.function, &call.args)
                .map_err(|e| format!("tick {tick}: {}", scripts.diagnostic(&vm, &e)))?;
            trace.calls.push(CallRecord {
                sequence: vm.host().inner.next_sequence(),
                tick: Some(tick),
                phase: "script".into(),
                function: call.function.clone(),
                result,
            });
        }
        for check in checks.get(&tick).into_iter().flatten() {
            let result = vm.call(&check.predicate, &[]).map_err(|e| {
                format!(
                    "tick {tick} {:?}: {}",
                    check.gate,
                    scripts.diagnostic(&vm, &e)
                )
            })?;
            if result != Value::Bool(true) {
                return Err(format!(
                    "chapter {:?} tick {tick} {:?} predicate {:?} returned {result}; expected true",
                    plan.name, check.gate, check.predicate
                ));
            }
            trace.calls.push(CallRecord {
                sequence: vm.host().inner.next_sequence(),
                tick: Some(tick),
                phase: format!("check:{:?}", check.gate),
                function: check.predicate.clone(),
                result,
            });
            trace
                .gates
                .push((tick, check.gate, check.predicate.clone()));
        }
    }
    trace.natives = std::mem::take(&mut vm.host().inner.natives);
    Ok(trace)
}

/// Each chapter gets a fresh host/VM. A failed chapter prevents the next from running.
/// Chapter order and sources come from the caller's parsed campaign manifest.
pub fn run_campaign<H: CampaignHost>(
    chapters: &[(&MapScripts, &ChapterPlan)],
    mut host: impl FnMut(usize, &MapScripts) -> Result<H, String>,
) -> Result<Vec<ChapterTrace>, String> {
    if chapters.is_empty() {
        return Err("campaign has no chapters".into());
    }
    let mut results = Vec::new();
    for (index, (scripts, plan)) in chapters.iter().enumerate() {
        let runtime = host(index, scripts)?;
        results.push(
            run_chapter(scripts, plan, runtime)
                .map_err(|e| format!("campaign chapter {index} {:?}: {e}", plan.name))?,
        );
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripts::ScriptAsset;
    const MAP: &[u8] = b"globals\ninteger stage = 0\nendglobals\nfunction config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nset stage = 1\nendfunction\nfunction Dialogue takes nothing returns boolean\nreturn stage >= 1\nendfunction\nfunction Quest takes nothing returns boolean\nreturn stage >= 2\nendfunction\nfunction Victory takes nothing returns boolean\nreturn stage >= 3\nendfunction\nfunction EndGame takes nothing returns boolean\nreturn stage >= 4\nendfunction\nfunction Callback takes nothing returns nothing\nset stage = stage + 1\nendfunction\nfunction Complete takes nothing returns nothing\nset stage = stage + 1\nendfunction\n";
    fn scripts() -> MapScripts {
        MapScripts::compile(
            AssetIdentity {
                edition: "synthetic".into(),
                version: "fixture-1".into(),
                source: "memory".into(),
            },
            ScriptAsset {
                label: "common.j",
                bytes: b"native FixtureUnsupported takes nothing returns nothing\n",
            },
            ScriptAsset {
                label: "Blizzard.j",
                bytes: b"function FixtureBJ takes nothing returns nothing\nendfunction\n",
            },
            ScriptAsset {
                label: "war3map.j",
                bytes: MAP,
            },
            None,
        )
        .unwrap()
    }
    struct Runtime {
        callback: FunctionId,
        deliver: bool,
    }
    impl Host for Runtime {
        fn native(&mut self, name: &str, _: &[Value]) -> Result<Value, String> {
            Err(format!("unsupported {name}"))
        }
    }
    impl CampaignHost for Runtime {
        fn advance_tick(&mut self, tick: u64) -> Result<Vec<FunctionId>, String> {
            Ok(if self.deliver && tick == 1 {
                vec![self.callback]
            } else {
                Vec::new()
            })
        }
    }
    fn plan() -> ChapterPlan {
        ChapterPlan {
            name: "synthetic-chapter".into(),
            ticks: 4,
            entrypoints: vec!["config".into(), "main".into()],
            calls: vec![
                ScriptCall {
                    tick: 2,
                    function: "Complete".into(),
                    args: Vec::new(),
                },
                ScriptCall {
                    tick: 3,
                    function: "Complete".into(),
                    args: Vec::new(),
                },
            ],
            outcomes: [
                (0, Gate::Dialogue, "Dialogue"),
                (1, Gate::Quest, "Quest"),
                (2, Gate::Victory, "Victory"),
                (3, Gate::EndGame, "EndGame"),
            ]
            .into_iter()
            .map(|(tick, gate, predicate)| OutcomeCheck {
                tick,
                gate,
                predicate: predicate.into(),
            })
            .collect(),
        }
    }
    #[test]
    fn every_gate_is_observed_and_a_missing_callback_cannot_pass() {
        let scripts = scripts();
        let plan = plan();
        let callback = scripts.program().function_id("Callback").unwrap();
        let first = run_chapter(
            &scripts,
            &plan,
            Runtime {
                callback,
                deliver: true,
            },
        )
        .unwrap();
        assert_eq!(first.gates.len(), 4);
        for _ in 0..100 {
            assert_eq!(
                run_chapter(
                    &scripts,
                    &plan,
                    Runtime {
                        callback,
                        deliver: true
                    }
                )
                .unwrap()
                .encode(),
                first.encode()
            );
        }
        let failure = run_chapter(
            &scripts,
            &plan,
            Runtime {
                callback,
                deliver: false,
            },
        )
        .unwrap_err();
        assert!(failure.contains("tick 1 Quest"), "{failure}");
        assert!(failure.contains("false"));
        let mut missing = plan.clone();
        missing.outcomes.pop();
        assert!(
            run_chapter(
                &scripts,
                &missing,
                Runtime {
                    callback,
                    deliver: true
                }
            )
            .unwrap_err()
            .contains("EndGame")
        );
    }
    #[test]
    fn campaign_stops_before_opening_next_chapter_after_failed_real_predicate() {
        let scripts = scripts();
        let plan = plan();
        let mut opened = Vec::new();
        let result = run_campaign(&[(&scripts, &plan), (&scripts, &plan)], |index, scripts| {
            opened.push(index);
            Ok(Runtime {
                callback: scripts.program().function_id("Callback").unwrap(),
                deliver: false,
            })
        });
        assert!(result.unwrap_err().contains("chapter 0"));
        assert_eq!(opened, [0]);
    }
    #[test]
    fn missing_entrypoint_and_out_of_bounds_call_fail_before_execution() {
        let scripts = scripts();
        let mut plan = plan();
        let callback = scripts.program().function_id("Callback").unwrap();
        plan.entrypoints.push("NotDeclared".into());
        assert!(
            run_chapter(
                &scripts,
                &plan,
                Runtime {
                    callback,
                    deliver: true
                }
            )
            .unwrap_err()
            .contains("NotDeclared")
        );
        plan.entrypoints.pop();
        plan.calls[0].tick = 4;
        assert!(
            run_chapter(
                &scripts,
                &plan,
                Runtime {
                    callback,
                    deliver: true
                }
            )
            .unwrap_err()
            .contains("outside chapter")
        );
    }
}

#[cfg(test)]
mod recording_tests {
    use super::*;
    use crate::scripts::ScriptAsset;
    #[test]
    fn native_observations_keep_order_relative_to_entrypoints_and_gate_calls() {
        let common = b"native Probe takes nothing returns boolean\n";
        let blizzard =
            b"function Observed takes nothing returns boolean\nreturn Probe()\nendfunction\n";
        let map = b"function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall Probe()\nendfunction\n";
        let scripts = MapScripts::compile(
            AssetIdentity {
                edition: "synthetic".into(),
                version: "1".into(),
                source: "memory".into(),
            },
            ScriptAsset {
                label: "common.j",
                bytes: common,
            },
            ScriptAsset {
                label: "Blizzard.j",
                bytes: blizzard,
            },
            ScriptAsset {
                label: "war3map.j",
                bytes: map,
            },
            None,
        )
        .unwrap();
        struct Observed(bool);
        impl Host for Observed {
            fn native(&mut self, name: &str, _: &[Value]) -> Result<Value, String> {
                if name == "Probe" {
                    Ok(Value::Bool(self.0))
                } else {
                    Err(name.into())
                }
            }
        }
        impl CampaignHost for Observed {
            fn advance_tick(&mut self, _: u64) -> Result<Vec<FunctionId>, String> {
                Ok(Vec::new())
            }
        }
        let plan = ChapterPlan {
            name: "host-observed".into(),
            ticks: 1,
            entrypoints: vec!["config".into(), "main".into()],
            calls: Vec::new(),
            outcomes: [Gate::Dialogue, Gate::Quest, Gate::Victory, Gate::EndGame]
                .into_iter()
                .map(|gate| OutcomeCheck {
                    tick: 0,
                    gate,
                    predicate: "Observed".into(),
                })
                .collect(),
        };
        assert!(
            run_chapter(&scripts, &plan, Observed(false))
                .unwrap_err()
                .contains("false")
        );
        let trace = run_chapter(&scripts, &plan, Observed(true)).unwrap();
        assert_eq!(trace.natives.len(), 5);
        assert!(trace.natives[0].sequence < trace.calls[1].sequence);
        let text = String::from_utf8(trace.encode()).unwrap();
        let events: Vec<_> = text
            .lines()
            .filter(|line| line.starts_with("event "))
            .collect();
        assert!(events[1].contains(" native "));
        assert!(events[2].contains(" call "));
        for (i, event) in events.iter().enumerate() {
            assert!(event.starts_with(&format!("event {i} ")));
        }
    }
}
