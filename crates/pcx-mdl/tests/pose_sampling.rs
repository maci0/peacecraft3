use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
};

use pcx_mdl::{
    Bone, BoneTrack, Interpolation, Pose, TrackKind, sample_bone_pose, sample_bone_track,
};

struct Counting;
thread_local! {
    // Const TLS avoids allocation and excludes the test runner's other threads.
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

fn count_allocation() {
    let _ = ALLOCATIONS.try_with(|count| {
        if let Some(n) = count.get() {
            count.set(Some(n.saturating_add(1)));
        }
    });
}

// GlobalAlloc is required to test actual allocations during borrowed sampling.
// SAFETY: System receives unchanged layouts and valid pointers from GlobalAlloc callers;
// const TLS counters cannot allocate and try_with tolerates thread teardown.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_allocation();
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn track(kind: TrackKind, interpolation: Interpolation, count: u32) -> BoneTrack {
    let width = if kind == TrackKind::Rotation { 4 } else { 3 };
    let tangents = matches!(
        interpolation,
        Interpolation::Hermite | Interpolation::Bezier
    );
    let mut raw = Vec::new();
    for (time, value) in [(10_u32, 2_f32), (110, 6.)]
        .into_iter()
        .take(count as usize)
    {
        raw.extend(time.to_le_bytes());
        for base in [value, -3., 9.]
            .into_iter()
            .take(if tangents { 3 } else { 1 })
        {
            for component in 0..width {
                raw.extend((base + component as f32).to_le_bytes());
            }
        }
    }
    BoneTrack {
        kind,
        count,
        interpolation,
        global_sequence_id: 0,
        offset: 0,
        stride: 4 + width * 4 * if tangents { 3 } else { 1 },
        raw,
    }
}

fn bone(tracks: Vec<BoneTrack>) -> Bone {
    Bone {
        offset: 0,
        span: 0,
        name: String::new(),
        object_id: 7,
        parent_id: -1,
        flags: 0,
        tracks,
        geoset_id: -1,
        geoset_anim_id: -1,
    }
}

fn expected(kind: TrackKind, value: f32) -> Pose {
    let mut pose = Pose::default();
    match kind {
        TrackKind::Translation => pose.translation = [value, value + 1., value + 2.],
        TrackKind::Scaling => pose.scale = [value, value + 1., value + 2.],
        TrackKind::Rotation => pose.rotation = [value, value + 1., value + 2., value + 3.],
    }
    pose
}

// Only the current thread's sampling window is counted.
#[test]
fn borrowed_sampling_preserves_curves_identities_and_allocates_nothing() {
    let mut tracks = Vec::new();
    for kind in [
        TrackKind::Translation,
        TrackKind::Rotation,
        TrackKind::Scaling,
    ] {
        for (interpolation, midpoint) in [
            (Interpolation::None, 2.),
            (Interpolation::Linear, 4.),
            (Interpolation::Hermite, 5.5),
            (Interpolation::Bezier, 3.25),
        ] {
            let t = track(kind, interpolation, 2);
            for (frame, value) in [(0, 2.), (10, 2.), (60, midpoint), (110, 6.), (200, 6.)] {
                assert_eq!(sample_bone_track(&t, frame), expected(kind, value));
                assert_eq!(
                    sample_bone_pose(&bone(vec![t.clone()]), frame),
                    expected(kind, value)
                );
            }
            let empty = track(kind, interpolation, 0);
            let mut empty_pose = Pose::default();
            match kind {
                TrackKind::Translation => empty_pose.translation = [0.; 3],
                TrackKind::Rotation => empty_pose.rotation = [0.; 4],
                TrackKind::Scaling => empty_pose.scale = [0.; 3],
            }
            assert_eq!(sample_bone_track(&empty, 60), empty_pose);
            let single = track(kind, interpolation, 1);
            assert_eq!(sample_bone_track(&single, 200), expected(kind, 2.));
            tracks.extend([t, empty, single]);
        }
    }
    let mut later = track(TrackKind::Translation, Interpolation::Linear, 2);
    later.raw[4..8].copy_from_slice(&20_f32.to_le_bytes());
    let repeated = bone(vec![tracks[0].clone(), later]);
    assert_eq!(
        sample_bone_pose(&repeated, 10),
        expected(TrackKind::Translation, 2.)
    );

    ALLOCATIONS.set(Some(0));
    for frame in 0..1000 {
        for t in &tracks {
            black_box(sample_bone_track(black_box(t), frame));
        }
        black_box(sample_bone_pose(black_box(&repeated), frame));
    }
    let allocations = ALLOCATIONS.replace(None).unwrap();
    assert_eq!(allocations, 0);
}

#[test]
fn local_sequence_sampling_excludes_other_clips_without_allocating() {
    use pcx_mdl::sample_bone_track_in_interval as sample;
    for kind in [
        TrackKind::Translation,
        TrackKind::Rotation,
        TrackKind::Scaling,
    ] {
        for interpolation in [
            Interpolation::None,
            Interpolation::Linear,
            Interpolation::Hermite,
            Interpolation::Bezier,
        ] {
            let t = track(kind, interpolation, 2);
            assert_eq!(sample(&t, 0, [0, 9]), Pose::default());
            assert_eq!(sample(&t, 70, [11, 109]), Pose::default());
            assert_eq!(sample(&t, 200, [111, 200]), Pose::default());
            assert_eq!(sample(&t, 80, [0, 10]), expected(kind, 2.));
            assert_eq!(sample(&t, 80, [110, 200]), expected(kind, 6.));
            for frame in [0, 10, 60, 110, 200] {
                assert_eq!(sample(&t, frame, [10, 110]), sample_bone_track(&t, frame));
            }
            ALLOCATIONS.with(|count| count.set(Some(0)));
            black_box(sample(black_box(&t), 60, [11, 109]));
            black_box(sample(black_box(&t), 60, [10, 110]));
            let allocated = ALLOCATIONS.with(|count| count.replace(None).unwrap());
            assert_eq!(allocated, 0);
        }
    }
}
