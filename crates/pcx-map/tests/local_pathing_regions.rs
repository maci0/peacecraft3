//! Explicit original-fixture import. Never stores extracted original members.
use pcx_map::Map;
use pcx_mpq::MpqSet;

#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_INSTALL pointing to a local original install"]
fn original_map_pathing_and_regions_are_consumed_exactly() {
    let install =
        std::env::var_os("PEACECRAFT3_FORMAT_INSTALL").expect("set PEACECRAFT3_FORMAT_INSTALL");
    let mut archives = MpqSet::open_wc3(&install).expect("open original install");
    let paths = archives.file_list().expect("original listfile");
    let mut counts = [0usize; 3];
    for path in paths.iter().filter(|path| {
        let name = path.to_ascii_lowercase();
        name.ends_with(".w3m") || name.ends_with(".w3x")
    }) {
        let data = archives
            .extract(path)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        check_map(path, &data, &mut counts);
    }
    let mut directories = vec![std::path::PathBuf::from(&install).join("Maps")];
    while let Some(directory) = directories.pop() {
        if !directory.exists() {
            continue;
        }
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file() {
                let path = entry.path();
                let name = path.to_string_lossy();
                if name.to_ascii_lowercase().ends_with(".w3m")
                    || name.to_ascii_lowercase().ends_with(".w3x")
                {
                    check_map(&name, &std::fs::read(&path).unwrap(), &mut counts);
                }
            }
        }
    }
    let [maps, cells, regions] = counts;
    assert!(
        maps > 0 && cells > 0 && regions > 0,
        "original fixtures must exercise all three"
    );
    eprintln!("install={install:?} maps={maps} pathing_cells={cells} regions={regions}");
}

fn check_map(path: &str, data: &[u8], counts: &mut [usize; 3]) {
    let map = Map::parse(data).unwrap_or_else(|error| panic!("{path}: {error}"));
    map.info()
        .unwrap_or_else(|error| panic!("{path}: map info: {error}"));
    let pathing = map
        .pathing()
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    let records = map
        .regions()
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    let doodads = map
        .doodads()
        .unwrap_or_else(|error| panic!("{path}: doodads: {error}"));
    assert_eq!(
        doodads.canonical_bytes(),
        doodads.raw(),
        "{path}: doodad round trip"
    );
    let units = map
        .units()
        .unwrap_or_else(|error| panic!("{path}: units: {error}"));
    for unit in &units.instances {
        if let pcx_map::RandomUnit::Unclassified(kind) = unit.random {
            eprintln!(
                "{path}: unit {} unclassified native random word {kind}",
                unit.creation_number
            );
        }
    }
    assert_eq!(
        units.canonical_bytes(),
        map.extract("war3mapUnits.doo").unwrap(),
        "{path}: unit round trip"
    );
    counts[0] += 1;
    counts[1] += pathing.cells.len();
    counts[2] += records.regions.len();
}
