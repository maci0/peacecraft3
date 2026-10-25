use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use pcx_assets::{
    Addon, ArchiveLayer, AssetMode, AssetPath, Behaviour, Build, ContentSource, Error, GameMode,
    LayerKind, ModeAxis, ModeError, OverrideDirectory, Resolver, SourceContent, SourceError,
    SourceInfo,
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pcx-assets-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, path: &str, bytes: &[u8]) {
        let target = self.0.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    fn mount(&self) -> OverrideDirectory {
        OverrideDirectory::open(&self.0, "user", None).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Clone, Copy)]
enum Fault {
    Inventory,
    Read,
}
struct MemorySource {
    info: SourceInfo,
    entries: Vec<(String, Option<SourceContent>)>,
    reads: Arc<Mutex<Vec<String>>>,
    fault: Option<Fault>,
}
impl MemorySource {
    fn new(id: &str, entries: &[(&str, &[u8])]) -> Self {
        Self {
            info: SourceInfo {
                id: id.into(),
                kind: LayerKind::Mpq,
                origin: format!("{id}.mpq"),
                version: Some("synthetic version".into()),
            },
            entries: entries
                .iter()
                .map(|(name, bytes)| {
                    (
                        name.to_string(),
                        Some(SourceContent {
                            bytes: bytes.to_vec(),
                            declared_length: bytes.len() as u64,
                            locator: format!("{id}::{name}"),
                        }),
                    )
                })
                .collect(),
            reads: Arc::new(Mutex::new(Vec::new())),
            fault: None,
        }
    }
    fn layer(self, priority: u32) -> ArchiveLayer {
        ArchiveLayer {
            priority,
            source: Box::new(self),
        }
    }
}
impl ContentSource for MemorySource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }
    fn paths(&self) -> Result<Vec<String>, SourceError> {
        if matches!(self.fault, Some(Fault::Inventory)) {
            return Err(SourceError::Unavailable {
                detail: "synthetic missing archive".into(),
            });
        }
        Ok(self.entries.iter().map(|(name, _)| name.clone()).collect())
    }
    fn read(&self, member: &str) -> Result<Option<SourceContent>, SourceError> {
        self.reads.lock().unwrap().push(member.into());
        if matches!(self.fault, Some(Fault::Read)) {
            return Err(SourceError::InvalidInput {
                detail: "synthetic damaged input".into(),
            });
        }
        Ok(self
            .entries
            .iter()
            .find(|(name, _)| name == member)
            .and_then(|(_, content)| content.clone()))
    }
    fn read_bounded(&self, member: &str, limit: u64) -> Result<Option<SourceContent>, SourceError> {
        if let Some((_, Some(content))) = self.entries.iter().find(|(name, _)| name == member)
            && content.declared_length > limit
        {
            return Err(SourceError::Limit {
                requested: content.declared_length,
                limit,
            });
        }
        self.read(member)
    }
}

#[test]
fn preload_reuses_the_same_archive_winner_and_preserves_read_budgets_and_live_overrides() {
    let low = MemorySource::new("low", &[("Art/Model.mdx", b"wrong")]);
    let high = MemorySource::new("high", &[("Art/Model.mdx", b"selected")]);
    let reads = Arc::clone(&high.reads);
    let mode = "1.29b-tft-vanilla-vanilla".parse().unwrap();
    let reader = Resolver::new(mode, None, vec![low.layer(1), high.layer(2)]).unwrap();
    assert_eq!(reader.preload_bounded("art/model.MDX", 8).unwrap(), 8);
    assert_eq!(reader.preload_cache_usage().unwrap(), (1, 8));
    let loaded = reader.resolve_bounded("Art\\Model.mdx", 8).unwrap();
    assert_eq!(loaded.bytes, b"selected");
    assert_eq!(loaded.provenance.source.id, "high");
    assert_eq!(reader.preload_bounded("art/model.mdx", 8).unwrap(), 8);
    assert_eq!(reads.lock().unwrap().len(), 1);
    assert!(reader.resolve_bounded("art/model.mdx", 7).is_err());
    assert!(reader.preload_bounded("../model.mdx", 8).is_err());
    assert!(reader.preload_bounded("absent.mdx", 8).is_err());
    assert_eq!(reads.lock().unwrap().len(), 1);
    let directory = Directory::new();
    directory.put("live.blp", b"first");
    let reader = Resolver::new(mode, Some(directory.mount()), Vec::new()).unwrap();
    reader.preload_bounded("live.blp", 8).unwrap();
    directory.put("live.blp", b"later");
    assert_eq!(
        reader.resolve_bounded("live.blp", 8).unwrap().bytes,
        b"later"
    );
}

#[test]
fn all_mode_axes_round_trip_independently() {
    for build in ["1.29b", "1.28c"] {
        for addon in ["tft", "noaddon"] {
            for assets in ["vanilla", "hd", "reforged"] {
                for behaviour in ["vanilla", "reforged", "fixes"] {
                    let text = format!("{build}-{addon}-{assets}-{behaviour}");
                    let mode: GameMode = text.parse().unwrap();
                    assert_eq!(mode.to_string(), text);
                    assert_eq!(
                        mode.is_fidelity_baseline(),
                        assets == "vanilla" && behaviour == "vanilla"
                    );
                }
            }
        }
    }
    let hd: GameMode = "1.29b-tft-hd-vanilla".parse().unwrap();
    assert_eq!(hd.assets, AssetMode::Hd);
    assert_eq!(hd.behaviour, Behaviour::Vanilla);
    let fixes: GameMode = "1.28c-noaddon-hd-fixes".parse().unwrap();
    assert_eq!(fixes.build, Build::V1_28c);
    assert_eq!(fixes.addon, Addon::NoAddon);
    assert_eq!(fixes.assets, AssetMode::Hd);
    assert_eq!(fixes.behaviour, Behaviour::Fixes);
    assert_eq!(GameMode::default().to_string(), "1.29b-tft-vanilla-vanilla");
    assert_eq!(AssetMode::default(), AssetMode::Vanilla);
    assert_eq!(Behaviour::default(), Behaviour::Vanilla);
    assert!(GameMode::default().is_fidelity_baseline());
}

#[test]
fn malformed_modes_have_typed_axis_or_shape_failures() {
    for (text, axis) in [
        ("unknown-tft-hd-vanilla", ModeAxis::Build),
        ("1.29b-roc-hd-vanilla", ModeAxis::Addon),
        ("1.29b-tft-upscaled-vanilla", ModeAxis::Assets),
        ("1.29b-tft-hd-fast", ModeAxis::Behaviour),
        ("1.29b-tft-hd-", ModeAxis::Behaviour),
    ] {
        assert!(
            matches!(text.parse::<GameMode>(), Err(ModeError::Component { axis: actual, .. }) if actual == axis)
        );
    }
    for text in [
        "",
        "1.29b",
        "1.29b-tft-hd",
        "1.29b-tft-hd-vanilla-extra",
        "1.29b+tft+hd+vanilla",
    ] {
        assert!(matches!(
            text.parse::<GameMode>(),
            Err(ModeError::Shape { .. })
        ));
    }
}

#[test]
fn normalized_paths_reject_traversal_and_preserve_non_ascii() {
    assert_eq!(
        AssetPath::new("Textures\\Some.BLP").unwrap().as_str(),
        "textures/some.blp"
    );
    assert_eq!(AssetPath::new("Ä/FOO").unwrap().as_str(), "Ä/foo");
    for path in [
        "",
        "/root/file",
        "\\server\\file",
        "C:\\file",
        "../file",
        "a/../b",
        "a/./b",
        "a//b",
        "a/",
        "a\0b",
        "a\nb",
    ] {
        assert!(AssetPath::new(path).is_err(), "{path:?}");
    }
}

#[test]
fn override_wins_and_archives_follow_explicit_priorities() {
    let directory = Directory::new();
    directory.put("Textures/Some.BLP", b"synthetic user bytes");
    let high = MemorySource::new(
        "arbitrary-name",
        &[("Textures\\Some.BLP", b"archive"), ("Shared", b"high")],
    );
    let high_reads = high.reads.clone();
    let low = MemorySource::new(
        "not-assumed-patch",
        &[("shared", b"low"), ("Only\\Low", b"base")],
    );
    let low_reads = low.reads.clone();
    let resolver = Resolver::new(
        GameMode::default(),
        Some(directory.mount()),
        vec![low.layer(0), high.layer(73)],
    )
    .unwrap();
    assert_eq!(resolver.source_for("TEXTURES/some.blp").unwrap().id, "user");
    assert_eq!(resolver.source_for("SHARED").unwrap().id, "arbitrary-name");
    assert_eq!(
        resolver.source_for("only/low").unwrap().id,
        "not-assumed-patch"
    );
    assert!(matches!(
        resolver.source_for("absent"),
        Err(Error::MissingAsset { .. })
    ));
    assert!(matches!(
        resolver.source_for("../escape"),
        Err(Error::Path(_))
    ));
    assert!(high_reads.lock().unwrap().is_empty());
    assert!(low_reads.lock().unwrap().is_empty());
    let override_asset = resolver.resolve("textures/some.blp").unwrap();
    assert_eq!(override_asset.bytes, b"synthetic user bytes");
    assert_eq!(override_asset.provenance.source.id, "user");
    assert_eq!(override_asset.provenance.source.kind, LayerKind::Override);
    assert_eq!(override_asset.provenance.source.version, None);
    assert_eq!(override_asset.provenance.archive_priority, None);
    assert_eq!(
        Path::new(&override_asset.provenance.locator),
        directory.0.join("Textures/Some.BLP")
    );
    assert!(high_reads.lock().unwrap().is_empty());
    assert!(low_reads.lock().unwrap().is_empty());
    let shared = resolver.resolve("SHARED").unwrap();
    assert_eq!(shared.bytes, b"high");
    assert_eq!(shared.provenance.source.id, "arbitrary-name");
    assert_eq!(shared.provenance.archive_priority, Some(73));
    assert_eq!(
        shared.provenance.source.version.as_deref(),
        Some("synthetic version")
    );
    assert_eq!(resolver.resolve("only/low").unwrap().bytes, b"base");
    assert_eq!(*low_reads.lock().unwrap(), vec!["Only\\Low"]);
}

#[test]
fn changing_art_does_not_change_archive_order_or_behaviour() {
    for art in ["vanilla", "hd", "reforged"] {
        let mode: GameMode = format!("1.29b-tft-{art}-vanilla").parse().unwrap();
        let mut high = MemorySource::new("chosen-high", &[("model", b"caller-chosen")]);
        high.info.kind = LayerKind::Casc;
        let resolver = Resolver::new(
            mode,
            None,
            vec![
                high.layer(9),
                MemorySource::new("chosen-low", &[("model", b"low")]).layer(1),
            ],
        )
        .unwrap();
        assert_eq!(resolver.resolve("model").unwrap().bytes, b"caller-chosen");
        assert_eq!(resolver.mode().behaviour, Behaviour::Vanilla);
    }
}

#[test]
fn index_json_records_every_winner_deterministically_without_contents() {
    let directory = Directory::new();
    directory.put("nested/Z.BLP", b"private synthetic contents");
    directory.put("index.json", b"old generated metadata");
    let mut archive = MemorySource::new(
        "archive",
        &[("b", b"two"), ("A", b"one"), ("nested/z.blp", b"shadowed")],
    );
    archive.info.origin = "synthetic \"origin\"\\path\nversion-source".into();
    archive.entries[1].1.as_mut().unwrap().locator = "literal \"member\"\\with\ntokens".into();
    let mode: GameMode = "1.29b-tft-hd-vanilla".parse().unwrap();
    let resolver = Resolver::new(mode, Some(directory.mount()), vec![archive.layer(4)]).unwrap();
    let index = resolver.index().unwrap();
    let json = index.to_json().unwrap();
    assert_eq!(json, resolver.index().unwrap().to_json().unwrap());
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["mode"], "1.29b-tft-hd-vanilla");
    assert_eq!(value["schema_version"], 1);
    let assets = value["assets"].as_object().unwrap();
    assert_eq!(assets.len(), 3);
    assert_eq!(assets["nested/z.blp"]["source"]["id"], "user");
    assert!(assets["nested/z.blp"]["source"]["version"].is_null());
    assert_eq!(
        assets["a"]["source"]["origin"],
        "synthetic \"origin\"\\path\nversion-source"
    );
    assert_eq!(assets["a"]["locator"], "literal \"member\"\\with\ntokens");
    assert_eq!(assets["a"]["byte_length"], 3);
    assert_eq!(assets["a"]["archive_priority"], 4);
    assert!(!json.contains("private synthetic contents"));
    let destination = resolver.write_index().unwrap();
    assert_eq!(destination, directory.0.join("index.json"));
    assert_eq!(fs::read_to_string(destination).unwrap(), json);
    assert!(!fs::read_dir(&directory.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_str()
            .unwrap()
            .starts_with(".pcx-assets-index-")
    }));
    let remounted = directory.mount();
    assert_eq!(
        remounted.paths().count(),
        1,
        "generated index must never become an override asset"
    );
}

#[test]
fn empty_payloads_and_empty_override_trees_are_real_inputs() {
    let directory = Directory::new();
    let resolver = Resolver::new(GameMode::default(), Some(directory.mount()), vec![]).unwrap();
    assert!(resolver.index().unwrap().assets.is_empty());
    resolver.write_index().unwrap();
    directory.put("zero", b"");
    let resolver = Resolver::new(GameMode::default(), Some(directory.mount()), vec![]).unwrap();
    let asset = resolver.resolve("zero").unwrap();
    assert!(asset.bytes.is_empty());
    assert_eq!(asset.provenance.byte_length, 0);
}

#[test]
fn unknown_missing_and_invalid_layers_are_typed() {
    assert!(matches!(
        "zip".parse::<LayerKind>(),
        Err(Error::UnknownLayerKind { .. })
    ));
    assert_eq!("mpq".parse::<LayerKind>().unwrap(), LayerKind::Mpq);
    assert_eq!("casc".parse::<LayerKind>().unwrap(), LayerKind::Casc);
    assert_eq!(
        "override".parse::<LayerKind>().unwrap(),
        LayerKind::Override
    );
    assert!(matches!(
        Resolver::new(GameMode::default(), None, vec![]),
        Err(Error::NoLayers)
    ));
    let directory = Directory::new();
    assert!(matches!(
        OverrideDirectory::open(directory.0.join("missing"), "user", None),
        Err(Error::MissingLayer { .. })
    ));
    directory.put("regular-file", b"x");
    assert!(matches!(
        OverrideDirectory::open(directory.0.join("regular-file"), "user", None),
        Err(Error::InvalidOverrideRoot { .. })
    ));
    let resolver = Resolver::new(
        GameMode::default(),
        None,
        vec![MemorySource::new("known", &[]).layer(1)],
    )
    .unwrap();
    assert!(matches!(
        resolver.layer_info("unknown"),
        Err(Error::UnknownLayer { .. })
    ));
    assert_eq!(resolver.layer_info("known").unwrap().id, "known");
    assert!(matches!(
        resolver.resolve("missing"),
        Err(Error::MissingAsset { .. })
    ));
    assert!(matches!(
        resolver.write_index(),
        Err(Error::MissingLayer { .. })
    ));
    let mut archive = MemorySource::new("wrong-kind", &[]);
    archive.info.kind = LayerKind::Override;
    assert!(matches!(
        Resolver::new(GameMode::default(), None, vec![archive.layer(1)]),
        Err(Error::InvalidArchiveKind { .. })
    ));
    for field in ["id", "origin", "version"] {
        let mut archive = MemorySource::new("invalid", &[]);
        match field {
            "id" => archive.info.id.clear(),
            "origin" => archive.info.origin.clear(),
            _ => archive.info.version = Some("".into()),
        }
        assert!(matches!(
            Resolver::new(GameMode::default(), None, vec![archive.layer(1)]),
            Err(Error::InvalidIdentity { .. })
        ));
    }
}

#[test]
fn ambiguous_layer_ids_priorities_and_member_aliases_are_refused() {
    assert!(matches!(
        Resolver::new(
            GameMode::default(),
            None,
            vec![
                MemorySource::new("same", &[]).layer(1),
                MemorySource::new("same", &[]).layer(2),
            ]
        ),
        Err(Error::AmbiguousLayer { .. })
    ));
    assert!(matches!(
        Resolver::new(
            GameMode::default(),
            None,
            vec![
                MemorySource::new("one", &[]).layer(3),
                MemorySource::new("two", &[]).layer(3),
            ]
        ),
        Err(Error::AmbiguousPriority { priority: 3, .. })
    ));
    assert!(matches!(
        Resolver::new(
            GameMode::default(),
            None,
            vec![MemorySource::new("aliases", &[("A\\B", b"one"), ("a/b", b"two")]).layer(1),]
        ),
        Err(Error::AmbiguousPath { .. })
    ));
    let directory = Directory::new();
    assert!(matches!(
        Resolver::new(
            GameMode::default(),
            Some(directory.mount()),
            vec![MemorySource::new("user", &[]).layer(1)]
        ),
        Err(Error::AmbiguousLayer { .. })
    ));
}

#[test]
fn invalid_member_paths_keep_layer_context() {
    assert!(matches!(Resolver::new(GameMode::default(), None, vec![
        MemorySource::new("invalid-member", &[("../outside", b"x")]).layer(1),
    ]), Err(Error::LayerPath { layer, .. }) if layer == "invalid-member"));
}

#[test]
fn failed_inventory_is_not_an_empty_archive() {
    let mut source = MemorySource::new("offline", &[]);
    source.fault = Some(Fault::Inventory);
    assert!(
        matches!(Resolver::new(GameMode::default(), None, vec![source.layer(1)]), Err(Error::Source {
        layer, path: None, source: SourceError::Unavailable { .. },
    }) if layer == "offline")
    );
}

#[test]
fn malformed_or_missing_winners_never_fall_back() {
    for fault in [None, Some(Fault::Read)] {
        let mut high = MemorySource::new("broken-high", &[("a", b"x")]);
        high.fault = fault;
        if high.fault.is_none() {
            high.entries[0].1 = None;
        }
        let low = MemorySource::new("good-low", &[("a", b"fallback")]);
        let reads = low.reads.clone();
        let resolver =
            Resolver::new(GameMode::default(), None, vec![high.layer(2), low.layer(1)]).unwrap();
        assert!(matches!(
            resolver.resolve("a"),
            Err(Error::MissingContent { .. } | Error::Source { .. })
        ));
        assert!(resolver.index().is_err());
        assert!(reads.lock().unwrap().is_empty());
    }
}

#[test]
fn truncated_and_overlong_adapter_reads_are_refused() {
    for (declared, truncated) in [(5, true), (1, false)] {
        let mut source = MemorySource::new("sized", &[("a", b"abc")]);
        source.entries[0].1.as_mut().unwrap().declared_length = declared;
        let resolver = Resolver::new(GameMode::default(), None, vec![source.layer(1)]).unwrap();
        let error = resolver.resolve("a").unwrap_err();
        if truncated {
            assert!(matches!(
                error,
                Error::TruncatedInput {
                    expected: 5,
                    actual: 3,
                    ..
                }
            ));
        } else {
            assert!(matches!(
                error,
                Error::LengthMismatch {
                    expected: 1,
                    actual: 3,
                    ..
                }
            ));
        }
    }
    let mut source = MemorySource::new("missing-locator", &[("a", b"abc")]);
    source.entries[0].1.as_mut().unwrap().locator.clear();
    let resolver = Resolver::new(GameMode::default(), None, vec![source.layer(1)]).unwrap();
    assert!(matches!(
        resolver.resolve("a"),
        Err(Error::InvalidIdentity {
            field: "content locator",
            ..
        })
    ));
}

#[test]
fn changed_override_lengths_do_not_truncate_or_fall_back() {
    for bytes in [b"ab".as_slice(), b"abcde".as_slice()] {
        let directory = Directory::new();
        directory.put("a", b"abcd");
        let low = MemorySource::new("lower", &[("a", b"fallback")]);
        let reads = low.reads.clone();
        let resolver = Resolver::new(
            GameMode::default(),
            Some(directory.mount()),
            vec![low.layer(1)],
        )
        .unwrap();
        directory.put("a", bytes);
        assert!(matches!(
            resolver.resolve("a"),
            Err(Error::TruncatedInput { .. } | Error::LengthMismatch { .. })
        ));
        assert!(reads.lock().unwrap().is_empty());
    }
}

#[test]
fn failed_index_generation_keeps_the_previous_index() {
    let directory = Directory::new();
    directory.put("index.json", b"prior complete index");
    let mut source = MemorySource::new("broken", &[("a", b"a")]);
    source.entries[0].1.as_mut().unwrap().declared_length = 7;
    let resolver = Resolver::new(
        GameMode::default(),
        Some(directory.mount()),
        vec![source.layer(1)],
    )
    .unwrap();
    assert!(matches!(
        resolver.write_index(),
        Err(Error::TruncatedInput { .. })
    ));
    assert_eq!(
        fs::read(directory.0.join("index.json")).unwrap(),
        b"prior complete index"
    );
}

#[test]
fn override_reads_are_live_but_inventory_requires_remounting() {
    let directory = Directory::new();
    directory.put("a", b"first");
    let resolver = Resolver::new(GameMode::default(), Some(directory.mount()), vec![]).unwrap();
    directory.put("a", b"later");
    directory.put("new", b"new fixture");
    assert_eq!(resolver.resolve("a").unwrap().bytes, b"later");
    assert!(matches!(
        resolver.resolve("new"),
        Err(Error::MissingAsset { .. })
    ));
    fs::remove_file(directory.0.join("a")).unwrap();
    assert!(matches!(resolver.resolve("a"), Err(Error::Io { .. })));
    let remounted = Resolver::new(GameMode::default(), Some(directory.mount()), vec![]).unwrap();
    assert_eq!(remounted.resolve("new").unwrap().bytes, b"new fixture");
}

#[cfg(unix)]
#[test]
fn reserved_index_aliases_and_replaced_root_are_refused() {
    use std::os::unix::fs::symlink;
    let aliases = Directory::new();
    aliases.put("INDEX.JSON", b"alias fixture");
    assert!(matches!(
        OverrideDirectory::open(&aliases.0, "user", None),
        Err(Error::ReservedPath { .. })
    ));

    let directory = Directory::new();
    directory.put("root/asset", b"fixture");
    let root = directory.0.join("root");
    let overrides = OverrideDirectory::open(&root, "user", None).unwrap();
    let resolver = Resolver::new(GameMode::default(), Some(overrides), vec![]).unwrap();
    let outside = Directory::new();
    outside.put("asset", b"fixture");
    fs::rename(&root, directory.0.join("old-root")).unwrap();
    symlink(&outside.0, &root).unwrap();
    assert!(matches!(
        resolver.resolve("asset"),
        Err(Error::UnsafeEntry { .. })
    ));
    assert!(matches!(
        resolver.write_index(),
        Err(Error::UnsafeEntry { .. })
    ));
}

#[cfg(unix)]
#[test]
fn filesystem_aliases_non_utf8_and_symlinks_are_refused() {
    use std::{
        ffi::OsString,
        os::unix::{ffi::OsStringExt, fs::symlink},
    };
    let aliases = Directory::new();
    aliases.put("A/One", b"one");
    aliases.put("a/one", b"two");
    assert!(matches!(
        OverrideDirectory::open(&aliases.0, "user", None),
        Err(Error::AmbiguousPath { .. })
    ));
    let encoding = Directory::new();
    fs::write(
        encoding.0.join(OsString::from_vec(vec![0xff])),
        b"synthetic",
    )
    .unwrap();
    assert!(matches!(
        OverrideDirectory::open(&encoding.0, "user", None),
        Err(Error::PathEncoding { .. })
    ));
    let links = Directory::new();
    let outside = Directory::new();
    outside.put("original", b"outside fixture");
    symlink(outside.0.join("original"), links.0.join("linked")).unwrap();
    assert!(matches!(
        OverrideDirectory::open(&links.0, "user", None),
        Err(Error::UnsafeEntry { .. })
    ));
}

#[cfg(unix)]
#[test]
fn replaced_override_ancestor_and_index_symlink_are_refused() {
    use std::os::unix::fs::symlink;
    let directory = Directory::new();
    directory.put("nested/a", b"inside");
    let resolver = Resolver::new(GameMode::default(), Some(directory.mount()), vec![]).unwrap();
    let outside = Directory::new();
    outside.put("a", b"outside");
    fs::rename(directory.0.join("nested"), directory.0.join("old")).unwrap();
    symlink(&outside.0, directory.0.join("nested")).unwrap();
    assert!(matches!(
        resolver.resolve("nested/a"),
        Err(Error::UnsafeEntry { .. })
    ));

    let index_directory = Directory::new();
    index_directory.put("asset", b"fixture");
    let resolver =
        Resolver::new(GameMode::default(), Some(index_directory.mount()), vec![]).unwrap();
    outside.put("index-target", b"untouched fixture");
    symlink(
        outside.0.join("index-target"),
        index_directory.0.join("index.json"),
    )
    .unwrap();
    assert!(matches!(
        resolver.write_index(),
        Err(Error::UnsafeEntry { .. })
    ));
    assert_eq!(
        fs::read(outside.0.join("index-target")).unwrap(),
        b"untouched fixture"
    );
}
