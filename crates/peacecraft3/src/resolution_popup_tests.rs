use super::*;
use bevy::{
    input::{
        mouse::{MouseScrollUnit, MouseWheel},
        touch::TouchPhase,
    },
    input_focus::{FocusCause, InputFocus, InputFocusVisible, tab_navigation::TabIndex},
};

const FDF: &str = r#"
Frame "BACKDROP" "Plate" {
    BackdropBackground "synthetic.tga",
}
Frame "MENU" "Rows" {
    FrameFont "TestFace", 0.01, "",
    FontColor 0.2 0.3 0.4 1,
    MenuTextHighlightColor 0.7 0.8 0.9 1,
    MenuItemHeight 0.03,
    MenuBorder 0.004,
    ControlBackdrop "Plate",
}
Frame "FRAME" "Root" {
    SetAllPoints,
    Frame "POPUPMENU" "ResolutionMenu" {
        Width 0.2,
        Height 0.03,
        SetPoint TOPLEFT, "Root", TOPLEFT, 0.1, -0.1,
        PopupMenuFrame "Rows",
    }
}
"#;

fn open(source: &str, count: u32) -> (World, Result<(), String>) {
    let mut world = World::new();
    let result = open_in(&mut world, source, count);
    (world, result)
}

fn open_in(world: &mut World, source: &str, count: u32) -> Result<(), String> {
    world.init_resource::<InputFocus>();
    world.init_resource::<InputFocusVisible>();
    world.init_resource::<Messages<MouseWheel>>();
    let root = world.spawn(Node::default()).id();
    let cache = MenuArtCache {
        fdf: Some(crate::fdf::Fdf::parse(source).unwrap()),
        fonts: [("TestFace".into(), Handle::default())].into(),
        ..default()
    };
    let navigation = crate::menu_screens::Navigation {
        resolutions: (0..count)
            .map(|n| crate::menu_screens::OfferedResolution::render_size(101 + n, 201 + n))
            .collect(),
        ..default()
    };
    let result = open_resolutions(&mut world.commands(), root, &cache, &navigation);
    world.flush();
    result
}

fn rows(world: &mut World) -> Vec<Entity> {
    let mut rows: Vec<_> = world
        .query::<(Entity, &TabIndex)>()
        .iter(world)
        .map(|(entity, index)| (index.0, entity))
        .collect();
    rows.sort_by_key(|(index, _)| *index);
    rows.into_iter().map(|(_, entity)| entity).collect()
}

fn layout_rows(world: &mut World) {
    let rows = rows(world);
    let scroller = world
        .query_filtered::<Entity, With<ResolutionScroll>>()
        .single(world)
        .unwrap();
    world.entity_mut(scroller).insert((
        ComputedNode {
            size: Vec2::new(200.0, 80.0),
            content_size: Vec2::new(200.0, 240.0),
            inverse_scale_factor: 0.5,
            ..default()
        },
        UiGlobalTransform::from_xy(100.0, 100.0),
    ));
    for (index, row) in rows.iter().enumerate() {
        world.entity_mut(*row).insert((
            ComputedNode {
                size: Vec2::new(200.0, 80.0),
                inverse_scale_factor: 0.5,
                ..default()
            },
            UiGlobalTransform::from_xy(100.0, 100.0 + index as f32 * 80.0),
        ));
    }
}

#[test]
fn resolution_popup_initial_keyboard_focus_scrolls_after_first_layout() {
    for keyboard in [false, true] {
        let mut app = App::new();
        open_in(app.world_mut(), FDF, 3).unwrap();
        let row = rows(app.world_mut())[2];
        app.world_mut().resource_mut::<InputFocusVisible>().0 = keyboard;
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(row, FocusCause::Auto);
        install_resolution_popup_input(&mut app);
        app.add_systems(PostUpdate, layout_rows.in_set(bevy::ui::UiSystems::Layout));
        app.update();
        let world = app.world_mut();
        let scroller = world
            .query_filtered::<Entity, With<ResolutionScroll>>()
            .single(world)
            .unwrap();
        assert_eq!(world.get::<ScrollPosition>(scroller).unwrap().0.y, 80.0);
    }
}

#[test]
fn resolution_popup_keeps_source_border_fixed_around_scrollable_rows() {
    let (mut world, result) = open(FDF, 20);
    result.unwrap();
    let popup = world
        .query_filtered::<Entity, With<ResolutionPopup>>()
        .single(&world)
        .unwrap();
    let scroller = world
        .query_filtered::<Entity, With<ResolutionScroll>>()
        .single(&world)
        .unwrap();
    assert_eq!(world.get::<Node>(popup).unwrap().overflow, Overflow::clip());
    assert_eq!(world.get::<ScrollPosition>(popup).unwrap().0, Vec2::ZERO);
    assert_eq!(world.get::<ChildOf>(scroller).unwrap().parent(), popup);
    let node = world.get::<Node>(scroller).unwrap();
    let inset = Val::Vh(0.004 / crate::mainmenu::FRAME_H * 100.0);
    assert_eq!([node.left, node.right, node.top, node.bottom], [inset; 4]);
    assert_eq!(node.overflow, Overflow::scroll_y());
    let rect = crate::fdf::Fdf::parse(FDF)
        .unwrap()
        .rect(
            "ResolutionMenu",
            (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H),
        )
        .unwrap();
    assert_eq!(
        world.get::<Node>(popup).unwrap().height,
        Val::Vh(rect.bottom / crate::mainmenu::FRAME_H * 100.0)
    );
    // The shared backdrop renderer attaches artwork beside, not inside, the scroll owner.
    assert!(
        world
            .get::<Children>(popup)
            .unwrap()
            .iter()
            .any(|child| child != scroller)
    );
    for (index, row) in rows(&mut world).into_iter().enumerate() {
        assert_eq!(world.get::<ChildOf>(row).unwrap().parent(), scroller);
        assert_eq!(
            world.get::<Name>(row).unwrap().as_str(),
            format!("{} x {}", 101 + index, 201 + index)
        );
        assert_eq!(world.get::<BackgroundColor>(row).unwrap().0, Color::NONE);
    }
}

#[test]
fn resolution_popup_hover_focus_and_wheel_use_authored_rows() {
    let (mut world, result) = open(FDF, 3);
    result.unwrap();
    let system = world.register_system(scroll_resolution_popup);
    let rows = rows(&mut world);
    let scroller = world
        .query_filtered::<Entity, With<ResolutionScroll>>()
        .single(&world)
        .unwrap();
    layout_rows(&mut world);
    world.entity_mut(rows[1]).insert(Hovered(true));
    world.run_system(system).unwrap();
    let colour = |world: &World, row| {
        world
            .get::<TextColor>(world.get::<Children>(row).unwrap()[0])
            .unwrap()
            .0
    };
    assert_eq!(colour(&world, rows[0]), srgba([0.2, 0.3, 0.4, 1.0]));
    assert_eq!(colour(&world, rows[1]), srgba([0.7, 0.8, 0.9, 1.0]));
    world.entity_mut(rows[1]).insert(Hovered(false));
    world.resource_mut::<InputFocusVisible>().0 = true;
    world
        .resource_mut::<InputFocus>()
        .set(rows[2], FocusCause::Navigated);
    world.run_system(system).unwrap();
    assert_eq!(colour(&world, rows[1]), srgba([0.2, 0.3, 0.4, 1.0]));
    assert_eq!(colour(&world, rows[2]), srgba([0.7, 0.8, 0.9, 1.0]));
    assert_eq!(world.get::<ScrollPosition>(scroller).unwrap().0.y, 80.0);
    world.resource_mut::<InputFocusVisible>().0 = false;
    world.entity_mut(rows[0]).insert(Hovered(true));
    world.get_mut::<ScrollPosition>(scroller).unwrap().0.y = 0.0;
    for y in [-1.0, f32::NAN] {
        world.resource_mut::<Messages<MouseWheel>>().clear();
        world.write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y,
            window: Entity::PLACEHOLDER,
            phase: TouchPhase::Moved,
        });
        world.run_system(system).unwrap();
        assert_eq!(world.get::<ScrollPosition>(scroller).unwrap().0.y, 40.0);
    }
    world.resource_mut::<Messages<MouseWheel>>().clear();
    world.get_mut::<ScrollPosition>(scroller).unwrap().0.y = 0.0;
    world.write_message(MouseWheel {
        unit: MouseScrollUnit::Pixel,
        x: 0.0,
        y: -24.0,
        window: Entity::PLACEHOLDER,
        phase: TouchPhase::Moved,
    });
    world.run_system(system).unwrap();
    assert_eq!(world.get::<ScrollPosition>(scroller).unwrap().0.y, 12.0);
    world
        .get_mut::<ComputedNode>(scroller)
        .unwrap()
        .inverse_scale_factor = 0.0;
    world.resource_mut::<InputFocusVisible>().0 = true;
    world
        .resource_mut::<InputFocus>()
        .set(rows[2], FocusCause::Auto);
    world.run_system(system).unwrap();
    assert!(
        world
            .get::<ScrollPosition>(scroller)
            .unwrap()
            .0
            .y
            .is_finite()
    );
}

#[test]
fn resolution_popup_refuses_missing_art_and_invalid_highlight_before_spawning() {
    for source in [
        FDF.replace("ControlBackdrop \"Plate\",", ""),
        FDF.replace("0.7 0.8 0.9 1", "2 0.8 0.9 1"),
    ] {
        let (mut world, result) = open(&source, 3);
        assert!(result.is_err());
        assert!(
            world
                .query_filtered::<Entity, With<ResolutionPopup>>()
                .iter(&world)
                .next()
                .is_none()
        );
    }
}
