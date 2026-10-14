//! Boundary and hardening tests for checked cinematic FDF ingestion.
//! These exercise metadata/catalog behavior only; they are not native proof.
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

#[test]
fn invalid_frame_ids_are_bounds_checked() {
    let catalog =
        parse("Frame \"TEXT\" \"one\" {\nSetPoint TOP, \"two\", TOP, 0, 0,\n}\n").unwrap();
    assert!(catalog.frame(FrameId(999)).is_none());
    assert!(matches!(
        catalog
            .relationships(FrameId(999), &[], None)
            .unwrap_err()
            .kind,
        FdfErrorKind::UnknownFrame(FrameId(999))
    ));
    let binding = FrameBinding {
        authored_name: "two".into(),
        target: Target::Frame(FrameId(999)),
    };
    assert!(matches!(
        catalog
            .relationships(FrameId(0), &[binding], None)
            .unwrap_err()
            .kind,
        FdfErrorKind::UnknownFrame(FrameId(999))
    ));
}

#[test]
fn missing_and_duplicate_bindings_keep_contextual_errors() {
    let catalog =
        parse("Frame \"TEXT\" \"one\" {\nSetPoint TOP, \"two\", TOP, 0, 0,\nSetAllPoints,\n}\n")
            .unwrap();
    let frame = catalog.frame_id("one").unwrap();
    let binding = FrameBinding {
        authored_name: "two".into(),
        target: Target::Context("caller".into()),
    };
    assert!(matches!(
        catalog.relationships(frame, &[], None).unwrap_err().kind,
        FdfErrorKind::MissingBinding(_)
    ));
    assert!(matches!(
        catalog
            .relationships(frame, &[binding.clone(), binding], None)
            .unwrap_err()
            .kind,
        FdfErrorKind::Duplicate(_)
    ));
    assert!(matches!(
        catalog
            .relationships(frame, &[], Some(Target::Context(String::new())))
            .unwrap_err()
            .kind,
        FdfErrorKind::MissingIdentity("context")
    ));
    let binding = FrameBinding {
        authored_name: "two".into(),
        target: Target::Context("caller".into()),
    };
    assert!(matches!(
        catalog
            .relationships(frame, std::slice::from_ref(&binding), None)
            .unwrap_err()
            .kind,
        FdfErrorKind::MissingParentContext
    ));
}

#[test]
fn inheritance_ambiguity_cycles_and_self_edges_are_refused() {
    for text in [
        "Frame \"TEXT\" \"a\" INHERITS \"a\" {\n}\n",
        "Frame \"TEXT\" \"a\" INHERITS \"b\" {\n}\nFrame \"TEXT\" \"b\" INHERITS \"c\" {\n}\nFrame \"TEXT\" \"c\" INHERITS \"a\" {\n}\n",
    ] {
        assert!(matches!(
            parse(text).unwrap_err().kind,
            FdfErrorKind::InheritanceCycle(_)
        ));
    }
    assert!(matches!(
        parse("Frame \"TEXT\" \"a\" INHERITS \"missing\" {\n}\n")
            .unwrap_err()
            .kind,
        FdfErrorKind::UnknownTemplate(_)
    ));
    assert!(matches!(
        FdfCatalog::parse(
            &[
                asset("first", "Frame \"TEXT\" \"shared\" {\nWidth 1.0,\n}\n"),
                asset("second", "Frame \"TEXT\" \"shared\" {\nWidth 2.0,\n}\n"),
            ],
            limits()
        )
        .unwrap_err()
        .kind,
        FdfErrorKind::Duplicate(_)
    ));
}

#[test]
fn resource_limits_use_checked_accounting() {
    let text = "Frame \"FRAME\" \"single\" {}\n";
    let another = text.replace("single", "another");
    assert_eq!(
        FdfCatalog::parse(
            &[asset("first", text), asset("second", &another)],
            FdfLimits {
                tokens: 6,
                ..limits()
            }
        )
        .unwrap_err()
        .kind,
        FdfErrorKind::Budget("tokens")
    );
    let deep = "Frame \"FRAME\" \"outer\" {\nFrame \"FRAME\" \"inner\" {\n}\n}\n";
    assert_eq!(
        FdfCatalog::parse(
            &[asset("deep", deep)],
            FdfLimits {
                depth: 1,
                ..limits()
            }
        )
        .unwrap_err()
        .kind,
        FdfErrorKind::Budget("frame depth")
    );
    let invalid = FdfLimits {
        tokens: 0,
        ..limits()
    };
    assert_eq!(
        FdfCatalog::parse(&[asset("empty", "")], invalid)
            .unwrap()
            .frames()
            .len(),
        0
    );
}
