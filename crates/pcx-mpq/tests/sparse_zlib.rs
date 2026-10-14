//! Synthetic bounded import/primary-order proof; no native/edition parity claim.
use pcx_mpq::{
    Error, ExtractionBudget, ExtractionCause, MpqArchive,
    checked::{CheckedArchive, ReadBudget, ReadError},
    flags,
    payload::{PayloadError, decompress_sector},
};
use std::io::Write;
const MEMBER: &str = "caller\\sparse-chain.bin";
fn zlib_sparse(sparse: &[u8]) -> Vec<u8> {
    let mut packed = vec![0x22];
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(sparse).unwrap();
    packed.extend(z.finish().unwrap());
    packed
}
fn maximally_framed(n: u32) -> Vec<u8> {
    let mut s = n.to_be_bytes().to_vec();
    for _ in 0..n {
        s.extend([0x80, 1]);
    }
    s
}
fn zero_stream(n: u32) -> Vec<u8> {
    let mut s = n.to_be_bytes().to_vec();
    let mut remaining = n;
    while remaining > 130 {
        s.push(0x7f);
        remaining -= 130;
    }
    if remaining > 0 {
        s.push(0x7f);
    }
    s
}
fn encrypt(data: &mut [u8], key: u32) {
    let table = pcx_mpq::crypt::table();
    let mut seed1 = key;
    let mut seed2 = 0xeeeeeeeeu32;
    for chunk in data.as_chunks_mut::<4>().0 {
        seed2 = seed2.wrapping_add(table[0x400 + (seed1 & 255) as usize]);
        let plain = u32::from_le_bytes(*chunk);
        chunk.copy_from_slice(&(plain ^ seed1.wrapping_add(seed2)).to_le_bytes());
        seed1 = ((!seed1) << 21).wrapping_add(0x11111111) | (seed1 >> 11);
        seed2 = plain
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
}
fn archive(packed: &[u8], file_size: u32, multi: bool) -> Vec<u8> {
    archive_sectors(&[packed], file_size, multi)
}
fn archive_sectors(sectors: &[&[u8]], file_size: u32, multi: bool) -> Vec<u8> {
    let file_at = 64u32;
    let mut block_flags = flags::EXISTING | flags::COMPRESS | flags::ENCRYPTED | flags::FIX_KEY;
    if !multi {
        block_flags |= flags::SINGLE_UNIT;
    }
    let key = pcx_mpq::payload::file_key(MEMBER, file_at, file_size, block_flags);
    let mut payload = if multi {
        let mut offset = ((sectors.len() + 1) * 4) as u32;
        let mut offsets = offset.to_le_bytes().to_vec();
        for sector in sectors {
            offset += sector.len() as u32;
            offsets.extend(offset.to_le_bytes());
        }
        encrypt(&mut offsets, key.wrapping_sub(1));
        for (i, sector) in sectors.iter().enumerate() {
            let mut body = sector.to_vec();
            encrypt(&mut body, key.wrapping_add(i as u32));
            offsets.extend(body);
        }
        offsets
    } else {
        assert_eq!(sectors.len(), 1);
        let mut body = sectors[0].to_vec();
        encrypt(&mut body, key);
        body
    };
    let hash_at = file_at + payload.len() as u32;
    let block_at = hash_at + 64;
    let mut hashes = vec![255u8; 64];
    let slot = pcx_mpq::hash::start_slot(MEMBER, 4);
    let (a, b) = pcx_mpq::hash::hash_string_both(MEMBER);
    let h = &mut hashes[slot * 16..slot * 16 + 16];
    h[..4].copy_from_slice(&a.to_le_bytes());
    h[4..8].copy_from_slice(&b.to_le_bytes());
    h[8..12].fill(0);
    h[12..].copy_from_slice(&0u32.to_le_bytes());
    encrypt(&mut hashes, pcx_mpq::hash::hash_table_key());
    let mut block = vec![];
    for n in [file_at, payload.len() as u32, file_size, block_flags] {
        block.extend(n.to_le_bytes());
    }
    encrypt(&mut block, pcx_mpq::hash::block_table_key());
    let mut raw = b"MPQ\x1a".to_vec();
    for n in [32, block_at + 16] {
        raw.extend(n.to_le_bytes());
    }
    raw.extend(0u16.to_le_bytes());
    raw.extend(0u16.to_le_bytes());
    for n in [hash_at, block_at, 4, 1] {
        raw.extend(n.to_le_bytes());
    }
    raw.resize(file_at as usize, 0);
    raw.append(&mut payload);
    raw.extend(hashes);
    raw.extend(block);
    raw
}
fn budget(raw: &[u8], decoded: usize, packed: usize) -> ReadBudget {
    ReadBudget {
        max_archive_bytes: raw.len(),
        max_archive_table_bytes: 80,
        max_raw_packed_bytes: packed,
        max_decoded_bytes: decoded,
    }
}
#[test]
fn encrypted_actual_consumers_and_expanding_intermediate() {
    let sparse = maximally_framed(64);
    assert_eq!(sparse.len(), 132);
    let packed = zlib_sparse(&sparse);
    assert!(packed.len() < 64);
    for multi in [false, true] {
        let raw = archive(&packed, 64, multi);
        let source = "explicit synthetic encrypted sparse/zlib archive";
        let bound = budget(&raw, 64, packed.len() + if multi { 8 } else { 0 });
        let view = CheckedArchive::parse(&raw, source, bound).unwrap();
        assert_eq!(view.extract(MEMBER, 64).unwrap().unwrap(), vec![1u8; 64]);
        let mut owned = MpqArchive::from_bytes(raw.clone(), source).unwrap();
        let actual = owned
            .extract_bounded(
                MEMBER,
                ExtractionBudget {
                    max_raw_packed_bytes: bound.max_raw_packed_bytes,
                    max_decoded_bytes: 64,
                },
                None,
            )
            .unwrap();
        assert_eq!(actual, vec![1u8; 64]);
        assert_eq!(
            pcx_mpq::fnv1a64(&raw),
            pcx_mpq::fnv1a64(&archive(&packed, 64, multi))
        );
    }
    let first = zlib_sparse(&maximally_framed(512));
    let second = zlib_sparse(&zero_stream(512));
    let raw = archive_sectors(&[&first, &second], 1024, true);
    let packed_bytes = 12 + first.len() + second.len();
    let view = CheckedArchive::parse(
        &raw,
        "caller two encrypted sectors",
        budget(&raw, 1024, packed_bytes),
    )
    .unwrap();
    let mut expected = vec![1u8; 512];
    expected.extend([0u8; 512]);
    assert_eq!(view.extract(MEMBER, 1024).unwrap().unwrap(), expected);
    let over = zlib_sparse(&[513u32.to_be_bytes().to_vec(), vec![0x7f, 0x7f, 0x7f, 0x7f]].concat());
    let bad = archive_sectors(&[&over, &second], 1024, true);
    let view = CheckedArchive::parse(
        &bad,
        "caller sector capacity",
        budget(&bad, 1024, 12 + over.len() + second.len()),
    )
    .unwrap();
    assert!(matches!(
        view.extract(MEMBER, 1024),
        Err(ReadError::ExtractionFailed {
            source: Error::Payload(PayloadError::Sparse(
                pcx_mpq::sparse::SparseError::DecodedSizeExceeds {
                    decoded: 513,
                    capacity: 512
                }
            )),
            ..
        })
    ));
    println!(
        "encrypted single, sector-table and actual two-sector extraction: intermediate may exceed final; per-sector final512 cannot borrow next sector; no Huffman/PE lookup"
    );
}
#[test]
fn header_final_trailing_and_corrupt_errors_have_source_context() {
    let malformed = [64u32.to_be_bytes().to_vec(), vec![0x80]].concat();
    let packed = zlib_sparse(&malformed);
    let raw = archive(&packed, 64, false);
    let view = CheckedArchive::parse(
        &raw,
        "caller malformed chain",
        budget(&raw, 64, packed.len()),
    )
    .unwrap();
    let failure = view.extract(MEMBER, 64).unwrap_err();
    assert!(
        matches!(failure,ReadError::ExtractionFailed{map,member,source:Error::Payload(PayloadError::Sparse(_))} if map=="caller malformed chain" && member==MEMBER)
    );
    let mut out = [0u8; 64];
    let mut trailing = zero_stream(64);
    trailing.push(0);
    assert!(matches!(
        decompress_sector(&zlib_sparse(&trailing), &mut out),
        Err(PayloadError::Sparse(_))
    ));
    let mut packed = zlib_sparse(&zero_stream(64));
    packed.push(0);
    assert!(matches!(
        decompress_sector(&packed, &mut out),
        Err(PayloadError::Zlib)
    ));
    assert!(matches!(
        decompress_sector(&[0x22, 0], &mut out),
        Err(PayloadError::Zlib)
    ));
    let over = [65u32.to_be_bytes().to_vec(), vec![0x7f]].concat();
    assert!(matches!(
        decompress_sector(&zlib_sparse(&over), &mut out),
        Err(PayloadError::Sparse(
            pcx_mpq::sparse::SparseError::DecodedSizeExceeds {
                decoded: 65,
                capacity: 64
            }
        ))
    ));
    let wrong = zlib_sparse(&zero_stream(63));
    let raw = archive(&wrong, 64, false);
    let mut owned = MpqArchive::from_bytes(raw, "caller size mismatch").unwrap();
    let error = owned
        .extract_bounded(
            MEMBER,
            ExtractionBudget {
                max_raw_packed_bytes: wrong.len(),
                max_decoded_bytes: 64,
            },
            None,
        )
        .unwrap_err();
    assert_eq!(error.member, MEMBER);
    assert!(matches!(
        *error.cause,
        ExtractionCause::Mpq(Error::Payload(PayloadError::SizeMismatch {
            file_size: 64,
            produced: 63
        }))
    ));
    println!(
        "final/header/trailing/corrupt refusals retain source/member and existing exact block-size semantics"
    );
}
#[test]
fn derived_stage_ceiling_is_tight_and_growth_refuses() {
    let valid = maximally_framed(64);
    let mut out = [0u8; 64];
    assert_eq!(
        decompress_sector(&zlib_sparse(&valid), &mut out).unwrap(),
        64
    );
    let mut over = valid;
    over.push(0);
    assert_eq!(over.len(), 133);
    assert!(matches!(
        decompress_sector(&zlib_sparse(&over), &mut out),
        Err(PayloadError::Zlib)
    ));
    let mut one_under = [0u8; 63];
    assert!(decompress_sector(&zlib_sparse(&maximally_framed(64)), &mut one_under).is_err());
    println!(
        "derived stage ceiling132 accepts exact boundary; inflated133 and one-under final capacity refuse before unbounded growth"
    );
}
#[test]
fn one_under_archive_budgets_refuse_before_decode() {
    let packed = zlib_sparse(&maximally_framed(64));
    let raw = archive(&packed, 64, false);
    for b in [
        budget(&raw, 63, packed.len()),
        budget(&raw, 64, packed.len() - 1),
    ] {
        let view = CheckedArchive::parse(&raw, "caller bounded chain", b).unwrap();
        assert!(matches!(
            view.extract(MEMBER, 64),
            Err(ReadError::BudgetExceeded { .. })
        ));
    }
    let view = CheckedArchive::parse(&raw, "caller bounded chain", budget(&raw, 64, packed.len()))
        .unwrap();
    assert!(matches!(
        view.extract(MEMBER, 63),
        Err(ReadError::BudgetExceeded { .. })
    ));
    let huge = archive(&packed, u32::MAX, false);
    let view = CheckedArchive::parse(
        &huge,
        "caller forged final size",
        budget(&huge, 64, packed.len()),
    )
    .unwrap();
    assert!(matches!(
        view.extract(MEMBER, 64),
        Err(ReadError::BudgetExceeded { .. })
    ));
    println!("archive packed/final/per-member one-under budgets remain preflight refusals");
}
#[test]
fn legacy_default_mask_and_verbatim_contracts_remain() {
    let mut out = [0u8; 64];
    for mask in [0x21, 0x23, 0x32, 0x62, 0x01, 0x12] {
        assert!(
            matches!(decompress_sector(&[mask,0],&mut out),Err(PayloadError::UnsupportedCompression{mask:actual}) if actual==mask)
        );
    }
    assert_eq!(decompress_sector(&[0, 9, 8], &mut out).unwrap(), 2);
    let flags = flags::COMPRESS | flags::SINGLE_UNIT;
    for byte in [0x20, 0x22] {
        let raw = vec![byte; 64];
        assert_eq!(
            pcx_mpq::payload::decode_block(&raw, 0, 64, flags, 512).unwrap(),
            raw
        );
    }
    println!(
        "other sparse combinations and default Huffman/LZMA refusals unchanged; stored-verbatim branch still owns mask-looking raw bytes"
    );
}

#[test]
fn standalone_sparse_uses_checked_archive_consumers_and_existing_size_rules() {
    let mut sparse = 64u32.to_be_bytes().to_vec();
    sparse.extend([0x82, 5, 6, 7, 0x7f]);
    let packed = [vec![0x20], sparse].concat();
    assert!(packed.len() < 64);
    let mut expected = vec![0; 64];
    expected[..3].copy_from_slice(&[5, 6, 7]);
    for multi in [false, true] {
        let raw = archive(&packed, 64, multi);
        let packed_bytes = packed.len() + if multi { 8 } else { 0 };
        let view = CheckedArchive::parse(
            &raw,
            "caller standalone sparse",
            budget(&raw, 64, packed_bytes),
        )
        .unwrap();
        assert_eq!(view.extract(MEMBER, 64).unwrap().unwrap(), expected);
        assert!(matches!(
            view.extract(MEMBER, 63),
            Err(ReadError::BudgetExceeded { .. })
        ));
        let mut owned = MpqArchive::from_bytes(raw, "caller standalone sparse").unwrap();
        assert_eq!(
            owned
                .extract_bounded(
                    MEMBER,
                    ExtractionBudget {
                        max_raw_packed_bytes: packed_bytes,
                        max_decoded_bytes: 64,
                    },
                    None
                )
                .unwrap(),
            expected
        );
    }
    let zeros = [vec![0x20], zero_stream(512)].concat();
    let ones = zlib_sparse(&maximally_framed(512));
    for (first, second) in [(&zeros, &ones), (&ones, &zeros)] {
        let raw = archive_sectors(&[first, second], 1024, true);
        let view = CheckedArchive::parse(
            &raw,
            "caller mixed sparse sectors",
            budget(&raw, 1024, 12 + first.len() + second.len()),
        )
        .unwrap();
        let mut expected = vec![if first[0] == 0x20 { 0 } else { 1 }; 512];
        expected.extend(vec![if second[0] == 0x20 { 0 } else { 1 }; 512]);
        assert_eq!(view.extract(MEMBER, 1024).unwrap().unwrap(), expected);
    }
    let mut out = [0; 64];
    assert!(matches!(
        decompress_sector(&[0x20, 0], &mut out),
        Err(PayloadError::Sparse(_))
    ));
    let oversized = [vec![0x20], 65u32.to_be_bytes().to_vec(), vec![0x7f]].concat();
    assert!(matches!(
        decompress_sector(&oversized, &mut out),
        Err(PayloadError::Sparse(
            pcx_mpq::sparse::SparseError::DecodedSizeExceeds {
                decoded: 65,
                capacity: 64
            }
        ))
    ));
    let wrong = [vec![0x20], zero_stream(63)].concat();
    let raw = archive(&wrong, 64, false);
    let view = CheckedArchive::parse(
        &raw,
        "caller standalone size mismatch",
        budget(&raw, 64, wrong.len()),
    )
    .unwrap();
    assert!(
        matches!(view.extract(MEMBER, 64), Err(ReadError::ExtractionFailed {
        map, member, source: Error::Payload(PayloadError::SizeMismatch { file_size: 64, produced: 63 })
    }) if map == "caller standalone size mismatch" && member == MEMBER)
    );
}
