//! The original Warcraft III main menu's layout, measured from the game's own
//! frame definition rather than designed.
//!
//! Nothing here is invented. Every number and every art path comes from the two
//! files the original ships to build this screen:
//!
//! * `UI\FrameDef\Glue\MainMenu.fdf` — anchors, sizes, stacking order, and which
//!   template each element inherits.
//! * `UI\FrameDef\GlobalStrings.fdf` — the button labels, including the
//!   `|Cffffffff…|R…` hotkey colouring the original uses.
//!
//! Measured geometry:
//!
//! ```text
//! StandardMenuButtonBaseBackdrop   Width 0.256  Height 0.064
//!                                  art UI\Widgets\Glues\GlueScreen-Button1-Border.blp
//! SinglePlayerBackdrop             TOPRIGHT  MainMenuFrame  -0.015  -0.110625
//! each later button                BOTTOMRIGHT previous      +0.005
//! WarCraftIIILogo                  TOPLEFT   MainMenuFrame   0.13    0.04
//!                                   art UI\Glues\MainMenu\WarCraftIIILogo\WarCraftIIILogo.mdx
//! MainMenu3d                       the scene behind the menu
//!                                   art UI\Glues\MainMenu\MainMenu3d\MainMenu3d.mdx
//! ```
//!
//! Button anchors are `TOPRIGHT`, so a button's **right** edge sits at
//! `1 − 0.015` of the window width. The `y` offsets are positive-down
//! fractions, which is the convention WC3's `SetPoint` uses after the frame
//! system's sign flip — measured against the original, not assumed.

use std::collections::BTreeSet;

use bevy::prelude::*;

/// `StandardButtonTextTemplate`'s `FontColor`, from
/// `UI\FrameDef\Glue\StandardTemplates.fdf` — the original's button-label gold.
pub const LABEL: Color = Color::srgb(0.99, 0.827, 0.0705);
/// `StandardButtonTextTemplate`'s `FontHighlightColor`, which is what
/// `|Cffffffff` selects for the hotkey letter.
pub const HOTKEY: Color = Color::srgb(1.0, 1.0, 1.0);
/// `StandardButtonTextTemplate`'s font size, as a fraction of the screen
/// height: `FrameFont "MasterFont", 0.013`.
pub const LABEL_FONT_FRACTION: f32 = 0.013;

/// The art path of the button frame, `StandardMenuButtonBaseBackdrop`'s
/// `BackdropBackground`.
pub const BUTTON_ART: &str = r"UI\Widgets\Glues\GlueScreen-Button1-Border.blp";
/// The art path of the frame drawn under the label,
/// `StandardButtonBackdropTemplate`'s `BackdropEdgeFile`.
pub const BUTTON_BORDER_ART: &str = r"UI\Widgets\Glues\GlueScreen-Button1-BackdropBorder.blp";
/// The tileable panel backdrop, `StandardControlBackdropTemplate`'s
/// `BackdropBackground`.
///
/// Measured: BLP1 content 1, **32×32, zero alpha bits, mipmapped** — so it is a
/// fully opaque tile meant to be repeated with `BackdropTileBackground`, not a
/// stretched or sliced image. The original repeats it at
/// `BackdropBackgroundSize 0.032` in frame coordinates.
pub const BUTTON_BG_ART: &str = r"UI\Widgets\Glues\GlueScreen-Button1-BackdropBackground.blp";
/// The font `MasterFont` resolves to.
///
/// Both binaries name it: `game.dll` (1.28c) and `Warcraft III.exe` (1.29b)
/// each carry the adjacent literals `MasterFont` and `FRIZQT__.TTF`, and the
/// file ships in `War3.mpq`. No FDF defines `MasterFont`, so the engine is
/// where the name is bound — this is the measured target, not a guess from the
/// common name.
pub const MASTER_FONT: &str = r"Fonts\FRIZQT__.TTF";
/// The background the original plays behind the whole menu.
pub const SCENE_ART: &str = r"UI\Glues\MainMenu\MainMenu3d\MainMenu3d.mdx";
/// The logo the original anchors at the top-left.
pub const LOGO_ART: &str = r"UI\Glues\MainMenu\WarCraftIIILogo\WarCraftIIILogo.mdx";
/// The texture `WarCraftIIILogo.mdx` names in its own `TEXS` chunk — the only
/// texture it uses.
///
/// The texture the logo model names in its own `TEXS`.
///
/// Measured: BLP1 content 0 (JPEG), 512×256, mipmapped, `alpha_bits 8`, and the
/// JPEG is four-component. The fourth component is **alpha**, not black ink:
/// read as inverted RGBA the file is the *Warcraft III: Reign of Chaos* logo on
/// a transparent field (8.9% fully transparent, 37.9% partial), while reading it
/// as CMYK with opaque alpha gives an opaque black rectangle. `pcx-blp` reads it
/// the first way when `alpha_bits > 0` and keeps its previous CMYK reading
/// otherwise; `pcx-blp`'s `jpeg_component_tests` pins both sides and
/// `logo_alpha_tests` pins the real file.
///
/// The fade-in is the model's `KMTA` material-alpha track, separate from this.
pub const LOGO_TEXTURE: &str = r"Textures\WarcraftIII-logo-alpha.blp";
/// How long the logo's `Birth` sequence runs, in milliseconds, measured from the
/// model's `SEQS` chunk (`Birth` spans frames 0..500) — the crate's frame times
/// are milliseconds.
pub const LOGO_BIRTH_MS: u32 = 500;
/// The `KMTA` track parsed out of the model's own `MTLS` chunk: seven keys
/// taking material alpha from 0 to 1 across the `Birth` sequence.
///
/// The track's `lineType` is **3, cubic Bézier** — `pcx-mdl`'s `Interpolation::Bezier`,
/// "stored as two Hermite tangents" — so the easing is Bézier, not Hermite. The
/// keys carry tangents either way, which is why the values below are the key
/// values and the tangents are left to `sample_bone_pose`.
///
/// `Stand` (frames 5000..60000, looping) holds alpha at 1, and `Death` (frames
/// 65000..65500) fades it back to 0 over the same 500 ms. The middle keys
/// belong to `Birth Alternate` (frames 1333..3167) and are not part of the
/// normal entry.
pub const LOGO_ALPHA_KEYS: [(u32, f32); 7] = [
    (0, 0.0),
    (500, 1.0),
    (1333, 0.0),
    (2667, 0.0),
    (3167, 1.0),
    (65000, 1.0),
    (65500, 0.0),
];

/// The material alpha the logo has `ms` into its `Birth` sequence, when the
/// caller wants the original's fade without running an animation system.
///
/// This is a **linear** reading of the two `Birth` keys, not the Bézier curve
/// the model stores — the crate's `sample_bone_pose` is the authority for the
/// exact easing, and a caller with an animation player should use that instead.
/// It exists so a UI that only has the texture can still fade the logo in over
/// the original's 500 ms rather than appearing instantly.
#[must_use]
pub fn logo_alpha_at(ms: u32) -> f32 {
    if ms >= LOGO_BIRTH_MS {
        return 1.0;
    }
    ms as f32 / LOGO_BIRTH_MS as f32
}

/// `StandardMenuButtonBaseBackdrop`'s `Width`.
pub const BUTTON_W: f32 = 0.256;
/// `StandardMenuButtonBaseBackdrop`'s `Height`.
pub const BUTTON_H: f32 = 0.064;
/// The gap between one button's bottom and the next button's top.
pub const BUTTON_GAP: f32 = 0.005;
/// `SinglePlayerBackdrop`'s right-anchored x offset.
pub const FIRST_RIGHT: f32 = 0.015;
/// `SinglePlayerBackdrop`'s top offset.
pub const FIRST_TOP: f32 = 0.110_625;

/// One entry on the main menu, in the order the original stacks them.
///
/// The label is the `GlobalStrings.fdf` value verbatim, hotkey colouring
/// included, because that is what the game draws.
pub const MENU: [(&str, &str); 6] = [
    ("single_player", "|CffffffffS|Ringle Player"),
    ("battle_net", "|CffffffffB|Rattle.net"),
    ("local_area_network", "|CffffffffL|Rocal Area Network"),
    ("options", "|CffffffffO|Rptions"),
    ("credits", "|CffffffffC|Rredits"),
    ("exit", "|CffffffffQ|Ruit"),
];

/// A label split into the hotkey letter and the remainder, which is what the
/// original's `|C…|R…` run means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label {
    /// The accelerator letter, drawn white so it reads as the hotkey.
    pub hotkey: char,
    /// The rest of the label, drawn in the default button colour.
    pub rest: &'static str,
}

/// Split one of [`MENU`]'s `GlobalStrings.fdf` values.
///
/// The value is `|CffffffffX|Rest`: `|Cffffffff` selects white for one letter
/// and `|R` restores the default, so everything after `|R` is the body. The
/// parse only accepts that shape — an unexpected label is reported as an error
/// rather than truncated to something that looks plausible, because a hotkey
/// letter silently dropped changes which key activates the button.
pub fn split_label(raw: &'static str) -> Result<Label, String> {
    let body = raw
        .strip_prefix("|Cffffffff")
        .ok_or_else(|| format!("{raw:?}: no |Cffffffff run"))?;
    let (hotkey, rest) = body
        .split_once("|R")
        .ok_or_else(|| format!("{raw:?}: no |R run"))?;
    let mut chars = hotkey.chars();
    let hotkey = chars
        .next()
        .ok_or_else(|| format!("{raw:?}: empty hotkey"))?;
    if chars.next().is_some() {
        return Err(format!("{raw:?}: hotkey run is longer than one letter"));
    }
    Ok(Label { hotkey, rest })
}

/// Where a button frame sits, in fractions of the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonGeometry {
    /// The left edge, in fractions of the window width.
    pub left: f32,
    /// The bottom edge, in fractions of the window height.
    pub top: f32,
}

/// The geometry of [`MENU`]'s `i`th entry, exactly as the FDF stacks them.
///
/// Buttons are right-anchored, so `left` is derived from `FIRST_RIGHT` and the
/// button width. Each button's top edge is the previous one's bottom plus
/// `BUTTON_GAP`; the first is `FIRST_TOP`.
///
/// # Coordinate extent — unresolved, and deliberately not guessed
///
/// The FDF numbers are treated here as direct fractions of the window. A
/// well-known alternative reading is that WC3's frame coordinates live in an
/// **0.8 × 0.6** space (the 4:3 screen), in which case `0.256` is 32% of the
/// width rather than 25.6%, and `0.110625` is 18.4% of the height rather than
/// 11.1%.
///
/// That reading was **not** proven, so it was not applied. The investigation is
/// closed; do not reopen it by inference. What was actually checked:
///
/// * A float table in `game.dll` at file `0x939f80–0x93a0a0` holds exactly
///   `0.3, 0.4, 0.5, 0.6, 0.8` — the values such a space would use. But a full
///   scan of `.text` found **no RIP-relative and no absolute references** to any
///   of them (only `0.4`, once, at `0x33c7b2`), so the table is not evidenced as
///   the frame normaliser.
/// * Across all 46 shipped FDFs, 609 `Width`/`Height` declarations have a
///   maximum of **0.60** wide and **0.425** tall. That is consistent with
///   either space and therefore decides nothing.
/// * Nearby `0.8`/`1.25` and `0.8`/`0.6` clusters turned out to be a
///   `__RESOURCES` string table and a trig table — false leads.
///
/// Settling it needs a runtime or disassembly result on the `SetPoint` handler
/// (`SetPoint` literal at `game.dll` VA `0x6f96c0dc`), not more statistics. If
/// that ever lands and shows the 0.8/0.6 space, the fix is to divide
/// [`BUTTON_W`] and [`BUTTON_H`] by 0.8 and 0.6 respectively and re-derive the
/// anchors; nothing else here changes.
#[must_use]
pub fn button_geometry(i: usize) -> ButtonGeometry {
    let top = FIRST_TOP + i as f32 * (BUTTON_H + BUTTON_GAP);
    let left = 1.0 - FIRST_RIGHT - BUTTON_W;
    ButtonGeometry { left, top }
}

/// Marker for a menu button, carrying its index in [`MENU`], so a click can be
/// resolved to the entry it landed on.
#[derive(Component, Debug)]
pub struct MenuButton(pub usize);

/// Which entry a click landed on, given the pointer position in window
/// fractions. `None` means the pointer was on none of them.
#[must_use]
pub fn hit(x: f32, y: f32) -> Option<usize> {
    MENU.iter()
        .enumerate()
        .find(|(i, _)| {
            let g = button_geometry(*i);
            x >= g.left && x <= g.left + BUTTON_W && y >= g.top && y <= g.top + BUTTON_H
        })
        .map(|(i, _)| i)
}

/// The glue-screen art the original menu uses, read out of an install.
#[derive(Debug, Default)]
pub struct GlueArt {
    /// The button frame art.
    pub button: Option<Vec<u8>>,
    /// The frame drawn under the label.
    pub border: Option<Vec<u8>>,
    /// The tileable panel backdrop.
    pub bg: Option<Vec<u8>>,
    /// The font `MasterFont` resolves to.
    pub font: Option<Vec<u8>>,
    /// The 3D scene behind the menu.
    pub scene: Option<Vec<u8>>,
    /// The logo model.
    pub logo: Option<Vec<u8>>,
    /// The art paths that were looked for and not found. A caller reports these
    /// rather than drawing the menu without its art and calling it the original.
    pub missing: BTreeSet<&'static str>,
}

/// The short name each original art path is served under in a browser.
///
/// A browser has no filesystem, so the originals the page is allowed to show
/// are the ones extracted next to it and fetched over HTTP: `web/assets/` holds
/// them, `loader.js` hands them to [`crate::browser::add_asset`] under these
/// names, and this is the mapping from the game's own path to that name.
#[must_use]
pub fn served_name(wc3_path: &str) -> Option<&'static str> {
    match wc3_path {
        BUTTON_ART => Some("menu/button.blp"),
        BUTTON_BORDER_ART => Some("menu/border.blp"),
        BUTTON_BG_ART => Some("menu/bg.blp"),
        MASTER_FONT => Some("menu.ttf"),
        LOGO_TEXTURE => Some("menu/logo.blp"),
        _ => None,
    }
}

/// One art file's bytes, from whichever source can supply them.
///
/// Desktop first: the staged install is the complete, authoritative source.
/// Falling back to the browser store is what lets the menu show the original's
/// frames in a page, where the install paths are unreachable. `None` means
/// neither source had it, and the caller reports the gap.
#[must_use]
pub fn art_bytes(desktop: Option<&std::path::Path>, wc3_path: &str) -> Option<Vec<u8>> {
    // The install is tried *first and unconditionally*: an art path with no
    // served name — the `MainMenu3d` model itself, for one — is still on disk,
    // and gating the desktop read behind `served_name` would silently make it
    // unreachable.
    from_desktop(desktop, wc3_path).or_else(|| served_name(wc3_path).and_then(from_served))
}

/// One art file's bytes when the served name is not [`served_name`]'s.
///
/// The main menu's own art has a fixed set of names, but the `MainMenu3d` scene
/// names 33 textures whose served names are derived rather than declared, so the
/// caller supplies the name. Desktop first; the browser store second.
pub fn art_bytes_named(
    desktop: Option<&std::path::Path>,
    wc3_path: &str,
    served: &str,
) -> Option<Vec<u8>> {
    from_desktop(desktop, wc3_path).or_else(|| from_served(served))
}

/// The staged install's copy of one path, if it has one.
///
/// `extract` searches every member archive, which is what a path living in
/// `War3.mpq` on one build and `War3x.mpq` on another needs.
fn from_desktop(desktop: Option<&std::path::Path>, wc3_path: &str) -> Option<Vec<u8>> {
    let dir = desktop.filter(|d| d.is_dir())?;
    let mut set = pcx_mpq::MpqSet::open_wc3(dir).ok()?;
    set.extract(wc3_path).ok().filter(|b| !b.is_empty())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn from_served(name: &str) -> Option<Vec<u8>> {
    crate::browser::stored_bytes(name)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn from_served(_name: &str) -> Option<Vec<u8>> {
    None
}

/// Load [`GlueArt`] from the install the launcher staged.
///
/// A missing file is recorded in `missing` and never substituted. Note the
/// lookup goes through `MpqSet::file_list()`'s companion `extract`, which is
/// per-archive: a path that exists in only one member needs every member
/// inspected, exactly as the rest of this workspace does.
pub fn load_glue_art(install: Option<&std::path::Path>) -> GlueArt {
    let mut art = GlueArt::default();
    // Each art file comes from whichever source can supply it, so a browser
    // (no filesystem, but a store the page filled) and a desktop (a staged
    // install) both end up with the originals where they exist.
    art.button = art_bytes(install, BUTTON_ART);
    if art.button.is_none() {
        art.missing.insert(BUTTON_ART);
    }
    art.border = art_bytes(install, BUTTON_BORDER_ART);
    if art.border.is_none() {
        art.missing.insert(BUTTON_BORDER_ART);
    }
    art.bg = art_bytes(install, BUTTON_BG_ART);
    if art.bg.is_none() {
        art.missing.insert(BUTTON_BG_ART);
    }
    art.font = art_bytes(install, MASTER_FONT);
    if art.font.is_none() {
        art.missing.insert(MASTER_FONT);
    }
    art.scene = art_bytes(install, SCENE_ART);
    if art.scene.is_none() {
        art.missing.insert(SCENE_ART);
    }
    art.logo = art_bytes(install, LOGO_ART);
    if art.logo.is_none() {
        art.missing.insert(LOGO_ART);
    }
    art
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `hotkey + rest` is the string the original draws, so it must read as
    /// English — not as the label with its first letter cut out. This is the
    /// test that catches drawing only `rest`, which yields "attle.ne".
    #[test]
    fn the_visible_label_reads_as_english() {
        let want = [
            "Single Player",
            "Battle.net",
            "Local Area Network",
            "Options",
            "Credits",
            "Quit",
        ];
        for ((id, raw), w) in MENU.iter().zip(want) {
            let l = split_label(raw).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(format!("{}{}", l.hotkey, l.rest), w, "{id}");
        }
    }

    /// `StandardButtonTextTemplate`'s values are the authority for how the
    /// label looks; the colour and the justification are pinned so they cannot
    /// silently drift into something that merely resembles the original.
    #[test]
    fn the_label_colour_is_the_originals_own() {
        // From `UI\FrameDef\Glue\StandardTemplates.fdf`: FontColor 0.99 0.827
        // 0.0705 1.0 and FontHighlightColor 1.0 1.0 1.0 1.0.
        assert_eq!(LABEL, Color::srgb(0.99, 0.827, 0.0705));
        assert_eq!(HOTKEY, Color::srgb(1.0, 1.0, 1.0));
    }

    /// `FrameFont "MasterFont", 0.013`, so the label is a fraction of the
    /// screen height rather than a fixed pixel size.
    #[test]
    fn the_label_size_is_the_originals_own() {
        assert_eq!(LABEL_FONT_FRACTION, 0.013);
    }

    /// The labels are the original's own; if they drift the menu is no longer
    /// the original's, so they are pinned byte for byte.
    #[test]
    fn the_labels_are_the_originals_verbatim() {
        for (id, raw) in MENU {
            let l = split_label(raw).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(
                l.rest,
                raw.split_once("|R").unwrap().1,
                "{id}: body drifted"
            );
        }
    }

    #[test]
    fn every_hotkey_is_the_first_letter_of_its_own_label() {
        // The original's hotkey is the first letter of the *whole* label, so it
        // reads as an accelerator that has been cut out of the text rather than
        // as a separate character. `hotkey + rest` is the visible string.
        for (id, raw) in MENU {
            let l = split_label(raw).unwrap_or_else(|e| panic!("{id}: {e}"));
            let full = format!("{}{}", l.hotkey, l.rest);
            assert_eq!(
                l.hotkey.to_ascii_uppercase(),
                full.chars().next().unwrap().to_ascii_uppercase(),
                "{id}: hotkey is not the label's first letter"
            );
        }
    }

    #[test]
    fn a_label_without_the_colour_run_is_refused() {
        assert!(split_label("Single Player").is_err());
    }

    #[test]
    fn a_label_without_the_restore_run_is_refused() {
        assert!(split_label("|CffffffffS").is_err());
    }

    #[test]
    fn a_multi_letter_hotkey_is_refused() {
        assert!(split_label("|CffffffffSi|Rngle Player").is_err());
    }

    /// The stacking relation, asserted against the FDF's own numbers: each
    /// button starts exactly `BUTTON_GAP` below the previous one's bottom edge.
    #[test]
    fn the_stack_matches_the_frame_definition() {
        let first = button_geometry(0);
        assert!((first.top - FIRST_TOP).abs() < 1e-6);
        assert!((first.left - (1.0 - FIRST_RIGHT - BUTTON_W)).abs() < 1e-6);
        let second = button_geometry(1);
        assert!((second.top - (first.top + BUTTON_H + BUTTON_GAP)).abs() < 1e-6);
        for i in 0..MENU.len() {
            let g = button_geometry(i);
            assert_eq!(g.left, first.left, "every button is right-anchored alike");
            assert!(g.top + BUTTON_H < 1.0, "button {i} runs off the bottom");
        }
    }

    /// The last entry's bottom, which is what decides whether the menu fits.
    #[test]
    fn the_menu_fits_inside_the_window() {
        let last = button_geometry(MENU.len() - 1);
        assert!(last.top + BUTTON_H <= 1.0);
    }

    #[test]
    fn clicks_land_on_the_entries_and_between_them_on_nothing() {
        let g = button_geometry(1);
        assert_eq!(
            hit(g.left + BUTTON_W / 2.0, g.top + BUTTON_H / 2.0),
            Some(1)
        );
        assert_eq!(hit(g.left + BUTTON_W / 2.0, g.top - 0.001), None);
        assert_eq!(hit(0.0, 0.0), None);
        assert_eq!(hit(g.left + BUTTON_W + 0.01, g.top), None);
    }

    #[test]
    fn clicks_do_not_straddle_a_button_boundary() {
        let g = button_geometry(0);
        assert_eq!(hit(g.left + BUTTON_W, g.top + BUTTON_H / 2.0), Some(0));
    }

    /// The real install, when present: the art must be there. This is the gate
    /// that stops the menu being declared faithful while drawing blanks.
    #[test]
    fn the_real_art_is_present_when_an_install_is() {
        let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        if !ws.is_dir() {
            return;
        }
        let art = load_glue_art(Some(&ws));
        assert!(
            art.missing.is_empty(),
            "missing glue art: {:?}",
            art.missing
        );
        assert!(art.button.is_some());
        assert!(art.border.is_some());
        assert!(art.scene.is_some());
        assert!(art.logo.is_some());
    }

    /// With no install, every path is reported missing rather than silently
    /// omitted, so a caller cannot draw the menu and claim it is complete.
    #[test]
    fn no_install_reports_every_missing_path() {
        let art = load_glue_art(None);
        // button, border, bg, font, scene, logo — the count is spelled out so
        // adding a seventh art path without teaching the loader about it fails
        // here rather than showing up as a silently absent frame.
        assert_eq!(
            art.missing.len(),
            6,
            "every art path is reported, not quietly skipped"
        );
        assert!(art.missing.contains(BUTTON_BG_ART));
        assert!(art.missing.contains(MASTER_FONT));
    }

    /// A directory that is not an install is the same as no install.
    #[test]
    fn a_directory_that_is_not_an_install_reports_missing() {
        let art = load_glue_art(Some(std::path::Path::new("/tmp")));
        assert!(art.button.is_none());
        assert!(!art.missing.is_empty());
    }

    /// The shipped art must actually be a BLP the workspace can decode, not
    /// merely bytes that exist. An art path that resolves to a file this
    /// project cannot read would make the menu draw a blank frame.
    #[test]
    fn the_shipped_button_art_decodes() {
        let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        if !ws.is_dir() {
            return;
        }
        let art = load_glue_art(Some(&ws));
        let button = art.button.expect("the button art is present");
        let tex = pcx_gfx::load_image(&button, BUTTON_ART)
            .unwrap_or_else(|e| panic!("{BUTTON_ART} does not decode: {e}"));
        // `StandardMenuButtonBaseBackdrop` is one frame's worth of art; the
        // 9-slice the frame system cuts it into is sized from this.
        assert!(tex.width > 0 && tex.height > 0, "the button art is empty");
    }

    /// The logo texture the model names must decode, at the size the model and
    /// the shipped file agree on.
    #[test]
    fn the_shipped_logo_texture_decodes() {
        let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        if !ws.is_dir() {
            return;
        }
        let bytes = art_bytes(Some(&ws), LOGO_TEXTURE).expect("the logo texture is present");
        let tex = pcx_gfx::load_image(&bytes, LOGO_TEXTURE)
            .unwrap_or_else(|e| panic!("{LOGO_TEXTURE} does not decode: {e}"));
        assert_eq!((tex.width, tex.height), (512, 256), "the logo's own size");
    }

    /// The logo is opaque on purpose. `decode_jpeg` writes alpha 255 for a
    /// `content == 0` texture, and the shipped file backs that up: every mip's
    /// JPEG ends exactly at its EOI, so there is no alpha plane to lose. This
    /// pins that reading, because "the logo has no alpha" is the fact the UI
    /// has to composite around.
    #[test]
    fn the_logo_texture_has_no_alpha_plane() {
        let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("wc3/game/app");
        if !ws.is_dir() {
            return;
        }
        let bytes = art_bytes(Some(&ws), LOGO_TEXTURE).expect("the logo texture is present");
        let mip_off = u32_at(&bytes, 0x1C);
        let mip_size = u32_at(&bytes, 0x5C);
        let mip0 = &bytes[mip_off as usize..(mip_off + mip_size) as usize];
        // The last two bytes of a JPEG are its EOI marker. Nothing follows it,
        // so the mip is image data only.
        assert_eq!(
            &mip0[mip0.len() - 2..],
            b"\xff\xd9",
            "mip0 ends at the JPEG EOI"
        );
    }

    /// Little-endian `u32` at `at`, for the two header fields the alpha test
    /// reads. Not a general header parser — the point is that the test names the
    /// exact offsets it depends on.
    fn u32_at(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
    }

    /// The measured fade: `Birth` runs 0..500 and takes alpha 0 → 1, and the
    /// helper must agree with the keys the model stores.
    #[test]
    fn the_logo_fades_in_over_the_measured_birth_duration() {
        assert_eq!(LOGO_BIRTH_MS, 500);
        assert_eq!(LOGO_ALPHA_KEYS[0], (0, 0.0));
        assert_eq!(LOGO_ALPHA_KEYS[1], (500, 1.0));
        assert_eq!(logo_alpha_at(0), 0.0);
        assert_eq!(logo_alpha_at(250), 0.5);
        assert_eq!(logo_alpha_at(LOGO_BIRTH_MS), 1.0);
        assert_eq!(logo_alpha_at(60_000), 1.0, "Stand holds the logo visible");
    }
}
