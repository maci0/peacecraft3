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
    pub columns: usize,
    pub rows: usize,
    /// Logical UI coordinates, with positive Y upwards.
    pub viewport: [f32; 2],
    /// Addressing selected by the command-button texture setter's source flags.
    pub icon_repeat: bool,
    frame_size: [f32; 2],
    insets: [f32; 4],
    offset: [f32; 2],
    button_size: [f32; 2],
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
            columns,
            rows,
            viewport,
            icon_repeat,
            frame_size,
            insets,
            offset,
            button_size,
        })
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
