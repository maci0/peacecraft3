//! Explicit original sources include archived TFT maps and loose .w3x maps.
use pcx_mpq::{MpqArchive, MpqSet};
use pcx_objects::{EXTENSIONS, MAP_ARCHIVE_OFFSET, ObjectFile};
use std::collections::BTreeMap;

#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_INSTALL containing original archives and Maps"]
fn original_object_families_round_trip_exactly() {
    let install =
        std::env::var_os("PEACECRAFT3_FORMAT_INSTALL").expect("set PEACECRAFT3_FORMAT_INSTALL");
    let mut counts = BTreeMap::new();
    let mut set = MpqSet::open_wc3(&install).unwrap();
    for path in set.file_list().unwrap().iter().filter(|path| is_map(path)) {
        inspect_objects(&set.extract(path).unwrap(), path, &mut counts);
    }
    let mut directories = vec![std::path::PathBuf::from(&install).join("Maps")];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).expect("original Maps directory") {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file() && is_map(&entry.path().to_string_lossy()) {
                let path = entry.path();
                inspect_objects(
                    &std::fs::read(&path).unwrap(),
                    &path.to_string_lossy(),
                    &mut counts,
                );
            }
        }
    }
    for extension in EXTENSIONS {
        assert!(
            counts.get(extension).is_some_and(|count| *count > 0),
            "fixture lacks {extension}"
        );
    }
    eprintln!("install={install:?} original_object_files={counts:?}");
}

fn is_map(path: &str) -> bool {
    let name = path.to_ascii_lowercase();
    name.ends_with(".w3m") || name.ends_with(".w3x")
}

fn inspect_objects(bytes: &[u8], path: &str, counts: &mut BTreeMap<&'static str, usize>) {
    let archive = bytes
        .get(MAP_ARCHIVE_OFFSET..)
        .expect("embedded original MPQ");
    let mut archive = MpqArchive::from_bytes(archive.to_vec(), path).unwrap();
    for extension in EXTENSIONS {
        let member = format!("war3map.{extension}");
        if !archive.contains(&member) {
            continue;
        }
        let data = archive.extract(&member).unwrap();
        let parsed = ObjectFile::parse(&member, &data)
            .unwrap_or_else(|error| panic!("{path} {member}: {error}"));
        assert_eq!(parsed.canonical_bytes(), data, "{path} {member}");
        *counts.entry(extension).or_insert(0) += 1;
    }
}
