//! Original-input VM gate for a campaign unit's generated death-drop action.
//!
//! Runs `Unit<N>_DropItems` from the map's generated `war3map.j`, with the selected
//! `common.j` and `Blizzard.j` supplying the `RandomDist*` and `UnitDropItem` bodies. The
//! host records every native the drop path calls. Random draws return the lower bound,
//! a fixed stream, so the draw count and bounds are checked, not the RNG values. Event
//! delivery from `EVENT_UNIT_DEATH` is not exercised; the trigger is called directly.
//!
//! Requires four local files, none committed:
//! - `PEACECRAFT3_CAMPAIGN_COMMON`: `common.j`
//! - `PEACECRAFT3_CAMPAIGN_BLIZZARD`: `Blizzard.j`
//! - `PEACECRAFT3_CAMPAIGN_SCRIPT`: that map's `war3map.j`
//! - `PEACECRAFT3_CAMPAIGN_UNITS`: that map's `war3mapUnits.doo`
use pcx_map::Units;
use pcx_play::playercolor_host::{EnumHost, Limits};
use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{HandleId, Host, Value};
use std::cell::RefCell;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::rc::Rc;

/// Radius of the random drop position in `UnitDropItem` (`Blizzard.j`).
const DROP_RADIUS: f32 = 32.0;

/// Conversion natives that `common.j` global initializers call before any drop runs.
/// Their names are the selection, and their values come from the selected `common.j`
/// through `EnumHost`.
const CONVERSION_NATIVES: &[&str] = &[
    "ConvertRace",
    "ConvertAllianceType",
    "ConvertRacePref",
    "ConvertIGameState",
    "ConvertFGameState",
    "ConvertPlayerState",
    "ConvertPlayerScore",
    "ConvertPlayerGameResult",
    "ConvertUnitState",
    "ConvertAIDifficulty",
    "ConvertGameEvent",
    "ConvertPlayerEvent",
    "ConvertPlayerUnitEvent",
    "ConvertWidgetEvent",
    "ConvertDialogEvent",
    "ConvertUnitEvent",
    "ConvertLimitOp",
    "ConvertUnitType",
    "ConvertGameSpeed",
    "ConvertPlacement",
    "ConvertStartLocPrio",
    "ConvertGameDifficulty",
    "ConvertGameType",
    "ConvertMapFlag",
    "ConvertMapDensity",
    "ConvertMapControl",
    "ConvertPlayerColor",
    "ConvertPlayerSlotState",
    "ConvertVolumeGroup",
    "ConvertCameraField",
    "ConvertBlendMode",
    "ConvertRarityControl",
    "ConvertTexMapFlags",
    "ConvertFogState",
    "ConvertEffectType",
    "ConvertVersion",
    "ConvertItemType",
    "ConvertAttackType",
    "ConvertDamageType",
    "ConvertWeaponType",
    "ConvertSoundType",
    "ConvertPathingType",
    "ConvertMouseButtonType",
];

/// Player-slot queries that `common.j` reads only while initializing globals. The Host
/// does not implement them (docs/re-notes/native-neutral-players.md), so the gate
/// returns a harness value. The drop-phase assertions require none of them to run.
const NEUTRAL_QUERY_NATIVES: &[&str] = &[
    "GetPlayerNeutralPassive",
    "GetPlayerNeutralAggressive",
    "GetBJMaxPlayers",
    "GetBJPlayerNeutralVictim",
    "GetBJPlayerNeutralExtra",
    "GetBJMaxPlayerSlots",
];

fn required(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")))
}

fn handle(token: u64) -> HandleId {
    HandleId::new(NonZeroU64::new(token).unwrap())
}

#[derive(Default)]
struct Recorder {
    trigger_unit: Option<HandleId>,
    trigger: Option<HandleId>,
    unit_type: i32,
    unit_x: f32,
    unit_y: f32,
    hidden: bool,
    next_token: u64,
    int_draws: Vec<(i32, i32)>,
    real_draws: Vec<(f32, f32)>,
    created: Vec<(i32, f32, f32)>,
    items: Vec<HandleId>,
    drop_ids: Vec<(HandleId, i32)>,
    destroyed: Vec<HandleId>,
    neutral_queries: Vec<String>,
}

impl Recorder {
    /// A new handle token for natives that only allocate an object.
    fn fresh(&mut self) -> u64 {
        self.next_token += 1;
        self.next_token
    }

    /// `Some` for a native the drop path calls, `None` to defer to the enum host.
    fn native(&mut self, name: &str, args: &[Value]) -> Option<Result<Value, String>> {
        Some(Ok(match (name, args) {
            ("GetTriggerUnit", []) => Value::Handle(self.trigger_unit.unwrap()),
            ("GetTriggeringTrigger", []) => Value::Handle(self.trigger.unwrap()),
            ("IsUnitHidden", [Value::Handle(_)]) => Value::Bool(self.hidden),
            ("GetUnitTypeId", [Value::Handle(_)]) => Value::Int(self.unit_type),
            ("GetUnitX", [Value::Handle(_)]) => Value::Real(self.unit_x),
            ("GetUnitY", [Value::Handle(_)]) => Value::Real(self.unit_y),
            ("GetRandomInt", [Value::Int(low), Value::Int(high)]) => {
                self.int_draws.push((*low, *high));
                Value::Int(*low)
            }
            ("GetRandomReal", [Value::Real(low), Value::Real(high)]) => {
                self.real_draws.push((*low, *high));
                Value::Real(*low)
            }
            ("CreateItem", [Value::Int(raw), Value::Real(x), Value::Real(y)]) => {
                self.created.push((*raw, *x, *y));
                let item = handle(self.fresh());
                self.items.push(item);
                Value::Handle(item)
            }
            ("SetItemDropID", [Value::Handle(item), Value::Int(unit)]) => {
                self.drop_ids.push((*item, *unit));
                Value::Null
            }
            ("GetItemLevel", [Value::Handle(_)]) => Value::Int(0),
            ("DestroyTrigger", [Value::Handle(trigger)]) => {
                self.destroyed.push(*trigger);
                Value::Null
            }
            ("CreateTimer", []) => Value::Handle(handle(self.fresh())),
            ("CreateGroup", []) => Value::Handle(handle(self.fresh())),
            (name, []) if NEUTRAL_QUERY_NATIVES.contains(&name) => {
                self.neutral_queries.push(name.to_string());
                Value::Int(0)
            }
            _ => return None,
        }))
    }
}

/// Shares the recorder between the VM, which owns its host, and the test.
struct Probe {
    recorder: Rc<RefCell<Recorder>>,
    enums: EnumHost,
}

impl Host for Probe {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        if name == "GetItemType" {
            // Harness value for the stock bookkeeping in `UpdateStockAvailability`. It writes
            // only `bj_stockAllowed*`, which the drop assertions do not read.
            return self.enums.native("ConvertItemType", &[Value::Int(0)]);
        }
        let recorded = self.recorder.borrow_mut().native(name, args);
        match recorded {
            Some(result) => result,
            None => self.enums.native(name, args),
        }
    }
}

fn compile() -> MapScripts {
    let common = std::fs::read(required("PEACECRAFT3_CAMPAIGN_COMMON")).unwrap();
    let blizzard = std::fs::read(required("PEACECRAFT3_CAMPAIGN_BLIZZARD")).unwrap();
    let map = std::fs::read(required("PEACECRAFT3_CAMPAIGN_SCRIPT")).unwrap();
    MapScripts::compile(
        AssetIdentity {
            edition: "legacy".into(),
            version: "1.29.2.9231".into(),
            source: "local campaign gate".into(),
        },
        ScriptAsset {
            label: "common.j",
            bytes: &common,
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: &blizzard,
        },
        ScriptAsset {
            label: "war3map.j",
            bytes: &map,
        },
        None,
    )
    .unwrap()
}

/// Rawcode, x and y of the placed unit with the given creation number.
fn placed(creation: u32) -> (i32, f32, f32) {
    let data = std::fs::read(required("PEACECRAFT3_CAMPAIGN_UNITS")).unwrap();
    let units = Units::parse(&data).unwrap();
    let unit = units
        .instances
        .iter()
        .find(|unit| unit.creation_number == creation)
        .expect("creation number present");
    (
        i32::from_be_bytes(unit.type_id),
        unit.position[0],
        unit.position[1],
    )
}

fn run_drop(function: &str, hidden: bool, creation: u32) -> Recorder {
    let scripts = compile();
    let (unit_type, x, y) = placed(creation);
    let recorder = Rc::new(RefCell::new(Recorder {
        trigger_unit: Some(handle(1)),
        trigger: Some(handle(2)),
        unit_type,
        unit_x: x,
        unit_y: y,
        hidden,
        next_token: 100,
        ..Recorder::default()
    }));
    let enums = EnumHost::checked_selected(
        scripts.shared_program(),
        "campaign drop gate",
        CONVERSION_NATIVES,
        Limits {
            max_handles: 4096,
            admission_steps: 1_000_000,
            expression_depth: 64,
            expression_work: 1_000_000,
        },
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    {
        let mut vm = scripts.vm(Probe {
            recorder: Rc::clone(&recorder),
            enums,
        });
        vm.init_globals().unwrap_or_else(|e| panic!("{e:?}"));
        recorder.borrow_mut().neutral_queries.clear();
        vm.call(function, &[]).unwrap_or_else(|e| panic!("{e:?}"));
    }
    Rc::try_unwrap(recorder)
        .unwrap_or_else(|_| panic!("recorder still shared"))
        .into_inner()
}

/// One visible death of placed unit `creation` through `function` must draw its one
/// authored set and create its `item` at the drawn position, once.
fn assert_single_visible_drop(function: &str, creation: u32, item: &[u8; 4]) {
    let (unit_type, x, y) = placed(creation);
    let recorder = run_drop(function, false, creation);
    // One RandomDistChoose draw over the single entry (chance 100).
    assert_eq!(recorder.int_draws, [(1, 100)]);
    // UnitDropItem: two position draws around the unit.
    assert_eq!(
        recorder.real_draws,
        [
            (x - DROP_RADIUS, x + DROP_RADIUS),
            (y - DROP_RADIUS, y + DROP_RADIUS),
        ]
    );
    // The item is the authored rawcode at the drawn position (lower bounds under the host stream).
    assert_eq!(
        recorder.created,
        [(i32::from_be_bytes(*item), x - DROP_RADIUS, y - DROP_RADIUS)]
    );
    // Drop origin is the dropping unit's type id.
    assert_eq!(recorder.drop_ids, [(recorder.items[0], unit_type)]);
    assert!(recorder.neutral_queries.is_empty());
    // The trigger destroys itself exactly once, the duplicate-death guard.
    assert_eq!(recorder.destroyed, [handle(2)]);
}

/// Creation 79, a named unit (`gg_unit_nban_0079`), whose set is one `pman` at chance 100.
#[test]
#[ignore = "requires PEACECRAFT3_CAMPAIGN_COMMON, BLIZZARD, SCRIPT and UNITS local original inputs"]
fn named_unit_drop_draws_one_set_and_creates_the_rolled_item() {
    assert_single_visible_drop("Unit000025_DropItems", 79, b"pman");
}

/// Creation 94, a named unit (`gg_unit_nban_0094`), whose set is one `shea` at chance 100.
#[test]
#[ignore = "requires PEACECRAFT3_CAMPAIGN_COMMON, BLIZZARD, SCRIPT and UNITS local original inputs"]
fn second_named_unit_drops_its_own_set() {
    assert_single_visible_drop("Unit000037_DropItems", 94, b"shea");
}

/// Creation 37, an `ngno` unit created by `CreateNeutralHostile` with no `gg_` name. Its
/// death trigger registers on the local `u`, and the binary creation sits at the same
/// position and type as that `CreateUnit` call.
#[test]
#[ignore = "requires PEACECRAFT3_CAMPAIGN_COMMON, BLIZZARD, SCRIPT and UNITS local original inputs"]
fn unnamed_unit_drop_links_through_the_local_creation_site() {
    assert_single_visible_drop("Unit000057_DropItems", 37, b"phea");
}

#[test]
#[ignore = "requires PEACECRAFT3_CAMPAIGN_COMMON, BLIZZARD, SCRIPT and UNITS local original inputs"]
fn hidden_unit_draws_nothing_and_still_destroys_its_trigger() {
    let recorder = run_drop("Unit000025_DropItems", true, 79);
    assert!(recorder.int_draws.is_empty());
    assert!(recorder.real_draws.is_empty());
    assert!(recorder.created.is_empty());
    assert_eq!(recorder.destroyed, [handle(2)]);
    assert!(recorder.neutral_queries.is_empty());
}
