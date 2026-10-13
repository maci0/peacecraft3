use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use blte::Limits;
use index::Index;
use storage::Storage;

fn provenance(member: &str) -> Provenance {
    Provenance {
        install: "synthetic".into(),
        version: "fixture-v1".into(),
        product: "fixture".into(),
        build_key: None,
        member: member.into(),
    }
}

fn limits() -> Limits {
    Limits {
        encoded_bytes: 1024 * 1024,
        decoded_bytes: 1024 * 1024,
    }
}

fn index_bytes(
    bucket: u8,
    keys: &[(Vec<u8>, u64, u32)],
    key_width: u8,
    segment_bits: u8,
) -> Vec<u8> {
    let mut header = vec![7, 0, bucket, 0, 4, 5, key_width, segment_bits];
    header.extend_from_slice(&0xabcdefu64.to_le_bytes());
    let mut data = Vec::new();
    data.extend_from_slice(&(header.len() as u32).to_le_bytes());
    data.extend_from_slice(&jenkins::hash(&header, 0, 0).0.to_le_bytes());
    data.extend_from_slice(&header);
    data.extend_from_slice(&[0; 8]);
    let mut table = Vec::new();
    let mut checksum = (0, 0);
    for (key, offset, size) in keys {
        let mut entry = key.clone();
        entry.extend_from_slice(&offset.to_be_bytes()[3..]);
        entry.extend_from_slice(&size.to_le_bytes());
        checksum = jenkins::hash(&entry, checksum.0, checksum.1);
        table.extend_from_slice(&entry);
    }
    data.extend_from_slice(&(table.len() as u32).to_le_bytes());
    data.extend_from_slice(&checksum.0.to_le_bytes());
    data.extend_from_slice(&table);
    data.extend_from_slice(&[0; 13]);
    data
}

fn compressed(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

fn multi_blte(chunks: &[(u8, &[u8])]) -> (Vec<u8>, Key) {
    let size = 12 + chunks.len() * 24;
    let mut header = b"BLTE".to_vec();
    header.extend_from_slice(&(size as u32).to_be_bytes());
    header.push(0x0f);
    header.extend_from_slice(&(chunks.len() as u32).to_be_bytes()[1..]);
    let mut data = Vec::new();
    for &(tag, content) in chunks {
        let mut raw = vec![tag];
        raw.extend(if tag == b'Z' {
            compressed(content)
        } else {
            content.to_vec()
        });
        header.extend_from_slice(&(raw.len() as u32).to_be_bytes());
        header.extend_from_slice(&(content.len() as u32).to_be_bytes());
        header.extend_from_slice(&md5::compute(&raw).0);
        data.extend_from_slice(&raw);
    }
    let key = md5::compute(&header).0;
    header.extend_from_slice(&data);
    (header, key)
}

fn single_blte(raw: &[u8]) -> (Vec<u8>, Key) {
    let mut data = b"BLTE\0\0\0\0".to_vec();
    data.extend_from_slice(raw);
    let key = md5::compute(&data).0;
    (data, key)
}

fn encoding_bytes(records: &[(Key, Key, u32)]) -> Vec<u8> {
    let mut page = vec![0; 1024];
    for (i, (ckey, ekey, size)) in records.iter().enumerate() {
        let offset = i * 38;
        page[offset..offset + 2].copy_from_slice(&1u16.to_le_bytes());
        page[offset + 2..offset + 6].copy_from_slice(&size.to_be_bytes());
        page[offset + 6..offset + 22].copy_from_slice(ckey);
        page[offset + 22..offset + 38].copy_from_slice(ekey);
    }
    let mut epage = vec![0; 1024];
    epage[..16].copy_from_slice(&records[0].1);
    let mut data = b"EN\x01\x10\x10".to_vec();
    data.extend_from_slice(&1u16.to_be_bytes());
    data.extend_from_slice(&1u16.to_be_bytes());
    data.extend_from_slice(&1u32.to_be_bytes());
    data.extend_from_slice(&1u32.to_be_bytes());
    data.push(0);
    data.extend_from_slice(&3u32.to_be_bytes());
    data.extend_from_slice(b"n\0\0");
    data.extend_from_slice(&records[0].0);
    data.extend_from_slice(&md5::compute(&page).0);
    data.extend_from_slice(&page);
    data.extend_from_slice(&records[0].1);
    data.extend_from_slice(&md5::compute(&epage).0);
    data.extend_from_slice(&epage);
    data.extend_from_slice(b"opaque synthetic tail");
    data
}

#[test]
fn index_counts_offsets_endianness_and_checksums_are_checked() {
    let raw = index_bytes(3, &[(vec![2, 4, 6], (11 << 27) | 123, 0x123456)], 3, 27);
    let index = Index::parse(&raw, provenance("03.idx")).unwrap();
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.header.key_bytes, 3);
    assert_eq!(index.entries[0].segment, 11);
    assert_eq!(index.entries[0].offset, 123);
    assert_eq!(index.entries[0].encoded_size, 0x123456);
    assert_eq!(index.zero_tail_bytes, 13);
    assert_eq!(index.evidence.md5, md5::compute(&raw).0);
    assert!(matches!(
        index.max_offset_semantics().kind,
        Failure::Unsupported { .. }
    ));
    for position in [9, 40] {
        let mut corrupted = raw.clone();
        corrupted[position] ^= 1;
        assert!(matches!(
            Index::parse(&corrupted, provenance("bad.idx"))
                .unwrap_err()
                .kind,
            Failure::Checksum { .. }
        ));
    }
    for end in [0, 7, 15, 31, 39, raw.len() - 14] {
        assert!(Index::parse(&raw[..end], provenance("cut.idx")).is_err());
    }
    let mut tail = raw.clone();
    *tail.last_mut().unwrap() = 1;
    assert!(matches!(
        Index::parse(&tail, provenance("tail.idx"))
            .unwrap_err()
            .kind,
        Failure::Unsupported { .. }
    ));
    let mut invalid = raw.clone();
    invalid[32..36].copy_from_slice(&13u32.to_le_bytes());
    assert!(matches!(
        Index::parse(&invalid, provenance("width.idx"))
            .unwrap_err()
            .kind,
        Failure::Malformed(_)
    ));
}

#[test]
fn unknown_index_layouts_are_typed_refusals() {
    for (position, value) in [(8, 5), (11, 1), (12, 0), (15, 40)] {
        let mut raw = index_bytes(0, &[], 9, 30);
        raw[position] = value;
        let checksum = jenkins::hash(&raw[8..24], 0, 0).0;
        raw[4..8].copy_from_slice(&checksum.to_le_bytes());
        assert!(Index::parse(&raw, provenance("unknown.idx")).is_err());
    }
    assert_eq!(jenkins::hash(b"", 0, 0), (0xdead_beef, 0xdead_beef));
}

#[test]
fn blte_plain_zlib_multichunk_hashes_and_limits() {
    let (raw, key) = multi_blte(&[(b'N', b"synthetic first"), (b'Z', b"synthetic second")]);
    let result = blte::decode(&raw, key, provenance("multi"), limits()).unwrap();
    assert_eq!(result.bytes, b"synthetic firstsynthetic second");
    assert_eq!(result.chunks.len(), 2);
    assert_eq!(result.content_key, md5::compute(&result.bytes).0);
    assert_eq!(result.chunks[1].mode, blte::Mode::Zlib);
    let mut corrupted = raw.clone();
    *corrupted.last_mut().unwrap() ^= 1;
    assert!(matches!(
        blte::decode(&corrupted, key, provenance("bad"), limits())
            .unwrap_err()
            .kind,
        Failure::Checksum { .. }
    ));
    let limited = Limits {
        encoded_bytes: 1000,
        decoded_bytes: 2,
    };
    assert!(matches!(
        blte::decode(&raw, key, provenance("limit"), limited)
            .unwrap_err()
            .kind,
        Failure::Limit { .. }
    ));
    let (raw, key) = single_blte(b"Nfixture");
    assert_eq!(
        blte::decode(&raw, key, provenance("single"), limits())
            .unwrap()
            .bytes,
        b"fixture"
    );
    let (raw, key) = single_blte(b"Eencrypted fixture");
    assert!(matches!(
        blte::decode(&raw, key, provenance("encrypted"), limits())
            .unwrap_err()
            .kind,
        Failure::Unsupported { .. }
    ));
}

#[test]
fn truncated_zlib_trailing_stream_and_trailing_blte_are_errors() {
    let mut packed = compressed(b"synthetic payload");
    packed.truncate(packed.len() - 2);
    let mut raw = vec![b'Z'];
    raw.extend(packed);
    let (encoded, key) = single_blte(&raw);
    assert!(blte::decode(&encoded, key, provenance("truncated zlib"), limits()).is_err());
    let mut raw = vec![b'Z'];
    raw.extend(compressed(b"fixture"));
    raw.extend(b"garbage");
    let (encoded, key) = single_blte(&raw);
    assert!(blte::decode(&encoded, key, provenance("trailing zlib"), limits()).is_err());
    let (mut encoded, key) = multi_blte(&[(b'N', b"fixture")]);
    encoded.push(0);
    assert!(matches!(
        blte::decode(&encoded, key, provenance("trailing BLTE"), limits())
            .unwrap_err()
            .kind,
        Failure::Malformed(_)
    ));
    for size in 0..12 {
        assert!(blte::decode(&encoded[..size], key, provenance("cut"), limits()).is_err());
    }
}

#[test]
fn encoding_joins_exact_keys_and_checks_both_page_hashes() {
    let raw = encoding_bytes(&[([1; 16], [2; 16], 17)]);
    let en = encoding::Encoding::parse(&raw, provenance("encoding")).unwrap();
    assert_eq!(en.contents[&[1; 16]].decoded_size, 17);
    assert_eq!(en.contents[&[1; 16]].ekeys, [[2; 16]]);
    assert_eq!(en.ckey_hashes_verified, 1);
    assert_eq!(en.ekey_hashes_verified, 1);
    assert!(matches!(
        en.interpret_specs().kind,
        Failure::Unsupported { .. }
    ));
    for pos in [57, 1113] {
        let mut corrupted = raw.clone();
        corrupted[pos] ^= 1;
        assert!(encoding::Encoding::parse(&corrupted, provenance("bad encoding")).is_err());
    }
    assert!(encoding::Encoding::parse(&raw[..1024], provenance("cut encoding")).is_err());
    let mut bad = raw.clone();
    bad[3] = 12;
    assert!(matches!(
        encoding::Encoding::parse(&bad, provenance("bad width"))
            .unwrap_err()
            .kind,
        Failure::Unsupported { .. }
    ));
}

#[test]
fn build_info_and_config_refuse_ambiguity_and_malformed_fields() {
    let info =
        b"Active!DEC:1|Version!STRING:0|Product!STRING:0\n0|old|fixture\n1|fixture-v1|fixture\n";
    assert_eq!(
        storage::parse_build_info(info, "fixture").unwrap()["Version"],
        "fixture-v1"
    );
    assert!(storage::parse_build_info(b"Active!DEC:1\n1\n1\n", "fixture").is_err());
    assert!(storage::parse_build_info(b"Active!DEC:1\n2\n", "fixture").is_err());
    assert!(storage::parse_build_info(b"Active!DEC:1|Version!STRING:0\n1\n", "fixture").is_err());
    assert!(storage::parse_config(b"key = value\nkey = other", "fixture").is_err());
    assert!(parse_key("../../invalid", "fixture").is_err());
}

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn synthetic_local_storage_roundtrip_has_verified_provenance_and_path_resolution() {
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    let dir = Temp(std::env::temp_dir().join(format!(
        "pcx-casc-fixture-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir_all(dir.0.join("Data/data")).unwrap();
    let payload = b"synthetic item bytes";
    let (asset, asset_ekey) = multi_blte(&[(b'Z', payload)]);
    let asset_ckey = md5::compute(payload).0;
    let root = crate::tvfs::tests::fixture(b"fixture/item.bin", asset_ekey, payload.len() as u32);
    let root_ckey = md5::compute(&root).0;
    let (encoded_root, root_ekey) = multi_blte(&[(b'N', &root)]);
    let mut records = vec![
        (asset_ckey, asset_ekey, payload.len() as u32),
        (root_ckey, root_ekey, root.len() as u32),
    ];
    records.sort_by_key(|record| record.0);
    let en = encoding_bytes(&records);
    let en_ckey = md5::compute(&en).0;
    let (encoded_en, en_ekey) = multi_blte(&[(b'N', &en)]);
    let mut parts = Vec::new();
    let mut entries = Vec::new();
    for (data, key) in [
        (&asset, asset_ekey),
        (&encoded_en, en_ekey),
        (&encoded_root, root_ekey),
    ] {
        let offset = parts.len() as u64;
        let mut reversed = key;
        reversed.reverse();
        let length = (data.len() + 30) as u32;
        if key == asset_ekey {
            reversed[..7].fill(0);
        }
        parts.extend_from_slice(&reversed);
        parts.extend_from_slice(&length.to_le_bytes());
        parts.extend_from_slice(&[0; 10]);
        parts.extend_from_slice(data);
        entries.push((key[..9].to_vec(), offset, length));
    }
    std::fs::write(dir.0.join("Data/data/data.000"), parts).unwrap();
    for bucket in 0..16 {
        std::fs::write(
            dir.0.join(format!("Data/data/{bucket:02x}00000001.idx")),
            index_bytes(bucket, if bucket == 0 { &entries } else { &[] }, 9, 30),
        )
        .unwrap();
    }
    let config = format!(
        "encoding = {} {}\nencoding-size = {} {}\nvfs-root = {} {}\nvfs-root-size = {} {}\n",
        hex(&en_ckey),
        hex(&en_ekey),
        en.len(),
        encoded_en.len(),
        hex(&root_ckey),
        hex(&root_ekey),
        root.len(),
        encoded_root.len()
    );
    let build_key = md5::compute(&config).0;
    let key = hex(&build_key);
    let folder = dir.0.join("Data/config").join(&key[..2]).join(&key[2..4]);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(&key), &config).unwrap();
    std::fs::write(dir.0.join(".build.info"),format!("Active!DEC:1|Build Key!HEX:16|Version!STRING:0|Product!STRING:0\n1|{key}|fixture-v1|fixture\n")).unwrap();
    let storage = Storage::open(&dir.0, limits()).unwrap();
    let encoding = storage.load_encoding().unwrap();
    let result = storage.read_content(&encoding, asset_ckey).unwrap();
    assert_eq!(result.decoded.bytes, payload);
    assert!(matches!(
        result.envelope_key_semantics().unwrap_err().kind,
        Failure::Unsupported {
            feature: "local envelope key claim",
            ..
        }
    ));
    assert_eq!(result.decoded.evidence.source.version, "fixture-v1");
    assert!(matches!(
        result.span_fields_semantics().kind,
        Failure::Unsupported { .. }
    ));
    let resolver = storage.load_tvfs(&encoding).unwrap();
    assert_eq!(
        storage.path_to_ekey(&encoding, "fixture/item.bin").unwrap(),
        asset_ekey
    );
    assert_eq!(
        storage
            .read_path(&resolver, "fixture/item.bin")
            .unwrap()
            .file
            .decoded
            .bytes,
        payload
    );
    assert!(matches!(
        storage.read_ekey([0xff; 16]).unwrap_err().kind,
        Failure::NotFound(_)
    ));
    assert_eq!(storage.data_part_sizes.len(), 1);
    let mut conflicting_alias = storage::Storage::open(&dir.0, limits()).unwrap();
    let root_pair = conflicting_alias.build.config_values["vfs-root"].clone();
    conflicting_alias
        .build
        .config_values
        .insert("vfs-1".into(), root_pair);
    conflicting_alias
        .build
        .config_values
        .insert("vfs-1-size".into(), vec!["1".into(), "1".into()]);
    assert!(conflicting_alias.load_tvfs(&encoding).is_err());
    let mut wrong_encoding = encoding;
    wrong_encoding.evidence.source.build_key = Some([0; 16]);
    assert!(storage.read_content(&wrong_encoding, asset_ckey).is_err());
    assert_eq!(
        storage.build.config_values.get("encoding").unwrap().len(),
        2
    );
    assert_ne!(storage.build.config_values, BTreeMap::new());
}
