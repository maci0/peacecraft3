use crate::mpq_fixtures::*;
use pcx_assets::{ArchiveLayer, ContentSource, MpqSource, NamedMember, Resolver, SourceError};
use pcx_mpq::{
    Error, ExtractionBudget, ExtractionCause, ExtractionError, MpqArchive,
    checked::{CheckedArchive, ReadBudget, ReadError},
    flags,
    payload::PayloadError,
};
fn budget(packed: usize, decoded: usize) -> ExtractionBudget {
    ExtractionBudget {
        max_raw_packed_bytes: packed,
        max_decoded_bytes: decoded,
    }
}

fn cap(error: &ExtractionError, region: &str) {
    assert!(
        matches!(error.cause.as_ref(),ExtractionCause::Preflight(ReadError::BudgetExceeded{member,..}) if *member==region)
    );
}
#[cfg_attr(test, test)]
pub fn stored_compressed_encrypted_and_sector_cases() {
    let member = String::from("Authored\\日本語 Named.bin");
    let plain = b"caller bytes";
    for extra in [
        flags::SINGLE_UNIT,
        0,
        flags::SINGLE_UNIT | flags::ENCRYPTED | flags::FIX_KEY,
        flags::ENCRYPTED | flags::FIX_KEY,
    ] {
        let mut data = plain.to_vec();
        if extra & flags::ENCRYPTED != 0 {
            let key = pcx_mpq::payload::file_key(
                &member,
                40,
                plain.len() as u32,
                flags::EXISTING | extra,
            );
            encrypt(&mut data, key);
        }
        let mut a = archive(&member, &data, plain.len() as u32, extra);
        let legacy = a.extract(&member).unwrap();
        assert_eq!(
            a.extract_bounded(&member, budget(data.len(), plain.len()), None)
                .unwrap(),
            legacy
        );
        assert_eq!(legacy, plain);
    }
    let plain = vec![b'Q'; 7000];
    let packed = zlib(&plain);
    let mut a = archive(
        &member,
        &packed,
        plain.len() as u32,
        flags::SINGLE_UNIT | flags::COMPRESS,
    );
    assert_eq!(
        a.extract_bounded(&member, budget(packed.len(), plain.len()), None)
            .unwrap(),
        plain
    );
    let mut sectors = vec![0; 12];
    let mut end = 12usize;
    for (i, chunk) in plain.chunks(4096).enumerate() {
        sectors[i * 4..i * 4 + 4].copy_from_slice(&(end as u32).to_le_bytes());
        let p = zlib(chunk);
        sectors.extend(&p);
        end += p.len();
    }
    sectors[8..12].copy_from_slice(&(end as u32).to_le_bytes());
    let key = pcx_mpq::payload::file_key(
        &member,
        40,
        plain.len() as u32,
        flags::EXISTING | flags::COMPRESS | flags::ENCRYPTED | flags::FIX_KEY,
    );
    let offsets = [
        12usize,
        u32::from_le_bytes(sectors[4..8].try_into().unwrap()) as usize,
        end,
    ];
    for i in 0..2 {
        encrypt(
            &mut sectors[offsets[i]..offsets[i + 1]],
            key.wrapping_add(i as u32),
        );
    }
    encrypt(&mut sectors[..12], key.wrapping_sub(1));
    let mut a = archive(
        &member,
        &sectors,
        plain.len() as u32,
        flags::COMPRESS | flags::ENCRYPTED | flags::FIX_KEY,
    );
    assert_eq!(
        a.extract_bounded(&member, budget(sectors.len(), plain.len()), None)
            .unwrap(),
        plain
    );
    let mut empty = archive(&member, &[], 0, flags::SINGLE_UNIT);
    assert!(
        empty
            .extract_bounded(&member, budget(0, 0), None)
            .unwrap()
            .is_empty()
    );
}
#[cfg_attr(test, test)]
pub fn budgets_forged_lengths_and_metadata_have_typed_preflight_refusals() {
    let member = "Bounded\\member.bin";
    for (packed, decoded, region) in [(7, 8, "packed member"), (8, 7, "decoded member")] {
        let mut a = archive(member, b"12345678", 8, flags::SINGLE_UNIT);
        let e = a
            .extract_bounded(member, budget(packed, decoded), None)
            .unwrap_err();
        cap(&e, region);
        assert_eq!(e.member, member);
        assert_eq!(e.archive.to_str(), Some("caller-owned synthetic.mpq"));
    }
    for (packed, decoded, region) in [
        (u32::MAX, 8, "packed member"),
        (8, u32::MAX, "decoded member"),
    ] {
        let bytes = raw(
            member,
            b"12345678",
            block(40, packed, decoded, flags::SINGLE_UNIT),
            &[0],
            0,
        );
        let mut a = MpqArchive::from_bytes(bytes, "forged source").unwrap();
        cap(
            &a.extract_bounded(member, budget(8, 8), None).unwrap_err(),
            region,
        );
    }
    for position in [u32::MAX, 1000] {
        let bytes = raw(
            member,
            b"12345678",
            block(position, 8, 8, flags::SINGLE_UNIT),
            &[0],
            0,
        );
        let mut a = MpqArchive::from_bytes(bytes, "bad span").unwrap();
        assert!(matches!(
            a.extract_bounded(member, budget(8, 8), None)
                .unwrap_err()
                .cause
                .as_ref(),
            ExtractionCause::Mpq(Error::Unsupported(
                "packed member extends past the declared archive"
            ))
        ));
    }
    for kind in 0..4 {
        let mut a = archive(member, b"12345678", 8, flags::SINGLE_UNIT);
        match kind {
            0 => a.header.sector_size_shift = 32,
            1 => a.header.hash_entries += 1,
            2 => a.header.format_version = 1,
            _ => a.header.hash_table_pos = 0,
        };
        assert!(matches!(
            a.extract_bounded(member, budget(8, 8), None)
                .unwrap_err()
                .cause
                .as_ref(),
            ExtractionCause::Mpq(Error::Unsupported(_))
        ));
    }
    if cfg!(target_pointer_width = "32") {
        let bytes = raw(
            member,
            b"x",
            block(40, 1, u32::MAX, flags::SINGLE_UNIT),
            &[0],
            0,
        );
        let mut a = MpqArchive::from_bytes(bytes, "layout").unwrap();
        assert!(matches!(
            a.extract_bounded(member, budget(1, usize::MAX), None)
                .unwrap_err()
                .cause
                .as_ref(),
            ExtractionCause::Preflight(ReadError::InvalidLayout { .. })
        ));
    }
}
#[cfg_attr(test, test)]
pub fn strict_selection_is_shared_and_legacy_lookup_retained() {
    let member = "Dynamic\\ordinal.bin";
    let bytes = raw(
        member,
        b"abc",
        block(40, 3, 3, flags::SINGLE_UNIT),
        &[0, 1],
        0,
    );
    let mut a = MpqArchive::from_bytes(bytes.clone(), "ambiguous").unwrap();
    assert_eq!(a.extract(member).unwrap(), b"abc");
    assert!(matches!(
        a.extract_bounded(member, budget(3, 3), None)
            .unwrap_err()
            .cause
            .as_ref(),
        ExtractionCause::Ambiguous { count: 2 }
    ));
    let v = CheckedArchive::parse(
        &bytes,
        "same ambiguous",
        ReadBudget {
            max_archive_bytes: bytes.len(),
            max_archive_table_bytes: 48,
            max_raw_packed_bytes: 3,
            max_decoded_bytes: 3,
        },
    )
    .unwrap();
    assert!(matches!(
        v.selected_block(member),
        Err(ReadError::AmbiguousMember { count: 2, .. })
    ));
    let bytes = raw(
        member,
        b"abc",
        block(40, 3, 3, flags::SINGLE_UNIT),
        &[0, 0],
        0,
    );
    let mut a = MpqArchive::from_bytes(bytes, "identical").unwrap();
    assert_eq!(
        a.extract_bounded(member, budget(3, 3), None).unwrap(),
        b"abc"
    );
    let bytes = raw(member, b"abc", block(40, 3, 3, flags::SINGLE_UNIT), &[0], 1);
    let mut a = MpqArchive::from_bytes(bytes, "bad index").unwrap();
    assert!(matches!(
        a.extract_bounded(member, budget(3, 3), None)
            .unwrap_err()
            .cause
            .as_ref(),
        ExtractionCause::Mpq(Error::BadBlockIndex { index: 1, count: 1 })
    ));
    let mut a = archive(member, b"abc", 3, flags::SINGLE_UNIT);
    assert!(
        matches!(a.extract_bounded("absent",budget(3,3),None).unwrap_err().cause.as_ref(),ExtractionCause::Mpq(Error::NotFound(name)) if name=="absent")
    );
}
#[cfg_attr(test, test)]
pub fn malformed_codecs_and_dictionary_boundaries_stay_typed() {
    let member = "Bad\\codec.bin";
    for mask in [0x20, 0x10, 0x12, 1] {
        let mut a = archive(member, &[mask, 0], 9, flags::SINGLE_UNIT | flags::COMPRESS);
        let e = a.extract_bounded(member, budget(2, 9), None).unwrap_err();
        assert!(
            matches!(e.cause.as_ref(),ExtractionCause::Mpq(Error::Payload(PayloadError::UnsupportedCompression{mask:m})) if *m==mask)
        );
    }
    let packed = [0, 4, 0x82, 0x24, 0x25, 0x8f, 0x80, 0x7f];
    let mut a = archive(member, &packed, 13, flags::SINGLE_UNIT | flags::IMPLODE);
    assert_eq!(
        a.extract_bounded(member, budget(8, 13), None).unwrap(),
        b"AIAIAIAIAIAIA"
    );
    for (literal, width) in [(1, 4), (0, 3), (0, 7)] {
        let mut packed = packed;
        packed[0] = literal;
        packed[1] = width;
        let mut a = archive(member, &packed, 13, flags::SINGLE_UNIT | flags::IMPLODE);
        assert!(matches!(
            a.extract_bounded(member, budget(8, 13), None)
                .unwrap_err()
                .cause
                .as_ref(),
            ExtractionCause::Mpq(Error::Payload(PayloadError::Pkware))
        ));
    }
    let mut packed = Vec::new();
    packed.extend(8u32.to_le_bytes());
    packed.extend(99u32.to_le_bytes());
    packed.extend([2, 0]);
    let mut a = archive(member, &packed, 12, flags::COMPRESS);
    assert!(matches!(
        a.extract_bounded(member, budget(packed.len(), 12), None)
            .unwrap_err()
            .cause
            .as_ref(),
        ExtractionCause::Mpq(Error::Payload(PayloadError::SectorOutOfRange { .. }))
    ));
}
#[cfg_attr(test, test)]
pub fn huffman_is_only_explicit_input_and_owned_source_context_survives() {
    let member = "Weights\\authored.bin";
    let packed = [1, 0, 0x82, 0];
    let mut a = archive(member, &packed, 2, flags::SINGLE_UNIT | flags::COMPRESS);
    a.set_huffman_table(&synthetic_pe(), "legacy installed synthetic.dll")
        .unwrap();
    assert!(matches!(
        a.extract_bounded(member, budget(4, 2), None)
            .unwrap_err()
            .cause
            .as_ref(),
        ExtractionCause::Mpq(Error::Payload(PayloadError::UnsupportedCompression {
            mask: 1
        }))
    ));
    let table = table();
    assert_eq!(table.source.to_str(), Some("caller synthetic weights.dll"));
    assert_eq!(
        a.extract_bounded(member, budget(4, 2), Some(&table))
            .unwrap(),
        b"AA"
    );
    assert_eq!(a.extract(member).unwrap(), b"AA");
}
#[cfg_attr(test, test)]
pub fn actual_mpq_source_and_resolver_enforce_caller_and_mount_policies() {
    let member = String::from("Raw\\Caller spelling.bin");
    let packed = zlib(&[b'S'; 128]);
    let mk = |cap, id: &str| {
        MpqSource::new_bounded(
            archive(&member, &packed, 128, flags::SINGLE_UNIT | flags::COMPRESS),
            id.into(),
            Some("caller version".into()),
            vec![NamedMember {
                asset: "logical/selected.bin".into(),
                member: member.clone(),
            }],
            budget(cap, 128),
            None,
        )
        .unwrap()
    };
    let s = mk(packed.len(), "selected");
    let info = s.info().clone();
    let r = Resolver::new(
        "1.29b-tft-hd-vanilla".parse().unwrap(),
        None,
        vec![ArchiveLayer {
            priority: 17,
            source: Box::new(s),
        }],
    )
    .unwrap();
    let a = r.resolve_bounded("Logical\\Selected.BIN", 128).unwrap();
    assert_eq!(a.bytes, vec![b'S'; 128]);
    assert_eq!(a.provenance.source, info);
    assert_eq!(a.provenance.archive_priority, Some(17));
    assert_eq!(a.provenance.locator, member);
    assert_eq!(a.provenance.byte_length, 128);
    {
        let (resolver, limit, region) = (&r, 127, "decoded member");
        let e = resolver
            .resolve_bounded("logical/selected.bin", limit)
            .unwrap_err();
        match e {
            pcx_assets::Error::Source {
                layer,
                path: Some(path),
                source: SourceError::MpqExtraction(e),
            } => {
                assert_eq!(layer, "selected");
                assert_eq!(path.as_str(), "logical/selected.bin");
                cap(&e, region);
                assert_eq!(e.member, member);
            }
            other => panic!("{other:?}"),
        };
    }
    let r = Resolver::new(
        "1.29b-tft-hd-vanilla".parse().unwrap(),
        None,
        vec![
            ArchiveLayer {
                priority: 17,
                source: Box::new(mk(packed.len() - 1, "selected")),
            },
            ArchiveLayer {
                priority: 3,
                source: Box::new(mk(packed.len(), "lower")),
            },
        ],
    )
    .unwrap();
    match r.resolve_bounded("logical/selected.bin", 128).unwrap_err() {
        pcx_assets::Error::Source {
            source: SourceError::MpqExtraction(e),
            ..
        } => cap(&e, "packed member"),
        other => panic!("{other:?}"),
    };
    let old = MpqSource::new(
        archive(&member, b"ok", 2, flags::SINGLE_UNIT),
        "legacy".into(),
        None,
        vec![NamedMember {
            asset: "a".into(),
            member: member.clone(),
        }],
        2,
    )
    .unwrap();
    assert!(matches!(
        old.read_bounded("a", 2),
        Err(SourceError::BudgetUnsupported { limit: 2 })
    ));
    assert_eq!(old.read("a").unwrap().unwrap().bytes, b"ok");
    let source = MpqSource::new_bounded(
        archive(
            &member,
            &[1, 0, 0x82, 0],
            2,
            flags::SINGLE_UNIT | flags::COMPRESS,
        ),
        "weighted source".into(),
        None,
        vec![NamedMember {
            asset: "weighted".into(),
            member,
        }],
        budget(4, 2),
        Some(table()),
    )
    .unwrap();
    assert_eq!(
        source.read_bounded("weighted", 2).unwrap().unwrap().bytes,
        b"AA"
    );
    assert!(source.read_bounded("not mounted", 2).unwrap().is_none());
}
