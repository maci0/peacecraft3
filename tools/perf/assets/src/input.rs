//! Reading fixed inputs from the local install (setup only, never measured).
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use pcx_mpq::{MpqArchive, MpqSet, huffman::HuffmanTable};

use crate::{Failure, digest::Fnv};

/// The four archives a campaign mount reads, highest priority first.
pub const ARCHIVES: [&str; 4] = ["war3x.mpq", "war3xlocal.mpq", "war3local.mpq", "war3.mpq"];

/// Executable whose image carries the Huffman and ADPCM tables (Legacy 1.29).
pub const CODEC_SOURCE: &str = "Warcraft III.exe";

pub fn open_set(install: &Path) -> Result<MpqSet, Failure> {
    MpqSet::open_named(install, &ARCHIVES).map_err(|e| format!("{}: {e}", install.display()))
}

pub fn archive_paths(install: &Path) -> Result<Vec<PathBuf>, Failure> {
    Ok(open_set(install)?
        .members()
        .iter()
        .map(|a| a.path().to_path_buf())
        .collect())
}

pub fn codec_table(install: &Path) -> Result<Arc<HuffmanTable>, Failure> {
    HuffmanTable::open(install.join(CODEC_SOURCE))
        .map(Arc::new)
        .map_err(|e| format!("{CODEC_SOURCE}: {e}"))
}

/// A set whose members already hold the codec table, so extraction never
/// re-discovers it from the install.
pub fn open_set_with_codec(install: &Path) -> Result<MpqSet, Failure> {
    let table = codec_table(install)?;
    let mut set = open_set(install)?;
    for member in set.members_mut() {
        member.set_codec_table(Arc::clone(&table));
    }
    Ok(set)
}

pub fn open_archive(path: &Path) -> Result<MpqArchive, Failure> {
    MpqArchive::open(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Extracts every path, recording each name and the bytes into `input`.
pub fn extract_all(
    set: &mut MpqSet,
    paths: &[&str],
    input: &mut Fnv,
) -> Result<Vec<(String, Vec<u8>)>, Failure> {
    let mut out = Vec::new();
    for &path in paths {
        let bytes = set.extract(path).map_err(|e| format!("{path}: {e}"))?;
        input.text(path);
        input.bytes(&bytes);
        out.push((path.to_string(), bytes));
    }
    Ok(out)
}

/// Members extracted by the MPQ workloads: models, textures, tables, scripts,
/// speech (Huffman and ADPCM sectors), an MP3 and a whole map archive.
pub const EXTRACT_MEMBERS: [&str; 24] = [
    "Units\\Human\\Footman\\Footman.mdx",
    "Buildings\\Orc\\GreatHall\\GreatHall.mdx",
    "Units\\Human\\HeroPaladin\\HeroPaladin.mdx",
    "UI\\Glues\\MainMenu\\MainMenu3d\\MainMenu3d.mdx",
    "Units\\Human\\HeroPaladin\\Heropaladin.blp",
    "TerrainArt\\Ashenvale\\Ashen_Dirt.blp",
    "ReplaceableTextures\\Selection\\SelectionCircleLarge.blp",
    "UI\\Glues\\Loading\\Backgrounds\\Campaigns\\Ashenvale-TopLeft.blp",
    "Units\\UnitData.slk",
    "Units\\unitUI.slk",
    "Units\\AbilityData.slk",
    "Units\\HumanUnitFunc.txt",
    "Units\\CommonAbilityFunc.txt",
    "Scripts\\Blizzard.j",
    "Scripts\\common.j",
    "Units\\Human\\Footman\\FootmanDeath.wav",
    "Units\\Human\\Footman\\FootmanReady1.wav",
    "Units\\Human\\Footman\\FootmanYes1.wav",
    "Units\\Human\\Footman\\FootmanWhat1.wav",
    "Sound\\Dialogue\\Extra\\MuradinTaunt1.mp3",
    "Maps\\Campaign\\Human03.w3m",
    "Maps\\Campaign\\Orc01.w3m",
    "(listfile)",
    "Units\\UnitBalance.slk",
];

/// Maps for the checked-reader, map and terrain workloads. Names that exist as
/// files under the install are read from disk, otherwise from the archives.
pub const CHECKED_MAPS: [&str; 4] = [
    "Maps\\Campaign\\Human03.w3m",
    "Maps\\Campaign\\Orc01.w3m",
    "Maps\\(12)IceCrown.w3m",
    "Maps\\(2)BootyBay.w3m",
];

/// Map members read through `CheckedArchive` (absent members are reported as such).
pub const CHECKED_MEMBERS: [&str; 8] = [
    "war3map.w3i",
    "war3map.w3e",
    "war3map.shd",
    "war3map.doo",
    "war3map.wpm",
    "war3map.wts",
    "war3map.j",
    "war3mapUnits.doo",
];

/// Reads maps by name from the install directory or, failing that, the archives.
pub fn read_maps(install: &Path, names: &[&str]) -> Result<Vec<(String, Vec<u8>)>, Failure> {
    let mut set = None;
    let mut out = Vec::new();
    for &name in names {
        let loose = install.join(name.replace('\\', "/"));
        let bytes = if loose.is_file() {
            std::fs::read(&loose).map_err(|e| format!("{}: {e}", loose.display()))?
        } else {
            if set.is_none() {
                set = Some(open_set_with_codec(install)?);
            }
            set.as_mut()
                .expect("set opened above")
                .extract(name)
                .map_err(|e| format!("{name}: {e}"))?
        };
        out.push((name.to_string(), bytes));
    }
    Ok(out)
}

/// Unit speech and acknowledgements: Huffman and ADPCM sectors, as a map preloads them.
pub const AUDIO_MEMBERS: [&str; 24] = [
    "Units\\Human\\Footman\\FootmanDeath.wav",
    "Units\\Human\\Footman\\FootmanPissed1.wav",
    "Units\\Human\\Footman\\FootmanPissed2.wav",
    "Units\\Human\\Footman\\FootmanPissed3.wav",
    "Units\\Human\\Footman\\FootmanPissed4.wav",
    "Units\\Human\\Footman\\FootmanReady1.wav",
    "Units\\Human\\Footman\\FootmanWarcry1.wav",
    "Units\\Human\\Footman\\FootmanWhat1.wav",
    "Units\\Human\\Footman\\FootmanWhat2.wav",
    "Units\\Human\\Footman\\FootmanWhat3.wav",
    "Units\\Human\\Footman\\FootmanWhat4.wav",
    "Units\\Human\\Footman\\FootmanYes1.wav",
    "Units\\Human\\Footman\\FootmanYes2.wav",
    "Units\\Human\\Footman\\FootmanYes3.wav",
    "Units\\Human\\Footman\\FootmanYes4.wav",
    "Units\\Human\\Footman\\FootmanYesAttack1.wav",
    "Units\\Human\\Footman\\FootmanYesAttack2.wav",
    "Units\\Human\\Footman\\FootmanYesAttack3.wav",
    "Units\\Human\\HeroPaladin\\HeroPaladinAttack1.wav",
    "Units\\Human\\HeroPaladin\\HeroPaladinDeath.wav",
    "Units\\Human\\HeroPaladin\\HeroPaladinReady1.wav",
    "Units\\Human\\HeroPaladin\\HeroPaladinWhat1.wav",
    "Units\\Human\\HeroPaladin\\HeroPaladinYes1.wav",
    "Units\\Human\\HeroPaladin\\HeroPaladinYesAttack1.wav",
];
