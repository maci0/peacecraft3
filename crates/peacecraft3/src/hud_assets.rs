//! Source-qualified HUD art, font, and placement.
//!
//! The caller supplies the FDF texts, skin table, race-section order, root
//! roles, viewport, and the bytes for every image and font the frames name.
//! Geometry comes from [`crate::hud::derive`]. Texture blocks, backdrop
//! records, and text styles are read from that same input. A missing file,
//! an unsupported frame, or an ambiguous skin hit refuses the candidate.

use std::collections::HashMap;

use crate::fdf::{Fdf, FdfError, Point, Rect};
use crate::glue_menu;
use crate::hud::{HudFrame, HudLayout};
use crate::menu_skin::{FontBinding, SkinError, SkinTable};
use crate::menu_text::{self, JustifyH, JustifyV};

/// Why a HUD candidate was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum HudError {
    /// The caller's input cannot be applied.
    Input(String),
    /// The FDF parser or layout derivation refused the text.
    Fdf(Box<FdfError>),
    /// The skin table refused a lookup.
    Skin(Box<SkinError>),
    /// A text frame's style or label could not be read.
    Text(String),
    /// An authored value has no supported rendering.
    Unsupported {
        /// Frame that carries it.
        frame: String,
        /// Record or frame kind.
        property: &'static str,
        /// The authored text.
        value: String,
    },
    /// A required authored value is absent.
    Missing {
        /// Frame that lacks it.
        frame: String,
        /// Record name.
        property: &'static str,
    },
    /// Image bytes are missing or will not decode.
    Image {
        /// Path the frame resolved to.
        path: String,
        /// Decoder or lookup detail.
        detail: String,
    },
    /// Font bytes are missing for a resolved face.
    Font {
        /// Skin file or face.
        path: String,
        /// Lookup detail.
        detail: String,
    },
    /// A caller budget was exceeded. No pixels are decoded after this.
    Budget {
        /// Which budget: input, image-input, font, font-aggregate, image-decoded, decoded, retained, or transient.
        scope: &'static str,
        /// Path or `<input>`.
        path: String,
        /// Measured byte count.
        actual: usize,
        /// Caller limit.
        limit: usize,
    },
}

impl std::fmt::Display for HudError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(text) => write!(f, "{text}"),
            Self::Fdf(error) => write!(f, "{error}"),
            Self::Skin(error) => write!(f, "{error}"),
            Self::Text(text) => write!(f, "{text}"),
            Self::Unsupported {
                frame,
                property,
                value,
            } => write!(f, "{frame:?} {property} {value:?} is unsupported"),
            Self::Missing { frame, property } => {
                write!(f, "{frame:?} is missing {property}")
            }
            Self::Image { path, detail } => write!(f, "image {path:?}: {detail}"),
            Self::Font { path, detail } => write!(f, "font {path:?}: {detail}"),
            Self::Budget {
                scope,
                path,
                actual,
                limit,
            } => write!(
                f,
                "{scope} budget for {path}: {actual} bytes exceeds {limit}"
            ),
        }
    }
}

/// Caller limits. Every field is required. Zero rejects a positive payload.
/// Nothing here is filled from a built-in default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HudBudgets {
    /// FDF, skin, globals, image bytes, and font bytes together.
    pub max_input_bytes: usize,
    /// One image file before its header is read.
    pub max_image_input_bytes: usize,
    /// One font file.
    pub max_font_bytes: usize,
    /// Every font file together.
    pub max_font_aggregate_bytes: usize,
    /// One image's decoded RGBA.
    pub max_image_decoded_bytes: usize,
    /// Every decoded RGBA buffer together.
    pub max_decoded_bytes: usize,
    /// Decoded RGBA plus font bytes kept after the inputs are dropped.
    pub max_retained_bytes: usize,
    /// Input bytes still held plus decoded RGBA during resolve.
    pub max_transient_bytes: usize,
}

/// One run from a HUD markup adapter. `colour` `None` keeps the frame `FontColor`.
#[derive(Debug, Clone, PartialEq)]
pub struct HudMarkupSpan {
    /// Visible characters.
    pub text: String,
    /// Encoded colour, when the adapter kept one.
    pub colour: Option<[f32; 4]>,
}

/// Adapter output. `notes` lists markup the adapter did not interpret.
#[derive(Debug, Clone, PartialEq)]
pub struct HudMarkupText {
    /// Runs in source order. Empty when the authored string is empty.
    pub spans: Vec<HudMarkupSpan>,
    /// Reported forms. They are not recoloured by this package.
    pub notes: Vec<String>,
}

/// How HUD text is read. Menu access-key colours stay in `menu_text::Label`.
#[derive(Debug, Clone)]
pub enum HudMarkup {
    /// Literal text. `|` or `^^` refuses the candidate.
    Plain,
    /// Caller parser. A refused string is not drawn with a substitute colour.
    Adapter(fn(&str) -> Result<HudMarkupText, String>),
}

impl std::error::Error for HudError {}

/// Caller-selected source. Compared as an opaque identity, not opened here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceId {
    /// Archive, member, and edition label the caller chose.
    pub identity: String,
}

/// One authored root and the role the caller assigns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootRole {
    /// Caller role, such as the console root they selected.
    pub role: String,
    /// Authored frame name.
    pub frame: String,
}

/// Bytes the caller already loaded, keyed by the path the HUD will request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedBytes {
    /// Exact path, or a `DecorateFileNames` suffix appended to a skin value.
    pub path: String,
    /// File contents.
    pub bytes: Vec<u8>,
}

/// Everything [`resolve`] needs. Nothing here is read from the process environment.
#[derive(Debug, Clone)]
pub struct HudInput {
    /// Selected source identity.
    pub source: SourceId,
    /// FDF texts in caller order. Later texts replace earlier frame names.
    pub fdf_texts: Vec<String>,
    /// `war3skins.txt` text from the same selected source.
    pub skin_text: String,
    /// Section names consulted in this order. No section outside the list is used.
    pub race_sections: Vec<String>,
    /// Authored roots and their caller roles.
    pub roots: Vec<RootRole>,
    /// Viewport in frame units.
    pub viewport: (f32, f32),
    /// `GlobalStrings` text. An empty string leaves `Text` values as authored.
    pub global_strings: String,
    /// Suffixes tried, in order, when `DecorateFileNames` is set and the
    /// resolved name contains no `.`. The caller chooses them.
    pub decorate_suffixes: Vec<String>,
    /// Image bytes.
    pub images: Vec<NamedBytes>,
    /// Font bytes.
    pub fonts: Vec<NamedBytes>,
    /// Explicit limits. There is no fallback budget.
    pub budgets: HudBudgets,
    /// Plain text, or a caller markup adapter. Not `menu_text::Label`.
    pub markup: HudMarkup,
    /// FDF member the caller selected. Package 06 can read this on each frame.
    pub source_member: String,
    /// Admitted map receipt path. Empty unless [`crate::hud_render`] filled it.
    pub receipt_path: String,
    /// Admitted binding revision. Zero is a real revision, not a missing one.
    pub binding_revision: u64,
}

/// Placement as percentages of the parent frame, or of the viewport for a root.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativeBox {
    /// Left edge, percent of the parent width.
    pub left: f32,
    /// Top edge, percent of the parent height, Y down.
    pub top: f32,
    /// Width, percent of the parent width.
    pub width: f32,
    /// Height, percent of the parent height.
    pub height: f32,
}

/// Pixel crop of a decoded image. `min_y` is the top of that image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvRect {
    /// Left pixel.
    pub min_x: f32,
    /// Top pixel.
    pub min_y: f32,
    /// Right pixel.
    pub max_x: f32,
    /// Bottom pixel.
    pub max_y: f32,
}

/// One image layer inside a frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedArt {
    /// Resolved path the bytes were stored under.
    pub path: String,
    /// Placement inside the owning frame.
    pub relative: RelativeBox,
    /// Crop of the decoded image. The range is absolute; reflection is `flip_x` / `flip_y`.
    pub uv: UvRect,
    /// Authored `TexCoord` runs right-to-left.
    pub flip_x: bool,
    /// Authored `TexCoord` runs bottom-to-top of the decoded image.
    pub flip_y: bool,
    /// Authored `AlphaMode`, when the block has one.
    /// `ALPHAKEY` is a source profile. It is not a colour cutoff.
    pub alpha_mode: Option<String>,
}

/// One labelled run.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedSpan {
    /// Visible characters.
    pub text: String,
    /// Encoded run colour. `None` uses the frame `FontColor`, not a hotkey colour.
    pub colour: Option<[f32; 4]>,
}

/// A text frame's font, colours, and runs.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedText {
    /// Face named by `FrameFont`.
    pub face: String,
    /// Skin file the face resolved to.
    pub font_path: String,
    /// `FrameFont` height as a percentage of the viewport height.
    pub height_vh: f32,
    /// `FontColor`.
    pub colour: [f32; 4],
    /// `FontHighlightColor`.
    pub highlight: [f32; 4],
    /// `FontShadowColor`.
    pub shadow: [f32; 4],
    /// Shadow offset as a fraction of the viewport, Y down.
    pub shadow_fraction: [f32; 2],
    /// Horizontal justification.
    pub justify_h: JustifyH,
    /// Vertical justification.
    pub justify_v: JustifyV,
    /// `FontJustificationOffset` as viewport percentages, Y down, when authored.
    pub padding_vw_vh: Option<[f32; 2]>,
    /// Word wrap when the frame authors a width.
    pub wrap: bool,
    /// Runs in authored order. Empty when the authored label is blank.
    pub spans: Vec<PlannedSpan>,
    /// Markup the adapter reported and did not recolour.
    pub notes: Vec<String>,
}

/// One derived frame ready to spawn.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedFrame {
    /// Authored name.
    pub name: String,
    /// Caller role when this frame is a supplied root.
    pub role: Option<String>,
    /// Parent frame name when that parent is in the derived layout.
    pub parent: Option<String>,
    /// Placement in the parent, or in the viewport when `parent` is absent.
    pub relative: RelativeBox,
    /// Backdrop and texture layers.
    pub art: Vec<PlannedArt>,
    /// Text, for a `TEXT` frame.
    pub text: Option<PlannedText>,
}

/// A decoded image the renderer uploads once per successful candidate.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    /// Path [`PlannedArt::path`] names.
    pub path: String,
    /// Decoded pixels.
    pub texture: pcx_gfx::LoadedTexture,
}

/// Font bytes kept under the skin path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFont {
    /// Skin file.
    pub path: String,
    /// Bytes the caller supplied for that file.
    pub bytes: Vec<u8>,
}

/// A candidate that can replace the mounted HUD.
#[derive(Debug, Clone)]
pub struct ResolvedHud {
    /// [`HudInput::source`] identity.
    pub source: String,
    /// Section order that produced the bindings.
    pub race_sections: Vec<String>,
    /// Role and frame pairs, in caller order.
    pub roots: Vec<(String, String)>,
    /// Viewport the layout used.
    pub viewport: (f32, f32),
    /// Frames in derivation order.
    pub frames: Vec<PlannedFrame>,
    /// Images the frames reference.
    pub images: Vec<DecodedImage>,
    /// Fonts the text frames reference.
    pub fonts: Vec<ResolvedFont>,
    /// Caller FDF member copied onto each spawned frame.
    pub source_member: String,
    /// Admitted map path when the admission consumer built this candidate.
    pub receipt_path: String,
    /// Admitted binding revision.
    pub binding_revision: u64,
}

/// Resolves `input` to a spawn plan. Does not modify a world.
///
/// # Errors
///
/// [`HudError`] when the input, FDF, skin, art, or font data cannot be drawn
/// without inventing a value.
pub fn resolve(input: &HudInput) -> Result<ResolvedHud, HudError> {
    validate(input)?;
    let input_bytes = account_input(input)?;
    let mut fdf = Fdf::default();
    for text in &input.fdf_texts {
        fdf.parse_into(text)
            .map_err(|error| HudError::Fdf(Box::new(error)))?;
    }
    let skin =
        SkinTable::parse(&input.skin_text).map_err(|error| HudError::Skin(Box::new(error)))?;
    let root_names: Vec<&str> = input.roots.iter().map(|root| root.frame.as_str()).collect();
    let layout = crate::hud::derive(&fdf, &root_names, input.viewport)
        .map_err(|error| HudError::Fdf(Box::new(error)))?;
    let images = byte_index(&input.images)?;
    let fonts = byte_index(&input.fonts)?;
    let mut sizes: HashMap<String, (u32, u32)> = HashMap::new();
    let sources = FrameSources {
        input,
        fdf: &fdf,
        skin: &skin,
        layout: &layout,
        images: &images,
        fonts: &fonts,
    };
    let mut frames = Vec::new();
    for frame in &layout.frames {
        frames.push(plan_frame(&sources, frame, &mut sizes)?);
    }
    let decoded_bytes = account_pixels(input, &sizes)?;
    account_fonts(input, &fonts, &frames)?;
    within(
        "retained",
        "<retained>",
        decoded_bytes.saturating_add(font_total(&fonts, &frames)?),
        input.budgets.max_retained_bytes,
    )?;
    within(
        "transient",
        "<transient>",
        input_bytes.saturating_add(decoded_bytes),
        input.budgets.max_transient_bytes,
    )?;
    let decoded = decode_images(input, &images, &sizes)?;
    let font_bytes = copy_fonts(&fonts, &frames)?;
    Ok(ResolvedHud {
        source: input.source.identity.clone(),
        race_sections: input.race_sections.clone(),
        roots: input
            .roots
            .iter()
            .map(|root| (root.role.clone(), root.frame.clone()))
            .collect(),
        viewport: input.viewport,
        frames,
        images: decoded,
        fonts: font_bytes,
        source_member: input.source_member.clone(),
        receipt_path: input.receipt_path.clone(),
        binding_revision: input.binding_revision,
    })
}

/// Percentages of `inner` inside `outer`. Y-up rectangles become a Y-down top.
pub fn relative_box(inner: Rect, outer: Rect) -> Result<RelativeBox, HudError> {
    if !(outer.width > 0.0 && outer.height > 0.0) {
        return Err(HudError::Input(format!(
            "parent extent {outer:?} cannot host a child"
        )));
    }
    let relative = RelativeBox {
        left: (inner.left - outer.left) / outer.width * 100.0,
        top: (outer.top() - inner.top()) / outer.height * 100.0,
        width: inner.width / outer.width * 100.0,
        height: inner.height / outer.height * 100.0,
    };
    if ![relative.left, relative.top, relative.width, relative.height]
        .into_iter()
        .all(|value| value.is_finite())
        || !(relative.width > 0.0 && relative.height > 0.0)
    {
        return Err(HudError::Unsupported {
            frame: format!("{inner:?}"),
            property: "rectangle",
            value: format!("{relative:?}"),
        });
    }
    Ok(relative)
}

fn validate(input: &HudInput) -> Result<(), HudError> {
    if input.source.identity.is_empty() {
        return Err(HudError::Input("source identity is empty".into()));
    }
    if input.source_member.is_empty() {
        return Err(HudError::Input("source member is empty".into()));
    }
    if input.fdf_texts.is_empty() {
        return Err(HudError::Input("no FDF text".into()));
    }
    if input.roots.is_empty() {
        return Err(HudError::Input("no root roles".into()));
    }
    let mut roles = std::collections::HashSet::new();
    let mut frames = std::collections::HashSet::new();
    for root in &input.roots {
        if root.role.is_empty() || root.frame.is_empty() {
            return Err(HudError::Input("a root role or frame name is empty".into()));
        }
        if !roles.insert(root.role.clone()) {
            return Err(HudError::Input(format!(
                "duplicate root role {}",
                root.role
            )));
        }
        if !frames.insert(root.frame.clone()) {
            return Err(HudError::Input(format!(
                "duplicate root frame {}",
                root.frame
            )));
        }
    }
    if input.race_sections.iter().any(|section| section.is_empty()) {
        return Err(HudError::Input("a race section name is empty".into()));
    }
    if input
        .decorate_suffixes
        .iter()
        .any(|suffix| suffix.is_empty())
    {
        return Err(HudError::Input("a decorate suffix is empty".into()));
    }
    Ok(())
}

fn byte_index(rows: &[NamedBytes]) -> Result<HashMap<&str, &[u8]>, HudError> {
    let mut index = HashMap::new();
    for row in rows {
        if row.path.is_empty() {
            return Err(HudError::Input("an asset path is empty".into()));
        }
        if index
            .insert(row.path.as_str(), row.bytes.as_slice())
            .is_some()
        {
            return Err(HudError::Input(format!(
                "duplicate asset path {}",
                row.path
            )));
        }
    }
    Ok(index)
}

#[derive(Clone, Copy)]
struct FrameSources<'a> {
    input: &'a HudInput,
    fdf: &'a Fdf,
    skin: &'a SkinTable,
    layout: &'a HudLayout,
    images: &'a HashMap<&'a str, &'a [u8]>,
    fonts: &'a HashMap<&'a str, &'a [u8]>,
}

fn plan_frame(
    sources: &FrameSources<'_>,
    frame: &HudFrame,
    sizes: &mut HashMap<String, (u32, u32)>,
) -> Result<PlannedFrame, HudError> {
    let FrameSources {
        input,
        fdf,
        skin,
        layout,
        images,
        fonts,
    } = *sources;
    if !matches!(
        frame.kind.as_str(),
        "FRAME" | "SIMPLEFRAME" | "BACKDROP" | "TEXT"
    ) {
        return Err(HudError::Unsupported {
            frame: frame.name.clone(),
            property: "frame kind",
            value: frame.kind.clone(),
        });
    }
    for (key, _) in &frame.records {
        if !allowed_record(&frame.kind, key) {
            return Err(HudError::Unsupported {
                frame: frame.name.clone(),
                property: "record",
                value: key.clone(),
            });
        }
    }
    let parent = frame
        .parent
        .clone()
        .filter(|name| layout.frame(name).is_some());
    let outer = match parent.as_deref().and_then(|name| layout.frame(name)) {
        Some(parent) => parent.rect,
        None => Rect {
            left: 0.0,
            bottom: 0.0,
            width: input.viewport.0,
            height: input.viewport.1,
        },
    };
    let relative = relative_box(frame.rect, outer)?;
    let mut art = plan_textures(input, fdf, skin, frame, images, sizes)?;
    if frame.kind == "BACKDROP" {
        art.extend(plan_backdrop(input, fdf, skin, frame, images, sizes)?);
        if art.is_empty() && frame.children.is_empty() {
            return Err(HudError::Missing {
                frame: frame.name.clone(),
                property: "backdrop art",
            });
        }
    }
    let text = plan_text(input, fdf, skin, frame, fonts)?;
    Ok(PlannedFrame {
        name: frame.name.clone(),
        role: input
            .roots
            .iter()
            .find(|root| root.frame == frame.name)
            .map(|root| root.role.clone()),
        parent,
        relative,
        art,
        text,
    })
}

fn allowed_record(kind: &str, key: &str) -> bool {
    const COMMON: &[&str] = &[
        "Width",
        "Height",
        "SetPoint",
        "SetAllPoints",
        "DecorateFileNames",
    ];
    if COMMON.contains(&key) {
        return true;
    }
    match kind {
        "BACKDROP" => matches!(
            key,
            "BackdropBackground"
                | "BackdropTileBackground"
                | "BackdropBackgroundSize"
                | "BackdropMirrored"
                | "BackdropBackgroundInsets"
                | "BackdropEdgeFile"
                | "BackdropCornerSize"
                | "BackdropCornerFlags"
        ),
        "TEXT" => matches!(
            key,
            "Text"
                | "FrameFont"
                | "FontColor"
                | "FontHighlightColor"
                | "FontDisabledColor"
                | "FontShadowColor"
                | "FontShadowOffset"
                | "FontJustificationH"
                | "FontJustificationV"
                | "FontJustificationOffset"
                | "FontFlags"
        ),
        _ => false,
    }
}

fn plan_textures(
    input: &HudInput,
    fdf: &Fdf,
    skin: &SkinTable,
    frame: &HudFrame,
    images: &HashMap<&str, &[u8]>,
    sizes: &mut HashMap<String, (u32, u32)>,
) -> Result<Vec<PlannedArt>, HudError> {
    let Some(source) = fdf.frame(&frame.name) else {
        return Err(HudError::Fdf(Box::new(FdfError::UnknownFrame(
            frame.name.clone(),
        ))));
    };
    let decorate = fdf.flag(&frame.name, "DecorateFileNames");
    let mut art = Vec::new();
    for block in &source.textures {
        for (key, _) in &block.records {
            if !matches!(
                key.as_str(),
                "File" | "Width" | "Height" | "TexCoord" | "AlphaMode" | "Anchor"
            ) {
                return Err(HudError::Unsupported {
                    frame: frame.name.clone(),
                    property: "texture record",
                    value: key.clone(),
                });
            }
        }
        let file = required_texture(&frame.name, &block.records, "File")?;
        let width = positive(
            &frame.name,
            "Width",
            &required_texture(&frame.name, &block.records, "Width")?,
        )?;
        let height = positive(
            &frame.name,
            "Height",
            &required_texture(&frame.name, &block.records, "Height")?,
        )?;
        let (point, x, y) = parse_anchor(
            &frame.name,
            &required_texture(&frame.name, &block.records, "Anchor")?,
        )?;
        let alpha = match optional_texture(&frame.name, &block.records, "AlphaMode")? {
            None => None,
            Some(value) => {
                let mode = unquote(&value);
                // Warsmash TextureFrame special-cases ADD as a blend frame.
                // ALPHAKEY and BLEND are ordinary texture draws. The ALPHAKEY
                // name is not a colour key and is not a texel cutoff.
                if mode != "BLEND" && mode != "ALPHAKEY" {
                    return Err(HudError::Unsupported {
                        frame: frame.name.clone(),
                        property: "AlphaMode",
                        value: mode,
                    });
                }
                Some(mode)
            }
        };
        let path = resolve_art_path(
            skin,
            &unquote(&file),
            &input.race_sections,
            decorate,
            &input.decorate_suffixes,
            images,
        )?;
        let (image_width, image_height) = probe_header(input, &path, images, sizes)?;
        let (uv, flip_x, flip_y) = match optional_texture(&frame.name, &block.records, "TexCoord")?
        {
            None => (full_uv(image_width, image_height), false, false),
            Some(value) => uv_rect(&frame.name, &value, image_width, image_height)?,
        };
        let placed = anchored_rect(frame.rect, point, x, y, width, height);
        art.push(PlannedArt {
            path,
            relative: relative_box(placed, frame.rect)?,
            uv,
            flip_x,
            flip_y,
            alpha_mode: alpha,
        });
    }
    Ok(art)
}

fn plan_backdrop(
    input: &HudInput,
    fdf: &Fdf,
    skin: &SkinTable,
    frame: &HudFrame,
    images: &HashMap<&str, &[u8]>,
    sizes: &mut HashMap<String, (u32, u32)>,
) -> Result<Vec<PlannedArt>, HudError> {
    let art =
        glue_menu::backdrop(fdf, &frame.name).map_err(|error| HudError::Fdf(Box::new(error)))?;
    let Some(art) = art else {
        return Ok(Vec::new());
    };
    if art.edge.is_some() || art.tile || art.mirrored || art.background_size.is_some() {
        return Err(HudError::Unsupported {
            frame: frame.name.clone(),
            property: "backdrop",
            value: "edge, tile, mirror, or background size".into(),
        });
    }
    let Some(token) = art.background.filter(|value| !value.is_empty()) else {
        return Ok(Vec::new());
    };
    let inset = art.inset.unwrap_or(0.0);
    if !inset.is_finite() || inset < 0.0 {
        return Err(HudError::Unsupported {
            frame: frame.name.clone(),
            property: "BackdropBackgroundInsets",
            value: inset.to_string(),
        });
    }
    let inner = Rect {
        left: frame.rect.left + inset,
        bottom: frame.rect.bottom + inset,
        width: frame.rect.width - 2.0 * inset,
        height: frame.rect.height - 2.0 * inset,
    };
    let path = resolve_art_path(
        skin,
        &token,
        &input.race_sections,
        fdf.flag(&frame.name, "DecorateFileNames"),
        &input.decorate_suffixes,
        images,
    )?;
    let (width, height) = probe_header(input, &path, images, sizes)?;
    Ok(vec![PlannedArt {
        path,
        relative: relative_box(inner, frame.rect)?,
        uv: full_uv(width, height),
        flip_x: false,
        flip_y: false,
        alpha_mode: None,
    }])
}

fn plan_text(
    input: &HudInput,
    fdf: &Fdf,
    skin: &SkinTable,
    frame: &HudFrame,
    fonts: &HashMap<&str, &[u8]>,
) -> Result<Option<PlannedText>, HudError> {
    if frame.kind != "TEXT" {
        return Ok(None);
    }
    let raw = fdf
        .text_value(&frame.name)
        .ok_or_else(|| HudError::Missing {
            frame: frame.name.clone(),
            property: "Text",
        })?;
    let literal = if input.global_strings.is_empty() {
        raw
    } else {
        menu_text::global_string(&input.global_strings, &raw).ok_or_else(|| HudError::Missing {
            frame: frame.name.clone(),
            property: "global string",
        })?
    };
    let (spans, notes) = if literal.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        read_markup(&input.markup, &frame.name, &literal)?
    };
    let texts: Vec<&str> = input.fdf_texts.iter().map(String::as_str).collect();
    let style = menu_text::text_style_in(&texts, &frame.name)
        .map_err(|error| HudError::Text(error.to_string()))?;
    if style.fixed_size {
        return Err(HudError::Unsupported {
            frame: frame.name.clone(),
            property: "FontFlags",
            value: "FIXEDSIZE".into(),
        });
    }
    let binding =
        skin_file(skin, &style.font, &input.race_sections)?.ok_or_else(|| HudError::Font {
            path: style.font.clone(),
            detail: "not defined in the caller section order".into(),
        })?;
    if !fonts.contains_key(binding.file.as_str()) {
        return Err(HudError::Font {
            path: binding.file.clone(),
            detail: "no supplied bytes".into(),
        });
    }
    let height_vh = style.height / input.viewport.1 * 100.0;
    let shadow_fraction = [
        style.shadow_offset[0] / input.viewport.0,
        -style.shadow_offset[1] / input.viewport.1,
    ];
    if !height_vh.is_finite() || shadow_fraction.iter().any(|value| !value.is_finite()) {
        return Err(HudError::Unsupported {
            frame: frame.name.clone(),
            property: "FrameFont",
            value: format!("{height_vh} {shadow_fraction:?}"),
        });
    }
    let padding_vw_vh = match fdf
        .numbers(&frame.name, "FontJustificationOffset")
        .map_err(|error| HudError::Fdf(Box::new(error)))?
    {
        None => None,
        Some(value) => {
            let [x, y] = <[f32; 2]>::try_from(value).map_err(|_| HudError::Unsupported {
                frame: frame.name.clone(),
                property: "FontJustificationOffset",
                value: "expected two numbers".into(),
            })?;
            if !x.is_finite() || !y.is_finite() {
                return Err(HudError::Unsupported {
                    frame: frame.name.clone(),
                    property: "FontJustificationOffset",
                    value: format!("{x} {y}"),
                });
            }
            Some([x / input.viewport.0 * 100.0, -y / input.viewport.1 * 100.0])
        }
    };
    Ok(Some(PlannedText {
        face: style.font,
        font_path: binding.file,
        height_vh,
        colour: style.colour,
        highlight: style.highlight,
        shadow: style.shadow_colour,
        shadow_fraction,
        justify_h: style.justify_h,
        justify_v: style.justify_v,
        padding_vw_vh,
        wrap: frame.width.is_some(),
        spans,
        notes,
    }))
}

fn read_markup(
    markup: &HudMarkup,
    frame: &str,
    literal: &str,
) -> Result<(Vec<PlannedSpan>, Vec<String>), HudError> {
    let marked = literal.contains('|') || literal.contains("^^");
    match markup {
        HudMarkup::Plain if marked => Err(HudError::Unsupported {
            frame: frame.to_string(),
            property: "markup",
            value: literal.to_string(),
        }),
        HudMarkup::Plain => Ok((
            vec![PlannedSpan {
                text: literal.to_string(),
                colour: None,
            }],
            Vec::new(),
        )),
        HudMarkup::Adapter(parse) => {
            let parsed =
                parse(literal).map_err(|detail| HudError::Text(format!("{frame}: {detail}")))?;
            if parsed.spans.is_empty() {
                return Err(HudError::Text(format!(
                    "{frame}: markup adapter returned no runs"
                )));
            }
            Ok((
                parsed
                    .spans
                    .into_iter()
                    .map(|span| PlannedSpan {
                        text: span.text,
                        colour: span.colour,
                    })
                    .collect(),
                parsed.notes,
            ))
        }
    }
}

fn skin_file(
    table: &SkinTable,
    key: &str,
    order: &[String],
) -> Result<Option<FontBinding>, HudError> {
    for section in order {
        let values: Vec<String> = table
            .sections()
            .iter()
            .filter(|candidate| candidate.name == *section)
            .flat_map(|candidate| candidate.entries.iter())
            .filter(|(entry, _)| entry == key)
            .map(|(_, value)| value.clone())
            .collect();
        match values.as_slice() {
            [] => continue,
            [file] if file.is_empty() => {
                return Err(HudError::Skin(Box::new(SkinError::EmptyValue {
                    section: section.clone(),
                    face: key.to_string(),
                })));
            }
            [file] => {
                return Ok(Some(FontBinding {
                    file: file.clone(),
                    section: section.clone(),
                }));
            }
            _ => {
                return Err(HudError::Skin(Box::new(SkinError::Duplicate {
                    section: section.clone(),
                    face: key.to_string(),
                    values,
                })));
            }
        }
    }
    Ok(None)
}

fn resolve_art_path(
    skin: &SkinTable,
    token: &str,
    order: &[String],
    decorate: bool,
    suffixes: &[String],
    images: &HashMap<&str, &[u8]>,
) -> Result<String, HudError> {
    if token.is_empty() {
        return Err(HudError::Image {
            path: token.to_string(),
            detail: "empty file name".into(),
        });
    }
    let base = skin_file(skin, token, order)?
        .map(|binding| binding.file)
        .unwrap_or_else(|| token.to_string());
    if images.contains_key(base.as_str()) {
        return Ok(base);
    }
    if !decorate || base.contains('.') {
        return Err(HudError::Image {
            path: base,
            detail: "no supplied bytes".into(),
        });
    }
    let matches: Vec<String> = suffixes
        .iter()
        .map(|suffix| format!("{base}{suffix}"))
        .filter(|candidate| images.contains_key(candidate.as_str()))
        .collect();
    match matches.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(HudError::Image {
            path: base,
            detail: "no supplied bytes for a DecorateFileNames suffix".into(),
        }),
        _ => Err(HudError::Image {
            path: base,
            detail: format!("more than one decorate suffix matches {matches:?}"),
        }),
    }
}

fn probe_header(
    input: &HudInput,
    path: &str,
    images: &HashMap<&str, &[u8]>,
    sizes: &mut HashMap<String, (u32, u32)>,
) -> Result<(u32, u32), HudError> {
    if let Some(size) = sizes.get(path) {
        return Ok(*size);
    }
    let bytes = images.get(path).ok_or_else(|| HudError::Image {
        path: path.to_string(),
        detail: "no supplied bytes".into(),
    })?;
    within(
        "image-input",
        path,
        bytes.len(),
        input.budgets.max_image_input_bytes,
    )?;
    let (width, height) = image_header(path, bytes)?;
    let rgba = rgba_len(path, width, height)?;
    within(
        "image-decoded",
        path,
        rgba,
        input.budgets.max_image_decoded_bytes,
    )?;
    sizes.insert(path.to_string(), (width, height));
    Ok((width, height))
}

fn image_header(path: &str, bytes: &[u8]) -> Result<(u32, u32), HudError> {
    let unsupported = |detail: String| HudError::Image {
        path: path.to_string(),
        detail: format!("unsupported header: {detail}"),
    };
    match pcx_gfx::AssetKind::detect(bytes) {
        pcx_gfx::AssetKind::Blp => {
            let header =
                pcx_blp::Header::parse(bytes).map_err(|error| unsupported(error.to_string()))?;
            if header.width == 0 || header.height == 0 {
                return Err(unsupported("zero extent".into()));
            }
            Ok((header.width, header.height))
        }
        pcx_gfx::AssetKind::Tga => {
            let header =
                pcx_tga::Header::parse(bytes).map_err(|error| unsupported(error.to_string()))?;
            if !matches!(
                header.image_type,
                pcx_tga::ImageType::UncompressedTrueColour | pcx_tga::ImageType::RleTrueColour
            ) {
                return Err(unsupported(format!("{:?}", header.image_type)));
            }
            if header.width == 0 || header.height == 0 {
                return Err(unsupported("zero extent".into()));
            }
            Ok((u32::from(header.width), u32::from(header.height)))
        }
        other => Err(unsupported(format!("{other:?}"))),
    }
}

fn rgba_len(path: &str, width: u32, height: u32) -> Result<usize, HudError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| usize::try_from(height).ok()?.checked_mul(width))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| HudError::Budget {
            scope: "image-decoded",
            path: path.to_string(),
            actual: usize::MAX,
            limit: 0,
        })
}

fn account_input(input: &HudInput) -> Result<usize, HudError> {
    let mut total = 0usize;
    for text in &input.fdf_texts {
        add_bytes(
            &mut total,
            "input",
            "<fdf>",
            text.len(),
            input.budgets.max_input_bytes,
        )?;
    }
    add_bytes(
        &mut total,
        "input",
        "<skin>",
        input.skin_text.len(),
        input.budgets.max_input_bytes,
    )?;
    add_bytes(
        &mut total,
        "input",
        "<globals>",
        input.global_strings.len(),
        input.budgets.max_input_bytes,
    )?;
    let mut fonts = 0usize;
    for font in &input.fonts {
        within(
            "font",
            &font.path,
            font.bytes.len(),
            input.budgets.max_font_bytes,
        )?;
        add_bytes(
            &mut fonts,
            "font-aggregate",
            &font.path,
            font.bytes.len(),
            input.budgets.max_font_aggregate_bytes,
        )?;
        add_bytes(
            &mut total,
            "input",
            &font.path,
            font.bytes.len(),
            input.budgets.max_input_bytes,
        )?;
    }
    for image in &input.images {
        within(
            "image-input",
            &image.path,
            image.bytes.len(),
            input.budgets.max_image_input_bytes,
        )?;
        add_bytes(
            &mut total,
            "input",
            &image.path,
            image.bytes.len(),
            input.budgets.max_input_bytes,
        )?;
    }
    Ok(total)
}

fn account_pixels(
    input: &HudInput,
    sizes: &HashMap<String, (u32, u32)>,
) -> Result<usize, HudError> {
    let mut total = 0usize;
    for (path, (width, height)) in sizes {
        let rgba = rgba_len(path, *width, *height)?;
        within(
            "image-decoded",
            path,
            rgba,
            input.budgets.max_image_decoded_bytes,
        )?;
        add_bytes(
            &mut total,
            "decoded",
            path,
            rgba,
            input.budgets.max_decoded_bytes,
        )?;
    }
    Ok(total)
}

fn account_fonts(
    input: &HudInput,
    fonts: &HashMap<&str, &[u8]>,
    frames: &[PlannedFrame],
) -> Result<(), HudError> {
    let mut total = 0usize;
    let mut seen = std::collections::HashSet::new();
    for frame in frames {
        let Some(text) = &frame.text else {
            continue;
        };
        if !seen.insert(text.font_path.as_str()) {
            continue;
        }
        let bytes = fonts
            .get(text.font_path.as_str())
            .ok_or_else(|| HudError::Font {
                path: text.font_path.clone(),
                detail: "no supplied bytes".into(),
            })?;
        within(
            "font",
            &text.font_path,
            bytes.len(),
            input.budgets.max_font_bytes,
        )?;
        add_bytes(
            &mut total,
            "font-aggregate",
            &text.font_path,
            bytes.len(),
            input.budgets.max_font_aggregate_bytes,
        )?;
    }
    Ok(())
}

fn font_total(fonts: &HashMap<&str, &[u8]>, frames: &[PlannedFrame]) -> Result<usize, HudError> {
    let mut total = 0usize;
    let mut seen = std::collections::HashSet::new();
    for frame in frames {
        let Some(text) = &frame.text else {
            continue;
        };
        if !seen.insert(text.font_path.as_str()) {
            continue;
        }
        let bytes = fonts
            .get(text.font_path.as_str())
            .ok_or_else(|| HudError::Font {
                path: text.font_path.clone(),
                detail: "no supplied bytes".into(),
            })?;
        total = total.saturating_add(bytes.len());
    }
    Ok(total)
}

fn decode_images(
    input: &HudInput,
    images: &HashMap<&str, &[u8]>,
    sizes: &HashMap<String, (u32, u32)>,
) -> Result<Vec<DecodedImage>, HudError> {
    let mut decoded = Vec::new();
    for path in sizes.keys() {
        let bytes = images.get(path.as_str()).ok_or_else(|| HudError::Image {
            path: path.clone(),
            detail: "no supplied bytes".into(),
        })?;
        let texture =
            pcx_gfx::load_image_bounded(bytes, path, input.budgets.max_image_decoded_bytes)
                .map_err(|error| HudError::Image {
                    path: path.clone(),
                    detail: error.to_string(),
                })?;
        within(
            "image-decoded",
            path,
            texture.rgba.len(),
            input.budgets.max_image_decoded_bytes,
        )?;
        decoded.push(DecodedImage {
            path: path.clone(),
            texture,
        });
    }
    Ok(decoded)
}

fn copy_fonts(
    fonts: &HashMap<&str, &[u8]>,
    frames: &[PlannedFrame],
) -> Result<Vec<ResolvedFont>, HudError> {
    let mut copied = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for frame in frames {
        let Some(text) = &frame.text else {
            continue;
        };
        if !seen.insert(text.font_path.clone()) {
            continue;
        }
        let bytes = fonts
            .get(text.font_path.as_str())
            .ok_or_else(|| HudError::Font {
                path: text.font_path.clone(),
                detail: "no supplied bytes".into(),
            })?;
        copied.push(ResolvedFont {
            path: text.font_path.clone(),
            bytes: bytes.to_vec(),
        });
    }
    Ok(copied)
}

fn within(scope: &'static str, path: &str, actual: usize, limit: usize) -> Result<(), HudError> {
    if actual > limit {
        Err(HudError::Budget {
            scope,
            path: path.to_string(),
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

fn add_bytes(
    total: &mut usize,
    scope: &'static str,
    path: &str,
    bytes: usize,
    limit: usize,
) -> Result<(), HudError> {
    let sum = total.checked_add(bytes).ok_or_else(|| HudError::Budget {
        scope,
        path: path.to_string(),
        actual: usize::MAX,
        limit,
    })?;
    within(scope, path, sum, limit)?;
    *total = sum;
    Ok(())
}

fn required_texture(
    frame: &str,
    records: &[(String, String)],
    key: &'static str,
) -> Result<String, HudError> {
    optional_texture(frame, records, key)?.ok_or_else(|| HudError::Missing {
        frame: frame.to_string(),
        property: key,
    })
}

fn optional_texture(
    frame: &str,
    records: &[(String, String)],
    key: &str,
) -> Result<Option<String>, HudError> {
    let mut found = None;
    for (record, value) in records {
        if record == key {
            if found.is_some() {
                return Err(HudError::Unsupported {
                    frame: frame.to_string(),
                    property: "texture record",
                    value: format!("duplicate {key}"),
                });
            }
            found = Some(value.clone());
        }
    }
    Ok(found)
}

fn positive(frame: &str, property: &'static str, value: &str) -> Result<f32, HudError> {
    let numbers = split_floats(value).map_err(|_| HudError::Unsupported {
        frame: frame.to_string(),
        property,
        value: value.to_string(),
    })?;
    match numbers.as_slice() {
        [number] if *number > 0.0 => Ok(*number),
        _ => Err(HudError::Unsupported {
            frame: frame.to_string(),
            property,
            value: value.to_string(),
        }),
    }
}

fn parse_anchor(frame: &str, value: &str) -> Result<(Point, f32, f32), HudError> {
    let parts: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() != 3 {
        return Err(HudError::Unsupported {
            frame: frame.to_string(),
            property: "Anchor",
            value: value.to_string(),
        });
    }
    let point = parse_point(parts[0]).ok_or_else(|| HudError::Unsupported {
        frame: frame.to_string(),
        property: "Anchor",
        value: parts[0].to_string(),
    })?;
    let x = one_finite(frame, "Anchor", parts[1])?;
    let y = one_finite(frame, "Anchor", parts[2])?;
    Ok((point, x, y))
}

fn one_finite(frame: &str, property: &'static str, value: &str) -> Result<f32, HudError> {
    split_floats(value)
        .ok()
        .and_then(|numbers| match numbers.as_slice() {
            [number] => Some(*number),
            _ => None,
        })
        .ok_or_else(|| HudError::Unsupported {
            frame: frame.to_string(),
            property,
            value: value.to_string(),
        })
}

fn parse_point(token: &str) -> Option<Point> {
    Some(match token.trim().to_ascii_uppercase().as_str() {
        "TOPLEFT" => Point::TopLeft,
        "TOP" => Point::Top,
        "TOPRIGHT" => Point::TopRight,
        "LEFT" => Point::Left,
        "CENTER" => Point::Center,
        "RIGHT" => Point::Right,
        "BOTTOMLEFT" => Point::BottomLeft,
        "BOTTOM" => Point::Bottom,
        "BOTTOMRIGHT" => Point::BottomRight,
        _ => return None,
    })
}

fn anchored_rect(parent: Rect, point: Point, x: f32, y: f32, width: f32, height: f32) -> Rect {
    let (px, py) = parent.point(point);
    let (ax, ay) = (px + x, py + y);
    let (left, bottom) = match point {
        Point::TopLeft => (ax, ay - height),
        Point::Top => (ax - width / 2.0, ay - height),
        Point::TopRight => (ax - width, ay - height),
        Point::Left => (ax, ay - height / 2.0),
        Point::Center => (ax - width / 2.0, ay - height / 2.0),
        Point::Right => (ax - width, ay - height / 2.0),
        Point::BottomLeft => (ax, ay),
        Point::Bottom => (ax - width / 2.0, ay),
        Point::BottomRight => (ax - width, ay),
    };
    Rect {
        left,
        bottom,
        width,
        height,
    }
}

fn uv_rect(
    frame: &str,
    value: &str,
    width: u32,
    height: u32,
) -> Result<(UvRect, bool, bool), HudError> {
    let numbers = split_floats(value).map_err(|_| HudError::Unsupported {
        frame: frame.to_string(),
        property: "TexCoord",
        value: value.to_string(),
    })?;
    let (u0, u1, v0, v1) = match numbers.as_slice() {
        [u0, u1, v0, v1] => (*u0, *u1, *v0, *v1),
        _ => {
            return Err(HudError::Unsupported {
                frame: frame.to_string(),
                property: "TexCoord",
                value: value.to_string(),
            });
        }
    };
    if [u0, u1, v0, v1]
        .into_iter()
        .any(|value| !(0.0..=1.0).contains(&value))
    {
        return Err(HudError::Unsupported {
            frame: frame.to_string(),
            property: "TexCoord",
            value: value.to_string(),
        });
    }
    if u0 == u1 || v0 == v1 {
        return Err(HudError::Unsupported {
            frame: frame.to_string(),
            property: "TexCoord",
            value: value.to_string(),
        });
    }
    // Four numbers are u0, u1, v0, v1 in authored order. A descending pair is
    // a reflection, drawn with ImageNode flips over the absolute range.
    // v=0 is the top of the decoded image: the selected ConsoleUI pairs its
    // top-anchored blocks with a v range that starts at 0. That pairing is
    // source text, not a native pixel probe.
    let (min_u, max_u) = if u0 <= u1 { (u0, u1) } else { (u1, u0) };
    let (min_v, max_v) = if v0 <= v1 { (v0, v1) } else { (v1, v0) };
    Ok((
        UvRect {
            min_x: min_u * width as f32,
            max_x: max_u * width as f32,
            min_y: min_v * height as f32,
            max_y: max_v * height as f32,
        },
        u0 > u1,
        v0 > v1,
    ))
}

fn full_uv(width: u32, height: u32) -> UvRect {
    UvRect {
        min_x: 0.0,
        min_y: 0.0,
        max_x: width as f32,
        max_y: height as f32,
    }
}

fn split_floats(value: &str) -> Result<Vec<f32>, ()> {
    let mut numbers = Vec::new();
    for token in value.split(|c: char| c == ',' || c.is_whitespace()) {
        if token.is_empty() {
            continue;
        }
        let parsed = token
            .parse::<f32>()
            .or_else(|_| token.trim_end_matches('f').parse::<f32>())
            .map_err(|_| ())?;
        if !parsed.is_finite() {
            return Err(());
        }
        numbers.push(parsed);
    }
    Ok(numbers)
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

#[cfg(test)]
pub(crate) fn sample_hud(sections: &[&str]) -> HudInput {
    tests::fixture(sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tga(width: u16, height: u16, rgba: [u8; 4]) -> Vec<u8> {
        let mut bytes = vec![0u8; 18];
        bytes[2] = 2;
        let width_bytes = width.to_le_bytes();
        let height_bytes = height.to_le_bytes();
        bytes[12] = width_bytes[0];
        bytes[13] = width_bytes[1];
        bytes[14] = height_bytes[0];
        bytes[15] = height_bytes[1];
        bytes[16] = 32;
        bytes[17] = 8;
        let pixel = [rgba[2], rgba[1], rgba[0], rgba[3]];
        for _ in 0..usize::from(width) * usize::from(height) {
            bytes.extend(pixel);
        }
        bytes
    }

    pub(super) fn fixture(sections: &[&str]) -> HudInput {
        HudInput {
            source: SourceId {
                identity: format!("fixture:{}", sections.join(",")),
            },
            fdf_texts: vec![FIXTURE.into()],
            skin_text: SKIN.into(),
            race_sections: sections
                .iter()
                .map(|section| (*section).to_string())
                .collect(),
            roots: vec![
                RootRole {
                    role: "console".into(),
                    frame: "Console".into(),
                },
                RootRole {
                    role: "plate".into(),
                    frame: "Plate".into(),
                },
            ],
            viewport: (0.5, 0.25),
            global_strings: String::new(),
            decorate_suffixes: vec![".blp".into()],
            images: vec![
                NamedBytes {
                    path: "art/human-tile.blp".into(),
                    bytes: tga(4, 2, [1, 2, 3, 255]),
                },
                NamedBytes {
                    path: "art/orc-tile.blp".into(),
                    bytes: tga(4, 2, [4, 5, 6, 255]),
                },
                NamedBytes {
                    path: "plates/a.blp".into(),
                    bytes: tga(1, 1, [7, 8, 9, 255]),
                },
            ],
            fonts: vec![NamedBytes {
                path: "fonts/body.ttf".into(),
                bytes: bevy::text::DEFAULT_FONT_DATA.to_vec(),
            }],
            budgets: test_budgets(),
            markup: HudMarkup::Plain,
            source_member: "ui/framedef/ui/fixture.fdf".into(),
            receipt_path: String::new(),
            binding_revision: 0,
        }
    }

    fn test_budgets() -> HudBudgets {
        let font = bevy::text::DEFAULT_FONT_DATA.len();
        HudBudgets {
            max_input_bytes: font + 8_192,
            max_image_input_bytes: 4_096,
            max_font_bytes: font,
            max_font_aggregate_bytes: font,
            max_image_decoded_bytes: 4_096,
            max_decoded_bytes: 8_192,
            max_retained_bytes: font + 8_192,
            max_transient_bytes: font + 16_384,
        }
    }

    const FIXTURE: &str = r#"
Frame "SIMPLEFRAME" "Console" {
    DecorateFileNames,
    Texture {
        File "Tile",
        Width 0.125,
        Height 0.0625,
        TexCoord 0, 0.5, 0, 1,
        AlphaMode "ALPHAKEY",
        Anchor BOTTOMLEFT, 0.125, 0,
    }
    Frame "TEXT" "Gold" {
        Width 0.125,
        Height 0.0625,
        SetPoint TOPLEFT, "Console", TOPLEFT, 0, 0,
        FrameFont "MasterFont", 0.03125, "",
        FontColor 1.0 0.5 0.25 1.0,
        FontHighlightColor 1 1 1 1,
        FontDisabledColor 0.25 0.25 0.25 1,
        FontShadowColor 0 0 0 1,
        FontShadowOffset 0.125 -0.0625,
        FontJustificationH JUSTIFYLEFT,
        FontJustificationV JUSTIFYTOP,
        FontFlags "",
        Text "Gold",
    }
}
Frame "BACKDROP" "Plate" {
    Width 0.125,
    Height 0.0625,
    BackdropBackground "plates/a.blp",
}
"#;

    const SKIN: &str = r#"
[Human]
Tile=art/human-tile

[Orc]
Tile=art/orc-tile

[Default]
MasterFont=fonts/body.ttf
"#;

    #[test]
    fn race_order_selects_the_tile_and_keeps_anchor_fractions() {
        let resolved = resolve(&fixture(&["Orc", "Human", "Default"])).unwrap();
        let console = resolved
            .frames
            .iter()
            .find(|frame| frame.name == "Console")
            .unwrap();
        assert_eq!(console.role.as_deref(), Some("console"));
        assert_eq!(
            console.relative,
            RelativeBox {
                left: 0.0,
                top: 0.0,
                width: 100.0,
                height: 100.0
            }
        );
        assert_eq!(console.art.len(), 1);
        assert_eq!(console.art[0].path, "art/orc-tile.blp");
        assert_eq!(console.art[0].alpha_mode.as_deref(), Some("ALPHAKEY"));
        assert_eq!(
            console.art[0].relative,
            RelativeBox {
                left: 25.0,
                top: 75.0,
                width: 25.0,
                height: 25.0
            }
        );
        assert_eq!(
            console.art[0].uv,
            UvRect {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 2.0,
                max_y: 2.0
            }
        );
        assert!(!console.art[0].flip_x);
        assert!(!console.art[0].flip_y);
        let gold = resolved
            .frames
            .iter()
            .find(|frame| frame.name == "Gold")
            .unwrap();
        assert_eq!(gold.parent.as_deref(), Some("Console"));
        assert_eq!(
            gold.relative,
            RelativeBox {
                left: 0.0,
                top: 0.0,
                width: 25.0,
                height: 25.0
            }
        );
        let text = gold.text.as_ref().unwrap();
        assert_eq!(text.font_path, "fonts/body.ttf");
        assert_eq!(text.height_vh, 12.5);
        assert_eq!(text.shadow_fraction, [0.25, 0.25]);
        assert_eq!(text.spans[0].text, "Gold");
        let plate = resolved
            .frames
            .iter()
            .find(|frame| frame.name == "Plate")
            .unwrap();
        assert_eq!(plate.art[0].path, "plates/a.blp");
        assert_eq!(
            plate.relative,
            RelativeBox {
                left: 0.0,
                top: 75.0,
                width: 25.0,
                height: 25.0
            }
        );
    }

    #[test]
    fn missing_art_unsupported_kind_and_ambiguous_suffix_are_refused() {
        let mut missing = fixture(&["Human", "Default"]);
        missing.images.clear();
        assert!(matches!(resolve(&missing), Err(HudError::Image { .. })));

        let mut sprite = fixture(&["Human", "Default"]);
        sprite.fdf_texts = vec!["Frame \"SPRITE\" \"Nope\" {\n}\n".into()];
        sprite.roots = vec![RootRole {
            role: "nope".into(),
            frame: "Nope".into(),
        }];
        assert!(matches!(
            resolve(&sprite),
            Err(HudError::Unsupported {
                property: "frame kind",
                ..
            })
        ));

        let mut suffixes = fixture(&["Human", "Default"]);
        suffixes.decorate_suffixes = vec![".blp".into(), ".tga".into()];
        suffixes.images.push(NamedBytes {
            path: "art/human-tile.tga".into(),
            bytes: tga(1, 1, [1, 1, 1, 255]),
        });
        assert!(matches!(resolve(&suffixes), Err(HudError::Image { .. })));

        let mut mode = fixture(&["Human", "Default"]);
        mode.fdf_texts[0] = mode.fdf_texts[0].replace("ALPHAKEY", "ADD");
        assert!(matches!(
            resolve(&mode),
            Err(HudError::Unsupported {
                property: "AlphaMode",
                ..
            })
        ));

        let mut crop = fixture(&["Human", "Default"]);
        crop.fdf_texts[0] =
            crop.fdf_texts[0].replace("TexCoord 0, 0.5, 0, 1,", "TexCoord 0, 0, 0, 1,");
        assert!(matches!(
            resolve(&crop),
            Err(HudError::Unsupported {
                property: "TexCoord",
                ..
            })
        ));

        let mut reflected = fixture(&["Human", "Default"]);
        reflected.fdf_texts[0] =
            reflected.fdf_texts[0].replace("TexCoord 0, 0.5, 0, 1,", "TexCoord 0.5, 0, 1, 0,");
        let resolved = resolve(&reflected).unwrap();
        let tile = &resolved
            .frames
            .iter()
            .find(|frame| frame.name == "Console")
            .unwrap()
            .art[0];
        assert!(tile.flip_x);
        assert!(tile.flip_y);
        assert_eq!(
            tile.uv,
            UvRect {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 2.0,
                max_y: 2.0
            }
        );
    }

    #[test]
    fn header_budget_refuses_before_a_short_pixel_payload_is_decoded() {
        let mut input = fixture(&["Human", "Default"]);
        input.budgets.max_image_decoded_bytes = 8;
        input.images[0].bytes = tga_header(64, 64);
        let error = resolve(&input).unwrap_err();
        assert!(
            matches!(
                error,
                HudError::Budget {
                    scope: "image-decoded",
                    ..
                }
            ),
            "{error}"
        );
        let text = error.to_string();
        assert!(!text.contains("image data ended"), "{text}");
        assert!(!text.contains("wants"), "{text}");
    }

    #[test]
    fn unsupported_header_is_reported_without_a_decode_error() {
        let mut input = fixture(&["Human", "Default"]);
        input.images[0].bytes = b"DDS \x00\x00".to_vec();
        let error = resolve(&input).unwrap_err();
        let HudError::Image { detail, .. } = error else {
            panic!("expected an image error");
        };
        assert!(detail.contains("unsupported header"), "{detail}");
    }

    #[test]
    fn blank_text_is_kept_and_menu_markup_is_not_recoloured() {
        let mut blank = fixture(&["Human", "Default"]);
        blank.fdf_texts[0] = blank.fdf_texts[0].replace("Text \"Gold\",", "Text \"\",");
        let resolved = resolve(&blank).unwrap();
        let gold = resolved
            .frames
            .iter()
            .find(|frame| frame.name == "Gold")
            .unwrap();
        let text = gold.text.as_ref().unwrap();
        assert!(text.spans.is_empty());
        assert!(text.notes.is_empty());

        let mut marked = fixture(&["Human", "Default"]);
        marked.fdf_texts[0] =
            marked.fdf_texts[0].replace("Text \"Gold\",", "Text \"|cffff0000Gold|r\",");
        let error = resolve(&marked).unwrap_err();
        assert!(
            matches!(
                error,
                HudError::Unsupported {
                    property: "markup",
                    ..
                }
            ),
            "{error}"
        );

        marked.markup = HudMarkup::Adapter(|raw| {
            assert!(raw.contains("|cffff0000"));
            Ok(HudMarkupText {
                spans: vec![HudMarkupSpan {
                    text: "Gold".into(),
                    colour: Some([1.0, 0.0, 0.0, 1.0]),
                }],
                notes: vec!["adapter-note".into()],
            })
        });
        let resolved = resolve(&marked).unwrap();
        let text = resolved
            .frames
            .iter()
            .find(|frame| frame.name == "Gold")
            .unwrap()
            .text
            .as_ref()
            .unwrap();
        assert_eq!(text.spans[0].colour, Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(text.notes, vec!["adapter-note".to_string()]);
        assert_ne!(text.spans[0].colour, Some(text.highlight));
    }

    fn tga_header(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 18];
        bytes[2] = 2;
        let width_bytes = width.to_le_bytes();
        let height_bytes = height.to_le_bytes();
        bytes[12] = width_bytes[0];
        bytes[13] = width_bytes[1];
        bytes[14] = height_bytes[0];
        bytes[15] = height_bytes[1];
        bytes[16] = 32;
        bytes[17] = 8;
        bytes
    }
}
