use super::*;

const CODE_VA: u32 = 0x5dd000;
const CODE_SIZE: u32 = 0x5000;
const DATA_VA: u32 = 0xe69000;
const DATA_SIZE: u32 = 0x15000;
const RAW: usize = 0x400;
/// Constants and table words live from here; one 16-byte row per table copy.
const TABLES: u32 = DATA_VA + 0x100;
const CONSTANTS: u32 = DATA_VA + 0x200;

/// Operands of the synthetic image, in the shape the client stores them.
#[derive(Clone)]
struct Operands {
    distance: [f32; 6],
    angle: [f32; 6],
    rotation: [f32; 18],
    zoom_ease: f32,
    rotation_ease: f32,
    wheel_unit: u32,
    wheel_shift: u8,
    window: f32,
    slider_max: u8,
    divisor: f32,
    ceiling: f32,
    offset: f32,
    default: f32,
    options: [u8; 3],
}

impl Operands {
    fn original_like() -> Self {
        let mut rotation = [90.0; 18];
        rotation[6..12].fill(20.0);
        rotation[12..].fill(160.0);
        Self {
            distance: [1650.0, 1600.0, 1500.0, 1400.0, 1275.0, 1200.0],
            angle: [304.0, 311.0, 318.0, 325.0, 332.0, 335.0],
            rotation,
            zoom_ease: 1.0,
            rotation_ease: 0.5,
            wheel_unit: 120,
            wheel_shift: 6,
            window: 1.0,
            slider_max: 100,
            divisor: 100.0,
            ceiling: 1.0,
            offset: 0.5,
            default: 1.0,
            options: [0x2a, 0x28, 0x29],
        }
    }
}

fn put(image: &mut [u8], va: u32, bytes: &[u8]) {
    let at = if va >= DATA_VA {
        RAW + CODE_SIZE as usize + (va - DATA_VA) as usize
    } else {
        RAW + (va - CODE_VA) as usize
    };
    image[at..at + bytes.len()].copy_from_slice(bytes);
}

fn float_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A PE with the instruction forms the profile reads at the reviewed addresses.
fn image(o: &Operands) -> Vec<u8> {
    let mut image = vec![0; RAW + (CODE_SIZE + DATA_SIZE) as usize];
    image[..2].copy_from_slice(b"MZ");
    image[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    image[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    image[0x86..0x88].copy_from_slice(&2u16.to_le_bytes());
    image[0x94..0x96].copy_from_slice(&64u16.to_le_bytes());
    image[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    image[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
    image[0xd0..0xd4].copy_from_slice(&0xc00000u32.to_le_bytes());
    for (index, (va, size, raw, flags)) in [
        (CODE_VA, CODE_SIZE, RAW as u32, 0x6000_0020u32),
        (DATA_VA, DATA_SIZE, RAW as u32 + CODE_SIZE, 0x4000_0040),
    ]
    .into_iter()
    .enumerate()
    {
        let section = 0xd8 + index * 40;
        for (offset, value) in [
            (8, size),
            (12, va - 0x400000),
            (16, size),
            (20, raw),
            (36, flags),
        ] {
            image[section + offset..section + offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    // Zoom setter: table copies. Row r of the data holds four consecutive words.
    let copy =
        |image: &mut Vec<u8>, row: u32, load: u32, store: &[u8], store_va: u32, words: &[f32]| {
            let source = TABLES + row * 16;
            put(image, source, &float_bytes(words));
            put(
                image,
                load,
                &[&[0x0f, 0x28, 0x05][..], &source.to_le_bytes()].concat(),
            );
            put(image, store_va, store);
        };
    copy(
        &mut image,
        0,
        0x5de306,
        &[0x0f, 0x11, 0x45, 0xd8],
        0x5de30f,
        &o.distance[..4],
    );
    copy(
        &mut image,
        1,
        0x5de31c,
        &[0x0f, 0x11, 0x85, 0x78, 0xff, 0xff, 0xff],
        0x5de326,
        &o.rotation[..4],
    );
    copy(
        &mut image,
        2,
        0x5de337,
        &[0x0f, 0x11, 0x45, 0x88],
        0x5de341,
        &o.rotation[4..8],
    );
    copy(
        &mut image,
        3,
        0x5de34e,
        &[0x0f, 0x11, 0x45, 0x98],
        0x5de357,
        &o.rotation[8..12],
    );
    copy(
        &mut image,
        4,
        0x5de365,
        &[0x0f, 0x11, 0x45, 0xa8],
        0x5de36c,
        &o.rotation[12..16],
    );
    copy(
        &mut image,
        5,
        0x5de3a3,
        &[0x0f, 0x11, 0x45, 0xc0],
        0x5de3aa,
        &o.angle[..4],
    );
    for (site, disp, value) in [
        (0x5de315, 0xb8u8, o.rotation[16]),
        (0x5de330, 0xbc, o.rotation[17]),
        (0x5de347, 0xd0, o.angle[4]),
        (0x5de35e, 0xd4, o.angle[5]),
        (0x5de370, 0xe8, o.distance[4]),
        (0x5de38c, 0xec, o.distance[5]),
    ] {
        put(
            &mut image,
            site,
            &[&[0xc7, 0x45, disp][..], &value.to_le_bytes()].concat(),
        );
    }
    put(&mut image, LEVEL_COUNT_COMPARE, &[0x83, 0xfa, 6]);
    put(&mut image, ROTATION_COUNT_COMPARE, &[0x83, 0xf9, 18]);
    put(&mut image, STATE_STRIDE_FIRST, &[0x8d, 0x04, 0x40]);
    put(&mut image, STATE_STRIDE_SECOND, &[0x8d, 0x0c, 0x42]);
    put(
        &mut image,
        ROTATION_LOAD,
        &[0xf3, 0x0f, 0x10, 0x84, 0x85, 0x78, 0xff, 0xff, 0xff],
    );
    put(
        &mut image,
        ANGLE_LOAD,
        &[0xf3, 0x0f, 0x10, 0x44, 0x85, 0xc0],
    );
    put(
        &mut image,
        DISTANCE_LOAD,
        &[0xf3, 0x0f, 0x10, 0x44, 0xb5, 0xd8],
    );
    put(&mut image, TOP_LEVEL_LOAD, &[0xb8, 5, 0, 0, 0]);
    for (site, seconds) in [
        (ZOOM_EASE_STORE, o.zoom_ease),
        (ROTATION_EASE_STORE, o.rotation_ease),
    ] {
        put(
            &mut image,
            site,
            &[&[0xc7, 0x04, 0x24][..], &seconds.to_le_bytes()].concat(),
        );
    }
    let magic =
        ((1u64 << (32 + u32::from(o.wheel_shift))).div_ceil(u64::from(o.wheel_unit))) as u32;
    put(
        &mut image,
        WHEEL_DIVIDE,
        &[
            &[0xb8][..],
            &magic.to_le_bytes(),
            &[0xf7, 0xe9, 0x03, 0xd1, 0xc1, 0xfa, o.wheel_shift],
        ]
        .concat(),
    );
    // Constants: window, divisor, ceiling, offset.
    let [window, divisor, ceiling, offset] = [0, 4, 8, 12].map(|n| CONSTANTS + n);
    put(&mut image, window, &o.window.to_le_bytes());
    put(&mut image, divisor, &o.divisor.to_le_bytes());
    put(&mut image, ceiling, &o.ceiling.to_le_bytes());
    put(&mut image, offset, &o.offset.to_le_bytes());
    let with_abs = |prefix: &[u8], address: u32| [prefix, &address.to_le_bytes()].concat();
    for site in [SPACE_WINDOW, BACKSPACE_WINDOW] {
        put(
            &mut image,
            site,
            &with_abs(&[0xf3, 0x0f, 0x58, 0x05], window),
        );
    }
    put(&mut image, KEY_SLIDER_MAX, &[0x83, 0xf9, o.slider_max]);
    put(&mut image, MOUSE_SLIDER_MAX, &[0x83, 0xf8, o.slider_max]);
    for site in [KEY_DIVISOR, MOUSE_DIVISOR] {
        put(
            &mut image,
            site,
            &with_abs(&[0xf3, 0x0f, 0x5e, 0x0d], divisor),
        );
    }
    put(
        &mut image,
        KEY_CEILING,
        &with_abs(&[0xf3, 0x0f, 0x10, 0x15], ceiling),
    );
    put(
        &mut image,
        MOUSE_CEILING,
        &with_abs(&[0x0f, 0x2f, 0x0d], ceiling),
    );
    for site in [KEY_OFFSET, MOUSE_OFFSET] {
        put(
            &mut image,
            site,
            &with_abs(&[0xf3, 0x0f, 0x58, 0x0d], offset),
        );
    }
    put(
        &mut image,
        MOUSE_DEFAULT_STORE,
        &[
            &[0xc7, 0x87, 0xec, 0x04, 0, 0][..],
            &o.default.to_le_bytes(),
        ]
        .concat(),
    );
    put(
        &mut image,
        KEY_DEFAULT_STORE,
        &[
            &[0xc7, 0x87, 0xf0, 0x04, 0, 0][..],
            &o.default.to_le_bytes(),
        ]
        .concat(),
    );
    put(
        &mut image,
        KEY_MULTIPLIER_STORE,
        &[0xf3, 0x0f, 0x11, 0x8f, 0xf0, 0x04, 0, 0],
    );
    put(
        &mut image,
        MOUSE_MULTIPLIER_STORE,
        &[0xf3, 0x0f, 0x11, 0x87, 0xec, 0x04, 0, 0],
    );
    put(&mut image, DISABLE_STORE, &[0x89, 0x47, 0x30]);
    for (site, index) in [OPTION_KEY, OPTION_MOUSE, OPTION_DISABLE]
        .into_iter()
        .zip(o.options)
    {
        put(&mut image, site, &[0x6a, index]);
    }
    image
}

fn view(level: usize, swing: Swing) -> View {
    View { level, swing }
}

fn rules() -> CameraInputRules {
    let decoded = decode(&image(&Operands::original_like())).unwrap();
    CameraInputRules {
        origin: AssetIdentity {
            source: "synthetic".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        },
        image_sha256: [0; 32],
        distance: decoded.table.distance,
        angle_of_attack: decoded.table.angle_of_attack,
        rotation: decoded.table.rotation,
        top_level: decoded.table.levels - 1,
        zoom_ease: decoded.zoom_ease,
        rotation_ease: decoded.rotation_ease,
        wheel_unit: decoded.wheel_unit,
        repeat_window: decoded.repeat_window,
        scroll: decoded.scroll,
        fallback: ScrollOptions {
            mouse: 50,
            key: 50,
            mouse_disable: 0,
        },
    }
}

#[test]
fn the_test_fixture_matches_the_decoded_original_shaped_image() {
    let (fixture, decoded) = (CameraInputRules::fixture(), rules());
    assert_eq!(fixture.distance, decoded.distance);
    assert_eq!(fixture.angle_of_attack, decoded.angle_of_attack);
    assert_eq!(fixture.rotation, decoded.rotation);
    assert_eq!(fixture.top_level, decoded.top_level);
    assert_eq!(
        (
            fixture.zoom_ease,
            fixture.rotation_ease,
            fixture.wheel_unit,
            fixture.repeat_window
        ),
        (
            decoded.zoom_ease,
            decoded.rotation_ease,
            decoded.wheel_unit,
            decoded.repeat_window
        )
    );
    assert_eq!(fixture.scroll, decoded.scroll);
}

#[test]
fn instruction_operands_supply_every_table_and_constant() {
    let o = Operands {
        distance: [11.0, 12.0, 13.0, 14.0, 15.0, 16.0],
        angle: [301.0, 302.0, 303.0, 304.0, 305.0, 306.0],
        rotation: std::array::from_fn(|i| 100.0 + i as f32),
        zoom_ease: 0.75,
        rotation_ease: 0.25,
        wheel_unit: 100,
        wheel_shift: 6,
        window: 2.5,
        slider_max: 80,
        divisor: 80.0,
        ceiling: 2.0,
        offset: 0.25,
        default: 3.0,
        options: [1, 2, 3],
    };
    let decoded = decode(&image(&o)).unwrap();
    assert_eq!(decoded.table.levels, 6);
    assert_eq!(decoded.table.distance, o.distance);
    assert_eq!(decoded.table.angle_of_attack, o.angle);
    assert_eq!(decoded.table.rotation, o.rotation);
    assert_eq!((decoded.zoom_ease, decoded.rotation_ease), (0.75, 0.25));
    assert_eq!(decoded.wheel_unit, 100);
    assert_eq!(decoded.repeat_window, 2.5);
    assert_eq!(decoded.scroll.slider_max, 80);
    // 40 / 80 + 0.25, then out of range keeps the constructor default.
    assert_eq!(decoded.scroll.multiplier(40), 0.75);
    assert_eq!(decoded.scroll.multiplier(81), 3.0);
    assert_eq!(
        (
            decoded.indices.key,
            decoded.indices.mouse,
            decoded.indices.disable
        ),
        (1, 2, 3)
    );
}

#[test]
fn every_reviewed_site_refuses_a_changed_instruction() {
    let good = image(&Operands::original_like());
    let mut sites = vec![
        LEVEL_COUNT_COMPARE,
        ROTATION_COUNT_COMPARE,
        STATE_STRIDE_FIRST,
        STATE_STRIDE_SECOND,
        ROTATION_LOAD,
        ANGLE_LOAD,
        DISTANCE_LOAD,
        TOP_LEVEL_LOAD,
        ZOOM_EASE_STORE,
        ROTATION_EASE_STORE,
        WHEEL_DIVIDE,
        SPACE_WINDOW,
        BACKSPACE_WINDOW,
        OPTION_KEY,
        OPTION_MOUSE,
        OPTION_DISABLE,
        KEY_SLIDER_MAX,
        MOUSE_SLIDER_MAX,
        KEY_DIVISOR,
        MOUSE_DIVISOR,
        KEY_CEILING,
        MOUSE_CEILING,
        KEY_OFFSET,
        MOUSE_OFFSET,
        MOUSE_DEFAULT_STORE,
        KEY_DEFAULT_STORE,
        KEY_MULTIPLIER_STORE,
        MOUSE_MULTIPLIER_STORE,
        DISABLE_STORE,
    ];
    sites.extend(FRAME_IMMEDIATES);
    sites.extend(
        FRAME_COPIES
            .iter()
            .flat_map(|(load, store)| [*load, *store]),
    );
    for site in sites {
        let mut changed = good.clone();
        changed[RAW + (site - CODE_VA) as usize] ^= 0xff;
        // The option index sites carry no opcode check beyond the push byte at +0.
        assert!(decode(&changed).is_err(), "{site:#x}");
    }
    assert!(decode(&good[..0x2000]).is_err());
}

#[test]
fn inconsistent_operands_are_refused_not_repaired() {
    let mut o = Operands::original_like();
    o.wheel_unit = 120;
    // A magic number that is not a division by a constant.
    let mut changed = image(&o);
    changed[RAW + (WHEEL_DIVIDE - CODE_VA) as usize + 1] ^= 0x10;
    assert!(decode(&changed).is_err());
    // A top level that disagrees with the table length.
    let mut changed = image(&o);
    changed[RAW + (TOP_LEVEL_LOAD - CODE_VA) as usize + 1] = 4;
    assert!(decode(&changed).is_err());
    // Windows that differ between Space and Backspace.
    let mut changed = image(&o);
    let at = RAW + (SPACE_WINDOW - CODE_VA) as usize + 4;
    changed[at..at + 4].copy_from_slice(&(CONSTANTS + 4).to_le_bytes());
    assert!(decode(&changed).is_err());
    // A negative ease and a non-positive distance.
    o.zoom_ease = -1.0;
    assert!(decode(&image(&o)).is_err());
    let mut o = Operands::original_like();
    o.distance[2] = 0.0;
    assert!(decode(&image(&o)).is_err());
    // The synthetic image is not the reviewed client.
    let origin = rules().origin;
    assert!(CameraInputRules::read(&image(&Operands::original_like()), origin).is_err());
}

#[test]
fn option_fallbacks_must_resolve_to_the_named_options() {
    let indices = OptionIndices {
        key: 42,
        mouse: 40,
        disable: 41,
    };
    let rows: Vec<OptionRow<'_>> = vec![
        (40, "mousescroll", b"50"),
        (41, "mousescrolldisable", b"0"),
        (42, "keyscroll", b"75"),
    ];
    assert_eq!(
        fallback_options(&rows, &indices).unwrap(),
        ScrollOptions {
            mouse: 50,
            key: 75,
            mouse_disable: 0
        }
    );
    let swapped: Vec<OptionRow<'_>> = vec![
        (40, "keyscroll", b"50"),
        (41, "mousescrolldisable", b"0"),
        (42, "mousescroll", b"50"),
    ];
    assert!(fallback_options(&swapped, &indices).is_err());
    let text: Vec<OptionRow<'_>> = vec![
        (40, "mousescroll", b"fast"),
        (41, "mousescrolldisable", b"0"),
        (42, "keyscroll", b"50"),
    ];
    assert!(fallback_options(&text, &indices).is_err());
    assert!(fallback_options(&rows[..2], &indices).is_err());
}

#[test]
fn zoom_clamps_to_the_table_and_eases_over_the_zoom_time() {
    let rules = rules();
    let mut state = view(0, Swing::Normal);
    assert_eq!(rules.zoom_out(&mut state), None);
    let ease = rules.zoom_in(&mut state).unwrap();
    assert_eq!(state.level, 1);
    assert_eq!(ease.seconds, rules.zoom_ease);
    assert_eq!(
        ease.pose,
        Pose {
            distance: 1600.0,
            angle_of_attack: 311.0,
            rotation: 90.0
        }
    );
    for _ in 0..10 {
        rules.zoom_in(&mut state);
    }
    assert_eq!(state.level, 5);
    assert_eq!(rules.zoom_in(&mut state), None);
    // The last level settles at the executable literals, not the [Camera] file.
    let pose = rules.pose(state);
    assert_eq!((pose.distance, pose.angle_of_attack), (1200.0, 335.0));
    assert_eq!(rules.zoom_out(&mut state).unwrap().pose.distance, 1275.0);
}

#[test]
fn wheel_moves_one_level_per_notch_and_ignores_smaller_deltas() {
    let rules = rules();
    let mut state = view(2, Swing::Normal);
    assert!(rules.wheel(&mut state, 119).is_none());
    assert!(rules.wheel(&mut state, -119).is_none());
    assert!(rules.wheel(&mut state, 0).is_none());
    assert!(rules.wheel(&mut state, 120).is_some());
    assert_eq!(state.level, 3);
    // A larger delta is still one level.
    assert!(rules.wheel(&mut state, 360).is_some());
    assert_eq!(state.level, 4);
    assert!(rules.wheel(&mut state, -120).is_some());
    assert!(rules.wheel(&mut state, -240).is_some());
    assert_eq!(state.level, 2);
    state.level = 0;
    assert!(rules.wheel(&mut state, -120).is_none());
    state.level = 5;
    assert!(rules.wheel(&mut state, 120).is_none());
}

#[test]
fn rotation_states_hold_until_the_matching_release() {
    let rules = rules();
    let mut state = view(3, Swing::Normal);
    let insert = rules.swing_press(&mut state, Swing::Insert).unwrap();
    assert_eq!(insert.pose.rotation, 20.0);
    assert_eq!(insert.seconds, rules.rotation_ease);
    // Repeats and the other key are ignored while one state is held.
    assert!(rules.swing_press(&mut state, Swing::Insert).is_none());
    assert!(rules.swing_press(&mut state, Swing::Delete).is_none());
    assert!(rules.swing_release(&mut state, Swing::Delete).is_none());
    // Zooming keeps the held rotation state.
    assert_eq!(rules.zoom_in(&mut state).unwrap().pose.rotation, 20.0);
    let back = rules.swing_release(&mut state, Swing::Insert).unwrap();
    assert_eq!((back.pose.rotation, back.pose.distance), (90.0, 1275.0));
    let delete = rules.swing_press(&mut state, Swing::Delete).unwrap();
    assert_eq!(delete.pose.rotation, 160.0);
    assert_eq!(
        rules
            .swing_release(&mut state, Swing::Delete)
            .unwrap()
            .pose
            .rotation,
        90.0
    );
    assert_eq!(state.swing, Swing::Normal);
}

#[test]
fn rotation_table_selects_by_level_plus_six_times_state() {
    let o = Operands {
        rotation: std::array::from_fn(|i| i as f32),
        ..Operands::original_like()
    };
    let decoded = decode(&image(&o)).unwrap();
    let mut rules = rules();
    rules.rotation = decoded.table.rotation;
    for (swing, state) in [(Swing::Normal, 0), (Swing::Insert, 1), (Swing::Delete, 2)] {
        for level in 0..6 {
            assert_eq!(
                rules.pose(view(level, swing)).rotation,
                (level + 6 * state) as f32
            );
        }
    }
}

#[test]
fn scroll_multiplier_follows_the_slider_and_keeps_the_default_out_of_range() {
    let scroll = rules().scroll;
    assert_eq!(scroll.multiplier(0), 0.5);
    assert_eq!(scroll.multiplier(50), 1.0);
    assert_eq!(scroll.multiplier(100), 1.5);
    assert_eq!(scroll.multiplier(101), 1.0);
    assert_eq!(scroll.multiplier(u32::MAX), 1.0);
}

#[test]
fn edge_margins_reproduce_the_measured_boundaries_at_three_resolutions() {
    use edge::*;
    // (size, scrolling left x, right x, top y, bottom y) from the live capture.
    let measured: [([f32; 2], [f32; 2], [f32; 2], [f32; 2], [f32; 2]); 3] = [
        (
            [1280.0, 960.0],
            [0.0, 6.0],
            [1274.0, 1279.0],
            [0.0, 6.0],
            [954.0, 959.0],
        ),
        (
            [1024.0, 768.0],
            [0.0, 5.0],
            [1019.0, 1023.0],
            [0.0, 5.0],
            [763.0, 767.0],
        ),
        // 1920x1080: the bottom boundary was not measured, so it is not asserted.
        (
            [1920.0, 1080.0],
            [0.0, 9.0],
            [1911.0, 1919.0],
            [0.0, 7.0],
            [0.0, 0.0],
        ),
    ];
    for (size, left, right, top, bottom) in measured {
        let mid = [size[0] / 2.0, size[1] / 2.0];
        for x in left[0] as u32..=left[1] as u32 {
            assert_eq!(edge_mask([x as f32, mid[1]], size), LEFT, "{size:?} {x}");
        }
        assert_eq!(edge_mask([left[1] + 1.0, mid[1]], size), 0, "{size:?}");
        for x in right[0] as u32..=right[1] as u32 {
            assert_eq!(edge_mask([x as f32, mid[1]], size), RIGHT, "{size:?} {x}");
        }
        assert_eq!(edge_mask([right[0] - 1.0, mid[1]], size), 0, "{size:?}");
        for y in top[0] as u32..=top[1] as u32 {
            assert_eq!(edge_mask([mid[0], y as f32], size), UP, "{size:?} {y}");
        }
        assert_eq!(edge_mask([mid[0], top[1] + 1.0], size), 0, "{size:?}");
        if bottom[1] > 0.0 {
            for y in bottom[0] as u32..=bottom[1] as u32 {
                assert_eq!(edge_mask([mid[0], y as f32], size), DOWN, "{size:?} {y}");
            }
            assert_eq!(edge_mask([mid[0], bottom[0] - 1.0], size), 0, "{size:?}");
        }
    }
    // Corners scroll both axes at once; (7, 7) does not at 1280x960.
    assert_eq!(edge_mask([6.0, 6.0], [1280.0, 960.0]), LEFT | UP);
    assert_eq!(edge_mask([7.0, 7.0], [1280.0, 960.0]), 0);
    assert_eq!(edge_mask([1279.0, 959.0], [1280.0, 960.0]), RIGHT | DOWN);
}

#[test]
fn repeat_cycles_continue_inside_the_window_and_restart_outside() {
    let window = 1.0;
    let mut cycle = Cycle::default();
    assert_eq!(cycle.next(0.0, window, 0), None);
    assert_eq!(cycle.next(10.0, window, 3), Some(0));
    assert_eq!(cycle.next(10.5, window, 3), Some(1));
    assert_eq!(cycle.next(11.25, window, 3), Some(2));
    assert_eq!(cycle.next(12.0, window, 3), Some(0));
    // Exactly one window after the previous press restarts.
    assert_eq!(cycle.next(13.0, window, 3), Some(0));
    // A shrunken list wraps instead of overrunning.
    assert_eq!(cycle.next(13.5, window, 3), Some(1));
    assert_eq!(cycle.next(14.0, window, 1), Some(0));
    // Space starts at the newest entry and steps back through the history.
    let mut cycle = Cycle::default();
    assert_eq!(cycle.previous(0.0, window, 0, 0), None);
    assert_eq!(cycle.previous(5.0, window, 3, 2), Some(2));
    assert_eq!(cycle.previous(5.5, window, 3, 2), Some(1));
    assert_eq!(cycle.previous(6.0, window, 3, 2), Some(0));
    assert_eq!(cycle.previous(6.5, window, 3, 2), Some(2));
    assert_eq!(cycle.previous(8.0, window, 3, 2), Some(2));
}

#[test]
#[ignore = "requires explicitly supplied reviewed Legacy client image"]
fn original_camera_rules_come_from_the_selected_image() {
    let path = std::env::var("PCX_CAMERA_CLIENT_IMAGE").expect("PCX_CAMERA_CLIENT_IMAGE");
    let bytes = std::fs::read(&path).unwrap();
    let origin = AssetIdentity {
        source: path,
        edition: "Legacy target".into(),
        version: "1.29.2.9231".into(),
    };
    let rules = CameraInputRules::read(&bytes, origin).unwrap();
    assert_eq!(rules.levels(), 6);
    let poses: Vec<_> = (0..6)
        .map(|level| {
            let pose = rules.pose(view(level, Swing::Normal));
            (pose.distance, pose.angle_of_attack, pose.rotation)
        })
        .collect();
    assert_eq!(
        poses,
        [
            (1650.0, 304.0, 90.0),
            (1600.0, 311.0, 90.0),
            (1500.0, 318.0, 90.0),
            (1400.0, 325.0, 90.0),
            (1275.0, 332.0, 90.0),
            (1200.0, 335.0, 90.0),
        ]
    );
    assert_eq!(rules.pose(view(0, Swing::Insert)).rotation, 20.0);
    assert_eq!(rules.pose(view(5, Swing::Delete)).rotation, 160.0);
    assert_eq!((rules.zoom_ease, rules.rotation_ease), (1.0, 0.5));
    assert_eq!((rules.wheel_unit, rules.repeat_window), (120, 1.0));
    assert_eq!(rules.scroll.multiplier(50), 1.0);
    assert_eq!(
        rules.fallback,
        ScrollOptions {
            mouse: 50,
            key: 50,
            mouse_disable: 0
        }
    );
}
