//! Player selection and orders on the published campaign session.
//!
//! Left click selects the nearest live unit whose source selection circle
//! (UnitUI `scale` times `[SelectionCircle] ScaleFactor`) covers the clicked
//! ground point; Shift adds. Right click attacks an enemy under the cursor or
//! moves to the ground point, for selected units the local player owns. Orders go
//! through the same order natives scripts use. Input is off while scripts disable
//! user control. Escape raises the local player's end-cinematic event.
use std::sync::{Arc, Weak};

use bevy::{ecs::system::SystemParam, prelude::*, window::PrimaryWindow};
use pcx_objects::editor::selection::SelectionCircles;
use pcx_play::{
    map_startup_host::MapStartupHost,
    player_command_log::{PlayerCommand, PlayerCommandLog},
    playercolor_host::{VisionCapability, VisionState},
};
use pcx_script::{HandleId, Host, Value};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet, CampaignSceneCamera},
    map_ground::GroundPublication,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};

const MISC_DATA: &str = "UI\\MiscData.txt";
const END_CINEMATIC: &str = "EVENT_PLAYER_END_CINEMATIC";
/// Bound on selection circle bands read from the profile.
const MAX_BANDS: usize = 16;
/// Ray-march stride and bisection count for cursor picking (numerical method only).
const MARCH_STEP: f32 = 16.0;
const REFINE_STEPS: usize = 24;

#[derive(Resource, Default)]
pub(crate) struct CampaignInput {
    pub circles: Option<SelectionCircles>,
    attempted: bool,
    session: Option<Weak<()>>,
    pub last_error: Option<String>,
    /// A command armed by its hotkey, waiting for a target click in the session that armed it.
    pub command: Option<ArmedCommand>,
    pub build: Option<crate::campaign_build_ui::Intent>,
    /// Accepted player commands of the prepared session, in clock order.
    pub commands: PlayerCommandLog,
}

/// Records an accepted command. A refused record is kept in `last_error`, not dropped.
fn record_command(input: &mut CampaignInput, clock: Option<f64>, command: PlayerCommand) {
    if let Err(error) = input.commands.record(clock, command) {
        input.last_error = Some(error.to_string());
    }
}

pub(crate) fn install(app: &mut App) {
    crate::button_actions::install(app);
    app.init_resource::<CampaignInput>()
        .init_resource::<crate::campaign_command_card::CommandCard>()
        .add_systems(
            Update,
            (
                (
                    (command_hotkey, load_circles)
                        .chain()
                        .before(crate::campaign_minimap::MinimapInputSet),
                    click.after(crate::campaign_minimap::MinimapInputSet),
                )
                    .chain()
                    .in_set(CampaignInputSet)
                    .after(crate::campaign_command_card::CommandCardSet)
                    .after(crate::campaign_hud::CampaignHudSet)
                    .after(crate::campaign_inventory::CampaignInventorySet)
                    .after(crate::campaign_script_dialogs::CampaignScriptDialogSet),
                end_cinematic
                    .after(crate::campaign_hud::CampaignHudSet)
                    .after(crate::campaign_script_dialogs::CampaignScriptDialogSet),
            ),
        )
        .add_systems(
            Update,
            command_button
                .after(CampaignInputSet)
                .after(crate::campaign_command_card::CommandCardSet)
                .before(crate::button_actions::clear),
        )
        .add_systems(Update, hud_menu.before(CampaignCameraSet))
        .add_systems(
            Update,
            use_inventory_slot
                .after(crate::campaign_hud::CampaignHudSet)
                .after(crate::campaign_inventory::CampaignInventorySet)
                .after(crate::campaign_script_dialogs::CampaignScriptDialogSet),
        );
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CampaignInputSet;

#[derive(SystemParam)]
struct CommandControls<'w> {
    view: Res<'w, CampaignCamera>,
    hud: Res<'w, crate::campaign_hud::CampaignHud>,
    inventory: Res<'w, crate::campaign_inventory::CampaignInventory>,
    dialogs: Res<'w, crate::campaign_script_dialogs::CampaignScriptDialogs>,
}

fn command_button(
    controls: CommandControls,
    actions: Res<crate::button_actions::ButtonActions>,
    buttons: Query<
        &crate::campaign_command_card::ButtonOrder,
        Without<bevy::ui::InteractionDisabled>,
    >,
    mut input: ResMut<CampaignInput>,
    mut card: ResMut<crate::campaign_command_card::CommandCard>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let witness = session.session_witness();
    let Some(host) = session.host() else { return };
    if !commands_available(&controls.view, &controls.hud, &controls.dialogs, host)
        || controls.inventory.pointer_consumed
        || host
            .objects()
            .presentation()
            .is_some_and(|state| state.interface_visible == Some(false))
    {
        return;
    }
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
        let objects = host.objects();
        if objects.selection().first() != Some(&button.unit)
            || objects
                .executed_unit(button.unit)
                .ok()
                .and_then(|record| objects.player_settings(record.owner).ok())
                .is_none_or(|player| Some(player.number) != objects.local_player_number())
        {
            continue;
        }
        input.build = None;
        let result = match button.kind {
            crate::campaign_command_card::Kind::CancelConstruction => {
                cancel_construction(host, &mut input, button.unit);
                continue;
            }
            crate::campaign_command_card::Kind::BuildMenu
            | crate::campaign_command_card::Kind::Build(_)
            | crate::campaign_command_card::Kind::BuildCancel => {
                if let Err(error) = activate_build(
                    host,
                    &mut input,
                    &mut card,
                    button.unit,
                    button.kind,
                    button.session.clone(),
                ) {
                    input.last_error = Some(error);
                }
                continue;
            }
            crate::campaign_command_card::Kind::TargetUnit => {
                match (
                    objects.holy_light_command(button.unit),
                    objects.chain_lightning_command(button.unit),
                ) {
                    (Err(error), _) | (_, Err(error)) => Err(error),
                    (Ok(holy), Ok(chain)) => {
                        let mut found = Vec::new();
                        if let Some(command) = holy {
                            found.push(command);
                        }
                        if let Some(command) = chain {
                            found.push(command);
                        }
                        Ok(match button.order {
                            Some(order) => found.into_iter().find(|command| command.order == order),
                            None => found.into_iter().next(),
                        })
                    }
                }
            }
            crate::campaign_command_card::Kind::Immediate => {
                objects.water_elemental_command(button.unit)
            }
            crate::campaign_command_card::Kind::Train(kind) => {
                input.command = None;
                immediate_command(host, &mut input, button.unit, kind);
                continue;
            }
            crate::campaign_command_card::Kind::Standard(kind) => {
                if let Err(error) =
                    activate_standard(host, &mut input, button.unit, kind, button.session.clone())
                {
                    input.last_error = Some(error);
                }
                continue;
            }
            crate::campaign_command_card::Kind::SkillMenu => {
                card.open_skills();
                if let Err(error) = host.objects_mut().fire_game_event("EVENT_GAME_SHOW_SKILL") {
                    input.last_error = Some(error.to_string());
                }
                continue;
            }
            crate::campaign_command_card::Kind::SkillCancel => {
                card.close_skills();
                continue;
            }
            crate::campaign_command_card::Kind::Learn(ability) => {
                learn_skill(host, &mut input, button.unit, ability);
                continue;
            }
        };
        match result {
            Ok(Some(command)) => match button.kind {
                crate::campaign_command_card::Kind::TargetUnit => {
                    input.command = Some(ArmedCommand {
                        order: command.order,
                        kind: crate::campaign_command_card::Kind::TargetUnit,
                        session: button.session.clone(),
                    });
                }
                crate::campaign_command_card::Kind::Immediate => {
                    input.command = None;
                    immediate_command(host, &mut input, button.unit, command.order);
                }
                crate::campaign_command_card::Kind::Standard(_)
                | crate::campaign_command_card::Kind::Train(_)
                | crate::campaign_command_card::Kind::SkillMenu
                | crate::campaign_command_card::Kind::SkillCancel
                | crate::campaign_command_card::Kind::Learn(_)
                | crate::campaign_command_card::Kind::BuildMenu
                | crate::campaign_command_card::Kind::Build(_)
                | crate::campaign_command_card::Kind::BuildCancel
                | crate::campaign_command_card::Kind::CancelConstruction => {
                    unreachable!("handled above")
                }
            },
            Err(error) => input.last_error = Some(error.to_string()),
            Ok(None) => {}
        }
    }
}

fn cancel_construction(host: &mut MapStartupHost, input: &mut CampaignInput, site: HandleId) {
    input.command = None;
    input.build = None;
    let objects = host.objects_mut();
    let clock = objects.timer_game_time();
    match objects.cancel_unit_construction(site) {
        Ok(true) => record_command(input, clock, PlayerCommand::CancelConstruction { site }),
        Ok(false) => {}
        Err(error) => input.last_error = Some(error.to_string()),
    }
}

fn activate_build(
    host: &mut MapStartupHost,
    input: &mut CampaignInput,
    card: &mut crate::campaign_command_card::CommandCard,
    worker: HandleId,
    kind: crate::campaign_command_card::Kind,
    session: Weak<()>,
) -> Result<(), String> {
    use crate::campaign_command_card::Kind;
    input.command = None;
    input.build = None;
    match kind {
        Kind::BuildMenu => {
            if host
                .objects()
                .unit_build_menu_command(worker)
                .map_err(|e| e.to_string())?
                .is_some()
            {
                card.open_builds();
            }
        }
        Kind::Build(kind) => {
            input.build =
                crate::campaign_build_ui::Intent::arm(host.objects(), worker, kind, session)?;
        }
        Kind::BuildCancel => card.close_skills(),
        _ => return Err("not a building command".into()),
    }
    Ok(())
}

/// Shared by the live ground click and headless command-surface tests.
pub(crate) fn place_build(
    host: &mut MapStartupHost,
    input: &mut CampaignInput,
    session: Option<&Weak<()>>,
    point: [f32; 2],
) -> Result<bool, String> {
    let Some(intent) = input.build.take() else {
        return Ok(false);
    };
    let objects = host.objects_mut();
    let clock = objects.timer_game_time();
    if let Some(accepted) = intent.commit(objects, session, point)? {
        record_command(
            input,
            clock,
            PlayerCommand::PointById {
                unit: accepted.worker,
                order: accepted.kind,
                x: accepted.position[0],
                y: accepted.position[1],
            },
        );
        Ok(true)
    } else {
        if intent.ready(objects, session)? {
            input.build = Some(intent);
        }
        Ok(false)
    }
}

fn immediate_command(
    host: &mut MapStartupHost,
    input: &mut CampaignInput,
    unit: HandleId,
    order: i32,
) {
    let objects = host.objects_mut();
    let clock = objects.timer_game_time();
    let result = objects.native(
        "IssueImmediateOrderById",
        &[Value::Handle(unit), Value::Int(order)],
    );
    record_order_result(
        input,
        clock,
        result,
        PlayerCommand::Immediate { unit, order },
    );
}

fn owned_standard_orders(
    objects: &pcx_play::playercolor_host::ObjectHost,
    kind: pcx_play::standard_command::Kind,
) -> Result<Vec<(HandleId, i32)>, String> {
    let local = objects
        .local_player_number()
        .ok_or("standard command has no local player")?;
    let mut orders = Vec::new();
    for &unit in objects.selection() {
        let record = objects.executed_unit(unit).map_err(|e| e.to_string())?;
        if objects
            .player_settings(record.owner)
            .map_err(|e| e.to_string())?
            .number
            != local
        {
            continue;
        }
        if let Some(order) = objects
            .standard_command_order(unit, kind)
            .map_err(|e| e.to_string())?
        {
            orders.push((unit, order));
        }
    }
    Ok(orders)
}

pub(crate) fn activate_standard(
    host: &mut MapStartupHost,
    input: &mut CampaignInput,
    unit: HandleId,
    kind: pcx_play::standard_command::Kind,
    session: Weak<()>,
) -> Result<(), String> {
    input.build = None;
    let Some(order) = host
        .objects()
        .standard_command_order(unit, kind)
        .map_err(|e| e.to_string())?
    else {
        return Ok(());
    };
    if kind.immediate() {
        input.command = None;
        for (unit, order) in owned_standard_orders(host.objects(), kind)? {
            immediate_command(host, input, unit, order);
        }
    } else {
        input.command = Some(ArmedCommand {
            order,
            kind: crate::campaign_command_card::Kind::Standard(kind),
            session,
        });
    }
    Ok(())
}

fn learn_skill(host: &mut MapStartupHost, input: &mut CampaignInput, unit: HandleId, ability: i32) {
    let alias = ability.to_be_bytes();
    let ready = match host.objects().skills_ready_to_learn(unit) {
        Ok(ready) => ready,
        Err(error) => {
            input.last_error = Some(error.to_string());
            return;
        }
    };
    if !ready.contains(&alias) {
        input.last_error = Some("shown skill is not ready to learn".into());
        return;
    }
    let clock = host.objects().timer_game_time();
    match host.objects_mut().native(
        "SelectHeroSkill",
        &[Value::Handle(unit), Value::Int(ability)],
    ) {
        Ok(Value::Null) => {
            let command = PlayerCommand::Learn { unit, ability };
            if let Err(error) = input.commands.record(clock, command) {
                input.last_error = Some(error.to_string());
            }
        }
        Ok(other) => input.last_error = Some(format!("SelectHeroSkill returned {other:?}")),
        Err(error) => input.last_error = Some(error.to_string()),
    }
}

fn record_order_result(
    input: &mut CampaignInput,
    clock: Option<f64>,
    result: Result<Value, String>,
    command: PlayerCommand,
) {
    match result {
        Ok(Value::Bool(true)) => record_command(input, clock, command),
        Err(error) => input.last_error = Some(error),
        _ => {}
    }
}

pub(crate) fn standard_target(
    objects: &mut pcx_play::playercolor_host::ObjectHost,
    input: &mut CampaignInput,
    kind: pcx_play::standard_command::Kind,
    point: [f32; 2],
    target: Option<HandleId>,
) -> Result<(), String> {
    use pcx_play::standard_command::Kind;
    if target.is_some() && kind == Kind::Patrol {
        return Err("Patrol imitation is not supported yet. Choose a ground point.".into());
    }
    for (unit, order) in owned_standard_orders(objects, kind)? {
        let Value::Str(name) = objects.native("OrderId2String", &[Value::Int(order)])? else {
            return Err("standard command has no admitted order name".into());
        };
        let clock = objects.timer_game_time();
        let (result, command) =
            if let Some(target) = target.filter(|_| matches!(kind, Kind::Attack | Kind::Move)) {
                (
                    objects.native(
                        "IssueTargetOrderById",
                        &[
                            Value::Handle(unit),
                            Value::Int(order),
                            Value::Handle(target),
                        ],
                    ),
                    PlayerCommand::Target {
                        unit,
                        order: name,
                        target,
                    },
                )
            } else {
                (
                    objects.native(
                        "IssuePointOrderById",
                        &[
                            Value::Handle(unit),
                            Value::Int(order),
                            Value::Real(point[0]),
                            Value::Real(point[1]),
                        ],
                    ),
                    PlayerCommand::Point {
                        unit,
                        order: name,
                        x: point[0],
                        y: point[1],
                    },
                )
            };
        record_order_result(input, clock, result, command);
    }
    Ok(())
}

/// Applies a pressed inventory slot as the local player's activation of the slot's held item.
/// The dialog, control and quest-log gates match the order path; the core command checks the
/// owner, the holder and the item's use.
fn use_inventory_slot(
    view: Res<CampaignCamera>,
    hud: Res<crate::campaign_hud::CampaignHud>,
    dialogs: Res<crate::campaign_script_dialogs::CampaignScriptDialogs>,
    mut inventory: ResMut<crate::campaign_inventory::CampaignInventory>,
    mut input: ResMut<CampaignInput>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let Some((unit, slot)) = inventory.use_request.take() else {
        return;
    };
    if !view.active() || view.menu_open() || hud.quest_log_open || dialogs.input_blocked() {
        return;
    }
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects_mut();
    if objects
        .presentation()
        .is_some_and(|p| p.control_enabled == Some(false))
    {
        return;
    }
    let Some(local) = objects.local_player_number() else {
        return;
    };
    if let Err(error) = objects.player_activate_slot(local, unit, slot) {
        input.last_error = Some(error.to_string());
    }
}

fn hud_menu(
    mut actions: MessageReader<crate::campaign_hud::CampaignHudAction>,
    mut quits: MessageReader<crate::campaign_script_dialogs::CampaignDialogQuit>,
    mut view: ResMut<CampaignCamera>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    for quit in quits.read() {
        if !session
            .session_witness()
            .is_some_and(|current| Weak::ptr_eq(&current, &quit.session))
        {
            continue;
        }
        let local = session
            .host()
            .and_then(|host| host.objects().local_player_settings())
            .map(|player| player.identity);
        if local != Some(quit.request.player) {
            continue;
        }
        if quit.request.score_screen {
            warn!(
                "script dialog quit requested a score screen; score presentation is not implemented"
            );
        }
        session.clear();
        view.suspend();
    }
    if actions
        .read()
        .any(|action| matches!(action, crate::campaign_hud::CampaignHudAction::Menu))
    {
        let open = !view.menu_open();
        view.set_menu_open(open);
    }
}

fn load_circles(
    mut input: ResMut<CampaignInput>,
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    session: Res<MapScriptSession<MapStartupHost>>,
) {
    let witness = session.session_witness();
    if !match (&input.session, &witness) {
        (Some(old), Some(new)) => Weak::ptr_eq(old, new),
        (None, None) => true,
        _ => false,
    } {
        // The command log belongs to the prepared session, begun before this session attached.
        let commands = std::mem::take(&mut input.commands);
        *input = CampaignInput {
            session: witness,
            commands,
            ..default()
        };
    }
    if input.attempted || session.context().is_none() {
        return;
    }
    let Some(bindings) = requests.bindings.as_ref() else {
        return;
    };
    if !requests
        .admission
        .shared_context()
        .as_ref()
        .zip(session.context())
        .is_some_and(|(admitted, active)| Arc::ptr_eq(admitted, active))
        || !requests
            .published_receipts
            .as_ref()
            .is_some_and(|published| Arc::ptr_eq(published, &bindings.receipts))
    {
        return;
    }
    let Some(reader) = sources.for_receipts(&bindings.receipts) else {
        return;
    };
    input.attempted = true;
    match reader
        .resolve_bounded(MISC_DATA, 2 << 20)
        .map_err(|e| e.to_string())
        .and_then(|asset| SelectionCircles::read(&asset.bytes, MISC_DATA, MAX_BANDS))
    {
        Ok(circles) => input.circles = Some(circles),
        Err(error) => input.last_error = Some(format!("{MISC_DATA}: {error}")),
    }
}

/// Escape raises the local player's `EVENT_PLAYER_END_CINEMATIC`; the original
/// raises it on every Escape press, not only during cinematics.
fn end_cinematic(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    view: Res<CampaignCamera>,
    hud: Res<crate::campaign_hud::CampaignHud>,
    dialogs: Res<crate::campaign_script_dialogs::CampaignScriptDialogs>,
    mut input: ResMut<CampaignInput>,
    mut card: ResMut<crate::campaign_command_card::CommandCard>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    if !keys.is_some_and(|k| k.just_pressed(KeyCode::Escape))
        || !view.active()
        || view.menu_open()
        || hud.quest_log_open
        || hud.quest_escape_consumed
        || dialogs.input_blocked()
    {
        return;
    }
    input.build = None;
    input.command = None;
    card.close_skills();
    let Some(host) = session.host() else {
        return;
    };
    let clock = host.objects_mut().timer_game_time();
    match host.objects_mut().fire_local_player_event(END_CINEMATIC) {
        Ok(fired) => {
            debug!("{END_CINEMATIC}: {fired} registrations queued");
            let command = PlayerCommand::LocalEvent {
                name: END_CINEMATIC.into(),
            };
            record_command(&mut input, clock, command);
        }
        Err(error) => input.last_error = Some(error.to_string()),
    }
}

/// `Err` means the click must stop before a selection or order write.
/// `Ok(None)` is an unadmitted world and keeps the previous pick.
fn pick_sight(
    objects: &pcx_play::playercolor_host::ObjectHost,
    input: &mut CampaignInput,
) -> Result<Option<(HandleId, bool, bool)>, ()> {
    match objects.vision_capability() {
        VisionCapability::Unadmitted => Ok(None),
        VisionCapability::Unavailable(reason) => {
            input.last_error = Some(reason.to_string());
            Err(())
        }
        VisionCapability::Ready { fog, mask } => match objects.local_player_settings() {
            Some(local) => Ok(Some((local.identity, fog, mask))),
            None => {
                input.last_error = Some("shared vision needs a local player".into());
                Err(())
            }
        },
    }
}

/// Ground point under the cursor, in game coordinates (x, y), by marching the
/// view ray down to the sampled terrain surface.
fn ground_point(ray: Ray3d, terrain: &pcx_map::Terrain, far: f32) -> Option<[f32; 2]> {
    let height = |p: Vec3| pcx_tera::world::sample_world_z(terrain, p.x, -p.z).map(|z| p.y - z);
    if !far.is_finite() || far <= 0.0 {
        return None;
    }
    let mut previous = None;
    let mut t = MARCH_STEP;
    while t <= far {
        if let Some(above) = height(ray.get_point(t)) {
            if above <= 0.0 {
                // Only bracket a surface crossing with two admitted samples.
                let (mut low, mut high) = (previous?, t);
                for _ in 0..REFINE_STEPS {
                    let mid = (low + high) * 0.5;
                    if height(ray.get_point(mid))? > 0.0 {
                        low = mid;
                    } else {
                        high = mid;
                    }
                }
                let hit = ray.get_point(high);
                return Some([hit.x, -hit.z]);
            }
            previous = Some(t);
        } else {
            previous = None;
        }
        let next = t + MARCH_STEP;
        if next <= t {
            return None;
        }
        t = next;
    }
    None
}

/// A unit command armed by its hotkey. It belongs to the session that armed it.
#[derive(Clone)]
pub(crate) struct ArmedCommand {
    order: i32,
    kind: crate::campaign_command_card::Kind,
    session: Weak<()>,
}

impl ArmedCommand {
    fn is_for(&self, session: Option<Weak<()>>) -> bool {
        session.is_some_and(|session| Weak::ptr_eq(&session, &self.session))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MinimapAction {
    Camera,
    PointOrder(pcx_play::standard_command::Kind),
    Cancel,
}

/// The minimap has point targets; unit-targeted spells cancel as on bare ground.
pub(crate) fn minimap_action(
    input: &mut CampaignInput,
    session: Option<Weak<()>>,
    primary: bool,
) -> MinimapAction {
    use pcx_play::standard_command::Kind;
    if input.build.take().is_some() {
        return MinimapAction::Cancel;
    }
    if let Some(armed) = input.command.take().filter(|a| a.is_for(session)) {
        return match armed.kind {
            crate::campaign_command_card::Kind::Standard(kind) if primary && !kind.immediate() => {
                MinimapAction::PointOrder(kind)
            }
            _ => MinimapAction::Cancel,
        };
    }
    if primary {
        MinimapAction::Camera
    } else {
        MinimapAction::PointOrder(Kind::Move)
    }
}

/// What a pointer press does while a command is armed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArmedPointer {
    /// No command is armed: the press is ordinary selection or orders.
    Pass,
    /// Right click, or a left click without a unit target, disarms the command.
    Cancel,
    /// A left click on a unit issues the armed order to the selected casters.
    Issue(i32),
}

fn armed_pointer(armed: Option<i32>, select: bool, right: bool, has_target: bool) -> ArmedPointer {
    match armed {
        None => ArmedPointer::Pass,
        Some(_) if right => ArmedPointer::Cancel,
        Some(order) if select && has_target => ArmedPointer::Issue(order),
        Some(_) if select => ArmedPointer::Cancel,
        Some(_) => ArmedPointer::Pass,
    }
}

/// Camera, HUD, inventory, dialogs and local control must all let input reach the session.
fn commands_open(
    view: &CampaignCamera,
    hud: &crate::campaign_hud::CampaignHud,
    inventory: &crate::campaign_inventory::CampaignInventory,
    dialogs: &crate::campaign_script_dialogs::CampaignScriptDialogs,
    host: &MapStartupHost,
) -> bool {
    !hud.pointer_consumed
        && !inventory.pointer_consumed
        && commands_available(view, hud, dialogs, host)
}

fn commands_available(
    view: &CampaignCamera,
    hud: &crate::campaign_hud::CampaignHud,
    dialogs: &crate::campaign_script_dialogs::CampaignScriptDialogs,
    host: &MapStartupHost,
) -> bool {
    let objects = host.objects();
    view.active()
        && !view.menu_open()
        && !hud.quest_log_open
        && !dialogs.input_blocked()
        && !objects
            .presentation()
            .is_some_and(|p| p.control_enabled == Some(false))
        && !objects.local_player_settings().is_some_and(|local| {
            objects
                .player_removal_state()
                .and_then(|state| state.get(local.identity))
                .is_some_and(|result| result.left)
        })
}

/// Dispatches authored standard and learned hotkeys. Closed input disarms targeting.
fn command_hotkey(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    controls: CommandControls,
    mut card: ResMut<crate::campaign_command_card::CommandCard>,
    mut input: ResMut<CampaignInput>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let CommandControls {
        view,
        hud,
        inventory,
        dialogs,
    } = controls;
    let Some(keys) = keys else {
        return;
    };
    let witness = session.session_witness();
    let Some(host) = session.host() else {
        input.command = None;
        input.build = None;
        return;
    };
    if !commands_open(&view, &hud, &inventory, &dialogs, host) {
        input.command = None;
        input.build = None;
        return;
    }
    let objects = host.objects();
    let local = objects.local_player_number();
    let primary = objects.selection().first().copied().filter(|&unit| {
        objects
            .executed_unit(unit)
            .ok()
            .and_then(|r| objects.player_settings(r.owner).ok())
            .is_some_and(|player| Some(player.number) == local)
    });
    if let Some(intent) = &input.build {
        match intent.current(objects, witness.as_ref()) {
            Ok(true) => {}
            Ok(false) => input.build = None,
            Err(error) => {
                input.build = None;
                input.last_error = Some(error);
            }
        }
    }
    if let Some(unit) = primary {
        if card
            .construction_cancel_hotkey(unit)
            .and_then(menu_key)
            .is_some_and(|key| keys.just_pressed(key))
        {
            cancel_construction(host, &mut input, unit);
            return;
        }
        let build = card
            .build_hotkeys(unit)
            .find(|(_, hotkey)| letter_key(*hotkey).is_some_and(|key| keys.just_pressed(key)));
        if let (Some((kind, _)), Some(witness)) = (build, witness.clone()) {
            if let Err(error) = activate_build(host, &mut input, &mut card, unit, kind, witness) {
                input.last_error = Some(error);
            }
            return;
        }
        for (kind, hotkey) in card.training_hotkeys(unit) {
            if letter_key(hotkey).is_some_and(|key| keys.just_pressed(key)) {
                input.command = None;
                input.build = None;
                immediate_command(host, &mut input, unit, kind);
                return;
            }
        }
        for profile in card.standard_profiles().filter(|_| !card.build_page_open()) {
            if letter_key(profile.hotkey).is_some_and(|key| keys.just_pressed(key))
                && host
                    .objects()
                    .standard_command_order(unit, profile.kind)
                    .ok()
                    .flatten()
                    .is_some()
                && let Some(session) = witness.clone()
            {
                if let Err(error) = activate_standard(host, &mut input, unit, profile.kind, session)
                {
                    input.last_error = Some(error);
                }
                return;
            }
        }
    }
    let objects = host.objects();
    if let Some(unit) = primary {
        match objects.water_elemental_command(unit) {
            Ok(Some(command))
                if letter_key(command.hotkey).is_some_and(|key| keys.just_pressed(key)) =>
            {
                input.command = None;
                input.build = None;
                immediate_command(host, &mut input, unit, command.order);
                return;
            }
            Err(error) => input.last_error = Some(error.to_string()),
            _ => {}
        }
    }
    let armed = objects
        .selection()
        .iter()
        .copied()
        .filter(|&u| {
            objects
                .executed_unit(u)
                .ok()
                .and_then(|r| objects.player_settings(r.owner).ok())
                .is_some_and(|p| Some(p.number) == local)
        })
        .filter_map(|u| {
            objects
                .holy_light_command(u)
                .inspect_err(|error| input.last_error = Some(error.to_string()))
                .ok()
                .flatten()
        })
        .find(|command| letter_key(command.hotkey).is_some_and(|k| keys.just_pressed(k)));
    if let (Some(command), Some(session)) = (armed, witness) {
        input.command = Some(ArmedCommand {
            order: command.order,
            kind: crate::campaign_command_card::Kind::TargetUnit,
            session,
        });
    }
}

fn menu_key(key: pcx_play::standard_command::MenuHotkey) -> Option<KeyCode> {
    use pcx_play::standard_command::MenuHotkey;
    match key {
        MenuHotkey::Letter(letter) => letter_key(letter),
        MenuHotkey::Code(code) if code == i32::from(b'\x1b') => Some(KeyCode::Escape),
        MenuHotkey::Code(_) => None,
    }
}

/// Keyboard key of an ASCII letter hotkey from the ability's `Hotkey` profile.
fn letter_key(letter: char) -> Option<KeyCode> {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KeyA,
        KeyCode::KeyB,
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyE,
        KeyCode::KeyF,
        KeyCode::KeyG,
        KeyCode::KeyH,
        KeyCode::KeyI,
        KeyCode::KeyJ,
        KeyCode::KeyK,
        KeyCode::KeyL,
        KeyCode::KeyM,
        KeyCode::KeyN,
        KeyCode::KeyO,
        KeyCode::KeyP,
        KeyCode::KeyQ,
        KeyCode::KeyR,
        KeyCode::KeyS,
        KeyCode::KeyT,
        KeyCode::KeyU,
        KeyCode::KeyV,
        KeyCode::KeyW,
        KeyCode::KeyX,
        KeyCode::KeyY,
        KeyCode::KeyZ,
    ];
    letter
        .is_ascii_uppercase()
        .then(|| LETTERS[usize::from(letter as u8 - b'A')])
}

#[derive(SystemParam)]
struct Picking<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    cameras: Query<
        'w,
        's,
        (
            &'static Camera,
            &'static GlobalTransform,
            &'static Projection,
        ),
        With<CampaignSceneCamera>,
    >,
    view: Res<'w, CampaignCamera>,
    ground: Res<'w, GroundPublication>,
    dialogs: Res<'w, crate::campaign_script_dialogs::CampaignScriptDialogs>,
    actors: Res<'w, crate::campaign_actors::UnitActors>,
    inventory: Res<'w, crate::campaign_inventory::CampaignInventory>,
}

fn click(
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    picking: Picking,
    hud: Res<crate::campaign_hud::CampaignHud>,
    mut input: ResMut<CampaignInput>,
    mut card: ResMut<crate::campaign_command_card::CommandCard>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let Picking {
        windows,
        cameras,
        view,
        ground,
        dialogs,
        actors,
        inventory,
    } = picking;
    let (Some(mouse), Some(circles)) = (mouse, input.circles.clone()) else {
        return;
    };
    let select = mouse.just_pressed(MouseButton::Left);
    let order = mouse.just_pressed(MouseButton::Right);
    if !(select || order)
        || !view.active()
        || view.menu_open()
        || hud.quest_log_open
        || hud.pointer_consumed
        || inventory.pointer_consumed
        || dialogs.input_blocked()
    {
        return;
    }
    let (Ok(window), Ok((camera, transform, projection))) = (windows.single(), cameras.single())
    else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if !camera
        .logical_viewport_rect()
        .is_some_and(|rect| rect.contains(cursor))
    {
        return;
    }
    let Ok(ray) = camera.viewport_to_world(transform, cursor) else {
        return;
    };
    let far = match projection {
        Projection::Perspective(p) => p.far,
        _ => return,
    };
    let Some(context) = ground.context.as_ref() else {
        return;
    };
    let Some(point) = ground_point(ray, &context.terrain, far) else {
        return;
    };
    let additive =
        keys.is_some_and(|k| k.pressed(KeyCode::ShiftLeft) || k.pressed(KeyCode::ShiftRight));
    let witness = session.session_witness();
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects_mut();
    let item_hit = witness
        .as_ref()
        .and_then(|witness| actors.item_hit(ray, far, witness, objects));
    if objects
        .presentation()
        .is_some_and(|p| p.control_enabled == Some(false))
    {
        return;
    }
    if objects.local_player_settings().is_some_and(|local| {
        objects
            .player_removal_state()
            .and_then(|state| state.get(local.identity))
            .is_some_and(|result| result.left)
    }) {
        return;
    }
    if input.build.is_some() {
        if order {
            input.build = None;
            card.close_skills();
        } else {
            match place_build(host, &mut input, witness.as_ref(), point) {
                Ok(true) => card.close_skills(),
                Ok(false) => {}
                Err(error) => input.last_error = Some(error),
            }
        }
        return;
    }
    let objects = host.objects_mut();
    let visibility = match pick_sight(objects, &mut input) {
        Ok(gate) => gate,
        Err(()) => return,
    };
    let mut vision_error = None;
    let picked = objects
        .executed_units()
        .filter(|(_, unit)| unit.shown && unit.combat.as_ref().is_none_or(|c| !c.dead))
        .filter_map(|(id, unit)| {
            let [x, y] = unit.position?;
            if let Some((viewer, fog, mask)) = visibility {
                match objects.vision_presented(viewer, [x, y], fog, mask) {
                    Ok(Some(VisionState::Visible)) => {}
                    Ok(_) => return None,
                    Err(error) => {
                        vision_error = Some(error.to_string());
                        return None;
                    }
                }
            }
            let scale = crate::campaign_actors::scalar(&unit.definition, "scale").ok()?;
            let distance = ((x - point[0]).powi(2) + (y - point[1]).powi(2)).sqrt();
            (distance <= scale * circles.scale_factor * 0.5).then_some((id, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id);
    if let Some(error) = vision_error {
        input.last_error = Some(error);
        return;
    }
    debug!(?point, ?picked, select, "campaign pointer hit");
    let standard = input
        .command
        .as_ref()
        .filter(|armed| armed.is_for(witness.clone()))
        .and_then(|armed| match armed.kind {
            crate::campaign_command_card::Kind::Standard(kind) => Some(kind),
            _ => None,
        });
    if let Some(kind) = standard {
        input.command = None;
        if select
            && !order
            && let Err(error) =
                standard_target(objects, &mut input, kind, point, picked.or(item_hit))
        {
            input.last_error = Some(error);
        }
        return;
    }
    let armed = input
        .command
        .as_ref()
        .filter(|armed| armed.is_for(witness.clone()))
        .map(|armed| armed.order);
    input.command = armed.and(input.command.take());
    match armed_pointer(armed, select, order, picked.is_some()) {
        ArmedPointer::Pass => {}
        ArmedPointer::Cancel => {
            input.command = None;
            return;
        }
        ArmedPointer::Issue(command) => {
            input.command = None;
            let Some(target) = picked else {
                return;
            };
            let local = objects.local_player_number();
            let casters: Vec<HandleId> = objects
                .selection()
                .iter()
                .copied()
                .filter(|&u| {
                    objects
                        .holy_light_command(u)
                        .ok()
                        .flatten()
                        .is_some_and(|c| c.order == command)
                        && objects
                            .executed_unit(u)
                            .ok()
                            .and_then(|r| objects.player_settings(r.owner).ok())
                            .is_some_and(|p| Some(p.number) == local)
                })
                .collect();
            for unit in casters {
                let order = match objects.native("OrderId2String", &[Value::Int(command)]) {
                    Ok(Value::Str(order)) => order,
                    _ => {
                        input.last_error = Some("command has no source order name".into());
                        continue;
                    }
                };
                let clock = objects.timer_game_time();
                let result = objects.native(
                    "IssueTargetOrderById",
                    &[
                        Value::Handle(unit),
                        Value::Int(command),
                        Value::Handle(target),
                    ],
                );
                record_order_result(
                    &mut input,
                    clock,
                    result,
                    PlayerCommand::Target {
                        unit,
                        order,
                        target,
                    },
                );
            }
            return;
        }
    }
    if select {
        if let Some(unit) = picked {
            objects.select_units(&[unit], !additive);
            let clock = objects.timer_game_time();
            let command = PlayerCommand::Select {
                units: vec![unit],
                replace: !additive,
            };
            record_command(&mut input, clock, command);
        }
        return;
    }
    let local = objects.local_player_number();
    let owned: Vec<HandleId> = objects
        .selection()
        .iter()
        .copied()
        .filter(|&u| {
            objects
                .executed_unit(u)
                .ok()
                .and_then(|r| objects.player_settings(r.owner).ok())
                .is_some_and(|p| Some(p.number) == local)
        })
        .collect();
    for unit in owned {
        let (result, command) = if let Some(item) = item_hit.filter(|&id| {
            objects
                .executed_item(id)
                .is_ok_and(|item| item.visible() == Some(true))
        }) {
            let result = objects.native(
                "IssueTargetOrder",
                &[
                    Value::Handle(unit),
                    Value::Str("smart".into()),
                    Value::Handle(item),
                ],
            );
            let command = PlayerCommand::Target {
                unit,
                order: "smart".into(),
                target: item,
            };
            (result, command)
        } else {
            use pcx_play::standard_command::Kind;
            let kind = if picked.is_some_and(|target| objects.is_enemy(unit, target)) {
                Kind::Attack
            } else {
                Kind::Move
            };
            let order = match objects.standard_command_order(unit, kind) {
                Ok(Some(order)) => order,
                Ok(None) => continue,
                Err(error) => {
                    input.last_error = Some(error.to_string());
                    continue;
                }
            };
            let name = match objects.native("OrderId2String", &[Value::Int(order)]) {
                Ok(Value::Str(name)) => name,
                Err(error) => {
                    input.last_error = Some(error);
                    continue;
                }
                _ => {
                    input.last_error = Some("standard command has no admitted order name".into());
                    continue;
                }
            };
            match picked {
                Some(target) => {
                    let result = objects.native(
                        "IssueTargetOrderById",
                        &[
                            Value::Handle(unit),
                            Value::Int(order),
                            Value::Handle(target),
                        ],
                    );
                    let command = PlayerCommand::Target {
                        unit,
                        order: name,
                        target,
                    };
                    (result, command)
                }
                None => {
                    let result = objects.native(
                        "IssuePointOrderById",
                        &[
                            Value::Handle(unit),
                            Value::Int(order),
                            Value::Real(point[0]),
                            Value::Real(point[1]),
                        ],
                    );
                    let command = PlayerCommand::Point {
                        unit,
                        order: name,
                        x: point[0],
                        y: point[1],
                    };
                    (result, command)
                }
            }
        };
        record_order_result(&mut input, objects.timer_game_time(), result, command);
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
pub(crate) fn run_command_button(app: &mut App) {
    use bevy::ecs::system::RunSystemOnce;
    app.world_mut().run_system_once(command_button).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "test-fixtures", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires unchanged Human01 and selected PCX_NATIVE_CHAPTER_* inputs"]
    fn selected_chapter_training_button_enqueues_and_rejects_stale_session() {
        use crate::campaign_command_card::{ButtonOrder, Kind};
        use bevy::{
            ecs::system::RunSystemOnce,
            ui_widgets::{Activate, Button},
        };
        let mut app = crate::native_campaign::tests::started_app();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(CampaignCamera::active_for_test(camera))
            .init_resource::<crate::campaign_hud::CampaignHud>()
            .init_resource::<crate::campaign_inventory::CampaignInventory>()
            .init_resource::<crate::campaign_script_dialogs::CampaignScriptDialogs>();
        let (producer, kind, witness, before) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let states = [
                "PLAYER_STATE_RESOURCE_GOLD",
                "PLAYER_STATE_RESOURCE_LUMBER",
                "PLAYER_STATE_RESOURCE_FOOD_CAP",
            ]
            .map(|name| session.global(name).unwrap());
            let witness = session.session_witness().unwrap();
            let objects = session.host().unwrap().objects_mut();
            let local = objects.local_player_number().unwrap();
            let (owner, position) = objects
                .executed_units()
                .find_map(|(_, unit)| {
                    (objects.player_settings(unit.owner).unwrap().number == local)
                        .then_some((unit.owner, unit.position.unwrap_or([unit.x, unit.y])))
                })
                .unwrap();
            // Diagnostic producer and facing; presentation and training values remain catalog-derived.
            let Value::Handle(producer) = objects
                .native(
                    "CreateUnit",
                    &[
                        Value::Handle(owner),
                        Value::Int(i32::from_be_bytes(*b"hbar")),
                        Value::Real(position[0]),
                        Value::Real(position[1]),
                        Value::Real(0.),
                    ],
                )
                .unwrap()
            else {
                panic!("producer handle")
            };
            let kinds = pcx_play::training_command::types(
                &objects.executed_unit(producer).unwrap().definition,
            )
            .unwrap();
            let kind = i32::from_be_bytes(kinds[0]);
            objects
                .native(
                    "SetPlayerTechMaxAllowed",
                    &[Value::Handle(owner), Value::Int(kind), Value::Int(100)],
                )
                .unwrap();
            for (state, amount) in states.into_iter().zip([10000, 10000, 100]) {
                objects
                    .native(
                        "SetPlayerState",
                        &[Value::Handle(owner), state, Value::Int(amount)],
                    )
                    .unwrap();
            }
            objects
                .native("EnableUserControl", &[Value::Bool(true)])
                .unwrap();
            objects
                .native("ShowInterface", &[Value::Bool(true), Value::Real(0.)])
                .unwrap();
            objects.native("PauseGame", &[Value::Bool(false)]).unwrap();
            objects
                .native("PauseUnit", &[Value::Handle(producer), Value::Bool(false)])
                .unwrap();
            objects.select_units(&[producer], true);
            let before = objects
                .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
                .unwrap()
                .unwrap();
            (producer, kind, witness, (owner, before))
        };
        let start = {
            let seed = app
                .world()
                .resource::<peacecraft3::campaign_startup::CampaignSeed>()
                .prepared
                .unwrap();
            let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
            pcx_play::player_command_log::StartIdentity::from_context(
                seed,
                session.context().unwrap(),
            )
        };
        app.world_mut()
            .resource_mut::<CampaignInput>()
            .commands
            .begin(start);
        let button = app
            .world_mut()
            .spawn((
                Button,
                ButtonOrder {
                    unit: producer,
                    kind: Kind::Train(kind),
                    session: witness,
                    order: None,
                },
            ))
            .id();
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().run_system_once(command_button).unwrap();
        assert!(
            app.world().resource::<CampaignInput>().last_error.is_none(),
            "{:?}",
            app.world().resource::<CampaignInput>().last_error
        );
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            assert_eq!(objects.unit_training_queue(producer).unwrap().len(), 1);
            assert_eq!(objects.unit_training_queue(producer).unwrap()[0].0, kind);
            assert!(
                objects
                    .player_resource_value(before.0, "PLAYER_STATE_RESOURCE_GOLD")
                    .unwrap()
                    .unwrap()
                    < before.1
            );
        }
        assert_eq!(
            app.world()
                .resource::<CampaignInput>()
                .commands
                .entries()
                .len(),
            1
        );
        app.world_mut()
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities
            .clear();
        let foreign = Arc::new(());
        app.world_mut()
            .get_mut::<ButtonOrder>(button)
            .unwrap()
            .session = Arc::downgrade(&foreign);
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().run_system_once(command_button).unwrap();
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert_eq!(
            session
                .host()
                .unwrap()
                .objects()
                .unit_training_queue(producer)
                .unwrap()
                .len(),
            1
        );
    }

    #[cfg(all(feature = "test-fixtures", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires unchanged Human01 and selected PCX_NATIVE_CHAPTER_* inputs"]
    fn selected_chapter_standard_buttons_act_on_owned_selection_and_recheck_control() {
        use crate::campaign_command_card::{ButtonOrder, Kind};
        use bevy::{
            ecs::system::RunSystemOnce,
            ui_widgets::{Activate, Button},
        };
        use pcx_play::standard_command::Kind as Standard;
        let mut app = crate::native_campaign::tests::started_app();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(CampaignCamera::active_for_test(camera))
            .init_resource::<crate::campaign_hud::CampaignHud>()
            .init_resource::<crate::campaign_inventory::CampaignInventory>()
            .init_resource::<crate::campaign_script_dialogs::CampaignScriptDialogs>();
        let (units, witness) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let witness = session.session_witness().unwrap();
            let objects = session.host().unwrap().objects_mut();
            let local = objects.local_player_number().unwrap();
            let units: Vec<_> = objects
                .executed_units()
                .filter(|(_, record)| {
                    record.shown
                        && record.activity.as_ref().is_some_and(|a| a.movable())
                        && record.combat.as_ref().is_some_and(|c| !c.dead)
                        && objects.player_settings(record.owner).unwrap().number == local
                })
                .map(|(id, _)| id)
                .take(2)
                .collect();
            assert_eq!(units.len(), 2, "two real owned movable units required");
            objects
                .native("EnableUserControl", &[Value::Bool(true)])
                .unwrap();
            objects
                .native("ShowInterface", &[Value::Bool(true), Value::Real(0.)])
                .unwrap();
            objects.native("PauseGame", &[Value::Bool(false)]).unwrap();
            for &unit in &units {
                objects
                    .native("PauseUnit", &[Value::Handle(unit), Value::Bool(false)])
                    .unwrap();
            }
            objects.select_units(&units, true);
            (units, witness)
        };
        let start = {
            let seed = app
                .world()
                .resource::<peacecraft3::campaign_startup::CampaignSeed>()
                .prepared
                .unwrap();
            let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
            pcx_play::player_command_log::StartIdentity::from_context(
                seed,
                session.context().unwrap(),
            )
        };
        app.world_mut()
            .resource_mut::<CampaignInput>()
            .commands
            .begin(start);
        let button = app
            .world_mut()
            .spawn((
                Button,
                ButtonOrder {
                    unit: units[0],
                    kind: Kind::Standard(Standard::Hold),
                    session: witness.clone(),
                    order: None,
                },
            ))
            .id();
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().run_system_once(command_button).unwrap();
        assert_eq!(
            app.world()
                .resource::<CampaignInput>()
                .commands
                .entries()
                .len(),
            2
        );
        app.world_mut()
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities
            .clear();
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            for &unit in &units {
                assert!(
                    objects
                        .executed_unit(unit)
                        .unwrap()
                        .activity
                        .as_ref()
                        .unwrap()
                        .held()
                );
            }
            objects
                .native("EnableUserControl", &[Value::Bool(false)])
                .unwrap();
        }
        app.world_mut()
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities
            .clear();
        app.world_mut().get_mut::<ButtonOrder>(button).unwrap().kind =
            Kind::Standard(Standard::Stop);
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().run_system_once(command_button).unwrap();
        assert_eq!(
            app.world()
                .resource::<CampaignInput>()
                .commands
                .entries()
                .len(),
            2
        );
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            objects
                .native("EnableUserControl", &[Value::Bool(true)])
                .unwrap();
        }
        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().run_system_once(command_button).unwrap();
        assert_eq!(
            app.world()
                .resource::<CampaignInput>()
                .commands
                .entries()
                .len(),
            4
        );
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let objects = session.host().unwrap().objects_mut();
        for &unit in &units {
            assert!(
                !objects
                    .executed_unit(unit)
                    .unwrap()
                    .activity
                    .as_ref()
                    .unwrap()
                    .held()
            );
        }
        assert!(
            standard_target(
                objects,
                &mut CampaignInput::default(),
                Standard::Patrol,
                [0., 0.],
                Some(units[0])
            )
            .is_err()
        );
        app.world_mut()
            .resource_scope(|world, mut input: Mut<CampaignInput>| {
                let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
                let objects = session.host().unwrap().objects_mut();
                standard_target(
                    objects,
                    &mut input,
                    Standard::Move,
                    [0.0, 0.0],
                    Some(units[1]),
                )
                .unwrap();
                assert_eq!(
                    objects
                        .executed_unit(units[0])
                        .unwrap()
                        .activity
                        .as_ref()
                        .unwrap()
                        .followed_unit(),
                    Some(units[1])
                );
                assert_eq!(
                    input.commands.entries().len(),
                    5,
                    "self-target must not add an accepted order"
                );
            });
    }

    #[test]
    fn only_accepted_orders_enter_the_player_journal() {
        let mut input = CampaignInput::default();
        let identity = |source: &str| pcx_play::scripts::AssetIdentity {
            source: source.into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        };
        input
            .commands
            .begin(pcx_play::player_command_log::StartIdentity {
                seed: 7,
                map_label: "synthetic command map".into(),
                map_sha256: [0x5a; 32],
                sources: pcx_play::map_admission::MapInputSources {
                    map: identity("map"),
                    common: identity("common"),
                    blizzard: identity("blizzard"),
                    catalog: identity("catalog"),
                },
            });
        let unit = HandleId::new(std::num::NonZeroU64::new(7).unwrap());
        let command = PlayerCommand::Immediate { unit, order: 9 };
        record_order_result(
            &mut input,
            Some(1.),
            Ok(Value::Bool(false)),
            command.clone(),
        );
        assert!(input.commands.entries().is_empty());
        record_order_result(
            &mut input,
            Some(1.),
            Err("fixture refusal".into()),
            command.clone(),
        );
        assert!(input.commands.entries().is_empty());
        assert_eq!(input.last_error.as_deref(), Some("fixture refusal"));
        record_order_result(&mut input, Some(1.), Ok(Value::Bool(true)), command.clone());
        assert_eq!(input.commands.entries().len(), 1);
        assert_eq!(input.commands.entries()[0].command, command);
    }

    #[cfg(all(feature = "test-fixtures", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires selected Human01 chapter inputs PCX_NATIVE_CHAPTER_* and local assets"]
    fn selected_chapter_command_click_rechecks_session_selection_and_control() {
        use crate::campaign_command_card::{ButtonOrder, Kind};
        use bevy::{
            ecs::system::RunSystemOnce,
            ui_widgets::{Activate, Button},
        };
        let mut app = crate::native_campaign::tests::started_app();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(CampaignCamera::active_for_test(camera))
            .init_resource::<crate::campaign_hud::CampaignHud>()
            .init_resource::<crate::campaign_inventory::CampaignInventory>()
            .init_resource::<crate::campaign_script_dialogs::CampaignScriptDialogs>();
        let (unit, witness, order) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let unit = match session.global("gg_unit_Hart_0023") {
                Some(Value::Handle(unit)) => unit,
                other => panic!("this explicit gate requires unchanged Human01 Arthas: {other:?}"),
            };
            let witness = session.session_witness().unwrap();
            let objects = session.host().unwrap().objects_mut();
            // Operator setup for the command component test, not campaign progression evidence.
            objects
                .native("EnableUserControl", &[Value::Bool(true)])
                .unwrap();
            objects
                .native("ShowInterface", &[Value::Bool(true), Value::Real(0.)])
                .unwrap();
            if objects.holy_light_command(unit).unwrap().is_none() {
                objects
                    .native(
                        "SelectHeroSkill",
                        &[
                            Value::Handle(unit),
                            Value::Int(i32::from_be_bytes(*b"AHhb")),
                        ],
                    )
                    .unwrap();
            }
            for old in objects.selection().to_vec() {
                objects
                    .native("SelectUnit", &[Value::Handle(old), Value::Bool(false)])
                    .unwrap();
            }
            objects
                .native("SelectUnit", &[Value::Handle(unit), Value::Bool(true)])
                .unwrap();
            let order = objects
                .holy_light_command(unit)
                .unwrap()
                .expect("source-learned Holy Light")
                .order;
            (unit, witness, order)
        };
        let other_session = Arc::new(());
        let stale = app
            .world_mut()
            .spawn((
                Button,
                ButtonOrder {
                    unit,
                    kind: Kind::TargetUnit,
                    session: Arc::downgrade(&other_session),
                    order: None,
                },
            ))
            .id();
        app.world_mut().trigger(Activate { entity: stale });
        app.world_mut().run_system_once(command_button).unwrap();
        assert!(app.world().resource::<CampaignInput>().command.is_none());
        app.world_mut()
            .resource_mut::<crate::button_actions::ButtonActions>()
            .entities
            .clear();
        let current = app
            .world_mut()
            .spawn((
                Button,
                ButtonOrder {
                    unit,
                    kind: Kind::TargetUnit,
                    session: witness.clone(),
                    order: None,
                },
            ))
            .id();
        app.world_mut().trigger(Activate { entity: current });
        app.world_mut().run_system_once(command_button).unwrap();
        let armed = app
            .world()
            .resource::<CampaignInput>()
            .command
            .as_ref()
            .unwrap();
        assert_eq!(armed.order, order);
        assert!(armed.is_for(Some(witness)));
        app.world_mut().resource_mut::<CampaignInput>().command = None;
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            session
                .host()
                .unwrap()
                .objects_mut()
                .native("EnableUserControl", &[Value::Bool(false)])
                .unwrap();
        }
        app.world_mut().run_system_once(command_button).unwrap();
        assert!(app.world().resource::<CampaignInput>().command.is_none());
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            objects
                .native("EnableUserControl", &[Value::Bool(true)])
                .unwrap();
            objects
                .native("SelectUnit", &[Value::Handle(unit), Value::Bool(false)])
                .unwrap();
        }
        app.world_mut().run_system_once(command_button).unwrap();
        assert!(app.world().resource::<CampaignInput>().command.is_none());
    }

    fn terrain() -> pcx_map::Terrain {
        let mut bytes = b"W3E!".to_vec();
        bytes.extend(11u32.to_le_bytes());
        bytes.push(b'L');
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(b"Ldrt");
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(b"CLdi");
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(0f32.to_le_bytes());
        bytes.extend(0f32.to_le_bytes());
        for _ in 0..4 {
            bytes.extend(8232u16.to_le_bytes());
            bytes.extend(8192u16.to_le_bytes());
            bytes.extend([0, 0, 0]);
        }
        pcx_map::Terrain::from_bytes(bytes).unwrap()
    }

    #[test]
    fn clearing_session_discards_the_previous_selection_source_attempt() {
        let witness = std::sync::Arc::new(());
        let mut app = App::new();
        app.insert_resource(CampaignInput {
            attempted: true,
            session: Some(std::sync::Arc::downgrade(&witness)),
            last_error: Some("old source".into()),
            ..default()
        })
        .init_resource::<MapRequests>()
        .init_resource::<SelectedMapSources>()
        .init_resource::<MapScriptSession<MapStartupHost>>()
        .add_systems(Update, load_circles);
        app.update();
        let input = app.world().resource::<CampaignInput>();
        assert!(!input.attempted);
        assert!(input.session.is_none());
        assert!(input.last_error.is_none());
    }

    #[test]
    fn cursor_rays_land_on_the_sampled_terrain_surface() {
        let terrain = terrain();
        let surface = pcx_tera::world::sample_world_z(&terrain, 40.0, 70.0).unwrap();
        // Slanted ray in render space (x, z, -y) aimed at game point (40, 70).
        let target = Vec3::new(40.0, surface, -70.0);
        let origin = target + Vec3::new(-30.0, 200.0, 45.0);
        let ray = Ray3d::new(origin, Dir3::new(target - origin).unwrap());
        let [x, y] = ground_point(ray, &terrain, 5000.0).unwrap();
        assert!(
            (x - 40.0).abs() < 0.01 && (y - 70.0).abs() < 0.01,
            "{x} {y}"
        );
        let up = Ray3d::new(origin, Dir3::Y);
        assert_eq!(ground_point(up, &terrain, 5000.0), None);
        // Leaving the admitted terrain is no hit, not a clamped guess.
        let away = Ray3d::new(origin, Dir3::new(Vec3::new(-1.0, -0.01, 0.0)).unwrap());
        assert_eq!(ground_point(away, &terrain, 5000.0), None);
    }
    #[test]
    fn cursor_rays_can_enter_the_map_before_crossing_its_surface() {
        let terrain = terrain();
        let surface = pcx_tera::world::sample_world_z(&terrain, 40.0, 70.0).unwrap();
        let target = Vec3::new(40.0, surface, -70.0);
        for offset in [Vec3::new(-200.0, 300.0, 0.0), Vec3::new(0.0, 300.0, 200.0)] {
            let origin = target + offset;
            let ray = Ray3d::new(origin, Dir3::new(target - origin).unwrap());
            let [x, y] = ground_point(ray, &terrain, 5000.0).unwrap();
            assert!((x - 40.0).abs() < 0.01 && (y - 70.0).abs() < 0.01);
            for far in [50.0, 0.0, f32::NAN, f32::INFINITY] {
                assert_eq!(ground_point(ray, &terrain, far), None);
            }
        }
        // Entering below the surface is not a surface intersection.
        let origin = target + Vec3::new(-200.0, -50.0, 0.0);
        assert_eq!(
            ground_point(Ray3d::new(origin, Dir3::X), &terrain, 5000.0),
            None
        );
    }

    #[test]
    fn armed_pointer_issues_on_a_unit_click_and_cancels_on_right_or_ground_clicks() {
        assert_eq!(armed_pointer(None, true, false, true), ArmedPointer::Pass);
        assert_eq!(
            armed_pointer(Some(9), true, false, true),
            ArmedPointer::Issue(9)
        );
        assert_eq!(
            armed_pointer(Some(9), true, false, false),
            ArmedPointer::Cancel
        );
        assert_eq!(
            armed_pointer(Some(9), false, true, true),
            ArmedPointer::Cancel
        );
        assert_eq!(
            armed_pointer(Some(9), false, false, false),
            ArmedPointer::Pass
        );
    }

    #[test]
    fn armed_command_belongs_only_to_the_session_that_armed_it() {
        let first = Arc::new(());
        let second = Arc::new(());
        let armed = ArmedCommand {
            order: 9,
            kind: crate::campaign_command_card::Kind::TargetUnit,
            session: Arc::downgrade(&first),
        };
        assert!(armed.is_for(Some(Arc::downgrade(&first))));
        assert!(!armed.is_for(Some(Arc::downgrade(&second))));
        assert!(!armed.is_for(None));
    }

    #[test]
    fn minimap_selects_point_orders_camera_or_cancellation_without_reusing_stale_commands() {
        use crate::campaign_command_card::Kind;
        use pcx_play::standard_command::Kind as Standard;
        let live = Arc::new(());
        let stale = Arc::new(());
        let mut input = CampaignInput::default();
        assert_eq!(
            minimap_action(&mut input, Some(Arc::downgrade(&live)), true),
            MinimapAction::Camera
        );
        assert_eq!(
            minimap_action(&mut input, Some(Arc::downgrade(&live)), false),
            MinimapAction::PointOrder(Standard::Move)
        );
        for kind in [Standard::Move, Standard::Attack, Standard::Patrol] {
            input.command = Some(ArmedCommand {
                order: 99,
                kind: Kind::Standard(kind),
                session: Arc::downgrade(&live),
            });
            assert_eq!(
                minimap_action(&mut input, Some(Arc::downgrade(&live)), true),
                MinimapAction::PointOrder(kind)
            );
            assert!(input.command.is_none());
            input.command = Some(ArmedCommand {
                order: 99,
                kind: Kind::Standard(kind),
                session: Arc::downgrade(&live),
            });
            assert_eq!(
                minimap_action(&mut input, Some(Arc::downgrade(&live)), false),
                MinimapAction::Cancel
            );
        }
        for kind in [Kind::TargetUnit, Kind::Immediate] {
            input.command = Some(ArmedCommand {
                order: 99,
                kind,
                session: Arc::downgrade(&live),
            });
            assert_eq!(
                minimap_action(&mut input, Some(Arc::downgrade(&live)), true),
                MinimapAction::Cancel
            );
            assert!(input.command.is_none());
        }
        input.command = Some(ArmedCommand {
            order: 99,
            kind: Kind::Standard(Standard::Attack),
            session: Arc::downgrade(&stale),
        });
        assert_eq!(
            minimap_action(&mut input, Some(Arc::downgrade(&live)), true),
            MinimapAction::Camera
        );
        assert!(input.command.is_none());
    }

    #[test]
    fn hotkey_letters_map_to_keys_and_other_characters_do_not() {
        assert_eq!(letter_key('T'), Some(KeyCode::KeyT));
        assert_eq!(letter_key('Z'), Some(KeyCode::KeyZ));
        assert_eq!(letter_key('1'), None);
        assert_eq!(letter_key('t'), None);
        assert_eq!(
            menu_key(pcx_play::standard_command::MenuHotkey::Code(i32::from(
                b'\x1b'
            ))),
            Some(KeyCode::Escape)
        );
        assert_eq!(
            menu_key(pcx_play::standard_command::MenuHotkey::Code(999)),
            None
        );
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "test-fixtures"))]
    #[test]
    fn unavailable_ledger_refuses_a_pick_before_an_order() {
        use pcx_play::map_startup_host::SettingsObservation;
        const EXTRA: &str = "type mapflag extends handle
constant native ConvertMapFlag takes integer i returns mapflag
native SetMapFlag takes mapflag flag, boolean value returns nothing
native IsMapFlagSet takes mapflag flag returns boolean
type gamespeed extends handle
constant native ConvertGameSpeed takes integer i returns gamespeed
constant native GetGameSpeed takes nothing returns gamespeed
native SetGameSpeed takes gamespeed speed returns nothing
native IsFogEnabled takes nothing returns boolean
native IsFogMaskEnabled takes nothing returns boolean
native FogEnable takes boolean enabled returns nothing
native FogMaskEnable takes boolean enabled returns nothing
globals
constant mapflag MAP_LOCK_SPEED = ConvertMapFlag(17*19)
constant gamespeed CURRENT_SPEED = ConvertGameSpeed(-17)
endglobals
";
        let mut app = App::new();
        peacecraft3::map_requests::install(&mut app);
        peacecraft3::campaign_startup::install(&mut app);
        let mut fixture = crate::campaign_startup_cases::fixture(
            "function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n",
        );
        fixture.common.extend_from_slice(EXTRA.as_bytes());
        fixture.rebuild();
        let resolver = fixture.resolver();
        let bound =
            peacecraft3::map_binding::bind_map(&resolver, fixture.input(1), fixture.limits())
                .unwrap();
        let mut inputs =
            crate::campaign_startup_cases::host_inputs(std::sync::Arc::clone(&bound.catalog.0));
        inputs
            .enumeration
            .names
            .extend(["ConvertGameSpeed".into(), "ConvertMapFlag".into()]);
        inputs.post_config.settings = Some(SettingsObservation {
            speed_global: "CURRENT_SPEED".into(),
            fog_enabled: true,
            fog_mask_enabled: true,
        });
        let bytes = std::sync::Arc::clone(&bound.catalog_map_bytes);
        app.insert_resource(peacecraft3::map_requests::MapRequests {
            bindings: Some(bound),
            ..default()
        })
        .insert_resource(peacecraft3::campaign_startup::CampaignHostSupply {
            revision: 1,
            inputs: Some(inputs),
        });
        app.world_mut()
            .write_message(peacecraft3::map_requests::MapRequest {
                label: "synthetic pick refusal".into(),
                map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
                identity: fixture.map_id.clone(),
                binding_revision: 1,
                payload: peacecraft3::map_requests::MapPayload::Shared(bytes),
            });
        app.update();
        assert!(
            app.world()
                .resource::<peacecraft3::campaign_startup::CampaignStartup>()
                .last_error
                .is_none(),
            "{:?}",
            app.world()
                .resource::<peacecraft3::campaign_startup::CampaignStartup>()
                .last_error
        );
        let witness = Arc::new(());
        let mut input = CampaignInput {
            command: Some(ArmedCommand {
                order: 7,
                kind: crate::campaign_command_card::Kind::TargetUnit,
                session: Arc::downgrade(&witness),
            }),
            ..default()
        };
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            let selected = objects.selection().to_vec();
            assert!(matches!(
                objects.vision_capability(),
                VisionCapability::Unavailable(_)
            ));
            assert!(pick_sight(objects, &mut input).is_err());
            assert_eq!(objects.selection(), selected.as_slice());
        }
        assert!(
            input
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("shared vision")),
            "{:?}",
            input.last_error
        );
        assert!(input.command.is_some());
        assert!(input.commands.entries().is_empty());
    }
}
