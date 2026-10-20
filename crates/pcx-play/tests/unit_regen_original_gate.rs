//! Explicit local-input gate for unit regeneration: reads the selected install
//! directly. Nothing is extracted to disk, and no scratch copy is consulted.

use pcx_mpq::{MpqSet, fnv1a64};
use pcx_objects::editor::misc::HeroVitals;
use sha2::{Digest, Sha256};

/// `Warcraft III.exe` of the selected Legacy install, resource version 1.29.2.9231.
const LEGACY_EXE_SHA256: &str = "3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed";
const REGEN_TYPES: [&str; 5] = ["always", "day", "night", "none", "blight"];

fn required_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::var_os("PEACECRAFT3_LEGACY_DIR")
        .ok_or("set PEACECRAFT3_LEGACY_DIR to the selected 1.29.2.9231 install")?;
    Ok(dir.into())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR pointing at the selected Legacy install"]
fn selected_install_regeneration_inputs_match_source_gate() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = required_dir()?;
    let exe = std::fs::read(dir.join("Warcraft III.exe"))?;
    assert_eq!(
        sha256_hex(&exe),
        LEGACY_EXE_SHA256,
        "selected executable identity"
    );

    let mut set = MpqSet::open_wc3(&dir)?;

    // UnitBalance: the three regeneration columns are present and every row's token is admitted.
    let balance = pcx_slq::Table::parse(&set.extract("Units\\UnitBalance.slk")?)?;
    for column in ["regenHP", "regenMana", "regenType"] {
        assert!(
            balance.column_of(column).is_some(),
            "UnitBalance missing {column}"
        );
    }
    let mut census = std::collections::BTreeMap::<&str, usize>::new();
    for row in balance.data_rows() {
        let token = row
            .by_name("regenType")
            .ok_or("UnitBalance row without regenType")?
            .trim();
        let token = REGEN_TYPES
            .iter()
            .find(|t| **t == token)
            .copied()
            .ok_or_else(|| format!("unadmitted regenType {token:?}"))?;
        *census.entry(token).or_default() += 1;
    }
    println!("UnitBalance regenType census: {census:?}");

    // MiscGame: the hero regeneration bonuses load through the production reader.
    let misc = set.extract("Units\\MiscGame.txt")?;
    let vitals = HeroVitals::read(&[("Units\\MiscGame.txt", misc.as_slice())], 8 << 20)?;
    println!(
        "MiscGame hero vitals: life={} mana={} regenStr={} regenInt={} fnv1a64={:016x}",
        vitals.life_per_strength,
        vitals.mana_per_intelligence,
        vitals.life_regen_per_strength,
        vitals.mana_regen_per_intelligence,
        fnv1a64(&misc)
    );
    assert_eq!(vitals.life_regen_per_strength, 0.05);
    assert_eq!(vitals.mana_regen_per_intelligence, 0.05);

    // Blizzard.j: the stock day window that gates day and night regeneration.
    let blizzard = set.extract("Scripts\\Blizzard.j")?;
    let text = String::from_utf8_lossy(&blizzard);
    for (name, value) in [("bj_TOD_DAWN", 6.0), ("bj_TOD_DUSK", 18.0)] {
        let line = text
            .lines()
            .find(|l| l.trim_start().starts_with("constant real") && l.contains(name))
            .ok_or_else(|| format!("Blizzard.j missing constant {name}"))?;
        let parsed: f64 = line
            .split('=')
            .nth(1)
            .ok_or("constant without initializer")?
            .trim()
            .parse()?;
        assert_eq!(parsed, value, "{name}");
    }
    println!("Blizzard.j fnv1a64={:016x}", fnv1a64(&blizzard));
    Ok(())
}
