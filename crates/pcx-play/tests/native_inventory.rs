//! Local-original inventory, not a static native list or a support-percentage claim.
use pcx_script::{ast::Script, natives::NativeTable};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Write};

fn inventory(sources: &[(&str, &str)]) -> Result<Value, String> {
    let trees: Vec<Script> = sources
        .iter()
        .map(|(label, text)| {
            pcx_script::parse_script(text).map_err(|e| format!("{label}: {}", e.render(text)))
        })
        .collect::<Result<_, _>>()?;
    let mut table = NativeTable::new();
    let mut names = BTreeMap::new();
    for ((label, _), tree) in sources.iter().zip(&trees) {
        for native in tree.natives() {
            table.insert(native, label);
            names.insert(native.name.as_str(), ());
        }
    }
    let mut declarations = Vec::new();
    for name in names.keys() {
        if table.ambiguous(name).is_some() {
            return Err(format!("ambiguous native {name}"));
        }
        let native = table.get(name).expect("inserted declaration");
        declarations.push(json!({
            "name": name,
            "signature": pcx_script::natives::signature_of(name, &native.params, &native.ret),
            "constant": native.is_constant,
            "source": native.source,
            "span": [native.span.start, native.span.end],
            "also_declared_in": native.also_declared_in.iter().map(|(source, span)| json!({"source": source, "span": [span.start, span.end]})).collect::<Vec<_>>(),
            "vm_intrinsic": matches!(table.call(name), Some(pcx_script::natives::NativeCall::Implemented(..))),
            "host_behavior": "unreviewed",
            "original_behavior": "unreviewed",
            "presentation": "unreviewed"
        }));
    }
    let source_records: Vec<_> = sources
        .iter()
        .zip(&trees)
        .map(|((label, text), tree)| {
            json!({
                "source": label,
                "sha256": Sha256::digest(text.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
                "bytes": text.len(),
                "native_declarations": tree.natives().count(),
                "library_functions": tree.functions().count()
            })
        })
        .collect();
    Ok(
        json!({"sources": source_records, "declarations": declarations,
        "note": "Exhaustive declaration inventory only. VM intrinsics and source mentions do not prove complete host behavior."}),
    )
}

#[test]
fn inventory_includes_unused_natives_and_refuses_conflicting_signatures() {
    let source = "native Used takes integer x returns nothing\nnative NeverCalled takes real x returns boolean\nfunction Helper takes nothing returns nothing\nendfunction\n";
    let report = inventory(&[("synthetic:common", source)]).unwrap();
    let natives = report["declarations"].as_array().unwrap();
    assert_eq!(natives.len(), 2);
    assert_eq!(natives[0]["name"], "NeverCalled");
    assert_eq!(natives[0]["host_behavior"], "unreviewed");
    assert_eq!(report["sources"][0]["library_functions"], 1);
    assert!(
        inventory(&[
            ("a", source),
            ("b", "native Used takes real x returns nothing\n")
        ])
        .is_err()
    );
    assert!(inventory(&[("broken", "native NotFinished")]).is_err());
}

#[test]
#[ignore = "requires PCX_SCRIPT_COMMON_J, PCX_SCRIPT_COMMON_AI, PCX_SCRIPT_BLIZZARD_J, PCX_SCRIPT_EDITION, PCX_SCRIPT_VERSION and PCX_SCRIPT_INVENTORY_OUTPUT"]
fn selected_original_full_api_inventory() {
    let edition = std::env::var("PCX_SCRIPT_EDITION").unwrap();
    let version = std::env::var("PCX_SCRIPT_VERSION").unwrap();
    assert!(!edition.trim().is_empty() && !version.trim().is_empty());
    let mut sources = Vec::new();
    for key in [
        "PCX_SCRIPT_COMMON_J",
        "PCX_SCRIPT_COMMON_AI",
        "PCX_SCRIPT_BLIZZARD_J",
    ] {
        let path = std::env::var(key).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() <= 8 << 20, "script input budget");
        sources.push((path, String::from_utf8(bytes).unwrap()));
    }
    let mut report = inventory(
        &sources
            .iter()
            .map(|(label, text)| (label.as_str(), text.as_str()))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    report["edition"] = json!(edition);
    report["version"] = json!(version);
    let output = std::env::var("PCX_SCRIPT_INVENTORY_OUTPUT").unwrap();
    let count = report["declarations"].as_array().unwrap().len();
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap()
        .write_all(&serde_json::to_vec_pretty(&report).unwrap())
        .unwrap();
    println!(
        "inventoried {count} distinct native declarations; behavior coverage remains separately reviewed"
    );
}
