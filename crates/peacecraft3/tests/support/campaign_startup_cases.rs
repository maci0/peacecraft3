use crate::binding::Fixture;
use bevy::prelude::*;
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_objects::editor::{EditorReadLimits, SlkId};
use pcx_play::camera_margins::{
    CameraMarginBinding, CameraMarginBudget, CameraMargins, SourceWindow,
};
use pcx_play::day_night;
use pcx_play::file_sounds;
use pcx_play::label_sounds::{self, SoundBank};
use pcx_play::map_startup_host::{
    CurrentResearchInput, DefendSelection, EnumContracts, MapStartupBudgets, MapStartupHost,
    MapStartupInputs, NeutralRuleSet, PostConfigObservations, StartupClockInput,
};
use pcx_play::neutral_player::{
    AGGRESSIVE_NATIVE_NAME, BJ_MAX_PLAYER_SLOTS_NATIVE_NAME, BJ_MAX_PLAYERS_NATIVE_NAME,
    BJ_NEUTRAL_EXTRA_NATIVE_NAME, BJ_NEUTRAL_VICTIM_NATIVE_NAME, LocalProfileApproval, NATIVE_NAME,
    RuleInputBudget, VerifiedNeutralRule,
};
use pcx_play::playercolor_host::{
    DestructableBudget, FilterBudget, ForceBudget, Limits, MapConfigBudget, PlayerAllianceBudget,
    PlayerBudget, PlayerStateRequestBudget, StockBudget, TechLimitBudget, TriggerBudget,
    UnitBudget, UnitStateBudget,
};
use pcx_play::scripts::{AssetIdentity, ScriptAsset};
use pcx_play::sound_parameters::{self, Banks};
use pcx_play::startup_clock::{self, StartupClock};
use pcx_script::Value;
use pcx_slq::CoordinateLimits;
use peacecraft3::browser_configuration::SelectedMapSources;
use peacecraft3::campaign_startup::{CampaignHostSupply, CampaignStartup, StartupError};
use peacecraft3::map_binding::bind_map;
use peacecraft3::map_requests::{MapPayload, MapRequest, MapRequests, RequestFailure};
use peacecraft3::map_script_session::{MapScriptSession, ScriptSessionError};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const COMMON: &str = "\
type agent extends handle
type timer extends agent
type group extends agent
type widget extends agent
type unit extends widget
type destructable extends widget
type player extends agent
type sound extends agent
type trigger extends agent
type event extends agent
type boolexpr extends agent
type filterfunc extends boolexpr
type force extends agent
type rect extends agent
type eventid extends handle
type unitevent extends eventid
type gamestate extends handle
type igamestate extends gamestate
type fgamestate extends gamestate
type limitop extends handle
type playercolor extends handle
type racepreference extends handle
type mapcontrol extends handle
type placement extends handle
type playerstate extends handle
type alliancetype extends handle
type unitstate extends handle
type startlocprio extends handle
type camerafield extends handle
type camerasetup extends handle
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native CreateGroup takes nothing returns group
native DestroyGroup takes group g returns nothing
native GroupClear takes group g returns nothing
native FirstOfGroup takes group g returns unit
native GroupAddUnit takes group g, unit u returns nothing
native GroupRemoveUnit takes group g, unit u returns nothing
constant native IsUnitInGroup takes unit u, group g returns boolean
native SetMapName takes string name returns nothing
native SetMapDescription takes string description returns nothing
native SetPlayers takes integer n returns nothing
native SetTeams takes integer n returns nothing
native GetPlayers takes nothing returns integer
native GetTeams takes nothing returns integer
native SetGamePlacement takes placement p returns nothing
constant native GetGamePlacement takes nothing returns placement
native DefineStartLocation takes integer i, real x, real y returns nothing
constant native GetStartLocationX takes integer i returns real
constant native GetStartLocationY takes integer i returns real
constant native Player takes integer number returns player
constant native GetPlayerId takes player p returns integer
native SetPlayerTeam takes player p, integer team returns nothing
native GetPlayerTeam takes player p returns integer
native SetPlayerStartLocation takes player p, integer index returns nothing
native ForcePlayerStartLocation takes player p, integer index returns nothing
native GetPlayerStartLocation takes player p returns integer
native SetPlayerColor takes player p, playercolor c returns nothing
native GetPlayerColor takes player p returns playercolor
native SetPlayerRacePreference takes player p, racepreference r returns nothing
native SetPlayerRaceSelectable takes player p, boolean b returns nothing
native GetPlayerSelectable takes player p returns boolean
native SetPlayerController takes player p, mapcontrol c returns nothing
native GetPlayerController takes player p returns mapcontrol
native SetPlayerName takes player p, string name returns nothing
native GetPlayerName takes player p returns string
native SetPlayerOnScoreScreen takes player p, boolean b returns nothing
native SetPlayerAlliance takes player p, player other, alliancetype which, boolean value returns nothing
constant native ConvertAllianceType takes integer i returns alliancetype
native SetPlayerState takes player p, playerstate which, integer value returns nothing
constant native ConvertPlayerState takes integer i returns playerstate
constant native SetPlayerTechMaxAllowed takes player p, integer tech, integer maximum returns nothing
constant native GetPlayerTechMaxAllowed takes player p, integer tech returns integer
constant native SetPlayerTechResearched takes player p, integer tech, integer level returns nothing
native SetStartLocPrioCount takes integer start, integer count returns nothing
native SetStartLocPrio takes integer start, integer slot, integer other, startlocprio priority returns nothing
native GetStartLocPrioSlot takes integer start, integer slot returns integer
native GetStartLocPrio takes integer start, integer slot returns startlocprio
native GetCameraMargin takes integer which returns real
native SetCameraBounds takes real a, real b, real c, real d, real e, real f, real g, real h returns nothing
constant native GetCameraBoundMinX takes nothing returns real
constant native GetCameraBoundMinY takes nothing returns real
constant native GetCameraBoundMaxX takes nothing returns real
constant native GetCameraBoundMaxY takes nothing returns real
native SetDayNightModels takes string terrain, string units returns nothing
native CreateMIDISound takes string label, integer a, integer b returns sound
native CreateSoundFromLabel takes string label, boolean looping, boolean is3d, boolean stop, integer fadeIn, integer fadeOut returns sound
native StartSound takes sound s returns nothing
native SetMapMusic takes string name, boolean random, integer index returns nothing
native CreateSound takes string file, boolean looping, boolean is3d, boolean stop, integer fadeIn, integer fadeOut, string eax returns sound
native SetSoundParamsFromLabel takes sound s, string label returns nothing
native SetSoundDuration takes sound s, integer duration returns nothing
native Rect takes real minx, real miny, real maxx, real maxy returns rect
native RemoveRect takes rect r returns nothing
native SetRect takes rect r, real minx, real miny, real maxx, real maxy returns nothing
native MoveRectTo takes rect r, real x, real y returns nothing
native GetRectCenterX takes rect r returns real
native GetRectCenterY takes rect r returns real
native GetRectMinX takes rect r returns real
native GetRectMinY takes rect r returns real
native GetRectMaxX takes rect r returns real
native GetRectMaxY takes rect r returns real
native SetSoundChannel takes sound s, integer channel returns nothing
native SetSoundVolume takes sound s, integer volume returns nothing
native SetSoundPitch takes sound s, real pitch returns nothing
native SetSoundDistanceCutoff takes sound s, real cutoff returns nothing
native SetSoundDistances takes sound s, real min, real max returns nothing
native SetSoundConeAngles takes sound s, real inside, real outside, integer outsideVolume returns nothing
native SetSoundConeOrientation takes sound s, real x, real y, real z returns nothing
native SetSoundPosition takes sound s, real x, real y, real z returns nothing
native SetSoundVelocity takes sound s, real x, real y, real z returns nothing
native RegisterStackedSound takes sound s, boolean byPosition, real width, real height returns nothing
native UnregisterStackedSound takes sound s, boolean byPosition, real width, real height returns nothing
native CreateCameraSetup takes nothing returns camerasetup
native CameraSetupSetField takes camerasetup which, camerafield field, real value, real duration returns nothing
native CameraSetupGetField takes camerasetup which, camerafield field returns real
native CameraSetupSetDestPosition takes camerasetup which, real x, real y, real duration returns nothing
native CameraSetupGetDestPositionX takes camerasetup which returns real
native CameraSetupGetDestPositionY takes camerasetup which returns real
constant native GetFloatGameState takes fgamestate which returns real
constant native ConvertFGameState takes integer i returns fgamestate
constant native ConvertIGameState takes integer i returns igamestate
constant native ConvertLimitOp takes integer i returns limitop
constant native ConvertUnitState takes integer i returns unitstate
constant native ConvertCameraField takes integer i returns camerafield
constant native ConvertUnitEvent takes integer i returns unitevent
native CreateDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable
native CreateUnit takes player p, integer unitid, real x, real y, real face returns unit
native SetUnitState takes unit which, unitstate state, real value returns nothing
native IssueImmediateOrder takes unit which, string order returns boolean
native CreateTrigger takes nothing returns trigger
native TriggerRegisterUnitEvent takes trigger which, unit u, unitevent event returns event
native TriggerRegisterTimerExpireEvent takes trigger which, timer t returns event
native TriggerRegisterGameStateEvent takes trigger which, gamestate state, limitop op, real limit returns event
native Filter takes code predicate returns filterfunc
native CreateForce takes nothing returns force
native ForceAddPlayer takes force which, player p returns nothing
native GetPlayerNeutralPassive takes nothing returns integer
native GetPlayerNeutralAggressive takes nothing returns integer
native GetBJMaxPlayers takes nothing returns integer
native GetBJPlayerNeutralVictim takes nothing returns integer
native GetBJPlayerNeutralExtra takes nothing returns integer
native GetBJMaxPlayerSlots takes nothing returns integer
native SetAllItemTypeSlots takes integer slots returns nothing
native SetAllUnitTypeSlots takes integer slots returns nothing
native SetItemTypeSlots takes unit u, integer slots returns nothing
native SetUnitTypeSlots takes unit u, integer slots returns nothing
globals
constant fgamestate CLOCK = ConvertFGameState(2)
constant igamestate IGAME = ConvertIGameState(1)
constant limitop LIM = ConvertLimitOp(0)
constant unitstate USTATE = ConvertUnitState(1)
constant camerafield CFIELD = ConvertCameraField(1)
constant playerstate PSTATE = ConvertPlayerState(1)
constant alliancetype ALLY = ConvertAllianceType(1)
constant unitevent UEVENT = ConvertUnitEvent(1)
endglobals
";

const MAP_OK: &str = "\
globals
integer marker = 1
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
endfunction
";

const MAP_REFUSED: &str = "\
globals
integer marker = 2
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call SetAllItemTypeSlots(1)
endfunction
";

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn origin(source: &str) -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic edition".into(),
        version: "synthetic revision".into(),
        source: source.into(),
    }
}

fn coordinates() -> CoordinateLimits {
    CoordinateLimits {
        max_row: 32,
        max_column: 16,
    }
}

fn empty_bank(label: &str) -> SoundBank {
    let table = b"ID;PWXL;N;E\r\nC;X1;Y1;K\"label\"\r\nC;X2;Y1;K\"DirectoryBase\"\r\nC;X3;Y1;K\"FileNames\"\r\nE\r\n";
    SoundBank::read(
        ScriptAsset {
            label,
            bytes: table,
        },
        origin(label),
        "label",
        &[],
        label_sounds::Budget {
            table_bytes: table.len(),
            coordinates: coordinates(),
            sounds: 0,
            resource_bytes: 64,
            total_resource_bytes: 64,
            metadata_bytes: 256,
        },
    )
    .expect(label)
}

fn budgets() -> MapStartupBudgets {
    MapStartupBudgets {
        groups: 8,
        map_config: MapConfigBudget {
            text_bytes: 64,
            start_locations: 4,
        },
        players: PlayerBudget {
            max_slots: 64,
            name_bytes: 256,
        },
        filters: FilterBudget { max_records: 4 },
        conditions: None,
        forces: ForceBudget {
            max_records: 4,
            max_memberships: 8,
        },
        state_requests: PlayerStateRequestBudget { max_records: 4 },
        alliances: PlayerAllianceBudget { max_records: 4 },
        start_priorities: 8,
        sound_starts: 4,
        map_music_bytes: 64,
        file_sound_eax_bytes: 32,
        stacked_sounds: 4,
        camera_setup_fields: 4,
        tech_limits: TechLimitBudget { max_records: 4 },
        tech_researched: TechLimitBudget { max_records: 4 },
        destructables: DestructableBudget { max_records: 4 },
        units: UnitBudget {
            max_records: 4,
            max_retained_fields: 4,
            max_modifications: 4,
        },
        unit_states: UnitStateBudget { max_records: 4 },
        triggers: TriggerBudget {
            max_triggers: 4,
            max_unit_events: 4,
            max_actions: 4,
        },
        trigger_conditions: None,
        player_unit_events: None,
        death_events: None,
        player_events: None,
        regions: None,
        timer_events: 4,
        game_state_events: 4,
        stock: None,
        quests: None,
        items: None,
    }
}

fn sylk(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
    let mut text = String::from("ID;PWXL;N;E\r\n");
    for (y, row) in std::iter::once(headers)
        .chain(rows.iter().copied())
        .enumerate()
    {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

fn defend() -> DefendSelection {
    let headers = [
        "alias",
        "code",
        "hero",
        "item",
        "checkDep",
        "levels",
        "reqLevel",
        "levelSkip",
        "Cool1",
        "Cost1",
    ];
    let ability = sylk(
        &headers,
        &[&["Dfnd", "Dcls", "0", "0", "1", "1", "0", "0", "0", "0"]],
    );
    let upgrade = sylk(&["upgradeid", "maxlevel"], &[&["Utec", "1"]]);
    let profile = "[Dfnd]\r\nOrder=hold\r\nUnorder=drop\r\nRequires=Utec\r\n";
    let tables = DefaultTables::from_bytes(
        Identity {
            install: "synthetic defend install".into(),
            edition_version: "synthetic defend revision".into(),
        },
        &[
            (SlkId::AbilityData.path(), ability.as_slice()),
            (SlkId::UpgradeData.path(), upgrade.as_slice()),
        ],
        &[("Units\\synthabilityfunc.txt", profile.as_bytes())],
        EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 128 << 10,
        },
    )
    .expect("synthetic defend tables");
    DefendSelection {
        tables,
        alias: "Dfnd".into(),
    }
}

fn camera() -> CameraMargins {
    let mut image = vec![0; 0x400];
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x2000),
        (0x100, 0x200),
        (0x104, 0x1000),
        (0x108, 0x200),
        (0x10c, 0x200),
        (0x11c, 0x40000040),
    ] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    image[..2].copy_from_slice(b"MZ");
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [(0x84, 0x14cu16), (0x86, 1), (0x94, 0x60), (0x98, 0x10b)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    let mut write = |at: usize, bytes: &[u8]| {
        image[at..at + bytes.len()].copy_from_slice(bytes);
        SourceWindow {
            va: 0x301000 + (at - 0x200) as u32,
            len: bytes.len() as u32,
            digest: digest(bytes),
        }
    };
    let section = write(0x200, b"SyntheticMargins\0");
    let keys = [
        write(0x220, b"z\0"),
        write(0x230, b"a\0"),
        write(0x240, b"q\0"),
        write(0x250, b"b\0"),
    ];
    let scale = write(0x260, &1.5f32.to_le_bytes());
    let config = b"[SyntheticMargins]\nz=2\na=4\nq=6\nb=8\n";
    CameraMargins::read(
        &image,
        CameraMarginBinding {
            origin: origin("synthetic camera image"),
            image_sha256: digest(&image),
            section,
            keys,
            scale,
        },
        config,
        origin("synthetic camera config"),
        CameraMarginBudget {
            image: RuleInputBudget {
                image_bytes: image.len(),
                profile_bytes: 256,
                sections: 2,
            },
            config_bytes: config.len(),
        },
    )
    .expect("synthetic camera")
}

fn clock() -> StartupClock {
    let mut image = vec![0; 0x400];
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x2000),
        (0x100, 0x200),
        (0x104, 0x1000),
        (0x108, 0x200),
        (0x10c, 0x200),
        (0x11c, 0x60000060),
    ] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    image[..2].copy_from_slice(b"MZ");
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [(0x84, 0x14cu16), (0x86, 1), (0x94, 0x60), (0x98, 0x10b)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    let initializer = [
        0x6a, 6, 0x68, 0, 0, 0, 0, 0xe8, 0, 0, 0, 0, 0x83, 0xc4, 8, 0xc3,
    ];
    image[0x200..0x210].copy_from_slice(&initializer);
    StartupClock::read(
        &image,
        startup_clock::Binding {
            origin: origin("synthetic clock image"),
            image_sha256: digest(&image),
            initializer: SourceWindow {
                va: 0x301000,
                len: initializer.len() as u32,
                digest: digest(&initializer),
            },
        },
        RuleInputBudget {
            image_bytes: image.len(),
            profile_bytes: initializer.len(),
            sections: 2,
        },
    )
    .expect("synthetic clock")
}

fn getter(threshold: u32, mask: i8, add: i8) -> Vec<u8> {
    let mut bytes = vec![0xa1];
    bytes.extend(0x11223344u32.to_le_bytes());
    bytes.extend([0x8b, 0x48, 0x28, 0x8d, 0x49, 0x18, 0xe8]);
    bytes.extend(37i32.to_le_bytes());
    bytes.extend([0x50, 0xe8]);
    bytes.extend((-73i32).to_le_bytes());
    bytes.extend([0x83, 0xc4, 4, 0x3d]);
    bytes.extend(threshold.to_le_bytes());
    bytes.extend([
        0x1b, 0xc0, 0x83, 0xe0, mask as u8, 0x83, 0xc0, add as u8, 0xc3,
    ]);
    bytes
}

fn header() -> Vec<u8> {
    let mut bytes = vec![0x83, 0xf8, 100, 0x76, 7, 0x33, 0xc0, 0xe9];
    bytes.extend(81i32.to_le_bytes());
    bytes.extend([0x83, 0xf8, 1, 0x72, 19, 0x53, 0x8b, 0xce, 0xe8]);
    bytes.extend((1234i32 - 25).to_le_bytes());
    bytes.extend([0x8d, 0x43, 4, 0x8b, 0xce, 0x50, 0xe8]);
    bytes.extend((1234i32 - 36).to_le_bytes());
    bytes
}

#[derive(Clone, Copy)]
enum RuleKind {
    Passive,
    Aggressive,
    MaxPlayers,
    Victim,
    Extra,
    Slots,
}

fn rule(kind: RuleKind) -> Arc<VerifiedNeutralRule> {
    let native_name = match kind {
        RuleKind::Passive => NATIVE_NAME,
        RuleKind::Aggressive => AGGRESSIVE_NATIVE_NAME,
        RuleKind::MaxPlayers => BJ_MAX_PLAYERS_NATIVE_NAME,
        RuleKind::Victim => BJ_NEUTRAL_VICTIM_NATIVE_NAME,
        RuleKind::Extra => BJ_NEUTRAL_EXTRA_NATIVE_NAME,
        RuleKind::Slots => BJ_MAX_PLAYER_SLOTS_NATIVE_NAME,
    };
    let base = 0x0030_0000u32;
    let rva = 0x1000u32;
    let raw = 0x200usize;
    let mut image = vec![0; raw + 0x1000];
    image[..2].copy_from_slice(b"MZ");
    image[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    image[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    image[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    image[0x94..0x96].copy_from_slice(&0x60u16.to_le_bytes());
    image[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    image[0xb4..0xb8].copy_from_slice(&base.to_le_bytes());
    image[0xd0..0xd4].copy_from_slice(&0x3000u32.to_le_bytes());
    let section = 0xf8;
    image[section + 8..section + 12].copy_from_slice(&0x1000u32.to_le_bytes());
    image[section + 12..section + 16].copy_from_slice(&rva.to_le_bytes());
    image[section + 16..section + 20].copy_from_slice(&0x1000u32.to_le_bytes());
    image[section + 20..section + 24].copy_from_slice(&(raw as u32).to_le_bytes());
    image[section + 36..section + 40].copy_from_slice(&0x6000_0060u32.to_le_bytes());
    let mut name = native_name.as_bytes().to_vec();
    name.push(0);
    let signature = b"()I\0".to_vec();
    let body = getter(257, -20, 61);
    let head = header();
    let offsets = [0x30usize, 0x100, 0x140, 0x200, 0x300];
    let va = |offset| base + rva + offset as u32;
    let mut registration = vec![0x68];
    registration.extend(va(offsets[1]).to_le_bytes());
    registration.push(0xb9);
    registration.extend(va(offsets[2]).to_le_bytes());
    registration.push(0xba);
    registration.extend(va(offsets[3]).to_le_bytes());
    let parts = [registration, name, signature, body, head];
    for (at, part) in offsets.iter().zip(&parts) {
        image[raw + at..raw + at + part.len()].copy_from_slice(part);
    }
    let mut profile = b"PCXNPR\0\x01".to_vec();
    profile.extend((image.len() as u64).to_le_bytes());
    profile.extend(digest(&image));
    for text in [
        "synthetic edition",
        "synthetic PE revision",
        "explicit local synthetic profile",
    ] {
        profile.extend((text.len() as u32).to_le_bytes());
        profile.extend(text.as_bytes());
    }
    for (at, part) in offsets.iter().zip(&parts) {
        profile.extend(va(*at).to_le_bytes());
        profile.extend((part.len() as u32).to_le_bytes());
        profile.extend(digest(part));
    }
    for offset in [1u32, 6, 11] {
        profile.extend(offset.to_le_bytes());
    }
    let approval = LocalProfileApproval::from_reviewed_fingerprint(digest(&profile));
    let budget = RuleInputBudget {
        image_bytes: image.len(),
        profile_bytes: profile.len(),
        sections: 2,
    };
    let imported = match kind {
        RuleKind::Passive => VerifiedNeutralRule::import(&image, &profile, &approval, budget),
        RuleKind::Aggressive => {
            VerifiedNeutralRule::import_aggressive(&image, &profile, &approval, budget)
        }
        RuleKind::MaxPlayers => VerifiedNeutralRule::import_bj_max_players_first_family(
            &image, &profile, &approval, budget,
        ),
        RuleKind::Victim => VerifiedNeutralRule::import_bj_neutral_victim_first_family(
            &image, &profile, &approval, budget,
        ),
        RuleKind::Extra => VerifiedNeutralRule::import_bj_neutral_extra_first_family(
            &image, &profile, &approval, budget,
        ),
        RuleKind::Slots => VerifiedNeutralRule::import_bj_max_player_slots_first_family(
            &image, &profile, &approval, budget,
        ),
    };
    Arc::new(imported.expect(native_name))
}

fn host_inputs(catalog: Arc<dyn pcx_sim::world::ObjectCatalog + Send + Sync>) -> MapStartupInputs {
    MapStartupInputs {
        random: None,
        enumeration: EnumContracts {
            context: "synthetic admitted map enumeration".into(),
            names: [
                "ConvertFGameState",
                "ConvertIGameState",
                "ConvertLimitOp",
                "ConvertUnitState",
                "ConvertCameraField",
                "ConvertPlayerState",
                "ConvertAllianceType",
                "ConvertUnitEvent",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            limits: Limits {
                max_handles: 64,
                admission_steps: 10_000,
                expression_depth: 16,
                expression_work: 10_000,
            },
        },
        rules: NeutralRuleSet {
            passive: rule(RuleKind::Passive),
            aggressive: rule(RuleKind::Aggressive),
            max_players: rule(RuleKind::MaxPlayers),
            victim: rule(RuleKind::Victim),
            extra: rule(RuleKind::Extra),
            slots: rule(RuleKind::Slots),
        },
        budgets: budgets(),
        camera: camera(),
        day_night: day_night::Assets::read(
            &[],
            day_night::Budget {
                models: 0,
                model_bytes: 64,
                total_bytes: 64,
                path_bytes: 64,
            },
        )
        .expect("day night"),
        label_sounds: empty_bank("synthetic midi bank"),
        label_sound_creation: vec![empty_bank("synthetic label bank")],
        file_sounds: file_sounds::Assets::read(
            &[],
            file_sounds::Budget {
                resources: 0,
                resource_bytes: 64,
                total_bytes: 64,
                metadata_bytes: 64,
            },
        )
        .expect("file sounds"),
        sound_parameters: Banks::read(
            &[],
            sound_parameters::BanksBudget {
                banks: 0,
                table_bytes: 64,
                total_table_bytes: 64,
                coordinates: coordinates(),
                definitions: 0,
                metadata_bytes: 64,
            },
        )
        .expect("sound parameters"),
        clock: StartupClockInput {
            clock: clock(),
            global_name: "CLOCK".into(),
            max_globals: 64,
            expression_depth: 16,
            expression_work: 10_000,
        },
        catalog,
        defend: defend(),
        current_research: None::<CurrentResearchInput>,
        timer_time: None,
        post_config: PostConfigObservations::default(),
    }
}

fn fixture(script: &str) -> Fixture {
    let mut fixture = Fixture::new(1);
    fixture.common = COMMON.as_bytes().to_vec();
    fixture
        .files
        .iter_mut()
        .find(|(name, _)| *name == "war3map.j")
        .unwrap()
        .1 = script.as_bytes().to_vec();
    fixture.rebuild();
    fixture
}

fn deliver(app: &mut App, script: &str, revision: u64) {
    let fixture = fixture(script);
    let resolver = fixture.resolver();
    let bound = bind_map(&resolver, fixture.input(revision), fixture.limits()).expect("bind");
    let catalog = Arc::clone(&bound.catalog.0);
    let bytes = bound.catalog_map_bytes.clone();
    let identity = fixture.map_id.clone();
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..Default::default()
    });
    app.insert_resource(CampaignHostSupply {
        revision,
        inputs: Some(host_inputs(catalog)),
    });
    app.world_mut().write_message(MapRequest {
        label: "chosen map label".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").expect("member"),
        identity,
        binding_revision: revision,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
}

fn session_marker(app: &mut App) -> Value {
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .global("marker")
        .expect("published session")
}

fn session_slots(app: &mut App) -> Option<usize> {
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .expect("published host")
        .objects_mut()
        .player_slots()
}

#[test]
fn delivered_maps_publish_one_session_and_keep_it_on_refusal_or_stale_bindings() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    app.init_resource::<SelectedMapSources>();
    deliver(&mut app, MAP_OK, 7);
    let first = {
        let requests = app.world().resource::<MapRequests>();
        assert!(requests.last_outcome.as_ref().unwrap().result.is_ok());
        let admitted = requests
            .admission
            .shared_context()
            .expect("first admission");
        let receipts = requests.published_receipts.clone().expect("receipts");
        assert!(
            app.world()
                .resource::<SelectedMapSources>()
                .for_receipts(&receipts)
                .is_none()
        );
        assert!(
            app.world()
                .resource::<CampaignStartup>()
                .last_error
                .is_none()
        );
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        let published = session.context().expect("first session");
        assert!(Arc::ptr_eq(published, &admitted));
        admitted.map_digest.hash
    };
    assert_eq!(session_marker(&mut app), Value::Int(1));
    assert!(session_slots(&mut app).is_some_and(|slots| slots > 0));

    deliver(&mut app, MAP_REFUSED, 8);
    let second = {
        let requests = app.world().resource::<MapRequests>();
        assert!(requests.last_outcome.as_ref().unwrap().result.is_ok());
        let admitted = requests
            .admission
            .shared_context()
            .expect("second admission");
        let receipts = requests
            .published_receipts
            .clone()
            .expect("second receipts");
        assert!(
            app.world()
                .resource::<SelectedMapSources>()
                .for_receipts(&receipts)
                .is_none()
        );
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        let published = session.context().expect("retained session");
        assert!(!Arc::ptr_eq(published, &admitted));
        assert_eq!(published.map_digest.hash, first);
        assert_ne!(admitted.map_digest.hash, first);
        admitted.map_digest.hash
    };
    let startup = app.world().resource::<CampaignStartup>();
    match startup.last_error.as_ref() {
        Some(StartupError::Session(ScriptSessionError::Execution(failure))) => {
            assert!(
                failure.diagnostic.contains("SetAllItemTypeSlots"),
                "{}",
                failure.diagnostic
            );
            assert!(
                failure.diagnostic.contains("unsupported by this host"),
                "{}",
                failure.diagnostic
            );
        }
        other => panic!("expected the unsupported native, got {other:?}"),
    }
    assert_eq!(session_marker(&mut app), Value::Int(1));

    let stale_bytes = app
        .world()
        .resource::<MapRequests>()
        .bindings
        .as_ref()
        .unwrap()
        .catalog_map_bytes
        .clone();
    let stale_identity = app
        .world()
        .resource::<MapRequests>()
        .bindings
        .as_ref()
        .unwrap()
        .map_identity
        .clone();
    let member = app
        .world()
        .resource::<MapRequests>()
        .bindings
        .as_ref()
        .unwrap()
        .receipts
        .map
        .path
        .clone();
    app.world_mut().write_message(MapRequest {
        label: "stale map".into(),
        map_member: member,
        identity: stale_identity,
        binding_revision: 7,
        payload: MapPayload::Shared(stale_bytes),
    });
    app.update();
    let requests = app.world().resource::<MapRequests>();
    assert!(matches!(
        requests.last_outcome.as_ref().unwrap().result,
        Err(RequestFailure::StaleBindings {
            requested: 7,
            current: 8
        })
    ));
    assert_eq!(
        requests.admission.context().unwrap().map_digest.hash,
        second
    );
    let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.context().unwrap().map_digest.hash, first);
    assert!(matches!(
        app.world().resource::<CampaignStartup>().last_error,
        Some(StartupError::Session(ScriptSessionError::Execution(_)))
    ));
}

#[test]
fn binding_revision_bump_abandons_pending_startup_without_session_replace() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    deliver(&mut app, MAP_OK, 7);
    let (context, catalog) = {
        let requests = app.world().resource::<MapRequests>();
        (
            requests.admission.shared_context().unwrap(),
            Arc::clone(&requests.bindings.as_ref().unwrap().catalog.0),
        )
    };
    let config = context.scripts.program().function_id("config").unwrap();
    let ticket = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .callback_ticket(config)
        .unwrap();
    app.world_mut()
        .resource_mut::<MapRequests>()
        .bindings
        .as_mut()
        .unwrap()
        .revision = 8;
    app.insert_resource(CampaignHostSupply {
        revision: 8,
        inputs: Some(host_inputs(catalog)),
    });
    app.update();
    assert!(
        app.world()
            .resource::<CampaignHostSupply>()
            .inputs
            .is_some(),
        "a stale successful outcome must not consume new binding inputs"
    );
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let requests = app.world().resource::<MapRequests>();
    let outcome = requests.last_outcome.as_ref().unwrap();
    assert_eq!(outcome.binding_revision, Some(7));
    assert!(outcome.result.is_ok());
    assert!(Arc::ptr_eq(
        &requests.admission.shared_context().unwrap(),
        &context
    ));
    assert_eq!(
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .call_callback(&ticket, &[])
            .unwrap(),
        Value::Null
    );
    assert_eq!(session_marker(&mut app), Value::Int(1));
}

#[test]
fn local_file_source_label_and_explicit_stock_profile_reach_the_same_session() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    let fixture = fixture(MAP_REFUSED);
    let resolver = fixture.resolver();
    let bound = bind_map(&resolver, fixture.input(3), fixture.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.budgets.stock = Some(StockBudget {
        max_unit_records: 0,
    });
    let bytes = bound.catalog_map_bytes.clone();
    let mut identity = fixture.map_id.clone();
    identity.source = "synthetic/local/selected.w3m".into();
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..Default::default()
    });
    app.insert_resource(CampaignHostSupply {
        revision: 3,
        inputs: Some(inputs),
    });
    app.world_mut().write_message(MapRequest {
        label: "local selected map".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: identity.clone(),
        binding_revision: 3,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    assert_eq!(session_marker(&mut app), Value::Int(2));
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.context().unwrap().sources.map, identity);
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .all_item_type_slots()
            .unwrap(),
        Some(1)
    );
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .stock_unit_records()
            .unwrap(),
        0
    );
}

#[test]
fn failed_post_config_observation_keeps_the_published_session() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    deliver(&mut app, MAP_OK, 7);
    let (context, catalog, bytes, member) = {
        let requests = app.world().resource::<MapRequests>();
        let bindings = requests.bindings.as_ref().unwrap();
        (
            requests.admission.shared_context().unwrap(),
            Arc::clone(&bindings.catalog.0),
            Arc::clone(&bindings.catalog_map_bytes),
            bindings.receipts.map.path.clone(),
        )
    };
    let ticket = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .callback_ticket(context.scripts.program().function_id("config").unwrap())
        .unwrap();
    let mut inputs = host_inputs(catalog);
    inputs.post_config.version_global = Some("NOT_AN_AUTHORED_GLOBAL".into());
    app.insert_resource(CampaignHostSupply {
        revision: 7,
        inputs: Some(inputs),
    });
    app.world_mut().write_message(MapRequest {
        label: "same map, invalid explicit observation".into(),
        map_member: member,
        identity: context.sources.map.clone(),
        binding_revision: 7,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
    match app
        .world()
        .resource::<CampaignStartup>()
        .last_error
        .as_ref()
    {
        Some(StartupError::Session(ScriptSessionError::Factory { source, .. })) => {
            assert!(source.to_string().contains("NOT_AN_AUTHORED_GLOBAL"))
        }
        other => panic!("expected post-config refusal, got {other:?}"),
    }
    assert!(Arc::ptr_eq(
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .unwrap(),
        &context
    ));
    assert!(!Arc::ptr_eq(
        &app.world()
            .resource::<MapRequests>()
            .admission
            .shared_context()
            .unwrap(),
        &context
    ));
    assert_eq!(
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .call_callback(&ticket, &[])
            .unwrap(),
        Value::Null
    );
    assert_eq!(session_marker(&mut app), Value::Int(1));
    let (admitted, catalog) = {
        let requests = app.world().resource::<MapRequests>();
        (
            requests.admission.shared_context().unwrap(),
            Arc::clone(&requests.bindings.as_ref().unwrap().catalog.0),
        )
    };
    app.insert_resource(CampaignHostSupply {
        revision: 7,
        inputs: Some(host_inputs(catalog)),
    });
    app.update();
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    assert!(Arc::ptr_eq(
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .unwrap(),
        &admitted
    ));
    assert!(
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .call_callback(&ticket, &[])
            .is_err()
    );
}

#[test]
fn corrected_host_inputs_retry_a_refused_candidate_without_redelivery() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    deliver(&mut app, MAP_OK, 7);
    let previous = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    deliver(&mut app, MAP_REFUSED, 8);
    let (context, catalog) = {
        let requests = app.world().resource::<MapRequests>();
        (
            requests.admission.shared_context().unwrap(),
            Arc::clone(&requests.bindings.as_ref().unwrap().catalog.0),
        )
    };
    app.update();
    assert!(matches!(
        app.world().resource::<CampaignStartup>().last_error,
        Some(StartupError::Session(ScriptSessionError::Execution(_)))
    ));
    assert!(
        app.world()
            .resource::<CampaignHostSupply>()
            .inputs
            .is_none()
    );
    assert!(previous.upgrade().is_some());

    let mut inputs = host_inputs(Arc::clone(&catalog));
    inputs.budgets.stock = Some(StockBudget {
        max_unit_records: 0,
    });
    app.insert_resource(CampaignHostSupply {
        revision: 7,
        inputs: Some(inputs),
    });
    app.update();
    assert!(matches!(
        app.world().resource::<CampaignStartup>().last_error,
        Some(StartupError::InputRevision {
            supply: 7,
            binding: 8
        })
    ));
    assert!(
        app.world()
            .resource::<CampaignHostSupply>()
            .inputs
            .is_some()
    );
    assert!(previous.upgrade().is_some());
    app.world_mut()
        .resource_mut::<CampaignHostSupply>()
        .revision = 8;
    app.update();
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    assert!(
        app.world()
            .resource::<CampaignHostSupply>()
            .inputs
            .is_none()
    );
    assert!(previous.upgrade().is_none());
    assert!(Arc::ptr_eq(
        app.world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .unwrap(),
        &context
    ));
    assert_eq!(session_marker(&mut app), Value::Int(2));

    let published = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    app.world_mut().resource_mut::<CampaignHostSupply>().inputs = Some(host_inputs(catalog));
    app.update();
    assert!(published.upgrade().is_some());
    assert!(
        app.world()
            .resource::<CampaignHostSupply>()
            .inputs
            .is_some()
    );
    assert_eq!(session_marker(&mut app), Value::Int(2));
}

#[path = "script_timer_cases.rs"]
mod script_timer_cases;

#[path = "player_unit_event_cases.rs"]
mod player_unit_event_cases;

#[path = "player_event_cases.rs"]
mod player_event_cases;

#[path = "region_cases.rs"]
mod region_cases;

#[path = "death_event_cases.rs"]
mod death_event_cases;

#[path = "quest_factory_cases.rs"]
mod quest_factory_cases;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_campaign_startup_recovery_gate() -> u32 {
    delivered_maps_publish_one_session_and_keep_it_on_refusal_or_stale_bindings();
    binding_revision_bump_abandons_pending_startup_without_session_replace();
    local_file_source_label_and_explicit_stock_profile_reach_the_same_session();
    failed_post_config_observation_keeps_the_published_session();
    corrected_host_inputs_retry_a_refused_candidate_without_redelivery();
    5
}

#[path = "item_factory_cases.rs"]
mod item_factory_cases;

#[path = "random_factory_cases.rs"]
mod random_factory_cases;
