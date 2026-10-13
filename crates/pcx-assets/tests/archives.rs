use pcx_assets::{
    ArchiveLayer, ContentSource, GameMode, MpqSource, NamedMember, Resolver, SourceError,
};
use pcx_mpq::{MpqArchive, crypt, flags, hash};

fn encrypt(words: &mut [u8], mut key: u32) {
    let mut seed = 0xeeee_eeeeu32;
    for word in words.as_chunks_mut::<4>().0 {
        seed = seed.wrapping_add(crypt::table()[0x400 + (key & 0xff) as usize]);
        let plain = u32::from_le_bytes(*word);
        *word = (plain ^ key.wrapping_add(seed)).to_le_bytes();
        key = ((!key) << 21).wrapping_add(0x1111_1111) | (key >> 11);
        seed = plain
            .wrapping_add(seed)
            .wrapping_add(seed << 5)
            .wrapping_add(3);
    }
}

// One raw synthetic member, no listfile; this exercises the actual encrypted
// tables and named extraction rather than mocking ContentSource.
fn archive(data: &[u8], declared: u32, extra_flags: u32) -> MpqArchive {
    archive_locales(data, declared, extra_flags, &[0])
}

fn archive_locales(data: &[u8], declared: u32, extra_flags: u32, locales: &[u16]) -> MpqArchive {
    let member = r"Authored\Named.bin";
    let hash_at = 40 + data.len();
    let block_at = hash_at + locales.len() * 16;
    let mut bytes = vec![0; block_at + 16];
    bytes[..4].copy_from_slice(b"MPQ\x1a");
    for (at, value) in [
        (4, 32),
        (8, bytes.len() as u32),
        (12, 3 << 16),
        (16, hash_at as u32),
        (20, block_at as u32),
        (24, locales.len() as u32),
        (28, 1),
    ] {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[40..hash_at].copy_from_slice(data);
    let name = hash::hash_string(member);
    for (at, value) in [
        (block_at, 40),
        (block_at + 4, data.len() as u32),
        (block_at + 8, declared),
        (
            block_at + 12,
            flags::EXISTING | flags::SINGLE_UNIT | extra_flags,
        ),
    ] {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (index, locale) in locales.iter().enumerate() {
        let at = hash_at + index * 16;
        bytes[at..at + 4].copy_from_slice(&name.name1.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&name.name2.to_le_bytes());
        bytes[at + 8..at + 10].copy_from_slice(&locale.to_le_bytes());
    }
    encrypt(&mut bytes[hash_at..block_at], hash::hash_table_key());
    encrypt(&mut bytes[block_at..], hash::block_table_key());
    MpqArchive::from_bytes(bytes, "synthetic-source.mpq").unwrap()
}

fn mapping() -> NamedMember {
    NamedMember {
        asset: "logical/model.bin".into(),
        member: r"Authored\Named.bin".into(),
    }
}

#[test]
fn named_mpq_mapping_reads_real_tables_and_preserves_locator_and_length() {
    let source = MpqSource::new(
        archive(b"authored", 8, 0),
        "source".into(),
        None,
        vec![mapping()],
        8,
    )
    .unwrap();
    assert_eq!(source.paths().unwrap(), ["logical/model.bin"]);
    assert!(source.read("outside/inventory.bin").unwrap().is_none());
    let resolver = Resolver::new(
        GameMode::default(),
        None,
        vec![ArchiveLayer {
            priority: 7,
            source: Box::new(source),
        }],
    )
    .unwrap();
    let asset = resolver.resolve("LOGICAL\\MODEL.bin").unwrap();
    assert_eq!(asset.bytes, b"authored");
    assert_eq!(asset.provenance.locator, r"Authored\Named.bin");
    assert_eq!(asset.provenance.byte_length, 8);
    assert_eq!(asset.provenance.archive_priority, Some(7));
    assert_eq!(asset.provenance.source.version, None);
}

#[test]
fn a_declared_missing_member_or_ambiguous_logical_mapping_is_refused() {
    let missing = NamedMember {
        asset: "somewhere.bin".into(),
        member: "absent.bin".into(),
    };
    assert!(matches!(
        MpqSource::new(
            archive(b"ok", 2, 0),
            "source".into(),
            None,
            vec![missing],
            2
        ),
        Err(SourceError::InvalidInput { .. })
    ));
    let mut duplicate = mapping();
    duplicate.asset = "LOGICAL\\MODEL.BIN".into();
    assert!(matches!(
        MpqSource::new(
            archive(b"ok", 2, 0),
            "source".into(),
            None,
            vec![mapping(), duplicate],
            2
        ),
        Err(SourceError::Ambiguous { .. })
    ));
}

#[test]
fn unsupported_content_in_the_winning_archive_does_not_fall_back() {
    let top = MpqSource::new(
        archive(&[0x20, 0], 20, flags::COMPRESS),
        "unsupported".into(),
        None,
        vec![mapping()],
        20,
    )
    .unwrap();
    let bottom = MpqSource::new(
        archive(b"fallback", 8, 0),
        "other".into(),
        None,
        vec![mapping()],
        8,
    )
    .unwrap();
    let resolver = Resolver::new(
        GameMode::default(),
        None,
        vec![
            ArchiveLayer {
                priority: 2,
                source: Box::new(top),
            },
            ArchiveLayer {
                priority: 1,
                source: Box::new(bottom),
            },
        ],
    )
    .unwrap();
    let error = resolver
        .resolve("logical/model.bin")
        .unwrap_err()
        .to_string();
    assert!(error.contains("unsupported"), "{error}");
}

#[test]
fn decoded_budget_and_locale_ambiguity_are_checked_before_reading_content() {
    assert!(matches!(
        MpqSource::new(
            archive(b"authored", 8, 0),
            "source".into(),
            None,
            vec![mapping()],
            7
        ),
        Err(SourceError::Limit {
            requested: 8,
            limit: 7
        })
    ));
    assert!(matches!(
        MpqSource::new(
            archive_locales(b"authored", 8, 0, &[0, 1033]),
            "source".into(),
            None,
            vec![mapping()],
            8
        ),
        Err(SourceError::Ambiguous { .. })
    ));
}
