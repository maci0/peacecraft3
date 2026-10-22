//! Per-player campaign script continuations over the admitted map world.
use crate::map_startup_host::NeutralStack;
use crate::scripts::MapScripts;
use pcx_assets::AssetPath;
use pcx_script::{Host, OwnedVm, ScriptCall, Value};
use std::collections::BTreeMap;
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
}

pub(crate) struct Runtime<W: AiWorld> {
    loader: Option<Loader>,
    active: BTreeMap<i32, OwnedVm<AiHost<W>>>,
    remaining_loads: usize,
}

impl<W: AiWorld> Default for Runtime<W> {
    fn default() -> Self {
        Self {
            loader: None,
            active: BTreeMap::new(),
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
        let inner = world
            .take()
            .ok_or("StartCampaignAI: world is already lent")?;
        let mut vm = OwnedVm::new_owned(scripts.shared_program(), AiHost::new(player, inner));
        let result = vm.init_globals().and_then(|()| {
            let main = vm.program().function_id("main").expect("admitted AI main");
            vm.run_thread(&[main])
        });
        *world = vm.host().world.take();
        result.map_err(|e| format!("StartCampaignAI player {player}: {e}"))?;
        self.active.insert(player, vm);
        Ok(())
    }

    pub(crate) fn next_wake(&mut self, until: f64) -> Option<(f64, i32, u64)> {
        self.active
            .iter_mut()
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
        callbacks: bool,
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
    const AI_COMMON: &str = "native GetAiPlayer takes nothing returns integer\nnative Sleep takes real seconds returns nothing\nnative StartThread takes code callback returns nothing\nnative GetUpgradeLevel takes integer id returns integer\n";

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
    fn upgrade_query_rejects_wrong_arity_and_type_without_touching_world() {
        let shared = Arc::new(Mutex::new(State::default()));
        let mut host = AiHost::new(1, World(Arc::clone(&shared)));
        for args in [
            vec![],
            vec![Value::Real(5.0)],
            vec![Value::Int(5), Value::Int(6)],
        ] {
            assert!(
                host.native("GetUpgradeLevel", &args)
                    .unwrap_err()
                    .contains("integer")
            );
            assert!(host.world.is_some());
            assert!(shared.lock().unwrap().gold.is_empty());
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
