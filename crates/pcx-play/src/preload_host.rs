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
        for (name, params) in NATIVES {
            let native = program
                .natives()
                .get(name)
                .ok_or_else(|| format!("{name}: selected common declaration absent"))?;
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
    pub(super) fn preload_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, String>> {
        if self.preload.is_none() || !NATIVES.iter().any(|(n, _)| *n == name) {
            return None;
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
                    self.preload.as_mut().unwrap().drain(usize::MAX, Some(0.))?
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
    fn run_preloader(&mut self, path: &str) -> Result<(), String> {
        let state = self.preload.as_mut().expect("admitted");
        if !state.filename(path)? {
            return Ok(());
        }
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
