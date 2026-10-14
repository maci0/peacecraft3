use pcx_map::{MapInfo, info::parse_info};
mod player_fixtures {
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

struct Fixture {
    player: player_fixtures::Fixture,
    force_count_at: usize,
    force_starts: Vec<usize>,
    force_bytes: usize,
    table_count_at: usize,
    table_starts: Vec<usize>,
    table_bytes: usize,
}
fn fixture(version: u32, force_names: &[&[u8]], table_names: &[&[u8]]) -> Fixture {
    let mut player = player_fixtures::fixture(version, &[b"Player"]);
    assert_eq!(player.record_starts[0], player.count_at + 4);
    let force_count_at = player.count_at + 4 + player.player_bytes;
    let mut bytes = player.bytes[..force_count_at].to_vec();
    bytes.extend((force_names.len() as u32).to_le_bytes());
    let force_at = bytes.len();
    let mut force_starts = Vec::new();
    for (index, name) in force_names.iter().enumerate() {
        force_starts.push(bytes.len());
        bytes.extend((0xaabb0000u32 + index as u32).to_le_bytes());
        bytes.extend((0x80000000u32 + index as u32).to_le_bytes());
        bytes.extend(*name);
        bytes.push(0);
    }
    let force_bytes = bytes.len() - force_at;
    bytes.extend(1u32.to_le_bytes());
    for word in [9u32, 8, 7, 6] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(1u32.to_le_bytes());
    for word in [5u32, 4] {
        bytes.extend(word.to_le_bytes());
    }
    let table_count_at = bytes.len();
    bytes.extend((table_names.len() as u32).to_le_bytes());
    let table_at = bytes.len();
    let mut table_starts = Vec::new();
    for (index, name) in table_names.iter().enumerate() {
        table_starts.push(bytes.len());
        bytes.extend((70u32 + index as u32).to_le_bytes());
        bytes.extend(*name);
        bytes.push(0);
        for word in [2u32, 11, 12, 1, 17, 21, 22] {
            bytes.extend(word.to_le_bytes());
        }
    }
    let table_bytes = bytes.len() - table_at;
    if version >= 0x18 {
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(80u32.to_le_bytes());
        bytes.extend(b"\xff\0");
        for word in [1u32, 1, 31, 32] {
            bytes.extend(word.to_le_bytes());
        }
    }
    player.bytes = bytes;
    Fixture {
        player,
        force_count_at,
        force_starts,
        force_bytes,
        table_count_at,
        table_starts,
        table_bytes,
    }
}
fn assert_fields(info: &MapInfo, input: &Fixture, force_names: &[&[u8]], table_names: &[&[u8]]) {
    player_fixtures::assert_retained(info, &input.player, &[b"Player"]);
    assert_eq!(info.forces.len(), force_names.len());
    assert_eq!(info.table_lists.len(), table_names.len());
    for (index, (record, name)) in info.forces.iter().zip(force_names).enumerate() {
        assert_eq!(record.flags, 0xaabb0000 + index as u32);
        assert_eq!(record.mask, 0x80000000 + index as u32);
        assert_eq!(record.name, String::from_utf8_lossy(name));
    }
    for (index, (record, name)) in info.table_lists.iter().zip(table_names).enumerate() {
        assert_eq!(record.id, 70 + index as u32);
        assert_eq!(record.name, String::from_utf8_lossy(name));
        assert_eq!(record.first, [11, 12]);
        assert_eq!(record.second.len(), 1);
        assert_eq!(record.second[0].id, 17);
        assert_eq!(record.second[0].tables, [21, 22]);
    }
    assert_eq!(info.start_locations, [[9, 8, 7, 6]]);
    assert_eq!(info.pairs, [[5, 4]]);
    if info.version() == 25 {
        assert_eq!(info.item_tables[0].name, "�");
        assert_eq!(info.item_tables[0].sets, [vec![[31, 32]]]);
    }
}

#[test]
fn force_spans_use_authored_name_bytes() {
    for version in [18, 25] {
        let names: &[&[u8]] = &[b"\xff", b"\xc3\xa9", b""];
        let input = fixture(version, names, &[]);
        let info = MapInfo::from_bytes(input.player.bytes.clone()).unwrap();
        assert_fields(&info, &input, names, &[]);
        assert_eq!(
            info.framing().force_bytes,
            input.force_bytes,
            "v{version}: authored force span"
        );
        assert!(info.framing().sums());
    }
}
#[test]
fn table_spans_include_nested_words_without_lossy_name_expansion() {
    for version in [18, 25] {
        let names: &[&[u8]] = &[b"\xe2\x82", b"\xf0\x9f\x8c\x8d"];
        let input = fixture(version, &[], names);
        let info = parse_info(&input.player.bytes).unwrap();
        assert_fields(&info, &input, &[], names);
        assert_eq!(
            info.framing().table_list_bytes,
            input.table_bytes,
            "v{version}: authored table-list span"
        );
        assert!(info.framing().sums());
    }
}
#[test]
fn authored_spans_are_not_claimed_as_mutated_decoded_record_sizes() {
    let input = fixture(25, &[b"Force"], &[b"Table"]);
    let mut info = MapInfo::parse(&input.player.bytes).unwrap();
    let before = info.framing();
    info.players[0].name = "renamed player".into();
    info.forces[0].name = "a much longer decoded force name".into();
    let old_decoded_len = info.table_lists[0].wire_len();
    info.table_lists[0].name.push_str(" changed");
    info.table_lists[0].first.push(33);
    info.table_lists[0].second[0].tables.push(34);
    assert_eq!(
        info.table_lists[0].wire_len(),
        old_decoded_len + " changed".len() + 8
    );
    let after = info.framing();
    assert_eq!(after.player_bytes, before.player_bytes);
    assert_eq!(after.force_bytes, before.force_bytes);
    assert_eq!(after.table_list_bytes, before.table_list_bytes);
    assert_eq!(info.raw(), input.player.bytes);
    assert_eq!(info.canonical_bytes(), input.player.bytes);
    info.forces.clear();
    info.table_lists.clear();
    let cleared = info.framing();
    assert_eq!((cleared.forces, cleared.table_lists), (0, 0));
    assert_eq!(cleared.force_bytes, input.force_bytes);
    assert_eq!(cleared.table_list_bytes, input.table_bytes);
    assert!(cleared.exact);
    assert!(cleared.sums());
}
#[test]
fn valid_and_empty_sections_and_truncated_grammar_remain_compatible() {
    for version in [18, 25] {
        for names in [&[][..], &[b"valid".as_slice(), b"\xc3\xa9".as_slice()][..]] {
            let input = fixture(version, names, names);
            let info = MapInfo::parse(&input.player.bytes).unwrap();
            assert_fields(&info, &input, names, names);
            assert_eq!(info.framing().force_bytes, input.force_bytes);
            assert_eq!(info.framing().table_list_bytes, input.table_bytes);
        }
        let input = fixture(version, &[b"Force"], &[b"Table"]);
        for end in input.force_starts[0]..input.force_starts[0] + input.force_bytes {
            assert!(MapInfo::parse(&input.player.bytes[..end]).is_err());
        }
        for end in input.table_starts[0]..input.table_starts[0] + input.table_bytes {
            assert!(MapInfo::parse(&input.player.bytes[..end]).is_err());
        }
        for at in [input.force_count_at, input.table_count_at] {
            let mut bytes = input.player.bytes.clone();
            bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(MapInfo::parse(&bytes).is_err());
        }
    }
}
