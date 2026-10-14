use pcx_gfx::{
    AssetKind,
    dds::{self, Failure, Format, Limits},
};

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn fixture(format: Format, width: u32, height: u32, levels: u32) -> Vec<u8> {
    let stride = if format == Format::Bc1 { 8 } else { 16 };
    let mut bytes = vec![0; 128];
    bytes[..4].copy_from_slice(b"DDS ");
    put(&mut bytes, 4, 124);
    put(&mut bytes, 8, if levels > 1 { 0xa1007 } else { 0x81007 });
    put(&mut bytes, 12, height);
    put(&mut bytes, 16, width);
    put(
        &mut bytes,
        20,
        width.div_ceil(4) * height.div_ceil(4) * stride,
    );
    put(&mut bytes, 28, levels);
    put(&mut bytes, 76, 32);
    put(&mut bytes, 80, 4);
    bytes[84..88].copy_from_slice(if format == Format::Bc1 {
        b"DXT1"
    } else {
        b"DXT5"
    });
    put(&mut bytes, 108, if levels > 1 { 0x401008 } else { 0x1000 });
    let (mut w, mut h) = (width, height);
    for _ in 0..levels {
        for _ in 0..w.div_ceil(4) * h.div_ceil(4) {
            if format == Format::Bc3 {
                bytes.extend_from_slice(&[255, 0, 0, 0, 0, 0, 0, 0]);
            }
            bytes.extend_from_slice(&[0, 248, 224, 7, 0, 0, 0, 0]);
        }
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    bytes
}
fn limits(bytes: &[u8]) -> Limits {
    Limits {
        max_encoded_bytes: bytes.len() as u64,
        max_rgba_bytes: 4096,
        max_working_bytes: 8192,
    }
}
fn error(bytes: &[u8]) -> dds::Error {
    dds::decode(bytes, "synthetic.dds", limits(bytes)).unwrap_err()
}

#[test]
fn bc1_four_colours_and_source_survive_the_official_decoder() {
    let mut bytes = fixture(Format::Bc1, 4, 4, 1);
    put(&mut bytes, 132, 0b11_10_01_00);
    assert_eq!(AssetKind::detect(&bytes), AssetKind::Dds);
    let decoded = dds::decode(&bytes, "explicit caller bytes", limits(&bytes)).unwrap();
    assert_eq!(decoded.texture.source, "explicit caller bytes");
    assert_eq!((decoded.texture.width, decoded.texture.height), (4, 4));
    assert_eq!(
        &decoded.texture.rgba[..16],
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 170, 85, 0, 255, 85, 170, 0, 255
        ]
    );
    assert_eq!(decoded.profile.mips[0].bytes, 128..136);
    assert!(matches!(
        pcx_gfx::load_image(&bytes, "unbudgeted"),
        Err(pcx_gfx::Error::Dds(dds::Error {
            kind: Failure::Unsupported(_),
            ..
        }))
    ));
}

#[test]
fn bc1_three_colour_mode_without_selector_three_is_supported() {
    let mut bytes = fixture(Format::Bc1, 4, 4, 1);
    bytes[128..132].copy_from_slice(&[0, 0, 255, 255]);
    put(&mut bytes, 132, 2);
    let texture = dds::decode(&bytes, "three colours", limits(&bytes))
        .unwrap()
        .texture;
    assert_eq!(&texture.rgba[..4], &[128, 128, 128, 255]);
}

#[test]
fn bc3_keeps_distinct_colour_and_fractional_alpha_lanes() {
    let mut bytes = fixture(Format::Bc3, 4, 4, 1);
    let alpha = 1u64 << 3 | 2 << 6 | 7 << 9;
    bytes[130..136].copy_from_slice(&alpha.to_le_bytes()[..6]);
    put(&mut bytes, 140, 0b11_10_01_00);
    let texture = dds::decode(&bytes, "alpha lanes", limits(&bytes))
        .unwrap()
        .texture;
    assert_eq!(
        &texture.rgba[..16],
        &[
            255, 0, 0, 255, 0, 255, 0, 0, 170, 85, 0, 218, 85, 170, 0, 36
        ]
    );
}

#[test]
fn bc3_other_alpha_endpoint_order_keeps_special_zero_and_full_entries() {
    let mut bytes = fixture(Format::Bc3, 4, 4, 1);
    bytes[128..130].copy_from_slice(&[0, 255]);
    let alpha = 6u64 | 7 << 3 | 2 << 6;
    bytes[130..136].copy_from_slice(&alpha.to_le_bytes()[..6]);
    let texture = dds::decode(&bytes, "other alpha order", limits(&bytes))
        .unwrap()
        .texture;
    assert_eq!(
        (texture.rgba[3], texture.rgba[7], texture.rgba[11]),
        (0, 255, 51)
    );
}

#[test]
fn complete_authored_rectangular_chain_includes_sub_block_mips() {
    let bytes = fixture(Format::Bc1, 8, 4, 4);
    let decoded = dds::decode(&bytes, "rectangular chain", limits(&bytes)).unwrap();
    let spans: Vec<_> = decoded
        .profile
        .mips
        .iter()
        .map(|m| (m.width, m.height, m.bytes.clone()))
        .collect();
    assert_eq!(
        spans,
        vec![
            (8, 4, 128..144),
            (4, 2, 144..152),
            (2, 1, 152..160),
            (1, 1, 160..168)
        ]
    );
    assert_eq!(decoded.texture.rgba.len(), 8 * 4 * 4);
    let mut bc3 = fixture(Format::Bc3, 8, 4, 4);
    assert_eq!(
        dds::decode(&bc3, "BC3 mips", limits(&bc3))
            .unwrap()
            .profile
            .mips
            .last()
            .unwrap()
            .bytes
            .end,
        bc3.len()
    );
    bc3.pop();
    assert!(matches!(error(&bc3).kind, Failure::Malformed(_)));
}

#[test]
fn malformed_envelopes_lengths_and_counts_are_typed() {
    let base = fixture(Format::Bc1, 4, 4, 1);
    for (at, value) in [
        (0, 0),
        (4, 123),
        (76, 31),
        (12, 0),
        (16, 0),
        (8, 0),
        (80, 0),
        (108, 0),
        (20, 7),
        (28, 4),
    ] {
        let mut bytes = base.clone();
        put(&mut bytes, at, value);
        let e = error(&bytes);
        assert!(matches!(e.kind, Failure::Malformed(_)), "at {at}: {e}");
        assert_eq!(e.source_name, "synthetic.dds");
    }
    for n in [0, 4, 127, 135] {
        assert!(matches!(error(&base[..n]).kind, Failure::Malformed(_)));
    }
    let mut trailing = base.clone();
    trailing.push(0);
    assert!(matches!(error(&trailing).kind, Failure::Malformed(_)));
    let mut mips = fixture(Format::Bc1, 8, 4, 4);
    put(&mut mips, 28, 0);
    assert!(matches!(error(&mips).kind, Failure::Malformed(_)));
    put(&mut mips, 28, 5);
    assert!(matches!(error(&mips).kind, Failure::Malformed(_)));
    put(&mut mips, 28, 4);
    put(&mut mips, 108, 0x1000);
    assert!(matches!(error(&mips).kind, Failure::Malformed(_)));
}

#[test]
fn ati2_dx10_cube_volume_and_unknown_semantics_are_refused() {
    let base = fixture(Format::Bc1, 4, 4, 1);
    for fourcc in [*b"ATI2", *b"DX10", *b"DXT3", *b"????"] {
        let mut bytes = base.clone();
        bytes[84..88].copy_from_slice(&fourcc);
        assert!(matches!(error(&bytes).kind, Failure::Unsupported(_)));
    }
    for (at, value) in [
        (112, 0x200),
        (112, 0x200000),
        (24, 1),
        (8, 0x881007),
        (8, 0x81017),
        (108, 0x1010),
        (116, 1),
        (120, 1),
        (124, 1),
        (80, 0x80000004),
        (88, 0x59583241),
        (92, 1),
    ] {
        let mut bytes = base.clone();
        put(&mut bytes, at, value);
        assert!(
            matches!(error(&bytes).kind, Failure::Unsupported(_)),
            "at {at}"
        );
    }
    let mut nonaligned = base.clone();
    put(&mut nonaligned, 16, 5);
    assert!(matches!(error(&nonaligned).kind, Failure::Unsupported(_)));
}

#[test]
fn nvtt_marker_is_preserved_only_under_its_explicit_profile() {
    let mut bytes = fixture(Format::Bc1, 4, 4, 1);
    put(&mut bytes, 80, 0x40000004);
    assert!(matches!(error(&bytes).kind, Failure::Unsupported(_)));
    put(&mut bytes, 68, u32::from_le_bytes(*b"NVTT"));
    let decoded = dds::decode(&bytes, "raw marker", limits(&bytes)).unwrap();
    assert_eq!(decoded.profile.pixel_flags, 0x40000004);
    assert_eq!(decoded.profile.reserved[9], u32::from_le_bytes(*b"NVTT"));
    assert_eq!(&decoded.texture.rgba[..4], &[255, 0, 0, 255]);
}

#[test]
fn punch_through_is_refused_in_top_and_later_mips_even_equal_endpoints() {
    for (width, levels, at) in [(4, 1, 128), (8, 2, 160)] {
        let mut bytes = fixture(Format::Bc1, width, width, levels);
        bytes[at..at + 4].copy_from_slice(&[0, 0, 0, 0]);
        put(&mut bytes, at + 4, 3);
        let e = error(&bytes);
        assert!(matches!(e.kind, Failure::Unsupported(_)));
        assert_eq!(e.offset, at);
    }
}

#[test]
fn all_three_caller_budgets_and_arithmetic_overflow_precede_decode() {
    let bytes = fixture(Format::Bc1, 4, 4, 1);
    for budget in [
        Limits {
            max_encoded_bytes: 135,
            ..limits(&bytes)
        },
        Limits {
            max_rgba_bytes: 63,
            ..limits(&bytes)
        },
        Limits {
            max_working_bytes: 111,
            ..limits(&bytes)
        },
    ] {
        assert!(matches!(
            dds::decode(&bytes, "budget", budget).unwrap_err().kind,
            Failure::Limits { .. }
        ));
    }
    let exact = Limits {
        max_encoded_bytes: 136,
        max_rgba_bytes: 64,
        max_working_bytes: 112,
    };
    assert_eq!(
        dds::decode(&bytes, "exact budgets", exact)
            .unwrap()
            .texture
            .rgba
            .len(),
        64
    );
    let mut huge = bytes;
    put(&mut huge, 16, u32::MAX - 3);
    put(&mut huge, 12, u32::MAX - 3);
    assert!(matches!(
        dds::decode(
            &huge,
            "overflow",
            Limits {
                max_encoded_bytes: huge.len() as u64,
                max_rgba_bytes: u64::MAX,
                max_working_bytes: u64::MAX
            }
        )
        .unwrap_err()
        .kind,
        Failure::Overflow(_)
    ));
}

#[test]
fn rgba_vector_layout_overflow_is_rejected_before_decode() {
    let mut bytes = fixture(Format::Bc1, 4, 4, 1);
    // The byte count fits usize but exceeds the platform's Vec layout limit.
    let (width, height) = if usize::BITS == 32 {
        (1 << 15, 1 << 14)
    } else {
        (1 << 30, 1 << 31)
    };
    put(&mut bytes, 16, width);
    put(&mut bytes, 12, height);
    let e = dds::inspect(
        &bytes,
        "oversized vector layout",
        Limits {
            max_encoded_bytes: bytes.len() as u64,
            max_rgba_bytes: u64::MAX,
            max_working_bytes: u64::MAX,
        },
    )
    .unwrap_err();
    assert_eq!(e.kind, Failure::Overflow("RGBA vector layout"));
}

#[test]
fn unflagged_single_mip_retains_the_authored_zero_count() {
    let mut bytes = fixture(Format::Bc1, 4, 4, 1);
    put(&mut bytes, 28, 0);
    let profile = dds::inspect(&bytes, "one base surface", limits(&bytes)).unwrap();
    assert_eq!(profile.authored_mip_count, 0);
    assert_eq!(profile.mips.len(), 1);
}

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR; explicitly named configured DDS fixtures"]
fn named_original_footman_dds_provenance_decode_and_ati2_refusal() {
    let install =
        std::env::var_os("PEACECRAFT3_REFORGED_DIR").expect("explicit original install required");
    let storage = pcx_casc::storage::Storage::open(
        std::path::Path::new(&install),
        pcx_casc::blte::Limits {
            encoded_bytes: 64 * 1024 * 1024,
            decoded_bytes: 256 * 1024 * 1024,
        },
    )
    .unwrap();
    let encoding = storage.load_encoding().unwrap();
    let resolver = storage.load_tvfs(&encoding).unwrap();
    for (path, ekey, ckey, format, rgba_md5) in [
        (
            "war3.w3mod:_hd.w3mod:units/human/footman/human_footman_main_diffuse.dds",
            "dad62eed624c84a8fcd4335893270f81",
            "25a4a52563136827ff89f3ebc966123a",
            Some(Format::Bc1),
            "67ab3c72a44d45aab7731a822e3ae16c",
        ),
        (
            "war3.w3mod:_hd.w3mod:units/human/footman/human_footman_main_orm.dds",
            "f9c63003c3c5c403e13452c45d517c81",
            "8a1008e9e79cbf2043459daf1b1263b6",
            Some(Format::Bc3),
            "fb593afff3d175c0bbeddd9f4b7dd4a5",
        ),
        (
            "war3.w3mod:_hd.w3mod:units/human/footman/human_footman_main_normal.dds",
            "a94fde4333e1fd247c30b65c3db6b4aa",
            "dba0c21180bef10bda12e16e709aa266",
            None,
            "",
        ),
    ] {
        let asset = storage.read_path(&resolver, path).unwrap();
        assert_eq!(pcx_casc::hex(&asset.resolved.ekey), ekey);
        assert_eq!(pcx_casc::hex(&asset.resolved.ckey), ckey);
        assert!(!asset.resolved.links.is_empty());
        let bytes = &asset.file.decoded.bytes;
        let width = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let pixels = u64::from(width).checked_mul(u64::from(height)).unwrap();
        let budgets = Limits {
            max_encoded_bytes: bytes.len() as u64,
            max_rgba_bytes: pixels.checked_mul(4).unwrap(),
            max_working_bytes: pixels
                .checked_mul(7)
                .unwrap()
                .checked_add(u64::from(width) * 4)
                .unwrap(),
        };
        match format {
            Some(format) => {
                let decoded = dds::decode(bytes, path, budgets).unwrap();
                assert_eq!(decoded.profile.format, format);
                assert_eq!(
                    (decoded.texture.width, decoded.texture.height),
                    (width, height)
                );
                assert_eq!(decoded.texture.rgba.len() as u64, pixels * 4);
                assert_eq!(decoded.profile.mips.last().unwrap().bytes.end, bytes.len());
                let hash = pcx_casc::Evidence::of(
                    asset.file.decoded.evidence.source.clone(),
                    &decoded.texture.rgba,
                );
                assert_eq!(pcx_casc::hex(&hash.md5), rgba_md5);
                println!(
                    "DDS_OK path={path} build={} ekey={ekey} ckey={ckey} bytes={} format={format:?} dimensions={width}x{height} mips={} rgba_bytes={} rgba_md5={}",
                    storage.build.info.source.version,
                    bytes.len(),
                    decoded.profile.mips.len(),
                    decoded.texture.rgba.len(),
                    pcx_casc::hex(&hash.md5)
                );
            }
            None => {
                let e = dds::decode(bytes, path, budgets).unwrap_err();
                assert!(matches!(e.kind, Failure::Unsupported(_)));
                assert_eq!(e.offset, 84);
                assert_eq!(e.source_name, path);
                println!(
                    "DDS_REFUSAL path={path} build={} ekey={ekey} ckey={ckey} bytes={} error={e}",
                    storage.build.info.source.version,
                    bytes.len()
                );
            }
        }
    }
}
