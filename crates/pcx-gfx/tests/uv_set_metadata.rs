use pcx_gfx::mdx_runtime::{self, PlanError, RuntimeInput, RuntimePlan};
use pcx_mdl::{Error, Pivots};

#[path = "support/uv_set_fixtures.rs"]
mod fixtures;
use fixtures::*;

#[test]
fn actual_walk_counts_zero_one_and_multiple_sets_without_changing_last_array() {
    for count in [0, 1, 2, 5] {
        let parsed = mesh(data_with_sets(count)).unwrap();
        assert_eq!(parsed.uv_set_count, Some(count));
        if count == 0 {
            assert_eq!(parsed.tex_coords.count, 0);
        } else {
            assert_eq!(
                parsed.tex_coords.get(0),
                Some([count as f32 + 1.0, count as f32 + 1.25])
            );
        }
    }
}

#[test]
fn repeated_uvbs_records_are_sets_even_when_payloads_agree() {
    let mut data = base();
    block(&mut data, b"UVAS", 2, &[]);
    uv(&mut data, 7.0);
    uv(&mut data, 7.0);
    let parsed = mesh(data).unwrap();
    assert_eq!(parsed.uv_set_count, Some(2));
    assert_eq!(parsed.tex_coords.get(0), Some([7.0, 7.25]));
}

#[test]
fn duplicate_uvas_is_refused_even_for_identical_empty_declarations() {
    let mut data = data_with_sets(0);
    let offset = data.len();
    block(&mut data, b"UVAS", 0, &[]);
    assert!(
        matches!(mesh(data), Err(Error::DuplicateGeosetTag { at, tag }) if at == offset && tag == *b"UVAS")
    );
}

#[test]
fn declaration_must_match_actual_uvbs_record_count() {
    for (declared, actual) in [(2, 1), (1, 2), (0, 1), (u32::MAX, 0)] {
        let mut data = base();
        block(&mut data, b"UVAS", declared, &[]);
        for _ in 0..actual {
            uv(&mut data, 3.0);
        }
        assert!(
            matches!(mesh(data), Err(Error::UvSetCount { declared: d, actual: a, .. }) if d == declared && a == actual)
        );
    }
}

#[test]
fn uvbs_without_uvas_has_no_invented_count() {
    let mut data = base();
    let offset = data.len();
    uv(&mut data, 4.0);
    assert!(matches!(mesh(data), Err(Error::MissingUvSetDeclaration { at }) if at == offset));
    assert_eq!(mesh(base()).unwrap().uv_set_count, None);
}

#[test]
fn truncated_headers_payloads_and_wrong_tags_fail_before_metadata_admission() {
    for tag in [b"UVAS", b"UVBS"] {
        for len in 1..8 {
            let mut data = base();
            block(&mut data, b"UVAS", 1, &[]);
            let mut header = tag.to_vec();
            header.extend(1u32.to_le_bytes());
            data.extend(&header[..len]);
            assert!(mesh(data).is_err(), "tag {tag:?}, length {len}");
        }
    }
    let mut data = data_with_sets(1);
    data.pop();
    assert!(matches!(mesh(data), Err(Error::Truncated { .. })));
    let mut data = base();
    block(&mut data, b"UVAS", 1, &[]);
    block(&mut data, b"UVBX", 2, &[0; 16]);
    assert!(matches!(mesh(data), Err(Error::UnknownGeosetTag(tag)) if tag == *b"UVBX"));
    let mut data = base();
    block(&mut data, b"UVAS", 1, &[]);
    block(&mut data, b"UVBS", u32::MAX, &[]);
    assert!(matches!(mesh(data), Err(Error::Truncated { .. })));
}

#[test]
fn actual_runtime_parse_carries_counts_by_geoset_ordinal() {
    let bytes = model(&[data_with_sets(2), data_with_sets(0), data_with_sets(5)]);
    let plan = RuntimePlan::parse(&bytes).unwrap();
    assert_eq!(plan.uv_set_counts, [Some(2), Some(0), Some(5)]);
    assert_eq!(
        plan.geosets.iter().map(|g| g.geoset_id).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(
        plan.geosets
            .iter()
            .map(|g| g.vertex_count)
            .collect::<Vec<_>>(),
        [2, 2, 2]
    );
}

#[test]
fn runtime_parse_preserves_no_uv_and_mixed_metadata_without_affecting_rig_admission() {
    let no_uv = RuntimePlan::parse(&model(&[base()])).unwrap();
    assert_eq!(no_uv.uv_set_counts, [None]);
    let mixed = RuntimePlan::parse(&model(&[
        data_with_sets(2),
        base(),
        data_with_sets(0),
        data_with_sets(1),
    ]))
    .unwrap();
    assert_eq!(mixed.uv_set_counts, [Some(2), None, Some(0), Some(1)]);
    assert_eq!(mixed.geosets.len(), 4);
    let mut malformed = base();
    block(&mut malformed, b"UVAS", 2, &[]);
    uv(&mut malformed, 1.0);
    assert!(matches!(
        RuntimePlan::parse(&model(&[malformed])),
        Err(PlanError::Decode(Error::UvSetCount { .. }))
    ));
}

#[test]
fn typed_checked_input_preserves_unavailable_state_instead_of_claiming_zero_or_one() {
    let plan = RuntimePlan::checked(RuntimeInput {
        bones: vec![],
        helpers: vec![],
        pivots: Pivots(vec![]),
        geosets: vec![],
        geoset_animations: vec![],
        materials: vec![],
        textures: vec![],
        texture_animations: vec![],
        global_sequences: vec![],
    })
    .unwrap();
    assert!(plan.uv_set_counts.is_empty());
    assert_eq!(
        RuntimePlan::parse(&model(&[])).unwrap().uv_set_counts,
        Vec::<Option<u32>>::new()
    );
}

#[test]
fn typed_nonempty_input_keeps_unknown_for_every_geoset() {
    let bytes = model(&[data_with_sets(1)]);
    let parsed = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    let input = RuntimeInput {
        bones: vec![],
        helpers: vec![],
        pivots: Pivots(vec![]),
        geosets: (0..3)
            .map(|_| mdx_runtime::GeosetInput {
                vertex_count: 2,
                material_id: 0,
                matrix_joint_ids: vec![],
            })
            .collect(),
        geoset_animations: vec![],
        materials: parsed.materials(&bytes).unwrap(),
        textures: parsed.textures(&bytes).unwrap(),
        texture_animations: vec![],
        global_sequences: vec![],
    };
    assert_eq!(
        RuntimePlan::checked(input).unwrap().uv_set_counts,
        [None, None, None]
    );
}

#[test]
fn short_subblock_reports_consistent_remaining_span_lengths() {
    let mut data = base();
    block(&mut data, b"UVAS", 1, &[]);
    let expected_at = data.len();
    block(&mut data, b"UVBS", 2, &[0; 4]);
    assert!(
        matches!(mesh(data), Err(Error::Truncated { what: "geoset sub-block body", at, need: 24, have: 12 }) if at == expected_at)
    );
}

#[test]
fn txan_runtime_entry_uses_parser_metadata_without_a_guessed_parallel_array() {
    use pcx_gfx::mdx_uv_translation::UvTranslationPlan;
    let one = RuntimePlan::parse(&model(&[data_with_sets(1)])).unwrap();
    let translation = UvTranslationPlan::from_runtime(&one, [3, 11], "synthetic model").unwrap();
    assert_eq!(
        translation
            .sample(0, 7, 101)
            .unwrap()
            .unwrap()
            .apply([2.0, 3.0]),
        [2.0, 3.0]
    );
    let multiple = RuntimePlan::parse(&model(&[data_with_sets(2)])).unwrap();
    assert!(
        UvTranslationPlan::from_runtime(&multiple, [3, 11], "synthetic model")
            .unwrap_err()
            .reason
            .contains("multiple UV sets")
    );
    let zero = RuntimePlan::parse(&model(&[data_with_sets(0)])).unwrap();
    assert!(
        UvTranslationPlan::from_runtime(&zero, [3, 11], "synthetic model")
            .unwrap_err()
            .reason
            .contains("missing authored UV")
    );
    let mut unavailable = one;
    unavailable.uv_set_counts = vec![None];
    assert!(
        UvTranslationPlan::from_runtime(&unavailable, [3, 11], "synthetic model")
            .unwrap_err()
            .reason
            .contains("metadata unavailable")
    );
    unavailable.uv_set_counts = vec![];
    assert!(
        UvTranslationPlan::from_runtime(&unavailable, [3, 11], "synthetic model")
            .unwrap_err()
            .reason
            .contains("metadata/GEOS count mismatch")
    );
}

#[test]
fn unknown_refusal_is_at_uv_binding_with_source_and_actual_geoset_context() {
    use pcx_gfx::mdx_uv_translation::UvTranslationPlan;
    let no_uv = RuntimePlan::parse(&model(&[base()])).unwrap();
    let error = UvTranslationPlan::from_runtime(&no_uv, [3, 11], "source-A").unwrap_err();
    assert_eq!(error.context, "source-A geoset 0");
    assert_eq!(error.reason, "UV-set metadata unavailable");
    let mixed =
        RuntimePlan::parse(&model(&[data_with_sets(1), base(), data_with_sets(1)])).unwrap();
    let error = UvTranslationPlan::from_runtime(&mixed, [3, 11], "source-B").unwrap_err();
    assert_eq!(error.context, "source-B geoset 1");
    assert_eq!(mixed.uv_set_counts, [Some(1), None, Some(1)]);
    let zero = RuntimePlan::parse(&model(&[data_with_sets(0)])).unwrap();
    let error = UvTranslationPlan::from_runtime(&zero, [3, 11], "source-C").unwrap_err();
    assert!(error.context.contains("source-C geoset 0"));
    assert!(error.reason.contains("missing authored UV coordinate set"));
}

#[test]
fn malformed_metadata_and_geoset_joins_are_refused_without_substitution() {
    use pcx_gfx::mdx_uv_translation::UvTranslationPlan;
    for metadata in [vec![], vec![Some(1), Some(1)]] {
        let mut plan = RuntimePlan::parse(&model(&[data_with_sets(1)])).unwrap();
        plan.uv_set_counts = metadata;
        let error = UvTranslationPlan::from_runtime(&plan, [3, 11], "source-D").unwrap_err();
        assert_eq!(error.context, "source-D");
        assert!(error.reason.contains("metadata/GEOS count mismatch"));
    }
    let mut plan = RuntimePlan::parse(&model(&[data_with_sets(1), data_with_sets(1)])).unwrap();
    plan.geosets[1].geoset_id = 0;
    let error = UvTranslationPlan::from_runtime(&plan, [3, 11], "source-E").unwrap_err();
    assert!(error.context.contains("source-E geoset 1"));
    assert!(error.reason.contains("inconsistent GEOS ordinal"));
}
