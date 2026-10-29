//! The on-screen error line (`WorldFrameMessage`) from the reviewed client image.
//!
//! The client posts an order or control-group refusal through `0x5ed6c0`: it
//! clears the frame at `UI+0x424`, then calls the frame's virtual method at
//! vtable offset `0x70` with the text, a colour word and a hold time. The frame
//! class's constructor `0x88b2b0` fixes the fade time and the text justification,
//! and the UI constructor `0x5e73f0` fixes the font and the text shadow. Every
//! value here is read from those instructions; the evidence is in
//! `docs/re-notes/input-chat-overlays.md`.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    message_layout::{Placement, Rules as MessageRules},
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

const IMAGE_BUDGET: usize = 32 << 20;
/// Longest skin key accepted from the image.
const MAX_KEY_NAME: u32 = 256;

// Posting routine `0x5ed6c0`.
const POST_COLOUR: u32 = 0x5ed6f3;
const POST_FRAME_FIELD: u32 = 0x5ed6e8;
const POST_CLEAR_CALL: u32 = 0x5ed702;
const POST_HOLD: u32 = 0x5ed713;
const POST_METHOD_SLOT: u32 = 0x5ed71e;
const POST_UNKNOWN_CLEAR_CALL: u32 = 0x5ed72b;
const POST_UNKNOWN_HOLD: u32 = 0x5ed73c;
const POST_UNKNOWN_METHOD_SLOT: u32 = 0x5ed74b;
// Frame class constructor `0x88b2b0` and the routines it names.
const CLASS_VTABLE_STORE: u32 = 0x88b2c2;
const CLASS_JUSTIFICATION: u32 = 0x88b3c4;
const CLASS_FADE: u32 = 0x88b3ce;
const CLASS_BOTTOM_UP: u32 = 0x88b3d8;
const CLASS_CLEAR_ROUTINE: u32 = 0x88b7b0;
const CLASS_SHADOW_ROUTINE: u32 = 0x88b6c0;
const CLASS_POST_METHOD: u32 = 0x88b510;
// UI constructor `0x5e73f0`, `WorldFrameMessage` block.
const UI_FONT_FLAGS: u32 = 0x5e83ba;
const UI_FONT_KEY: u32 = 0x5e83c4;
const UI_FRAME_FIELD: u32 = 0x5e838f;
const UI_SHADOW_X: u32 = 0x5e8432;
const UI_SHADOW_Y: u32 = 0x5e8440;
const UI_SHADOW_COLOUR: u32 = 0x5e844a;
const UI_SHADOW_FRAME: u32 = 0x5e8450;
const UI_SHADOW_CALL: u32 = 0x5e8456;

/// Offset of the post method in the frame class's virtual table.
const POST_SLOT_OFFSET: u32 = 0x70;
/// Instance field offsets inside the frame class (`this+0x140`, `+0x150`, `+0x174`).
const FIELD_JUSTIFICATION: u32 = 0x140;
const FIELD_FADE: u32 = 0x150;
const FIELD_BOTTOM_UP: u32 = 0x174;
/// UI object field holding the error frame.
const UI_ERROR_FRAME_FIELD: u32 = 0x424;
/// The client's flag value for left-justified text. Inferred from the order of the
/// `JUSTIFYLEFT`/`JUSTIFYCENTER`/`JUSTIFYRIGHT` tokens and from the upkeep frame,
/// which selects `2` explicitly and is a centred line.
const JUSTIFY_LEFT: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    /// `[r, g, b, a]` in 0..=255.
    pub colour: [u8; 4],
    /// Frame units; Y is up.
    pub offset: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct ErrorLineRules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// The frame the line sits in, in UI frame units with Y up.
    pub frame: Placement,
    /// `[FontHeights]` key of `UI\MiscUI.txt` that sizes the text.
    pub height_key: String,
    /// `[Default]` key of `UI\war3skins.txt` that names the font file.
    pub font_key: String,
    /// `[r, g, b, a]` of the text, from the client's `0xAARRGGBB` word.
    pub colour: [u8; 4],
    /// Seconds the line stays fully visible.
    pub hold_seconds: f32,
    /// Seconds the alpha then falls linearly to zero.
    pub fade_seconds: f32,
    pub shadow: Shadow,
}

impl ErrorLineRules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: String| format!("error line {}: {reason}", origin.source);
        if image.len() > IMAGE_BUDGET
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail("unsupported client identity".into()));
        }
        let digest = sha256(image);
        if digest != REVIEWED_IMAGE {
            return Err(fail(
                "unsupported client image; no rules substituted".into(),
            ));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: IMAGE_BUDGET,
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

    /// Opacity in `0.0..=1.0` `elapsed` seconds after the post, or `None` once the
    /// line is gone. The client lowers the alpha byte by remaining fade time over
    /// the fade time, so the fall is linear.
    pub fn opacity(&self, elapsed: f32) -> Option<f32> {
        if !elapsed.is_finite() || elapsed < 0.0 {
            return None;
        }
        if elapsed <= self.hold_seconds {
            return Some(1.0);
        }
        let remaining = self.hold_seconds + self.fade_seconds - elapsed;
        (remaining > 0.0).then(|| remaining / self.fade_seconds)
    }

    pub(crate) fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let layout = MessageRules::decode(origin.clone(), image_sha256, &mut read)?;
        // The post routine clears the frame, then posts through the virtual method.
        for (va, target) in [
            (POST_CLEAR_CALL, CLASS_CLEAR_ROUTINE),
            (POST_UNKNOWN_CLEAR_CALL, CLASS_CLEAR_ROUTINE),
            (UI_SHADOW_CALL, CLASS_SHADOW_ROUTINE),
        ] {
            let displacement = target.wrapping_sub(va + 5);
            expect(&mut read, va, &[0xe8], displacement)?;
        }
        for va in [POST_METHOD_SLOT, POST_UNKNOWN_METHOD_SLOT] {
            let operand = bytes(&mut read, va, &[0xff, 0x50], 1)?;
            if operand != [POST_SLOT_OFFSET as u8] {
                return Err(format!("unexpected virtual call at {va:#x}"));
            }
        }
        expect(
            &mut read,
            POST_FRAME_FIELD,
            &[0x8b, 0x8e],
            UI_ERROR_FRAME_FIELD,
        )?;
        expect(
            &mut read,
            UI_FRAME_FIELD,
            &[0x89, 0x87],
            UI_ERROR_FRAME_FIELD,
        )?;
        expect(
            &mut read,
            UI_SHADOW_FRAME,
            &[0x8b, 0x8f],
            UI_ERROR_FRAME_FIELD,
        )?;
        let vtable = word(&mut read, CLASS_VTABLE_STORE, &[0xc7, 0x06])?;
        let slot = vtable
            .checked_add(POST_SLOT_OFFSET)
            .ok_or("error frame vtable overflows")?;
        if word_at(&mut read, slot)? != CLASS_POST_METHOD {
            return Err("the post method is not the reviewed routine".into());
        }
        // Colour and hold time, with the unknown-error path agreeing.
        let colour_word = word(&mut read, POST_COLOUR, &[0xc7, 0x45, 0x0c])?;
        let hold_seconds = number(&mut read, POST_HOLD, &[0xc7, 0x04, 0x24])?;
        if number(&mut read, POST_UNKNOWN_HOLD, &[0xc7, 0x04, 0x24])? != hold_seconds {
            return Err("the two post paths disagree on the hold time".into());
        }
        let fade_seconds = number(&mut read, CLASS_FADE, &fade_prefix(FIELD_FADE))?;
        if hold_seconds < 0.0 || fade_seconds <= 0.0 {
            return Err("invalid hold or fade time".into());
        }
        expect(
            &mut read,
            CLASS_JUSTIFICATION,
            &fade_prefix(FIELD_JUSTIFICATION),
            JUSTIFY_LEFT,
        )
        .map_err(|_| "the error frame is not left justified".to_string())?;
        expect(&mut read, CLASS_BOTTOM_UP, &fade_prefix(FIELD_BOTTOM_UP), 1)
            .map_err(|_| "the error frame does not stack from the bottom".to_string())?;
        // Font and shadow of the frame.
        if bytes(&mut read, UI_FONT_FLAGS, &[0x6a], 1)? != [1] {
            return Err("unexpected font flags".into());
        }
        let font_pointer = word(&mut read, UI_FONT_KEY, &[0x68])?;
        let font_key = text(&mut read, font_pointer)?;
        let shadow_colour = word(&mut read, UI_SHADOW_COLOUR, &[0xc7, 0x00])?;
        let shadow_offset = [
            number(
                &mut read,
                UI_SHADOW_X,
                &[0xc7, 0x85, 0xe0, 0xfe, 0xff, 0xff],
            )?,
            number(
                &mut read,
                UI_SHADOW_Y,
                &[0xc7, 0x85, 0xe4, 0xfe, 0xff, 0xff],
            )?,
        ];
        Ok(Self {
            origin,
            image_sha256,
            frame: layout.error_frame(),
            height_key: layout.message_key.clone(),
            font_key,
            colour: argb(colour_word),
            hold_seconds,
            fade_seconds,
            shadow: Shadow {
                colour: argb(shadow_colour),
                offset: shadow_offset,
            },
        })
    }
}

fn expect(
    read: &mut dyn FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    va: u32,
    prefix: &[u8],
    expected: u32,
) -> Result<(), String> {
    if word(read, va, prefix)? == expected {
        Ok(())
    } else {
        Err(format!("unexpected operand at {va:#x}"))
    }
}

/// `mov dword [esi + disp32], imm32` with the displacement filled in.
fn fade_prefix(displacement: u32) -> Vec<u8> {
    let mut prefix = vec![0xc7, 0x86];
    prefix.extend(displacement.to_le_bytes());
    prefix
}

fn argb(word: u32) -> [u8; 4] {
    let [b, g, r, a] = word.to_le_bytes();
    [r, g, b, a]
}

fn bytes(
    read: &mut dyn FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    va: u32,
    prefix: &[u8],
    len: usize,
) -> Result<Vec<u8>, String> {
    let found = read(va, prefix.len() + len, true)?;
    if found.len() != prefix.len() + len || !found.starts_with(prefix) {
        return Err(format!("unsupported instruction at {va:#x}"));
    }
    Ok(found[prefix.len()..].to_vec())
}

fn word(
    read: &mut dyn FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    va: u32,
    prefix: &[u8],
) -> Result<u32, String> {
    let operand: [u8; 4] = bytes(read, va, prefix, 4)?
        .try_into()
        .map_err(|_| "truncated operand")?;
    Ok(u32::from_le_bytes(operand))
}

fn number(
    read: &mut dyn FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    va: u32,
    prefix: &[u8],
) -> Result<f32, String> {
    let value = f32::from_bits(word(read, va, prefix)?);
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("nonfinite operand at {va:#x}"))
    }
}

fn word_at(
    read: &mut dyn FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    va: u32,
) -> Result<u32, String> {
    let data: [u8; 4] = read(va, 4, false)?
        .try_into()
        .map_err(|_| format!("truncated data at {va:#x}"))?;
    Ok(u32::from_le_bytes(data))
}

fn text(
    read: &mut dyn FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    pointer: u32,
) -> Result<String, String> {
    let mut found = Vec::new();
    for offset in 0..MAX_KEY_NAME {
        let address = pointer.checked_add(offset).ok_or("key address overflows")?;
        let [byte] = read(address, 1, false)?[..] else {
            return Err("truncated key".into());
        };
        if byte == 0 {
            let key = String::from_utf8(found).map_err(|e| e.to_string())?;
            return if key.is_empty() || key.chars().any(char::is_control) {
                Err("invalid key".into())
            } else {
                Ok(key)
            };
        }
        found.push(byte);
    }
    Err("overlong key".into())
}

#[cfg(test)]
#[path = "error_line_rules_tests.rs"]
mod tests;
