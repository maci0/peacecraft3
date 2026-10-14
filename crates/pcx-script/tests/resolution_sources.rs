use pcx_script::{
    Prelude, ResolveDiagnostic, Stage, parse, resolve_with, resolve_with_diagnostics,
};

fn refusal(main: &str, preludes: &[(&str, &str)]) -> ResolveDiagnostic {
    let script = parse(main).unwrap();
    let parsed: Vec<_> = preludes
        .iter()
        .map(|(_, text)| parse(text).unwrap())
        .collect();
    let inputs: Vec<_> = preludes
        .iter()
        .zip(&parsed)
        .map(|((label, _), tree)| Prelude::new(label, tree))
        .collect();
    let diagnostic = resolve_with_diagnostics(&script, &inputs).unwrap_err();
    assert_eq!(diagnostic.error.stage, Stage::Resolve);
    assert_eq!(
        resolve_with(&script, &inputs).unwrap_err(),
        diagnostic.error
    );
    diagnostic
}

#[test]
fn prelude_type_failure_keeps_its_label_even_if_it_matches_legacy_main_label() {
    for label in ["common.j", "the map script"] {
        let error = refusal("", &[(label, "type Broken extends Missing\n")]);
        assert_eq!(error.source_label.as_deref(), Some(label));
        assert_eq!(error.error.span.start, 0);
        assert!(error.error.message.contains("Broken"));
    }
}

#[test]
fn prelude_type_cycle_keeps_root_declaration_source() {
    let error = refusal("", &[("types.j", "type A extends B\ntype B extends A\n")]);
    assert_eq!(error.source_label.as_deref(), Some("types.j"));
    assert!(error.error.message.contains("cyclic"));
}

#[test]
fn main_type_override_retains_main_origin() {
    let error = refusal(
        "type A extends integer\n",
        &[("types.j", "type A extends Missing\n")],
    );
    assert_eq!(error.source_label, None);
    assert!(error.error.message.contains("not a handle type"));
    let main = parse("type A extends handle\n").unwrap();
    let prelude = parse("type A extends Missing\n").unwrap();
    let program = resolve_with_diagnostics(&main, &[Prelude::new("types.j", &prelude)]).unwrap();
    assert_eq!(program.types().count(), 1);
}

#[test]
fn missing_library_dependency_tracks_the_authored_requirement() {
    let error = refusal(
        "",
        &[("library.j", "library L requires Missing\nendlibrary\n")],
    );
    assert_eq!(error.source_label.as_deref(), Some("library.j"));
    assert!(error.error.message.contains("requires `Missing`"));
    let error = refusal("library L requires Missing\nendlibrary\n", &[]);
    assert_eq!(error.source_label, None);
}

#[test]
fn cyclic_library_diagnostic_tracks_the_span_owner() {
    let error = refusal(
        "",
        &[(
            "library.j",
            "library A requires B\nendlibrary\nlibrary B requires A\nendlibrary\n",
        )],
    );
    assert_eq!(error.source_label.as_deref(), Some("library.j"));
    assert!(error.error.message.contains("requires itself"));
}

#[test]
fn declaration_conflict_tracks_the_later_prelude() {
    let error = refusal(
        "",
        &[
            (
                "first.j",
                "function F takes nothing returns nothing\nendfunction\n",
            ),
            (
                "later.j",
                "function F takes integer x returns nothing\ncall Unchecked()\nendfunction\n",
            ),
        ],
    );
    assert_eq!(error.source_label.as_deref(), Some("later.j"));
    assert!(error.error.message.contains("different signatures"));
}

#[test]
fn main_body_failure_does_not_inherit_a_prelude_origin() {
    let error = refusal(
        "function F takes nothing returns nothing\ncall Missing()\nendfunction\n",
        &[("types.j", "type Unit extends handle\n")],
    );
    assert_eq!(error.source_label, None);
    assert!(error.error.message.contains("Missing"));
}

#[test]
fn prelude_body_check_policy_is_unchanged() {
    let main = parse("").unwrap();
    let prelude =
        parse("function F takes nothing returns nothing\ncall Missing()\nendfunction\n").unwrap();
    let inputs = [Prelude::new("body.j", &prelude)];
    assert!(resolve_with(&main, &inputs).is_ok());
    assert!(resolve_with_diagnostics(&main, &inputs).is_ok());
}
