use pcx_play::custom_scripts::{
    CustomScriptBudget, CustomScriptError, CustomScriptInputs, CustomScripts,
};
use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{NullHost, Value};
use pcx_trig::wts::{BOM, Error, Table, Unsupported};
use pcx_trig::{TriggerArity, Wct, Wtg};
fn record(id: &str, body: &[u8], comment: Option<&[u8]>, blank: bool) -> Vec<u8> {
    let mut out = format!("STRING {id}\r\n").into_bytes();
    if let Some(comment) = comment {
        out.extend(comment);
    }
    out.extend(b"{\r\n");
    out.extend(body);
    out.extend(b"\r\n}\r\n");
    if blank {
        out.extend(b"\r\n");
    }
    out
}
fn identity() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "fixture-1".into(),
        source: "memory".into(),
    }
}
fn asset<'a>(label: &'a str, bytes: &'a [u8]) -> ScriptAsset<'a> {
    ScriptAsset { label, bytes }
}
fn map(wts: Option<&[u8]>) -> Result<MapScripts, String> {
    MapScripts::compile(
        identity(),
        asset("common.fixture", b"// supplied prelude\n"),
        asset("bj.fixture", b"// supplied prelude\n"),
        asset(
            "map.fixture",
            b"function Fetch takes nothing returns string\nreturn \"TRIGSTR_7\"\nendfunction\n",
        ),
        wts.map(|b| asset("authored.wts", b)),
    )
}
#[test]
fn late_record_failure_keeps_actual_offset_and_kind() {
    let mut bytes = record("1", b"valid", None, true);
    let at = bytes.len();
    bytes.extend(b"STRING 2\r\n{\r\nno closing line");
    assert!(
        matches!(Table::parse(&bytes),Err(Error::Unsupported {at:got,detail:Unsupported::BodyNotClosed}) if got==at)
    );
    let error = map(Some(&bytes)).err().unwrap();
    assert!(
        error.starts_with("authored.wts: unsupported wts construct"),
        "{error}"
    );
    assert!(error.contains(&format!("0x{at:x}")), "{error}");
}
#[test]
fn raw_comments_bom_order_nonutf8_and_control_codes_survive() {
    let mut bytes = BOM.to_vec();
    bytes.extend(record(
        "4294967295",
        b"\xff|n{brace}\r\nline",
        Some(b"// supplied\r\n// annotation\r\n"),
        true,
    ));
    bytes.extend(record("3", b"\"quoted\"|c12345678|r", None, false));
    let table = Table::parse(&bytes).unwrap();
    assert_eq!(table.raw(), bytes);
    assert_eq!(table.canonical_bytes(), bytes);
    assert_eq!(table.ids().collect::<Vec<_>>(), [u32::MAX, 3]);
    assert!(table.ids_are_unique());
    assert_eq!(
        table.body(u32::MAX),
        Some(b"\xff|n{brace}\r\nline".as_slice())
    );
    assert_eq!(
        table.get(u32::MAX).unwrap().comment,
        Some(b"// supplied\r\n// annotation\r\n".as_slice())
    );
    assert!(matches!(
        table.decode_utf8(u32::MAX),
        Err(Error::NotUtf8 {
            id: u32::MAX,
            at: 0,
            byte: 255
        })
    ));
    assert_eq!(table.decode_utf8(3).unwrap(), "\"quoted\"|c12345678|r");
    assert_eq!(table.framing().sum_of_parts(), bytes.len());
}
#[test]
fn every_truncated_prefix_and_empty_forms_keep_existing_refusal() {
    let full = record("7", b"raw", Some(b"// authored\r\n"), false);
    for end in 0..full.len() {
        assert!(Table::parse(&full[..end]).is_err(), "prefix {end}");
    }
    assert!(Table::parse(&[]).is_err());
    assert!(Table::parse(&BOM).is_err());
    assert_eq!(
        Table::parse(&record("7", b"", None, false))
            .unwrap()
            .body(7),
        Some(b"".as_slice())
    );
}
#[test]
fn duplicate_identity_policy_stays_with_existing_consumers() {
    let mut bytes = record("7", b"first", None, false);
    bytes.extend(record("7", b"second", None, false));
    let table = Table::parse(&bytes).unwrap();
    assert!(!table.ids_are_unique());
    assert_eq!(table.body(7), Some(b"first".as_slice()));
    assert!(
        map(Some(&bytes))
            .err()
            .unwrap()
            .contains("authored.wts: duplicate WTS ids")
    );
}
#[test]
fn present_malformed_missing_and_used_invalid_text_remain_distinct() {
    let absent = map(None).unwrap();
    assert!(
        absent
            .vm(NullHost)
            .call("Fetch", &[])
            .unwrap_err()
            .message
            .contains("TRIGSTR_7 is not in this map's war3map.wts")
    );
    assert!(
        map(Some(b""))
            .err()
            .unwrap()
            .contains("authored.wts: wts truncated")
    );
    let missing = map(Some(&record("8", b"else", None, false))).unwrap();
    assert!(
        missing
            .vm(NullHost)
            .call("Fetch", &[])
            .unwrap_err()
            .message
            .contains("TRIGSTR_7 is not in this map's war3map.wts")
    );
    let mut bytes = record("7", b"|nAuthored|c12345678|r", None, false);
    bytes.extend(record("8", b"\xff", None, false));
    let valid = map(Some(&bytes)).unwrap();
    assert_eq!(
        valid.vm(NullHost).call("Fetch", &[]).unwrap(),
        Value::Str("|nAuthored|c12345678|r".into())
    );
    let invalid = map(Some(&record("7", b"\xff", None, false))).unwrap();
    let mut vm = invalid.vm(NullHost);
    let error = vm.call("Fetch", &[]).unwrap_err();
    let context = invalid.diagnostic(&vm, &error);
    assert!(
        context.contains("authored.wts: WTS entry 7 is not UTF-8 at body byte 0"),
        "{context}"
    );
    assert!(context.contains("map.fixture:2:"), "{context}");
}
fn custom(wts: &[u8], max_wts_bytes: usize) -> Result<CustomScripts, CustomScriptError> {
    let mut wct_bytes = 0u32.to_le_bytes().to_vec();
    wct_bytes.extend(0u32.to_le_bytes());
    let wct = Wct::parse(&wct_bytes).unwrap();
    let mut wtg_bytes = b"WTG!".to_vec();
    for word in [4u32, 0, 0, 0, 0] {
        wtg_bytes.extend(word.to_le_bytes());
    }
    let wtg = Wtg::parse(&wtg_bytes, &TriggerArity::new()).unwrap();
    CustomScripts::compile(
        identity(),
        CustomScriptInputs {
            common: asset("common.fixture", b"// supplied prelude\n"),
            blizzard: asset("bj.fixture", b"// supplied prelude\n"),
            context: None,
            wct_label: "authored.wct",
            wct: &wct,
            wtg_label: "authored.wtg",
            wtg: &wtg,
            wts: Some(asset("authored.wts", wts)),
        },
        CustomScriptBudget {
            max_wts_bytes,
            ..Default::default()
        },
    )
}
#[test]
fn existing_finite_wts_budget_precedes_framing_and_exact_boundary_works() {
    let malformed = b"STRING bad";
    assert!(
        matches!(custom(malformed,malformed.len()-1),Err(CustomScriptError::Budget {region:"WTS",size,limit,..}) if size==malformed.len() && limit==malformed.len()-1)
    );
    assert!(
        matches!(custom(malformed,malformed.len()),Err(CustomScriptError::Wts {label,..}) if label=="authored.wts")
    );
    let valid = record("7", b"|nText", None, false);
    assert!(custom(&valid, valid.len()).is_ok());
    assert!(matches!(
        custom(&valid, valid.len() - 1),
        Err(CustomScriptError::Budget { region: "WTS", .. })
    ));
}
