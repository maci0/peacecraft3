use super::*;
use bevy::time::TimeUpdateStrategy;
use pcx_play::{error_line_rules::Shadow, message_layout::Placement, scripts::AssetIdentity};
use std::time::Duration;

const HOLD: f32 = 4.0;
const FADE: f32 = 2.0;

fn loaded() -> Loaded {
    Loaded {
        rules: ErrorLineRules {
            origin: AssetIdentity {
                source: "synthetic client".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            image_sha256: [0; 32],
            frame: Placement {
                left: 0.2,
                bottom: 0.15,
                width: 0.4,
                height: 0.06,
                viewport: [0.8, 0.6],
            },
            height_key: "Synthetic".into(),
            font_key: "SyntheticFont".into(),
            colour: [0x10, 0x20, 0x30, 0xff],
            hold_seconds: HOLD,
            fade_seconds: FADE,
            shadow: Shadow {
                colour: [0, 0, 0, 0xff],
                offset: [0.002, -0.002],
            },
        },
        style: MessageStyle {
            font_path: "synthetic.ttf".into(),
            height: 0.03,
        },
        font: Handle::default(),
    }
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(CampaignCamera::active_for_test(Entity::PLACEHOLDER))
        .init_resource::<ErrorLine>()
        .insert_resource(ErrorView {
            loaded: Some(loaded()),
            ..default()
        })
        .add_systems(Update, show);
    // The first frame starts the clock without advancing it.
    app.update();
    app
}

fn post(app: &mut App, text: &str) {
    let now = app.world().resource::<Time<Real>>().elapsed_secs_f64();
    app.world_mut().resource_mut::<ErrorLine>().post(text, now);
}

/// Moves the real clock by `seconds` and runs one frame.
fn frame(app: &mut App, seconds: f32) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
        seconds,
    )));
    app.update();
}

fn lines(app: &mut App) -> Vec<(String, Color, Node)> {
    app.world_mut()
        .query::<(&Text, &TextColor, &ChildOf)>()
        .iter(app.world())
        .map(|(text, colour, parent)| {
            (
                text.0.clone(),
                colour.0,
                app.world().get::<Node>(parent.parent()).unwrap().clone(),
            )
        })
        .collect()
}

fn alpha(colour: Color) -> u8 {
    (colour.to_srgba().alpha * 255.0).round() as u8
}

#[test]
fn an_unposted_line_draws_nothing() {
    let mut app = app();
    frame(&mut app, 0.1);
    assert!(lines(&mut app).is_empty());
}

#[test]
fn a_post_draws_one_line_in_the_frame_with_the_client_colour() {
    let mut app = app();
    post(&mut app, "Not enough gold.");
    frame(&mut app, 0.1);
    let shown = lines(&mut app);
    let [(text, colour, node)] = shown.as_slice() else {
        panic!("one line expected, found {}", shown.len());
    };
    assert_eq!(text, "Not enough gold.");
    assert_eq!(*colour, Color::srgba_u8(0x10, 0x20, 0x30, 0xff));
    for (actual, expected) in [
        (node.left, 25.0),
        (node.bottom, 25.0),
        (node.width, 50.0),
        (node.height, 10.0),
    ] {
        let Val::Percent(actual) = actual else {
            panic!("relative layout expected, found {actual:?}");
        };
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }
    assert_eq!(node.display, Display::Flex);
}

#[test]
fn the_line_holds_then_fades_then_goes() {
    let mut app = app();
    post(&mut app, "x");
    frame(&mut app, HOLD);
    assert_eq!(alpha(lines(&mut app)[0].1), 255, "held for the full hold");
    frame(&mut app, FADE / 2.0);
    assert_eq!(
        alpha(lines(&mut app)[0].1),
        128,
        "half way through the fade"
    );
    frame(&mut app, FADE / 2.0 + 0.01);
    assert!(lines(&mut app).is_empty(), "gone after hold plus fade");
}

#[test]
fn the_shadow_fades_with_the_text() {
    let mut app = app();
    post(&mut app, "x");
    frame(&mut app, HOLD + FADE / 2.0);
    let alphas: Vec<u8> = app
        .world_mut()
        .query::<&TextShadow>()
        .iter(app.world())
        .map(|shadow| alpha(shadow.color))
        .collect();
    assert_eq!(alphas, [128]);
}

#[test]
fn a_new_post_replaces_the_line_and_restarts_it() {
    let mut app = app();
    post(&mut app, "first");
    frame(&mut app, HOLD + FADE / 2.0);
    post(&mut app, "second");
    frame(&mut app, 0.5);
    let shown = lines(&mut app);
    let [(text, colour, _)] = shown.as_slice() else {
        panic!("one line expected, found {}", shown.len());
    };
    assert_eq!(text, "second");
    assert_eq!(alpha(*colour), 255);
}

#[test]
fn the_same_text_posted_again_restarts_the_timers() {
    let mut app = app();
    post(&mut app, "same");
    frame(&mut app, HOLD + FADE / 2.0);
    assert_eq!(alpha(lines(&mut app)[0].1), 128);
    post(&mut app, "same");
    frame(&mut app, 0.1);
    assert_eq!(alpha(lines(&mut app)[0].1), 255);
}
