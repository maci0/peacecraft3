use super::*;
use std::error::Error as _;

fn table(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
    let mut text = String::from("ID;PWXL;N;E\r\n");
    for (y, row) in std::iter::once(headers)
        .chain(rows.iter().copied())
        .enumerate()
    {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

fn fixture(rows: &[&[&str]]) -> Vec<u8> {
    table(&["tileID", "dir", "file"], rows)
}

#[test]
fn selects_requested_row_in_either_order_and_preserves_authored_spelling() {
    let first = &["Aaaa", "synthetic\\early", "early"][..];
    let chosen = &["Zzzz", "Synthetic\\Chosen", "Authored_Name"][..];
    for rows in [[first, chosen], [chosen, first]] {
        assert_eq!(
            terrain_tile_path_from_slk(&fixture(&rows), *b"Zzzz").unwrap(),
            "Synthetic\\Chosen\\Authored_Name.blp"
        );
    }
}

#[test]
fn identity_is_exact_without_case_or_whitespace_fallback() {
    let bytes = fixture(&[
        &["Zzzz", "synthetic", "chosen"],
        &[" Zzzz", "synthetic", "other"],
    ]);
    assert!(matches!(
        terrain_tile_path_from_slk(&bytes, *b"zzzz")
            .unwrap_err()
            .kind,
        TerrainTileErrorKind::UnknownTile
    ));
}

#[test]
fn absent_tile_does_not_choose_available_row() {
    let error = terrain_tile_path_from_slk(&fixture(&[&["Aaaa", "synthetic", "early"]]), *b"Zzzz")
        .unwrap_err();
    assert_eq!(error.tile_id, *b"Zzzz");
    assert_eq!(error.source_name, TERRAIN_SLK);
    assert!(matches!(error.kind, TerrainTileErrorKind::UnknownTile));
}

#[test]
fn duplicate_id_is_ambiguous_even_when_one_row_has_no_file() {
    for file in ["same", "", "different"] {
        let bytes = fixture(&[&["Zzzz", "synthetic", "same"], &["Zzzz", "synthetic", file]]);
        assert!(matches!(
            terrain_tile_path_from_slk(&bytes, *b"Zzzz")
                .unwrap_err()
                .kind,
            TerrainTileErrorKind::AmbiguousTile {
                first: 2,
                second: 3
            }
        ));
    }
}

#[test]
fn missing_or_empty_authored_values_are_errors() {
    for (row, column) in [
        (&["Zzzz", "", "chosen"][..], "dir"),
        (&["Zzzz", "synthetic", ""][..], "file"),
        (&["Zzzz", "synthetic"][..], "file"),
    ] {
        assert!(
            matches!(terrain_tile_path_from_slk(&fixture(&[row]), *b"Zzzz").unwrap_err().kind,
            TerrainTileErrorKind::MissingValue { row: 2, column: found } if found == column)
        );
    }
}

#[test]
fn missing_and_duplicate_headers_are_typed_errors() {
    let missing = table(&["tileID", "dir"], &[&["Zzzz", "synthetic"]]);
    assert!(matches!(
        terrain_tile_path_from_slk(&missing, *b"Zzzz")
            .unwrap_err()
            .kind,
        TerrainTileErrorKind::MissingColumn("file")
    ));
    let duplicate = table(
        &["tileID", "dir", "file", "file"],
        &[&["Zzzz", "synthetic", "a", "b"]],
    );
    assert!(matches!(
        terrain_tile_path_from_slk(&duplicate, *b"Zzzz")
            .unwrap_err()
            .kind,
        TerrainTileErrorKind::AmbiguousColumn("file")
    ));
}

#[test]
fn parser_failure_retains_source_chain() {
    let error = terrain_tile_path_from_slk(b"", *b"Zzzz").unwrap_err();
    assert!(matches!(
        error.kind,
        TerrainTileErrorKind::Parse(pcx_slq::Error::Empty)
    ));
    assert!(error.source().unwrap().source().is_some());
}

#[test]
fn refuses_records_after_terminator() {
    let mut bytes = fixture(&[&["Zzzz", "synthetic", "chosen"]]);
    bytes.extend_from_slice(b"C;X1;Y3;K\"Zzzz\"\r\n");
    assert!(matches!(
        terrain_tile_path_from_slk(&bytes, *b"Zzzz")
            .unwrap_err()
            .kind,
        TerrainTileErrorKind::TableAnomaly {
            kind: pcx_slq::AnomalyKind::StrayDataAfterEnd,
            ..
        }
    ));
}

#[test]
fn table_read_failure_retains_install_table_id_and_underlying_error() {
    let mut calls = Vec::new();
    let error = load_from("synthetic-edition", *b"Zzzz", |path| {
        calls.push(path.to_owned());
        Err(pcx_mpq::Error::NotFound(path.to_owned()))
    })
    .unwrap_err();
    assert_eq!(calls, [TERRAIN_SLK]);
    assert_eq!(
        error.source_name,
        format!("synthetic-edition::{TERRAIN_SLK}")
    );
    assert_eq!(error.tile_id, *b"Zzzz");
    assert!(matches!(
        error.kind,
        TerrainTileErrorKind::Read(pcx_mpq::Error::NotFound(_))
    ));
    assert!(error.source().unwrap().source().is_some());
}

#[test]
fn invalid_selection_never_attempts_texture_read() {
    let mut calls = Vec::new();
    let error = load_from("synthetic-edition", *b"Zzzz", |path| {
        calls.push(path.to_owned());
        Ok(fixture(&[&["Aaaa", "synthetic", "early"]]))
    })
    .unwrap_err();
    assert_eq!(calls, [TERRAIN_SLK]);
    assert!(matches!(error.kind, TerrainTileErrorKind::UnknownTile));
}

#[test]
fn texture_read_and_decode_failures_have_selected_path_context() {
    for decode in [false, true] {
        let mut calls = Vec::new();
        let error = load_from("synthetic-edition", *b"Zzzz", |path| {
            calls.push(path.to_owned());
            if path == TERRAIN_SLK {
                Ok(fixture(&[&["Zzzz", "synthetic", "chosen"]]))
            } else if decode {
                Ok(vec![255; 8])
            } else {
                Err(pcx_mpq::Error::NotFound(path.to_owned()))
            }
        })
        .unwrap_err();
        assert_eq!(calls, [TERRAIN_SLK, "synthetic\\chosen.blp"]);
        assert_eq!(
            error.source_name,
            "synthetic-edition::synthetic\\chosen.blp"
        );
        assert_eq!(error.tile_id, *b"Zzzz");
        if decode {
            assert!(matches!(error.kind, TerrainTileErrorKind::Decode(_)));
        } else {
            assert!(matches!(error.kind, TerrainTileErrorKind::Read(_)));
        }
    }
}

#[test]
fn successful_load_preserves_synthetic_authored_texels() {
    // Synthetic uncompressed true-color TGA, 1x1, top-left origin, BGR pixel.
    let mut tga = vec![0; 18];
    tga[2] = 2;
    tga[12] = 1;
    tga[14] = 1;
    tga[16] = 24;
    tga[17] = 0x20;
    tga.extend_from_slice(&[19, 31, 47]);
    let texture = load_from("synthetic-edition", *b"Zzzz", |path| {
        Ok(if path == TERRAIN_SLK {
            fixture(&[&["Zzzz", "synthetic", "chosen"]])
        } else {
            tga.clone()
        })
    })
    .unwrap();
    assert_eq!((texture.width, texture.height), (1, 1));
    assert_eq!(texture.rgba, [47, 31, 19, 255]);
    assert_eq!(texture.source, "synthetic\\chosen.blp");
}

#[test]
fn absent_install_is_read_error_without_default_tile() {
    let path = Path::new(".scratch/viewer-terrain-tests/synthetic-absent-install");
    assert!(!path.exists());
    for error in [
        terrain_tile_path(path, *b"Zzzz").unwrap_err(),
        load_terrain_tile(path, *b"Zzzz").unwrap_err(),
    ] {
        assert!(error.source_name.contains("synthetic-absent-install"));
        assert_eq!(error.tile_id, *b"Zzzz");
        assert!(matches!(error.kind, TerrainTileErrorKind::Read(_)));
    }
}
