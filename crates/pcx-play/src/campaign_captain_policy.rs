//! Captain selector and query operands from the reviewed client image. This is a source
//! reader: it decodes instruction operands; the captain group/unit/home
//! semantic boundary is unimplemented here.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
};

#[derive(Clone, Debug)]
pub struct CaptainPolicy {
    pub image_sha256: Sha256Digest,
    pub first_selector_mask: u8,
    pub second_selector_mask: u8,
    pub defenders: DefenderRules,
}

#[derive(Clone, Debug)]
pub struct DefenderRules {
    pub alternates: [([u8; 4], [u8; 4]); 5],
    pub count_step: i32,
    pub health_priority: f32,
    pub excluded_orders: [i32; 2],
}

#[derive(Debug, thiserror::Error)]
#[error("campaign captain policy: {0}")]
pub struct Error(&'static str);

// VAs and lengths identify instruction windows in the reviewed PE. Every value
// returned below is decoded from an operand within those windows.
const WINDOWS: [(u32, usize, bool); 35] = [
    (0xab645c, 20, true),  // SetCaptainHome registration banner (sig/name/handler)
    (0xab6f10, 80, true),  // SetCaptainHome handler entry
    (0xac26d0, 124, true), // selector (masks + slots + resolver/home write)
    (0xab644d, 15, true),  // CreateCaptains primary registration
    (0xab4771, 17, true),  // CreateCaptains handler -> main-work join (abd160)
    (0xabd2ad, 15, true),  // create first-pair home writes
    (0xabd352, 15, true),  // create second-pair home writes
    (0xab6591, 20, true),  // CaptainGroupSize registration
    (0xab657d, 20, true),  // CaptainIsEmpty registration
    (0xab3e1a, 31, true),  // size result paths
    (0xab3f0a, 26, true),  // empty result path
    (0xabe270, 27, true),  // first captain reference resolver
    (0xac9b30, 6, true),   // association counter update
    (0xf15e34, 17, false), // CaptainGroupSize name
    (0xf15e24, 15, false), // CaptainIsEmpty name
    (0xe6f674, 4, false),  // integer query signature
    (0xe700c8, 4, false),  // boolean query signature
    (0xab65e4, 20, true),  // CaptainInCombat registration
    (0xab3e40, 70, true),  // owned AI context + boolean forwarding
    (0xabbd90, 118, true), // captain selection + target-reference predicate
    (0xf15e8c, 16, false), // CaptainInCombat name
    (0xf15e84, 5, false),  // (B)B signature
    (0xad5b57, 40, true),  // initial empty target-reference pairs
    (0xab66c3, 20, true),  // AddDefenders registration
    (0xab3a30, 76, true),  // owned AI context and argument forwarding
    (0xabaa10, 70, true),  // second captain reference and recruiting dispatch
    (0xabb130, 296, true), // directed alternate types and completion paths
    (0xabaf8d, 25, true),  // current-order exclusions
    (0xabafac, 28, true),  // health-priority comparison
    (0x10db0ac, 4, false), // health-priority threshold
    (0xabb0d9, 8, true),   // newly recruited unit decrements remaining count
    (0xabca05, 8, true),   // existing member decrements remaining count
    (0xf15f28, 13, false), // AddDefenders name
    (0xf15c88, 6, false),  // (II)B
    (0xabb076, 19, true),  // canonical association and AI-component update
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

fn jmp(bytes: &[u8], va: u32, offset: usize) -> Result<u32, Error> {
    at(bytes, offset, &[0xe9])?;
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

impl CaptainPolicy {
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
        Self::decode(&windows, REVIEWED_IMAGE)
    }

    fn decode(w: &[&[u8]], image_sha256: Sha256Digest) -> Result<Self, Error> {
        if w.len() != WINDOWS.len() {
            return Err(Error("incomplete captain-policy windows"));
        }
        let [
            home_reg,
            home_handler,
            sel,
            capt_reg,
            create_jmp,
            create1,
            create2,
            size_reg,
            empty_reg,
            size_result,
            empty_result,
            first_reference,
            counter_update,
            size_name,
            empty_name,
            size_signature,
            empty_signature,
        ] = &w[..17]
        else {
            return Err(Error("incomplete captain-policy windows"));
        };
        for (window, (_, length, _)) in w.iter().zip(WINDOWS) {
            if window.len() < length {
                return Err(Error("truncated captain-policy window"));
            }
        }

        // SetCaptainHome registration binds handler 0xab6f10 via the registrar.
        at(home_reg, 0, &[0x68])?;
        if word(home_reg, 1)? != 0xe6f5a8 {
            return Err(Error("SetCaptainHome signature changed"));
        }
        at(home_reg, 5, &[0x68])?;
        if word(home_reg, 6)? != 0xf15d34 {
            return Err(Error("SetCaptainHome name changed"));
        }
        at(home_reg, 10, &[0x68])?;
        if word(home_reg, 11)? != 0xab6f10 {
            return Err(Error("SetCaptainHome handler changed"));
        }
        if call(home_reg, WINDOWS[0].0, 15)? != 0x8ccc80 {
            return Err(Error("SetCaptainHome registrar changed"));
        }

        // Handler entry reads the AI-context global and dispatches both paths to
        // the selector.
        at(home_handler, 0, &[0x55, 0x8b, 0xec])?;
        at(home_handler, 3, &[0xa1])?;
        if word(home_handler, 4)? != 0x11b4a10 {
            return Err(Error("AI context global changed"));
        }
        if call(home_handler, WINDOWS[1].0, 50)? != 0xac26d0 {
            return Err(Error("SetCaptainHome first dispatch changed"));
        }
        if call(home_handler, WINDOWS[1].0, 73)? != 0xac26d0 {
            return Err(Error("SetCaptainHome second dispatch changed"));
        }

        // Selector derives the two single-bit masks and their context slots, and
        // binds the resolver + home writer for both.
        // F6 TEST r/m8, imm8 does not sign-extend, so bit 0x80 is a valid mask.
        at(sel, 4, &[0xf6, 0x45, 0x08])?;
        let first_selector_mask = *sel.get(7).ok_or(Error("missing first selector mask"))?;
        if first_selector_mask == 0 || first_selector_mask.count_ones() != 1 {
            return Err(Error("first selector mask is not a single nonzero bit"));
        }
        at(sel, 15, &[0x8d, 0x88])?;
        let first_slot = word(sel, 17)?;
        if first_slot == 0 {
            return Err(Error("first selector slot is empty"));
        }
        let first_end = first_slot
            .checked_add(8)
            .ok_or(Error("first selector reference pair overflows"))?;
        // Conditional skip: first test -> second test (offset 56).
        at(sel, 13, &[0x74])?;
        if 15 + i32::from(sel[14] as i8) != 56 {
            return Err(Error("first selector JE does not skip to the second test"));
        }
        if call(sel, WINDOWS[2].0, 31)? != 0x7fc7e0 {
            return Err(Error("first selector resolver changed"));
        }
        if call(sel, WINDOWS[2].0, 48)? != 0xad0630 {
            return Err(Error("first selector home writer changed"));
        }

        at(sel, 56, &[0xf6, 0x45, 0x08])?;
        let second_selector_mask = *sel.get(59).ok_or(Error("missing second selector mask"))?;
        if second_selector_mask == 0
            || second_selector_mask.count_ones() != 1
            || second_selector_mask == first_selector_mask
        {
            return Err(Error(
                "second selector mask is not a distinct single nonzero bit",
            ));
        }
        at(sel, 62, &[0x8d, 0x88])?;
        let second_slot = word(sel, 64)?;
        if second_slot == 0 || second_slot == first_slot {
            return Err(Error("second selector slot differs from first"));
        }
        let second_end = second_slot
            .checked_add(8)
            .ok_or(Error("second selector reference pair overflows"))?;
        if first_slot < second_end && second_slot < first_end {
            return Err(Error("selector reference pairs overlap"));
        }
        // Conditional skip: second test -> end (offset 116).
        at(sel, 60, &[0x74])?;
        if 62 + i32::from(sel[61] as i8) != 116 {
            return Err(Error("second selector JE does not skip to the end"));
        }
        if call(sel, WINDOWS[2].0, 78)? != 0x7fc7e0 {
            return Err(Error("second selector resolver changed"));
        }
        if call(sel, WINDOWS[2].0, 95)? != 0xad0630 {
            return Err(Error("second selector home writer changed"));
        }

        // CreateCaptains primary registration binds handler 0xab4750, and the
        // handler joins the main work (abd160).
        at(capt_reg, 0, &[0x68])?;
        if word(capt_reg, 1)? != 0xf15d24 {
            return Err(Error("CreateCaptains name changed"));
        }
        at(capt_reg, 5, &[0x68])?;
        if word(capt_reg, 6)? != 0xab4750 {
            return Err(Error("CreateCaptains handler changed"));
        }
        if call(capt_reg, WINDOWS[3].0, 10)? != 0x8ccc80 {
            return Err(Error("CreateCaptains registrar changed"));
        }
        // handler -> main-work join: 0xab4750 tail-jumps to 0xabd160.
        at(create_jmp, 2, &[0x8b, 0xc8])?;
        if jmp(create_jmp, WINDOWS[4].0, 4)? != 0xabd160 {
            return Err(Error("CreateCaptains handler does not join the main work"));
        }

        // The main work writes the two encoded reference pairs; the selector's
        // slots must match those create-write offsets (second word = slot + 4).
        at(create1, 0, &[0x89, 0x87])?;
        if word(create1, 2)? != first_slot {
            return Err(Error("create first slot differs from selector"));
        }
        at(create1, 9, &[0x89, 0x87])?;
        if word(create1, 11)? != first_end - 4 {
            return Err(Error("create first slot second word differs"));
        }
        at(create2, 0, &[0x89, 0x87])?;
        if word(create2, 2)? != second_slot {
            return Err(Error("create second slot differs from selector"));
        }
        at(create2, 9, &[0x89, 0x87])?;
        if word(create2, 11)? != second_end - 4 {
            return Err(Error("create second slot second word differs"));
        }

        for (registration, index, signature, name, handler) in [
            (*size_reg, 7, WINDOWS[15].0, WINDOWS[13].0, 0xab3da0),
            (*empty_reg, 8, WINDOWS[16].0, WINDOWS[14].0, 0xab3e90),
        ] {
            for (offset, target) in [(0, signature), (5, name), (10, handler)] {
                at(registration, offset, &[0x68])?;
                if word(registration, offset + 1)? != target {
                    return Err(Error("captain query registration changed"));
                }
            }
            if call(registration, WINDOWS[index].0, 15)? != 0x8ccc80 {
                return Err(Error("captain query registrar changed"));
            }
        }
        at(size_name, 0, b"CaptainGroupSize\0")?;
        at(empty_name, 0, b"CaptainIsEmpty\0")?;
        at(size_signature, 0, b"()I\0")?;
        at(empty_signature, 0, b"()B\0")?;

        // Query results share the first reference and the association counter;
        // this relationship alone does not qualify runtime membership.
        at(first_reference, 0, &[0x8b, 0x81])?;
        at(first_reference, 6, &[0x81, 0xc1])?;
        if word(first_reference, 2)? != first_end - 4 || word(first_reference, 8)? != first_slot {
            return Err(Error("query reference differs from first captain slot"));
        }
        at(
            first_reference,
            12,
            &[0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x03, 0x33, 0xc0, 0xc3],
        )?;
        if jmp(first_reference, WINDOWS[11].0, 22)? != 0x7fc7e0 {
            return Err(Error("captain query reference resolver changed"));
        }
        for offset in [0, 19] {
            if call(size_result, WINDOWS[9].0, offset)? != WINDOWS[11].0 {
                return Err(Error("size query first-reference dispatch changed"));
            }
        }
        if call(size_result, WINDOWS[9].0, 12)? != 0x7fc7e0
            || call(empty_result, WINDOWS[10].0, 0)? != 0x7fc7e0
            || call(empty_result, WINDOWS[10].0, 7)? != WINDOWS[11].0
        {
            return Err(Error("captain query result dispatch changed"));
        }
        at(size_result, 5, &[0x8b, 0x80])?;
        at(size_result, 11, &[0xc3])?;
        at(size_result, 17, &[0x8b, 0xc8])?;
        at(size_result, 24, &[0x8b, 0x80])?;
        at(size_result, 30, &[0xc3])?;
        at(empty_result, 5, &[0x8b, 0xc8])?;
        at(empty_result, 12, &[0x33, 0xc9, 0x39, 0x88])?;
        at(empty_result, 20, &[0x0f, 0x94, 0xc1, 0x8b, 0xc1, 0xc3])?;
        at(counter_update, 0, &[0x01, 0xb3])?;
        let counter = word(size_result, 7)?;
        if counter == 0
            || word(size_result, 26)? != counter
            || word(empty_result, 16)? != counter
            || word(counter_update, 2)? != counter
        {
            return Err(Error("captain query and association counters differ"));
        }
        combat_query(&w[17..23], first_slot, second_slot)?;
        let defenders = defender_rules(&w[23..], second_slot, w[18])?;

        Ok(Self {
            image_sha256,
            first_selector_mask,
            second_selector_mask,
            defenders,
        })
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let source = synthetic_windows();
        Self::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0x42; 32],
        )
        .expect("synthetic captain-policy instructions")
    }
}

impl DefenderRules {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if !(self.count_step < 0 && i8::try_from(self.count_step).is_ok())
            || !self.health_priority.is_finite()
            || !(0.0..=1.0).contains(&self.health_priority)
            || self.excluded_orders.iter().any(|order| *order <= 0)
            || self.excluded_orders[0] == self.excluded_orders[1]
        {
            return Err(Error("invalid defender count, health or order operands"));
        }
        for (i, (base, alternate)) in self.alternates.iter().enumerate() {
            if base == alternate
                || base
                    .iter()
                    .chain(alternate)
                    .any(|b| *b == 0 || !b.is_ascii())
                || self.alternates[..i]
                    .iter()
                    .any(|(a, b)| a == base || a == alternate || b == base || b == alternate)
            {
                return Err(Error("ambiguous defender alternate types"));
            }
        }
        Ok(())
    }
}

fn defender_rules(w: &[&[u8]], second: u32, combat_handler: &[u8]) -> Result<DefenderRules, Error> {
    let [
        registration,
        handler,
        selector,
        recruit,
        orders,
        priority,
        threshold,
        new_count,
        old_count,
        name,
        signature,
        attach,
    ] = w
    else {
        return Err(Error("incomplete defender windows"));
    };
    for (offset, target) in [(0, WINDOWS[33].0), (5, WINDOWS[32].0), (10, WINDOWS[24].0)] {
        at(registration, offset, &[0x68])?;
        if word(registration, offset + 1)? != target {
            return Err(Error("AddDefenders registration changed"));
        }
    }
    if call(registration, WINDOWS[23].0, 15)? != 0x8ccc80 {
        return Err(Error("AddDefenders registrar changed"));
    }
    at(name, 0, b"AddDefenders\0")?;
    at(signature, 0, b"(II)B\0")?;
    at(
        handler,
        0,
        combat_handler
            .get(..14)
            .ok_or(Error("short context handler"))?,
    )?;
    if call(handler, WINDOWS[24].0, 14)? != 0x43eca0
        || call(handler, WINDOWS[24].0, 56)? != 0x7fc7e0
        || call(handler, WINDOWS[24].0, 49)? != WINDOWS[25].0
        || call(handler, WINDOWS[24].0, 69)? != WINDOWS[25].0
    {
        return Err(Error("AddDefenders context forwarding changed"));
    }
    at(
        handler,
        19,
        &[
            0x83, 0xc4, 0x04, 0x33, 0xc0, 0x5d, 0xc3, 0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23,
            0x01, 0x83, 0xf8, 0xff, 0x75, 0x11, 0xff, 0x75, 0x0c, 0x33, 0xc0, 0xff, 0x75, 0x08,
            0x8b, 0xc8,
        ],
    )?;
    at(handler, 54, &[0x5d, 0xc3])?;
    at(
        handler,
        61,
        &[0xff, 0x75, 0x0c, 0x8b, 0xc8, 0xff, 0x75, 0x08],
    )?;
    at(handler, 74, &[0x5d, 0xc3])?;

    at(
        selector,
        0,
        &[0x55, 0x8b, 0xec, 0x56, 0x8b, 0xf1, 0x8b, 0x86],
    )?;
    at(selector, 12, &[0x8d, 0x8e])?;
    if word(selector, 8)?
        != second
            .checked_add(4)
            .ok_or(Error("defender reference overflow"))?
        || word(selector, 14)? != second
    {
        return Err(Error("defender reference differs from second captain"));
    }
    at(
        selector,
        18,
        &[
            0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x15, 0xff, 0x75, 0x0c, 0x33, 0xc0, 0x8b, 0xce,
            0xff, 0x75, 0x08, 0x50,
        ],
    )?;
    at(selector, 41, &[0x5e, 0x5d, 0xc2, 0x08, 0])?;
    at(
        selector,
        51,
        &[0xff, 0x75, 0x0c, 0x8b, 0xce, 0xff, 0x75, 0x08, 0x50],
    )?;
    at(selector, 65, &[0x5e, 0x5d, 0xc2, 0x08, 0])?;
    if call(selector, WINDOWS[25].0, 46)? != 0x7fc7e0
        || call(selector, WINDOWS[25].0, 36)? != WINDOWS[26].0
        || call(selector, WINDOWS[25].0, 60)? != WINDOWS[26].0
    {
        return Err(Error("defender controller resolution changed"));
    }

    at(
        recruit,
        0,
        &[
            0x55, 0x8b, 0xec, 0x51, 0x53, 0x8b, 0x5d, 0x08, 0x89, 0x4d, 0xfc, 0x85, 0xdb, 0x75,
            0x13, 0x6a, 0x57,
        ],
    )?;
    if call(recruit, WINDOWS[26].0, 17)? != 0x43eca0 {
        return Err(Error("missing defender controller error path"));
    }
    at(
        recruit,
        22,
        &[
            0x83, 0xc4, 0x04, 0x33, 0xc0, 0x5b, 0x8b, 0xe5, 0x5d, 0xc2, 0x0c, 0, 0x56, 0x57, 0x8b,
            0x7d, 0x10,
        ],
    )?;
    for (offset, bytes) in [
        (39, &[0x81, 0xff][..]),
        (45, &[0x77, 0x25, 0x74, 0x1c, 0x8b, 0xc7, 0x2d]),
        (56, &[0x74, 0x0c, 0x83, 0xe8]),
        (61, &[0x75, 0x25, 0xbe]),
        (68, &[0xeb, 0x2e, 0xbe]),
        (75, &[0xeb, 0x27, 0xbe]),
        (82, &[0xeb, 0x20, 0x81, 0xff]),
        (90, &[0x74, 0x13, 0x81, 0xff]),
        (98, &[0x74, 0x04, 0x33, 0xf6, 0xeb, 0x0c, 0xbe]),
        (109, &[0xeb, 0x05, 0xbe]),
        (116, &[0x57, 0x8d, 0x45, 0x0c, 0x50, 0x53]),
        (
            271,
            &[
                0x33, 0xc0, 0x5f, 0x5e, 0x5b, 0x8b, 0xe5, 0x5d, 0xc2, 0x0c, 0,
            ],
        ),
        (
            282,
            &[
                0x5f, 0x5e, 0xb8, 1, 0, 0, 0, 0x5b, 0x8b, 0xe5, 0x5d, 0xc2, 0x0c, 0,
            ],
        ),
    ] {
        at(recruit, offset, bytes)?;
    }
    for offset in [122, 151] {
        if call(recruit, WINDOWS[26].0, offset)? != 0xabc8c0 {
            return Err(Error("existing defender counting changed"));
        }
    }
    for offset in [174, 201, 226, 252] {
        if call(recruit, WINDOWS[26].0, offset)? != 0xabaeb0 {
            return Err(Error("defender recruiting passes changed"));
        }
    }
    at(recruit, 127, &[0x8b, 0x45, 0x0c, 0x85, 0xc0, 0x0f, 0x84])?;
    if WINDOWS[26]
        .0
        .wrapping_add(138)
        .wrapping_add(word(recruit, 134)?)
        != WINDOWS[26].0 + 282
    {
        return Err(Error("defender completion branch changed"));
    }
    at(recruit, 257, &[0x8b, 0x45, 0x0c, 0x85, 0xc0, 0x74, 0x12])?;

    at(orders, 0, &[0x8b, 0x40, 0x24, 0x3d])?;
    at(orders, 8, &[0x0f, 0x84])?;
    at(orders, 14, &[0x3d])?;
    at(orders, 19, &[0x0f, 0x84])?;
    for (end, offset) in [(14, 10), (25, 21)] {
        if WINDOWS[27]
            .0
            .wrapping_add(end)
            .wrapping_add(word(orders, offset)?)
            != 0xabb0e1
        {
            return Err(Error("defender order exclusions no longer skip the unit"));
        }
    }
    at(priority, 0, &[0x8d, 0x45, 0xf0, 0x8b, 0xce, 0x50])?;
    at(priority, 11, &[0xf3, 0x0f, 0x10, 0x05])?;
    at(priority, 19, &[0x0f, 0x2f, 0, 0x0f, 0x87])?;
    if call(priority, WINDOWS[28].0, 6)? != 0x635990
        || word(priority, 15)? != WINDOWS[29].0
        || WINDOWS[28]
            .0
            .wrapping_add(28)
            .wrapping_add(word(priority, 24)?)
            != 0xabb0e1
    {
        return Err(Error("defender health priority changed"));
    }
    for (window, end) in [(new_count, 0xabb127u32), (old_count, 0xabca54u32)] {
        at(window, 0, &[0x8b, 0x45, 0x0c, 0x83, 0])?;
        at(window, 6, &[0x74])?;
        let address = if end == 0xabb127 {
            WINDOWS[30].0
        } else {
            WINDOWS[31].0
        };
        let branch = *window.get(7).ok_or(Error("short defender count"))? as i8;
        if address
            .wrapping_add(8)
            .wrapping_add_signed(i32::from(branch))
            != end
        {
            return Err(Error("defender count completion target changed"));
        }
    }
    let step = i32::from(*new_count.get(5).ok_or(Error("short defender decrement"))? as i8);
    if old_count.get(5) != new_count.get(5) {
        return Err(Error("old and new defender count steps differ"));
    }
    at(attach, 0, &[0x8b, 0x5d, 0x08, 0x8b, 0xcb, 0x56])?;
    at(attach, 11, &[0x8b, 0x4d, 0xf4])?;
    if call(attach, WINDOWS[34].0, 6)? != 0xac9090 || call(attach, WINDOWS[34].0, 14)? != 0xabc600 {
        return Err(Error("defender unit association changed"));
    }
    let base = word(recruit, 52)?;
    let delta = i32::from(*recruit.get(60).ok_or(Error("short alternate type"))? as i8);
    let rules = DefenderRules {
        alternates: [
            (base.to_be_bytes(), word(recruit, 71)?.to_be_bytes()),
            (
                base.wrapping_add_signed(delta).to_be_bytes(),
                word(recruit, 64)?.to_be_bytes(),
            ),
            (
                word(recruit, 41)?.to_be_bytes(),
                word(recruit, 78)?.to_be_bytes(),
            ),
            (
                word(recruit, 86)?.to_be_bytes(),
                word(recruit, 112)?.to_be_bytes(),
            ),
            (
                word(recruit, 94)?.to_be_bytes(),
                word(recruit, 105)?.to_be_bytes(),
            ),
        ],
        count_step: step,
        health_priority: f32::from_bits(word(threshold, 0)?),
        excluded_orders: [word(orders, 4)? as i32, word(orders, 15)? as i32],
    };
    rules.validate()?;
    Ok(rules)
}

fn combat_query(w: &[&[u8]], first: u32, second: u32) -> Result<(), Error> {
    let [registration, handler, query, name, signature, initial] = w else {
        return Err(Error("incomplete combat-query windows"));
    };
    for (offset, target) in [(0, WINDOWS[21].0), (5, WINDOWS[20].0), (10, WINDOWS[18].0)] {
        at(registration, offset, &[0x68])?;
        if word(registration, offset + 1)? != target {
            return Err(Error("CaptainInCombat registration changed"));
        }
    }
    if call(registration, WINDOWS[17].0, 15)? != 0x8ccc80 {
        return Err(Error("CaptainInCombat registrar changed"));
    }
    at(name, 0, b"CaptainInCombat\0")?;
    at(signature, 0, b"(B)B\0")?;
    at(
        handler,
        0,
        &[
            0x55, 0x8b, 0xec, 0xa1, 0x10, 0x4a, 0x1b, 0x01, 0x85, 0xc0, 0x75, 0x0e, 0x6a, 0x57,
        ],
    )?;
    if call(handler, WINDOWS[18].0, 14)? != 0x43eca0
        || call(handler, WINDOWS[18].0, 53)? != 0x7fc7e0
        || call(handler, WINDOWS[18].0, 46)? != WINDOWS[19].0
        || call(handler, WINDOWS[18].0, 63)? != WINDOWS[19].0
    {
        return Err(Error("CaptainInCombat context dispatch changed"));
    }
    at(
        handler,
        19,
        &[
            0x83, 0xc4, 0x04, 0x33, 0xc0, 0x5d, 0xc3, 0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23,
            0x01, 0x83, 0xf8, 0xff, 0x75, 0x0e, 0xff, 0x75, 0x08, 0x33, 0xc0, 0x8b, 0xc8,
        ],
    )?;
    at(handler, 51, &[0x5d, 0xc3])?;
    at(handler, 58, &[0xff, 0x75, 0x08, 0x8b, 0xc8])?;
    at(handler, 68, &[0x5d, 0xc3])?;
    at(
        query,
        0,
        &[
            0x55, 0x8b, 0xec, 0x83, 0x7d, 0x08, 0, 0x56, 0x74, 0x08, 0x81, 0xc1,
        ],
    )?;
    at(query, 16, &[0xeb, 0x06, 0x81, 0xc1])?;
    if word(query, 12)? != first || word(query, 20)? != second {
        return Err(Error("combat selector differs from captain references"));
    }
    at(
        query,
        24,
        &[
            0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x04, 0x33, 0xf6, 0xeb, 0x07,
        ],
    )?;
    for offset in [38, 64, 92] {
        if call(query, WINDOWS[19].0, offset)? != 0x7fc7e0 {
            return Err(Error("combat target reference resolver changed"));
        }
    }
    at(query, 43, &[0x8b, 0xf0, 0x8b, 0x96])?;
    at(query, 51, &[0x8d, 0x8e])?;
    at(query, 57, &[0x23, 0x11, 0x83, 0xfa, 0xff, 0x74, 0x09])?;
    at(query, 69, &[0x85, 0xc0, 0x75, 0x1c, 0x8b, 0x86])?;
    at(query, 79, &[0x8d, 0x8e])?;
    at(query, 85, &[0x23, 0x01, 0x83, 0xf8, 0xff, 0x74, 0x13])?;
    at(
        query,
        97,
        &[
            0x85, 0xc0, 0x74, 0x0a, 0xb8, 1, 0, 0, 0, 0x5e, 0x5d, 0xc2, 4, 0, 0x33, 0xc0, 0x5e,
            0x5d, 0xc2, 4, 0,
        ],
    )?;
    let a = word(query, 53)?;
    let b = word(query, 81)?;
    let a_end = a
        .checked_add(8)
        .ok_or(Error("combat target pair overflow"))?;
    let b_end = b
        .checked_add(8)
        .ok_or(Error("combat target pair overflow"))?;
    if a == 0
        || b == 0
        || a < b_end && b < a_end
        || word(query, 47)? != a_end - 4
        || word(query, 75)? != b_end - 4
    {
        return Err(Error("combat target pairs overlap or differ"));
    }
    for (offset, field) in [(0, a_end - 4), (10, a), (20, b_end - 4), (30, b)] {
        at(initial, offset, &[0xc7, 0x83])?;
        if word(initial, offset + 2)? != field || word(initial, offset + 6)? != u32::MAX {
            return Err(Error(
                "initial combat targets differ from queried references",
            ));
        }
    }
    Ok(())
}

// Non-stock synthetic operands prove decode reads operands, not constants. The
// fixture identity [0x42; 32] is shared with GatheringPolicy::fixture() so
// cross-policy admission tests exercise a valid synthetic source identity.
#[cfg(any(test, feature = "test-fixtures"))]
const FIRST_MASK: u8 = 0x10;
#[cfg(any(test, feature = "test-fixtures"))]
const SECOND_MASK: u8 = 0x20;
#[cfg(any(test, feature = "test-fixtures"))]
const FIRST_SLOT: u32 = 0x330;
#[cfg(any(test, feature = "test-fixtures"))]
const SECOND_SLOT: u32 = 0x340;

#[cfg(any(test, feature = "test-fixtures"))]
fn synthetic_windows() -> Vec<Vec<u8>> {
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
    fn jmp(w: &mut [Vec<u8>], i: usize, at: usize, target: u32) {
        put(w, i, at, &[0xe9]);
        dword(
            w,
            i,
            at + 1,
            target.wrapping_sub(WINDOWS[i].0 + at as u32 + 5),
        );
    }
    fn move_pair(w: &mut [Vec<u8>], i: usize, slot: u32) {
        put(w, i, 0, &[0x89, 0x87]);
        dword(w, i, 2, slot);
        put(w, i, 6, &[0x8b, 0x41, 0x18]);
        put(w, i, 9, &[0x89, 0x87]);
        dword(w, i, 11, slot + 4);
    }
    let mut w = WINDOWS
        .iter()
        .map(|(_, len, _)| vec![0; *len])
        .collect::<Vec<_>>();

    // SetCaptainHome registration (sig, name, handler, registrar).
    put(&mut w, 0, 0, &[0x68]);
    dword(&mut w, 0, 1, 0xe6f5a8);
    put(&mut w, 0, 5, &[0x68]);
    dword(&mut w, 0, 6, 0xf15d34);
    put(&mut w, 0, 10, &[0x68]);
    dword(&mut w, 0, 11, 0xab6f10);
    call(&mut w, 0, 15, 0x8ccc80);

    // SetCaptainHome handler entry.
    put(&mut w, 1, 0, &[0x55, 0x8b, 0xec]);
    put(&mut w, 1, 3, &[0xa1]);
    dword(&mut w, 1, 4, 0x11b4a10);
    call(&mut w, 1, 50, 0xac26d0);
    call(&mut w, 1, 73, 0xac26d0);

    // Selector.
    put(&mut w, 2, 4, &[0xf6, 0x45, 0x08, FIRST_MASK]);
    put(&mut w, 2, 13, &[0x74, 0x29]); // first test -> second test (56)
    put(&mut w, 2, 15, &[0x8d, 0x88]);
    dword(&mut w, 2, 17, FIRST_SLOT);
    call(&mut w, 2, 31, 0x7fc7e0);
    call(&mut w, 2, 48, 0xad0630);
    put(&mut w, 2, 56, &[0xf6, 0x45, 0x08, SECOND_MASK]);
    put(&mut w, 2, 60, &[0x74, 0x36]); // second test -> end (116)
    put(&mut w, 2, 62, &[0x8d, 0x88]);
    dword(&mut w, 2, 64, SECOND_SLOT);
    call(&mut w, 2, 78, 0x7fc7e0);
    call(&mut w, 2, 95, 0xad0630);

    // CreateCaptains registration.
    put(&mut w, 3, 0, &[0x68]);
    dword(&mut w, 3, 1, 0xf15d24);
    put(&mut w, 3, 5, &[0x68]);
    dword(&mut w, 3, 6, 0xab4750);
    call(&mut w, 3, 10, 0x8ccc80);

    // CreateCaptains handler -> main-work join.
    put(&mut w, 4, 0, &[0x33, 0xc0, 0x8b, 0xc8]);
    jmp(&mut w, 4, 4, 0xabd160);

    // Create home-write pairs (selector slots on both ends).
    move_pair(&mut w, 5, FIRST_SLOT);
    move_pair(&mut w, 6, SECOND_SLOT);

    for (index, signature, name, handler) in [
        (7, WINDOWS[15].0, WINDOWS[13].0, 0xab3da0),
        (8, WINDOWS[16].0, WINDOWS[14].0, 0xab3e90),
    ] {
        for (offset, target) in [(0, signature), (5, name), (10, handler)] {
            put(&mut w, index, offset, &[0x68]);
            dword(&mut w, index, offset + 1, target);
        }
        call(&mut w, index, 15, 0x8ccc80);
    }
    call(&mut w, 9, 0, WINDOWS[11].0);
    call(&mut w, 9, 12, 0x7fc7e0);
    put(&mut w, 9, 17, &[0x8b, 0xc8]);
    call(&mut w, 9, 19, WINDOWS[11].0);
    for (load, ret) in [(5, 11), (24, 30)] {
        put(&mut w, 9, load, &[0x8b, 0x80]);
        dword(&mut w, 9, load + 2, 0x440);
        put(&mut w, 9, ret, &[0xc3]);
    }
    call(&mut w, 10, 0, 0x7fc7e0);
    put(&mut w, 10, 5, &[0x8b, 0xc8]);
    call(&mut w, 10, 7, WINDOWS[11].0);
    put(&mut w, 10, 12, &[0x33, 0xc9, 0x39, 0x88]);
    dword(&mut w, 10, 16, 0x440);
    put(&mut w, 10, 20, &[0x0f, 0x94, 0xc1, 0x8b, 0xc1, 0xc3]);
    put(&mut w, 11, 0, &[0x8b, 0x81]);
    dword(&mut w, 11, 2, FIRST_SLOT + 4);
    put(&mut w, 11, 6, &[0x81, 0xc1]);
    dword(&mut w, 11, 8, FIRST_SLOT);
    put(
        &mut w,
        11,
        12,
        &[0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x03, 0x33, 0xc0, 0xc3],
    );
    jmp(&mut w, 11, 22, 0x7fc7e0);
    put(&mut w, 12, 0, &[0x01, 0xb3]);
    dword(&mut w, 12, 2, 0x440);
    for (index, bytes) in [
        (13, b"CaptainGroupSize\0".as_slice()),
        (14, b"CaptainIsEmpty\0".as_slice()),
        (15, b"()I\0".as_slice()),
        (16, b"()B\0".as_slice()),
    ] {
        put(&mut w, index, 0, bytes);
    }
    for (offset, target) in [(0, WINDOWS[21].0), (5, WINDOWS[20].0), (10, WINDOWS[18].0)] {
        put(&mut w, 17, offset, &[0x68]);
        dword(&mut w, 17, offset + 1, target);
    }
    call(&mut w, 17, 15, 0x8ccc80);
    put(&mut w, 20, 0, b"CaptainInCombat\0");
    put(&mut w, 21, 0, b"(B)B\0");
    put(
        &mut w,
        18,
        0,
        &[
            0x55, 0x8b, 0xec, 0xa1, 0x10, 0x4a, 0x1b, 0x01, 0x85, 0xc0, 0x75, 0x0e, 0x6a, 0x57,
        ],
    );
    call(&mut w, 18, 14, 0x43eca0);
    put(
        &mut w,
        18,
        19,
        &[
            0x83, 0xc4, 0x04, 0x33, 0xc0, 0x5d, 0xc3, 0x8d, 0x48, 0x2c, 0x8b, 0x41, 0x04, 0x23,
            0x01, 0x83, 0xf8, 0xff, 0x75, 0x0e, 0xff, 0x75, 0x08, 0x33, 0xc0, 0x8b, 0xc8,
        ],
    );
    call(&mut w, 18, 46, WINDOWS[19].0);
    put(&mut w, 18, 51, &[0x5d, 0xc3]);
    call(&mut w, 18, 53, 0x7fc7e0);
    put(&mut w, 18, 58, &[0xff, 0x75, 0x08, 0x8b, 0xc8]);
    call(&mut w, 18, 63, WINDOWS[19].0);
    put(&mut w, 18, 68, &[0x5d, 0xc3]);
    put(
        &mut w,
        19,
        0,
        &[
            0x55, 0x8b, 0xec, 0x83, 0x7d, 0x08, 0, 0x56, 0x74, 0x08, 0x81, 0xc1,
        ],
    );
    dword(&mut w, 19, 12, FIRST_SLOT);
    put(&mut w, 19, 16, &[0xeb, 0x06, 0x81, 0xc1]);
    dword(&mut w, 19, 20, SECOND_SLOT);
    put(
        &mut w,
        19,
        24,
        &[
            0x8b, 0x41, 0x04, 0x23, 0x01, 0x83, 0xf8, 0xff, 0x75, 0x04, 0x33, 0xf6, 0xeb, 0x07,
        ],
    );
    for offset in [38, 64, 92] {
        call(&mut w, 19, offset, 0x7fc7e0);
    }
    put(&mut w, 19, 43, &[0x8b, 0xf0, 0x8b, 0x96]);
    dword(&mut w, 19, 47, 0x314);
    put(&mut w, 19, 51, &[0x8d, 0x8e]);
    dword(&mut w, 19, 53, 0x310);
    put(&mut w, 19, 57, &[0x23, 0x11, 0x83, 0xfa, 0xff, 0x74, 0x09]);
    put(&mut w, 19, 69, &[0x85, 0xc0, 0x75, 0x1c, 0x8b, 0x86]);
    dword(&mut w, 19, 75, 0x32c);
    put(&mut w, 19, 79, &[0x8d, 0x8e]);
    dword(&mut w, 19, 81, 0x328);
    put(&mut w, 19, 85, &[0x23, 0x01, 0x83, 0xf8, 0xff, 0x74, 0x13]);
    put(
        &mut w,
        19,
        97,
        &[
            0x85, 0xc0, 0x74, 0x0a, 0xb8, 1, 0, 0, 0, 0x5e, 0x5d, 0xc2, 4, 0, 0x33, 0xc0, 0x5e,
            0x5d, 0xc2, 4, 0,
        ],
    );
    for (offset, field) in [(0, 0x314), (10, 0x310), (20, 0x32c), (30, 0x328)] {
        put(&mut w, 22, offset, &[0xc7, 0x83]);
        dword(&mut w, 22, offset + 2, field);
        dword(&mut w, 22, offset + 6, u32::MAX);
    }

    for (offset, target) in [(0, WINDOWS[33].0), (5, WINDOWS[32].0), (10, WINDOWS[24].0)] {
        put(&mut w, 23, offset, &[0x68]);
        dword(&mut w, 23, offset + 1, target);
    }
    call(&mut w, 23, 15, 0x8ccc80);
    let context = w[18][..14].to_vec();
    put(&mut w, 24, 0, &context);
    call(&mut w, 24, 14, 0x43eca0);
    put(
        &mut w,
        24,
        19,
        &[
            0x83, 0xc4, 4, 0x33, 0xc0, 0x5d, 0xc3, 0x8d, 0x48, 0x2c, 0x8b, 0x41, 4, 0x23, 1, 0x83,
            0xf8, 0xff, 0x75, 0x11, 0xff, 0x75, 0x0c, 0x33, 0xc0, 0xff, 0x75, 8, 0x8b, 0xc8,
        ],
    );
    call(&mut w, 24, 49, WINDOWS[25].0);
    put(&mut w, 24, 54, &[0x5d, 0xc3]);
    call(&mut w, 24, 56, 0x7fc7e0);
    put(
        &mut w,
        24,
        61,
        &[0xff, 0x75, 0x0c, 0x8b, 0xc8, 0xff, 0x75, 8],
    );
    call(&mut w, 24, 69, WINDOWS[25].0);
    put(&mut w, 24, 74, &[0x5d, 0xc3]);
    put(
        &mut w,
        25,
        0,
        &[0x55, 0x8b, 0xec, 0x56, 0x8b, 0xf1, 0x8b, 0x86],
    );
    dword(&mut w, 25, 8, SECOND_SLOT + 4);
    put(&mut w, 25, 12, &[0x8d, 0x8e]);
    dword(&mut w, 25, 14, SECOND_SLOT);
    put(
        &mut w,
        25,
        18,
        &[
            0x23, 1, 0x83, 0xf8, 0xff, 0x75, 0x15, 0xff, 0x75, 0x0c, 0x33, 0xc0, 0x8b, 0xce, 0xff,
            0x75, 8, 0x50,
        ],
    );
    call(&mut w, 25, 36, WINDOWS[26].0);
    put(&mut w, 25, 41, &[0x5e, 0x5d, 0xc2, 8, 0]);
    call(&mut w, 25, 46, 0x7fc7e0);
    put(
        &mut w,
        25,
        51,
        &[0xff, 0x75, 0x0c, 0x8b, 0xce, 0xff, 0x75, 8, 0x50],
    );
    call(&mut w, 25, 60, WINDOWS[26].0);
    put(&mut w, 25, 65, &[0x5e, 0x5d, 0xc2, 8, 0]);
    put(
        &mut w,
        26,
        0,
        &[
            0x55, 0x8b, 0xec, 0x51, 0x53, 0x8b, 0x5d, 8, 0x89, 0x4d, 0xfc, 0x85, 0xdb, 0x75, 0x13,
            0x6a, 0x57,
        ],
    );
    call(&mut w, 26, 17, 0x43eca0);
    put(
        &mut w,
        26,
        22,
        &[
            0x83, 0xc4, 4, 0x33, 0xc0, 0x5b, 0x8b, 0xe5, 0x5d, 0xc2, 0x0c, 0, 0x56, 0x57, 0x8b,
            0x7d, 0x10,
        ],
    );
    for (offset, bytes) in [
        (39, &[0x81, 0xff][..]),
        (45, &[0x77, 0x25, 0x74, 0x1c, 0x8b, 0xc7, 0x2d]),
        (56, &[0x74, 0x0c, 0x83, 0xe8]),
        (61, &[0x75, 0x25, 0xbe]),
        (68, &[0xeb, 0x2e, 0xbe]),
        (75, &[0xeb, 0x27, 0xbe]),
        (82, &[0xeb, 0x20, 0x81, 0xff]),
        (90, &[0x74, 0x13, 0x81, 0xff]),
        (98, &[0x74, 4, 0x33, 0xf6, 0xeb, 0x0c, 0xbe]),
        (109, &[0xeb, 5, 0xbe]),
        (116, &[0x57, 0x8d, 0x45, 0x0c, 0x50, 0x53]),
        (127, &[0x8b, 0x45, 0x0c, 0x85, 0xc0, 0x0f, 0x84]),
        (257, &[0x8b, 0x45, 0x0c, 0x85, 0xc0, 0x74, 0x12]),
        (
            271,
            &[
                0x33, 0xc0, 0x5f, 0x5e, 0x5b, 0x8b, 0xe5, 0x5d, 0xc2, 0x0c, 0,
            ],
        ),
        (
            282,
            &[
                0x5f, 0x5e, 0xb8, 1, 0, 0, 0, 0x5b, 0x8b, 0xe5, 0x5d, 0xc2, 0x0c, 0,
            ],
        ),
    ] {
        put(&mut w, 26, offset, bytes);
    }
    for (offset, code) in [
        (41, b"c001"),
        (52, b"a001"),
        (64, b"b002"),
        (71, b"b001"),
        (78, b"c002"),
        (86, b"d001"),
        (94, b"e001"),
        (105, b"e002"),
        (112, b"d002"),
    ] {
        dword(&mut w, 26, offset, u32::from_be_bytes(*code));
    }
    put(&mut w, 26, 60, &[1]);
    for offset in [122, 151] {
        call(&mut w, 26, offset, 0xabc8c0);
    }
    for offset in [174, 201, 226, 252] {
        call(&mut w, 26, offset, 0xabaeb0);
    }
    dword(&mut w, 26, 134, 282 - 138);
    put(&mut w, 27, 0, &[0x8b, 0x40, 0x24, 0x3d]);
    dword(&mut w, 27, 4, 1701);
    put(&mut w, 27, 8, &[0x0f, 0x84]);
    dword(&mut w, 27, 10, 0xabb0e1 - (WINDOWS[27].0 + 14));
    put(&mut w, 27, 14, &[0x3d]);
    dword(&mut w, 27, 15, 1702);
    put(&mut w, 27, 19, &[0x0f, 0x84]);
    dword(&mut w, 27, 21, 0xabb0e1 - (WINDOWS[27].0 + 25));
    put(&mut w, 28, 0, &[0x8d, 0x45, 0xf0, 0x8b, 0xce, 0x50]);
    call(&mut w, 28, 6, 0x635990);
    put(&mut w, 28, 11, &[0xf3, 0x0f, 0x10, 5]);
    dword(&mut w, 28, 15, WINDOWS[29].0);
    put(&mut w, 28, 19, &[0x0f, 0x2f, 0, 0x0f, 0x87]);
    dword(&mut w, 28, 24, 0xabb0e1 - (WINDOWS[28].0 + 28));
    dword(&mut w, 29, 0, 0.25f32.to_bits());
    for (i, target) in [(30, 0xabb127u32), (31, 0xabca54)] {
        put(&mut w, i, 0, &[0x8b, 0x45, 0x0c, 0x83, 0, 0xff, 0x74]);
        put(&mut w, i, 7, &[(target - (WINDOWS[i].0 + 8)) as u8]);
    }
    put(&mut w, 32, 0, b"AddDefenders\0");
    put(&mut w, 33, 0, b"(II)B\0");
    put(&mut w, 34, 0, &[0x8b, 0x5d, 8, 0x8b, 0xcb, 0x56]);
    call(&mut w, 34, 6, 0xac9090);
    put(&mut w, 34, 11, &[0x8b, 0x4d, 0xf4]);
    call(&mut w, 34, 14, 0xabc600);

    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defender_operands_drive_aliases_priority_and_counting() {
        let source = synthetic_windows();
        let decode = |source: &[Vec<u8>]| {
            CaptainPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0x42; 32],
            )
        };
        let rules = decode(&source).unwrap().defenders;
        assert_eq!(
            rules.alternates,
            [
                (*b"a001", *b"b001"),
                (*b"a002", *b"b002"),
                (*b"c001", *b"c002"),
                (*b"d001", *b"d002"),
                (*b"e001", *b"e002")
            ]
        );
        assert_eq!(rules.count_step, -1);
        assert_eq!(rules.health_priority, 0.25);
        assert_eq!(rules.excluded_orders, [1701, 1702]);
        let mut changed = source.clone();
        changed[26][52..56].copy_from_slice(&u32::from_be_bytes(*b"z003").to_le_bytes());
        changed[26][60] = 2;
        changed[29].copy_from_slice(&0.75f32.to_le_bytes());
        changed[30][5] = (-2i8) as u8;
        changed[31][5] = (-2i8) as u8;
        let rules = decode(&changed).unwrap().defenders;
        assert_eq!(rules.alternates[0].0, *b"z003");
        assert_eq!(rules.alternates[1].0, *b"z005");
        assert_eq!(rules.count_step, -2);
        assert_eq!(rules.health_priority, 0.75);
        for (i, at) in [
            (23, 1),
            (23, 16),
            (24, 10),
            (24, 37),
            (24, 50),
            (24, 75),
            (25, 8),
            (25, 14),
            (25, 24),
            (25, 37),
            (26, 14),
            (26, 40),
            (26, 46),
            (26, 123),
            (26, 134),
            (26, 175),
            (26, 263),
            (26, 285),
            (27, 10),
            (27, 21),
            (28, 7),
            (28, 15),
            (28, 24),
            (30, 7),
            (31, 7),
            (32, 12),
            (33, 5),
            (34, 7),
        ] {
            let mut mutant = source.clone();
            mutant[i][at] ^= 1;
            assert!(decode(&mutant).is_err(), "window {i} byte {at}");
        }
        for value in [f32::NAN, -0.1, 1.1] {
            let mut mutant = source.clone();
            mutant[29].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&mutant).is_err());
        }
        for step in [0, 1] {
            let mut mutant = source.clone();
            mutant[30][5] = step;
            mutant[31][5] = step;
            assert!(decode(&mutant).is_err());
        }
    }

    #[test]
    fn combat_query_joins_selector_targets_and_initial_empty_references() {
        let source = synthetic_windows();
        let decode = |source: &[Vec<u8>]| {
            CaptainPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32],
            )
        };
        assert!(decode(&source).is_ok());
        for (index, offset) in [
            (17, 1),
            (17, 16),
            (18, 10),
            (18, 39),
            (18, 47),
            (18, 68),
            (19, 9),
            (19, 12),
            (19, 20),
            (19, 47),
            (19, 63),
            (19, 72),
            (19, 75),
            (19, 91),
            (19, 100),
            (19, 102),
            (20, 15),
            (21, 4),
            (22, 6),
        ] {
            let mut mutant = source.clone();
            mutant[index][offset] ^= 1;
            assert!(decode(&mutant).is_err(), "window {index} byte {offset}");
        }
        for (a, b) in [(0x410u32, 0x428u32), (0x428, 0x410)] {
            let mut changed = source.clone();
            for (offset, value) in [(47, a + 4), (53, a), (75, b + 4), (81, b)] {
                changed[19][offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            for (offset, value) in [(2, a + 4), (12, a), (22, b + 4), (32, b)] {
                changed[22][offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            assert!(decode(&changed).is_ok());
            changed[19][81..85].copy_from_slice(&a.to_le_bytes());
            changed[19][75..79].copy_from_slice(&(a + 4).to_le_bytes());
            changed[22][22..26].copy_from_slice(&(a + 4).to_le_bytes());
            changed[22][32..36].copy_from_slice(&a.to_le_bytes());
            assert!(decode(&changed).is_err());
        }
    }

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
        assert!(CaptainPolicy::read(b"MZunsupported", budget).is_err());
    }

    #[test]
    fn synthetic_operands_drive_the_policy() {
        let policy = CaptainPolicy::fixture();
        assert_eq!(policy.first_selector_mask, FIRST_MASK);
        assert_eq!(policy.second_selector_mask, SECOND_MASK);
        assert_ne!(policy.first_selector_mask, policy.second_selector_mask);
        assert_eq!(policy.image_sha256, [0x42; 32]);
    }

    #[test]
    fn synthetic_nonstock_masks_discriminate() {
        // F6 TEST imm8 masks are unsigned u8; 0x80 is a valid single bit.
        let mut source = synthetic_windows();
        source[2][7] = 0x80;
        let policy = CaptainPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("accepted 0x80 first mask");
        assert_eq!(policy.first_selector_mask, 0x80);

        source = synthetic_windows();
        source[2][7] = 0x40;
        let policy = CaptainPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("changed first mask operand");
        assert_eq!(policy.first_selector_mask, 0x40);

        source = synthetic_windows();
        source[2][59] = 0x80;
        let policy = CaptainPolicy::decode(
            &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            [0; 32],
        )
        .expect("accepted distinct 0x80 second mask");
        assert_eq!(policy.second_selector_mask, 0x80);

        source = synthetic_windows();
        source[2][59] = FIRST_MASK;
        assert!(
            CaptainPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0; 32],
            )
            .is_err(),
            "non-distinct second selector mask should refuse"
        );
    }

    #[test]
    fn malformed_branch_or_slot_relationships_refuse() {
        let invalid: &[fn(&mut Vec<Vec<u8>>)] = &[
            |w| w[0][0] ^= 1,
            |w| w[1][0] ^= 1,
            |w| w[2][4] ^= 1,
            |w| w[3][0] ^= 1,
            |w| w[0][11] ^= 0x10, // SetCaptainHome handler change
            |w| w[3][6] ^= 0x10,  // CreateCaptains handler change
            |w| w[1][4] ^= 0x10,  // AI context global change
            |w| w[2][14] ^= 1,    // first JE skip target changed
            |w| w[2][61] ^= 1,    // second JE skip target changed
            |w| w[2][64] = FIRST_SLOT.to_le_bytes()[0], // second == first
            |w| w[2][7] = 0x00,   // non-single-bit first mask
            |w| w[2][59] = 0x00,  // non-single-bit second mask
            |w| w[5][2] ^= 0x10,  // create first slot no longer matches selector
            |w| w[6][11] ^= 0x10, // create second slot second word differs
        ];
        for (index, mutate) in invalid.iter().enumerate() {
            let mut source = synthetic_windows();
            mutate(&mut source);
            assert!(
                CaptainPolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32],
                )
                .is_err(),
                "case {index} should refuse"
            );
        }
    }

    #[test]
    fn overflowing_or_overlapping_reference_pairs_refuse() {
        for (first, second) in [
            (u32::MAX, SECOND_SLOT),
            (FIRST_SLOT, u32::MAX),
            (FIRST_SLOT, FIRST_SLOT + 4),
            (FIRST_SLOT, FIRST_SLOT + 7),
        ] {
            let mut source = synthetic_windows();
            source[2][17..21].copy_from_slice(&first.to_le_bytes());
            source[2][64..68].copy_from_slice(&second.to_le_bytes());
            for (index, slot) in [(5, first), (6, second)] {
                source[index][2..6].copy_from_slice(&slot.to_le_bytes());
                source[index][11..15].copy_from_slice(&slot.wrapping_add(4).to_le_bytes());
            }
            assert!(
                CaptainPolicy::decode(
                    &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                    [0; 32],
                )
                .is_err()
            );
        }
    }

    #[test]
    fn query_counter_and_first_reference_follow_source_operands() {
        let mut source = synthetic_windows();
        let slot = 0x510u32;
        for (index, offset, value) in [
            (2, 17, slot),
            (5, 2, slot),
            (5, 11, slot + 4),
            (11, 2, slot + 4),
            (11, 8, slot),
            (19, 12, slot),
            (9, 7, 0x620),
            (9, 26, 0x620),
            (10, 16, 0x620),
            (12, 2, 0x620),
        ] {
            source[index][offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        let decode = |source: &[Vec<u8>]| {
            CaptainPolicy::decode(
                &source.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                [0x42; 32],
            )
        };
        assert!(decode(&source).is_ok());
        for (index, offset) in [
            (7, 11),
            (8, 16),
            (9, 1),
            (9, 7),
            (9, 26),
            (10, 16),
            (10, 21),
            (11, 2),
            (11, 8),
            (11, 18),
            (12, 2),
            (13, 16),
            (14, 14),
            (15, 3),
            (16, 3),
        ] {
            let mut changed = source.clone();
            changed[index][offset] ^= 1;
            assert!(decode(&changed).is_err(), "window {index} offset {offset}");
        }
    }

    #[test]
    fn every_truncated_window_refuses_without_indexing_past_its_end() {
        let source = synthetic_windows();
        for (index, window) in source.iter().enumerate() {
            for length in 0..window.len() {
                let mut windows = source.iter().map(Vec::as_slice).collect::<Vec<_>>();
                windows[index] = &window[..length];
                assert!(
                    CaptainPolicy::decode(&windows, [0x42; 32]).is_err(),
                    "window {index} length {length}"
                );
            }
        }
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local client"]
    fn reviewed_original_image_decodes() {
        let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").expect("PCX_WAR3_EXE required"))
            .expect("read reviewed client");
        let budget = RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: WINDOWS.iter().map(|(_, len, _)| len).sum(),
            sections: 96,
        };
        let policy = CaptainPolicy::read(&image, budget).expect("selected captain-policy operands");
        assert_eq!(policy.image_sha256, REVIEWED_IMAGE);
        assert_eq!(policy.first_selector_mask, 0x01);
        assert_eq!(policy.second_selector_mask, 0x02);
        assert_ne!(policy.first_selector_mask, policy.second_selector_mask);
        assert_eq!(policy.defenders.count_step, -1);
        assert_eq!(policy.defenders.health_priority, 0.5);
        assert_eq!(policy.defenders.alternates[0], (*b"edoc", *b"edcm"));
        assert_eq!(policy.defenders.alternates[1], (*b"edot", *b"edtm"));
    }
}
