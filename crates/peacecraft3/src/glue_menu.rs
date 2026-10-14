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

/// One of the eight pieces of a `BackdropEdgeFile`, in atlas slot order.
///
/// The order is read from the shipped edge files' pixels
/// (`GlueScreen-Button1-BackdropBorder.blp`, `GlueScreen-Button2-…`): eight
/// square slots, left to right, whose chamfers and outer borders sit on these
/// sides. The top and bottom slots are stored as vertical strips with their outer
/// border on the left and right, so they are drawn turned a quarter turn
/// clockwise. See `docs/re-notes/menu-border-atlas.md`.
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

    /// The atlas slot this piece occupies.
    #[must_use]
    pub fn slot(self) -> usize {
        self as usize
    }

    /// Whether the slot is stored as a vertical strip and drawn turned.
    #[must_use]
    pub fn turned(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }

    /// The `BackdropCornerFlags` token that enables this piece.
    fn flag(self) -> &'static str {
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
    /// The pieces `BackdropCornerFlags` enables, in atlas slot order.
    pub pieces: Vec<EdgePiece>,
}

/// A `BACKDROP` frame's art.
#[derive(Debug, Clone, PartialEq)]
pub struct Backdrop {
    pub background: Option<String>,
    /// `BackdropTileBackground`.
    pub tile: bool,
    /// `BackdropMirrored`: drawn flipped left to right.
    pub mirrored: bool,
    /// `BackdropBackgroundInsets`, which the shipped menu always writes equal.
    pub inset: f32,
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
    pub pushed_text_offset: [f32; 2],
    /// `ControlShortcutKey`.
    pub shortcut: Option<char>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Backdrop(Backdrop),
    Button(Button),
}

/// One drawable frame and where it sits, in the engine's Y-up frame units.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub frame: String,
    pub rect: Rect,
    pub kind: Kind,
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
        None => 0.0,
        Some([a, b, c, d]) if a == b && b == c && c == d => *a,
        Some(_) => return Err(missing(frame, "equal BackdropBackgroundInsets")),
    };
    Ok(Some(Backdrop {
        background,
        tile: fdf.flag(frame, "BackdropTileBackground"),
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
                additive: fdf.string(&name, "HighlightAlphaMode").as_deref() == Some("ADD"),
            }),
            _ => return Err(missing(&name, "HighlightType \"FILETEXTURE\"")),
        },
    };
    let pushed_text_offset = match fdf.numbers(frame, "ButtonPushedTextOffset")?.as_deref() {
        None => [0.0, 0.0],
        Some([x, y]) => [*x, *y],
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
    matches!(kind, "GLUETEXTBUTTON" | "GLUEBUTTON" | "TEXTBUTTON" | "BUTTON")
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
    let mut items = Vec::new();
    let mut unsupported = Vec::new();
    let frame = fdf
        .frame(root)
        .ok_or_else(|| FdfError::UnknownFrame(root.to_string()))?;
    let mut pending: Vec<String> = frame.children.iter().rev().cloned().collect();
    while let Some(name) = pending.pop() {
        let frame = fdf
            .frame(&name)
            .ok_or_else(|| FdfError::UnknownFrame(name.clone()))?;
        let kind = frame.kind.as_str();
        if is_button(kind) {
            items.push(Item {
                rect: fdf.rect(&name, viewport)?,
                kind: Kind::Button(button(fdf, &name)?),
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
    app.add_plugins(UiMaterialPlugin::<AdditiveImage>::default())
        .init_resource::<GlueImages>()
        .add_systems(
            PreUpdate,
            shortcut_keys.after(bevy::input::InputSystems),
        )
        .add_systems(Update, (load_art, show_states, place_shadows));
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

/// A clickable FDF frame. Clicks are dispatched by [`GlueButton::frame`].
#[derive(Component, Debug)]
pub struct GlueButton {
    pub frame: String,
    pub shortcut: Option<char>,
    /// `ButtonPushedTextOffset` as a percentage of the button's width and
    /// height, Y down.
    pub pushed_text_offset: Vec2,
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

/// Decoded art by path and edge piece, shared by every frame that names it.
#[derive(Resource, Default)]
pub struct GlueImages {
    images: std::collections::HashMap<(String, Option<EdgePiece>), Handle<Image>>,
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
    let local = Rect {
        left: 0.0,
        bottom: 0.0,
        width: size.x,
        height: size.y,
    };
    if let Some(path) = &art.background {
        let inner = Rect {
            left: art.inset,
            bottom: art.inset,
            width: size.x - 2.0 * art.inset,
            height: size.y - 2.0 * art.inset,
        };
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
        commands.entity(parent).add_child(child);
    }
    let Some(edge) = &art.edge else {
        return;
    };
    let c = edge.corner_size;
    let (w, h) = (size.x, size.y);
    for &piece in &edge.pieces {
        let (left, bottom, width, height) = match piece {
            EdgePiece::UpperLeft => (0.0, h - c, c, c),
            EdgePiece::UpperRight => (w - c, h - c, c, c),
            EdgePiece::BottomLeft => (0.0, 0.0, c, c),
            EdgePiece::BottomRight => (w - c, 0.0, c, c),
            EdgePiece::Left => (0.0, c, c, h - 2.0 * c),
            EdgePiece::Right => (w - c, c, c, h - 2.0 * c),
            EdgePiece::Top => (c, h - c, w - 2.0 * c, c),
            EdgePiece::Bottom => (c, 0.0, w - 2.0 * c, c),
        };
        let rect = Rect {
            left,
            bottom,
            width,
            height,
        };
        let child = commands
            .spawn((
                placed(&rect, &local),
                PendingArt {
                    path: edge.file.clone(),
                    piece: Some(piece),
                    mirrored: false,
                    tile: false,
                    additive: false,
                },
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(parent).add_child(child);
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
            ))
            .id();
        commands.entity(root).add_child(node);
        match &item.kind {
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
                        pushed_text_offset: Vec2::new(
                            button.pushed_text_offset[0] / size.x * 100.0,
                            -button.pushed_text_offset[1] / size.y * 100.0,
                        ),
                    },
                ));
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

/// The bytes of one art path: the install, else the browser's store.
fn art_bytes(install: Option<&std::path::Path>, path: &str) -> Option<Vec<u8>> {
    crate::mainmenu::art_bytes(install, path).or_else(|| crate::mainmenu::from_served(path))
}

/// Cuts one piece out of a `BackdropEdgeFile`, turning the top and bottom
/// strips a quarter turn clockwise.
fn edge_piece(texture: &pcx_gfx::LoadedTexture, piece: EdgePiece) -> Result<Image, String> {
    let (w, h) = (texture.width as usize, texture.height as usize);
    let slots = EdgePiece::ALL.len();
    if w != h * slots {
        return Err(format!(
            "{}: {w}x{h} is not {slots} square slots",
            texture.source
        ));
    }
    let x0 = piece.slot() * h;
    let mut rgba = Vec::with_capacity(h * h * 4);
    for y in 0..h {
        for x in 0..h {
            // Clockwise: the output's (x, y) reads the source's (y, h-1-x).
            let (sx, sy) = if piece.turned() { (y, h - 1 - x) } else { (x, y) };
            let at = ((sy * w) + x0 + sx) * 4;
            rgba.extend_from_slice(&texture.rgba[at..at + 4]);
        }
    }
    let side = h as u32;
    Ok(pcx_gfx::viewer::rgba_image(&pcx_gfx::LoadedTexture {
        width: side,
        height: side,
        rgba,
        ..texture.clone()
    }))
}

/// Whether every texel of an RGBA8 buffer is the same colour.
fn uniform(rgba: &[u8]) -> bool {
    match rgba.get(..4) {
        Some(first) => rgba.as_chunks::<4>().0.iter().all(|p| p == first),
        None => false,
    }
}

/// Decodes pending art and puts it on its node.
fn load_art(
    mut commands: Commands,
    mut cache: ResMut<GlueImages>,
    mut images: ResMut<Assets<Image>>,
    mut additive: ResMut<Assets<AdditiveImage>>,
    pending: Query<(Entity, &PendingArt)>,
) {
    for (entity, art) in &pending {
        let key = (art.path.clone(), art.piece);
        let handle = match cache.images.get(&key) {
            Some(handle) => handle.clone(),
            None => {
                let install = cache.install.clone();
                let decoded = art_bytes(install.as_deref(), &art.path)
                    .ok_or_else(|| format!("{} is not readable", art.path))
                    .and_then(|bytes| {
                        pcx_gfx::load_image(&bytes, &art.path).map_err(|e| e.to_string())
                    })
                    .and_then(|texture| match art.piece {
                        Some(piece) => edge_piece(&texture, piece),
                        // ponytail: tiling is drawn as a stretch, which is the
                        // same image only for a one-colour texture; the menu's
                        // tiled fills all are. A patterned tile is refused.
                        None if art.tile && !uniform(&texture.rgba) => Err(format!(
                            "{}: patterned BackdropTileBackground is not drawn",
                            art.path
                        )),
                        None => Ok(pcx_gfx::viewer::rgba_image(&texture)),
                    });
                match decoded {
                    Ok(image) => {
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
        let mut entity = commands.entity(entity);
        entity.remove::<PendingArt>();
        if art.additive {
            entity.insert(MaterialNode(additive.add(AdditiveImage { texture: handle })));
        } else {
            let mut image = ImageNode::new(handle).with_mode(NodeImageMode::Stretch);
            image.flip_x = art.mirrored;
            entity.insert(image);
        }
    }
}

/// Shows each button's normal, pushed, disabled and mouse-over art for its
/// state, and moves its label by `ButtonPushedTextOffset` while pressed.
fn show_states(
    buttons: Query<(&GlueButton, &Hovered, Has<Pressed>, Has<InteractionDisabled>)>,
    mut layers: Query<(&StateLayer, &mut Visibility)>,
    mut labels: Query<(&GlueLabel, &ChildOf)>,
    mut nodes: Query<&mut Node>,
) {
    for (layer, mut visibility) in &mut layers {
        let Ok((_, hovered, pressed, disabled)) = buttons.get(layer.button) else {
            continue;
        };
        let [_, has_pushed, has_disabled] = layer.has;
        let shown = match layer.state {
            State::MouseOver => hovered.get() && !disabled,
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
            button.pushed_text_offset
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
fn shortcut_keys(
    mut keys: MessageReader<bevy::input::keyboard::KeyboardInput>,
    buttons: Query<(Entity, &GlueButton), Without<InteractionDisabled>>,
    mut actions: ResMut<crate::button_actions::ButtonActions>,
) {
    for key in keys.read() {
        let bevy::input::keyboard::Key::Character(text) = &key.logical_key else {
            continue;
        };
        if !key.state.is_pressed() || key.repeat {
            continue;
        }
        for (entity, button) in &buttons {
            let hit = button
                .shortcut
                .is_some_and(|c| text.chars().eq(c.to_lowercase()) || text.chars().eq(c.to_uppercase()));
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
        assert_eq!(normal.inset, 0.004);
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
        assert_eq!(b.pushed_text_offset, [-0.002, -0.003]);
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
        fdf.parse_into(&text(crate::mainmenu::MAIN_MENU_FDF)).unwrap();
        let (items, unsupported) = control_items(&fdf, CONTROL_LAYER, (0.8, 0.6)).unwrap();
        assert!(unsupported.is_empty(), "{unsupported:?}");
        let buttons: Vec<(&str, &Button)> = items
            .iter()
            .filter_map(|i| match &i.kind {
                Kind::Button(b) => Some((i.frame.as_str(), b)),
                Kind::Backdrop(_) => None,
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
            assert!(button.mouse_over.as_ref().is_some_and(|h| h.additive), "{name}");
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
}
