//! Source planning for the normal leaderboard panel. Mounting is deliberately
//! separate from the globally replacing console HUD submission path.
use crate::{fdf::Fdf, menu_skin::SkinTable};
use pcx_play::{
    inventory_layout::Rect,
    leaderboard_layout::{Placement, Rules},
};

/// Parsed selected assets; their receipts remain owned by the campaign loader.
/// No font, palette, panel dimension or backdrop fallback is supplied here.
pub(crate) struct Source {
    pub fdf: Fdf,
    pub skin: SkinTable,
    pub row_font_path: String,
    pub row_font_height: f32,
    pub root_width: f32,
}

pub(crate) struct Row<'a> {
    pub label: &'a str,
    pub label_color: Option<[u8; 4]>,
    /// Effective row flags, after canonical host inheritance, not board defaults.
    pub names: Option<bool>,
    pub values: Option<bool>,
    pub icons: Option<bool>,
}

#[derive(Clone, Debug)]
pub(crate) struct PlannedRow {
    pub label: String,
    pub color: [u8; 4],
    pub rect: Rect,
}

pub(crate) struct Plan {
    pub placement: Placement,
    pub rows: Vec<PlannedRow>,
}

impl Source {
    pub fn parse(
        panel: &str,
        templates: &str,
        skin: &str,
        misc_ui: &str,
        rules: &Rules,
    ) -> Result<Self, String> {
        let mut fdf = Fdf::parse(templates).map_err(|e| e.to_string())?;
        fdf.parse_into(panel).map_err(|e| e.to_string())?;
        for name in [
            "Leaderboard",
            "LeaderboardBackdrop",
            "LeaderboardTitle",
            "LeaderboardListContainer",
        ] {
            fdf.frame(name)
                .ok_or_else(|| format!("leaderboard source lacks {name}"))?;
        }
        if !fdf.flag("LeaderboardBackdrop", "SetAllPoints") {
            return Err("leaderboard backdrop requires authored SetAllPoints".into());
        }
        let root_width = fdf.width("Leaderboard").map_err(|e| e.to_string())?;
        if !root_width.is_finite() || root_width <= 0. {
            return Err("invalid leaderboard FDF width".into());
        }
        let skin = SkinTable::parse(skin).map_err(|e| e.to_string())?;
        let row_font_path = skin
            .unique_file(&rules.font_key)
            .map_err(|e| e.to_string())?
            .file;
        // Reuse the existing sectioned source parser; require one exact profile record.
        let profile = SkinTable::parse(misc_ui).map_err(|e| e.to_string())?;
        let values: Vec<_> = profile
            .sections()
            .iter()
            .filter(|s| s.name == rules.font_section)
            .flat_map(|s| s.entries.iter())
            .filter(|(key, _)| key == &rules.font_height_key)
            .collect();
        let [(_, value)] = values.as_slice() else {
            return Err(format!(
                "leaderboard profile requires unique [{}/{}]",
                rules.font_section, rules.font_height_key
            ));
        };
        let row_font_height: f32 = value
            .parse()
            .map_err(|e| format!("leaderboard font height: {e}"))?;
        if !row_font_height.is_finite() || row_font_height <= 0. {
            return Err("invalid leaderboard profile font height".into());
        }
        Ok(Self {
            fdf,
            skin,
            row_font_path,
            row_font_height,
            root_width,
        })
    }

    /// Minimum authored Human02 branch: hidden title and name-only rows.
    /// Other styles refuse until their measured text/icon rendering is connected.
    pub fn plan_names(
        &self,
        rules: &Rules,
        viewport: [f32; 2],
        requested: i32,
        rows: &[Row<'_>],
        alternate_parent: bool,
    ) -> Result<Plan, String> {
        if alternate_parent {
            return Err("leaderboard timer/multiboard parent is unqualified".into());
        }
        let placement = rules.placement(viewport, self.root_width, requested, None)?;
        let mut planned = Vec::with_capacity(rows.len().min(rules.max_rows as usize));
        if rows.len() > rules.max_rows as usize {
            return Err("leaderboard requires unsupported scrolling".into());
        }
        for (index, row) in rows.iter().enumerate() {
            if row.names != Some(true)
                || row.values != Some(false)
                || row.icons.unwrap_or(rules.creation_icons)
            {
                return Err("leaderboard row style needs measured value/icon presentation".into());
            }
            let color = row
                .label_color
                .ok_or("leaderboard row palette color is unqualified")?;
            let parsed = crate::rich_text::parse(row.label).map_err(|e| e.to_string())?;
            if !parsed.unsupported.is_empty() {
                return Err("leaderboard label contains unsupported substitutions".into());
            }
            let slot = rules.row(placement.list, index as u32)?;
            let [left, right] = rules.name_span(slot.width, None)?;
            planned.push(PlannedRow {
                label: row.label.into(),
                color,
                rect: Rect {
                    x: slot.x + left,
                    y: slot.y,
                    width: right - left,
                    height: slot.height,
                },
            });
        }
        Ok(Plan {
            placement,
            rows: planned,
        })
    }
}

use crate::{
    campaign_hud::upper_buttons::{image_sampler, read, skin_file, text},
    campaign_script_dialogs::{BackdropDraw, draw_backdrop},
    glue_menu::{self, Backdrop, BackdropImage},
    hud_assets::{PlannedSpan, PlannedText},
    menu_text::{JustifyH, JustifyV},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use pcx_play::{map_startup_host::MapStartupHost, playercolor_host::ScriptLeaderboard};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignHud, CampaignPresentationSet},
    map_binding::AssetReceipt,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::{collections::HashMap, sync::Weak};

struct Loaded {
    source: Source,
    rules: Rules,
    viewport: [f32; 2],
    backdrop: Backdrop,
    font: Handle<Font>,
    images: HashMap<(String, bool), Handle<Image>>,
    materials: Vec<Handle<BackdropImage>>,
}
#[derive(Resource, Default)]
pub(crate) struct CampaignLeaderboards {
    witness: Option<Weak<()>>,
    root: Option<Entity>,
    loaded: Option<Loaded>,
    attempted: bool,
    revision: Option<u64>,
    pub last_error: Option<String>,
    pub receipts: Vec<AssetReceipt>,
}
#[derive(SystemParam)]
struct RenderAssets<'w> {
    fonts: ResMut<'w, Assets<Font>>,
    images: ResMut<'w, Assets<Image>>,
    materials: ResMut<'w, Assets<BackdropImage>>,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignLeaderboards>().add_systems(
        Update,
        project
            .after(CampaignCameraSet)
            .before(CampaignPresentationSet),
    );
}
fn clear(
    commands: &mut Commands,
    state: &mut CampaignLeaderboards,
    materials: &mut Assets<BackdropImage>,
) {
    if let Some(root) = state.root.take() {
        commands.entity(root).despawn();
    }
    if let Some(loaded) = state.loaded.as_mut() {
        for handle in loaded.materials.drain(..) {
            materials.remove(handle.id());
        }
    }
    state.revision = None;
}
fn project(
    mut commands: Commands,
    mut state: ResMut<CampaignLeaderboards>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    view: Res<CampaignCamera>,
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    mut assets: RenderAssets,
) {
    let witness = session.session_witness();
    if !match (&state.witness, &witness) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    } {
        reset(&mut commands, &mut state, witness, &mut assets);
    }
    let board = session.host().and_then(|host| {
        let objects = host.objects();
        let player = objects.local_player_settings()?.identity;
        let state = objects.leaderboard_state()?;
        let board = state.get(state.player_board(player)?)?;
        (board.displayed == Some(true)).then(|| (state.revision(), board.clone()))
    });
    let Some((revision, board)) = board.filter(|_| view.active()) else {
        present(&mut commands, &mut state, None, &mut assets);
        return;
    };
    if state.revision == Some(revision) && state.root.is_some() {
        return;
    }
    if state.loaded.is_none() {
        if state.attempted {
            return;
        }
        let Some((rules, viewport)) = session.host().and_then(|h| {
            Some((
                h.objects().leaderboard_layout()?.clone(),
                h.objects().message_layout()?.viewport,
            ))
        }) else {
            state.attempted = true;
            state.last_error =
                Some("leaderboard source-native layout/viewport rules are not admitted".into());
            return;
        };
        let (Some(context), Some(bindings)) = (session.context(), requests.bindings.as_ref())
        else {
            return;
        };
        if !requests
            .admission
            .shared_context()
            .as_ref()
            .is_some_and(|c| std::sync::Arc::ptr_eq(c, context))
            || !requests
                .published_receipts
                .as_ref()
                .is_some_and(|p| std::sync::Arc::ptr_eq(p, &bindings.receipts))
        {
            return;
        }
        let Some(reader) = sources.for_receipts(&bindings.receipts) else {
            return;
        };
        let race = session
            .host()
            .and_then(|h| h.objects().local_race_preference_constant().ok().flatten());
        state.attempted = true;
        let result = race
            .as_deref()
            .and_then(|s| s.strip_prefix("RACE_PREF_"))
            .ok_or_else(|| "leaderboard has no selected local race".to_owned())
            .and_then(|race| {
                load(
                    &reader,
                    race,
                    rules,
                    viewport,
                    &mut assets,
                    &mut state.receipts,
                )
            });
        match result {
            Ok(loaded) => state.loaded = Some(loaded),
            Err(error) => {
                state.last_error = Some(error);
                return;
            }
        }
    }
    present(
        &mut commands,
        &mut state,
        Some((revision, &board)),
        &mut assets,
    );
}
fn reset(
    commands: &mut Commands,
    state: &mut CampaignLeaderboards,
    witness: Option<Weak<()>>,
    assets: &mut RenderAssets,
) {
    clear(commands, state, &mut assets.materials);
    if let Some(loaded) = state.loaded.take() {
        assets.fonts.remove(loaded.font.id());
        for handle in loaded.images.values() {
            assets.images.remove(handle.id());
        }
    }
    *state = CampaignLeaderboards {
        witness,
        ..default()
    };
}
fn present(
    commands: &mut Commands,
    state: &mut CampaignLeaderboards,
    board: Option<(u64, &ScriptLeaderboard)>,
    assets: &mut RenderAssets,
) {
    let Some((revision, board)) = board.filter(|(_, b)| b.displayed == Some(true)) else {
        clear(commands, state, &mut assets.materials);
        return;
    };
    if state.revision == Some(revision) && state.root.is_some() {
        return;
    }
    clear(commands, state, &mut assets.materials);
    let Some(loaded) = state.loaded.as_mut() else {
        return;
    };
    match paint(
        commands,
        loaded,
        board,
        &assets.images,
        &mut assets.materials,
    ) {
        Ok(root) => {
            state.root = Some(root);
            state.revision = Some(revision);
            state.last_error = None;
        }
        Err(error) => state.last_error = Some(format!("leaderboard {:?}: {error}", board.id)),
    }
}

fn load(
    reader: &pcx_assets::Resolver,
    race: &str,
    rules: Rules,
    viewport: [f32; 2],
    assets: &mut RenderAssets,
    receipts: &mut Vec<AssetReceipt>,
) -> Result<Loaded, String> {
    let source = Source::parse(
        &text(reader, "UI\\FrameDef\\UI\\Leaderboard.fdf", receipts)?,
        &text(reader, "UI\\FrameDef\\UI\\EscMenuTemplates.fdf", receipts)?,
        &text(reader, "UI\\war3skins.txt", receipts)?,
        &text(reader, "UI\\MiscUI.txt", receipts)?,
        &rules,
    )?;
    let sections = vec![
        source
            .skin
            .sections()
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(race))
            .ok_or("leaderboard skin lacks selected race")?
            .name
            .clone(),
        "Default".into(),
    ];
    let mut backdrop = glue_menu::backdrop(&source.fdf, "LeaderboardBackdrop")
        .map_err(|e| e.to_string())?
        .ok_or("leaderboard lacks backdrop")?;
    if let Some(path) = backdrop.background.as_mut() {
        *path = skin_file(&source.skin, &sections, path)?;
    }
    if let Some(edge) = backdrop.edge.as_mut() {
        edge.file = skin_file(&source.skin, &sections, &edge.file)?;
    }
    if backdrop.tile {
        let corner = backdrop
            .edge
            .as_ref()
            .ok_or("leaderboard tile fallback requires source corner size")?
            .corner_size;
        backdrop.background_size =
            Some(rules.backdrop_tile_size(backdrop.background_size, corner)?);
    }
    let bytes = read(reader, &source.row_font_path, 16 << 20, receipts)?;
    if !crate::hud_render::font_registers(&bytes) {
        return Err("leaderboard font has no readable face".into());
    }
    let font = Font::from_bytes(bytes);
    let mut images = HashMap::new();
    let mut total = 0usize;
    for (path, tiled) in backdrop
        .background
        .iter()
        .map(|p| (p, backdrop.tile))
        .chain(backdrop.edge.iter().map(|e| (&e.file, false)))
    {
        let bytes = read(reader, path, 4 << 20, receipts)?;
        let texture = pcx_gfx::load_image_bounded(&bytes, path, 16 << 20)
            .map_err(|e| format!("{path}: {e}"))?;
        total = total
            .checked_add(texture.rgba_bytes().ok_or("invalid leaderboard texture")?)
            .filter(|n| *n <= 64 << 20)
            .ok_or("leaderboard texture budget exceeded")?;
        let mut image = crate::menu_composition::image(&texture);
        image.sampler = bevy::image::ImageSampler::Descriptor(image_sampler(tiled));
        images.insert((path.clone(), tiled), image);
    }
    Ok(Loaded {
        source,
        rules,
        viewport,
        backdrop,
        font: assets.fonts.add(font),
        images: images
            .into_iter()
            .map(|(k, v)| (k, assets.images.add(v)))
            .collect(),
        materials: Vec::new(),
    })
}
fn place(rect: Rect, viewport: [f32; 2]) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: percent(rect.x / viewport[0] * 100.),
        top: percent((viewport[1] - rect.y - rect.height) / viewport[1] * 100.),
        width: percent(rect.width / viewport[0] * 100.),
        height: percent(rect.height / viewport[1] * 100.),
        ..default()
    }
}
fn paint(
    commands: &mut Commands,
    loaded: &mut Loaded,
    board: &ScriptLeaderboard,
    images: &Assets<Image>,
    materials: &mut Assets<BackdropImage>,
) -> Result<Entity, String> {
    let style = board.style.ok_or("leaderboard style is unqualified")?;
    if style[0] {
        return Err("leaderboard title presentation is not connected".into());
    }
    let requested = board
        .size_by_item_count
        .ok_or("leaderboard display size is unqualified")?;
    let rows: Vec<_> = board
        .items
        .iter()
        .map(|i| Row {
            label: &i.label,
            label_color: i.label_color,
            names: i.style[0],
            values: i.style[1],
            icons: i.style[2],
        })
        .collect();
    let plan = loaded
        .source
        .plan_names(&loaded.rules, loaded.viewport, requested, &rows, false)?;
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.),
                height: percent(100.),
                ..default()
            },
            CampaignHud,
            Pickable::IGNORE,
            Name::new("Script leaderboard"),
        ))
        .id();
    let result = (|| -> Result<(), String> {
        let panel = commands
            .spawn((
                place(plan.placement.root, loaded.viewport),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(panel);
        draw_backdrop(
            commands,
            panel,
            BackdropDraw {
                images: &loaded.images,
                materials: &mut loaded.materials,
            },
            &loaded.backdrop,
            Vec2::new(plan.placement.root.width, plan.placement.root.height),
            images,
            materials,
        )?;
        for row in plan.rows {
            let parsed = crate::rich_text::parse(&row.label).map_err(|e| e.to_string())?;
            let color = row.color.map(|v| f32::from(v) / f32::from(u8::MAX));
            let text = PlannedText {
                face: loaded.rules.font_key.clone(),
                font_path: loaded.source.row_font_path.clone(),
                height_vh: loaded.source.row_font_height / loaded.viewport[1] * 100.,
                colour: color,
                highlight: color,
                shadow: loaded
                    .rules
                    .row_shadow
                    .map(|v| f32::from(v) / f32::from(u8::MAX)),
                shadow_fraction: [
                    loaded.rules.row_shadow_offset[0] / loaded.viewport[0],
                    -loaded.rules.row_shadow_offset[1] / loaded.viewport[1],
                ],
                justify_h: JustifyH::Left,
                justify_v: JustifyV::Middle,
                padding_vw_vh: None,
                wrap: false,
                spans: parsed
                    .spans
                    .into_iter()
                    .map(|s| PlannedSpan {
                        text: s.text,
                        colour: s.colour,
                    })
                    .collect(),
                notes: Vec::new(),
            };
            let mut node = place(row.rect, loaded.viewport);
            crate::hud_render::style_text_node(&mut node, &text);
            let slot = commands.spawn((node, Pickable::IGNORE)).id();
            commands.entity(root).add_child(slot);
            crate::hud_render::spawn_text(commands, slot, &text, &loaded.font);
        }
        Ok(())
    })();
    if let Err(error) = result {
        commands.entity(root).despawn();
        return Err(error);
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_play::scripts::AssetIdentity;
    use pcx_script::HandleId;
    fn rules() -> Rules {
        Rules {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "1".into(),
            },
            image_sha256: [37; 32],
            world_bottom_left: [0., 2.],
            world_top_right: [0., -3.],
            root_offset: [-4., -5.],
            list_insets: [1., 2., 3., 4.],
            row_height: 7.,
            row_gap: 2.,
            max_rows: 8,
            title_extra: [4., 10.],
            container_title_offset: [-2., -3.],
            container_hidden_offset: [0., -2.],
            container_bottom_offset: [0., 1.],
            name_left: 2.,
            name_after_column: -3.,
            name_right: -2.,
            value_right: -4.,
            creation_icons: false,
            backdrop_background_default: 0.,
            row_shadow: [7, 13, 23, 101],
            row_shadow_offset: [0.3, -0.4],
            font_section: "Heights".into(),
            font_height_key: "Rows".into(),
            font_key: "Face".into(),
        }
    }
    fn source(rules: &Rules) -> Source {
        Source::parse(
            r#"Frame "FRAME" "Leaderboard" {
            Width 40,
            Height 20,
            Frame "BACKDROP" "LeaderboardBackdrop" {
                SetAllPoints,
            }
            Frame "TEXT" "LeaderboardTitle" {
            }
            Frame "FRAME" "LeaderboardListContainer" {
            }
        }"#,
            "",
            "[Default]\nFace=synthetic-font.ttf\n",
            "[Heights]\nRows=2\n",
            rules,
        )
        .unwrap()
    }
    fn handle(n: u64) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(n).unwrap())
    }
    fn board() -> ScriptLeaderboard {
        ScriptLeaderboard {
            id: handle(7),
            label: Some("synthetic title".into()),
            displayed: Some(true),
            size_by_item_count: Some(1),
            style: Some([false, true, false, false]),
            label_color: Some([13, 29, 57, 101]),
            value_color: None,
            items: vec![pcx_play::playercolor_host::LeaderboardItem {
                player: handle(3),
                label: "earned count".into(),
                value: 0,
                style: [Some(true), Some(false), Some(false)],
                label_color: Some([13, 29, 57, 101]),
                value_color: None,
            }],
        }
    }
    #[derive(Resource)]
    struct Snapshot {
        board: Option<ScriptLeaderboard>,
        revision: u64,
        replace_session: bool,
    }
    fn tick(
        mut commands: Commands,
        mut state: ResMut<CampaignLeaderboards>,
        snapshot: Res<Snapshot>,
        mut assets: RenderAssets,
    ) {
        if snapshot.replace_session {
            reset(&mut commands, &mut state, None, &mut assets);
        } else {
            present(
                &mut commands,
                &mut state,
                snapshot.board.as_ref().map(|b| (snapshot.revision, b)),
                &mut assets,
            );
        }
    }
    fn spans(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        world
            .query::<&TextSpan>()
            .iter(world)
            .map(|s| s.0.clone())
            .collect()
    }
    #[test]
    fn leaderboard_ecs_mount_change_hide_destroy_and_session_cleanup() {
        let r = rules();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Font>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<BackdropImage>>()
            .insert_resource(CampaignLeaderboards {
                loaded: Some(Loaded {
                    source: source(&r),
                    rules: r,
                    viewport: [120., 90.],
                    backdrop: Backdrop {
                        background: None,
                        tile: false,
                        background_size: None,
                        mirrored: false,
                        inset: None,
                        edge: None,
                    },
                    font: Handle::default(),
                    images: HashMap::new(),
                    materials: Vec::new(),
                }),
                ..default()
            })
            .insert_resource(Snapshot {
                board: Some(board()),
                revision: 1,
                replace_session: false,
            })
            .add_systems(Update, tick);
        app.update();
        assert_eq!(spans(&mut app), ["earned count"]);
        assert!(
            app.world()
                .resource::<CampaignLeaderboards>()
                .last_error
                .is_none()
        );
        let first = app.world().resource::<CampaignLeaderboards>().root.unwrap();
        {
            let mut s = app.world_mut().resource_mut::<Snapshot>();
            s.board.as_mut().unwrap().items[0].label = "earned next".into();
            s.revision += 1;
        }
        app.update();
        assert_eq!(spans(&mut app), ["earned next"]);
        assert!(app.world().get_entity(first).is_err());
        app.world_mut()
            .resource_mut::<Snapshot>()
            .board
            .as_mut()
            .unwrap()
            .displayed = Some(false);
        app.update();
        assert!(spans(&mut app).is_empty());
        app.world_mut()
            .resource_mut::<Snapshot>()
            .board
            .as_mut()
            .unwrap()
            .displayed = Some(true);
        app.update();
        assert_eq!(spans(&mut app), ["earned next"]);
        app.world_mut().resource_mut::<Snapshot>().board = None;
        app.update();
        assert!(spans(&mut app).is_empty());
        app.world_mut().resource_mut::<Snapshot>().board = Some(board());
        app.update();
        assert_eq!(spans(&mut app), ["earned count"]);
        app.world_mut().resource_mut::<Snapshot>().replace_session = true;
        app.update();
        let state = app.world().resource::<CampaignLeaderboards>();
        assert!(state.root.is_none() && state.loaded.is_none() && state.revision.is_none());
        assert!(spans(&mut app).is_empty());
    }
    #[test]
    fn leaderboard_planner_refuses_unqualified_palette_styles_and_profiles() {
        let r = rules();
        let s = source(&r);
        let mut row = Row {
            label: "synthetic",
            label_color: None,
            names: Some(true),
            values: Some(false),
            icons: None,
        };
        assert!(s.plan_names(&r, [120., 90.], 1, &[row], false).is_err());
        row = Row {
            label: "synthetic",
            label_color: Some([3, 5, 7, 11]),
            names: Some(true),
            values: Some(true),
            icons: None,
        };
        assert!(s.plan_names(&r, [120., 90.], 1, &[row], false).is_err());
        assert!(s.plan_names(&r, [120., 90.], 1, &[], true).is_err());
        assert!(Source::parse("", "", "", "", &r).is_err());
    }
    #[test]
    #[ignore = "requires explicit PCX_WAR3_EXE, PCX_LEADERBOARD_FDF, PCX_LEADERBOARD_TEMPLATES, PCX_LEADERBOARD_SKIN and PCX_LEADERBOARD_MISCUI local assets"]
    fn leaderboard_selected_sources_plan_authored_name_only_branch() {
        let input = |name| std::fs::read(std::env::var_os(name).expect(name)).unwrap();
        let bytes = input("PCX_WAR3_EXE");
        let r = Rules::reviewed_legacy(
            &bytes,
            AssetIdentity {
                source: "explicit local qualification".into(),
                edition: "legacy".into(),
                version: "1.29.2.9231".into(),
            },
        )
        .unwrap();
        let string = |name| String::from_utf8(input(name)).unwrap();
        let s = Source::parse(
            &string("PCX_LEADERBOARD_FDF"),
            &string("PCX_LEADERBOARD_TEMPLATES"),
            &string("PCX_LEADERBOARD_SKIN"),
            &string("PCX_LEADERBOARD_MISCUI"),
            &r,
        )
        .unwrap();
        let backdrop = glue_menu::backdrop(&s.fdf, "LeaderboardBackdrop")
            .unwrap()
            .unwrap();
        assert!(backdrop.tile);
        let corner = backdrop.edge.as_ref().unwrap().corner_size;
        assert!(
            r.backdrop_tile_size(backdrop.background_size, corner)
                .unwrap()
                > 0.
        );
        let viewport = pcx_play::message_layout::Rules::reviewed_legacy(&bytes, r.origin.clone())
            .unwrap()
            .viewport;
        let plan = s
            .plan_names(
                &r,
                viewport,
                1,
                &[Row {
                    label: "synthetic qualification",
                    label_color: Some([7, 13, 23, 101]),
                    names: Some(true),
                    values: Some(false),
                    icons: Some(false),
                }],
                false,
            )
            .unwrap();
        assert_eq!(plan.rows.len(), 1);
        assert!(plan.placement.root.width > plan.rows[0].rect.width);
    }
}
