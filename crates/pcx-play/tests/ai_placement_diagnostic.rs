//! Human05 placement diagnostic. Requires the selected local install and map.
#![cfg(feature = "test-fixtures")]

use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode, Resolver};
use pcx_play::ai_placement::{
    BuffType, ExclusionContext, ExclusionError, ExclusionVisibility, Grid, KinematicWindow,
    MovementEvidence, Rules, beyond_cast_reach, exclude_unit,
};
use pcx_play::headless_startup::start;
use pcx_script::{Host, Value};
use pcx_sim::world::{Definition, FieldValue};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn text<'a>(definition: &'a Definition, column: &str) -> &'a str {
    let (_, field) = definition.column_field(column).unwrap();
    match field.value.as_ref().unwrap() {
        FieldValue::Cell(value) => value,
        FieldValue::Override(pcx_objects::Value::Str(value)) => std::str::from_utf8(value).unwrap(),
        value => panic!("{column}: {value:?}"),
    }
}

fn dimensions(resolver: &Resolver, path: &str) -> [u32; 2] {
    let asset = resolver.resolve_bounded(path, 1 << 20).unwrap();
    let header = pcx_tga::Header::parse(&asset.bytes).unwrap();
    [u32::from(header.width), u32::from(header.height)]
}

fn building_facing(angle: f32) -> f32 {
    assert!(angle.is_finite() && angle > i32::MIN as f32 && angle < i32::MAX as f32);
    let angle = (angle as i32) % 360;
    ((angle / 90 * 90).rem_euclid(360)) as f32
}

fn probe_point(value: &str) -> [f32; 2] {
    let (x, y) = value.split_once(',').expect("probe coordinates");
    [
        x.trim().parse().expect("probe x"),
        y.trim().parse().expect("probe y"),
    ]
}

fn scripted_spawns(script: &str, kind: &str) -> Vec<[f32; 2]> {
    let marker = format!("CreateUnit( p, '{kind}', ");
    script
        .lines()
        .filter_map(|line| {
            let coordinates = line.split_once(&marker)?.1;
            let (x, tail) = coordinates.split_once(',')?;
            let (y, _) = tail.split_once(',')?;
            Some([
                x.trim().parse().expect("spawn x"),
                y.trim().parse().expect("spawn y"),
            ])
        })
        .collect()
}

fn aliases_using_buffs(
    abilities: &pcx_slq::Table,
    buffs: &[&str],
) -> Result<BTreeSet<String>, String> {
    let columns: Vec<_> = abilities
        .headers()
        .filter_map(|(column, name)| name.starts_with("BuffID").then_some(column))
        .collect();
    if columns.is_empty() {
        return Err("ability buff columns missing".into());
    }
    let mut aliases = BTreeSet::new();
    for row in abilities.data_rows() {
        if columns
            .iter()
            .filter_map(|&column| row.get(column))
            .any(|value| value.split(',').any(|id| buffs.contains(&id.trim())))
        {
            aliases.insert(
                row.by_name("alias")
                    .ok_or("ability alias missing")?
                    .to_owned(),
            );
        }
    }
    Ok(aliases)
}

fn override_mentions(
    record: &pcx_objects::Record,
    matches: impl Fn(&str) -> bool,
) -> Result<bool, String> {
    for entry in &record.mods {
        if let pcx_objects::Value::Str(value) = &entry.value {
            let value = std::str::from_utf8(value).map_err(|_| "non-UTF8 object override")?;
            if value.split(',').any(|token| matches(token.trim())) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn counter_source_scan(
    resolver: &Resolver,
    script: &str,
    map: &pcx_map::Map<'_>,
    label: &str,
    budget: pcx_mpq::checked::ReadBudget,
) -> Result<(), String> {
    let read = |name| -> Result<pcx_slq::Table, String> {
        let asset = resolver
            .resolve_bounded(name, 8 << 20)
            .map_err(|error| error.to_string())?;
        pcx_slq::Table::parse(&asset.bytes).map_err(|error| error.to_string())
    };
    let abilities = read("Units/AbilityData.slk")?;
    let buffs = ["Bprg", "Bchd", "BEer", "Bena", "Beng"];
    let mut aliases = aliases_using_buffs(&abilities, &buffs)?;
    aliases.extend(["Achd".to_owned(), "Aloa".to_owned()]);
    let extract = |name| -> Result<Option<Vec<u8>>, String> {
        map.extract_checked(label, name, budget)
            .map_err(|error| error.to_string())
    };
    if let Some(bytes) = extract("war3map.w3a")? {
        let overlay = pcx_objects::ObjectFile::parse("war3map.w3a", &bytes)
            .map_err(|error| error.to_string())?;
        for record in overlay.records() {
            let original = pcx_objects::fourcc_text(record.original);
            let affected = aliases.contains(&original)
                || override_mentions(record, |token| buffs.contains(&token))?;
            if affected {
                aliases.insert(if record.modified == [0; 4] {
                    original
                } else {
                    pcx_objects::fourcc_text(record.modified)
                });
            }
        }
    }
    for alias in &aliases {
        if script.contains(&format!("'{alias}'")) {
            return Err(format!(
                "map script directly references counter effect ability {alias}"
            ));
        }
    }
    let mut carriers = BTreeSet::new();
    for (path, id_column, ability_columns, overlay_name) in [
        (
            "Units/UnitAbilities.slk",
            "unitAbilID",
            &["abilList", "heroAbilList"][..],
            "war3map.w3u",
        ),
        (
            "Units/ItemData.slk",
            "itemID",
            &["abilList"][..],
            "war3map.w3t",
        ),
    ] {
        let table = read(path)?;
        for row in table.data_rows() {
            let Some(id) = row.by_name(id_column) else {
                continue;
            };
            let found: BTreeSet<_> = ability_columns
                .iter()
                .filter_map(|name| row.by_name(name))
                .flat_map(|value| value.split(',').map(str::trim))
                .filter(|alias| aliases.contains(*alias))
                .collect();
            if !found.is_empty() {
                carriers.insert(id.to_owned());
                if script.contains(&format!("'{id}'"))
                    && !(id == "nmrr" && found == BTreeSet::from(["ACen"]))
                {
                    return Err(format!(
                        "map script can create counter-effect carrier {id}: {found:?}"
                    ));
                }
            }
        }
        if let Some(bytes) = extract(overlay_name)? {
            let overlay = pcx_objects::ObjectFile::parse(overlay_name, &bytes)
                .map_err(|error| error.to_string())?;
            for record in overlay.records() {
                let original = pcx_objects::fourcc_text(record.original);
                let id = if record.modified == [0; 4] {
                    original.clone()
                } else {
                    pcx_objects::fourcc_text(record.modified)
                };
                let modified = override_mentions(record, |token| aliases.contains(token))?;
                if script.contains(&format!("'{id}'"))
                    && (modified || (id != original && carriers.contains(&original)))
                {
                    return Err(format!(
                        "map object {id} has an unresolved counter-effect override"
                    ));
                }
            }
        }
    }
    if script.contains("ChooseRandomItem(")
        || script.contains("ChooseRandomCreep(")
        || script.contains("UnitAddAbility(")
    {
        return Err("map script can choose an unenumerated counter-effect carrier".into());
    }
    for line in script.lines() {
        if [
            "CreateUnit(",
            "CreateNUnitsAtLoc(",
            "CreateItem(",
            "CreateItemLoc(",
            "RandomDistAddItem(",
            "UnitAddItemToSlotById(",
        ]
        .iter()
        .any(|name| line.contains(name))
            && !line.contains('\'')
        {
            return Err("map script creates a unit or item without a literal rawcode".into());
        }
    }
    println!(
        "selected counter-effect ability and carrier scan passed; item drops and object overrides included"
    );
    Ok(())
}

fn probe_order_worker(log: &str, kind: [u8; 4], owner: i32) -> [f32; 2] {
    let marker = format!("/id{}/p{owner}@", i32::from_be_bytes(kind));
    let event = log.split_once(&marker).expect("probe order").1;
    probe_point(event.split_once("/point").expect("probe point order").0)
}

fn probe_order_elapsed(log: &str, kind: [u8; 4], owner: i32) -> f64 {
    let marker = format!("/id{}/p{owner}@", i32::from_be_bytes(kind));
    let prefix = log.split_once(&marker).expect("probe order").0;
    prefix
        .rsplit_once(" E:t")
        .expect("probe time")
        .1
        .parse()
        .expect("elapsed time")
}

fn probe_clock_delay(script: &str) -> f64 {
    script
        .lines()
        .find_map(|line| {
            let args = line
                .trim()
                .strip_prefix("call TimerStart(CreateTimer(), ")?;
            let value = args.strip_suffix(", false, function PCXO_Start)")?;
            Some(value.parse().expect("probe clock delay"))
        })
        .expect("probe clock start")
}

fn probe_order_site(log: &str, kind: [u8; 4], owner: i32) -> Option<[f32; 2]> {
    let marker = format!("/id{}/p{owner}@", i32::from_be_bytes(kind));
    let event = log.split_once(&marker)?.1;
    let point = event.split_once("/point")?.1.split_once("/target")?.0;
    Some(probe_point(point))
}

fn probe_start(log: &str) -> Option<[f32; 2]> {
    let start = log.split_once("start index=")?.1;
    let point = start.split_once('@')?.1;
    Some(probe_point(point.split_once('"')?.0))
}

fn sample_index(path: &std::path::Path) -> Option<usize> {
    if path.extension()? != "txt" {
        return None;
    }
    path.file_stem()?
        .to_str()?
        .strip_prefix("PCXO-")?
        .parse()
        .ok()
}

fn probe_sample_workers(log: &str) -> Vec<[f32; 2]> {
    log.split(" A:")
        .skip(1)
        .map(|worker| probe_point(worker.split_once("/o").expect("probe worker order").0))
        .collect()
}

fn probe_spawn(script: &str, map_script: &str) -> ([f32; 2], f32, usize) {
    let line = script
        .lines()
        .find(|line| line.contains("CreateNUnitsAtLoc(1, 'uaco', Player(5), "))
        .expect("probe spawn");
    let point = if let Some(location) = line.split_once("Location(") {
        probe_point(location.1.split_once(')').unwrap().0)
    } else {
        let rect = map_script
            .lines()
            .find(|line| line.contains("set gg_rct_Acolyte_Appear = Rect("))
            .expect("probe spawn rectangle");
        let values: Vec<f32> = rect
            .split_once("Rect(")
            .unwrap()
            .1
            .split_once(')')
            .unwrap()
            .0
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect();
        [(values[0] + values[2]) / 2., (values[1] + values[3]) / 2.]
    };
    let facing = line
        .rsplit_once(',')
        .unwrap()
        .1
        .trim()
        .trim_end_matches(')')
        .parse()
        .unwrap();
    let count = script
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("exitwhen i >= ")
                .map(|value| value.parse().unwrap())
        })
        .expect("probe worker count");
    (point, facing, count)
}

#[test]
fn probe_spawn_reads_location_or_rectangle_from_source() {
    let location =
        "exitwhen i >= 3\ncall CreateNUnitsAtLoc(1, 'uaco', Player(5), Location(12., 24.), 70.)";
    assert_eq!(probe_spawn(location, ""), ([12., 24.], 70., 3));
    let region = "exitwhen i >= 2\ncall CreateNUnitsAtLoc(1, 'uaco', Player(5), GetRectCenter(gg_rct_Acolyte_Appear), 90.)";
    let map = "set gg_rct_Acolyte_Appear = Rect( 10., 20., 30., 40. )";
    assert_eq!(probe_spawn(region, map), ([20., 30.], 90., 2));
}

#[test]
fn buff_source_scan_uses_all_ranked_columns() {
    let data = b"ID;PWXL;N;E\r\nB;X3;Y3;D0\r\nC;X1;Y1;K\"alias\"\r\nC;X2;K\"BuffID1\"\r\nC;X3;K\"BuffID2\"\r\nC;X1;Y2;K\"A001\"\r\nC;X2;K\"B001\"\r\nC;X1;Y3;K\"A002\"\r\nC;X3;K\"B002, B003\"\r\nE\r\n";
    let table = pcx_slq::Table::parse(data).unwrap();
    assert_eq!(
        aliases_using_buffs(&table, &["B003"]).unwrap(),
        BTreeSet::from(["A002".into()])
    );
    assert!(aliases_using_buffs(&table, &["B999"]).unwrap().is_empty());
    let mut record = pcx_objects::Record {
        original: *b"A001",
        modified: [0; 4],
        mods: vec![pcx_objects::Mod {
            field: *b"X001",
            value_type: pcx_objects::ValueType::Str,
            selectors: None,
            value: pcx_objects::Value::Str(b"B002, B003".to_vec()),
            end_word: 0,
        }],
    };
    assert_eq!(override_mentions(&record, |id| id == "B003"), Ok(true));
    record.mods[0].value = pcx_objects::Value::Str(vec![0xff]);
    assert!(override_mentions(&record, |_| false).is_err());
}

#[test]
fn building_facing_truncates_before_normalizing() {
    assert_eq!(building_facing(-17.5), 0.);
    assert_eq!(building_facing(-95.5), 270.);
    assert_eq!(building_facing(183.5), 180.);
    assert_eq!(building_facing(451.5), 90.);
}

#[test]
fn order_and_sample_inputs_read_the_observed_worker_coordinates() {
    assert_eq!(
        scripted_spawns("set u = CreateUnit( p, 'X001', 13.0, -2.0, 0.0 )", "X001"),
        [[13., -2.]]
    );
    assert_eq!(
        probe_order_elapsed(
            " E:t2/id1094861636/p7@12.5,-3/point4,5/targetnone",
            *b"ABCD",
            7
        ),
        2.
    );
    assert_eq!(
        probe_clock_delay(" call TimerStart(CreateTimer(), 3., false, function PCXO_Start)"),
        3.
    );
    assert_eq!(
        probe_order_worker(
            " E:t2/id1094861636/p7@12.5,-3/point4,5/targetnone",
            *b"ABCD",
            7
        ),
        [12.5, -3.]
    );
    assert_eq!(
        probe_sample_workers("acolytes A:12.5,-3/o42 A:9,8/o0"),
        [[12.5, -3.], [9., 8.]]
    );
    assert_eq!(
        probe_order_site(
            " E:t2/id1094861636/p7@12.5,-3/point4,5/targetnone",
            *b"ABCD",
            7
        ),
        Some([4., 5.])
    );
    assert_eq!(probe_start("start index=9@14,-8\""), Some([14., -8.]));
    assert_eq!(sample_index(std::path::Path::new("PCXO-11.txt")), Some(11));
    assert_eq!(sample_index(std::path::Path::new("other.txt")), None);
}

fn variant_ranks(
    root: &std::path::Path,
    rules: &Rules,
    bounds: [[f32; 2]; 2],
    dims: [u32; 2],
    quarter: u8,
    mine: [f32; 2],
    baseline_start: [f32; 2],
) {
    let mut paths: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    paths.sort();
    for path in paths {
        let data = path.join("run/prefix/drive_c/users/wine/Documents/Warcraft III/CustomMapData");
        if !data.is_dir() {
            continue;
        }
        let mut samples: Vec<_> = std::fs::read_dir(data)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                Some((sample_index(&path)?, path))
            })
            .collect();
        samples.sort();
        let log = samples
            .into_iter()
            .map(|(_, path)| std::fs::read_to_string(path).unwrap())
            .find(|log| probe_order_site(log, *b"unpl", 5).is_some());
        let Some(log) = log else {
            println!("variant={} no_necropolis_order", path.display());
            continue;
        };
        let site = probe_order_site(&log, *b"unpl", 5).unwrap();
        let worker = probe_order_worker(&log, *b"unpl", 5);
        let start = probe_start(&log).unwrap_or_else(|| {
            assert_eq!(path.file_name().unwrap(), "baseline");
            baseline_start
        });
        for (name, origin) in [("mine", mine), ("start", start), ("worker", worker)] {
            let grid = Grid::sample(rules, origin, |_| Ok((0, false))).unwrap();
            let mut rank = 0_usize;
            let mut found = None;
            grid.search(
                |point| {
                    rank += 1;
                    let point = rules.snap(point, bounds, dims, quarter)?;
                    if point == site && found.is_none() && grid.diagnostic_cell(point).is_some() {
                        found = Some(rank);
                    }
                    Ok((point, grid.diagnostic_indices(point)))
                },
                |_| Ok(true),
                |_| Ok(false),
            )
            .unwrap();
            println!(
                "variant={} site={site:?} worker={worker:?} start={start:?} origin={name} bfs_rank={found:?}",
                path.display()
            );
        }
    }
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL, PCX_STARTUP_MAP, PCX_HUMAN05_DECISION_LOG, PCX_HUMAN05_ORDER_PROBE and PCX_HUMAN05_PROBE_SCRIPT"]
fn human05_placement_diagnostic() {
    let full_reach = std::env::var_os("PCX_REACH_FULL_ORACLE").is_some()
        || (std::env::var_os("PCX_HUMAN05_TRACE_ORIGIN").is_some()
            && std::env::var_os("PCX_REACH_TRACE").is_none()
            && std::env::var_os("PCX_TOWN_EXPECT").is_none());
    if full_reach {
        assert!(std::env::var_os("PCX_HUMAN05_TRACE_ORIGIN").is_some());
    }
    if !full_reach
        && std::env::var_os("PCX_REACH_TRACE").is_none()
        && std::env::var_os("PCX_TOWN_EXPECT").is_none()
    {
        println!("native reach is bypassed in the exclusion scan");
    }
    let install = PathBuf::from(std::env::var("PCX_STARTUP_INSTALL").unwrap());
    let map = PathBuf::from(std::env::var("PCX_STARTUP_MAP").unwrap());
    let bytes = std::fs::read(&map).unwrap();
    let route_log =
        std::fs::read_to_string(std::env::var("PCX_HUMAN05_DECISION_LOG").unwrap()).unwrap();
    let point = route_log
        .split_once("by \"uaco\" at Some([")
        .unwrap()
        .1
        .split_once(']')
        .unwrap()
        .0;
    let (x, y) = point.split_once(',').unwrap();
    let route_worker = [
        x.trim().parse::<f32>().unwrap(),
        y.trim().parse::<f32>().unwrap(),
    ];
    let probe = PathBuf::from(std::env::var("PCX_HUMAN05_ORDER_PROBE").unwrap());
    let order_file = std::env::var("PCX_REACH_ORDER_FILE").unwrap_or_else(|_| "PCXO-11.txt".into());
    let sample_file =
        std::env::var("PCX_REACH_SAMPLE_FILE").unwrap_or_else(|_| "PCXO-10.txt".into());
    let order_log = std::fs::read_to_string(probe.join(order_file)).unwrap();
    let probe_script =
        std::fs::read_to_string(std::env::var("PCX_HUMAN05_PROBE_SCRIPT").unwrap()).unwrap();
    let nominal_time =
        probe_clock_delay(&probe_script) + probe_order_elapsed(&order_log, *b"unpl", 5);
    let sample_log = std::fs::read_to_string(probe.join(sample_file)).unwrap();
    let order_worker = probe_order_worker(&order_log, *b"unpl", 5);
    let mut sample_workers = probe_sample_workers(&sample_log);
    assert_eq!(sample_workers.len(), 4);
    let image_path = install.join("Warcraft III.exe");
    let image = std::fs::read(&image_path).unwrap();
    let identity =
        pcx_play::campaign_inputs::client_identity(&image, &image_path.display().to_string());
    let rules = Rules::reviewed_legacy(&image, identity.clone()).unwrap();
    pcx_play::native_path_hierarchy::EntryRules::reviewed_legacy(&image, &rules).unwrap();
    let mode = GameMode {
        build: Build::V1_29b,
        addon: Addon::NoAddon,
        assets: AssetMode::Vanilla,
        behaviour: Behaviour::Vanilla,
    };
    let image_rules = pcx_play::campaign_data_set::ImageRules::read(&image, identity).unwrap();
    let recipe = pcx_play::campaign_data_set::CampaignRecipe::pin(
        &install,
        mode,
        image_rules,
        &image,
        None,
        "VERSION_REIGN_OF_CHAOS",
    )
    .unwrap();
    let map_budget = pcx_mpq::checked::ReadBudget {
        max_archive_bytes: 64 << 20,
        max_archive_table_bytes: 8 << 20,
        max_raw_packed_bytes: 16 << 20,
        max_decoded_bytes: 16 << 20,
    };
    let resolver = recipe
        .open_map(&bytes, &map.display().to_string(), map_budget)
        .unwrap()
        .resolver;
    let misc = resolver
        .resolve_bounded("Units/MiscData.txt", 4 << 20)
        .unwrap();
    let map_misc = pcx_map::Map::parse(&bytes)
        .unwrap()
        .extract_checked(&map.display().to_string(), "war3mapMisc.txt", map_budget)
        .unwrap();
    let mut profiles = vec![("Units/MiscData.txt", misc.bytes.as_slice())];
    if let Some(ref map_misc) = map_misc {
        profiles.push(("war3mapMisc.txt", map_misc.as_slice()));
    }
    let ([Some(angle)], _) =
        pcx_objects::editor::misc::misc_scalars(&profiles, ["BuildingAngle"], 8 << 20).unwrap()
    else {
        panic!("selected Misc/BuildingAngle is missing")
    };
    let facing = building_facing(angle);
    let mut startup = start(&install, mode, &map.display().to_string(), &bytes, 1, None).unwrap();
    let objects = startup.session.host().unwrap().objects_mut();
    objects.diagnostic_set_snapshot_time(nominal_time).unwrap();
    println!(
        "snapshot game clock {:?}; nominal order time, timer callbacks and movement were not replayed",
        objects.timer_game_time()
    );
    let player = objects.native("Player", &[Value::Int(5)]).unwrap();
    let Value::Handle(player_handle) = player else {
        panic!("player handle")
    };
    if let Ok(expected) = std::env::var("PCX_TOWN_EXPECT") {
        use pcx_play::ai_placement::{TownUnitClass, select_registered_town_reference};
        let map_script = pcx_map::Map::parse(&bytes)
            .unwrap()
            .extract_checked(&map.display().to_string(), "war3map.j", map_budget)
            .unwrap()
            .unwrap();
        let (spawn, face, count) =
            probe_spawn(&probe_script, std::str::from_utf8(&map_script).unwrap());
        let mine = objects
            .executed_units()
            .find(|(_, unit)| {
                unit.definition.rawcode == *b"ngol" && unit.position == Some([5696., 3456.])
            })
            .unwrap()
            .1
            .position
            .unwrap();
        let mut units = Vec::new();
        for _ in 0..count {
            let Value::Handle(id) = objects
                .native(
                    "CreateUnit",
                    &[
                        Value::Handle(player_handle),
                        Value::Int(i32::from_be_bytes(*b"uaco")),
                        Value::Real(spawn[0]),
                        Value::Real(spawn[1]),
                        Value::Real(face),
                    ],
                )
                .unwrap()
            else {
                panic!("town fixture worker")
            };
            let unit = objects.executed_unit(id).unwrap();
            let class = if text(&unit.definition, "type")
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("peon"))
            {
                TownUnitClass::Peon
            } else {
                TownUnitClass::Other
            };
            units.push((id, unit.position.unwrap(), class));
        }
        let selected = select_registered_town_reference(
            units.iter().map(|(id, _, class)| (*id, true, *class)),
        )
        .unwrap();
        let position = units.iter().find(|(id, _, _)| *id == selected).unwrap().1;
        let center = rules
            .town_center(spawn, Some(mine), Some(position))
            .unwrap();
        println!(
            "town snapshot spawn={spawn:?} candidates={units:?} selected={position:?} center={center:?} (fixture assumes registration in creation order)"
        );
        assert_eq!(center, probe_point(&expected));
        return;
    }
    let start_index = objects
        .player_settings(player_handle)
        .unwrap()
        .start_location
        .unwrap();
    let start = objects
        .map_config()
        .unwrap()
        .start_locations
        .iter()
        .find(|p| p.index == start_index)
        .unwrap();
    let start_point = [start.x, start.y];
    let (_, mine_record) = objects
        .executed_units()
        .find(|(_, u)| u.definition.rawcode == *b"ngol" && u.position == Some([5696., 3456.]))
        .unwrap();
    let mine = mine_record.position.unwrap();
    let center = rules.town_center(start_point, Some(mine), None).unwrap();
    println!(
        "orange start {start_point:?}; primary mine {mine:?}; diagnostic town {center:?}; facing {facing}"
    );
    let catalog = objects.unit_catalog().unwrap();
    let acolyte = catalog.resolve(*b"uaco").unwrap();
    let worker_radius = text(&acolyte, "collision").parse::<f32>().unwrap();
    let caster = catalog.resolve(*b"nmrr").unwrap();
    let caster_radius = text(&caster, "collision").parse::<f32>().unwrap();
    let game_misc = resolver
        .resolve_bounded("Units/MiscGame.txt", 4 << 20)
        .unwrap();
    let mut game_profiles = vec![("Units/MiscGame.txt", game_misc.bytes.as_slice())];
    if let Some(ref map_misc) = map_misc {
        game_profiles.push(("war3mapMisc.txt", map_misc.as_slice()));
    }
    let limits = pcx_objects::editor::misc::MoveSpeedLimits::read(&game_profiles, 8 << 20).unwrap();
    let source_speed = text(&caster, "spd")
        .parse::<f32>()
        .unwrap()
        .max(limits.max_unit);
    let target_speed = text(&acolyte, "spd")
        .parse::<f32>()
        .unwrap()
        .max(limits.max_unit);
    let range = objects.diagnostic_ai_cast_range(*b"ACen", 1).unwrap();
    let map_script = pcx_map::Map::parse(&bytes)
        .unwrap()
        .extract_checked(&map.display().to_string(), "war3map.j", map_budget)
        .unwrap()
        .unwrap();
    counter_source_scan(
        &resolver,
        std::str::from_utf8(&map_script).unwrap(),
        &pcx_map::Map::parse(&bytes).unwrap(),
        &map.display().to_string(),
        map_budget,
    )
    .unwrap();
    let spawns = scripted_spawns(std::str::from_utf8(&map_script).unwrap(), "nmrr");
    assert_eq!(spawns.len(), 2);
    let mut observed_starts = vec![(
        "baseline",
        nominal_time,
        probe_sample_workers(&std::fs::read_to_string(probe.join("PCXO-2.txt")).unwrap()),
    )];
    if let Ok(path) = std::env::var("PCX_HUMAN05_FARM_ORDER") {
        let path = PathBuf::from(path);
        let order = std::fs::read_to_string(&path).unwrap();
        let elapsed = order
            .split_once("tag=order t=")
            .unwrap()
            .1
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap();
        let initial = std::fs::read_to_string(path.with_file_name("PCXO-2.txt")).unwrap();
        observed_starts.push((
            "farm",
            probe_clock_delay(&probe_script) + elapsed,
            probe_sample_workers(&initial),
        ));
    }
    for (layout, elapsed, starts) in observed_starts {
        assert_eq!(starts.len(), 4);
        for origin in &spawns {
            for initial in &starts {
                let source = KinematicWindow {
                    origin: *origin,
                    maximum_speed: source_speed,
                    elapsed,
                };
                let target = KinematicWindow {
                    origin: *initial,
                    maximum_speed: target_speed,
                    elapsed,
                };
                let outside = beyond_cast_reach(
                    Some(source),
                    Some(target),
                    range,
                    [caster_radius, worker_radius],
                );
                println!(
                    "{layout} observed-start cast bound source={origin:?} target={initial:?} outside={outside:?}"
                );
                if std::env::var_os("PCX_REACH_TRACE").is_none()
                    && std::env::var_os("PCX_REACH_ORACLE_SITE").is_none()
                {
                    assert_eq!(outside, Ok(true));
                }
            }
        }
    }
    let mut blocked_sample = vec![
        [5611.785, 2795.562],
        [5676.686, 2656.12],
        [5779.904, 2595.492],
        [5468.591, 2861.923],
    ];
    let exact_farm = std::env::var("PCX_HUMAN05_FARM_ORDER").ok();
    if let Some(path) = &exact_farm {
        let log = std::fs::read_to_string(path).unwrap();
        blocked_sample = probe_sample_workers(&log);
        assert_eq!(blocked_sample.len(), 4);
        let issuer = probe_point(
            log.split_once("issuer ")
                .unwrap()
                .1
                .split_once(" site ")
                .unwrap()
                .0,
        );
        let index = blocked_sample.iter().position(|p| *p == issuer).unwrap();
        blocked_sample.swap(0, index);
    }
    if std::env::var_os("PCX_REACH_TRACE").is_some()
        || std::env::var_os("PCX_REACH_ORACLE_SITE").is_some()
    {
        let closest = sample_workers
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let distance =
                    |p: &&[f32; 2]| (p[0] - order_worker[0]).hypot(p[1] - order_worker[1]);
                distance(a).total_cmp(&distance(b))
            })
            .unwrap()
            .0;
        sample_workers.remove(closest);
    }
    let mut workers = Vec::new();
    for point in std::iter::once(order_worker).chain(sample_workers.into_iter().take(3)) {
        let created = objects
            .native(
                "CreateUnit",
                &[
                    Value::Handle(player_handle),
                    Value::Int(i32::from_be_bytes(*b"uaco")),
                    Value::Real(point[0]),
                    Value::Real(point[1]),
                    Value::Real(facing),
                ],
            )
            .unwrap();
        let Value::Handle(handle) = created else {
            panic!("acolyte creation")
        };
        workers.push(handle);
    }
    let worker = workers[0];
    if let Ok(trace) = std::env::var("PCX_REACH_TRACE") {
        let center = probe_point(&std::env::var("PCX_HUMAN05_TRACE_ORIGIN").unwrap());
        let mobile: Vec<_> = objects
            .executed_units()
            .filter(|(_, unit)| text(&unit.definition, "isbldg") == "0")
            .map(|(id, _)| id)
            .collect();
        let snapshot = reach_snapshot(
            objects.diagnostic_placement().unwrap(),
            worker,
            &mobile,
            &image,
            &rules,
        );
        compare_reach_snapshot(&snapshot, center, &trace);
        return;
    }
    let necropolis = catalog.resolve(*b"unpl").unwrap();
    let path = text(&necropolis, "pathTex");
    let dims = dimensions(&resolver, path);
    println!("construction footprint {path} dimensions {dims:?}");
    let quarter = (facing / 90.).rem_euclid(4.) as u8;
    assert_eq!(text(&necropolis, "preventPlace"), "unbuildable");
    assert!(matches!(text(&necropolis, "requirePlace"), "_" | "" | "-"));

    if let Ok(root) = std::env::var("PCX_HUMAN05_VARIANTS") {
        let bounds = objects.diagnostic_placement().unwrap().world_bounds();
        variant_ranks(
            std::path::Path::new(&root),
            &rules,
            bounds,
            dims,
            quarter,
            mine,
            start_point,
        );
    }

    let mut refusals = Vec::new();
    for (blocked, oracle) in [(false, [6208., 2752.]), (true, [5952., 2432.])] {
        if blocked && std::env::var_os("PCX_REACH_ORACLE_SITE").is_some() {
            continue;
        }
        let oracle = std::env::var("PCX_REACH_ORACLE_SITE")
            .map(|value| probe_point(&value))
            .unwrap_or(oracle);
        if blocked {
            for &handle in &workers {
                objects
                    .native(
                        "SetUnitPathing",
                        &[Value::Handle(handle), Value::Bool(false)],
                    )
                    .unwrap();
            }
            for (&handle, &point) in workers.iter().zip(&blocked_sample) {
                objects
                    .native(
                        "SetUnitPosition",
                        &[
                            Value::Handle(handle),
                            Value::Real(point[0]),
                            Value::Real(point[1]),
                        ],
                    )
                    .unwrap();
            }
            for &handle in &workers {
                objects
                    .native(
                        "SetUnitPathing",
                        &[Value::Handle(handle), Value::Bool(true)],
                    )
                    .unwrap();
            }
            let passive = objects.native("Player", &[Value::Int(15)]).unwrap();
            objects
                .native(
                    "CreateUnit",
                    &[
                        passive,
                        Value::Int(i32::from_be_bytes(*b"hhou")),
                        Value::Real(6208.),
                        Value::Real(2752.),
                        Value::Real(facing),
                    ],
                )
                .unwrap();
            if exact_farm.is_none() {
                objects
                    .native(
                        "SetUnitPosition",
                        &[
                            Value::Handle(worker),
                            Value::Real(5577.521),
                            Value::Real(2708.195),
                        ],
                    )
                    .unwrap();
            }
            println!("farm order sample {blocked_sample:?}");
            let actual = workers
                .iter()
                .map(|handle| objects.executed_unit(*handle).unwrap().position.unwrap())
                .collect::<Vec<_>>();
            if exact_farm.is_some() {
                assert_eq!(actual, blocked_sample);
            }
            println!("farm recreated worker positions {actual:?}");
        }
        let checkpoint = objects.diagnostic_placement_checkpoint();
        for &handle in &workers {
            let record = objects.executed_unit(handle).unwrap();
            let buffs = objects.diagnostic_ai_buffs(handle).unwrap();
            let pause_requested = objects.placement_events(handle).unwrap().any(|event| {
                matches!(
                    event.kind,
                    pcx_play::ai_placement::PlacementEventKind::PauseUnit(_)
                )
            });
            println!(
                "fixture blocked={blocked} unit={handle:?} paused={} pause_requested={} sleeping={} buffs={buffs:?}",
                record.paused_since.is_some(),
                pause_requested,
                record.is_sleeping(),
            );
        }
        let worker_point = objects.executed_unit(worker).unwrap().position.unwrap();
        println!("probe-sampled first worker {worker_point:?}, collision radius {worker_radius}");
        let mut origins = vec![
            ("mine", center),
            ("start", start_point),
            ("probe-worker", worker_point),
            ("route-worker", route_worker),
        ];
        if let Ok(observed) = std::env::var("PCX_HUMAN05_TRACE_ORIGIN") {
            origins.push(("native-trace", probe_point(&observed)));
        }
        let structures: Vec<_> = objects
            .executed_units()
            .filter_map(|(_, unit)| {
                let p = unit.position.unwrap_or([unit.x, unit.y]);
                let nearby = origins.iter().any(|(_, origin)| {
                    (p[0] - origin[0]).hypot(p[1] - origin[1]) <= rules.diagnostic_radius()
                });
                if !nearby || text(&unit.definition, "isbldg") != "1" {
                    return None;
                }
                let path = text(&unit.definition, "pathTex");
                let kind = match text(&unit.definition, "buffType") {
                    "townhall" => BuffType::TownHall,
                    "resource" => BuffType::Resource,
                    "factory" => BuffType::Factory,
                    "buffer" => BuffType::Buffer,
                    _ => BuffType::Other,
                };
                let radius = match kind {
                    BuffType::Other => 0,
                    _ => text(&unit.definition, "buffRadius").parse::<f32>().unwrap() as i32,
                };
                Some((
                    unit.definition.rawcode,
                    p,
                    dimensions(&resolver, path),
                    kind,
                    radius,
                ))
            })
            .collect();
        let unit_evidence = objects
            .executed_units()
            .filter(|(id, _)| *id != worker)
            .map(|(id, record)| {
                let position = record.position.ok_or(ExclusionError::Unavailable {
                    unit: id,
                    source_name: "accepted unit position",
                })?;
                let evidence_rectangle = rules
                    .exclusion_rectangle(position.map(f32::floor), dims, quarter)
                    .map_err(|_| ExclusionError::Unavailable {
                        unit: id,
                        source_name: "unit evidence rectangle",
                    })?;
                let (shared, evidence) = match objects.diagnostic_ai_exclusion(
                    id,
                    player_handle,
                    evidence_rectangle,
                    ExclusionVisibility::NotChecked,
                    checkpoint,
                    MovementEvidence::CountersZero,
                ) {
                    Ok(true) => (true, MovementEvidence::CountersZero),
                    Ok(false) => (false, MovementEvidence::UnknownSource("movement ability")),
                    Err(ExclusionError::UnknownMovement { source_name, .. }) => {
                        (true, MovementEvidence::UnknownSource(source_name))
                    }
                    Err(ExclusionError::UnknownBuff { rawcode, .. }) => {
                        (true, MovementEvidence::UnknownBuff(rawcode))
                    }
                    Err(error) => return Err(error),
                };
                Ok((id, position, shared, evidence, record.definition.rawcode))
            })
            .collect::<Result<Vec<_>, ExclusionError>>()
            .unwrap();
        let mobile: Vec<_> = objects
            .executed_units()
            .filter(|(_, unit)| text(&unit.definition, "isbldg") == "0")
            .map(|(id, _)| id)
            .collect();
        let placement = objects.diagnostic_placement().unwrap();
        let bounds = placement.world_bounds();
        let reach = full_reach.then(|| reach_snapshot(placement, worker, &mobile, &image, &rules));
        if let Ok(point) = std::env::var("PCX_HUMAN05_TRACE_CELL") {
            let point = probe_point(&point);
            println!(
                "blocked={blocked} native trace cell {point:?}: {:?}",
                placement.diagnostic_cell_state(point).unwrap()
            );
        }
        if blocked {
            let cell_size = placement.rules().cell_size();
            let mask = placement.rules().ground_mask() | 0x08;
            for x in -(dims[0] as i32 / 2)..(dims[0] as i32 / 2) {
                for y in -(dims[1] as i32 / 2)..(dims[1] as i32 / 2) {
                    let point = [
                        oracle[0] + x as f32 * cell_size,
                        oracle[1] + y as f32 * cell_size,
                    ];
                    let state = placement.diagnostic_cell_state(point).unwrap();
                    if state.0 & mask != 0 || state.1 != 0 || state.2 != 0 {
                        println!(
                            "oracle-nearby {point:?} flags={:#x} units={} footprints={}",
                            state.0, state.1, state.2
                        );
                    }
                }
            }
        }
        for (origin_name, center) in origins {
            if full_reach && origin_name != "native-trace" {
                continue;
            }
            for swap in [false, true] {
                let mut grid = Grid::sample(&rules, center, |p| {
                    placement.diagnostic_sample(p).map_err(|e| e.to_string())
                })
                .unwrap();
                for (code, p, size, kind, radius) in &structures {
                    if (p[0] - center[0]).hypot(p[1] - center[1]) > rules.diagnostic_radius() {
                        continue;
                    }
                    let mut size = *size;
                    if swap {
                        size.swap(0, 1);
                    }
                    if !quarter.is_multiple_of(2) {
                        size.swap(0, 1);
                    }
                    let step = grid.diagnostic_step();
                    let cells: Vec<_> = (0..size[0])
                        .flat_map(|x| (0..size[1]).map(move |y| [x, y]))
                        .map(|[x, y]| {
                            grid.diagnostic_object_indices([
                                p[0] + (x as f32 - (size[0] - 1) as f32 / 2.) * step,
                                p[1] + (y as f32 - (size[1] - 1) as f32 / 2.) * step,
                            ])
                            .unwrap()
                        })
                        .collect();
                    println!(
                        "object {} {p:?} texture {size:?} {kind:?} radius {radius}",
                        String::from_utf8_lossy(code)
                    );
                    grid.score_rectangle(cells, *kind, *radius).unwrap();
                }
                let placement = std::cell::RefCell::new(&mut *placement);
                let rank = std::cell::Cell::new(0usize);
                let oracle_rank = std::cell::Cell::new(None);
                let first_valid = std::cell::Cell::new(None);
                let first_same_score_valid = std::cell::Cell::new(None);
                let oracle_score = grid.diagnostic_cell(oracle).map(|(_, score)| score);
                let validate = |p| -> Result<bool, String> {
                    let rectangle = rules.exclusion_rectangle(p, dims, quarter)?;
                    let mut excluded = vec![worker];
                    for &(id, position, shared, evidence, rawcode) in &unit_evidence {
                        if exclude_unit(
                            id,
                            position,
                            rectangle,
                            ExclusionContext::AiPlacement,
                            shared,
                            ExclusionVisibility::NotChecked,
                            evidence,
                        )
                        .map_err(|error| {
                            format!(
                                "candidate {p:?} dequeue {} unit {}: {error}",
                                rank.get(),
                                String::from_utf8_lossy(&rawcode)
                            )
                        })? {
                            excluded.push(id);
                        }
                    }
                    placement
                        .borrow_mut()
                        .diagnostic_construction_position_excluding(
                            p, facing, path, 0x08, 0, &excluded,
                        )
                        .map(|value| value.is_some())
                        .map_err(|error| error.to_string())
                };
                let scan = grid.search(
                    |p| {
                        rank.set(rank.get() + 1);
                        let p = rules.snap(p, bounds, dims, quarter)?;
                        if p == oracle
                            && oracle_rank.get().is_none()
                            && grid.diagnostic_cell(p).is_some()
                        {
                            oracle_rank.set(Some(rank.get()));
                        }
                        Ok((p, grid.diagnostic_indices(p)))
                    },
                    |p| match &reach {
                        Some(snapshot) => snapshot.query(center, p).map(|(value, _)| value != -1),
                        None => Ok(true),
                    },
                    |p| {
                        let valid = validate(p)?;
                        if valid && first_valid.get().is_none() {
                            first_valid.set(Some((p, rank.get())));
                        }
                        if valid
                            && first_same_score_valid.get().is_none()
                            && grid.diagnostic_cell(p).map(|(_, score)| score) == oracle_score
                        {
                            first_same_score_valid.set(Some((p, rank.get())));
                        }
                        Ok(full_reach && valid)
                    },
                );
                if let Err(error) = scan {
                    println!(
                        "blocked={blocked} origin={origin_name} axis_swap={swap} unresolved={error}"
                    );
                    refusals.push(error);
                    continue;
                }
                if full_reach {
                    let chosen = scan.unwrap();
                    println!(
                        "full reach blocked={blocked} axis_swap={swap} chosen={chosen:?} dequeues={}",
                        rank.get()
                    );
                    assert_eq!(chosen, Some(oracle));
                    let expected_rank = std::env::var("PCX_REACH_ORACLE_RANK")
                        .map(|value| value.parse().unwrap())
                        .unwrap_or(if blocked { 280 } else { 1 });
                    assert_eq!(rank.get(), expected_rank);
                    continue;
                }
                if exact_farm.is_some() && origin_name == "native-trace" {
                    assert_eq!(
                        first_same_score_valid.get(),
                        Some((oracle, if blocked { 280 } else { 1 }))
                    );
                }
                println!(
                    "blocked={blocked} origin={origin_name} center={center:?} axis_swap={swap} oracle_bfs_rank={:?} first_valid={:?} first_same_score_valid={:?} total_pops={}",
                    oracle_rank.get(),
                    first_valid.get(),
                    first_same_score_valid.get(),
                    rank.get()
                );
                for reach in [0, 1, 2] {
                    let mut town_checks = 0;
                    let result = grid.search(
                        |p| {
                            let p = rules.snap(p, bounds, dims, quarter)?;
                            Ok((p, grid.diagnostic_indices(p)))
                        },
                        |p| {
                            if reach == 2 {
                                town_checks += 1;
                                if town_checks > 8 {
                                    return Err("town-origin route diagnostic work limit".into());
                                }
                            }
                            if reach != 0 {
                                placement
                                    .borrow()
                                    .diagnostic_reach(
                                        worker,
                                        if reach == 1 { worker_point } else { center },
                                        p,
                                        worker_radius,
                                    )
                                    .map_err(|e| e.to_string())
                            } else {
                                Ok(true)
                            }
                        },
                        validate,
                    );
                    let oracle_cell = grid.diagnostic_cell(oracle);
                    let chosen = match result {
                        Ok(chosen) => chosen,
                        Err(error) => {
                            println!(
                                "blocked={blocked} origin={origin_name} axis_swap={swap} reach_variant={reach} oracle_cell={oracle_cell:?} oracle_bfs_rank={:?} unresolved={error}",
                                oracle_rank.get()
                            );
                            continue;
                        }
                    };
                    let chosen_cell = chosen.and_then(|p| grid.diagnostic_cell(p));
                    let distance = chosen.map(|p| (p[0] - oracle[0]).hypot(p[1] - oracle[1]));
                    let oracle_valid = validate(oracle).unwrap();
                    println!(
                        "blocked={blocked} origin={origin_name} axis_swap={swap} reach_variant={reach} chosen={chosen:?} chosen_cell={chosen_cell:?} oracle={oracle:?} oracle_cell={oracle_cell:?} oracle_bfs_rank={:?} oracle_valid={oracle_valid} distance={distance:?}",
                        oracle_rank.get()
                    );
                }
            }
        }
    }
    println!("mine oracle {mine:?}: mine-target order is not a decoded grid search");
    println!("ziggurats: validator refuses the source blighted requirement");
    assert!(
        refusals.is_empty(),
        "AI placement exclusions unresolved: {}",
        refusals.join("; ")
    );
}

struct ReachSnapshot {
    levels: Vec<([usize; 2], Vec<pcx_play::native_path_hierarchy::Cell>)>,
    bounds: [[f32; 2]; 2],
    cell_size: f32,
    group: usize,
    priority: u32,
    result_scale: u32,
    removal_limit: usize,
    movement: i32,
    count: u32,
}

fn reach_snapshot(
    placement: &pcx_play::unit_placement::Placement,
    worker: pcx_script::HandleId,
    mobile: &[pcx_script::HandleId],
    image: &[u8],
    rules: &Rules,
) -> ReachSnapshot {
    use pcx_play::native_path_hierarchy::{base, diagnostic_operands, parent};
    let (mask, priority, result_scale, removal_limit, movement, count) =
        diagnostic_operands(image, rules).unwrap();
    let bounds = placement.world_bounds();
    let cell_size = placement.rules().cell_size();
    let dimensions = std::array::from_fn::<_, 2, _>(|axis| {
        ((bounds[1][axis] - bounds[0][axis]) / cell_size) as usize
    });
    let rectangle = placement.diagnostic_unit_rectangle(worker).unwrap();
    let group = result_scale as usize;
    assert_eq!(group, 2, "unrecognized spatial sample grouping");
    if let Ok(value) = std::env::var("PCX_REACH_INSPECT_CELL") {
        let point = probe_point(&value).map(|v| v as usize);
        let span = 1_usize << (count - 1);
        let lower = point.map(|v| v / span * span * group);
        for y in lower[1]..lower[1] + span * group {
            for x in lower[0]..lower[0] + span * group {
                let world = [
                    bounds[0][0] + (x as f32 + 0.5) * cell_size,
                    bounds[0][1] + (y as f32 + 0.5) * cell_size,
                ];
                let state = placement.diagnostic_cell_state(world).unwrap();
                if state.0 & mask != 0 || state.1 != 0 || state.2 != 0 {
                    println!("reach inspect point={world:?} state={state:?}");
                }
            }
        }
    }
    let mut levels = Vec::new();
    let size = [dimensions[0].div_ceil(group), dimensions[1].div_ceil(group)];
    let mut cells = Vec::with_capacity(size[0] * size[1]);
    for y in 0..size[1] {
        for x in 0..size[0] {
            let mut samples = [true; 4];
            for (i, sample) in samples.iter_mut().enumerate() {
                let p = [x * group + i % group, y * group + i / group];
                if p[0] < dimensions[0] && p[1] < dimensions[1] {
                    let point = std::array::from_fn(|axis| {
                        bounds[0][axis] + (p[axis] as f32 + 0.5) * cell_size
                    });
                    let (flags, units, footprints) =
                        placement.diagnostic_cell_excluding(point, mobile).unwrap();
                    *sample = flags & mask != 0 || units != 0 || footprints != 0;
                }
            }
            let cleared = (0..2).all(|axis| {
                let coordinate = [x, y][axis] * group;
                coordinate < rectangle[1][axis] as usize
                    && coordinate + group > rectangle[0][axis] as usize
            });
            cells.push(base(samples, false, cleared));
        }
    }
    levels.push((size, cells));
    while levels.len() < count as usize {
        let (size, previous) = levels.last().unwrap();
        let next_size = size.map(|v| v.div_ceil(2));
        let mut next = Vec::with_capacity(next_size[0] * next_size[1]);
        for y in 0..next_size[1] {
            for x in 0..next_size[0] {
                let children = std::array::from_fn(|i| {
                    let p = [x * 2 + i % 2, y * 2 + i / 2];
                    (p[0] < size[0] && p[1] < size[1]).then(|| previous[p[1] * size[0] + p[0]])
                });
                next.push(parent(children));
            }
        }
        levels.push((next_size, next));
    }
    println!(
        "reach snapshot bounds={bounds:?} builder={rectangle:?}; link and per-entry state unqualified"
    );
    ReachSnapshot {
        levels,
        bounds,
        cell_size,
        group,
        priority,
        result_scale,
        removal_limit,
        movement,
        count,
    }
}

impl ReachSnapshot {
    fn query(&self, center: [f32; 2], point: [f32; 2]) -> Result<(i32, usize), String> {
        self.query_observed(center, point, |_, _, _| {})
    }

    fn query_observed(
        &self,
        center: [f32; 2],
        point: [f32; 2],
        observe: impl FnMut(
            pcx_play::native_path_hierarchy::Node,
            pcx_play::native_path_queue::Entry,
            u32,
        ),
    ) -> Result<(i32, usize), String> {
        use pcx_play::native_path_hierarchy::{
            BaseLink, BaseNeighbor, Cell, CoarseInput, Lookup, Node, SearchInput, distance,
            expand_base, expand_coarse, lookup, search_observed,
        };
        let cell = |level: u32, point: [i32; 2]| -> Option<Cell> {
            let (size, cells) = self.levels.get(level as usize)?;
            let [x, y] = point.map(|v| usize::try_from(v).ok());
            let (x, y) = (x?, y?);
            (x < size[0] && y < size[1]).then(|| cells[y * size[0] + x])
        };
        let coordinates = |point: [f32; 2]| -> [i32; 2] {
            std::array::from_fn(|axis| {
                ((point[axis] - self.bounds[0][axis]) / (self.cell_size * self.group as f32))
                    .floor() as i32
            })
        };
        let start = coordinates(center);
        let goal = coordinates(point);
        if cell(0, start).is_none() {
            return Err("reach start is outside the snapshot".into());
        }
        let resolve = |level, position| lookup(level, self.count, position, cell);
        let open = |position| Some(matches!(cell(0, position), Some(Cell::Open { .. })));
        let start_node = resolve(0, start).unwrap();
        let goal_node = match resolve(0, goal).unwrap() {
            // A blocked goal has no native node, but still supplies the heuristic coordinate.
            Lookup::Blocked => Lookup::Open {
                level: 0,
                position: goal,
            },
            value => value,
        };
        if start == goal || matches!(start_node, Lookup::Blocked) {
            Ok((
                distance(start, goal, self.result_scale)
                    .map(|value| value as i32)
                    .unwrap(),
                0,
            ))
        } else if let (
            Lookup::Open {
                level: a,
                position: s,
            },
            Lookup::Open {
                level: b,
                position: g,
            },
        ) = (start_node, goal_node)
        {
            let input = SearchInput {
                start: Node {
                    level: a,
                    position: s,
                },
                goal: Node {
                    level: b,
                    position: g,
                },
                priority_scale: self.priority,
                result_scale: self.result_scale,
                link_priority_cost: 0,
                link_result_cost: 0,
                removal_limit: self.removal_limit,
                node_limit: self.levels.iter().map(|(_, cells)| cells.len()).sum(),
            };
            search_observed(
                input,
                |node| {
                    if node.level == 0 {
                        expand_base(
                            node.position,
                            self.movement,
                            self.movement,
                            BaseLink::Absent,
                            resolve,
                            open,
                        )
                    } else {
                        expand_coarse(
                            CoarseInput {
                                node,
                                movement_width: self.movement,
                                clearance_width: self.movement,
                                max_visits: input.node_limit,
                            },
                            resolve,
                            open,
                        )
                        .map(|nodes| {
                            nodes
                                .into_iter()
                                .map(|node| BaseNeighbor {
                                    node,
                                    via_link: false,
                                })
                                .collect()
                        })
                    }
                },
                observe,
            )
            .map(|result| {
                (
                    result.distance.map(|value| value as i32).unwrap_or(-1),
                    result.removals,
                )
            })
            .map_err(|e| e.to_string())
        } else {
            Err("unresolved base lookup".to_owned())
        }
    }
}

fn compare_reach_snapshot(snapshot: &ReachSnapshot, center: [f32; 2], trace: &str) {
    let mut mismatches = 0;
    let mut queries = 0;
    for line in std::fs::read_to_string(trace).unwrap().lines().skip(1) {
        let values: Vec<_> = line.split(',').collect();
        if values.len() < 7 || values[5].is_empty() {
            continue;
        }
        queries += 1;
        let rank = values[0].parse::<usize>().unwrap();
        let point = [
            values[3].parse::<f32>().unwrap(),
            values[4].parse::<f32>().unwrap(),
        ];
        let expected = values[5].parse::<i32>().unwrap();
        let actual = snapshot.query(center, point);
        if let Ok(node_rank) = std::env::var("PCX_REACH_NODE_RANK")
            && rank == node_rank.parse::<usize>().unwrap()
        {
            let log =
                std::fs::read_to_string(std::env::var("PCX_REACH_NODE_LOG").unwrap()).unwrap();
            let first_query = log.split_once("Stopped on breakpoint 4 at").unwrap().0;
            let removals = first_query
                .matches("Stopped on breakpoint 3 at 0x000000008a3c45")
                .count();
            let mut recorded = Vec::new();
            for block in first_query.split("Stopped on breakpoint 3 at").skip(1) {
                let rows: Vec<Vec<u32>> = block
                    .lines()
                    .filter_map(|line| {
                        let (address, words) = line.split_once(':')?;
                        if !address.starts_with("0x") {
                            return None;
                        }
                        Some(
                            words
                                .split_whitespace()
                                .map(|word| u32::from_str_radix(word, 16).unwrap())
                                .collect(),
                        )
                    })
                    .collect();
                let first = &rows[0];
                let level = (rows[2][0] >> 16) & 0xff;
                let queue = &rows[3];
                recorded.push((
                    pcx_play::native_path_hierarchy::Node {
                        position: [first[0] as i32, first[1] as i32],
                        level,
                    },
                    queue[0],
                    queue[2],
                    first[2],
                ));
            }
            let mut observed = Vec::new();
            snapshot
                .query_observed(center, point, |node, entry, current| {
                    observed.push((node, entry.priority, entry.generation, current))
                })
                .unwrap();
            for (i, (original, recreated)) in recorded.iter().zip(&observed).enumerate() {
                if original != recreated {
                    println!(
                        "reach rank={rank} removal={} original={original:?} recreation={recreated:?}",
                        i + 1
                    );
                }
            }
            assert_eq!(observed, recorded, "reach queue removal trace differs");
            println!(
                "reach rank={rank} removals original={removals} recreation={:?}",
                actual.as_ref().map(|v| v.1)
            );
            assert_eq!(actual.as_ref().unwrap().1, removals);
        }
        if actual.as_ref().map(|v| v.0).ok() != Some(expected) {
            mismatches += 1;
            println!(
                "reach mismatch rank={rank} point={point:?} expected={expected} actual={actual:?}"
            );
        }
    }
    println!("reach comparison queries={queries} mismatches={mismatches}");
    assert_ne!(queries, 0, "reach table has no queries");
    if let Ok(expected) = std::env::var("PCX_REACH_EXPECT_QUERIES") {
        assert_eq!(queries, expected.parse::<usize>().unwrap());
    }
    assert_eq!(
        mismatches, 0,
        "reach snapshot differs from the native table"
    );
}
