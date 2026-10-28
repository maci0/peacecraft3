//! Windows virtual-key codes of physical keys. The client stores and compares hotkeys in
//! these codes (`docs/re-notes/input-command-keys.md`); the numbering is the Windows
//! format, not a game value.
use bevy::prelude::KeyCode;

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];
const LETTERS: [KeyCode; 26] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
];
const NUMPAD: [KeyCode; 10] = [
    KeyCode::Numpad0,
    KeyCode::Numpad1,
    KeyCode::Numpad2,
    KeyCode::Numpad3,
    KeyCode::Numpad4,
    KeyCode::Numpad5,
    KeyCode::Numpad6,
    KeyCode::Numpad7,
    KeyCode::Numpad8,
    KeyCode::Numpad9,
];
const FUNCTION: [KeyCode; 12] = [
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
];
const BACKSPACE: u32 = 0x08;
const TAB: u32 = 0x09;
const ENTER: u32 = 0x0D;
const ESCAPE: u32 = 0x1B;
const SPACE: u32 = 0x20;
const DIGIT_0: u32 = 0x30;
const LETTER_A: u32 = 0x41;
const NUMPAD_0: u32 = 0x60;
const FUNCTION_1: u32 = 0x70;

/// The virtual-key code of a physical key, for the keys a hotkey can name.
pub(crate) fn code(key: KeyCode) -> Option<u32> {
    let offset = |table: &[KeyCode], first: u32| {
        table
            .iter()
            .position(|&entry| entry == key)
            .and_then(|index| u32::try_from(index).ok())
            .map(|index| first + index)
    };
    match key {
        KeyCode::Backspace => Some(BACKSPACE),
        KeyCode::Tab => Some(TAB),
        KeyCode::Enter => Some(ENTER),
        KeyCode::Escape => Some(ESCAPE),
        KeyCode::Space => Some(SPACE),
        _ => offset(&DIGITS, DIGIT_0)
            .or_else(|| offset(&LETTERS, LETTER_A))
            .or_else(|| offset(&NUMPAD, NUMPAD_0))
            .or_else(|| offset(&FUNCTION, FUNCTION_1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map_to_the_windows_codes_the_client_compares() {
        assert_eq!(code(KeyCode::KeyA), Some(0x41));
        assert_eq!(code(KeyCode::KeyZ), Some(0x5A));
        assert_eq!(code(KeyCode::Digit0), Some(0x30));
        assert_eq!(code(KeyCode::Digit9), Some(0x39));
        assert_eq!(code(KeyCode::Numpad7), Some(0x67));
        assert_eq!(code(KeyCode::Numpad1), Some(0x61));
        assert_eq!(code(KeyCode::F1), Some(0x70));
        assert_eq!(code(KeyCode::F12), Some(0x7B));
        assert_eq!(code(KeyCode::Escape), Some(0x1B));
        assert_eq!(code(KeyCode::Space), Some(0x20));
        assert_eq!(code(KeyCode::ShiftLeft), None);
        assert_eq!(code(KeyCode::ArrowUp), None);
    }
}
