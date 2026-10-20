//! Transmission frame geometry and artwork from the selected CinematicPanel FDF.
use crate::{
    fdf::{Fdf, Rect},
    glue_menu::{self, BackdropImage, EdgePiece},
    menu_skin::SkinTable,
    menu_text::TextStyle,
};
use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use peacecraft3::map_binding::{AssetReceipt, receipt};
use std::collections::HashMap;

const PANEL: &str = "UI\\FrameDef\\UI\\CinematicPanel.fdf";
const TEMPLATES: &str = "UI\\FrameDef\\UI\\EscMenuTemplates.fdf";
const SKIN: &str = "UI\\war3skins.txt";
const SPEAKER: &str = "CinematicSpeakerText";
const DIALOGUE: &str = "CinematicDialogueText";
const VIEW: (f32, f32) = (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H);

pub(super) struct DialogueAssets {
    pub receipts: Vec<AssetReceipt>,
    pub unsupported: Vec<String>,
    fdf: Fdf,
    styles: [TextStyle; 2],
    fonts: HashMap<String, Handle<Font>>,
    images: HashMap<(String, bool), Handle<Image>>,
    backdrops: Vec<(Rect, Vec<Layer>)>,
    materials: Vec<Handle<BackdropImage>>,
}
struct Layer {
    rect: Rect,
    image: String,
    piece: Option<EdgePiece>,
    repeats: Option<Vec2>,
}

impl Layer {
    fn image_key(&self) -> (String, bool) {
        (self.image.clone(), self.repeats.is_some())
    }
}
fn image_sampler(tiled: bool) -> bevy::image::ImageSamplerDescriptor {
    // Addressing follows the authored tiling flag; an edge or stretched
    // background samples within its own source rectangle.
    let address = if tiled {
        bevy::image::ImageAddressMode::Repeat
    } else {
        bevy::image::ImageAddressMode::ClampToEdge
    };
    bevy::image::ImageSamplerDescriptor {
        address_mode_u: address,
        address_mode_v: address,
        ..default()
    }
}

fn node(rect: Rect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: percent(rect.left / VIEW.0 * 100.),
        top: percent((VIEW.1 - rect.top()) / VIEW.1 * 100.),
        width: percent(rect.width / VIEW.0 * 100.),
        height: percent(rect.height / VIEW.1 * 100.),
        ..default()
    }
}

fn read(
    reader: &pcx_assets::Resolver,
    path: &str,
    limit: u64,
    receipts: &mut Vec<AssetReceipt>,
) -> Result<Vec<u8>, String> {
    let asset = reader
        .resolve_bounded(path, limit)
        .map_err(|e| format!("{path}: {e}"))?;
    receipts.push(receipt(&asset));
    Ok(asset.bytes)
}
fn text(
    reader: &pcx_assets::Resolver,
    path: &str,
    receipts: &mut Vec<AssetReceipt>,
) -> Result<String, String> {
    String::from_utf8(read(reader, path, 1 << 20, receipts)?).map_err(|e| format!("{path}: {e}"))
}
fn art_path(
    skin: &SkinTable,
    token: &str,
    race: &str,
    reader: &pcx_assets::Resolver,
) -> Result<String, String> {
    let mut path = None;
    for section in [race, "Default"] {
        let values: Vec<_> = skin
            .sections()
            .iter()
            .filter(|s| s.name.eq_ignore_ascii_case(section))
            .flat_map(|s| &s.entries)
            .filter(|(key, _)| key == token)
            .collect();
        if values.len() > 1 {
            return Err(format!("{SKIN}: ambiguous {section}/{token}"));
        }
        if let Some((_, value)) = values.first() {
            path = Some(value.clone());
            break;
        }
    }
    let path = path.unwrap_or_else(|| token.to_owned());
    for candidate in [path.clone(), format!("{path}.blp")] {
        if pcx_assets::AssetPath::new(&candidate).is_ok_and(|p| reader.paths().any(|a| *a == p)) {
            return Ok(candidate);
        }
    }
    Err(format!("{PANEL}: missing artwork {token} ({path})"))
}

fn checked_rect(fdf: &Fdf, name: &str) -> Result<Rect, String> {
    let mut pending = vec![name.to_owned()];
    let mut checked = std::collections::HashSet::new();
    while let Some(frame) = pending.pop() {
        if !checked.insert(frame.clone()) {
            continue;
        }
        for property in ["Width", "Height"] {
            if let Some(values) = fdf.numbers(&frame, property).map_err(|e| e.to_string())?
                && !matches!(values.as_slice(), [value] if value.is_finite() && *value > 0.)
            {
                return Err(format!("{PANEL}: {frame}: invalid authored {property}"));
            }
        }
        if let Some(anchor) = fdf.set_point(&frame) {
            if !anchor.x.is_finite() || !anchor.y.is_finite() {
                return Err(format!(
                    "{PANEL}: {frame}: invalid authored SetPoint offset"
                ));
            }
            pending.push(anchor.relative);
        }
        if fdf.flag(&frame, "SetAllPoints")
            && let Some(parent) = fdf.frame(&frame).and_then(|f| f.parent.clone())
        {
            pending.push(parent);
        }
        let rect = fdf.rect(&frame, VIEW).map_err(|e| e.to_string())?;
        if [
            rect.left,
            rect.bottom,
            rect.width,
            rect.height,
            rect.left + rect.width,
            rect.top(),
        ]
        .into_iter()
        .any(|v| !v.is_finite())
            || rect.width < 0.
            || rect.height <= 0.
        {
            return Err(format!("{PANEL}: {frame}: invalid resolved rectangle"));
        }
    }
    fdf.rect(name, VIEW).map_err(|e| e.to_string())
}

fn prepare_fdf(panel: &str, templates: &str) -> Result<(Fdf, [TextStyle; 2]), String> {
    let mut fdf = Fdf::parse(templates).map_err(|e| format!("{TEMPLATES}: {e}"))?;
    fdf.parse_into(panel).map_err(|e| format!("{PANEL}: {e}"))?;
    let styles = [
        crate::menu_text::render_style(&fdf, SPEAKER)?,
        crate::menu_text::render_style(&fdf, DIALOGUE)?,
    ];
    // The native text frames have intrinsic sizes. Prime their line height just
    // as the existing menu renderer does; Bevy shapes and wraps the visible text.
    for (name, style) in [SPEAKER, DIALOGUE].into_iter().zip(&styles) {
        fdf.measure_text(name, (0., style.height));
    }
    checked_rect(&fdf, SPEAKER)?;
    checked_rect(&fdf, DIALOGUE)?;
    Ok((fdf, styles))
}

impl DialogueAssets {
    pub fn load(
        reader: &pcx_assets::Resolver,
        race: &str,
        images: &mut Assets<Image>,
        fonts: &mut Assets<Font>,
    ) -> Result<Self, String> {
        let mut receipts = Vec::new();
        let panel = text(reader, PANEL, &mut receipts)?;
        let templates = text(reader, TEMPLATES, &mut receipts)?;
        let skin = SkinTable::parse(&text(reader, SKIN, &mut receipts)?)
            .map_err(|e| format!("{SKIN}: {e:?}"))?;
        let (fdf, styles) = prepare_fdf(&panel, &templates)?;
        let mut decoded_fonts = HashMap::new();
        for style in &styles {
            if decoded_fonts.contains_key(&style.font) {
                continue;
            }
            let path = skin
                .unique_file(&style.font)
                .map_err(|e| format!("{SKIN}: {e:?}"))?
                .file;
            let bytes = read(reader, &path, 16 << 20, &mut receipts)?;
            if !crate::hud_render::font_registers(&bytes) {
                return Err(format!("{path}: no readable font face"));
            }
            decoded_fonts.insert(style.font.clone(), Font::from_bytes(bytes));
        }
        let mut decoded_images = HashMap::new();
        let mut decoded_bytes = 0usize;
        let mut backdrops = Vec::new();
        let mut unsupported = Vec::new();
        let frame_names: Vec<_> = fdf
            .frames()
            .iter()
            .filter(|f| f.kind == "BACKDROP")
            .map(|f| f.name.clone())
            .collect();
        for name in frame_names {
            // Templates carry no cinematic instance geometry.
            if !name.starts_with("Cinematic") {
                continue;
            }
            let rect = checked_rect(&fdf, &name)?;
            let size = Vec2::new(rect.width, rect.height);
            if !size.is_finite() || size.min_element() <= 0. {
                return Err(format!("{name}: invalid frame rectangle"));
            }
            let mut layers = Vec::new();
            if let Some(token) = fdf.string(&name, "BackdropBackground") {
                let insets = fdf
                    .numbers(&name, "BackdropBackgroundInsets")
                    .map_err(|e| e.to_string())?;
                let inset = match insets.as_deref() {
                    None => Some(0.),
                    Some([a, b, c, d]) if a == b && b == c && c == d => Some(*a),
                    Some([_, _, _, _]) => {
                        unsupported.push(format!("{name}: unequal background inset component order is unresolved; background omitted"));
                        None
                    }
                    _ => return Err(format!("{name}: malformed BackdropBackgroundInsets")),
                };
                if let Some(inset) = inset {
                    let inner = Rect {
                        left: inset,
                        bottom: inset,
                        width: size.x - 2. * inset,
                        height: size.y - 2. * inset,
                    };
                    if !inset.is_finite() || inset < 0. || inner.width <= 0. || inner.height <= 0. {
                        return Err(format!("{name}: invalid background inset"));
                    }
                    let repeats = if fdf.flag(&name, "BackdropTileBackground") {
                        let values = fdf
                            .numbers(&name, "BackdropBackgroundSize")
                            .map_err(|e| e.to_string())?
                            .ok_or_else(|| format!("{name}: missing tile size"))?;
                        let [tile] = values.as_slice() else {
                            return Err(format!("{name}: malformed tile size"));
                        };
                        if !tile.is_finite() || *tile <= 0. {
                            return Err(format!("{name}: invalid tile size"));
                        }
                        Some(Vec2::new(inner.width, inner.height) / *tile)
                    } else {
                        None
                    };
                    layers.push(Layer {
                        rect: inner,
                        image: art_path(&skin, &token, race, reader)?,
                        piece: None,
                        repeats,
                    });
                }
            }
            // Read the edge independently: unsupported background insets do not
            // erase authored border pieces that the existing renderer can draw.
            if let Some(token) = fdf.string(&name, "BackdropEdgeFile") {
                let flags = fdf
                    .string(&name, "BackdropCornerFlags")
                    .ok_or_else(|| format!("{name}: missing edge flags"))?;
                let values = fdf
                    .numbers(&name, "BackdropCornerSize")
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("{name}: missing corner size"))?;
                let [corner] = values.as_slice() else {
                    return Err(format!("{name}: malformed corner size"));
                };
                if !corner.is_finite() || *corner <= 0. {
                    return Err(format!("{name}: invalid corner size"));
                }
                let image = art_path(&skin, &token, race, reader)?;
                for flag in flags.split('|') {
                    let piece = EdgePiece::ALL
                        .into_iter()
                        .find(|p| p.flag() == flag.trim())
                        .ok_or_else(|| format!("{name}: unknown edge flag {flag}"))?;
                    let rect = glue_menu::edge_rect(piece, size, *corner);
                    if rect.width > 0. && rect.height > 0. {
                        layers.push(Layer {
                            rect,
                            image: image.clone(),
                            piece: Some(piece),
                            repeats: None,
                        });
                    }
                }
            }
            for layer in &layers {
                if decoded_images.contains_key(&layer.image_key()) {
                    continue;
                }
                let bytes = read(reader, &layer.image, 4 << 20, &mut receipts)?;
                let texture = pcx_gfx::load_image_bounded(&bytes, &layer.image, 16 << 20)
                    .map_err(|e| format!("{}: {e}", layer.image))?;
                decoded_bytes = decoded_bytes
                    .checked_add(texture.rgba_bytes().ok_or("invalid decoded texture size")?)
                    .ok_or("transmission texture size overflow")?;
                if decoded_bytes > 64 << 20 {
                    return Err("transmission textures exceed 64 MiB resource budget".into());
                }
                if let Some(piece) = layer.piece {
                    glue_menu::edge_source_rect(piece, texture.width, texture.height)
                        .map_err(|e| format!("{}: {e:?}", layer.image))?;
                }
                let mut image = crate::menu_composition::image(&texture);
                image.sampler =
                    bevy::image::ImageSampler::Descriptor(image_sampler(layer.repeats.is_some()));
                decoded_images.insert(layer.image_key(), image);
            }
            backdrops.push((rect, layers));
        }
        Ok(Self {
            receipts,
            unsupported,
            fdf,
            styles,
            fonts: decoded_fonts
                .into_iter()
                .map(|(k, v)| (k, fonts.add(v)))
                .collect(),
            images: decoded_images
                .into_iter()
                .map(|(k, v)| (k, images.add(v)))
                .collect(),
            backdrops,
            materials: Vec::new(),
        })
    }

    pub fn clear_materials(&mut self, materials: &mut Assets<BackdropImage>) {
        for material in self.materials.drain(..) {
            materials.remove(material.id());
        }
    }
    pub fn release(
        mut self,
        images: &mut Assets<Image>,
        fonts: &mut Assets<Font>,
        materials: &mut Assets<BackdropImage>,
    ) {
        self.clear_materials(materials);
        for handle in self.images.values() {
            images.remove(handle.id());
        }
        for handle in self.fonts.values() {
            fonts.remove(handle.id());
        }
    }

    pub fn paint(
        &mut self,
        commands: &mut Commands,
        scene: &pcx_play::playercolor_host::CinematicScene,
        images: &Assets<Image>,
        materials: &mut Assets<BackdropImage>,
    ) -> Result<Entity, String> {
        let parsed = crate::rich_text::parse(&scene.text).map_err(|e| e.to_string())?;
        if !parsed.unsupported.is_empty() {
            return Err("transmission contains unsupported substitutions".into());
        }
        if self
            .images
            .values()
            .any(|handle| images.get(handle).is_none())
        {
            return Err("transmission image was removed".into());
        }
        for (_, layers) in &self.backdrops {
            for layer in layers {
                if let Some(piece) = layer.piece {
                    let image = images
                        .get(&self.images[&layer.image_key()])
                        .expect("checked images");
                    glue_menu::edge_source_rect(piece, image.width(), image.height())
                        .map_err(|e| format!("{}: {e:?}", layer.image))?;
                }
            }
        }
        let root = commands
            .spawn((
                Node {
                    width: percent(100.),
                    height: percent(100.),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                super::CampaignHud,
                Pickable::IGNORE,
                Name::new("Campaign transmission"),
            ))
            .id();
        for (rect, layers) in &self.backdrops {
            for layer in layers {
                let placed = Rect {
                    left: rect.left + layer.rect.left,
                    bottom: rect.bottom + layer.rect.bottom,
                    ..layer.rect
                };
                let handle = self.images[&layer.image_key()].clone();
                let image = images
                    .get(&handle)
                    .ok_or("transmission image was removed")?;
                let size = Vec2::new(image.width() as f32, image.height() as f32);
                let mut child = commands.spawn((node(placed), Pickable::IGNORE));
                if let Some(piece) = layer.piece {
                    let source = glue_menu::edge_source_rect(piece, image.width(), image.height())
                        .map_err(|e| format!("{e:?}"))?;
                    if matches!(piece, EdgePiece::Top | EdgePiece::Bottom) {
                        let material = materials.add(glue_menu::horizontal_edge_material(
                            handle, source, size, false,
                        ));
                        child.insert(MaterialNode(material.clone()));
                        self.materials.push(material);
                    } else {
                        let mut image_node = ImageNode::new(handle);
                        image_node.rect = Some(source);
                        child.insert(image_node);
                    }
                } else if let Some(repeats) = layer.repeats {
                    let material = materials.add(BackdropImage {
                        texture: handle,
                        uv_transform: Vec4::new(repeats.x, repeats.y, 0., 0.),
                        uv_rows: Vec4::new(1., 0., 0., 1.),
                    });
                    child.insert(MaterialNode(material.clone()));
                    self.materials.push(material);
                } else {
                    child.insert(ImageNode::new(handle));
                }
                let id = child.id();
                commands.entity(root).add_child(id);
            }
        }
        for (i, name) in [SPEAKER, DIALOGUE].into_iter().enumerate() {
            let style = &self.styles[i];
            let rect = checked_rect(&self.fdf, name)?;
            let mut position = node(rect);
            position.height = Val::Auto;
            // Intrinsic speaker width remains content-sized. Dialogue width is authored.
            if self.fdf.record(name, "Width").is_none() {
                position.width = Val::Auto;
            }
            let font = TextFont {
                font: bevy::text::FontSource::Handle(self.fonts[&style.font].clone()),
                font_size: bevy::text::FontSize::Vh(style.height / VIEW.1 * 100.),
                ..default()
            };
            let color = Color::srgba(
                style.colour[0],
                style.colour[1],
                style.colour[2],
                style.colour[3],
            );
            let text = commands
                .spawn((
                    position,
                    Text::new(if i == 0 {
                        scene.speaker.clone()
                    } else {
                        String::new()
                    }),
                    font.clone(),
                    TextColor(color),
                    TextLayout::new(
                        match style.justify_h {
                            crate::menu_text::JustifyH::Left => bevy::text::Justify::Left,
                            crate::menu_text::JustifyH::Center => bevy::text::Justify::Center,
                            crate::menu_text::JustifyH::Right => bevy::text::Justify::Right,
                        },
                        if self.fdf.record(name, "Width").is_some() {
                            bevy::text::LineBreak::WordBoundary
                        } else {
                            bevy::text::LineBreak::NoWrap
                        },
                    ),
                    TextShadow {
                        offset: Vec2::ZERO,
                        color: Color::srgba(
                            style.shadow_colour[0],
                            style.shadow_colour[1],
                            style.shadow_colour[2],
                            style.shadow_colour[3],
                        ),
                    },
                    glue_menu::FrameShadow(Vec2::from(style.shadow_offset)),
                    Pickable::IGNORE,
                ))
                .id();
            if i == 1 {
                for span in &parsed.spans {
                    let c = span
                        .colour
                        .map(|c| Color::srgba(c[0], c[1], c[2], c[3]))
                        .unwrap_or(color);
                    let child = commands
                        .spawn((TextSpan::new(span.text.clone()), font.clone(), TextColor(c)))
                        .id();
                    commands.entity(text).add_child(child);
                }
            }
            commands.entity(root).add_child(text);
        }
        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SYNTHETIC: &str = r#"
Frame "FRAME" "CinematicPanel" {
    SetAllPoints,
    Frame "BACKDROP" "CinematicPortraitCover" {
        Width 0.12,
        Height 0.18,
        SetPoint BOTTOMLEFT, "CinematicPanel", BOTTOMLEFT, 0.03, 0.07,
    }
    Frame "TEXT" "CinematicSpeakerText" INHERITS "TestText" {
        SetPoint TOPLEFT, "CinematicPortraitCover", TOPRIGHT, 0.02, -0.04,
    }
    Frame "TEXT" "CinematicDialogueText" INHERITS "TestText" {
        Width 0.41,
        SetPoint TOPLEFT, "CinematicSpeakerText", BOTTOMLEFT, 0.01, -0.03,
        FontColor 0.2 0.4 0.6,
    }
}
"#;
    const TEMPLATE: &str = r#"
Frame "TEXT" "TestText" {
    FrameFont "SyntheticFace", 0.025, "",
    FontColor 0.3 0.2 0.1 0.8,
}
"#;
    #[test]
    fn shared_artwork_keeps_each_authored_tiling_mode() {
        let mut layer = Layer {
            rect: Rect {
                left: 0.,
                bottom: 0.,
                width: 1.,
                height: 1.,
            },
            image: "shared-synthetic.blp".into(),
            piece: None,
            repeats: None,
        };
        let stretched_key = layer.image_key();
        let stretched_sampler = image_sampler(stretched_key.1);
        assert_eq!(
            stretched_sampler.address_mode_u,
            bevy::image::ImageAddressMode::ClampToEdge
        );
        assert_eq!(
            stretched_sampler.address_mode_v,
            bevy::image::ImageAddressMode::ClampToEdge
        );
        layer.repeats = Some(Vec2::new(3., 2.));
        let tiled_key = layer.image_key();
        let tiled_sampler = image_sampler(tiled_key.1);
        assert_ne!(stretched_key, tiled_key);
        assert_eq!(
            tiled_sampler.address_mode_u,
            bevy::image::ImageAddressMode::Repeat
        );
        assert_eq!(
            tiled_sampler.address_mode_v,
            bevy::image::ImageAddressMode::Repeat
        );
        let modes = HashMap::from([
            (stretched_key, stretched_sampler),
            (tiled_key, tiled_sampler),
        ]);
        assert_eq!(
            modes.len(),
            2,
            "shared paths cannot make the first sampler win"
        );
    }

    #[test]
    fn transmission_follows_authored_anchor_chain_font_and_colour() {
        let (fdf, styles) = prepare_fdf(SYNTHETIC, TEMPLATE).unwrap();
        let speaker = fdf.rect(SPEAKER, VIEW).unwrap();
        let line = fdf.rect(DIALOGUE, VIEW).unwrap();
        assert!((speaker.left - 0.17).abs() < 1e-6);
        assert!((speaker.top() - 0.21).abs() < 1e-6);
        assert!((line.left - 0.18).abs() < 1e-6);
        assert!((line.top() - 0.155).abs() < 1e-6);
        assert_eq!(line.width, 0.41);
        assert_eq!(styles[0].font, "SyntheticFace");
        assert_eq!(styles[0].height, 0.025);
        assert_eq!(styles[1].colour, [0.2, 0.4, 0.6, 1.]);
        let (changed, styles) = prepare_fdf(
            &SYNTHETIC.replace("0.41", "0.32"),
            &TEMPLATE.replace("0.025", "0.019"),
        )
        .unwrap();
        assert_eq!(changed.rect(DIALOGUE, VIEW).unwrap().width, 0.32);
        assert_eq!(styles[0].height, 0.019);
        for invalid in ["NaN", "inf", "-0.1", "0", "bad"] {
            assert!(
                prepare_fdf(&SYNTHETIC.replace("0.41", invalid), TEMPLATE).is_err(),
                "invalid authored width {invalid}"
            );
            assert!(
                prepare_fdf(&SYNTHETIC.replace("0.18", invalid), TEMPLATE).is_err(),
                "invalid anchor ancestor height {invalid}"
            );
        }
        assert!(prepare_fdf(&SYNTHETIC.replace("0.02, -0.04", "NaN, -0.04"), TEMPLATE).is_err());
        assert!(prepare_fdf(&SYNTHETIC.replace("0.02, -0.04", "0.02, inf"), TEMPLATE).is_err());
        assert!(prepare_fdf(SYNTHETIC, "").is_err());
    }

    #[test]
    #[ignore = "requires explicit local original FDF directory via PEACECRAFT3_DIALOGUE_FDF_DIR"]
    fn local_original_transmission_frames_parse() {
        let dir = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_DIALOGUE_FDF_DIR")
                .expect("explicit original FDF input directory"),
        );
        let panel = std::fs::read_to_string(dir.join("CinematicPanel.fdf")).unwrap();
        let templates = std::fs::read_to_string(dir.join("EscMenuTemplates.fdf")).unwrap();
        let (fdf, styles) = prepare_fdf(&panel, &templates).unwrap();
        for (name, style) in [SPEAKER, DIALOGUE].into_iter().zip(styles) {
            let rect = fdf.rect(name, VIEW).unwrap();
            assert!(rect.left.is_finite() && rect.top().is_finite());
            assert!(style.height > 0.);
        }
    }

    #[test]
    fn frame_positions_remain_relative_at_any_resolution() {
        let n = node(Rect {
            left: VIEW.0 / 4.,
            bottom: VIEW.1 / 8.,
            width: VIEW.0 / 2.,
            height: VIEW.1 / 4.,
        });
        assert_eq!(n.left, percent(25.));
        assert_eq!(n.top, percent(62.5));
        assert_eq!(n.width, percent(50.));
        assert_eq!(n.height, percent(25.));
    }
}
