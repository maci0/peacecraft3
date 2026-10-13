//! Explicit local fixture. No successful skips, CDN requests or asset writes.
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use pcx_casc::blte::{Limits, Mode};
use pcx_casc::storage::Storage;
use pcx_casc::{Failure, hex, parse_key};

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR pointing to the original local install"]
fn all_local_indexes_and_configured_manifests() {
    let install = PathBuf::from(
        std::env::var_os("PEACECRAFT3_REFORGED_DIR")
            .expect("set PEACECRAFT3_REFORGED_DIR; no original fixture is not a successful skip"),
    );
    // Caller resource policy, independent of authored sizes.
    let limits = Limits {
        encoded_bytes: 64 * 1024 * 1024,
        decoded_bytes: 256 * 1024 * 1024,
    };
    let store = Storage::open(&install, limits).unwrap();
    assert!(install.join("_retail_").is_dir());
    println!(
        "source={} version={} product={} build_info_bytes={} build_info_md5={} config_bytes={} config_md5={}",
        install.display(),
        store.build.info.source.version,
        store.build.info.source.product,
        store.build.info.size,
        hex(&store.build.info.md5),
        store.build.config.size,
        hex(&store.build.config.md5)
    );
    let mut unique = BTreeSet::new();
    let mut count = 0;
    for index in &store.indexes {
        for entry in &index.entries {
            assert!(
                unique.insert(entry.key_prefix.clone()),
                "duplicate key prefix"
            );
        }
        assert!(
            index
                .entries
                .windows(2)
                .all(|pair| pair[0].key_prefix < pair[1].key_prefix)
        );
        count += index.entries.len();
        println!(
            "idx={} bytes={} md5={} entries={} table_bytes={} header_checksum={:08x} table_checksum={:08x} tail_bytes={} widths={}/{}/{} segment_bits={} opaque_max={:#x}",
            index.evidence.source.member,
            index.evidence.size,
            hex(&index.evidence.md5),
            index.entries.len(),
            index.header.table_bytes,
            index.header.header_checksum,
            index.header.table_checksum,
            index.zero_tail_bytes,
            index.header.key_bytes,
            index.header.offset_bytes,
            index.header.size_bytes,
            index.header.segment_bits,
            index.header.opaque_max_offset
        );
        assert!(matches!(
            index.max_offset_semantics().kind,
            Failure::Unsupported { .. }
        ));
    }
    println!(
        "indexes={} entries={count} unique_prefixes={} data_parts={} data_part_bytes={} header_checksums_ok={} table_checksums_ok={} out_of_bounds=0",
        store.indexes.len(),
        unique.len(),
        store.data_part_sizes.len(),
        store.data_part_sizes.values().sum::<u64>(),
        store.indexes.len(),
        store.indexes.len()
    );
    let encoding = store.load_encoding().unwrap();
    println!(
        "encoding_bytes={} encoding_ckey={} records={} ckey_pages={} ekey_pages={} ckey_page_hashes_ok={} ekey_page_hashes_ok={} opaque_spec_bytes={} opaque_tail_bytes={}",
        encoding.evidence.size,
        hex(&encoding.evidence.md5),
        encoding.contents.len(),
        encoding.header.ckey_page_count,
        encoding.header.ekey_page_count,
        encoding.ckey_hashes_verified,
        encoding.ekey_hashes_verified,
        encoding.opaque_specs.len(),
        encoding.opaque_tail.len()
    );
    assert!(matches!(
        encoding.interpret_specs().kind,
        Failure::Unsupported { .. }
    ));
    let pairs: Vec<_> = store
        .build
        .config_values
        .iter()
        .filter(|(name, value)| {
            value.len() == 2
                && (name.as_str() == "encoding"
                    || name.as_str() == "vfs-root"
                    || name.starts_with("vfs-") && !name.ends_with("-size"))
        })
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    let mut decoded_files = 0;
    let mut manifest_keys = BTreeSet::new();
    let mut decoded_bytes = 0;
    let mut chunks: BTreeMap<&str, usize> = BTreeMap::new();
    for (name, pair) in pairs {
        let ckey = parse_key(&pair[0], &name).unwrap();
        let ekey = parse_key(&pair[1], &name).unwrap();
        manifest_keys.insert(ekey);
        let file = store
            .read_ekey(ekey)
            .unwrap_or_else(|err| panic!("{name}: {err}"));
        assert_eq!(file.decoded.content_key, ckey, "{name} configured CKey");
        store
            .verify_config_size(&format!("{name}-size"), &file)
            .unwrap();
        if name != "encoding" {
            assert_eq!(
                store.read_content(&encoding, ckey).unwrap().decoded.bytes,
                file.decoded.bytes
            );
        }
        for chunk in &file.decoded.chunks {
            *chunks
                .entry(match chunk.mode {
                    Mode::Plain => "N",
                    Mode::Zlib => "Z",
                })
                .or_default() += 1;
        }
        decoded_files += 1;
        decoded_bytes += file.decoded.bytes.len();
        println!(
            "manifest={name} decoded_bytes={} encoded_bytes={} chunks={} ckey_verified=true ekey_verified=true span_fields=opaque",
            file.decoded.bytes.len(),
            file.decoded.evidence.size,
            file.decoded.chunks.len()
        );
    }
    let root_ckey = parse_key(&store.build.config_values["root"][0], "root").unwrap();
    let root = store.read_content(&encoding, root_ckey).unwrap();
    if let Err(refusal) = root.envelope_key_semantics() {
        println!("root_envelope_refusal={refusal}");
    }
    println!(
        "content_root_bytes={} content_root_ckey={} signature={}",
        root.decoded.bytes.len(),
        hex(&root.decoded.content_key),
        hex(root.decoded.bytes.get(..4).unwrap_or(&root.decoded.bytes))
    );
    let refusal = store
        .path_to_ekey(&encoding, "synthetic/path-probe")
        .unwrap_err();
    assert!(matches!(
        refusal.kind,
        Failure::Unsupported {
            feature: "path to EKey",
            ..
        }
    ));
    println!("path_refusal={refusal}");
    println!(
        "verified_configured_manifests={decoded_files} unique_manifest_ekeys={} decoded_manifest_bytes={decoded_bytes} chunk_modes={chunks:?} root_content_verified=true path_mapping=unsupported espec_execution=unsupported full_store_codec_coverage=unproven",
        manifest_keys.len()
    );
}
