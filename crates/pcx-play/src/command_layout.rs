//! Command-bar geometry from the explicitly reviewed client image.
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
    /// Skin key naming the console parent used by the command-bar constructor.
    pub console_key: String,
    pub active_key: String,
    pub badge: RankBadge,
    pub disabled_key: String,
    disabled_format: String,
    pub columns: usize,
    pub rows: usize,
    /// Logical UI coordinates, with positive Y upwards.
    pub viewport: [f32; 2],
    /// Addressing selected by the command-button texture setter's source flags.
    pub icon_repeat: bool,
    /// Normal gameplay world frame, relative to the logical UI viewport.
    pub world_frame: Rect,
    frame_size: [f32; 2],
    insets: [f32; 4],
    offset: [f32; 2],
    button_size: [f32; 2],
}

/// Number overlay attached to a command button's bottom-right corner.
#[derive(Clone, Debug)]
pub struct RankBadge {
    pub art_key: String,
    pub font_key: String,
    pub section: String,
    pub height_key: String,
    pub size: [f32; 2],
    pub offset: [f32; 2],
    pub text_offset: [f32; 2],
    pub colour: u32,
}

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: String| format!("command layout {}: {reason}", origin.source);
        if image.len() > 32 << 20
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail("unsupported client identity".into()));
        }
        let digest = sha256(image);
        if digest != REVIEWED_IMAGE {
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
        Self::decode(origin.clone(), digest, |va, len, code| {
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
        fn operand(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
            len: usize,
        ) -> Result<Vec<u8>, String> {
            let bytes = read(va, prefix.len() + len, true)?;
            if bytes.len() != prefix.len() + len || !bytes.starts_with(prefix) {
                return Err(format!("unsupported command instruction at {va:#x}"));
            }
            Ok(bytes[prefix.len()..].to_vec())
        }
        fn word(bytes: Vec<u8>) -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                bytes.try_into().map_err(|_| "truncated command word")?,
            ))
        }
        fn number(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
        ) -> Result<f32, String> {
            let value = f32::from_bits(word(operand(read, va, prefix, 4)?)?);
            if !value.is_finite() {
                return Err(format!("nonfinite command geometry at {va:#x}"));
            }
            Ok(value)
        }
        fn referenced(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
        ) -> Result<f32, String> {
            let pointer = word(operand(read, va, prefix, 4)?)?;
            let value = f32::from_bits(word(read(pointer, 4, false)?)?);
            if !value.is_finite() {
                return Err(format!("nonfinite command viewport at {va:#x}"));
            }
            Ok(value)
        }
        for (va, target) in [
            (0x5e805d_u32, 0x61d130_u32),
            (0x61d24d, 0x874900),
            (0x5eafb6, 0x635390),
            (0x5eafd5, 0x5d3c80),
            (0x5d3caf, 0x5d3e30),
            (0x5d3e60, 0x8750b0),
        ] {
            let displacement = target.wrapping_sub(va + 5);
            if word(operand(&mut read, va, &[0xe8], 4)?)? != displacement {
                return Err(format!("unsupported command texture setter at {va:#x}"));
            }
        }
        if word(operand(&mut read, 0x5e8074, &[0x89, 0x87], 4)?)?
            != word(operand(&mut read, 0x5eafab, &[0x8b, 0x8b], 4)?)?
        {
            return Err("command texture setter uses another UI field".into());
        }
        let mask = operand(&mut read, 0x887ce5, &[0x83, 0xe0], 1)?[0];
        let icon_repeat = operand(&mut read, 0x8750db, &[0x6a], 1)?[0] & mask != 0;
        if (operand(&mut read, 0x875141, &[0x6a], 1)?[0] & mask != 0) != icon_repeat {
            return Err("command image sampler relation".into());
        }
        let console_pointer = word(operand(&mut read, 0x5e7ca5, &[0x68], 4)?)?;
        let mut console = Vec::new();
        for offset in 0..4096 {
            let address = console_pointer
                .checked_add(offset)
                .ok_or("command parent string overflow")?;
            let byte = read(address, 1, false)?;
            let [byte] = byte.as_slice() else {
                return Err("truncated command parent string".into());
            };
            if *byte == 0 {
                break;
            }
            console.push(*byte);
        }
        if console.is_empty() || console.len() == 4096 {
            return Err("missing or overlong command parent skin key".into());
        }
        let console_key = String::from_utf8(console).map_err(|e| e.to_string())?;
        if console_key.chars().any(char::is_control) {
            return Err("invalid command parent skin key".into());
        }
        fn source_text(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            pointer: u32,
        ) -> Result<String, String> {
            let mut bytes = Vec::new();
            for offset in 0..4096 {
                let address = pointer
                    .checked_add(offset)
                    .ok_or("command string overflow")?;
                let value = read(address, 1, false)?;
                let [byte] = value.as_slice() else {
                    return Err("truncated command string".into());
                };
                if *byte == 0 {
                    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                    if text.is_empty() || text.chars().any(char::is_control) {
                        return Err("invalid command string".into());
                    }
                    return Ok(text);
                }
                bytes.push(*byte);
            }
            Err("overlong command string".into())
        }
        let mut source_string = |va| {
            let pointer = word(operand(&mut read, va, &[0x68], 4)?)?;
            source_text(&mut read, pointer)
        };
        let disabled_key = source_string(0x5d3eaa)?;
        let disabled_format = source_string(0x5d3eb8)?;
        let active_key = source_string(0x62c08c)?;
        if source_string(0x85b6b4)? != "ADD" {
            return Err("unsupported active command blend name".into());
        }
        let art_key = source_string(0x62c561)?;
        let height_key = source_string(0x62c5cc)?;
        let font_key = source_string(0x62c5f8)?;
        let section_global = word(operand(&mut read, 0x62c5d1, &[0xff, 0x35], 4)?)?;
        let section_pointer = word(read(section_global, 4, false)?)?;
        let section = source_text(&mut read, section_pointer)?;
        for (va, target) in [
            (0x62c4c3_u32, 0x888120_u32),
            (0x62c4f4, 0x873a10),
            (0x62c507, 0x873830),
            (0x62c51a, 0x873b40),
            (0x62c552, 0x8864c0),
            (0x62c578, 0x887c20),
            (0x62c587, 0x873750),
            (0x62c5bf, 0x886400),
            (0x62c608, 0x887470),
            (0x62c62d, 0x873a10),
        ] {
            if word(operand(&mut read, va, &[0xe8], 4)?)? != target.wrapping_sub(va + 5) {
                return Err("command number overlay call relation".into());
            }
        }
        for (va, anchor) in [(0x62c4ef, 8), (0x62c4f2, 8), (0x62c623, 4), (0x62c62b, 4)] {
            if operand(&mut read, va, &[0x6a], 1)? != [anchor] {
                return Err("command number overlay anchor pair".into());
            }
        }
        if u32::from(operand(&mut read, 0x62c0b8, &[0x6a], 1)?[0])
            != word(operand(&mut read, 0x85b6c6, &[0xc7, 0x43, 0x38], 4)?)?
        {
            return Err("active command blend differs from source ADD mapping".into());
        }
        for (va, target) in [
            (0x62c07f_u32, 0x8864c0_u32),
            (0x62c0ad, 0x887c20),
            (0x62c0ba, 0x887410),
            (0x62c0c8, 0x873750),
        ] {
            if word(operand(&mut read, va, &[0xe8], 4)?)? != target.wrapping_sub(va + 5) {
                return Err("active command texture call relation".into());
            }
        }
        let badge = RankBadge {
            art_key,
            font_key,
            section,
            height_key,
            size: [
                number(&mut read, 0x62c500, &[0xc7, 0x04, 0x24])?,
                number(&mut read, 0x62c513, &[0xc7, 0x04, 0x24])?,
            ],
            offset: [
                number(&mut read, 0x62c4e8, &[0xc7, 0x04, 0x24])?,
                number(&mut read, 0x62c4e0, &[0xc7, 0x44, 0x24, 0x04])?,
            ],
            text_offset: [
                number(&mut read, 0x62c61c, &[0xc7, 0x04, 0x24])?,
                number(&mut read, 0x62c614, &[0xc7, 0x44, 0x24, 0x04])?,
            ],
            colour: word(operand(&mut read, 0x885fcb, &[0xc7, 0x46, 0x6d], 4)?)?,
        };
        if badge.size.iter().any(|v| *v <= 0.) {
            return Err("command number overlay dimensions".into());
        }
        let parts = disabled_format.split("%s").collect::<Vec<_>>();
        if parts.len() != 3 || parts.iter().any(|part| part.contains('%')) {
            return Err("unsupported disabled command art format".into());
        }
        let columns = usize::from(operand(&mut read, 0x61d1c4, &[0x6a], 1)?[0]);
        let rows = usize::from(operand(&mut read, 0x61d1c6, &[0x6a], 1)?[0]);
        if columns == 0
            || rows == 0
            || operand(&mut read, 0x61d357, &[0x83, 0xff], 1)?[0] as usize != rows
            || operand(&mut read, 0x61d364, &[0x83, 0xfe], 1)?[0] as usize != columns
        {
            return Err("command grid allocation/loop relation".into());
        }
        let frame_size = [
            number(&mut read, 0x61d1d6, &[0xc7, 0x04, 0x24])?,
            number(&mut read, 0x61d1e5, &[0xc7, 0x04, 0x24])?,
        ];
        let offset = [
            number(&mut read, 0x61d200, &[0xc7, 0x04, 0x24])?,
            number(&mut read, 0x61d1f8, &[0xc7, 0x44, 0x24, 0x04])?,
        ];
        // The geometry translation below supports the decoded bottom-right pair only.
        if operand(&mut read, 0x61d207, &[0x6a], 1)? != [8]
            || operand(&mut read, 0x61d20a, &[0x6a], 1)? != [8]
        {
            return Err("unsupported command bar anchor pair".into());
        }
        let button_size = [
            number(&mut read, 0x61d2e1, &[0xc7, 0x04, 0x24])?,
            number(&mut read, 0x61d2f0, &[0xc7, 0x04, 0x24])?,
        ];
        let insets = [
            number(&mut read, 0x88c7e2, &[0xc7, 0x86, 0x40, 1, 0, 0])?,
            number(&mut read, 0x88c7ec, &[0xc7, 0x86, 0x44, 1, 0, 0])?,
            number(&mut read, 0x88c7f6, &[0xc7, 0x86, 0x48, 1, 0, 0])?,
            number(&mut read, 0x88c800, &[0xc7, 0x86, 0x4c, 1, 0, 0])?,
        ];
        let viewport = [
            referenced(&mut read, 0x640ea8, &[0xf3, 0x0f, 0x10, 5])?,
            referenced(&mut read, 0x640e96, &[0xf3, 0x0f, 0x10, 0x1d])?,
        ];
        // Both world-frame anchors must address the same constructed child.
        for (va, prefix) in [(0x5e7b5a, &[0x8b, 0x8f][..]), (0x5e7b92, &[0x8b, 0x8f][..])] {
            if word(operand(&mut read, va, prefix, 4)?)?
                != word(operand(&mut read, 0x5e77e7, &[0x89, 0x87], 4)?)?
            {
                return Err("world frame constructor/anchor relation".into());
            }
        }
        for (va, target) in [
            (0x5e77d2_u32, 0x608c10_u32),
            (0x5e7b68, 0x873a10),
            (0x5e7ba0, 0x873a10),
        ] {
            if word(operand(&mut read, va, &[0xe8], 4)?)? != target.wrapping_sub(va + 5) {
                return Err("world frame anchor call relation".into());
            }
        }
        for (va, anchor) in [(0x5e7b57, 6), (0x5e7b60, 6), (0x5e7b8f, 2), (0x5e7b98, 2)] {
            if operand(&mut read, va, &[0x6a], 1)? != [anchor] {
                return Err("world frame anchor pair".into());
            }
        }
        let x = number(&mut read, 0x5e7b50, &[0xc7, 0x04, 0x24])?;
        let bottom = number(&mut read, 0x5e7b48, &[0xc7, 0x44, 0x24, 0x04])?;
        let right = viewport[0] + number(&mut read, 0x5e7b86, &[0xc7, 0x04, 0x24])?;
        let top = viewport[1] + number(&mut read, 0x5e7b7c, &[0xc7, 0x44, 0x24, 0x04])?;
        let world_frame = Rect {
            x,
            y: bottom,
            width: right - x,
            height: top - bottom,
        };
        if x < 0.
            || bottom < 0.
            || right > viewport[0]
            || top > viewport[1]
            || world_frame.width <= 0.
            || world_frame.height <= 0.
        {
            return Err("world frame exceeds source viewport".into());
        }
        if frame_size
            .iter()
            .chain(&button_size)
            .chain(&viewport)
            .any(|v| *v <= 0.)
            || insets.iter().any(|v| *v < 0.)
            || button_size[0] > (frame_size[0] - insets[0] - insets[2]) / columns as f32
            || button_size[1] > (frame_size[1] - insets[1] - insets[3]) / rows as f32
        {
            return Err("invalid command geometry dimensions".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            console_key,
            active_key,
            badge,
            disabled_key,
            disabled_format,
            columns,
            rows,
            viewport,
            icon_repeat,
            world_frame,
            frame_size,
            insets,
            offset,
            button_size,
        })
    }

    pub fn disabled_art(&self, folder: &str, art: &str) -> Result<String, String> {
        let basename = art
            .rsplit_once('\\')
            .or_else(|| art.rsplit_once('/'))
            .map_or(art, |(_, name)| name);
        if folder.is_empty() || basename.is_empty() {
            return Err("missing disabled command art input".into());
        }
        let parts = self.disabled_format.split("%s").collect::<Vec<_>>();
        let [head, middle, tail] = parts.as_slice() else {
            return Err("disabled command art format".into());
        };
        Ok(format!("{head}{folder}{middle}{basename}{tail}"))
    }

    /// `Buttonpos` column and row; row zero is the top row in the native grid.
    pub fn slot(&self, column: usize, row: usize) -> Result<Rect, String> {
        if column >= self.columns || row >= self.rows {
            return Err("command Buttonpos exceeds the source grid".into());
        }
        let slot = Rect {
            x: self.viewport[0] + self.offset[0] - self.frame_size[0]
                + self.insets[0]
                + column as f32
                    * ((self.frame_size[0] - self.insets[0] - self.insets[2])
                        / self.columns as f32),
            y: self.offset[1] + self.frame_size[1]
                - self.insets[1]
                - row as f32
                    * ((self.frame_size[1] - self.insets[1] - self.insets[3]) / self.rows as f32)
                - self.button_size[1],
            width: self.button_size[0],
            height: self.button_size[1],
        };
        if [slot.x, slot.y, slot.width, slot.height]
            .iter()
            .any(|v| !v.is_finite())
        {
            return Err("command slot arithmetic overflow".into());
        }
        Ok(slot)
    }
}

#[cfg(test)]
#[path = "command_layout_tests.rs"]
mod tests;
