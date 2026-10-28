//! Minimap CPU-image drawing facts from the reviewed client: the map-edge border lines,
//! the unit marker block sizes and the camera outline colour.
//!
//! Sites are in the texture updater (`0x658ac0`), the marker pass (`0x65ba90`), the draw
//! method (`0x63fd30`) and the constructor (`0x620000`). Marker choice, hero/resource/
//! neutral icons, creep camps and pings are not decoded here.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

/// A row or column of the CPU image, relative to the map padding on that axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// `pad + n`
    PadPlus(i32),
    /// `edge - pad - n`
    EdgeMinusPad(i32),
}

impl Edge {
    /// The image index, or `None` outside `0..edge`.
    pub fn index(self, edge: u32, pad: u32) -> Option<u32> {
        let index = match self {
            Self::PadPlus(n) => i64::from(pad) + i64::from(n),
            Self::EdgeMinusPad(n) => i64::from(edge) - i64::from(pad) - i64::from(n),
        };
        u32::try_from(index).ok().filter(|i| *i < edge)
    }
}

/// Lines written over the whole image after the fog alpha, where the map is narrower or
/// shorter than the square image.
#[derive(Clone, Debug)]
pub struct Border {
    /// Profile section and key of the packed ARGB colour.
    pub section: String,
    pub key: String,
    /// The lines are drawn only when the two paddings differ by more than this.
    pub minimum_pad_difference: u32,
    /// Vertical lines, used when the map is taller than wide. Padding is the left padding.
    pub columns: Vec<Edge>,
    /// Horizontal lines, used when the map is wider than tall. Padding is the top padding.
    pub rows: Vec<Edge>,
}

#[derive(Clone, Debug)]
pub struct Draw {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// Edge of the square CPU image the client allocates.
    pub edge: u32,
    pub border: Border,
    /// Side in image pixels of the unit marker block for class 0 and class 1.
    pub marker_sides: [u32; 2],
    /// Camera outline RGB; its alpha is the control's alpha.
    pub outline_rgb: [u8; 3],
}

impl Draw {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: String| format!("minimap draw layout {}: {reason}", origin.source);
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
        let mut remaining = 64usize << 10;
        Self::decode(origin.clone(), REVIEWED_IMAGE, |va, len, code| {
            remaining = remaining.checked_sub(len).ok_or("profile read budget")?;
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
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        type Read<'a> = &'a mut dyn FnMut(u32, usize, bool) -> Result<Vec<u8>, String>;
        fn operand(read: Read, va: u32, prefix: &[u8], len: usize) -> Result<Vec<u8>, String> {
            let bytes = read(va, prefix.len() + len, true)?;
            if bytes.len() != prefix.len() + len || !bytes.starts_with(prefix) {
                return Err(format!("unsupported minimap draw instruction at {va:#x}"));
            }
            Ok(bytes[prefix.len()..].to_vec())
        }
        fn word(bytes: Vec<u8>) -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| "truncated minimap draw word")?,
            ))
        }
        fn imm(read: Read, va: u32, prefix: &[u8]) -> Result<u32, String> {
            word(operand(read, va, prefix, 4)?)
        }
        fn string(read: Read, start: u32) -> Result<String, String> {
            let mut bytes = Vec::new();
            for offset in 0..256 {
                let va = start.checked_add(offset).ok_or("minimap string overflow")?;
                let value = read(va, 1, false)?;
                let [byte] = value.as_slice() else {
                    return Err("truncated minimap string".into());
                };
                if *byte == 0 {
                    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                    return if text.is_empty() || text.chars().any(char::is_control) {
                        Err("invalid minimap string".into())
                    } else {
                        Ok(text)
                    };
                }
                bytes.push(*byte);
            }
            Err("overlong minimap string".into())
        }
        let read: Read = &mut read;

        // The image edge, from the updater's row count and its row stride shift.
        let edge = imm(read, 0x658afd, &[0xbe])?;
        if imm(read, 0x658d67, &[0xbe])? != edge
            || edge == 0
            || edge > 1 << 12
            || 1u32 << operand(read, 0x658dc6, &[0xc1, 0xe7], 1)?[0] != edge * 4
            || 1u32 << operand(read, 0x658ded, &[0xc1, 0xe6], 1)?[0] != edge * 4
        {
            return Err("minimap image edge relation".into());
        }
        let stride = edge * 4;

        // Border colour: the constructor stores one ARGB profile value, the updater loads it.
        operand(read, 0x620480, &[0x8d, 0x45, 0xf0], 0)?;
        operand(read, 0x620488, &[0x50], 0)?;
        let call = 0x620489u32;
        if imm(read, call, &[0xe8])? != 0x430390u32.wrapping_sub(call + 5) {
            return Err("minimap border colour parser".into());
        }
        let key_start = imm(read, 0x620473, &[0x6a, 0, 0x68])?;
        let key = string(read, key_start)?;
        let section_start = imm(read, 0x620483, &[0x68])?;
        let section = string(read, section_start)?;
        let field = imm(read, 0x62049d, &[0x89, 0x87])?;
        if imm(read, 0x658d59, &[0x8b, 0x83])? != field {
            return Err("minimap border colour field relation".into());
        }

        // Axis paddings: both are read from the fields the border lines are placed by.
        let width_fraction = imm(read, 0x658d1d, &[0xf3, 0x0f, 0x10, 0x83])?;
        let height_fraction = imm(read, 0x658d25, &[0xf3, 0x0f, 0x10, 0x8b])?;
        let pad_x = imm(read, 0x658d3a, &[0x8b, 0x83])?;
        let pad_y = imm(read, 0x658d40, &[0x8b, 0xbb])?;
        if width_fraction == height_fraction || pad_x == pad_y {
            return Err("minimap border axis fields".into());
        }
        operand(
            read,
            0x658d2d,
            &[0x0f, 0x2e, 0xc1, 0x9f, 0xf6, 0xc4, 0x44, 0x0f, 0x8b],
            4,
        )?;
        let threshold = operand(
            read,
            0x658d46,
            &[0x2b, 0xc7, 0x99, 0x33, 0xc2, 0x2b, 0xc2, 0x83, 0xf8],
            1,
        )?[0];
        operand(read, 0x658d50, &[0x0f, 0x8e], 4)?;
        // Taller than wide takes the column branch, otherwise the row branch.
        operand(read, 0x658d56, &[0x0f, 0x2f, 0xc8], 0)?;
        imm(read, 0x658d5f, &[0x8b, 0x93])?;
        operand(read, 0x658d65, &[0x76, 0x5f], 0)?;

        // Columns: four single-pixel stores per row, all through the left padding field.
        // The row pointer is advanced by one stride before the stores, which address back.
        if imm(read, 0x658d75, &[0x8d, 0x92])? != stride {
            return Err("minimap border column stride".into());
        }
        let store_base = (stride as i32).wrapping_neg();
        let mut columns = Vec::new();
        for (load, sub, store) in [
            (0x658d70u32, 0x658d7bu32, 0x658d81u32),
            (0x658d88, 0x658d8d, 0x658d93),
        ] {
            let last = i32::try_from(imm(read, load, &[0xb9])?).map_err(|_| "border column")?;
            if imm(read, sub, &[0x2b, 0x8b])? != pad_x {
                return Err("minimap border column padding".into());
            }
            if imm(read, store, &[0x89, 0x84, 0x8a])? as i32 != store_base {
                return Err("minimap border column store".into());
            }
            columns.push(Edge::EdgeMinusPad(edge as i32 - last));
        }
        for (load, store, n) in [(0x658d9au32, 0x658da0u32, 0i32), (0x658da7, 0x658dad, -1)] {
            if imm(read, load, &[0x8b, 0x8b])? != pad_x {
                return Err("minimap border column padding".into());
            }
            let displacement = imm(read, store, &[0x89, 0x84, 0x8a])? as i32;
            if displacement != store_base + 4 * n {
                return Err("minimap border column store".into());
            }
            columns.push(Edge::PadPlus(n));
        }
        operand(read, 0x658db4, &[0x83, 0xee, 0x01, 0x75, 0xb7], 0)?;

        // Rows: two runs of two rows, by the top padding.
        let first = imm(read, 0x658dc9, &[0x81, 0xc7])? as i32;
        let run = imm(read, 0x658dd1, &[0xbf])?;
        if first % stride as i32 != 0 || run != imm(read, 0x658de8, &[0xba])? || run % edge != 0 {
            return Err("minimap border row runs".into());
        }
        if imm(read, 0x658de2, &[0x2b, 0xb3])? != pad_y {
            return Err("minimap border row padding".into());
        }
        operand(read, 0x658dcf, &[0x03, 0xd7], 0)?;
        operand(read, 0x658df0, &[0x03, 0xb3], 4)?;
        let height = run / edge;
        let mut rows = Vec::new();
        for n in 0..height as i32 {
            rows.push(Edge::PadPlus(first / stride as i32 + n));
        }
        for n in 0..height as i32 {
            rows.push(Edge::EdgeMinusPad(-n));
        }

        // Marker block: half = 1 << (class + increments); the block is two halves wide.
        if imm(read, 0x65c0f5, &[0xbb])? != 1 {
            return Err("minimap marker base".into());
        }
        operand(read, 0x65c0fd, &[0x8b, 0x4d, 0xd4, 0x41], 0)?;
        operand(read, 0x65c107, &[0xd3, 0xe3], 0)?;
        if imm(read, 0x65c109, &[0xb9])? != edge + 1 {
            return Err("minimap marker clamp".into());
        }
        operand(read, 0x65c15b, &[0x8d, 0x04, 0x1b, 0x2b, 0xc8], 0)?;
        operand(read, 0x65c1e9, &[0x83, 0x7d, 0xd4, 0x00], 0)?;
        operand(read, 0x65c1fa, &[0x0f, 0x84, 0xf0, 0, 0, 0], 0)?;
        // The class is the structure test's 0 or 1 result.
        if imm(read, 0x696b72, &[0xb8])? != 1 {
            return Err("minimap marker class value".into());
        }
        operand(read, 0x696b7b, &[0x33, 0xc0], 0)?;
        let marker_sides = [0, 1].map(|class: u32| 2 * (1u32 << (class + 1)));

        // Outline colour: the control's alpha shifted up, or'd with a constant RGB.
        operand(
            read,
            0x640676,
            &[0x0f, 0xb6, 0x87, 0xb4, 0, 0, 0, 0xc1, 0xe0, 0x18],
            0,
        )?;
        let rgb = imm(read, 0x640680, &[0x0d])?;
        if rgb >> 24 != 0 {
            return Err("minimap outline alpha operand".into());
        }
        let [_, red, green, blue] = rgb.to_be_bytes();

        Ok(Self {
            origin,
            image_sha256,
            edge,
            border: Border {
                section,
                key,
                minimum_pad_difference: u32::from(threshold),
                columns,
                rows,
            },
            marker_sides,
            outline_rgb: [red, green, blue],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const EDGE: u32 = 64;
    const STRIDE: u32 = EDGE * 4;

    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic minimap draw image".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        }
    }
    fn put(data: &mut BTreeMap<u32, u8>, va: u32, bytes: &[u8]) {
        for (index, byte) in bytes.iter().enumerate() {
            data.insert(va + index as u32, *byte);
        }
    }
    fn op(data: &mut BTreeMap<u32, u8>, va: u32, prefix: &[u8], value: u32) {
        put(data, va, prefix);
        put(data, va + prefix.len() as u32, &value.to_le_bytes());
    }
    /// A client with a 64 pixel image, other field offsets, other keys and other constants.
    fn fixture() -> BTreeMap<u32, u8> {
        let mut d = BTreeMap::new();
        let (pad_x, pad_y) = (0x2f0u32, 0x2f4u32);
        op(&mut d, 0x658afd, &[0xbe], EDGE);
        op(&mut d, 0x658d67, &[0xbe], EDGE);
        put(&mut d, 0x658dc6, &[0xc1, 0xe7, 8]);
        put(&mut d, 0x658ded, &[0xc1, 0xe6, 8]);
        put(&mut d, 0x620480, &[0x8d, 0x45, 0xf0]);
        put(&mut d, 0x620488, &[0x50]);
        op(
            &mut d,
            0x620489,
            &[0xe8],
            0x430390u32.wrapping_sub(0x620489 + 5),
        );
        op(&mut d, 0x620473, &[0x6a, 0, 0x68], 0x1000);
        op(&mut d, 0x620483, &[0x68], 0x1010);
        put(&mut d, 0x1000, b"RimColor\0");
        put(&mut d, 0x1010, b"Frame\0");
        op(&mut d, 0x62049d, &[0x89, 0x87], 0xa44);
        op(&mut d, 0x658d59, &[0x8b, 0x83], 0xa44);
        op(&mut d, 0x658d1d, &[0xf3, 0x0f, 0x10, 0x83], 0x2e8);
        op(&mut d, 0x658d25, &[0xf3, 0x0f, 0x10, 0x8b], 0x2ec);
        op(&mut d, 0x658d3a, &[0x8b, 0x83], pad_x);
        op(&mut d, 0x658d40, &[0x8b, 0xbb], pad_y);
        op(
            &mut d,
            0x658d2d,
            &[0x0f, 0x2e, 0xc1, 0x9f, 0xf6, 0xc4, 0x44, 0x0f, 0x8b],
            0xc8,
        );
        put(
            &mut d,
            0x658d46,
            &[0x2b, 0xc7, 0x99, 0x33, 0xc2, 0x2b, 0xc2, 0x83, 0xf8, 3],
        );
        op(&mut d, 0x658d50, &[0x0f, 0x8e], 0xac);
        put(&mut d, 0x658d56, &[0x0f, 0x2f, 0xc8]);
        op(&mut d, 0x658d5f, &[0x8b, 0x93], 0x1e4);
        put(&mut d, 0x658d65, &[0x76, 0x5f]);
        op(&mut d, 0x658d75, &[0x8d, 0x92], STRIDE);
        let back = STRIDE.wrapping_neg();
        op(&mut d, 0x658d70, &[0xb9], EDGE - 3);
        op(&mut d, 0x658d7b, &[0x2b, 0x8b], pad_x);
        op(&mut d, 0x658d81, &[0x89, 0x84, 0x8a], back);
        op(&mut d, 0x658d88, &[0xb9], EDGE - 5);
        op(&mut d, 0x658d8d, &[0x2b, 0x8b], pad_x);
        op(&mut d, 0x658d93, &[0x89, 0x84, 0x8a], back);
        op(&mut d, 0x658d9a, &[0x8b, 0x8b], pad_x);
        op(&mut d, 0x658da0, &[0x89, 0x84, 0x8a], back);
        op(&mut d, 0x658da7, &[0x8b, 0x8b], pad_x);
        op(&mut d, 0x658dad, &[0x89, 0x84, 0x8a], back.wrapping_sub(4));
        put(&mut d, 0x658db4, &[0x83, 0xee, 0x01, 0x75, 0xb7]);
        op(&mut d, 0x658dc9, &[0x81, 0xc7], back.wrapping_sub(STRIDE));
        op(&mut d, 0x658dd1, &[0xbf], EDGE * 3);
        op(&mut d, 0x658de2, &[0x2b, 0xb3], pad_y);
        op(&mut d, 0x658de8, &[0xba], EDGE * 3);
        put(&mut d, 0x658dcf, &[0x03, 0xd7]);
        op(&mut d, 0x658df0, &[0x03, 0xb3], 0x1e4);
        op(&mut d, 0x65c0f5, &[0xbb], 1);
        put(&mut d, 0x65c0fd, &[0x8b, 0x4d, 0xd4, 0x41]);
        put(&mut d, 0x65c107, &[0xd3, 0xe3]);
        op(&mut d, 0x65c109, &[0xb9], EDGE + 1);
        put(&mut d, 0x65c15b, &[0x8d, 0x04, 0x1b, 0x2b, 0xc8]);
        put(&mut d, 0x65c1e9, &[0x83, 0x7d, 0xd4, 0x00]);
        put(&mut d, 0x65c1fa, &[0x0f, 0x84, 0xf0, 0, 0, 0]);
        op(&mut d, 0x696b72, &[0xb8], 1);
        put(&mut d, 0x696b7b, &[0x33, 0xc0]);
        put(
            &mut d,
            0x640676,
            &[0x0f, 0xb6, 0x87, 0xb4, 0, 0, 0, 0xc1, 0xe0, 0x18],
        );
        op(&mut d, 0x640680, &[0x0d], 0x0012_3456);
        d
    }
    fn decode(d: &BTreeMap<u32, u8>) -> Result<Draw, String> {
        Draw::decode(origin(), sha256(b"fixture"), |va, n, _| {
            (0..n)
                .map(|i| {
                    d.get(&(va + i as u32))
                        .copied()
                        .ok_or_else(|| format!("fixture missing {:#x}", va + i as u32))
                })
                .collect()
        })
    }

    #[test]
    fn border_markers_and_outline_come_from_the_operands() {
        let draw = decode(&fixture()).unwrap();
        assert_eq!(draw.edge, EDGE);
        assert_eq!(draw.border.section, "Frame");
        assert_eq!(draw.border.key, "RimColor");
        assert_eq!(draw.border.minimum_pad_difference, 3);
        assert_eq!(
            draw.border.columns,
            [
                Edge::EdgeMinusPad(3),
                Edge::EdgeMinusPad(5),
                Edge::PadPlus(0),
                Edge::PadPlus(-1)
            ]
        );
        assert_eq!(
            draw.border.rows,
            [
                Edge::PadPlus(-2),
                Edge::PadPlus(-1),
                Edge::PadPlus(0),
                Edge::EdgeMinusPad(0),
                Edge::EdgeMinusPad(-1),
                Edge::EdgeMinusPad(-2)
            ]
        );
        assert_eq!(draw.marker_sides, [4, 8]);
        assert_eq!(draw.outline_rgb, [0x12, 0x34, 0x56]);
    }

    #[test]
    fn edges_index_relative_to_padding_and_drop_outside_the_image() {
        assert_eq!(Edge::PadPlus(-1).index(64, 10), Some(9));
        assert_eq!(Edge::PadPlus(-1).index(64, 0), None);
        assert_eq!(Edge::EdgeMinusPad(1).index(64, 10), Some(53));
        assert_eq!(Edge::EdgeMinusPad(-1).index(64, 0), None);
        assert_eq!(Edge::EdgeMinusPad(0).index(64, 64), Some(0));
    }

    #[test]
    fn changed_instructions_and_truncations_refuse() {
        let base = fixture();
        for (va, bytes) in [
            (0x658d67, vec![0xbd]),
            (0x658dc8, vec![9]),
            (0x620489, vec![0xe9]),
            (0x62049f, vec![0x01]),
            (0x658d5b, vec![0x11]),
            (0x658d4f, vec![0x83, 0xf9]),
            (0x658d81, vec![0x88]),
            (0x658d7d, vec![0x12]),
            (0x658dad, vec![0x00, 0xfd]),
            (0x658dd2, vec![0xff]),
            (0x65c100, vec![0x40]),
            (0x65c0f6, vec![2]),
            (0x65c109, vec![0xba]),
            (0x696b73, vec![2]),
            (0x640684, vec![1]),
        ] {
            let mut d = base.clone();
            put(&mut d, va, &bytes);
            assert!(decode(&d).is_err(), "mutant {va:#x}");
        }
        for va in base.keys() {
            let mut d = base.clone();
            d.remove(va);
            assert!(decode(&d).is_err(), "truncation {va:#x}");
        }
        assert!(Draw::reviewed_legacy(b"not a client", origin()).is_err());
    }

    #[test]
    #[ignore = "requires PCX_PLAYER_RESOURCE_IMAGE selecting the reviewed local client"]
    fn supplied_client_draw_operands_are_admitted() {
        let path = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let draw = Draw::reviewed_legacy(
            &bytes,
            AssetIdentity {
                source: path,
                edition: "explicit selected local client".into(),
                version: "reviewed client".into(),
            },
        )
        .unwrap();
        assert_eq!(draw.image_sha256, REVIEWED_IMAGE);
        assert_eq!(draw.border.key, "MinimapColorBorder");
        assert_eq!(draw.border.section, "Minimap");
        assert_eq!(draw.marker_sides, [4, 8]);
        assert_eq!(draw.edge, 256);
    }
}
