//! Held inventory geometry from the explicitly reviewed local client image.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Debug)]
pub struct Tooltip {
    pub background_key: String,
    pub edge_key: String,
    pub font_key: String,
    pub name_height_key: String,
    pub description_height_key: String,
    pub name_colour: u32,
    pub description_colour: u32,
    pub name_flags: u8,
    pub description_flags: u8,
    pub description_cap: f32,
    pub outer_padding: f32,
    pub title_inset: [f32; 2],
    pub description_gap: f32,
    pub height_gap: f32,
    pub inset: f32,
    pub edge_size: f32,
    pub edge_flags: u32,
    pub background_repeat: bool,
    pub edge_repeat: bool,
    /// Descriptor index selected by the native tooltip checkbox read. Its name and
    /// fallback come from `Defaults::discover` on the same image.
    pub preference_index: u32,
    points: [(u32, u32); 2],
}
#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub console_key: String,
    pub empty_key: String,
    pub disabled_key: String,
    pub cover_key: String,
    pub font_key: String,
    pub font_section: String,
    pub font_height_key: String,
    pub label_key: String,
    pub label_colour: u32,
    pub shadow_colour: u32,
    pub shadow_offset: [f32; 2],
    pub label_flags: u32,
    pub count: usize,
    pub viewport: [f32; 2],
    pub icon_repeat: bool,
    pub cover_repeat: bool,
    pub cover_uv: [f32; 4],
    pub tooltip: Tooltip,
    info: Rect,
    cover: Rect,
    label: Rect,
    slot_size: [f32; 2],
    slot_origin: [f32; 2],
    slot_step: [f32; 2],
    column_mask: usize,
    row_shift: u32,
    slot_keys: Vec<u32>,
}
/// Reads the instruction at `va`, checks its opcode bytes and returns the `size` operand
/// bytes after them.
fn operand(
    read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    va: u32,
    prefix: &[u8],
    size: usize,
) -> Result<Vec<u8>, String> {
    let b = read(va, prefix.len() + size, true)?;
    if b.get(..prefix.len()) != Some(prefix) {
        return Err(format!("unsupported inventory instruction at {va:#x}"));
    }
    Ok(b[prefix.len()..].to_vec())
}
fn word(b: Vec<u8>) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        b.try_into().map_err(|_| "truncated inventory operand")?,
    ))
}

/// The windows virtual-key code of NumPad 0. The client labels slot keys by NumPad digit
/// and stores virtual-key codes; the two agree through this base (`VK_NUMPAD0`).
const NUMPAD_0: u32 = 0x60;

/// The virtual-key code each inventory slot button stores as its hotkey.
///
/// Evidence, all read from the image: the button update stores `table[slot]` in the button's
/// hotkey field (`0x649448` to `0x649455`), the key matcher compares that field with the
/// pressed key (`0x63919e`), and the tooltip builder labels the same slots through a jump
/// table of NumPad digits (`0x64927c`, one `mov ebx, digit` per slot). The reader refuses
/// unless the three agree on the field and on every slot: key = `VK_NUMPAD0` + label.
fn slot_keys(
    read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    count: usize,
) -> Result<Vec<u32>, String> {
    let field = operand(read, 0x649455, &[0x89, 0x88], 4)?;
    if operand(read, 0x63919e, &[0x39, 0x98], 4)? != field {
        return Err("inventory hotkey field relation".into());
    }
    let table = word(operand(read, 0x64944e, &[0x8b, 0x0c, 0x8d], 4)?)?;
    let labels = word(operand(read, 0x64927c, &[0xff, 0x24, 0x85], 4)?)?;
    let last = usize::from(operand(read, 0x649277, &[0x83, 0xf8], 1)?[0]);
    if last + 1 != count {
        return Err("inventory hotkey label count".into());
    }
    (0..count)
        .map(|slot| {
            let offset = u32::try_from(slot * 4).map_err(|_| "inventory hotkey slot")?;
            let key = word(read(table.wrapping_add(offset), 4, false)?)?;
            let target = word(read(labels.wrapping_add(offset), 4, false)?)?;
            let label = word(operand(read, target, &[0xbb], 4)?)?;
            if key != NUMPAD_0 + label {
                return Err(format!("inventory slot {slot} key and label disagree"));
            }
            Ok(key)
        })
        .collect()
}

impl Rules {
    /// Shared selected-unit information frame in the logical ConsoleUI surface.
    pub fn info_panel(&self) -> Rect {
        self.info
    }
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |s: &str| format!("inventory layout {}: {s}", origin.source);
        let digest = sha256(image);
        if image.len() > 32 << 20
            || digest != REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail("unsupported client identity"));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| fail(&e.to_string()))?;
        Self::decode(origin.clone(), digest, |va, n, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "truncated inventory record".into())
        })
        .map_err(|e| fail(&e))
    }
    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        fn string(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            pointer: u32,
        ) -> Result<String, String> {
            let mut text = Vec::new();
            for offset in 0..4096 {
                let b = read(
                    pointer
                        .checked_add(offset)
                        .ok_or("inventory string address overflow")?,
                    1,
                    false,
                )?;
                if b[0] == 0 {
                    return String::from_utf8(text).map_err(|e| e.to_string());
                }
                text.push(b[0]);
            }
            Err("inventory string budget".into())
        }
        fn pushed_string(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
        ) -> Result<String, String> {
            let p = word(operand(read, va, &[0x68], 4)?)?;
            string(read, p)
        }
        fn immediate(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
        ) -> Result<f32, String> {
            let v = f32::from_bits(word(operand(read, va, prefix, 4)?)?);
            if !v.is_finite() {
                return Err("nonfinite inventory geometry".into());
            }
            Ok(v)
        }
        fn referenced(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
        ) -> Result<f32, String> {
            let p = word(operand(read, va, prefix, 4)?)?;
            let v = f32::from_bits(word(read(p, 4, false)?)?);
            if !v.is_finite() {
                return Err("nonfinite inventory table float".into());
            }
            Ok(v)
        }
        let mask = operand(&mut read, 0x887ce5, &[0x83, 0xe0], 1)?[0];
        let icon_repeat = operand(&mut read, 0x8750db, &[0x6a], 1)?[0] & mask != 0;
        if (operand(&mut read, 0x875141, &[0x6a], 1)?[0] & mask != 0) != icon_repeat {
            return Err("inventory image sampler relation".into());
        }
        let cover_repeat = operand(&mut read, 0x61e8bd, &[0x6a], 1)?[0] & mask != 0;
        let cover_pointer = word(operand(&mut read, 0x61e8d6, &[0x0f, 0x28, 5], 4)?)?;
        let cover_values = read(cover_pointer, 16, false)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect::<Vec<_>>();
        let cover_uv = [
            cover_values[1],
            cover_values[0],
            cover_values[3],
            cover_values[2],
        ];
        if cover_uv
            .iter()
            .any(|n| !n.is_finite() || !(0.0..=1.0).contains(n))
            || cover_uv[0] >= cover_uv[2]
            || cover_uv[1] >= cover_uv[3]
        {
            return Err("inventory cover source UV rectangle".into());
        }
        let viewport = [
            referenced(&mut read, 0x640ea8, &[0xf3, 0x0f, 0x10, 5])?,
            referenced(&mut read, 0x640e96, &[0xf3, 0x0f, 0x10, 0x1d])?,
        ];
        if viewport.iter().any(|n| *n <= 0.) {
            return Err("inventory logical viewport bounds".into());
        }
        let edge_flags = word(operand(&mut read, 0x622213, &[0xc7, 0x43, 0x2c], 4)?)?;
        let background_repeat =
            word(operand(&mut read, 0x62221b, &[0xc7, 0x43, 0x30], 4)?)? & u32::from(mask) != 0;
        let edge_repeat = operand(&mut read, 0x88dd0e, &[0x6a], 1)?[0] & mask != 0;
        let inset = immediate(&mut read, 0x622231, &[0xc7, 0x43, 0x48])?;
        for (va, field) in [(0x622238, 0x4c), (0x62223f, 0x40), (0x622246, 0x44)] {
            if immediate(&mut read, va, &[0xc7, 0x43, field])? != inset {
                return Err("unequal native tooltip background insets unsupported".into());
            }
        }
        let outer_padding = referenced(&mut read, 0x65a9f9, &[0xf3, 0x0f, 0x58, 5])?;
        if referenced(&mut read, 0x65aa10, &[0xf3, 0x0f, 0x58, 5])? != outer_padding {
            return Err("tooltip width/height padding relation".into());
        }
        let gap_pointer = word(operand(&mut read, 0x65a9b5, &[0xd8, 5], 4)?)?;
        let above = word(operand(&mut read, 0x640eca, &[0xbf], 4)?)?;
        let below = word(operand(&mut read, 0x640efb, &[0xbf], 4)?)?;
        let above_delta = i32::from(operand(&mut read, 0x640ecf, &[0x8d, 0x5f], 1)?[0] as i8);
        let below_delta = i32::from(operand(&mut read, 0x640f00, &[0x8d, 0x5f], 1)?[0] as i8);
        let points = [
            (above, above.wrapping_add_signed(above_delta)),
            (below, below.wrapping_add_signed(below_delta)),
        ];
        if points.iter().any(|(own, parent)| *own > 8 || *parent > 8) {
            return Err("unsupported native tooltip frame point".into());
        }
        let tooltip = Tooltip {
            points,
            background_key: pushed_string(&mut read, 0x6221f8)?,
            edge_key: pushed_string(&mut read, 0x6221e8)?,
            font_key: pushed_string(&mut read, 0x6222f1)?,
            name_height_key: pushed_string(&mut read, 0x6222c8)?,
            description_height_key: pushed_string(&mut read, 0x62239d)?,
            name_colour: word(operand(&mut read, 0x622314, &[0xc7, 0x45, 8], 4)?)?,
            description_colour: word(operand(&mut read, 0x6223e9, &[0xc7, 0x45, 8], 4)?)?,
            name_flags: operand(&mut read, 0x62234a, &[0x6a], 1)?[0],
            description_flags: operand(&mut read, 0x622424, &[0x6a], 1)?[0],
            description_cap: referenced(&mut read, 0x65a924, &[0xf3, 0x0f, 0x5d, 0x0d])?,
            outer_padding,
            title_inset: [
                immediate(&mut read, 0x622333, &[0xc7, 4, 0x24])?,
                -immediate(&mut read, 0x62232b, &[0xc7, 0x44, 0x24, 4])?,
            ],
            description_gap: -immediate(&mut read, 0x65a8b5, &[0xc7, 0x44, 0x24, 4])?,
            height_gap: f32::from_bits(word(read(gap_pointer, 4, false)?)?),
            inset,
            edge_size: immediate(&mut read, 0x62222a, &[0xc7, 0x43, 0x38])?,
            edge_flags,
            background_repeat,
            edge_repeat,
            preference_index: u32::from(operand(&mut read, 0x6491aa, &[0x6a], 1)?[0]),
        };
        if [
            tooltip.description_cap,
            tooltip.outer_padding,
            tooltip.edge_size,
        ]
        .iter()
        .any(|n| !n.is_finite() || *n <= 0.)
            || [
                tooltip.inset,
                tooltip.height_gap,
                tooltip.description_gap,
                tooltip.title_inset[0],
                tooltip.title_inset[1],
            ]
            .iter()
            .any(|n| !n.is_finite() || *n < 0.)
        {
            return Err("invalid tooltip source extent".into());
        }
        let console_key = pushed_string(&mut read, 0x5e7ca5)?;
        let field = operand(&mut read, 0x5e7cbd, &[0x89, 0xb7], 4)?;
        if operand(&mut read, 0x5e8096, &[0xff, 0xb7], 4)? != field
            || operand(&mut read, 0x5e7d28, &[0x8b, 0x8f], 4)? != field
        {
            return Err("inventory console parent relation".into());
        }
        // Native SetAllPoints connects this selected skin frame to GameUI's logical root.
        operand(&mut read, 0x5e7d19, &[0x8d, 0x87], 4)?;
        operand(&mut read, 0x5e7d2e, &[0xe8], 4)?;
        let info = Rect {
            x: immediate(&mut read, 0x61e654, &[0xc7, 4, 0x24])?,
            y: immediate(&mut read, 0x61e64c, &[0xc7, 0x44, 0x24, 4])?,
            width: immediate(&mut read, 0x61e672, &[0xc7, 4, 0x24])?,
            height: immediate(&mut read, 0x61e681, &[0xc7, 4, 0x24])?,
        };
        let own = operand(&mut read, 0x61e65e, &[0x6a], 1)?[0];
        let parent = operand(&mut read, 0x61e65b, &[0x6a], 1)?[0];
        // Frame-point enumeration is a format invariant, not a measured coordinate.
        if own != 6 || parent != 6 {
            return Err("unsupported inventory InfoPanel anchor".into());
        }
        let slot_size = [
            immediate(&mut read, 0x61f54b, &[0xc7, 4, 0x24])?,
            immediate(&mut read, 0x61f55a, &[0xc7, 4, 0x24])?,
        ];
        let slot_origin = [
            referenced(&mut read, 0x61f5fc, &[0xf3, 0x0f, 0x58, 5])?,
            referenced(&mut read, 0x61f59c, &[0xf3, 0x0f, 0x10, 0x0d])?,
        ];
        let slot_step = [
            referenced(&mut read, 0x61f5f4, &[0xf3, 0x0f, 0x59, 5])?,
            referenced(&mut read, 0x61f5ca, &[0xf3, 0x0f, 0x59, 5])?,
        ];
        if read(0x61f5a4, 2, true)? != [0xd1, 0xe8] {
            return Err("unsupported inventory row arithmetic".into());
        }
        let row_shift = 1; // x86 D1 /5 shifts by one.
        let column_mask = usize::from(operand(&mut read, 0x61f5c3, &[0x83, 0xe0], 1)?[0]);
        if column_mask != (1usize << row_shift) - 1 {
            return Err("inconsistent inventory row/column operands".into());
        }
        let count = usize::from(operand(&mut read, 0x61f69f, &[0x83, 0xfb], 1)?[0]);
        if count == 0 || operand(&mut read, 0x61f4d8, &[0x6a], 1)?[0] as usize != count {
            return Err("inconsistent inventory button count".into());
        }
        let slot_keys = slot_keys(&mut read, count)?;
        if operand(&mut read, 0x61f60e, &[0x6a], 1)? != [0]
            || operand(&mut read, 0x61f609, &[0x6a], 1)? != [2]
        {
            return Err("unsupported inventory slot anchor".into());
        }
        let cover_width = immediate(&mut read, 0x61e829, &[0xc7, 4, 0x24])?;
        let cover_height = immediate(&mut read, 0x61e83c, &[0xc7, 4, 0x24])?;
        let cover_x = immediate(&mut read, 0x61e85b, &[0xc7, 4, 0x24])?;
        let cover_y = immediate(&mut read, 0x61e853, &[0xc7, 0x44, 0x24, 4])?;
        if operand(&mut read, 0x61e862, &[0x6a], 1)? != [8] {
            return Err("unsupported inventory cover anchor".into());
        }
        let cover = Rect {
            x: cover_x - cover_width,
            y: cover_y,
            width: cover_width,
            height: cover_height,
        };
        let label = Rect {
            x: immediate(&mut read, 0x61e9ab, &[0xc7, 4, 0x24])?,
            y: immediate(&mut read, 0x61e9a3, &[0xc7, 0x44, 0x24, 4])?,
            width: immediate(&mut read, 0x61e9d6, &[0xc7, 4, 0x24])?,
            height: immediate(&mut read, 0x61e9c3, &[0xc7, 4, 0x24])?,
        };
        if operand(&mut read, 0x61e9b5, &[0x6a], 1)? != [0]
            || operand(&mut read, 0x61e9b2, &[0x6a], 1)? != [2]
        {
            return Err("unsupported inventory label anchor".into());
        }
        let font_section_pointer = word(operand(&mut read, 0x61e939, &[0xff, 0x35], 4)?)?;
        let section_address = word(read(font_section_pointer, 4, false)?)?;
        let font_section = string(&mut read, section_address)?;
        let font_height_key = pushed_string(&mut read, 0x61e934)?;
        let font_key = pushed_string(&mut read, 0x61e963)?;
        let label_key = pushed_string(&mut read, 0x61ea25)?;
        let label_colour = word(operand(
            &mut read,
            0x61e989,
            &[0xc7, 0x85, 0x6c, 0xff, 0xff, 0xff],
            4,
        )?)?;
        let shadow_colour = word(operand(
            &mut read,
            0x61ea0a,
            &[0xc7, 0x85, 0x6c, 0xff, 0xff, 0xff],
            4,
        )?)?;
        let shadow_offset = [
            immediate(&mut read, 0x61e9f5, &[0xc7, 0x85, 0x60, 0xff, 0xff, 0xff])?,
            immediate(&mut read, 0x61ea00, &[0xc7, 0x85, 0x64, 0xff, 0xff, 0xff])?,
        ];
        let label_flags = word(operand(
            &mut read,
            0x8864ac,
            &[0xc7, 0x86, 0xc4, 0, 0, 0],
            4,
        )?)?;
        if [
            info.width,
            info.height,
            label.width,
            label.height,
            slot_size[0],
            slot_size[1],
            slot_step[0],
            slot_step[1],
        ]
        .iter()
        .any(|n| *n <= 0.)
        {
            return Err("nonpositive inventory extent".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            console_key,
            empty_key: pushed_string(&mut read, 0x61f639)?,
            disabled_key: pushed_string(&mut read, 0x645240)?,
            cover_key: pushed_string(&mut read, 0x61e8c1)?,
            font_key,
            font_section,
            font_height_key,
            label_key,
            label_colour,
            shadow_colour,
            shadow_offset,
            label_flags,
            count,
            viewport,
            icon_repeat,
            cover_repeat,
            cover_uv,
            tooltip,
            info,
            cover,
            label,
            slot_size,
            slot_origin,
            slot_step,
            column_mask,
            row_shift,
            slot_keys,
        })
    }
    /// The virtual-key code that uses the item in `slot`, from the client image.
    pub fn slot_key(&self, slot: usize) -> Option<u32> {
        self.slot_keys.get(slot).copied()
    }
    pub fn slot(&self, index: usize) -> Result<Rect, String> {
        if index >= self.count {
            return Err("inventory slot exceeds source button count".into());
        }
        Ok(Rect {
            x: self.info.x
                + self.info.width
                + self.slot_origin[0]
                + (index & self.column_mask) as f32 * self.slot_step[0],
            y: self.info.y + self.info.height + self.slot_origin[1]
                - (index >> self.row_shift) as f32 * self.slot_step[1]
                - self.slot_size[1],
            width: self.slot_size[0],
            height: self.slot_size[1],
        })
    }
    pub fn cover(&self) -> Rect {
        self.cover
    }
    /// The reviewed normal tooltip attaches to the hovered slot and applies the
    /// client's two boundary branches, including its below/left fallback.
    pub fn tooltip_rect(&self, slot: Rect, width: f32, height: f32) -> Result<Rect, String> {
        if [slot.x, slot.y, slot.width, slot.height, width, height]
            .iter()
            .any(|n| !n.is_finite())
            || width <= 0.
            || height <= 0.
        {
            return Err("invalid measured inventory tooltip extent".into());
        }
        let right = slot.x + slot.width;
        let top = slot.y + slot.height;
        let fits_x = right + width < self.viewport[0];
        let fits_y = top + height < self.viewport[1];
        let x_offset = if fits_x {
            0.
        } else {
            self.viewport[0] - (right + width)
        };
        let y_offset = if fits_x && !fits_y {
            self.viewport[1] - (top + height)
        } else {
            0.
        };
        let (own, parent) = self.tooltip.points[usize::from(!fits_x && !fits_y)];
        let point = |which: u32, rect: Rect| {
            [
                rect.x + (which % 3) as f32 * rect.width / 2.,
                rect.y + (2 - which / 3) as f32 * rect.height / 2.,
            ]
        };
        let target = point(parent, slot);
        let origin = point(
            own,
            Rect {
                x: 0.,
                y: 0.,
                width,
                height,
            },
        );
        let (x, y) = (
            target[0] + x_offset - origin[0],
            target[1] + y_offset - origin[1],
        );
        Ok(Rect {
            x,
            y,
            width,
            height,
        })
    }
    pub fn label(&self) -> Rect {
        Rect {
            x: self.info.x + self.info.width + self.label.x,
            y: self.info.y + self.info.height + self.label.y - self.label.height,
            ..self.label
        }
    }
}

/// Synthetic layouts for unit and ECS tests in dependent crates. These values
/// are not the supplied client's data.
#[cfg(any(test, feature = "test-fixtures"))]
pub mod fixtures {
    use super::*;
    pub fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic inventory image".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        }
    }
    pub fn synthetic() -> Rules {
        Rules {
            origin: origin(),
            image_sha256: sha256(b"synthetic"),
            console_key: "Console".into(),
            empty_key: "Empty".into(),
            disabled_key: "Disabled".into(),
            cover_key: "Cover".into(),
            font_key: "Font".into(),
            font_section: "Heights".into(),
            font_height_key: "Caption".into(),
            label_key: "LABEL".into(),
            label_colour: 0xff123456,
            shadow_colour: 0x80112233,
            shadow_offset: [0.002, -0.003],
            label_flags: 2,
            count: 4,
            viewport: [1., 1.],
            icon_repeat: false,
            cover_repeat: false,
            cover_uv: [0., 0.2, 1., 1.],
            info: Rect {
                x: 0.1,
                y: 0.2,
                width: 0.2,
                height: 0.15,
            },
            cover: Rect {
                x: 0.5,
                y: 0.,
                width: 0.2,
                height: 0.2,
            },
            label: Rect {
                x: 0.01,
                y: 0.02,
                width: 0.08,
                height: 0.02,
            },
            slot_size: [0.04, 0.03],
            slot_origin: [0.02, -0.005],
            slot_step: [0.05, 0.06],
            column_mask: 1,
            row_shift: 1,
            slot_keys: vec![0x41, 0x42, 0x43, 0x44],
            tooltip: Tooltip {
                background_key: "Background".into(),
                edge_key: "Edge".into(),
                font_key: "TipFont".into(),
                name_height_key: "Name".into(),
                description_height_key: "Body".into(),
                name_colour: 0xff123456,
                description_colour: 0xff654321,
                name_flags: 1,
                description_flags: 1,
                description_cap: 0.25,
                outer_padding: 0.02,
                title_inset: [0.01, 0.01],
                description_gap: 0.01,
                height_gap: 0.01,
                inset: 0.001,
                edge_size: 0.01,
                edge_flags: 0xff,
                background_repeat: true,
                edge_repeat: true,
                preference_index: 2,
                points: [(6, 2), (2, 6)],
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{
        fixtures::{origin, synthetic},
        *,
    };
    #[test]
    fn relative_inventory_and_tooltip_geometry_follow_the_supplied_operands() {
        let mut rules = synthetic();
        let first = rules.slot(0).unwrap();
        let second = rules.slot(1).unwrap();
        let third = rules.slot(2).unwrap();
        assert!((first.x - 0.32).abs() < 0.000001);
        assert!((first.y - 0.315).abs() < 0.000001);
        assert!((second.x - first.x - rules.slot_step[0]).abs() < 0.000001);
        assert!((first.y - third.y - rules.slot_step[1]).abs() < 0.000001);
        assert!(rules.slot(4).is_err());
        let title = rules.label();
        assert!((title.x - 0.31).abs() < 0.000001);
        let tip = rules.tooltip_rect(first, 0.2, 0.1).unwrap();
        assert!((tip.x - first.x - first.width).abs() < 0.000001);
        assert!((tip.y - first.y - first.height).abs() < 0.000001);
        rules.slot_step = [0.12, 0.09];
        rules.info.x += 0.07;
        assert!((rules.slot(1).unwrap().x - first.x - 0.19).abs() < 0.000001);
        let edge = rules
            .tooltip_rect(
                Rect {
                    x: 0.9,
                    y: 0.2,
                    width: 0.05,
                    height: 0.04,
                },
                0.2,
                0.1,
            )
            .unwrap();
        assert!((edge.x - 0.8).abs() < 0.000001);
        assert!(rules.tooltip_rect(first, f32::NAN, 0.1).is_err());
    }
    #[test]
    fn sampler_admission_uses_the_source_mask_and_rejects_inconsistent_paths() {
        let run = |mask, existing, new| {
            Rules::decode(origin(), sha256(b"fixture"), |va, n, _| {
                let data = match va {
                    0x887ce5 => vec![0x83, 0xe0, mask],
                    0x8750db => vec![0x6a, existing],
                    0x875141 => vec![0x6a, new],
                    _ => return Err("fixture stopped after sampler admission".into()),
                };
                assert_eq!(data.len(), n);
                Ok(data)
            })
        };
        assert!(run(1, 0, 1).unwrap_err().contains("sampler relation"));
        assert!(run(2, 0, 1).unwrap_err().contains("stopped after sampler"));
        assert!(
            Rules::decode(origin(), sha256(b"bad"), |_, n, _| Ok(vec![0; n]))
                .unwrap_err()
                .contains("instruction")
        );
    }
    /// A synthetic image slice for `slot_keys`: the same instruction shapes at the reviewed
    /// addresses, with nonstock labels and a relocated hotkey field and tables.
    fn key_image(
        labels: &[u32],
        keys: &[u32],
        field: [u32; 2],
        last: u8,
    ) -> impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String> {
        const KEYS: u32 = 0x2000;
        const JUMPS: u32 = 0x3000;
        const TARGETS: u32 = 0x4000;
        let (labels, keys) = (labels.to_vec(), keys.to_vec());
        move |va, n, _| {
            let word = |value: u32| value.to_le_bytes().to_vec();
            let mut bytes = match va {
                0x649455 => [vec![0x89, 0x88], word(field[0])].concat(),
                0x63919e => [vec![0x39, 0x98], word(field[1])].concat(),
                0x64944e => [vec![0x8b, 0x0c, 0x8d], word(KEYS)].concat(),
                0x64927c => [vec![0xff, 0x24, 0x85], word(JUMPS)].concat(),
                0x649277 => vec![0x83, 0xf8, last],
                _ if (KEYS..KEYS + 4 * keys.len() as u32).contains(&va) => {
                    word(keys[((va - KEYS) / 4) as usize])
                }
                _ if (JUMPS..JUMPS + 4 * labels.len() as u32).contains(&va) => {
                    word(TARGETS + 0x10 * ((va - JUMPS) / 4))
                }
                _ if (TARGETS..TARGETS + 0x10 * labels.len() as u32).contains(&va) => {
                    [vec![0xbb], word(labels[((va - TARGETS) / 0x10) as usize])].concat()
                }
                _ => return Err(format!("stopped at {va:#x}")),
            };
            bytes.resize(n.max(bytes.len()), 0);
            Ok(bytes)
        }
    }
    #[test]
    fn slot_keys_are_the_stored_codes_when_table_labels_and_matcher_agree() {
        let labels = [3, 9, 0, 1];
        let keys = [0x63, 0x69, 0x60, 0x61];
        assert_eq!(
            slot_keys(&mut key_image(&labels, &keys, [0x5ac, 0x5ac], 3), 4).unwrap(),
            keys
        );
        let run = |labels: &[u32], keys: &[u32], field: [u32; 2], last: u8, count| {
            slot_keys(&mut key_image(labels, keys, field, last), count).unwrap_err()
        };
        assert!(run(&labels, &keys, [0x5ac, 0x5b0], 3, 4).contains("field relation"));
        assert!(run(&labels, &keys, [0x5ac, 0x5ac], 4, 4).contains("label count"));
        assert!(run(&labels, &keys, [0x5ac, 0x5ac], 3, 5).contains("label count"));
        assert!(
            run(&labels, &[0x63, 0x69, 0x60, 0x62], [0x5ac, 0x5ac], 3, 4)
                .contains("slot 3 key and label disagree")
        );
        assert!(run(&labels, &keys[..3], [0x5ac, 0x5ac], 3, 4).contains("stopped at"));
    }
    #[test]
    fn cover_source_rectangle_is_admitted_only_as_ordered_unit_uv() {
        let decode = |uv: [f32; 4]| {
            // Stored order is the inverse of the `cover_uv` field order.
            let stored = [uv[1], uv[0], uv[3], uv[2]];
            let bytes = stored
                .iter()
                .flat_map(|f| f.to_le_bytes())
                .collect::<Vec<_>>();
            Rules::decode(origin(), sha256(b"fixture"), move |va, _, _| match va {
                0x887ce5 => Ok(vec![0x83, 0xe0, 1]),
                0x8750db | 0x875141 | 0x61e8bd => Ok(vec![0x6a, 0]),
                0x61e8d6 => Ok(vec![0x0f, 0x28, 5, 0x00, 0x10, 0x00, 0x00]),
                0x1000 => Ok(bytes.clone()),
                _ => Err(format!("stopped at {va:#x}")),
            })
        };
        let error = |uv| decode(uv).unwrap_err();
        assert!(error([0.5, 0., 0.4, 1.]).contains("cover source UV"));
        assert!(error([0., 0.5, 1., 0.4]).contains("cover source UV"));
        assert!(error([0., 0., 1.5, 1.]).contains("cover source UV"));
        assert!(error([0., f32::NAN, 1., 1.]).contains("cover source UV"));
        assert!(error([0., 0.2, 1., 1.]).contains("stopped at 0x640ea8"));
    }
    #[test]
    #[ignore = "requires PCX_PLAYER_RESOURCE_IMAGE selecting the explicitly supplied reviewed client"]
    fn supplied_client_inventory_layout_and_tooltip_operands_are_admitted() {
        let path = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").expect("explicit local client image");
        let bytes = std::fs::read(&path).unwrap();
        let rules = Rules::reviewed_legacy(
            &bytes,
            AssetIdentity {
                source: path,
                edition: "explicit selected local client".into(),
                version: "reviewed client; digest retained".into(),
            },
        )
        .unwrap();
        assert!(!rules.console_key.is_empty());
        assert!(!rules.font_height_key.is_empty());
        let keys: Vec<_> = (0..rules.count)
            .map(|i| rules.slot_key(i).unwrap())
            .collect();
        assert_eq!(rules.slot_key(rules.count), None);
        let mut distinct = keys.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), keys.len(), "{keys:?}");
        for index in 0..rules.count {
            let slot = rules.slot(index).unwrap();
            assert!(slot.width > 0. && slot.height > 0.);
            rules
                .tooltip_rect(
                    slot,
                    rules.tooltip.description_cap,
                    rules.tooltip.outer_padding + slot.height,
                )
                .unwrap();
        }
    }
}
