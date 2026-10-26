//! Leaderboard geometry read from the reviewed client, alongside runtime FDF data.
use crate::{
    inventory_layout::Rect,
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub world_bottom_left: [f32; 2],
    pub world_top_right: [f32; 2],
    pub root_offset: [f32; 2],
    /// Top, horizontal, bottom, scrollbar list insets, in source logical UI units.
    pub list_insets: [f32; 4],
    pub row_height: f32,
    pub row_gap: f32,
    pub max_rows: u32,
    pub title_extra: [f32; 2],
    pub container_title_offset: [f32; 2],
    pub container_hidden_offset: [f32; 2],
    pub container_bottom_offset: [f32; 2],
    pub name_left: f32,
    pub name_after_column: f32,
    pub name_right: f32,
    pub value_right: f32,
    pub creation_icons: bool,
    pub backdrop_background_default: f32,
    pub row_shadow: [u8; 4],
    pub row_shadow_offset: [f32; 2],
    pub font_section: String,
    pub font_height_key: String,
    pub font_key: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub root: Rect,
    pub list: Rect,
}

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: String| format!("leaderboard layout {}: {reason}", origin.source);
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
        Self::decode(origin.clone(), sha256(image), |va, len, code| {
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
        fn word(bytes: &[u8]) -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                bytes.try_into().map_err(|_| "truncated word")?,
            ))
        }
        fn operand(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
        ) -> Result<u32, String> {
            let bytes = read(va, prefix.len() + 4, true)?;
            if !bytes.starts_with(prefix) {
                return Err(format!("unsupported instruction at {va:#x}"));
            }
            word(bytes.get(prefix.len()..).ok_or("truncated instruction")?)
        }
        fn scalar(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
        ) -> Result<f32, String> {
            let v = f32::from_bits(operand(read, va, prefix)?);
            if !v.is_finite() {
                return Err(format!("nonfinite geometry at {va:#x}"));
            }
            Ok(v)
        }
        fn string(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            pointer: u32,
        ) -> Result<String, String> {
            let mut bytes = Vec::new();
            for offset in 0..4096 {
                let address = pointer
                    .checked_add(offset)
                    .ok_or("string pointer overflow")?;
                let raw = read(address, 1, false)?;
                let [byte] = raw.as_slice() else {
                    return Err("truncated string".into());
                };
                if *byte == 0 {
                    let value = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                    if value.is_empty() || value.chars().any(char::is_control) {
                        return Err("invalid layout key".into());
                    }
                    return Ok(value);
                }
                bytes.push(*byte);
            }
            Err("overlong layout key".into())
        }
        let stack = &[0xc7, 0x04, 0x24][..];
        let stack_y = &[0xc7, 0x44, 0x24, 0x04][..];
        // These call/anchor identities qualify the normal WorldFrame branch only.
        for (va, target) in [
            (0x5e7b68_u32, 0x873a10_u32),
            (0x5e7ba0, 0x873a10),
            (0x5f3ddb, 0x873a10),
            (0x652c8e, 0x62aa80),
        ] {
            if operand(&mut read, va, &[0xe8])? != target.wrapping_sub(va + 5) {
                return Err(format!("unsupported layout call at {va:#x}"));
            }
        }
        let world_bottom_left = [
            scalar(&mut read, 0x5e7b50, stack)?,
            scalar(&mut read, 0x5e7b48, stack_y)?,
        ];
        let world_top_right = [
            scalar(&mut read, 0x5e7b86, stack)?,
            scalar(&mut read, 0x5e7b7c, stack_y)?,
        ];
        let root_offset = [
            scalar(&mut read, 0x5f3d8b, stack)?,
            scalar(&mut read, 0x5f3d81, stack_y)?,
        ];
        let list_insets = [
            scalar(&mut read, 0x6332ab, stack)?,
            scalar(&mut read, 0x6332a3, stack_y)?,
            scalar(&mut read, 0x63329b, &[0xc7, 0x44, 0x24, 0x08])?,
            scalar(&mut read, 0x633293, &[0xc7, 0x44, 0x24, 0x0c])?,
        ];
        let row_height = scalar(&mut read, 0x6332bf, stack)?;
        let row_gap = scalar(&mut read, 0x6332d5, stack)?;
        let max_rows = operand(&mut read, 0x62aa88, &[0xbf])?;
        let mut title_extra = [0.; 2];
        for (index, va) in [0x62aaad, 0x62aaa3].into_iter().enumerate() {
            let pointer = operand(&mut read, va, &[0xf3, 0x0f, 0x10, 0x15])?;
            title_extra[index] = f32::from_bits(word(&read(pointer, 4, false)?)?);
        }
        let container_title_offset = [
            scalar(&mut read, 0x652bf0, stack)?,
            scalar(&mut read, 0x652be8, stack_y)?,
        ];
        let container_hidden_offset = [
            scalar(&mut read, 0x652c27, stack)?,
            scalar(&mut read, 0x652c1f, stack_y)?,
        ];
        let container_bottom_offset = [
            scalar(&mut read, 0x652c6d, stack)?,
            scalar(&mut read, 0x652c63, stack_y)?,
        ];
        let name_left = scalar(&mut read, 0x6579a8, stack)?;
        let name_after_column = scalar(&mut read, 0x6579d8, stack)?;
        let name_right = scalar(&mut read, 0x6579f8, stack)?;
        let value_right = scalar(&mut read, 0x657907, stack)?;
        let creation_icons = match operand(&mut read, 0x629671, &[0xc7, 0x46, 0x60])? {
            0 => false,
            1 => true,
            _ => return Err("unsupported row creation icon flag".into()),
        };
        let backdrop_background_default =
            scalar(&mut read, 0x87a764, &[0xc7, 0x86, 0x4c, 0x02, 0x00, 0x00])?;
        let packed_shadow = operand(&mut read, 0x639794, &[0xc7, 0x00])?;
        let [b, g, r, a] = packed_shadow.to_le_bytes();
        let row_shadow = [r, g, b, a];
        let row_shadow_offset = [
            scalar(&mut read, 0x639776, &[0xc7, 0x45, 0xe0])?,
            scalar(&mut read, 0x63977d, &[0xc7, 0x45, 0xe4])?,
        ];
        // Profile fields 120/124 become frame corner/background size. A zero
        // background divisor selects the corner-size field in the draw path.
        for (va, bytes) in [
            (0x897a82, &[0x8b, 0x86, 0x20, 0x01, 0x00, 0x00][..]),
            (0x897a88, &[0x89, 0x83, 0x48, 0x02, 0x00, 0x00]),
            (0x897aa1, &[0x8b, 0x86, 0x24, 0x01, 0x00, 0x00]),
            (0x897aa7, &[0x89, 0x83, 0x4c, 0x02, 0x00, 0x00]),
            (0x87ae25, &[0xf3, 0x0f, 0x10, 0x87, 0x4c, 0x02, 0x00, 0x00]),
            (0x87ae3b, &[0xf3, 0x0f, 0x10, 0x97, 0x48, 0x02, 0x00, 0x00]),
        ] {
            if read(va, bytes.len(), true)? != bytes {
                return Err(format!("unsupported backdrop binding at {va:#x}"));
            }
        }
        let font_height_pointer = operand(&mut read, 0x6395d1, &[0x68])?;
        let font_pointer = operand(&mut read, 0x6395fc, &[0x68])?;
        let font_section = string(&mut read, 0xe841a0)?;
        let font_height_key = string(&mut read, font_height_pointer)?;
        let font_key = string(&mut read, font_pointer)?;
        if row_height <= 0.
            || row_gap < 0.
            || max_rows == 0
            || list_insets.iter().any(|v| *v < 0.)
            || title_extra.iter().any(|v| !v.is_finite() || *v < 0.)
        {
            return Err("invalid row geometry".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            world_bottom_left,
            world_top_right,
            root_offset,
            list_insets,
            row_height,
            row_gap,
            max_rows,
            title_extra,
            container_title_offset,
            container_hidden_offset,
            container_bottom_offset,
            name_left,
            name_after_column,
            name_right,
            value_right,
            creation_icons,
            backdrop_background_default,
            row_shadow,
            row_shadow_offset,
            font_section,
            font_height_key,
            font_key,
        })
    }

    pub fn backdrop_tile_size(&self, authored: Option<f32>, corner: f32) -> Result<f32, String> {
        let value = authored.unwrap_or(self.backdrop_background_default);
        let tile = if value == 0. { corner } else { value };
        if !tile.is_finite() || tile <= 0. {
            return Err("invalid leaderboard backdrop divisor".into());
        }
        Ok(tile)
    }

    /// `title` is the FDF anchor result after measuring its source font.
    /// Alternate timer/multiboard parents must be refused by the caller.
    pub fn placement(
        &self,
        viewport: [f32; 2],
        width: f32,
        requested: i32,
        title: Option<Rect>,
    ) -> Result<Placement, String> {
        if viewport.iter().any(|v| !v.is_finite() || *v <= 0.)
            || !width.is_finite()
            || width <= 0.
            || requested < 0
        {
            return Err("invalid leaderboard placement input".into());
        }
        let count = (requested as u32).min(self.max_rows);
        let height = count as f32 * (self.row_height + self.row_gap)
            + self.list_insets[0]
            + self.list_insets[2]
            + self.title_extra[usize::from(title.is_some())];
        let root = Rect {
            x: viewport[0] + self.world_top_right[0] + self.root_offset[0] - width,
            y: viewport[1] + self.world_top_right[1] + self.root_offset[1] - height,
            width,
            height,
        };
        let top_left = match title {
            Some(t) => [
                t.x + self.container_title_offset[0],
                t.y + self.container_title_offset[1],
            ],
            None => [
                root.x + self.container_hidden_offset[0],
                root.y + root.height + self.container_hidden_offset[1],
            ],
        };
        let right = root.x + root.width + self.container_bottom_offset[0];
        let bottom = root.y + self.container_bottom_offset[1];
        let list = Rect {
            x: top_left[0],
            y: bottom,
            width: right - top_left[0],
            height: top_left[1] - bottom,
        };
        if [root.x, root.y, root.height, list.width, list.height]
            .iter()
            .any(|v| !v.is_finite())
            || list.width <= 0.
            || list.height < 0.
        {
            return Err("invalid derived leaderboard rectangle".into());
        }
        Ok(Placement { root, list })
    }

    pub fn row(&self, list: Rect, index: u32) -> Result<Rect, String> {
        if index >= self.max_rows {
            return Err("leaderboard row exceeds admitted cap".into());
        }
        // The current renderer admits no scrolling or alternate panel branch.
        let row = Rect {
            x: list.x + self.list_insets[1],
            y: list.y + list.height
                - self.list_insets[0]
                - index as f32 * (self.row_height + self.row_gap)
                - self.row_height,
            width: list.width - self.list_insets[1] * 2.,
            height: self.row_height,
        };
        if !row.width.is_finite() || row.width <= 0. || row.y < list.y {
            return Err("leaderboard row requires unsupported scrolling".into());
        }
        Ok(row)
    }

    /// Intrinsic value width comes from the shaped source-font text, never a fixed column.
    pub fn name_span(&self, row_width: f32, value_width: Option<f32>) -> Result<[f32; 2], String> {
        let right = match value_width {
            Some(width) if width.is_finite() && width >= 0. => {
                row_width + self.value_right - width + self.name_after_column
            }
            Some(_) => return Err("invalid intrinsic value width".into()),
            None => row_width + self.name_right,
        };
        if !row_width.is_finite() || right < self.name_left {
            return Err("leaderboard name column has no span".into());
        }
        Ok([self.name_left, right])
    }
}

#[cfg(test)]
#[path = "leaderboard_layout_tests.rs"]
mod tests;
