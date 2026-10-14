use pcx_trig::{Error, Wct};
fn container(header: u32, blocks: &[&[u8]]) -> Vec<u8> {
    let mut bytes = header.to_le_bytes().to_vec();
    bytes.extend(u32::try_from(blocks.len()).unwrap().to_le_bytes());
    for block in blocks {
        bytes.extend(u32::try_from(block.len()).unwrap().to_le_bytes());
        bytes.extend(*block);
    }
    bytes
}
#[test]
fn impossible_declared_count_is_refused_before_any_length_walk() {
    let mut bytes = 0u32.to_le_bytes().to_vec();
    bytes.extend(u32::MAX.to_le_bytes());
    assert!(matches!(
        Wct::parse(&bytes),
        Err(Error::WctCountOverruns {
            at: 4,
            count: u32::MAX,
            length_words: 0
        })
    ));
    bytes.extend(0u32.to_le_bytes());
    assert!(matches!(
        Wct::parse(&bytes),
        Err(Error::WctCountOverruns {
            at: 4,
            count: u32::MAX,
            length_words: 1
        })
    ));
}
#[test]
fn valid_header_raw_text_and_empty_slot_positions_are_preserved() {
    let data = container(17, &[b"first\0", b"", &[0xff, 0], b"third\0"]);
    let parsed = Wct::parse(&data).unwrap();
    assert_eq!(parsed.unknown0(), 17);
    assert_eq!(parsed.blocks().len(), 4);
    assert_eq!(parsed.blocks()[0].text, b"first\0");
    assert!(parsed.blocks()[1].text.is_empty());
    assert_eq!(parsed.blocks()[2].text, [0xff, 0]);
    assert_eq!(parsed.blocks()[3].text, b"third\0");
    assert_eq!(parsed.raw(), data);
    assert_eq!(parsed.canonical_bytes(), data);
    assert!(parsed.framing().sums());
    assert_eq!(parsed.framing().text_bytes, 6 + 2 + 6);
    assert!(parsed.framing().texts_are_nul_terminated);
}
#[test]
fn missing_count_or_length_is_not_a_valid_empty_container_or_slot() {
    assert!(matches!(
        Wct::parse(&[]),
        Err(Error::Truncated {
            what: "unknown0",
            ..
        })
    ));
    assert!(matches!(
        Wct::parse(&0u32.to_le_bytes()),
        Err(Error::Truncated {
            what: "block count",
            ..
        })
    ));
    assert!(Wct::parse(&container(0, &[])).unwrap().blocks().is_empty());
    let one = container(0, &[b""]);
    assert_eq!(Wct::parse(&one).unwrap().blocks().len(), 1);
    assert!(matches!(
        Wct::parse(&one[..8]),
        Err(Error::WctCountOverruns { count: 1, .. })
    ));
    let mut missing = one;
    missing[8..12].copy_from_slice(&1u32.to_le_bytes());
    assert!(matches!(
        Wct::parse(&missing),
        Err(Error::Truncated {
            what: "text",
            at: 8,
            need: 1,
            have: 0
        })
    ));
}
#[test]
fn malformed_second_span_reports_its_own_length_word() {
    let mut data = container(0, &[b"first\0", b""]);
    let at = data.len() - 4;
    data[at..].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(
        matches!(Wct::parse(&data),Err(Error::Truncated {what:"text",at:got,need,have:0}) if got==at && need==u32::MAX as usize)
    );
}
#[test]
fn trailing_bytes_are_refused_and_not_absorbed_into_a_block() {
    let mut data = container(0, &[b"a\0"]);
    let end = data.len();
    data.push(0);
    assert!(
        matches!(Wct::parse(&data),Err(Error::TrailingBytes {end:got,len}) if got==end && len==data.len())
    );
}
#[test]
fn raw_malformed_text_is_not_normalized_by_container_framing() {
    let data = container(0, &[b"missing final nul", b"embedded\0nul\0", b"", b"\0"]);
    let parsed = Wct::parse(&data).unwrap();
    assert_eq!(parsed.canonical_bytes(), data);
    assert!(!parsed.framing().texts_are_nul_terminated);
    assert_eq!(parsed.blocks()[0].text, b"missing final nul");
    assert_eq!(parsed.blocks()[1].text, b"embedded\0nul\0");
}
#[test]
fn every_truncated_prefix_of_a_fixed_declared_container_is_refused() {
    let data = container(0, &[b"abc\0", b"", b"xyz\0"]);
    for end in 0..data.len() {
        assert!(Wct::parse(&data[..end]).is_err(), "prefix {end}");
    }
}
#[test]
fn many_present_empty_blocks_round_trip_without_a_guessed_count_cap() {
    let blocks = vec![b"".as_slice(); 257];
    let data = container(0, &blocks);
    let parsed = Wct::parse(&data).unwrap();
    assert_eq!(parsed.blocks().len(), blocks.len());
    assert!(parsed.blocks().iter().all(|b| b.text.is_empty()));
    assert_eq!(parsed.canonical_bytes(), data);
}
