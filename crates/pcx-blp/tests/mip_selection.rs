use pcx_blp::{Error, Header, MAX_MIPS, bytes_per_pixel, decode_mip, mip_bytes};

fn fixture() -> Vec<u8> {
    let mut bytes = vec![0; 0xa6];
    bytes[..4].copy_from_slice(b"BLP1");
    for (at, word) in [
        (4, 1_u32),
        (8, 8),
        (12, 2),
        (16, 1),
        (20, 5),
        (24, 1),
        (0x1c, 0xa0),
        (0x5c, 4),
        (0x20, 0xa4),
        (0x60, 2),
    ] {
        bytes[at..at + 4].copy_from_slice(&word.to_le_bytes());
    }
    bytes[0x9c..0xa0].copy_from_slice(&[19, 71, 139, 0]);
    bytes[0xa0..].copy_from_slice(&[0, 0, 73, 129, 0, 211]);
    bytes
}

#[test]
fn valid_top_and_lower_mips_preserve_color_and_alpha() {
    let bytes = fixture();
    let header = Header::parse(&bytes).unwrap();
    let top = decode_mip(&bytes, &header, 0).unwrap();
    assert_eq!(top, [139, 71, 19, 73, 139, 71, 19, 129]);
    assert_eq!(pcx_blp::decode(&bytes).unwrap().1, top);
    assert_eq!(decode_mip(&bytes, &header, 1).unwrap(), [139, 71, 19, 211]);
    let mut opaque = header.clone();
    opaque.mip_sizes[0] = 2;
    assert_eq!(
        decode_mip(&bytes, &opaque, 0).unwrap(),
        [139, 71, 19, 255, 139, 71, 19, 255]
    );
}

#[test]
fn absent_slots_remain_inspectable_but_cannot_decode_as_images() {
    let bytes = fixture();
    let header = Header::parse(&bytes).unwrap();
    assert!(mip_bytes(&bytes, &header, 2).unwrap().is_empty());
    assert_eq!(bytes_per_pixel(&bytes, &header, 2).unwrap(), None);
    assert!(matches!(
        decode_mip(&bytes, &header, 2),
        Err(Error::UnrecognisedMip {
            index: 2,
            size: 0,
            ..
        })
    ));
}

#[test]
fn outside_header_slot_is_an_error_instead_of_a_panic() {
    let bytes = fixture();
    let header = Header::parse(&bytes).unwrap();
    for index in [MAX_MIPS, usize::MAX] {
        assert!(
            matches!(mip_bytes(&bytes, &header, index), Err(Error::InvalidMipIndex { index: bad }) if bad == index)
        );
        assert!(
            matches!(decode_mip(&bytes, &header, index), Err(Error::InvalidMipIndex { index: bad }) if bad == index)
        );
        assert!(
            matches!(bytes_per_pixel(&bytes, &header, index), Err(Error::InvalidMipIndex { index: bad }) if bad == index)
        );
    }
}

#[test]
fn jpeg_rejects_absent_or_outside_slots_before_shared_metadata() {
    let bytes = fixture();
    let mut header = Header::parse(&bytes).unwrap();
    header.content = 0;
    assert!(matches!(
        pcx_blp::decode_jpeg_with_limit(&bytes, &header, 2, 1024),
        Err(Error::UnrecognisedMip {
            index: 2,
            size: 0,
            ..
        })
    ));
    for index in [MAX_MIPS, usize::MAX] {
        assert!(
            matches!(pcx_blp::decode_jpeg_with_limit(&bytes, &header, index, 1024), Err(Error::InvalidMipIndex { index: bad }) if bad == index)
        );
    }
}

#[test]
fn dimension_work_is_bounded_without_changing_halving() {
    let bytes = fixture();
    let mut header = Header::parse(&bytes).unwrap();
    for width in [0, 1, 3, 17, u32::MAX] {
        header.width = width;
        header.height = u32::MAX;
        let mut expected = (width, u32::MAX);
        for index in 0..=64 {
            assert_eq!(header.mip_size(index), expected);
            expected = ((expected.0 >> 1).max(1), (expected.1 >> 1).max(1));
        }
        assert_eq!(header.mip_size(usize::MAX), (1, 1));
    }
}

#[test]
fn malformed_payloads_refuse_before_dimension_driven_allocation() {
    let bytes = fixture();
    let mut header = Header::parse(&bytes).unwrap();
    for dimensions in [(0, 1), (3, 1), (u32::MAX, u32::MAX), (1 << 31, 1)] {
        (header.width, header.height) = dimensions;
        assert!(matches!(
            decode_mip(&bytes, &header, 0),
            Err(Error::UnrecognisedMip { .. })
        ));
    }
    header = Header::parse(&bytes).unwrap();
    header.mip_offsets[0] = 0;
    assert!(matches!(
        mip_bytes(&bytes, &header, 0),
        Err(Error::UnrecognisedMip { .. })
    ));
    header.mip_offsets[0] = u32::MAX;
    assert!(matches!(
        decode_mip(&bytes, &header, 0),
        Err(Error::MipOutOfBounds { .. })
    ));
    header.content = 0;
    header.mip_offsets[0] = 0xa0;
    let mut corrupt = bytes.clone();
    corrupt[0x9c..0xa0].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(pcx_blp::jpeg_header(&corrupt, &header).is_err());
}

#[test]
fn cli_dump_refuses_bad_indexes_and_handles_a_short_tail() {
    let path = std::env::temp_dir().join(format!("pcx-blp-mip-{}.blp", std::process::id()));
    std::fs::write(&path, fixture()).unwrap();
    let run = |index: &str| {
        std::process::Command::new(env!("CARGO_BIN_EXE_blpx"))
            .args(["dump", path.to_str().unwrap(), index, "0"])
            .output()
            .unwrap()
    };
    let invalid = run(&usize::MAX.to_string());
    let valid = run("0");
    let absent = run("2");
    let repeated = run(&usize::MAX.to_string());
    std::fs::remove_file(path).unwrap();
    assert_eq!(invalid.status.code(), Some(1));
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("header slots"));
    assert_eq!(repeated.status.code(), invalid.status.code());
    assert_eq!(repeated.stderr, invalid.stderr);
    assert!(valid.status.success(), "{:?}", valid);
    assert!(valid.stderr.is_empty());
    assert!(String::from_utf8_lossy(&valid.stdout).contains("tail[-16..]"));
    assert!(absent.status.success());
    assert!(absent.stderr.is_empty());
}
