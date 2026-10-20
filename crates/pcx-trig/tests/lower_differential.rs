//! Explicit original-asset differential: compares each lowered trigger's
//! generated functions with the `war3map.j` the World Editor wrote for the same
//! shipped map. Whitespace and wrapper-function numbering are normalized. Known
//! 2002-to-1.29 drift is counted separately and never folded into exact matches.
//! Opt-in and read-only.

use pcx_trig::catalog::Catalog;
use pcx_trig::lower::lower;
use pcx_trig::{Wct, Wtg, extract_trig_file, legacy_dir, map_trig_files, shipped_maps};
use std::collections::BTreeMap;

const SAMPLES_PER_MAP: usize = 3;

fn env(name: &str) -> String {
    std::env::var(format!("PEACECRAFT3_TRIGGER_{name}"))
        .unwrap_or_else(|_| panic!("set PEACECRAFT3_TRIGGER_{name}"))
}

/// Function name to its body lines, from JASS text.
fn functions(jass: &str) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let mut current: Option<(String, Vec<String>)> = None;
    // The editor writes bare-CR line endings in some maps, so CR splits lines too.
    for line in jass.split(['\n', '\r']) {
        let trimmed = line.trim();
        // The 2002 editor sometimes writes `endfunction` followed by a comment on the same line.
        if trimmed.starts_with("endfunction") {
            if let Some((name, body)) = current.take() {
                out.insert(name, body);
            }
        } else if let Some((_, body)) = current.as_mut() {
            body.push(trimmed.to_string());
        } else if let Some(rest) = trimmed.strip_prefix("function ") {
            current = Some((
                rest.split(' ').next().unwrap_or_default().to_string(),
                Vec::new(),
            ));
        }
    }
    out
}

/// A comment statement: a `//` line, or `call CommentString` (a no-op stub in Blizzard.j).
/// The comparison leaves both forms out, because the editor writes either for the same text.
fn is_comment_statement(line: &str) -> bool {
    line.starts_with("//") || line.starts_with("call CommentString(")
}

/// Whitespace removed, and every wrapper suffix `_FuncNNN[A-Z]?` replaced by `_FUNC`.
fn normalize(body: &[String]) -> String {
    let joined: String = body
        .iter()
        .filter(|l| !is_comment_statement(l))
        .flat_map(|l| l.chars())
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut out = String::new();
    let mut rest = joined.as_str();
    while let Some(at) = rest.find("_Func") {
        out.push_str(&rest[..at]);
        out.push_str("_FUNC");
        let after = &rest[at + "_Func".len()..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        let letter = after[digits..]
            .bytes()
            .next()
            .filter(u8::is_ascii_uppercase)
            .map_or(0, |_| 1);
        rest = &after[digits + letter..];
    }
    out.push_str(rest);
    out
}

/// Known drift, derived from the file: a TriggerData `ScriptName` alias (a BJ
/// wrapper the 1.29 editor writes) appears under the plain name in the 2002
/// script, and the texture path is `.blp` in TriggerData but `.tga` in the
/// 2002 script.
fn drift_normalized(text: &str, catalog: &Catalog) -> String {
    let mut out = text.replace(".blp\"", ".tga\"");
    for definition in catalog.definitions() {
        if let Some(alias) = &definition.script_name {
            out = out.replace(
                &format!("call{alias}("),
                &format!("call{}(", definition.name),
            );
            out = out.replace(&format!("({alias}("), &format!("({}(", definition.name));
        }
    }
    out
}

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR (or the default local 1.29b tree) and PEACECRAFT3_TRIGGER_EDITION/VERSION"]
fn lowered_trigger_functions_match_the_editor_output_where_they_can() {
    let dir = legacy_dir().expect("set PEACECRAFT3_LEGACY_DIR to a 1.29b install");
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let data = set.extract("UI\\TriggerData.txt").unwrap();
    let catalog = Catalog::parse(
        "UI\\TriggerData.txt",
        &env("EDITION"),
        &env("VERSION"),
        &String::from_utf8_lossy(&data),
    )
    .unwrap();
    let arity = catalog.arity().unwrap();
    let (mut compared, mut exact, mut drift) = (0usize, 0usize, 0usize);
    for map in shipped_maps(&mut set).unwrap() {
        let files = map_trig_files(&mut set, &map).unwrap();
        let Some(wtg_name) = files.iter().find(|f| f.eq_ignore_ascii_case("war3map.wtg")) else {
            continue;
        };
        let bytes = extract_trig_file(&mut set, &map, wtg_name).unwrap();
        let parsed = Wtg::parse(&bytes, &arity).unwrap();
        let wct_bytes = extract_trig_file(&mut set, &map, "war3map.wct").unwrap();
        let wct = Wct::parse(&wct_bytes).unwrap();
        let Ok(ours_text) = lower(&parsed, &wct, &catalog) else {
            continue;
        };
        let editor_bytes = extract_trig_file(&mut set, &map, "war3map.j").unwrap();
        let editor_text = String::from_utf8_lossy(&editor_bytes).into_owned();
        let ours = functions(&ours_text);
        let editor = functions(&editor_text);
        let (mut map_compared, mut map_exact, mut map_drift, mut samples) =
            (0usize, 0usize, 0usize, 0usize);
        let generated = ours.keys().filter(|n| {
            (n.starts_with("Trig_") && (n.ends_with("_Actions") || n.ends_with("_Conditions")))
                || n.starts_with("InitTrig_")
                || matches!(
                    n.as_str(),
                    "InitGlobals" | "InitCustomTriggers" | "RunInitializationTriggers"
                )
        });
        for name in generated {
            let Some(editor_body) = editor.get(name) else {
                println!("{map}: {name}: absent from editor output");
                continue;
            };
            map_compared += 1;
            let ours_norm = normalize(&ours[name]);
            let editor_norm = normalize(editor_body);
            if ours_norm == editor_norm {
                map_exact += 1;
            } else if drift_normalized(&ours_norm, &catalog)
                == drift_normalized(&editor_norm, &catalog)
            {
                map_drift += 1;
            } else if samples < SAMPLES_PER_MAP {
                samples += 1;
                println!("{map}: {name} differs\n  ours:   {ours_norm}\n  editor: {editor_norm}");
            }
        }
        compared += map_compared;
        exact += map_exact;
        drift += map_drift;
        println!("{map}: {map_exact} exact, {map_drift} drift, {map_compared} compared");
    }
    println!(
        "differential: {exact} exact, {drift} drift-only, {} residual of {compared} action bodies",
        compared - exact - drift
    );
}
