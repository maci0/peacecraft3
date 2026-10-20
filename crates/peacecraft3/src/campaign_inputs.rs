//! Prepare the existing startup factory from the admitted source reader.
use crate::{campaign_assets, map_requests::MapBindings};
use pcx_assets::Resolver;
use pcx_objects::editor::{
    EditorReadLimits, SlkId,
    defaults::{DefaultTables, Family, Identity},
};
use pcx_play::{map_admission::ParsedMapContext, map_startup_host::*, scripts::AssetIdentity};
use std::sync::Arc;

/// Zoom index the application camera also selects for its near clip.
const GAME_CAMERA_ZOOM: usize = 0;

/// Fresh single-player policy: the sole authored human slot is the local client.
/// Explicit seed and selected edition are game inputs, not map-derived defaults.
pub fn prepare_new_game(
    context: &Arc<ParsedMapContext>,
    bindings: &MapBindings,
    reader: &Resolver,
    image: &[u8],
    client: AssetIdentity,
    seed: i32,
    expansion: bool,
) -> Result<MapStartupInputs, String> {
    let mut humans = context.world.info.human_players();
    let local = humans
        .next()
        .ok_or("fresh campaign has no authored human slot")?;
    if humans.next().is_some() {
        return Err("fresh campaign requires an explicit choice among multiple human slots".into());
    }
    let local = i32::try_from(local.number()).map_err(|_| "local player number")?;
    let mut inputs = prepare(context, bindings, reader, image, client.clone())?;
    let settings = pcx_play::campaign_client::fresh_settings(image, client.clone())?;
    let placement_rules = pcx_play::campaign_client::placement_rules(image, client.clone())?;
    let triggers = pcx_play::campaign_client::fresh_triggers(image, client.clone())?;
    let orders = pcx_play::campaign_client::order_registry(image, client.clone())?;
    let activity = pcx_play::campaign_client::unit_activity(image, client.clone(), &orders)?;
    inputs.budgets.unit_in_range = Some(65536);
    inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
        initial_enabled: triggers.initial_enabled,
        timer_event_global: conversion_global(
            context.scripts.program(),
            "ConvertGameEvent",
            triggers.timer_event,
        )?,
        max_timed_events: 65536,
        max_parked_threads: 65536,
    });
    let map = pcx_map::Map::parse(&bindings.catalog_map_bytes).map_err(|e| e.to_string())?;
    let edits = map
        .extract_checked(
            &bindings.map_label,
            "war3map.w3q",
            pcx_mpq::checked::ReadBudget {
                max_archive_bytes: 64 << 20,
                max_archive_table_bytes: 8 << 20,
                max_raw_packed_bytes: 4 << 20,
                max_decoded_bytes: 4 << 20,
            },
        )
        .map_err(|e| e.to_string())?;
    let stock_misc = reader
        .resolve_bounded("Units/MiscData.txt", 4 << 20)
        .map_err(|e| e.to_string())?;
    let (stock_label, _) =
        campaign_assets::identity(&stock_misc, &context.sources.map.edition, 16 << 10)?;
    let map_misc = map
        .extract_checked(
            &bindings.map_label,
            "war3mapMisc.txt",
            pcx_mpq::checked::ReadBudget {
                max_archive_bytes: 64 << 20,
                max_archive_table_bytes: 8 << 20,
                max_raw_packed_bytes: 4 << 20,
                max_decoded_bytes: 4 << 20,
            },
        )
        .map_err(|e| e.to_string())?;
    let mut day_profiles = vec![(stock_label.as_str(), stock_misc.bytes.as_slice())];
    if let Some(bytes) = &map_misc {
        day_profiles.push((bindings.map_label.as_str(), bytes.as_slice()));
    }
    inputs.resources = Some(pcx_play::player_resources::Rules::discover(
        image,
        client.clone(),
        &day_profiles,
        pcx_play::neutral_player::RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 8 << 20,
            sections: 96,
        },
    )?);
    let day_cycle = pcx_objects::editor::misc::DayCycle::read(&day_profiles, 8 << 20)?;
    let game_misc = reader
        .resolve_bounded("Units/MiscGame.txt", 4 << 20)
        .map_err(|e| e.to_string())?;
    let (game_label, _) =
        campaign_assets::identity(&game_misc, &context.sources.map.edition, 16 << 10)?;
    let mut game_profiles = vec![(game_label.as_str(), game_misc.bytes.as_slice())];
    if let Some(bytes) = &map_misc {
        game_profiles.push((bindings.map_label.as_str(), bytes.as_slice()));
    }
    let hero_vitals = pcx_objects::editor::misc::HeroVitals::read(&game_profiles, 8 << 20)?;
    let combat = pcx_objects::editor::misc::CombatRules::read(&game_profiles, 8 << 20)?;
    let camera_config = reader
        .resolve_bounded("UI/MiscData.txt", 2 << 20)
        .map_err(|e| e.to_string())?;
    let game_camera = pcx_objects::editor::camera::game_camera(
        &camera_config.bytes,
        "UI/MiscData.txt",
        GAME_CAMERA_ZOOM,
        2 << 20,
    )?;
    let upgrades = edits
        .as_ref()
        .map(|bytes| {
            pcx_objects::ObjectFile::parse("war3map.w3q", bytes)
                .map(|file| pcx_objects::editor::Overlay::of(&file))
                .map_err(|e| e.to_string())
        })
        .transpose()?
        .unwrap_or_default();
    let research = pcx_play::upgrade_research::Rules::read(
        image,
        client,
        &inputs.defend.tables,
        &upgrades,
        inputs.catalog.as_ref(),
        pcx_play::upgrade_research::Budget {
            upgrades: 10000,
            modifications_per_upgrade: 4096,
        },
    )?;
    let participants = context
        .world
        .info
        .players()
        .iter()
        .filter(|p| {
            i32::try_from(p.number()).ok() == Some(local)
                || matches!(
                    p.control(),
                    pcx_map::info::CONTROLLER_COMPUTER
                        | pcx_map::info::CONTROLLER_COMPUTER_EASY
                        | pcx_map::info::CONTROLLER_COMPUTER_INSANE
                )
        })
        .map(|p| i32::try_from(p.number()).map_err(|_| "participant player number".to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    inputs.post_config = PostConfigObservations {
        new_game: Some(NewGameInput {
            seed,
            orders: Some(Arc::new(orders)),
            activity: Some(activity),
            research: Some(research),
            placement_rules: Some(placement_rules),
            unit_range_event: Some("EVENT_UNIT_TARGET_IN_RANGE".into()),
            day_cycle: Some(day_cycle),
            game_camera: Some(game_camera),
            hero_vitals: Some(hero_vitals),
            combat: Some(combat),
        }),
        local_player: Some(local),
        participants: Some(participants),
        settings: Some(SettingsObservation {
            speed_global: conversion_global(
                context.scripts.program(),
                "ConvertGameSpeed",
                settings.speed,
            )?,
            fog_enabled: settings.fog_enabled,
            fog_mask_enabled: settings.fog_mask_enabled,
        }),
        // ponytail: Normal until the campaign menu offers the difficulty choice.
        difficulty_global: Some("MAP_DIFFICULTY_NORMAL".into()),
        version_global: Some(
            if expansion {
                "VERSION_FROZEN_THRONE"
            } else {
                "VERSION_REIGN_OF_CHAOS"
            }
            .into(),
        ),
        ..Default::default()
    };
    Ok(inputs)
}

fn conversion_global(
    program: &pcx_script::Program,
    conversion: &str,
    value: i32,
) -> Result<String, String> {
    use pcx_script::ast::{Expr, Literal};
    let native = program
        .natives()
        .get(conversion)
        .ok_or_else(|| format!("missing {conversion}"))?;
    let mut matches = program.globals().iter().filter(|global| {
        if !global.is_constant || global.array || global.source != native.source || global.ty.kind != native.ret.kind { return false; }
        let Some(Expr::Call { callee, args, .. }) = &global.init else { return false; };
        matches!((callee.as_ref(), args.as_slice()), (Expr::Ident(name, _), [Expr::Literal(Literal::Int(literal), _)]) if name.as_str()==conversion && literal.value == i64::from(value))
    });
    let name = matches
        .next()
        .ok_or_else(|| format!("missing source {conversion} global"))?
        .name
        .clone();
    if matches.next().is_some() {
        return Err(format!("ambiguous source {conversion} global"));
    }
    Ok(name)
}

pub fn prepare(
    context: &Arc<ParsedMapContext>,
    bindings: &MapBindings,
    reader: &Resolver,
    image: &[u8],
    client: AssetIdentity,
) -> Result<MapStartupInputs, String> {
    if context.sources.catalog != bindings.catalog_identity
        || context.map_digest.hash != pcx_mpq::fnv1a64(&bindings.catalog_map_bytes)
    {
        return Err("campaign preparation inputs differ from the admitted map/catalog".into());
    }
    let edition = &context.sources.map.edition;
    let tables = defaults(reader, &context.sources.catalog)?;
    let defend = defend_alias(&tables)?;
    let map = pcx_map::Map::parse(&bindings.catalog_map_bytes).map_err(|e| e.to_string())?;
    let budget = pcx_mpq::checked::ReadBudget {
        max_archive_bytes: 64 << 20,
        max_archive_table_bytes: 8 << 20,
        max_raw_packed_bytes: 4 << 20,
        max_decoded_bytes: 4 << 20,
    };
    let map_misc = map
        .extract_checked(&bindings.map_label, "war3mapMisc.txt", budget)
        .map_err(|e| e.to_string())?;
    let stock_misc = reader
        .resolve_bounded("Units/MiscGame.txt", 4 << 20)
        .map_err(|e| e.to_string())?;
    let (stock_label, _) = campaign_assets::identity(&stock_misc, edition, 16 << 10)?;
    let mut progression = vec![(stock_label.as_str(), stock_misc.bytes.as_slice())];
    if let Some(bytes) = &map_misc {
        progression.push((bindings.map_label.as_str(), bytes.as_slice()));
    }
    let progression = pcx_objects::editor::hero::Progression::read(&progression, 8 << 20)?;
    let ability_bytes = map
        .extract_checked(&bindings.map_label, "war3map.w3a", budget)
        .map_err(|e| e.to_string())?;
    let abilities = match ability_bytes {
        Some(bytes) => pcx_objects::editor::Overlay::of(
            &pcx_objects::ObjectFile::parse("war3map.w3a", &bytes).map_err(|e| e.to_string())?,
        ),
        None => pcx_objects::editor::Overlay::default(),
    };
    let camera_config = reader
        .resolve_bounded("UI/MiscData.txt", 2 << 20)
        .map_err(|e| e.to_string())?;
    let (_, camera_origin) = campaign_assets::identity(&camera_config, edition, 16 << 10)?;
    let source = pcx_play::campaign_client::read(
        image,
        client.clone(),
        &camera_config.bytes,
        camera_origin,
        progression,
    )?;
    let rules = NeutralRuleSet::discover_first_family(
        image,
        client.clone(),
        pcx_play::neutral_player::DiscoveryBudget {
            image: pcx_play::neutral_player::RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
            registration_candidates: 100000,
            work_bytes: 256 << 20,
        },
    )
    .map_err(|e| e.to_string())?;
    let sound_budget = pcx_play::sound_parameters::BanksBudget {
        banks: 64,
        table_bytes: 4 << 20,
        total_table_bytes: 64 << 20,
        coordinates: pcx_slq::CoordinateLimits {
            max_row: 10000,
            max_column: 256,
        },
        definitions: 4096,
        metadata_bytes: 1 << 20,
    };
    let sound_sources = campaign_assets::SoundSources::read(reader, edition, sound_budget)?;
    let label_budget = pcx_play::label_sounds::Budget {
        table_bytes: 4 << 20,
        coordinates: sound_budget.coordinates,
        sounds: 128,
        resource_bytes: 64 << 20,
        total_resource_bytes: 128 << 20,
        metadata_bytes: 1 << 20,
    };
    let labels = sound_sources.empty_label_bank(label_budget)?;
    let constructors = vec![sound_sources.empty_label_bank(label_budget)?];
    Ok(MapStartupInputs {
        enumeration: enum_contracts(context)?,
        rules,
        budgets: budgets(),
        camera: source.camera,
        // Executed-path loaders attached by campaign_startup fill these stores.
        day_night: pcx_play::day_night::Assets::read(
            &[],
            pcx_play::day_night::Budget {
                models: 32,
                model_bytes: 16 << 20,
                total_bytes: 64 << 20,
                path_bytes: 16 << 10,
            },
        )
        .map_err(|e| e.to_string())?,
        label_sounds: labels,
        label_sound_creation: constructors,
        file_sounds: pcx_play::file_sounds::Assets::read(
            &[],
            pcx_play::file_sounds::Budget {
                resources: 256,
                resource_bytes: 64 << 20,
                total_bytes: 128 << 20,
                metadata_bytes: 1 << 20,
            },
        )
        .map_err(|e| e.to_string())?,
        sound_parameters: pcx_play::sound_parameters::Banks::read(&[], sound_budget)
            .map_err(|e| e.to_string())?,
        sound_playback: Some(sound_playback::Rules::reviewed_legacy(
            image,
            client.clone(),
        )?),
        resources: None,
        dialog_layout: Some(pcx_play::dialog_layout::Rules::reviewed_legacy(
            image,
            client.clone(),
        )?),
        clock: StartupClockInput {
            clock: source.clock,
            global_name: "GAME_STATE_TIME_OF_DAY".into(),
            max_globals: 16384,
            expression_depth: 64,
            expression_work: 1 << 20,
        },
        // Recreation elapsed-time origin; this is not a source time-of-day value.
        timer_time: Some(0.0),
        trigger_runtime: None,
        random: Some(RandomInput {
            policy: Arc::new(source.random),
            max_secondary: 256,
        }),
        numeric: Some(source.numeric),
        experience_handicap: Some(source.experience),
        hero_skills: Some(HeroSkillsInput {
            rules: source.hero,
            abilities,
            budget: pcx_play::playercolor_host::HeroBudget {
                abilities_per_unit: 128,
                overlay_objects: 4096,
                modifications_per_ability: 4096,
            },
        }),
        terrain_pathing: None,
        catalog: Arc::clone(&bindings.catalog.0),
        defend: DefendSelection {
            tables,
            alias: defend,
        },
        current_research: None,
        // Authored controller roles alone do not establish these runtime inputs.
        post_config: PostConfigObservations::default(),
    })
}

fn enum_contracts(context: &ParsedMapContext) -> Result<EnumContracts, String> {
    use pcx_script::ast::Expr;
    let program = context.scripts.program();
    let mut names = std::collections::BTreeSet::new();
    // Select the conversion calls authored in the common constant declarations;
    // checked_selected still validates source, signature, ancestry and domains.
    for global in program.globals() {
        let Some(Expr::Call { callee, .. }) = &global.init else {
            continue;
        };
        let Expr::Ident(name, _) = callee.as_ref() else {
            continue;
        };
        let Some(native) = program.natives().get(name.as_str()) else {
            continue;
        };
        if global.is_constant
            && !global.array
            && name.as_str().starts_with("Convert")
            && global.source == native.source
            && global.ty.kind == native.ret.kind
        {
            names.insert(name.as_str().to_owned());
        }
    }
    if names.is_empty() {
        return Err("selected common source has no constant conversion contracts".into());
    }
    Ok(EnumContracts {
        context: format!("campaign source {:?}", context.sources.common),
        names: names.into_iter().collect(),
        limits: pcx_play::playercolor_host::Limits {
            max_handles: 100000,
            admission_steps: 1 << 20,
            expression_depth: 64,
            expression_work: 4 << 20,
        },
    })
}

fn defaults(reader: &Resolver, origin: &AssetIdentity) -> Result<DefaultTables, String> {
    let mut tables = Vec::new();
    let mut profiles = Vec::new();
    let mut remaining = 64usize << 20;
    for id in SlkId::ALL {
        let asset = reader
            .resolve_bounded(id.path(), remaining.min(4 << 20) as u64)
            .map_err(|e| e.to_string())?;
        remaining = remaining
            .checked_sub(asset.bytes.len())
            .ok_or("stock table byte budget exceeded")?;
        tables.push((id.path().to_owned(), asset));
    }
    for (visited, path) in reader.paths().enumerate() {
        if visited >= 100000 {
            return Err("stock profile inventory work budget exceeded".into());
        }
        let Some(leaf) = path.as_str().strip_prefix("units/") else {
            continue;
        };
        if leaf.contains('/')
            || ![
                "unitfunc.txt",
                "unitstrings.txt",
                "abilityfunc.txt",
                "abilitystrings.txt",
                "upgradefunc.txt",
                "upgradestrings.txt",
                "itemfunc.txt",
                "itemstrings.txt",
            ]
            .iter()
            .any(|suffix| leaf.ends_with(suffix))
        {
            continue;
        }
        if profiles.len() >= 256 {
            return Err("stock profile count budget exceeded".into());
        }
        let asset = reader
            .resolve_bounded(path.as_str(), remaining.min(4 << 20) as u64)
            .map_err(|e| e.to_string())?;
        remaining = remaining
            .checked_sub(asset.bytes.len())
            .ok_or("stock profile byte budget exceeded")?;
        profiles.push((path.as_str().to_owned(), asset));
    }
    let tables = tables
        .iter()
        .map(|(n, a)| (n.as_str(), a.bytes.as_slice()))
        .collect::<Vec<_>>();
    let profiles = profiles
        .iter()
        .map(|(n, a)| (n.as_str(), a.bytes.as_slice()))
        .collect::<Vec<_>>();
    DefaultTables::from_bytes(
        Identity {
            install: origin.source.clone(),
            edition_version: format!("{}; {}", origin.edition, origin.version),
        },
        &tables,
        &profiles,
        EditorReadLimits {
            max_listed_paths: 512,
            max_asset_bytes: 4 << 20,
            max_aggregate_input_bytes: 128 << 20,
        },
    )
    .map_err(|e| e.to_string())
}

fn defend_alias(tables: &DefaultTables) -> Result<String, String> {
    let mut matches = Vec::new();
    for alias in tables.rawcodes(Family::Ability) {
        let object = tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?;
        let on = object.profile("Order");
        let off = object.profile("Unorder");
        if matches!(on.as_slice(),[c] if c.parse_error.is_none() && c.text=="defend")
            && matches!(off.as_slice(),[c] if c.parse_error.is_none() && c.text=="undefend")
        {
            matches.push(alias.to_owned());
        }
    }
    match matches.as_slice() {
        [alias] => Ok(alias.clone()),
        _ => Err("selected sources must identify exactly one supported Defend order pair".into()),
    }
}

fn budgets() -> MapStartupBudgets {
    use pcx_play::playercolor_host::*;
    MapStartupBudgets {
        groups: 10000,
        map_config: MapConfigBudget {
            text_bytes: 1 << 20,
            start_locations: 10000,
        },
        players: PlayerBudget {
            max_slots: 10000,
            name_bytes: 1 << 20,
        },
        filters: FilterBudget { max_records: 4096 },
        conditions: Some(ConditionBudget { max_records: 4096 }),
        forces: ForceBudget {
            max_records: 4096,
            max_memberships: 65536,
        },
        state_requests: PlayerStateRequestBudget { max_records: 4096 },
        alliances: PlayerAllianceBudget { max_records: 65536 },
        start_priorities: 65536,
        sound_starts: 4096,
        map_music_bytes: 16 << 10,
        file_sound_eax_bytes: 16 << 10,
        stacked_sounds: 4096,
        camera_setup_fields: 4096,
        tech_limits: TechLimitBudget { max_records: 65536 },
        tech_researched: TechLimitBudget { max_records: 65536 },
        destructables: DestructableBudget { max_records: 10000 },
        units: UnitBudget {
            max_records: 10000,
            max_retained_fields: 4096,
            max_modifications: 4096,
        },
        unit_states: UnitStateBudget { max_records: 65536 },
        triggers: TriggerBudget {
            max_triggers: 10000,
            max_unit_events: 65536,
            max_actions: 65536,
        },
        trigger_conditions: Some(65536),
        player_unit_events: Some(65536),
        death_events: Some(65536),
        unit_in_range: None,
        player_events: Some(65536),
        regions: Some(RegionBudget {
            max_regions: 10000,
            max_rect_additions: 65536,
            max_events: 65536,
        }),
        timer_events: 65536,
        game_state_events: 65536,
        stock: Some(StockBudget {
            max_unit_records: 65536,
        }),
        dialogs: Some(DialogBudget {
            max_dialogs: 4096,
            max_buttons: 16384,
            text_bytes: 4 << 20,
            max_events: 65536,
            max_quit_requests: 4096,
        }),
        quests: Some(QuestBudget {
            max_quests: 4096,
            max_items: 16384,
            text_bytes: 4 << 20,
            max_events: 65536,
        }),
        items: Some(ItemBudget {
            max_records: 10000,
            max_retained_fields: 4096,
            max_modifications: 4096,
        }),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn conversion_names_come_from_selected_constants_and_refuse_ambiguity() {
        let common = |extra: &str| {
            pcx_script::parse_script(&format!("type gamespeed extends handle\ntype gameevent extends handle\nconstant native ConvertGameSpeed takes integer n returns gamespeed\nconstant native ConvertGameEvent takes integer n returns gameevent\nglobals\nconstant gamespeed Authored = ConvertGameSpeed(7)\nconstant gameevent AuthoredTimer = ConvertGameEvent(19)\n{extra}\nendglobals\n")).unwrap()
        };
        let empty = pcx_script::parse_script("").unwrap();
        let selected = common("");
        let program = pcx_script::resolve_with(
            &empty,
            &[pcx_script::Prelude::new("selected common", &selected)],
        )
        .unwrap();
        assert_eq!(
            super::conversion_global(&program, "ConvertGameSpeed", 7).unwrap(),
            "Authored"
        );
        assert_eq!(
            super::conversion_global(&program, "ConvertGameEvent", 19).unwrap(),
            "AuthoredTimer"
        );
        assert!(super::conversion_global(&program, "ConvertGameSpeed", 2).is_err());
        let duplicate = common("constant gamespeed Alias = ConvertGameSpeed(7)");
        let program = pcx_script::resolve_with(
            &empty,
            &[pcx_script::Prelude::new("selected common", &duplicate)],
        )
        .unwrap();
        assert!(super::conversion_global(&program, "ConvertGameSpeed", 7).is_err());
        let duplicate = common("constant gameevent AnotherTimer = ConvertGameEvent(19)");
        let program = pcx_script::resolve_with(
            &empty,
            &[pcx_script::Prelude::new("selected common", &duplicate)],
        )
        .unwrap();
        assert!(super::conversion_global(&program, "ConvertGameEvent", 19).is_err());
    }
}
