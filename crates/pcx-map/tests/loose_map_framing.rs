//! Opens every loose map under a local install and reports what opened and what
//! was refused, grouped by reason. Reads real assets, so it is opt-in.
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Map-level refusals and archive-level refusals share one report. Digits are
/// folded to `#` so that one refusal rule with different sizes is one line.
fn reason_key(message: &str) -> String {
    message
        .chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect()
}

fn loose_maps(maps_dir: PathBuf) -> Vec<PathBuf> {
    let mut pending = vec![maps_dir];
    let mut maps = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries =
            std::fs::read_dir(&directory).unwrap_or_else(|error| panic!("{directory:?}: {error}"));
        for entry in entries {
            let entry = entry.unwrap_or_else(|error| panic!("{directory:?}: {error}"));
            let path = entry.path();
            let kind = entry
                .file_type()
                .unwrap_or_else(|error| panic!("{path:?}: {error}"));
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file()
                && path.extension().is_some_and(|extension| {
                    ["w3m", "w3x", "w3n"]
                        .iter()
                        .any(|map| extension.eq_ignore_ascii_case(map))
                })
            {
                maps.push(path);
            }
        }
    }
    maps.sort();
    maps
}

#[test]
#[ignore = "reads real assets: set PEACECRAFT3_INSTALL to a Warcraft III install root that holds Maps/"]
fn loose_maps_open_or_refuse_with_reasons() {
    let install = std::env::var_os("PEACECRAFT3_INSTALL").expect("PEACECRAFT3_INSTALL is not set");
    let maps = loose_maps(PathBuf::from(install).join("Maps"));
    assert!(
        !maps.is_empty(),
        "no loose maps under the supplied Maps tree"
    );

    let mut opened = 0usize;
    let mut refused = BTreeMap::<String, Vec<PathBuf>>::new();
    for path in &maps {
        let refusal = match std::fs::read(path) {
            Err(error) => Some(format!("unreadable: {error}")),
            Ok(data) => match pcx_map::Map::parse(&data) {
                Err(error) => Some(format!("map: {error}")),
                Ok(map) => map
                    .open_archive()
                    .err()
                    .map(|error| format!("archive: {error}")),
            },
        };
        match refusal {
            None => opened += 1,
            Some(reason) => refused
                .entry(reason_key(&reason))
                .or_default()
                .push(path.clone()),
        }
    }

    eprintln!(
        "loose maps: {} opened, {} refused",
        opened,
        maps.len() - opened
    );
    for (reason, files) in &refused {
        eprintln!("  {} x {reason}", files.len());
        for file in files {
            eprintln!("      {}", file.display());
        }
    }
}
