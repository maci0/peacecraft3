//! Preload scripts have their own globals and share the admitted world and asset reader.
use super::MapStartupHost;
use crate::preload::{Report, Rules, State};
use pcx_assets::Resolver;
use pcx_script::{Host, Prelude, RealOps, Value, Vm};
use std::sync::Arc;

const NATIVES: [(&str, &[&str]); 6] = [
    ("Preloader", &["string"]),
    ("Preload", &["string"]),
    ("PreloadEnd", &["real"]),
    ("PreloadStart", &[]),
    ("PreloadRefresh", &[]),
    ("PreloadEndEx", &[]),
];
const GEN_NATIVES: [(&str, &[&str]); 3] = [
    ("PreloadGenClear", &[]),
    ("PreloadGenStart", &[]),
    ("PreloadGenEnd", &["string"]),
];
impl MapStartupHost {
    pub fn admit_preloader(&mut self, rules: Rules, reader: Arc<Resolver>) -> Result<(), String> {
        if self.preload.is_some()
            || self.objects().player_client_sha256() != Some(rules.image_sha256)
        {
            return Err("Preloader: source differs or already admitted".into());
        }
        let program = self.context().scripts.program();
        let common = &self
            .context()
            .scripts
            .digests
            .first()
            .ok_or("Preloader: missing common source")?
            .label;
        for (name, params) in NATIVES.into_iter().chain(GEN_NATIVES) {
            let Some(native) = program.natives().get(name) else {
                continue;
            };
            if native.source != *common
                || native.is_constant
                || native.ret.kind.name() != "nothing"
                || native.ret.array
                || native.params.len() != params.len()
                || native
                    .params
                    .iter()
                    .zip(params)
                    .any(|(p, t)| p.ty.array || p.ty.kind.name() != *t)
            {
                return Err(format!("{name}: selected common declaration differs"));
            }
        }
        self.preload = Some(State::new(rules, reader));
        Ok(())
    }
    pub fn preload_report(&self) -> Option<&Report> {
        self.preload.as_ref().map(|s| &s.report)
    }
    /// Native async progress for a loading consumer: queued, warmed, still
    /// pending and visited bytes. No wall clock is read here.
    pub fn preload_progress(&self) -> Option<(usize, usize, usize, u64)> {
        self.preload.as_ref().map(|s| s.progress())
    }
    /// Drop every still-pending request; the warmed prefix is retained.
    pub fn preload_cancel_pending(&mut self) -> usize {
        self.preload.as_mut().map_or(0, |s| s.cancel_pending())
    }
    /// Warm up to `limit` pending requests without touching the recorded timeout.
    pub fn preload_poll(&mut self, limit: usize) -> Result<usize, String> {
        self.preload
            .as_mut()
            .ok_or_else(|| "Preload: not admitted".to_owned())
            .and_then(|s| s.poll(limit))
    }
    /// Release retained archive winners; loose overrides stay live.
    pub fn preload_release_retained(&self) -> Result<(usize, usize), String> {
        self.preload
            .as_ref()
            .ok_or_else(|| "Preload: not admitted".to_owned())
            .and_then(|s| s.release_retained())
    }
    pub fn preload_generated(&self, path: &str) -> Option<&str> {
        self.preload
            .as_ref()?
            .report
            .generated
            .get(path)
            .map(String::as_str)
    }
    pub(super) fn preload_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, String>> {
        if self.preload.is_none()
            || !(NATIVES
                .iter()
                .chain(GEN_NATIVES.iter())
                .any(|(n, _)| *n == name))
        {
            return None;
        }
        // Refuse undeclared optional generator natives without touching state.
        if GEN_NATIVES.iter().any(|(n, _)| *n == name)
            && self
                .context()
                .scripts
                .program()
                .natives()
                .get(name)
                .is_none()
        {
            return Some(Err(format!("{name}: selected common declaration absent")));
        }
        Some((|| {
            match (name, args) {
                ("Preloader", [Value::Str(path)]) => self.run_preloader(path)?,
                ("Preload", [Value::Str(path)]) => self.preload.as_mut().unwrap().queue(path)?,
                ("PreloadEnd", [Value::Real(timeout)]) => self
                    .preload
                    .as_mut()
                    .unwrap()
                    .drain(usize::MAX, Some(*timeout))?,
                ("PreloadStart", []) => {
                    let s = self.preload.as_mut().unwrap();
                    s.drain(usize::MAX, Some(s.start_timeout()))?;
                }
                ("PreloadRefresh", []) => {
                    let s = self.preload.as_mut().unwrap();
                    s.drain(s.refresh_assets(), None)?;
                }
                ("PreloadEndEx", []) => {
                    // EndEx takes no timeout argument: warm everything that is
                    // still pending without recording a deadline. (A genuine
                    // zero `PreloadEnd` timeout records the deadline and lets
                    // the caller poll/cancel instead; see `drain`.)
                    self.preload.as_mut().unwrap().drain(usize::MAX, None)?
                }
                ("PreloadGenClear", []) => self.preload.as_mut().unwrap().gen_clear(),
                ("PreloadGenStart", []) => self.preload.as_mut().unwrap().gen_start()?,
                ("PreloadGenEnd", [Value::Str(path)]) => {
                    self.preload.as_mut().unwrap().gen_end(path)?
                }
                _ => {
                    return Err(format!(
                        "{name}: arguments do not match the selected declaration"
                    ));
                }
            }
            Ok(Value::Null)
        })())
    }
    fn run_preloader(&mut self, path: &pcx_script::ScriptString) -> Result<(), String> {
        let state = self.preload.as_mut().expect("admitted");
        if !state.filename(path)? {
            return Ok(());
        }
        let path = path
            .as_utf8()
            .map_err(|why| format!("Preloader: unsupported path UTF8: {why}"))?;
        if state.depth >= 8 || state.report.scripts.len() >= 128 {
            return Err("Preloader: script recursion/count budget".into());
        }
        state
            .report
            .scripts
            .try_reserve(1)
            .map_err(|_| "Preloader: script report allocation")?;
        state.report.scripts.push(path.into());
        state.depth += 1;
        let result = self.execute_preload_script(path);
        self.preload.as_mut().expect("retained").depth -= 1;
        result
    }
    fn execute_preload_script(&mut self, path: &str) -> Result<(), String> {
        let state = self.preload.as_ref().expect("admitted");
        let reader = Arc::clone(&state.reader);
        let common_path = state.common().to_owned();
        let entry = state.entry().to_owned();
        let script = match reader.resolve_bounded(path, 1 << 20) {
            Ok(asset) => asset,
            Err(pcx_assets::Error::MissingAsset { .. }) => {
                self.preload
                    .as_mut()
                    .unwrap()
                    .report
                    .missing
                    .push(path.into());
                return Ok(());
            }
            Err(error) => return Err(format!("Preloader {path}: {error}")),
        };
        let common = reader
            .resolve_bounded(&common_path, 1 << 20)
            .map_err(|e| format!("Preloader {common_path}: {e}"))?;
        let context = Arc::clone(self.context());
        let common_label = &context
            .scripts
            .digests
            .first()
            .ok_or("Preloader: common source absent")?
            .label;
        let common_source = context
            .scripts
            .source(common_label)
            .ok_or("Preloader: admitted common bytes absent")?;
        if common.bytes != common_source.as_bytes() {
            return Err("Preloader: resolved common differs from admitted common".into());
        }
        let (label, _) =
            crate::campaign_assets::identity(&script, &context.sources.map.edition, 16 << 10)?;
        let source = std::str::from_utf8(&script.bytes).map_err(|e| format!("{label}: {e}"))?;
        let tree =
            pcx_script::parse(source).map_err(|e| format!("{label}: {}", e.render(source)))?;
        if tree.natives().next().is_some() {
            return Err(format!(
                "{label}: preload scripts cannot introduce foreign native declarations"
            ));
        }
        let common_tree = pcx_script::parse(common_source)
            .map_err(|e| format!("{common_label}: {}", e.render(common_source)))?;
        let program = pcx_script::resolve_with_diagnostics(
            &tree,
            &[Prelude::new(common_label, &common_tree)],
        )
        .map_err(|d| format!("{label}: {}", d.error.render(source)))?;
        let function = program
            .function(&entry)
            .ok_or_else(|| format!("{label}: preload entry {entry} is absent"))?;
        if function.source != "the map script"
            || !function.params.is_empty()
            || function.ret.kind.name() != "nothing"
        {
            return Err(format!("{label}: preload entry signature differs"));
        }
        let mut vm = Vm::new(&program, Shared(self));
        let result = vm.call(&entry, &[]);
        match result {
            Ok(Value::Null) => Ok(()),
            Ok(_) => Err(format!("{label}: preload entry returned a value")),
            Err(error) => {
                let text = if vm.error_source() == Some(common_label) {
                    common_source
                } else {
                    source
                };
                Err(format!("Preloader {label}: {}", error.render(text)))
            }
        }
    }
}
struct Shared<'a>(&'a mut MapStartupHost);
impl Host for Shared<'_> {
    fn string_hash(&self, bytes: &[u8]) -> Result<i32, String> {
        self.0.string_hash(bytes)
    }
    fn native_table_string(&self, id: u32) -> Result<Option<String>, String> {
        self.0.native_table_string(id)
    }
    fn native_with_strings(
        &mut self,
        name: &str,
        args: &[Value],
        strings: &pcx_script::text::NativeStringLookup<'_>,
    ) -> Result<Value, String> {
        if args.iter().any(|v| matches!(v, Value::Code(_))) {
            return Err(format!(
                "Preloader {name}: cross-program callbacks are unsupported"
            ));
        }
        self.0.native_with_strings(name, args, strings)
    }
    fn real_ops(&self) -> Option<&dyn RealOps> {
        self.0.real_ops()
    }
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        // Function IDs in this isolated VM cannot be installed as callbacks in the map VM.
        if args.iter().any(|v| matches!(v, Value::Code(_))) {
            return Err(format!(
                "Preloader {name}: cross-program callbacks are unsupported"
            ));
        }
        self.0.native(name, args)
    }
}

#[cfg(test)]
mod native_string_tests {
    use super::*;
    const WTS: &[u8] = b"STRING 7\r\n{\r\nTRIGSTR_23\r\n}\r\nSTRING 23\r\n{\r\nother\r\n}\r\n";
    #[test]
    fn string_hash_preload_and_startup_forward_raw_and_refuse_unlent_world() {
        let policy = crate::playercolor_host::ScriptHashPolicy::synthetic_fixture();
        let expected = policy.hash(b"TRIGSTR_7").unwrap();
        let world = crate::neutral_player::native_hash_world_fixture(None, Some(policy));
        let program = world.context().scripts.shared_program();
        let mut startup = MapStartupHost::timer_session_fixture(world);
        assert_eq!(startup.string_hash(b"TRIGSTR_7").unwrap(), expected);
        let mut vm = Vm::new(&program, Shared(&mut startup));
        assert_eq!(
            vm.call("hash", &[Value::Str("TRIGSTR_7".into())]).unwrap(),
            Value::Int(expected)
        );
        drop(vm);
        let world = startup.inner.take().unwrap();
        assert!(
            Shared(&mut startup)
                .string_hash(b"raw")
                .unwrap_err()
                .contains("world is not lent")
        );
        startup.inner = Some(world);
        assert_eq!(
            Shared(&mut startup).string_hash(b"TRIGSTR_7").unwrap(),
            expected
        );
    }
    #[test]
    fn native_string_preload_shared_uses_selected_table_and_retains_callback_guard() {
        let world = crate::neutral_player::native_string_world_fixture(Some(WTS));
        let program = world.context().scripts.shared_program();
        let callback = program.function_id("length").unwrap();
        let mut startup = MapStartupHost::timer_session_fixture(world);
        let mut vm = Vm::new(&program, Shared(&mut startup));
        assert_eq!(vm.call("length", &[]).unwrap(), Value::Int(10));
        drop(vm);
        let mut shared = Shared(&mut startup);
        let source = |_| Ok(Some("explicit source".into()));
        shared
            .native_with_strings("SetMapName", &[Value::Str("TRIGSTR_7".into())], &source)
            .unwrap();
        assert!(
            shared
                .native("SetMapName", &[Value::Code(callback)])
                .unwrap_err()
                .contains("cross-program callbacks")
        );
        assert!(
            shared
                .native_with_strings("SetMapName", &[Value::Code(callback)], &source)
                .unwrap_err()
                .contains("cross-program callbacks")
        );
        assert_eq!(
            startup
                .objects()
                .map_config()
                .unwrap()
                .name
                .as_ref()
                .map(|s| s.as_utf8().unwrap()),
            Some("explicit source")
        );
    }
    #[test]
    fn native_string_preload_shared_keeps_missing_source_distinct_from_absent_entry() {
        let world = crate::neutral_player::native_string_world_fixture(None);
        let mut startup = MapStartupHost::timer_session_fixture(world);
        assert!(
            Shared(&mut startup)
                .native_table_string(7)
                .unwrap_err()
                .contains("source was not supplied")
        );
        let world = crate::neutral_player::native_string_world_fixture(Some(WTS));
        let mut startup = MapStartupHost::timer_session_fixture(world);
        assert_eq!(Shared(&mut startup).native_table_string(91).unwrap(), None);
        let world = startup.inner.take().unwrap();
        assert!(
            Shared(&mut startup)
                .native_table_string(7)
                .unwrap_err()
                .contains("world is not lent")
        );
        startup.inner = Some(world);
    }
}
