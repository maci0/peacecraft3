use super::*;
use crate::{
    campaign_placement::{DoodadAppearance, PlacedDoodad, RectangleSelection},
    playercolor_host::{EnumHost, Limits},
    unit_model::{Animation, Model},
};
use pcx_script::{Host, Prelude};
use std::sync::Arc;

const COMMON: &str = "type agent extends handle
type timer extends agent
type rect extends agent
type playercolor extends handle
constant native ConvertPlayerColor takes integer i returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
native Rect takes real a, real b, real c, real d returns rect
native RemoveRect takes rect r returns nothing
native SetRect takes rect r, real a, real b, real c, real d returns nothing
native MoveRectTo takes rect r, real x, real y returns nothing
native GetRectCenterX takes rect r returns real
native GetRectCenterY takes rect r returns real
native GetRectMinX takes rect r returns real
native GetRectMinY takes rect r returns real
native GetRectMaxX takes rect r returns real
native GetRectMaxY takes rect r returns real
native SetDoodadAnimationRect takes rect r, integer id, string clip, boolean random returns nothing
globals
constant playercolor C = ConvertPlayerColor(37)
endglobals
";

fn host(common: &str) -> ObjectHost {
    let declarations = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = pcx_script::resolve_with(
        &map,
        &[Prelude::new("synthetic:doodad-common", &declarations)],
    )
    .unwrap();
    let inner = EnumHost::checked_selected(
        Arc::new(program),
        "synthetic:map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 16,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(inner)
        .unwrap()
        .with_rectangles()
        .unwrap()
        .with_timer_runtime(7.)
        .unwrap()
}
fn row(id: u32, kind: [u8; 4], x: f32) -> PlacedDoodad {
    let sequences = [("Stand", 123, 823, false), ("Death", 1987, 2143, true)]
        .into_iter()
        .map(
            |(name, interval_start, interval_end, non_looping)| pcx_mdl::Sequence {
                name: name.into(),
                interval_start,
                interval_end,
                non_looping,
                move_speed: 0.,
                trailing: [0; 7],
            },
        )
        .collect();
    let model = Arc::new(Model {
        origin: crate::scripts::AssetIdentity {
            source: "synthetic:model".into(),
            edition: "fixture".into(),
            version: "test".into(),
        },
        member: "Synthetic/Decoration.mdx".into(),
        bytes: Arc::from([]),
        sequences,
        blend_time_ms: 0,
        joints: Vec::new(),
        global_sequences: Vec::new(),
    });
    PlacedDoodad {
        shown: true,
        selection: RectangleSelection::Point,
        source: pcx_map::Doodad {
            type_id: kind,
            variation: 0,
            position: [x, -4., 137.],
            angle: 0.75,
            scale: [0.75, 1.25, 2.],
            flags: 2,
            life: 255,
            creation_number: id,
        },
        appearance: Ok(DoodadAppearance {
            tint: [33., 91., 151.],
            animation: Animation::new(model, 0.).unwrap(),
        }),
    }
}
fn rectangle(h: &mut ObjectHost) -> Value {
    h.native("Rect", &[-2., -7., 3., 5.].map(Value::Real))
        .unwrap()
}
fn animate(h: &mut ObjectHost, rect: Value, id: [u8; 4], clip: &str) -> Result<Value, String> {
    h.native(
        NATIVE,
        &[
            rect,
            Value::Int(i32::from_be_bytes(id)),
            Value::Str(clip.into()),
            Value::Bool(false),
        ],
    )
}

#[test]
fn rectangle_selects_canonical_instances_and_restarts_the_authored_clip() {
    let mut h = host(COMMON);
    h.admit_placed_doodads(vec![
        row(17, *b"Zx91", -2.),
        row(93, *b"Zx91", 3.),
        row(4, *b"Zx91", 3.125),
        row(72, *b"Zx92", 0.),
    ])
    .unwrap();
    let rect = rectangle(&mut h);
    animate(&mut h, rect.clone(), *b"Zx91", "death").unwrap();
    let rows: Vec<_> = h.placed_doodads().collect();
    for r in &rows[..2] {
        let a = &r.appearance.as_ref().unwrap().animation;
        assert_eq!(a.model.sequences[a.sequence.unwrap()].name, "Death");
        assert_eq!(a.sample(7.).unwrap().unwrap().frame, 1987);
        assert_eq!(a.sample(8.).unwrap().unwrap().frame, 2143);
    }
    for r in &rows[2..] {
        assert_eq!(r.appearance.as_ref().unwrap().animation.started, 0.);
    }
    h.native("RemoveRect", &[rect]).unwrap();
    assert_eq!(
        h.placed_doodads().next().unwrap().source.position,
        [-2., -4., 137.]
    );
    assert_eq!(
        h.placed_doodads()
            .next()
            .unwrap()
            .appearance
            .as_ref()
            .unwrap()
            .tint,
        [33., 91., 151.]
    );
}

#[test]
fn a_failed_batch_never_partially_changes_earlier_matches() {
    let mut h = host(COMMON);
    let mut invalid = row(22, *b"Zx91", 1.);
    invalid.appearance = Err("synthetic missing source model".into());
    h.admit_placed_doodads(vec![row(17, *b"Zx91", 0.), invalid])
        .unwrap();
    let rect = rectangle(&mut h);
    assert!(
        animate(&mut h, rect.clone(), *b"Zx91", "death")
            .unwrap_err()
            .contains("missing source model")
    );
    assert_eq!(
        h.placed_doodads()
            .next()
            .unwrap()
            .appearance
            .as_ref()
            .unwrap()
            .animation
            .started,
        0.
    );
    assert!(
        h.native(
            NATIVE,
            &[
                rect,
                Value::Int(-1),
                Value::Str("stand".into()),
                Value::Bool(true)
            ]
        )
        .is_err()
    );
    assert_eq!(
        h.native(
            NATIVE,
            &[
                Value::Null,
                Value::Int(-1),
                Value::Str("death".into()),
                Value::Bool(false)
            ]
        )
        .unwrap(),
        Value::Null
    );
}

#[test]
fn malformed_contract_refuses_admission() {
    let mut h = host(&COMMON.replace(
        "string clip, boolean random",
        "integer clip, boolean random",
    ));
    assert!(h.admit_placed_doodads(vec![]).is_err());
    assert_eq!(h.placed_doodads().count(), 0);
}

#[test]
fn duplicate_creation_numbers_are_distinct_placements() {
    // UndeadX07a authors decorative doodads that share a creation number; the number is
    // an editor label, so every row is admitted in file order.
    let mut h = host(COMMON);
    h.admit_placed_doodads(vec![row(1, *b"Zx91", 0.), row(1, *b"Zx92", 5.)])
        .unwrap();
    let placed: Vec<(u32, f32)> = h
        .placed_doodads()
        .map(|row| (row.source.creation_number, row.source.position[0]))
        .collect();
    assert_eq!(placed, [(1, 0.), (1, 5.)]);
}

fn footprint(
    id: u32,
    kind: [u8; 4],
    position: [f32; 2],
    angle: f32,
    size: [u16; 2],
) -> PlacedDoodad {
    let mut placed = row(id, kind, position[0]);
    placed.source.position[1] = position[1];
    placed.source.angle = angle;
    placed.selection = RectangleSelection::Footprint {
        width: size[0],
        height: size[1],
    };
    placed
}

fn selected_by(rows: Vec<PlacedDoodad>) -> Vec<u32> {
    let mut h = host(COMMON);
    h.admit_placed_doodads(rows).unwrap();
    let rect = rectangle(&mut h);
    animate(&mut h, rect, *b"Zx91", "death").unwrap();
    h.placed_doodads()
        .filter(|r| r.appearance.as_ref().unwrap().animation.started != 0.)
        .map(|r| r.source.creation_number)
        .collect()
}

#[test]
fn footprint_boxes_overlap_the_rectangle_strictly_and_turn_with_the_facing() {
    // Rectangle (-2, -7) to (3, 5); a 2 by 1 footprint has half extents 32 and 16.
    let half_turn = std::f32::consts::FRAC_PI_2;
    let selected = selected_by(vec![
        footprint(1, *b"Zx91", [30., -4.], 0., [2, 1]),
        // The box edge touches maximum x exactly, so it does not overlap.
        footprint(2, *b"Zx91", [35., -4.], 0., [2, 1]),
        // A quarter turn exchanges the half extents: the edge is now at 19 - 16 = 3.
        footprint(3, *b"Zx91", [19., -4.], half_turn, [2, 1]),
        footprint(4, *b"Zx91", [18., -4.], half_turn, [2, 1]),
        // Along y the unturned half extent is 16: 21 - 16 = 5 touches maximum y.
        footprint(5, *b"Zx91", [0., 21.], 0., [2, 1]),
        footprint(6, *b"Zx91", [0., 20.], 0., [2, 1]),
        // Negative facings normalize: -pi/2 is the quarter turn that swaps.
        footprint(7, *b"Zx91", [18., -4.], -half_turn, [2, 1]),
        // Nearest quarter: 40 degrees stays unturned, 50 degrees turns (edge at 3).
        footprint(8, *b"Zx91", [19., -4.], 40f32.to_radians(), [2, 1]),
        footprint(9, *b"Zx91", [19., -4.], 50f32.to_radians(), [2, 1]),
    ]);
    assert_eq!(selected, [1, 4, 6, 7, 8]);
}

#[test]
fn a_point_doodad_keeps_inclusive_edges_beside_footprint_doodads() {
    let selected = selected_by(vec![
        row(1, *b"Zx91", 3.),
        row(2, *b"Zx91", 3.5),
        footprint(3, *b"Zx91", [-2., 30.], 0., [0, 5]),
    ]);
    assert_eq!(selected, [1]);
}

#[test]
fn an_unreadable_footprint_refuses_only_when_its_type_is_selected() {
    let mut unread = row(1, *b"Zx92", 0.);
    unread.selection = RectangleSelection::Unresolved("synthetic missing texture".into());
    let mut h = host(COMMON);
    h.admit_placed_doodads(vec![unread]).unwrap();
    let rect = rectangle(&mut h);
    animate(&mut h, rect.clone(), *b"Zx91", "death").unwrap();
    let error = animate(&mut h, rect, *b"Zx92", "death").unwrap_err();
    assert!(error.contains("synthetic missing texture"), "{error}");
}

#[test]
fn a_non_finite_facing_refuses_instead_of_looping() {
    for angle in [f32::NEG_INFINITY, f32::NAN, -1.0e30] {
        let mut h = host(COMMON);
        h.admit_placed_doodads(vec![footprint(1, *b"Zx91", [0., 0.], angle, [1, 1])])
            .unwrap();
        let rect = rectangle(&mut h);
        assert!(animate(&mut h, rect, *b"Zx91", "death").is_err(), "{angle}");
    }
}
