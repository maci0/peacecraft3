//! `GetGameTypeSelected`: the game-info flags word and the launch routes that set it,
//! decoded from the reviewed client. See `docs/re-notes/chapter-startup-refusals.md`.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

/// How the map was started. Routes that are not decoded have no variant, so a script that
/// asks for the game type there keeps its refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchRoute {
    /// The main menu Credits button (`CreditsButton`, `0x56a140`).
    CreditsButton,
    /// The single-player menu start command (`0x58e050`).
    SinglePlayerStart,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// Bits of the request flags the host keeps in the game info (`and eax, 0x3ff`).
    stored_mask: u32,
    /// Bits `GetGameTypeSelected` clears (`and eax, ~8`).
    cleared: u32,
    credits_flags: u32,
    start_flags: u32,
}

/// Native `GetGameTypeSelected`: `mov eax,[game]; mov eax,[eax+0x30]; mov eax,[eax+0x34];
/// and eax,imm8; ret`. The game global is a data address, so only the offsets are checked.
const NATIVE: (u32, usize) = (0x492810, 15);
/// Host game creation: `mov eax,[ebx+0x148]; and eax,imm32`, the flags stored in the game info.
const HOST_MASK: (u32, usize) = (0x51eb49, 11);
/// `push 0; push 0; push 2; push 0; push flags; lea eax,[ebp-X]; push eax; push esi;
/// call 0x576f50`, the Credits button's map launch.
const CREDITS_LAUNCH: (u32, usize) = (0x56a1ab, 23);
/// `push 0; push eax; push ecx; push ecx; push 2; push 0; push flags; lea eax,[esi+X];
/// push eax; push esi; lea ecx,[esi+0x380]; call 0x576f50`, the start command's launch.
const START_LAUNCH: (u32, usize) = (0x58e0ab, 33);
/// `mov ebx,[ebp+0x10]` and `mov [esi+0xc],ebx` in the shared launch-request setter.
const SETTER_LOAD: (u32, usize) = (0x576f6c, 3);
const SETTER_STORE: (u32, usize) = (0x576fd9, 3);
/// The listener's `push [edi+0xc]`, the request flags passed to the local-game creation call.
const LISTENER_FLAGS: (u32, usize) = (0x568258, 3);
const SETTER: u32 = 0x576f50;

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let context = format!("game type rules {}", origin.source);
        let fail = |why: &str| format!("{context}: {why}");
        if image.len() > 32 << 20
            || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
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
        Self::decode(origin, sha256(image), |(va, n)| {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "truncated game type instruction window".into())
        })
        .map_err(|e| fail(&e))
    }

    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut((u32, usize)) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let native = read(NATIVE)?;
        if native[0] != 0xa1
            || native[5..11] != [0x8b, 0x40, 0x30, 0x8b, 0x40, 0x34]
            || native[11..13] != [0x83, 0xe0]
            || native[14] != 0xc3
        {
            return Err("unsupported GetGameTypeSelected handler".into());
        }
        // `and eax, imm8` sign-extends, so the cleared bits are the complement of that mask.
        let cleared = !(i32::from(i8::from_le_bytes([native[13]])) as u32);

        let host = read(HOST_MASK)?;
        if host[..2] != [0x8b, 0x83] || host[2..6] != 0x148u32.to_le_bytes() || host[6] != 0x25 {
            return Err("unsupported game creation flags window".into());
        }
        let stored_mask = u32::from_le_bytes(host[7..11].try_into().map_err(|_| "mask")?);

        let setter_load = read(SETTER_LOAD)?;
        let setter_store = read(SETTER_STORE)?;
        let listener = read(LISTENER_FLAGS)?;
        if setter_load != [0x8b, 0x5d, 0x10]
            || setter_store != [0x89, 0x5e, 0x0c]
            || listener != [0xff, 0x77, 0x0c]
        {
            return Err("launch request flags are not passed as decoded".into());
        }

        let credits = read(CREDITS_LAUNCH)?;
        if credits[..8] != [0x6a, 0x00, 0x6a, 0x00, 0x6a, 0x02, 0x6a, 0x00]
            || credits[8] != 0x6a
            || credits[10] != 0x8d
            || credits[16..18] != [0x50, 0x56]
            || relative_call(CREDITS_LAUNCH.0 + 18, &credits[18..23])? != SETTER
        {
            return Err("unsupported Credits button launch".into());
        }
        let credits_flags = u32::from(credits[9]);

        let start = read(START_LAUNCH)?;
        if start[..9] != [0x6a, 0x00, 0x50, 0x51, 0x51, 0x6a, 0x02, 0x6a, 0x00]
            || start[9] != 0x68
            || start[14] != 0x8d
            || start[20..22] != [0x50, 0x56]
            || start[22..24] != [0x8d, 0x8e]
            || start[24..28] != 0x380u32.to_le_bytes()
            || relative_call(START_LAUNCH.0 + 28, &start[28..33])? != SETTER
        {
            return Err("unsupported single-player start launch".into());
        }
        let start_flags = u32::from_le_bytes(start[10..14].try_into().map_err(|_| "flags")?);

        Ok(Self {
            origin,
            image_sha256,
            stored_mask,
            cleared,
            credits_flags,
            start_flags,
        })
    }

    /// The value `GetGameTypeSelected` returns: the request flags the host stores, with
    /// the cleared bits removed.
    pub fn selected(&self, route: LaunchRoute) -> i32 {
        let flags = match route {
            LaunchRoute::CreditsButton => self.credits_flags,
            LaunchRoute::SinglePlayerStart => self.start_flags,
        };
        (flags & self.stored_mask & !self.cleared) as i32
    }
}

/// Target of the `call rel32` at `va`.
fn relative_call(va: u32, window: &[u8]) -> Result<u32, String> {
    match window {
        [0xe8, a, b, c, d] => Ok(va
            .wrapping_add(5)
            .wrapping_add_signed(i32::from_le_bytes([*a, *b, *c, *d]))),
        _ => Err("expected a relative call".into()),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn call(va: u32, target: u32) -> [u8; 5] {
        let mut bytes = [0xe8, 0, 0, 0, 0];
        bytes[1..].copy_from_slice(&target.wrapping_sub(va + 5).to_le_bytes());
        bytes
    }

    fn windows(credits: u8, start: u32, mask: u32, clear: u8) -> BTreeMap<u32, Vec<u8>> {
        let mut native = vec![0xa1, 1, 2, 3, 4, 0x8b, 0x40, 0x30, 0x8b, 0x40, 0x34];
        native.extend_from_slice(&[0x83, 0xe0, clear, 0xc3]);
        let mut host = vec![0x8b, 0x83];
        host.extend_from_slice(&0x148u32.to_le_bytes());
        host.push(0x25);
        host.extend_from_slice(&mask.to_le_bytes());
        let mut credits_window = vec![0x6a, 0, 0x6a, 0, 0x6a, 2, 0x6a, 0, 0x6a, credits];
        credits_window.extend_from_slice(&[0x8d, 0x85, 0, 0, 0, 0, 0x50, 0x56]);
        credits_window.extend_from_slice(&call(0x56a1ab + 18, SETTER));
        let mut start_window = vec![0x6a, 0, 0x50, 0x51, 0x51, 0x6a, 2, 0x6a, 0, 0x68];
        start_window.extend_from_slice(&start.to_le_bytes());
        start_window.extend_from_slice(&[0x8d, 0x86, 0, 0, 0, 0, 0x50, 0x56, 0x8d, 0x8e]);
        start_window.extend_from_slice(&0x380u32.to_le_bytes());
        start_window.extend_from_slice(&call(0x58e0ab + 28, SETTER));
        BTreeMap::from([
            (NATIVE.0, native),
            (HOST_MASK.0, host),
            (SETTER_LOAD.0, vec![0x8b, 0x5d, 0x10]),
            (SETTER_STORE.0, vec![0x89, 0x5e, 0x0c]),
            (LISTENER_FLAGS.0, vec![0xff, 0x77, 0x0c]),
            (CREDITS_LAUNCH.0, credits_window),
            (START_LAUNCH.0, start_window),
        ])
    }

    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic:game-type-image".into(),
            edition: "synthetic".into(),
            version: "test".into(),
        }
    }

    fn decode_with(windows: &BTreeMap<u32, Vec<u8>>) -> Result<Rules, String> {
        Rules::decode(origin(), [0; 32], |(va, n)| {
            windows
                .get(&va)
                .filter(|v| v.len() == n)
                .cloned()
                .ok_or_else(|| "missing synthetic window".into())
        })
    }

    #[test]
    fn selected_value_is_the_stored_request_flags_without_the_cleared_bit() {
        let rules = decode_with(&windows(1, 0x100, 0x3ff, 0xf7)).unwrap();
        assert_eq!(rules.selected(LaunchRoute::CreditsButton), 1);
        assert_eq!(rules.selected(LaunchRoute::SinglePlayerStart), 0x100);
        // The mask, the cleared bit and both request flags come from the windows.
        let rules = decode_with(&windows(0xf, 0x30c, 0x0ff, 0xfb)).unwrap();
        assert_eq!(rules.selected(LaunchRoute::CreditsButton), 0xb);
        assert_eq!(rules.selected(LaunchRoute::SinglePlayerStart), 0x08);
    }

    #[test]
    fn credits_flag_bit_eight_is_cleared() {
        let rules = decode_with(&windows(9, 0x108, 0x3ff, 0xf7)).unwrap();
        assert_eq!(rules.selected(LaunchRoute::CreditsButton), 1);
        assert_eq!(rules.selected(LaunchRoute::SinglePlayerStart), 0x100);
    }

    #[test]
    fn every_window_is_required_and_its_opcodes_are_checked() {
        let good = windows(1, 0x100, 0x3ff, 0xf7);
        for &va in good.keys() {
            let mut missing = good.clone();
            missing.remove(&va);
            assert!(decode_with(&missing).is_err(), "{va:#x} absent");
            let mut changed = good.clone();
            changed.get_mut(&va).unwrap()[0] ^= 0x40;
            assert!(decode_with(&changed).is_err(), "{va:#x} opcode changed");
        }
    }

    #[test]
    fn a_launch_that_calls_another_function_refuses() {
        let mut data = windows(1, 0x100, 0x3ff, 0xf7);
        let window = data.get_mut(&CREDITS_LAUNCH.0).unwrap();
        window[18..23].copy_from_slice(&call(0x56a1ab + 18, SETTER + 0x10));
        assert!(decode_with(&data).is_err());
        let mut data = windows(1, 0x100, 0x3ff, 0xf7);
        let window = data.get_mut(&START_LAUNCH.0).unwrap();
        window[28..33].copy_from_slice(&call(0x58e0ab + 28, SETTER + 0x10));
        assert!(decode_with(&data).is_err());
    }

    #[test]
    fn a_wrong_image_identity_refuses() {
        assert!(Rules::reviewed_legacy(&[0; 64], origin()).is_err());
    }
}
