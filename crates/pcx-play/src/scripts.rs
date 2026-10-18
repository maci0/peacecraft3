//! Compile an explicitly identified map's actual script sources, without I/O.
//! The caller reads local archives; no source bytes enter a trace.
use pcx_script::{Host, NullHost, OwnedVm, Prelude, Program, Value, Vm};
use pcx_trig::wts::Table;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetIdentity {
    pub edition: String,
    pub version: String,
    /// Archive/install identity supplied by the importer, never guessed from a path.
    pub source: String,
}
#[derive(Clone, Copy)]
pub struct ScriptAsset<'a> {
    /// Archive + member identity, retained in diagnostics and trace provenance.
    pub label: &'a str,
    pub bytes: &'a [u8],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDigest {
    pub label: String,
    /// FNV-1a is a reproducibility checksum, not authentication.
    pub hash: u64,
}

pub struct MapScripts {
    pub identity: AssetIdentity,
    pub digests: Vec<SourceDigest>,
    program: Arc<Program>,
    sources: BTreeMap<String, String>,
    map_label: String,
    wts: Option<Table>,
    wts_label: Option<String>,
}
impl MapScripts {
    /// Load precisely one install's preludes and one map. No edition unions.
    pub fn compile(
        identity: AssetIdentity,
        common: ScriptAsset<'_>,
        blizzard: ScriptAsset<'_>,
        map: ScriptAsset<'_>,
        wts: Option<ScriptAsset<'_>>,
    ) -> Result<Self, String> {
        if identity.edition.is_empty() || identity.version.is_empty() || identity.source.is_empty()
        {
            return Err("edition, version and asset source must be explicit".into());
        }
        for prelude in [common, blizzard] {
            if prelude.label == "the map script" {
                return Err(format!(
                    "{}: prelude label aliases the VM main source identity",
                    prelude.label
                ));
            }
        }
        let mut sources = BTreeMap::new();
        let mut digests = Vec::new();
        let mut trees = Vec::new();
        for asset in [common, blizzard, map] {
            if asset.bytes.is_empty() {
                return Err(format!("{}: script asset is empty", asset.label));
            }
            if asset.label.is_empty() || sources.contains_key(asset.label) {
                return Err(format!(
                    "missing or duplicate script source {:?}",
                    asset.label
                ));
            }
            let source = std::str::from_utf8(asset.bytes).map_err(|e| {
                format!(
                    "{}: script is not UTF-8 at byte {}",
                    asset.label,
                    e.valid_up_to()
                )
            })?;
            let tree = pcx_script::parse(source)
                .map_err(|e| format!("{}:{}", asset.label, e.render(source)))?;
            sources.insert(asset.label.to_string(), source.to_string());
            digests.push(SourceDigest {
                label: asset.label.into(),
                hash: pcx_script::fnv1a64(asset.bytes),
            });
            trees.push(tree);
        }
        let program = pcx_script::resolve_with_diagnostics(
            &trees[2],
            &[
                Prelude::new(common.label, &trees[0]),
                Prelude::new(blizzard.label, &trees[1]),
            ],
        )
        .map_err(|diagnostic| {
            let label = diagnostic.source_label.as_deref().unwrap_or(map.label);
            match sources.get(label) {
                Some(source) => format!("{label}:{}", diagnostic.error.render(source)),
                None => format!("{label}:{}", diagnostic.error),
            }
        })?;
        let wts_label = wts.map(|w| w.label.to_string());
        let wts = wts
            .map(|asset| {
                if asset.label.is_empty() || sources.contains_key(asset.label) {
                    return Err(format!("missing or duplicate WTS source {:?}", asset.label));
                }
                let table =
                    Table::parse(asset.bytes).map_err(|e| format!("{}: {e}", asset.label))?;
                if !table.ids_are_unique() {
                    return Err(format!("{}: duplicate WTS ids", asset.label));
                }
                digests.push(SourceDigest {
                    label: asset.label.into(),
                    hash: pcx_script::fnv1a64(asset.bytes),
                });
                Ok(table)
            })
            .transpose()?;
        Ok(Self {
            identity,
            digests,
            program: Arc::new(program),
            sources,
            map_label: map.label.into(),
            wts,
            wts_label,
        })
    }
    pub fn program(&self) -> &Program {
        &self.program
    }
    /// Share this exact compiled program with a session-owned Host.
    pub fn shared_program(&self) -> Arc<Program> {
        Arc::clone(&self.program)
    }
    pub fn vm<H: Host>(&self, host: H) -> Vm<'_, LocalizedHost<'_, H>> {
        Vm::new(
            &self.program,
            LocalizedHost {
                inner: host,
                table: self.wts.as_ref(),
                label: self.wts_label.as_deref(),
            },
        )
    }
    /// Retain this map's program, localization and diagnostic sources across calls.
    pub fn owned_vm<H: Host>(self: Arc<Self>, host: H) -> OwnedVm<OwnedLocalizedHost<H>> {
        OwnedVm::new_owned(
            Arc::clone(&self.program),
            OwnedLocalizedHost {
                inner: host,
                context: self,
            },
        )
    }
    pub fn diagnostic<H: Host>(&self, vm: &Vm<'_, H>, error: &pcx_script::Error) -> String {
        self.render_diagnostic(vm.error_source(), error)
    }
    fn render_diagnostic(&self, source: Option<&str>, error: &pcx_script::Error) -> String {
        let label = match source {
            Some("the map script") | None => &self.map_label,
            Some(label) => label,
        };
        match self.sources.get(label) {
            Some(source) => format!("{label}:{}", error.render(source)),
            None => format!("{label}:{error}"),
        }
    }
    /// A useful fail-closed probe: load globals, config, then main with no game host.
    /// This returns the first genuine unsupported native, never synthetic success.
    pub fn probe(&self) -> Result<(), String> {
        let mut vm = self.vm(NullHost);
        for function in ["config", "main"] {
            vm.call(function, &[])
                .map_err(|e| self.diagnostic(&vm, &e))?;
        }
        Ok(())
    }
}

pub struct LocalizedHost<'a, H> {
    pub inner: H,
    table: Option<&'a Table>,
    label: Option<&'a str>,
}
impl<'a, H> LocalizedHost<'a, H> {
    pub(crate) fn new(inner: H, table: Option<&'a Table>, label: Option<&'a str>) -> Self {
        Self {
            inner,
            table,
            label,
        }
    }
}
impl<H: Host> Host for LocalizedHost<'_, H> {
    fn real_ops(&self) -> Option<&dyn pcx_script::RealOps> {
        self.inner.real_ops()
    }

    fn suspend(&mut self, name: &str, args: &[Value]) -> bool {
        self.inner.suspend(name, args)
    }
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        self.inner.native(name, args)
    }
    fn resolve_table_string(&self, id: u32) -> Result<Option<String>, String> {
        resolve_table_string(self.table, self.label, id)
    }
    fn code_tag(&self, tag: &str) -> Option<String> {
        self.inner.code_tag(tag)
    }
}

/// A localized host retaining precisely the compiled map context its VM uses.
pub struct OwnedLocalizedHost<H> {
    pub inner: H,
    context: Arc<MapScripts>,
}
impl<H> OwnedLocalizedHost<H> {
    pub fn context(&self) -> &Arc<MapScripts> {
        &self.context
    }

    /// Render the VM's error using its retained source label and source text.
    pub fn diagnostic(&self, source: Option<&str>, error: &pcx_script::Error) -> String {
        self.context.render_diagnostic(source, error)
    }
}
impl<H: Host> Host for OwnedLocalizedHost<H> {
    fn real_ops(&self) -> Option<&dyn pcx_script::RealOps> {
        self.inner.real_ops()
    }

    fn suspend(&mut self, name: &str, args: &[Value]) -> bool {
        self.inner.suspend(name, args)
    }
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        self.inner.native(name, args)
    }
    fn resolve_table_string(&self, id: u32) -> Result<Option<String>, String> {
        resolve_table_string(
            self.context.wts.as_ref(),
            self.context.wts_label.as_deref(),
            id,
        )
    }
    fn code_tag(&self, tag: &str) -> Option<String> {
        self.inner.code_tag(tag)
    }
}

fn resolve_table_string(
    table: Option<&Table>,
    label: Option<&str>,
    id: u32,
) -> Result<Option<String>, String> {
    let Some(table) = table else {
        return Ok(None);
    };
    let Some(body) = table.body(id) else {
        return Ok(None);
    };
    std::str::from_utf8(body)
        .map(|s| Some(s.to_string()))
        .map_err(|e| {
            format!(
                "{}: WTS entry {id} is not UTF-8 at body byte {}",
                label.unwrap_or("war3map.wts"),
                e.valid_up_to()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> AssetIdentity {
        AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture-1".into(),
            source: "in-memory".into(),
        }
    }
    fn load(
        common: &[u8],
        blizzard: &[u8],
        map: &[u8],
        wts: Option<&[u8]>,
    ) -> Result<MapScripts, String> {
        let common = if common.is_empty() {
            b"native FixtureUnsupported takes nothing returns nothing\n".as_slice()
        } else {
            common
        };
        let blizzard = if blizzard.is_empty() {
            b"function FixtureBJ takes nothing returns nothing\nendfunction\n".as_slice()
        } else {
            blizzard
        };
        let map = if map.is_empty() {
            b"function main takes nothing returns nothing\nendfunction\n".as_slice()
        } else {
            map
        };
        MapScripts::compile(
            identity(),
            ScriptAsset {
                label: "common.j",
                bytes: common,
            },
            ScriptAsset {
                label: "Blizzard.j",
                bytes: blizzard,
            },
            ScriptAsset {
                label: "map/war3map.j",
                bytes: map,
            },
            wts.map(|bytes| ScriptAsset {
                label: "map/war3map.wts",
                bytes,
            }),
        )
    }
    #[test]
    fn both_localized_hosts_forward_a_park_without_executing_the_native() {
        struct Park;
        impl Host for Park {
            fn suspend(&mut self, name: &str, args: &[Value]) -> bool {
                assert_eq!(name, "Park");
                assert!(args.is_empty());
                true
            }
            fn native(&mut self, _: &str, _: &[Value]) -> Result<Value, String> {
                Err("a parked native must not run".into())
            }
        }
        let scripts = Arc::new(
            load(
                b"native Park takes nothing returns integer\n",
                b"function Inner takes nothing returns integer\nreturn Park()+1\nendfunction\n",
                b"function main takes nothing returns integer\nreturn Inner()+2\nendfunction\n",
                None,
            )
            .unwrap(),
        );
        let mut borrowed = scripts.vm(Park);
        assert_eq!(
            borrowed.invoke("main", &[]).unwrap(),
            pcx_script::Exec::Suspended
        );
        assert_eq!(
            borrowed.resume(Value::Int(4)).unwrap(),
            pcx_script::Exec::Finished(Value::Int(7))
        );
        let mut owned = Arc::clone(&scripts).owned_vm(Park);
        assert_eq!(
            owned.invoke("main", &[]).unwrap(),
            pcx_script::Exec::Suspended
        );
        assert_eq!(
            owned.resume(Value::Int(4)).unwrap(),
            pcx_script::Exec::Finished(Value::Int(7))
        );
        assert!(Arc::ptr_eq(owned.host().context(), &scripts));
    }

    #[test]
    fn actual_supplied_prelude_body_runs_and_localization_keeps_bytes() {
        let scripts = load(b"native Capture takes string text returns nothing\n", b"function Message takes nothing returns nothing\ncall Capture(\"TRIGSTR_5\")\nendfunction\n", b"function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall Message()\nendfunction\n", Some(b"STRING 5\r\n{\r\n|cffff0000Authored|r|nText\r\n}\r\n")).unwrap();
        struct Capture(Vec<String>);
        impl Host for Capture {
            fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
                match (name, args) {
                    ("Capture", [Value::Str(text)]) => {
                        self.0.push(text.clone());
                        Ok(Value::Null)
                    }
                    _ => Err(name.into()),
                }
            }
        }
        let mut vm = scripts.vm(Capture(Vec::new()));
        vm.call("config", &[]).unwrap();
        vm.call("main", &[]).unwrap();
        assert_eq!(vm.host().inner.0, ["|cffff0000Authored|r|nText"]);
        assert_eq!(scripts.digests.len(), 4);
    }
    #[test]
    fn unsupported_native_reports_its_actual_prelude_source_and_line() {
        let scripts = load(b"native Missing takes nothing returns nothing\n", b"function BJ takes nothing returns nothing\ncall Missing()\nendfunction\n", b"function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall BJ()\nendfunction\n", None).unwrap();
        let error = scripts.probe().unwrap_err();
        assert!(error.contains("Blizzard.j:2:"), "{error}");
        assert!(error.contains("Missing"));
    }
    #[test]
    fn only_used_wts_entry_needs_utf8_and_missing_is_distinct() {
        let scripts = load(
            b"",
            b"",
            b"function main takes nothing returns string\nreturn \"TRIGSTR_8\"\nendfunction\n",
            Some(b"STRING 8\r\n{\r\n\xff\r\n}\r\n"),
        )
        .unwrap();
        let mut vm = scripts.vm(NullHost);
        let error = vm.call("main", &[]).unwrap_err();
        let error = scripts.diagnostic(&vm, &error);
        assert!(error.contains("entry 8 is not UTF-8"), "{error}");
        assert!(error.contains("map/war3map.j:2:"), "{error}");
        assert!(
            load(
                b"",
                b"",
                b"",
                Some(b"STRING 1\r\n{\r\na\r\n}\r\nSTRING 1\r\n{\r\nb\r\n}\r\n")
            )
            .err()
            .unwrap()
            .contains("duplicate WTS")
        );
    }
}

#[cfg(test)]
mod local_original_tests {
    use super::*;
    /// Extracted files remain local. All paths and edition/version evidence are
    /// supplied explicitly; absence fails instead of silently skipping coverage.
    #[test]
    #[ignore = "requires local original script paths and measured asset identity via PEACECRAFT3_SCRIPT_* environment variables"]
    fn original_sources_compile_and_probe_fails_closed_at_first_unsupported_native() {
        let env = |name: &str| {
            std::env::var(format!("PEACECRAFT3_SCRIPT_{name}"))
                .unwrap_or_else(|_| panic!("set PEACECRAFT3_SCRIPT_{name} explicitly"))
        };
        let identity = AssetIdentity {
            edition: env("EDITION"),
            version: env("VERSION"),
            source: env("SOURCE"),
        };
        let paths = [env("COMMON"), env("BLIZZARD"), env("MAP")];
        let bytes: Vec<_> = paths
            .iter()
            .map(|p| std::fs::read(p).unwrap_or_else(|e| panic!("{p}: {e}")))
            .collect();
        let asset = |i: usize| ScriptAsset {
            label: &paths[i],
            bytes: &bytes[i],
        };
        let scripts = MapScripts::compile(identity, asset(0), asset(1), asset(2), None)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(
            !scripts.program().natives().is_empty(),
            "original common.j must declare natives"
        );
        let error = scripts
            .probe()
            .expect_err("a host-free original map may not pretend to run game natives");
        assert!(
            error.contains("native `") && error.contains("not implemented"),
            "{error}"
        );
        println!("actual original-source execution boundary: {error}");
    }
}
