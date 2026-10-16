use super::*;
use crate::classic_frame::{Decoder, FrameError, FrameLimits, decode_one};

pub(super) fn profile() -> DiscoveryProfile {
    DiscoveryProfile {
        product: [80, 88, 51, 87],
        version: 7,
    }
}

pub(super) fn fixture() -> Vec<u8> {
    // Freely synthetic GHost++ sender fixture: g/m/h, selected product and version.
    let hex = "f7304300505833570700000002000000030000006700000101010101010141a90141112333456d01016901010004000000050000000100000006000000070000001122";
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn encode(stat: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for group in stat.chunks(7) {
        let mask = group
            .iter()
            .enumerate()
            .fold(1, |mask, (index, byte)| mask | ((byte & 1) << (index + 1)));
        out.push(mask);
        out.extend(group.iter().map(|byte| byte | 1));
    }
    out
}

fn with_stat(stat: &[u8]) -> Vec<u8> {
    let original = fixture();
    let mut result = original[..4 + 16 + 1 + 1 + 1].to_vec();
    result.extend(encode(stat));
    result.push(0);
    result.extend(&original[original.len() - 22..]);
    let length = result.len() as u16;
    result[2..4].copy_from_slice(&length.to_le_bytes());
    result
}

#[test]
fn gameinfo_keeps_selected_product_version_and_map() {
    let wire = fixture();
    let info = decode_one(&wire, wire.len())
        .unwrap()
        .game_info(profile(), 18)
        .unwrap();
    assert_eq!(info.profile, profile());
    assert_eq!((info.host_counter, info.entry_key), (2, 3));
    assert_eq!(info.game_name, b"g");
    assert!(std::ptr::eq(info.game_name.as_ptr(), wire[20..].as_ptr()));
    assert_eq!(info.map_flags(), [0; 4]);
    assert_eq!(info.stat_reserved(), 0);
    assert_eq!(info.map_dimensions(), [0x4000, 0x4000]);
    assert_eq!(info.map_path(), b"m");
    assert_eq!(info.host_name(), b"h");
    assert_eq!(info.map_crc(), [0x11, 0x22, 0x33, 0x44]);
    assert_eq!(
        (info.slots_total, info.slots_open, info.uptime, info.port),
        (4, 6, 7, 0x2211)
    );
    assert_eq!(info.game_type, [5, 0, 0, 0]);
    assert_eq!(info.tail_word, [1, 0, 0, 0]);
    assert!(info.stat_extension().is_empty());
    let mut other = profile();
    other.version = 8;
    assert_eq!(
        decode_one(&wire, wire.len())
            .unwrap()
            .game_info(other, 18)
            .unwrap_err(),
        DiscoveryError::Profile
    );
    other = profile();
    other.product = [51, 82, 65, 87];
    assert_eq!(
        decode_one(&wire, wire.len())
            .unwrap()
            .game_info(other, 18)
            .unwrap_err(),
        DiscoveryError::Profile
    );
    assert_eq!(
        decode_one(&wire, wire.len())
            .unwrap()
            .game_info(profile(), 17)
            .unwrap_err(),
        DiscoveryError::StatBudget {
            bytes: 18,
            limit: 17
        }
    );
    assert!(matches!(
        decode_one(&[0xf7, 48, 0, 0x10], 32),
        Err(FrameError::Oversize {
            declared: 4096,
            limit: 32
        })
    ));
    assert_eq!(
        profile().search_request(),
        [0xf7, 47, 16, 0, 80, 88, 51, 87, 7, 0, 0, 0, 0, 0, 0, 0]
    );
    let search = profile().search_request();
    decode_one(&search, search.len())
        .unwrap()
        .check_search_game(profile())
        .unwrap();
    let mut invalid = search;
    invalid[12] = 1;
    assert!(matches!(
        decode_one(&invalid, invalid.len())
            .unwrap()
            .check_search_game(profile()),
        Err(DiscoveryError::Packet(_))
    ));
}

#[test]
fn decoded_extensions_remain_opaque_through_borrowed_and_streamed_callers() {
    let original = fixture();
    let info = decode_one(&original, original.len())
        .unwrap()
        .game_info(profile(), 18)
        .unwrap();
    let mut stat = info.stat.clone();
    let extra: Vec<u8> = (0..20).map(|x| x * 13).collect();
    stat.extend(&extra);
    let wire = with_stat(&stat);
    let decoded = decode_one(&wire, wire.len())
        .unwrap()
        .game_info(profile(), stat.len())
        .unwrap();
    assert_eq!(decoded.stat_extension(), extra);
    assert_eq!(decoded.map_path(), b"m");
    assert_eq!(decoded.host_name(), b"h");
    let mut decoder = Decoder::new(FrameLimits {
        max_frame_bytes: wire.len(),
        max_frames: 1,
    })
    .unwrap();
    assert!(decoder.push(&wire[..10]).unwrap().is_empty());
    let frames = decoder.push(&wire[10..]).unwrap();
    decoder.finish().unwrap();
    assert_eq!(
        frames[0]
            .game_info(profile(), stat.len())
            .unwrap()
            .stat_extension(),
        extra
    );
    let mut high = profile();
    high.version = 0x0102_0304;
    let mut wire = wire;
    wire[8..12].copy_from_slice(&high.version.to_le_bytes());
    assert_eq!(
        decode_one(&wire, wire.len())
            .unwrap()
            .game_info(high, stat.len())
            .unwrap()
            .profile
            .version,
        high.version
    );
}

#[test]
fn malformed_shapes_refuse_before_publishing_gameinfo() {
    let wire = fixture();
    for length in 4..wire.len() {
        let mut short = wire[..length].to_vec();
        short[2..4].copy_from_slice(&(length as u16).to_le_bytes());
        assert!(
            decode_one(&short, short.len())
                .unwrap()
                .game_info(profile(), 1024)
                .is_err()
        );
    }
    for (at, value) in [(0, super::super::BNCS_FAMILY), (1, 4), (23, 2)] {
        let mut bad = wire.clone();
        bad[at] = value;
        assert!(matches!(
            decode_one(&bad, bad.len())
                .unwrap()
                .game_info(profile(), 1024),
            Err(DiscoveryError::Packet(_))
        ));
    }
    let original = decode_one(&wire, wire.len())
        .unwrap()
        .game_info(profile(), 18)
        .unwrap();
    let mut bad = original.stat.clone();
    bad[13..].fill(b'x');
    let packet = with_stat(&bad);
    assert!(matches!(
        decode_one(&packet, packet.len())
            .unwrap()
            .game_info(profile(), 1024),
        Err(DiscoveryError::Packet("unterminated map path"))
    ));
    let mut bad = original.stat.clone();
    *bad.last_mut().unwrap() = 1;
    let packet = with_stat(&bad);
    assert!(matches!(
        decode_one(&packet, packet.len())
            .unwrap()
            .game_info(profile(), 1024),
        Err(DiscoveryError::Packet("missing stat suffix"))
    ));
}
