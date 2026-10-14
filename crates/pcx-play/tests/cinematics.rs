//! Declaration ingestion tests; no renderer, Host or lifecycle mocks.
use pcx_play::cinematics::*;

fn limits() -> FdfLimits {
    FdfLimits {
        input_bytes: 128 * 1024,
        tokens: 16 * 1024,
        frames: 256,
        depth: 32,
    }
}
fn asset<'a>(label: &'a str, text: &'a str) -> FdfAsset<'a> {
    FdfAsset {
        label,
        text,
        edition: "synthetic-edition",
        version: "synthetic-version",
        source: "synthetic-fixture",
    }
}
fn parse(text: &str) -> Result<FdfCatalog, FdfError> {
    FdfCatalog::parse(&[asset("synthetic.fdf", text)], limits())
}
fn value(record: &Record) -> f64 {
    match record.values[0] {
        Atom::Number { value, .. } => value,
        _ => panic!("numeric fixture"),
    }
}
fn fixture(prefix: &str, width: f64, color: &str, font: &str, art: &str) -> String {
    let anchor_x = width * 0.071;
    let anchor_y = width * -0.089;
    let other_x = width * -0.043;
    let other_y = width * 0.061;
    format!(
        r#"IncludeFile "{prefix}-definitions.fdf",
Frame "TEXT" "{prefix}-template" {{
    FrameFont "{font}", 0.023f, "synthetic-flags",
    FontColor {color},
    BackdropBackground "{art}",
    Text "synthetic // text /* remains content */",
}}
Frame "FRAME" "{prefix}-root" {{
    SetAllPoints,
    Frame "TEXT" "{prefix}-label" INHERITS WITHCHILDREN "{prefix}-template" {{
        Width {width},
        SetPoint TOPLEFT, "{prefix}-root", TOPLEFT, {anchor_x}, {anchor_y},
        SetPoint BOTTOMRIGHT, "{prefix}-root", BOTTOMRIGHT, {other_x}, {other_y},
        UnknownRelationship "{prefix}-opaque", 7,
    }}
}}
"#
    )
}

#[test]
fn renamed_and_changed_asset_values_drive_ingestion() {
    for (prefix, width, colors, font, art) in [
        (
            "amber",
            1.25,
            "0.11 0.22 0.33",
            "Fonts\\SyntheticOne.ttf",
            "Art\\SyntheticOne.blp",
        ),
        (
            "violet",
            2.75,
            "0.44 0.55 0.66 0.77",
            "Fonts\\SyntheticTwo.ttf",
            "Art\\SyntheticTwo.blp",
        ),
    ] {
        let text = fixture(prefix, width, colors, font, art);
        let catalog = parse(&text).unwrap();
        let root = catalog.frame_id(&format!("{prefix}-root")).unwrap();
        let label = catalog.frame_id(&format!("{prefix}-label")).unwrap();
        let template = catalog.frame_id(&format!("{prefix}-template")).unwrap();
        let declaration = &catalog.frames()[label.0];
        assert_eq!(value(declaration.record("Width").unwrap()), width);
        assert_eq!(declaration.lexical_parent, Some(root));
        assert_eq!(catalog.frames()[root.0].children, [label]);
        assert_eq!(
            declaration.inherits,
            Some(Inheritance {
                template,
                with_children: true
            })
        );
        assert!(declaration.record("Height").is_none());
        assert!(
            declaration.record("FrameFont").is_none(),
            "no guessed inheritance merge"
        );
        assert_eq!(declaration.anchors.len(), 2);
        assert_eq!(declaration.anchors[0].x, width * 0.071);
        assert_eq!(declaration.anchors[1].y, width * 0.061);
        let style = &catalog.frames()[template.0];
        assert_eq!(
            style.record("FontColor").unwrap().values.len(),
            colors.split_whitespace().count()
        );
        assert_eq!(
            style.record("FrameFont").unwrap().values[0],
            Atom::Quoted(font.into())
        );
        assert_eq!(
            style.record("BackdropBackground").unwrap().values[0],
            Atom::Quoted(art.into())
        );
        assert_eq!(
            style.record("Text").unwrap().values,
            [Atom::Quoted(
                "synthetic // text /* remains content */".into()
            )]
        );
        assert!(
            declaration
                .record("UnknownRelationship")
                .unwrap()
                .raw
                .contains("-opaque")
        );
        for kind in [
            DependencyKind::Include,
            DependencyKind::Template,
            DependencyKind::Font,
            DependencyKind::Art,
            DependencyKind::Anchor,
        ] {
            assert!(
                catalog
                    .dependencies()
                    .iter()
                    .any(|dependency| dependency.kind == kind)
            );
        }
        assert_eq!(catalog.sources()[0].edition, "synthetic-edition");
        assert_eq!(catalog.sources()[0].version, "synthetic-version");
    }
    let first = parse(&fixture(
        "one",
        1.25,
        "0.11 0.22 0.33",
        "font-one",
        "art-one",
    ))
    .unwrap();
    let second = parse(&fixture(
        "two",
        2.75,
        "0.44 0.55 0.66",
        "font-two",
        "art-two",
    ))
    .unwrap();
    assert_ne!(first.sources()[0].fnv1a64, second.sources()[0].fnv1a64);
}

#[test]
fn every_anchor_and_parent_context_requires_explicit_binding() {
    let catalog = parse(&fixture("binding", 1.25, "0.11 0.22 0.33", "font", "art")).unwrap();
    let root = catalog.frame_id("binding-root").unwrap();
    let label = catalog.frame_id("binding-label").unwrap();
    assert!(matches!(
        catalog.relationships(root, &[], None).unwrap_err().kind,
        FdfErrorKind::MissingParentContext
    ));
    let context = Target::Context("caller-owned-active-context".into());
    assert_eq!(
        catalog
            .relationships(root, &[], Some(context.clone()))
            .unwrap()
            .all_points,
        Some(context)
    );
    assert!(matches!(
        catalog.relationships(label, &[], None).unwrap_err().kind,
        FdfErrorKind::MissingBinding(_)
    ));
    let binding = FrameBinding {
        authored_name: "binding-root".into(),
        target: Target::Frame(root),
    };
    let result = catalog
        .relationships(label, std::slice::from_ref(&binding), None)
        .unwrap();
    assert_eq!(result.anchors.len(), 2);
    assert_eq!(result.anchors[0].target, Target::Frame(root));
    assert_eq!(result.anchors[1].target, Target::Frame(root));
    assert!(result.all_points.is_none());
    assert!(matches!(
        catalog
            .relationships(label, &[binding.clone(), binding], None)
            .unwrap_err()
            .kind,
        FdfErrorKind::Duplicate(_)
    ));
    assert!(matches!(
        catalog
            .relationships(root, &[], Some(Target::Frame(FrameId(999))))
            .unwrap_err()
            .kind,
        FdfErrorKind::UnknownFrame(_)
    ));
    assert!(matches!(
        catalog
            .relationships(root, &[], Some(Target::Context(String::new())))
            .unwrap_err()
            .kind,
        FdfErrorKind::MissingIdentity("context")
    ));
}

#[test]
fn inheritance_requires_unique_supplied_templates_and_refuses_cycles() {
    let absent = "Frame \"TEXT\" \"derived\" INHERITS \"missing-template\" {\n}\n";
    assert!(matches!(
        parse(absent).unwrap_err().kind,
        FdfErrorKind::UnknownTemplate(_)
    ));
    let cycle = "Frame \"TEXT\" \"first\" INHERITS \"second\" {\n}\nFrame \"TEXT\" \"second\" INHERITS \"first\" {\n}\n";
    assert!(matches!(
        parse(cycle).unwrap_err().kind,
        FdfErrorKind::InheritanceCycle(_)
    ));
    let definition = "Frame \"TEXT\" \"shared\" {\nWidth 1.25,\n}\n";
    let changed = definition.replace("1.25", "2.75");
    for sources in [
        [asset("first", definition), asset("second", &changed)],
        [asset("second", &changed), asset("first", definition)],
    ] {
        assert!(matches!(
            FdfCatalog::parse(&sources, limits()).unwrap_err().kind,
            FdfErrorKind::Duplicate(_)
        ));
    }
    let catalog = FdfCatalog::parse(
        &[
            asset(
                "derived",
                "Frame \"TEXT\" \"derived\" INHERITS \"shared\" {\n}\n",
            ),
            asset("template", definition),
        ],
        limits(),
    )
    .unwrap();
    assert_eq!(
        catalog.frames()[0].inherits.as_ref().unwrap().template,
        catalog.frame_id("shared").unwrap()
    );
}

#[test]
fn duplicate_properties_frames_and_exact_anchors_fail() {
    for body in [
        "Width 1.25,\nWidth 2.75,\n",
        "SetAllPoints,\nSetAllPoints,\n",
        "Width 1.25,\nSetPoint TOP, \"target\", TOP, 0.071, 0.061,\nSetPoint TOP, \"target\", TOP, 0.071f, 0.061,\n",
    ] {
        let text = format!("Frame \"TEXT\" \"duplicate\" {{\n{body}}}\n");
        assert!(matches!(
            parse(&text).unwrap_err().kind,
            FdfErrorKind::Duplicate(_)
        ));
    }
    assert!(matches!(
        parse("Frame \"FRAME\" \"same\" {}\nFrame \"FRAME\" \"same\" {}\n")
            .unwrap_err()
            .kind,
        FdfErrorKind::Duplicate(_)
    ));
    assert!(matches!(
        FdfCatalog::parse(&[asset("same", ""), asset("same", "")], limits())
            .unwrap_err()
            .kind,
        FdfErrorKind::Duplicate(_)
    ));
}

#[test]
fn malformed_numbers_shapes_strings_comments_and_frames_are_errors() {
    for property in [
        "Width NaN,",
        "Width inf,",
        "Width -inf,",
        "Height 1e999,",
        "Width 1.2.3,",
        "Width authored-unknown,",
        "FontColor 0.11 0.22,",
        "BackdropBackgroundInsets 1 2 3,",
        "FrameFont \"font\", broken, \"\",",
        "BackdropBackground \"one\" \"two\",",
        "SetPoint UNKNOWN, \"target\", TOP, 0.071, 0.061,",
        "SetPoint TOP, \"target\", TOP, 0.071,",
        "SetAllPoints 1,",
        "Width 1.25",
        "Width 1.25, Height 2.75,",
        "Width, 1.25,",
        "Width 1.25,,",
        "SetPoint TOP, \"target\", TOP, 0.071, 0.061,,",
        "SetPoint TOP \"target\" TOP 0.071 0.061,",
        "FrameFont \"font\" 0.023 \"\",",
    ] {
        let text = format!("Frame \"TEXT\" \"bad\" {{\n{property}\n}}\n");
        let error = parse(&text).unwrap_err();
        assert_eq!(error.source, "synthetic.fdf");
        assert!(error.offset > 0);
    }
    for text in [
        "Frame \"TEXT\" \"open\" {\n",
        "Frame \"TEXT\" \"open\" {\nText \"unterminated",
        "/* unterminated",
        "Frame \"TEXT\" \"bad\" {\nText \"line\nbreak\",\n}\n",
    ] {
        assert!(matches!(
            parse(text).unwrap_err().kind,
            FdfErrorKind::Unterminated(_)
        ));
    }
    assert!(matches!(
        parse(
            r#"Frame "TEXT" "escaped" {
Text "synthetic\"quote",
}
"#
        )
        .unwrap_err()
        .kind,
        FdfErrorKind::Unsupported(_)
    ));
    assert!(matches!(
        parse("}\n").unwrap_err().kind,
        FdfErrorKind::Syntax(_)
    ));
    assert!(matches!(
        parse("UnsupportedTopLevel \"raw\",\n").unwrap_err().kind,
        FdfErrorKind::Unsupported(_)
    ));
}

#[test]
fn comments_and_f_suffix_are_parsed_without_rewriting_raw_records() {
    let text = "/* block\ncomment */\nFrame \"TEXT\" \"comment-fixture\" {\nWidth 1.25f, // trailing comment\nHeight 2.75,\n}\n";
    let catalog = parse(text).unwrap();
    let record = catalog.frames()[0].record("Width").unwrap();
    assert_eq!(record.raw, "Width 1.25f,");
    assert_eq!(
        record.values[0],
        Atom::Number {
            spelling: "1.25f".into(),
            value: 1.25
        }
    );
}

#[test]
fn all_budgets_and_identity_are_checked() {
    let text = "Frame \"FRAME\" \"outer\" {\nFrame \"FRAME\" \"inner\" {\n}\n}\n";
    for (limited, expected) in [
        (
            FdfLimits {
                input_bytes: text.len() - 1,
                ..limits()
            },
            "input bytes",
        ),
        (
            FdfLimits {
                tokens: 1,
                ..limits()
            },
            "tokens",
        ),
        (
            FdfLimits {
                frames: 1,
                ..limits()
            },
            "frames",
        ),
        (
            FdfLimits {
                depth: 1,
                ..limits()
            },
            "frame depth",
        ),
    ] {
        assert_eq!(
            FdfCatalog::parse(&[asset("budget", text)], limited)
                .unwrap_err()
                .kind,
            FdfErrorKind::Budget(expected)
        );
    }
    let other = text
        .replace("outer", "other-outer")
        .replace("inner", "other-inner");
    assert_eq!(
        FdfCatalog::parse(
            &[asset("first", text), asset("second", &other)],
            FdfLimits {
                input_bytes: text.len() + other.len() - 1,
                ..limits()
            }
        )
        .unwrap_err()
        .kind,
        FdfErrorKind::Budget("input bytes")
    );
    let single = "Frame \"FRAME\" \"single\" {}\n";
    let another = single.replace("single", "another");
    assert_eq!(
        FdfCatalog::parse(
            &[asset("first", single), asset("second", &another)],
            FdfLimits {
                tokens: 8,
                ..limits()
            }
        )
        .unwrap_err()
        .kind,
        FdfErrorKind::Budget("tokens")
    );
    let catalog = FdfCatalog::parse(
        &[asset("exact", text)],
        FdfLimits {
            input_bytes: text.len(),
            frames: 2,
            depth: 2,
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(catalog.frames().len(), 2);
    for missing in ["label", "edition", "version", "source"] {
        let mut source = asset("identity", text);
        match missing {
            "label" => source.label = "",
            "edition" => source.edition = "",
            "version" => source.version = "",
            "source" => source.source = "",
            _ => unreachable!(),
        }
        assert_eq!(
            FdfCatalog::parse(&[source], limits()).unwrap_err().kind,
            FdfErrorKind::MissingIdentity(missing)
        );
    }
}

#[test]
#[ignore = "requires exact local original FDF paths, expected FNV hashes and explicit archive identity/context"]
fn explicit_original_fdf_ingestion_gate() {
    let required = |key: &str| std::env::var(key).unwrap_or_else(|_| panic!("{key} is required"));
    let paths = [
        required("PEACECRAFT3_CINEMATIC_FDF"),
        required("PEACECRAFT3_DIALOG_FDF"),
        required("PEACECRAFT3_TEMPLATE_FDF"),
    ];
    let hashes: Vec<u64> = [
        "PEACECRAFT3_CINEMATIC_FDF_HASH",
        "PEACECRAFT3_DIALOG_FDF_HASH",
        "PEACECRAFT3_TEMPLATE_FDF_HASH",
    ]
    .into_iter()
    .map(|key| {
        u64::from_str_radix(required(key).trim_start_matches("0x"), 16)
            .expect("expected FNV-1a hex")
    })
    .collect();
    let texts: Vec<String> = paths
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("read explicit local original FDF"))
        .collect();
    let edition = required("PEACECRAFT3_FDF_EDITION");
    let version = required("PEACECRAFT3_FDF_VERSION");
    let source = required("PEACECRAFT3_FDF_SOURCE");
    let context = required("PEACECRAFT3_FDF_CONTEXT");
    let inputs: Vec<FdfAsset<'_>> = paths
        .iter()
        .zip(&texts)
        .map(|(label, text)| FdfAsset {
            label,
            text,
            edition: &edition,
            version: &version,
            source: &source,
        })
        .collect();
    let catalog = FdfCatalog::parse(&inputs, limits()).expect("checked original declarations");
    for (identity, expected) in catalog.sources().iter().zip(hashes) {
        assert_eq!(identity.fnv1a64, expected);
    }
    for (index, frame) in catalog.frames().iter().enumerate() {
        let bindings: Vec<FrameBinding> = frame
            .anchors
            .iter()
            .map(|anchor| FrameBinding {
                authored_name: anchor.relative_name.clone(),
                target: Target::Frame(
                    catalog
                        .frame_id(&anchor.relative_name)
                        .expect("explicitly selected supplied target"),
                ),
            })
            .fold(Vec::new(), |mut bindings, binding| {
                if !bindings
                    .iter()
                    .any(|prior: &FrameBinding| prior.authored_name == binding.authored_name)
                {
                    bindings.push(binding);
                }
                bindings
            });
        let result = catalog
            .relationships(
                FrameId(index),
                &bindings,
                Some(Target::Context(context.clone())),
            )
            .unwrap();
        assert_eq!(result.anchors.len(), frame.anchors.len());
    }
    eprintln!(
        "Original declaration gate: sources={:?}; frames={} dependencies={}; no rendering/native execution",
        catalog.sources(),
        catalog.frames().len(),
        catalog.dependencies().len()
    );
}
