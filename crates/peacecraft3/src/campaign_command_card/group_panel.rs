//! Multiple-selection group panel: a grid of unit buttons shown in the unit information frame
//! while two or more units are selected. Geometry and the subgroup highlight come from the
//! client ([`pcx_play::group_panel_layout`]); evidence is in
//! `docs/re-notes/input-selection-rules.md`.
use super::*;
use crate::campaign_selection_rules::{Modifiers, click_group_button};
use pcx_play::{group_panel_layout, inventory_layout::Rect, scripts::AssetIdentity};

/// Value the client stores in the highlight's blend-mode field. Its name is not decoded;
/// the highlight texture has no alpha channel, which only an additive blend can use.
const ADDITIVE_BLEND_MODE: u32 = 3;

/// Layout read from the client image.
pub(super) struct Layout {
    rules: group_panel_layout::Rules,
    info: Rect,
}

impl Layout {
    pub(super) fn read(image: &[u8], identity: AssetIdentity) -> Result<Self, String> {
        let rules = group_panel_layout::Rules::reviewed_legacy(image, identity.clone())?;
        let info = pcx_play::inventory_layout::Rules::reviewed_legacy(image, identity)?
            .info_panel();
        if rules.highlight_blend != ADDITIVE_BLEND_MODE {
            return Err("group panel highlight blend mode is not the additive value".into());
        }
        Ok(Self { rules, info })
    }

    pub(super) fn highlight_key(&self) -> &str {
        &self.rules.highlight_key
    }
}

/// Layout plus the skin art of the subgroup highlight.
pub(super) struct Source {
    layout: Layout,
    highlight_art: String,
    highlight: Option<Handle<crate::glue_menu::AdditiveImage>>,
}

impl Source {
    pub(super) fn new(layout: Layout, highlight_art: String) -> Self {
        Self {
            layout,
            highlight_art,
            highlight: None,
        }
    }

    pub(super) fn release(&mut self, materials: &mut Assets<crate::glue_menu::AdditiveImage>) {
        if let Some(material) = self.highlight.take() {
            materials.remove(material.id());
        }
    }
}

/// Selects, removes or switches to this unit. The press belongs to the group panel.
#[derive(Component, Clone)]
pub(crate) struct GroupButton {
    unit: HandleId,
    session: Weak<()>,
}

/// One painted button's identity. A change rebuilds the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Cell {
    unit: HandleId,
    art: String,
    /// The unit belongs to the active subgroup.
    highlighted: bool,
    mana: bool,
}

struct Fills {
    health: HeroFill,
    mana: Option<HeroFill>,
}

#[derive(Default)]
pub(super) struct State {
    root: Option<Entity>,
    parent: Option<Entity>,
    cells: Vec<Cell>,
    fills: Vec<Fills>,
    pub(super) error: Option<String>,
}

impl State {
    pub(super) fn root(&self) -> Option<Entity> {
        self.root
    }
}

/// Units in subgroup order, then selection order, at most `capacity`, each flagged when it
/// belongs to the active subgroup. The cell order within the original panel is not decoded
/// (recreation choice).
fn order(
    groups: &[pcx_play::selection_subgroups::Subgroup],
    active: Option<usize>,
    capacity: usize,
) -> Vec<(HandleId, bool)> {
    groups
        .iter()
        .enumerate()
        .flat_map(|(index, group)| {
            group
                .units
                .iter()
                .map(move |&unit| (unit, active == Some(index)))
        })
        .take(capacity)
        .collect()
}

/// Buttons with their live health and mana fractions. Empty unless two or more units are
/// selected.
fn view(objects: &ObjectHost, capacity: usize) -> Result<Vec<(Cell, [f32; 2])>, String> {
    if objects.selection().len() < 2 {
        return Ok(Vec::new());
    }
    let mut cells = Vec::new();
    for (unit, highlighted) in order(
        &objects.subgroups()?,
        objects.active_subgroup()?,
        capacity,
    ) {
        let record = objects.executed_unit(unit).map_err(|e| e.to_string())?;
        let vitals = record
            .vitals
            .as_ref()
            .ok_or("group panel: live vitals unavailable")?;
        if !vitals.max_life.is_finite()
            || vitals.max_life <= 0.
            || !vitals.max_mana.is_finite()
            || vitals.max_mana < 0.
        {
            return Err("group panel: invalid live maximum".into());
        }
        let mana = vitals.max_mana > 0.;
        cells.push((
            Cell {
                unit,
                art: pcx_play::training_command::icon(&record.definition)?,
                highlighted,
                mana,
            },
            [
                fraction(vitals.life, vitals.max_life),
                if mana {
                    fraction(vitals.mana, vitals.max_mana)
                } else {
                    0.
                },
            ],
        ));
    }
    Ok(cells)
}

/// Multiplies the texture's colour channels by `argb`, so an additive draw carries the tint.
fn tinted(image: &Image, argb: u32) -> Result<Image, String> {
    let [_, r, g, b] = argb.to_be_bytes();
    let mut tinted = image.clone();
    let data = tinted
        .data
        .as_mut()
        .ok_or("group panel highlight has no pixel data")?;
    if data.len() % 4 != 0 {
        return Err("group panel highlight is not RGBA8".into());
    }
    for pixel in data.chunks_exact_mut(4) {
        for (channel, tint) in pixel.iter_mut().zip([r, g, b]) {
            *channel = (u16::from(*channel) * u16::from(tint) / 255) as u8;
        }
    }
    Ok(tinted)
}

/// Mounts the group buttons under the console parent and follows the units' live vitals.
/// Buttons are rebuilt only when the cells change; otherwise only fill widths move.
pub(super) fn sync(
    frame: &mut HeroFrame<'_, '_, '_, '_, '_, '_>,
    materials: &mut Assets<crate::glue_menu::AdditiveImage>,
    card: &mut CommandCard,
    source: &mut super::Source,
    reader: &pcx_assets::Resolver,
    objects: &ObjectHost,
    witness: &Weak<()>,
    active: bool,
) -> Result<(), String> {
    let Some(parent) = card.parent else {
        return Ok(());
    };
    let capacity = source
        .group_panel
        .as_ref()
        .map_err(Clone::clone)?
        .layout
        .rules
        .cells();
    let entries = view(objects, capacity)?;
    let cells: Vec<Cell> = entries.iter().map(|(cell, _)| cell.clone()).collect();
    let display = if active && !cells.is_empty() {
        Display::Flex
    } else {
        Display::None
    };
    let group = &mut card.group;
    let mounted = group
        .root
        .filter(|root| frame.nodes.get(*root).is_ok())
        .filter(|_| group.parent == Some(parent) && group.cells == cells);
    if let Some(root) = mounted {
        if frame
            .nodes
            .get(root)
            .is_ok_and(|node| node.display != display)
        {
            frame.commands.entity(root).insert(root_node(display));
        }
        for (fills, (_, live)) in group.fills.iter_mut().zip(&entries) {
            for (fill, value) in std::iter::once((&mut fills.health, live[0]))
                .chain(fills.mana.as_mut().map(|fill| (fill, live[1])))
            {
                if fill.fraction != value {
                    fill.fraction = value;
                    frame
                        .nodes
                        .get_mut(fill.entity)
                        .map_err(|_| "group panel fill node is missing")?
                        .width = Val::Percent(fill.full * value);
                }
            }
        }
        return Ok(());
    }
    if let Some(root) = group.root.take() {
        frame.commands.entity(root).try_despawn();
    }
    group.fills.clear();
    group.cells.clear();
    let hero = super::hero_source(source)?;
    let (health_art, mana_art) = (hero.health_art.clone(), hero.mana_art.clone());
    let bars = hero.rules.clone();
    let panel = source.group_panel.as_ref().map_err(Clone::clone)?;
    let (rules, info, highlight_art) = (
        panel.layout.rules.clone(),
        panel.layout.info,
        panel.highlight_art.clone(),
    );
    if rules.viewport != bars.viewport {
        return Err("group panel and hero bar disagree on the logical viewport".into());
    }
    let icons = cells
        .iter()
        .map(|cell| image(reader, source, &cell.art, frame.images))
        .collect::<Result<Vec<_>, _>>()?;
    let fill_art = [
        image(reader, source, &health_art, frame.images)?,
        image(reader, source, &mana_art, frame.images)?,
    ];
    let highlight = if cells.iter().any(|cell| cell.highlighted) {
        let panel = source.group_panel.as_mut().map_err(|e| e.clone())?;
        if panel.highlight.is_none() {
            let texture = image(reader, source, &highlight_art, frame.images)?;
            let decoded = frame
                .images
                .get(&texture)
                .ok_or("group panel highlight image is missing")?;
            let tinted = frame.images.add(tinted(decoded, rules.highlight_tint)?);
            let panel = source.group_panel.as_mut().map_err(|e| e.clone())?;
            panel.highlight = Some(materials.add(crate::glue_menu::AdditiveImage::new(tinted)));
        }
        source
            .group_panel
            .as_ref()
            .map_err(Clone::clone)?
            .highlight
            .clone()
    } else {
        None
    };
    let root = frame
        .commands
        .spawn((root_node(display), CampaignInterface, Pickable::IGNORE))
        .id();
    frame.commands.entity(parent).add_child(root);
    for (index, ((cell, live), icon)) in entries.iter().zip(icons).enumerate() {
        if cell.highlighted
            && let Some(material) = &highlight
        {
            frame.commands.spawn((
                viewport_node(rules.highlight(info, index)?, rules.viewport),
                MaterialNode(material.clone()),
                Pickable::IGNORE,
                ChildOf(root),
            ));
        }
        frame.commands.spawn((
            viewport_node(rules.icon(info, index)?, rules.viewport),
            ImageNode::new(icon).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
            Button,
            bevy::picking::hover::Hovered::default(),
            GroupButton {
                unit: cell.unit,
                session: witness.clone(),
            },
            ChildOf(root),
        ));
        let mut bar_fills = Vec::with_capacity(2);
        let health = (
            rules.health_bar(info, index)?,
            &bars.health,
            &fill_art[0],
            live[0],
        );
        let mana = cell.mana.then(|| {
            rules
                .mana_bar(info, index)
                .map(|bar| (bar, &bars.mana, &fill_art[1], live[1]))
        });
        for (bar, style, texture, value) in
            std::iter::once(health).chain(mana.transpose()?.into_iter())
        {
            let outset = Rect {
                x: bar.x - bars.inset,
                y: bar.y - bars.inset,
                width: bar.width + 2. * bars.inset,
                height: bar.height + 2. * bars.inset,
            };
            frame.commands.spawn((
                viewport_node(outset, rules.viewport),
                BackgroundColor(colour(bars.background)),
                Pickable::IGNORE,
                ChildOf(root),
            ));
            let full = bar.width / rules.viewport[0] * 100.;
            let mut node = viewport_node(bar, rules.viewport);
            node.width = Val::Percent(full * value);
            let entity = frame
                .commands
                .spawn((
                    node,
                    ImageNode::new(texture.clone())
                        .with_mode(bevy::ui::widget::NodeImageMode::Stretch)
                        .with_color(colour(style.tint)),
                    Pickable::IGNORE,
                    ChildOf(root),
                ))
                .id();
            bar_fills.push(HeroFill {
                entity,
                full,
                fraction: value,
            });
        }
        let mut bar_fills = bar_fills.into_iter();
        group.fills.push(Fills {
            health: bar_fills.next().ok_or("group panel health bar is missing")?,
            mana: bar_fills.next(),
        });
    }
    group.root = Some(root);
    group.parent = Some(parent);
    group.cells = cells;
    Ok(())
}

/// A group-panel press: see [`click_group_button`].
pub(super) fn click(
    actions: Res<crate::button_actions::ButtonActions>,
    buttons: Query<&GroupButton, Without<bevy::ui::InteractionDisabled>>,
    controls: HeroControls,
    game_input: Res<peacecraft3::game_input::GameInput>,
    boxes: Res<crate::campaign_box_select::BoxSelect>,
    mut input: ResMut<crate::campaign_input::CampaignInput>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let witness = session.session_witness();
    let Some(host) = session.host() else { return };
    if !crate::campaign_input::commands_available(
        &controls.view,
        &controls.hud,
        &controls.dialogs,
        host,
    ) {
        return;
    }
    let modifiers = Modifiers::read(&game_input);
    for entity in &actions.entities {
        let Ok(button) = buttons.get(*entity) else {
            continue;
        };
        if !witness
            .as_ref()
            .is_some_and(|current| Weak::ptr_eq(current, &button.session))
        {
            continue;
        }
        let objects = host.objects_mut();
        let local = objects.local_player_number();
        let owned = objects
            .executed_unit(button.unit)
            .ok()
            .and_then(|record| objects.player_settings(record.owner).ok())
            .is_some_and(|player| Some(player.number) == local);
        if owned
            && let Err(error) =
                click_group_button(objects, &mut input, button.unit, modifiers, boxes.cap())
        {
            input.last_error = Some(error);
        }
    }
}
