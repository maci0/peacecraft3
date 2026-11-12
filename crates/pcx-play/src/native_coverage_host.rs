//! Native/API coverage accounting against the selected program.
//! Declaration counts (1,383 natives / 1,044 library functions in the measured
//! Legacy observation) are inventory facts, not support claims. This module
//! reports, for the admitted program, which natives have host dispatch and
//! which remain explicit refusals. No stock native list is copied here.
//!
//! # The audited list
//!
//! The host's dispatch is spread over its own dispatch chain: `ObjectHost`
//! hands a name to each host module in turn, and a module claims a name by
//! matching it. A module may hand the name on again — `unit_native` passes it
//! to the visibility, look-at, pause, revive and sleep modules — so the chain
//! reaches further than the entry point names. The list below is the names
//! that whole chain claims, grouped by the file that dispatches them, so
//! every entry can be traced to its dispatch.
//!
//! The list cannot be derived from one place — the dispatch uses plain arms,
//! or-pattern arms, tuple arms, `name ==` guards, `matches!` lists, contract
//! tables and the `RealOps` trait — so it is maintained here and checked by
//! `the_audit_lists_every_native_the_host_dispatches`, which reads the
//! crate's source and fails when a dispatch name is missing from it. The
//! check reads the dispatch arms it can see; a name that only appears in a
//! module's contract table or a helper's name list is not seen, so the list
//! can still fall behind those shapes.
use super::ObjectHost;

/// Coverage for one admitted program.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeCoverage {
    /// Natives with host dispatch in this build.
    pub supported: Vec<String>,
    /// Declared natives without host dispatch; calls refuse explicitly.
    pub unsupported: Vec<String>,
    /// Library functions in the admitted program (callable JASS, no host needed).
    pub library_functions: usize,
}

impl ObjectHost {
    /// Names with host dispatch in this build. This is the audit surface;
    /// adding a native wires it here through its existing host module.
    pub fn supported_native_names() -> Vec<&'static str> {
        vec![
            // ally_color_host.rs
            "GetAllyColorFilterState",
            "SetAllyColorFilterState",
            // camera_host.rs
            "GetCameraMargin",
            "SetCameraBounds",
            // camera_setup_host.rs
            "CameraSetupApplyForceDuration",
            "CameraSetupGetDestPositionLoc",
            "CameraSetupGetDestPositionX",
            "CameraSetupGetDestPositionY",
            "CameraSetupGetField",
            "CameraSetupSetDestPosition",
            "CameraSetupSetField",
            "CreateCameraSetup",
            // camera_view_host.rs
            "CameraSetSmoothingFactor",
            "CameraSetSourceNoise",
            "CameraSetTargetNoise",
            "GetCameraField",
            "GetCameraTargetPositionLoc",
            "GetCameraTargetPositionX",
            "PanCameraToTimed",
            "ResetToGameCamera",
            "SetCameraField",
            "SetCameraQuickPosition",
            "SetCameraTargetController",
            // campaign_flee_host.rs
            "SetUnitsFlee",
            // day_night_host.rs
            "SetDayNightModels",
            // dialog_host.rs
            "GetClickedButton",
            "GetClickedDialog",
            // doodad_host.rs
            "SetDoodadAnimationRect",
            // environment_host.rs
            "ResetTerrainFog",
            "SetSkyModel",
            "SetTerrainFogEx",
            // event_delivery_host.rs
            "GetAttacker",
            "GetCancelledStructure",
            "GetConstructedStructure",
            "GetConstructingStructure",
            "GetEventPlayerState",
            "GetEventUnitState",
            "GetIssuedOrderId",
            "GetKillingUnit",
            "GetLearnedSkill",
            "GetLearnedSkillLevel",
            "GetLearningUnit",
            "GetLevelingUnit",
            "GetManipulatedItem",
            "GetManipulatingUnit",
            "GetSellingUnit",
            "GetSoldItem",
            "GetSpellAbilityId",
            "GetSpellTargetUnit",
            "GetSummonedUnit",
            "GetSummoningUnit",
            "GetTrainedUnit",
            "GetTrainedUnitType",
            "GetTriggerDestructable",
            "GetTriggerPlayer",
            "GetTriggerWidget",
            // file_sound_host.rs
            "CreateSound",
            "CreateSoundFilenameWithLabel",
            // filter_host.rs
            "DestroyBoolExpr",
            // fog_modifier_host.rs
            "CreateFogModifierRadius",
            "CreateFogModifierRadiusLoc",
            "CreateFogModifierRect",
            "DestroyFogModifier",
            "FogModifierStart",
            "FogModifierStop",
            // force_host.rs
            "CreateForce",
            "ForForce",
            "ForceAddPlayer",
            "GetEnumPlayer",
            // game_event_host.rs
            "ConvertGameEvent",
            "TriggerRegisterGameEvent",
            // player_chat_event_host.rs
            "TriggerRegisterPlayerChatEvent",
            // group_iteration_host.rs
            "ForGroup",
            "GetEnumUnit",
            // harvest_runtime.rs
            "GetUnitCurrentOrder",
            "IssueImmediateOrder",
            "IssueImmediateOrderById",
            "IssuePointOrder",
            "IssuePointOrderById",
            "IssuePointOrderByIdLoc",
            "IssuePointOrderLoc",
            "IssueTargetOrder",
            "IssueTargetOrderById",
            // item_host.rs
            "IsItemVisible",
            "SetItemPosition",
            "SetItemVisible",
            // leaderboard_host.rs
            "DestroyLeaderboard",
            "LeaderboardDisplay",
            // location_host.rs
            "GetLocationX",
            "GetLocationY",
            "GetUnitLoc",
            "Location",
            "MoveLocation",
            "RemoveLocation",
            // map_config_host.rs
            "IsMapFlagSet",
            "SetMapFlag",
            // map_music_host.rs
            "ClearMapMusic",
            "EndThematicMusic",
            "PlayThematicMusic",
            "SetMapMusic",
            // object_host.rs
            "Atan2",
            "Cos",
            "CreateBlightedGoldmine",
            "CreateGroup",
            "CreateTimer",
            "CreateUnitAtLoc",
            "DestroyGroup",
            "DestroyTimer",
            "FirstOfGroup",
            "GetUnitDefaultAcquireRange",
            "GroupAddUnit",
            "GroupClear",
            "GroupRemoveUnit",
            "IsUnitAlly",
            "IsUnitEnemy",
            "IsUnitInGroup",
            "Sin",
            "SquareRoot",
            "TriggerRegisterVariableEvent",
            // play_music_host.rs
            "PlayMusic",
            "PlayMusicEx",
            "PlayThematicMusicEx",
            "ResumeMusic",
            "SetMusicPlayPosition",
            "SetMusicVolume",
            "SetThematicMusicPlayPosition",
            "StopMusic",
            // player_host.rs
            "GetLocalPlayer",
            "GetPlayerHandicap",
            "GetPlayerHandicapXP",
            "GetPlayerSlotState",
            "GetPlayerTechCount",
            "GetPlayerTechResearched",
            "SetPlayerHandicap",
            "SetPlayerHandicapXP",
            // preload_host.rs
            "Preload",
            "PreloadEnd",
            "PreloadEndEx",
            "PreloadGenClear",
            "PreloadGenEnd",
            "PreloadGenStart",
            "PreloadRefresh",
            "PreloadStart",
            "Preloader",
            // presentation_host.rs
            "EnableOcclusion",
            // rect_host.rs
            "GetRectCenterX",
            "GetRectCenterY",
            "GetRectMaxX",
            "GetRectMaxY",
            "GetRectMinX",
            "GetRectMinY",
            "GetWorldBounds",
            "MoveRectTo",
            "MoveRectToLoc",
            "Rect",
            "RemoveRect",
            "SetRect",
            // script_random_host.rs
            "GetRandomInt",
            "GetRandomReal",
            "SetRandomSeed",
            // selection_host.rs
            "BlzIsUnitSelectable",
            "BlzUnitDisableSelection",
            "ClearSelection",
            "EnableDragSelect",
            "EnablePreSelect",
            "EnableSelect",
            "IsUnitSelected",
            "SelectUnit",
            "SyncSelections",
            // sound_host.rs
            "CreateMIDISound",
            "CreateSoundFromLabel",
            "GetSoundDuration",
            "SetSoundDuration",
            "StartSound",
            "StopSound",
            // sound_parameter_host.rs
            "SetSoundParamsFromLabel",
            // stacked_sound_host.rs
            "RegisterStackedSound",
            "UnregisterStackedSound",
            // startup_clock_host.rs
            "GetFloatGameState",
            // startup_settings_host.rs
            "FogEnable",
            "FogMaskEnable",
            "GetDefaultDifficulty",
            "GetGameDifficulty",
            "GetGameSpeed",
            "IsFogEnabled",
            "IsFogMaskEnabled",
            "SetCampaignMenuRace",
            "SetCampaignMenuRaceEx",
            "SetDefaultDifficulty",
            "SetGameDifficulty",
            "SetGameSpeed",
            "VersionGet",
            // time_of_day_host.rs
            "GetTimeOfDayScale",
            "SetTimeOfDayScale",
            "SuspendTimeOfDay",
            // timer_event_host.rs
            "TriggerRegisterTimerExpireEvent",
            // timer_runtime_host.rs
            "GetExpiredTimer",
            "PauseTimer",
            "ResumeTimer",
            "TimerGetElapsed",
            "TimerGetRemaining",
            "TimerGetTimeout",
            "TimerStart",
            // trigger_runtime_host.rs
            "DestroyTrigger",
            "GetTriggerEventId",
            "GetTriggeringTrigger",
            "TriggerRegisterTimerEvent",
            // unit_activity_host.rs
            "GetUnitDefaultMoveSpeed",
            "GetUnitFacing",
            "GetUnitMoveSpeed",
            "SetUnitFacing",
            "SetUnitFacingTimed",
            "SetUnitMoveSpeed",
            "SetUnitPathing",
            "SetUnitPosition",
            "SetUnitPositionLoc",
            // unit_construction_host.rs
            "IssueBuildOrderById",
            // unit_host.rs
            "AddUnitAnimationProperties",
            "CreateUnit",
            "GetFoodMade",
            "GetFoodUsed",
            "GetPlayerStructureCount",
            "GetPlayerUnitCount",
            "QueueUnitAnimation",
            "SetUnitAnimation",
            "SetUnitAnimationWithRarity",
            "SetUnitBlendTime",
            "SetUnitTimeScale",
            "UnitResetCooldown",
            "UnitShareVision",
            // unit_mirror_image_host.rs
            "IsUnitIllusion",
            // unit_morph_host.rs
            "GetUnitFlyHeight",
            "SetUnitFlyHeight",
            // unit_pause_host.rs
            "IsUnitPaused",
            "PauseUnit",
            // alt_minimap_icon_host.rs
            "SetAltMinimapIcon",
            // hero_button_reserve_host.rs
            "SetReservedLocalHeroButtons",
            // unit_alt_icon_host.rs
            "UnitSetUsesAltIcon",
            // unit_creep_guard_host.rs
            "SetUnitCreepGuard",
            // unit_user_data_host.rs
            "GetUnitUserData",
            "SetUnitUserData",
            // unit_waygate_host.rs
            "WaygateActivate",
            "WaygateGetDestinationX",
            "WaygateGetDestinationY",
            "WaygateIsActive",
            "WaygateSetDestination",
            // unit_look_at_host.rs
            "ResetUnitLookAt",
            "SetUnitLookAt",
            // unit_position_host.rs
            "GetFilterUnit",
            "GetUnitX",
            "GetUnitY",
            "GroupEnumUnitsInRange",
            "GroupEnumUnitsInRangeOfLoc",
            "GroupEnumUnitsOfType",
            "GroupEnumUnitsOfTypeCounted",
            "GroupEnumUnitsInRectCounted",
            "GroupEnumUnitsInRangeCounted",
            "GroupEnumUnitsInRangeOfLocCounted",
            "GroupEnumUnitsInRect",
            "GroupEnumUnitsOfPlayer",
            // unit_range_runtime_host.rs
            "GetEnteringUnit",
            "GetTriggerUnit",
            // unit_revive_host.rs
            "ReviveHero",
            "ReviveHeroLoc",
            // unit_sleep_host.rs
            "UnitAddSleep",
            "UnitCanSleep",
            "UnitIsSleeping",
            "UnitWakeUp",
            // unit_summon_host.rs
            "UnitApplyTimedLife",
            // unit_vitals_host.rs
            "IsPointBlighted",
            "SetBlight",
            "SetBlightLoc",
            // unit_state_event_host.rs
            "TriggerRegisterUnitStateEvent",
            // unit_visibility_host.rs
            "IsUnitHidden",
            "ShowUnit",
            // weather_host.rs
            "AddWeatherEffect",
            "EnableWeatherEffect",
            "RemoveWeatherEffect",
            // widget_enumeration_host.rs
            "EnumDestructablesInRect",
            "EnumItemsInRect",
            "GetEnumDestructable",
            "GetEnumItem",
            "GetFilterDestructable",
            "GetFilterItem",
        ]
    }

    /// Compare the admitted program's declarations against host dispatch.
    pub fn native_coverage(&self) -> NativeCoverage {
        let supported = Self::supported_native_names()
            .into_iter()
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>();
        let mut unsupported = Vec::new();
        for (name, _) in self.inner.program.natives().iter() {
            if !supported.contains(name) {
                unsupported.push(name.clone());
            }
        }
        unsupported.sort();
        unsupported.dedup();
        let supported: Vec<String> = self
            .inner
            .program
            .natives()
            .iter()
            .filter(|(name, _)| supported.contains(*name))
            .map(|(name, _)| name.clone())
            .collect();
        NativeCoverage {
            supported,
            unsupported,
            library_functions: self.inner.program.functions().len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectHost;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    #[test]
    fn supported_names_are_unique_and_nonempty() {
        let names = ObjectHost::supported_native_names();
        assert!(!names.is_empty());
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "duplicate entries: {names:?}");
    }

    /// The crate's own source, read so the audit can be checked against the
    /// dispatch it claims to describe.
    fn source_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
    }

    /// The crate's production sources, by file name. Test modules are left
    /// out: their helpers are not part of the dispatch.
    fn sources() -> BTreeMap<String, String> {
        let dir = source_dir();
        let mut out = BTreeMap::new();
        for entry in std::fs::read_dir(&dir).expect("the crate's source") {
            let path = entry.expect("a source entry").path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            if name.ends_with("_tests.rs") || name.ends_with("tests.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a source file");
            out.insert(name, text);
        }
        out
    }

    /// The body of `fn <name>` in `text`, by brace matching.
    fn fn_body(text: &str, name: &str) -> Option<String> {
        let needle = format!("fn {name}(");
        let mut search = text;
        while let Some(at) = search.find(&needle) {
            let after = &search[at + needle.len()..];
            let open = after.find('{')?;
            let mut depth = 0usize;
            for (index, byte) in after[open..].bytes().enumerate() {
                match byte {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(after[open..open + index + 1].to_string());
                        }
                    }
                    _ => {}
                }
            }
            search = &search[at + needle.len()..];
        }
        None
    }

    /// The host modules `body` hands a name to, in dispatch order.
    fn dispatch_modules(body: &str) -> BTreeSet<String> {
        let mut modules = BTreeSet::new();
        let mut rest = body;
        while let Some(at) = rest.find("self.") {
            rest = &rest[at + "self.".len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let name = &rest[..end];
            if name.ends_with("_native") && rest[end..].starts_with("(name, args)") {
                modules.insert(name.to_string());
            }
            rest = &rest[end..];
        }
        modules
    }

    /// A string literal in `body`: its content, the offset of its opening
    /// quote, and the offset just past its closing quote.
    fn string_literal(body: &str, at: usize) -> Option<(&str, usize, usize)> {
        let bytes = body.as_bytes();
        if bytes.get(at) != Some(&b'"') {
            return None;
        }
        let start = at + 1;
        let mut end = start;
        while end < bytes.len() {
            match bytes[end] {
                b'\\' => end += 2,
                b'"' => return Some((&body[start..end], at, end + 1)),
                _ => end += 1,
            }
        }
        None
    }

    /// Every string literal in `body`, as (content, open quote, past close).
    fn string_literals(body: &str) -> Vec<(&str, usize, usize)> {
        let mut out = Vec::new();
        let mut at = 0usize;
        while at < body.len() {
            let Some(offset) = body[at..].find('"') else {
                break;
            };
            let quote = at + offset;
            match string_literal(body, quote) {
                Some(literal) => {
                    out.push(literal);
                    at = literal.2;
                }
                None => break,
            }
        }
        out
    }

    /// Whether a match arm's `=>` follows `from`.
    ///
    /// The scan ends at a `;`, `{` or `}` at depth zero, so it cannot run past
    /// the arm it is reading into the next statement. A `)` at depth zero ends
    /// it too, so a name inside `matches!(name, "A" | "B")` is not read as an
    /// arm.
    fn arm_follows(body: &str, from: usize) -> bool {
        let bytes = body.as_bytes();
        let mut depth = 0usize;
        let mut index = from;
        while index < bytes.len() {
            match bytes[index] {
                b'"' => match string_literal(body, index) {
                    Some(literal) => index = literal.2,
                    None => return false,
                },
                b'(' | b'[' => depth += 1,
                b')' | b']' => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                b'{' | b'}' => {
                    if depth == 0 {
                        return false;
                    }
                    depth += 1;
                }
                b';' => {
                    if depth == 0 {
                        return false;
                    }
                }
                b'=' if depth == 0 && bytes.get(index + 1) == Some(&b'>') => return true,
                _ => {}
            }
            index += 1;
        }
        false
    }

    /// The index of the `)` closing the `(` at `open`.
    fn closing_paren(body: &str, open: usize) -> Option<usize> {
        let bytes = body.as_bytes();
        let mut depth = 0usize;
        let mut index = open;
        while index < bytes.len() {
            match bytes[index] {
                b'"' => {
                    let literal = string_literal(body, index)?;
                    index = literal.2;
                }
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
            index += 1;
        }
        None
    }

    /// The names `body` dispatches: plain arms, or-arms, tuple arms, `name ==`
    /// comparisons and `matches!(name, …)` lists.
    ///
    /// A literal is a claim only when the dispatch reaches it, so a signature
    /// table row — `("X", &["string"])`, which no arm follows — is not
    /// counted.
    fn dispatched_names(body: &str) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for (content, open, past) in string_literals(body) {
            if content.is_empty() || !content.starts_with(|c: char| c.is_ascii_uppercase()) {
                continue;
            }
            // A constant in the same spelling as a native is not a native:
            // the crate writes its own markers in upper case.
            if content
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
            {
                continue;
            }
            let before = body[..open].trim_end();
            let after = body[past..].trim_start();
            // A plain arm, `"X" => …`.
            if after.starts_with("=>") {
                names.insert(content.to_string());
                continue;
            }
            // An or-arm, `"X" | "Y" => …`.
            if after.starts_with('|') && arm_follows(body, past) {
                names.insert(content.to_string());
                continue;
            }
            // A tuple arm, `("X", …) => …`. The `(` before the literal opens
            // the pattern, and the arm follows the pattern's closing `)`.
            if before.ends_with('(') && after.starts_with(',') {
                let pattern = before.len() - 1;
                if let Some(close) = closing_paren(body, pattern)
                    && arm_follows(body, close + 1)
                {
                    names.insert(content.to_string());
                    continue;
                }
            }
            // A `name == "X"` or `name != "X"` comparison.
            if before.ends_with("name ==") || before.ends_with("name !=") {
                names.insert(content.to_string());
                continue;
            }
            // A `matches!(name, "X" | …)` list.
            if let Some(list) = body[..open].rfind("matches!(") {
                let list_start = list + "matches!(".len();
                if let Some(close) = body[list_start..].find(')') {
                    let list_end = list_start + close;
                    if open > list_start && open < list_end {
                        let items = &body[list_start..list_end];
                        if items.trim_start().starts_with("name,") {
                            names.insert(content.to_string());
                        }
                    }
                }
            }
        }
        names
    }

    /// The host modules the dispatch reaches, following the chain.
    ///
    /// `ObjectHost::native` hands a name to each host module in turn, and a
    /// module may hand it on again: `unit_native` passes the name to
    /// `unit_visibility_native` and the other unit modules. The audit must
    /// cover every module the chain reaches, not only the ones the entry
    /// point names.
    fn dispatch_chain(sources: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        let own =
            fn_body(&sources["object_host.rs"], "native").expect("ObjectHost's native dispatch");
        let mut chain: BTreeMap<String, String> = BTreeMap::new();
        let mut pending: Vec<String> = dispatch_modules(&own).into_iter().collect::<Vec<String>>();
        while let Some(module) = pending.pop() {
            if chain.contains_key(&module) {
                continue;
            }
            let mut body = None;
            for text in sources.values() {
                if let Some(found) = fn_body(text, &module) {
                    body = Some(found);
                    break;
                }
            }
            let body = body.unwrap_or_else(|| panic!("no source defines {module}"));
            for nested in dispatch_modules(&body) {
                if !chain.contains_key(&nested) {
                    pending.push(nested);
                }
            }
            chain.insert(module, body);
        }
        chain
    }

    /// The audit must name every native the host dispatches.
    ///
    /// The check reads the crate's own source, because the dispatch is not
    /// described in one place: `ObjectHost` hands a name to each host module
    /// in turn, and each module claims the names it dispatches. A dispatch
    /// name that the audit does not list is reported as unsupported, which is
    /// the wrong answer, so this fails when one appears.
    ///
    /// The check follows the whole chain, so a module the entry point reaches
    /// through another module is covered too. It sees the shapes a dispatch
    /// uses — a plain arm, an or-arm, a tuple arm, a `name ==` comparison and
    /// a `matches!` list. A name that only appears in a module's contract
    /// table, in a helper's name list or through the `RealOps` trait is not
    /// seen here; those are in the list and the re-note records them.
    #[test]
    fn the_audit_lists_every_native_the_host_dispatches() {
        let sources = sources();
        let own =
            fn_body(&sources["object_host.rs"], "native").expect("ObjectHost's native dispatch");
        let mut dispatched = dispatched_names(&own);
        let chain = dispatch_chain(&sources);
        assert!(!chain.is_empty(), "the dispatch chain must be readable");
        for body in chain.values() {
            dispatched.extend(dispatched_names(body));
        }
        let audited: BTreeSet<String> = ObjectHost::supported_native_names()
            .into_iter()
            .map(str::to_string)
            .collect();
        // The check must see the dispatch, or it proves nothing.
        assert!(
            dispatched.len() > 150,
            "the check read only {} dispatched names; it is not reading the dispatch",
            dispatched.len()
        );
        let missing: Vec<&String> = dispatched.difference(&audited).collect();
        assert!(
            missing.is_empty(),
            "the audit does not list {} dispatched native(s): {missing:?}",
            missing.len()
        );
    }

    /// The check must read every shape the dispatch uses.
    ///
    /// The first version of the check scanned forward from a literal's opening
    /// quote for the first character that was not alphanumeric, which stopped
    /// at the closing quote. The text after the literal therefore still began
    /// with a quote, and the plain-arm and tuple-arm shapes never matched:
    /// only the `matches!` list and the comparison were read, and the check
    /// saw 152 of the 233 names the dispatch claims. These cases pin each
    /// shape, so that reading cannot silently fall back.
    #[test]
    fn the_check_reads_every_dispatch_shape() {
        let body = r#"
            match name {
                "PlainArm" => 1,
                "OrArm" | "OrArmSecond" => 2,
                ("TupleArm", [Value::Int(n)]) if *n >= 0 => 3,
                _ => 4,
            }
            if name == "Compared" {
                5
            }
            if matches!(name, "Listed" | "ListedSecond") {
                6
            }
        "#;
        let names = dispatched_names(body);
        for expected in [
            "PlainArm",
            "OrArm",
            "OrArmSecond",
            "TupleArm",
            "Compared",
            "Listed",
            "ListedSecond",
        ] {
            assert!(
                names.contains(expected),
                "the check missed {expected}: {names:?}"
            );
        }
    }

    /// A signature table row is not a dispatch.
    ///
    /// A contract table names a native and its parameter types, and no arm
    /// follows it. Reading it as a dispatch would put a name in the audit that
    /// the host only declares.
    #[test]
    fn a_signature_table_row_is_not_a_dispatch() {
        let body = r#"const CONTRACTS: &[(&str, &[&str])] = &[("Tabled", &["string"])];"#;
        assert!(
            dispatched_names(body).is_empty(),
            "a contract row was read as a dispatch"
        );
    }

    /// The check must reach a module the entry point reaches through another
    /// module.
    ///
    /// `ObjectHost::native` names the host modules it calls directly.
    /// `unit_native` passes the name on to the visibility, look-at, pause,
    /// revive and sleep modules, which the entry point never names. A check
    /// that stopped at the entry point would not read them, and the natives
    /// they dispatch would be reported as unsupported.
    #[test]
    fn the_check_follows_the_whole_dispatch_chain() {
        let sources = sources();
        let chain = dispatch_chain(&sources);
        for nested in [
            "unit_visibility_native",
            "unit_look_at_native",
            "unit_pause_native",
            "unit_waygate_native",
            "unit_user_data_native",
            "unit_revive_native",
            "unit_sleep_native",
        ] {
            assert!(
                chain.contains_key(nested),
                "the chain does not reach {nested}"
            );
        }
    }
}
