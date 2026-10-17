//! Tooltip for one HUD frame and one caller-supplied field.
//!
//! `hud_markup_adapter` is a [`crate::hud_assets::HudMarkup::Adapter`]
//! function. `HudMarkup::Plain` is unchanged and still refuses `|` and `^^`.
//!
//! A mounted [`crate::hud_render::HudSourceAttachment`] supplies identity
//! only. [`TooltipSupply::record`] supplies the field text. Install does not
//! show a tooltip: the supply, one matching live frame, a caller placement,
//! and hover or visible keyboard focus are all required. The existing HUD planner
//! supplies font, justification, wrapping and shadow style; placement is explicit.
//! `Tip` and `Ubertip` are not ranked.
//!
//! Assumptions, read from `b4b4a5c`: `HudSourceAttachment` is spawned by
//! `mount_resolved` with `source_label` equal to `HudInput.source.identity`,
//! `source_member`, `frame`, `receipt_path`, and `binding_revision` (zero is
//! a real revision). `HudMarkup::Adapter` receives the resolved literal and
//! an empty span list is a HUD text error. `RelativeBox` percentages are the
//! placement `hud_render` already writes as `Val::Percent`. No command-card
//! type exists on this main.

use bevy::asset::Handle;
use bevy::ecs::system::SystemParam;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::Font;

use crate::hud_assets::{HudMarkupSpan, HudMarkupText, PlannedSpan, PlannedText, RelativeBox};
use crate::hud_render::{HudRoot, HudSourceAttachment, HudSurface};
use crate::rich_text::{self, RichSpan, RichTextError};

/// Marks the one live frame selected by the supplied source identity.
#[derive(Component, Debug)]
pub struct TooltipOwner;

/// The one chosen field. The owner does not contain this text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TooltipRecord {
    /// Must equal the attachment's source label.
    pub source_label: String,
    /// Must equal the attachment's source member.
    pub source_member: String,
    /// Must equal the attachment's frame name.
    pub frame: String,
    /// Must equal the attachment's receipt, including empty.
    pub receipt_path: String,
    /// Must equal the attachment's revision, including zero.
    pub binding_revision: u64,
    /// Caller-selected field name.
    pub field: String,
    /// Markup-preserving value. Empty is a blank label.
    pub text: String,
}

/// Caller values required to draw. There is no default budget, colour, size,
/// or placement.
#[derive(Resource, Clone, Debug)]
pub struct TooltipSupply {
    /// The only field considered.
    pub field: String,
    /// Text and identity for that field.
    pub record: TooltipRecord,
    /// Handle the caller resolved for `style.font_path`.
    pub font: Handle<Font>,
    /// Text style resolved from the selected FDF by the existing HUD planner.
    /// Its label runs are replaced by the selected tooltip field.
    pub style: PlannedText,
    /// Percent box the caller already resolved. `None` does not place the view.
    pub placement: Option<RelativeBox>,
    /// Caller's `HudBudgets.max_input_bytes`, copied by that caller.
    pub max_input_bytes: usize,
}

/// The tooltip text entity. Hidden until a selected owner paints.
#[derive(Resource, Clone, Copy, Debug)]
pub struct TooltipView(pub Entity);

/// Why the selected owner is not drawn.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TooltipFault {
    /// Matching frame, when one was selected.
    pub owner: Option<Entity>,
    /// Budget, identity, markup, or placement failure. The source string is kept.
    pub detail: String,
}

/// Closed `^^` tags on the visible string.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TooltipMarkupNotes(pub Vec<String>);

#[derive(Component)]
struct TooltipAttempt {
    owner: Entity,
}

#[derive(SystemParam)]
struct TooltipScene<'w, 's> {
    nodes: Query<'w, 's, &'static mut Node>,
    fonts: Option<Res<'w, Assets<Font>>>,
    surface: Option<Res<'w, HudSurface>>,
    frames: Query<
        'w,
        's,
        (
            Entity,
            &'static HudSourceAttachment,
            Option<&'static Hovered>,
        ),
    >,
    owners: Query<'w, 's, Entity, With<TooltipOwner>>,
    parents: Query<'w, 's, &'static ChildOf>,
    roots: Query<'w, 's, (), With<HudRoot>>,
    attempts: Query<'w, 's, &'static TooltipAttempt>,
}

/// `HudMarkup::Adapter` entry. Colours are the hex channels, not hotkey highlights.
///
/// # Errors
///
/// [`rich_text::parse`] refused the string. An empty string is a blank label.
pub fn hud_markup_adapter(raw: &str) -> Result<HudMarkupText, String> {
    let parsed = parse_markup(raw)?;
    Ok(HudMarkupText {
        spans: parsed
            .spans
            .iter()
            .map(|span| HudMarkupSpan {
                text: span.text.clone(),
                colour: span.colour,
            })
            .collect(),
        notes: parsed.notes,
    })
}

/// Registers the hidden view and refresh before UI preparation. Drawing waits for a
/// mounted frame, a supply, and selection.
pub fn install(app: &mut App) {
    app.add_systems(Startup, spawn_tooltip_view).add_systems(
        PostUpdate,
        refresh_tooltips.before(bevy::ui::UiSystems::Prepare),
    );
}

/// Spawns the hidden tooltip entity once.
pub fn spawn_tooltip_view(mut commands: Commands, existing: Option<Res<TooltipView>>) {
    if existing
        .as_ref()
        .is_some_and(|view| commands.get_entity(view.0).is_ok())
    {
        return;
    }
    let view = spawn_hidden(&mut commands);
    commands.insert_resource(TooltipView(view));
}

// The same mounted surface supplies the camera and percentage coordinate space.
fn refresh_tooltips(
    mut commands: Commands,
    mut scene: TooltipScene,
    supply: Option<Res<TooltipSupply>>,
    view: Option<Res<TooltipView>>,
    focus: Option<Res<InputFocus>>,
    keyboard: Option<Res<InputFocusVisible>>,
) {
    let Some(view_id) = view.and_then(|view| scene.nodes.get(view.0).is_ok().then_some(view.0))
    else {
        let view = spawn_hidden(&mut commands);
        commands.insert_resource(TooltipView(view));
        return;
    };
    let Some(supply) = supply else {
        clear_owners(&mut commands, &scene.owners, None);
        clear_view(&mut commands, view_id);
        return;
    };
    let mut matched = scene.frames.iter().filter(|(_, attachment, _)| {
        supply.field == supply.record.field
            && !supply.field.is_empty()
            && same_identity(attachment, &supply.record)
    });
    let first = matched.next();
    let Some((entity, _, hovered)) = first.filter(|_| matched.next().is_none()) else {
        clear_owners(&mut commands, &scene.owners, None);
        clear_view(&mut commands, view_id);
        return;
    };
    clear_owners(&mut commands, &scene.owners, Some(entity));
    if !scene.owners.contains(entity) {
        commands.entity(entity).insert(TooltipOwner);
    }
    if !hovered.is_some_and(Hovered::get)
        && !(keyboard.is_some_and(|visible| visible.0)
            && focus.and_then(|focus| focus.get()) == Some(entity))
    {
        clear_view(&mut commands, view_id);
        return;
    }
    let Some(root) = scene
        .parents
        .iter_ancestors(entity)
        .find(|parent| scene.roots.contains(*parent))
    else {
        fail(
            &mut commands,
            view_id,
            Some(entity),
            "tooltip owner has no mounted HUD surface".into(),
        );
        return;
    };
    if scene.parents.get(view_id).map(ChildOf::parent).ok() != Some(root) {
        commands.entity(root).add_child(view_id);
    }
    if let Err(detail) = input_budget(&supply) {
        fail(&mut commands, view_id, Some(entity), detail);
        return;
    }
    if !scene
        .fonts
        .as_ref()
        .is_some_and(|fonts| fonts.contains(supply.font.id()))
    {
        fail(
            &mut commands,
            view_id,
            Some(entity),
            format!("tooltip font {:?} is unavailable", supply.style.font_path),
        );
        return;
    }
    if scene
        .attempts
        .get(view_id)
        .is_ok_and(|attempt| attempt.owner == entity && !supply.is_changed())
    {
        return;
    }
    let attempt = TooltipAttempt { owner: entity };
    let painted = match paint(&supply) {
        Ok(painted) => painted,
        Err(detail) => {
            fail(&mut commands, view_id, Some(entity), detail);
            commands.entity(view_id).insert(attempt);
            return;
        }
    };
    let Some(placement) = painted.placement else {
        fail(
            &mut commands,
            view_id,
            Some(entity),
            "tooltip placement was not supplied".into(),
        );
        commands.entity(view_id).insert(attempt);
        return;
    };
    if let Ok(mut node) = scene.nodes.get_mut(view_id) {
        node.position_type = PositionType::Absolute;
        node.left = Val::Percent(placement.left);
        node.top = Val::Percent(placement.top);
        node.width = Val::Percent(placement.width);
        node.height = Val::Percent(placement.height);
    }
    commands.entity(view_id).remove::<TooltipFault>();
    if painted.notes.is_empty() {
        commands.entity(view_id).remove::<TooltipMarkupNotes>();
    } else {
        commands
            .entity(view_id)
            .insert(TooltipMarkupNotes(painted.notes));
    }
    commands.entity(view_id).despawn_children();
    commands
        .entity(view_id)
        .insert((Visibility::Visible, attempt));
    if let Ok(mut node) = scene.nodes.get_mut(view_id) {
        node.padding = UiRect::default();
        crate::hud_render::style_text_node(&mut node, &supply.style);
    }
    let mut text = supply.style.clone();
    text.spans = painted
        .spans
        .into_iter()
        .map(|span| PlannedSpan {
            text: span.text,
            colour: span.colour,
        })
        .collect();
    text.notes = Vec::new();
    let child = crate::hud_render::spawn_text(&mut commands, view_id, &text, &supply.font);
    if let Some(surface) = &scene.surface
        && surface.pixels.is_finite()
        && surface.pixels.cmpgt(Vec2::ZERO).all()
    {
        commands.entity(child).insert(TextShadow {
            offset: Vec2::from_array(text.shadow_fraction) * surface.pixels,
            color: Color::srgba(
                text.shadow[0],
                text.shadow[1],
                text.shadow[2],
                text.shadow[3],
            ),
        });
    }
}

struct Painted {
    spans: Vec<RichSpan>,
    notes: Vec<String>,
    placement: Option<RelativeBox>,
}

fn paint(supply: &TooltipSupply) -> Result<Painted, String> {
    if supply.field.is_empty() || supply.record.field != supply.field {
        return Err("tooltip field does not match the supplied record".to_string());
    }
    if supply.style.font_path.is_empty() || supply.style.face.is_empty() {
        return Err("tooltip font path is empty".to_string());
    }
    if ![supply.style.colour, supply.style.shadow]
        .iter()
        .flatten()
        .all(|channel| channel.is_finite())
        || !supply
            .style
            .shadow_fraction
            .iter()
            .all(|value| value.is_finite())
        || supply
            .style
            .padding_vw_vh
            .is_some_and(|padding| padding.iter().any(|value| !value.is_finite()))
    {
        return Err("tooltip base colour is not finite".to_string());
    }
    let size = supply.style.height_vh;
    if !size.is_finite() || size <= 0.0 {
        return Err("tooltip font size must be finite and positive".into());
    }
    if let Some(placement) = supply.placement
        && !placement_is_valid(placement)
    {
        return Err("tooltip placement must be finite with non-negative extents".to_string());
    }
    let parsed = parse_markup(&supply.record.text)?;
    if !supply.record.text.is_empty() && parsed.spans.is_empty() {
        return Err(format!(
            "tooltip markup produced no runs in {:?}",
            supply.record.text
        ));
    }
    Ok(Painted {
        spans: parsed.spans,
        notes: parsed.notes,
        placement: supply.placement,
    })
}

struct ParsedMarkup {
    spans: Vec<RichSpan>,
    notes: Vec<String>,
}

fn parse_markup(raw: &str) -> Result<ParsedMarkup, String> {
    if raw.is_empty() {
        return Ok(ParsedMarkup {
            spans: Vec::new(),
            notes: Vec::new(),
        });
    }
    match rich_text::parse(raw) {
        Ok(parsed) => Ok(ParsedMarkup {
            spans: parsed.spans,
            notes: parsed
                .unsupported
                .into_iter()
                .map(|note| note.form)
                .collect(),
        }),
        Err(error @ RichTextError::Truncated { .. } | error @ RichTextError::Malformed { .. }) => {
            Err(error.to_string())
        }
    }
}

fn input_budget(supply: &TooltipSupply) -> Result<(), String> {
    let mut total = 0usize;
    for part in [
        supply.field.len(),
        supply.record.source_label.len(),
        supply.record.source_member.len(),
        supply.record.frame.len(),
        supply.record.receipt_path.len(),
        supply.record.field.len(),
        supply.record.text.len(),
        supply.style.font_path.len(),
        supply.style.face.len(),
    ]
    .into_iter()
    .chain(supply.style.spans.iter().map(|span| span.text.len()))
    .chain(supply.style.notes.iter().map(String::len))
    {
        total = total
            .checked_add(part)
            .ok_or_else(|| "tooltip input length overflows".to_string())?;
        if total > supply.max_input_bytes {
            return Err(format!(
                "tooltip input exceeds max_input_bytes {}",
                supply.max_input_bytes
            ));
        }
    }
    Ok(())
}

fn same_identity(attachment: &HudSourceAttachment, record: &TooltipRecord) -> bool {
    attachment.source_label == record.source_label
        && attachment.source_member == record.source_member
        && attachment.frame == record.frame
        && attachment.receipt_path == record.receipt_path
        && attachment.binding_revision == record.binding_revision
}

fn placement_is_valid(placement: RelativeBox) -> bool {
    [
        placement.left,
        placement.top,
        placement.width,
        placement.height,
    ]
    .iter()
    .all(|value| value.is_finite())
        && placement.width >= 0.0
        && placement.height >= 0.0
}

fn clear_owners(
    commands: &mut Commands,
    owners: &Query<Entity, With<TooltipOwner>>,
    keep: Option<Entity>,
) {
    for entity in owners.iter().filter(|entity| Some(*entity) != keep) {
        commands.entity(entity).remove::<TooltipOwner>();
    }
}

fn spawn_hidden(commands: &mut Commands) -> Entity {
    commands
        .spawn((Node::default(), Visibility::Hidden, Pickable::IGNORE))
        .id()
}

fn clear_view(commands: &mut Commands, view: Entity) {
    commands.entity(view).despawn_children();
    commands.entity(view).insert(Visibility::Hidden);
    commands.entity(view).remove::<TooltipAttempt>();
    commands.entity(view).remove::<TooltipMarkupNotes>();
    commands.entity(view).remove::<TooltipFault>();
}

fn fail(commands: &mut Commands, view: Entity, owner: Option<Entity>, detail: String) {
    clear_view(commands, view);
    commands.entity(view).insert(TooltipFault { owner, detail });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud_assets::{HudMarkup, RelativeBox, sample_hud};
    use crate::hud_render::{HudMount, HudSourceAttachment, HudSubmit};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::input_focus::{InputFocus, InputFocusVisible};
    use bevy::picking::hover::Hovered;
    use bevy::text::FontSource;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Image>>();
        app.init_resource::<Assets<Font>>();
        crate::hud_render::install(&mut app);
        install(&mut app);
        app
    }

    fn coloured_input(markup: HudMarkup) -> crate::hud_assets::HudInput {
        let mut input = sample_hud(&["Human", "Default"]);
        input.fdf_texts[0] =
            input.fdf_texts[0].replace("Text \"Gold\",", "Text \"|cffff0000Gold|r\",");
        input.markup = markup;
        input
    }

    fn named(app: &mut App, name: &str) -> Entity {
        let mut query = app.world_mut().query::<(Entity, &Name)>();
        let found: Vec<Entity> = query
            .iter(app.world())
            .filter(|(_, node)| node.as_str() == name)
            .map(|(entity, _)| entity)
            .collect();
        assert_eq!(found.len(), 1, "{name}");
        found[0]
    }

    fn spans(world: &World, view: Entity) -> Vec<(String, Color)> {
        let Some(children) = world.get::<Children>(view) else {
            return Vec::new();
        };
        children
            .iter()
            .flat_map(|text| {
                world
                    .get::<Children>(text)
                    .into_iter()
                    .flat_map(|children| children.iter())
            })
            .map(|child| {
                (
                    world.get::<TextSpan>(child).unwrap().0.clone(),
                    world.get::<TextColor>(child).unwrap().0,
                )
            })
            .collect()
    }

    fn mount_adapter(app: &mut App) {
        let plain = coloured_input(HudMarkup::Plain);
        app.world_mut().write_message(HudSubmit { input: plain });
        app.update();
        let error = app.world().resource::<HudMount>().error.clone().unwrap();
        assert!(error.to_string().contains("markup"), "{error}");
        assert!(app.world().resource::<HudMount>().root.is_none());

        let adapted = coloured_input(HudMarkup::Adapter(hud_markup_adapter));
        let mut style = crate::hud_assets::resolve(&adapted)
            .unwrap()
            .frames
            .into_iter()
            .find(|frame| frame.name == "Gold")
            .unwrap()
            .text
            .unwrap();
        style.colour = [0.25, 0.5, 0.75, 1.0];
        let budget = adapted.budgets.max_input_bytes;
        app.world_mut().write_message(HudSubmit { input: adapted });
        app.update();
        let mount = app.world().resource::<HudMount>();
        assert!(mount.error.is_none(), "{:?}", mount.error);
        assert!(mount.root.is_some());

        let gold = named(app, "Gold");
        let text = app
            .world()
            .get::<Children>(gold)
            .unwrap()
            .iter()
            .next()
            .unwrap();
        let span = app
            .world()
            .get::<Children>(text)
            .unwrap()
            .iter()
            .next()
            .unwrap();
        assert_eq!(app.world().get::<TextSpan>(span).unwrap().0, "Gold");
        assert_eq!(
            app.world().get::<TextColor>(span).unwrap().0,
            Color::srgba(1.0, 0.0, 0.0, 1.0)
        );
        let handle = match &app.world().get::<TextFont>(text).unwrap().font {
            FontSource::Handle(handle) => handle.clone(),
            other => panic!("mounted face was not a handle: {other:?}"),
        };
        let attachment = app
            .world()
            .get::<HudSourceAttachment>(gold)
            .unwrap()
            .clone();
        app.insert_resource(TooltipSupply {
            field: "Tip".to_string(),
            record: TooltipRecord {
                source_label: attachment.source_label,
                source_member: attachment.source_member,
                frame: attachment.frame,
                receipt_path: attachment.receipt_path,
                binding_revision: attachment.binding_revision,
                field: "Tip".to_string(),
                text: "|cffff0000赤|r|n|cff00ff00AB|r".to_string(),
            },
            font: handle,
            style,
            placement: Some(RelativeBox {
                left: 12.0,
                top: 34.0,
                width: 56.0,
                height: 78.0,
            }),
            max_input_bytes: budget,
        });
    }

    #[test]
    fn tooltip_hud_mount_owner_hover_focus_replace_and_retire() {
        let mut app = test_app();
        mount_adapter(&mut app);
        app.update();
        let gold = named(&mut app, "Gold");
        let view = app.world().resource::<TooltipView>().0;
        assert!(app.world().get::<TooltipOwner>(gold).is_some());
        assert!(spans(app.world(), view).is_empty());
        assert_eq!(app.world().get::<Node>(view).unwrap().left, Val::Auto);

        app.world_mut().entity_mut(gold).insert(Hovered(true));
        app.update();
        let inherited = Color::srgba(0.25, 0.5, 0.75, 1.0);
        let pointer = spans(app.world(), view);
        assert_eq!(
            pointer,
            vec![
                ("赤".to_string(), Color::srgba(1.0, 0.0, 0.0, 1.0)),
                ("\n".to_string(), inherited),
                ("AB".to_string(), Color::srgba(0.0, 1.0, 0.0, 1.0)),
            ]
        );
        assert!(pointer.iter().all(|span| span.0 != "Gold"));
        let node = app.world().get::<Node>(view).unwrap();
        assert_eq!(node.left, Val::Percent(12.0));
        assert_eq!(node.top, Val::Percent(34.0));
        assert_eq!(node.width, Val::Percent(56.0));
        assert_eq!(node.height, Val::Percent(78.0));
        assert_eq!(
            app.world()
                .get::<TextFont>(app.world().get::<Children>(view).unwrap()[0])
                .unwrap()
                .font_size,
            bevy::text::FontSize::Vh(app.world().resource::<TooltipSupply>().style.height_vh)
        );

        app.world_mut().entity_mut(gold).insert(Hovered(false));
        app.insert_resource(InputFocus::from_entity(gold));
        app.insert_resource(InputFocusVisible(true));
        app.update();
        assert_eq!(spans(app.world(), view), pointer);

        app.insert_resource(InputFocus::default());
        app.insert_resource(InputFocusVisible(false));
        app.update();
        assert!(spans(app.world(), view).is_empty());

        app.world_mut().entity_mut(gold).insert(Hovered(true));
        app.world_mut().resource_mut::<TooltipSupply>().record.text = "gamma".to_string();
        app.update();
        assert_eq!(
            spans(app.world(), view),
            vec![("gamma".to_string(), inherited)]
        );

        app.world_mut().resource_mut::<TooltipSupply>().record.text = "|cffff".to_string();
        app.update();
        assert!(spans(app.world(), view).is_empty());
        assert_eq!(
            app.world().get::<Visibility>(view).unwrap(),
            &Visibility::Hidden
        );
        assert!(
            app.world()
                .get::<TooltipFault>(view)
                .unwrap()
                .detail
                .contains("|cffff")
        );
        assert_eq!(
            app.world().get::<Node>(view).unwrap().left,
            Val::Percent(12.0)
        );

        let root = app.world().resource::<HudMount>().root.unwrap();
        app.world_mut().entity_mut(root).despawn();
        app.update();
        assert!(spans(app.world(), view).is_empty());
        assert!(app.world().get::<HudSourceAttachment>(gold).is_none());
        assert!(app.world().get::<TooltipOwner>(gold).is_none());
        assert!(
            !app.world()
                .resource::<TooltipSupply>()
                .record
                .text
                .is_empty()
        );
    }

    #[test]
    fn tooltip_missing_placement_and_input_limit_do_not_position_the_view() {
        let mut app = test_app();
        mount_adapter(&mut app);
        app.world_mut().resource_mut::<TooltipSupply>().placement = None;
        let gold = named(&mut app, "Gold");
        app.world_mut().entity_mut(gold).insert(Hovered(true));
        app.update();
        let view = app.world().resource::<TooltipView>().0;
        assert!(spans(app.world(), view).is_empty());
        assert_eq!(app.world().get::<Node>(view).unwrap().left, Val::Auto);
        assert_eq!(
            app.world().get::<Node>(view).unwrap().position_type,
            PositionType::Relative
        );
        assert!(
            app.world()
                .get::<TooltipFault>(view)
                .unwrap()
                .detail
                .contains("placement")
        );

        {
            let mut supply = app.world_mut().resource_mut::<TooltipSupply>();
            supply.placement = Some(RelativeBox {
                left: 1.0,
                top: 1.0,
                width: 1.0,
                height: 1.0,
            });
            supply.max_input_bytes = 1;
        }
        app.update();
        assert!(spans(app.world(), view).is_empty());
        assert_eq!(app.world().get::<Node>(view).unwrap().left, Val::Auto);
        assert!(
            app.world()
                .get::<TooltipFault>(view)
                .unwrap()
                .detail
                .contains("max_input_bytes")
        );
    }

    #[test]
    fn tooltip_blank_field_is_blank_and_a_different_field_has_no_owner() {
        let mut app = test_app();
        mount_adapter(&mut app);
        {
            let mut supply = app.world_mut().resource_mut::<TooltipSupply>();
            supply.record.text.clear();
        }
        let gold = named(&mut app, "Gold");
        app.world_mut().entity_mut(gold).insert(Hovered(true));
        app.update();
        let view = app.world().resource::<TooltipView>().0;
        assert!(spans(app.world(), view).is_empty());
        assert_eq!(
            app.world().get::<Visibility>(view).unwrap(),
            &Visibility::Visible
        );
        assert_eq!(
            app.world().get::<Node>(view).unwrap().left,
            Val::Percent(12.0)
        );

        app.world_mut().resource_mut::<TooltipSupply>().field = "Ubertip".to_string();
        app.update();
        assert!(app.world().get::<TooltipOwner>(gold).is_none());
        assert!(spans(app.world(), view).is_empty());
        assert_eq!(
            app.world().get::<Visibility>(view).unwrap(),
            &Visibility::Hidden
        );
    }

    #[test]
    fn tooltip_rejects_invalid_font_size_and_negative_extent_before_positioning() {
        let mut app = test_app();
        mount_adapter(&mut app);
        let gold = named(&mut app, "Gold");
        app.world_mut().entity_mut(gold).insert(Hovered(true));
        app.world_mut()
            .resource_mut::<TooltipSupply>()
            .style
            .height_vh = f32::NAN;
        app.update();
        let view = app.world().resource::<TooltipView>().0;
        assert_eq!(
            app.world().get::<Visibility>(view),
            Some(&Visibility::Hidden)
        );
        assert_eq!(app.world().get::<Node>(view).unwrap().left, Val::Auto);
        assert!(
            app.world()
                .get::<TooltipFault>(view)
                .unwrap()
                .detail
                .contains("font")
        );
        {
            let mut supply = app.world_mut().resource_mut::<TooltipSupply>();
            supply.style.height_vh = 2.0;
            supply.placement.as_mut().unwrap().width = -1.0;
        }
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(view),
            Some(&Visibility::Hidden)
        );
        assert_eq!(app.world().get::<Node>(view).unwrap().left, Val::Auto);
        assert!(
            app.world()
                .get::<TooltipFault>(view)
                .unwrap()
                .detail
                .contains("placement")
        );
    }

    #[test]
    fn tooltip_uses_its_owners_mounted_surface_and_reuses_unchanged_spans() {
        let mut app = test_app();
        mount_adapter(&mut app);
        let root = app.world().resource::<HudMount>().root.unwrap();
        let camera = app.world_mut().spawn(Camera2d).id();
        let surface = app
            .world_mut()
            .spawn((Node::default(), UiTargetCamera(camera)))
            .id();
        app.world_mut().entity_mut(surface).add_child(root);
        let gold = named(&mut app, "Gold");
        app.world_mut().entity_mut(gold).insert(Hovered(true));
        app.update();
        let view = app.world().resource::<TooltipView>().0;
        assert_eq!(
            app.world().get::<ChildOf>(view).map(ChildOf::parent),
            Some(root)
        );
        let before: Vec<Entity> = app.world().get::<Children>(view).unwrap().iter().collect();
        for _ in 0..3 {
            app.update();
        }
        let after: Vec<Entity> = app.world().get::<Children>(view).unwrap().iter().collect();
        assert_eq!(after, before);
        app.world_mut().entity_mut(root).despawn();
        app.update();
        assert!(app.world().get_entity(view).is_err());
    }

    #[test]
    fn tooltip_keeps_the_planned_font_wrap_justification_and_shadow() {
        let mut app = test_app();
        mount_adapter(&mut app);
        let gold = named(&mut app, "Gold");
        app.world_mut().entity_mut(gold).insert(Hovered(true));
        app.world_mut().resource_mut::<HudSurface>().pixels = Vec2::new(200.0, 100.0);
        {
            let mut supply = app.world_mut().resource_mut::<TooltipSupply>();
            supply.style.justify_h = crate::menu_text::JustifyH::Right;
            supply.style.justify_v = crate::menu_text::JustifyV::Bottom;
            supply.style.wrap = true;
            supply.style.shadow = [0.1, 0.2, 0.3, 0.4];
            supply.style.padding_vw_vh = Some([2.0, 3.0]);
        }
        app.update();
        let view = app.world().resource::<TooltipView>().0;
        let text = app.world().get::<Children>(view).unwrap()[0];
        let layout = app.world().get::<TextLayout>(text).unwrap();
        assert_eq!(layout.justify, bevy::text::Justify::Right);
        assert_eq!(layout.linebreak, bevy::text::LineBreak::WordBoundary);
        assert_eq!(
            app.world().get::<TextShadow>(text).unwrap().color,
            Color::srgba(0.1, 0.2, 0.3, 0.4)
        );
        assert_eq!(
            app.world().get::<TextShadow>(text).unwrap().offset,
            Vec2::from_array(
                app.world()
                    .resource::<TooltipSupply>()
                    .style
                    .shadow_fraction
            ) * app.world().resource::<HudSurface>().pixels
        );
        let node = app.world().get::<Node>(view).unwrap();
        assert_eq!(node.justify_content, JustifyContent::FlexEnd);
        assert_eq!(node.align_items, AlignItems::FlexEnd);
        assert_eq!(node.padding.left, Val::Vw(2.0));
        app.world_mut()
            .resource_mut::<TooltipSupply>()
            .style
            .padding_vw_vh = None;
        app.update();
        assert_eq!(
            app.world().get::<Node>(view).unwrap().padding,
            UiRect::default()
        );
    }

    #[test]
    fn tooltip_hides_when_its_font_is_retired_without_a_supply_change() {
        let mut app = test_app();
        mount_adapter(&mut app);
        let gold = named(&mut app, "Gold");
        app.world_mut().entity_mut(gold).insert(Hovered(true));
        app.update();
        let view = app.world().resource::<TooltipView>().0;
        assert!(!spans(app.world(), view).is_empty());
        let font = app.world().resource::<TooltipSupply>().font.id();
        app.world_mut().resource_mut::<Assets<Font>>().remove(font);
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(view),
            Some(&Visibility::Hidden)
        );
        assert!(spans(app.world(), view).is_empty());
        assert!(
            app.world()
                .get::<TooltipFault>(view)
                .unwrap()
                .detail
                .contains("unavailable")
        );
        {
            let mut supply = app.world_mut().resource_mut::<TooltipSupply>();
            supply.style.font_path = "x".repeat(supply.max_input_bytes + 1);
        }
        app.update();
        let fault = app.world().get::<TooltipFault>(view).unwrap();
        assert!(fault.detail.contains("max_input_bytes"));
        assert!(fault.detail.len() < 100);
    }

    #[test]
    fn tooltip_menu_hotkey_parser_still_drops_the_hex() {
        let label = crate::menu_text::Label::parse("|CffffffffS|Ringle").unwrap();
        assert_eq!(label.hotkey(), Some('S'));
        assert_eq!(label.visible(), "Single");
        let _ = app_runs_refresh_without_a_supply();
    }

    fn app_runs_refresh_without_a_supply() -> bool {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        install(&mut app);
        app.update();
        let view = app.world().resource::<TooltipView>().0;
        assert_eq!(
            app.world().get::<Visibility>(view).unwrap(),
            &Visibility::Hidden
        );
        assert!(spans(app.world(), view).is_empty());
        app.world_mut().run_system_once(refresh_tooltips).unwrap();
        true
    }
}
