//! Resource slots from the selected ResourceBar FDF and effective player state.
//! Root placement at the viewport's top right is a recreation choice: the FDF
//! supplies slot relationships, but does not author its native root attachment.
use crate::{fdf::Fdf, menu_skin::SkinTable, menu_text};
use bevy::{ecs::system::SystemParam, prelude::*};
use pcx_play::{map_startup_host::MapStartupHost, playercolor_host::ObjectHost};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignHud, CampaignInterface, CampaignPresentationSet},
    map_binding::AssetReceipt,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::{collections::HashMap, sync::Weak};

const MEMBER: &str = "UI\\FrameDef\\UI\\ResourceBar.fdf";
const ROOT: &str = "ResourceBarFrame";
// Application payload limit; independent of source frame/art values.
const MAX_DECODED_BYTES: usize = 64 << 20;
#[derive(Component)]
struct ResourceRoot;
#[derive(Resource, Default)]
struct ResourceBar {
    witness: Option<Weak<()>>,
    attempted: bool,
    root: Option<Entity>,
    source: Option<Source>,
    last_error: Option<String>,
}
struct Source {
    receipts: Vec<AssetReceipt>,
    globals: String,
    width: f32,
    height: f32,
    labels: Vec<Label>,
    icons: Vec<Icon>,
    fonts: HashMap<String, Handle<Font>>,
    images: HashMap<String, Handle<Image>>,
}
struct Icon {
    path: String,
    width: f32,
    height: f32,
    anchor: Anchor,
}
struct Label {
    name: String,
    anchor: Anchor,
    width: Option<f32>,
    height: f32,
    length: Option<usize>,
    style: menu_text::TextStyle,
    entity: Option<Entity>,
    rendered: String,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Anchor {
    right: bool,
    x: f32,
    y: f32,
}
#[derive(SystemParam)]
struct RenderAssets<'w> {
    fonts: ResMut<'w, Assets<Font>>,
    images: ResMut<'w, Assets<Image>>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<ResourceBar>().add_systems(
        Update,
        (project, publish, show_root)
            .chain()
            .after(CampaignCameraSet)
            .before(CampaignPresentationSet),
    );
}
fn project(
    mut commands: Commands,
    mut state: ResMut<ResourceBar>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    sources: Res<SelectedMapSources>,
    requests: Res<MapRequests>,
    camera: Res<CampaignCamera>,
    assets: RenderAssets,
) {
    let RenderAssets {
        mut fonts,
        mut images,
    } = assets;
    let witness = session.session_witness();
    let same = match (&state.witness, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same {
        if let Some(root) = state.root.take() {
            commands.entity(root).despawn();
        }
        if let Some(source) = state.source.take() {
            for image in source.images.values() {
                images.remove(image.id());
            }
            for font in source.fonts.values() {
                fonts.remove(font.id());
            }
        }
        *state = ResourceBar {
            witness,
            ..default()
        };
    }
    if !camera.active() {
        return;
    }
    if !state.attempted {
        let (Some(context), Some(bindings)) = (session.context(), requests.bindings.as_ref())
        else {
            return;
        };
        if !requests
            .admission
            .shared_context()
            .as_ref()
            .is_some_and(|published| std::sync::Arc::ptr_eq(published, context))
            || !requests
                .published_receipts
                .as_ref()
                .is_some_and(|published| std::sync::Arc::ptr_eq(published, &bindings.receipts))
        {
            return;
        }
        let Some(reader) = sources.for_receipts(&bindings.receipts) else {
            return;
        };
        let Some(host) = session.host() else {
            return;
        };
        state.attempted = true;
        let result = load(
            &reader,
            host.objects()
                .local_race_preference_constant()
                .map_err(|e| e.to_string()),
            &mut fonts,
            &mut images,
        );
        match result {
            Ok(mut source) => {
                let root = paint(&mut commands, &mut source);
                state.root = Some(root);
                state.source = Some(source);
            }
            Err(error) => state.last_error = Some(error),
        }
    }
    let Some(host) = session.host() else {
        return;
    };
    let Some(source) = state.source.as_mut() else {
        return;
    };
    let (values, mut errors) = balances(host.objects(), &source.globals);
    for label in &mut source.labels {
        let mut value = values.get(label.name.as_str()).cloned().unwrap_or_default();
        let parsed = match resource_runs(&value, label.length) {
            Ok(parsed) => parsed,
            Err(error) => {
                errors.push(format!("{}: {error}", label.name));
                value.clear();
                crate::rich_text::parse("").expect("empty markup")
            }
        };
        if label.rendered != value {
            if let Some(entity) = label.entity {
                commands
                    .entity(entity)
                    .despawn_children()
                    .insert(Text::default());
                for span in parsed.spans {
                    let colour = span.colour.unwrap_or(label.style.colour);
                    let child = commands
                        .spawn((
                            TextSpan::new(span.text),
                            TextFont {
                                font: source.fonts[&label.style.font].clone().into(),
                                font_size: bevy::text::FontSize::Vh(
                                    label.style.height / crate::mainmenu::FRAME_H * 100.,
                                ),
                                ..default()
                            },
                            TextColor(Color::srgba(colour[0], colour[1], colour[2], colour[3])),
                        ))
                        .id();
                    commands.entity(entity).add_child(child);
                }
            }
            label.rendered = value;
        }
    }
    state.last_error = (!errors.is_empty()).then(|| errors.join("; "));
}
fn show_root(camera: Res<CampaignCamera>, mut roots: Query<&mut Node, With<ResourceRoot>>) {
    for mut node in &mut roots {
        node.display = if camera.active() {
            Display::Flex
        } else {
            Display::None
        };
    }
}
fn publish(state: Res<ResourceBar>, mut hud: ResMut<super::CampaignHud>) {
    let receipts = state
        .source
        .as_ref()
        .map(|source| source.receipts.as_slice())
        .unwrap_or_default();
    if hud.resource_receipts != receipts {
        hud.resource_receipts = receipts.to_vec();
    }
    if hud.resource_error != state.last_error {
        hud.resource_error = state.last_error.clone();
    }
}
fn normalize(text: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut icons = 0usize;
    for raw in text.lines() {
        let line = raw.split_once("//").map_or(raw, |(line, _)| line).trim();
        if let Some(rest) = line.strip_prefix("Texture INHERITS ") {
            out.push_str(&format!(
                "Frame \"TEXTURE\" \"$ResourceIcon{icons}\" INHERITS {rest}\n"
            ));
            icons += 1;
        } else if let Some(rest) = line.strip_prefix("Texture ") {
            if !rest.starts_with('"') {
                return Err(format!("{MEMBER}: unsupported Texture header {line}"));
            }
            out.push_str(&format!("Frame \"TEXTURE\" {rest}\n"));
        } else if let Some(rest) = line.strip_prefix("String ") {
            out.push_str(&format!("Frame \"TEXT\" {rest}\n"));
        } else if let Some(rest) = line.strip_prefix("Font ") {
            out.push_str(&format!("FrameFont {rest}\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok(out)
}
fn anchor(fdf: &Fdf, name: &str) -> Result<Anchor, String> {
    let raw = fdf
        .record(name, "Anchor")
        .ok_or_else(|| format!("{name}: missing Anchor"))?;
    let values: Vec<_> = raw.split(',').map(str::trim).collect();
    let [point, x, y] = values.as_slice() else {
        return Err(format!("{name}: malformed Anchor {raw}"));
    };
    let right = match *point {
        "TOPLEFT" => false,
        "TOPRIGHT" => true,
        _ => return Err(format!("{name}: unsupported Anchor {point}")),
    };
    let number = |value: &str| {
        value
            .trim_end_matches('f')
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("{name}: invalid Anchor offset {value}"))
    };
    Ok(Anchor {
        right,
        x: number(x)?,
        y: number(y)?,
    })
}
fn dimension(value: Result<f32, crate::fdf::FdfError>, name: &str) -> Result<f32, String> {
    let value = value.map_err(|error| format!("{name}: {error}"))?;
    if !value.is_finite() || value <= 0. {
        return Err(format!("{name}: non-positive/non-finite dimension"));
    }
    Ok(value)
}
fn load(
    reader: &pcx_assets::Resolver,
    race: Result<Option<String>, String>,
    fonts: &mut Assets<Font>,
    images: &mut Assets<Image>,
) -> Result<Source, String> {
    use super::upper_buttons::{read, skin_file, text};
    let mut receipts = Vec::new();
    let skin = SkinTable::parse(&text(reader, super::SKIN, &mut receipts)?)
        .map_err(|error| error.to_string())?;
    let sections = super::race_sections(&skin, race?)?;
    let globals = text(reader, r"UI\FrameDef\GlobalStrings.fdf", &mut receipts)?;
    let fdf = Fdf::parse(&normalize(&text(reader, MEMBER, &mut receipts)?)?)
        .map_err(|error| format!("{MEMBER}: {error}"))?;
    let width = dimension(fdf.width(ROOT), ROOT)?;
    let height = dimension(fdf.height(ROOT), ROOT)?;
    let mut icons = Vec::new();
    let mut labels = Vec::new();
    let mut decoded_fonts = HashMap::new();
    let mut decoded_images = HashMap::new();
    let mut retained = 0usize;
    for frame in fdf
        .frames()
        .iter()
        .filter(|frame| frame.parent.as_deref() == Some(ROOT))
    {
        let anchor = anchor(&fdf, &frame.name)?;
        let height = dimension(fdf.height(&frame.name), &frame.name)?;
        match frame.kind.as_str() {
            "TEXTURE" => {
                let width = dimension(fdf.width(&frame.name), &frame.name)?;
                let key = fdf
                    .string(&frame.name, "File")
                    .ok_or_else(|| format!("{}: missing texture File", frame.name))?;
                let token = skin_file(&skin, &sections, &key)?;
                let path = std::iter::once(token.clone())
                    .chain(
                        fdf.flag(ROOT, "DecorateFileNames")
                            .then(|| format!("{token}{}", super::IMAGE_SUFFIX)),
                    )
                    .find(|candidate| {
                        pcx_assets::AssetPath::new(candidate)
                            .is_ok_and(|path| reader.paths().any(|entry| *entry == path))
                    })
                    .ok_or_else(|| format!("{MEMBER}: missing texture {token}"))?;
                if !decoded_images.contains_key(&path) {
                    let bytes = read(reader, &path, 4 << 20, &mut receipts)?;
                    let texture = pcx_gfx::load_image_bounded(
                        &bytes,
                        &path,
                        (16 << 20).min(MAX_DECODED_BYTES - retained),
                    )
                    .map_err(|error| error.to_string())?;
                    retained += texture
                        .rgba_bytes()
                        .ok_or_else(|| format!("{path}: invalid decoded payload size"))?;
                    let mut image = pcx_gfx::viewer::rgba_image(&texture);
                    image.sampler = bevy::image::ImageSampler::Descriptor(
                        super::upper_buttons::image_sampler(false),
                    );
                    decoded_images.insert(path.clone(), image);
                }
                icons.push(Icon {
                    path,
                    width,
                    height,
                    anchor,
                });
            }
            "TEXT" => {
                if !matches!(
                    frame.name.as_str(),
                    "ResourceBarGoldText"
                        | "ResourceBarLumberText"
                        | "ResourceBarSupplyText"
                        | "ResourceBarUpkeepText"
                ) {
                    return Err(format!(
                        "{MEMBER}: unsupported resource text binding {}",
                        frame.name
                    ));
                }
                let style = menu_text::render_style(&fdf, &frame.name)?;
                if !decoded_fonts.contains_key(&style.font) {
                    let path = skin_file(&skin, &sections, &style.font)?;
                    let bytes = read(reader, &path, 16 << 20, &mut receipts)?;
                    retained=retained.checked_add(bytes.len()).filter(|total|*total<=MAX_DECODED_BYTES).ok_or_else(||format!("{path}: aggregate decoded/font payload exceeds {MAX_DECODED_BYTES} bytes"))?;
                    if !crate::hud_render::font_registers(&bytes) {
                        return Err(format!("{path}: unreadable font"));
                    }
                    decoded_fonts.insert(style.font.clone(), Font::from_bytes(bytes));
                }
                let width = if fdf.record(&frame.name, "Width").is_some() {
                    Some(dimension(fdf.width(&frame.name), &frame.name)?)
                } else {
                    None
                };
                let length = fdf
                    .record(&frame.name, "TextLength")
                    .map(|value| {
                        value
                            .parse::<usize>()
                            .map_err(|_| format!("{}: malformed TextLength", frame.name))
                    })
                    .transpose()?;
                labels.push(Label {
                    name: frame.name.clone(),
                    anchor,
                    width,
                    height,
                    length,
                    style,
                    entity: None,
                    rendered: String::new(),
                });
            }
            other => return Err(format!("{MEMBER}: unsupported resource child {other}")),
        }
    }
    for name in [
        "ResourceBarGoldText",
        "ResourceBarLumberText",
        "ResourceBarSupplyText",
        "ResourceBarUpkeepText",
    ] {
        if !labels.iter().any(|label| label.name == name) {
            return Err(format!("{MEMBER}: missing {name}"));
        }
    }
    Ok(Source {
        globals,
        receipts,
        width,
        height,
        labels,
        icons,
        fonts: decoded_fonts
            .into_iter()
            .map(|(key, font)| (key, fonts.add(font)))
            .collect(),
        images: decoded_images
            .into_iter()
            .map(|(key, image)| (key, images.add(image)))
            .collect(),
    })
}
fn anchored(anchor: Anchor, parent: (f32, f32)) -> Node {
    let mut node = Node {
        position_type: PositionType::Absolute,
        top: Val::Percent(-anchor.y / parent.1 * 100.),
        ..default()
    };
    if anchor.right {
        node.right = Val::Percent(-anchor.x / parent.0 * 100.);
    } else {
        node.left = Val::Percent(anchor.x / parent.0 * 100.);
    }
    node
}
fn paint(commands: &mut Commands, source: &mut Source) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Percent(0.),
                top: Val::Percent(0.),
                width: Val::Percent(source.width / crate::mainmenu::FRAME_W * 100.),
                height: Val::Percent(source.height / crate::mainmenu::FRAME_H * 100.),
                ..default()
            },
            GlobalZIndex(10),
            CampaignHud,
            CampaignInterface,
            ResourceRoot,
            Pickable::IGNORE,
        ))
        .id();
    for icon in &source.icons {
        let mut node = anchored(icon.anchor, (source.width, source.height));
        node.width = Val::Percent(icon.width / source.width * 100.);
        node.height = Val::Percent(icon.height / source.height * 100.);
        let child = commands
            .spawn((
                node,
                ImageNode::new(source.images[&icon.path].clone()),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(child);
    }
    for label in &mut source.labels {
        let mut node = anchored(label.anchor, (source.width, source.height));
        node.height = Val::Percent(label.height / source.height * 100.);
        node.width = label
            .width
            .map_or(Val::Auto, |width| Val::Percent(width / source.width * 100.));
        let colour = label.style.colour;
        let child = commands
            .spawn((
                node,
                Text::default(),
                TextFont {
                    font: source.fonts[&label.style.font].clone().into(),
                    font_size: bevy::text::FontSize::Vh(
                        label.style.height / crate::mainmenu::FRAME_H * 100.,
                    ),
                    ..default()
                },
                TextColor(Color::srgba(colour[0], colour[1], colour[2], colour[3])),
                TextLayout::new(
                    match label.style.justify_h {
                        menu_text::JustifyH::Left => bevy::text::Justify::Left,
                        menu_text::JustifyH::Center => bevy::text::Justify::Center,
                        menu_text::JustifyH::Right => bevy::text::Justify::Right,
                    },
                    bevy::text::LineBreak::NoWrap,
                ),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(child);
        label.entity = Some(child);
    }
    root
}
fn balances(objects: &ObjectHost, globals: &str) -> (HashMap<&'static str, String>, Vec<String>) {
    let mut values = HashMap::new();
    let mut errors = Vec::new();
    let Some(player) = objects
        .local_player_settings()
        .map(|player| player.identity)
    else {
        return (
            values,
            vec!["local effective resource owner is unavailable".into()],
        );
    };
    let mut value = |name: &str| match objects.player_resource_value(player, name) {
        Ok(Some(value)) => Some(value),
        Ok(None) => {
            errors.push(format!("{name}: effective value unavailable"));
            None
        }
        Err(error) => {
            errors.push(format!("{name}: {error}"));
            None
        }
    };
    for (frame, setting) in [
        ("ResourceBarGoldText", "PLAYER_STATE_RESOURCE_GOLD"),
        ("ResourceBarLumberText", "PLAYER_STATE_RESOURCE_LUMBER"),
    ] {
        if let Some(number) = value(setting) {
            values.insert(frame, number.to_string());
        }
    }
    if let (Some(used), Some(cap)) = (
        value("PLAYER_STATE_RESOURCE_FOOD_USED"),
        value("PLAYER_STATE_RESOURCE_FOOD_CAP"),
    ) {
        // Neutral mathematical separator is a recreation choice; no authored format binding is established.
        values.insert("ResourceBarSupplyText", format!("{used}/{cap}"));
    }
    match objects.player_upkeep_label(player) {
        Ok(Some(binding)) => match upkeep_text(globals, &binding) {
            Ok(text) => {
                values.insert("ResourceBarUpkeepText", text);
            }
            Err(error) => errors.push(format!("ResourceBarUpkeepText: {error}")),
        },
        Ok(None) => {
            errors.push("ResourceBarUpkeepText: effective upkeep admission unavailable".into())
        }
        Err(error) => errors.push(format!("ResourceBarUpkeepText: {error}")),
    }
    (values, errors)
}
fn resource_runs(value: &str, length: Option<usize>) -> Result<crate::rich_text::RichText, String> {
    let parsed = crate::rich_text::parse(value).map_err(|e| e.to_string())?;
    if !parsed.unsupported.is_empty() {
        return Err("unsupported resource text substitution".into());
    }
    if length.is_some_and(|limit| {
        parsed
            .spans
            .iter()
            .map(|s| s.text.chars().count())
            .sum::<usize>()
            > limit
    }) {
        return Err("effective value exceeds authored TextLength".into());
    }
    Ok(parsed)
}
fn upkeep_text(
    globals: &str,
    binding: &pcx_play::player_resources::UpkeepLabel,
) -> Result<String, String> {
    let text = if let Some(text) = menu_text::global_string(globals, &binding.primary_key) {
        text
    } else if let Some((key, suffix)) = &binding.fallback {
        let text = menu_text::global_string(globals, key)
            .ok_or_else(|| format!("missing upkeep global string {key}"))?;
        format!("{text}{suffix}")
    } else {
        return Err(format!(
            "missing upkeep global string {}",
            binding.primary_key
        ));
    };
    // Do not silently split UTF-8 or colour markup at a native byte-buffer bound.
    if text.len() >= binding.capacity {
        return Err("localized upkeep exceeds native destination capacity".into());
    }
    Ok(text)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upkeep_localization_uses_primary_then_source_fallback_and_preserves_colour() {
        let globals =
            "StringList {\n PRIMARY \"|cffff0000Changed\",\n FALLBACK \"|cff00ff00Other\",\n}\n";
        let mut binding = pcx_play::player_resources::UpkeepLabel {
            primary_key: "PRIMARY".into(),
            fallback: Some(("FALLBACK".into(), " (7)".into())),
            capacity: 100,
        };
        let text = upkeep_text(globals, &binding).unwrap();
        let parsed = resource_runs(&text, Some(7)).unwrap();
        assert_eq!(parsed.spans[0].text, "Changed");
        assert_eq!(parsed.spans[0].colour, Some([1., 0., 0., 1.]));
        binding.primary_key = "ABSENT".into();
        assert_eq!(
            upkeep_text(globals, &binding).unwrap(),
            "|cff00ff00Other (7)"
        );
        binding.capacity = 4;
        assert!(upkeep_text(globals, &binding).is_err());
        assert!(resource_runs(&text, Some(6)).is_err());
        assert!(resource_runs("|cBAD", None).is_err());
    }
    #[test]
    fn source_templates_keep_icon_inheritance_and_counter_anchor_offsets() {
        let input = "Texture \"Icon\" {\nWidth 0.023,\nHeight 0.017,\n}\nString \"Counter\" {\nFont \"Face\",0.014,\nHeight 0.02,\nTextLength 6,\n}\nFrame \"SIMPLEFRAME\" \"ResourceBarFrame\" {\nWidth 0.31,\nHeight 0.04,\nTexture INHERITS \"Icon\" {\nAnchor TOPLEFT,0.013,-0.002,\nFile \"Gold\",\n}\nString \"Value\" INHERITS \"Counter\" {\nAnchor TOPRIGHT,-0.04,-0.003,\n}\n}\n";
        let fdf = Fdf::parse(&normalize(input).unwrap()).unwrap();
        assert_eq!(fdf.width("$ResourceIcon0").unwrap(), 0.023);
        assert_eq!(
            anchor(&fdf, "Value").unwrap(),
            Anchor {
                right: true,
                x: -0.04,
                y: -0.003
            }
        );
        assert_eq!(
            menu_text::render_style(&fdf, "Value").unwrap().height,
            0.014
        );
        assert_eq!(fdf.record("Value", "TextLength").as_deref(), Some("6"));
        let node = anchored(anchor(&fdf, "Value").unwrap(), (0.4, 0.1));
        assert!(matches!(node.right,Val::Percent(value) if (value-10.).abs()<0.00001));
        assert_eq!(node.top, Val::Percent(3.));
    }
    #[test]
    fn malformed_or_unsupported_source_anchors_refuse() {
        for record in ["TOPLEFT,NaN,0", "BOTTOMLEFT,0,0", "TOPRIGHT,1"] {
            let fdf = Fdf::parse(&format!(
                "Frame \"TEXT\" \"Value\" {{\nAnchor {record},\n}}\n"
            ))
            .unwrap();
            assert!(anchor(&fdf, "Value").is_err());
        }
        assert!(normalize("Texture {\n}\n").is_err());
    }
    #[test]
    #[ignore = "requires PCX_HUD_FIXTURE_DIR with locally extracted original ResourceBar.fdf"]
    fn original_resource_bar_frames_parse() {
        let root = std::path::PathBuf::from(
            std::env::var_os("PCX_HUD_FIXTURE_DIR").expect("PCX_HUD_FIXTURE_DIR required"),
        );
        let fdf = Fdf::parse(
            &normalize(&std::fs::read_to_string(root.join("ResourceBar.fdf")).unwrap()).unwrap(),
        )
        .unwrap();
        dimension(fdf.width(ROOT), ROOT).unwrap();
        dimension(fdf.height(ROOT), ROOT).unwrap();
        for frame in fdf
            .frames()
            .iter()
            .filter(|frame| frame.parent.as_deref() == Some(ROOT))
        {
            anchor(&fdf, &frame.name).unwrap();
            dimension(fdf.height(&frame.name), &frame.name).unwrap();
            if frame.kind == "TEXTURE" {
                dimension(fdf.width(&frame.name), &frame.name).unwrap();
                assert!(fdf.string(&frame.name, "File").is_some());
            } else {
                assert_eq!(frame.kind, "TEXT");
                assert!(menu_text::render_style(&fdf, &frame.name).unwrap().height > 0.);
            }
        }
    }
}
