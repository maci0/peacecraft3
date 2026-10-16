use super::*;

const CODE_RAW: usize = 0x200;
const CODE_LEN: usize = 0x1000;
const DATA_RAW: usize = 0x1200;
const DATA_LEN: usize = 0x400;
const DATA_RVA: u32 = 0x3000;

struct ScoreSpec {
    sentinel: i32,
    wildcard: i32,
    adapter_penalty: u32,
    kind_penalty: u32,
    flag_penalty: u32,
    shift: u8,
    mask: u8,
    quant_max: u32,
    depth_kind: i32,
    depth_penalty: u32,
    refresh_penalty: u32,
}

struct Spec {
    base: u32,
    depth_a: i32,
    depth_b: i32,
    min_width: i32,
    flag: u8,
    kind: i32,
    center: i32,
    score: ScoreSpec,
    limit: u8,
    with_depth: &'static str,
    size_only: &'static str,
}

struct Fixture {
    bytes: Vec<u8>,
    origin: AssetIdentity,
    budget: DiscoveryBudget,
}

fn score_spec() -> ScoreSpec {
    ScoreSpec {
        sentinel: 70,
        wildcard: 90,
        adapter_penalty: 500,
        kind_penalty: 400,
        flag_penalty: 200,
        shift: 4,
        mask: 15,
        quant_max: 1000,
        depth_kind: 3,
        depth_penalty: 8,
        refresh_penalty: 4,
    }
}

fn spec(base: u32) -> Spec {
    Spec {
        base,
        depth_a: 24,
        depth_b: 15,
        min_width: 200,
        flag: 0,
        kind: 3,
        center: 60,
        score: score_spec(),
        limit: 32,
        with_depth: "%d/%d/%d",
        size_only: "%d/%d",
    }
}

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn rel(at: usize, land: usize, buf: &mut [u8]) {
    buf[at] = u8::try_from(land - (at + 1)).unwrap();
}

fn mark(buf: &mut Vec<u8>, opcode: u8) -> usize {
    buf.push(opcode);
    buf.push(0);
    buf.len() - 1
}

fn cmp_reg(buf: &mut Vec<u8>, short: u8, long: u8, value: i32) {
    if (1..128).contains(&value) {
        buf.extend_from_slice(&[0x83, short, value as u8]);
    } else {
        buf.extend_from_slice(&[0x81, long]);
        buf.extend_from_slice(&value.to_le_bytes());
    }
}

fn sub_eax(buf: &mut Vec<u8>, value: i32) {
    if (1..128).contains(&value) {
        buf.extend_from_slice(&[0x83, 0xe8, value as u8]);
    } else {
        buf.push(0x2d);
        buf.extend_from_slice(&value.to_le_bytes());
    }
}

fn filter_bytes(spec: &Spec) -> Vec<u8> {
    let mut buf = vec![0x8b, 0x4d, 0xc8];
    cmp_reg(&mut buf, 0xf9, 0xf9, spec.depth_a);
    let je = mark(&mut buf, 0x74);
    cmp_reg(&mut buf, 0xf9, 0xf9, spec.depth_b);
    let jne = mark(&mut buf, 0x75);
    let admit = buf.len();
    rel(je, admit, &mut buf);
    buf.extend_from_slice(&[0x8b, 0x55, 0xcc]);
    cmp_reg(&mut buf, 0xfa, 0xfa, spec.min_width);
    let jb = mark(&mut buf, 0x72);
    let reject = buf.len() + 16;
    rel(jne, reject, &mut buf);
    rel(jb, reject, &mut buf);
    buf.extend_from_slice(&[0xc6, 0x85, 0xb0, 0xfd, 0xff, 0xff, spec.flag]);
    buf.extend_from_slice(&[0xc7, 0x85, 0xa4, 0xfd, 0xff, 0xff]);
    buf.extend_from_slice(&spec.kind.to_le_bytes());
    buf
}

fn refresh_bytes(center: i32) -> Vec<u8> {
    let mut buf = vec![0x8b, 0x43, 0x20];
    sub_eax(&mut buf, center);
    buf.extend_from_slice(&[0x99, 0x8b, 0xc8, 0x8b, 0x46, 0x20, 0x33, 0xca]);
    sub_eax(&mut buf, center);
    buf.extend_from_slice(&[
        0x2b, 0xca, 0x99, 0x33, 0xc2, 0x2b, 0xc2, 0x3b, 0xc1, 0x7d, 0,
    ]);
    buf
}

fn add_esi(buf: &mut Vec<u8>, value: u32) {
    if value < 128 {
        buf.extend_from_slice(&[0x83, 0xc6, value as u8]);
    } else {
        buf.extend_from_slice(&[0x81, 0xc6]);
        buf.extend_from_slice(&value.to_le_bytes());
    }
}

fn score_bytes(spec: &ScoreSpec) -> Vec<u8> {
    let mut buf = vec![0x3d];
    buf.extend_from_slice(&spec.sentinel.to_le_bytes());
    let skip_call = mark(&mut buf, 0x75);
    buf.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
    rel(skip_call, buf.len(), &mut buf);
    buf.extend_from_slice(&[0x8b, 0x0f, 0x81, 0xf9]);
    buf.extend_from_slice(&spec.wildcard.to_le_bytes());
    let skip_ecx = mark(&mut buf, 0x74);
    buf.push(0x3d);
    buf.extend_from_slice(&spec.wildcard.to_le_bytes());
    let skip_eax = mark(&mut buf, 0x74);
    buf.extend_from_slice(&[0x3b, 0xc8, 0xba]);
    buf.extend_from_slice(&spec.adapter_penalty.to_le_bytes());
    buf.extend_from_slice(&[0x0f, 0x45, 0xf2]);
    rel(skip_ecx, buf.len(), &mut buf);
    rel(skip_eax, buf.len(), &mut buf);
    buf.extend_from_slice(&[0x8b, 0x4f, 0x18, 0x3b, 0x4d, 0xf0]);
    let skip_kind = mark(&mut buf, 0x74);
    buf.extend_from_slice(&[0x81, 0xc6]);
    buf.extend_from_slice(&spec.kind_penalty.to_le_bytes());
    rel(skip_kind, buf.len(), &mut buf);
    buf.extend_from_slice(&[0x8a, 0x47, 0x24, 0x3a, 0x45, 0xfc]);
    let skip_flag = mark(&mut buf, 0x74);
    buf.extend_from_slice(&[0x81, 0xc6]);
    buf.extend_from_slice(&spec.flag_penalty.to_le_bytes());
    rel(skip_flag, buf.len(), &mut buf);
    buf.extend_from_slice(&[
        0x8b, 0x47, 0x0c, 0x2b, 0x45, 0xe4, 0x99, 0x53, 0x8b, 0xd8, 0x8b, 0x47, 0x08, 0x2b, 0x45,
        0xe0, 0x33, 0xda, 0x2b, 0xda, 0x99, 0x33, 0xc2, 0x2b, 0xc2, 0x03, 0xd8, 0x85, 0xdb,
    ]);
    let skip_quant = mark(&mut buf, 0x7e);
    buf.extend_from_slice(&[0x8b, 0xc3, 0x99, 0x83, 0xe2, spec.mask, 0x03, 0xc2, 0xba]);
    buf.extend_from_slice(&spec.quant_max.to_le_bytes());
    buf.extend_from_slice(&[
        0xc1, 0xf8, spec.shift, 0x40, 0x3b, 0xc2, 0x0f, 0x4f, 0xc2, 0xc1, 0xe0, spec.shift, 0x03,
        0xf0,
    ]);
    rel(skip_quant, buf.len(), &mut buf);
    buf.push(0x5b);
    if (0..128).contains(&spec.depth_kind) {
        buf.extend_from_slice(&[0x83, 0xe9, spec.depth_kind as u8]);
    } else {
        buf.extend_from_slice(&[0x81, 0xe9]);
        buf.extend_from_slice(&spec.depth_kind.to_le_bytes());
    }
    let skip_depth = mark(&mut buf, 0x75);
    buf.extend_from_slice(&[0x8b, 0x47, 0x1c, 0x3b, 0x45, 0xf4]);
    let skip_add = mark(&mut buf, 0x74);
    add_esi(&mut buf, spec.depth_penalty);
    rel(skip_add, buf.len(), &mut buf);
    buf.extend_from_slice(&[0x8b, 0x4f, 0x20, 0x85, 0xc9]);
    let skip_candidate = mark(&mut buf, 0x74);
    buf.extend_from_slice(&[0x8b, 0x45, 0xf8, 0x85, 0xc0]);
    let skip_query = mark(&mut buf, 0x74);
    buf.extend_from_slice(&[0x3b, 0xc8]);
    if spec.refresh_penalty < 128 {
        buf.extend_from_slice(&[0x8d, 0x46, spec.refresh_penalty as u8]);
    } else {
        buf.extend_from_slice(&[0x8d, 0x86]);
        buf.extend_from_slice(&(spec.refresh_penalty as i32).to_le_bytes());
    }
    let unequal = mark(&mut buf, 0x75);
    let restore = buf.len();
    buf.extend_from_slice(&[0x8b, 0xc6]);
    rel(skip_depth, restore, &mut buf);
    rel(skip_candidate, restore, &mut buf);
    rel(skip_query, restore, &mut buf);
    rel(unequal, buf.len(), &mut buf);
    buf
}

fn label_bytes(with_depth: u32, size_only: u32, limit: u8) -> Vec<u8> {
    let mut buf = vec![0x8b, 0x48, 0x1c, 0x85, 0xc9];
    let short_path = mark(&mut buf, 0x74);
    buf.extend_from_slice(&[
        0x51, 0xff, 0x70, 0x0c, 0xff, 0x70, 0x08, 0x8d, 0x45, 0xbc, 0x68,
    ]);
    buf.extend_from_slice(&with_depth.to_le_bytes());
    buf.extend_from_slice(&[0x6a, limit, 0x50, 0xe8, 0, 0, 0, 0, 0x83, 0xc4, 0x18]);
    let skip_short = mark(&mut buf, 0xeb);
    rel(short_path, buf.len(), &mut buf);
    buf.extend_from_slice(&[0xff, 0x70, 0x0c, 0xff, 0x70, 0x08, 0x8d, 0x45, 0xbc, 0x68]);
    buf.extend_from_slice(&size_only.to_le_bytes());
    buf.extend_from_slice(&[0x6a, limit, 0x50, 0xe8, 0, 0, 0, 0, 0x83, 0xc4, 0x14]);
    rel(skip_short, buf.len(), &mut buf);
    buf
}

fn write_text(bytes: &mut [u8], at: usize, text: &str) {
    bytes[at..at + text.len()].copy_from_slice(text.as_bytes());
    bytes[at + text.len()] = 0;
}

fn fixture(spec: &Spec) -> Fixture {
    let mut bytes = vec![0xcc; DATA_RAW + DATA_LEN];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [(0x84, 0x14cu16), (0x86, 2), (0x94, 0x60), (0x98, 0x10b)] {
        bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, spec.base),
        (0xd0, 0x5000),
        (0x100, CODE_LEN as u32),
        (0x104, 0x1000),
        (0x108, CODE_LEN as u32),
        (0x10c, CODE_RAW as u32),
        (0x11c, 0x6000_0020),
        (0x128, DATA_LEN as u32),
        (0x12c, DATA_RVA),
        (0x130, DATA_LEN as u32),
        (0x134, DATA_RAW as u32),
        (0x144, 0x4000_0040),
    ] {
        put(&mut bytes, at, value);
    }
    write_text(&mut bytes, DATA_RAW, spec.with_depth);
    write_text(&mut bytes, DATA_RAW + 32, spec.size_only);
    let sites = [
        (0x20, filter_bytes(spec)),
        (0x120, refresh_bytes(spec.center)),
        (0x220, score_bytes(&spec.score)),
        (
            0x480,
            label_bytes(spec.base + DATA_RVA, spec.base + DATA_RVA + 32, spec.limit),
        ),
    ];
    for (at, site) in sites {
        bytes[CODE_RAW + at..CODE_RAW + at + site.len()].copy_from_slice(&site);
    }
    Fixture {
        bytes,
        origin: AssetIdentity {
            edition: "selected skin V1".into(),
            version: "pattern-qualified PE32; release identity not inferred".into(),
            source: "synthetic:display-modes".into(),
        },
        budget: DiscoveryBudget {
            image: RuleInputBudget {
                image_bytes: 0x4000,
                profile_bytes: 1024,
                sections: 2,
            },
            label_bytes: 64,
            candidate_limit: 8,
            work_bytes: 0x10_000,
        },
    }
}

fn policy(fixture: &Fixture) -> Policy {
    Policy::discover(&fixture.bytes, fixture.origin.clone(), fixture.budget).unwrap()
}

fn problem(fixture: &Fixture) -> Problem {
    Policy::discover(&fixture.bytes, fixture.origin.clone(), fixture.budget)
        .err()
        .unwrap()
        .problem
}

fn host(
    width: u32,
    height: u32,
    depth: Option<u32>,
    refresh: Option<u32>,
    adapter: u32,
    mode_index: u32,
) -> HostRecord {
    HostRecord {
        width,
        height,
        depth,
        refresh_millihertz: refresh,
        adapter,
        mode_index,
    }
}

fn query(policy: &Policy, width: i32, height: i32, refresh: i32, adapter: u32) -> Query {
    Query {
        width,
        height,
        depth: 0,
        refresh,
        adapter,
        kind: policy.kind(),
        flag: policy.flag(),
    }
}

#[test]
fn discovers_operands_labels_and_identity_from_the_image() {
    let fixture = fixture(&spec(0x400000));
    let policy = policy(&fixture);
    assert_eq!(policy.origin().source, fixture.origin.source);
    assert_eq!(policy.origin().edition, fixture.origin.edition);
    assert_eq!(policy.kind(), 3);
    assert_eq!(policy.flag(), 0);
    assert_eq!(policy.sentinel(), 70);
    assert_eq!(policy.wildcard(), 90);
    let rows = policy
        .rows(&[host(800, 600, Some(24), Some(60_000), 2, 7)], Some(0))
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "800/600/24");
    assert_eq!(rows[0].mode_index, 7);
    assert_eq!(rows[0].kind, 3);
    assert_eq!(rows[0].flag, 0);
}

#[test]
fn relocated_bases_and_changed_immediates_change_admission() {
    for base in [0x400000, 0x500000] {
        let mut wide = spec(base);
        wide.min_width = 400;
        wide.center = 200;
        wide.score.depth_penalty = 180;
        wide.score.refresh_penalty = 240;
        let policy = policy(&fixture(&wide));
        assert!(
            policy
                .rows(&[host(240, 180, Some(24), None, 2, 0)], Some(0))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            policy
                .rows(&[host(400, 300, Some(15), None, 2, 1)], Some(0))
                .unwrap()[0]
                .label,
            "400/300/15"
        );
    }
    let mut narrow = spec(0x400000);
    narrow.min_width = 40;
    let policy = policy(&fixture(&narrow));
    assert_eq!(
        policy
            .rows(&[host(240, 180, Some(24), None, 2, 0)], Some(0))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn equivalent_label_sites_share_a_profile_and_conflicting_limits_refuse() {
    let spec = spec(0x400000);
    let mut image = fixture(&spec);
    let labels = label_bytes(spec.base + DATA_RVA, spec.base + DATA_RVA + 32, spec.limit);
    let at = CODE_RAW + 0x580;
    image.bytes[at..at + labels.len()].copy_from_slice(&labels);
    let rows = policy(&image)
        .rows(&[host(800, 600, Some(24), None, 2, 0)], None)
        .unwrap();
    assert_eq!(rows[0].label, "800/600/24");

    let conflicting = label_bytes(
        spec.base + DATA_RVA,
        spec.base + DATA_RVA + 32,
        spec.limit - 1,
    );
    image.bytes[at..at + conflicting.len()].copy_from_slice(&conflicting);
    assert!(matches!(problem(&image), Problem::Ambiguous));
}

#[test]
fn duplicate_filter_is_ambiguous_and_broken_profiles_are_absent() {
    let mut duplicated = fixture(&spec(0x400000));
    let second = filter_bytes(&spec(0x400000));
    duplicated.bytes[CODE_RAW + 0xa0..CODE_RAW + 0xa0 + second.len()].copy_from_slice(&second);
    assert!(matches!(problem(&duplicated), Problem::Ambiguous));

    let mut missing = fixture(&spec(0x400000));
    missing.bytes[CODE_RAW + 0x20] = 0x90;
    assert!(matches!(problem(&missing), Problem::None));

    let mut truncated = fixture(&spec(0x400000));
    truncated.bytes[CODE_RAW + 0x250..CODE_RAW + 0x280].fill(0xcc);
    assert!(matches!(problem(&truncated), Problem::None));

    let mut extra = spec(0x400000);
    extra.with_depth = "%d/%d/%q";
    assert!(matches!(problem(&fixture(&extra)), Problem::None));

    let mut bare = fixture(&spec(0x400000));
    let end = DATA_RAW + DATA_LEN - 4;
    bare.bytes[end..].fill(b'A');
    let broken = label_bytes(
        0x400000 + DATA_RVA + (end - DATA_RAW) as u32,
        0x400000 + DATA_RVA + 32,
        32,
    );
    bare.bytes[CODE_RAW + 0x480..CODE_RAW + 0x480 + broken.len()].copy_from_slice(&broken);
    assert!(matches!(problem(&bare), Problem::None));
}

#[test]
fn budgets_reject_extra_work_and_extra_candidates() {
    let mut tight = fixture(&spec(0x400000));
    tight.budget.work_bytes = CODE_LEN;
    assert!(matches!(problem(&tight), Problem::Budget));
    let mut few = fixture(&spec(0x400000));
    few.budget.candidate_limit = 3;
    assert!(matches!(problem(&few), Problem::Budget));
    let mut blank = fixture(&spec(0x400000));
    blank.origin.edition.clear();
    assert!(matches!(problem(&blank), Problem::Identity));
}

#[test]
fn empty_and_filtered_input_adds_no_fallback_row() {
    let policy = policy(&fixture(&spec(0x400000)));
    assert!(policy.rows(&[], Some(0)).unwrap().is_empty());
    assert!(
        policy
            .select(&[], query(&policy, 800, 600, 0, 2))
            .unwrap()
            .is_none()
    );
    let rows = policy
        .rows(
            &[
                host(100, 80, Some(24), Some(60_000), 2, 0),
                host(800, 600, Some(8), Some(60_000), 2, 1),
                host(800, 600, Some(0), Some(60_000), 2, 2),
            ],
            Some(0),
        )
        .unwrap();
    assert!(rows.is_empty());
    assert!(
        policy
            .rows(&[host(800, 600, Some(24), None, 2, 0)], Some(1))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn grouping_keeps_first_seen_order_closer_refresh_and_duplicate_labels() {
    let policy = policy(&fixture(&spec(0x400000)));
    let rows = policy
        .rows(
            &[
                host(800, 600, Some(24), Some(90_000), 2, 1),
                host(800, 600, Some(24), Some(50_000), 2, 2),
                host(640, 480, Some(24), Some(70_000), 2, 3),
                host(640, 480, Some(24), Some(50_000), 2, 4),
                host(800, 600, Some(24), Some(90_000), 9, 5),
                host(720, 400, None, Some(50_000), 2, 6),
                host(720, 400, None, Some(60_500), 2, 7),
            ],
            Some(0),
        )
        .unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        ["800/600/24", "640/480/24", "800/600/24", "720/400"]
    );
    assert_eq!(rows[0].refresh_millihertz, Some(50_000));
    assert_eq!(rows[0].mode_index, 2);
    assert_eq!(rows[1].refresh_millihertz, Some(70_000));
    assert_eq!(rows[1].mode_index, 3);
    assert_eq!(rows[2].adapter, 9);
    assert_eq!(rows[3].refresh_millihertz, Some(60_500));
    assert_eq!(rows[3].mode_index, 7);
    assert!(rows[3].depth.is_none());
}

#[test]
fn selection_prefers_the_closer_size_keeps_ties_and_scores_refresh_without_rounding() {
    let policy = policy(&fixture(&spec(0x400000)));
    let sized = policy
        .rows(
            &[
                host(900, 700, Some(24), Some(60_000), 2, 0),
                host(800, 600, Some(24), Some(60_000), 2, 1),
            ],
            Some(0),
        )
        .unwrap();
    assert_eq!(
        policy
            .select(&sized, query(&policy, 800, 600, 0, 2))
            .unwrap(),
        Some(1)
    );

    let tied = vec![
        Row {
            label: "a".into(),
            width: 101,
            height: 100,
            depth: Some(24),
            refresh_millihertz: Some(60_000),
            adapter: 2,
            mode_index: 0,
            kind: policy.kind(),
            flag: 0,
        },
        Row {
            label: "b".into(),
            width: 102,
            height: 100,
            depth: Some(24),
            refresh_millihertz: Some(60_000),
            adapter: 2,
            mode_index: 1,
            kind: policy.kind(),
            flag: 0,
        },
    ];
    assert_eq!(
        policy
            .select(&tied, query(&policy, 100, 100, 0, 2))
            .unwrap(),
        Some(0)
    );

    let refreshed = policy
        .rows(
            &[
                host(800, 600, Some(24), Some(59_000), 2, 0),
                host(800, 600, Some(15), Some(60_500), 2, 1),
                host(800, 600, Some(24), Some(60_000), 9, 2),
            ],
            Some(0),
        )
        .unwrap();
    assert_eq!(
        policy
            .select(&refreshed, query(&policy, 800, 600, 0, 2))
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        policy
            .select(&refreshed, query(&policy, 800, 600, 60, 2))
            .unwrap(),
        Some(1)
    );

    let mut ask = query(&policy, 800, 600, 0, policy.sentinel() as u32);
    ask.adapter = u32::try_from(policy.sentinel()).unwrap();
    assert!(matches!(
        policy.select(&sized, ask).err().unwrap().problem,
        Problem::Operand
    ));
}

#[test]
fn adapter_mismatch_outranks_a_small_size_delta_and_overflow_is_refused() {
    let normal_policy = policy(&fixture(&spec(0x400000)));
    let rows = vec![
        Row {
            label: "mismatch".into(),
            width: 800,
            height: 600,
            depth: Some(24),
            refresh_millihertz: None,
            adapter: 9,
            mode_index: 0,
            kind: normal_policy.kind(),
            flag: 0,
        },
        Row {
            label: "near".into(),
            width: 801,
            height: 600,
            depth: Some(24),
            refresh_millihertz: None,
            adapter: 2,
            mode_index: 1,
            kind: normal_policy.kind(),
            flag: 0,
        },
    ];
    assert_eq!(
        normal_policy
            .select(&rows, query(&normal_policy, 800, 600, 0, 2))
            .unwrap(),
        Some(1)
    );

    let mut heavy = spec(0x400000);
    heavy.score.kind_penalty = u32::MAX;
    let policy = policy(&fixture(&heavy));
    let row = Row {
        label: "heavy".into(),
        width: 801,
        height: 600,
        depth: Some(24),
        refresh_millihertz: None,
        adapter: 2,
        mode_index: 0,
        kind: 9,
        flag: 0,
    };
    assert!(matches!(
        policy
            .select(&[row], query(&policy, 800, 600, 0, 2))
            .err()
            .unwrap()
            .problem,
        Problem::Overflow
    ));
}

#[test]
fn oversize_labels_fail_instead_of_being_truncated() {
    let mut spec = spec(0x400000);
    spec.limit = 4;
    spec.min_width = 80;
    let policy = policy(&fixture(&spec));
    assert!(matches!(
        policy
            .rows(&[host(80, 80, Some(24), None, 2, 0)], Some(0))
            .err()
            .unwrap()
            .problem,
        Problem::Label
    ));
}

#[test]
#[ignore = "requires PEACECRAFT3_OPTIONS_CLIENT with an explicitly selected original client"]
fn selected_original_mode_profile_is_admitted() {
    use std::io::Read;
    let path = std::path::PathBuf::from(
        std::env::var_os("PEACECRAFT3_OPTIONS_CLIENT").expect("explicit original client required"),
    );
    let mut file = std::fs::File::open(&path).unwrap();
    let max_bytes = 32 * 1024 * 1024;
    assert!(file.metadata().unwrap().len() <= max_bytes as u64);
    let mut image = Vec::new();
    file.by_ref()
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut image)
        .unwrap();
    assert!(image.len() <= max_bytes);
    let origin = AssetIdentity {
        edition: "explicit local client; adjacent archives unqualified".into(),
        version: "selected input; pattern qualification only".into(),
        source: path.display().to_string(),
    };
    let policy = Policy::discover(
        &image,
        origin.clone(),
        DiscoveryBudget {
            image: RuleInputBudget {
                image_bytes: max_bytes,
                profile_bytes: 128 * 1024,
                sections: 96,
            },
            label_bytes: 256,
            candidate_limit: 32,
            work_bytes: 64 * 1024 * 1024,
        },
    )
    .unwrap();
    assert_eq!(policy.origin(), &origin);
}
