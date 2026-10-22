use super::*;

const WAR3_DESCRIPTOR: u32 = 0x112d8b0;
const WAR3_LOCAL_DESCRIPTOR: u32 = 0x112d9b8;
const WAR3X_DESCRIPTOR: u32 = 0x112dac0;
const WAR3X_LOCAL_DESCRIPTOR: u32 = 0x112dbc8;
const MOD_DESCRIPTOR: u32 = 0x112dcd0;
const DEPRECATED_DESCRIPTOR: u32 = 0x112ddd8;
const WAR3_HANDLE: u32 = 0x112deec;
const WAR3X_HANDLE: u32 = 0x112def0;
const WAR3_LOCAL_HANDLE: u32 = 0x112def4;
const WAR3X_LOCAL_HANDLE: u32 = 0x112def8;
const MOD_HANDLE: u32 = 0x112d898;
const DEPRECATED_HANDLE: u32 = 0x112d89c;

struct Fixture<'a> {
    names: [&'a str; 4],
    special: [&'a str; 2],
    selected_priorities: [u8; 4],
    datasets: [&'a str; 2],
    format: &'a str,
    start: u8,
    step: u8,
    mod_priority: u8,
    deprecated_priority: u8,
    mask: u8,
}

impl Default for Fixture<'_> {
    fn default() -> Self {
        Self {
            names: ["a.mpq", "b.mpq", "c.mpq", "d.mpq"],
            special: ["mod.mpq", "old.mpq"],
            selected_priorities: [0x60, 0x61, 0x62, 0x63],
            datasets: ["Green", "Blue"],
            format: "%s_V%d",
            start: 7,
            step: 2,
            mod_priority: 0x33,
            deprecated_priority: 0x44,
            mask: 0x80,
        }
    }
}

fn fixture(f: Fixture<'_>) -> Result<ImageRules, Error> {
    rules(f, |_, _| Ok(()))
}

fn push_va(opcode: u8, value: u32) -> Vec<u8> {
    let mut bytes = vec![opcode];
    bytes.extend(value.to_le_bytes());
    bytes
}

fn mov_table_op(value: u32) -> Vec<u8> {
    let mut bytes = vec![0x8b, 0x8f];
    bytes.extend(value.to_le_bytes());
    bytes
}

fn lea_op(value: u32) -> Vec<u8> {
    let mut bytes = vec![0x8d, 0x87];
    bytes.extend(value.to_le_bytes());
    bytes
}

fn table_op(value: u32, modrm: u8) -> Vec<u8> {
    let mut bytes = vec![0x8b, modrm, 0x85];
    bytes.extend(value.to_le_bytes());
    bytes
}

fn literal_site(source: u32, destination: u32) -> Vec<u8> {
    let mut bytes = push_va(0x68, 64);
    bytes.extend(push_va(0x68, source));
    bytes.extend(push_va(0x68, destination));
    bytes.push(0xe8);
    bytes
}

fn local_site(suffix: u32, destination: u32) -> Vec<u8> {
    let mut bytes = push_va(0x68, suffix);
    bytes.push(0x56);
    bytes.extend(push_va(0x68, 0xe68a28));
    bytes.extend(push_va(0x68, 0x104));
    bytes.push(0x68);
    bytes.extend(destination.to_le_bytes());
    bytes.push(0xe8);
    bytes
}

fn fallback_site(source: u32, destination: u32) -> Vec<u8> {
    let mut bytes = push_va(0x68, 64);
    bytes.extend(push_va(0x68, source));
    bytes.extend(push_va(0x68, destination));
    bytes.push(0xe8);
    bytes.resize(20, 0);
    bytes
}

fn handle_table(values: [u32; 2]) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}

fn descriptor_table(values: [u32; 2]) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}

fn rules(
    f: Fixture<'_>,
    mut alter: impl FnMut(u32, &mut Vec<u8>) -> Result<(), String>,
) -> Result<ImageRules, Error> {
    let strings = [
        (0xe68900u32, f.names[0]),
        (0xe6890c, f.names[1]),
        (0xe6891c, f.names[2]),
        (0xe68928, f.names[3]),
        (0xe68938, f.special[0]),
        (0xe68944, f.special[1]),
        (0xe68980, "enUS"),
        (0xe68994, f.format),
        (0xe68a28, "%s-%s"),
        (0xe69000, f.datasets[0]),
        (0xe69010, f.datasets[1]),
    ];
    let limit = f.start.wrapping_add(f.step.wrapping_mul(2));
    ImageRules::decode(
        AssetIdentity {
            source: "synthetic:campaign".into(),
            edition: "synthetic".into(),
            version: "1".into(),
        },
        sha256(&[]),
        |va, n| {
            let mut bytes = match va {
                0x71dbd6 => vec![
                    0x8b, 0x88, 0x44, 0x1e, 0x00, 0x00, 0x33, 0xc0, 0x83, 0xf9, 2, 0x0f, 0x46, 0xc1,
                ],
                0x7046c5 => vec![0x8b, 0x80, 0x40, 0x01, 0x00, 0x00, 0x83, 0xe0, f.mask],
                0x704701 => vec![0x8b, 0x85, 0xbc, 0xcb, 0xff, 0xff, 0x83, 0xe0, f.mask],
                0x428dd8 => vec![0xbe, f.start, 0, 0, 0],
                0x428e2f => vec![0x83, 0xc6, f.step],
                0x428e32 => vec![0x83, 0xc7, 4],
                0x428e35 => vec![0x83, 0xfe, limit],
                0x428de0 => mov_table_op(0x10db140),
                0x428dea => lea_op(WAR3_HANDLE),
                0x428e0a => mov_table_op(0x10db148),
                0x428e19 => lea_op(WAR3_LOCAL_HANDLE),
                0x428e43 => push_va(0x68, MOD_DESCRIPTOR),
                0x428e3a => push_va(0x68, MOD_HANDLE),
                0x428e56 => push_va(0x68, DEPRECATED_DESCRIPTOR),
                0x428e4d => push_va(0x68, DEPRECATED_HANDLE),
                0x428e41 => vec![0x6a, f.mod_priority],
                0x428e54 => vec![0x6a, f.deprecated_priority],
                0x428be9 => literal_site(0xe68900, WAR3_DESCRIPTOR),
                0x428bff => literal_site(0xe6891c, WAR3X_DESCRIPTOR),
                0x428c13 => literal_site(0xe68938, MOD_DESCRIPTOR),
                0x428c27 => literal_site(0xe68944, DEPRECATED_DESCRIPTOR),
                0x428c3b => local_site(0xe6890c, WAR3_LOCAL_DESCRIPTOR),
                0x428c58 => local_site(0xe68928, WAR3X_LOCAL_DESCRIPTOR),
                0x428c83 => fallback_site(0xe6890c, WAR3_LOCAL_DESCRIPTOR),
                0x428cab => fallback_site(0xe68928, WAR3X_LOCAL_DESCRIPTOR),
                0x42905b => vec![0x6a, f.selected_priorities[0]],
                0x42907f => vec![0x6a, f.selected_priorities[1]],
                0x42909d => vec![0x6a, f.selected_priorities[2]],
                0x4290bb => vec![0x6a, f.selected_priorities[3]],
                0x429009 => table_op(0x10db170, 0x0c),
                0x429024 => table_op(0x10db180, 0x0c),
                0x429010 => table_op(0x10db178, 0x1c),
                0x429017 => table_op(0x10db188, 0x3c),
                0x10db140 => descriptor_table([WAR3_DESCRIPTOR, WAR3X_DESCRIPTOR]),
                0x10db148 => descriptor_table([WAR3_LOCAL_DESCRIPTOR, WAR3X_LOCAL_DESCRIPTOR]),
                0x10db170 => handle_table([WAR3_HANDLE, WAR3X_HANDLE]),
                0x10db178 => handle_table([WAR3_LOCAL_HANDLE, WAR3X_LOCAL_HANDLE]),
                0x10db180 | 0x10db188 => handle_table([MOD_HANDLE; 2]),
                0x10db190 => {
                    let mut table = Vec::new();
                    table.extend(0xe69000u32.to_le_bytes());
                    table.extend(0xe69010u32.to_le_bytes());
                    table
                }
                _ => strings
                    .iter()
                    .find(|(string_va, _)| *string_va == va)
                    .map(|(_, text)| text.as_bytes().to_vec())
                    .ok_or_else(|| format!("unexpected read at {va:#x}"))?,
            };
            if bytes.len() < n {
                bytes.resize(n, 0);
            }
            bytes.truncate(n);
            alter(va, &mut bytes)?;
            Ok(bytes)
        },
    )
}

#[test]
fn operands_not_fixture_semantics_drive_rules() {
    let rules = fixture(Fixture::default()).unwrap();
    assert_eq!(rules.selector_clamp, 2);
    assert_eq!(rules.flag_mask, 0x80);
    assert_eq!(rules.datasets, ["Green", "Blue"]);
    assert_eq!(
        rules.base[0],
        MountSpec {
            archive: "a.mpq".into(),
            priority: 7
        }
    );
    assert_eq!(
        rules.base[1],
        MountSpec {
            archive: "b.mpq".into(),
            priority: 8
        }
    );
    assert_eq!(
        rules.base[2],
        MountSpec {
            archive: "c.mpq".into(),
            priority: 9
        }
    );
    assert_eq!(
        rules.base[3],
        MountSpec {
            archive: "d.mpq".into(),
            priority: 10
        }
    );
    assert_eq!(rules.base[4].priority, 0x33);
    assert_eq!(rules.base[5].priority, 0x44);
    assert_eq!(
        rules.selected[0],
        [
            MountSpec {
                archive: "a.mpq".into(),
                priority: 0x60
            },
            MountSpec {
                archive: "mod.mpq".into(),
                priority: 0x63
            },
            MountSpec {
                archive: "b.mpq".into(),
                priority: 0x62
            },
        ]
    );
    assert_eq!(
        rules.selected[1],
        [
            MountSpec {
                archive: "c.mpq".into(),
                priority: 0x60
            },
            MountSpec {
                archive: "mod.mpq".into(),
                priority: 0x63
            },
            MountSpec {
                archive: "d.mpq".into(),
                priority: 0x62
            },
        ]
    );
}

#[test]
fn selected_tables_are_followed_through_base_handle_identity() {
    let rules = rules(Fixture::default(), |va, bytes| {
        if va == 0x10db170 {
            bytes[..4].copy_from_slice(&WAR3_LOCAL_HANDLE.to_le_bytes());
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(rules.selected[0][0].archive, "b.mpq");
    assert_eq!(rules.selected[1][0].archive, "c.mpq");
}

#[test]
fn unlisted_selected_handle_refuses() {
    assert!(
        rules(Fixture::default(), |va, bytes| {
            if va == 0x10db170 {
                bytes[..4].copy_from_slice(&0xdead_beefu32.to_le_bytes());
            }
            Ok(())
        })
        .is_err()
    );
}

#[test]
fn local_archive_name_fallback_drives_descriptor() {
    let rules = rules(Fixture::default(), |va, bytes| {
        if va == 0x428c83 {
            bytes[..5].fill(0);
        }
        Ok(())
    });
    assert!(rules.is_err());
}

#[test]
fn war3patch_presence_refuses_before_archive_selection() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pcx-campaign-war3patch-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("war3patch.mpq"), []).unwrap();
    assert!(refuse_war3patch(&dir).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn unresolved_locale_campaign_archive_refuses_before_archive_selection() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let rules = fixture(Fixture::default()).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "pcx-campaign-locale-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("enUS-b.mpq"), []).unwrap();
    assert!(refuse_unresolved_locale(&dir, &rules).is_err());
    std::fs::remove_file(dir.join("enUS-b.mpq")).unwrap();
    std::fs::write(dir.join("b.mpq"), []).unwrap();
    assert!(refuse_unresolved_locale(&dir, &rules).is_ok());
    std::fs::remove_dir_all(&dir).unwrap();
}

fn encrypt_table(data: &mut [u8], key: u32) {
    let table = pcx_mpq::crypt::table();
    let (mut seed1, mut seed2) = (key, 0xeeee_eeeeu32);
    for chunk in data.as_chunks_mut::<4>().0 {
        seed2 = seed2.wrapping_add(table[0x400 + (seed1 & 255) as usize]);
        let plain = u32::from_le_bytes(*chunk);
        chunk.copy_from_slice(&(plain ^ seed1.wrapping_add(seed2)).to_le_bytes());
        seed1 = ((!seed1) << 21).wrapping_add(0x1111_1111) | (seed1 >> 11);
        seed2 = plain
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
}

fn write_table_archive(
    path: &Path,
    listed: &str,
    hash_matches: bool,
    block_index: u32,
    blocks: &[pcx_mpq::BlockEntry],
) {
    let hash_at = 32u32;
    let block_at = hash_at + 16;
    let archive_size = block_at + blocks.len() as u32 * 16;
    let mut hashes = [0xffu8; 16];
    if hash_matches {
        let (name1, name2) = pcx_mpq::hash::hash_string_both(listed);
        hashes[..4].copy_from_slice(&name1.to_le_bytes());
        hashes[4..8].copy_from_slice(&name2.to_le_bytes());
        hashes[12..].copy_from_slice(&block_index.to_le_bytes());
    }
    encrypt_table(&mut hashes, pcx_mpq::hash::hash_table_key());
    let mut table = Vec::new();
    for block in blocks {
        table.extend(block.file_pos.to_le_bytes());
        table.extend(block.compressed_size.to_le_bytes());
        table.extend(block.file_size.to_le_bytes());
        table.extend(block.flags.to_le_bytes());
    }
    encrypt_table(&mut table, pcx_mpq::hash::block_table_key());
    let mut raw = b"MPQ\x1a".to_vec();
    raw.extend(32u32.to_le_bytes());
    raw.extend(archive_size.to_le_bytes());
    raw.extend(0u16.to_le_bytes());
    raw.extend(0u16.to_le_bytes());
    raw.extend(hash_at.to_le_bytes());
    raw.extend(block_at.to_le_bytes());
    raw.extend(1u32.to_le_bytes());
    raw.extend((blocks.len() as u32).to_le_bytes());
    raw.extend(hashes);
    raw.extend(table);
    assert_eq!(raw.len(), archive_size as usize);
    std::fs::write(path, raw).unwrap();
}

fn write_candidate_archive(
    path: &Path,
    listed: &str,
    candidates: &[(u16, u16, u32)],
    blocks: &[pcx_mpq::BlockEntry],
) {
    let hash_at = 32u32;
    let hash_count = candidates.len() as u32 + 1;
    let block_at = hash_at + hash_count * 16;
    let archive_size = block_at + blocks.len() as u32 * 16;
    let start = pcx_mpq::hash::start_slot(listed, hash_count as usize);
    let (name1, name2) = pcx_mpq::hash::hash_string_both(listed);
    let mut hashes = vec![0xffu8; hash_count as usize * 16];
    for (offset, (locale, platform, block_index)) in candidates.iter().copied().enumerate() {
        let slot = (start + offset) % hash_count as usize;
        let entry = &mut hashes[slot * 16..slot * 16 + 16];
        entry[..4].copy_from_slice(&name1.to_le_bytes());
        entry[4..8].copy_from_slice(&name2.to_le_bytes());
        entry[8..10].copy_from_slice(&locale.to_le_bytes());
        entry[10..12].copy_from_slice(&platform.to_le_bytes());
        entry[12..].copy_from_slice(&block_index.to_le_bytes());
    }
    encrypt_table(&mut hashes, pcx_mpq::hash::hash_table_key());
    let mut table = Vec::new();
    for block in blocks {
        table.extend(block.file_pos.to_le_bytes());
        table.extend(block.compressed_size.to_le_bytes());
        table.extend(block.file_size.to_le_bytes());
        table.extend(block.flags.to_le_bytes());
    }
    encrypt_table(&mut table, pcx_mpq::hash::block_table_key());
    let mut raw = b"MPQ\x1a".to_vec();
    raw.extend(32u32.to_le_bytes());
    raw.extend(archive_size.to_le_bytes());
    raw.extend(0u16.to_le_bytes());
    raw.extend(0u16.to_le_bytes());
    raw.extend(hash_at.to_le_bytes());
    raw.extend(block_at.to_le_bytes());
    raw.extend(hash_count.to_le_bytes());
    raw.extend((blocks.len() as u32).to_le_bytes());
    raw.extend(hashes);
    raw.extend(table);
    assert_eq!(raw.len(), archive_size as usize);
    std::fs::write(path, raw).unwrap();
}

fn table_temp_dir(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pcx-campaign-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn canonical_members_keeps_tombstones_and_stale_listings_absent() {
    let dir = table_temp_dir("tombstone");
    let member = "scripts\\common.j";
    let live = pcx_mpq::BlockEntry {
        flags: pcx_mpq::flags::EXISTING,
        ..pcx_mpq::BlockEntry::default()
    };
    let stale = dir.join("stale.mpq");
    write_table_archive(&stale, member, false, 0, &[live]);
    assert!(
        canonical_members(&pcx_mpq::MpqArchive::open(&stale).unwrap(), member)
            .unwrap()
            .is_empty()
    );
    let tombstone = dir.join("tombstone.mpq");
    write_table_archive(
        &tombstone,
        member,
        true,
        0,
        &[pcx_mpq::BlockEntry {
            flags: pcx_mpq::flags::DELETE_MARKER,
            ..pcx_mpq::BlockEntry::default()
        }],
    );
    assert!(
        canonical_members(&pcx_mpq::MpqArchive::open(&tombstone).unwrap(), member)
            .unwrap()
            .is_empty()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn canonical_members_refuses_mixed_live_and_tombstone_candidates_before_fallback() {
    let dir = table_temp_dir("mixed-candidates");
    let member = "scripts\\common.j";
    let live = pcx_mpq::BlockEntry {
        flags: pcx_mpq::flags::EXISTING,
        ..pcx_mpq::BlockEntry::default()
    };
    let tombstone = pcx_mpq::BlockEntry {
        flags: pcx_mpq::flags::DELETE_MARKER,
        ..pcx_mpq::BlockEntry::default()
    };
    for (name, candidates) in [
        ("deleted-first.mpq", [(0, 0, 0), (0, 0, 1)]),
        ("live-first.mpq", [(0, 0, 1), (0, 0, 0)]),
    ] {
        let archive = dir.join(name);
        write_candidate_archive(&archive, member, &candidates, &[tombstone, live]);
        let error = canonical_members(&pcx_mpq::MpqArchive::open(&archive).unwrap(), member)
            .unwrap_err()
            .to_string();
        assert!(error.contains(name), "{error}");
        assert!(error.contains("common.j"), "{error}");
        assert!(error.contains("both live and deleted"), "{error}");
    }
    let low = dir.join("low.mpq");
    write_table_archive(&low, member, true, 0, &[live]);
    assert_eq!(
        canonical_members(&pcx_mpq::MpqArchive::open(&low).unwrap(), member).unwrap(),
        [("scripts/common.j".to_string(), member.to_string())],
        "the lower archive is readable, so hiding the mixed high candidate would activate it"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn canonical_members_refuses_a_corrupt_winner_without_falling_back() {
    let dir = table_temp_dir("corrupt-winner");
    let member = "scripts\\common.j";
    let missing_block = dir.join("high-missing-block.mpq");
    write_table_archive(&missing_block, member, true, 7, &[]);
    let error = canonical_members(&pcx_mpq::MpqArchive::open(&missing_block).unwrap(), member)
        .unwrap_err()
        .to_string();
    assert!(error.contains("high-missing-block.mpq"), "{error}");
    assert!(error.contains("common.j"), "{error}");
    assert!(error.contains("outside the block table"), "{error}");
    let non_existent = dir.join("high-non-existent.mpq");
    write_table_archive(
        &non_existent,
        member,
        true,
        0,
        &[pcx_mpq::BlockEntry::default()],
    );
    let error = canonical_members(&pcx_mpq::MpqArchive::open(&non_existent).unwrap(), member)
        .unwrap_err()
        .to_string();
    assert!(error.contains("high-non-existent.mpq"), "{error}");
    assert!(error.contains("common.j"), "{error}");
    assert!(error.contains("non-existent block"), "{error}");
    let low = dir.join("low.mpq");
    write_table_archive(
        &low,
        member,
        true,
        0,
        &[pcx_mpq::BlockEntry {
            flags: pcx_mpq::flags::EXISTING,
            ..pcx_mpq::BlockEntry::default()
        }],
    );
    assert_eq!(
        canonical_members(&pcx_mpq::MpqArchive::open(&low).unwrap(), member).unwrap(),
        [("scripts/common.j".to_string(), member.to_string())]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn provided_common_must_match_the_archive_selected_common() {
    assert!(matching_common_sha256(b"outer", b"archive").is_err());
    assert_eq!(
        matching_common_sha256(b"same", b"same").unwrap(),
        sha256(b"same")
    );
}

#[test]
fn namespace_stripping_exposes_only_members_under_the_selected_prefix() {
    let raw = vec![
        (
            "scripts/common.j".to_string(),
            "Scripts\\Common.j".to_string(),
        ),
        (
            "scripts/other.j".to_string(),
            "Scripts\\Other.j".to_string(),
        ),
        ("units/orc.txt".to_string(), "Units\\Orc.txt".to_string()),
    ];
    let stripped = stripped_members(&raw, "Scripts").unwrap();
    assert_eq!(
        stripped,
        [
            ("common.j".to_string(), "Scripts\\Common.j".to_string()),
            ("other.j".to_string(), "Scripts\\Other.j".to_string()),
        ]
    );
}

#[test]
fn selector_and_flags_choose_the_dataset_and_format_prefix() {
    let rules = fixture(Fixture::default()).unwrap();
    let cases = [
        (Some(1), 0, "Blue_V0"),
        (Some(1), 0x80, "Blue_V0"),
        (Some(2), 0, "Green_V0"),
        (Some(2), 0x80, "Green_V0"),
        (Some(0), 0, "Blue_V0"),
        (Some(0), 0x80, "Green_V0"),
        (Some(3), 0x80, "Green_V0"),
        (None, 0, "Blue_V0"),
    ];
    for (loading_id, flags, expected) in cases {
        let selected = rules.select(loading_id, flags, 0).unwrap();
        assert_eq!(selected.prefix, expected);
    }
    let selected = rules.select(Some(2), 0, 1).unwrap();
    assert_eq!(selected.prefix, "Green_V1");
    assert!(rules.select(Some(2), 0, 2).is_err());
}

#[test]
fn namespace_plan_prefers_present_prefixed_members_then_base() {
    let rules = fixture(Fixture {
        format: "ns/%d/%s",
        ..Fixture::default()
    })
    .unwrap();
    let selection = rules.select(Some(2), 0, 1).unwrap();
    let layers = rules
        .plan_layers(
            &selection,
            &["Units/MiscGame.txt", "Shared.txt"],
            |archive, member| {
                matches!(
                    (archive, member),
                    ("mod.mpq", "ns/1/Green\\Units\\MiscGame.txt")
                        | ("a.mpq", "Units/MiscGame.txt")
                        | ("c.mpq", "Shared.txt")
                )
            },
        )
        .unwrap();
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0].prefix.as_deref(), Some("ns/1/Green"));
    assert_eq!(
        layers[0].members[0].member,
        "ns/1/Green\\Units\\MiscGame.txt"
    );
    assert_eq!(layers[1].members[0].asset, "Shared.txt");
    assert_eq!(layers[2].prefix, None);
    assert_eq!(layers[2].members[0].asset, "Units/MiscGame.txt");
}

#[test]
fn malformed_windows_and_ambiguous_priorities_refuse() {
    assert!(
        fixture(Fixture {
            selected_priorities: [7, 8, 9, 10],
            ..Fixture::default()
        })
        .is_err()
    );
    assert!(
        fixture(Fixture {
            datasets: ["same", "same"],
            ..Fixture::default()
        })
        .is_err()
    );
    assert!(
        fixture(Fixture {
            format: "%s_V%s",
            ..Fixture::default()
        })
        .is_err()
    );
    assert!(
        fixture(Fixture {
            names: ["War3Patch.mpq", "b.mpq", "c.mpq", "d.mpq"],
            ..Fixture::default()
        })
        .is_err()
    );
    assert!(
        fixture(Fixture {
            datasets: ["", "Blue"],
            ..Fixture::default()
        })
        .is_err()
    );
}

#[test]
fn malformed_instruction_windows_refuse() {
    assert!(
        rules(Fixture::default(), |va, bytes| {
            if va == 0x71dbd6 {
                bytes[0] = 0;
            }
            Ok(())
        })
        .is_err()
    );
    assert!(
        rules(Fixture::default(), |va, bytes| {
            if va == 0x10db190 {
                bytes[..4].fill(0);
            }
            Ok(())
        })
        .is_err()
    );
}

fn common(global: &str) -> String {
    format!(
        "type version extends handle\n\
         constant native ConvertVersion takes integer i returns version\n\
         globals\n\
         constant version {global}\n\
         endglobals\n"
    )
}

#[test]
fn edition_is_read_from_the_selected_common_declaration() {
    let low = common("VERSION_REIGN_OF_CHAOS=ConvertVersion(60-60)");
    assert_eq!(
        edition_from_common(low.as_bytes(), "VERSION_REIGN_OF_CHAOS").unwrap(),
        0
    );
    let high = common("VERSION_FROZEN_THRONE=ConvertVersion(60+3)");
    assert_eq!(
        edition_from_common(high.as_bytes(), "VERSION_FROZEN_THRONE").unwrap(),
        63
    );
    let changed = common("VERSION_FROZEN_THRONE=ConvertVersion(61+3)");
    assert_eq!(
        edition_from_common(changed.as_bytes(), "VERSION_FROZEN_THRONE").unwrap(),
        64
    );
}

#[test]
fn missing_ambiguous_or_malformed_edition_declarations_refuse() {
    let missing = common("VERSION_FROZEN_THRONE=ConvertVersion(1)");
    assert!(edition_from_common(missing.as_bytes(), "VERSION_REIGN_OF_CHAOS").is_err());

    let ambiguous = "type version extends handle\n\
        constant native ConvertVersion takes integer i returns version\n\
        globals\n\
        constant version VERSION_FROZEN_THRONE=ConvertVersion(1)\n\
        constant version VERSION_FROZEN_THRONE=ConvertVersion(2)\n\
        endglobals\n";
    assert!(edition_from_common(ambiguous.as_bytes(), "VERSION_FROZEN_THRONE").is_err());

    for source in [
        "constant native ConvertVersion takes integer i returns version\n\
         globals\nconstant version VERSION_FROZEN_THRONE=ConvertVersion(1)\nendglobals\n",
        "type version extends handle\n\
         native ConvertVersion takes integer i returns version\n\
         globals\nconstant version VERSION_FROZEN_THRONE=ConvertVersion(1)\nendglobals\n",
        "type version extends handle\n\
         constant native ConvertVersion takes nothing returns version\n\
         globals\nconstant version VERSION_FROZEN_THRONE=ConvertVersion(1)\nendglobals\n",
        "type version extends handle\n\
         constant native ConvertVersion takes integer i returns integer\n\
         globals\nconstant version VERSION_FROZEN_THRONE=ConvertVersion(1)\nendglobals\n",
        "type version extends handle\n\
         constant native ConvertVersion takes integer i returns version\n\
         globals\nversion VERSION_FROZEN_THRONE=ConvertVersion(1)\nendglobals\n",
    ] {
        assert!(edition_from_common(source.as_bytes(), "VERSION_FROZEN_THRONE").is_err());
    }
}

#[test]
#[ignore = "requires explicit PCX_CAMPAIGN_IMAGE selected original executable"]
fn local_original_rules_match_reviewed_operands() {
    let path = std::env::var("PCX_CAMPAIGN_IMAGE").expect("explicit original executable");
    let image = std::fs::read(&path).unwrap();
    let rules = ImageRules::read(
        &image,
        AssetIdentity {
            source: path,
            edition: "Legacy".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    assert_eq!(rules.selector_clamp, 2);
    assert_eq!(rules.flag_mask, 4);
    assert_eq!(rules.namespace_format, "%s_V%d");
    assert_eq!(rules.datasets, ["Melee", "Custom"]);
    let base: Vec<_> = rules
        .base
        .iter()
        .map(|mount| (mount.archive.as_str(), mount.priority))
        .collect();
    assert_eq!(
        base,
        [
            ("War3.mpq", 1),
            ("War3Local.mpq", 2),
            ("War3x.mpq", 3),
            ("War3xLocal.mpq", 4),
            ("War3Mod.mpq", 9),
            ("Deprecated.mpq", 0),
        ]
    );
    let roc: Vec<_> = rules.selected[0]
        .iter()
        .map(|mount| (mount.archive.as_str(), mount.priority))
        .collect();
    assert_eq!(
        roc,
        [
            ("War3.mpq", 0xa),
            ("War3Mod.mpq", 0xd),
            ("War3Local.mpq", 0xb),
        ]
    );
    let tft: Vec<_> = rules.selected[1]
        .iter()
        .map(|mount| (mount.archive.as_str(), mount.priority))
        .collect();
    assert_eq!(
        tft,
        [
            ("War3x.mpq", 0xa),
            ("War3Mod.mpq", 0xd),
            ("War3xLocal.mpq", 0xb),
        ]
    );
}

#[test]
#[ignore = "requires explicit PCX_CAMPAIGN_COMMON selected original common.j"]
fn local_original_common_declares_the_two_edition_enumerands() {
    let path = std::env::var("PCX_CAMPAIGN_COMMON").expect("explicit original common.j");
    let common = std::fs::read(path).unwrap();
    assert_eq!(
        edition_from_common(&common, "VERSION_REIGN_OF_CHAOS").unwrap(),
        0
    );
    assert_eq!(
        edition_from_common(&common, "VERSION_FROZEN_THRONE").unwrap(),
        1
    );
}

fn map_info(loading_id: u32, flags: u32) -> MapInfo {
    let mut bytes = Vec::new();
    for word in [18u32, 0, 0] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend([0; 4]);
    bytes.extend([0; 32 + 16 + 8]);
    bytes.extend(flags.to_le_bytes());
    bytes.push(0);
    bytes.extend(0u32.to_le_bytes());
    bytes.extend([0; 3]);
    bytes.extend(loading_id.to_le_bytes());
    bytes.extend([0; 3]);
    bytes.extend(0u32.to_le_bytes());
    bytes.extend([0; 16]);
    MapInfo::parse(&bytes).expect("synthetic map info")
}

#[test]
#[ignore = "requires PCX_CAMPAIGN_INSTALL, PCX_CAMPAIGN_IMAGE and PCX_CAMPAIGN_COMMON"]
fn local_original_recipe_opens_a_complete_campaign_reader() {
    let install = std::env::var("PCX_CAMPAIGN_INSTALL").expect("explicit original install");
    let image_path = std::env::var("PCX_CAMPAIGN_IMAGE").expect("explicit original executable");
    let common_path = std::env::var("PCX_CAMPAIGN_COMMON").expect("explicit original common.j");
    let image = std::fs::read(&image_path).unwrap();
    let common = std::fs::read(&common_path).unwrap();
    let rules = ImageRules::read(
        &image,
        AssetIdentity {
            source: image_path,
            edition: "Legacy".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    let recipe = CampaignRecipe::pin(
        Path::new(&install),
        GameMode::default(),
        rules,
        &image,
        None,
        "VERSION_FROZEN_THRONE",
    )
    .unwrap();
    assert_eq!(recipe.edition(), 1);
    assert_eq!(recipe.image(), image.as_slice());
    assert_eq!(sha256(recipe.image()), recipe.image_sha256());
    let selected_common = recipe
        .base_resolver()
        .unwrap()
        .resolve("Scripts\\common.j")
        .unwrap();
    assert_eq!(selected_common.bytes, common);
    assert_eq!(sha256(&selected_common.bytes), recipe.common_sha256());
    let info = map_info(2, 0);
    assert_eq!(info.loading_id, Some(2));
    assert_eq!(info.flags, 0);
    let reader = recipe.open(&info).unwrap();
    assert_eq!(reader.selection.prefix, "Melee_V1");
    assert!(reader.resolver.paths().count() > 0);
    let resolved = reader.resolver.resolve("Scripts\\common.j").unwrap();
    assert!(!resolved.bytes.is_empty());
}

fn install_file(install: &Path, name: &str) -> Option<std::path::PathBuf> {
    std::fs::read_dir(install).ok()?.find_map(|entry| {
        let path = entry.ok()?.path();
        path.file_name()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            .then_some(path)
    })
}

fn opened_archive(
    path: &Path,
    table: &std::sync::Arc<pcx_mpq::huffman::HuffmanTable>,
) -> pcx_mpq::MpqArchive {
    let mut archive = pcx_mpq::MpqArchive::open(path).unwrap();
    archive.set_codec_table(std::sync::Arc::clone(table));
    archive
}

#[test]
#[ignore = "requires PCX_CAMPAIGN_INSTALL, PCX_CAMPAIGN_IMAGE and PCX_CAMPAIGN_COMMON"]
fn local_original_recipe_selects_the_custom_v0_namespace_and_base_fallback() {
    let install = std::env::var("PCX_CAMPAIGN_INSTALL").expect("explicit original install");
    let install = Path::new(&install);
    let image_path = std::env::var("PCX_CAMPAIGN_IMAGE").expect("explicit original executable");
    let common_path = std::env::var("PCX_CAMPAIGN_COMMON").expect("explicit original common.j");
    let image = std::fs::read(&image_path).unwrap();
    let common = std::fs::read(&common_path).unwrap();
    let rules = ImageRules::read(
        &image,
        AssetIdentity {
            source: image_path.clone(),
            edition: "Legacy".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    let expected_rules = rules.clone();
    let table = std::sync::Arc::new(
        pcx_mpq::huffman::HuffmanTable::from_pe(&image, Path::new(&image_path)).unwrap(),
    );
    let recipe = CampaignRecipe::pin(
        install,
        GameMode::default(),
        rules,
        &image,
        Some(&common),
        "VERSION_REIGN_OF_CHAOS",
    )
    .unwrap();
    assert_eq!(recipe.edition(), 0);
    let reader = recipe.open(&map_info(1, 0)).unwrap();
    assert_eq!(reader.selection.prefix, "Custom_V0");

    let namespace_only = "Units\\NotUsed_UnitData.slk";
    for mount in &expected_rules.base {
        let Some(path) = install_file(install, &mount.archive) else {
            continue;
        };
        assert!(
            opened_archive(&path, &table)
                .find_block(namespace_only)
                .is_none(),
            "{} unexpectedly carries the base spelling of {namespace_only}",
            path.display()
        );
    }
    let prefixed = format!("Custom_V0\\{namespace_only}");
    let mut namespace_mounts = expected_rules.selected_mounts(0).unwrap().to_vec();
    namespace_mounts.sort_by_key(|mount| std::cmp::Reverse(mount.priority));
    let (namespace_mount, namespace_origin, namespace_bytes) = namespace_mounts
        .into_iter()
        .find_map(|mount| {
            let path = install_file(install, &mount.archive)?;
            let mut archive = opened_archive(&path, &table);
            archive.find_block(&prefixed)?;
            let bytes = archive.extract(&prefixed).unwrap();
            Some((mount, path, bytes))
        })
        .expect("selected namespace archive carries the namespace-only member");
    let origin = namespace_origin.to_str().unwrap();
    let version = format!(
        "campaign image sha256:{} common sha256:{} edition:0",
        digest_hex(&recipe.image_sha256()),
        digest_hex(&recipe.common_sha256())
    );
    let resolved = reader.resolver.resolve(namespace_only).unwrap();
    assert_eq!(resolved.bytes, namespace_bytes);
    assert_eq!(sha256(&resolved.bytes), sha256(&namespace_bytes));
    assert_eq!(resolved.provenance.locator, format!("{origin}::{prefixed}"));
    assert_eq!(
        resolved.provenance.archive_priority,
        Some(namespace_mount.priority)
    );
    assert_eq!(resolved.provenance.source.origin, origin);
    assert_eq!(
        resolved.provenance.source.version.as_deref(),
        Some(version.as_str())
    );
    assert_eq!(
        resolved.provenance.source.id,
        format!(
            "campaign archive {origin} priority {} namespace Custom_V0 {version}",
            namespace_mount.priority
        )
    );
    assert_eq!(
        resolved.provenance.byte_length,
        namespace_bytes.len() as u64
    );

    let base_only = "Scripts\\common.j";
    for mount in expected_rules.selected_mounts(0).unwrap() {
        let Some(path) = install_file(install, &mount.archive) else {
            continue;
        };
        assert!(
            opened_archive(&path, &table)
                .find_block(&format!("Custom_V0\\{base_only}"))
                .is_none(),
            "{} unexpectedly carries a Custom_V0 spelling of {base_only}",
            path.display()
        );
    }
    let mut base_mounts = expected_rules.base.clone();
    base_mounts.sort_by_key(|mount| std::cmp::Reverse(mount.priority));
    let (base_mount, base_origin, base_bytes) = base_mounts
        .into_iter()
        .find_map(|mount| {
            let path = install_file(install, &mount.archive)?;
            let mut archive = opened_archive(&path, &table);
            archive.find_block(base_only)?;
            let bytes = archive.extract(base_only).unwrap();
            Some((mount, path, bytes))
        })
        .expect("a base archive carries the base-only member");
    let origin = base_origin.to_str().unwrap();
    let resolved = reader.resolver.resolve(base_only).unwrap();
    assert_eq!(resolved.bytes, base_bytes);
    assert_eq!(sha256(&resolved.bytes), sha256(&base_bytes));
    assert_eq!(
        resolved.provenance.locator,
        format!("{origin}::{base_only}")
    );
    assert_eq!(
        resolved.provenance.archive_priority,
        Some(base_mount.priority)
    );
    assert_eq!(resolved.provenance.source.origin, origin);
    assert_eq!(
        resolved.provenance.source.id,
        format!(
            "campaign archive {origin} priority {} base {version}",
            base_mount.priority
        )
    );
    assert_eq!(resolved.provenance.byte_length, base_bytes.len() as u64);
}

#[test]
#[ignore = "requires PCX_CAMPAIGN_INSTALL, PCX_CAMPAIGN_IMAGE and PCX_CAMPAIGN_COMMON"]
fn local_original_recipe_refuses_a_mismatched_common_blob() {
    let install = std::env::var("PCX_CAMPAIGN_INSTALL").expect("explicit original install");
    let image_path = std::env::var("PCX_CAMPAIGN_IMAGE").expect("explicit original executable");
    let common_path = std::env::var("PCX_CAMPAIGN_COMMON").expect("explicit original common.j");
    let image = std::fs::read(&image_path).unwrap();
    let mut common = std::fs::read(&common_path).unwrap();
    common.push(b'\n');
    let rules = ImageRules::read(
        &image,
        AssetIdentity {
            source: image_path,
            edition: "Legacy".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    let error = CampaignRecipe::pin(
        Path::new(&install),
        GameMode::default(),
        rules,
        &image,
        Some(&common),
        "VERSION_FROZEN_THRONE",
    )
    .unwrap_err();
    assert!(error.to_string().contains("common.j"), "{error}");
}

#[cfg(unix)]
#[test]
#[ignore = "requires PCX_CAMPAIGN_INSTALL, PCX_CAMPAIGN_IMAGE and PCX_CAMPAIGN_COMMON"]
fn local_original_recipe_retains_its_opened_archive_generation() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let install = std::env::var("PCX_CAMPAIGN_INSTALL").expect("explicit original install");
    let image_path = std::env::var("PCX_CAMPAIGN_IMAGE").expect("explicit original executable");
    let common_path = std::env::var("PCX_CAMPAIGN_COMMON").expect("explicit original common.j");
    let image = std::fs::read(&image_path).unwrap();
    let common = std::fs::read(&common_path).unwrap();
    let rules = ImageRules::read(
        &image,
        AssetIdentity {
            source: image_path,
            edition: "Legacy".into(),
            version: "1.29.2.9231".into(),
        },
    )
    .unwrap();
    let dir = std::env::temp_dir().join(format!(
        "pcx-campaign-generation-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(&install).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mpq"))
        {
            std::os::unix::fs::symlink(&path, dir.join(entry.file_name())).unwrap();
        }
    }
    let replacement_rules = rules.clone();
    let recipe = CampaignRecipe::pin(
        &dir,
        GameMode::default(),
        rules,
        &image,
        Some(&common),
        "VERSION_FROZEN_THRONE",
    )
    .unwrap();
    let base = recipe.base_resolver().unwrap();
    let (map_member, map_bytes, info, map_origin) = base
        .paths()
        .filter(|asset| asset.as_str().to_ascii_lowercase().ends_with(".w3m"))
        .find_map(|asset| {
            let member = asset.as_str();
            let resolved = base.resolve(member).ok()?;
            let info = pcx_map::Map::parse(&resolved.bytes).ok()?.info().ok()?;
            Some((
                member.to_string(),
                resolved.bytes,
                info,
                resolved.provenance.source.origin,
            ))
        })
        .expect("a base archive carries a campaign map");
    let before = base.resolve("Scripts\\common.j").unwrap().bytes;
    let replaced = dir.join(Path::new(&map_origin).file_name().unwrap());
    std::fs::remove_file(&replaced).unwrap();
    std::fs::write(&replaced, b"not an mpq").unwrap();
    let reader = recipe.open(&info).unwrap();
    assert_eq!(
        reader.resolver.resolve(&map_member).unwrap().bytes,
        map_bytes
    );
    let after = reader.resolver.resolve("Scripts\\common.j").unwrap().bytes;
    assert_eq!(before, after);
    assert!(
        CampaignRecipe::pin(
            &dir,
            GameMode::default(),
            replacement_rules,
            &image,
            Some(&common),
            "VERSION_FROZEN_THRONE",
        )
        .is_err()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
