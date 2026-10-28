//! Human05 placement diagnostic. Requires the selected local install and map.
#![cfg(feature = "test-fixtures")]

use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode, Resolver};
use pcx_play::ai_placement::{BuffType, Grid, Rules};
use pcx_play::headless_startup::start;
use pcx_script::{Host, Value};
use pcx_sim::world::{Definition, FieldValue};
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
    [x.parse().expect("probe x"), y.parse().expect("probe y")]
}

fn probe_order_worker(log: &str, kind: [u8; 4], owner: i32) -> [f32; 2] {
    let marker = format!("/id{}/p{owner}@", i32::from_be_bytes(kind));
    let event = log.split_once(&marker).expect("probe order").1;
    probe_point(event.split_once("/point").expect("probe point order").0)
}

fn probe_sample_workers(log: &str) -> Vec<[f32; 2]> {
    log.split(" A:")
        .skip(1)
        .map(|worker| probe_point(worker.split_once("/o").expect("probe worker order").0))
        .collect()
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
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL, PCX_STARTUP_MAP, PCX_HUMAN05_DECISION_LOG and PCX_HUMAN05_ORDER_PROBE"]
fn human05_placement_diagnostic() {
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
    let order_log = std::fs::read_to_string(probe.join("PCXO-11.txt")).unwrap();
    let sample_log = std::fs::read_to_string(probe.join("PCXO-10.txt")).unwrap();
    let order_worker = probe_order_worker(&order_log, *b"unpl", 5);
    let sample_workers = probe_sample_workers(&sample_log);
    assert_eq!(sample_workers.len(), 4);
    let image_path = install.join("Warcraft III.exe");
    let image = std::fs::read(&image_path).unwrap();
    let identity =
        pcx_play::campaign_inputs::client_identity(&image, &image_path.display().to_string());
    let rules = Rules::reviewed_legacy(&image, identity.clone()).unwrap();
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
    let player = objects.native("Player", &[Value::Int(5)]).unwrap();
    let Value::Handle(player_handle) = player else {
        panic!("player handle")
    };
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
    let blocked_sample = [
        [5611.785, 2795.562],
        [5676.686, 2656.12],
        [5779.904, 2595.492],
        [5468.591, 2861.923],
    ];
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
    let necropolis = catalog.resolve(*b"unpl").unwrap();
    let path = text(&necropolis, "pathTex");
    let dims = dimensions(&resolver, path);
    let quarter = (facing / 90.).rem_euclid(4.) as u8;
    assert_eq!(text(&necropolis, "preventPlace"), "unbuildable");
    assert!(matches!(text(&necropolis, "requirePlace"), "_" | "" | "-"));

    for (blocked, oracle) in [(false, [6208., 2752.]), (true, [5952., 2432.])] {
        if blocked {
            for (&handle, point) in workers.iter().zip(blocked_sample) {
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
        let worker_point = objects.executed_unit(worker).unwrap().position.unwrap();
        println!("probe-sampled first worker {worker_point:?}, collision radius {worker_radius}");
        let origins = [
            ("mine", center),
            ("start", start_point),
            ("probe-worker", worker_point),
            ("route-worker", route_worker),
        ];
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
        let placement = objects.diagnostic_placement().unwrap();
        let bounds = placement.world_bounds();
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
                grid.search(
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
                    |_| Ok(true),
                    |p| {
                        let valid = placement
                            .borrow_mut()
                            .diagnostic_construction_position(p, facing, path, 0x08, 0)
                            .map_err(|e| e.to_string())?
                            .is_some();
                        if valid && first_valid.get().is_none() {
                            first_valid.set(Some((p, rank.get())));
                        }
                        if valid
                            && first_same_score_valid.get().is_none()
                            && grid.diagnostic_cell(p).map(|(_, score)| score) == oracle_score
                        {
                            first_same_score_valid.set(Some((p, rank.get())));
                        }
                        Ok(false)
                    },
                )
                .unwrap();
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
                        |p| {
                            placement
                                .borrow_mut()
                                .diagnostic_construction_position(p, facing, path, 0x08, 0)
                                .map(|v| v.is_some())
                                .map_err(|e| e.to_string())
                        },
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
                    let oracle_valid = placement
                        .borrow_mut()
                        .diagnostic_construction_position(oracle, facing, path, 0x08, 0)
                        .unwrap()
                        .is_some();
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
}
