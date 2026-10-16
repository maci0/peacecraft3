use super::*;

const CODE_RAW: usize = 0x200;
const CODE_LEN: usize = 0x1000;
const CODE_RVA: u32 = 0x1000;
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
        (0x104, CODE_RVA),
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
            monitor_ordinal: None,
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
            monitor_ordinal: None,
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
            monitor_ordinal: None,
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
            monitor_ordinal: None,
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
        monitor_ordinal: None,
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

const PLANTED_KIND: i32 = 11;

#[test]
fn popup_grouping_keeps_first_source_key_and_separate_monitors_and_flags() {
    let spec = spec(0x400000);
    let mut image = fixture(&spec);
    plant_popup(
        &mut image.bytes,
        spec.base,
        [(151, 97), (151, 97)],
        super::WS_POPUP,
        3,
        true,
        16,
    );
    write_pair(&mut image.bytes, DATA_RAW + 0x90, 151, 97);
    write_pair(&mut image.bytes, DATA_RAW + 0x98, 151, 97);
    let admitted = policy(&image);
    let monitors = [bound(4, 151, 97), bound(7, 151, 97)];
    let rows = admitted.popup_rows(&monitors, Some(0), 3).unwrap();
    assert_eq!(labels(&rows), ["151/97", "151/97"]);
    assert_eq!(rows[0].monitor_ordinal, Some(4));
    assert_eq!(rows[1].monitor_ordinal, Some(7));
    assert!(rows.iter().all(|row| row.mode_index == 0));

    let rows = admitted.popup_rows(&monitors, None, 3).unwrap();
    assert_eq!(labels(&rows), ["151/97", "151/97", "151/97", "151/97"]);
    assert_eq!(
        rows.iter().map(|row| row.flag).collect::<Vec<_>>(),
        [0, 1, 0, 1]
    );
}

#[test]
fn popup_profile_requires_zero_depth_and_refresh_stores_before_omitting_them() {
    for slot in [0xcc, 0xd0] {
        let spec = spec(0x400000);
        let mut image = fixture(&spec);
        plant_popup(
            &mut image.bytes,
            spec.base,
            [(151, 97), (400, 280)],
            super::WS_POPUP,
            3,
            true,
            16,
        );
        let store = image
            .bytes
            .windows(7)
            .position(|bytes| bytes == [0xc7, 0x45, slot, 0, 0, 0, 0])
            .unwrap();
        image.bytes[store + 3] = 1;
        assert!(!policy(&image).has_popup_profile());
    }
}

#[test]
fn popup_rows_emit_the_captured_full_monitor_bound() {
    let spec = spec(0x400000);
    let mut image = fixture(&spec);
    plant_popup(
        &mut image.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP,
        3,
        true,
        16,
    );
    let admitted = policy(&image);
    assert!(admitted.has_popup_profile());
    assert_eq!(admitted.popup_presentation_kind(), Some(PLANTED_KIND));
    assert!(admitted.rows(&[], Some(0)).unwrap().is_empty());
    let enumerated = admitted
        .rows(&[host(800, 600, Some(24), Some(60_000), 2, 3)], Some(0))
        .unwrap();
    assert_eq!(enumerated[0].label, "800/600/24");
    assert!(enumerated[0].monitor_ordinal.is_none());

    let rows = popup_at(&admitted, 210, 150, Some(0));
    assert_eq!(labels(&rows), ["151/97", "210/150"]);
    assert_eq!(rows[1].width, 210);
    assert_eq!(rows[1].height, 150);
    assert_eq!(rows[1].mode_index, 0);
    assert!(rows.iter().all(|row| row.label != "4/45"));
    assert!(rows.iter().all(|row| {
        row.flag == 0
            && row.kind == PLANTED_KIND
            && row.adapter == 4
            && row.monitor_ordinal == Some(0)
            && row.depth.is_none()
            && row.refresh_millihertz.is_none()
    }));

    let wide = popup_at(&admitted, 1366, 768, Some(0));
    assert_eq!(labels(&wide), ["151/97", "400/280", "1366/768"]);
    assert!(
        wide.iter()
            .any(|row| row.width == 1366 && row.height == 768)
    );
    assert!(
        wide.iter()
            .all(|row| (row.width, row.height) != (1024, 768) && row.label != "4/45")
    );

    assert_eq!(
        labels(&popup_at(&admitted, 210, 150, None)),
        ["151/97", "119/85", "203/145", "210/150"]
    );
    assert_eq!(
        labels(&popup_at(&admitted, 210, 150, Some(1))),
        ["119/85", "203/145"]
    );
    let square = popup_at(&admitted, 400, 400, None);
    assert_eq!(
        labels(&square),
        ["151/97", "400/280", "119/85", "203/145", "400/400"]
    );
    assert!(square.iter().all(|row| row.label != "333/222"));

    let monitors = admitted
        .popup_rows(&[bound(0, 210, 150), bound(1, 100, 80)], Some(0), 4)
        .unwrap();
    assert_eq!(labels(&monitors), ["151/97", "210/150", "100/80"]);
    assert_eq!(
        monitors
            .iter()
            .map(|row| row.monitor_ordinal)
            .collect::<Vec<_>>(),
        [Some(0), Some(0), Some(1)]
    );
    assert!(monitors.iter().all(|row| row.adapter == 4));
    assert!(
        monitors
            .iter()
            .any(|row| row.monitor_ordinal != Some(row.adapter))
    );
    let skipped = admitted
        .popup_rows(&[bound(9, 0, 150), bound(0, 210, 150)], Some(0), 4)
        .unwrap();
    assert_eq!(labels(&skipped), ["151/97", "210/150"]);
    assert!(skipped.iter().all(|row| row.monitor_ordinal == Some(0)));
    assert_eq!(labels(&popup_at(&admitted, 1, 1, Some(0))), ["1/1"]);
    assert!(matches!(
        admitted
            .popup_rows(&[bound(0, 210, 150)], Some(0), u32::MAX)
            .err()
            .unwrap()
            .problem,
        Problem::Overflow
    ));

    plant_popup(
        &mut image.bytes,
        spec.base,
        [(221, 155), (113, 75)],
        super::WS_POPUP,
        3,
        true,
        16,
    );
    let changed = popup_at(&policy(&image), 210, 150, Some(0));
    assert_eq!(labels(&changed), ["113/75", "210/150"]);
    assert_eq!(changed[0].mode_index, 1);
}

#[test]
fn equivalent_frame_and_capture_moves_admit_the_full_monitor_row() {
    for prologue in [[0x55, 0x89, 0xe5], [0x55, 0x8b, 0xec]] {
        for width_move in [[0x89, 0xc3], [0x8b, 0xd8]] {
            let spec = spec(0x400000);
            let mut image = fixture(&spec);
            plant_popup(
                &mut image.bytes,
                spec.base,
                [(151, 97), (400, 280)],
                super::WS_POPUP,
                3,
                true,
                16,
            );
            image.bytes[CODE_RAW + 0x600..CODE_RAW + 0x603].copy_from_slice(&prologue);
            image.bytes[CODE_RAW + 0x680..CODE_RAW + 0x683].copy_from_slice(&prologue);
            let capture = image
                .bytes
                .windows(8)
                .position(|bytes| bytes == [0x83, 0xc4, 0x08, 0x89, 0x55, 0x08, 0x89, 0xc3])
                .unwrap();
            image.bytes[capture + 6..capture + 8].copy_from_slice(&width_move);
            let admitted = policy(&image);
            assert!(admitted.has_popup_profile());
            assert_eq!(
                labels(&popup_at(&admitted, 210, 150, Some(0))),
                ["151/97", "210/150"]
            );
        }
    }
}

#[test]
fn a_missing_final_record_omits_that_family() {
    let spec = spec(0x400000);
    let mut image = fixture(&spec);
    plant_popup(
        &mut image.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP,
        3,
        false,
        16,
    );
    let admitted = policy(&image);
    assert!(admitted.has_popup_profile());
    assert_eq!(admitted.popup_presentation_kind(), Some(PLANTED_KIND));
    let rows = popup_at(&admitted, 210, 150, Some(0));
    assert_eq!(labels(&rows), ["151/97"]);
    assert!(rows.iter().all(|row| row.label != "210/150"));
}

#[test]
fn unqualified_popup_style_keeps_the_enumeration_path() {
    let spec = spec(0x400000);
    let mut framed = fixture(&spec);
    plant_popup(
        &mut framed.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP | 0x00c0_0000,
        3,
        true,
        16,
    );
    let framed_policy = policy(&framed);
    assert!(!framed_policy.has_popup_profile());
    assert!(framed_policy.popup_presentation_kind().is_none());
    let rows = framed_policy
        .rows(&[host(800, 600, Some(24), None, 2, 0)], Some(0))
        .unwrap();
    assert_eq!(rows[0].label, "800/600/24");
    assert!(rows[0].monitor_ordinal.is_none());
    assert!(popup_at(&framed_policy, 210, 150, Some(0)).is_empty());

    let mut same_selector = fixture(&spec);
    plant_popup(
        &mut same_selector.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP,
        2,
        true,
        16,
    );
    let refused = policy(&same_selector);
    assert!(!refused.has_popup_profile());
    assert_eq!(
        refused
            .rows(&[host(800, 600, Some(24), None, 2, 0)], Some(0))
            .unwrap()[0]
            .label,
        "800/600/24"
    );
}

#[test]
fn a_complete_popup_caller_rejects_a_malformed_table() {
    let spec = spec(0x400000);
    let mut negative = fixture(&spec);
    plant_popup(
        &mut negative.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP,
        3,
        true,
        16,
    );
    write_pair(&mut negative.bytes, DATA_RAW + 0x90, 119, -1);
    assert!(matches!(problem(&negative), Problem::Operand));

    let mut empty = fixture(&spec);
    plant_popup(
        &mut empty.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP,
        3,
        true,
        0,
    );
    empty.budget.image.profile_bytes = 15;
    assert!(matches!(problem(&empty), Problem::Operand));
}

#[test]
fn popup_discovery_charges_before_table_reads() {
    let spec = spec(0x400000);
    let mut limited = fixture(&spec);
    plant_popup(
        &mut limited.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP,
        3,
        true,
        16,
    );
    write_pair(&mut limited.bytes, DATA_RAW + 0x90, 119, 0);
    limited.budget.candidate_limit = 4;
    assert!(matches!(problem(&limited), Problem::Budget));

    let mut admitted = fixture(&spec);
    plant_popup(
        &mut admitted.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP,
        3,
        true,
        16,
    );
    admitted.budget.candidate_limit = 5;
    assert!(policy(&admitted).has_popup_profile());

    let mut profile_budget = fixture(&spec);
    plant_popup(
        &mut profile_budget.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP,
        3,
        true,
        16,
    );
    write_pair(&mut profile_budget.bytes, DATA_RAW + 0x90, -4, 85);
    profile_budget.budget.image.profile_bytes = 40;
    assert!(matches!(problem(&profile_budget), Problem::Budget));

    let mut work_budget = fixture(&spec);
    plant_popup(
        &mut work_budget.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP,
        3,
        true,
        16,
    );
    write_pair(&mut work_budget.bytes, DATA_RAW + 0x90, 119, 0);
    let label_bytes = spec.with_depth.len() + 1 + spec.size_only.len() + 1;
    let allowance = scanned_work_before_second_table(&work_budget.bytes, label_bytes);
    work_budget.budget.work_bytes = allowance;
    assert!(matches!(problem(&work_budget), Problem::Budget));
    work_budget.budget.work_bytes = allowance + 16;
    assert!(matches!(problem(&work_budget), Problem::Operand));

    let mut many_bounds = fixture(&spec);
    for off in [0x500usize, 0x560, 0x5c0] {
        write_bounds(&mut many_bounds.bytes, off, super::WS_POPUP, 3);
    }
    many_bounds.budget.candidate_limit = 6;
    assert!(matches!(problem(&many_bounds), Problem::Budget));

    let mut many_calls = fixture(&spec);
    write_bounds(&mut many_calls.bytes, 0x600, super::WS_POPUP, 3);
    write_pair(&mut many_calls.bytes, DATA_RAW + 0x90, 0, 1);
    plant_matching_calls(&mut many_calls.bytes, 0x700, 25, 8, false);
    many_calls.budget.candidate_limit = 8;
    assert!(matches!(problem(&many_calls), Problem::Budget));

    let mut many_groups = fixture(&spec);
    write_bounds(&mut many_groups.bytes, 0x600, super::WS_POPUP, 3);
    write_pair(&mut many_groups.bytes, DATA_RAW + 0x90, -1, 1);
    plant_matching_calls(&mut many_groups.bytes, 0x700, 9, 16, true);
    many_groups.budget.candidate_limit = 8;
    assert!(matches!(problem(&many_groups), Problem::Budget));
}

#[test]
fn popup_rows_cap_monitors_and_emitted_rows() {
    let spec = spec(0x400000);
    let mut image = fixture(&spec);
    plant_popup(
        &mut image.bytes,
        spec.base,
        [(151, 97), (400, 280)],
        super::WS_POPUP,
        3,
        true,
        16,
    );
    let admitted = policy(&image);
    let monitors: Vec<_> = (0..super::MAX_HOST_RECORDS + 1)
        .map(|ordinal| bound(u32::try_from(ordinal).unwrap(), 0, 0))
        .collect();
    assert!(matches!(
        admitted
            .popup_rows(&monitors, Some(0), 4)
            .err()
            .unwrap()
            .problem,
        Problem::Budget
    ));
    let aggregate: Vec<_> = (0..1400).map(|ordinal| bound(ordinal, 1366, 768)).collect();
    assert!(matches!(
        admitted
            .popup_rows(&aggregate, Some(0), 4)
            .err()
            .unwrap()
            .problem,
        Problem::Budget
    ));
}

#[test]
#[ignore = "requires PEACECRAFT3_OPTIONS_CLIENT with an explicitly selected original client"]
fn selected_original_popup_profile_uses_the_full_monitor_bound() {
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
        origin,
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
    assert!(policy.has_popup_profile());
    let kind = policy
        .popup_presentation_kind()
        .expect("flag-0 popup rows share one presentation kind");
    let rows = policy
        .popup_rows(&[bound(0, 1920, 1080), bound(1, 800, 600)], Some(0), 1)
        .unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|row| {
        row.flag == 0 && row.adapter == 1 && row.kind == kind && row.monitor_ordinal.is_some()
    }));
    let mut wide = 0usize;
    let mut narrow = 0usize;
    for row in &rows {
        match row.monitor_ordinal {
            Some(0) => {
                assert!(row.width <= 1920 && row.height <= 1080);
                wide += 1;
            }
            Some(1) => {
                assert!(row.width <= 800 && row.height <= 600);
                narrow += 1;
            }
            _ => panic!("popup row monitor ordinal"),
        }
    }
    assert!(wide > 0);
    assert!(narrow <= wide);
    assert!(
        rows.iter()
            .any(|row| row.monitor_ordinal == Some(0) && row.width == 1920 && row.height == 1080)
    );
    assert!(
        rows.iter()
            .any(|row| row.monitor_ordinal == Some(1) && row.width == 800 && row.height == 600)
    );
    assert!(policy.rows(&[], Some(0)).unwrap().is_empty());
}

fn scanned_work_before_second_table(bytes: &[u8], label_bytes: usize) -> usize {
    let code = &bytes[CODE_RAW..CODE_RAW + CODE_LEN];
    let prefix = [0x8b, 0x75, 0x08, 0x33, 0xc9, 0x8b, 0xc6, 0x33, 0xd2];
    let mut patterns = code
        .windows(prefix.len())
        .enumerate()
        .filter(|(_, window)| window.starts_with(&prefix));
    let (pattern, _) = patterns.next().expect("bounds cascade");
    assert!(patterns.next().is_none());
    let bounds_window = pattern - pattern.saturating_sub(40);
    let mut calls = Vec::new();
    let mut index = 0usize;
    while index + 5 <= code.len() {
        if code[index] == 0xe8
            && index >= 3
            && code[index - 3] == 0x56
            && code[index - 2] == 0x6a
            && code[index - 1] == 2
        {
            calls.push(index);
        }
        index += 1;
    }
    assert_eq!(calls.len(), 3);
    let entry_windows: usize = calls
        .iter()
        .map(|call| call - call.saturating_sub(0x800))
        .sum();
    CODE_LEN + label_bytes + bounds_window + CODE_LEN + entry_windows + 16
}

fn bound(ordinal: u32, width: u32, height: u32) -> MonitorBound {
    MonitorBound {
        ordinal,
        width,
        height,
    }
}

fn labels(rows: &[Row]) -> Vec<&str> {
    rows.iter().map(|row| row.label.as_str()).collect()
}

fn popup_at(policy: &Policy, width: u32, height: u32, flag: Option<u8>) -> Vec<Row> {
    policy
        .popup_rows(&[bound(0, width, height)], flag, 4)
        .unwrap()
}

fn push_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn push_kind(buf: &mut Vec<u8>, kind: i32, flag: u8) {
    buf.extend_from_slice(&[0xc7, 0x45, 0xc8]);
    buf.extend_from_slice(&kind.to_le_bytes());
    buf.extend_from_slice(&[0xc6, 0x45, 0xd4, flag]);
}

fn bounds_bytes(style2: u32, work_selector: u8) -> Vec<u8> {
    let mut buf = vec![0x55, 0x89, 0xe5, 0x90, 0x90];
    buf.extend_from_slice(&[0x8b, 0x75, 0x08, 0x33, 0xc9, 0x8b, 0xc6, 0x33, 0xd2]);
    buf.extend_from_slice(&[0x83, 0xe8, 0x01]);
    let je1 = mark(&mut buf, 0x74);
    buf.extend_from_slice(&[0x83, 0xe8, 0x01]);
    let je2 = mark(&mut buf, 0x74);
    buf.extend_from_slice(&[0x83, 0xe8, 0x01]);
    let jne = mark(&mut buf, 0x75);
    buf.push(0xba);
    push_u32(&mut buf, 0x06cf_0000);
    let jmp3 = mark(&mut buf, 0xeb);
    let style2_at = buf.len();
    rel(je2, style2_at, &mut buf);
    buf.push(0xba);
    push_u32(&mut buf, style2);
    let jmp2 = mark(&mut buf, 0xeb);
    let style1_at = buf.len();
    rel(je1, style1_at, &mut buf);
    buf.push(0xba);
    push_u32(&mut buf, 0x8608_0000);
    let join = buf.len();
    rel(jmp3, join, &mut buf);
    rel(jmp2, join, &mut buf);
    buf.push(0xb9);
    push_u32(&mut buf, super::WS_EX_APPWINDOW);
    let past = buf.len();
    rel(jne, past, &mut buf);
    buf.extend_from_slice(&[
        0x51,
        0x6a,
        0x00,
        0x52,
        0x8d,
        0x45,
        0xec,
        0x0f,
        0x57,
        0xc0,
        0x50,
        0x0f,
        0x11,
        0x45,
        0xec,
        0xff,
        0x15,
        0,
        0,
        0,
        0,
        0x83,
        0xfe,
        work_selector,
        0x8b,
        0x75,
        0x0c,
        0x0f,
        0x94,
        0xc0,
    ]);
    buf
}

fn write_bounds(bytes: &mut [u8], off: usize, style2: u32, work_selector: u8) {
    let bounds = bounds_bytes(style2, work_selector);
    assert!(off >= 0x4c0);
    assert!(off + bounds.len() <= CODE_LEN);
    bytes[CODE_RAW + off..CODE_RAW + off + bounds.len()].copy_from_slice(&bounds);
}

fn emit_call(buf: &mut Vec<u8>, slots: &mut Vec<(usize, usize)>, target_off: usize) {
    buf.push(0xe8);
    let at = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes());
    slots.push((at, target_off));
}

fn write_pair(bytes: &mut [u8], at: usize, width: i32, height: i32) {
    bytes[at..at + 4].copy_from_slice(&width.to_le_bytes());
    bytes[at + 4..at + 8].copy_from_slice(&height.to_le_bytes());
}

fn full_record_bytes(kind: i32, flag: u8) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&[0x8b, 0x45, 0x08, 0xb9]);
    push_u32(&mut buf, 0x0102_0304);
    buf.extend_from_slice(&[0x89, 0x75, 0xb0, 0xc7, 0x45, 0xb4]);
    push_u32(&mut buf, 0);
    buf.extend_from_slice(&[
        0x89, 0x5d, 0xb8, 0x89, 0x45, 0xbc, 0x89, 0x5d, 0xc0, 0x89, 0x45, 0xc4, 0xc7, 0x45, 0xcc,
    ]);
    push_u32(&mut buf, 0);
    buf.extend_from_slice(&[0xc7, 0x45, 0xd0]);
    push_u32(&mut buf, 0);
    push_kind(&mut buf, kind, flag);
    buf.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
    buf.extend_from_slice(&[
        0x0f, 0x10, 0x45, 0xb0, 0x46, 0x0f, 0x11, 0x00, 0x0f, 0x10, 0x45, 0xc0, 0x0f, 0x11, 0x40,
        0x10, 0xf3, 0x0f, 0x7e, 0x45, 0xd0, 0x66, 0x0f, 0xd6, 0x40, 0x20, 0x3b, 0x75, 0xf4,
    ]);
    buf
}

fn plant_popup(
    bytes: &mut [u8],
    base: u32,
    flag0: [(i32, i32); 2],
    style2: u32,
    work_selector: u8,
    with_full: bool,
    span: u32,
) {
    let bounds_off = 0x600usize;
    let caller_off = 0x680usize;
    write_bounds(bytes, bounds_off, style2, work_selector);
    let table0 = base + DATA_RVA + 0x80;
    let table1 = table0 + 16;
    let decoy = table1 + 16;
    let mut caller = Vec::new();
    let mut slots = Vec::new();
    caller.extend_from_slice(&[0x55, 0x89, 0xe5]);
    caller.push(0xbf);
    push_u32(&mut caller, decoy);
    caller.push(0x90);
    caller.extend_from_slice(&[0x56, 0x6a, 0x03]);
    emit_call(&mut caller, &mut slots, bounds_off);
    caller.resize(caller.len() + 16, 0x90);
    caller.extend_from_slice(&[0x56, 0x6a, 0x02]);
    emit_call(&mut caller, &mut slots, bounds_off);
    caller.extend_from_slice(&[0x83, 0xc4, 0x08, 0x89, 0x55, 0x08, 0x89, 0xc3]);
    for (start, flag) in [(table0, 0u8), (table1, 1)] {
        caller.push(0xbf);
        push_u32(&mut caller, start);
        caller.push(0x90);
        caller.extend_from_slice(&[0x56, 0x6a, 0x02]);
        emit_call(&mut caller, &mut slots, bounds_off);
        for slot in [0xcc, 0xd0] {
            caller.extend_from_slice(&[0xc7, 0x45, slot]);
            push_u32(&mut caller, 0);
        }
        push_kind(&mut caller, PLANTED_KIND, flag);
        caller.extend_from_slice(&[0x83, 0xc7, 0x08, 0x81, 0xff]);
        push_u32(&mut caller, start + span);
    }
    if with_full {
        caller.extend_from_slice(&full_record_bytes(PLANTED_KIND, 0));
    }
    assert!(caller_off + caller.len() <= CODE_LEN);
    bytes[CODE_RAW + caller_off..CODE_RAW + caller_off + caller.len()].copy_from_slice(&caller);
    for (at, target_off) in slots {
        let next = u32::try_from(caller_off + at + 4).unwrap();
        let rel = (CODE_RVA + u32::try_from(target_off).unwrap()).wrapping_sub(CODE_RVA + next);
        bytes[CODE_RAW + caller_off + at..CODE_RAW + caller_off + at + 4]
            .copy_from_slice(&rel.to_le_bytes());
    }
    write_pair(bytes, DATA_RAW + 0x80, flag0[0].0, flag0[0].1);
    write_pair(bytes, DATA_RAW + 0x88, flag0[1].0, flag0[1].1);
    write_pair(bytes, DATA_RAW + 0x90, 119, 85);
    write_pair(bytes, DATA_RAW + 0x98, 203, 145);
    write_pair(bytes, DATA_RAW + 0xA0, 333, 222);
}

fn plant_matching_calls(bytes: &mut [u8], at: usize, count: usize, stride: usize, prologue: bool) {
    let body = if prologue { 11 } else { 8 };
    assert!(stride >= body);
    assert!(at + count * stride <= CODE_LEN);
    for index in 0..count {
        let slot = at + index * stride;
        let cursor = if prologue {
            bytes[CODE_RAW + slot..CODE_RAW + slot + 3].copy_from_slice(&[0x55, 0x89, 0xe5]);
            slot + 3
        } else {
            slot
        };
        bytes[CODE_RAW + cursor..CODE_RAW + cursor + 3].copy_from_slice(&[0x56, 0x6a, 0x02]);
        let call = cursor + 3;
        bytes[CODE_RAW + call] = 0xe8;
        let next = u32::try_from(call + 5).unwrap();
        let rel = (CODE_RVA + 0x600).wrapping_sub(CODE_RVA + next);
        bytes[CODE_RAW + call + 1..CODE_RAW + call + 5].copy_from_slice(&rel.to_le_bytes());
    }
}
