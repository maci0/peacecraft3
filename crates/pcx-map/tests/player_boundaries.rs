use pcx_map::{
    Error, MapInfo,
    info::{FramingViolation, parse_info},
};
mod fixtures {
    use pcx_map::MapInfo;

    pub struct Fixture {
        pub bytes: Vec<u8>,
        pub count_at: usize,
        pub record_starts: Vec<usize>,
        pub player_bytes: usize,
    }

    pub fn fixture(version: u32, names: &[&[u8]]) -> Fixture {
        let mut bytes = Vec::new();
        for word in [version, 0x10203040, 0x50607080] {
            bytes.extend(word.to_le_bytes());
        }
        for value in ["map", "description", "recommended", "head4"] {
            bytes.extend(value.as_bytes());
            bytes.push(0);
        }
        for value in 0..8 {
            bytes.extend((value as f32).to_le_bytes());
        }
        for word in [1u32, 2, 3, 4, 63, 31, 0xabcdef01] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.push(b'Z');
        bytes.extend(9u32.to_le_bytes());
        if version >= 0x14 {
            bytes.extend(b"first\0");
        }
        bytes.extend(b"fog\0second\0third\0");
        bytes.extend(10u32.to_le_bytes());
        if version >= 0x14 {
            bytes.extend(b"loading\0");
        }
        bytes.extend(b"loadtext\0loadtitle\0loadsubtitle\0");
        if version >= 0x13 {
            bytes.extend(2u32.to_le_bytes());
            for value in [1f32, 2., 3.] {
                bytes.extend(value.to_le_bytes());
            }
            bytes.extend(0x10203040u32.to_le_bytes());
        }
        if version >= 0x15 {
            bytes.extend(0x50607080u32.to_le_bytes());
        }
        if version >= 0x16 {
            bytes.extend(b"sound\0");
        }
        if version >= 0x17 {
            bytes.push(b'R');
        }
        if version >= 0x19 {
            bytes.extend(0x99aabbccu32.to_le_bytes());
        }
        let count_at = bytes.len();
        bytes.extend((names.len() as u32).to_le_bytes());
        let records_at = bytes.len();
        let mut record_starts = Vec::new();
        for (ordinal, name) in names.iter().enumerate() {
            record_starts.push(bytes.len());
            for word in [0x10203040u32 + ordinal as u32, 0x50607080, 99, 17] {
                bytes.extend(word.to_le_bytes());
            }
            bytes.extend(*name);
            bytes.push(0);
            bytes.extend((-1.25f32).to_le_bytes());
            bytes.extend(9.5f32.to_le_bytes());
            bytes.extend(0x11223344u32.to_le_bytes());
            bytes.extend(0xaabbccddu32.to_le_bytes());
        }
        let player_bytes = bytes.len() - records_at;
        // Force/start-location/pair/table-list sections, plus v25 item tables.
        for _ in 0..4 {
            bytes.extend(0u32.to_le_bytes());
        }
        if version >= 0x18 {
            bytes.extend(0u32.to_le_bytes());
        }
        Fixture {
            bytes,
            count_at,
            record_starts,
            player_bytes,
        }
    }

    pub fn assert_retained(info: &MapInfo, input: &Fixture, names: &[&[u8]]) {
        assert_eq!(info.raw(), input.bytes);
        assert_eq!(info.canonical_bytes(), input.bytes);
        assert_eq!(info.constant_a, 0x10203040);
        assert_eq!(info.constant_b, Some(0x50607080));
        assert_eq!(info.flags, 0xabcdef01);
        assert_eq!(info.players().len(), names.len());
        for (ordinal, (player, name)) in info.players().iter().zip(names).enumerate() {
            assert_eq!(player.controller, 0x10203040 + ordinal as u32);
            assert_eq!(player.race, 0x50607080);
            assert_eq!(player.start_point, 99);
            assert_eq!(player.colour, 17);
            assert_eq!(player.name, String::from_utf8_lossy(name));
            assert_eq!((player.start_x, player.start_y), (-1.25, 9.5));
            assert_eq!(
                (player.ally_priorities, player.ally_mask),
                (0x11223344, 0xaabbccdd)
            );
        }
    }
}
use fixtures::{assert_retained, fixture};

#[test]
fn authored_player_spans_survive_lossy_name_decoding() {
    for version in [18, 25] {
        let names: &[&[u8]] = &[b"\xff", b"\xc3\xa9", b"\xe2\x82", b""];
        let input = fixture(version, names);
        let info = MapInfo::from_bytes(input.bytes.clone()).unwrap();
        assert_retained(&info, &input, names);
        assert_eq!(
            info.framing().player_bytes,
            input.player_bytes,
            "v{version}: player span must use authored bytes"
        );
        assert_eq!(
            parse_info(&input.bytes).unwrap().framing().player_bytes,
            input.player_bytes
        );
        assert!(info.framing().sums());
    }
}

#[test]
fn valid_names_and_empty_player_sections_remain_compatible() {
    for version in [18, 25] {
        for names in [
            &[][..],
            &[b"one".as_slice(), b"\xc3\xa9".as_slice(), b""][..],
        ] {
            let input = fixture(version, names);
            let info = MapInfo::parse(&input.bytes).unwrap();
            assert_retained(&info, &input, names);
            assert_eq!(info.framing().player_bytes, input.player_bytes);
            assert!(info.framing().sums());
            assert!(info.framing().exact);
        }
    }
}

#[test]
fn every_partial_player_span_is_refused_without_defaults() {
    for version in [18, 25] {
        let input = fixture(version, &[b"player"]);
        let record_at = input.record_starts[0];
        let record_end = record_at + input.player_bytes;
        for end in record_at..record_end {
            assert!(
                MapInfo::parse(&input.bytes[..end]).is_err(),
                "v{version}: player prefix {end}"
            );
        }
        for count in [2u32, u32::MAX] {
            let mut bytes = input.bytes.clone();
            bytes[input.count_at..input.count_at + 4].copy_from_slice(&count.to_le_bytes());
            assert!(matches!(MapInfo::parse(&bytes), Err(Error::InfoFraming(_))));
        }
        let name_at = record_at + 16;
        let mut bytes = input.bytes[..name_at].to_vec();
        bytes.extend(b"unterminated");
        assert!(
            matches!(MapInfo::parse(&bytes),Err(Error::InfoFraming(FramingViolation::UnterminatedString{at})) if at==name_at)
        );
        let mut trailing = input.bytes.clone();
        trailing.push(0xff);
        assert!(matches!(
            MapInfo::parse(&trailing),
            Err(Error::InfoFraming(FramingViolation::TrailingBytes {
                have: 1,
                ..
            }))
        ));
    }
}

#[test]
fn player_roles_use_the_four_authored_words_without_renumbering_sparse_slots() {
    for version in [18, 25] {
        let mut input = fixture(version, &[b"user", b"neutral", b"computer"]);
        for (&at, words) in
            input
                .record_starts
                .iter()
                .zip([[8u32, 1, 2, 1], [1, 3, 4, 0], [0, 2, 3, 7]])
        {
            for (index, word) in words.into_iter().enumerate() {
                input.bytes[at + index * 4..at + index * 4 + 4]
                    .copy_from_slice(&word.to_le_bytes());
            }
        }
        let info = MapInfo::from_bytes(input.bytes.clone()).unwrap();
        assert_eq!(info.canonical_bytes(), input.bytes);
        assert_eq!(
            info.players()
                .iter()
                .map(|p| p.number())
                .collect::<Vec<_>>(),
            vec![8, 1, 0]
        );
        assert_eq!(
            info.human_players().map(|p| p.number()).collect::<Vec<_>>(),
            vec![8]
        );
        assert_eq!(info.race_counts(), [(2, 1), (3, 1), (4, 1)].into());
        assert_eq!(info.players()[0].fixed_start(), Some(true));
        assert_eq!(info.players()[1].fixed_start(), Some(false));
        assert_eq!(info.players()[2].fixed_start(), None);
    }
}
