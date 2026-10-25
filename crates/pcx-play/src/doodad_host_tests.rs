use super::*;
use crate::{
    campaign_placement::{DoodadAppearance, PlacedDoodad},
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
        point_selection: true,
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
fn malformed_contract_and_duplicate_creation_numbers_refuse_admission() {
    let mut h = host(&COMMON.replace(
        "string clip, boolean random",
        "integer clip, boolean random",
    ));
    assert!(h.admit_placed_doodads(vec![]).is_err());
    assert_eq!(h.placed_doodads().count(), 0);
    let mut h = host(COMMON);
    assert!(
        h.admit_placed_doodads(vec![row(1, *b"Zx91", 0.), row(1, *b"Zx92", 0.)])
            .is_err()
    );
    assert_eq!(h.placed_doodads().count(), 0);
}
