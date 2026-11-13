//! Selected-client creation input and caller-owned melee difficulty snapshot.
#[path = "campaign_melee_checkpoint.rs"]
mod checkpoint;
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: [u8; 32],
    levels: [i32; 3],
    flags: [u32; 2],
    initial_flags: u32,
    easy_result: i32,
    base_result: i32,
}

#[derive(Clone, Debug)]
pub struct Input {
    pub image_sha256: [u8; 32],
    results: Vec<(i32, i32)>,
}

fn shape(bytes: &[u8], at: usize, expected: &[u8]) -> Result<(), String> {
    if bytes.get(at..at + expected.len()) == Some(expected) {
        Ok(())
    } else {
        Err(format!("MeleeDifficulty instruction relationship at {at}"))
    }
}
fn word(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes
        .get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or("MeleeDifficulty truncated operand".into())
}
fn call(bytes: &[u8], base: u32, at: usize) -> Result<u32, String> {
    shape(bytes, at, &[0xe8])?;
    Ok(base
        .wrapping_add(at as u32 + 5)
        .wrapping_add(word(bytes, at + 1)?))
}
impl Rules {
    pub fn read(image: &[u8]) -> Result<Self, String> {
        if image.len() > (32 << 20) || sha256(image) != REVIEWED_IMAGE {
            return Err("MeleeDifficulty selected image differs".into());
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 4096,
                sections: 96,
            },
        )
        .map_err(|e| e.to_string())?;
        Self::decode(REVIEWED_IMAGE, 4096, |va, n, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or("MeleeDifficulty truncated window".into())
        })
    }
    fn decode(
        image_sha256: [u8; 32],
        mut budget: usize,
        mut source: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut read = |va, n, code| -> Result<Vec<u8>, String> {
            budget = budget
                .checked_sub(n)
                .ok_or("MeleeDifficulty source budget")?;
            let bytes = source(va, n, code)?;
            if bytes.len() != n {
                return Err("MeleeDifficulty truncated window".into());
            }
            Ok(bytes)
        };
        let reg = read(0xab69e4, 20, true)?;
        for at in [0, 5, 10] {
            shape(&reg, at, &[0x68])?;
        }
        if call(&reg, 0xab69e4, 15)? != 0x8ccc80
            || read(word(&reg, 1)?, 4, false)? != b"()I\0"
            || read(word(&reg, 6)?, 16, false)? != b"MeleeDifficulty\0"
        {
            return Err("MeleeDifficulty registration differs".into());
        }
        let handler = word(&reg, 11)?;
        let q = read(handler, 113, true)?;
        for (at, bytes) in [
            (0, &[0x8b, 0x0d][..]),
            (6, &[0x85, 0xc9, 0x75, 0x0d, 0x6a, 0x57]),
            (17, &[0x83, 0xc4, 4, 0x33, 0xc0, 0xc3, 0x8b, 0x41]),
            (26, &[0x23, 0x41]),
            (
                29,
                &[
                    0x83, 0xf8, 0xff, 0x75, 4, 0x33, 0xc0, 0xeb, 0x0e, 0x8d, 0x49,
                ],
            ),
            (46, &[0x8b, 0x0d]),
            (52, &[0xf7, 0x80]),
            (62, &[0x74, 6, 0xb8]),
            (69, &[0xc3, 0x8b, 0x41]),
            (73, &[0x83, 0xc1]),
            (
                76,
                &[0x23, 1, 0x83, 0xf8, 0xff, 0x75, 4, 0x33, 0xc0, 0xeb, 5],
            ),
            (92, &[0x8b, 0x80]),
            (98, &[0x25]),
            (103, &[0xf7, 0xd8, 0x1b, 0xc0, 0xf7, 0xd8, 0x83, 0xc0]),
            (112, &[0xc3]),
        ] {
            shape(&q, at, bytes)?;
        }
        let owner = read(0xab49c0, 5, true)?;
        let slot = read(0xab49d6, 3, true)?;
        shape(&owner, 0, &[0xa1])?;
        shape(&slot, 0, &[0x8d, 0x48])?;
        if word(&owner, 1)? != word(&q, 2)?
            || word(&q, 48)? != word(&q, 2)?
            || q[25] != q[28].wrapping_add(4)
            || q[40] != q[28]
            || q[72] != q[25]
            || q[75] != q[28]
            || slot[2] != q[28]
            || call(&q, handler, 41)? != 0x7fc7e0
            || call(&q, handler, 87)? != 0x7fc7e0
        {
            return Err("MeleeDifficulty owned AI context differs".into());
        }
        let writer = read(0xabd3ed, 35, true)?;
        for (at, bytes) in [
            (0, &[0x8b, 0x45, 0x0c, 0x83, 0xe8][..]),
            (6, &[0x74, 0x11, 0x83, 0xe8]),
            (11, &[0x75, 0x16, 0x81, 0x8e]),
            (23, &[0xeb, 0x0a, 0x81, 0x8e]),
        ] {
            shape(&writer, at, bytes)?;
        }
        let field = word(&q, 54)?;
        let flags = [word(&writer, 31)?, word(&writer, 19)?];
        if word(&q, 94)? != field
            || word(&writer, 15)? != field
            || word(&writer, 27)? != field
            || word(&q, 58)? != flags[0]
            || word(&q, 99)? != flags[0] | flags[1]
            || flags.iter().any(|v| v.count_ones() != 1)
            || flags[0] & flags[1] != 0
        {
            return Err("MeleeDifficulty flag joins differ".into());
        }
        let initial = read(0xab98df, 10, true)?;
        shape(&initial, 0, &[0xc7, 0x87])?;
        let initial_flags = word(&initial, 6)?;
        if word(&initial, 2)? != field || initial_flags & (flags[0] | flags[1]) != 0 {
            return Err("MeleeDifficulty initial flags differ".into());
        }
        let vtable = read(0xab950c, 6, true)?;
        shape(&vtable, 0, &[0xc7, 7])?;
        let class_slot = read(
            word(&vtable, 2)?
                .checked_add(0x1c)
                .ok_or("AI class slot overflow")?,
            4,
            false,
        )?;
        let class = read(word(&class_slot, 0)?, 6, true)?;
        let allocated_class = read(0xabcc74, 5, true)?;
        shape(&class, 0, &[0xb8])?;
        shape(&class, 5, &[0xc3])?;
        shape(&allocated_class, 0, &[0x68])?;
        if word(&class, 1)? != word(&allocated_class, 1)? {
            return Err("MeleeDifficulty constructed AI class differs".into());
        }
        let wrapper = read(0x4c0ed0, 22, true)?;
        shape(
            &wrapper,
            0,
            &[
                0x55, 0x8b, 0xec, 0x56, 0xff, 0x75, 8, 0x8b, 0xf1, 0x0f, 0xb6, 0x46,
            ],
        )?;
        shape(&wrapper, 13, &[0x50])?;
        shape(&wrapper, 19, &[0x83, 0xc4, 8])?;
        if call(&wrapper, 0x4c0ed0, 14)? != 0xabd3a0 {
            return Err("AI creation target differs".into());
        }
        let startup = read(0x47e546, 11, true)?;
        shape(&startup, 0, &[0xff, 0xb1])?;
        if call(&startup, 0x47e546, 6)? != 0x4c0ed0 {
            return Err("AI startup constructor differs".into());
        }
        let getter = read(0x490ab2, 6, true)?;
        shape(&getter, 0, &[0x8b, 0x80])?;
        let reset = read(0x4bdf8c, 10, true)?;
        shape(&reset, 0, &[0xc7, 0x86])?;
        let lobby = read(0x51dfd8, 10, true)?;
        shape(&lobby, 0, &[0x0f, 0xb6, 0x46])?;
        shape(&lobby, 4, &[0x89, 0x82])?;
        let fresh = read(0x51e693, 4, true)?;
        shape(&fresh, 0, &[0xc6, 0x46])?;
        if word(&startup, 2)? != word(&getter, 2)?
            || word(&getter, 2)? != word(&reset, 2)?
            || word(&lobby, 6)? != word(&getter, 2)?
            || lobby[3] != fresh[2]
            || word(&reset, 6)? != u32::from(fresh[3])
        {
            return Err("MeleeDifficulty player/lobby difficulty source differs".into());
        }
        let easy = i32::from(writer[5] as i8);
        let hard = easy.wrapping_add(i32::from(writer[10] as i8));
        let normal = word(&reset, 6)? as i32;
        if easy == hard || normal == easy || normal == hard {
            return Err("MeleeDifficulty input levels overlap".into());
        }
        Ok(Self {
            image_sha256,
            levels: [easy, normal, hard],
            flags,
            initial_flags,
            easy_result: word(&q, 65)? as i32,
            base_result: i32::from(q[111] as i8),
        })
    }
    fn result(&self, level: i32) -> Result<i32, String> {
        let at = self
            .levels
            .iter()
            .position(|&v| v == level)
            .ok_or_else(|| format!("MeleeDifficulty AI input {level} is not qualified"))?;
        let flags = self.initial_flags
            | match at {
                0 => self.flags[0],
                2 => self.flags[1],
                _ => 0,
            };
        Ok(if flags & self.flags[0] != 0 {
            self.easy_result
        } else {
            self.base_result + i32::from(flags & (self.flags[0] | self.flags[1]) != 0)
        })
    }
    /// Fresh single-player launch, with no external lobby difficulty overrides.
    pub fn fresh_input(self, players: impl IntoIterator<Item = i32>) -> Result<Input, String> {
        let mut entries = Vec::new();
        for player in players {
            if entries.len() == super::MAX_PLAYERS {
                return Err("MeleeDifficulty input capacity".into());
            }
            entries
                .try_reserve(1)
                .map_err(|_| "MeleeDifficulty input allocation")?;
            entries.push((player, self.levels[1]));
        }
        Input::new(self, &entries)
    }
}
impl Input {
    /// Explicit pre-start per-player AI-difficulty inputs, never game difficulty.
    pub fn new(rules: Rules, entries: &[(i32, i32)]) -> Result<Self, String> {
        if entries.len() > super::MAX_PLAYERS {
            return Err("MeleeDifficulty input capacity".into());
        }
        let mut results = Vec::new();
        results
            .try_reserve_exact(entries.len())
            .map_err(|_| "MeleeDifficulty input allocation")?;
        for &(player, level) in entries {
            if player < 0 || results.iter().any(|&(p, _)| p == player) {
                return Err("MeleeDifficulty duplicate or negative player input".into());
            }
            results.push((player, rules.result(level)?));
        }
        Ok(Self {
            image_sha256: rules.image_sha256,
            results,
        })
    }
    pub(crate) fn for_player(&self, player: i32) -> Result<i32, String> {
        self.results
            .iter()
            .find(|&&(p, _)| p == player)
            .map(|&(_, v)| v)
            .ok_or_else(|| format!("MeleeDifficulty input missing for AI player {player}"))
    }
}

#[cfg(test)]
impl Rules {
    pub(crate) fn fixture() -> Self {
        tests::rules()
    }
}
#[cfg(test)]
#[path = "campaign_melee_difficulty_policy_tests.rs"]
mod tests;
