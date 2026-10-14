//! Adapter for compiling authored WC3 WCT custom-text trigger bodies.
//!
//! Each non-empty WCT block is JASS in trigger order. This module compiles those
//! bodies against caller-supplied preludes and preserves their WCT/WTG association
//! for diagnostics. It does not synthesize JASS prelude text or lower WTG ECAs.

use pcx_play::scripts::{AssetIdentity, ScriptAsset};
use pcx_script::{parse as parse_script, resolve_with, Host, Prelude, Program};
use pcx_trig::{wct_wtg_violations, Wct, Wtg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomScriptBudget {
    pub max_wct_bytes: usize,
    pub max_block_bytes: usize,
    pub max_custom_blocks: usize,
    pub max_prelude_bytes: usize,
    pub max_aggregate_input_bytes: usize,
}

impl CustomScriptBudget {
    pub const DEFAULT: Self = Self {
        max_wct_bytes: 4 * 1024 * 1024,
        max_block_bytes: 1024 * 1024,
        max_custom_blocks: 65_536,
        max_prelude_bytes: 8 * 1024 * 1024,
        max_aggregate_input_bytes: 128 * 1024 * 1024,
    };
}

impl Default for CustomScriptBudget {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CustomScriptError {
    #[error("asset identity field {field:?} must not be empty")]
    InvalidIdentity { field: &'static str },
    #[error("input {region:?} exceeds size budget: declared {declared_bytes} bytes, limit is {max_bytes} bytes")]
    BudgetExceeded {
        region: &'static str,
        declared_bytes: usize,
        max_bytes: usize,
    },
    #[error("authored WCT/WTG association failed: {reason}")]
    Association { reason: String },
    #[error("WCT block {index} for trigger {trigger:?} is not UTF-8 at byte {at}")]
    InvalidUtf8 {
        index: usize,
        trigger: String,
        at: usize,
    },
    #[error("WCT block {index} for trigger {trigger:?} parse failed: {reason}")]
    ParseFailed {
        index: usize,
        trigger: String,
        reason: String,
    },
    #[error("trigger {trigger:?} WCT block {index} compilation failed: {reason}")]
    CompileFailed {
        index: usize,
        trigger: String,
        reason: String,
    },
    #[error("aggregate input bytes {declared_bytes} exceed budget {max_bytes}")]
    AggregateExceeded {
        declared_bytes: usize,
        max_bytes: usize,
    },
    #[error("{}", .0)]
    Prelude(String),
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct CustomScript {
    pub index: usize,
    pub trigger: String,
    pub label: String,
    pub source: String,
    pub digest: u64,
    pub program: Program,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct CustomScripts {
    pub identity: AssetIdentity,
    pub items: Vec<CustomScript>,
}

impl CustomScripts {
    #[allow(clippy::too_many_arguments)]
    pub fn compile<'a>(
        identity: AssetIdentity,
        common: ScriptAsset<'_>,
        blizzard: ScriptAsset<'_>,
        map: Option<ScriptAsset<'_>>,
        wct: &Wct<'a>,
        wtg: &Wtg<'a>,
        wts: Option<ScriptAsset<'_>>,
        budget: CustomScriptBudget,
    ) -> Result<Self, CustomScriptError> {
        if identity.edition.is_empty() {
            return Err(CustomScriptError::InvalidIdentity { field: "edition" });
        }
        if identity.version.is_empty() {
            return Err(CustomScriptError::InvalidIdentity { field: "version" });
        }
        if identity.source.is_empty() {
            return Err(CustomScriptError::InvalidIdentity { field: "source" });
        }

        let mut input_bytes = 0usize;
        let wct_bytes = wct.canonical_bytes().len();
        if wct_bytes > budget.max_wct_bytes {
            return Err(CustomScriptError::BudgetExceeded {
                region: "wct",
                declared_bytes: wct_bytes,
                max_bytes: budget.max_wct_bytes,
            });
        }
        input_bytes = input_bytes.saturating_add(wct_bytes);

        let violations = wct_wtg_violations(wct, wtg);
        if !violations.is_empty() {
            return Err(CustomScriptError::Association {
                reason: violations.join("; "),
            });
        }
        if wct.blocks().len() > budget.max_custom_blocks {
            return Err(CustomScriptError::BudgetExceeded {
                region: "custom blocks",
                declared_bytes: wct.blocks().len(),
                max_bytes: budget.max_custom_blocks,
            });
        }

        let preludes: &[(&str, &[u8])] = &[];
        let _ = preludes;
        let common_source = std::str::from_utf8(common.bytes).map_err(|e| {
            CustomScriptError::Prelude(format!(
                "{}: not UTF-8 at byte {}",
                common.label,
                e.valid_up_to()
            ))
        })?;
        let blizzard_source = std::str::from_utf8(blizzard.bytes).map_err(|e| {
            CustomScriptError::Prelude(format!(
                "{}: not UTF-8 at byte {}",
                blizzard.label,
                e.valid_up_to()
            ))
        })?;
        let common_tree = pcx_script::parse(common_source).map_err(|e| {
            CustomScriptError::Prelude(format!("{}:{}", common.label, e.render(common_source)))
        })?;
        let blizzard_tree = pcx_script::parse(blizzard_source).map_err(|e| {
            CustomScriptError::Prelude(format!("{}:{}", blizzard.label, e.render(blizzard_source)))
        })?;

        let common_len = common.bytes.len();
        let blizzard_len = blizzard.bytes.len();
        if common_len > budget.max_prelude_bytes.saturating_sub(blizzard_len) {
            return Err(CustomScriptError::BudgetExceeded {
                region: "prelude",
                declared_bytes: common_len + blizzard_len,
                max_bytes: budget.max_prelude_bytes,
            });
        }
        input_bytes = input_bytes.saturating_add(common_len + blizzard_len);

        let map_tree = match map {
            Some(asset) => {
                let source = std::str::from_utf8(asset.bytes).map_err(|e| {
                    CustomScriptError::Prelude(format!(
                        "{}: not UTF-8 at byte {}",
                        asset.label,
                        e.valid_up_to()
                    ))
                })?;
                let tree = parse_script(source).map_err(|e| {
                    CustomScriptError::Prelude(format!("{}:{}", asset.label, e.render(source)))
                })?;
                input_bytes = input_bytes.saturating_add(asset.bytes.len());
                Some((asset.label.to_string(), tree))
            }
            None => None,
        };

        let wts_table = wts
            .map(|asset| {
                input_bytes = input_bytes.saturating_add(asset.bytes.len());
                let table = pcx_trig::wts::Table::parse(asset.bytes)
                    .map_err(|e| CustomScriptError::Prelude(format!("{}: {e}", asset.label)))?;
                if !table.ids_are_unique() {
                    return Err(CustomScriptError::Prelude(format!(
                        "{}: duplicate WTS ids",
                        asset.label
                    )));
                }
                Ok(table)
            })
            .transpose()?;

        let prelude_list_base = {
            let mut v = vec![
                Prelude::new(common.label, &common_tree),
                Prelude::new(blizzard.label, &blizzard_tree),
            ];
            if let Some((label, tree)) = &map_tree {
                v.push(Prelude::new(label, tree));
            }
            v
        };
        let wct_label = "war3map.wct";

        if input_bytes > budget.max_aggregate_input_bytes {
            return Err(CustomScriptError::AggregateExceeded {
                declared_bytes: input_bytes,
                max_bytes: budget.max_aggregate_input_bytes,
            });
        }

        let mut items = Vec::new();
        for (index, block) in wct.blocks().iter().enumerate() {
            if block.text.is_empty() {
                continue;
            }
            let trigger = String::from_utf8_lossy(&wtg.triggers()[index].name).to_string();
            if block.text.len() > budget.max_block_bytes {
                return Err(CustomScriptError::BudgetExceeded {
                    region: "wct block",
                    declared_bytes: block.text.len(),
                    max_bytes: budget.max_block_bytes,
                });
            }

            let block_len = block.text.len();
            let source_text = if block.text.last() == Some(&0) {
                &block.text[..block_len - 1]
            } else {
                &block.text[..]
            };
            let source =
                std::str::from_utf8(source_text).map_err(|e| CustomScriptError::InvalidUtf8 {
                    index,
                    trigger: trigger.clone(),
                    at: e.valid_up_to(),
                })?;

            let tree = parse_script(source).map_err(|e| CustomScriptError::ParseFailed {
                index,
                trigger: trigger.clone(),
                reason: format!(
                    "{}:{}",
                    label_for(wct_label, index, &trigger),
                    e.render(source)
                ),
            })?;

            let label = label_for(wct_label, index, &trigger);
            let mut prelude_list = prelude_list_base.clone();
            // prelude lifetime requires re-borrow; build dynamically instead
            let _ = &mut prelude_list;
            let program = resolve_with(&tree, &prelude_list_base).map_err(|e| {
                CustomScriptError::CompileFailed {
                    index,
                    trigger: trigger.clone(),
                    reason: format!("{}:{}", label, e.render(source)),
                }
            })?;

            items.push(CustomScript {
                index,
                trigger,
                label,
                source: source.to_string(),
                digest: pcx_script::fnv1a64(&block.text),
                program,
            });
        }
        let _ = common_tree;
        let _ = wts_table;

        Ok(CustomScripts { identity, items })
    }

    #[allow(dead_code)]
    pub fn diagnostic<H: Host>(
        &self,
        item: usize,
        vm: &pcx_script::Vm<'_, H>,
        error: &pcx_script::Error,
    ) -> String {
        match self.items.get(item) {
            Some(s) => match vm.error_source() {
                Some(src) if src != s.label && src != "the map script" => format!("{src}:{error}"),
                _ => format!("{}:{}", s.label, error.render(&s.source)),
            },
            None => format!("custom script {item}: {error}"),
        }
    }
}

fn label_for(wct_label: &str, index: usize, trigger: &str) -> String {
    format!("{wct_label}:trigger[{index}]:{trigger}")
}
