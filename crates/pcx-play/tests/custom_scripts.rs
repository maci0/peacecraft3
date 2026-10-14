use pcx_play::custom_scripts::{
    CustomScriptBudget, CustomScriptError, CustomScriptInputs, CustomScripts,
};
use pcx_play::scripts::{AssetIdentity, ScriptAsset};
use pcx_script::{Host, NullHost, Stage, Value};
use pcx_trig::{TriggerArity, Wct, Wtg};

fn identity() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "fixture-1".into(),
        source: "memory".into(),
    }
}
fn pair(blocks: &[&[u8]]) -> (Vec<u8>, Vec<u8>) {
    let mut wct = Vec::new();
    wct.extend_from_slice(&0u32.to_le_bytes());
    wct.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
    let mut wtg = b"WTG!".to_vec();
    for word in [4, 0, 0, 0, blocks.len() as u32] {
        wtg.extend_from_slice(&word.to_le_bytes());
    }
    for (i, block) in blocks.iter().enumerate() {
        wct.extend_from_slice(&(block.len() as u32).to_le_bytes());
        wct.extend_from_slice(block);
        wtg.extend_from_slice(format!("Fixture{i}").as_bytes());
        wtg.extend_from_slice(&[0, 0]);
        for word in [1, u32::from(!block.is_empty()), 0, 0, 0, 0] {
            wtg.extend_from_slice(&word.to_le_bytes());
        }
    }
    (wct, wtg)
}
fn asset<'a>(label: &'a str, bytes: &'a [u8]) -> ScriptAsset<'a> {
    ScriptAsset { label, bytes }
}
const COMMON: &[u8] = b"native Capture takes string text returns nothing\nnative Unsupported takes nothing returns nothing\n";
const BLIZZARD: &[u8] = b"function AuthoredBJ takes nothing returns nothing\nendfunction\n";
fn compile(
    blocks: &[&[u8]],
    context: Option<&[u8]>,
    wts: Option<&[u8]>,
    budget: CustomScriptBudget,
) -> Result<CustomScripts, CustomScriptError> {
    let (wct, wtg) = pair(blocks);
    let wct = Wct::parse(&wct).unwrap();
    let wtg = Wtg::parse(&wtg, &TriggerArity::new()).unwrap();
    CustomScripts::compile(
        identity(),
        CustomScriptInputs {
            common: asset("common.j", COMMON),
            blizzard: asset("Blizzard.j", BLIZZARD),
            context: context.map(|bytes| asset("caller-context.j", bytes)),
            wct_label: "fixture.wct",
            wct: &wct,
            wtg_label: "fixture.wtg",
            wtg: &wtg,
            wts: wts.map(|bytes| asset("fixture.wts", bytes)),
        },
        budget,
    )
}

#[test]
fn custom_blocks_share_functions_globals_and_keep_empty_slots() {
    let first = b"function First takes nothing returns integer\nreturn Later()\nendfunction\0";
    let later = b"globals\ninteger counter = 5\nendglobals\nfunction Later takes nothing returns integer\nset counter = counter + 2\nreturn counter\nendfunction\0";
    let scripts = compile(
        &[first, b"", later],
        None,
        None,
        CustomScriptBudget::default(),
    )
    .unwrap();
    assert_eq!(
        scripts
            .items()
            .iter()
            .map(|item| item.index)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(scripts.items()[1].trigger, b"Fixture2");
    assert_eq!(
        scripts.source(&scripts.items()[0]).unwrap().as_bytes(),
        &first[..first.len() - 1]
    );
    assert_eq!(scripts.items()[0].digest, pcx_script::fnv1a64(first));
    assert_eq!(scripts.digests.len(), 4);
    let mut vm = scripts.vm(NullHost);
    assert_eq!(vm.call("First", &[]).unwrap(), Value::Int(7));
    assert_eq!(vm.call("First", &[]).unwrap(), Value::Int(9));
}

#[test]
fn caller_context_is_required_for_its_authored_symbols() {
    let body = b"function Run takes nothing returns integer\nreturn CallerValue\nendfunction\0";
    assert!(matches!(
        compile(&[body], None, None, CustomScriptBudget::default()),
        Err(CustomScriptError::Script {
            stage: Stage::Resolve,
            ..
        })
    ));
    let scripts = compile(
        &[body],
        Some(b"globals\ninteger CallerValue = 23\nendglobals\n"),
        None,
        CustomScriptBudget::default(),
    )
    .unwrap();
    assert_eq!(
        scripts.vm(NullHost).call("Run", &[]).unwrap(),
        Value::Int(23)
    );
}

#[test]
fn prelude_graph_errors_are_not_rendered_against_a_wct_block() {
    let (wct, wtg) = pair(&[b"function Run takes nothing returns nothing\nendfunction\0"]);
    let wct = Wct::parse(&wct).unwrap();
    let wtg = Wtg::parse(&wtg, &TriggerArity::new()).unwrap();
    for (common, blizzard, expected, message) in [
        (
            b"type Broken extends Missing\n".as_slice(),
            BLIZZARD,
            "common.j",
            "Broken",
        ),
        (
            COMMON,
            b"library Broken requires Missing\nendlibrary\n".as_slice(),
            "Blizzard.j",
            "requires `Missing`",
        ),
        (
            b"function F takes nothing returns nothing\nendfunction\n".as_slice(),
            b"function F takes integer x returns nothing\ncall Unchecked()\nendfunction\n"
                .as_slice(),
            "Blizzard.j",
            "different signatures",
        ),
    ] {
        let error = CustomScripts::compile(
            identity(),
            CustomScriptInputs {
                common: asset("common.j", common),
                blizzard: asset("Blizzard.j", blizzard),
                context: None,
                wct_label: "fixture.wct",
                wct: &wct,
                wtg_label: "fixture.wtg",
                wtg: &wtg,
                wts: None,
            },
            CustomScriptBudget::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, CustomScriptError::Script { label, stage: Stage::Resolve, reason }
                if label == expected && reason.starts_with("1:") && reason.contains(message))
        );
    }
}

#[test]
fn prelude_labels_cannot_alias_the_vm_main_source_identity() {
    let (wct, wtg) = pair(&[]);
    let wct = Wct::parse(&wct).unwrap();
    let wtg = Wtg::parse(&wtg, &TriggerArity::new()).unwrap();
    for common_is_reserved in [false, true] {
        let error = CustomScripts::compile(
            identity(),
            CustomScriptInputs {
                common: asset(
                    if common_is_reserved {
                        "the map script"
                    } else {
                        "common.j"
                    },
                    COMMON,
                ),
                blizzard: asset(
                    if common_is_reserved {
                        "Blizzard.j"
                    } else {
                        "the map script"
                    },
                    BLIZZARD,
                ),
                context: None,
                wct_label: "fixture.wct",
                wct: &wct,
                wtg_label: "fixture.wtg",
                wtg: &wtg,
                wts: None,
            },
            CustomScriptBudget::default(),
        )
        .unwrap_err();
        assert!(
            matches!(error, CustomScriptError::Identity { label } if label == "the map script")
        );
    }
}

#[test]
fn compile_and_runtime_errors_name_the_actual_later_block() {
    let first = b"function First takes nothing returns nothing\ncall Later()\nendfunction\0";
    let invalid = b"function Later takes nothing returns nothing\ncall Missing()\nendfunction\0";
    let error = compile(
        &[first, b"", invalid],
        None,
        None,
        CustomScriptBudget::default(),
    )
    .unwrap_err();
    assert!(
        matches!(&error, CustomScriptError::Script { label, stage: Stage::Resolve, reason } if label == "fixture.wct:trigger[2]" && reason.starts_with("2:"))
    );
    let later = b"function Later takes nothing returns nothing\ncall Unsupported()\nendfunction\0";
    let scripts = compile(
        &[first, b"", later],
        None,
        None,
        CustomScriptBudget::default(),
    )
    .unwrap();
    let mut vm = scripts.vm(NullHost);
    let error = vm.call("First", &[]).unwrap_err();
    let diagnostic = scripts.diagnostic(&vm, &error);
    assert!(
        diagnostic.starts_with("fixture.wct:trigger[2]:2:"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("Unsupported"));
}

#[test]
fn actual_wts_text_is_resolved_through_the_existing_host() {
    #[derive(Default)]
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
    let body =
        b"function Run takes nothing returns nothing\ncall Capture(\"TRIGSTR_8\")\nendfunction\0";
    let scripts = compile(
        &[body],
        None,
        Some(b"STRING 8\r\n{\r\n|nAuthored|cffff0000\r\n}\r\n"),
        CustomScriptBudget::default(),
    )
    .unwrap();
    let mut vm = scripts.vm(Capture::default());
    vm.call("Run", &[]).unwrap();
    assert_eq!(vm.host().inner.0, ["|nAuthored|cffff0000"]);
    assert_eq!(scripts.digests.len(), 5);
    assert!(
        matches!(compile(&[body], None, Some(b"STRING bad"), CustomScriptBudget::default()), Err(CustomScriptError::Wts { label, .. }) if label == "fixture.wts")
    );
    assert!(matches!(
        compile(
            &[body],
            None,
            Some(b"STRING 8\n{\na\n}\nSTRING 8\n{\nb\n}\n"),
            CustomScriptBudget::default()
        ),
        Err(CustomScriptError::Wts { .. })
    ));
}

#[test]
fn budgets_refuse_before_parsing_input_and_allow_exact_boundaries() {
    let (wct_bytes, wtg_bytes) = pair(&[b"@\0"]);
    let wct = Wct::parse(&wct_bytes).unwrap();
    let wtg = Wtg::parse(&wtg_bytes, &TriggerArity::new()).unwrap();
    let make = || CustomScriptInputs {
        common: asset("common.j", b"@"),
        blizzard: asset("Blizzard.j", b"@"),
        context: None,
        wct_label: "fixture.wct",
        wct: &wct,
        wtg_label: "fixture.wtg",
        wtg: &wtg,
        wts: None,
    };
    assert!(matches!(
        CustomScripts::compile(
            identity(),
            make(),
            CustomScriptBudget {
                max_prelude_bytes: 0,
                ..Default::default()
            }
        ),
        Err(CustomScriptError::Budget {
            region: "preludes",
            ..
        })
    ));
    assert!(matches!(
        CustomScripts::compile(
            identity(),
            make(),
            CustomScriptBudget {
                max_aggregate_input_bytes: 0,
                ..Default::default()
            }
        ),
        Err(CustomScriptError::Budget {
            region: "aggregate input",
            ..
        })
    ));
    let body = b"function Run takes nothing returns nothing\nendfunction\0";
    let (wct_bytes, wtg_bytes) = pair(&[body]);
    let budget = CustomScriptBudget {
        max_wct_bytes: wct_bytes.len(),
        max_wtg_bytes: wtg_bytes.len(),
        max_block_bytes: body.len(),
        max_custom_blocks: 1,
        max_prelude_bytes: COMMON.len() + BLIZZARD.len(),
        max_aggregate_input_bytes: wct_bytes.len()
            + wtg_bytes.len()
            + COMMON.len()
            + BLIZZARD.len(),
        ..Default::default()
    };
    compile(&[body], None, None, budget).unwrap();
    for changed in [
        CustomScriptBudget {
            max_wct_bytes: budget.max_wct_bytes - 1,
            ..budget
        },
        CustomScriptBudget {
            max_wtg_bytes: budget.max_wtg_bytes - 1,
            ..budget
        },
        CustomScriptBudget {
            max_block_bytes: budget.max_block_bytes - 1,
            ..budget
        },
        CustomScriptBudget {
            max_custom_blocks: 0,
            ..budget
        },
        CustomScriptBudget {
            max_aggregate_input_bytes: budget.max_aggregate_input_bytes - 1,
            ..budget
        },
    ] {
        assert!(matches!(
            compile(&[body], None, None, changed),
            Err(CustomScriptError::Budget { .. })
        ));
    }
    assert!(matches!(
        compile(
            &[body],
            Some(b"@"),
            None,
            CustomScriptBudget {
                max_context_bytes: 0,
                ..Default::default()
            }
        ),
        Err(CustomScriptError::Budget {
            region: "context",
            ..
        })
    ));
    assert!(matches!(
        compile(
            &[body],
            None,
            Some(b"@"),
            CustomScriptBudget {
                max_wts_bytes: 0,
                ..Default::default()
            }
        ),
        Err(CustomScriptError::Budget { region: "WTS", .. })
    ));
}

#[test]
fn duplicate_functions_and_globals_refuse_instead_of_overlaying() {
    for body in [
        b"function Same takes nothing returns nothing\nendfunction\0".as_slice(),
        b"globals\ninteger Same = 1\nendglobals\0".as_slice(),
    ] {
        assert!(
            matches!(compile(&[body, body], None, None, CustomScriptBudget::default()), Err(CustomScriptError::Script { label, reason, .. }) if label == "fixture.wct:trigger[1]" && reason.contains("duplicate"))
        );
    }
}

#[test]
fn framing_association_and_identity_errors_are_distinct() {
    let body = b"function Run takes nothing returns nothing\nendfunction\0";
    assert!(matches!(
        compile(
            &[&body[..body.len() - 1]],
            None,
            None,
            CustomScriptBudget::default()
        ),
        Err(CustomScriptError::Terminator { index: 0, .. })
    ));
    assert!(matches!(
        compile(&[b"a\0b\0"], None, None, CustomScriptBudget::default()),
        Err(CustomScriptError::Terminator { .. })
    ));
    let (mut wct_bytes, wtg_bytes) = pair(&[body]);
    let wtg = Wtg::parse(&wtg_bytes, &TriggerArity::new()).unwrap();
    let (empty_wct, _) = pair(&[]);
    for (bytes, expect_header) in [
        (&empty_wct[..], false),
        (
            {
                wct_bytes[..4].copy_from_slice(&1u32.to_le_bytes());
                &wct_bytes[..]
            },
            true,
        ),
    ] {
        let wct = Wct::parse(bytes).unwrap();
        let inputs = || CustomScriptInputs {
            common: asset("common.j", COMMON),
            blizzard: asset("Blizzard.j", BLIZZARD),
            context: None,
            wct_label: "fixture.wct",
            wct: &wct,
            wtg_label: "fixture.wtg",
            wtg: &wtg,
            wts: None,
        };
        let error = CustomScripts::compile(identity(), inputs(), CustomScriptBudget::default())
            .unwrap_err();
        assert!(if expect_header {
            matches!(error, CustomScriptError::Header { .. })
        } else {
            matches!(error, CustomScriptError::Association { .. })
        });
        let mut id = identity();
        id.edition = " ".into();
        assert!(matches!(
            CustomScripts::compile(id, inputs(), CustomScriptBudget::default()),
            Err(CustomScriptError::Identity { .. })
        ));
    }
}

#[test]
fn empty_wct_has_no_synthetic_trigger_and_raw_names_are_preserved() {
    let scripts = compile(&[], None, None, CustomScriptBudget::default()).unwrap();
    assert!(scripts.items().is_empty());
    assert!(scripts.program().function("Run").is_none());
    let body = b"function Run takes nothing returns nothing\nendfunction\0";
    let (wct_bytes, mut wtg_bytes) = pair(&[body]);
    // First trigger name starts at byte 24 in this synthetic v4 envelope.
    wtg_bytes[24] = 0xff;
    let wct = Wct::parse(&wct_bytes).unwrap();
    let wtg = Wtg::parse(&wtg_bytes, &TriggerArity::new()).unwrap();
    let scripts = CustomScripts::compile(
        identity(),
        CustomScriptInputs {
            common: asset("common.j", COMMON),
            blizzard: asset("Blizzard.j", BLIZZARD),
            context: None,
            wct_label: "fixture.wct",
            wct: &wct,
            wtg_label: "fixture.wtg",
            wtg: &wtg,
            wts: None,
        },
        CustomScriptBudget::default(),
    )
    .unwrap();
    assert_eq!(scripts.items()[0].trigger[0], 0xff);
}

#[test]
#[ignore = "requires explicit PEACECRAFT3_WCT_INSTALL, MAP_MEMBER, COMMON_MEMBER, BLIZZARD_MEMBER and TRIGGER_DATA_MEMBER"]
fn original_wct_and_authored_context_compile_together() {
    use std::collections::BTreeSet;
    let env = |field| {
        std::env::var(format!("PEACECRAFT3_WCT_{field}"))
            .unwrap_or_else(|_| panic!("set PEACECRAFT3_WCT_{field}"))
    };
    let install = env("INSTALL");
    let map_member = env("MAP_MEMBER");
    let common_member = env("COMMON_MEMBER");
    let blizzard_member = env("BLIZZARD_MEMBER");
    let catalog_member = env("TRIGGER_DATA_MEMBER");
    let mut archives = pcx_mpq::MpqSet::open_wc3(std::path::Path::new(&install)).unwrap();
    let common = archives.extract(&common_member).unwrap();
    let blizzard = archives.extract(&blizzard_member).unwrap();
    let catalog = archives.extract(&catalog_member).unwrap();
    let arity = TriggerArity::from_trigger_data(std::str::from_utf8(&catalog).unwrap()).unwrap();
    let wct_bytes = pcx_trig::extract_trig_file(&mut archives, &map_member, "war3map.wct").unwrap();
    let wtg_bytes = pcx_trig::extract_trig_file(&mut archives, &map_member, "war3map.wtg").unwrap();
    let map_bytes = pcx_trig::extract_trig_file(&mut archives, &map_member, "war3map.j").unwrap();
    let wct = Wct::parse(&wct_bytes).unwrap();
    let wtg = Wtg::parse(&wtg_bytes, &arity).unwrap();
    let mut custom_names = BTreeSet::new();
    for block in wct.blocks().iter().filter(|block| !block.text.is_empty()) {
        let body = std::str::from_utf8(block.text.strip_suffix(&[0]).unwrap()).unwrap();
        let parsed = pcx_script::parse(body).unwrap();
        custom_names.extend(
            parsed
                .all_functions()
                .iter()
                .map(|function| function.name.as_str().to_owned()),
        );
    }
    assert!(
        !custom_names.is_empty(),
        "select an original map with custom text"
    );
    let map_source = std::str::from_utf8(&map_bytes).unwrap();
    let map_tree = pcx_script::parse(map_source).unwrap();
    let map_names: BTreeSet<_> = map_tree
        .all_functions()
        .iter()
        .map(|function| function.name.as_str().to_owned())
        .collect();
    assert!(
        custom_names.is_subset(&map_names),
        "custom functions must also occur in the supplied compiled map"
    );
    // Preserve authored map spans, excluding the custom functions supplied by WCT.
    // This is explicit test context derivation, not GUI-trigger desugaring.
    let mut context = String::new();
    for item in &map_tree.items {
        if matches!(item, pcx_script::Item::Function(function) if custom_names.contains(function.name.as_str()))
        {
            continue;
        }
        context.push_str(item.span().text(map_source));
        context.push('\n');
    }
    let wct_label = format!("{map_member}::war3map.wct");
    let wtg_label = format!("{map_member}::war3map.wtg");
    let context_label = format!("{map_member}::war3map.j/authored-non-WCT-spans");
    let scripts = CustomScripts::compile(
        AssetIdentity {
            edition: "caller-selected local archive set".into(),
            version: "archive release unverified".into(),
            source: install,
        },
        CustomScriptInputs {
            common: asset(&common_member, &common),
            blizzard: asset(&blizzard_member, &blizzard),
            context: Some(asset(&context_label, context.as_bytes())),
            wct_label: &wct_label,
            wct: &wct,
            wtg_label: &wtg_label,
            wtg: &wtg,
            wts: None,
        },
        CustomScriptBudget::default(),
    )
    .unwrap();
    assert_eq!(
        scripts.items().len(),
        wct.blocks()
            .iter()
            .filter(|block| !block.text.is_empty())
            .count()
    );
    for name in &custom_names {
        assert!(scripts.program().function(name).is_some(), "{name}");
    }
    for item in scripts.items() {
        assert_eq!(
            item.digest,
            pcx_script::fnv1a64(&wct.blocks()[item.index].text)
        );
        assert_eq!(item.trigger, wtg.triggers()[item.index].name);
    }
    println!(
        "slots={} custom_blocks={} custom_functions={} program_functions={} context_bytes={}",
        wct.blocks().len(),
        scripts.items().len(),
        custom_names.len(),
        scripts.program().functions().len(),
        context.len()
    );
}
