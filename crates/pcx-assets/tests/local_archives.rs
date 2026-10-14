//! Explicit local original input; contents remain in memory and are not saved.
use pcx_assets::{
    ArchiveLayer, CascSource, ContentSource, GameMode, MpqSource, NamedMember, Resolver,
};

#[test]
#[ignore = "requires PEACECRAFT3_ASSET_MPQ and PEACECRAFT3_ASSET_MEMBER"]
fn an_explicit_original_mpq_member_passes_through_the_asset_layer() {
    let path = std::env::var_os("PEACECRAFT3_ASSET_MPQ").expect("set original MPQ path");
    let member = std::env::var("PEACECRAFT3_ASSET_MEMBER").expect("set exact original member");
    let mut archive = pcx_mpq::MpqArchive::open(path).unwrap();
    let expected = archive.extract(&member).unwrap();
    let source = MpqSource::new(
        archive,
        "explicit-original".into(),
        None,
        vec![NamedMember {
            asset: "selected/asset.bin".into(),
            member: member.clone(),
        }],
        64 * 1024 * 1024,
    )
    .unwrap();
    let resolver = Resolver::new(
        GameMode::default(),
        None,
        vec![ArchiveLayer {
            priority: 1,
            source: Box::new(source),
        }],
    )
    .unwrap();
    let actual = resolver.resolve("selected/asset.bin").unwrap();
    assert_eq!(actual.bytes, expected);
    assert_eq!(actual.provenance.locator, member);
    assert_eq!(actual.provenance.byte_length, actual.bytes.len() as u64);
    eprintln!(
        "MPQ member={member:?} bytes={} source={:?}",
        actual.bytes.len(),
        actual.provenance.source
    );
}

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR and PEACECRAFT3_REFORGED_ASSET"]
fn an_explicit_tvfs_member_retains_verified_keys_and_content_through_layering() {
    let path = std::env::var_os("PEACECRAFT3_REFORGED_DIR").expect("set original CASC install");
    let member =
        std::env::var("PEACECRAFT3_REFORGED_ASSET").expect("set explicit stored TVFS path");
    let storage = pcx_casc::storage::Storage::open(
        std::path::Path::new(&path),
        pcx_casc::blte::Limits {
            encoded_bytes: 64 * 1024 * 1024,
            decoded_bytes: 64 * 1024 * 1024,
        },
    )
    .unwrap();
    let encoding = storage.load_encoding().unwrap();
    let tvfs = storage.load_tvfs(&encoding).unwrap();
    let expected = storage.read_path(&tvfs, &member).unwrap();
    let source = CascSource::new(
        storage,
        tvfs,
        "explicit-original".into(),
        vec![NamedMember {
            asset: "selected/asset.bin".into(),
            member: member.clone(),
        }],
    )
    .unwrap();
    let identity = source.identity("selected/asset.bin").unwrap().unwrap();
    assert_eq!(identity.ekey, expected.resolved.ekey);
    assert_eq!(identity.ckey, expected.resolved.ckey);
    assert_eq!(identity.links.len(), expected.resolved.links.len());
    assert_eq!(identity.build_config, expected.resolved.build_config);
    assert!(source.read("outside/inventory.bin").unwrap().is_none());
    eprintln!(
        "TVFS member={member:?} ekey={} ckey={} bytes={} build={:?}",
        pcx_casc::hex(&identity.ekey),
        pcx_casc::hex(&identity.ckey),
        identity.decoded_size,
        source.info().version
    );
    let resolver = Resolver::new(
        GameMode::default(),
        None,
        vec![ArchiveLayer {
            priority: 1,
            source: Box::new(source),
        }],
    )
    .unwrap();
    let actual = resolver.resolve("selected/asset.bin").unwrap();
    assert_eq!(actual.bytes, expected.file.decoded.bytes);
    let bounded = resolver
        .resolve_bounded("selected/asset.bin", expected.resolved.decoded_size)
        .unwrap();
    assert_eq!(bounded, actual);
    if let Some(limit) = expected.resolved.decoded_size.checked_sub(1) {
        assert!(matches!(
            resolver.resolve_bounded("selected/asset.bin", limit),
            Err(pcx_assets::Error::Source {
                source: pcx_assets::SourceError::CascRead(error),
                ..
            }) if error.kind == pcx_casc::Failure::Limit {
                needed: expected.resolved.decoded_size,
                limit,
            }
        ));
    }
    assert_eq!(
        actual.provenance.byte_length,
        expected.resolved.decoded_size
    );
    let locator: serde_json::Value = serde_json::from_str(&actual.provenance.locator).unwrap();
    assert_eq!(locator["tvfs"], member);
    assert_eq!(locator["ekey"], pcx_casc::hex(&expected.resolved.ekey));
    assert_eq!(locator["ckey"], pcx_casc::hex(&expected.resolved.ckey));
    assert_eq!(
        locator["manifests"].as_array().unwrap().len(),
        expected.resolved.links.len()
    );
}
