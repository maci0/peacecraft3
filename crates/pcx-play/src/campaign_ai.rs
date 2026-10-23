//! Per-player campaign script continuations over the admitted map world.
use crate::map_startup_host::NeutralStack;
use crate::scripts::MapScripts;
use pcx_assets::AssetPath;
use pcx_script::{Host, OwnedVm, ScriptCall, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const MAX_PLAYERS: usize = 32;
const MAX_SLEEPERS: usize = 128;

#[derive(Clone)]
pub struct SourceNames {
    pub(crate) common: String,
    pub(crate) ai_common: String,
    pub(crate) fallback_prefix: String,
}

impl SourceNames {
    pub(crate) fn selected(image: &[u8]) -> Result<Self, String> {
        if crate::neutral_player::sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("StartCampaignAI: selected client image differs".into());
        }
        let pe = crate::neutral_rule_input::Pe::parse(
            image,
            crate::neutral_player::RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 0,
                sections: 96,
            },
        )
        .map_err(|e| format!("StartCampaignAI image: {e}"))?;
        let push_target = |at: u32| -> Result<u32, String> {
            let bytes = pe.read_from(at, true).map_err(|e| e.to_string())?;
            let [0x68, a, b, c, d, ..] = bytes else {
                return Err(format!("StartCampaignAI image push at {at:#x}"));
            };
            Ok(u32::from_le_bytes([*a, *b, *c, *d]))
        };
        let call_target = |at: u32| -> Result<u32, String> {
            let bytes = pe.read_from(at, true).map_err(|e| e.to_string())?;
            let [0xe8, a, b, c, d, ..] = bytes else {
                return Err(format!("StartCampaignAI image call at {at:#x}"));
            };
            let relative = i32::from_le_bytes([*a, *b, *c, *d]);
            Ok(at.wrapping_add(5).wrapping_add_signed(relative))
        };
        if [0xac3505, 0xac351d, 0xac352c]
            .into_iter()
            .map(call_target)
            .collect::<Result<Vec<_>, _>>()?
            != [0xac02d0; 3]
            || call_target(0xac02f8)? != 0x4593b0
            || call_target(0xac035a)? != 0x4593b0
        {
            return Err("StartCampaignAI image loader call chain differs".into());
        }
        let names = Self::from_addresses(
            |va| {
                let bytes = pe.read_from(va, false).map_err(|e| e.to_string())?;
                let end = bytes
                    .iter()
                    .take(256)
                    .position(|&byte| byte == 0)
                    .ok_or("StartCampaignAI image string exceeds 255 bytes")?;
                std::str::from_utf8(&bytes[..end])
                    .map(str::to_owned)
                    .map_err(|e| e.to_string())
            },
            [
                push_target(0xac3500)?,
                push_target(0xac350f)?,
                push_target(0xac0326)?,
            ],
        )?;
        Ok(names)
    }

    fn from_addresses(
        mut read: impl FnMut(u32) -> Result<String, String>,
        [common, ai_common, prefix]: [u32; 3],
    ) -> Result<Self, String> {
        let names = Self {
            common: read(common)?,
            ai_common: read(ai_common)?,
            fallback_prefix: read(prefix)?,
        };
        for name in [&names.common, &names.ai_common] {
            AssetPath::new(name).map_err(|e| format!("StartCampaignAI image source: {e}"))?;
        }
        AssetPath::new(&format!("{}probe.ai", names.fallback_prefix))
            .map_err(|e| format!("StartCampaignAI image fallback: {e}"))?;
        Ok(names)
    }

    pub(crate) fn resolve(
        &self,
        reader: &pcx_assets::Resolver,
        name: &str,
    ) -> Result<pcx_assets::ResolvedAsset, String> {
        AssetPath::new(name).map_err(|e| e.to_string())?;
        match reader.resolve_bounded(name, 1 << 20) {
            Ok(asset) => Ok(asset),
            Err(pcx_assets::Error::MissingAsset { .. }) => reader
                .resolve_bounded(&format!("{}{name}", self.fallback_prefix), 1 << 20)
                .map_err(|e| format!("StartCampaignAI {name}: {e}")),
            Err(error) => Err(format!("StartCampaignAI {name}: {error}")),
        }
    }
}

type Loader = Box<dyn FnMut(&str) -> Result<Arc<MapScripts>, String> + Send + Sync>;

pub(crate) trait AiWorld: Host {
    fn ai_clock(&self) -> Option<f64>;
    fn has_map_player_state_callbacks(&self) -> bool;
    fn ai_upgrade_level(&mut self, player: i32, upgrade: i32) -> Result<i32, String>;
    fn ai_unit_cost(&mut self, player: i32, unit: i32, column: &str) -> Result<i32, String>;
    fn ai_unit_count(&mut self, player: i32, unit: i32, done: bool) -> Result<i32, String>;
    fn ai_player_unit_type_count(
        &mut self,
        player: Option<pcx_script::HandleId>,
        unit: i32,
    ) -> Result<i32, String>;
    /// Registers the player as active before its AI main runs. Returns whether
    /// this call created the context so a failed start can roll it back.
    fn ai_activate_player(&mut self, player: i32) -> Result<bool, String>;
    fn ai_deactivate_player(&mut self, player: i32) -> Result<(), String>;
    fn ai_set_target_heroes(&mut self, player: i32, enabled: bool) -> Result<(), String>;
    fn ai_set_units_flee(&mut self, player: i32, enabled: bool) -> Result<(), String>;
    fn ai_set_heroes_flee(&mut self, player: i32, enabled: bool) -> Result<(), String>;
    fn ai_set_groups_flee(&mut self, player: i32, enabled: bool) -> Result<(), String>;
    fn ai_set_new_heroes(&mut self, player: i32, enabled: bool) -> Result<(), String>;
    fn ai_set_campaign_mode(&mut self, player: i32) -> Result<(), String>;
    fn ai_set_replacement_count(&mut self, player: i32, count: i32) -> Result<(), String>;
    fn ai_set_slow_chopping(&mut self, player: i32, enabled: bool) -> Result<(), String>;
    fn ai_stop_gathering(&mut self, player: i32) -> Result<(), String>;
    fn ai_create_captains(&mut self, player: i32) -> Result<(), String>;
    fn ai_set_captain_home(
        &mut self,
        player: i32,
        selector: i32,
        home: [f32; 2],
    ) -> Result<(), String>;
    fn ai_set_group_timed_life(&mut self, player: i32, enabled: bool) -> Result<(), String>;
    fn ai_captain_group_size(&mut self, player: i32) -> Result<i32, String>;
    fn ai_captain_is_empty(&mut self, player: i32) -> Result<bool, String>;
}

impl AiWorld for NeutralStack {
    fn ai_clock(&self) -> Option<f64> {
        self.inner()
            .inner()
            .inner()
            .inner()
            .inner()
            .inner()
            .timer_game_time()
    }

    fn has_map_player_state_callbacks(&self) -> bool {
        self.inner()
            .inner()
            .inner()
            .inner()
            .inner()
            .inner()
            .retained_player_state_events()
            .is_ok_and(|count| count > 0)
    }

    fn ai_upgrade_level(&mut self, player: i32, upgrade: i32) -> Result<i32, String> {
        let identity = self
            .inner()
            .inner()
            .inner()
            .inner()
            .inner()
            .inner()
            .player_identity(player)
            .ok_or("GetUpgradeLevel: AI player identity is not admitted")?;
        match self.native(
            "GetPlayerTechCount",
            &[
                Value::Handle(identity),
                Value::Int(upgrade),
                Value::Bool(true),
            ],
        )? {
            Value::Int(level) => Ok(level),
            _ => Err("GetUpgradeLevel: player research returned a non-integer".into()),
        }
    }

    fn ai_unit_cost(&mut self, player: i32, unit: i32, column: &str) -> Result<i32, String> {
        let objects = self.inner().inner().inner().inner().inner().inner();
        let identity = objects
            .player_identity(player)
            .ok_or("campaign AI cost player identity is not admitted")?;
        objects
            .ai_unit_cost(identity, unit, column)
            .map_err(|e| e.to_string())
    }

    fn ai_activate_player(&mut self, player: i32) -> Result<bool, String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .activate_campaign_ai_player(player)
            .map_err(|e| e.to_string())
    }

    fn ai_unit_count(&mut self, player: i32, unit: i32, done: bool) -> Result<i32, String> {
        self.inner()
            .inner()
            .inner()
            .inner()
            .inner()
            .inner()
            .ai_unit_count(player, unit, done)
            .map_err(|e| e.to_string())
    }

    fn ai_player_unit_type_count(
        &mut self,
        player: Option<pcx_script::HandleId>,
        unit: i32,
    ) -> Result<i32, String> {
        self.inner()
            .inner()
            .inner()
            .inner()
            .inner()
            .inner()
            .ai_player_unit_type_count(player, unit)
            .map_err(|e| e.to_string())
    }

    fn ai_deactivate_player(&mut self, player: i32) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .deactivate_campaign_ai_player(player)
            .map_err(|e| e.to_string())
    }

    fn ai_set_target_heroes(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .set_ai_target_heroes(player, enabled)
            .map_err(|e| e.to_string())
    }

    fn ai_set_units_flee(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .set_ai_units_flee(player, enabled)
            .map_err(|e| e.to_string())
    }

    fn ai_set_heroes_flee(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .set_ai_heroes_flee(player, enabled)
            .map_err(|e| e.to_string())
    }

    fn ai_set_groups_flee(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .set_ai_groups_flee(player, enabled)
            .map_err(|e| e.to_string())
    }

    fn ai_set_new_heroes(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .set_ai_new_heroes(player, enabled)
            .map_err(|e| e.to_string())
    }

    fn ai_set_campaign_mode(&mut self, player: i32) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .set_ai_campaign_mode(player)
            .map_err(|e| e.to_string())
    }

    fn ai_set_replacement_count(&mut self, player: i32, count: i32) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .set_ai_replacement_count(player, count)
            .map_err(|e| e.to_string())
    }

    fn ai_set_slow_chopping(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .set_ai_slow_chopping(player, enabled)
            .map_err(|e| e.to_string())
    }

    fn ai_stop_gathering(&mut self, player: i32) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .stop_ai_gathering(player)
            .map_err(|e| e.to_string())
    }
    fn ai_create_captains(&mut self, player: i32) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .create_ai_captains(player)
            .map_err(|e| e.to_string())
    }

    fn ai_set_captain_home(
        &mut self,
        player: i32,
        selector: i32,
        home: [f32; 2],
    ) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .set_ai_captain_home(player, selector, home)
            .map_err(|e| e.to_string())
    }

    fn ai_set_group_timed_life(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .set_ai_group_timed_life(player, enabled)
            .map_err(|e| e.to_string())
    }

    fn ai_captain_group_size(&mut self, player: i32) -> Result<i32, String> {
        let objects = self.inner().inner().inner().inner().inner().inner();
        let owner = objects
            .player_identity(player)
            .ok_or("CaptainGroupSize: player is not admitted")?;
        objects
            .campaign_captain_group_size(owner)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "CaptainGroupSize: captain policy is not admitted".into())
    }

    fn ai_captain_is_empty(&mut self, player: i32) -> Result<bool, String> {
        let objects = self.inner().inner().inner().inner().inner().inner();
        let owner = objects
            .player_identity(player)
            .ok_or("CaptainIsEmpty: player is not admitted")?;
        objects
            .campaign_captain_is_empty(owner)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "CaptainIsEmpty: captain policy is not admitted".into())
    }
}

pub(crate) struct Runtime<W: AiWorld> {
    loader: Option<Loader>,
    active: BTreeMap<i32, OwnedVm<AiHost<W>>>,
    pending_retirement: BTreeSet<i32>,
    remaining_loads: usize,
}

impl<W: AiWorld> Default for Runtime<W> {
    fn default() -> Self {
        Self {
            loader: None,
            active: BTreeMap::new(),
            pending_retirement: BTreeSet::new(),
            remaining_loads: 64,
        }
    }
}

impl<W: AiWorld> Runtime<W> {
    pub(crate) fn set_loader(
        &mut self,
        loader: impl FnMut(&str) -> Result<Arc<MapScripts>, String> + Send + Sync + 'static,
    ) {
        self.loader = Some(Box::new(loader));
    }

    pub(crate) fn start(
        &mut self,
        world: &mut Option<W>,
        player: i32,
        path: &str,
    ) -> Result<(), String> {
        if path.len() > 16 << 10 {
            return Err("StartCampaignAI: script path exceeds 16 KiB".into());
        }
        let path = AssetPath::new(path).map_err(|e| format!("StartCampaignAI: {e}"))?;
        if self.pending_retirement.contains(&player) {
            world
                .as_mut()
                .ok_or("StartCampaignAI: world is already lent")?
                .ai_deactivate_player(player)
                .map_err(|e| format!("StartCampaignAI player {player}: retirement: {e}"))?;
            self.pending_retirement.remove(&player);
            self.active.remove(&player);
            return Err(format!(
                "StartCampaignAI player {player}: failed context retired; retry start"
            ));
        }
        if self.active.contains_key(&player) {
            return Ok(());
        }
        if self.active.len() >= MAX_PLAYERS {
            return Err("StartCampaignAI: player budget".into());
        }
        let loader = self
            .loader
            .as_mut()
            .ok_or("StartCampaignAI: pinned AI source reader is missing")?;
        self.remaining_loads = self
            .remaining_loads
            .checked_sub(1)
            .ok_or("StartCampaignAI: source load attempt budget")?;
        let scripts = loader(path.as_str())?;
        admit_ai_natives(&scripts)?;
        let program = scripts.program();
        let entry = program
            .function("main")
            .ok_or("StartCampaignAI: AI main is missing")?;
        if entry.source != "the map script"
            || !entry.params.is_empty()
            || entry.ret.kind.name() != "nothing"
        {
            return Err("StartCampaignAI: AI main has wrong source or signature".into());
        }
        let mut inner = world
            .take()
            .ok_or("StartCampaignAI: world is already lent")?;
        let activated = match inner.ai_activate_player(player) {
            Ok(activated) => activated,
            Err(error) => {
                *world = Some(inner);
                return Err(format!("StartCampaignAI player {player}: {error}"));
            }
        };
        let mut vm = OwnedVm::new_owned(scripts.shared_program(), AiHost::new(player, inner));
        let result = vm.init_globals().and_then(|()| {
            let main = vm.program().function_id("main").expect("admitted AI main");
            vm.run_thread(&[main])
        });
        let mut borrowed = vm.host().world.take().expect("AI world is lent");
        let retirement = if result.is_err() && activated {
            borrowed.ai_deactivate_player(player)
        } else {
            Ok(())
        };
        *world = Some(borrowed);
        if let Err(error) = retirement {
            self.active.insert(player, vm);
            self.pending_retirement.insert(player);
            return Err(format!(
                "StartCampaignAI player {player}: retirement: {error}"
            ));
        }
        result.map_err(|e| format!("StartCampaignAI player {player}: {e}"))?;
        self.active.insert(player, vm);
        Ok(())
    }

    pub(crate) fn next_wake(&mut self, until: f64) -> Option<(f64, i32, u64)> {
        self.active
            .iter_mut()
            .filter(|(player, _)| !self.pending_retirement.contains(player))
            .filter_map(|(&player, vm)| {
                vm.host()
                    .sleepers
                    .iter()
                    .filter(|(_, deadline)| **deadline <= until)
                    .map(move |(&ticket, &deadline)| (deadline, player, ticket))
                    .min_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)))
    }

    pub(crate) fn resume(
        &mut self,
        world: &mut Option<W>,
        player: i32,
        ticket: u64,
    ) -> Result<(), String> {
        if self.pending_retirement.contains(&player) {
            return Err("campaign AI context is pending retirement".into());
        }
        if !self.active.contains_key(&player) {
            return Err("campaign AI player is not running".into());
        }
        let borrowed = world.take().ok_or("campaign AI world is already lent")?;
        let mut vm = self.active.remove(&player).expect("checked AI player");
        vm.host().world = Some(borrowed);
        let result = vm.resume_thread(ticket);
        *world = vm.host().world.take();
        if result.is_ok() {
            self.active.insert(player, vm);
        } else if let Some(world) = world.as_mut()
            && let Err(error) = world.ai_deactivate_player(player)
        {
            self.active.insert(player, vm);
            self.pending_retirement.insert(player);
            return Err(format!("campaign AI player {player}: retirement: {error}"));
        }
        result.map_err(|e| format!("campaign AI player {player}: {e}"))
    }

    #[cfg(test)]
    pub(crate) fn global(&self, player: i32, name: &str) -> Option<Value> {
        self.active.get(&player).map(|vm| vm.global(name))
    }
}

fn admit_ai_natives(scripts: &MapScripts) -> Result<(), String> {
    let source = &scripts
        .digests
        .get(1)
        .ok_or("StartCampaignAI: AI common source is missing")?
        .label;
    for (name, params, result) in [
        ("GetAiPlayer", &[][..], "integer"),
        ("Sleep", &["real"][..], "nothing"),
        ("StartThread", &["code"][..], "nothing"),
        ("GetUpgradeLevel", &["integer"][..], "integer"),
        ("GetUnitGoldCost", &["integer"][..], "integer"),
        ("GetUnitWoodCost", &["integer"][..], "integer"),
    ] {
        let native = scripts
            .program()
            .natives()
            .get(name)
            .ok_or_else(|| format!("StartCampaignAI: {name} has no unambiguous declaration"))?;
        if native.source != *source
            || !native.also_declared_in.is_empty()
            || native.is_constant
            || native.params.len() != params.len()
            || native
                .params
                .iter()
                .zip(params)
                .any(|(param, expected)| param.ty.array || param.ty.kind.name() != *expected)
            || native.ret.array
            || native.ret.kind.name() != result
        {
            return Err(format!(
                "StartCampaignAI: {name} AI common declaration differs"
            ));
        }
    }
    for name in [
        "SetTargetHeroes",
        "SetUnitsFlee",
        "SetHeroesFlee",
        "SetGroupsFlee",
        "SetNewHeroes",
        "SetSlowChopping",
        "GroupTimedLife",
    ] {
        let Some(native) = scripts.program().natives().get(name) else {
            continue;
        };
        let expected = scripts
            .digests
            .get(1)
            .ok_or("StartCampaignAI: AI common source is missing")?
            .label
            .as_str();
        if native.source != expected
            || !native.also_declared_in.is_empty()
            || native.is_constant
            || native.params.len() != 1
            || native.params[0].ty.array
            || native.params[0].ty.kind.name() != "boolean"
            || native.ret.array
            || native.ret.kind.name() != "nothing"
        {
            return Err(format!(
                "StartCampaignAI: {name} AI common declaration differs"
            ));
        }
    }
    for (name, params) in [
        ("GetUnitCount", &["integer"][..]),
        ("GetUnitCountDone", &["integer"][..]),
        ("GetPlayerUnitTypeCount", &["player", "integer"][..]),
    ] {
        let Some(native) = scripts.program().natives().get(name) else {
            continue;
        };
        if native.source != *source
            || !native.also_declared_in.is_empty()
            || native.is_constant
            || native.params.len() != params.len()
            || native
                .params
                .iter()
                .zip(params)
                .any(|(p, expected)| p.ty.array || p.ty.kind.name() != *expected)
            || native.ret.array
            || native.ret.kind.name() != "integer"
        {
            return Err(format!(
                "StartCampaignAI: {name} AI common declaration differs"
            ));
        }
    }
    if let Some(native) = scripts.program().natives().get("StopGathering")
        && (native.source != *source
            || !native.also_declared_in.is_empty()
            || native.is_constant
            || !native.params.is_empty()
            || native.ret.array
            || native.ret.kind.name() != "nothing")
    {
        return Err("StartCampaignAI: StopGathering AI common declaration differs".into());
    }
    for (name, params) in [
        ("CreateCaptains", &[][..]),
        ("SetCampaignAI", &[][..]),
        ("SetReplacementCount", &["integer"][..]),
        ("SetCaptainHome", &["integer", "real", "real"][..]),
    ] {
        let Some(native) = scripts.program().natives().get(name) else {
            continue;
        };
        if native.source != *source
            || !native.also_declared_in.is_empty()
            || native.is_constant
            || native.params.len() != params.len()
            || native
                .params
                .iter()
                .zip(params)
                .any(|(param, expected)| param.ty.array || param.ty.kind.name() != *expected)
            || native.ret.array
            || native.ret.kind.name() != "nothing"
        {
            return Err(format!(
                "StartCampaignAI: {name} AI common declaration differs"
            ));
        }
    }
    for (name, result) in [
        ("CaptainGroupSize", "integer"),
        ("CaptainIsEmpty", "boolean"),
    ] {
        let Some(native) = scripts.program().natives().get(name) else {
            continue;
        };
        if native.source != *source
            || !native.also_declared_in.is_empty()
            || native.is_constant
            || !native.params.is_empty()
            || native.ret.array
            || native.ret.kind.name() != result
        {
            return Err(format!(
                "StartCampaignAI: {name} AI common declaration differs"
            ));
        }
    }
    Ok(())
}

pub(crate) struct AiHost<W: AiWorld> {
    player: i32,
    world: Option<W>,
    pending_sleep: Option<f64>,
    sleepers: BTreeMap<u64, f64>,
    next_ticket: u64,
}

impl<W: AiWorld> AiHost<W> {
    fn new(player: i32, world: W) -> Self {
        Self {
            player,
            world: Some(world),
            pending_sleep: None,
            sleepers: BTreeMap::new(),
            next_ticket: 0,
        }
    }

    fn world(&self) -> Result<&W, String> {
        self.world
            .as_ref()
            .ok_or("campaign AI world is not lent".into())
    }

    fn world_mut(&mut self) -> Result<&mut W, String> {
        self.world
            .as_mut()
            .ok_or("campaign AI world is not lent".into())
    }
}

impl<W: AiWorld> Host for AiHost<W> {
    fn real_ops(&self) -> Option<&dyn pcx_script::RealOps> {
        self.world.as_ref().and_then(Host::real_ops)
    }

    fn begin_script_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, String> {
        if name == "StartThread" {
            let [Value::Code(id)] = args else {
                return Err("StartThread: expected AI callback".into());
            };
            return Ok(Some(ScriptCall::Spawn(vec![*id])));
        }
        Ok(None)
    }

    fn suspend(&mut self, name: &str, args: &[Value]) -> bool {
        if name != "Sleep" {
            return false;
        }
        let [seconds] = args else { return false };
        let Some(seconds) = seconds.as_real() else {
            return false;
        };
        let Ok(world) = self.world() else {
            return false;
        };
        let Some(now) = world.ai_clock() else {
            return false;
        };
        let deadline = now + f64::from(seconds);
        if seconds <= 0.0 || !deadline.is_finite() || self.sleepers.len() >= MAX_SLEEPERS {
            return false;
        }
        self.pending_sleep = Some(deadline);
        true
    }

    fn park_thread(&mut self) -> Result<u64, String> {
        let deadline = self
            .pending_sleep
            .take()
            .ok_or("Sleep: no pending deadline")?;
        self.next_ticket = self
            .next_ticket
            .checked_add(1)
            .ok_or("Sleep: ticket budget")?;
        self.sleepers.insert(self.next_ticket, deadline);
        Ok(self.next_ticket)
    }

    fn resume_thread(&mut self, ticket: u64) -> Result<(), String> {
        let deadline = self
            .sleepers
            .remove(&ticket)
            .ok_or("Sleep: unknown ticket")?;
        if self.world()?.ai_clock().is_none_or(|now| now < deadline) {
            return Err("Sleep: game clock has not reached the wake deadline".into());
        }
        Ok(())
    }

    fn leave_thread(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        match name {
            "GetAiPlayer" if args.is_empty() => Ok(Value::Int(self.player)),
            "Player"
            | "GetPlayers"
            | "GetPlayerState"
            | "GetGameDifficulty"
            | "GetPlayerNeutralPassive"
            | "GetPlayerNeutralAggressive" => self.world_mut()?.native(name, args),
            name if name.starts_with("Convert") && matches!(args, [Value::Int(_)]) => {
                self.world_mut()?.native(name, args)
            }
            "SetPlayerState" => {
                if self.world()?.has_map_player_state_callbacks() {
                    return Err(
                        "SetPlayerState: map callbacks from campaign AI are not admitted".into(),
                    );
                }
                self.world_mut()?.native(name, args)
            }
            "GetUpgradeLevel" => {
                let [Value::Int(upgrade)] = args else {
                    return Err("GetUpgradeLevel: expected an integer upgrade ID".into());
                };
                let player = self.player;
                Ok(Value::Int(
                    self.world_mut()?.ai_upgrade_level(player, *upgrade)?,
                ))
            }
            "GetUnitGoldCost" | "GetUnitWoodCost" => {
                let [Value::Int(unit)] = args else {
                    return Err(format!("{name}: expected an integer unit ID"));
                };
                let column = if name == "GetUnitGoldCost" {
                    "goldcost"
                } else {
                    "lumbercost"
                };
                let player = self.player;
                Ok(Value::Int(
                    self.world_mut()?.ai_unit_cost(player, *unit, column)?,
                ))
            }
            "GetUnitCount" | "GetUnitCountDone" => {
                let [Value::Int(unit)] = args else {
                    return Err(format!("{name}: expected one integer unit ID"));
                };
                let player = self.player;
                Ok(Value::Int(self.world_mut()?.ai_unit_count(
                    player,
                    *unit,
                    name == "GetUnitCountDone",
                )?))
            }
            "GetPlayerUnitTypeCount" => {
                let [owner, Value::Int(unit)] = args else {
                    return Err("GetPlayerUnitTypeCount: expected player, integer".into());
                };
                let owner = match owner {
                    Value::Null => None,
                    Value::Handle(owner) => Some(*owner),
                    _ => return Err("GetPlayerUnitTypeCount: expected player, integer".into()),
                };
                Ok(Value::Int(
                    self.world_mut()?.ai_player_unit_type_count(owner, *unit)?,
                ))
            }
            "SetTargetHeroes" => {
                let [Value::Bool(enabled)] = args else {
                    return Err("SetTargetHeroes: expected a boolean".into());
                };
                let player = self.player;
                self.world_mut()?.ai_set_target_heroes(player, *enabled)?;
                Ok(Value::Null)
            }
            "SetUnitsFlee" => {
                let [Value::Bool(enabled)] = args else {
                    return Err("SetUnitsFlee: expected a boolean".into());
                };
                let player = self.player;
                self.world_mut()?.ai_set_units_flee(player, *enabled)?;
                Ok(Value::Null)
            }
            "SetHeroesFlee" => {
                let [Value::Bool(enabled)] = args else {
                    return Err("SetHeroesFlee: expected a boolean".into());
                };
                let player = self.player;
                self.world_mut()?.ai_set_heroes_flee(player, *enabled)?;
                Ok(Value::Null)
            }
            "SetGroupsFlee" => {
                let [Value::Bool(enabled)] = args else {
                    return Err("SetGroupsFlee: expected a boolean".into());
                };
                let player = self.player;
                self.world_mut()?.ai_set_groups_flee(player, *enabled)?;
                Ok(Value::Null)
            }
            "SetNewHeroes" => {
                let [Value::Bool(enabled)] = args else {
                    return Err("SetNewHeroes: expected a boolean".into());
                };
                let player = self.player;
                self.world_mut()?.ai_set_new_heroes(player, *enabled)?;
                Ok(Value::Null)
            }
            "SetSlowChopping" => {
                let [Value::Bool(enabled)] = args else {
                    return Err("SetSlowChopping: expected a boolean".into());
                };
                let player = self.player;
                self.world_mut()?.ai_set_slow_chopping(player, *enabled)?;
                Ok(Value::Null)
            }
            "StopGathering" => {
                if !args.is_empty() {
                    return Err("StopGathering: expected no arguments".into());
                }
                let player = self.player;
                self.world_mut()?.ai_stop_gathering(player)?;
                Ok(Value::Null)
            }
            "CreateCaptains" => {
                if !args.is_empty() {
                    return Err("CreateCaptains: expected no arguments".into());
                }
                let player = self.player;
                self.world_mut()?.ai_create_captains(player)?;
                Ok(Value::Null)
            }
            "SetCaptainHome" => {
                let [Value::Int(selector), Value::Real(x), Value::Real(y)] = args else {
                    return Err("SetCaptainHome: expected integer, real, real".into());
                };
                let player = self.player;
                self.world_mut()?
                    .ai_set_captain_home(player, *selector, [*x, *y])?;
                Ok(Value::Null)
            }
            "SetCampaignAI" => {
                if !args.is_empty() {
                    return Err("SetCampaignAI: expected no arguments".into());
                }
                let player = self.player;
                self.world_mut()?.ai_set_campaign_mode(player)?;
                Ok(Value::Null)
            }
            "SetReplacementCount" => {
                let [Value::Int(count)] = args else {
                    return Err("SetReplacementCount: expected one integer".into());
                };
                let player = self.player;
                self.world_mut()?.ai_set_replacement_count(player, *count)?;
                Ok(Value::Null)
            }
            "GroupTimedLife" => {
                let [Value::Bool(enabled)] = args else {
                    return Err("GroupTimedLife: expected a boolean".into());
                };
                let player = self.player;
                self.world_mut()?
                    .ai_set_group_timed_life(player, *enabled)?;
                Ok(Value::Null)
            }
            "CaptainGroupSize" => {
                if !args.is_empty() {
                    return Err("CaptainGroupSize: expected no arguments".into());
                }
                let player = self.player;
                Ok(Value::Int(self.world_mut()?.ai_captain_group_size(player)?))
            }
            "CaptainIsEmpty" => {
                if !args.is_empty() {
                    return Err("CaptainIsEmpty: expected no arguments".into());
                }
                let player = self.player;
                Ok(Value::Bool(self.world_mut()?.ai_captain_is_empty(player)?))
            }
            "Sleep" => Err("Sleep: invalid duration or thread budget".into()),
            _ => Err(format!(
                "campaign AI native {name} has no source-qualified world behavior"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripts::{AssetIdentity, ScriptAsset};
    use pcx_assets::{
        ArchiveLayer, ContentSource, GameMode, LayerKind, SourceContent, SourceError, SourceInfo,
    };
    use pcx_script::HandleId;
    use std::num::NonZeroU64;
    use std::sync::Mutex;

    #[derive(Default)]
    struct State {
        clock: f64,
        gold: BTreeMap<u64, i32>,
        unit_counts: BTreeMap<(i32, i32, bool), i32>,
        player_unit_counts: BTreeMap<(HandleId, i32), i32>,
        callbacks: bool,
        active: BTreeMap<i32, bool>,
        target_heroes: BTreeMap<i32, bool>,
        units_flee: BTreeMap<i32, bool>,
        heroes_flee: BTreeMap<i32, bool>,
        groups_flee: BTreeMap<i32, bool>,
        new_heroes: BTreeMap<i32, bool>,
        campaign_modes: BTreeSet<i32>,
        replacement_counts: BTreeMap<i32, i32>,
        slow_chopping: BTreeMap<i32, bool>,
        gathering: BTreeMap<i32, bool>,
        captain_creates: BTreeMap<i32, usize>,
        captain_homes: Vec<(i32, i32, [f32; 2])>,
        group_timed_life: BTreeMap<i32, bool>,
        retirement_blocked: bool,
    }

    struct World(Arc<Mutex<State>>);

    impl AiWorld for World {
        fn ai_clock(&self) -> Option<f64> {
            Some(self.0.lock().unwrap().clock)
        }

        fn has_map_player_state_callbacks(&self) -> bool {
            self.0.lock().unwrap().callbacks
        }

        fn ai_upgrade_level(&mut self, _player: i32, _upgrade: i32) -> Result<i32, String> {
            Err("test world has no admitted upgrade research".into())
        }
        fn ai_unit_cost(&mut self, _player: i32, _unit: i32, _column: &str) -> Result<i32, String> {
            Err("mock costs are not admitted".into())
        }
        fn ai_unit_count(&mut self, player: i32, unit: i32, done: bool) -> Result<i32, String> {
            let state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state
                .unit_counts
                .get(&(player, unit, done))
                .copied()
                .ok_or("mock unit count is not admitted".into())
        }
        fn ai_player_unit_type_count(
            &mut self,
            player: Option<HandleId>,
            unit: i32,
        ) -> Result<i32, String> {
            let Some(player) = player else { return Ok(0) };
            self.0
                .lock()
                .unwrap()
                .player_unit_counts
                .get(&(player, unit))
                .copied()
                .ok_or("mock explicit unit count is not admitted".into())
        }
        fn ai_activate_player(&mut self, player: i32) -> Result<bool, String> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .active
                .insert(player, false)
                .is_none())
        }
        fn ai_deactivate_player(&mut self, player: i32) -> Result<(), String> {
            if self.0.lock().unwrap().retirement_blocked {
                return Err("route retirement refused".into());
            }
            self.0.lock().unwrap().active.remove(&player);
            self.0.lock().unwrap().target_heroes.remove(&player);
            self.0.lock().unwrap().units_flee.remove(&player);
            self.0.lock().unwrap().heroes_flee.remove(&player);
            self.0.lock().unwrap().groups_flee.remove(&player);
            self.0.lock().unwrap().new_heroes.remove(&player);
            self.0.lock().unwrap().campaign_modes.remove(&player);
            self.0.lock().unwrap().replacement_counts.remove(&player);
            self.0.lock().unwrap().slow_chopping.remove(&player);
            self.0.lock().unwrap().gathering.remove(&player);
            self.0.lock().unwrap().captain_creates.remove(&player);
            self.0.lock().unwrap().group_timed_life.remove(&player);
            Ok(())
        }
        fn ai_set_target_heroes(&mut self, player: i32, enabled: bool) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.target_heroes.insert(player, enabled);
            Ok(())
        }
        fn ai_set_units_flee(&mut self, player: i32, enabled: bool) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.units_flee.insert(player, enabled);
            Ok(())
        }
        fn ai_set_heroes_flee(&mut self, player: i32, enabled: bool) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.heroes_flee.insert(player, enabled);
            Ok(())
        }
        fn ai_set_groups_flee(&mut self, player: i32, enabled: bool) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.groups_flee.insert(player, enabled);
            Ok(())
        }
        fn ai_set_new_heroes(&mut self, player: i32, enabled: bool) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.new_heroes.insert(player, enabled);
            Ok(())
        }

        fn ai_set_slow_chopping(&mut self, player: i32, enabled: bool) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.slow_chopping.insert(player, enabled);
            Ok(())
        }

        fn ai_set_campaign_mode(&mut self, player: i32) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.campaign_modes.insert(player);
            Ok(())
        }
        fn ai_set_replacement_count(&mut self, player: i32, count: i32) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.replacement_counts.insert(player, count);
            Ok(())
        }
        fn ai_stop_gathering(&mut self, player: i32) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.gathering.insert(player, false);
            Ok(())
        }
        fn ai_create_captains(&mut self, player: i32) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            *state.captain_creates.entry(player).or_default() += 1;
            Ok(())
        }

        fn ai_set_captain_home(
            &mut self,
            player: i32,
            selector: i32,
            home: [f32; 2],
        ) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.captain_homes.push((player, selector, home));
            Ok(())
        }

        fn ai_set_group_timed_life(&mut self, player: i32, enabled: bool) -> Result<(), String> {
            let mut state = self.0.lock().unwrap();
            if !state.active.contains_key(&player) {
                return Err("AI player is not active".into());
            }
            state.group_timed_life.insert(player, enabled);
            Ok(())
        }
        fn ai_captain_group_size(&mut self, _player: i32) -> Result<i32, String> {
            Ok(0)
        }
        fn ai_captain_is_empty(&mut self, _player: i32) -> Result<bool, String> {
            Ok(true)
        }
    }

    impl Host for World {
        fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
            match (name, args) {
                ("GetPlayerNeutralPassive", []) => Ok(Value::Int(15)),
                ("GetPlayerNeutralAggressive", []) => Ok(Value::Int(12)),
                ("GetGameDifficulty", []) => {
                    Ok(Value::Handle(HandleId::new(NonZeroU64::new(101).unwrap())))
                }
                (name, [Value::Int(n)]) if name.starts_with("Convert") => Ok(Value::Handle(
                    HandleId::new(NonZeroU64::new((*n as u32 as u64) + 100).unwrap()),
                )),
                ("Player", [Value::Int(n)]) if *n >= 0 => Ok(Value::Handle(HandleId::new(
                    NonZeroU64::new((*n as u64) + 1).unwrap(),
                ))),
                ("GetPlayerState", [Value::Handle(player), Value::Null]) => Ok(Value::Int(
                    *self
                        .0
                        .lock()
                        .unwrap()
                        .gold
                        .get(&player.token())
                        .unwrap_or(&0),
                )),
                ("SetPlayerState", [Value::Handle(player), Value::Null, Value::Int(value)]) => {
                    self.0.lock().unwrap().gold.insert(player.token(), *value);
                    Ok(Value::Null)
                }
                _ => Err(format!("test world does not admit {name}")),
            }
        }
    }

    const COMMON: &str = "type agent extends handle\ntype player extends agent\ntype playerstate extends handle\nnative Player takes integer number returns player\nnative GetPlayerState takes player whichPlayer, playerstate whichState returns integer\nnative SetPlayerState takes player whichPlayer, playerstate whichState, integer value returns nothing\n";
    const AI_COMMON: &str = "native GetAiPlayer takes nothing returns integer\nnative Sleep takes real seconds returns nothing\nnative StartThread takes code callback returns nothing\nnative GetUpgradeLevel takes integer id returns integer\nnative GetUnitGoldCost takes integer unitid returns integer\nnative GetUnitWoodCost takes integer unitid returns integer\n";

    fn program(label: &str, source: &str) -> Arc<MapScripts> {
        program_with_ai_common(label, source, AI_COMMON)
    }

    fn program_with_ai_common(label: &str, source: &str, ai_common: &str) -> Arc<MapScripts> {
        program_with_sources(label, source, COMMON, ai_common)
    }

    fn program_with_sources(
        label: &str,
        source: &str,
        common: &str,
        ai_common: &str,
    ) -> Arc<MapScripts> {
        Arc::new(
            MapScripts::compile(
                AssetIdentity {
                    edition: "synthetic".into(),
                    version: "ai-world".into(),
                    source: label.into(),
                },
                ScriptAsset {
                    label: "synthetic:common",
                    bytes: common.as_bytes(),
                },
                ScriptAsset {
                    label: "synthetic:common-ai",
                    bytes: ai_common.as_bytes(),
                },
                ScriptAsset {
                    label,
                    bytes: source.as_bytes(),
                },
                None,
            )
            .unwrap(),
        )
    }

    const AI_TARGET: &str = "native SetTargetHeroes takes boolean flag returns nothing\n";
    const AI_FLEE: &str = "native SetUnitsFlee takes boolean flag returns nothing\n";
    const AI_HERO_FLEE: &str = "native SetHeroesFlee takes boolean flag returns nothing\n";
    const AI_GROUP_FLEE: &str = "native SetGroupsFlee takes boolean flag returns nothing\n";
    const AI_NEW_HEROES: &str = "native SetNewHeroes takes boolean flag returns nothing\n";
    const AI_SLOW_CHOPPING: &str = "native SetSlowChopping takes boolean flag returns nothing\n";
    const AI_GATHERING: &str = "native StopGathering takes nothing returns nothing\n";
    const AI_CAPTAINS: &str = "native CreateCaptains takes nothing returns nothing\nnative SetCaptainHome takes integer which, real x, real y returns nothing\n";

    #[test]
    fn hero_flee_flags_follow_each_ai_context_across_sleep() {
        let script = program_with_ai_common(
            "synthetic:hero-flee.ai",
            "function main takes nothing returns nothing\ncall SetHeroesFlee(GetAiPlayer() == 2)\ncall Sleep(0.25)\ncall SetHeroesFlee(GetAiPlayer() != 2)\nendfunction\n",
            &format!("{AI_COMMON}{AI_HERO_FLEE}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        for player in [1, 2] {
            runtime.start(&mut world, player, "hero-flee.ai").unwrap();
        }
        assert_eq!(
            shared.lock().unwrap().heroes_flee,
            [(1, false), (2, true)].into()
        );
        shared.lock().unwrap().clock = 0.25;
        while let Some((_, player, ticket)) = runtime.next_wake(0.25) {
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        assert_eq!(
            shared.lock().unwrap().heroes_flee,
            [(1, true), (2, false)].into()
        );
        assert!(world.is_some());
    }

    #[test]
    fn malformed_hero_flee_abi_refuses_before_any_world_write() {
        let body = "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()), null, 17)\ncall SetHeroesFlee(true)\nendfunction\n";
        for (common, ai_common) in [
            (
                COMMON.into(),
                format!(
                    "{AI_COMMON}{}",
                    AI_HERO_FLEE.replace("boolean flag", "integer flag")
                ),
            ),
            (
                COMMON.into(),
                format!(
                    "{AI_COMMON}{}",
                    AI_HERO_FLEE.replace("returns nothing", "returns integer")
                ),
            ),
            (COMMON.into(), format!("{AI_COMMON}constant {AI_HERO_FLEE}")),
            (format!("{COMMON}{AI_HERO_FLEE}"), AI_COMMON.into()),
        ] {
            let script = program_with_sources("synthetic:hero-flee.ai", body, &common, &ai_common);
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&script)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(Arc::clone(&shared)));
            let error = runtime.start(&mut world, 1, "hero-flee.ai").unwrap_err();
            assert!(error.contains("SetHeroesFlee"), "{error}");
            let state = shared.lock().unwrap();
            assert!(state.active.is_empty());
            assert!(state.heroes_flee.is_empty());
            assert!(state.gold.is_empty());
            assert!(world.is_some());
        }
    }

    #[test]
    fn hero_flee_native_refuses_bad_arguments_and_failed_main_retires_flag() {
        let shared = Arc::new(Mutex::new(State::default()));
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        for args in [
            vec![],
            vec![Value::Int(1)],
            vec![Value::Null],
            vec![Value::Bool(true), Value::Bool(false)],
        ] {
            assert!(host.native("SetHeroesFlee", &args).is_err());
            assert!(shared.lock().unwrap().heroes_flee.is_empty());
        }
        let script = program_with_ai_common(
            "synthetic:hero-flee.ai",
            "function main takes nothing returns nothing\ncall SetHeroesFlee(true)\ncall GetUnitGoldCost(0)\nendfunction\n",
            &format!("{AI_COMMON}{AI_HERO_FLEE}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let mut world = Some(World(Arc::clone(&shared)));
        assert!(runtime.start(&mut world, 1, "hero-flee.ai").is_err());
        assert!(shared.lock().unwrap().heroes_flee.is_empty());
        assert!(world.is_some());
    }

    #[test]
    fn group_flee_state_is_per_player_before_and_after_captain_creation() {
        let script = program_with_ai_common(
            "synthetic:group-flee.ai",
            "function main takes nothing returns nothing\ncall SetGroupsFlee(GetAiPlayer() == 2)\ncall CreateCaptains()\ncall SetCaptainHome(1, 10.0, 20.0)\ncall Sleep(0.25)\ncall SetGroupsFlee(GetAiPlayer() != 2)\nendfunction\n",
            &format!("{AI_COMMON}{AI_GROUP_FLEE}{AI_CAPTAINS}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        for player in [1, 2] {
            runtime.start(&mut world, player, "group-flee.ai").unwrap();
        }
        {
            let state = shared.lock().unwrap();
            assert_eq!(state.groups_flee, [(1, false), (2, true)].into());
            assert_eq!(state.captain_creates, [(1, 1), (2, 1)].into());
            assert_eq!(
                state.captain_homes,
                vec![(1, 1, [10.0, 20.0]), (2, 1, [10.0, 20.0])]
            );
            assert!(state.units_flee.is_empty());
            assert!(state.heroes_flee.is_empty());
        }
        let remaining_loads = runtime.remaining_loads;
        runtime.start(&mut world, 1, "ignored.ai").unwrap();
        assert_eq!(runtime.remaining_loads, remaining_loads);
        shared.lock().unwrap().clock = 0.25;
        while let Some((_, player, ticket)) = runtime.next_wake(0.25) {
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        let state = shared.lock().unwrap();
        assert_eq!(state.groups_flee, [(1, true), (2, false)].into());
        assert_eq!(state.captain_creates, [(1, 1), (2, 1)].into());
        assert_eq!(
            state.captain_homes,
            vec![(1, 1, [10.0, 20.0]), (2, 1, [10.0, 20.0])]
        );
        assert!(world.is_some());
    }

    #[test]
    fn group_flee_abi_and_bad_calls_refuse_before_world_mutation() {
        let body = "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()), null, 17)\ncall SetGroupsFlee(true)\nendfunction\n";
        for (common, ai_common) in [
            (
                COMMON.into(),
                format!(
                    "{AI_COMMON}{}",
                    AI_GROUP_FLEE.replace("boolean flag", "integer flag")
                ),
            ),
            (
                COMMON.into(),
                format!(
                    "{AI_COMMON}{}",
                    AI_GROUP_FLEE.replace("returns nothing", "returns integer")
                ),
            ),
            (
                COMMON.into(),
                format!("{AI_COMMON}constant {AI_GROUP_FLEE}"),
            ),
            (format!("{COMMON}{AI_GROUP_FLEE}"), AI_COMMON.into()),
        ] {
            let script = program_with_sources("synthetic:group-flee.ai", body, &common, &ai_common);
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&script)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(Arc::clone(&shared)));
            let error = runtime.start(&mut world, 1, "group-flee.ai").unwrap_err();
            assert!(error.contains("SetGroupsFlee"), "{error}");
            let state = shared.lock().unwrap();
            assert!(state.active.is_empty());
            assert!(state.groups_flee.is_empty());
            assert!(state.gold.is_empty());
            assert!(world.is_some());
        }

        let shared = Arc::new(Mutex::new(State::default()));
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        for args in [
            vec![],
            vec![Value::Int(1)],
            vec![Value::Null],
            vec![Value::Bool(true), Value::Bool(false)],
        ] {
            assert!(host.native("SetGroupsFlee", &args).is_err());
            assert!(shared.lock().unwrap().groups_flee.is_empty());
        }
    }

    #[test]
    fn failed_ai_start_retires_only_its_group_flee_state() {
        let peer = program_with_ai_common(
            "synthetic:peer.ai",
            "function main takes nothing returns nothing\ncall SetGroupsFlee(false)\ncall Sleep(1.0)\nendfunction\n",
            &format!("{AI_COMMON}{AI_GROUP_FLEE}"),
        );
        let failing = program_with_ai_common(
            "synthetic:fail.ai",
            "function main takes nothing returns nothing\ncall SetGroupsFlee(true)\ncall GetUnitGoldCost(0)\nendfunction\n",
            &format!("{AI_COMMON}{AI_GROUP_FLEE}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |name| {
            Ok(if name == "peer.ai" {
                Arc::clone(&peer)
            } else {
                Arc::clone(&failing)
            })
        });
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        runtime.start(&mut world, 2, "peer.ai").unwrap();
        assert!(runtime.start(&mut world, 1, "fail.ai").is_err());
        let state = shared.lock().unwrap();
        assert_eq!(state.groups_flee, [(2, false)].into());
        assert!(state.active.contains_key(&2));
        assert!(!state.active.contains_key(&1));
        assert!(world.is_some());
    }

    #[test]
    fn refused_retirement_keeps_the_vm_and_world_until_a_retry_succeeds() {
        let failing = program_with_ai_common(
            "synthetic:retire.ai",
            "function main takes nothing returns nothing\ncall SetGroupsFlee(true)\ncall GetUnitGoldCost(0)\nendfunction\n",
            &format!("{AI_COMMON}{AI_GROUP_FLEE}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&failing)));
        let shared = Arc::new(Mutex::new(State {
            retirement_blocked: true,
            ..State::default()
        }));
        let mut world = Some(World(Arc::clone(&shared)));
        let error = runtime.start(&mut world, 1, "retire.ai").unwrap_err();
        assert!(error.contains("retirement"), "{error}");
        assert!(runtime.active.contains_key(&1));
        assert!(runtime.pending_retirement.contains(&1));
        assert!(world.is_some());
        assert_eq!(shared.lock().unwrap().groups_flee.get(&1), Some(&true));
        shared.lock().unwrap().retirement_blocked = false;
        let error = runtime.start(&mut world, 1, "retire.ai").unwrap_err();
        assert!(error.contains("failed context retired"), "{error}");
        assert!(!runtime.active.contains_key(&1));
        assert!(runtime.pending_retirement.is_empty());
        assert!(!shared.lock().unwrap().active.contains_key(&1));
        assert!(world.is_some());
    }

    #[test]
    fn captain_natives_keep_ai_players_and_sleeping_calls_separate() {
        let script = program_with_ai_common(
            "synthetic:captain.ai",
            "function main takes nothing returns nothing\ncall CreateCaptains()\ncall SetCaptainHome(3, 24.0, 32.0)\ncall Sleep(0.25)\ncall SetCaptainHome(1, 48.0, 64.0)\nendfunction\n",
            &format!("{AI_COMMON}{AI_CAPTAINS}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        runtime.start(&mut world, 1, "captain.ai").unwrap();
        runtime.start(&mut world, 2, "captain.ai").unwrap();
        {
            let state = shared.lock().unwrap();
            assert_eq!(state.captain_creates.get(&1), Some(&1));
            assert_eq!(state.captain_creates.get(&2), Some(&1));
            assert_eq!(
                state.captain_homes,
                vec![(1, 3, [24., 32.]), (2, 3, [24., 32.])]
            );
        }
        shared.lock().unwrap().clock = 0.25;
        while let Some((_, player, ticket)) = runtime.next_wake(0.25) {
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        assert_eq!(
            shared.lock().unwrap().captain_homes,
            vec![
                (1, 3, [24., 32.]),
                (2, 3, [24., 32.]),
                (1, 1, [48., 64.]),
                (2, 1, [48., 64.]),
            ]
        );
        assert!(world.is_some());
    }

    #[test]
    fn captain_abi_refuses_before_globals_touch_the_world() {
        let body = "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()), null, 17)\nendfunction\n";
        for bad in [
            "native CreateCaptains takes nothing returns integer\n",
            "native CreateCaptains takes integer n returns nothing\n",
            "native SetCaptainHome takes integer which, real x, real y returns integer\n",
            "native SetCaptainHome takes integer which, integer x, real y returns nothing\n",
        ] {
            let script =
                program_with_ai_common("synthetic:captain.ai", body, &format!("{AI_COMMON}{bad}"));
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&script)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(Arc::clone(&shared)));
            let error = runtime.start(&mut world, 1, "captain.ai").unwrap_err();
            assert!(error.contains("Captain"), "{error}");
            let state = shared.lock().unwrap();
            assert!(state.active.is_empty());
            assert!(state.gold.is_empty());
            assert!(state.captain_creates.is_empty());
            assert!(state.captain_homes.is_empty());
            assert!(world.is_some());
        }
        let script = program_with_sources(
            "synthetic:captain.ai",
            body,
            &format!("{COMMON}native CreateCaptains takes nothing returns nothing\n"),
            AI_COMMON,
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        let error = runtime.start(&mut world, 1, "captain.ai").unwrap_err();
        assert!(error.contains("CreateCaptains"), "{error}");
        assert!(shared.lock().unwrap().gold.is_empty());
        assert!(world.is_some());
    }

    #[test]
    fn gathering_uses_active_player_and_keeps_sleeping_context() {
        let script = program_with_ai_common(
            "synthetic:gather.ai",
            "function main takes nothing returns nothing\ncall StopGathering()\ncall Sleep(0.25)\ncall StopGathering()\nendfunction\n",
            &format!("{AI_COMMON}{AI_GATHERING}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        runtime.start(&mut world, 1, "gather.ai").unwrap();
        runtime.start(&mut world, 2, "gather.ai").unwrap();
        assert_eq!(
            shared.lock().unwrap().gathering,
            [(1, false), (2, false)].into()
        );
        let remaining = runtime.remaining_loads;
        runtime.start(&mut world, 1, "again.ai").unwrap();
        assert_eq!(runtime.remaining_loads, remaining);
        shared.lock().unwrap().clock = 0.25;
        while let Some((_, player, ticket)) = runtime.next_wake(0.25) {
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        assert_eq!(
            shared.lock().unwrap().gathering,
            [(1, false), (2, false)].into()
        );
        assert!(world.is_some());
    }

    #[test]
    fn gathering_abi_refuses_before_world_writes() {
        let body = "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()), null, 17)\nendfunction\n";
        for (common, ai_common) in [
            (
                COMMON.to_string(),
                format!("{AI_COMMON}native StopGathering takes nothing returns integer\n"),
            ),
            (
                COMMON.to_string(),
                format!("{AI_COMMON}native StopGathering takes integer flag returns nothing\n"),
            ),
            (
                COMMON.to_string(),
                format!("{AI_COMMON}constant {AI_GATHERING}"),
            ),
            (format!("{COMMON}{AI_GATHERING}"), AI_COMMON.to_string()),
        ] {
            let script = program_with_sources("synthetic:gather.ai", body, &common, &ai_common);
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&script)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(Arc::clone(&shared)));
            let error = runtime.start(&mut world, 1, "gather.ai").unwrap_err();
            assert!(error.contains("StopGathering"), "{error}");
            let state = shared.lock().unwrap();
            assert!(state.active.is_empty());
            assert!(state.gold.is_empty());
            assert!(state.gathering.is_empty());
            assert!(world.is_some());
        }
    }

    #[test]
    fn gathering_dispatch_rejects_arguments_and_failed_start_retires_state() {
        let shared = Arc::new(Mutex::new(State::default()));
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        for args in [vec![Value::Null], vec![Value::Bool(false)]] {
            assert!(host.native("StopGathering", &args).is_err());
            assert!(shared.lock().unwrap().gathering.is_empty());
        }
        let script = program_with_ai_common(
            "synthetic:gather.ai",
            "function main takes nothing returns nothing\ncall StopGathering()\ncall GetUnitGoldCost(0)\nendfunction\n",
            &format!("{AI_COMMON}{AI_GATHERING}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let mut world = Some(World(Arc::clone(&shared)));
        assert!(runtime.start(&mut world, 1, "gather.ai").is_err());
        let state = shared.lock().unwrap();
        assert!(state.active.is_empty());
        assert!(state.gathering.is_empty());
        assert!(world.is_some());
    }

    #[test]
    fn flee_flags_use_each_ai_player_and_wake_without_changing_targeting_flags() {
        let ai_common = format!("{AI_COMMON}{AI_TARGET}{AI_FLEE}");
        let script = program_with_ai_common(
            "synthetic:flee.ai",
            "function main takes nothing returns nothing\ncall SetTargetHeroes(GetAiPlayer() == 2)\ncall SetUnitsFlee(GetAiPlayer() == 1)\ncall Sleep(0.25)\ncall SetUnitsFlee(GetAiPlayer() != 1)\nendfunction\n",
            &ai_common,
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        for player in [1, 2] {
            runtime.start(&mut world, player, "flee.ai").unwrap();
        }
        let loads = runtime.remaining_loads;
        runtime.start(&mut world, 1, "other.ai").unwrap();
        assert_eq!(runtime.remaining_loads, loads);
        {
            let state = shared.lock().unwrap();
            assert_eq!(state.units_flee, [(1, true), (2, false)].into());
            assert_eq!(state.target_heroes, [(1, false), (2, true)].into());
        }
        shared.lock().unwrap().clock = 0.25;
        while let Some((_, player, ticket)) = runtime.next_wake(0.25) {
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        let state = shared.lock().unwrap();
        assert_eq!(state.units_flee, [(1, false), (2, true)].into());
        assert_eq!(state.target_heroes, [(1, false), (2, true)].into());
        assert!(world.is_some());
    }

    #[test]
    fn malformed_flee_declarations_refuse_before_activation_or_main_writes() {
        let body = "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()), null, 17)\ncall SetUnitsFlee(true)\nendfunction\n";
        for (common, ai_common) in [
            (
                COMMON.into(),
                format!(
                    "{AI_COMMON}{}",
                    AI_FLEE.replace("boolean flag", "integer flag")
                ),
            ),
            (
                COMMON.into(),
                format!(
                    "{AI_COMMON}{}",
                    AI_FLEE.replace("returns nothing", "returns integer")
                ),
            ),
            (COMMON.into(), format!("{AI_COMMON}constant {AI_FLEE}")),
            (format!("{COMMON}{AI_FLEE}"), AI_COMMON.into()),
        ] {
            let script = program_with_sources("synthetic:flee.ai", body, &common, &ai_common);
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&script)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(Arc::clone(&shared)));
            let error = runtime.start(&mut world, 1, "flee.ai").unwrap_err();
            assert!(error.contains("SetUnitsFlee"), "{error}");
            let state = shared.lock().unwrap();
            assert!(state.active.is_empty());
            assert!(state.units_flee.is_empty());
            assert!(state.gold.is_empty());
            assert!(world.is_some());
        }
    }

    #[test]
    fn flee_dispatch_refuses_bad_arguments_and_failed_main_retires_both_flags() {
        let shared = Arc::new(Mutex::new(State::default()));
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        for args in [
            vec![],
            vec![Value::Int(1)],
            vec![Value::Null],
            vec![Value::Bool(true), Value::Bool(false)],
        ] {
            let error = host.native("SetUnitsFlee", &args).unwrap_err();
            assert!(error.contains("SetUnitsFlee"), "{error}");
            assert!(shared.lock().unwrap().units_flee.is_empty());
        }
        let script = program_with_ai_common(
            "synthetic:flee.ai",
            "function main takes nothing returns nothing\ncall SetTargetHeroes(true)\ncall SetUnitsFlee(true)\ncall GetUnitGoldCost(0)\nendfunction\n",
            &format!("{AI_COMMON}{AI_TARGET}{AI_FLEE}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let mut world = Some(World(Arc::clone(&shared)));
        assert!(runtime.start(&mut world, 1, "flee.ai").is_err());
        let state = shared.lock().unwrap();
        assert!(state.active.is_empty());
        assert!(state.target_heroes.is_empty());
        assert!(state.units_flee.is_empty());
        assert!(world.is_some());
    }

    #[test]
    fn new_heroes_flags_use_each_ai_player_and_wake_without_changing_other_flags() {
        let ai_common = format!("{AI_COMMON}{AI_TARGET}{AI_NEW_HEROES}");
        let script = program_with_ai_common(
            "synthetic:new-heroes.ai",
            "function main takes nothing returns nothing\ncall SetNewHeroes(GetAiPlayer() == 2)\ncall SetTargetHeroes(GetAiPlayer() == 1)\ncall Sleep(0.25)\ncall SetNewHeroes(GetAiPlayer() != 2)\nendfunction\n",
            &ai_common,
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        for player in [1, 2] {
            runtime.start(&mut world, player, "new-heroes.ai").unwrap();
        }
        {
            let state = shared.lock().unwrap();
            assert_eq!(state.new_heroes, [(1, false), (2, true)].into());
            assert_eq!(state.target_heroes, [(1, true), (2, false)].into());
            assert!(state.units_flee.is_empty());
        }
        shared.lock().unwrap().clock = 0.25;
        while let Some((_, player, ticket)) = runtime.next_wake(0.25) {
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        let state = shared.lock().unwrap();
        assert_eq!(state.new_heroes, [(1, true), (2, false)].into());
        assert_eq!(state.target_heroes, [(1, true), (2, false)].into());
        assert!(state.units_flee.is_empty());
        assert!(world.is_some());
    }

    #[test]
    fn campaign_mode_is_owned_by_each_program_and_survives_sleep() {
        let script = program_with_ai_common(
            "synthetic:mode.ai",
            "function main takes nothing returns nothing\nif GetAiPlayer() == 1 then\ncall SetCampaignAI()\nendif\ncall Sleep(0.25)\ncall SetCampaignAI()\nendfunction\n",
            &format!("{AI_COMMON}native SetCampaignAI takes nothing returns nothing\n"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        for player in [1, 2] {
            runtime.start(&mut world, player, "mode.ai").unwrap();
        }
        assert_eq!(shared.lock().unwrap().campaign_modes, [1].into());
        shared.lock().unwrap().clock = 0.25;
        while let Some((_, player, ticket)) = runtime.next_wake(0.25) {
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        assert_eq!(shared.lock().unwrap().campaign_modes, [1, 2].into());
        assert!(world.is_some());
    }

    #[test]
    fn campaign_mode_abi_refuses_before_main_and_arguments_do_not_write() {
        for declaration in [
            "native SetCampaignAI takes integer mode returns nothing\n",
            "native SetCampaignAI takes nothing returns boolean\n",
            "constant native SetCampaignAI takes nothing returns nothing\n",
        ] {
            let script = program_with_ai_common(
                "synthetic:mode.ai",
                "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()),null,17)\nendfunction\n",
                &format!("{AI_COMMON}{declaration}"),
            );
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&script)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(Arc::clone(&shared)));
            let error = runtime.start(&mut world, 1, "mode.ai").unwrap_err();
            assert!(error.contains("SetCampaignAI"), "{error}");
            assert!(shared.lock().unwrap().gold.is_empty());
            assert!(shared.lock().unwrap().active.is_empty());
        }
        let shared = Arc::new(Mutex::new(State::default()));
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        assert!(host.native("SetCampaignAI", &[]).is_err());
        shared.lock().unwrap().active.insert(1, false);
        assert!(host.native("SetCampaignAI", &[Value::Bool(true)]).is_err());
        assert!(shared.lock().unwrap().campaign_modes.is_empty());
        assert_eq!(host.native("SetCampaignAI", &[]).unwrap(), Value::Null);
        assert_eq!(shared.lock().unwrap().campaign_modes, [1].into());
    }

    #[test]
    fn replacement_counts_are_program_local_and_survive_sleep_and_repeat_start() {
        let script = program_with_ai_common(
            "synthetic:replacements.ai",
            "function main takes nothing returns nothing\ncall SetReplacementCount(GetAiPlayer()*17)\ncall Sleep(0.25)\ncall SetReplacementCount(-GetAiPlayer()*29)\nendfunction\n",
            &format!("{AI_COMMON}native SetReplacementCount takes integer count returns nothing\n"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        for player in [1, 2] {
            runtime
                .start(&mut world, player, "replacements.ai")
                .unwrap();
        }
        assert_eq!(
            shared.lock().unwrap().replacement_counts,
            [(1, 17), (2, 34)].into()
        );
        shared.lock().unwrap().clock = 0.25;
        while let Some((_, player, ticket)) = runtime.next_wake(0.25) {
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        runtime.start(&mut world, 1, "replacements.ai").unwrap();
        assert_eq!(
            shared.lock().unwrap().replacement_counts,
            [(1, -29), (2, -58)].into()
        );
        assert!(world.is_some());
    }

    #[test]
    fn unit_count_dispatch_preserves_owner_and_completion_mode() {
        let shared = Arc::new(Mutex::new(State::default()));
        let explicit = HandleId::new(NonZeroU64::new(99).unwrap());
        {
            let mut state = shared.lock().unwrap();
            state.unit_counts = [((1, 17, false), 9), ((1, 17, true), 3)].into();
            state.player_unit_counts.insert((explicit, 17), 5);
        }
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        assert!(host.native("GetUnitCount", &[Value::Int(17)]).is_err());
        shared.lock().unwrap().active.insert(1, false);
        assert_eq!(
            host.native("GetUnitCount", &[Value::Int(17)]).unwrap(),
            Value::Int(9)
        );
        assert_eq!(
            host.native("GetUnitCountDone", &[Value::Int(17)]).unwrap(),
            Value::Int(3)
        );
        assert_eq!(
            host.native(
                "GetPlayerUnitTypeCount",
                &[Value::Handle(explicit), Value::Int(17)]
            )
            .unwrap(),
            Value::Int(5)
        );
        assert_eq!(
            host.native("GetPlayerUnitTypeCount", &[Value::Null, Value::Int(17)])
                .unwrap(),
            Value::Int(0)
        );
        for name in ["GetUnitCount", "GetUnitCountDone"] {
            for args in [
                vec![],
                vec![Value::Bool(true)],
                vec![Value::Int(17), Value::Int(17)],
            ] {
                assert!(host.native(name, &args).is_err());
            }
        }
        for args in [
            vec![Value::Int(17)],
            vec![Value::Int(1), Value::Int(17)],
            vec![Value::Handle(explicit), Value::Real(17.)],
        ] {
            assert!(host.native("GetPlayerUnitTypeCount", &args).is_err());
        }
        assert_eq!(
            shared.lock().unwrap().unit_counts,
            [((1, 17, false), 9), ((1, 17, true), 3)].into()
        );
    }

    #[test]
    fn unit_count_declarations_are_checked_before_activation_or_world_writes() {
        for (name, params) in [
            ("GetUnitCount", "integer unitid"),
            ("GetUnitCountDone", "integer unitid"),
            ("GetPlayerUnitTypeCount", "player owner, integer unitid"),
        ] {
            for (common, ai_common) in [
                (
                    COMMON.to_owned(),
                    format!("{AI_COMMON}native {name} takes {params} returns boolean\n"),
                ),
                (
                    COMMON.to_owned(),
                    format!("{AI_COMMON}constant native {name} takes {params} returns integer\n"),
                ),
                (
                    COMMON.to_owned(),
                    format!("{AI_COMMON}native {name} takes nothing returns integer\n"),
                ),
                (
                    format!("{COMMON}native {name} takes {params} returns integer\n"),
                    AI_COMMON.to_owned(),
                ),
                (
                    format!("{COMMON}native {name} takes {params} returns integer\n"),
                    format!("{AI_COMMON}native {name} takes {params} returns integer\n"),
                ),
            ] {
                let script = program_with_sources(
                    "synthetic:count.ai",
                    "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()),null,17)\nendfunction\n",
                    &common,
                    &ai_common,
                );
                let mut runtime = Runtime::<World>::default();
                runtime.set_loader(move |_| Ok(Arc::clone(&script)));
                let shared = Arc::new(Mutex::new(State::default()));
                let mut world = Some(World(Arc::clone(&shared)));
                let error = runtime.start(&mut world, 1, "count.ai").unwrap_err();
                assert!(error.contains(name), "{error}");
                assert!(shared.lock().unwrap().gold.is_empty());
                assert!(shared.lock().unwrap().active.is_empty());
            }
        }
    }

    #[test]
    fn replacement_abi_and_runtime_arguments_refuse_without_writes() {
        for declaration in [
            "native SetReplacementCount takes boolean count returns nothing\n",
            "native SetReplacementCount takes integer count returns integer\n",
            "constant native SetReplacementCount takes integer count returns nothing\n",
            "native SetReplacementCount takes nothing returns nothing\n",
        ] {
            let script = program_with_ai_common(
                "synthetic:replacements.ai",
                "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()),null,17)\nendfunction\n",
                &format!("{AI_COMMON}{declaration}"),
            );
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&script)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(Arc::clone(&shared)));
            let error = runtime.start(&mut world, 1, "replacements.ai").unwrap_err();
            assert!(error.contains("SetReplacementCount"), "{error}");
            assert!(shared.lock().unwrap().gold.is_empty());
            assert!(shared.lock().unwrap().active.is_empty());
        }
        let shared = Arc::new(Mutex::new(State::default()));
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        assert!(
            host.native("SetReplacementCount", &[Value::Int(17)])
                .is_err()
        );
        shared.lock().unwrap().active.insert(1, false);
        for args in [
            vec![],
            vec![Value::Bool(true)],
            vec![Value::Real(1.0)],
            vec![Value::Int(1), Value::Int(2)],
        ] {
            assert!(host.native("SetReplacementCount", &args).is_err());
            assert!(shared.lock().unwrap().replacement_counts.is_empty());
        }
        assert_eq!(
            host.native("SetReplacementCount", &[Value::Int(i32::MIN)])
                .unwrap(),
            Value::Null
        );
        assert_eq!(
            shared.lock().unwrap().replacement_counts,
            [(1, i32::MIN)].into()
        );
        let failing = program_with_ai_common(
            "synthetic:replacement-failure.ai",
            "function main takes nothing returns nothing\ncall SetReplacementCount(73)\ncall Unsupported()\nendfunction\n",
            &format!(
                "{AI_COMMON}native SetReplacementCount takes integer count returns nothing\nnative Unsupported takes nothing returns nothing\n"
            ),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&failing)));
        let mut world = Some(World(Arc::clone(&shared)));
        assert!(
            runtime
                .start(&mut world, 2, "replacement-failure.ai")
                .is_err()
        );
        assert_eq!(
            shared.lock().unwrap().replacement_counts,
            [(1, i32::MIN)].into()
        );
        assert!(!shared.lock().unwrap().active.contains_key(&2));
        assert!(world.is_some());
    }

    #[test]
    fn malformed_new_heroes_declarations_refuse_before_activation_or_main_writes() {
        let body = "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()), null, 17)\ncall SetNewHeroes(true)\nendfunction\n";
        for (common, ai_common) in [
            (
                COMMON.into(),
                format!(
                    "{AI_COMMON}{}",
                    AI_NEW_HEROES.replace("boolean flag", "integer flag")
                ),
            ),
            (
                COMMON.into(),
                format!(
                    "{AI_COMMON}{}",
                    AI_NEW_HEROES.replace("returns nothing", "returns integer")
                ),
            ),
            (
                COMMON.into(),
                format!("{AI_COMMON}constant {AI_NEW_HEROES}"),
            ),
            (format!("{COMMON}{AI_NEW_HEROES}"), AI_COMMON.into()),
        ] {
            let script = program_with_sources("synthetic:new-heroes.ai", body, &common, &ai_common);
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&script)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(Arc::clone(&shared)));
            let error = runtime.start(&mut world, 1, "new-heroes.ai").unwrap_err();
            assert!(error.contains("SetNewHeroes"), "{error}");
            let state = shared.lock().unwrap();
            assert!(state.active.is_empty());
            assert!(state.new_heroes.is_empty());
            assert!(state.gold.is_empty());
            assert!(world.is_some());
        }
    }

    #[test]
    fn new_heroes_dispatch_refuses_bad_arguments_and_failed_main_retires_the_flag() {
        let shared = Arc::new(Mutex::new(State::default()));
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        for args in [
            vec![],
            vec![Value::Int(1)],
            vec![Value::Null],
            vec![Value::Bool(true), Value::Bool(false)],
        ] {
            let error = host.native("SetNewHeroes", &args).unwrap_err();
            assert!(error.contains("SetNewHeroes"), "{error}");
            assert!(shared.lock().unwrap().new_heroes.is_empty());
        }
        let script = program_with_ai_common(
            "synthetic:new-heroes.ai",
            "function main takes nothing returns nothing\ncall SetTargetHeroes(true)\ncall SetNewHeroes(true)\ncall GetUnitGoldCost(0)\nendfunction\n",
            &format!("{AI_COMMON}{AI_TARGET}{AI_NEW_HEROES}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let mut world = Some(World(Arc::clone(&shared)));
        assert!(runtime.start(&mut world, 1, "new-heroes.ai").is_err());
        let state = shared.lock().unwrap();
        assert!(state.active.is_empty());
        assert!(state.target_heroes.is_empty());
        assert!(state.new_heroes.is_empty());
        assert!(world.is_some());
    }

    #[test]
    fn slow_chopping_flags_use_each_ai_player_and_wake_without_changing_other_flags() {
        let ai_common = format!("{AI_COMMON}{AI_TARGET}{AI_SLOW_CHOPPING}");
        let script = program_with_ai_common(
            "synthetic:slow-chopping.ai",
            "function main takes nothing returns nothing\ncall SetSlowChopping(GetAiPlayer() == 2)\ncall SetTargetHeroes(GetAiPlayer() == 1)\ncall Sleep(0.25)\ncall SetSlowChopping(GetAiPlayer() != 2)\nendfunction\n",
            &ai_common,
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        for player in [1, 2] {
            runtime
                .start(&mut world, player, "slow-chopping.ai")
                .unwrap();
        }
        {
            let state = shared.lock().unwrap();
            assert_eq!(state.slow_chopping, [(1, false), (2, true)].into());
            assert_eq!(state.target_heroes, [(1, true), (2, false)].into());
            assert!(state.units_flee.is_empty());
        }
        shared.lock().unwrap().clock = 0.25;
        while let Some((_, player, ticket)) = runtime.next_wake(0.25) {
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        let state = shared.lock().unwrap();
        assert_eq!(state.slow_chopping, [(1, true), (2, false)].into());
        assert_eq!(state.target_heroes, [(1, true), (2, false)].into());
        assert!(state.units_flee.is_empty());
        assert!(world.is_some());
    }

    #[test]
    fn malformed_slow_chopping_declarations_refuse_before_activation_or_main_writes() {
        let body = "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()), null, 17)\ncall SetSlowChopping(true)\nendfunction\n";
        for (common, ai_common) in [
            (
                COMMON.into(),
                format!(
                    "{AI_COMMON}{}",
                    AI_SLOW_CHOPPING.replace("boolean flag", "integer flag")
                ),
            ),
            (
                COMMON.into(),
                format!(
                    "{AI_COMMON}{}",
                    AI_SLOW_CHOPPING.replace("returns nothing", "returns integer")
                ),
            ),
            (
                COMMON.into(),
                format!("{AI_COMMON}constant {AI_SLOW_CHOPPING}"),
            ),
            (format!("{COMMON}{AI_SLOW_CHOPPING}"), AI_COMMON.into()),
        ] {
            let script =
                program_with_sources("synthetic:slow-chopping.ai", body, &common, &ai_common);
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&script)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(Arc::clone(&shared)));
            let error = runtime
                .start(&mut world, 1, "slow-chopping.ai")
                .unwrap_err();
            assert!(error.contains("SetSlowChopping"), "{error}");
            let state = shared.lock().unwrap();
            assert!(state.active.is_empty());
            assert!(state.slow_chopping.is_empty());
            assert!(state.gold.is_empty());
            assert!(world.is_some());
        }
    }

    #[test]
    fn slow_chopping_dispatch_refuses_bad_arguments_and_failed_main_retires_the_flag() {
        let shared = Arc::new(Mutex::new(State::default()));
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        for args in [
            vec![],
            vec![Value::Int(1)],
            vec![Value::Null],
            vec![Value::Bool(true), Value::Bool(false)],
        ] {
            let error = host.native("SetSlowChopping", &args).unwrap_err();
            assert!(error.contains("SetSlowChopping"), "{error}");
            assert!(shared.lock().unwrap().slow_chopping.is_empty());
        }
        let script = program_with_ai_common(
            "synthetic:slow-chopping.ai",
            "function main takes nothing returns nothing\ncall SetTargetHeroes(true)\ncall SetSlowChopping(true)\ncall GetUnitGoldCost(0)\nendfunction\n",
            &format!("{AI_COMMON}{AI_TARGET}{AI_SLOW_CHOPPING}"),
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let mut world = Some(World(Arc::clone(&shared)));
        assert!(runtime.start(&mut world, 1, "slow-chopping.ai").is_err());
        let state = shared.lock().unwrap();
        assert!(state.active.is_empty());
        assert!(state.target_heroes.is_empty());
        assert!(state.slow_chopping.is_empty());
        assert!(world.is_some());
    }

    #[test]
    fn failed_wake_retires_only_its_discarded_vm_and_shared_flags() {
        let common = format!("{AI_COMMON}{AI_TARGET}{AI_FLEE}{AI_GROUP_FLEE}{AI_SLOW_CHOPPING}");
        let failed = program_with_ai_common(
            "synthetic:failed.ai",
            "function main takes nothing returns nothing\ncall SetTargetHeroes(true)\ncall SetUnitsFlee(true)\ncall SetGroupsFlee(true)\ncall SetSlowChopping(true)\ncall Sleep(0.25)\ncall GetUnitGoldCost(0)\nendfunction\n",
            &common,
        );
        let peer = program_with_ai_common(
            "synthetic:peer.ai",
            "function main takes nothing returns nothing\ncall SetTargetHeroes(false)\ncall SetUnitsFlee(false)\ncall SetGroupsFlee(false)\ncall SetSlowChopping(false)\ncall Sleep(1.0)\nendfunction\n",
            &common,
        );
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |name| match name {
            "failed.ai" => Ok(Arc::clone(&failed)),
            "peer.ai" => Ok(Arc::clone(&peer)),
            _ => Err("missing synthetic program".into()),
        });
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        runtime.start(&mut world, 1, "failed.ai").unwrap();
        runtime.start(&mut world, 2, "peer.ai").unwrap();
        let loads = runtime.remaining_loads;
        shared.lock().unwrap().clock = 0.25;
        let (_, player, ticket) = runtime.next_wake(0.25).unwrap();
        assert_eq!(player, 1);
        let error = runtime.resume(&mut world, player, ticket).unwrap_err();
        assert!(error.contains("GetUnitGoldCost"), "{error}");
        assert!(!runtime.active.contains_key(&1));
        assert!(runtime.active.contains_key(&2));
        assert_eq!(runtime.remaining_loads, loads);
        assert!(world.is_some());
        assert!(world.as_mut().unwrap().ai_set_units_flee(1, true).is_err());
        assert!(world.as_mut().unwrap().ai_set_groups_flee(1, true).is_err());
        assert!(
            world
                .as_mut()
                .unwrap()
                .ai_set_slow_chopping(1, true)
                .is_err()
        );
        let state = shared.lock().unwrap();
        assert_eq!(state.active.keys().copied().collect::<Vec<_>>(), [2]);
        assert_eq!(state.target_heroes, [(2, false)].into());
        assert_eq!(state.units_flee, [(2, false)].into());
        assert_eq!(state.groups_flee, [(2, false)].into());
        assert_eq!(state.slow_chopping, [(2, false)].into());
        drop(state);
        assert_eq!(runtime.next_wake(1.0).unwrap().1, 2);
    }

    #[test]
    fn set_target_heroes_requires_the_admitted_ai_common_declaration_and_active_player() {
        let ai_common = format!("{AI_COMMON}{AI_TARGET}");
        let body = "function main takes nothing returns nothing\ncall SetTargetHeroes(true)\nendfunction\n";
        let script = program_with_ai_common("synthetic:red.ai", body, &ai_common);
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        runtime.start(&mut world, 1, "red.ai").unwrap();
        {
            let state = shared.lock().unwrap();
            assert_eq!(state.active.get(&1), Some(&false));
            assert_eq!(state.target_heroes.get(&1), Some(&true));
        }
        assert!(world.is_some());
    }

    #[test]
    fn failed_ai_main_rolls_back_the_activation_and_target_flag() {
        let ai_common = format!("{AI_COMMON}{AI_TARGET}");
        let body = "function main takes nothing returns nothing\ncall SetTargetHeroes(true)\ncall GetUnitGoldCost(0)\nendfunction\n";
        let script = program_with_ai_common("synthetic:red.ai", body, &ai_common);
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        let error = runtime.start(&mut world, 1, "red.ai").unwrap_err();
        assert!(error.contains("GetUnitGoldCost"), "{error}");
        let state = shared.lock().unwrap();
        assert!(state.active.is_empty());
        assert!(state.target_heroes.is_empty());
        assert!(world.is_some());
    }

    #[test]
    fn malformed_set_target_heroes_declaration_refuses_before_main_runs() {
        let ai_common = format!(
            "{AI_COMMON}{}",
            AI_TARGET.replace("boolean flag", "integer flag")
        );
        let body = "function main takes nothing returns nothing\ncall SetTargetHeroes(true)\nendfunction\n";
        let script = program_with_ai_common("synthetic:red.ai", body, &ai_common);
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        let error = runtime.start(&mut world, 1, "red.ai").unwrap_err();
        assert!(error.contains("SetTargetHeroes"), "{error}");
        assert!(error.contains("declaration"), "{error}");
        assert!(shared.lock().unwrap().active.is_empty());
        assert!(world.is_some());
    }

    #[test]
    fn malformed_ai_native_abi_refuses_before_globals_or_spawned_child_write() {
        let body = "globals\ninteger g = 0\nendglobals\nfunction child takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()), null, 11)\nendfunction\nfunction main takes nothing returns nothing\ncall StartThread(function child)\nendfunction\n";
        for declaration in [
            AI_COMMON.replace(
                "StartThread takes code callback returns nothing",
                "StartThread takes code callback returns integer",
            ),
            AI_COMMON.replace("native Sleep", "constant native Sleep"),
            AI_COMMON.replace(
                "GetAiPlayer takes nothing returns integer",
                "GetAiPlayer takes nothing returns real",
            ),
            AI_COMMON.replace(
                "GetUpgradeLevel takes integer id returns integer",
                "GetUpgradeLevel takes integer id returns real",
            ),
            AI_COMMON.replace(
                "GetUnitGoldCost takes integer unitid returns integer",
                "GetUnitGoldCost takes integer unitid returns real",
            ),
            AI_COMMON.replace(
                "GetUnitWoodCost takes integer unitid returns integer",
                "GetUnitWoodCost takes real unitid returns integer",
            ),
        ] {
            let script = program_with_ai_common("synthetic:red.ai", body, &declaration);
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&script)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(Arc::clone(&shared)));
            let error = runtime.start(&mut world, 1, "red.ai").unwrap_err();
            assert!(error.contains("declaration"), "{error}");
            assert!(world.is_some());
            assert!(shared.lock().unwrap().gold.is_empty());
        }
        let common = format!("{COMMON}{AI_COMMON}");
        let script =
            program_with_sources("synthetic:red.ai", body, &common, "// empty AI common\n");
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        let error = runtime.start(&mut world, 1, "red.ai").unwrap_err();
        assert!(error.contains("declaration"), "{error}");
        assert!(world.is_some());
        assert!(shared.lock().unwrap().gold.is_empty());

        let moved = format!("{COMMON}native GetUpgradeLevel takes integer id returns integer\n");
        let ai_common = AI_COMMON.replace(
            "native GetUpgradeLevel takes integer id returns integer\n",
            "",
        );
        let script = program_with_sources("synthetic:red.ai", body, &moved, &ai_common);
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        let error = runtime.start(&mut world, 1, "red.ai").unwrap_err();
        assert!(error.contains("GetUpgradeLevel"), "{error}");
        assert!(world.is_some());
        assert!(shared.lock().unwrap().gold.is_empty());
    }

    #[test]
    fn ai_queries_reject_wrong_arity_and_type_without_touching_world() {
        let shared = Arc::new(Mutex::new(State::default()));
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        for name in ["GetUpgradeLevel", "GetUnitGoldCost", "GetUnitWoodCost"] {
            for args in [
                vec![],
                vec![Value::Real(5.0)],
                vec![Value::Int(5), Value::Int(6)],
            ] {
                assert!(host.native(name, &args).unwrap_err().contains("integer"));
                assert!(host.world.is_some());
                assert!(shared.lock().unwrap().gold.is_empty());
            }
        }
    }

    struct Source {
        info: SourceInfo,
        exact: Option<Result<&'static [u8], ()>>,
    }

    impl ContentSource for Source {
        fn info(&self) -> &SourceInfo {
            &self.info
        }
        fn paths(&self) -> Result<Vec<String>, SourceError> {
            let mut paths = vec!["Alt\\unit.ai".into()];
            if self.exact.is_some() {
                paths.push("unit.ai".into());
            }
            Ok(paths)
        }
        fn read(&self, member: &str) -> Result<Option<SourceContent>, SourceError> {
            let bytes = match member {
                "Alt\\unit.ai" => b"fallback".as_slice(),
                "unit.ai" => self
                    .exact
                    .as_ref()
                    .ok_or(SourceError::InvalidInput {
                        detail: "absent".into(),
                    })?
                    .as_ref()
                    .map_err(|()| SourceError::InvalidInput {
                        detail: "unreadable".into(),
                    })?,
                _ => return Ok(None),
            };
            Ok(Some(SourceContent {
                bytes: bytes.to_vec(),
                declared_length: bytes.len() as u64,
                locator: member.into(),
            }))
        }
        fn read_bounded(
            &self,
            member: &str,
            limit: u64,
        ) -> Result<Option<SourceContent>, SourceError> {
            let content = self.read(member)?;
            if content
                .as_ref()
                .is_some_and(|content| content.declared_length > limit)
            {
                return Err(SourceError::Limit {
                    requested: content.unwrap().declared_length,
                    limit,
                });
            }
            Ok(content)
        }
    }

    #[test]
    fn image_named_exact_source_wins_and_only_absence_uses_prefix() {
        let names = SourceNames::from_addresses(
            |at| {
                Ok(match at {
                    1 => "root.j",
                    2 => "other.ai",
                    3 => "Alt\\",
                    _ => panic!("unexpected address"),
                }
                .into())
            },
            [1, 2, 3],
        )
        .unwrap();
        assert_eq!(names.common, "root.j");
        for (exact, expected) in [
            (Some(Ok(b"exact".as_slice())), Some(b"exact".as_slice())),
            (None, Some(b"fallback".as_slice())),
            (Some(Err(())), None),
        ] {
            let reader = pcx_assets::Resolver::new(
                GameMode::default(),
                None,
                vec![ArchiveLayer {
                    priority: 1,
                    source: Box::new(Source {
                        info: SourceInfo {
                            id: "synthetic".into(),
                            kind: LayerKind::Mpq,
                            origin: "synthetic.mpq".into(),
                            version: Some("fixture".into()),
                        },
                        exact,
                    }),
                }],
            )
            .unwrap();
            let result = names.resolve(&reader, "unit.ai");
            match expected {
                Some(bytes) => assert_eq!(result.unwrap().bytes, bytes),
                None => assert!(result.unwrap_err().to_string().contains("unreadable")),
            }
        }
    }

    #[test]
    fn two_ai_programs_keep_globals_and_sleepers_separate_on_one_world() {
        let source = "globals\ninteger localCount = 0\nendglobals\nfunction child takes nothing returns nothing\ncall Sleep(1.0)\nset localCount = localCount + 10\ncall SetPlayerState(Player(GetAiPlayer()), null, GetPlayerState(Player(GetAiPlayer()), null) + 1)\nendfunction\nfunction main takes nothing returns nothing\nset localCount = 1\ncall StartThread(function child)\ncall Sleep(2.0)\nset localCount = localCount + 100\nendfunction\n";
        let red = program("synthetic:red.ai", source);
        let purple = program("synthetic:purple.ai", source);
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |name| match name {
            "red.ai" => Ok(Arc::clone(&red)),
            "purple.ai" => Ok(Arc::clone(&purple)),
            _ => Err("missing synthetic AI".into()),
        });
        let shared = Arc::new(Mutex::new(State::default()));
        let mut world = Some(World(Arc::clone(&shared)));
        runtime.start(&mut world, 1, "red.ai").unwrap();
        assert_eq!(runtime.active[&1].global("localCount"), Value::Int(1));
        runtime.start(&mut world, 2, "purple.ai").unwrap();
        let remaining = runtime.remaining_loads;
        runtime.start(&mut world, 1, "purple.ai").unwrap();
        assert_eq!(runtime.remaining_loads, remaining);
        assert!(runtime.start(&mut world, 3, "missing.ai").is_err());
        assert_eq!(runtime.active.len(), 2);
        assert!(world.is_some());
        assert_eq!(runtime.active[&1].global("localCount"), Value::Int(1));
        assert_eq!(runtime.active[&2].global("localCount"), Value::Int(1));
        assert_eq!(runtime.next_wake(0.5), None);
        shared.lock().unwrap().clock = 1.0;
        for _ in 0..2 {
            let (_, player, ticket) = runtime.next_wake(1.0).unwrap();
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        assert_eq!(shared.lock().unwrap().gold.len(), 2);
        assert_eq!(runtime.active[&1].global("localCount"), Value::Int(11));
        assert_eq!(runtime.active[&2].global("localCount"), Value::Int(11));
        shared.lock().unwrap().clock = 2.0;
        for _ in 0..2 {
            let (_, player, ticket) = runtime.next_wake(2.0).unwrap();
            runtime.resume(&mut world, player, ticket).unwrap();
        }
        assert_eq!(runtime.active[&1].global("localCount"), Value::Int(111));
        assert_eq!(runtime.active[&2].global("localCount"), Value::Int(111));
        let remaining = runtime.remaining_loads;
        runtime.start(&mut world, 1, "purple.ai").unwrap();
        assert_eq!(runtime.remaining_loads, remaining);
        assert_eq!(runtime.active[&1].global("localCount"), Value::Int(111));
    }

    #[test]
    fn map_callback_risk_refuses_ai_write_before_world_mutation() {
        let source = "function main takes nothing returns nothing\ncall SetPlayerState(Player(GetAiPlayer()), null, 7)\nendfunction\n";
        let script = program("synthetic:red.ai", source);
        let mut runtime = Runtime::<World>::default();
        runtime.set_loader(move |_| Ok(Arc::clone(&script)));
        let shared = Arc::new(Mutex::new(State {
            callbacks: true,
            ..State::default()
        }));
        let mut world = Some(World(Arc::clone(&shared)));
        assert!(
            runtime
                .start(&mut world, 1, "red.ai")
                .unwrap_err()
                .contains("map callbacks")
        );
        assert!(world.is_some());
        assert!(shared.lock().unwrap().gold.is_empty());
    }

    #[test]
    #[ignore = "requires the locally supplied selected campaign script sources"]
    fn selected_human02_ai_sources_execute_to_named_frontiers() {
        let read = |key: &str| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let names = SourceNames::selected(&read("PCX_CAMPAIGN_IMAGE")).unwrap();
        assert!(!names.common.is_empty() && !names.ai_common.is_empty());
        let common = read("PCX_CAMPAIGN_AI_COMMON_J");
        let ai_common = read("PCX_CAMPAIGN_AI_COMMON_AI");
        let red = read("PCX_CAMPAIGN_AI_RED");
        let purple = read("PCX_CAMPAIGN_AI_PURPLE");
        for (name, bytes) in [("red.ai", red), ("purple.ai", purple)] {
            let scripts = Arc::new(
                MapScripts::compile(
                    AssetIdentity {
                        edition: "selected-local".into(),
                        version: "selected-client".into(),
                        source: name.into(),
                    },
                    ScriptAsset {
                        label: "selected:common.j",
                        bytes: &common,
                    },
                    ScriptAsset {
                        label: "selected:common.ai",
                        bytes: &ai_common,
                    },
                    ScriptAsset {
                        label: name,
                        bytes: &bytes,
                    },
                    None,
                )
                .unwrap(),
            );
            let mut runtime = Runtime::<World>::default();
            runtime.set_loader(move |_| Ok(Arc::clone(&scripts)));
            let shared = Arc::new(Mutex::new(State::default()));
            let mut world = Some(World(shared));
            let error = runtime.start(&mut world, 1, name).unwrap_err();
            assert!(world.is_some());
            assert!(
                error.contains("no source-qualified world behavior"),
                "{name}: {error}"
            );
            let frontier = if name == "red.ai" {
                "SetTargetHeroes"
            } else {
                "SetHeroLevels"
            };
            assert!(error.contains(frontier), "{name}: {error}");
            eprintln!("{name}: {error}");
        }
    }
}
