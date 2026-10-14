//! Compile ordered WCT bodies into one caller-supplied JASS environment.
//! WTG association is checked, but GUI ECAs are not lowered and no trigger is run.

use crate::scripts::{AssetIdentity, LocalizedHost, ScriptAsset, SourceDigest};
use pcx_script::{Host, Prelude, Program, Script, Stage, Vm};
use pcx_trig::{Wct, Wtg, wct_wtg_violations};
use std::{alloc::Layout, collections::BTreeMap, ops::Range};

/// Input-byte policy, not native limits or a bound on compiler/VM memory.
#[derive(Debug, Clone, Copy)]
pub struct CustomScriptBudget {
    pub max_wct_bytes: usize,
    pub max_wtg_bytes: usize,
    pub max_block_bytes: usize,
    pub max_custom_blocks: usize,
    pub max_prelude_bytes: usize,
    pub max_context_bytes: usize,
    pub max_wts_bytes: usize,
    pub max_aggregate_input_bytes: usize,
}
impl Default for CustomScriptBudget {
    fn default() -> Self {
        Self {
            max_wct_bytes: 4 * 1024 * 1024,
            max_wtg_bytes: 8 * 1024 * 1024,
            max_block_bytes: 1024 * 1024,
            max_custom_blocks: 65_536,
            max_prelude_bytes: 8 * 1024 * 1024,
            max_context_bytes: 16 * 1024 * 1024,
            max_wts_bytes: 4 * 1024 * 1024,
            max_aggregate_input_bytes: 128 * 1024 * 1024,
        }
    }
}

pub struct CustomScriptInputs<'a, 'data> {
    pub common: ScriptAsset<'a>,
    pub blizzard: ScriptAsset<'a>,
    /// Authored declarations supplied by the caller, before the ordered blocks.
    /// This does not generate missing GUI globals or initialization functions.
    pub context: Option<ScriptAsset<'a>>,
    pub wct_label: &'a str,
    pub wct: &'a Wct<'data>,
    pub wtg_label: &'a str,
    pub wtg: &'a Wtg<'data>,
    pub wts: Option<ScriptAsset<'a>>,
}

#[derive(Debug, thiserror::Error)]
pub enum CustomScriptError {
    #[error("missing or duplicate input identity {label:?}")]
    Identity { label: String },
    #[error("{label}: {region} size {size} exceeds input policy {limit}")]
    Budget {
        label: String,
        region: &'static str,
        size: usize,
        limit: usize,
    },
    #[error("input size arithmetic overflow in {region}")]
    Overflow { region: &'static str },
    #[error("{region}: allocation/span layout is unsupported for {size} bytes")]
    Platform { region: &'static str, size: usize },
    #[error("{region}: allocation failed: {reason}")]
    Allocation {
        region: &'static str,
        reason: String,
    },
    #[error("{label}: unsupported WCT header word {word}")]
    Header { label: String, word: u32 },
    #[error("{wct} / {wtg}: {reason}")]
    Association {
        wct: String,
        wtg: String,
        reason: String,
    },
    #[error("{label}: text is not UTF-8 at byte {at}")]
    Utf8 { label: String, at: usize },
    #[error("{label}: block {index} is not exactly one NUL-terminated text")]
    Terminator { label: String, index: usize },
    #[error("{label}: {reason}")]
    Script {
        label: String,
        stage: Stage,
        reason: String,
    },
    #[error("{label}: {reason}")]
    Wts { label: String, reason: String },
}

#[derive(Debug)]
pub struct CustomScript {
    /// Slot in the WCT/WTG pair; empty GUI slots do not renumber later blocks.
    pub index: usize,
    pub trigger: Vec<u8>,
    pub label: String,
    /// Fingerprint of the complete stored text, including its final NUL.
    pub digest: u64,
    pub range: Range<usize>,
}

#[derive(Debug)]
pub struct CustomScripts {
    pub identity: AssetIdentity,
    items: Vec<CustomScript>,
    pub digests: Vec<SourceDigest>,
    program: Program,
    source: String,
    context: Option<(String, Range<usize>)>,
    prelude_sources: BTreeMap<String, String>,
    wct_label: String,
    wts: Option<pcx_trig::wts::Table>,
    wts_label: Option<String>,
}

fn sum(a: usize, b: usize, region: &'static str) -> Result<usize, CustomScriptError> {
    a.checked_add(b)
        .ok_or(CustomScriptError::Overflow { region })
}
fn limit(
    label: &str,
    region: &'static str,
    size: usize,
    cap: usize,
) -> Result<(), CustomScriptError> {
    if size > cap {
        return Err(CustomScriptError::Budget {
            label: label.into(),
            region,
            size,
            limit: cap,
        });
    }
    Ok(())
}
fn text<'a>(label: &str, bytes: &'a [u8]) -> Result<&'a str, CustomScriptError> {
    std::str::from_utf8(bytes).map_err(|error| CustomScriptError::Utf8 {
        label: label.into(),
        at: error.valid_up_to(),
    })
}
fn parse(label: &str, source: &str) -> Result<Script, CustomScriptError> {
    pcx_script::parse(source).map_err(|error| CustomScriptError::Script {
        label: label.into(),
        stage: error.stage,
        reason: error.render(source),
    })
}

impl CustomScripts {
    pub fn compile(
        identity: AssetIdentity,
        input: CustomScriptInputs<'_, '_>,
        budget: CustomScriptBudget,
    ) -> Result<Self, CustomScriptError> {
        for (label, value) in [
            ("edition", &identity.edition),
            ("version", &identity.version),
            ("source", &identity.source),
        ] {
            if value.trim().is_empty() {
                return Err(CustomScriptError::Identity {
                    label: label.into(),
                });
            }
        }
        let mut labels = vec![
            input.common.label,
            input.blizzard.label,
            input.wct_label,
            input.wtg_label,
        ];
        labels.extend(input.context.map(|asset| asset.label));
        labels.extend(input.wts.map(|asset| asset.label));
        for asset in [input.common, input.blizzard] {
            if asset.label == "the map script" {
                return Err(CustomScriptError::Identity {
                    label: asset.label.into(),
                });
            }
        }
        for (i, label) in labels.iter().enumerate() {
            if label.trim().is_empty() || labels[..i].contains(label) {
                return Err(CustomScriptError::Identity {
                    label: (*label).into(),
                });
            }
        }
        limit(
            input.wct_label,
            "WCT",
            input.wct.raw().len(),
            budget.max_wct_bytes,
        )?;
        limit(
            input.wtg_label,
            "WTG",
            input.wtg.raw().len(),
            budget.max_wtg_bytes,
        )?;
        limit(
            input.wct_label,
            "slots",
            input.wct.blocks().len(),
            budget.max_custom_blocks,
        )?;
        let prelude_bytes = sum(
            input.common.bytes.len(),
            input.blizzard.bytes.len(),
            "preludes",
        )?;
        limit(
            input.common.label,
            "preludes",
            prelude_bytes,
            budget.max_prelude_bytes,
        )?;
        let mut aggregate = sum(input.wct.raw().len(), input.wtg.raw().len(), "containers")?;
        aggregate = sum(aggregate, prelude_bytes, "aggregate input")?;
        let mut joined_size = 0;
        if let Some(context) = input.context {
            limit(
                context.label,
                "context",
                context.bytes.len(),
                budget.max_context_bytes,
            )?;
            aggregate = sum(aggregate, context.bytes.len(), "aggregate input")?;
            joined_size = sum(context.bytes.len(), 1, "joined source")?;
        }
        if let Some(wts) = input.wts {
            limit(wts.label, "WTS", wts.bytes.len(), budget.max_wts_bytes)?;
            aggregate = sum(aggregate, wts.bytes.len(), "aggregate input")?;
        }
        for block in input.wct.blocks() {
            limit(
                input.wct_label,
                "block",
                block.text.len(),
                budget.max_block_bytes,
            )?;
            // Each stored NUL is replaced with a separating newline after validation.
            joined_size = sum(joined_size, block.text.len(), "joined source")?;
        }
        limit(
            input.wct_label,
            "aggregate input",
            aggregate,
            budget.max_aggregate_input_bytes,
        )?;
        if joined_size > u32::MAX as usize || Layout::array::<u8>(joined_size).is_err() {
            return Err(CustomScriptError::Platform {
                region: "joined source",
                size: joined_size,
            });
        }
        if input.wct.unknown0() != 0 {
            return Err(CustomScriptError::Header {
                label: input.wct_label.into(),
                word: input.wct.unknown0(),
            });
        }
        let violations = wct_wtg_violations(input.wct, input.wtg);
        if !violations.is_empty() {
            return Err(CustomScriptError::Association {
                wct: input.wct_label.into(),
                wtg: input.wtg_label.into(),
                reason: violations.join("; "),
            });
        }
        for asset in [input.common, input.blizzard] {
            if asset.bytes.is_empty() {
                return Err(CustomScriptError::Identity {
                    label: asset.label.into(),
                });
            }
            limit(
                asset.label,
                "script spans",
                asset.bytes.len(),
                u32::MAX as usize,
            )?;
        }
        let common_source = text(input.common.label, input.common.bytes)?;
        let blizzard_source = text(input.blizzard.label, input.blizzard.bytes)?;
        let common = parse(input.common.label, common_source)?;
        let blizzard = parse(input.blizzard.label, blizzard_source)?;
        let mut source = String::new();
        source
            .try_reserve_exact(joined_size)
            .map_err(|error| CustomScriptError::Allocation {
                region: "joined source",
                reason: error.to_string(),
            })?;
        let context = input
            .context
            .map(|asset| {
                let body = text(asset.label, asset.bytes)?;
                parse(asset.label, body)?;
                let range = 0..body.len();
                source.push_str(body);
                source.push('\n');
                Ok((asset.label.to_owned(), range))
            })
            .transpose()?;
        let mut items = Vec::new();
        items
            .try_reserve_exact(input.wct.blocks().len())
            .map_err(|error| CustomScriptError::Allocation {
                region: "block metadata",
                reason: error.to_string(),
            })?;
        for (index, (block, trigger)) in input
            .wct
            .blocks()
            .iter()
            .zip(input.wtg.triggers())
            .enumerate()
        {
            if block.text.is_empty() {
                continue;
            }
            let label = format!("{}:trigger[{index}]", input.wct_label);
            let body = block
                .text
                .strip_suffix(&[0])
                .filter(|body| !body.contains(&0))
                .ok_or_else(|| CustomScriptError::Terminator {
                    label: input.wct_label.into(),
                    index,
                })?;
            let body = text(&label, body)?;
            parse(&label, body)?;
            let start = source.len();
            source.push_str(body);
            let range = start..source.len();
            source.push('\n');
            items.push(CustomScript {
                index,
                trigger: trigger.name.clone(),
                label,
                digest: pcx_script::fnv1a64(&block.text),
                range,
            });
        }
        let tree = pcx_script::parse(&source).map_err(|error| {
            source_error(&source, context.as_ref(), &items, input.wct_label, error)
        })?;
        // A full compiled map passed as context would duplicate the WCT bodies.
        // Refuse collisions rather than selecting one declaration implicitly.
        check_declarations(&tree).map_err(|error| {
            source_error(&source, context.as_ref(), &items, input.wct_label, error)
        })?;
        let program = pcx_script::resolve_with_diagnostics(
            &tree,
            &[
                Prelude::new(input.common.label, &common),
                Prelude::new(input.blizzard.label, &blizzard),
            ],
        )
        .map_err(|diagnostic| match diagnostic.source_label {
            Some(label) => {
                let original = if label == input.common.label {
                    common_source
                } else {
                    blizzard_source
                };
                CustomScriptError::Script {
                    label,
                    stage: diagnostic.error.stage,
                    reason: diagnostic.error.render(original),
                }
            }
            None => source_error(
                &source,
                context.as_ref(),
                &items,
                input.wct_label,
                diagnostic.error,
            ),
        })?;
        let wts = input
            .wts
            .map(|asset| {
                let table = pcx_trig::wts::Table::parse(asset.bytes).map_err(|error| {
                    CustomScriptError::Wts {
                        label: asset.label.into(),
                        reason: error.to_string(),
                    }
                })?;
                if !table.ids_are_unique() {
                    return Err(CustomScriptError::Wts {
                        label: asset.label.into(),
                        reason: "duplicate WTS ids".into(),
                    });
                }
                Ok(table)
            })
            .transpose()?;
        let digests = [
            Some(input.common),
            Some(input.blizzard),
            input.context,
            Some(ScriptAsset {
                label: input.wct_label,
                bytes: input.wct.raw(),
            }),
            Some(ScriptAsset {
                label: input.wtg_label,
                bytes: input.wtg.raw(),
            }),
            input.wts,
        ]
        .into_iter()
        .flatten()
        .map(|asset| SourceDigest {
            label: asset.label.into(),
            hash: pcx_script::fnv1a64(asset.bytes),
        })
        .collect();
        Ok(Self {
            identity,
            items,
            digests,
            program,
            source,
            context,
            prelude_sources: BTreeMap::from([
                (input.common.label.into(), common_source.into()),
                (input.blizzard.label.into(), blizzard_source.into()),
            ]),
            wct_label: input.wct_label.into(),
            wts,
            wts_label: input.wts.map(|asset| asset.label.into()),
        })
    }
    pub fn program(&self) -> &Program {
        &self.program
    }
    pub fn items(&self) -> &[CustomScript] {
        &self.items
    }
    pub fn source(&self, item: &CustomScript) -> Option<&str> {
        self.source.get(item.range.clone())
    }
    pub fn vm<H: Host>(&self, host: H) -> Vm<'_, LocalizedHost<'_, H>> {
        Vm::new(
            &self.program,
            LocalizedHost::new(host, self.wts.as_ref(), self.wts_label.as_deref()),
        )
    }
    pub fn diagnostic<H: Host>(&self, vm: &Vm<'_, H>, error: &pcx_script::Error) -> String {
        match vm.error_source() {
            None | Some("the map script") => {
                let error = source_error(
                    &self.source,
                    self.context.as_ref(),
                    &self.items,
                    &self.wct_label,
                    error.clone(),
                );
                match error {
                    CustomScriptError::Script { label, reason, .. } => format!("{label}:{reason}"),
                    _ => error.to_string(),
                }
            }
            Some(label) => match self.prelude_sources.get(label) {
                Some(source) => format!("{label}:{}", error.render(source)),
                None => format!("{label}:{error}"),
            },
        }
    }
}

fn check_declarations(tree: &Script) -> Result<(), pcx_script::Error> {
    let mut declarations = BTreeMap::new();
    let mut add = |namespace, name: &pcx_script::Ident, span| {
        if declarations
            .insert((namespace, name.as_str().to_owned()), span)
            .is_some()
        {
            return Err(pcx_script::Error {
                stage: Stage::Resolve,
                span,
                message: format!(
                    "duplicate {namespace} declaration `{}` in custom-script inputs",
                    name.as_str()
                ),
            });
        }
        Ok(())
    };
    for global in tree.globals() {
        add("global", &global.name, global.span)?;
    }
    for function in tree.all_functions() {
        add("function", &function.name, function.span)?;
    }
    for native in tree.natives() {
        add("function", &native.name, native.span)?;
    }
    for item in &tree.items {
        if let pcx_script::Item::Type(definition) = item {
            add("type", &definition.name, definition.span)?;
        }
    }
    Ok(())
}

fn source_error(
    source: &str,
    context: Option<&(String, Range<usize>)>,
    items: &[CustomScript],
    fallback: &str,
    mut error: pcx_script::Error,
) -> CustomScriptError {
    let at = error.span.start as usize;
    let location = context
        .map(|(label, range)| (label.as_str(), range))
        .into_iter()
        .chain(items.iter().map(|item| (item.label.as_str(), &item.range)))
        .find(|(_, range)| range.start <= at && at <= range.end);
    let (label, body) = match location {
        Some((label, range)) => {
            error.span.start = error.span.start.saturating_sub(range.start as u32);
            error.span.end = error
                .span
                .end
                .saturating_sub(range.start as u32)
                .min(range.len() as u32);
            (label, &source[range.clone()])
        }
        None => (fallback, source),
    };
    CustomScriptError::Script {
        label: label.into(),
        stage: error.stage,
        reason: error.render(body),
    }
}
