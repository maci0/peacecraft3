//! Multiple-selection group panel geometry from the reviewed client.
//!
//! The panel is a grid of group buttons inside the unit information frame. Each button is a
//! frame holding a subgroup highlight, a command-button icon and two status bars. This reads
//! layout and the highlight's skin key and tint only: which units fill the cells, the click
//! behaviour and the bar styling (shared with the hero bar, see [`crate::hero_bar_layout`])
//! are not qualified here.
use crate::{
    inventory_layout::Rect,
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
    unit_panel_layout::Reader,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// Logical frame coordinates, positive Y upwards.
    pub viewport: [f32; 2],
    pub columns: usize,
    pub rows: usize,
    /// Skin key of the subgroup highlight texture.
    pub highlight_key: String,
    /// Packed 0xAARRGGBB tint of the highlight.
    pub highlight_tint: u32,
    /// Value stored into the highlight's blend-mode field. Its meaning is not decoded.
    pub highlight_blend: u32,
    grid: [f32; 2],
    grid_offset: [f32; 2],
    button: [f32; 2],
    highlight: [f32; 2],
    icon: [f32; 2],
    icon_offset: [f32; 2],
    bar: [f32; 2],
    health_offset: [f32; 2],
    mana_offset: [f32; 2],
}

// Operands of the reviewed 1.29.2.9231 client: the group-panel constructor (`0x61ec50`) and
// the loop that builds its buttons (`0x638150`).
const WIDTH_SETTER: u32 = 0x873b40;
const HEIGHT_SETTER: u32 = 0x873830;
const SET_POINT: u32 = 0x873a10;
const SET_ALL_POINTS: u32 = 0x873750;
const SET_COLOUR: u32 = 0x886360;
const SET_BLEND: u32 = 0x887410;
const GRID_INSET_CONSTRUCTOR: u32 = 0x88c7a0;
const GRID_CALL: u32 = 0x88d050;
const GRID_CELL: u32 = 0x88c970;
const COMMAND_BUTTON_CONSTRUCTOR: u32 = 0x622690;
const BAR_CONSTRUCTOR: u32 = 0x6218d0;
const GROUP_CONSTRUCTOR: u32 = 0x61ec50;
const GROUP_CONSTRUCTOR_CALL: u32 = 0x61e719;
const BUTTON_LOOP: u32 = 0x638150;
const HEALTH_BAR_TYPE: u8 = 3;
const MANA_BAR_TYPE: u8 = 4;

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: String| format!("group panel layout {}: {reason}", origin.source);
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
            (GROUP_CONSTRUCTOR_CALL, GROUP_CONSTRUCTOR),
            (0x61ec97, SET_ALL_POINTS),
            (0x61ece5, GRID_INSET_CONSTRUCTOR),
            (0x61ed02, WIDTH_SETTER),
            (0x61ed15, HEIGHT_SETTER),
            (0x61ed39, SET_POINT),
            (0x61ed48, GRID_CALL),
            (0x61ed4f, BUTTON_LOOP),
            (0x638240, SET_ALL_POINTS),
            (0x6381ed, WIDTH_SETTER),
            (0x6381fc, HEIGHT_SETTER),
            (0x638317, SET_POINT),
            (0x638353, SET_COLOUR),
            (0x638360, SET_BLEND),
            (0x63832a, HEIGHT_SETTER),
            (0x63833d, WIDTH_SETTER),
            (0x638546, GRID_CELL),
            (0x6383aa, COMMAND_BUTTON_CONSTRUCTOR),
            (0x6383c7, WIDTH_SETTER),
            (0x6383da, HEIGHT_SETTER),
            (0x6383fe, SET_POINT),
            (0x63842e, BAR_CONSTRUCTOR),
            (0x63844b, WIDTH_SETTER),
            (0x63845e, HEIGHT_SETTER),
            (0x638487, SET_POINT),
            (0x6384b7, BAR_CONSTRUCTOR),
            (0x6384d4, WIDTH_SETTER),
            (0x6384e7, HEIGHT_SETTER),
            (0x638510, SET_POINT),
        ] {
            if r.call_target(va)? != target {
                return Err(format!("unexpected call target at {va:#x}"));
            }
        }
        // Grid dimensions, and the button loop bounds that must agree with them.
        let dims = r.bytes(0x61ed44, 4, true)?;
        if dims[0] != 0x6a || dims[2] != 0x6a || dims[1] == 0 || dims[3] == 0 {
            return Err("group panel grid dimensions".into());
        }
        let (columns, rows) = (usize::from(dims[1]), usize::from(dims[3]));
        if r.bytes(0x63854c, 3, true)? != [0x83, 0xfb, dims[1]]
            || r.bytes(0x638559, 3, true)? != [0x83, 0xfe, dims[3]]
        {
            return Err("group panel loop bounds differ from the grid".into());
        }
        // Anchor chains. Panel grid: BOTTOM to the panel's BOTTOM. Button content frame and
        // highlight: CENTER to CENTER. Icon: CENTER to the button's TOP. Health bar: TOP to
        // the icon's BOTTOM. Mana bar: TOP to the health bar's BOTTOM.
        let anchors: [(u32, &[u8]); 8] = [
            (0x61ec85, &[0x6a, 1, 0x56]),
            (0x61ed34, &[0x6a, 7, 0x57, 0x6a, 7]),
            (0x638231, &[0x6a, 1, 0x56]),
            (0x63830d, &[0x6a, 4, 0xff, 0xb6, 0x30, 1, 0, 0, 0x6a, 4]),
            (0x6383f9, &[0x6a, 1, 0x56, 0x6a, 4]),
            (0x63847d, &[0x6a, 7, 0xff, 0xb6, 0x38, 1, 0, 0, 0x6a, 1]),
            (0x638506, &[0x6a, 7, 0xff, 0xb6, 0x3c, 1, 0, 0, 0x6a, 1]),
            (0x638427, &[0x6a, HEALTH_BAR_TYPE, 0x6a, 0, 0x56]),
        ];
        for (va, expected) in anchors {
            if r.bytes(va, expected.len(), true)? != expected {
                return Err(format!("unsupported group panel anchor chain at {va:#x}"));
            }
        }
        if r.bytes(0x6384b0, 5, true)? != [0x6a, MANA_BAR_TYPE, 0x6a, 0, 0x56] {
            return Err("group panel mana bar type".into());
        }
        let pair = |r: &mut Reader<_>, x: u32, y: u32, tag: &str| -> Result<[f32; 2], String> {
            let value = [
                r.number(x, &[0xc7, 4, 0x24])?,
                r.number(y, &[0xc7, 4, 0x24])?,
            ];
            if value.iter().any(|v| *v <= 0.) {
                return Err(format!("group panel {tag} size"));
            }
            Ok(value)
        };
        let grid = pair(&mut r, 0x61ecfb, 0x61ed0e, "grid")?;
        let button = pair(&mut r, 0x6381e6, 0x6381f5, "button")?;
        let icon = pair(&mut r, 0x6383c0, 0x6383d3, "icon")?;
        let bar = pair(&mut r, 0x638444, 0x638457, "health bar")?;
        if pair(&mut r, 0x6384cd, 0x6384e0, "mana bar")? != bar {
            return Err("group panel health and mana bar sizes differ".into());
        }
        // The highlight's height is set before its width.
        let highlight = [
            r.number(0x638336, &[0xc7, 4, 0x24])?,
            r.number(0x638323, &[0xc7, 4, 0x24])?,
        ];
        if highlight.iter().any(|v| *v <= 0.) {
            return Err("group panel highlight size".into());
        }
        let offset = |r: &mut Reader<_>, x: u32, y: u32| -> Result<[f32; 2], String> {
            Ok([
                r.number(x, &[0xc7, 4, 0x24])?,
                r.number(y, &[0xc7, 0x44, 0x24, 4])?,
            ])
        };
        let grid_offset = offset(&mut r, 0x61ed2d, 0x61ed25)?;
        if offset(&mut r, 0x638306, 0x6382fe)? != [0., 0.] {
            return Err("group panel highlight is not centred on the button".into());
        }
        let icon_offset = offset(&mut r, 0x6383f2, 0x6383ea)?;
        let health_offset = offset(&mut r, 0x638476, 0x63846e)?;
        let mana_offset = offset(&mut r, 0x6384ff, 0x6384f7)?;
        let highlight_tint = r.word(0x63834c, &[0xc7, 0x45, 0xe4])?;
        let blend = r.bytes(0x63835e, 2, true)?;
        if blend[0] != 0x6a {
            return Err("group panel highlight blend mode".into());
        }
        let highlight_key = r.string(0x6382d0)?;
        let viewport = [
            r.referenced(0x640ea8, &[0xf3, 0x0f, 0x10, 5])?,
            r.referenced(0x640e96, &[0xf3, 0x0f, 0x10, 0x1d])?,
        ];
        if viewport.iter().any(|v| *v <= 0.) || highlight_key.is_empty() {
            return Err("invalid group panel geometry".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            viewport,
            columns,
            rows,
            highlight_key,
            highlight_tint,
            highlight_blend: u32::from(blend[1]),
            grid,
            grid_offset,
            button,
            highlight,
            icon,
            icon_offset,
            bar,
            health_offset,
            mana_offset,
        })
    }

    /// Number of cells: the most units the panel can show.
    pub fn cells(&self) -> usize {
        self.columns * self.rows
    }

    /// Button rectangle of the zero-based cell `index`, filling rows left to right and top to
    /// bottom, inside the unit information frame `info`.
    pub fn button(&self, info: Rect, index: usize) -> Result<Rect, String> {
        if index >= self.cells() {
            return Err("group panel cell exceeds the source grid".into());
        }
        let (row, column) = (index / self.columns, index % self.columns);
        let grid_left = info.x + info.width / 2. + self.grid_offset[0] - self.grid[0] / 2.;
        let grid_top = info.y + self.grid_offset[1] + self.grid[1];
        let top = grid_top - row as f32 * (self.grid[1] / self.rows as f32);
        finite(Rect {
            x: grid_left + column as f32 * (self.grid[0] / self.columns as f32),
            y: top - self.button[1],
            width: self.button[0],
            height: self.button[1],
        })
    }

    /// Subgroup highlight, centred on the button.
    pub fn highlight(&self, info: Rect, index: usize) -> Result<Rect, String> {
        let button = self.button(info, index)?;
        finite(Rect {
            x: button.x + (button.width - self.highlight[0]) / 2.,
            y: button.y + (button.height - self.highlight[1]) / 2.,
            width: self.highlight[0],
            height: self.highlight[1],
        })
    }

    /// Unit icon, centred horizontally and hung from the button's top edge.
    pub fn icon(&self, info: Rect, index: usize) -> Result<Rect, String> {
        let button = self.button(info, index)?;
        let centre_y = button.y + button.height + self.icon_offset[1];
        finite(Rect {
            x: button.x + button.width / 2. + self.icon_offset[0] - self.icon[0] / 2.,
            y: centre_y - self.icon[1] / 2.,
            width: self.icon[0],
            height: self.icon[1],
        })
    }

    pub fn health_bar(&self, info: Rect, index: usize) -> Result<Rect, String> {
        let icon = self.icon(info, index)?;
        self.bar_below(icon, self.health_offset)
    }

    pub fn mana_bar(&self, info: Rect, index: usize) -> Result<Rect, String> {
        let health = self.health_bar(info, index)?;
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

fn finite(rect: Rect) -> Result<Rect, String> {
    if [rect.x, rect.y, rect.width, rect.height]
        .iter()
        .any(|v| !v.is_finite())
    {
        return Err("group panel arithmetic overflow".into());
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

    const INFO: Rect = Rect {
        x: 0.3,
        y: 0.0,
        width: 0.2,
        height: 0.1,
    };

    /// Synthetic client memory with geometry unlike the reviewed image: 3 columns, 2 rows.
    fn fixture() -> BTreeMap<u32, u8> {
        let mut map = BTreeMap::new();
        for (va, target) in [
            (GROUP_CONSTRUCTOR_CALL, GROUP_CONSTRUCTOR),
            (0x61ec97, SET_ALL_POINTS),
            (0x61ece5, GRID_INSET_CONSTRUCTOR),
            (0x61ed02, WIDTH_SETTER),
            (0x61ed15, HEIGHT_SETTER),
            (0x61ed39, SET_POINT),
            (0x61ed48, GRID_CALL),
            (0x61ed4f, BUTTON_LOOP),
            (0x638240, SET_ALL_POINTS),
            (0x6381ed, WIDTH_SETTER),
            (0x6381fc, HEIGHT_SETTER),
            (0x638317, SET_POINT),
            (0x638353, SET_COLOUR),
            (0x638360, SET_BLEND),
            (0x63832a, HEIGHT_SETTER),
            (0x63833d, WIDTH_SETTER),
            (0x638546, GRID_CELL),
            (0x6383aa, COMMAND_BUTTON_CONSTRUCTOR),
            (0x6383c7, WIDTH_SETTER),
            (0x6383da, HEIGHT_SETTER),
            (0x6383fe, SET_POINT),
            (0x63842e, BAR_CONSTRUCTOR),
            (0x63844b, WIDTH_SETTER),
            (0x63845e, HEIGHT_SETTER),
            (0x638487, SET_POINT),
            (0x6384b7, BAR_CONSTRUCTOR),
            (0x6384d4, WIDTH_SETTER),
            (0x6384e7, HEIGHT_SETTER),
            (0x638510, SET_POINT),
        ] {
            call(&mut map, va, target);
        }
        put(&mut map, 0x61ed44, &[0x6a, 3, 0x6a, 2]);
        put(&mut map, 0x63854c, &[0x83, 0xfb, 3]);
        put(&mut map, 0x638559, &[0x83, 0xfe, 2]);
        put(&mut map, 0x61ec85, &[0x6a, 1, 0x56]);
        put(&mut map, 0x61ed34, &[0x6a, 7, 0x57, 0x6a, 7]);
        put(&mut map, 0x638231, &[0x6a, 1, 0x56]);
        put(
            &mut map,
            0x63830d,
            &[0x6a, 4, 0xff, 0xb6, 0x30, 1, 0, 0, 0x6a, 4],
        );
        put(&mut map, 0x6383f9, &[0x6a, 1, 0x56, 0x6a, 4]);
        put(
            &mut map,
            0x63847d,
            &[0x6a, 7, 0xff, 0xb6, 0x38, 1, 0, 0, 0x6a, 1],
        );
        put(
            &mut map,
            0x638506,
            &[0x6a, 7, 0xff, 0xb6, 0x3c, 1, 0, 0, 0x6a, 1],
        );
        put(&mut map, 0x638427, &[0x6a, HEALTH_BAR_TYPE, 0x6a, 0, 0x56]);
        put(&mut map, 0x6384b0, &[0x6a, MANA_BAR_TYPE, 0x6a, 0, 0x56]);
        let prefix = [0xc7, 4, 0x24];
        let low = [0xc7, 0x44, 0x24, 4];
        number(&mut map, 0x61ecfb, &prefix, 0.3);
        number(&mut map, 0x61ed0e, &prefix, 0.1);
        number(&mut map, 0x61ed2d, &prefix, -0.01);
        number(&mut map, 0x61ed25, &low, 0.0);
        number(&mut map, 0x6381e6, &prefix, 0.1);
        number(&mut map, 0x6381f5, &prefix, 0.05);
        number(&mut map, 0x6383c0, &prefix, 0.04);
        number(&mut map, 0x6383d3, &prefix, 0.04);
        number(&mut map, 0x6383f2, &prefix, 0.0);
        number(&mut map, 0x6383ea, &low, -0.02);
        for (width, height, x, y) in [
            (0x638444, 0x638457, 0x638476, 0x63846e),
            (0x6384cd, 0x6384e0, 0x6384ff, 0x6384f7),
        ] {
            number(&mut map, width, &prefix, 0.04);
            number(&mut map, height, &prefix, 0.004);
            number(&mut map, x, &prefix, 0.0);
            number(&mut map, y, &low, -0.003);
        }
        number(&mut map, 0x638323, &prefix, 0.07);
        number(&mut map, 0x638336, &prefix, 0.06);
        number(&mut map, 0x638306, &prefix, 0.0);
        number(&mut map, 0x6382fe, &low, 0.0);
        op(&mut map, 0x63834c, &[0xc7, 0x45, 0xe4], 0xff11_2233);
        put(&mut map, 0x63835e, &[0x6a, 3]);
        op(&mut map, 0x6382d0, &[0x68], 0x4000);
        put(&mut map, 0x4000, b"SyntheticHighlight\0");
        op(&mut map, 0x640ea8, &[0xf3, 0x0f, 0x10, 5], 0x7000);
        op(&mut map, 0x640e96, &[0xf3, 0x0f, 0x10, 0x1d], 0x7004);
        put(&mut map, 0x7000, &0.8f32.to_le_bytes());
        put(&mut map, 0x7004, &0.6f32.to_le_bytes());
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

    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn cells_fill_rows_left_to_right_from_the_bottom_centred_grid() {
        let rules = rules(&fixture()).unwrap();
        assert_eq!((rules.columns, rules.rows, rules.cells()), (3, 2, 6));
        let first = rules.button(INFO, 0).unwrap();
        // Grid is 0.3 wide centred on the frame, shifted by -0.01.
        close(first.x, 0.3 + 0.1 - 0.01 - 0.15);
        close(first.y + first.height, 0.1);
        let second = rules.button(INFO, 1).unwrap();
        close(second.x - first.x, 0.1);
        close(second.y, first.y);
        let below = rules.button(INFO, 3).unwrap();
        close(below.x, first.x);
        close(first.y - below.y, 0.05);
        assert!(rules.button(INFO, 6).is_err());
    }

    #[test]
    fn highlight_icon_and_bars_hang_from_their_anchors() {
        let rules = rules(&fixture()).unwrap();
        let button = rules.button(INFO, 4).unwrap();
        let highlight = rules.highlight(INFO, 4).unwrap();
        close(
            highlight.x + highlight.width / 2.,
            button.x + button.width / 2.,
        );
        close(
            highlight.y + highlight.height / 2.,
            button.y + button.height / 2.,
        );
        assert_eq!((highlight.width, highlight.height), (0.06, 0.07));
        let icon = rules.icon(INFO, 4).unwrap();
        close(icon.x + icon.width / 2., button.x + button.width / 2.);
        close(icon.y + icon.height / 2., button.y + button.height - 0.02);
        let health = rules.health_bar(INFO, 4).unwrap();
        close(health.y + health.height, icon.y - 0.003);
        let mana = rules.mana_bar(INFO, 4).unwrap();
        close(mana.y + mana.height, health.y - 0.003);
        assert_eq!((health.x, health.width), (icon.x, icon.width));
    }

    #[test]
    fn highlight_art_and_tint_come_from_the_client() {
        let rules = rules(&fixture()).unwrap();
        assert_eq!(rules.highlight_key, "SyntheticHighlight");
        assert_eq!(rules.highlight_tint, 0xff11_2233);
        assert_eq!(rules.highlight_blend, 3);
        assert_eq!(rules.viewport, [0.8, 0.6]);
    }

    #[test]
    fn changed_instructions_refuse() {
        for (va, byte) in [
            (0x61ed34_u32, 6u8),
            (0x63830d, 5),
            (0x6383f9, 0),
            (0x63854c, 0x84),
            (0x638427, 9),
        ] {
            let mut map = fixture();
            map.insert(va, byte);
            assert!(rules(&map).is_err(), "{va:#x}");
        }
        // Loop bound disagrees with the grid.
        let mut map = fixture();
        put(&mut map, 0x63854c, &[0x83, 0xfb, 4]);
        assert!(rules(&map).is_err());
        let mut map = fixture();
        call(&mut map, 0x638546, 0x88c971);
        assert!(rules(&map).is_err());
        let mut map = fixture();
        map.remove(&0x4000);
        assert!(rules(&map).is_err());
        // An off-centre highlight is a layout this decoder does not model.
        let mut map = fixture();
        number(&mut map, 0x638306, &[0xc7, 4, 0x24], 0.01);
        assert!(rules(&map).is_err());
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE from the selected local install"]
    fn selected_client_matches_the_decoded_panel_layout() {
        let path =
            std::path::PathBuf::from(std::env::var_os("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE"));
        let image = std::fs::read(&path).expect("selected executable");
        let identity = crate::campaign_inputs::client_identity(&image, &path.display().to_string());
        let info = crate::inventory_layout::Rules::reviewed_legacy(&image, identity.clone())
            .unwrap()
            .info_panel();
        let rules = Rules::reviewed_legacy(&image, identity).unwrap();
        println!("{rules:#?}\ninfo {info:?}");
        for index in 0..rules.cells() {
            println!(
                "cell {index}: {:?} {:?} {:?} {:?}",
                rules.button(info, index).unwrap(),
                rules.icon(info, index).unwrap(),
                rules.health_bar(info, index).unwrap(),
                rules.mana_bar(info, index).unwrap()
            );
        }
        assert_eq!((rules.columns, rules.rows), (6, 2));
        assert_eq!(rules.highlight_key, "SelectedSubgroupHighlight");
        assert_eq!(rules.highlight_tint, 0xffff_e400);
    }
}
