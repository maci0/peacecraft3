//! Targeting operands from the reviewed client image. This is a source reader,
//! not an implementation of the client's complete acquisition scorer.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct TargetingPolicy {
    pub image_sha256: Sha256Digest,
    pub target_heroes_default: bool,
    pub context_target_heroes_mask: u32,
    pub attached_target_heroes_mask: u8,
    pub hero_first: u8,
    pub hero_last: u8,
    pub hero_exclusion_mask: u32,
    pub creation_exclusion_flag: u32,
    pub hero_priority_mask: u32,
    pub hero_min_game_difficulty: u8,
    pub highest_priority_mask: u32,
    pub highest_ability_rawcode: [u8; 4],
    pub highest_counter_offset: u32,
    pub highest_counter_limit: i8,
    pub game_difficulty_global: u32,
    pub game_difficulty_child_offset: u8,
    pub game_difficulty_byte_offset: u8,
    pub promotion_from: u8,
    pub promotion_to: u8,
    pub optional_damage_mask: u32,
    pub optional_damage_threshold: f32,
    pub optional_damage_gate: u8,
    pub optional_class_mask: u32,
    pub optional_unclassified_mask: u32,
    pub unclassified_rawcodes: [[u8; 4]; 3],
    /// Peon channel priority (legacy field name; not a hero-level test).
    pub optional_level_mask: u32,
    pub optional_level_gate: u8,
    pub peon_channel: Option<PeonChannelBand>,
}

#[derive(Clone, Debug)]
pub struct PeonChannelBand {
    pub unit_type: i32,
    pub category: u8,
}

impl PeonChannelBand {
    fn decode(
        level: &[u8],
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, Error>,
    ) -> Result<Self, Error> {
        at(level, 24, &[0xf6, 0x80])?;
        at(level, 33, &[0x83, 0xb8])?;
        at(level, 40, &[0x75, 0x0b])?;
        if level[30].count_ones() != 1 {
            return Err(Error("peon class mask is not one bit"));
        }
        let initial = read(0x675a48, 10, true)?;
        at(&initial, 0, &[0xc7, 0x86])?;
        if word(&initial, 2)? != word(level, 35)? || word(&initial, 6)? == u32::from(level[39]) {
            return Err(Error("channel and initial category are not distinct"));
        }
        let classes = read(0x48a660, 44, true)?;
        at(&classes, 16, &[0x39, 0x0c, 0xc5])?;
        at(&classes, 26, &[0x83, 0xf8])?;
        at(&classes, 35, &[0x8b, 0x04, 0xc5])?;
        let table = word(&classes, 38)?;
        if word(&classes, 19)? != table.checked_add(4).ok_or(Error("class table overflow"))? {
            return Err(Error("class table pair differs"));
        }
        let rows = read(table, usize::from(classes[28]) * 8, false)?;
        let mut types = rows
            .as_chunks::<8>()
            .0
            .iter()
            .filter(|row| word(*row, 0).ok() == Some(u32::from(level[30])));
        let unit_type = word(types.next().ok_or(Error("peon class mask absent"))?, 4)? as i32;
        if types.next().is_some() {
            return Err(Error("ambiguous peon class mask"));
        }
        let unit_type_consumer = read(0x49a35d, 6, true)?;
        at(&unit_type_consumer, 0, &[0x23, 0x86])?;
        if word(&unit_type_consumer, 2)? != word(level, 26)? {
            return Err(Error("peon classification field differs"));
        }
        let mut activation = None;
        // Reviewed class/vtable roles; helper and name addresses are read, not copied.
        for (table, name) in [
            (0xf18c98_u32, b"CAbilitySpell\0".as_slice()),
            (0xf4ad58, b"CAbilityRepair\0".as_slice()),
        ] {
            let vtable = read(table, 0x384, false)?;
            let getter = read(word(&vtable, 0x58)?, 6, true)?;
            at(&getter, 0, &[0xb8])?;
            at(&getter, 5, &[0xc3])?;
            if read(word(&getter, 1)?, name.len(), false)? != name {
                return Err(Error("channel ability class differs"));
            }
            let method = word(&vtable, 0x380)?;
            if activation
                .replace(method)
                .is_some_and(|previous| previous != method)
            {
                return Err(Error("repair channel activation differs"));
            }
        }
        let activation = activation.ok_or(Error("missing channel activation"))?;
        let body = read(activation, 304, true)?;
        at(&body, 295, &[0x6a])?;
        at(&body, 297, &[0x8b, 0xcf])?;
        let category = body[296];
        if category != level[39] || category & 0x80 != 0 {
            return Err(Error("peon casting category differs"));
        }
        let setter = call(&body, activation, 299)?;
        let message = read(setter, 113, true)?;
        at(&message, 78, &[0x8b, 0x45, 0x08])?;
        at(&message, 97, &[0xc7, 0x46, 0x30])?;
        at(&message, 105, &[0x89, 0x46, 0x34])?;
        let message_id = word(&message, 100)?;
        let dispatch = read(0x672cb5, 30, true)?;
        at(&dispatch, 0, &[0x2d])?;
        at(&dispatch, 5, &[0x3d])?;
        at(&dispatch, 16, &[0x0f, 0xb6, 0x80])?;
        at(&dispatch, 23, &[0xff, 0x24, 0x85])?;
        let index = message_id
            .checked_sub(word(&dispatch, 1)?)
            .ok_or(Error("channel message range"))?;
        if index > word(&dispatch, 6)? {
            return Err(Error("channel message range"));
        }
        let byte = read(
            word(&dispatch, 19)?
                .checked_add(index)
                .ok_or(Error("channel dispatch overflow"))?,
            1,
            false,
        )?[0];
        let target = read(
            word(&dispatch, 26)?
                .checked_add(u32::from(byte) * 4)
                .ok_or(Error("channel dispatch overflow"))?,
            4,
            false,
        )?;
        let consumer = read(word(&target, 0)?, 12, true)?;
        at(
            &consumer,
            0,
            &[0x8b, 0x41, 0x0c, 0x8b, 0x40, 0x34, 0x89, 0x86],
        )?;
        if word(&consumer, 8)? != word(level, 35)? {
            return Err(Error("channel category consumer differs"));
        }
        Ok(Self {
            unit_type,
            category,
        })
    }
}

#[derive(Debug, thiserror::Error)]
#[error("campaign targeting policy: {0}")]
pub struct Error(&'static str);

// VAs and lengths identify instruction windows in the reviewed PE. Every game
// rule returned below is decoded from an operand within those windows.
const WINDOWS: [(u32, usize, bool); 20] = [
    (0xabf9e2, 10, true),  // context constructor
    (0xac36ed, 22, true),  // context setter and clear
    (0xac372d, 4, true),   // attached-unit setter
    (0xac3749, 4, true),   // attached-unit clear
    (0x499030, 35, true),  // candidate hero predicate
    (0x687eb9, 18, true),  // unit creation exclusion flag
    (0xb24042, 5, true),   // scorer game-settings global
    (0xb24096, 50, true),  // game-difficulty scorer read/promotion
    (0x4927a0, 30, true),  // GetGameDifficulty
    (0x4a8730, 19, true),  // SetGameDifficulty
    (0xb2457b, 4, true),   // hero band gate
    (0xb245c3, 84, true),  // hero predicate/getter and score OR
    (0xb243cd, 45, true),  // highest band and ability probe
    (0xb244ae, 71, true),  // optional damage/class bands
    (0x10db0bc, 4, false), // optional damage threshold
    (0xb24677, 31, true),  // optional classification band
    (0xb24450, 53, true),  // optional level band
    (0x69f655, 7, true),   // attached policy getter, first resolution path
    (0x69f664, 7, true),   // attached policy getter, second resolution path
    (0xb24704, 24, true),  // three unclassified rawcode comparisons
];

fn at(bytes: &[u8], offset: usize, expected: &[u8]) -> Result<(), Error> {
    (bytes.get(offset..offset + expected.len()) == Some(expected))
        .then_some(())
        .ok_or(Error("reviewed instruction relationship changed"))
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(Error("truncated instruction operand"))?
            .try_into()
            .map_err(|_| Error("truncated instruction operand"))?,
    ))
}

fn call(bytes: &[u8], va: u32, offset: usize) -> Result<u32, Error> {
    at(bytes, offset, &[0xe8])?;
    Ok(va
        .wrapping_add(offset as u32 + 5)
        .wrapping_add(word(bytes, offset + 1)?))
}

fn budgeted_windows(budget: RuleInputBudget) -> Result<(), Error> {
    let mut total = 0usize;
    for (_, len, _) in WINDOWS {
        total = total
            .checked_add(len)
            .ok_or(Error("window budget exceeded"))?;
        if total > budget.profile_bytes {
            return Err(Error("window budget exceeded"));
        }
    }
    Ok(())
}

impl TargetingPolicy {
    pub fn read(image: &[u8], budget: RuleInputBudget) -> Result<Self, Error> {
        if image.len() > budget.image_bytes {
            return Err(Error("image budget exceeded"));
        }
        budgeted_windows(budget)?;
        if sha256(image) != REVIEWED_IMAGE {
            return Err(Error("unsupported client image"));
        }
        let pe = Pe::parse(image, budget).map_err(|_| Error("invalid selected PE"))?;
        let mut windows = Vec::with_capacity(WINDOWS.len());
        for (va, len, code) in WINDOWS {
            windows.push(
                pe.read_from(va, code)
                    .map_err(|_| Error("missing selected instruction window"))?
                    .get(..len)
                    .ok_or(Error("short selected instruction window"))?,
            );
        }
        let mut policy = Self::decode(&windows, REVIEWED_IMAGE)?;
        let mut remaining =
            budget.profile_bytes - WINDOWS.iter().map(|(_, len, _)| len).sum::<usize>();
        policy.peon_channel = Some(PeonChannelBand::decode(
            windows[16],
            |address, length, code| {
                remaining = remaining
                    .checked_sub(length)
                    .ok_or(Error("channel window budget exceeded"))?;
                pe.read_from(address, code)
                    .map_err(|_| Error("channel source mapping"))?
                    .get(..length)
                    .map(<[u8]>::to_vec)
                    .ok_or(Error("truncated channel window"))
            },
        )?);
        Ok(policy)
    }

    fn decode(w: &[&[u8]], image_sha256: Sha256Digest) -> Result<Self, Error> {
        let [
            constructor,
            setter,
            node,
            node_clear,
            hero,
            creation,
            score_global,
            difficulty,
            getter,
            game_setter,
            hero_gate,
            hero_score,
            highest,
            optional,
            threshold,
            classification,
            level,
            getter_a,
            getter_b,
            unclassified,
        ] = w
        else {
            return Err(Error("incomplete targeting windows"));
        };
        at(constructor, 0, &[0xc7, 0x86])?;
        at(setter, 0, &[0x81, 0x89])?;
        at(setter, 10, &[0xeb, 0x0a, 0x81, 0xa1])?;
        let context_offset = word(setter, 2)?;
        if word(constructor, 2)? != context_offset
            || word(constructor, 6)? != 0
            || word(setter, 14)? != context_offset
        {
            return Err(Error("context flag initializer/setter differ"));
        }
        let context_mask = word(setter, 6)?;
        if word(setter, 18)? != !context_mask || context_mask.count_ones() != 1 {
            return Err(Error("context flag clear differs"));
        }
        at(node, 0, &[0x83, 0x48])?;
        at(node_clear, 0, &[0x83, 0x60])?;
        if node[2] != node_clear[2]
            || node_clear[3] != !node[3]
            || node[3].count_ones() != 1
            || node[3] & 0x80 != 0
        {
            return Err(Error("attached-unit flag differs"));
        }
        for getter in [getter_a, getter_b] {
            at(getter, 0, &[0x8b, 0x40])?;
            at(getter, 3, &[0x83, 0xe0])?;
            if getter[2] != node[2] || getter[5] != node[3] || getter[6] != 0xc3 {
                return Err(Error("attached policy getter differs from setter"));
            }
        }

        at(hero, 0, &[0x0f, 0xb6, 0x41])?;
        at(hero, 4, &[0x83, 0xf8])?;
        at(hero, 7, &[0x72, 0x17, 0x83, 0xf8])?;
        at(hero, 12, &[0x77, 0x12, 0xf7, 0x81])?;
        at(
            hero,
            24,
            &[0x75, 0x06, 0xb8, 0x01, 0, 0, 0, 0xc3, 0x33, 0xc0, 0xc3],
        )?;
        at(creation, 0, &[0xf7, 0xc3])?;
        at(creation, 6, &[0x74, 0x0a, 0x81, 0x8e])?;
        if word(hero, 16)? != word(creation, 10)? {
            return Err(Error("hero exclusion field differs from creation"));
        }
        let exclusion_mask = word(hero, 20)?;
        if exclusion_mask != word(creation, 14)? || exclusion_mask.count_ones() != 1 {
            return Err(Error("hero exclusion mask differs from creation"));
        }
        let first = hero[6];
        let last = hero[11];
        if first > last || first & 0x80 != 0 || last & 0x80 != 0 {
            return Err(Error("hero rawcode range is not a positive imm8"));
        }

        at(difficulty, 0, &[0x8b, 0x40])?;
        at(difficulty, 3, &[0x8a, 0x40])?;
        at(
            difficulty,
            6,
            &[0x0f, 0xb6, 0xc8, 0x89, 0x4d, 0x10, 0x83, 0xf9],
        )?;
        at(difficulty, 15, &[0x75, 0x21])?;
        at(
            difficulty,
            17,
            &[0x83, 0x7d, 0xd4, 0x00, 0x75, 0x1b, 0x8b, 0x4d, 0xf0],
        )?;
        if call(difficulty, 0xb24096, 26)? != 0x67e910 {
            return Err(Error("difficulty promotion helper changed"));
        }
        at(difficulty, 34, &[0xba])?;
        at(difficulty, 39, &[0x85, 0xc0, 0x0f, 0x44, 0xca])?;
        at(getter, 0, &[0xa1])?;
        at(score_global, 0, &[0xa1])?;
        at(getter, 22, &[0x8b, 0x40])?;
        at(getter, 25, &[0x0f, 0xb6, 0x40])?;
        at(game_setter, 3, &[0xa1])?;
        at(game_setter, 8, &[0x8b, 0x48])?;
        at(game_setter, 14, &[0x88, 0x41])?;
        let global = word(getter, 1)?;
        if word(score_global, 1)? != global
            || word(game_setter, 4)? != global
            || getter[24] != difficulty[2]
            || getter[28] != difficulty[5]
            || game_setter[10] != difficulty[2]
            || game_setter[16] != difficulty[5]
        {
            return Err(Error("scorer difficulty is not the game setting"));
        }
        let promotion_from = difficulty[14];
        let promotion_to = word(difficulty, 35)?;
        if promotion_from & 0x80 != 0
            || promotion_to > u32::from(u8::MAX)
            || promotion_to <= u32::from(promotion_from)
        {
            return Err(Error("difficulty promotion is invalid"));
        }

        at(hero_gate, 0, &[0x83, 0x7d, 0x10])?;
        if hero_gate[3] & 0x80 != 0 {
            return Err(Error("hero difficulty gate is a negative imm8"));
        }
        at(hero_score, 0, &[0x8b, 0x4d, 0x08, 0xe8])?;
        if call(hero_score, 0xb245c3, 3)? != 0x499030 || call(hero_score, 0xb245c3, 15)? != 0x69f620
        {
            return Err(Error("hero predicate or attached policy getter changed"));
        }
        at(hero_score, 76, &[0x81, 0xcb])?;
        let hero_priority_mask = word(hero_score, 78)?;
        if hero_priority_mask == 0 {
            return Err(Error("empty hero priority mask"));
        }

        at(highest, 0, &[0x8b, 0x45, 0x08, 0x83, 0xb8])?;
        at(highest, 10, &[0x7f, 0x21])?;
        at(highest, 20, &[0x68])?;
        if call(highest, 0xb243cd, 27)? != 0x6d6570 {
            return Err(Error("highest-band ability lookup changed"));
        }
        at(highest, 39, &[0x81, 0xcb])?;
        let highest_mask = word(highest, 41)?;
        if highest_mask.count_ones() != 1 || highest_mask <= hero_priority_mask {
            return Err(Error("highest priority band changed"));
        }

        at(optional, 0, &[0x83, 0x7d, 0x10])?;
        if optional[3] & 0x80 != 0 {
            return Err(Error("optional difficulty gate is a negative imm8"));
        }
        at(optional, 26, &[0x0f, 0x2f, 0x05])?;
        at(optional, 35, &[0x81, 0xcb])?;
        at(optional, 44, &[0xf6, 0x80])?;
        at(optional, 53, &[0xf7, 0x80])?;
        at(optional, 65, &[0x81, 0xcb])?;
        if word(optional, 29)? != 0x10db0bc {
            return Err(Error("optional threshold pointer changed"));
        }
        let damage_threshold = f32::from_bits(word(threshold, 0)?);
        if !damage_threshold.is_finite() || damage_threshold < 0.0 {
            return Err(Error("optional damage threshold invalid"));
        }
        if word(optional, 59)? != exclusion_mask {
            return Err(Error("optional exclusion mask differs"));
        }
        at(classification, 0, &[0x8b, 0x4d, 0x08])?;
        if call(classification, 0xb24677, 3)? != 0x6969c0 {
            return Err(Error("classification helper changed"));
        }
        at(classification, 12, &[0xb8])?;
        at(level, 0, &[0x83, 0x7d, 0x10])?;
        if level[3] & 0x80 != 0 {
            return Err(Error("level difficulty gate is a negative imm8"));
        }
        at(level, 42, &[0x81, 0xcb])?;
        let optional_masks = [
            word(optional, 37)?,
            word(optional, 67)?,
            word(classification, 13)?,
            word(level, 44)?,
        ];
        if optional_masks.iter().any(|mask| mask.count_ones() != 1)
            || optional_masks
                .iter()
                .enumerate()
                .any(|(index, mask)| optional_masks[index + 1..].contains(mask))
        {
            return Err(Error("optional score bands overlap or are empty"));
        }
        let mut unclassified_rawcodes = [[0; 4]; 3];
        for (index, rawcode) in unclassified_rawcodes.iter_mut().enumerate() {
            let offset = index * 8;
            at(unclassified, offset, &[0x81, 0xfe])?;
            at(unclassified, offset + 6, &[0x74])?;
            if (offset + 8) as isize + isize::from(unclassified[offset + 7] as i8) != 0x23 {
                return Err(Error("unclassified comparison branch changed"));
            }
            *rawcode = word(unclassified, offset + 2)?.to_be_bytes();
        }
        if unclassified_rawcodes
            .iter()
            .any(|code| !code.iter().all(u8::is_ascii_graphic))
            || unclassified_rawcodes.windows(2).any(|pair| {
                pair[0][..3] != pair[1][..3] || pair[0][3].checked_add(1) != Some(pair[1][3])
            })
        {
            return Err(Error("unclassified rawcode family changed"));
        }
        Ok(Self {
            image_sha256,
            target_heroes_default: word(constructor, 6)? & context_mask != 0,
            context_target_heroes_mask: context_mask,
            attached_target_heroes_mask: node[3],
            hero_first: first,
            hero_last: last,
            hero_exclusion_mask: exclusion_mask,
            creation_exclusion_flag: word(creation, 2)?,
            hero_priority_mask,
            hero_min_game_difficulty: hero_gate[3],
            highest_priority_mask: highest_mask,
            highest_ability_rawcode: word(highest, 21)?.to_be_bytes(),
            highest_counter_offset: word(highest, 5)?,
            highest_counter_limit: highest[9] as i8,
            game_difficulty_global: global,
            game_difficulty_child_offset: difficulty[2],
            game_difficulty_byte_offset: difficulty[5],
            promotion_from,
            promotion_to: promotion_to as u8,
            optional_damage_mask: optional_masks[0],
            optional_damage_threshold: damage_threshold,
            optional_damage_gate: optional[3],
            optional_class_mask: optional_masks[1],
            optional_unclassified_mask: optional_masks[2],
            unclassified_rawcodes,
            optional_level_mask: optional_masks[3],
            optional_level_gate: level[3],
            peon_channel: None,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let source = synthetic_windows();
        let mut policy = Self::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .expect("synthetic targeting instructions");
        let data = synthetic_channel_source(&source[16]);
        policy.peon_channel = Some(
            PeonChannelBand::decode(&source[16], |address, length, _| {
                data.get(&address)
                    .and_then(|b| b.get(..length))
                    .map(<[u8]>::to_vec)
                    .ok_or(Error("missing synthetic channel role"))
            })
            .expect("synthetic channel operands"),
        );
        policy
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
fn synthetic_windows() -> Vec<Vec<u8>> {
    let mut w = WINDOWS
        .iter()
        .map(|(_, len, _)| vec![0; *len])
        .collect::<Vec<_>>();
    fn put(w: &mut [Vec<u8>], i: usize, at: usize, data: &[u8]) {
        w[i][at..at + data.len()].copy_from_slice(data);
    }
    fn dword(w: &mut [Vec<u8>], i: usize, at: usize, data: u32) {
        put(w, i, at, &data.to_le_bytes());
    }
    fn call(w: &mut [Vec<u8>], i: usize, at: usize, target: u32) {
        put(w, i, at, &[0xe8]);
        dword(
            w,
            i,
            at + 1,
            target.wrapping_sub(WINDOWS[i].0 + at as u32 + 5),
        );
    }
    put(&mut w, 0, 0, &[0xc7, 0x86]);
    dword(&mut w, 0, 2, 0x330);
    put(&mut w, 1, 0, &[0x81, 0x89]);
    dword(&mut w, 1, 2, 0x330);
    dword(&mut w, 1, 6, 0x80);
    put(&mut w, 1, 10, &[0xeb, 0x0a, 0x81, 0xa1]);
    dword(&mut w, 1, 14, 0x330);
    dword(&mut w, 1, 18, !0x80);
    put(&mut w, 2, 0, &[0x83, 0x48, 0x28, 0x10]);
    put(&mut w, 3, 0, &[0x83, 0x60, 0x28, !0x10]);
    put(
        &mut w,
        4,
        0,
        &[
            0x0f, 0xb6, 0x41, 0x37, 0x83, 0xf8, b'J', 0x72, 0x17, 0x83, 0xf8, b'Q', 0x77, 0x12,
            0xf7, 0x81,
        ],
    );
    dword(&mut w, 4, 16, 0x164);
    dword(&mut w, 4, 20, 0x2000_0000);
    put(
        &mut w,
        4,
        24,
        &[0x75, 0x06, 0xb8, 0x01, 0, 0, 0, 0xc3, 0x33, 0xc0, 0xc3],
    );
    put(&mut w, 5, 0, &[0xf7, 0xc3]);
    dword(&mut w, 5, 2, 0x8000);
    put(&mut w, 5, 6, &[0x74, 0x0a, 0x81, 0x8e]);
    dword(&mut w, 5, 10, 0x164);
    dword(&mut w, 5, 14, 0x2000_0000);
    put(&mut w, 6, 0, &[0xa1]);
    dword(&mut w, 6, 1, 0x1234_5678);
    put(
        &mut w,
        7,
        0,
        &[
            0x8b, 0x40, 0x38, 0x8a, 0x40, 0x48, 0x0f, 0xb6, 0xc8, 0x89, 0x4d, 0x10, 0x83, 0xf9,
            0x02, 0x75, 0x21, 0x83, 0x7d, 0xd4, 0, 0x75, 0x1b, 0x8b, 0x4d, 0xf0,
        ],
    );
    call(&mut w, 7, 26, 0x67e910);
    put(&mut w, 7, 34, &[0xba]);
    dword(&mut w, 7, 35, 4);
    put(&mut w, 7, 39, &[0x85, 0xc0, 0x0f, 0x44, 0xca]);
    put(&mut w, 8, 0, &[0xa1]);
    dword(&mut w, 8, 1, 0x1234_5678);
    put(&mut w, 8, 22, &[0x8b, 0x40, 0x38, 0x0f, 0xb6, 0x40, 0x48]);
    put(&mut w, 9, 3, &[0xa1]);
    dword(&mut w, 9, 4, 0x1234_5678);
    put(&mut w, 9, 8, &[0x8b, 0x48, 0x38]);
    put(&mut w, 9, 14, &[0x88, 0x41, 0x48]);
    put(&mut w, 10, 0, &[0x83, 0x7d, 0x10, 0x02]);
    put(&mut w, 11, 0, &[0x8b, 0x4d, 0x08]);
    call(&mut w, 11, 3, 0x499030);
    call(&mut w, 11, 15, 0x69f620);
    put(&mut w, 11, 76, &[0x81, 0xcb]);
    dword(&mut w, 11, 78, 0x4200_0000);
    put(&mut w, 12, 0, &[0x8b, 0x45, 0x08, 0x83, 0xb8]);
    dword(&mut w, 12, 5, 0x220);
    put(&mut w, 12, 10, &[0x7f, 0x21]);
    put(&mut w, 12, 20, &[0x68]);
    put(&mut w, 12, 21, b"dcba");
    call(&mut w, 12, 27, 0x6d6570);
    put(&mut w, 12, 39, &[0x81, 0xcb]);
    dword(&mut w, 12, 41, 0x8000_0000);
    put(&mut w, 13, 0, &[0x83, 0x7d, 0x10, 0x04]);
    put(&mut w, 13, 26, &[0x0f, 0x2f, 0x05]);
    dword(&mut w, 13, 29, 0x10db0bc);
    put(&mut w, 13, 35, &[0x81, 0xcb]);
    dword(&mut w, 13, 37, 0x4000_0000);
    put(&mut w, 13, 44, &[0xf6, 0x80]);
    put(&mut w, 13, 53, &[0xf7, 0x80]);
    dword(&mut w, 13, 59, 0x2000_0000);
    put(&mut w, 13, 65, &[0x81, 0xcb]);
    dword(&mut w, 13, 67, 0x1000_0000);
    dword(&mut w, 14, 0, 0.625_f32.to_bits());
    put(&mut w, 15, 0, &[0x8b, 0x4d, 0x08]);
    call(&mut w, 15, 3, 0x6969c0);
    put(&mut w, 15, 12, &[0xb8]);
    dword(&mut w, 15, 13, 0x2000_0000);
    put(&mut w, 16, 0, &[0x83, 0x7d, 0x10, 0x02]);
    put(&mut w, 16, 24, &[0xf6, 0x80]);
    dword(&mut w, 16, 26, 0x654);
    put(&mut w, 16, 30, &[0x40]);
    put(&mut w, 16, 33, &[0x83, 0xb8]);
    dword(&mut w, 16, 35, 0x6a0);
    put(&mut w, 16, 39, &[7, 0x75, 0x0b]);
    put(&mut w, 16, 42, &[0x81, 0xcb]);
    dword(&mut w, 16, 44, 0x0008_0000);
    put(&mut w, 17, 0, &[0x8b, 0x40, 0x28, 0x83, 0xe0, 0x10, 0xc3]);
    put(&mut w, 18, 0, &[0x8b, 0x40, 0x28, 0x83, 0xe0, 0x10, 0xc3]);
    for (index, rawcode) in [*b"xyz4", *b"xyz5", *b"xyz6"].into_iter().enumerate() {
        let offset = index * 8;
        put(&mut w, 19, offset, &[0x81, 0xfe]);
        dword(&mut w, 19, offset + 2, u32::from_be_bytes(rawcode));
        put(&mut w, 19, offset + 6, &[0x74, (0x23 - offset - 8) as u8]);
    }
    w
}

#[cfg(any(test, feature = "test-fixtures"))]
fn synthetic_channel_source(level: &[u8]) -> std::collections::BTreeMap<u32, Vec<u8>> {
    use std::collections::BTreeMap;
    let mut data = BTreeMap::new();
    data.insert(
        0x675a48,
        [
            vec![0xc7, 0x86],
            level[35..39].to_vec(),
            2_u32.to_le_bytes().to_vec(),
        ]
        .concat(),
    );
    let mut classes = vec![0; 44];
    classes[16..19].copy_from_slice(&[0x39, 0x0c, 0xc5]);
    classes[19..23].copy_from_slice(&0x13004_u32.to_le_bytes());
    classes[26..29].copy_from_slice(&[0x83, 0xf8, 1]);
    classes[35..38].copy_from_slice(&[0x8b, 0x04, 0xc5]);
    classes[38..42].copy_from_slice(&0x13000_u32.to_le_bytes());
    data.insert(0x48a660, classes);
    data.insert(
        0x13000,
        [u32::from(level[30]).to_le_bytes(), 16_u32.to_le_bytes()].concat(),
    );
    data.insert(
        0x49a35d,
        [vec![0x23, 0x86], level[26..30].to_vec()].concat(),
    );
    for (table, getter, name_address, name) in [
        (
            0xf18c98,
            0x11000_u32,
            0x12000_u32,
            b"CAbilitySpell\0".as_slice(),
        ),
        (0xf4ad58, 0x11100, 0x12100, b"CAbilityRepair\0".as_slice()),
    ] {
        let mut bytes = vec![0; 0x384];
        bytes[0x58..0x5c].copy_from_slice(&getter.to_le_bytes());
        bytes[0x380..0x384].copy_from_slice(&0x14000_u32.to_le_bytes());
        data.insert(table, bytes);
        data.insert(
            getter,
            [vec![0xb8], name_address.to_le_bytes().to_vec(), vec![0xc3]].concat(),
        );
        data.insert(name_address, name.to_vec());
    }
    let mut activation = vec![0; 304];
    activation[295..300].copy_from_slice(&[0x6a, level[39], 0x8b, 0xcf, 0xe8]);
    activation[300..304].copy_from_slice(&0x15000_u32.wrapping_sub(0x14000 + 304).to_le_bytes());
    data.insert(0x14000, activation);
    let mut message = vec![0; 113];
    message[78..81].copy_from_slice(&[0x8b, 0x45, 8]);
    message[97..100].copy_from_slice(&[0xc7, 0x46, 0x30]);
    message[100..104].copy_from_slice(&0x20003_u32.to_le_bytes());
    message[105..108].copy_from_slice(&[0x89, 0x46, 0x34]);
    data.insert(0x15000, message);
    let mut dispatch = vec![0; 30];
    dispatch[0] = 0x2d;
    dispatch[1..5].copy_from_slice(&0x20000_u32.to_le_bytes());
    dispatch[5] = 0x3d;
    dispatch[6..10].copy_from_slice(&8_u32.to_le_bytes());
    dispatch[16..19].copy_from_slice(&[0x0f, 0xb6, 0x80]);
    dispatch[19..23].copy_from_slice(&0x18000_u32.to_le_bytes());
    dispatch[23..26].copy_from_slice(&[0xff, 0x24, 0x85]);
    dispatch[26..30].copy_from_slice(&0x19000_u32.to_le_bytes());
    data.insert(0x672cb5, dispatch);
    data.insert(0x18003, vec![2]);
    data.insert(0x19008, 0x1a000_u32.to_le_bytes().to_vec());
    data.insert(
        0x1a000,
        [
            vec![0x8b, 0x41, 0x0c, 0x8b, 0x40, 0x34, 0x89, 0x86],
            level[35..39].to_vec(),
        ]
        .concat(),
    );
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_budget_is_exact() {
        let total = WINDOWS.iter().map(|(_, len, _)| len).sum();
        let budget = RuleInputBudget {
            image_bytes: 1,
            profile_bytes: total,
            sections: 1,
        };
        assert!(budgeted_windows(budget).is_ok());
        assert!(
            budgeted_windows(RuleInputBudget {
                profile_bytes: total - 1,
                ..budget
            })
            .is_err()
        );
    }

    #[test]
    fn changed_source_image_refuses_before_parsing() {
        let budget = RuleInputBudget {
            image_bytes: 1024,
            profile_bytes: 1024,
            sections: 1,
        };
        assert!(TargetingPolicy::read(b"MZunsupported", budget).is_err());
    }

    #[test]
    fn synthetic_instruction_operands_drive_the_policy() {
        let policy = TargetingPolicy::fixture();
        assert_eq!((policy.hero_first, policy.hero_last), (b'J', b'Q'));
        assert_eq!(policy.context_target_heroes_mask, 0x80);
        assert_eq!(policy.hero_exclusion_mask, 0x2000_0000);
        assert_eq!(policy.creation_exclusion_flag, 0x8000);
        assert_eq!(policy.game_difficulty_global, 0x1234_5678);
        assert_eq!((policy.promotion_from, policy.promotion_to), (2, 4));
        assert_eq!(policy.highest_ability_rawcode, *b"abcd");
        assert_eq!(policy.optional_damage_threshold, 0.625);
        assert_eq!(policy.optional_unclassified_mask, 0x2000_0000);
        assert_eq!(policy.unclassified_rawcodes, [*b"xyz4", *b"xyz5", *b"xyz6"]);
        let mut source = synthetic_windows();
        for (index, rawcode) in [*b"Abc7", *b"Abc8", *b"Abc9"].into_iter().enumerate() {
            source[19][index * 8 + 2..index * 8 + 6]
                .copy_from_slice(&u32::from_be_bytes(rawcode).to_le_bytes());
        }
        assert_eq!(
            TargetingPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32]
            )
            .expect("changed family operands")
            .unclassified_rawcodes,
            [*b"Abc7", *b"Abc8", *b"Abc9"]
        );
        source[19][14] ^= 1;
        assert!(
            TargetingPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32]
            )
            .is_err()
        );
        source = synthetic_windows();
        source[19][7] ^= 1;
        assert!(
            TargetingPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32]
            )
            .is_err()
        );
        source = synthetic_windows();
        source[5][14] ^= 1;
        assert!(
            TargetingPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32]
            )
            .is_err()
        );
        source = synthetic_windows();
        source[9][16] ^= 1;
        assert!(
            TargetingPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32]
            )
            .is_err()
        );
        source = synthetic_windows();
        source[17][5] ^= 1;
        assert!(
            TargetingPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32]
            )
            .is_err()
        );
        source = synthetic_windows();
        source[14].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(
            TargetingPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32]
            )
            .is_err()
        );
    }

    #[test]
    fn sign_extended_imm8_operands_cannot_be_reported_as_unsigned_policy() {
        let refuses = |source: Vec<Vec<u8>>| {
            assert!(
                TargetingPolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32]
                )
                .is_err()
            );
        };
        for (window, offset) in [(4, 11), (10, 3), (13, 3), (16, 3)] {
            let mut source = synthetic_windows();
            source[window][offset] |= 0x80;
            refuses(source);
        }
        let mut source = synthetic_windows();
        source[4][6] = 0x80;
        source[4][11] = 0x90;
        refuses(source);

        let mut source = synthetic_windows();
        source[7][14] = 0x82;
        source[7][35..39].copy_from_slice(&131_u32.to_le_bytes());
        refuses(source);

        let mut source = synthetic_windows();
        source[2][3] = 0x80;
        source[3][3] = 0x7f;
        source[17][5] = 0x80;
        source[18][5] = 0x80;
        refuses(source);
    }

    #[test]
    fn channel_class_message_and_dispatch_follow_nonstock_relocated_operands() {
        let source = synthetic_windows();
        let level = &source[16];
        let mut data = synthetic_channel_source(level);
        let mut activation = data.remove(&0x14000).unwrap();
        let setter = data.remove(&0x15000).unwrap();
        activation[300..304]
            .copy_from_slice(&0x25000_u32.wrapping_sub(0x24000 + 304).to_le_bytes());
        data.insert(0x24000, activation);
        data.insert(0x25000, setter);
        for table in [0xf18c98, 0xf4ad58] {
            data.get_mut(&table).unwrap()[0x380..0x384].copy_from_slice(&0x24000_u32.to_le_bytes());
        }
        let decode = |data: &std::collections::BTreeMap<u32, Vec<u8>>| {
            PeonChannelBand::decode(level, |address, length, _| {
                data.get(&address)
                    .and_then(|b| b.get(..length))
                    .map(<[u8]>::to_vec)
                    .ok_or(Error("missing synthetic channel role"))
            })
        };
        let band = decode(&data).unwrap();
        assert_eq!((band.unit_type, band.category), (16, 7));
        let total: usize = data.values().map(Vec::len).sum();
        for (budget, expected) in [(total - 1, false), (total, true)] {
            let mut remaining = budget;
            let result = PeonChannelBand::decode(level, |address, length, _| {
                remaining = remaining
                    .checked_sub(length)
                    .ok_or(Error("synthetic channel budget"))?;
                data.get(&address)
                    .and_then(|b| b.get(..length))
                    .map(<[u8]>::to_vec)
                    .ok_or(Error("missing synthetic channel role"))
            });
            assert_eq!(result.is_ok(), expected);
        }
        for (address, offset) in [
            (0x24000, 296),
            (0x25000, 100),
            (0x1a000, 8),
            (0x13000, 0),
            (0x12100, 0),
        ] {
            let mut broken = data.clone();
            broken.get_mut(&address).unwrap()[offset] ^= 1;
            assert!(
                decode(&broken).is_err(),
                "channel operand {address:x}:{offset}"
            );
        }
        for (&address, bytes) in &data {
            let mut broken = data.clone();
            broken.get_mut(&address).unwrap().truncate(bytes.len() - 1);
            assert!(decode(&broken).is_err(), "truncated role {address:x}");
        }
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
    fn reviewed_original_image_decodes() {
        let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE required"))
            .expect("read reviewed client");
        let policy = TargetingPolicy::read(
            &image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .expect("selected instruction relationship");
        let fallback = crate::default_difficulty_image::native_default_fallback(&image).unwrap();
        println!(
            "selected targeting gates: fallback={fallback} hero={} damage={} peon={}",
            policy.hero_min_game_difficulty,
            policy.optional_damage_gate,
            policy.optional_level_gate
        );
        assert_eq!(policy.image_sha256, REVIEWED_IMAGE);
        assert!(policy.peon_channel.is_some());
        assert!(!policy.target_heroes_default);
        assert!(policy.hero_first <= policy.hero_last);
        assert!(policy.highest_priority_mask > policy.hero_priority_mask);
        assert!(policy.unclassified_rawcodes.windows(2).all(|pair| {
            pair[0][..3] == pair[1][..3] && pair[0][3].checked_add(1) == Some(pair[1][3])
        }));
    }
}
