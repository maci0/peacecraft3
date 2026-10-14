use pcx_objects::Error;
use pcx_objects::editor::{Editor, EditorReadLimits, path::SlkId};
use std::{convert::Infallible, path::Path};

const TABLE: &[u8] = b"ID;PWXL;N;E\r\nB;Y2;X2\r\nC;X1;Y1;K\"unitID\"\r\nC;X2;Y1;K\"race\"\r\nC;X1;Y2;K\"TEST\"\r\nC;X2;Y2;K\"authored\"\r\nE\r\n";
fn limits() -> EditorReadLimits {
    EditorReadLimits {
        max_listed_paths: 1,
        max_asset_bytes: TABLE.len(),
        max_aggregate_input_bytes: TABLE.len(),
    }
}
#[test]
fn absent_input_stays_explicit_without_substitution() {
    let editor =
        Editor::from_reader::<Infallible>(Path::new("explicit source"), &[], limits(), |_| {
            Ok(None)
        })
        .unwrap();
    assert_eq!(editor.absent(), SlkId::ALL);
    assert_eq!(editor.build(), "explicit source");
}
#[test]
fn reader_failure_retains_typed_cause_member_and_source() {
    let error = Editor::from_reader(Path::new("explicit source"), &[], limits(), |_| {
        Err(std::io::Error::other("read refusal"))
    })
    .unwrap_err();
    let Error::EditorAsset {
        source_label,
        member,
        source,
    } = error
    else {
        panic!("reader error must remain contextual")
    };
    assert_eq!(source_label, "explicit source");
    assert_eq!(member, SlkId::ALL[0].path());
    assert_eq!(
        source.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::Other
    );
}
#[test]
fn malformed_present_table_retains_typed_parse_cause_and_source() {
    let error =
        Editor::from_reader::<Infallible>(Path::new("explicit source"), &[], limits(), |name| {
            Ok((name == SlkId::UnitData.path()).then(|| b"bad signature".to_vec()))
        })
        .unwrap_err();
    let Error::EditorAsset {
        source_label,
        member,
        source,
    } = error
    else {
        panic!("parse error must remain contextual")
    };
    assert_eq!(source_label, "explicit source");
    assert_eq!(member, SlkId::UnitData.path());
    assert!(source.downcast_ref::<pcx_slq::Error>().is_some());
}
#[test]
fn valid_supplied_table_preserves_rows_at_exact_input_limits() {
    let editor =
        Editor::from_reader::<Infallible>(Path::new("explicit source"), &[], limits(), |name| {
            Ok((name == SlkId::UnitData.path()).then(|| TABLE.to_vec()))
        })
        .unwrap();
    assert!(editor.row(SlkId::UnitData, "TEST").is_some());
    assert!(!editor.absent().contains(&SlkId::UnitData));
    assert!(editor.absent().contains(&SlkId::UnitBalance));
}
#[test]
fn member_and_aggregate_limits_precede_table_parsing() {
    for budget in [
        EditorReadLimits {
            max_asset_bytes: 0,
            ..limits()
        },
        EditorReadLimits {
            max_aggregate_input_bytes: 0,
            ..limits()
        },
    ] {
        let error =
            Editor::from_reader::<Infallible>(Path::new("explicit source"), &[], budget, |name| {
                Ok((name == SlkId::UnitData.path()).then(|| b"bad signature".to_vec()))
            })
            .unwrap_err();
        assert!(matches!(
            error,
            Error::Unsupported {
                what: "editor supplied input budget",
                ..
            }
        ));
    }
}
#[test]
fn inventory_limit_refuses_before_any_reader_call() {
    let mut calls = 0;
    let error = Editor::from_reader::<Infallible>(
        Path::new("explicit source"),
        &["one".into(), "two".into()],
        limits(),
        |_| {
            calls += 1;
            Ok(None)
        },
    )
    .unwrap_err();
    assert_eq!(calls, 0);
    assert!(matches!(
        error,
        Error::Unsupported {
            what: "editor listed path budget",
            ..
        }
    ));
}
#[cfg(not(target_arch = "wasm32"))]
mod archive {
    use super::*;
    include!("support/editor_mpq.rs");
    #[test]
    fn legacy_archive_api_does_not_demote_present_decode_failure() {
        let name = SlkId::UnitData.path();
        let mut bytes = fixture(&[
            (name, &[4, 0xaa]),
            ("(listfile)", b"Units\\UnitData.slk\r\n"),
        ]);
        table_mut(&mut bytes, false, |table| {
            table[8..12].copy_from_slice(&3u32.to_le_bytes());
            table[12..16].copy_from_slice(
                &(pcx_mpq::flags::EXISTING
                    | pcx_mpq::flags::SINGLE_UNIT
                    | pcx_mpq::flags::COMPRESS)
                    .to_le_bytes(),
            );
        });
        let dir = std::env::temp_dir().join(format!("pcx-editor-reader-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("War3.mpq"), &bytes[MPQ..]).unwrap();
        let result = {
            let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
            assert!(set.extract(name).is_err());
            Editor::open_in(&mut set, &dir)
        };
        std::fs::remove_dir_all(&dir).unwrap();
        let Error::EditorAsset { member, source, .. } = result.unwrap_err() else {
            panic!("matching table failure must propagate")
        };
        assert_eq!(member, name);
        assert!(source.downcast_ref::<pcx_mpq::Error>().is_some());
    }
}
