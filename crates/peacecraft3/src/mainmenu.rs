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
/// How long the logo's `Birth` sequence runs, in milliseconds, measured from the
/// model's `SEQS` chunk (`Birth` spans frames 0..500) — the crate's frame times
/// are milliseconds.
#[cfg(test)]
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
#[cfg(test)]
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
#[cfg(test)]
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
/// Signed separation: +0.005 at BOTTOMRIGHT overlaps the previous frame.
pub const BUTTON_GAP: f32 = -0.005;
pub const FRAME_W: f32 = 0.8;
pub const FRAME_H: f32 = 0.6;
pub const EXIT_W: f32 = 0.24;
pub const EXIT_BOTTOM: f32 = 0.05;
/// `SinglePlayerBackdrop`'s right-anchored x offset.
pub const FIRST_RIGHT: f32 = 0.015;
/// `SinglePlayerBackdrop`'s top offset.
pub const FIRST_TOP: f32 = 0.110_625;

/// One entry on the main menu, in the order the original stacks them.
///
/// Only the id is here: the label is **not** carried as text. The authored string
/// lives in `GlobalStrings.fdf`, and which key belongs to which entry is read from
/// the menu's own frame definitions — the button's `ButtonText` names its text
/// frame, and that frame's `Text` record holds the key — so no key is duplicated
/// here either.
pub const MENU: [&str; 6] = [
    "single_player",
    "battle_net",
    "local_area_network",
    "options",
    "credits",
    "exit",
];

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
    crate::menu_skin::SkinTable::parse(skin)?.font_file_unique(face)
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

/// Why an entry's label could not be derived.
#[derive(Debug, Clone, PartialEq)]
pub enum LabelProblem {
    /// The menu's frame definitions could not be read.
    Fdf(crate::fdf::FdfError),
    /// The button frame names no `ButtonText`.
    NoButtonText(String),
    /// The text frame records no `Text`.
    NoTextKey(String),
    /// The strings text does not define the key.
    NoString(String),
    /// The authored markup will not parse.
    Markup(crate::menu_text::TextError),
}

impl std::fmt::Display for LabelProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fdf(e) => write!(f, "the menu frames could not be read: {e}"),
            Self::NoButtonText(b) => write!(f, "{b:?} names no ButtonText"),
            Self::NoTextKey(t) => write!(f, "text frame {t:?} records no Text"),
            Self::NoString(k) => write!(f, "GlobalStrings has no {k:?}"),
            Self::Markup(e) => write!(f, "the label markup will not parse: {e}"),
        }
    }
}

impl std::error::Error for LabelProblem {}

impl From<crate::fdf::FdfError> for LabelProblem {
    fn from(e: crate::fdf::FdfError) -> Self {
        Self::Fdf(e)
    }
}

/// The text frame entry `i`'s button names through its `ButtonText`.
///
/// This is the frame whose style the entry draws with, so a per-entry override of
/// a font record is honoured instead of every entry sharing one template.
///
/// # Errors
///
/// [`LabelProblem::NoButtonText`] when the button names none.
pub fn menu_text_frame(fdf: &crate::fdf::Fdf, i: usize) -> Result<String, LabelProblem> {
    let button = MENU_FRAMES[i].0;
    fdf.button_text(button)
        .ok_or_else(|| LabelProblem::NoButtonText(button.to_string()))
}

/// The `GlobalStrings.fdf` key entry `i` declares.
///
/// Reached through the frame definitions rather than a table here: the button's
/// `ButtonText` names its text frame, and that frame's `Text` record holds the key.
///
/// # Errors
///
/// [`LabelProblem::NoButtonText`] or [`LabelProblem::NoTextKey`] when either
/// record is absent. Nothing is substituted.
pub fn menu_label_key(fdf: &crate::fdf::Fdf, i: usize) -> Result<String, LabelProblem> {
    let text_frame = menu_text_frame(fdf, i)?;
    fdf.text_value(&text_frame)
        .ok_or(LabelProblem::NoTextKey(text_frame))
}

/// The authored label for entry `i`, parsed from its `GlobalStrings` value.
///
/// # Errors
///
/// [`LabelProblem`] when the key, the string or its markup cannot be read.
pub fn menu_label(
    strings: &str,
    fdf: &crate::fdf::Fdf,
    i: usize,
) -> Result<crate::menu_text::Label, LabelProblem> {
    let key = menu_label_key(fdf, i)?;
    let raw = crate::menu_text::global_string(strings, &key).ok_or(LabelProblem::NoString(key))?;
    crate::menu_text::Label::parse(&raw).map_err(LabelProblem::Markup)
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

/// Where a button frame sits, in fractions of the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonGeometry {
    /// The left edge, in fractions of the window width.
    pub left: f32,
    /// The top edge, in fractions of the authored viewport height.
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

/// The geometry of [`MENU`]'s `i`th entry, exactly as the FDF stacks them.
///
/// Buttons are right-anchored, so `left` is derived from `FIRST_RIGHT` and the
/// button width. Each button's top edge is the previous one's bottom plus
/// `BUTTON_GAP`; the first is `FIRST_TOP`.
///
/// Original 1.29.2.9231 CFrame rendering divides X by 0.8 and Y by 0.6;
/// its relative anchor evaluator adds the FDF's offsets. See
/// `docs/re-notes/glue-coordinate-runtime.md`. Quit has its own bottom anchor.
#[must_use]
pub fn button_geometry(i: usize) -> ButtonGeometry {
    let width = if i == 5 { EXIT_W } else { BUTTON_W } / FRAME_W;
    let height = BUTTON_H / FRAME_H;
    let top = if i == 5 {
        1.0 - EXIT_BOTTOM / FRAME_H - height
    } else {
        (FIRST_TOP + i as f32 * (BUTTON_H + BUTTON_GAP)) / FRAME_H
    };
    ButtonGeometry {
        left: 1.0 - FIRST_RIGHT / FRAME_W - width,
        top,
        width,
        height,
    }
}

/// The FDF the menu's frame definitions live in.
pub const MAIN_MENU_FDF: &str = r"UI\FrameDef\Glue\MainMenu.fdf";
/// The shared template file `MainMenu.fdf` inherits from.
pub const STANDARD_TEMPLATES_FDF: &str = r"UI\FrameDef\Glue\StandardTemplates.fdf";

/// The `(button, plate)` frame each [`MENU`] entry is defined by, in the same
/// order as [`MENU`].
pub const MENU_FRAMES: [(&str, &str); 6] = [
    ("SinglePlayerButton", "SinglePlayerBackdrop"),
    ("BattleNetButton", "BattleNetBackdrop"),
    ("LocalAreaNetworkButton", "LocalAreaNetworkBackdrop"),
    ("OptionsButton", "OptionsBackdrop"),
    ("CreditsButton", "CreditsBackdrop"),
    ("ExitButton", "ExitBackdrop"),
];

/// The inner fill's rectangle for [`MENU`]'s `i`th entry, in **frame units**,
/// derived from the parsed FDF: `(left, top, width, height)` relative to the
/// entry's outer plate, with `top` measured downward as the UI draws it.
///
/// Every number comes from the frame definitions — the plate's size, the inner
/// button's size and anchor chain, and the backdrop's insets. Nothing is a
/// constant here, and a frame or property the FDF does not define is an error,
/// not a default.
///
/// # Errors
///
/// [`crate::fdf::FdfError`] when a frame, its `ControlBackdrop`, its size or its
/// insets are missing, or when the anchor chain does not terminate. Unequal
/// insets are also refused: their written order is not established, so assuming
/// one would be inventing an asset fact.
pub fn button_fill(
    fdf: &crate::fdf::Fdf,
    i: usize,
) -> Result<(f32, f32, f32, f32), crate::fdf::FdfError> {
    use crate::fdf::FdfError;
    let (button, plate) = MENU_FRAMES[i];
    let viewport = (FRAME_W, FRAME_H);
    let plate_rect = fdf.rect(plate, viewport)?;
    let inner = fdf.rect(button, viewport)?;
    let backdrop = fdf
        .control_backdrop(button)
        .ok_or_else(|| FdfError::Missing {
            frame: button.to_string(),
            property: "ControlBackdrop",
        })?;
    let [a, b, c, d] = fdf.insets(&backdrop)?;
    if !(a == b && b == c && c == d) {
        return Err(FdfError::Missing {
            frame: backdrop,
            property: "equal BackdropBackgroundInsets",
        });
    }
    let inset = a;
    let width = inner.width - 2.0 * inset;
    let height = inner.height - 2.0 * inset;
    Ok((
        inner.left + inset - plate_rect.left,
        plate_rect.top() - (inner.bottom + inset + height),
        width,
        height,
    ))
}

/// The outer plate's `(width, height)` for [`MENU`]'s `i`th entry, from the FDF.
///
/// # Errors
///
/// [`crate::fdf::FdfError`] when the plate or its size is not defined.
pub fn plate_size(fdf: &crate::fdf::Fdf, i: usize) -> Result<(f32, f32), crate::fdf::FdfError> {
    let (_, plate) = MENU_FRAMES[i];
    Ok((fdf.width(plate)?, fdf.height(plate)?))
}

/// Marker for a menu button, carrying its index in [`MENU`], so a click can be
/// resolved to the entry it landed on.
#[derive(Component, Debug)]
pub struct MenuButton(pub usize);
/// Which entry a click landed on, given the pointer position in window
/// fractions. `None` means the pointer was on none of them.
#[must_use]
#[cfg(test)]
pub fn hit(x: f32, y: f32) -> Option<usize> {
    MENU.iter()
        .enumerate()
        .find(|(i, _)| {
            let g = button_geometry(*i);
            x >= g.left && x <= g.left + g.width && y >= g.top && y <= g.top + g.height
        })
        .map(|(i, _)| i)
}

/// The glue-screen art the original menu uses, read out of an install.
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

    /// A menu tree and a strings file written with the **shipped markup shapes**,
    /// including `KEY_EXIT`'s interior hotkey.
    const FDF_LABELS: &str = r#"
Frame "FRAME" "MainMenuFrame" {
    Frame "GLUETEXTBUTTON" "SinglePlayerButton" {
        ButtonText "SinglePlayerButtonText",
        Frame "TEXT" "SinglePlayerButtonText" {
            Text "KEY_SINGLE_PLAYER",
        }
    }
    Frame "GLUETEXTBUTTON" "ExitButton" {
        ButtonText "ExitButtonText",
        Frame "TEXT" "ExitButtonText" {
            Text "KEY_EXIT",
        }
    }
}
"#;
    const STRINGS_LABELS: &str = r#"
KEY_SINGLE_PLAYER        "|CffffffffS|Ringle Player",
KEY_EXIT                 "E|Cffffffffx|Rit",
"#;

    /// The key is read from the menu's own frames — the button's `ButtonText`
    /// names its text frame, and that frame's `Text` holds the key.
    #[test]
    fn the_label_key_comes_from_the_menu_frames() {
        let f = crate::fdf::Fdf::parse(FDF_LABELS).unwrap();
        assert_eq!(menu_label_key(&f, 0).unwrap(), "KEY_SINGLE_PLAYER");
        assert_eq!(menu_label_key(&f, 5).unwrap(), "KEY_EXIT");
        // An entry whose button the frames do not define is reported.
        assert!(menu_label_key(&f, 2).is_err());
    }

    /// The labels come from the strings text, and every authored run is kept:
    /// `KEY_EXIT` is `E` + hotkey `x` + `it`, so it reads "Exit" with the `E`.
    #[test]
    fn the_labels_come_from_the_strings_including_the_interior_hotkey() {
        let f = crate::fdf::Fdf::parse(FDF_LABELS).unwrap();
        let single = menu_label(STRINGS_LABELS, &f, 0).unwrap();
        assert_eq!(single.visible(), "Single Player");
        assert_eq!(single.hotkey(), Some('S'));
        let exit = menu_label(STRINGS_LABELS, &f, 5).unwrap();
        assert_eq!(exit.visible(), "Exit");
        assert_eq!(exit.hotkey(), Some('x'));
        assert_eq!(
            exit.spans.len(),
            3,
            "prefix, hotkey and suffix are all kept"
        );
        assert!(!exit.spans[0].hotkey && exit.spans[1].hotkey && !exit.spans[2].hotkey);
    }

    /// A key the strings text does not define is reported, never substituted.
    #[test]
    fn a_missing_string_is_reported() {
        let f = crate::fdf::Fdf::parse(FDF_LABELS).unwrap();
        assert!(matches!(
            menu_label("KEY_OTHER \"x\",\n", &f, 0),
            Err(LabelProblem::NoString(_))
        ));
    }

    /// The label's style is the template's own, read from the asset — this is
    /// what replaced the copied `LABEL_FONT_FRACTION` and `HOTKEY`.
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

    /// The stacking relation, asserted against the FDF's own numbers: each
    /// upper button starts 0.005 above the previous one's bottom edge.
    #[test]
    fn the_stack_matches_the_frame_definition() {
        let first = button_geometry(0);
        assert!((first.top - FIRST_TOP / FRAME_H).abs() < 1e-6);
        assert!((first.left - (1.0 - (FIRST_RIGHT + BUTTON_W) / FRAME_W)).abs() < 1e-6);
        let second = button_geometry(1);
        assert!((second.top - (first.top + (BUTTON_H + BUTTON_GAP) / FRAME_H)).abs() < 1e-6);
        for i in 0..5 {
            let g = button_geometry(i);
            assert_eq!(g.left, first.left, "every button is right-anchored alike");
            assert!(g.top + g.height < 1.0, "button {i} runs off the bottom");
        }
    }

    /// The last entry's bottom, which is what decides whether the menu fits.
    #[test]
    fn the_menu_fits_inside_the_window() {
        let last = button_geometry(MENU.len() - 1);
        assert!(last.top + last.height <= 1.0);
    }

    /// A minimal FDF standing in for the two shipped files: the templates the
    /// menu's frames inherit, and one entry's chain.
    const FDF_TEMPLATES: &str = r#"
Frame "BACKDROP" "StandardMenuButtonBaseBackdrop" {
    Width 0.256,
    Height 0.064,
}
Frame "BACKDROP" "StandardButtonBackdropTemplate" {
    BackdropBackgroundInsets 0.004 0.004 0.004 0.004,
}
Frame "GLUEBUTTON" "StandardIconicButtonTemplate" {
    Height 0.031,
    Width 0.031,
}
Frame "TEXTBUTTON" "StandardButtonTemplate" {
    Height 0.031,
    Width 0.179,
    ControlBackdrop "ButtonBackdropTemplate",
    Frame "BACKDROP" "ButtonBackdropTemplate" INHERITS "StandardButtonBackdropTemplate" {
    }
}
"#;
    const FDF_MENU: &str = r#"
Frame "FRAME" "MainMenuFrame" {
    Frame "BACKDROP" "SinglePlayerBackdrop" INHERITS "StandardMenuButtonBaseBackdrop" {
        SetPoint TOPRIGHT,"MainMenuFrame",TOPRIGHT, -0.015, -0.110625,
        Frame "GLUEBUTTON" "EditionButton" INHERITS "StandardIconicButtonTemplate" {
            SetPoint TOPRIGHT, "SinglePlayerBackdrop", TOPRIGHT, -0.012, -0.0165,
        }
        Frame "GLUETEXTBUTTON" "SinglePlayerButton" INHERITS WITHCHILDREN "StandardButtonTemplate" {
            Width 0.149,
            SetPoint TOPRIGHT, "EditionButton", TOPLEFT, 0.001, 0.0,
        }
    }
}
"#;

    fn fixture_fdf() -> crate::fdf::Fdf {
        let mut f = crate::fdf::Fdf::default();
        f.parse_into(FDF_TEMPLATES).unwrap();
        f.parse_into(FDF_MENU).unwrap();
        f
    }

    /// The fill is derived from the FDF, and reproduces the authored box:
    /// the chain `0.256 - 0.012 - 0.031 + 0.001 - 0.149` puts the inner button's
    /// left at `0.065`, so with the `0.004` inset the fill's left is `0.069`.
    #[test]
    fn the_inner_fill_is_derived_from_the_fdf() {
        let f = fixture_fdf();
        let (left, top, width, height) = button_fill(&f, 0).unwrap();
        assert!((left - 0.069).abs() < 1e-6, "left {left}");
        assert!((top - 0.0205).abs() < 1e-6, "top {top}");
        assert!((width - 0.141).abs() < 1e-6, "width {width}");
        assert!((height - 0.023).abs() < 1e-6, "height {height}");
    }

    /// Against the **shipped** FDFs: the derivation reproduces the authored
    /// fills. Ignored because it needs a local install; run with `--ignored` and
    /// `PEACECRAFT3_LEGACY_DIR` set. It fails when the files are absent rather
    /// than passing silently.
    #[test]
    #[ignore = "needs a local Warcraft III install; run with --ignored and PEACECRAFT3_LEGACY_DIR set"]
    fn the_shipped_fdfs_derive_the_authored_fills() {
        let install = std::env::var_os("PEACECRAFT3_LEGACY_DIR").map(std::path::PathBuf::from);
        let mut f = crate::fdf::Fdf::default();
        for path in [STANDARD_TEMPLATES_FDF, MAIN_MENU_FDF] {
            let bytes = art_bytes(install.as_deref(), path)
                .unwrap_or_else(|| panic!("{path} not found; set PEACECRAFT3_LEGACY_DIR"));
            f.parse_into(std::str::from_utf8(&bytes).expect("FDF is text"))
                .expect("FDF parses");
        }
        // Left is 0.069 for the five 0.256 plates and 0.064 for Quit's 0.24 one;
        // widths are 0.141, 0.171 and 0.160; the vertical box is shared.
        for i in 0..5 {
            let (l, t, _, h) = button_fill(&f, i).expect("fill");
            assert!((l - 0.069).abs() < 1e-4, "entry {i} left {l}");
            assert!((t - 0.0205).abs() < 1e-4, "entry {i} top {t}");
            assert!((h - 0.023).abs() < 1e-4, "entry {i} height {h}");
        }
        assert!((button_fill(&f, 0).unwrap().2 - 0.141).abs() < 1e-4);
        assert!((button_fill(&f, 2).unwrap().2 - 0.171).abs() < 1e-4);
        assert!((button_fill(&f, 5).unwrap().0 - 0.064).abs() < 1e-4);
        assert!((button_fill(&f, 5).unwrap().2 - 0.160).abs() < 1e-4);
        assert!((plate_size(&f, 5).unwrap().0 - 0.24).abs() < 1e-6);
    }

    /// A frame the FDF does not define is reported, never defaulted.
    #[test]
    fn a_missing_menu_frame_is_reported() {
        let f = fixture_fdf();
        // The fixture defines only entry 0.
        assert!(button_fill(&f, 2).is_err());
    }

    #[test]
    fn clicks_land_on_the_entries_and_between_them_on_nothing() {
        let g = button_geometry(1);
        assert_eq!(hit(g.left + g.width / 2.0, g.top + g.height / 2.0), Some(1));
        let quit = button_geometry(5);
        assert_eq!(hit(quit.left + quit.width / 2.0, quit.top - 0.01), None);
        assert_eq!(hit(0.0, 0.0), None);
        assert_eq!(hit(g.left + g.width + 0.01, g.top), None);
    }

    #[test]
    fn clicks_do_not_straddle_a_button_boundary() {
        let g = button_geometry(0);
        assert_eq!(hit(g.left + g.width, g.top + g.height / 2.0), Some(0));
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
