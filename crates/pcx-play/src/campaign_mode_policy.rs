//! Selected campaign configuration, mode propagation and silent debug calls.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct CampaignModePolicy {
    pub image_sha256: Sha256Digest,
    pub context_mask: u32,
    pub unit_mask: u32,
    pub campaign_mode: bool,
    pub replacement_count_field: u32,
    /// The replacement limit an AI context holds until a script sets it: the immediate the
    /// context initializer stores in that field.
    pub default_replacement: i32,
    pub guard_posts: GuardPostRules,
}

#[derive(Clone, Copy, Debug)]
pub struct GuardPostRules {
    pub initial_count: i32,
    pub binding_increment: i32,
    pub released_count: i32,
    pub return_order: i32,
}

#[derive(Debug, thiserror::Error)]
#[error("campaign mode policy: {0}")]
pub struct Error(&'static str);

const WINDOWS: [(u32, usize, bool); 41] = [
    (0xab6887, 20, true),
    (0xab6e80, 60, true),
    (0xe70fac, 4, false),
    (0xf160a0, 14, false),
    (0xac2620, 35, true),
    (0xac2660, 45, true),
    (0xabb3ce, 25, true),
    (0xac39d9, 12, true),
    (0xab69bc, 20, true),
    (0xab7450, 67, true),
    (0xe6f4a0, 5, false),
    (0xf161a0, 20, false),
    (0xab605c, 20, true),
    (0xab48b0, 1, true),
    (0xe779d0, 6, false),
    (0xeb4374, 12, false),
    (0xab6070, 20, true),
    (0xab48c0, 1, true),
    (0xe77830, 7, false),
    (0xf159dc, 13, false),
    (0xab6084, 20, true),
    (0xab48d0, 1, true),
    (0xf159ec, 8, false),
    (0xf159f4, 14, false),
    (0xab609b, 20, true),
    (0xab48e0, 1, true),
    (0xf15a04, 9, false),
    (0xf15a10, 15, false),
    (0xab6420, 20, true),
    (0xab4900, 54, true),
    (0xabdda0, 129, true),
    (0xf15d00, 15, false),
    (0xabacb0, 21, true),
    (0xad3ab1, 3, true),
    (0xac15a7, 21, true),
    (0xab6434, 20, true),
    (0xab6ca0, 54, true),
    (0xac1b20, 97, true),
    (0xf15d10, 17, false),
    (0xad3b50, 156, true),
    (0xabfa00, 10, true),
];

const DEBUG_TEXT: [(&[u8], &[u8]); 4] = [
    (b"DisplayText\0", b"(IS)V\0"),
    (b"DisplayTextI\0", b"(ISI)V\0"),
    (b"DisplayTextII\0", b"(ISII)V\0"),
    (b"DisplayTextIII\0", b"(ISIII)V\0"),
];

fn at(bytes: &[u8], offset: usize, expected: &[u8]) -> Result<(), Error> {
    (bytes.get(offset..offset + expected.len()) == Some(expected))
        .then_some(())
        .ok_or(Error("instruction relationship changed"))
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(Error("short operand"))?
            .try_into()
            .map_err(|_| Error("short operand"))?,
    ))
}

fn call(bytes: &[u8], va: u32, offset: usize, target: u32) -> Result<(), Error> {
    at(bytes, offset, &[0xe8])?;
    (va.wrapping_add(offset as u32 + 5)
        .wrapping_add(word(bytes, offset + 1)?)
        == target)
        .then_some(())
        .ok_or(Error("call target changed"))
}

impl CampaignModePolicy {
    pub fn read(image: &[u8], budget: RuleInputBudget) -> Result<Self, Error> {
        if image.len() > budget.image_bytes {
            return Err(Error("image budget exceeded"));
        }
        if WINDOWS.iter().map(|(_, length, _)| length).sum::<usize>() > budget.profile_bytes {
            return Err(Error("window budget exceeded"));
        }
        if sha256(image) != REVIEWED_IMAGE {
            return Err(Error("unsupported client image"));
        }
        let pe = Pe::parse(image, budget).map_err(|_| Error("invalid selected PE"))?;
        let windows = WINDOWS
            .iter()
            .map(|&(va, length, code)| {
                pe.read_from(va, code)
                    .map_err(|_| Error("missing window"))?
                    .get(..length)
                    .ok_or(Error("short window"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::decode(&windows, REVIEWED_IMAGE)
    }

    fn decode(windows: &[&[u8]], image_sha256: Sha256Digest) -> Result<Self, Error> {
        if windows.len() != WINDOWS.len() {
            return Err(Error("incomplete windows"));
        }
        let [
            registration,
            handler,
            signature,
            name,
            mode,
            units,
            admission,
            consumer,
            replacement_registration,
            replacement_handler,
            replacement_signature,
            replacement_name,
        ] = &windows[..12]
        else {
            return Err(Error("incomplete windows"));
        };
        for (bytes, (_, length, _)) in windows.iter().zip(WINDOWS) {
            if bytes.len() < length {
                return Err(Error("truncated window"));
            }
        }
        for (offset, target) in [(0, WINDOWS[2].0), (5, WINDOWS[3].0), (10, WINDOWS[1].0)] {
            at(registration, offset, &[0x68])?;
            if word(registration, offset + 1)? != target {
                return Err(Error("registration pointer changed"));
            }
        }
        call(registration, WINDOWS[0].0, 15, 0x8ccc80)?;
        at(signature, 0, b"()V\0")?;
        at(name, 0, b"SetCampaignAI\0")?;
        at(handler, 0, &[0xa1])?;
        if word(handler, 1)? != 0x11b4a10 {
            return Err(Error("owned AI context changed"));
        }
        at(handler, 5, &[0x85, 0xc0, 0x75, 0x0b, 0x6a, 0x57])?;
        call(handler, WINDOWS[1].0, 11, 0x43eca0)?;
        at(handler, 16, &[0x83, 0xc4, 0x04, 0xc3])?;
        at(
            handler,
            20,
            &[
                0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x0c, 0x33,
                0xc0, 0x6a,
            ],
        )?;
        at(handler, 37, &[0x8b, 0xc8])?;
        call(handler, WINDOWS[1].0, 39, WINDOWS[4].0)?;
        at(handler, 44, &[0xc3])?;
        call(handler, WINDOWS[1].0, 45, 0x7fc7e0)?;
        at(handler, 50, &[0x6a])?;
        at(handler, 52, &[0x8b, 0xc8])?;
        call(handler, WINDOWS[1].0, 54, WINDOWS[4].0)?;
        at(handler, 59, &[0xc3])?;
        if handler[36] > 1 || handler[36] != handler[51] {
            return Err(Error("mode arguments differ or are not boolean"));
        }
        at(
            mode,
            0,
            &[
                0x55, 0x8b, 0xec, 0x53, 0x8b, 0x5d, 0x08, 0x56, 0x57, 0x85, 0xdb, 0x75, 0x0c, 0x81,
                0x89,
            ],
        )?;
        let field = word(mode, 15)?;
        let context_mask = word(mode, 19)?;
        at(mode, 23, &[0xeb, 0x0a, 0x81, 0xa1])?;
        if !context_mask.is_power_of_two()
            || word(mode, 27)? != field
            || word(mode, 31)? != !context_mask
        {
            return Err(Error("context set/clear relationship changed"));
        }
        call(units, WINDOWS[5].0, 0, 0x6933c0)?;
        at(
            units,
            5,
            &[0x85, 0xc0, 0x74, 0x24, 0x85, 0xdb, 0x74, 0x1c, 0x83, 0x60],
        )?;
        at(units, 17, &[0xeb, 0x1a])?;
        at(units, 41, &[0x83, 0x48])?;
        let unit_mask = u32::from(units[44]);
        if !unit_mask.is_power_of_two()
            || units[44] >= 0x80
            || units[15] != units[43]
            || (i32::from(units[16] as i8) as u32) != !unit_mask
        {
            return Err(Error("unit set/clear relationship changed"));
        }
        at(admission, 0, &[0x8b, 0x80])?;
        at(admission, 6, &[0xc1, 0xe8])?;
        at(
            admission,
            9,
            &[0xf7, 0xd0, 0xa8, 0x01, 0x74, 0x06, 0x83, 0x66],
        )?;
        at(admission, 19, &[0xeb, 0x04, 0x83, 0x4e])?;
        if word(admission, 2)? != field
            || admission[8] >= 32
            || 1u32 << admission[8] != context_mask
            || admission[17] != units[15]
            || admission[23] != units[15]
            || admission[18] != units[16]
            || admission[24] != units[44]
        {
            return Err(Error("future unit admission differs from mode transition"));
        }
        at(consumer, 0, &[0xf7, 0x87])?;
        at(consumer, 10, &[0x74])?;
        if word(consumer, 2)? != field
            || word(consumer, 6)? != context_mask
            || WINDOWS[7]
                .0
                .wrapping_add(12)
                .wrapping_add(consumer[11] as i8 as u32)
                != 0xac3a36
        {
            return Err(Error("mode consumer differs"));
        }
        for (offset, target) in [(0, WINDOWS[10].0), (5, WINDOWS[11].0), (10, WINDOWS[9].0)] {
            at(replacement_registration, offset, &[0x68])?;
            if word(replacement_registration, offset + 1)? != target {
                return Err(Error("replacement registration changed"));
            }
        }
        call(replacement_registration, WINDOWS[8].0, 15, 0x8ccc80)?;
        at(replacement_signature, 0, b"(I)V\0")?;
        at(replacement_name, 0, b"SetReplacementCount\0")?;
        let h = replacement_handler;
        at(h, 0, &[0x55, 0x8b, 0xec, 0xa1])?;
        if word(h, 4)? != word(handler, 1)? {
            return Err(Error("replacement AI context differs"));
        }
        at(h, 8, &[0x85, 0xc0, 0x75, 0x0d, 0xc7, 0x45, 0x08])?;
        if word(h, 15)? != 0x57 {
            return Err(Error("replacement context error differs"));
        }
        at(h, 19, &[0x5d, 0xe9])?;
        if WINDOWS[9].0.wrapping_add(25).wrapping_add(word(h, 21)?) != 0x43eca0 {
            return Err(Error("replacement error target differs"));
        }
        at(
            h,
            25,
            &[
                0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x0d, 0x8b,
                0x4d, 0x08, 0x33, 0xc0, 0x89, 0x88,
            ],
        )?;
        let replacement_count_field = word(h, 45)?;
        at(h, 49, &[0x5d, 0xc3])?;
        call(h, WINDOWS[9].0, 51, 0x7fc7e0)?;
        at(h, 56, &[0x8b, 0x4d, 0x08, 0x89, 0x88])?;
        at(h, 65, &[0x5d, 0xc3])?;
        if word(h, 61)? != replacement_count_field
            || replacement_count_field == 0
            || !replacement_count_field.is_multiple_of(4)
            || replacement_count_field == field
        {
            return Err(Error("replacement integer storage differs"));
        }
        for (index, (name, signature)) in DEBUG_TEXT.iter().enumerate() {
            let base = 12 + 4 * index;
            let registration = windows[base];
            for (offset, target) in [
                (0, WINDOWS[base + 2].0),
                (5, WINDOWS[base + 3].0),
                (10, WINDOWS[base + 1].0),
            ] {
                at(registration, offset, &[0x68])?;
                if word(registration, offset + 1)? != target {
                    return Err(Error("debug text registration changed"));
                }
            }
            call(registration, WINDOWS[base].0, 15, 0x8ccc80)?;
            at(windows[base + 1], 0, &[0xc3])?;
            at(windows[base + 2], 0, signature)?;
            at(windows[base + 3], 0, name)?;
        }
        let initializer = windows[40];
        if initializer[0] != 0xc7
            || !(0x80..=0x87).contains(&initializer[1])
            || word(initializer, 2)? != replacement_count_field
        {
            return Err(Error("replacement default initializer differs"));
        }
        let default_replacement = word(initializer, 6)? as i32;
        let return_order = return_post_order(windows, word(handler, 1)?)?;
        let guard_posts = guard_post_rules(
            &windows[28..35],
            replacement_count_field,
            word(handler, 1)?,
            return_order,
        )?;
        Ok(Self {
            image_sha256,
            context_mask,
            unit_mask,
            campaign_mode: handler[36] != 0,
            replacement_count_field,
            default_replacement,
            guard_posts,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let source = synthetic();
        Self::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .expect("synthetic campaign mode")
    }
}

fn guard_post_rules(
    w: &[&[u8]],
    limit: u32,
    context: u32,
    return_order: i32,
) -> Result<GuardPostRules, Error> {
    let [reg, handler, consumer, name, initial, bind, release] = w else {
        return Err(Error("incomplete guard-post windows"));
    };
    for (offset, target) in [(0, WINDOWS[2].0), (5, WINDOWS[31].0), (10, WINDOWS[29].0)] {
        at(reg, offset, &[0x68])?;
        if word(reg, offset + 1)? != target {
            return Err(Error("FillGuardPosts registration changed"));
        }
    }
    call(reg, WINDOWS[28].0, 15, 0x8ccc80)?;
    at(name, 0, b"FillGuardPosts\0")?;
    at(handler, 0, &[0xa1])?;
    if word(handler, 1)? != context {
        return Err(Error("guard-post context differs"));
    }
    at(handler, 5, &[0x85, 0xc0, 0x75, 0x0b, 0x6a, 0x57])?;
    call(handler, WINDOWS[29].0, 11, 0x43eca0)?;
    at(
        handler,
        16,
        &[
            0x83, 0xc4, 4, 0xc3, 0x8d, 0x48, 0x2c, 0x8b, 0x41, 4, 0x23, 1, 0x83, 0xf8, 0xff, 0x75,
            9, 0x33, 0xc0, 0x8b, 0xc8, 0xe9,
        ],
    )?;
    call(handler, WINDOWS[29].0, 42, 0x7fc7e0)?;
    at(handler, 47, &[0x8b, 0xc8, 0xe9])?;
    for offset in [37, 49] {
        if WINDOWS[29]
            .0
            .wrapping_add(offset + 5)
            .wrapping_add(word(handler, offset as usize + 1)?)
            != WINDOWS[30].0
        {
            return Err(Error("guard-post forwarding changed"));
        }
    }
    for (offset, bytes) in [
        (0, &[0x53, 0x8b, 0xd9, 0x8b, 0x83][..]),
        (9, &[0x8d, 0x8b]),
        (15, &[0x23, 1, 0x83, 0xf8, 0xff, 0x74, 0x69, 0x56]),
        (28, &[0x8b, 0xf0, 0x85, 0xf6, 0x74, 0x5c, 0x57, 0x8b, 0x56]),
        (38, &[0x8d, 0x4e]),
        (41, &[0x23, 0x11, 0x83, 0xfa, 0xff, 0x74, 9]),
        (53, &[0x85, 0xc0, 0x75, 0x2c, 0x8b, 0x46]),
        (60, &[0x3b, 0x83]),
        (66, &[0x7f, 0x21, 0x8b, 0x7e]),
        (71, &[0x57]),
        (
            77,
            &[
                0x83, 0xc4, 4, 0x8b, 0xcb, 0x6a, 0xff, 0x57, 0x85, 0xc0, 0x74, 7,
            ],
        ),
        (94, &[0xeb, 5]),
        (101, &[0x8b, 0x46]),
        (104, &[0x8d, 0x4e]),
        (107, &[0x23, 1, 0x83, 0xf8, 0xff, 0x74, 0x0b]),
        (
            119,
            &[0x8b, 0xf0, 0x85, 0xf6, 0x75, 0xa6, 0x5f, 0x5e, 0x5b, 0xc3],
        ),
    ] {
        at(consumer, offset, bytes)?;
    }
    for offset in [23, 48, 114] {
        call(consumer, WINDOWS[30].0, offset, 0x7fc7e0)?;
    }
    call(consumer, WINDOWS[30].0, 72, 0x5cc9f0)?;
    call(consumer, WINDOWS[30].0, 89, 0xac3070)?;
    call(consumer, WINDOWS[30].0, 96, 0xac2e00)?;
    let unit_field = u32::from(consumer[40]);
    let counter = consumer[59];
    if word(consumer, 62)? != limit
        || word(consumer, 5)?
            != word(consumer, 11)?
                .checked_add(4)
                .ok_or(Error("post-list reference overflow"))?
        || u32::from(consumer[37]) != unit_field + 4
        || consumer[106].checked_add(4) != Some(consumer[103])
        || unit_field % 4 != 0
        || counter % 4 != 0
        || u32::from(counter) < unit_field + 8
        || consumer[70] >= counter
        || consumer[106] <= counter
    {
        return Err(Error("guard-post field relationships changed"));
    }
    for window in [initial, release] {
        at(window, 0, &[0xc7, 0x46, consumer[37]])?;
        at(window, 7, &[0xc7, 0x46, consumer[40]])?;
        at(window, 14, &[0xc7, 0x46, counter])?;
        if word(window, 3)? != u32::MAX || word(window, 10)? != u32::MAX {
            return Err(Error("guard-post empty references changed"));
        }
    }
    at(bind, 0, &[0xff, 0x46, counter])?;
    Ok(GuardPostRules {
        initial_count: word(initial, 17)? as i32,
        binding_increment: 1, // x86 INC increments by one, independently of its operand.
        released_count: word(release, 17)? as i32,
        return_order,
    })
}

fn return_post_order(w: &[&[u8]], context: u32) -> Result<i32, Error> {
    let (reg, handler, consumer, action) = (w[35], w[36], w[37], w[39]);
    for (offset, target) in [(0, WINDOWS[2].0), (5, WINDOWS[38].0), (10, WINDOWS[36].0)] {
        at(reg, offset, &[0x68])?;
        if word(reg, offset + 1)? != target {
            return Err(Error("ReturnGuardPosts registration changed"));
        }
    }
    call(reg, WINDOWS[35].0, 15, 0x8ccc80)?;
    at(w[38], 0, b"ReturnGuardPosts\0")?;
    // Both natives use the same owned-context wrapper, with different tail targets.
    at(handler, 0, &w[29][..11])?;
    call(handler, WINDOWS[36].0, 11, 0x43eca0)?;
    at(handler, 16, &w[29][16..37])?;
    if word(handler, 1)? != context {
        return Err(Error("return-post context differs"));
    }
    at(handler, 37, &[0xe9])?;
    call(handler, WINDOWS[36].0, 42, 0x7fc7e0)?;
    at(handler, 47, &[0x8b, 0xc8, 0xe9])?;
    for offset in [37, 49] {
        if WINDOWS[36]
            .0
            .wrapping_add(offset + 5)
            .wrapping_add(word(handler, offset as usize + 1)?)
            != WINDOWS[37].0
        {
            return Err(Error("return-post forwarding changed"));
        }
    }
    for (offset, bytes) in [
        (0, &[0x8b, 0x81][..]),
        (6, &[0x81, 0xc1]),
        (12, &[0x23, 1, 0x83, 0xf8, 0xff, 0x74, 0x4d, 0x56]),
        (25, &[0x8b, 0xf0, 0x85, 0xf6, 0x74, 0x40, 0x90, 0x8b, 0x56]),
        (35, &[0x8d, 0x4e]),
        (38, &[0x23, 0x11, 0x83, 0xfa, 0xff, 0x74, 0x1a]),
        (50, &[0x85, 0xc0, 0x74, 0x11, 0x83, 0xb8]),
        (60, &[0, 0x75, 8, 0x50, 0x8b, 0xce]),
        (71, &[0x8b, 0x46]),
        (74, &[0x8d, 0x4e]),
        (77, &[0x23, 1, 0x83, 0xf8, 0xff, 0x74, 0x0b]),
        (89, &[0x8b, 0xf0, 0x85, 0xf6, 0x75, 0xc1, 0x5e, 0xc3]),
    ] {
        at(consumer, offset, bytes)?;
    }
    for offset in [20, 45, 84] {
        call(consumer, WINDOWS[37].0, offset, 0x7fc7e0)?;
    }
    call(consumer, WINDOWS[37].0, 66, WINDOWS[39].0)?;
    if word(consumer, 2)? != word(w[30], 5)?
        || word(consumer, 8)? != word(w[30], 11)?
        || consumer[34] != w[30][37]
        || consumer[37] != w[30][40]
        || consumer[73] != w[30][103]
        || consumer[76] != w[30][106]
    {
        return Err(Error("return and fill post fields differ"));
    }
    for (offset, bytes) in [
        (
            0,
            &[
                0x55, 0x8b, 0xec, 0x83, 0xec, 8, 0x56, 0x8b, 0x75, 8, 0x57, 0x8b, 0xf9, 0x8b, 0xce,
                0x6a, 1,
            ][..],
        ),
        (17, &[0x8b, 0x47]),
        (20, &[0x89, 0x45, 0xfc, 0x8b, 0x47]),
        (
            26,
            &[
                0x89, 0x45, 0xf8, 0x8d, 0x45, 0xfc, 0x50, 0x8d, 0x45, 0xf8, 0x50, 0x8b, 6, 0x68,
            ],
        ),
        (44, &[0xff, 0x90, 0xb8, 0, 0, 0, 0x8b, 0xc8]),
        (57, &[0x85, 0xc0, 0x75, 0x57, 0x8b, 0x86]),
        (67, &[0x85, 0xc0, 0x74, 0x4d, 0x83, 0x78]),
        (
            74,
            &[
                0, 0x7f, 0x47, 0x8b, 6, 0x8b, 0xce, 0xff, 0x90, 0xec, 0, 0, 0,
            ],
        ),
        (87, &[0x8b, 0x4f]),
        (90, &[0x6a, 1, 0x6a, 1, 0x6a, 0, 0x89, 0x4d, 8, 0x8b, 0x4f]),
        (
            102,
            &[
                0x6a, 0, 0x89, 0x4d, 0xf8, 0x8d, 0x4d, 8, 0x51, 0x8d, 0x4d, 0xf8, 0x51, 0x6a, 0,
                0x6a, 0, 0x50, 0x68,
            ],
        ),
        (130, &[0x83, 0xc4, 0x20, 0x8b, 0xce, 0x50]),
        (141, &[0x8b, 0xce]),
        (148, &[0x5f, 0x5e, 0x8b, 0xe5, 0x5d, 0xc2, 4, 0]),
    ] {
        at(action, offset, bytes)?;
    }
    for (offset, target) in [
        (52, 0x7ff460),
        (125, 0x69f760),
        (136, 0x690590),
        (143, 0x699d90),
    ] {
        call(action, WINDOWS[39].0, offset, target)?;
    }
    if action[19] != action[89]
        || action[25] != action[101]
        || action[25].checked_add(8) != Some(action[19])
        || action[19].checked_add(8) != Some(consumer[37])
    {
        return Err(Error("return-post home fields differ"));
    }
    Ok(word(action, 121)? as i32)
}

#[cfg(any(test, feature = "test-fixtures"))]
fn synthetic() -> Vec<Vec<u8>> {
    let mut w: Vec<_> = WINDOWS
        .iter()
        .map(|(_, length, _)| vec![0; *length])
        .collect();
    fn put(w: &mut [Vec<u8>], i: usize, offset: usize, bytes: &[u8]) {
        w[i][offset..offset + bytes.len()].copy_from_slice(bytes);
    }
    fn word(w: &mut [Vec<u8>], i: usize, offset: usize, value: u32) {
        put(w, i, offset, &value.to_le_bytes());
    }
    fn call(w: &mut [Vec<u8>], i: usize, offset: usize, target: u32) {
        put(w, i, offset, &[0xe8]);
        word(
            w,
            i,
            offset + 1,
            target.wrapping_sub(WINDOWS[i].0 + offset as u32 + 5),
        );
    }
    for (offset, target) in [(0, WINDOWS[2].0), (5, WINDOWS[3].0), (10, WINDOWS[1].0)] {
        put(&mut w, 0, offset, &[0x68]);
        word(&mut w, 0, offset + 1, target);
    }
    call(&mut w, 0, 15, 0x8ccc80);
    put(&mut w, 2, 0, b"()V\0");
    put(&mut w, 3, 0, b"SetCampaignAI\0");
    put(&mut w, 1, 0, &[0xa1]);
    word(&mut w, 1, 1, 0x11b4a10);
    put(&mut w, 1, 5, &[0x85, 0xc0, 0x75, 0x0b, 0x6a, 0x57]);
    call(&mut w, 1, 11, 0x43eca0);
    put(
        &mut w,
        1,
        16,
        &[
            0x83, 0xc4, 0x04, 0xc3, 0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8,
            0xff, 0x75, 0x0c, 0x33, 0xc0, 0x6a, 1, 0x8b, 0xc8,
        ],
    );
    call(&mut w, 1, 39, WINDOWS[4].0);
    put(&mut w, 1, 44, &[0xc3]);
    call(&mut w, 1, 45, 0x7fc7e0);
    put(&mut w, 1, 50, &[0x6a, 1, 0x8b, 0xc8]);
    call(&mut w, 1, 54, WINDOWS[4].0);
    put(&mut w, 1, 59, &[0xc3]);
    put(
        &mut w,
        4,
        0,
        &[
            0x55, 0x8b, 0xec, 0x53, 0x8b, 0x5d, 0x08, 0x56, 0x57, 0x85, 0xdb, 0x75, 0x0c, 0x81,
            0x89,
        ],
    );
    word(&mut w, 4, 15, 0x350);
    word(&mut w, 4, 19, 0x8000);
    put(&mut w, 4, 23, &[0xeb, 0x0a, 0x81, 0xa1]);
    word(&mut w, 4, 27, 0x350);
    word(&mut w, 4, 31, !0x8000);
    call(&mut w, 5, 0, 0x6933c0);
    put(
        &mut w,
        5,
        5,
        &[
            0x85, 0xc0, 0x74, 0x24, 0x85, 0xdb, 0x74, 0x1c, 0x83, 0x60, 0x30, !0x20, 0xeb, 0x1a,
        ],
    );
    put(&mut w, 5, 41, &[0x83, 0x48, 0x30, 0x20]);
    put(&mut w, 6, 0, &[0x8b, 0x80]);
    word(&mut w, 6, 2, 0x350);
    put(
        &mut w,
        6,
        6,
        &[
            0xc1, 0xe8, 15, 0xf7, 0xd0, 0xa8, 1, 0x74, 6, 0x83, 0x66, 0x30, !0x20, 0xeb, 4, 0x83,
            0x4e, 0x30, 0x20,
        ],
    );
    put(&mut w, 7, 0, &[0xf7, 0x87]);
    word(&mut w, 7, 2, 0x350);
    word(&mut w, 7, 6, 0x8000);
    put(&mut w, 7, 10, &[0x74, 0x51]);
    for (offset, target) in [(0, WINDOWS[10].0), (5, WINDOWS[11].0), (10, WINDOWS[9].0)] {
        put(&mut w, 8, offset, &[0x68]);
        word(&mut w, 8, offset + 1, target);
    }
    call(&mut w, 8, 15, 0x8ccc80);
    put(&mut w, 10, 0, b"(I)V\0");
    put(&mut w, 11, 0, b"SetReplacementCount\0");
    put(&mut w, 9, 0, &[0x55, 0x8b, 0xec, 0xa1]);
    word(&mut w, 9, 4, 0x11b4a10);
    put(&mut w, 9, 8, &[0x85, 0xc0, 0x75, 0x0d, 0xc7, 0x45, 0x08]);
    word(&mut w, 9, 15, 0x57);
    put(&mut w, 9, 19, &[0x5d, 0xe9]);
    word(&mut w, 9, 21, 0x43eca0u32.wrapping_sub(WINDOWS[9].0 + 25));
    put(
        &mut w,
        9,
        25,
        &[
            0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x0d, 0x8b,
            0x4d, 0x08, 0x33, 0xc0, 0x89, 0x88,
        ],
    );
    word(&mut w, 9, 45, 0x380);
    put(&mut w, 9, 49, &[0x5d, 0xc3]);
    call(&mut w, 9, 51, 0x7fc7e0);
    put(&mut w, 9, 56, &[0x8b, 0x4d, 0x08, 0x89, 0x88]);
    word(&mut w, 9, 61, 0x380);
    put(&mut w, 9, 65, &[0x5d, 0xc3]);
    for (index, (name, signature)) in DEBUG_TEXT.iter().enumerate() {
        let base = 12 + 4 * index;
        for (offset, target) in [
            (0, WINDOWS[base + 2].0),
            (5, WINDOWS[base + 3].0),
            (10, WINDOWS[base + 1].0),
        ] {
            put(&mut w, base, offset, &[0x68]);
            word(&mut w, base, offset + 1, target);
        }
        call(&mut w, base, 15, 0x8ccc80);
        put(&mut w, base + 1, 0, &[0xc3]);
        put(&mut w, base + 2, 0, signature);
        put(&mut w, base + 3, 0, name);
    }
    for (offset, target) in [(0, WINDOWS[2].0), (5, WINDOWS[31].0), (10, WINDOWS[29].0)] {
        put(&mut w, 28, offset, &[0x68]);
        word(&mut w, 28, offset + 1, target);
    }
    call(&mut w, 28, 15, 0x8ccc80);
    put(&mut w, 31, 0, b"FillGuardPosts\0");
    put(&mut w, 29, 0, &[0xa1]);
    word(&mut w, 29, 1, 0x11b4a10);
    put(&mut w, 29, 5, &[0x85, 0xc0, 0x75, 0x0b, 0x6a, 0x57]);
    call(&mut w, 29, 11, 0x43eca0);
    put(
        &mut w,
        29,
        16,
        &[
            0x83, 0xc4, 4, 0xc3, 0x8d, 0x48, 0x2c, 0x8b, 0x41, 4, 0x23, 1, 0x83, 0xf8, 0xff, 0x75,
            9, 0x33, 0xc0, 0x8b, 0xc8, 0xe9,
        ],
    );
    call(&mut w, 29, 42, 0x7fc7e0);
    put(&mut w, 29, 47, &[0x8b, 0xc8, 0xe9]);
    for offset in [37, 49] {
        word(
            &mut w,
            29,
            offset + 1,
            WINDOWS[30]
                .0
                .wrapping_sub(WINDOWS[29].0 + offset as u32 + 5),
        );
    }
    for (offset, bytes) in [
        (0, &[0x53, 0x8b, 0xd9, 0x8b, 0x83][..]),
        (9, &[0x8d, 0x8b]),
        (15, &[0x23, 1, 0x83, 0xf8, 0xff, 0x74, 0x69, 0x56]),
        (28, &[0x8b, 0xf0, 0x85, 0xf6, 0x74, 0x5c, 0x57, 0x8b, 0x56]),
        (38, &[0x8d, 0x4e]),
        (41, &[0x23, 0x11, 0x83, 0xfa, 0xff, 0x74, 9]),
        (53, &[0x85, 0xc0, 0x75, 0x2c, 0x8b, 0x46]),
        (60, &[0x3b, 0x83]),
        (66, &[0x7f, 0x21, 0x8b, 0x7e]),
        (71, &[0x57]),
        (
            77,
            &[
                0x83, 0xc4, 4, 0x8b, 0xcb, 0x6a, 0xff, 0x57, 0x85, 0xc0, 0x74, 7,
            ],
        ),
        (94, &[0xeb, 5]),
        (101, &[0x8b, 0x46]),
        (104, &[0x8d, 0x4e]),
        (107, &[0x23, 1, 0x83, 0xf8, 0xff, 0x74, 0x0b]),
        (
            119,
            &[0x8b, 0xf0, 0x85, 0xf6, 0x75, 0xa6, 0x5f, 0x5e, 0x5b, 0xc3],
        ),
    ] {
        put(&mut w, 30, offset, bytes);
    }
    word(&mut w, 30, 5, 0x404);
    word(&mut w, 30, 11, 0x400);
    word(&mut w, 30, 62, 0x380);
    for (offset, value) in [
        (37, 0x54),
        (40, 0x50),
        (59, 0x60),
        (70, 0x4c),
        (103, 0x6c),
        (106, 0x68),
    ] {
        put(&mut w, 30, offset, &[value]);
    }
    for offset in [23, 48, 114] {
        call(&mut w, 30, offset, 0x7fc7e0);
    }
    for (offset, target) in [(72, 0x5cc9f0), (89, 0xac3070), (96, 0xac2e00)] {
        call(&mut w, 30, offset, target);
    }
    for (index, count) in [(32, 3), (34, 910)] {
        put(&mut w, index, 0, &[0xc7, 0x46, 0x54]);
        word(&mut w, index, 3, u32::MAX);
        put(&mut w, index, 7, &[0xc7, 0x46, 0x50]);
        word(&mut w, index, 10, u32::MAX);
        put(&mut w, index, 14, &[0xc7, 0x46, 0x60]);
        word(&mut w, index, 17, count);
    }
    put(&mut w, 33, 0, &[0xff, 0x46, 0x60]);
    for (offset, target) in [(0, WINDOWS[2].0), (5, WINDOWS[38].0), (10, WINDOWS[36].0)] {
        put(&mut w, 35, offset, &[0x68]);
        word(&mut w, 35, offset + 1, target);
    }
    call(&mut w, 35, 15, 0x8ccc80);
    put(&mut w, 38, 0, b"ReturnGuardPosts\0");
    w[36] = w[29].clone();
    call(&mut w, 36, 11, 0x43eca0);
    call(&mut w, 36, 42, 0x7fc7e0);
    for offset in [37, 49] {
        word(
            &mut w,
            36,
            offset + 1,
            WINDOWS[37]
                .0
                .wrapping_sub(WINDOWS[36].0 + offset as u32 + 5),
        );
    }
    for (offset, bytes) in [
        (0, &[0x8b, 0x81][..]),
        (6, &[0x81, 0xc1]),
        (12, &[0x23, 1, 0x83, 0xf8, 0xff, 0x74, 0x4d, 0x56]),
        (
            25,
            &[
                0x8b, 0xf0, 0x85, 0xf6, 0x74, 0x40, 0x90, 0x8b, 0x56, 0x54, 0x8d, 0x4e, 0x50,
            ],
        ),
        (38, &[0x23, 0x11, 0x83, 0xfa, 0xff, 0x74, 0x1a]),
        (50, &[0x85, 0xc0, 0x74, 0x11, 0x83, 0xb8]),
        (60, &[0, 0x75, 8, 0x50, 0x8b, 0xce]),
        (
            71,
            &[
                0x8b, 0x46, 0x6c, 0x8d, 0x4e, 0x68, 0x23, 1, 0x83, 0xf8, 0xff, 0x74, 0x0b,
            ],
        ),
        (89, &[0x8b, 0xf0, 0x85, 0xf6, 0x75, 0xc1, 0x5e, 0xc3]),
    ] {
        put(&mut w, 37, offset, bytes);
    }
    word(&mut w, 37, 2, 0x404);
    word(&mut w, 37, 8, 0x400);
    word(&mut w, 37, 56, 0x510);
    for offset in [20, 45, 84] {
        call(&mut w, 37, offset, 0x7fc7e0);
    }
    call(&mut w, 37, 66, WINDOWS[39].0);
    for (offset, bytes) in [
        (
            0,
            &[
                0x55, 0x8b, 0xec, 0x83, 0xec, 8, 0x56, 0x8b, 0x75, 8, 0x57, 0x8b, 0xf9, 0x8b, 0xce,
                0x6a, 1,
            ][..],
        ),
        (17, &[0x8b, 0x47, 0x48]),
        (20, &[0x89, 0x45, 0xfc, 0x8b, 0x47, 0x40]),
        (
            26,
            &[
                0x89, 0x45, 0xf8, 0x8d, 0x45, 0xfc, 0x50, 0x8d, 0x45, 0xf8, 0x50, 0x8b, 6, 0x68,
            ],
        ),
        (44, &[0xff, 0x90, 0xb8, 0, 0, 0, 0x8b, 0xc8]),
        (57, &[0x85, 0xc0, 0x75, 0x57, 0x8b, 0x86]),
        (67, &[0x85, 0xc0, 0x74, 0x4d, 0x83, 0x78, 0x58]),
        (
            74,
            &[
                0, 0x7f, 0x47, 0x8b, 6, 0x8b, 0xce, 0xff, 0x90, 0xec, 0, 0, 0,
            ],
        ),
        (87, &[0x8b, 0x4f, 0x48]),
        (
            90,
            &[0x6a, 1, 0x6a, 1, 0x6a, 0, 0x89, 0x4d, 8, 0x8b, 0x4f, 0x40],
        ),
        (
            102,
            &[
                0x6a, 0, 0x89, 0x4d, 0xf8, 0x8d, 0x4d, 8, 0x51, 0x8d, 0x4d, 0xf8, 0x51, 0x6a, 0,
                0x6a, 0, 0x50, 0x68,
            ],
        ),
        (130, &[0x83, 0xc4, 0x20, 0x8b, 0xce, 0x50]),
        (141, &[0x8b, 0xce]),
        (148, &[0x5f, 0x5e, 0x8b, 0xe5, 0x5d, 0xc2, 4, 0]),
    ] {
        put(&mut w, 39, offset, bytes);
    }
    put(&mut w, 40, 0, &[0xc7, 0x86]);
    word(&mut w, 40, 2, 0x380);
    word(&mut w, 40, 6, 5);
    word(&mut w, 39, 40, 0x330000);
    word(&mut w, 39, 63, 0x570);
    word(&mut w, 39, 121, 431);
    for (offset, target) in [
        (52, 0x7ff460),
        (125, 0x69f760),
        (136, 0x690590),
        (143, 0x699d90),
    ] {
        call(&mut w, 39, offset, target);
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(w: &[Vec<u8>]) -> Result<CampaignModePolicy, Error> {
        CampaignModePolicy::decode(&w.iter().map(Vec::as_slice).collect::<Vec<_>>(), [0x42; 32])
    }

    #[test]
    fn mode_and_nonstock_masks_are_source_operands() {
        let mut w = synthetic();
        let p = decode(&w).unwrap();
        assert_eq!(
            (p.context_mask, p.unit_mask, p.campaign_mode),
            (0x8000, 0x20, true)
        );
        assert_eq!(p.replacement_count_field, 0x380);
        assert_eq!(p.default_replacement, 5);
        w[40][2..6].copy_from_slice(&0x398u32.to_le_bytes());
        for offset in [45, 61] {
            w[9][offset..offset + 4].copy_from_slice(&0x398u32.to_le_bytes());
        }
        w[30][62..66].copy_from_slice(&0x398u32.to_le_bytes());
        assert_eq!(decode(&w).unwrap().replacement_count_field, 0x398);
        w[1][36] = 0;
        w[1][51] = 0;
        assert!(!decode(&w).unwrap().campaign_mode);
    }

    #[test]
    fn guard_post_counts_and_field_relationships_are_source_bound() {
        let mut w = synthetic();
        let p = decode(&w).unwrap();
        assert_eq!(p.guard_posts.initial_count, 3);
        assert_eq!(p.guard_posts.binding_increment, 1);
        assert_eq!(p.guard_posts.released_count, 910);
        assert_eq!(p.guard_posts.return_order, 431);
        w[32][17..21].copy_from_slice(&(-4i32).to_le_bytes());
        w[34][17..21].copy_from_slice(&1234i32.to_le_bytes());
        let p = decode(&w).unwrap();
        assert_eq!(p.guard_posts.initial_count, -4);
        assert_eq!(p.guard_posts.released_count, 1234);
        for (window, offset) in [
            (28, 11),
            (29, 1),
            (29, 38),
            (29, 50),
            (30, 37),
            (30, 59),
            (30, 62),
            (30, 66),
            (30, 90),
            (30, 97),
            (30, 106),
            (31, 13),
            (32, 3),
            (33, 1),
            (33, 2),
            (34, 10),
            (34, 16),
        ] {
            let mut w = synthetic();
            w[window][offset] ^= 1;
            assert!(decode(&w).is_err(), "window {window} offset {offset}");
        }
    }

    #[test]
    fn return_order_and_post_relationships_are_source_bound() {
        let mut w = synthetic();
        w[39][121..125].copy_from_slice(&1931i32.to_le_bytes());
        assert_eq!(decode(&w).unwrap().guard_posts.return_order, 1931);
        for (window, offset) in [
            (35, 1),
            (35, 11),
            (36, 1),
            (36, 12),
            (36, 38),
            (36, 43),
            (36, 50),
            (37, 2),
            (37, 8),
            (37, 18),
            (37, 34),
            (37, 37),
            (37, 53),
            (37, 60),
            (37, 61),
            (37, 67),
            (37, 73),
            (37, 94),
            (38, 16),
            (39, 19),
            (39, 25),
            (39, 59),
            (39, 69),
            (39, 75),
            (39, 89),
            (39, 101),
            (39, 120),
            (39, 126),
            (39, 137),
            (39, 144),
            (39, 155),
        ] {
            let mut bad = w.clone();
            bad[window][offset] ^= 1;
            assert!(decode(&bad).is_err(), "window {window} offset {offset}");
        }
        for index in 35..40 {
            for len in 0..w[index].len() {
                let mut short = w.clone();
                short[index].truncate(len);
                assert!(decode(&short).is_err());
            }
        }
    }

    #[test]
    fn relationships_strings_and_all_truncations_refuse() {
        for (i, offset) in [
            (0, 11),
            (1, 8),
            (1, 32),
            (1, 36),
            (1, 40),
            (2, 3),
            (3, 13),
            (4, 19),
            (4, 31),
            (5, 12),
            (5, 16),
            (5, 44),
            (6, 8),
            (6, 18),
            (7, 6),
            (7, 11),
            (8, 11),
            (9, 4),
            (9, 10),
            (9, 21),
            (9, 27),
            (9, 37),
            (9, 40),
            (9, 47),
            (9, 50),
            (9, 52),
            (9, 58),
            (9, 61),
            (9, 66),
            (10, 4),
            (11, 19),
        ] {
            let mut w = synthetic();
            w[i][offset] ^= 1;
            assert!(decode(&w).is_err(), "window {i} offset {offset}");
        }
        for (i, (_, length, _)) in WINDOWS.iter().enumerate() {
            for length in 0..*length {
                let mut w = synthetic();
                w[i].truncate(length);
                assert!(decode(&w).is_err(), "window {i} length {length}");
            }
        }
    }

    #[test]
    fn debug_text_requires_the_bound_primary_return_and_exact_contract() {
        for index in 0..4 {
            let base = 12 + 4 * index;
            for (window, offset) in [
                (base, 1),
                (base, 6),
                (base, 11),
                (base, 16),
                (base + 1, 0),
                (base + 2, WINDOWS[base + 2].1 - 1),
                (base + 3, WINDOWS[base + 3].1 - 1),
            ] {
                let mut w = synthetic();
                w[window][offset] ^= 1;
                assert!(decode(&w).is_err(), "window {window} offset {offset}");
            }
        }
    }

    #[test]
    fn unqualified_image_and_insufficient_budget_refuse() {
        let budget = RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 0,
            sections: 96,
        };
        assert!(CampaignModePolicy::read(b"MZ", budget).is_err());
        assert!(
            CampaignModePolicy::read(
                b"MZ",
                RuleInputBudget {
                    profile_bytes: 1024,
                    ..budget
                }
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
    fn reviewed_original_image_decodes() {
        let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE")).unwrap();
        let total = WINDOWS.iter().map(|(_, length, _)| length).sum();
        let budget = RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: total,
            sections: 96,
        };
        let p = CampaignModePolicy::read(&image, budget).unwrap();
        assert_eq!(
            (p.context_mask, p.unit_mask, p.campaign_mode),
            (0x4000, 0x40, true)
        );
        assert_eq!(p.image_sha256, REVIEWED_IMAGE);
        assert_eq!(p.replacement_count_field, 0x1d4);
        assert_eq!(p.default_replacement, 3);
        assert_eq!(p.guard_posts.initial_count, 0);
        assert_eq!(p.guard_posts.binding_increment, 1);
        assert_eq!(p.guard_posts.released_count, 99999);
        assert_eq!(p.guard_posts.return_order, 0xd000f);
        assert!(
            CampaignModePolicy::read(
                &image,
                RuleInputBudget {
                    profile_bytes: total - 1,
                    ..budget
                }
            )
            .is_err()
        );
    }
}
