//! Timed-text frame geometry from the explicitly reviewed client image.
use crate::{
    inventory_layout::Rect,
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

/// A message rectangle in logical UI coordinates, with positive Y upwards.
/// `viewport` is the full UI extent that the rectangle is relative to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub left: f32,
    pub bottom: f32,
    pub width: f32,
    pub height: f32,
    pub viewport: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// Skin key naming the frame that spans the full UI.
    pub console_key: String,
    /// Skin key naming the frame that receives placed script text.
    pub message_key: String,
    pub viewport: [f32; 2],
    message: Rect,
    unit_size: [f32; 2],
    inset: f32,
    top_margin: f32,
    span: f32,
}

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: String| format!("message layout {}: {reason}", origin.source);
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
                return Err(format!("unsupported message instruction at {va:#x}"));
            }
            Ok(bytes[prefix.len()..].to_vec())
        }
        fn word(bytes: Vec<u8>) -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                bytes.try_into().map_err(|_| "truncated message word")?,
            ))
        }
        fn number(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
        ) -> Result<f32, String> {
            let value = f32::from_bits(word(operand(read, va, prefix, 4)?)?);
            if !value.is_finite() {
                return Err(format!("nonfinite message geometry at {va:#x}"));
            }
            Ok(value)
        }
        fn referenced(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
        ) -> Result<(u32, f32), String> {
            let pointer = word(operand(read, va, prefix, 4)?)?;
            let value = f32::from_bits(word(read(pointer, 4, false)?)?);
            if !value.is_finite() {
                return Err(format!("nonfinite message constant at {va:#x}"));
            }
            Ok((pointer, value))
        }
        fn call(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            target: u32,
        ) -> Result<(), String> {
            let displacement = target.wrapping_sub(va + 5);
            if word(operand(read, va, &[0xe8], 4)?)? != displacement {
                return Err(format!("unsupported message call at {va:#x}"));
            }
            Ok(())
        }
        fn skin(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            pointer: u32,
            what: &str,
        ) -> Result<String, String> {
            let mut bytes = Vec::new();
            for offset in 0..4096 {
                let address = pointer
                    .checked_add(offset)
                    .ok_or_else(|| format!("{what} overflow"))?;
                let raw = read(address, 1, false)?;
                let [byte] = raw.as_slice() else {
                    return Err(format!("truncated {what}"));
                };
                if *byte == 0 {
                    break;
                }
                bytes.push(*byte);
            }
            if bytes.is_empty() || bytes.len() == 4096 {
                return Err(format!("missing or overlong {what}"));
            }
            let key = String::from_utf8(bytes).map_err(|e| e.to_string())?;
            if key.chars().any(char::is_control) {
                return Err(format!("invalid {what}"));
            }
            Ok(key)
        }
        for (va, target) in [(0x5e7d2e_u32, 0x873750_u32), (0x5f4900, 0x873a10)] {
            call(&mut read, va, target)?;
        }
        let console_pointer = word(operand(&mut read, 0x5e7ca5, &[0x68], 4)?)?;
        let message_pointer = word(operand(&mut read, 0x5e8395, &[0x68], 4)?)?;
        let unit_pointer = word(operand(&mut read, 0x5e85ac, &[0x68], 4)?)?;
        let console_key = skin(&mut read, console_pointer, "message console key")?;
        let message_key = skin(&mut read, message_pointer, "message frame key")?;
        let unit_key = skin(&mut read, unit_pointer, "message unit frame key")?;
        if console_key != "SimpleConsole"
            || message_key != "WorldFrameMessage"
            || unit_key != "WorldFrameUnitMessage"
        {
            return Err("unexpected message frame keys".into());
        }
        let console_field = word(operand(&mut read, 0x5e7cbd, &[0x89, 0xb7], 4)?)?;
        if word(operand(&mut read, 0x5e7d28, &[0x8b, 0x8f], 4)?)? != console_field
            || word(operand(&mut read, 0x5e841f, &[0xff, 0xb7], 4)?)? != console_field
        {
            return Err("message console parent relation".into());
        }
        if word(operand(&mut read, 0x5e838f, &[0x89, 0x87], 4)?)? != 0x424
            || word(operand(&mut read, 0x5e85a6, &[0x89, 0x87], 4)?)? != 0x428
            || word(operand(&mut read, 0x5e8636, &[0xff, 0xb7], 4)?)? != 0x424
            || word(operand(&mut read, 0x5f4874, &[0x8b, 0x8e], 4)?)? != 0x424
            || word(operand(&mut read, 0x5f48f8, &[0xff, 0xb6], 4)?)? != 0x424
        {
            return Err("message frame field relation".into());
        }
        let message_width = number(&mut read, 0x5e83e4, &[0xc7, 0x04, 0x24])?;
        let message_height = number(&mut read, 0x5e83f7, &[0xc7, 0x04, 0x24])?;
        let message_anchor = [
            number(&mut read, 0x5e8416, &[0xc7, 0x04, 0x24])?,
            number(&mut read, 0x5e840e, &[0xc7, 0x44, 0x24, 0x04])?,
        ];
        if operand(&mut read, 0x5e841d, &[0x6a], 1)? != [7]
            || operand(&mut read, 0x5e8425, &[0x6a], 1)? != [7]
        {
            return Err("unsupported message frame anchor pair".into());
        }
        let unit_size = [
            number(&mut read, 0x5e85fb, &[0xc7, 0x04, 0x24])?,
            number(&mut read, 0x5e860e, &[0xc7, 0x04, 0x24])?,
        ];
        let unit_anchor = [
            number(&mut read, 0x5e862d, &[0xc7, 0x04, 0x24])?,
            number(&mut read, 0x5e8625, &[0xc7, 0x44, 0x24, 0x04])?,
        ];
        if operand(&mut read, 0x5e8634, &[0x6a], 1)? != [1]
            || operand(&mut read, 0x5e863c, &[0x6a], 1)? != [7]
            || operand(&mut read, 0x5f48f6, &[0x6a], 1)? != [1]
            || operand(&mut read, 0x5f48fe, &[0x6a], 1)? != [7]
        {
            return Err("unsupported message unit anchor pair".into());
        }
        let (viewport_width_pointer, viewport_width) =
            referenced(&mut read, 0xab297d, &[0xf3, 0x0f, 0x5e, 0x05])?;
        let (_, viewport_height) = referenced(&mut read, 0xab29a0, &[0xf3, 0x0f, 0x5e, 0x05])?;
        let (width_pointer, width_scale) =
            referenced(&mut read, 0x5f48c0, &[0xf3, 0x0f, 0x10, 0x05])?;
        if width_pointer != viewport_width_pointer || width_scale != viewport_width {
            return Err("message viewport relation".into());
        }
        let (_, top_margin) = referenced(&mut read, 0x5f4887, &[0xf3, 0x0f, 0x58, 0x15])?;
        let (_, inset) = referenced(&mut read, 0x5f48a5, &[0xf3, 0x0f, 0x5c, 0x2d])?;
        let (_, span) = referenced(&mut read, 0x5f4894, &[0xf3, 0x0f, 0x10, 0x05])?;
        if [
            message_width,
            message_height,
            unit_size[0],
            unit_size[1],
            viewport_width,
            viewport_height,
        ]
        .iter()
        .any(|v| *v <= 0.)
            || inset < 0.
            || top_margin < 0.
        {
            return Err("invalid message geometry dimensions".into());
        }
        let message_center_x = viewport_width * 0.5 + message_anchor[0];
        let message = Rect {
            x: message_center_x - message_width * 0.5,
            y: message_anchor[1],
            width: message_width,
            height: message_height,
        };
        if unit_anchor[0] != -inset || unit_anchor[1] != top_margin {
            return Err("message unit anchor/consumer relation".into());
        }
        if viewport_width - (message.x + message.width - inset) <= 0.
            || span <= message.y + message.height + top_margin
        {
            return Err("invalid message travel scale".into());
        }
        let rules = Self {
            origin,
            image_sha256,
            console_key,
            message_key,
            viewport: [viewport_width, viewport_height],
            message,
            unit_size,
            inset,
            top_margin,
            span,
        };
        let zero = rules.placement(0., 0.)?;
        let initial_left = message_center_x + unit_anchor[0] - unit_size[0] * 0.5;
        let initial_bottom = message.y + message.height + unit_anchor[1];
        if (zero.left - initial_left).abs() > 1e-5 || (zero.bottom - initial_bottom).abs() > 1e-5 {
            return Err("message initializer/consumer disagreement".into());
        }
        Ok(rules)
    }

    /// Native placement of the unit message frame for a script `(x, y)` position.
    /// Negative coordinates clamp to zero; both axes scale upward from the bottom.
    pub fn placement(&self, x: f32, y: f32) -> Result<Placement, String> {
        if !x.is_finite() || !y.is_finite() {
            return Err("nonfinite message position".into());
        }
        let x = x.max(0.);
        let y = y.max(0.);
        let top = self.message.y + self.message.height;
        let right = self.message.x + self.message.width;
        let top_edge = top + self.top_margin;
        let right_inset = right - self.inset;
        let left = self.message.x
            + self.message.width * 0.5
            + x * (self.viewport[0] - right_inset)
            + right_inset
            - right
            - self.unit_size[0] * 0.5;
        let bottom = top + y * (self.span - top_edge) + top_edge - top;
        let width = self.unit_size[0];
        let height = self.unit_size[1];
        if [left, bottom].iter().any(|v| !v.is_finite()) {
            return Err("message placement arithmetic overflow".into());
        }
        Ok(Placement {
            left,
            bottom,
            width,
            height,
            viewport: self.viewport,
        })
    }
}

#[cfg(test)]
#[path = "message_layout_tests.rs"]
pub(crate) mod tests;
