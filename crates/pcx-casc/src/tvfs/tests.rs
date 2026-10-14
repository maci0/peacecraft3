use super::*;

fn source() -> Provenance {
    Provenance {
        install: "fixture".into(),
        version: "fixture-v1".into(),
        product: "fixture".into(),
        build_key: None,
        member: "synthetic.tvfs".into(),
    }
}

pub(crate) fn fixture(path: &[u8], key: Key, size: u32) -> Vec<u8> {
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
    path.extend_from_slice(&(FOLDER | (children.len() as u32 + 4)).to_be_bytes());
    path.extend(children);
    let mut data = b"TVFS".to_vec();
    data.extend_from_slice(&[1, 38, 9, 9]);
    data.extend_from_slice(&LOWERCASE.to_be_bytes());
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

#[test]
fn checked_header_trie_and_span_references() {
    let data = fixture(b"fixture/item.bin", [5; 16], 17);
    let manifest = Manifest::parse(&data, source(), 1024 * 1024).unwrap();
    assert_eq!(manifest.entries.len(), 1);
    assert!(matches!(
        Manifest::parse(&data, source(), 2).unwrap_err().kind,
        Failure::Limit { .. }
    ));
    assert_eq!(manifest.observed_depth, 1);
    assert_eq!(manifest.header.flags, LOWERCASE);
    let entry = &manifest.entries[b"fixture/item.bin".as_slice()];
    let FileRecord::Spans(spans) = &entry.record else {
        panic!("supported synthetic span")
    };
    assert_eq!(spans[0].key_prefix, vec![5; 9]);
    assert_eq!(spans[0].size, 17);
    assert_eq!(manifest.cft_record(&spans[0]).unwrap(), &[5; 9]);
    assert!(matches!(
        manifest.control_semantics().kind,
        Failure::Unsupported { .. }
    ));
    for end in [0, 3, 37, 38, data.len() - 1] {
        assert!(Manifest::parse(&data[..end], source(), 1024 * 1024).is_err());
    }
    for (position, value) in [(4, 2), (5, 47), (6, 8), (8, 0x80)] {
        let mut bad = data.clone();
        bad[position] = value;
        assert!(matches!(
            Manifest::parse(&bad, source(), 1024 * 1024)
                .unwrap_err()
                .kind,
            Failure::Unsupported { .. }
        ));
    }
    let mut overlap = data.clone();
    overlap[20..24].copy_from_slice(&38u32.to_be_bytes());
    assert!(Manifest::parse(&overlap, source(), 1024 * 1024).is_err());
    let mut depth = data.clone();
    depth[36..38].copy_from_slice(&0u16.to_be_bytes());
    assert!(Manifest::parse(&depth, source(), 1024 * 1024).is_err());
    let mut extent = data.clone();
    extent[39..43].copy_from_slice(&(FOLDER | 0x7fffffff).to_be_bytes());
    assert!(Manifest::parse(&extent, source(), 1024 * 1024).is_err());
}

#[test]
fn fragments_separators_unicode_bytes_and_sibling_restore() {
    let mut children = vec![5];
    children.extend_from_slice(b"units");
    children.push(2);
    children.extend_from_slice(b"ab");
    children.push(0xff);
    let mut leaves = vec![1, b'c', 0xff];
    leaves.extend_from_slice(&0u32.to_be_bytes());
    leaves.extend_from_slice(&[1, b'd', 0xff]);
    leaves.extend_from_slice(&0u32.to_be_bytes());
    children.extend_from_slice(&(FOLDER | (leaves.len() as u32 + 4)).to_be_bytes());
    children.extend(leaves);
    let mut data = from_tables(children, vec![1, 0, 0, 0, 0, 0, 0, 0, 17, 0], vec![3; 9], 2);
    let manifest = Manifest::parse(&data, source(), 1024 * 1024).unwrap();
    assert!(manifest.entries.contains_key(b"units/abc".as_slice()));
    assert!(manifest.entries.contains_key(b"units/abd".as_slice()));
    // Fragments are opaque bytes; neither incomplete UTF-8 nor legacy bytes
    // acquire a replacement character or a guessed code page.
    data = fixture(b"name-\xad.bin", [4; 16], 19);
    assert!(
        Manifest::parse(&data, source(), 1024 * 1024)
            .unwrap()
            .entries
            .contains_key(b"name-\xad.bin".as_slice())
    );
    assert_eq!(offset_width(255), 1);
    assert_eq!(offset_width(256), 2);
    assert_eq!(offset_width(65536), 3);
}

#[test]
fn bad_leaf_offsets_counts_and_cft_extents_are_explicit() {
    let data = fixture(b"item", [7; 16], 17);
    let manifest = Manifest::parse(&data, source(), 1024 * 1024).unwrap();
    let vo = manifest.header.vfs_table.start;
    let mut count = data.clone();
    count[vo] = 255;
    let parsed = Manifest::parse(&count, source(), 1024 * 1024).unwrap();
    assert!(matches!(
        parsed.entries[b"item".as_slice()].record,
        FileRecord::Unsupported(_)
    ));
    let mut span = data.clone();
    span[vo] = 2;
    assert!(Manifest::parse(&span, source(), 1024 * 1024).is_err());
    let mut cft = data.clone();
    cft[vo + 9] = 1;
    assert!(Manifest::parse(&cft, source(), 1024 * 1024).is_err());
    let mut leaf = data.clone();
    let value = manifest.entries[b"item".as_slice()].node_offset + 1 + 4 + 1;
    leaf[value..value + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(Manifest::parse(&leaf, source(), 1024 * 1024).is_err());
}

fn resolver_fixture() -> Resolver {
    let root_key = [1; 16];
    let child_key = [2; 16];
    let asset_key = [3; 16];
    let root = Manifest::parse(&fixture(b"module", child_key, 77), source(), 1024 * 1024).unwrap();
    let child = Manifest::parse(
        &fixture(b"units/item.bin", asset_key, 19),
        source(),
        1024 * 1024,
    )
    .unwrap();
    let evidence = root.evidence.clone();
    Resolver {
        root: root_key,
        build_config: evidence.clone(),
        encoding: evidence,
        manifests: BTreeMap::from([
            (
                root_key,
                ConfiguredManifest {
                    config_fields: vec!["vfs-root".into()],
                    ekey: root_key,
                    ckey: [11; 16],
                    encoded_size: 0,
                    manifest: root,
                },
            ),
            (
                child_key,
                ConfiguredManifest {
                    config_fields: vec!["vfs-2".into()],
                    ekey: child_key,
                    ckey: [12; 16],
                    encoded_size: 0,
                    manifest: child,
                },
            ),
        ]),
        representations: BTreeMap::from([
            (
                child_key[..9].to_vec(),
                vec![Representation {
                    ekey: child_key,
                    ckey: [12; 16],
                    size: 77,
                }],
            ),
            (
                asset_key[..9].to_vec(),
                vec![Representation {
                    ekey: asset_key,
                    ckey: [13; 16],
                    size: 19,
                }],
            ),
        ]),
    }
}

#[test]
fn nested_links_provenance_and_refusals_never_pick_a_prefix_match() {
    let mut resolver = resolver_fixture();
    let resolved = resolver.resolve("MODULE:Units/Item.bin").unwrap();
    assert_eq!(resolved.ekey, [3; 16]);
    assert_eq!(resolved.ckey, [13; 16]);
    assert_eq!(resolved.decoded_size, 19);
    assert_eq!(resolved.links.len(), 2);
    assert_eq!(resolved.links[0].config_fields, ["vfs-root"]);
    assert!(matches!(
        resolver.resolve("units/item.bin").unwrap_err().kind,
        Failure::NotFound(_)
    ));
    assert!(matches!(
        resolver
            .resolve("module:units/item.bin:extra")
            .unwrap_err()
            .kind,
        Failure::Unsupported {
            feature: "unconfigured TVFS mount",
            ..
        }
    ));
    let candidates = resolver.representations.get_mut([3; 9].as_slice()).unwrap();
    candidates.push(candidates[0].clone());
    assert!(matches!(
        resolver.resolve("module:units/item.bin").unwrap_err().kind,
        Failure::Unsupported {
            feature: "ambiguous TVFS EKey prefix",
            ..
        }
    ));
    let candidates = resolver.representations.get_mut([3; 9].as_slice()).unwrap();
    candidates.pop();
    let configured = resolver.manifests.get_mut(&[2; 16]).unwrap();
    let FileRecord::Spans(spans) = &mut configured
        .manifest
        .entries
        .get_mut(b"units/item.bin".as_slice())
        .unwrap()
        .record
    else {
        unreachable!()
    };
    spans[0].file_offset = 1;
    assert!(matches!(
        resolver.resolve("module:units/item.bin").unwrap_err().kind,
        Failure::Unsupported {
            feature: "TVFS partial-span execution",
            ..
        }
    ));
}

#[test]
fn write_header_extension_and_full_key_width_are_bounded() {
    let original = fixture(b"item", [5; 16], 17);
    let mut full = original.clone();
    full[6] = 16;
    full[7] = 16;
    full[32..36].copy_from_slice(&16u32.to_be_bytes());
    full.extend_from_slice(&[5; 7]);
    let manifest = Manifest::parse(&full, source(), 1024 * 1024).unwrap();
    let FileRecord::Spans(spans) = &manifest.entries[b"item".as_slice()].record else {
        unreachable!()
    };
    assert_eq!(spans[0].key_prefix, vec![5; 16]);

    let mut extended = original[..38].to_vec();
    extended[5] = 46;
    extended[8..12].copy_from_slice(&0x0fu32.to_be_bytes());
    for offset in [12, 20, 28] {
        let value = u32::from_be_bytes(extended[offset..offset + 4].try_into().unwrap());
        extended[offset..offset + 4].copy_from_slice(&(value + 8).to_be_bytes());
    }
    extended.extend_from_slice(&((original.len() + 8) as u32).to_be_bytes());
    extended.extend_from_slice(&5u32.to_be_bytes());
    extended.extend_from_slice(&original[38..]);
    extended.extend_from_slice(b"test\0");
    let manifest = Manifest::parse(&extended, source(), 1024 * 1024).unwrap();
    assert_eq!(manifest.header.flags, 15);
    assert_eq!(manifest.header.espec_table.as_ref().unwrap().len(), 5);
    let mut invalid = extended;
    invalid[38..42].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(Manifest::parse(&invalid, source(), 1024 * 1024).is_err());
}
