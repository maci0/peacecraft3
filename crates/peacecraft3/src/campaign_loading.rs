//! Application state of the campaign loading screen, from the mission launch to
//! the first playable frame. The screen's content comes from
//! `pcx_play::campaign_loading`; this module owns when it is shown, how its bar
//! advances and when the held simulation is released. See
//! `docs/re-notes/campaign-loading-screen.md`.
use crate::{
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_startup::{CampaignHostSupply, CampaignStartup, CampaignStartupSet, StartupError},
    map_ground::GroundPublication,
    map_requests::{MapRequestSet, MapRequests},
    map_script_session::MapScriptSession,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use pcx_assets::Resolver;
use pcx_map::info::MapInfo;
use pcx_play::{
    campaign_loading::{
        MeleeGameType, MeleePanel, MinDisplay, Progress, ProgressRules, Screen, ScreenTable,
        ScreenText, Selection, StringTable, TABLE_PATH, resolve_text, select,
    },
    map_startup_host::MapStartupHost,
};

use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

/// Upper bound on the table read through the selected resolver.
const TABLE_BYTES: u64 = 4 << 20;
/// Upper bound on the skin table read when the melee background is selected.
const SKIN_BYTES: u64 = 4 << 20;
/// Upper bound on one map-supplied member (loading model, minimap image).
const MAP_MEMBER_BYTES: usize = 16 << 20;
/// Recreation choice: the bar advances in equal shares at these load stages.
/// The original's own stage weights are code immediates that are not attributed
/// to one load path.
const STAGES: usize = 4;
/// Skin table that carries the race-specific melee backgrounds.
const SKIN_PATH: &str = r"UI\war3skins.txt";
/// Skin key of the melee background model. The table carries one entry per
/// race section plus a `[Default]` fallback; which entry the original picks is
/// not decoded, so the local race selects the section.
const MELEE_BACKGROUND_KEY: &str = "LoadingMeleeBackground";

/// A map-supplied loading model, extracted from the map itself.
#[derive(Clone, Debug)]
pub struct CustomModel {
    /// Member path as the header names it.
    pub path: String,
    pub bytes: Vec<u8>,
}

/// One melee player row with its authored race and controller words.
#[derive(Clone, Debug)]
pub struct MeleeSlotView {
    pub name: String,
    pub race: u32,
    pub controller: u32,
}

/// The melee panel's minimap image, extracted from the map itself.
#[derive(Clone, Debug)]
pub struct MinimapImage {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// What the melee panel shows. The background comes from the skin table's
/// race section; the minimap from the map when it ships one.
#[derive(Clone, Debug)]
pub struct MeleeScreen {
    pub background: String,
    pub map_name: String,
    pub game_type: MeleeGameType,
    pub slots: Vec<MeleeSlotView>,
    pub minimap: Option<MinimapImage>,
}

/// The screen's content: either the campaign panel or the melee panel.
#[derive(Clone, Debug)]
pub enum LoadingKind {
    Campaign { screen: Screen, text: ScreenText },
    Melee(MeleeScreen),
}

/// What the screen is built from. The resolver is the one the table was read
/// through, so the frames, models and textures come from the same sources.
/// A map-supplied background or minimap travels as extracted bytes, since map
/// members are not resolver-addressable.
pub struct LoadingRequest {
    pub kind: LoadingKind,
    pub custom: Option<CustomModel>,
    pub resolver: Arc<Resolver>,
    pub rules: ProgressRules,
    pub min_display: MinDisplay,
    /// Minimum display time of the press-a-key state. The original enforces
    /// it only for a network game; a local screen ends on the first press.
    pub min_hold: Duration,
    /// Whether the network branch applies. Every session this build admits is
    /// a local single-player one, so this is false; the branch exists because
    /// the original selects it from the session provider tag.
    pub networked: bool,
}

/// Global-string key of a melee slot's race value. The keys are the shipped
/// `HUMAN`, `ORC`, `UNDEAD`, `NIGHT_ELF` and `RANDOM` values; which key the
/// original reads for a slot is not decoded, so an unknown race word takes
/// the random value as a recreation choice.
pub fn melee_race_key(race: u32) -> &'static str {
    match race {
        1 => "HUMAN",
        2 => "ORC",
        3 => "UNDEAD",
        4 => "NIGHT_ELF",
        _ => "RANDOM",
    }
}

fn melee_background(resolver: &Resolver, race: Option<u32>) -> Result<String, String> {
    let asset = resolver
        .resolve_bounded(SKIN_PATH, SKIN_BYTES)
        .map_err(|e| format!("{SKIN_PATH}: {e}"))?;
    let skin = std::str::from_utf8(&asset.bytes)
        .map_err(|_| format!("{SKIN_PATH}: not UTF-8 text"))?;
    // One entry per race section plus a `[Default]` fallback is the shipped
    // shape, so a section-aware lookup is used instead of a unique-file rule.
    // This mirrors the `menu_skin` grammar (`[Section]` headers, `key=value`
    // rows, `//` comments) for this key only.
    let section = match race {
        Some(1) => "Human",
        Some(2) => "Orc",
        Some(3) => "Undead",
        Some(4) => "NightElf",
        _ => "Default",
    };
    let pick = |section: &str| {
        let mut current = "";
        for raw in skin.lines() {
            let line = raw.split("//").next().unwrap_or("").trim();
            if let Some(name) = line.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                current = name.trim();
                continue;
            }
            if !current.eq_ignore_ascii_case(section) {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == MELEE_BACKGROUND_KEY && !value.trim().is_empty() {
                    return Some(value.trim().to_owned());
                }
            }
        }
        None
    };
    pick(section)
        .or_else(|| pick("Default"))
        .ok_or_else(|| format!("{SKIN_PATH}: no {MELEE_BACKGROUND_KEY} for section {section}"))
}

impl LoadingRequest {
    /// Build the screen a map header selects. `map` is the parsed map and its
    /// label, for members that live inside the map archive: the custom loading
    /// model and the melee minimap. Without it a custom model refuses loudly
    /// and the mission loads without a screen, as before; a melee panel is
    /// built without its minimap.
    #[allow(clippy::too_many_arguments)]
    pub fn read(
        resolver: Arc<Resolver>,
        map: Option<(&pcx_map::Map<'_>, &str)>,
        info: &MapInfo,
        strings: Option<&StringTable>,
        rules: ProgressRules,
        min_display: &MinDisplay,
        networked: bool,
    ) -> Result<Self, String> {
        let asset = resolver
            .resolve_bounded(TABLE_PATH, TABLE_BYTES)
            .map_err(|e| format!("{TABLE_PATH}: {e}"))?;
        let table = std::str::from_utf8(&asset.bytes)
            .map_err(|_| format!("{TABLE_PATH}: not UTF-8 text"))
            .and_then(|text| ScreenTable::parse(text).map_err(|e| e.to_string()))?;
        let selection = select(info, &table, strings).map_err(|e| e.to_string())?;
        match selection {
            Selection::Campaign(screen) => {
                let text = resolve_text(&info.loading_screen(), strings)
                    .map_err(|e| e.to_string())?;
                let min_hold = min_display.hold(&text);
                let custom = match &screen.origin {
                    pcx_play::campaign_loading::Origin::Custom => {
                        let (map, label) = map.ok_or_else(|| {
                            format!(
                                "{}: map-supplied loading model {} is not mounted; \
                                 the mission loads without its screen",
                                label_of(info),
                                screen.model
                            )
                        })?;
                        let bytes = map
                            .extract(&screen.model)
                            .map_err(|e| format!("{label}: {}: {e}", screen.model))?;
                        if bytes.len() > MAP_MEMBER_BYTES {
                            return Err(format!(
                                "{label}: {}: {} bytes exceed the map-member ceiling",
                                screen.model,
                                bytes.len()
                            ));
                        }
                        Some(CustomModel {
                            path: screen.model.clone(),
                            bytes,
                        })
                    }
                    pcx_play::campaign_loading::Origin::Table { .. } => None,
                };
                Ok(Self {
                    kind: LoadingKind::Campaign { screen, text },
                    custom,
                    resolver,
                    rules,
                    min_display: *min_display,
                    min_hold,
                    networked,
                })
            }
            Selection::Melee(panel) => {
                let MeleePanel {
                    map_name,
                    game_type,
                    slots,
                } = panel;
                // The local race selects the skin section; with no human slot
                // the default background applies.
                let race = info
                    .human_players()
                    .next()
                    .map(|player| player.race_id());
                let background = melee_background(&resolver, race)?;
                let text = ScreenText {
                    text: String::new(),
                    title: map_name.clone(),
                    subtitle: String::new(),
                };
                let min_hold = min_display.hold(&text);
                let minimap = match map {
                    Some((map, _)) => map
                        .content_files()
                        .minimap_image
                        .and_then(|name| {
                            map.extract(name)
                                .ok()
                                .filter(|bytes| bytes.len() <= MAP_MEMBER_BYTES)
                                .map(|bytes| MinimapImage {
                                    name: name.to_owned(),
                                    bytes,
                                })
                        }),
                    None => None,
                };
                Ok(Self {
                    kind: LoadingKind::Melee(MeleeScreen {
                        background,
                        map_name,
                        game_type,
                        slots: slots
                            .into_iter()
                            .map(|slot| MeleeSlotView {
                                name: slot.name,
                                race: slot.race,
                                controller: slot.controller,
                            })
                            .collect(),
                        minimap,
                    }),
                    custom: None,
                    resolver,
                    rules,
                    min_display: *min_display,
                    min_hold,
                    networked,
                })
            }
        }
    }
}

fn label_of(info: &MapInfo) -> &str {
    if info.name.is_empty() {
        "map"
    } else {
        &info.name
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    /// The bar fills while the map loads.
    Loading,
    /// Loaded: the bar is full and a key or pointer press starts the game.
    /// In a network game the press is honored only after the minimum display
    /// time elapses, and it moves to the waiting state instead of starting.
    AwaitKey,
    /// A network press was made: the bar text waits for the other players.
    /// This build admits local sessions only, so nothing resolves this state;
    /// it exists because the original draws it.
    WaitingForPeers,
}

/// What the running application shows about the load in progress.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Signals {
    /// The launched map's bindings are published.
    pub admitted: bool,
    /// Its host inputs are prepared.
    pub prepared: bool,
    /// Its script session is running.
    pub published: bool,
    /// Its terrain is published, so the view can be drawn.
    pub ready: bool,
    /// A stage refused; the error is reported by that stage.
    pub failed: bool,
}

#[derive(Resource, Default)]
pub struct CampaignLoading {
    phase: Phase,
    request: Option<Arc<LoadingRequest>>,
    progress: Progress,
    reached: usize,
    /// The bindings revision and session at the launch, which a new load must replace.
    revision: Option<u64>,
    /// Whether the bindings were already admitted for this revision at the
    /// launch. A chapter launch begins its screen before admission, so the
    /// first stage waits for new bindings; a continuation or browser launch
    /// begins after, so its bindings already count.
    begun_admitted: bool,
    session: Option<Weak<()>>,
    /// Failure already on display at the launch.
    outcome: Option<(String, Option<u64>)>,
    generation: u64,
    /// Whether this screen takes the network branch.
    networked: bool,
    /// Earliest moment a network press is honored. `None` for a local screen,
    /// which ends on the first press.
    deadline: Option<Instant>,
    /// Newest bindings revision that refused a screen, so the refusal is
    /// reported once instead of every frame.
    unbuildable: Option<u64>,
}

/// The last successfully read bar and hold rules, retained so a launch whose
/// selected image is unavailable (a browser session that never committed one)
/// can still pose its bar from previously admitted values. The provenance is
/// logged wherever it is used; these are never defaults.
#[derive(Resource, Default)]
pub struct LastRules {
    pub rules: Option<(ProgressRules, MinDisplay)>,
}

impl CampaignLoading {
    /// Start the screen for a launched mission. A screen already shown is replaced.
    /// `pre_admitted` is whether `requests` already admits this launch's
    /// bindings: false for a chapter launch, true for a continuation or
    /// browser launch that builds its screen after admission.
    pub fn begin(
        &mut self,
        request: LoadingRequest,
        requests: &MapRequests,
        session: Option<Weak<()>>,
        pre_admitted: bool,
    ) {
        let networked = request.networked;
        let deadline = networked.then(|| Instant::now() + request.min_hold);
        *self = Self {
            phase: Phase::Loading,
            request: Some(Arc::new(request)),
            revision: requests.bindings.as_ref().map(|b| b.revision),
            session,
            outcome: requests
                .last_outcome
                .as_ref()
                .filter(|o| o.result.is_err())
                .map(|o| (o.label.clone(), o.binding_revision)),
            generation: self.generation + 1,
            networked,
            deadline,
            begun_admitted: pre_admitted,
            ..Self::default()
        };
    }

    /// Drop the screen without starting the game.
    pub fn cancel(&mut self) {
        self.phase = Phase::Idle;
        self.request = None;
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The bindings revision of the current or last screen. A fresh binding
    /// carries its own screen through the chapter launch; any other fresh
    /// binding still needs one.
    pub fn revision(&self) -> Option<u64> {
        self.revision
    }

    /// The newest bindings revision that refused a screen, reported once.
    pub fn unbuildable(&self) -> Option<u64> {
        self.unbuildable
    }

    /// Record that this bindings revision refused a screen, so the refusal is
    /// reported once instead of every frame.
    pub fn mark_unbuildable(&mut self, revision: u64) {
        self.unbuildable = Some(revision);
    }

    /// The screen is up: the menu and the held game sit behind it.
    pub fn shown(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// The simulation waits until the screen ends, as the original's load hold does.
    pub fn holds_simulation(&self) -> bool {
        self.shown()
    }

    pub fn request(&self) -> Option<&Arc<LoadingRequest>> {
        self.request.as_ref()
    }

    /// Changes whenever a new screen begins, so a view can tell screens apart.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Bar position in `0.0..=1.0`.
    pub fn fraction(&self) -> f32 {
        self.request
            .as_ref()
            .map_or(0.0, |r| self.progress.fraction(&r.rules))
    }

    /// Remaining minimum hold, if the network branch still enforces one.
    pub fn hold_remaining(&self) -> Option<Duration> {
        let at = self.deadline?;
        let now = Instant::now();
        (at > now).then(|| at.duration_since(now))
    }

    /// One frame: report newly reached stages, ease the bar, and end the load
    /// when the view is ready.
    pub fn observe(&mut self, signals: Signals) {
        let Some(request) = self.request.clone() else {
            return;
        };
        if self.phase != Phase::Loading {
            return;
        }
        if signals.failed {
            self.cancel();
            return;
        }
        let reached = [
            signals.admitted,
            signals.prepared,
            signals.published,
            signals.ready,
        ];
        while self.reached < STAGES && reached[self.reached] {
            self.progress.report(1.0 / STAGES as f32);
            self.reached += 1;
        }
        if signals.ready {
            self.progress.finish(&request.rules);
            self.phase = Phase::AwaitKey;
        } else {
            self.progress.step(&request.rules);
        }
    }

    /// A key or pointer press while the loaded screen waits ends it. A local
    /// screen ends at once; a network screen waits for its minimum display
    /// time and then for the other players.
    pub fn press(&mut self) -> bool {
        if self.phase != Phase::AwaitKey {
            return false;
        }
        if self.networked && self.hold_remaining().is_some() {
            return false;
        }
        if self.networked {
            self.phase = Phase::WaitingForPeers;
            return false;
        }
        self.cancel();
        true
    }

    fn fresh_outcome_failed(&self, requests: &MapRequests) -> bool {
        requests.last_outcome.as_ref().is_some_and(|outcome| {
            outcome.result.is_err()
                && self.outcome.as_ref() != Some(&(outcome.label.clone(), outcome.binding_revision))
        })
    }
}

/// The campaign view's part of the load. The original ends the load when its game
/// start sequence finishes, whether or not the map set a camera, so readiness is
/// the published terrain, not an active camera.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ViewState {
    ready: bool,
    /// The terrain for this load was refused.
    failed: bool,
}

/// A refused ground replacement keeps the previous ground mounted, so an error
/// beside a ground that is not the running session's means this load's terrain was
/// refused.
fn view_state<K: ?Sized>(
    camera_ready: bool,
    ground_error: bool,
    mounted: Option<&Arc<K>>,
    current: Option<&Arc<K>>,
) -> ViewState {
    let matches = mounted
        .zip(current)
        .is_some_and(|(mounted, current)| Arc::ptr_eq(mounted, current));
    ViewState {
        ready: camera_ready,
        failed: ground_error && !matches,
    }
}

/// The load's observable state, read from the published resources.
fn signals(
    loading: &CampaignLoading,
    requests: &MapRequests,
    supply: &CampaignHostSupply,
    startup: &CampaignStartup,
    witness: Option<Weak<()>>,
    view: ViewState,
) -> Signals {
    let revision = requests.bindings.as_ref().map(|b| b.revision);
    let admitted = revision.is_some_and(|r| Some(r) != loading.revision || loading.begun_admitted)
        && requests
            .last_outcome
            .as_ref()
            .is_some_and(|o| o.result.is_ok() && o.binding_revision == revision);
    let prepared = admitted && revision == Some(supply.revision);
    let published = prepared
        && witness.as_ref().is_some_and(|w| {
            loading
                .session
                .as_ref()
                .is_none_or(|old| !Weak::ptr_eq(old, w))
        });
    let startup_failed = admitted
        && match &startup.last_error {
            Some(StartupError::Preparation { binding, .. }) => Some(*binding) == revision,
            Some(_) => prepared,
            None => false,
        };
    Signals {
        admitted,
        prepared,
        published,
        ready: published && view.ready,
        failed: loading.fresh_outcome_failed(requests)
            || startup_failed
            || (published && view.failed),
    }
}

/// What the load is observed through.
#[derive(SystemParam)]
struct LoadState<'w> {
    requests: Res<'w, MapRequests>,
    supply: Res<'w, CampaignHostSupply>,
    startup: Res<'w, CampaignStartup>,
    session: Res<'w, MapScriptSession<MapStartupHost>>,
    camera: Option<Res<'w, CampaignCamera>>,
    ground: Option<Res<'w, GroundPublication>>,
}

#[derive(SystemParam)]
struct Presses<'w> {
    keys: Option<ResMut<'w, ButtonInput<KeyCode>>>,
    mouse: Option<ResMut<'w, ButtonInput<MouseButton>>>,
}

fn advance_loading(mut loading: ResMut<CampaignLoading>, state: LoadState, presses: Presses) {
    let LoadState {
        requests,
        supply,
        startup,
        session,
        camera,
        ground,
    } = state;
    let Presses { keys, mouse } = presses;
    if !loading.shown() {
        return;
    }
    let view = view_state(
        camera.is_some_and(|c| c.view_ready()),
        ground.as_ref().is_some_and(|g| g.last_error.is_some()),
        ground.as_ref().and_then(|g| g.context.as_ref()),
        session.context(),
    );
    let observed = signals(
        &loading,
        &requests,
        &supply,
        &startup,
        session.session_witness(),
        view,
    );
    // Only a press made while the loaded screen was already waiting counts.
    let waiting = loading.phase() == Phase::AwaitKey;
    loading.observe(observed);
    let mut pressed = false;
    if let Some(mut keys) = keys
        && keys.get_just_pressed().next().is_some()
    {
        pressed |= waiting;
        keys.clear();
    }
    if let Some(mut mouse) = mouse
        && mouse.get_just_pressed().next().is_some()
    {
        pressed |= waiting;
        mouse.clear();
    }
    // Presses are consumed while the screen is up so they do not reach the menu
    // or the game behind it.
    if pressed {
        loading.press();
    }
}

/// The loading state is current for the frame once this set has run.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampaignLoadingSet;

/// Retain the shown screen's rules for a later launch whose selected image is
/// unavailable. The values stay asset-derived; only their provenance changes.
fn retain_rules(loading: Res<CampaignLoading>, mut last: ResMut<LastRules>) {
    if let Some(request) = loading.request() {
        let rules = (request.rules, request.min_display);
        if last.rules != Some(rules) {
            last.rules = Some(rules);
        }
    }
}

pub fn install(app: &mut App) {
    app.init_resource::<CampaignLoading>()
        .init_resource::<LastRules>()
        .add_systems(
            Update,
            (retain_rules, advance_loading)
                .chain()
                .in_set(CampaignLoadingSet)
                .after(MapRequestSet::Deliver)
                .after(CampaignStartupSet)
                .after(CampaignCameraSet),
        );
}

#[cfg(test)]
#[path = "campaign_loading_tests.rs"]
mod tests;
