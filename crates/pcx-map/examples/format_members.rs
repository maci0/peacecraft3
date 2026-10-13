//! Local map-member census; imports stay in memory, only framing summaries print.
use pcx_map::Map;
use pcx_mpq::MpqSet;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let install = std::env::var_os("PEACECRAFT3_FORMAT_INSTALL")
        .ok_or("set PEACECRAFT3_FORMAT_INSTALL to the local original install")?;
    let mut set = MpqSet::open_wc3(&install)?;
    let paths = set.file_list()?;
    let mut census: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut maps = 0;
    for path in paths.iter().filter(|path| is_map(path)) {
        let bytes = set.extract(path)?;
        inspect_map(&bytes, &mut census)?;
        maps += 1;
    }
    let mut directories = vec![std::path::PathBuf::from(&install).join("Maps")];
    while let Some(directory) = directories.pop() {
        if !directory.exists() {
            continue;
        }
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file() && is_map(&entry.path().to_string_lossy()) {
                let bytes = std::fs::read(entry.path())?;
                inspect_map(&bytes, &mut census)
                    .map_err(|error| format!("{}: {error}", entry.path().display()))?;
                maps += 1;
            }
        }
    }
    println!("install={install:?} maps={maps}");
    for (name, headers) in census {
        println!("{name}: {headers:?}");
    }
    Ok(())
}

fn inspect_map(
    bytes: &[u8],
    census: &mut BTreeMap<String, BTreeMap<String, usize>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let map = Map::parse(bytes)?;
    let mut archive = map.open_archive()?;
    for member in [
        "war3map.wpm",
        "war3map.w3r",
        "war3mapUnits.doo",
        "war3map.doo",
        "war3map.w3i",
        "war3map.w3a",
        "war3map.w3d",
        "war3map.w3b",
        "war3map.w3h",
        "war3map.w3q",
        "war3map.wpmp",
    ] {
        if !archive.contains(member) {
            continue;
        }
        let data = archive.extract(member)?;
        let header = data
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        *census
            .entry(member.to_owned())
            .or_default()
            .entry(header)
            .or_default() += 1;
    }
    Ok(())
}

fn is_map(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    path.ends_with(".w3m") || path.ends_with(".w3x")
}
