//! Explicit selected-original gate for Help/Tips default font.
//!
//! Run with `PEACECRAFT3_LEGACY_DIR` set to the supplied Legacy 1.29.2.9231
//! install. Reads `Warcraft III.exe`, `UI\MiscUI.txt`,
//! `UI\FrameDef\UI\EscMenuTemplates.fdf` and `UI\HelpStrings.txt` /
//! `UI\TipStrings.txt` from that install only; nothing is copied into the
//! repository and no original bytes are asserted beyond identity digests
//! already recorded in `docs/re-notes/escape-help-tips.md`.
//!
//! Verifies:
//! - executable identity is the reviewed Legacy image,
//! - `DefaultFont::read` decodes Misc/FontTexture operands, path prefix/format,
//!   slot, height table and axis factor from that image,
//! - the decoded `Fonts\<texture>` path resolves to an existing member in the
//!   install's archives,
//! - wrapping height (`table[slot] * axis`) is independent of the FDF
//!   `TextAreaLineHeight` row height.

use pcx_mpq::MpqSet;
use pcx_play::escape_help_tips::{DefaultFont, HelpStrings, TipStrings, text_area_numbers};
use pcx_play::scripts::AssetIdentity;
use sha2::{Digest, Sha256};

const LEGACY_EXE_SHA256: &str = "3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed";

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR pointing at the selected Legacy install"]
fn selected_original_help_tips_default_font_gate() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var_os("PEACECRAFT3_LEGACY_DIR")
        .expect("set PEACECRAFT3_LEGACY_DIR to the selected 1.29.2.9231 install");
    let dir = std::path::PathBuf::from(dir);
    let exe = std::fs::read(dir.join("Warcraft III.exe"))?;
    assert_eq!(
        sha256_hex(&exe),
        LEGACY_EXE_SHA256,
        "selected executable identity"
    );

    let mut set = MpqSet::open_wc3(&dir)?;
    let misc = set.extract(r"UI\MiscUI.txt")?;
    let misc_text = String::from_utf8(misc.clone())?;
    let origin = AssetIdentity {
        source: dir.join("Warcraft III.exe").display().to_string(),
        edition: "selected Legacy menu client".into(),
        version: "1.29.2.9231".into(),
    };
    let font = DefaultFont::read(&exe, origin, &misc_text)
        .map_err(|e| format!("DefaultFont::read: {e}"))?;
    println!(
        "DefaultFont section={} key={} prefix={} format={:?} texture={} path={} slot={} table={} axis={} wrapping={}",
        font.section,
        font.key,
        font.path_prefix,
        font.path_format,
        font.font_texture,
        font.font_path,
        font.wrapping_slot,
        font.table_height,
        font.axis_factor,
        font.wrapping_height()
    );
    assert_eq!(font.section, "Misc");
    assert_eq!(font.key, "FontTexture");
    assert_eq!(font.wrapping_slot, 0);
    assert!(font.table_height > 0.0 && font.axis_factor > 0.0);
    assert_eq!(
        font.font_path,
        format!("{}\\{}", font.path_prefix, font.font_texture)
    );

    // Decoded path must resolve to an actual font member in the selected archives.
    let font_member = font.font_path.replace('\\', "/");
    println!("font member lookup: {font_member}");
    assert!(
        set.contains(&font.font_path) || set.contains(&font_member),
        "decoded font path must resolve in selected archives: {}",
        font.font_path
    );
    let font_bytes = set
        .extract(&font.font_path)
        .or_else(|_| set.extract(&font_member))?;
    println!("font bytes: {}", font_bytes.len());
    assert!(!font_bytes.is_empty(), "decoded font is empty");

    // FDF row height is a separate input from wrapping height.
    let templates = set.extract(r"UI\FrameDef\UI\EscMenuTemplates.fdf")?;
    let templates_text = String::from_utf8(templates)?;
    let (line_height, inset) = text_area_numbers(&templates_text, "EscMenuTextAreaTemplate")?;
    println!("FDF TextAreaLineHeight={line_height} TextAreaInset={inset}");
    assert!(line_height > 0.0 && inset >= 0.0);
    assert_ne!(
        font.wrapping_height().to_bits(),
        line_height.to_bits(),
        "wrapping height must be independent of FDF row height"
    );

    // Help/Tips content parses through the production parsers.
    let help = HelpStrings::parse(&String::from_utf8(set.extract(r"UI\HelpStrings.txt")?)?)?;
    println!("HelpStrings chars: {}", help.body.chars().count());
    assert!(!help.body.is_empty());
    let tips = TipStrings::parse(&String::from_utf8(set.extract(r"UI\TipStrings.txt")?)?)?;
    println!("TipStrings count: {}", tips.tips.len());
    assert!(!tips.tips.is_empty());
    Ok(())
}
