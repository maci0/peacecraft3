//! The main menu's control layer, built from the parsed FDF frame tree.
//!
//! Every placement, size, backdrop, edge, highlight, state and label record
//! comes from `UI\FrameDef\Glue\MainMenu.fdf` and the templates it inherits
//! (`StandardTemplates.fdf`). [`control_items`] walks the `ControlLayer` frame
//! and turns each drawable frame into an [`Item`]: a backdrop or a button with
//! its normal, pushed, disabled and mouse-over art. The launcher spawns those as
//! UI nodes positioned in the authored `0.8 × 0.6` frame.
//!
//! `RealmSelect`, the other `SetAllPoints` child of `MainMenuFrame`, is the
//! Battle.net realm chooser (`BNET_REALM_SELECT`) and is not shown on entry.

use crate::fdf::{Fdf, FdfError, Rect};

/// The frame holding the main menu's buttons.
pub const CONTROL_LAYER: &str = "ControlLayer";

/// Authored backdrop flag names, without a native atlas slot or rotation claim.
/// See `docs/re-notes/menu-border-atlas.md`: their pixel mapping is unproved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgePiece {
    Left,
    Right,
    Top,
    Bottom,
    UpperLeft,
    UpperRight,
    BottomLeft,
    BottomRight,
}

impl EdgePiece {
    pub const ALL: [Self; 8] = [
        Self::Left,
        Self::Right,
        Self::Top,
        Self::Bottom,
        Self::UpperLeft,
        Self::UpperRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    /// The `BackdropCornerFlags` token that enables this piece.
    pub(crate) fn flag(self) -> &'static str {
        match self {
            Self::Left => "L",
            Self::Right => "R",
            Self::Top => "T",
            Self::Bottom => "B",
            Self::UpperLeft => "UL",
            Self::UpperRight => "UR",
            Self::BottomLeft => "BL",
            Self::BottomRight => "BR",
        }
    }
}

/// A `BackdropEdgeFile` and the pieces its frame draws.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub file: String,
    /// `BackdropCornerSize`: the corner square and the edge thickness.
    pub corner_size: f32,
    /// The pieces `BackdropCornerFlags` enables; no slot mapping is attached.
    pub pieces: Vec<EdgePiece>,
}

/// A `BACKDROP` frame's art.
#[derive(Debug, Clone, PartialEq)]
pub struct Backdrop {
    pub background: Option<String>,
    /// `BackdropTileBackground`.
    pub tile: bool,
    /// Authored square tile size in frame units; absent is preserved.
    pub background_size: Option<f32>,
    /// Authored `BackdropMirrored`; its native operation remains unproved.
    pub mirrored: bool,
    /// `BackdropBackgroundInsets`, which the shipped menu always writes equal.
    pub inset: Option<f32>,
    pub edge: Option<Edge>,
}

/// A `HIGHLIGHT` frame's `FILETEXTURE` art.
#[derive(Debug, Clone, PartialEq)]
pub struct Highlight {
    pub file: String,
    /// `HighlightAlphaMode "ADD"`; otherwise `"BLEND"`.
    pub additive: bool,
}

/// A button's states and label.
#[derive(Debug, Clone, PartialEq)]
pub struct Button {
    pub normal: Option<Backdrop>,
    pub pushed: Option<Backdrop>,
    pub disabled: Option<Backdrop>,
    pub mouse_over: Option<Highlight>,
    /// The `ButtonText` frame.
    pub text: Option<String>,
    /// `ButtonPushedTextOffset`, frame units, Y up.
    pub pushed_text_offset: Option<[f32; 2]>,
    /// `ControlShortcutKey`.
    pub shortcut: Option<char>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Backdrop(Backdrop),
    Button(Box<Button>),
    Text,
    Widget(crate::menu_screens::Widget),
}

/// One drawable frame and where it sits, in the engine's Y-up frame units.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub frame: String,
    pub rect: Rect,
    pub kind: Kind,
}

/// Apply the client's overrides without changing the authored layout or borders.
pub fn edition_button_art(
    fdf: &Fdf,
    items: &mut [Item],
    art: &pcx_play::menu_edition::ButtonArt,
) -> Result<(), FdfError> {
    let Some(item) = items.iter_mut().find(|item| item.frame == art.frame) else {
        return Err(FdfError::UnknownFrame(art.frame.clone()));
    };
    let Kind::Button(button) = &mut item.kind else {
        return Err(missing(&art.frame, "button"));
    };
    for (property, expected) in [
        "ControlBackdrop",
        "ControlPushedBackdrop",
        "ControlDisabledBackdrop",
    ]
    .into_iter()
    .zip(&art.backdrops)
    {
        if fdf.string(&art.frame, property).as_ref() != Some(expected) {
            return Err(missing(&art.frame, property));
        }
    }
    let states = [&mut button.normal, &mut button.pushed, &mut button.disabled];
    if states
        .iter()
        .any(|state| state.as_ref().is_none_or(|art| art.background.is_none()))
    {
        return Err(missing(&art.frame, "BackdropBackground"));
    }
    for (state, path) in states.into_iter().zip(&art.textures) {
        state.as_mut().expect("checked backdrop").background = Some(path.clone());
    }
    Ok(())
}

fn missing(frame: &str, property: &'static str) -> FdfError {
    FdfError::Missing {
        frame: frame.to_string(),
        property,
    }
}

fn one_number(fdf: &Fdf, frame: &str, key: &'static str) -> Result<Option<f32>, FdfError> {
    match fdf.numbers(frame, key)?.as_deref() {
        None => Ok(None),
        Some([v]) => Ok(Some(*v)),
        Some(_) => Err(missing(frame, key)),
    }
}

/// The art a `BACKDROP` frame declares, or `None` when it draws nothing.
///
/// # Errors
///
/// [`FdfError`] for an edge file without a corner size, an unknown corner flag,
/// or unequal insets, whose written order is not established.
pub fn backdrop(fdf: &Fdf, frame: &str) -> Result<Option<Backdrop>, FdfError> {
    let background = fdf.string(frame, "BackdropBackground");
    let edge = match fdf.string(frame, "BackdropEdgeFile") {
        None => None,
        Some(file) => {
            let corner_size = one_number(fdf, frame, "BackdropCornerSize")?
                .ok_or_else(|| missing(frame, "BackdropCornerSize"))?;
            let flags = fdf
                .string(frame, "BackdropCornerFlags")
                .ok_or_else(|| missing(frame, "BackdropCornerFlags"))?;
            let tokens: Vec<&str> = flags.split('|').map(str::trim).collect();
            if let Some(unknown) = tokens
                .iter()
                .find(|t| !EdgePiece::ALL.iter().any(|p| p.flag() == **t))
            {
                return Err(FdfError::BadNumber {
                    frame: frame.to_string(),
                    property: "BackdropCornerFlags",
                    text: (*unknown).to_string(),
                });
            }
            let pieces = EdgePiece::ALL
                .into_iter()
                .filter(|p| tokens.contains(&p.flag()))
                .collect();
            Some(Edge {
                file,
                corner_size,
                pieces,
            })
        }
    };
    if background.is_none() && edge.is_none() {
        return Ok(None);
    }
    let inset = match fdf.numbers(frame, "BackdropBackgroundInsets")?.as_deref() {
        None => None,
        Some([a, b, c, d]) if a == b && b == c && c == d => Some(*a),
        Some(_) => return Err(missing(frame, "equal BackdropBackgroundInsets")),
    };
    Ok(Some(Backdrop {
        background,
        tile: fdf.flag(frame, "BackdropTileBackground"),
        background_size: one_number(fdf, frame, "BackdropBackgroundSize")?,
        mirrored: fdf.flag(frame, "BackdropMirrored"),
        inset,
        edge,
    }))
}

fn state_backdrop(fdf: &Fdf, button: &str, key: &str) -> Result<Option<Backdrop>, FdfError> {
    match fdf.string(button, key) {
        None => Ok(None),
        Some(name) => {
            if fdf.frame(&name).is_none() {
                return Err(FdfError::UnknownFrame(name));
            }
            backdrop(fdf, &name)
        }
    }
}

/// A button frame's states, label and shortcut.
///
/// # Errors
///
/// [`FdfError`] when a named state frame is missing or malformed.
pub fn button(fdf: &Fdf, frame: &str) -> Result<Button, FdfError> {
    let mouse_over = match fdf.string(frame, "ControlMouseOverHighlight") {
        None => None,
        Some(name) => match fdf.string(&name, "HighlightType").as_deref() {
            Some("FILETEXTURE") => Some(Highlight {
                file: fdf
                    .string(&name, "HighlightAlphaFile")
                    .ok_or_else(|| missing(&name, "HighlightAlphaFile"))?,
                additive: match fdf.string(&name, "HighlightAlphaMode").as_deref() {
                    Some("ADD") => true,
                    Some("BLEND") => false,
                    None => return Err(missing(&name, "HighlightAlphaMode")),
                    Some(mode) => {
                        return Err(FdfError::Unsupported {
                            frame: name.clone(),
                            property: "HighlightAlphaMode",
                            value: mode.into(),
                        });
                    }
                },
            }),
            _ => return Err(missing(&name, "HighlightType \"FILETEXTURE\"")),
        },
    };
    let pushed_text_offset = match fdf.numbers(frame, "ButtonPushedTextOffset")?.as_deref() {
        None => None,
        Some([x, y]) => Some([*x, *y]),
        Some(_) => return Err(missing(frame, "ButtonPushedTextOffset x y")),
    };
    Ok(Button {
        normal: state_backdrop(fdf, frame, "ControlBackdrop")?,
        pushed: state_backdrop(fdf, frame, "ControlPushedBackdrop")?,
        disabled: state_backdrop(fdf, frame, "ControlDisabledBackdrop")?,
        mouse_over,
        text: fdf.button_text(frame),
        pushed_text_offset,
        shortcut: fdf
            .string(frame, "ControlShortcutKey")
            .and_then(|k| k.chars().next()),
    })
}

/// Whether a frame type is a clickable control.
fn is_button(kind: &str) -> bool {
    matches!(
        kind,
        "GLUETEXTBUTTON" | "GLUEBUTTON" | "TEXTBUTTON" | "BUTTON"
    )
}

/// Every drawable frame under `root`, parents before children, in file order,
/// so later items draw over earlier ones.
///
/// # Errors
///
/// [`FdfError`] when `root` is missing or a frame's rectangle or art cannot be
/// resolved. Frame types this renderer does not draw are returned by name in
/// the second list rather than skipped silently.
pub fn control_items(
    fdf: &Fdf,
    root: &str,
    viewport: (f32, f32),
) -> Result<(Vec<Item>, Vec<String>), FdfError> {
    control_items_for(fdf, root, viewport, |_| Ok(true))
}

pub fn control_items_for(
    fdf: &Fdf,
    root: &str,
    viewport: (f32, f32),
    include: impl Fn(&str) -> Result<bool, FdfError>,
) -> Result<(Vec<Item>, Vec<String>), FdfError> {
    let mut items = Vec::new();
    let mut unsupported = Vec::new();
    let frame = fdf
        .frame(root)
        .ok_or_else(|| FdfError::UnknownFrame(root.to_string()))?;
    let mut pending: Vec<String> = frame.children.iter().rev().cloned().collect();
    let mut remaining = fdf.frames().len();
    while let Some(name) = pending.pop() {
        if remaining == 0 {
            return Err(FdfError::Unsupported {
                frame: name,
                property: "frame traversal",
                value: "repeated child expansion exceeds the parsed frame count".into(),
            });
        }
        remaining -= 1;
        if !include(&name)? {
            continue;
        }
        let frame = fdf
            .frame(&name)
            .ok_or_else(|| FdfError::UnknownFrame(name.clone()))?;
        let kind = frame.kind.as_str();
        if is_button(kind) {
            items.push(Item {
                rect: fdf.rect(&name, viewport)?,
                kind: Kind::Button(Box::new(button(fdf, &name)?)),
                frame: name,
            });
            continue;
        }
        match kind {
            "BACKDROP" => {
                if let Some(art) = backdrop(fdf, &name)? {
                    items.push(Item {
                        rect: fdf.rect(&name, viewport)?,
                        kind: Kind::Backdrop(art),
                        frame: name.clone(),
                    });
                }
            }
            "FRAME" => {}
            "TEXT" => {
                if fdf.text_value(&name).is_some() {
                    items.push(Item {
                        rect: fdf.rect(&name, viewport)?,
                        kind: Kind::Text,
                        frame: name.clone(),
                    });
                }
            }
            "SLIDER" | "CHECKBOX" | "GLUECHECKBOX" | "POPUPMENU" | "GLUEPOPUPMENU" => {
                if let Some(widget) = crate::menu_screens::widget(fdf, &name)? {
                    items.push(Item {
                        rect: fdf.rect(&name, viewport)?,
                        kind: Kind::Widget(widget),
                        frame: name.clone(),
                    });
                    continue;
                }
            }
            _ => unsupported.push(format!("{kind} {name}")),
        }
        pending.extend(frame.children.iter().rev().cloned());
    }
    Ok((items, unsupported))
}

// ---------------------------------------------------------------------------
// Bevy UI.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor,
};
use bevy::shader::ShaderRef;
use bevy::ui::widget::{ImageNode, NodeImageMode};
use bevy::ui::{InteractionDisabled, Pressed};
use bevy::ui_widgets::Button as UiButton;

/// The authored frame the menu is laid out in (`docs/re-notes/glue-coordinate-runtime.md`).
const VIEWPORT: (f32, f32) = (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H);

const ADDITIVE_SHADER: &str = "embedded://peacecraft3/glue_additive.wgsl";

pub fn install(app: &mut App) {
    bevy::asset::embedded_asset!(app, "glue_additive.wgsl");
    bevy::asset::embedded_asset!(app, "glue_backdrop.wgsl");
    app.add_plugins((
        UiMaterialPlugin::<AdditiveImage>::default(),
        UiMaterialPlugin::<BackdropImage>::default(),
    ))
    .init_resource::<GlueImages>()
    .add_systems(PreUpdate, shortcut_keys.after(bevy::input::InputSystems))
    .add_systems(
        Update,
        (load_art, show_states, place_shadows, update_setting_art),
    );
}

/// `HighlightAlphaMode "ADD"`: the texture is added to what is under it.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct AdditiveImage {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
}

impl UiMaterial for AdditiveImage {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(ADDITIVE_SHADER.into())
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _key: bevy::ui_render::prelude::UiMaterialKey<Self>,
    ) {
        let add = BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        if let Some(target) = descriptor
            .fragment
            .as_mut()
            .and_then(|f| f.targets.first_mut())
            .and_then(Option::as_mut)
        {
            target.blend = Some(BlendState {
                color: add,
                alpha: add,
            });
        }
    }
}

/// Repeated backdrop art. Repeat counts derive from its laid-out frame rectangle
/// divided by the authored tile size, so resizing changes physical tile dimensions
/// with the UI while preserving authored repeats independently on each axis.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct BackdropImage {
    #[texture(0)]
    #[sampler(1)]
    pub(crate) texture: Handle<Image>,
    #[uniform(2)]
    pub(crate) uv_transform: Vec4,
    #[uniform(3)]
    pub(crate) uv_rows: Vec4,
}

impl UiMaterial for BackdropImage {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("embedded://peacecraft3/glue_backdrop.wgsl".into())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) fn composition_test_nodes(
    world: &mut World,
    camera: Entity,
    texture: Handle<Image>,
    width: f32,
    top: f32,
    height: f32,
) {
    let backdrop = world
        .resource_mut::<Assets<BackdropImage>>()
        .add(BackdropImage {
            texture: texture.clone(),
            uv_transform: Vec4::new(1.0, 1.0, 0.0, 0.0),
            uv_rows: Vec4::new(1.0, 0.0, 0.0, 1.0),
        });
    let additive = world
        .resource_mut::<Assets<AdditiveImage>>()
        .add(AdditiveImage { texture });
    let node = |top| Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(top),
        width: Val::Px(width),
        height: Val::Px(height),
        ..default()
    };
    world.spawn((node(top), UiTargetCamera(camera), MaterialNode(backdrop)));
    world.spawn((
        node(top + height),
        UiTargetCamera(camera),
        MaterialNode(additive),
    ));
}

#[derive(Component)]
struct BackdropRepeat(Vec2);

/// A clickable FDF frame. Clicks are dispatched by [`GlueButton::frame`].
#[derive(Component, Debug)]
pub struct GlueButton {
    pub frame: String,
    pub shortcut: Option<char>,
    /// `ButtonPushedTextOffset` as a percentage of the button's width and
    /// height, Y down.
    pub pushed_text_offset: Option<Vec2>,
}

/// Which button state a layer draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    Pushed,
    Disabled,
    MouseOver,
}

#[derive(Component, Debug)]
struct StateLayer {
    button: Entity,
    state: State,
    /// Whether the button has its own art for the state this layer stands in
    /// for; a button without pushed art keeps its normal art while pressed.
    has: [bool; 3],
}

/// The label of a button, drawn from its `ButtonText` frame.
#[derive(Component, Debug)]
pub struct GlueLabel {
    pub frame: String,
    pub button: Entity,
}

/// A text shadow whose `FontShadowOffset` is in frame units.
#[derive(Component, Debug)]
pub struct FrameShadow(pub Vec2);

/// An image still waiting for its art.
#[derive(Component, Debug, Clone)]
struct PendingArt {
    path: String,
    piece: Option<EdgePiece>,
    mirrored: bool,
    tile: bool,
    additive: bool,
}

/// Decoded art by path and sampler policy. Every edge piece shares the strip
/// image; its source rectangle belongs to the ImageNode, not another decode.
#[derive(Resource, Default)]
pub struct GlueImages {
    images: std::collections::HashMap<(String, bool), Handle<Image>>,
    /// Reported once; a browser keeps retrying because its store fills late.
    failed: std::collections::HashSet<String>,
    pub install: Option<std::path::PathBuf>,
}

impl GlueImages {
    /// Drops everything decoded, for a new install or asset store.
    pub fn reset(&mut self, install: Option<std::path::PathBuf>) {
        self.images.clear();
        self.failed.clear();
        self.install = install;
    }
}

/// A node at `rect` inside a parent at `parent`, both in frame units.
fn placed(rect: &Rect, parent: &Rect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent((rect.left - parent.left) / parent.width * 100.0),
        top: Val::Percent((parent.top() - rect.top()) / parent.height * 100.0),
        width: Val::Percent(rect.width / parent.width * 100.0),
        height: Val::Percent(rect.height / parent.height * 100.0),
        ..default()
    }
}

fn fill() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

/// Spawns a backdrop's background and edge pieces as children of `parent`,
/// whose size in frame units is `size`.
fn spawn_backdrop(commands: &mut Commands, parent: Entity, art: &Backdrop, size: Vec2) {
    if !size.is_finite() || size.cmple(Vec2::ZERO).any() {
        warn!("Menu backdrop has invalid frame size {size:?}");
        return;
    }
    let local = Rect {
        left: 0.0,
        bottom: 0.0,
        width: size.x,
        height: size.y,
    };
    if let Some(path) = &art.background {
        // Renderer convention: an omitted inset leaves the entire frame available.
        let inset = art.inset.unwrap_or(0.0);
        let inner = Rect {
            left: inset,
            bottom: inset,
            width: size.x - 2.0 * inset,
            height: size.y - 2.0 * inset,
        };
        if !inset.is_finite() || inset < 0.0 || inner.width <= 0.0 || inner.height <= 0.0 {
            warn!("Menu backdrop {path}: invalid background inset {inset}");
        } else if art
            .background_size
            .is_some_and(|v| !v.is_finite() || v <= 0.0)
        {
            warn!("Menu backdrop {path}: invalid BackdropBackgroundSize");
        } else {
            let child = commands
                .spawn((
                    placed(&inner, &local),
                    PendingArt {
                        path: path.clone(),
                        piece: None,
                        mirrored: art.mirrored,
                        tile: art.tile,
                        additive: false,
                    },
                    Pickable::IGNORE,
                ))
                .id();
            if art.tile
                && let Some(tile_size) = art.background_size
            {
                let repeats = Vec2::new(inner.width, inner.height) / tile_size;
                if repeats.is_finite() && repeats.cmpgt(Vec2::ZERO).all() {
                    commands.entity(child).insert(BackdropRepeat(repeats));
                }
            }
            commands.entity(parent).add_child(child);
        }
    }
    if let Some(edge) = &art.edge {
        if !edge.corner_size.is_finite() || edge.corner_size <= 0.0 {
            warn!("Menu edge {}: invalid BackdropCornerSize", edge.file);
            return;
        }
        for &piece in &edge.pieces {
            let mut rect = edge_rect(piece, size, edge.corner_size);
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }
            if art.mirrored {
                rect.left = size.x - rect.left - rect.width;
            }
            let child = commands
                .spawn((
                    placed(&rect, &local),
                    PendingArt {
                        path: edge.file.clone(),
                        piece: Some(piece),
                        mirrored: art.mirrored,
                        tile: false,
                        additive: false,
                    },
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(parent).add_child(child);
        }
    }
}

/// Conventional nine-slice placement; corners shrink together to fit small frames.
pub(crate) fn edge_rect(piece: EdgePiece, size: Vec2, corner: f32) -> Rect {
    let c = corner.min(size.x * 0.5).min(size.y * 0.5);
    let (left, bottom, width, height) = match piece {
        EdgePiece::Left => (0.0, c, c, size.y - 2.0 * c),
        EdgePiece::Right => (size.x - c, c, c, size.y - 2.0 * c),
        EdgePiece::Top => (c, size.y - c, size.x - 2.0 * c, c),
        EdgePiece::Bottom => (c, 0.0, size.x - 2.0 * c, c),
        EdgePiece::UpperLeft => (0.0, size.y - c, c, c),
        EdgePiece::UpperRight => (size.x - c, size.y - c, c, c),
        EdgePiece::BottomLeft => (0.0, 0.0, c, c),
        EdgePiece::BottomRight => (size.x - c, 0.0, c, c),
    };
    Rect {
        left,
        bottom,
        width,
        height,
    }
}

#[derive(Component)]
struct SettingThumbArt {
    owner: Entity,
    control: Vec2,
    thumb: Vec2,
    vertical: bool,
}

#[derive(Component)]
struct SettingCheckArt {
    owner: Entity,
    disabled: bool,
    has_disabled: bool,
}

fn setting_size(fdf: &Fdf, frame: &str) -> Result<Vec2, FdfError> {
    let width = one_number(fdf, frame, "Width")?.ok_or_else(|| missing(frame, "Width"))?;
    let height = one_number(fdf, frame, "Height")?.ok_or_else(|| missing(frame, "Height"))?;
    let size = Vec2::new(width, height);
    if !size.is_finite() || size.cmple(Vec2::ZERO).any() {
        return Err(FdfError::Unsupported {
            frame: frame.into(),
            property: "control art size",
            value: format!("{size:?}"),
        });
    }
    Ok(size)
}

fn decorate_control_backdrops(
    commands: &mut Commands,
    owner: Entity,
    parent: Entity,
    fdf: &Fdf,
    frame: &str,
    size: Vec2,
) -> Result<(), FdfError> {
    let normal = state_backdrop(fdf, frame, "ControlBackdrop")?;
    let pushed = state_backdrop(fdf, frame, "ControlPushedBackdrop")?;
    let disabled = state_backdrop(fdf, frame, "ControlDisabledBackdrop")?;
    let has = [normal.is_some(), pushed.is_some(), disabled.is_some()];
    for (state, art) in [
        (State::Normal, normal),
        (State::Pushed, pushed),
        (State::Disabled, disabled),
    ] {
        if let Some(art) = art {
            let layer = commands
                .spawn((
                    fill(),
                    StateLayer {
                        button: owner,
                        state,
                        has,
                    },
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(parent).add_child(layer);
            spawn_backdrop(commands, layer, &art, size);
        }
    }
    Ok(())
}

fn setting_highlight(fdf: &Fdf, frame: &str, key: &str) -> Result<Option<Highlight>, FdfError> {
    let Some(name) = fdf.string(frame, key) else {
        return Ok(None);
    };
    if fdf.frame(&name).is_none() {
        return Err(FdfError::UnknownFrame(name));
    }
    if fdf.string(&name, "HighlightType").as_deref() != Some("FILETEXTURE") {
        return Err(missing(&name, "HighlightType FILETEXTURE"));
    }
    let file = fdf
        .string(&name, "HighlightAlphaFile")
        .ok_or_else(|| missing(&name, "HighlightAlphaFile"))?;
    let additive = match fdf.string(&name, "HighlightAlphaMode").as_deref() {
        Some("ADD") => true,
        Some("BLEND") => false,
        None => return Err(missing(&name, "HighlightAlphaMode")),
        Some(value) => {
            return Err(FdfError::Unsupported {
                frame: name,
                property: "HighlightAlphaMode",
                value: value.into(),
            });
        }
    };
    Ok(Some(Highlight { file, additive }))
}

/// Attach authored backdrop/checkbox/thumb/popup-arrow art to an existing setting.
/// Call once per rebuilt control. Geometry conventions are documented in
/// `docs/re-notes/menu-reconstruction-renderer.md`; values remain with the widget.
pub fn decorate_setting(
    commands: &mut Commands,
    node: Entity,
    fdf: &Fdf,
    frame: &str,
    size: Vec2,
) -> Result<(), FdfError> {
    if !size.is_finite() || size.cmple(Vec2::ZERO).any() {
        return Err(FdfError::Unsupported {
            frame: frame.into(),
            property: "control art size",
            value: format!("{size:?}"),
        });
    }
    decorate_control_backdrops(commands, node, node, fdf, frame, size)?;
    let kind = fdf
        .frame(frame)
        .ok_or_else(|| FdfError::UnknownFrame(frame.into()))?
        .kind
        .as_str();
    match kind {
        "SLIDER" => {
            if let Some(thumb_frame) = fdf.string(frame, "SliderThumbButtonFrame") {
                let thumb = setting_size(fdf, &thumb_frame)?;
                let horizontal = fdf.flag(frame, "SliderLayoutHorizontal");
                let vertical = fdf.flag(frame, "SliderLayoutVertical");
                if horizontal == vertical {
                    return Err(FdfError::Unsupported {
                        frame: frame.into(),
                        property: "slider art orientation",
                        value: "requires one authored layout flag".into(),
                    });
                }
                if if vertical {
                    thumb.y > size.y
                } else {
                    thumb.x > size.x
                } {
                    return Err(FdfError::Unsupported {
                        frame: frame.into(),
                        property: "SliderThumbButtonFrame",
                        value: "thumb exceeds slider travel axis".into(),
                    });
                }
                let min = one_number(fdf, frame, "SliderMinValue")?
                    .ok_or_else(|| missing(frame, "SliderMinValue"))?;
                let max = one_number(fdf, frame, "SliderMaxValue")?
                    .ok_or_else(|| missing(frame, "SliderMaxValue"))?;
                let value = one_number(fdf, frame, "SliderInitialValue")?
                    .ok_or_else(|| missing(frame, "SliderInitialValue"))?;
                if !min.is_finite()
                    || !max.is_finite()
                    || !value.is_finite()
                    || min >= max
                    || !(min..=max).contains(&value)
                {
                    return Err(FdfError::Unsupported {
                        frame: frame.into(),
                        property: "slider art range",
                        value: format!("{min}..{max}, initial {value}"),
                    });
                }
                let art = SettingThumbArt {
                    owner: node,
                    control: size,
                    thumb,
                    vertical,
                };
                let mut rect = Rect {
                    left: 0.0,
                    bottom: 0.0,
                    width: thumb.x,
                    height: thumb.y,
                };
                set_thumb_rect(
                    &mut rect,
                    &art,
                    ((f64::from(value) - f64::from(min)) / (f64::from(max) - f64::from(min)))
                        as f32,
                );
                let child = commands
                    .spawn((
                        placed(
                            &rect,
                            &Rect {
                                left: 0.0,
                                bottom: 0.0,
                                width: size.x,
                                height: size.y,
                            },
                        ),
                        art,
                        if crate::menu_screens::supported(frame) {
                            Visibility::Inherited
                        } else {
                            Visibility::Hidden
                        },
                        bevy::ui_widgets::SliderThumb,
                        Pickable::IGNORE,
                    ))
                    .id();
                commands.entity(node).add_child(child);
                decorate_control_backdrops(commands, node, child, fdf, &thumb_frame, thumb)?;
            }
        }
        "CHECKBOX" | "GLUECHECKBOX" => {
            let checked = setting_highlight(fdf, frame, "CheckBoxCheckHighlight")?;
            let disabled = setting_highlight(fdf, frame, "CheckBoxDisabledCheckHighlight")?;
            let has_disabled = disabled.is_some();
            for (disabled, art) in [(false, checked), (true, disabled)] {
                if let Some(art) = art {
                    let child = commands
                        .spawn((
                            fill(),
                            SettingCheckArt {
                                owner: node,
                                disabled,
                                has_disabled,
                            },
                            PendingArt {
                                path: art.file,
                                piece: None,
                                mirrored: false,
                                tile: false,
                                additive: art.additive,
                            },
                            Visibility::Hidden,
                            Pickable::IGNORE,
                        ))
                        .id();
                    commands.entity(node).add_child(child);
                }
            }
        }
        "POPUPMENU" | "GLUEPOPUPMENU" => {
            if let Some(arrow_frame) = fdf.string(frame, "PopupArrowFrame") {
                let arrow = setting_size(fdf, &arrow_frame)?;
                if arrow.cmpgt(size).any() {
                    return Err(FdfError::Unsupported {
                        frame: frame.into(),
                        property: "PopupArrowFrame",
                        value: "arrow exceeds control rectangle".into(),
                    });
                }
                let rect = Rect {
                    left: size.x - arrow.x,
                    bottom: (size.y - arrow.y) * 0.5,
                    width: arrow.x,
                    height: arrow.y,
                };
                let child = commands
                    .spawn((
                        placed(
                            &rect,
                            &Rect {
                                left: 0.0,
                                bottom: 0.0,
                                width: size.x,
                                height: size.y,
                            },
                        ),
                        Pickable::IGNORE,
                    ))
                    .id();
                commands.entity(node).add_child(child);
                decorate_control_backdrops(commands, node, child, fdf, &arrow_frame, arrow)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn set_thumb_rect(rect: &mut Rect, art: &SettingThumbArt, fraction: f32) {
    if art.vertical {
        rect.left = (art.control.x - art.thumb.x) * 0.5;
        rect.bottom = fraction.clamp(0.0, 1.0) * (art.control.y - art.thumb.y);
    } else {
        rect.left = fraction.clamp(0.0, 1.0) * (art.control.x - art.thumb.x);
        rect.bottom = (art.control.y - art.thumb.y) * 0.5;
    }
}

fn update_setting_art(
    values: Query<(
        &bevy::ui_widgets::SliderValue,
        &bevy::ui_widgets::SliderRange,
    )>,
    checked: Query<(Has<bevy::ui::Checked>, Has<InteractionDisabled>)>,
    mut thumbs: Query<(&SettingThumbArt, &mut Node, &mut Visibility), Without<SettingCheckArt>>,
    previews: Query<(), With<crate::menu_settings::preview::Control>>,
    mut checks: Query<(&SettingCheckArt, &mut Visibility)>,
) {
    for (art, mut node, mut visibility) in &mut thumbs {
        let Ok((value, range)) = values.get(art.owner) else {
            if previews.contains(art.owner) {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        if previews.contains(art.owner) {
            *visibility = Visibility::Inherited;
        }
        if !value.0.is_finite()
            || !range.start().is_finite()
            || !range.end().is_finite()
            || range.start() >= range.end()
        {
            continue;
        }
        let mut rect = Rect {
            left: 0.0,
            bottom: 0.0,
            width: art.thumb.x,
            height: art.thumb.y,
        };
        set_thumb_rect(
            &mut rect,
            art,
            ((f64::from(value.0) - f64::from(range.start()))
                / (f64::from(range.end()) - f64::from(range.start()))) as f32,
        );
        let want = placed(
            &rect,
            &Rect {
                left: 0.0,
                bottom: 0.0,
                width: art.control.x,
                height: art.control.y,
            },
        );
        if node.left != want.left || node.top != want.top {
            node.left = want.left;
            node.top = want.top;
        }
    }
    for (art, mut visibility) in &mut checks {
        let Ok((checked, disabled)) = checked.get(art.owner) else {
            continue;
        };
        let shown = checked
            && if art.disabled {
                disabled
            } else {
                !disabled || !art.has_disabled
            };
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
    }
}

/// Spawns `items` under `root`, which covers the authored frame.
pub fn spawn(commands: &mut Commands, root: Entity, items: &[Item]) {
    let frame = Rect {
        left: 0.0,
        bottom: 0.0,
        width: VIEWPORT.0,
        height: VIEWPORT.1,
    };
    for item in items {
        let size = Vec2::new(item.rect.width, item.rect.height);
        let node = commands
            .spawn((
                placed(&item.rect, &frame),
                Name::new(item.frame.clone()),
                crate::menu_screens::ScreenPosition(item.frame.clone()),
            ))
            .id();
        commands.entity(root).add_child(node);
        match &item.kind {
            Kind::Text => {
                commands.entity(node).insert((
                    Pickable::IGNORE,
                    Node {
                        flex_direction: FlexDirection::Column,
                        ..placed(&item.rect, &frame)
                    },
                ));
                let label = commands
                    .spawn((
                        Text::new(""),
                        GlueLabel {
                            frame: item.frame.clone(),
                            button: node,
                        },
                        Pickable::IGNORE,
                    ))
                    .id();
                commands.entity(node).add_child(label);
            }
            Kind::Widget(_) => {}
            Kind::Backdrop(art) => {
                commands.entity(node).insert(Pickable::IGNORE);
                spawn_backdrop(commands, node, art, size);
            }
            Kind::Button(button) => {
                commands.entity(node).insert((
                    UiButton,
                    Hovered::default(),
                    GlueButton {
                        frame: item.frame.clone(),
                        shortcut: button.shortcut,
                        pushed_text_offset: button.pushed_text_offset.map(|offset| {
                            Vec2::new(offset[0] / size.x * 100.0, -offset[1] / size.y * 100.0)
                        }),
                    },
                ));
                if button.text.is_some() && button.pushed_text_offset.is_none() {
                    warn!(
                        "Menu label {}: absent ButtonPushedTextOffset has no verified default; pressed-label displacement omitted",
                        item.frame
                    );
                }
                let has = [
                    button.normal.is_some(),
                    button.pushed.is_some(),
                    button.disabled.is_some(),
                ];
                for (state, art) in [
                    (State::Normal, &button.normal),
                    (State::Pushed, &button.pushed),
                    (State::Disabled, &button.disabled),
                ] {
                    let Some(art) = art else { continue };
                    let layer = commands
                        .spawn((
                            fill(),
                            StateLayer {
                                button: node,
                                state,
                                has,
                            },
                            Pickable::IGNORE,
                        ))
                        .id();
                    commands.entity(node).add_child(layer);
                    spawn_backdrop(commands, layer, art, size);
                }
                if let Some(glow) = &button.mouse_over {
                    let layer = commands
                        .spawn((
                            fill(),
                            StateLayer {
                                button: node,
                                state: State::MouseOver,
                                has,
                            },
                            PendingArt {
                                path: glow.file.clone(),
                                piece: None,
                                mirrored: false,
                                tile: false,
                                additive: glow.additive,
                            },
                            Visibility::Hidden,
                            Pickable::IGNORE,
                        ))
                        .id();
                    commands.entity(node).add_child(layer);
                }
                if let Some(text) = &button.text {
                    let label = commands
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..fill()
                            },
                            Pickable::IGNORE,
                        ))
                        .with_child((
                            Text::new(""),
                            GlueLabel {
                                frame: text.clone(),
                                button: node,
                            },
                            Pickable::IGNORE,
                        ))
                        .id();
                    commands.entity(node).add_child(label);
                }
            }
        }
    }
}

/// Attaches interactive setting widgets after their asset-derived rectangles
/// have been spawned. The FDF remains the source of ranges and styles.
/// The bytes of one art path: the install, else the browser's store.
fn art_bytes(install: Option<&std::path::Path>, path: &str) -> Option<Vec<u8>> {
    crate::mainmenu::art_bytes(install, path).or_else(|| crate::mainmenu::from_served(path))
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ArtError {
    #[error("{width}x{height}: expected a horizontal eight-square edge strip")]
    UnsupportedEdgeShape { width: u32, height: u32 },
}

/// Implementation policy, not a claim about native atlas internals: slots are
/// L,R,T,B,UL,UR,BL,BR; horizontal edge sampling rotates clockwise.
pub(crate) fn edge_source_rect(
    piece: EdgePiece,
    width: u32,
    height: u32,
) -> Result<bevy::math::Rect, ArtError> {
    if height == 0 || height.checked_mul(8) != Some(width) {
        return Err(ArtError::UnsupportedEdgeShape { width, height });
    }
    let slot = match piece {
        EdgePiece::Left => 0,
        EdgePiece::Right => 1,
        EdgePiece::Top => 2,
        EdgePiece::Bottom => 3,
        EdgePiece::UpperLeft => 4,
        EdgePiece::UpperRight => 5,
        EdgePiece::BottomLeft => 6,
        EdgePiece::BottomRight => 7,
    };
    let x = (slot * height) as f32;
    Ok(bevy::math::Rect::new(
        x,
        0.0,
        x + height as f32,
        height as f32,
    ))
}

/// Horizontal edge slots are stored vertically; rotate their sampling clockwise.
/// This is a reconstruction convention supported by the source strip and corners.
pub(crate) fn horizontal_edge_material(
    texture: Handle<Image>,
    rect: bevy::math::Rect,
    image_size: Vec2,
    mirrored: bool,
) -> BackdropImage {
    let scale = rect.size() / image_size;
    let origin = rect.min / image_size;
    BackdropImage {
        texture,
        uv_rows: Vec4::new(0.0, 1.0, if mirrored { 1.0 } else { -1.0 }, 0.0),
        uv_transform: Vec4::new(
            scale.x,
            scale.y,
            origin.x,
            origin.y + if mirrored { 0.0 } else { scale.y },
        ),
    }
}

/// Whether every texel of an RGBA8 buffer is the same colour.
fn uniform(rgba: &[u8]) -> bool {
    match rgba.get(..4) {
        Some(first) => {
            let (pixels, remainder) = rgba.as_chunks::<4>();
            remainder.is_empty() && pixels.iter().all(|p| p == first)
        }
        None => false,
    }
}

/// Decodes pending art and puts it on its node.
fn load_art(
    mut commands: Commands,
    mut cache: ResMut<GlueImages>,
    mut images: ResMut<Assets<Image>>,
    mut additive: ResMut<Assets<AdditiveImage>>,
    mut backgrounds: ResMut<Assets<BackdropImage>>,
    pending: Query<(Entity, &PendingArt, Option<&BackdropRepeat>)>,
) {
    for (entity, art, repeats) in &pending {
        let key = (art.path.clone(), art.tile);
        let handle = match cache.images.get(&key) {
            Some(handle) => handle.clone(),
            None => {
                let decoded = art_bytes(cache.install.as_deref(), &art.path)
                    .ok_or_else(|| format!("{} is not readable", art.path))
                    .and_then(|bytes| {
                        pcx_gfx::load_image(&bytes, &art.path).map_err(|e| e.to_string())
                    });
                match decoded {
                    Ok(texture) => {
                        let mut image = crate::menu_composition::image(&texture);
                        if art.tile {
                            image.sampler = bevy::image::ImageSampler::Descriptor(
                                bevy::image::ImageSamplerDescriptor {
                                    address_mode_u: bevy::image::ImageAddressMode::Repeat,
                                    address_mode_v: bevy::image::ImageAddressMode::Repeat,
                                    ..default()
                                },
                            );
                        }
                        let handle = images.add(image);
                        cache.images.insert(key, handle.clone());
                        handle
                    }
                    Err(error) => {
                        if cache.failed.insert(art.path.clone()) {
                            warn!("Menu art: {error}");
                        }
                        if cfg!(not(target_arch = "wasm32")) {
                            commands.entity(entity).remove::<PendingArt>();
                        }
                        continue;
                    }
                }
            }
        };
        let Some(image_asset) = images.get(&handle) else {
            continue;
        };
        let rect = match art.piece {
            Some(piece) => match edge_source_rect(piece, image_asset.width(), image_asset.height())
            {
                Ok(rect) => Some(rect),
                Err(error) => {
                    if cache.failed.insert(art.path.clone()) {
                        warn!("Menu art {}: {error}", art.path);
                    }
                    commands.entity(entity).remove::<PendingArt>();
                    continue;
                }
            },
            None => None,
        };
        let patterned = !image_asset.data.as_deref().is_some_and(uniform);
        let mut entity = commands.entity(entity);
        entity.remove::<PendingArt>();
        if art.additive {
            entity.insert(MaterialNode(
                additive.add(AdditiveImage { texture: handle }),
            ));
        } else if matches!(art.piece, Some(EdgePiece::Top | EdgePiece::Bottom)) {
            let source = rect.expect("checked source rectangle for an edge piece");
            let image_size = Vec2::new(image_asset.width() as f32, image_asset.height() as f32);
            entity.insert(MaterialNode(backgrounds.add(horizontal_edge_material(
                handle,
                source,
                image_size,
                art.mirrored,
            ))));
        } else if art.tile
            && patterned
            && let Some(repeats) = repeats
        {
            let r = repeats.0;
            let transform = if art.mirrored {
                Vec4::new(-r.x, r.y, r.x, 0.0)
            } else {
                Vec4::new(r.x, r.y, 0.0, 0.0)
            };
            entity.insert(MaterialNode(backgrounds.add(BackdropImage {
                texture: handle,
                uv_transform: transform,
                uv_rows: Vec4::new(1.0, 0.0, 0.0, 1.0),
            })));
        } else {
            let mut image = ImageNode::new(handle).with_mode(NodeImageMode::Stretch);
            image.rect = rect;
            if art.tile && patterned {
                // With no authored tile size, retain intrinsic image dimensions
                // in logical UI pixels as an explicit reconstruction convention.
                image.image_mode = NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: 1.0,
                };
            }
            image.flip_x = art.mirrored;
            entity.insert(image);
        }
    }
}

/// Shows each button's normal, pushed, disabled and mouse-over art for its
/// state, and moves its label by `ButtonPushedTextOffset` while pressed.
fn show_states(
    buttons: Query<(
        &GlueButton,
        &Hovered,
        Has<Pressed>,
        Has<InteractionDisabled>,
    )>,
    mut layers: Query<(&StateLayer, &mut Visibility)>,
    mut labels: Query<(&GlueLabel, &ChildOf)>,
    mut nodes: Query<&mut Node>,
    focus: Option<Res<bevy::input_focus::InputFocus>>,
    focus_visible: Option<Res<bevy::input_focus::InputFocusVisible>>,
) {
    for (layer, mut visibility) in &mut layers {
        let Ok((_, hovered, pressed, disabled)) = buttons.get(layer.button) else {
            continue;
        };
        let [_, has_pushed, has_disabled] = layer.has;
        let shown = match layer.state {
            State::MouseOver => {
                (hovered.get()
                    || (focus_visible.as_ref().is_some_and(|v| v.0)
                        && focus
                            .as_ref()
                            .is_some_and(|f| f.get() == Some(layer.button))))
                    && !disabled
            }
            State::Disabled => disabled,
            State::Pushed => pressed && !disabled,
            State::Normal => !(disabled && has_disabled) && !(pressed && has_pushed && !disabled),
        };
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
    }
    for (label, parent) in &mut labels {
        let Ok((button, _, pressed, disabled)) = buttons.get(label.button) else {
            continue;
        };
        let Ok(mut node) = nodes.get_mut(parent.parent()) else {
            continue;
        };
        let offset = if pressed && !disabled {
            let Some(offset) = button.pushed_text_offset else {
                continue;
            };
            offset
        } else {
            Vec2::ZERO
        };
        let (left, top) = (Val::Percent(offset.x), Val::Percent(offset.y));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    }
}

/// `ControlShortcutKey`: pressing the key activates its enabled button, as a
/// click does. Runs before `Update`, so every action consumer sees it.
pub(crate) fn shortcut_keys(
    mut keys: MessageReader<bevy::input::keyboard::KeyboardInput>,
    buttons: Query<(Entity, &GlueButton), Without<InteractionDisabled>>,
    mut actions: ResMut<crate::button_actions::ButtonActions>,
    presentation: Option<Res<peacecraft3::campaign_presentation::CampaignPresentation>>,
) {
    if !crate::button_actions::input_allowed(presentation) {
        keys.clear();
        return;
    }
    for key in keys.read() {
        let bevy::input::keyboard::Key::Character(text) = &key.logical_key else {
            continue;
        };
        if !key.state.is_pressed() || key.repeat {
            continue;
        }
        for (entity, button) in &buttons {
            let hit = button.shortcut.is_some_and(|c| {
                text.chars().eq(c.to_lowercase()) || text.chars().eq(c.to_uppercase())
            });
            if hit && !actions.entities.contains(&entity) {
                actions.entities.push(entity);
            }
        }
    }
}

/// Converts each label's `FontShadowOffset` from frame units to the logical
/// pixels `TextShadow` takes, from the UI camera's viewport height.
fn place_shadows(
    camera: Query<&Camera, With<bevy::ui::IsDefaultUiCamera>>,
    mut shadows: Query<(&FrameShadow, &mut TextShadow)>,
) {
    let Some(height) = camera
        .single()
        .ok()
        .and_then(Camera::logical_viewport_size)
        .map(|s| s.y)
    else {
        return;
    };
    let per_unit = height / VIEWPORT.1;
    for (frame, mut shadow) in &mut shadows {
        // Frame Y is up; UI Y is down.
        let offset = Vec2::new(frame.0.x, -frame.0.y) * per_unit;
        if shadow.offset != offset {
            shadow.offset = offset;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edition_art_changes_all_states_and_preserves_source_chrome() {
        let fdf = Fdf::parse(
            r#"
Frame "GLUEBUTTON" "Switch" {
    ControlBackdrop "Up",
    ControlPushedBackdrop "Down",
    ControlDisabledBackdrop "Disabled",
}
Frame "BACKDROP" "Up" {
    BackdropBackground "old-up.blp",
    BackdropTileBackground,
}
Frame "BACKDROP" "Down" {
    BackdropBackground "old-down.blp",
}
Frame "BACKDROP" "Disabled" {
    BackdropBackground "old-disabled.blp",
}
"#,
        )
        .unwrap();
        let rect = Rect {
            left: 0.2,
            bottom: 0.1,
            width: 0.03,
            height: 0.04,
        };
        let original = button(&fdf, "Switch").unwrap();
        let mut items = vec![Item {
            frame: "Switch".into(),
            rect,
            kind: Kind::Button(Box::new(original.clone())),
        }];
        let mut art = pcx_play::menu_edition::ButtonArt {
            frame: "Switch".into(),
            backdrops: ["Up".into(), "Down".into(), "Disabled".into()],
            textures: [
                "other-up.blp".into(),
                "other-down.blp".into(),
                "other-disabled.blp".into(),
            ],
        };
        for paths in [
            art.textures.clone(),
            [
                "old-up.blp".into(),
                "old-down.blp".into(),
                "old-disabled.blp".into(),
            ],
        ] {
            art.textures = paths;
            edition_button_art(&fdf, &mut items, &art).unwrap();
            let Kind::Button(current) = &items[0].kind else {
                panic!("button")
            };
            let mut expected = original.clone();
            for (state, path) in [
                &mut expected.normal,
                &mut expected.pushed,
                &mut expected.disabled,
            ]
            .into_iter()
            .zip(&art.textures)
            {
                state.as_mut().unwrap().background = Some(path.clone());
            }
            assert_eq!(current.as_ref(), &expected);
            assert_eq!(items[0].rect, rect);
        }
        let before = items.clone();
        art.backdrops[1] = "unrelated-frame".into();
        assert!(edition_button_art(&fdf, &mut items, &art).is_err());
        assert_eq!(
            items, before,
            "bad source binding cannot partially change art"
        );
    }

    #[test]
    fn repeated_named_subtrees_cannot_expand_without_a_bound() {
        let fdf = Fdf::parse(
            r#"
Frame "FRAME" "Root" {
Frame "FRAME" "A" {
}
Frame "FRAME" "A" {
}
Frame "FRAME" "A" {
Frame "FRAME" "B" {
}
}
}
"#,
        )
        .unwrap();
        assert!(matches!(
            control_items(&fdf, "Root", (0.8, 0.6)),
            Err(FdfError::Unsupported {
                property: "frame traversal",
                ..
            })
        ));
    }

    const TEMPLATES: &str = r#"
Frame "BACKDROP" "BaseBackdrop" {
    Width 0.256,
    Height 0.064,
    BackdropBlendAll,
    BackdropBackground  "plate.blp",
}
Frame "BACKDROP" "FillTemplate" {
    BackdropTileBackground,
    BackdropBackground  "fill.blp",
    BackdropCornerFlags "UL|UR|BL|BR|T|L|B|R",
    BackdropCornerSize  0.016,
    BackdropBackgroundSize  0.032,
    BackdropBackgroundInsets 0.004 0.004 0.004 0.004,
    BackdropEdgeFile  "edge.blp",
}
Frame "HIGHLIGHT" "GlowTemplate" {
    HighlightType "FILETEXTURE",
    HighlightAlphaFile "glow.blp",
    HighlightAlphaMode "ADD",
}
Frame "TEXTBUTTON" "ButtonTemplate" {
    Height 0.031,
    Width 0.179,
    ButtonPushedTextOffset -0.002f -0.003f,
    ControlBackdrop "Up",
    Frame "BACKDROP" "Up" INHERITS "FillTemplate" {
    }
    ControlPushedBackdrop "Down",
    Frame "BACKDROP" "Down" INHERITS "FillTemplate" {
        BackdropEdgeFile  "edge-down.blp",
    }
    ControlMouseOverHighlight "Glow",
    Frame "HIGHLIGHT" "Glow" INHERITS "GlowTemplate" {
    }
}
"#;

    const MENU: &str = r#"
Frame "FRAME" "MainMenuFrame" {
    SetAllPoints,
    Frame "FRAME" "Hidden" {
        SetAllPoints,
    }
    Frame "FRAME" "ControlLayer" {
        SetAllPoints,
        Frame "BACKDROP" "OneBackdrop" INHERITS "BaseBackdrop" {
            SetPoint TOPRIGHT,"MainMenuFrame",TOPRIGHT, -0.015, -0.110625,
            Frame "GLUETEXTBUTTON" "OneButton" INHERITS WITHCHILDREN "ButtonTemplate" {
                SetPoint TOPRIGHT,"OneBackdrop",TOPRIGHT, -0.012, -0.0165,
                ControlShortcutKey "S",
                ButtonText "OneText",
                Frame "TEXT" "OneText" {
                    Text "KEY_ONE",
                }
            }
        }
        Frame "BACKDROP" "TwoBackdrop" INHERITS "BaseBackdrop" {
            Width 0.24,
            SetPoint BOTTOMRIGHT,"MainMenuFrame",BOTTOMRIGHT,-0.015,0.05,
            BackdropMirrored,
        }
        Frame "SPRITE" "Odd" {
            SetAllPoints,
        }
    }
}
"#;

    fn parsed() -> Fdf {
        let mut fdf = Fdf::parse(TEMPLATES).unwrap();
        fdf.parse_into(MENU).unwrap();
        fdf
    }

    #[test]
    fn control_layer_items_follow_anchors_and_templates() {
        let (items, unsupported) = control_items(&parsed(), CONTROL_LAYER, (0.8, 0.6)).unwrap();
        let names: Vec<&str> = items.iter().map(|i| i.frame.as_str()).collect();
        assert_eq!(names, ["OneBackdrop", "OneButton", "TwoBackdrop"]);
        assert_eq!(unsupported, ["SPRITE Odd"]);
        let plate = &items[0].rect;
        assert!((plate.left - (0.8 - 0.015 - 0.256)).abs() < 1e-6);
        assert!((plate.top() - (0.6 - 0.110625)).abs() < 1e-6);
        let button = &items[1].rect;
        assert!((button.left + button.width - (plate.left + plate.width - 0.012)).abs() < 1e-6);
        assert!((button.width - 0.179).abs() < 1e-6);
        let exit = &items[2];
        assert!((exit.rect.bottom - 0.05).abs() < 1e-6);
        assert!(matches!(&exit.kind, Kind::Backdrop(b) if b.mirrored && b.edge.is_none()));
    }

    #[test]
    fn button_states_come_from_their_named_frames() {
        let (items, _) = control_items(&parsed(), CONTROL_LAYER, (0.8, 0.6)).unwrap();
        let Kind::Button(b) = &items[1].kind else {
            panic!("button expected");
        };
        let normal = b.normal.as_ref().unwrap();
        assert_eq!(normal.background.as_deref(), Some("fill.blp"));
        assert!(normal.tile);
        assert_eq!(normal.inset, Some(0.004));
        let edge = normal.edge.as_ref().unwrap();
        assert_eq!(edge.file, "edge.blp");
        assert_eq!(edge.corner_size, 0.016);
        assert_eq!(edge.pieces, EdgePiece::ALL);
        assert_eq!(
            b.pushed.as_ref().unwrap().edge.as_ref().unwrap().file,
            "edge-down.blp"
        );
        assert!(b.disabled.is_none());
        assert_eq!(
            b.mouse_over,
            Some(Highlight {
                file: "glow.blp".into(),
                additive: true
            })
        );
        assert_eq!(b.text.as_deref(), Some("OneText"));
        assert_eq!(b.pushed_text_offset, Some([-0.002, -0.003]));
        assert_eq!(b.shortcut, Some('S'));
    }

    /// Against the shipped 1.29b frame definitions: every main-menu control,
    /// including the edition and realm buttons, resolves with its states.
    #[test]
    #[ignore = "requires PEACECRAFT3_LEGACY_DIR with the original 1.29b archives"]
    fn shipped_control_layer_resolves() {
        let install = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_LEGACY_DIR").expect("explicit 1.29b install"),
        );
        let text = |path: &str| {
            String::from_utf8(crate::mainmenu::art_bytes(Some(&install), path).expect(path))
                .unwrap()
        };
        let mut fdf = Fdf::parse(&text(crate::mainmenu::STANDARD_TEMPLATES_FDF)).unwrap();
        fdf.parse_into(&text(crate::mainmenu::MAIN_MENU_FDF))
            .unwrap();
        let (items, unsupported) = control_items(&fdf, CONTROL_LAYER, (0.8, 0.6)).unwrap();
        assert!(unsupported.is_empty(), "{unsupported:?}");
        let buttons: Vec<(&str, &Button)> = items
            .iter()
            .filter_map(|i| match &i.kind {
                Kind::Button(b) => Some((i.frame.as_str(), b.as_ref())),
                Kind::Backdrop(_) | Kind::Text | Kind::Widget(_) => None,
            })
            .collect();
        let names: Vec<&str> = buttons.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            [
                "EditionButton",
                "SinglePlayerButton",
                "RealmButton",
                "BattleNetButton",
                "LocalAreaNetworkButton",
                "OptionsButton",
                "CreditsButton",
                "ExitButton"
            ]
        );
        for (name, button) in &buttons {
            assert!(button.normal.is_some(), "{name}");
            assert!(button.pushed.is_some(), "{name}");
            assert!(button.disabled.is_some(), "{name}");
            assert!(
                button.mouse_over.as_ref().is_some_and(|h| h.additive),
                "{name}"
            );
        }
        let plates = items
            .iter()
            .filter(|i| matches!(i.kind, Kind::Backdrop(_)))
            .count();
        assert_eq!(plates, 6);
    }

    #[test]
    fn corner_flags_select_pieces_and_refuse_unknown_tokens() {
        let fdf = Fdf::parse(
            "Frame \"BACKDROP\" \"A\" {\n    BackdropEdgeFile \"e.blp\",\n    BackdropCornerSize 0.01,\n    BackdropCornerFlags \"T|UL\",\n}\nFrame \"BACKDROP\" \"B\" {\n    BackdropEdgeFile \"e.blp\",\n    BackdropCornerSize 0.01,\n    BackdropCornerFlags \"T|X\",\n}\nFrame \"BACKDROP\" \"C\" {\n    BackdropBackground \"b.blp\",\n    BackdropBackgroundInsets 0.1 0.2 0.1 0.1,\n}\n",
        )
        .unwrap();
        let a = backdrop(&fdf, "A").unwrap().unwrap();
        assert_eq!(
            a.edge.unwrap().pieces,
            [EdgePiece::Top, EdgePiece::UpperLeft]
        );
        assert!(backdrop(&fdf, "B").is_err());
        assert!(backdrop(&fdf, "C").is_err());
    }

    #[test]
    fn absent_geometry_fields_remain_absent_and_unknown_blends_refuse() {
        let source = "Frame \"BACKDROP\" \"Background\" {\n BackdropBackground \"fixture.blp\",\n}\nFrame \"HIGHLIGHT\" \"Glow\" {\n HighlightType \"FILETEXTURE\",\n HighlightAlphaFile \"fixture.blp\",\n HighlightAlphaMode \"BLEND\",\n}\nFrame \"TEXTBUTTON\" \"Button\" {\n ControlMouseOverHighlight \"Glow\",\n}\n";
        let fdf = Fdf::parse(source).unwrap();
        assert_eq!(backdrop(&fdf, "Background").unwrap().unwrap().inset, None);
        let button = button(&fdf, "Button").unwrap();
        assert_eq!(button.pushed_text_offset, None);
        assert!(!button.mouse_over.unwrap().additive);
        let unknown = Fdf::parse(&source.replace("\"BLEND\"", "\"UNPROVED\"")).unwrap();
        assert!(matches!(
            super::button(&unknown, "Button"),
            Err(FdfError::Unsupported {
                property: "HighlightAlphaMode",
                ..
            })
        ));
        let missing = Fdf::parse(&source.replace(" HighlightAlphaMode \"BLEND\",\n", "")).unwrap();
        assert!(matches!(
            super::button(&missing, "Button"),
            Err(FdfError::Missing {
                property: "HighlightAlphaMode",
                ..
            })
        ));
    }

    fn image_world() -> World {
        let mut world = World::new();
        world.init_resource::<GlueImages>();
        world.init_resource::<Assets<Image>>();
        world.init_resource::<Assets<AdditiveImage>>();
        world.init_resource::<Assets<BackdropImage>>();
        world
    }

    fn cache_image(world: &mut World, path: &str, tile: bool, width: u32, height: u32) {
        let texture = pcx_gfx::LoadedTexture {
            mipmaps: Vec::new(),
            source: path.into(),
            width,
            height,
            rgba: (0..width * height)
                .flat_map(|i| [i as u8, 20, 30, 255])
                .collect(),
        };
        let handle = world
            .resource_mut::<Assets<Image>>()
            .add(crate::menu_composition::image(&texture));
        world
            .resource_mut::<GlueImages>()
            .images
            .insert((path.into(), tile), handle);
    }

    #[test]
    fn all_edge_slices_render_with_distinct_source_rectangles_and_mirroring() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = image_world();
        let mut entities = Vec::new();
        for piece in EdgePiece::ALL {
            cache_image(&mut world, "edge.blp", false, 16, 2);
            entities.push((
                piece,
                world
                    .spawn(PendingArt {
                        path: "edge.blp".into(),
                        piece: Some(piece),
                        mirrored: true,
                        tile: false,
                        additive: false,
                    })
                    .id(),
            ));
        }
        world.run_system_once(load_art).unwrap();
        for (piece, entity) in entities {
            if matches!(piece, EdgePiece::Top | EdgePiece::Bottom) {
                let node = world.get::<MaterialNode<BackdropImage>>(entity).unwrap();
                let material = world
                    .resource::<Assets<BackdropImage>>()
                    .get(&node.0)
                    .unwrap();
                let rect = edge_source_rect(piece, 16, 2).unwrap();
                assert_eq!(material.uv_rows, Vec4::new(0.0, 1.0, 1.0, 0.0));
                assert_eq!(
                    material.uv_transform,
                    Vec4::new(0.125, 1.0, rect.min.x / 16.0, 0.0)
                );
                // Independent asymmetric 2x2 markers: clockwise, then mirrored.
                for (mirrored, expected) in [(false, [2, 0, 3, 1]), (true, [0, 2, 1, 3])] {
                    let material = horizontal_edge_material(
                        material.texture.clone(),
                        rect,
                        Vec2::new(16.0, 2.0),
                        mirrored,
                    );
                    let mut actual = Vec::new();
                    for y in 0..2 {
                        for x in 0..2 {
                            let uv = Vec2::new((x as f32 + 0.5) / 2.0, (y as f32 + 0.5) / 2.0);
                            let rotated = Vec2::new(
                                material.uv_rows.xy().dot(uv),
                                material.uv_rows.zw().dot(uv),
                            );
                            let sample =
                                rotated * material.uv_transform.xy() + material.uv_transform.zw();
                            let source = sample * Vec2::new(16.0, 2.0) - rect.min;
                            actual.push(source.y.floor() as u32 * 2 + source.x.floor() as u32);
                        }
                    }
                    assert_eq!(actual, expected);
                }
                assert!(world.get::<ImageNode>(entity).is_none());
            } else {
                let image = world.get::<ImageNode>(entity).unwrap();
                assert_eq!(image.rect, Some(edge_source_rect(piece, 16, 2).unwrap()));
                assert!(image.flip_x);
            }
            assert!(world.get::<PendingArt>(entity).is_none());
        }
        assert!(edge_source_rect(EdgePiece::Left, 15, 2).is_err());
        assert!(edge_source_rect(EdgePiece::Left, 8, 0).is_err());
        assert!(edge_source_rect(EdgePiece::Left, u32::MAX, u32::MAX).is_err());
    }

    #[test]
    fn patterned_background_keeps_per_axis_repeats_and_mirror_transform() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = image_world();
        cache_image(&mut world, "pattern.blp", true, 2, 1);
        let entity = world
            .spawn((
                PendingArt {
                    path: "pattern.blp".into(),
                    piece: None,
                    mirrored: true,
                    tile: true,
                    additive: false,
                },
                BackdropRepeat(Vec2::new(3.5, 1.25)),
            ))
            .id();
        world.run_system_once(load_art).unwrap();
        let node = world.get::<MaterialNode<BackdropImage>>(entity).unwrap();
        let material = world
            .resource::<Assets<BackdropImage>>()
            .get(&node.0)
            .unwrap();
        assert_eq!(material.uv_transform, Vec4::new(-3.5, 1.25, 3.5, 0.0));
        assert!(world.get::<PendingArt>(entity).is_none());
        // No authored tile size uses the explicit intrinsic-image tiling convention.
        let refused = world
            .spawn(PendingArt {
                path: "pattern.blp".into(),
                piece: None,
                mirrored: false,
                tile: true,
                additive: false,
            })
            .id();
        world.run_system_once(load_art).unwrap();
        assert!(matches!(
            world.get::<ImageNode>(refused).unwrap().image_mode,
            NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 1.0
            }
        ));
    }

    #[test]
    fn omitted_insets_render_full_background_and_authored_size_drives_repeats() {
        use bevy::ecs::system::RunSystemOnce;
        fn build(mut commands: Commands) {
            let fdf = Fdf::parse("Frame \"BACKDROP\" \"A\" {\n BackdropBackground \"pattern.blp\",\n BackdropTileBackground,\n BackdropBackgroundSize 0.02,\n}\n").unwrap();
            let root = commands.spawn(Node::default()).id();
            spawn_backdrop(
                &mut commands,
                root,
                &backdrop(&fdf, "A").unwrap().unwrap(),
                Vec2::new(0.1, 0.06),
            );
        }
        let mut world = image_world();
        world.run_system_once(build).unwrap();
        let mut q = world.query::<(&Node, &BackdropRepeat)>();
        let (node, repeat) = q.single(&world).unwrap();
        assert_eq!(node.width, Val::Percent(100.0));
        assert_eq!(node.height, Val::Percent(100.0));
        assert_eq!(repeat.0, Vec2::new(5.0, 3.0));
        let top = edge_rect(EdgePiece::Top, Vec2::new(0.2, 0.1), 0.02);
        assert_eq!(
            (top.left, top.bottom, top.width, top.height),
            (0.02, 0.08, 0.16, 0.02)
        );
        let small = edge_rect(EdgePiece::UpperLeft, Vec2::new(0.01, 0.01), 0.02);
        assert_eq!((small.width, small.height), (0.005, 0.005));
    }

    #[test]
    fn uniformity_requires_complete_nonempty_rgba_texels() {
        assert!(uniform(&[1, 2, 3, 4, 1, 2, 3, 4]));
        assert!(!uniform(&[1, 2, 3, 4, 9, 2, 3, 4]));
        assert!(!uniform(&[1, 2, 3, 4, 1]));
        assert!(!uniform(&[]));
    }
    #[test]
    fn settings_art_tracks_authored_geometry_values_checked_and_disabled_states() {
        use bevy::ecs::system::RunSystemOnce;
        fn build(mut commands: Commands) {
            let fdf = Fdf::parse(
                r#"
Frame "BACKDROP" "Base" {
 BackdropBackground "synthetic.blp",
}
Frame "HIGHLIGHT" "Checked" {
 HighlightType "FILETEXTURE",
 HighlightAlphaFile "checked.blp",
 HighlightAlphaMode "BLEND",
}
Frame "HIGHLIGHT" "DisabledChecked" {
 HighlightType "FILETEXTURE",
 HighlightAlphaFile "disabled-checked.blp",
 HighlightAlphaMode "ADD",
}
Frame "BUTTON" "Thumb" {
 Width 0.02,
 Height 0.04,
 ControlBackdrop "Base",
}
Frame "SLIDER" "Volume" {
 SliderLayoutHorizontal,
 SliderMinValue 10,
 SliderMaxValue 30,
 SliderInitialValue 20,
 SliderThumbButtonFrame "Thumb",
 ControlBackdrop "Base",
}
Frame "GLUECHECKBOX" "Toggle" {
 ControlBackdrop "Base",
 CheckBoxCheckHighlight "Checked",
 CheckBoxDisabledCheckHighlight "DisabledChecked",
}
Frame "BUTTON" "Arrow" {
 Width 0.02,
 Height 0.02,
 ControlBackdrop "Base",
}
Frame "GLUEPOPUPMENU" "Popup" {
 PopupArrowFrame "Arrow",
}
"#,
            )
            .unwrap();
            for (frame, size) in [
                ("Volume", Vec2::new(0.2, 0.04)),
                ("Toggle", Vec2::splat(0.04)),
                ("Popup", Vec2::new(0.2, 0.04)),
            ] {
                let node = commands.spawn((Node::default(), Name::new(frame))).id();
                decorate_setting(&mut commands, node, &fdf, frame, size).unwrap();
            }
        }
        fn percent(value: Val, expected: f32) {
            let Val::Percent(actual) = value else {
                panic!("expected percent");
            };
            assert!((actual - expected).abs() <= 100.0 * f32::EPSILON);
        }
        let mut world = image_world();
        world.run_system_once(build).unwrap();
        let (thumb, owner) = {
            let mut q = world.query::<(Entity, &SettingThumbArt)>();
            let (entity, art) = q.single(&world).unwrap();
            (entity, art.owner)
        };
        assert!(world.get::<bevy::ui_widgets::SliderThumb>(thumb).is_some());
        percent(world.get::<Node>(thumb).unwrap().left, 45.0);
        world.entity_mut(owner).insert((
            bevy::ui_widgets::SliderValue(30.0),
            bevy::ui_widgets::SliderRange::new(10.0, 30.0),
        ));
        world.run_system_once(update_setting_art).unwrap();
        percent(world.get::<Node>(thumb).unwrap().left, 90.0);
        world.entity_mut(owner).insert((
            bevy::ui_widgets::SliderValue(0.0),
            bevy::ui_widgets::SliderRange::new(-f32::MAX, f32::MAX),
        ));
        world.run_system_once(update_setting_art).unwrap();
        percent(world.get::<Node>(thumb).unwrap().left, 45.0);
        let toggle = {
            let mut q = world.query::<&SettingCheckArt>();
            q.iter(&world).next().unwrap().owner
        };
        world.entity_mut(toggle).insert(bevy::ui::Checked);
        world.run_system_once(update_setting_art).unwrap();
        {
            let mut q = world.query::<(&SettingCheckArt, &Visibility)>();
            for (art, visibility) in q.iter(&world) {
                assert_eq!(
                    *visibility,
                    if art.disabled {
                        Visibility::Hidden
                    } else {
                        Visibility::Inherited
                    }
                );
            }
        }
        world.entity_mut(toggle).insert(InteractionDisabled);
        world.run_system_once(update_setting_art).unwrap();
        {
            let mut q = world.query::<(&SettingCheckArt, &Visibility)>();
            for (art, visibility) in q.iter(&world) {
                assert_eq!(
                    *visibility,
                    if art.disabled {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    }
                );
            }
        }
        let popup = {
            let mut q = world.query::<(Entity, &Name)>();
            q.iter(&world)
                .find(|(_, name)| name.as_str() == "Popup")
                .unwrap()
                .0
        };
        let arrow = world.get::<Children>(popup).unwrap()[0];
        let node = world.get::<Node>(arrow).unwrap();
        percent(node.left, 90.0);
        percent(node.top, 25.0);
        percent(node.width, 10.0);
        percent(node.height, 50.0);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen::prelude::wasm_bindgen]
    pub fn glue_backdrop_synthetic_check() -> u32 {
        all_edge_slices_render_with_distinct_source_rectangles_and_mirroring();
        patterned_background_keeps_per_axis_repeats_and_mirror_transform();
        omitted_insets_render_full_background_and_authored_size_drives_repeats();
        settings_art_tracks_authored_geometry_values_checked_and_disabled_states();
        4
    }
    #[test]
    fn unavailable_preview_thumb_hides_initial_and_resets_after_a_valid_preview() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let owner = world.spawn(bevy::ui_widgets::Button).id();
        let widget = crate::menu_screens::Widget::Slider {
            min: 0.0,
            max: 10.0,
            initial: 5.0,
            step: 3.0,
        };
        let missing = crate::menu_settings::preview::Snapshot::unavailable(
            "synthetic".into(),
            "No admitted client defaults.",
        );
        crate::menu_settings::preview::attach(
            &mut world.commands(),
            owner,
            "GammaSlider",
            &widget,
            &missing,
        );
        world.flush();
        let thumb = world
            .spawn((
                Node::default(),
                Visibility::Inherited,
                SettingThumbArt {
                    owner,
                    control: Vec2::new(10.0, 2.0),
                    thumb: Vec2::new(1.0, 1.0),
                    vertical: false,
                },
            ))
            .id();
        let check = world
            .spawn((
                Visibility::Inherited,
                SettingCheckArt {
                    owner,
                    disabled: true,
                    has_disabled: true,
                },
            ))
            .id();
        world.run_system_once(update_setting_art).unwrap();
        assert_eq!(*world.get::<Visibility>(check).unwrap(), Visibility::Hidden);
        assert_eq!(*world.get::<Visibility>(thumb).unwrap(), Visibility::Hidden);
        world.entity_mut(owner).insert((
            bevy::ui_widgets::SliderValue(7.0),
            bevy::ui_widgets::SliderRange::new(0.0, 10.0),
        ));
        world.run_system_once(update_setting_art).unwrap();
        assert_eq!(
            *world.get::<Visibility>(thumb).unwrap(),
            Visibility::Inherited
        );
        let Val::Percent(left) = world.get::<Node>(thumb).unwrap().left else {
            panic!("relative thumb position required")
        };
        assert!((left - 63.0).abs() < 0.001);
        crate::menu_settings::preview::attach(
            &mut world.commands(),
            owner,
            "GammaSlider",
            &widget,
            &missing,
        );
        world.flush();
        world.run_system_once(update_setting_art).unwrap();
        assert_eq!(*world.get::<Visibility>(thumb).unwrap(), Visibility::Hidden);
    }
}
