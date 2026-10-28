//! Hero-bar geometry and status-bar styles from the reviewed client.
//!
//! The bar is a one-column, seven-row grid anchored to the top-left of the console parent.
//! Each row holds a command-button-sized icon with a health bar and a mana bar stacked
//! beneath it. This reads layout and bar styling only: hero ordering, selection input and
//! the level-up point model are not qualified here.
use crate::{
    inventory_layout::Rect,
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
    unit_panel_layout::Reader,
};

/// Texture key and tint of one status bar, read from the shared bar constructor.
#[derive(Clone, Debug)]
pub struct Bar {
    pub fill_key: String,
    /// Packed 0xAARRGGBB.
    pub tint: u32,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// Logical frame coordinates, positive Y upwards.
    pub viewport: [f32; 2],
    pub rows: usize,
    pub health: Bar,
    pub mana: Bar,
    /// Packed 0xAARRGGBB colour of the rectangle behind each bar.
    pub background: u32,
    /// Distance the background extends past the bar on every side.
    pub inset: f32,
    frame: [f32; 2],
    insets: [f32; 4],
    anchor: [f32; 2],
    button: [f32; 2],
    bar: [f32; 2],
    health_offset: [f32; 2],
    mana_offset: [f32; 2],
}

// Constructor operands of the reviewed 1.29.2.9231 client: the hero-bar grid, one hero-bar
// button, and the shared status-bar frame.
const GRID_CALL: u32 = 0x88d050;
const WIDTH_SETTER: u32 = 0x873b40;
const HEIGHT_SETTER: u32 = 0x873830;
const SET_POINT: u32 = 0x873a10;
const BAR_CONSTRUCTOR: u32 = 0x6218d0;
const GRID_INSET_CONSTRUCTOR: u32 = 0x88c7a0;
const BUTTON_BASE_CONSTRUCTOR: u32 = 0x61d3b0;
const HERO_BAR_CONSTRUCTOR: u32 = 0x61e000;
const HERO_BAR_CONSTRUCTOR_CALL: u32 = 0x5e8179;
/// GameUI field holding the console parent, shared with the command-bar creation.
const CONSOLE_PARENT_FIELD: u32 = 0x468;
const HEALTH_BAR_TYPE: u8 = 3;
const MANA_BAR_TYPE: u8 = 4;

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: String| format!("hero bar layout {}: {reason}", origin.source);
        if image.len() > 32 << 20
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
            || sha256(image) != REVIEWED_IMAGE
        {
            return Err(fail("unsupported client identity".into()));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| fail(e.to_string()))?;
        Self::decode(origin.clone(), REVIEWED_IMAGE, |va, len, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated operand at {va:#x}"))
        })
        .map_err(fail)
    }

    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut r = Reader(read);
        for (va, target) in [
            (0x61e09c_u32, GRID_CALL),
            (0x61e0ab, WIDTH_SETTER),
            (0x61e0ba, HEIGHT_SETTER),
            (0x61e030, GRID_INSET_CONSTRUCTOR),
            (0x61e190, 0xab28d0),
            (0x61e1b8, SET_POINT),
            (0x61e211, BUTTON_BASE_CONSTRUCTOR),
            (0x61d497, WIDTH_SETTER),
            (0x61d4a6, HEIGHT_SETTER),
            (0x61e2a9, BAR_CONSTRUCTOR),
            (0x61e2c6, WIDTH_SETTER),
            (0x61e2d9, HEIGHT_SETTER),
            (0x61e2fd, SET_POINT),
            (0x61e32d, BAR_CONSTRUCTOR),
            (0x61e34a, WIDTH_SETTER),
            (0x61e35d, HEIGHT_SETTER),
            (0x61e386, SET_POINT),
            (HERO_BAR_CONSTRUCTOR_CALL, HERO_BAR_CONSTRUCTOR),
        ] {
            if r.call_target(va)? != target {
                return Err(format!("unexpected call target at {va:#x}"));
            }
        }
        // The bar frame is created with the same console-parent field as the command bar.
        if r.word(0x5e8171, &[0xff, 0xb7])? != CONSOLE_PARENT_FIELD {
            return Err("hero bar parent is not the console parent".into());
        }
        let columns = r.bytes(0x61e07e, 2, true)?;
        let rows = r.bytes(0x61e080, 2, true)?;
        if columns[0] != 0x6a || rows[0] != 0x6a || columns[1] != 1 || rows[1] == 0 {
            return Err("hero bar grid dimensions".into());
        }
        let rows = usize::from(rows[1]);
        if r.bytes(0x61e12f, 3, true)? != [0x83, 0xfb, rows as u8] {
            return Err("hero bar grid allocation/loop relation".into());
        }
        let frame = [
            r.number(0x61e0a4, &[0xc7, 4, 0x24])?,
            r.number(0x61e0b3, &[0xc7, 4, 0x24])?,
        ];
        // TOPLEFT to the parent's TOPLEFT. The x passes through the wide-window mapping,
        // which is the identity at the 4:3 logical viewport used here.
        if r.bytes(0x61e1b1, 7, true)? != [0x6a, 0, 0xff, 0x75, 8, 0x6a, 0] {
            return Err("unsupported hero bar anchor pair".into());
        }
        let anchor_x = r.number(0x61e189, &[0xc7, 4, 0x24])?;
        if r.number(0x61e176, &[0xc7, 0x45, 0xf0])? != anchor_x {
            return Err("hero bar anchor x operands differ".into());
        }
        let anchor = [anchor_x, r.number(0x61e1a4, &[0xc7, 0x44, 0x24, 4])?];
        let button = [
            r.number(0x61d490, &[0xc7, 4, 0x24])?,
            r.number(0x61d49f, &[0xc7, 4, 0x24])?,
        ];
        let insets = [
            r.number(0x88c7e2, &[0xc7, 0x86, 0x40, 1, 0, 0])?,
            r.number(0x88c7ec, &[0xc7, 0x86, 0x44, 1, 0, 0])?,
            r.number(0x88c7f6, &[0xc7, 0x86, 0x48, 1, 0, 0])?,
            r.number(0x88c800, &[0xc7, 0x86, 0x4c, 1, 0, 0])?,
        ];
        let viewport = [
            r.referenced(0x640ea8, &[0xf3, 0x0f, 0x10, 5])?,
            r.referenced(0x640e96, &[0xf3, 0x0f, 0x10, 0x1d])?,
        ];

        // Health bar: TOP to the button's BOTTOM. Mana bar: TOP to the health bar's BOTTOM.
        if r.bytes(0x61e2a2, 2, true)? != [0x6a, HEALTH_BAR_TYPE]
            || r.bytes(0x61e326, 2, true)? != [0x6a, MANA_BAR_TYPE]
            || r.bytes(0x61e2f8, 4, true)? != [0x6a, 7, 0x57, 0x6a]
            || r.bytes(0x61e2fc, 1, true)?[0] != 1
            || r.bytes(0x61e37c, 9, true)? != [0x6a, 7, 0xff, 0xb7, 0xd0, 1, 0, 0, 0x6a]
            || r.bytes(0x61e385, 1, true)?[0] != 1
        {
            return Err("hero bar status-bar anchor chain".into());
        }
        let bar = [
            r.number(0x61e2bf, &[0xc7, 4, 0x24])?,
            r.number(0x61e2d2, &[0xc7, 4, 0x24])?,
        ];
        if [
            r.number(0x61e343, &[0xc7, 4, 0x24])?,
            r.number(0x61e356, &[0xc7, 4, 0x24])?,
        ] != bar
        {
            return Err("hero bar health and mana bar sizes differ".into());
        }
        let health_offset = [
            r.number(0x61e2f1, &[0xc7, 4, 0x24])?,
            r.number(0x61e2e9, &[0xc7, 0x44, 0x24, 4])?,
        ];
        let mana_offset = [
            r.number(0x61e375, &[0xc7, 4, 0x24])?,
            r.number(0x61e36d, &[0xc7, 0x44, 0x24, 4])?,
        ];

        // Shared bar constructor: background rectangle, texture key table, tint cases.
        let keys = r.word(0x621ac2, &[0xff, 0x34, 0x85])?;
        let cases = r.word(0x621af3, &[0xff, 0x24, 0x85])?;
        if r.bytes(0x621aea, 3, true)? != [0x83, 0xf8, 5] {
            return Err("status-bar type domain".into());
        }
        let alpha = r.bytes(0x621ae6, 4, true)?;
        if alpha[..3] != [0xc6, 0x45, 0xb] {
            return Err("status-bar alpha instruction".into());
        }
        let mut style = |kind: u8| -> Result<Bar, String> {
            let slot = u32::from(kind) * 4;
            let key = table_word(&mut r, keys, slot)?;
            let case = table_word(&mut r, cases, slot)?;
            let code = r.bytes(case, 10, true)?;
            if code[..4] != [0x66, 0xc7, 0x45, 9] || code[6..9] != [0xc6, 0x45, 8] {
                return Err(format!("status-bar tint case for type {kind}"));
            }
            Ok(Bar {
                fill_key: r.string_at(key)?,
                tint: u32::from_le_bytes([code[9], code[4], code[5], alpha[3]]),
            })
        };
        let health = style(HEALTH_BAR_TYPE)?;
        let mana = style(MANA_BAR_TYPE)?;
        // The background rectangle takes all four corners of the bar, outset by the inset.
        for (va, point) in [
            (0x621a25_u32, 0u8),
            (0x621a4d, 2),
            (0x621a7c, 6),
            (0x621aae, 8),
        ] {
            if r.bytes(va, 5, true)? != [0x6a, point, 0x56, 0x6a, point] {
                return Err("status-bar background anchors".into());
            }
        }
        let inset = r.referenced(0x621a00, &[0xf3, 0x0f, 0x10, 5])?;
        let background = r.word(0x6219f4, &[0xc7, 0x45, 0x10])?;

        if frame
            .iter()
            .chain(&button)
            .chain(&bar)
            .chain(&viewport)
            .any(|v| *v <= 0.)
            || insets.iter().any(|v| *v < 0.)
            || inset < 0.
            || button[0] > (frame[0] - insets[0] - insets[2])
            || button[1] > (frame[1] - insets[1] - insets[3]) / rows as f32
            || bar[0] > button[0]
        {
            return Err("invalid hero bar geometry".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            viewport,
            rows,
            health,
            mana,
            background,
            inset,
            frame,
            insets,
            anchor,
            button,
            bar,
            health_offset,
            mana_offset,
        })
    }

    /// Icon rectangle for a zero-based row, top row first.
    pub fn button(&self, row: usize) -> Result<Rect, String> {
        if row >= self.rows {
            return Err("hero bar row exceeds the source grid".into());
        }
        let top = self.viewport[1] + self.anchor[1]
            - self.insets[1]
            - row as f32 * ((self.frame[1] - self.insets[1] - self.insets[3]) / self.rows as f32);
        let rect = Rect {
            x: self.anchor[0] + self.insets[0],
            y: top - self.button[1],
            width: self.button[0],
            height: self.button[1],
        };
        finite(rect)
    }

    pub fn health_bar(&self, row: usize) -> Result<Rect, String> {
        let button = self.button(row)?;
        self.bar_below(button, self.health_offset)
    }

    pub fn mana_bar(&self, row: usize) -> Result<Rect, String> {
        let health = self.health_bar(row)?;
        self.bar_below(health, self.mana_offset)
    }

    /// Bar whose TOP point sits at `above`'s BOTTOM point plus `offset` (positive Y up).
    fn bar_below(&self, above: Rect, offset: [f32; 2]) -> Result<Rect, String> {
        finite(Rect {
            x: above.x + above.width / 2. + offset[0] - self.bar[0] / 2.,
            y: above.y + offset[1] - self.bar[1],
            width: self.bar[0],
            height: self.bar[1],
        })
    }
}

/// Pointer stored at `slot` bytes into the data table at `table`.
fn table_word<F: FnMut(u32, usize, bool) -> Result<Vec<u8>, String>>(
    r: &mut Reader<F>,
    table: u32,
    slot: u32,
) -> Result<u32, String> {
    let va = table.checked_add(slot).ok_or("status-bar table overflow")?;
    let bytes: [u8; 4] = r
        .bytes(va, 4, false)?
        .try_into()
        .map_err(|_| "truncated status-bar table")?;
    Ok(u32::from_le_bytes(bytes))
}

fn finite(rect: Rect) -> Result<Rect, String> {
    if [rect.x, rect.y, rect.width, rect.height]
        .iter()
        .any(|v| !v.is_finite())
    {
        return Err("hero bar arithmetic overflow".into());
    }
    Ok(rect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn put(map: &mut BTreeMap<u32, u8>, va: u32, bytes: &[u8]) {
        for (i, byte) in bytes.iter().enumerate() {
            map.insert(va + i as u32, *byte);
        }
    }
    fn op(map: &mut BTreeMap<u32, u8>, va: u32, prefix: &[u8], value: u32) {
        put(map, va, prefix);
        put(map, va + prefix.len() as u32, &value.to_le_bytes());
    }
    fn call(map: &mut BTreeMap<u32, u8>, va: u32, target: u32) {
        op(map, va, &[0xe8], target.wrapping_sub(va + 5));
    }
    fn number(map: &mut BTreeMap<u32, u8>, va: u32, prefix: &[u8], value: f32) {
        op(map, va, prefix, value.to_bits());
    }

    /// Synthetic client memory with geometry unlike the reviewed image.
    fn fixture() -> BTreeMap<u32, u8> {
        let mut map = BTreeMap::new();
        for (va, target) in [
            (0x61e09c_u32, GRID_CALL),
            (0x61e0ab, WIDTH_SETTER),
            (0x61e0ba, HEIGHT_SETTER),
            (0x61e030, GRID_INSET_CONSTRUCTOR),
            (0x61e190, 0xab28d0),
            (0x61e1b8, SET_POINT),
            (0x61e211, BUTTON_BASE_CONSTRUCTOR),
            (0x61d497, WIDTH_SETTER),
            (0x61d4a6, HEIGHT_SETTER),
            (0x61e2a9, BAR_CONSTRUCTOR),
            (0x61e2c6, WIDTH_SETTER),
            (0x61e2d9, HEIGHT_SETTER),
            (0x61e2fd, SET_POINT),
            (0x61e32d, BAR_CONSTRUCTOR),
            (0x61e34a, WIDTH_SETTER),
            (0x61e35d, HEIGHT_SETTER),
            (0x61e386, SET_POINT),
            (HERO_BAR_CONSTRUCTOR_CALL, HERO_BAR_CONSTRUCTOR),
        ] {
            call(&mut map, va, target);
        }
        op(&mut map, 0x5e8171, &[0xff, 0xb7], CONSOLE_PARENT_FIELD);
        put(&mut map, 0x61e07e, &[0x6a, 1]);
        put(&mut map, 0x61e080, &[0x6a, 4]);
        put(&mut map, 0x61e12f, &[0x83, 0xfb, 4]);
        number(&mut map, 0x61e0a4, &[0xc7, 4, 0x24], 0.05);
        number(&mut map, 0x61e0b3, &[0xc7, 4, 0x24], 0.32);
        put(&mut map, 0x61e1b1, &[0x6a, 0, 0xff, 0x75, 8, 0x6a, 0]);
        number(&mut map, 0x61e189, &[0xc7, 4, 0x24], 0.01);
        number(&mut map, 0x61e176, &[0xc7, 0x45, 0xf0], 0.01);
        number(&mut map, 0x61e1a4, &[0xc7, 0x44, 0x24, 4], -0.03);
        number(&mut map, 0x61d490, &[0xc7, 4, 0x24], 0.04);
        number(&mut map, 0x61d49f, &[0xc7, 4, 0x24], 0.04);
        for (va, prefix) in [
            (0x88c7e2_u32, [0xc7, 0x86, 0x40, 1, 0, 0]),
            (0x88c7ec, [0xc7, 0x86, 0x44, 1, 0, 0]),
            (0x88c7f6, [0xc7, 0x86, 0x48, 1, 0, 0]),
            (0x88c800, [0xc7, 0x86, 0x4c, 1, 0, 0]),
        ] {
            number(&mut map, va, &prefix, 0.);
        }
        op(&mut map, 0x640ea8, &[0xf3, 0x0f, 0x10, 5], 0x7000);
        op(&mut map, 0x640e96, &[0xf3, 0x0f, 0x10, 0x1d], 0x7004);
        put(&mut map, 0x7000, &0.8f32.to_le_bytes());
        put(&mut map, 0x7004, &0.6f32.to_le_bytes());
        put(&mut map, 0x61e2a2, &[0x6a, HEALTH_BAR_TYPE]);
        put(&mut map, 0x61e326, &[0x6a, MANA_BAR_TYPE]);
        put(&mut map, 0x61e2f8, &[0x6a, 7, 0x57, 0x6a]);
        put(&mut map, 0x61e2fc, &[1]);
        put(
            &mut map,
            0x61e37c,
            &[0x6a, 7, 0xff, 0xb7, 0xd0, 1, 0, 0, 0x6a],
        );
        put(&mut map, 0x61e385, &[1]);
        for va in [0x61e2bf, 0x61e343] {
            number(&mut map, va, &[0xc7, 4, 0x24], 0.04);
        }
        for va in [0x61e2d2, 0x61e356] {
            number(&mut map, va, &[0xc7, 4, 0x24], 0.005);
        }
        number(&mut map, 0x61e2f1, &[0xc7, 4, 0x24], 0.);
        number(&mut map, 0x61e2e9, &[0xc7, 0x44, 0x24, 4], -0.002);
        number(&mut map, 0x61e375, &[0xc7, 4, 0x24], 0.);
        number(&mut map, 0x61e36d, &[0xc7, 0x44, 0x24, 4], -0.003);
        op(&mut map, 0x621ac2, &[0xff, 0x34, 0x85], 0x4000);
        op(&mut map, 0x621af3, &[0xff, 0x24, 0x85], 0x4100);
        put(&mut map, 0x621aea, &[0x83, 0xf8, 5]);
        put(&mut map, 0x621ae6, &[0xc6, 0x45, 0xb, 0xee]);
        for (kind, key, case, [g, r, b]) in [
            (3u32, 0x4200u32, 0x4300u32, [0x11u8, 0x22u8, 0x33u8]),
            (4, 0x4220, 0x4320, [0x44, 0x55, 0x66]),
        ] {
            put(&mut map, 0x4000 + kind * 4, &key.to_le_bytes());
            put(&mut map, 0x4100 + kind * 4, &case.to_le_bytes());
            put(&mut map, key, format!("SyntheticFill{kind}\0").as_bytes());
            put(&mut map, case, &[0x66, 0xc7, 0x45, 9, g, r]);
            put(&mut map, case + 6, &[0xc6, 0x45, 8, b]);
        }
        for (va, point) in [
            (0x621a25_u32, 0u8),
            (0x621a4d, 2),
            (0x621a7c, 6),
            (0x621aae, 8),
        ] {
            put(&mut map, va, &[0x6a, point, 0x56, 0x6a, point]);
        }
        op(&mut map, 0x621a00, &[0xf3, 0x0f, 0x10, 5], 0x7010);
        put(&mut map, 0x7010, &0.001f32.to_le_bytes());
        op(&mut map, 0x6219f4, &[0xc7, 0x45, 0x10], 0xff102030);
        map
    }

    fn rules(map: &BTreeMap<u32, u8>) -> Result<Rules, String> {
        Rules::decode(
            AssetIdentity {
                source: "fixture".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            REVIEWED_IMAGE,
            |va, len, _| {
                (0..len as u32)
                    .map(|i| {
                        map.get(&(va + i))
                            .copied()
                            .ok_or_else(|| format!("unmapped {va:#x}"))
                    })
                    .collect()
            },
        )
    }

    #[test]
    fn rows_stack_downward_from_the_top_left_anchor() {
        let rules = rules(&fixture()).unwrap();
        assert_eq!(rules.rows, 4);
        let first = rules.button(0).unwrap();
        assert_eq!(first.x, 0.01);
        assert!((first.y - (0.6 - 0.03 - 0.04)).abs() < 1e-6);
        let second = rules.button(1).unwrap();
        assert!((first.y - second.y - 0.32 / 4.).abs() < 1e-6);
        assert!(rules.button(4).is_err());
    }

    #[test]
    fn bars_hang_below_the_icon_and_each_other() {
        let rules = rules(&fixture()).unwrap();
        let button = rules.button(0).unwrap();
        let health = rules.health_bar(0).unwrap();
        let mana = rules.mana_bar(0).unwrap();
        assert!((health.y - (button.y - 0.002 - 0.005)).abs() < 1e-6);
        assert!((mana.y - (health.y - 0.003 - 0.005)).abs() < 1e-6);
        assert_eq!((health.x, health.width), (button.x, button.width));
    }

    #[test]
    fn bar_styles_come_from_the_type_tables() {
        let rules = rules(&fixture()).unwrap();
        assert_eq!(rules.health.fill_key, "SyntheticFill3");
        assert_eq!(rules.mana.fill_key, "SyntheticFill4");
        assert_eq!(rules.health.tint, 0xee_22_11_33);
        assert_eq!(rules.mana.tint, 0xee_55_44_66);
        assert_eq!((rules.background, rules.inset), (0xff102030, 0.001));
    }

    #[test]
    fn changed_instructions_refuse() {
        for (va, byte) in [(0x61e1b1_u32, 6u8), (0x61e2a2, 9), (0x621aea, 0x84)] {
            let mut map = fixture();
            map.insert(va, byte);
            assert!(rules(&map).is_err(), "{va:#x}");
        }
        let mut map = fixture();
        number(&mut map, 0x61e176, &[0xc7, 0x45, 0xf0], 0.02);
        assert!(rules(&map).is_err());
        let mut map = fixture();
        call(&mut map, 0x61e2a9, 0x6218d1);
        assert!(rules(&map).is_err());
        let mut map = fixture();
        map.remove(&0x7010);
        assert!(rules(&map).is_err());
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE from the selected local install"]
    fn selected_client_matches_the_decoded_bar_layout() {
        let path =
            std::path::PathBuf::from(std::env::var_os("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE"));
        let image = std::fs::read(&path).expect("selected executable");
        let identity = crate::campaign_inputs::client_identity(&image, &path.display().to_string());
        let rules = Rules::reviewed_legacy(&image, identity).unwrap();
        println!("{rules:#?}");
        for row in 0..rules.rows {
            println!(
                "row {row}: {:?} {:?} {:?}",
                rules.button(row).unwrap(),
                rules.health_bar(row).unwrap(),
                rules.mana_bar(row).unwrap()
            );
        }
        assert_eq!(rules.rows, 7);
        assert_eq!(rules.health.fill_key, "SimpleHpBarConsoleSmall");
        assert_eq!(rules.mana.fill_key, "SimpleManaBarConsoleSmall");
        assert_eq!(rules.health.tint, 0xff00ff00);
        assert_eq!(rules.mana.tint, 0xff0000ff);
    }
}
