//! Game-mode selection and asset staging.
//!
//! A "mode" is the full description of *which* Warcraft III you are playing:
//! which classic build, whether the Frozen Throne addon is enabled, where the
//! textures come from, and which behaviour set runs. Those axes are independent,
//! so they are separate fields rather than one enum — `vanilla + reforged-hd`
//! is a mode, and so is `1.29b + tft + reforged behaviour`.
//!
//! The launcher's job is to turn a mode plus a source (a directory you already
//! extracted, an ISO, or a Battle.net folder) into a staged asset tree, and to
//! verify the result by opening it and counting what landed. It copies rather
//! than symlinks or mounts, because the game itself reads from a plain
//! directory and because ISO extraction has to land somewhere anyway.

use std::path::{Path, PathBuf};

use pcx_mpq::{Install, MpqSet, Version};

/// Which retail build the run is faithful to.
pub use pcx_mpq::Version as Build;

/// Whether the Frozen Throne addon is enabled.
///
/// Reign of Chaos alone and The Frozen Throne are different games: different
/// units, abilities, tilesets and campaigns. So this is a first-class setting,
/// not a detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Addon {
    /// Reign of Chaos only. TFT archives are not staged.
    Roc,
    /// The Frozen Throne, the default for anything built after 2003.
    Tft,
}

impl std::fmt::Display for Addon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl Addon {
    pub fn label(self) -> &'static str {
        match self {
            Addon::Roc => "roc",
            Addon::Tft => "tft",
        }
    }
}

/// Where the textures, models and sounds come from.
///
/// This is deliberately separate from [`Behaviour`]: Reforged's art is a
/// presentation choice and its gameplay changes are not, and conflating them
/// makes the §6 fidelity gate unmeasurable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetMode {
    /// The classic archives exactly as shipped. The fidelity baseline.
    Vanilla,
    /// Classic models and data with Reforged HD textures where they exist.
    VanillaReforgedHd,
    /// Reforged art wholesale.
    Reforged,
}

impl std::fmt::Display for AssetMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl AssetMode {
    pub fn label(self) -> &'static str {
        match self {
            AssetMode::Vanilla => "vanilla",
            AssetMode::VanillaReforgedHd => "vanilla+reforged-hd",
            AssetMode::Reforged => "reforged",
        }
    }
}

/// Which behaviour set the simulation runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behaviour {
    /// The classic build's behaviour, bugs included.
    Vanilla,
    /// Reforged behaviour. An opt-in deviation.
    Reforged,
    /// Only the fixes marked `adopt` in [`crate::fixes`] applied.
    ReforgedFixes,
}

impl std::fmt::Display for Behaviour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl Behaviour {
    pub fn label(self) -> &'static str {
        match self {
            Behaviour::Vanilla => "vanilla",
            Behaviour::Reforged => "reforged",
            Behaviour::ReforgedFixes => "reforged-fixes",
        }
    }
}

/// A complete description of which Warcraft III to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameMode {
    pub build: Build,
    pub addon: Addon,
    pub assets: AssetMode,
    pub behaviour: Behaviour,
}

impl GameMode {
    /// The fidelity baseline: 1.29b with the addon, classic art, classic
    /// behaviour.
    pub fn default_for(build: Build) -> Self {
        Self {
            build,
            addon: Addon::Tft,
            assets: AssetMode::Vanilla,
            behaviour: Behaviour::Vanilla,
        }
    }

    /// Stable identifier, used to name a staged tree and to stamp a trace.
    pub fn id(&self) -> String {
        format!(
            "{}-{}-{}-{}",
            self.build, self.addon, self.assets, self.behaviour
        )
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let mut build = None;
        let mut addon = None;
        let mut assets = None;
        let mut behaviour = None;
        for part in s.split(['+', '-']) {
            let p = part.trim().to_ascii_lowercase();
            match p.as_str() {
                "1.28" | "1.28c" | "128" => build = Some(pcx_mpq::Version::V1_28),
                "1.29" | "1.29b" | "129" => build = Some(pcx_mpq::Version::V1_29),
                "tft" | "frozen" | "addon" => addon = Some(Addon::Tft),
                // The addon's own label, so a mode id always round-trips.
                "noaddon" | "classic" | "roc" => addon = Some(Addon::Roc),
                "hd" | "reforgedhd" => {
                    assets = Some(AssetMode::VanillaReforgedHd);
                    behaviour = Some(Behaviour::Vanilla);
                }
                "vanilla" => {
                    if assets.is_none() {
                        assets = Some(AssetMode::Vanilla);
                    }
                    if behaviour.is_none() {
                        behaviour = Some(Behaviour::Vanilla);
                    }
                }
                "reforged" if behaviour.is_none() => behaviour = Some(Behaviour::Reforged),
                "reforged" => assets = Some(AssetMode::Reforged),
                "fixes" => behaviour = Some(Behaviour::ReforgedFixes),
                "" => {}
                other => return Err(format!("unknown mode component {other:?}")),
            }
        }
        Ok(Self {
            build: build.ok_or("missing build (1.28c / 1.29b)")?,
            addon: addon.ok_or("missing addon (tft / noaddon)")?,
            assets: assets.ok_or("missing asset mode (vanilla / hd / reforged)")?,
            behaviour: behaviour.ok_or("missing behaviour (vanilla / reforged / fixes)")?,
        })
    }
}

impl std::fmt::Display for GameMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id())
    }
}

/// An asset source the launcher can stage from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A directory that already holds a WC3 install.
    Directory(PathBuf),
    /// A retail ISO or self-extracting installer.
    Iso(PathBuf),
    /// A Battle.net install folder (contains `Warcraft III*` products).
    BattleNet(PathBuf),
}

impl Source {
    pub fn label(&self) -> &'static str {
        match self {
            Source::Directory(_) => "directory",
            Source::Iso(_) => "iso",
            Source::BattleNet(_) => "battle.net",
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Source::Directory(p) | Source::Iso(p) | Source::BattleNet(p) => p,
        }
    }
}

/// Classifies a path the user gave us.
///
/// A Battle.net folder is recognised by holding more than one `Warcraft III*`
/// directory; an ISO by its extension; anything else that already contains
/// WC3 archives is a plain directory.
pub fn classify(path: impl AsRef<Path>) -> Result<Source, Error> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(Error::NotFound(path.display().to_string()));
    }

    if path.is_dir() {
        let mut products = 0;
        if let Ok(rd) = std::fs::read_dir(path) {
            for e in rd.flatten() {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("Warcraft III") && e.path().is_dir() {
                    products += 1;
                }
            }
        }
        if products >= 1 && path.join("Warcraft III").is_dir() {
            return Ok(Source::BattleNet(path.to_path_buf()));
        }
        if MpqSet::open_wc3(path).is_ok() {
            return Ok(Source::Directory(path.to_path_buf()));
        }
        // A directory that holds product folders but is not itself an install.
        let mut seen = Vec::new();
        if let Ok(rd) = std::fs::read_dir(path) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with("Warcraft III") {
                    seen.push(e.path());
                }
            }
        }
        if !seen.is_empty() {
            return Ok(Source::BattleNet(path.to_path_buf()));
        }
        return Err(Error::NotAnInstall(path.display().to_string()));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if matches!(ext.as_str(), "iso" | "exe" | "cab" | "bin" | "7z" | "zip") {
        return Ok(Source::Iso(path.to_path_buf()));
    }
    Err(Error::NotAnInstall(path.display().to_string()))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0} does not exist")]
    NotFound(String),
    #[error("{0} is not a Warcraft III install or installer")]
    NotAnInstall(String),
    #[error("staging failed: {0}")]
    Stage(String),
}

/// Archives a mode needs, in the order the game reads them.
///
/// 1.29b spells them `War3.mpq` / `War3Local.mpq`; 1.28c spells them `war3.mpq`
/// / `War3xlocal.mpq`. Matching is case-insensitive so one list serves both.
pub fn required_archives(mode: &GameMode) -> Vec<&'static str> {
    let mut v = vec!["war3patch.mpq", "war3x.mpq", "war3.mpq", "deprecated.mpq"];
    if mode.addon == Addon::Tft {
        v.insert(2, "war3xlocal.mpq");
        v.insert(3, "war3local.mpq");
    }
    v
}

/// Result of staging: what landed, and what the staged tree turned out to be.
#[derive(Debug, Clone)]
pub struct Staged {
    pub mode: GameMode,
    pub dir: PathBuf,
    /// Archives copied, by file name.
    pub archives: Vec<String>,
    pub bytes: u64,
    /// Build the staged tree reported once opened.
    pub detected_build: Option<Version>,
    /// Files the staged archives expose.
    pub file_count: usize,
}

/// Stages `mode` from `source` into `dest`.
///
/// The destination is created if missing and **overwritten** if it already
/// holds a tree for the same mode, so re-staging after changing modes cannot
/// leave a mixture of two modes behind.
pub fn stage(mode: &GameMode, source: &Source, dest: impl AsRef<Path>) -> Result<Staged, Error> {
    let dest = dest.as_ref();
    std::fs::create_dir_all(dest)?;

    let src_dir = match source {
        Source::Directory(p) => p.clone(),
        Source::BattleNet(p) => pick_product(p, mode)?,
        Source::Iso(p) => extract_iso(p, dest)?,
    };

    let wanted = required_archives(mode);
    let mut copied = Vec::new();
    let mut bytes = 0u64;
    for name in wanted {
        let Some(from) = find_ci(&src_dir, name) else {
            continue;
        };
        let to = dest.join(
            from.file_name()
                .ok_or_else(|| Error::Stage(format!("{from:?} has no file name")))?,
        );
        std::fs::copy(&from, &to).map_err(|e| Error::Stage(format!("{}: {e}", from.display())))?;
        bytes += std::fs::metadata(&to).map(|m| m.len()).unwrap_or(0);
        copied.push(to.file_name().unwrap().to_string_lossy().into_owned());
    }
    if copied.is_empty() {
        if is_casc_install(&src_dir) {
            return Err(Error::Stage(format!(
                "{} is a CASC install (Reforged), and staging from CASC is not \
                 implemented — it is tracked as RE-Q17. Use a classic MPQ \
                 install, or `vanilla` / `hd` asset modes, which stage from the \
                 1.28c and 1.29b MPQ trees.",
                src_dir.display()
            )));
        }
        return Err(Error::Stage(format!(
            "no WC3 archives found in {}",
            src_dir.display()
        )));
    }

    // Verify by opening what we just wrote rather than trusting the copy.
    let mut install = Install::open(dest).ok();
    let (detected_build, file_count) = match install.as_mut() {
        Some(i) => (Some(i.version), i.file_list().map(|f| f.len()).unwrap_or(0)),
        None => (None, 0),
    };

    Ok(Staged {
        mode: *mode,
        dir: dest.to_path_buf(),
        archives: copied,
        bytes,
        detected_build,
        file_count,
    })
}

/// Finds the install directory for `mode` inside a Battle.net folder.
///
/// The legacy build and the Reforged build live side by side under
/// `Warcraft III` and `Warcraft III (Legacy)`, so which one a mode wants is
/// decided by its asset mode rather than by the user remembering the name.
fn pick_product(root: &Path, mode: &GameMode) -> Result<PathBuf, Error> {
    let want_reforged = mode.assets == AssetMode::Reforged;
    let mut found = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if !n.starts_with("Warcraft III") {
                continue;
            }
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let is_legacy = n.contains("Legacy");
            if is_legacy == want_reforged {
                continue;
            }
            found.push(p);
        }
    }
    found
        .into_iter()
        .next()
        .ok_or_else(|| Error::Stage("no matching Warcraft III product".into()))
}

/// Extracts a retail installer with `innoextract`, into a fresh subdirectory.
///
/// **Not working for the sources on this machine**, and the reason is worth
/// recording rather than hiding: the GOG `Complete Edition.iso` is a repack
/// whose payload sits in `ELAMIGOS*.bin`, and `War3TFT_127b_English.exe` carries
/// no Inno Setup signature at all. Both make `innoextract` exit 2. The two
/// sources that do matter — an already-extracted directory and a Battle.net
/// folder — work, and those are what a 1.29b-targeted run actually uses.
fn extract_iso(iso: &Path, dest: &Path) -> Result<PathBuf, Error> {
    let out = dest.join("extracted");
    let status = std::process::Command::new("innoextract")
        .arg("--gog")
        .arg("--output-dir")
        .arg(&out)
        .arg(iso)
        .status()
        .map_err(|e| Error::Stage(format!("innoextract: {e}")))?;
    if !status.success() {
        return Err(Error::Stage(format!(
            "{} is not a plain Inno Setup installer, so innoextract exited {status}; \
             use an extracted directory or a Battle.net folder instead",
            iso.display()
        )));
    }
    // innoextract writes into `app/` under the output directory.
    let cand = out.join("app");
    if cand.is_dir() {
        return Ok(cand);
    }
    Ok(out)
}

/// Finds `name` in `dir` ignoring case.
fn find_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let want = name.to_ascii_lowercase();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            if n == want && e.path().is_file() {
                return Some(e.path());
            }
        }
    }
    None
}

/// True when a directory is a CASC install rather than an MPQ one.
///
/// Reforged carries its assets under `Data/` as CASC archives with no `.mpq` in
/// sight, so a staging request aimed at it cannot be satisfied by copying files
/// around — it needs the CASC reader, which is RE-Q17. Stating that plainly is
/// more useful than "no WC3 archives found".
fn is_casc_install(dir: &Path) -> bool {
    dir.join("Data").is_dir() || dir.join("data").is_dir()
}

/// CLI front-end.
///
///   peacecraft3 launch sources [paths...]        classify what it was given
///   peacecraft3 launch plan    <mode>            show what a mode would stage
///   peacecraft3 launch stage   <mode> <src> <dst>  copy and verify
pub fn main() -> i32 {
    main_with(&std::env::args().skip(1).collect::<Vec<_>>())
}

/// The launcher's argument handling, split out so the GUI and any caller can
/// run it against an explicit argument list.
pub fn main_with(args: &[String]) -> i32 {
    let Some(cmd) = args.first().map(String::as_str) else {
        help();
        return 2;
    };
    match cmd {
        "sources" => {
            if args.len() < 2 {
                help();
                return 2;
            }
            let mut bad = 0;
            for raw in &args[1..] {
                match classify(raw) {
                    Ok(s) => println!("{:<12} {}", s.label(), s.path().display()),
                    Err(e) => {
                        eprintln!("{:<12} {raw} ({e})", "rejected");
                        bad += 1;
                    }
                }
            }
            if bad > 0 { 1 } else { 0 }
        }
        "plan" => {
            let Some(raw) = args.get(1) else {
                eprintln!("usage: peacecraft3 launch plan <mode>");
                return 2;
            };
            match GameMode::parse(raw) {
                Ok(m) => {
                    println!("mode {m}");
                    println!("  build     {}", m.build);
                    println!("  addon     {}", m.addon);
                    println!("  assets    {}", m.assets);
                    println!("  behaviour {}", m.behaviour);
                    println!("  would stage {} archives:", required_archives(&m).len());
                    for a in required_archives(&m) {
                        println!("    {a}");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
        "stage" => {
            let (Some(raw), Some(src), Some(dst)) = (args.get(1), args.get(2), args.get(3)) else {
                eprintln!("usage: peacecraft3 launch stage <mode> <source> <dest>");
                return 2;
            };
            let Ok(mode) = GameMode::parse(raw) else {
                eprintln!("bad mode {raw}");
                return 1;
            };
            let source = match classify(src) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{src}: {e}");
                    return 1;
                }
            };
            match stage(&mode, &source, dst) {
                Ok(st) => {
                    println!("staged {} -> {}", mode, st.dir.display());
                    println!("  {} archives, {} bytes", st.archives.len(), st.bytes);
                    for a in &st.archives {
                        println!("    {a}");
                    }
                    match st.detected_build {
                        Some(v) => println!("  opened as {v}"),
                        None => println!("  WARNING: the staged tree would not open"),
                    }
                    println!("  {} files exposed", st.file_count);
                    if st.detected_build != Some(mode.build) {
                        eprintln!(
                            "  NOTE: mode asked for {} but the staged tree is {}",
                            mode.build,
                            st.detected_build
                                .map_or("unknown".into(), |v| v.to_string())
                        );
                    }
                    0
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
        other => {
            eprintln!("unknown launch command {other:?}");
            help();
            2
        }
    }
}

fn help() {
    eprintln!(
        "peacecraft3 launch — stage Warcraft III assets and pick a game mode

WHAT THE MODES MEAN
  vanilla        the classic game, bugs included — the fidelity baseline
  hd             classic models with Reforged HD textures where they exist
  reforged       Reforged throughout, art *and* behaviour
  fixes          only the specific bug fixes we adopted, per the fix ledger
                 (docs/reforged-fixes.md); everything else stays vanilla
  tft            The Frozen Throne: full units, abilities, campaigns
  noaddon        Reign of Chaos only, before the addon existed

USAGE
  peacecraft3 launch sources [path...]           classify each path
  peacecraft3 launch plan <mode>                 show what a mode stages
  peacecraft3 launch stage <mode> <src> <dest>   copy archives and verify

MODES  <build>-<addon>-[<assets>-]<behaviour>
  build      1.28c  1.29b
  addon      tft    noaddon
  assets     vanilla  hd (classic art, Reforged HD textures)  reforged
  behaviour  vanilla  reforged  fixes

  `assets` and `behaviour` are separate on purpose: Reforged's art is a
  presentation choice and its gameplay changes are not, so `1.29b-tft-hd-vanilla`
  and `1.29b-tft-reforged-reforged` are both expressible and never confused.

EXAMPLES
  peacecraft3 launch stage 1.29b-tft-vanilla-vanilla ~/wc3/app out/129b
  peacecraft3 launch stage 1.29b-tft-hd-vanilla ~/Warcraft3 out/hd"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_mode_is_vanilla_on_both_axes() {
        for b in [Build::V1_28, Build::V1_29] {
            let m = GameMode::default_for(b);
            assert_eq!(m.assets, AssetMode::Vanilla);
            assert_eq!(m.behaviour, Behaviour::Vanilla);
            assert_eq!(m.addon, Addon::Tft);
        }
    }

    #[test]
    fn modes_round_trip_through_their_identifier() {
        for m in [
            GameMode::default_for(Build::V1_29),
            GameMode {
                build: Build::V1_28,
                addon: Addon::Roc,
                assets: AssetMode::VanillaReforgedHd,
                behaviour: Behaviour::Vanilla,
            },
            GameMode {
                build: Build::V1_29,
                addon: Addon::Tft,
                assets: AssetMode::Reforged,
                behaviour: Behaviour::Reforged,
            },
        ] {
            assert_eq!(GameMode::parse(&m.id()).unwrap(), m, "round trip {m}");
        }
    }

    #[test]
    fn hd_implies_classic_behaviour() {
        // `vanilla+hd` must not silently pick Reforged behaviour, because then
        // the fidelity gate would be measured against the wrong thing.
        let m = GameMode::parse("1.29b+tft+vanilla+hd").unwrap();
        assert_eq!(m.assets, AssetMode::VanillaReforgedHd);
        assert_eq!(m.behaviour, Behaviour::Vanilla);
    }

    #[test]
    fn a_mode_missing_a_component_is_an_error_not_a_default() {
        assert!(GameMode::parse("1.29b").is_err(), "build alone");
        assert!(GameMode::parse("1.29b+tft").is_err(), "no asset mode");
        assert!(GameMode::parse("tft+vanilla").is_err(), "no build");
        assert!(GameMode::parse("1.29b+tft+banana").is_err(), "unknown word");
    }

    #[test]
    fn archive_lists_differ_by_addon_and_stay_case_insensitive() {
        let tft = GameMode::default_for(Build::V1_29);
        let roc = GameMode {
            addon: Addon::Roc,
            ..GameMode::default_for(Build::V1_28)
        };
        let tft_list = required_archives(&tft);
        let roc_list = required_archives(&roc);
        assert!(tft_list.contains(&"war3xlocal.mpq"));
        assert!(
            !roc_list.contains(&"war3xlocal.mpq"),
            "ROC has no TFT archive"
        );
        // Lower-cased throughout, since the two builds spell them differently.
        assert!(tft_list.iter().all(|a| a == &a.to_ascii_lowercase()));
    }

    /// The 1.29b and 1.28c installs that are actually on this machine, so the
    /// classifier is measured against real sources rather than a fixture.
    #[test]
    #[ignore = "requires local Warcraft III and Battle.net installations"]
    fn classifies_the_installs_that_are_actually_present() {
        let candidates = [
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("wc3/game/app"),
            Path::new("/home/maci/Faugus/battle-net/drive_c/Program Files (x86)").to_path_buf(),
        ];
        let mut seen_directory = false;
        let mut seen_bnet = false;
        for p in candidates {
            if !p.is_dir() {
                continue;
            }
            match classify(&p).unwrap_or_else(|e| panic!("{p:?}: {e}")) {
                Source::Directory(_) => seen_directory = true,
                Source::BattleNet(_) => seen_bnet = true,
                other => panic!("{p:?} classified as {other:?}"),
            }
        }
        assert!(
            seen_directory,
            "the extracted 1.28c tree was not classified as a directory"
        );
        assert!(
            seen_bnet,
            "the Battle.net folder was not classified as such"
        );
    }
}
