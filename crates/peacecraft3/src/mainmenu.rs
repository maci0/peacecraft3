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

// Only the test-only `GlueArt` collects a set of missing paths.
#[cfg(test)]
use std::collections::BTreeSet;

use bevy::prelude::*;

/// The launcher's own accent gold, for PeaceCraft's UI rather than the original
/// menu.
///
/// The menu label's colour, highlight, size, alignment and shadow are **not**
/// copied here: they are read from `StandardButtonTextTemplate` through
/// [`crate::menu_text::text_style`], so they cannot drift from the asset.
pub const LABEL: Color = Color::srgb(0.99, 0.827, 0.0705);

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
/// The file the shipped `MasterFont` face resolves to, as a **test fixture**.
///
/// Production no longer carries this binding: the face comes from the FDF and the
/// file from the skin table, through [`menu_font_file`]. This constant exists only
/// so `load_glue_art`'s tests can assert the shipped file is present, and it is
/// not a fallback — nothing in production reads it.
#[cfg(test)]
pub const MASTER_FONT: &str = r"Fonts\FRIZQT__.TTF";
/// The background the original plays behind the whole menu.
#[cfg(test)]
pub const SCENE_ART: &str = r"UI\Glues\MainMenu\MainMenu3d\MainMenu3d.mdx";
/// The logo the original anchors at the top-left.
#[cfg(test)]
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
/// The authored glue frame: the original divides X by 0.8 and Y by 0.6
/// (`docs/re-notes/glue-coordinate-runtime.md`).
pub const FRAME_W: f32 = 0.8;
pub const FRAME_H: f32 = 0.6;
/// The `GlobalStrings.fdf` the labels come from.
pub const GLOBAL_STRINGS_FDF: &str = r"UI\FrameDef\GlobalStrings.fdf";

/// The shipped skin table, the face → font-file binding's source.
///
/// A `FrameFont` record names a **face**; this table names its file. Read at
/// runtime rather than mapped by a constant here, so the binding stays the
/// asset's.
pub const SKIN_TABLE: &str = r"UI\war3skins.txt";

/// The font file `face` names, from the supplied skin table.
///
/// Resolution is **unambiguous across the whole table**: the engine's section
/// precedence is not established, so a face defined more than once — in one block
/// or across sections — is refused rather than guessed at. No fallback face or
/// file is substituted when the table is missing or the face is absent.
///
/// # Errors
///
/// [`crate::menu_skin::SkinError`] for a malformed table, an unknown face, a
/// duplicated face or an empty binding.
pub fn menu_font_file(
    skin: &str,
    face: &str,
) -> Result<crate::menu_skin::FontBinding, crate::menu_skin::SkinError> {
    crate::menu_skin::SkinTable::parse(skin)?.unique_file(face)
}

/// The served name for a derived font file, so a browser can supply it.
///
/// The font path is derived from the skin table and so cannot appear in
/// [`served_name`]'s fixed list; this maps any derived path to its served name,
/// the file's own name under `menu/fonts/`.
#[must_use]
pub fn served_font_name(font_path: &str) -> Option<String> {
    let base = font_path
        .rsplit(['\\', '/'])
        .next()
        .filter(|b| !b.is_empty())?;
    Some(format!("menu/fonts/{base}"))
}

/// The style of the text frame named `frame`, resolved across `texts`.
///
/// The frame is the one entry's own `ButtonText` names, so the template it
/// inherits from is whatever that frame says — not an assumed shared template —
/// and a per-entry record override wins.
///
/// # Errors
///
/// [`crate::menu_text::TextError`] when the frame, its chain or one of the records
/// is missing, ambiguous or unusable.
pub fn menu_text_style(
    texts: &[&str],
    frame: &str,
) -> Result<crate::menu_text::TextStyle, crate::menu_text::TextError> {
    crate::menu_text::text_style_in(texts, frame)
}

/// The FDF the menu's frame definitions live in.
pub const MAIN_MENU_FDF: &str = r"UI\FrameDef\Glue\MainMenu.fdf";
/// The shared template file `MainMenu.fdf` inherits from.
pub const STANDARD_TEMPLATES_FDF: &str = r"UI\FrameDef\Glue\StandardTemplates.fdf";
#[derive(Debug, Default)]
#[cfg(test)]
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
        SKIN_TABLE => Some("menu/war3skins.txt"),
        LOGO_TEXTURE => Some("menu/logo.blp"),
        MAIN_MENU_FDF => Some("menu/mainmenu.fdf"),
        GLOBAL_STRINGS_FDF => Some("menu/globalstrings.fdf"),
        STANDARD_TEMPLATES_FDF => Some("menu/standardtemplates.fdf"),
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
#[cfg(test)]
pub fn load_glue_art(install: Option<&std::path::Path>) -> GlueArt {
    let mut art = GlueArt {
        button: art_bytes(install, BUTTON_ART),
        ..Default::default()
    };
    // Each art file comes from whichever source can supply it, so a browser
    // (no filesystem, but a store the page filled) and a desktop (a staged
    // install) both end up with the originals where they exist.
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

    #[test]
    fn the_label_style_comes_from_the_template() {
        let templates = r#"
Frame "TEXT" "StandardButtonTextTemplate" {
    FrameFont "MasterFont", 0.013, "",
    FontJustificationH JUSTIFYCENTER,
    FontJustificationV JUSTIFYMIDDLE,
    FontFlags "FIXEDSIZE",
    FontColor 0.99 0.827 0.0705 1.0,
    FontHighlightColor 1.0 1.0 1.0 1.0,
    FontDisabledColor 0.5 0.5 0.5 1.0,
    FontShadowColor 0.0 0.0 0.0 0.9,
    FontShadowOffset 0.001 -0.001,
}
"#;
        let style = menu_text_style(&[templates], "StandardButtonTextTemplate").unwrap();
        assert_eq!(style.font, "MasterFont");
        assert_eq!(style.height, 0.013);
        assert_eq!(style.colour, [0.99, 0.827, 0.0705, 1.0]);
        assert_eq!(style.highlight, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(style.disabled, [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(style.justify_h, crate::menu_text::JustifyH::Center);
        assert_eq!(style.justify_v, crate::menu_text::JustifyV::Middle);
        assert_eq!(style.shadow_offset, [0.001, -0.001]);
        assert!(style.fixed_size);
    }

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
}
