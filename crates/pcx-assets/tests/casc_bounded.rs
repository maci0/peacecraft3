//! Public source integration over a complete, local synthetic CASC fixture.
#[path = "../../pcx-casc/src/jenkins.rs"]
mod index_checksum;
use pcx_casc::blte::Limits;
use std::io::Write;
fn limits() -> Limits {
    Limits {
        encoded_bytes: 1 << 20,
        decoded_bytes: 1 << 20,
    }
}
pub fn index_bytes(
    bucket: u8,
    keys: &[(Vec<u8>, u64, u32)],
    key_width: u8,
    segment_bits: u8,
) -> Vec<u8> {
    let mut header = vec![7, 0, bucket, 0, 4, 5, key_width, segment_bits];
    header.extend_from_slice(&0xabcdefu64.to_le_bytes());
    let mut data = Vec::new();
    data.extend_from_slice(&(header.len() as u32).to_le_bytes());
    data.extend_from_slice(&index_checksum::hash(&header, 0, 0).0.to_le_bytes());
    data.extend_from_slice(&header);
    data.extend_from_slice(&[0; 8]);
    let mut table = Vec::new();
    let mut checksum = (0, 0);
    for (key, offset, size) in keys {
        let mut entry = key.clone();
        entry.extend_from_slice(&offset.to_be_bytes()[3..]);
        entry.extend_from_slice(&size.to_le_bytes());
        checksum = index_checksum::hash(&entry, checksum.0, checksum.1);
        table.extend_from_slice(&entry);
    }
    data.extend_from_slice(&(table.len() as u32).to_le_bytes());
    data.extend_from_slice(&checksum.0.to_le_bytes());
    data.extend_from_slice(&table);
    data.extend_from_slice(&[0; 13]);
    data
}

pub fn compressed(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

pub fn multi_blte(chunks: &[(u8, &[u8])]) -> (Vec<u8>, Key) {
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

pub fn single_blte(raw: &[u8]) -> (Vec<u8>, Key) {
    let mut data = b"BLTE\0\0\0\0".to_vec();
    data.extend_from_slice(raw);
    let key = md5::compute(&data).0;
    (data, key)
}

pub fn encoding_bytes(records: &[(Key, Key, u32)]) -> Vec<u8> {
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

pub fn tvfs(path: &[u8], key: Key, size: u32) -> Vec<u8> {
    let mut node = vec![path.len() as u8];
    node.extend_from_slice(path);
    node.push(0xff);
    node.extend_from_slice(&0u32.to_be_bytes());
    from_tables(
        node,
        vec![
            1,
            0,
            0,
            0,
            0,
            (size >> 24) as u8,
            (size >> 16) as u8,
            (size >> 8) as u8,
            size as u8,
            0,
        ],
        key[..9].to_vec(),
        1,
    )
}

fn from_tables(children: Vec<u8>, vfs: Vec<u8>, cft: Vec<u8>, depth: u16) -> Vec<u8> {
    let mut path = vec![0xff];
    path.extend_from_slice(&(0x8000_0000 | (children.len() as u32 + 4)).to_be_bytes());
    path.extend(children);
    let mut data = b"TVFS".to_vec();
    data.extend_from_slice(&[1, 38, 9, 9]);
    data.extend_from_slice(&8u32.to_be_bytes());
    let mut offset = 38;
    for table in [&path, &vfs, &cft] {
        data.extend_from_slice(&(offset as u32).to_be_bytes());
        data.extend_from_slice(&(table.len() as u32).to_be_bytes());
        offset += table.len();
    }
    data.extend_from_slice(&depth.to_be_bytes());
    data.extend(path);
    data.extend(vfs);
    data.extend(cft);
    data
}

use pcx_assets::{ArchiveLayer, CascSource, ContentSource, NamedMember, Resolver};
use pcx_casc::{storage::Storage, *};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
pub struct Fixture {
    pub root: PathBuf,
    pub asset: Vec<u8>,
    pub ekey: Key,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
impl Fixture {
    pub fn new(payload: &[u8], framed: bool, tag: u8, declared: u32, wrong_ckey: bool) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::var_os("PCX_CASC_TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!(
                "synthetic-casc-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
        std::fs::create_dir_all(root.join("Data/data")).unwrap();
        let (asset, ekey) = if framed {
            multi_blte(&[(tag, payload)])
        } else {
            let mut raw = vec![tag];
            raw.extend(if tag == b'Z' {
                compressed(payload)
            } else {
                payload.to_vec()
            });
            single_blte(&raw)
        };
        let ckey = if wrong_ckey {
            md5::compute(b"different synthetic content").0
        } else {
            md5::compute(payload).0
        };
        let manifest = tvfs(b"caller/item.bin", ekey, declared);
        let manifest_ckey = md5::compute(&manifest).0;
        let (encoded_root, root_ekey) = multi_blte(&[(b'N', &manifest)]);
        let mut records = vec![
            (ckey, ekey, declared),
            (manifest_ckey, root_ekey, manifest.len() as u32),
        ];
        records.sort_by_key(|r| r.0);
        let en = encoding_bytes(&records);
        let en_ckey = md5::compute(&en).0;
        let (encoded_en, en_ekey) = multi_blte(&[(b'N', &en)]);
        let mut parts = Vec::new();
        let mut entries = Vec::new();
        for (data, key) in [
            (&asset, ekey),
            (&encoded_en, en_ekey),
            (&encoded_root, root_ekey),
        ] {
            let offset = parts.len() as u64;
            let mut reversed = key;
            reversed.reverse();
            let length = (data.len() + 30) as u32;
            parts.extend(reversed);
            parts.extend(length.to_le_bytes());
            parts.extend([0; 10]);
            parts.extend(data);
            entries.push((key[..9].to_vec(), offset, length));
        }
        std::fs::write(root.join("Data/data/data.000"), parts).unwrap();
        for bucket in 0..16 {
            std::fs::write(
                root.join(format!("Data/data/{bucket:02x}00000001.idx")),
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
            hex(&manifest_ckey),
            hex(&root_ekey),
            manifest.len(),
            encoded_root.len()
        );
        let key = hex(&md5::compute(&config).0);
        let dir = root.join("Data/config").join(&key[..2]).join(&key[2..4]);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(&key), config).unwrap();
        std::fs::write(root.join(".build.info"),format!("Active!DEC:1|Build Key!HEX:16|Version!STRING:0|Product!STRING:0\n1|{key}|fixture-v1|fixture\n")).unwrap();
        Self { root, asset, ekey }
    }
    pub fn mount(&self) -> (Storage, pcx_casc::tvfs::Resolver) {
        let storage = Storage::open(&self.root, limits()).unwrap();
        let encoding = storage.load_encoding().unwrap();
        let resolver = storage.load_tvfs(&encoding).unwrap();
        (storage, resolver)
    }
    pub fn source(&self) -> CascSource {
        self.source_named("selected CASC")
    }
    pub fn source_named(&self, id: &str) -> CascSource {
        let (s, r) = self.mount();
        CascSource::new(
            s,
            r,
            id.into(),
            vec![NamedMember {
                asset: "logical/selected.bin".into(),
                member: "Caller/Item.BIN".into(),
            }],
        )
        .unwrap()
    }
    pub fn resolver(&self) -> Resolver {
        Resolver::new(
            "1.29b-tft-hd-vanilla".parse().unwrap(),
            None,
            vec![ArchiveLayer {
                priority: 23,
                source: Box::new(self.source()),
            }],
        )
        .unwrap()
    }
    pub fn replace_asset(&self, asset: &[u8]) {
        assert_eq!(asset.len(), self.asset.len());
        let path = self.root.join("Data/data/data.000");
        let mut parts = std::fs::read(&path).unwrap();
        parts[30..30 + asset.len()].copy_from_slice(asset);
        std::fs::write(path, parts).unwrap();
    }
}

fn typed(error: pcx_assets::Error) -> Box<pcx_casc::Error> {
    match error {
        pcx_assets::Error::Source {
            source: pcx_assets::SourceError::CascRead(e),
            ..
        } => e,
        other => panic!("unexpected: {other:?}"),
    }
}
#[test]
fn exact_limits_retain_legacy_bytes_and_receipts() {
    for (tag, framed) in [(b'N', true), (b'Z', true), (b'N', false), (b'Z', false)] {
        let payload = vec![b'A'; 128];
        let f = Fixture::new(&payload, framed, tag, payload.len() as u32, false);
        let source = f.source();
        let identity = source
            .identity("logical/selected.bin")
            .unwrap()
            .unwrap()
            .clone();
        let resolver = f.resolver();
        let old = resolver.resolve("logical/selected.bin").unwrap();
        let bounded = resolver
            .resolve_bounded("Logical\\Selected.BIN", payload.len() as u64)
            .unwrap();
        assert_eq!(old, bounded);
        assert_eq!(bounded.bytes, payload);
        assert_eq!(bounded.provenance.archive_priority, Some(23));
        assert_eq!(bounded.provenance.source, source.info().clone());
        assert_eq!(
            bounded.provenance.source.version.as_deref(),
            Some("fixture-v1")
        );
        let locator: serde_json::Value = serde_json::from_str(&bounded.provenance.locator).unwrap();
        assert_eq!(locator["tvfs"], "Caller/Item.BIN");
        assert_eq!(locator["ekey"], hex(&identity.ekey));
        assert_eq!(locator["ckey"], hex(&identity.ckey));
        assert_eq!(locator["build_config"], hex(&identity.build_config.md5));
        assert_eq!(locator["encoding"], hex(&identity.encoding.md5));
        assert_eq!(
            locator["manifests"][0]["manifest_md5"],
            hex(&identity.links[0].manifest.md5)
        );
        assert_eq!(
            locator["archive"],
            f.root.join("Data/data/data.000").to_str().unwrap()
        );
    }
    let f = Fixture::new(&[], true, b'N', 0, false);
    let a = f
        .resolver()
        .resolve_bounded("logical/selected.bin", 0)
        .unwrap();
    assert!(a.bytes.is_empty());
    assert_eq!(a.provenance.byte_length, 0);
}
#[test]
fn declared_metadata_refuses_before_missing_backing_io() {
    let f = Fixture::new(&[b'B'; 128], true, b'Z', 128, false);
    let expected = f
        .source()
        .identity("logical/selected.bin")
        .unwrap()
        .unwrap()
        .clone();
    let resolver = f.resolver();
    std::fs::remove_file(f.root.join("Data/data/data.000")).unwrap();
    match resolver
        .resolve_bounded("Logical\\Selected.BIN", 127)
        .unwrap_err()
    {
        pcx_assets::Error::Source {
            layer,
            path: Some(path),
            source: pcx_assets::SourceError::CascRead(e),
        } => {
            assert_eq!(layer, "selected CASC");
            assert_eq!(path.as_str(), "logical/selected.bin");
            assert_eq!(
                e.kind,
                Failure::Limit {
                    needed: 128,
                    limit: 127
                }
            );
            assert!(e.source_name.contains(&expected.encoding.source.member));
            assert!(e.source_name.contains(&hex(b"Caller/Item.BIN")));
        }
        error => panic!("unexpected: {error:?}"),
    }
    let e = typed(
        resolver
            .resolve_bounded("logical/selected.bin", 128)
            .unwrap_err(),
    );
    assert!(matches!(e.kind, Failure::Io(_)));
    assert_eq!(
        e.source_name,
        f.root.join("Data/data/data.000").to_string_lossy()
    );
}
struct Counted {
    source: CascSource,
    reads: std::sync::Arc<AtomicUsize>,
}
impl pcx_assets::ContentSource for Counted {
    fn info(&self) -> &pcx_assets::SourceInfo {
        self.source.info()
    }
    fn paths(&self) -> Result<Vec<String>, pcx_assets::SourceError> {
        self.source.paths()
    }
    fn read(
        &self,
        path: &str,
    ) -> Result<Option<pcx_assets::SourceContent>, pcx_assets::SourceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.source.read(path)
    }
    fn read_bounded(
        &self,
        path: &str,
        limit: u64,
    ) -> Result<Option<pcx_assets::SourceContent>, pcx_assets::SourceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.source.read_bounded(path, limit)
    }
}
#[test]
fn oversized_winner_never_reads_available_lower_source() {
    let high = Fixture::new(&[b'H'; 129], true, b'Z', 129, false);
    let low = Fixture::new(b"lower", true, b'N', 5, false);
    let reads = std::sync::Arc::new(AtomicUsize::new(0));
    let low_source = low.source_named("lower CASC");
    assert_eq!(
        low_source
            .read_bounded("logical/selected.bin", 5)
            .unwrap()
            .unwrap()
            .bytes,
        b"lower"
    );
    let resolver = Resolver::new(
        "1.29b-tft-hd-vanilla".parse().unwrap(),
        None,
        vec![
            ArchiveLayer {
                priority: 23,
                source: Box::new(high.source_named("higher CASC")),
            },
            ArchiveLayer {
                priority: 3,
                source: Box::new(Counted {
                    source: low_source,
                    reads: reads.clone(),
                }),
            },
        ],
    )
    .unwrap();
    std::fs::remove_file(high.root.join("Data/data/data.000")).unwrap();
    match resolver
        .resolve_bounded("logical/selected.bin", 128)
        .unwrap_err()
    {
        pcx_assets::Error::Source {
            layer,
            source: pcx_assets::SourceError::CascRead(e),
            ..
        } => {
            assert_eq!(layer, "higher CASC");
            assert_eq!(
                e.kind,
                Failure::Limit {
                    needed: 129,
                    limit: 128
                }
            );
        }
        error => panic!("unexpected: {error:?}"),
    };
    assert_eq!(reads.load(Ordering::SeqCst), 0);
}
#[test]
fn integrity_failures_stay_typed_through_resolver() {
    let f = Fixture::new(&[b'D'; 128], true, b'Z', 128, false);
    let resolver = f.resolver();
    let mut corrupt = f.asset.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    f.replace_asset(&corrupt);
    assert!(matches!(
        typed(
            resolver
                .resolve_bounded("logical/selected.bin", 128)
                .unwrap_err()
        )
        .kind,
        Failure::Checksum { .. }
    ));
    let f = Fixture::new(&[b'E'; 128], false, b'N', 128, false);
    let resolver = f.resolver();
    let mut corrupt = f.asset.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    f.replace_asset(&corrupt);
    assert!(matches!(
        typed(
            resolver
                .resolve_bounded("logical/selected.bin", 128)
                .unwrap_err()
        )
        .kind,
        Failure::Checksum { .. }
    ));
    let f = Fixture::new(&[b'F'; 128], false, b'N', 128, true);
    assert!(matches!(
        typed(
            f.resolver()
                .resolve_bounded("logical/selected.bin", 128)
                .unwrap_err()
        )
        .kind,
        Failure::Checksum { .. }
    ));
}
#[test]
fn forged_decoded_size_limits_actual_decoder_output() {
    for framed in [true, false] {
        let payload = vec![b'G'; 9000];
        let f = Fixture::new(&payload, framed, b'Z', 2, false);
        let resolver = f.resolver();
        assert!(matches!(
            typed(
                resolver
                    .resolve_bounded("logical/selected.bin", payload.len() as u64)
                    .unwrap_err()
            )
            .kind,
            Failure::Limit { limit: 2, .. }
        ));
        assert!(matches!(
            resolver.resolve("logical/selected.bin"),
            Err(pcx_assets::Error::Source {
                source: pcx_assets::SourceError::InvalidInput { .. },
                ..
            })
        ));
    }
}
