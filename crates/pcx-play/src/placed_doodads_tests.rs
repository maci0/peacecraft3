use super::*;

fn table(headers: &[&str], rows: &[Vec<&str>]) -> Vec<u8> {
    let mut text = "ID;PWXL;N;E\n".to_owned();
    for (y, row) in std::iter::once(headers)
        .chain(rows.iter().map(Vec::as_slice))
        .enumerate()
    {
        for (x, value) in row.iter().enumerate() {
            text += &format!("C;X{};Y{};K\"{value}\"\n", x + 1, y + 1);
        }
    }
    text += "E\n";
    text.replace('\n', "\r\n").into_bytes()
}
fn tables() -> DefaultTables {
    let data = table(
        &[
            "doodID", "file", "numVar", "vertR01", "vertG01", "vertB01", "vertR02", "vertG02",
            "vertB02",
        ],
        &[vec![
            "Zx91",
            "Synthetic\\Decoration",
            "2",
            "33",
            "91",
            "151",
            "47",
            "103",
            "179",
        ]],
    );
    let metadata = table(
        &["ID", "field", "slk", "type", "index", "category", "repeat"],
        &[
            vec!["Zfil", "file", "DoodadData", "model", "0", "art", "0"],
            vec!["Zvar", "numVar", "DoodadData", "int", "0", "art", "0"],
            vec!["Zred", "vertR", "DoodadData", "int", "0", "art", "2"],
            vec!["Zgrn", "vertG", "DoodadData", "int", "0", "art", "2"],
            vec!["Zblu", "vertB", "DoodadData", "int", "0", "art", "2"],
        ],
    );
    DefaultTables::from_bytes(
        pcx_objects::editor::defaults::Identity {
            install: "synthetic".into(),
            edition_version: "nonstock".into(),
        },
        &[
            (SlkId::Doodads.path(), &data),
            (SlkId::DoodadMetaData.path(), &metadata),
        ],
        &[],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 16384,
            max_aggregate_input_bytes: 32768,
        },
    )
    .unwrap()
}
fn instance() -> pcx_map::Doodad {
    pcx_map::Doodad {
        type_id: *b"Zx91",
        variation: 1,
        position: [3., 7., 11.],
        angle: 0.75,
        scale: [0.75, 1.25, 2.],
        flags: 2,
        life: 255,
        creation_number: 73,
    }
}
#[test]
fn variation_path_and_colour_come_from_the_selected_table_and_inventory() {
    let paths = ["synthetic/decoration0.mdx", "synthetic/decoration1.mdx"];
    let (member, tint) = appearance_source(
        &tables(),
        &Overlay::default(),
        &instance(),
        paths.into_iter(),
    )
    .unwrap();
    assert_eq!(member, paths[1]);
    assert_eq!(tint, [47., 103., 179.]);
    let mut invalid = instance();
    invalid.variation = 2;
    assert!(
        appearance_source(&tables(), &Overlay::default(), &invalid, paths.into_iter()).is_err()
    );
    invalid = instance();
    invalid.scale[0] = f32::NAN;
    assert!(
        appearance_source(&tables(), &Overlay::default(), &invalid, paths.into_iter()).is_err()
    );
}
#[test]
fn repeated_colour_edits_select_the_authored_variation_and_refuse_ambiguous_selectors() {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "Zx91".into(),
        vec![pcx_objects::Mod {
            field: *b"Zred",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([2, 0]),
            value: pcx_objects::Value::Int(197),
            end_word: 0,
        }],
    );
    let paths = ["synthetic/decoration0.mdx", "synthetic/decoration1.mdx"];
    let read =
        |overlay: &Overlay| appearance_source(&tables(), overlay, &instance(), paths.into_iter());
    assert_eq!(read(&overlay).unwrap().1, [197., 103., 179.]);
    overlay.mods.get_mut("Zx91").unwrap()[0].selectors = Some([1, 0]);
    assert_eq!(read(&overlay).unwrap().1, [47., 103., 179.]);
    for selector in [None, Some([0, 0]), Some([3, 0]), Some([2, 1])] {
        overlay.mods.get_mut("Zx91").unwrap()[0].selectors = selector;
        assert!(read(&overlay).is_err(), "{selector:?}");
    }
}
#[test]
fn a_single_variation_model_must_be_an_unambiguous_source_member() {
    assert_eq!(
        model_member(
            "Synthetic\\Decoration.mdl",
            0,
            1,
            ["synthetic/decoration.mdx"].into_iter()
        )
        .unwrap(),
        "synthetic/decoration.mdx"
    );
    assert!(
        model_member(
            "Synthetic\\Decoration",
            0,
            1,
            ["synthetic/decoration.mdx", "synthetic/decoration0.mdx"].into_iter()
        )
        .is_err()
    );
    assert!(
        model_member(
            "Synthetic\\Decoration",
            1,
            2,
            ["synthetic/decoration.mdx"].into_iter()
        )
        .is_err()
    );
}
