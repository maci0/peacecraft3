//! Minimap side buttons and the idle-worker button from the reviewed client.
//!
//! The GameUI constructor builds five buttons (signal, terrain, ally colours, creeps,
//! formation) anchored TOPLEFT to the console parent's BOTTOMLEFT, and `CIdleButton`
//! anchors BOTTOMLEFT to the same corner. Each button's art keys, tooltip keys and
//! initial option value are read from the client. Button actions and which units count
//! as idle workers are not decoded here.
use crate::{
    inventory_layout::Rect,
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
    unit_panel_layout::Reader,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Signal,
    Terrain,
    Ally,
    Creep,
    Formation,
}

/// Skin keys of the three button states.
#[derive(Clone, Debug)]
pub struct StateArt {
    pub enabled: String,
    pub pushed: String,
    pub disabled: String,
}

#[derive(Clone, Debug)]
pub struct Button {
    pub control: Control,
    /// Logical frame coordinates, positive Y upwards.
    pub rect: Rect,
    /// `GlobalStrings` keys of the tooltip title and description.
    pub tip_key: String,
    pub uber_key: String,
    /// Art by option value. Terrain, creep and formation have values 0 and 1; ally colours
    /// has 0 (off), 1 (inactive) and 2 (active); the signal button has one entry.
    pub art: Vec<StateArt>,
    /// The option value the client starts with.
    pub initial: u8,
}

#[derive(Clone, Debug)]
pub struct Idle {
    /// Native BOTTOMLEFT attached to the console parent's BOTTOMLEFT.
    pub rect: Rect,
    pub art_key: String,
    pub tip_key: String,
    pub desc_key: String,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// Logical frame coordinates, positive Y upwards.
    pub viewport: [f32; 2],
    /// Top to bottom.
    pub buttons: Vec<Button>,
    pub idle: Idle,
}

// Reviewed 1.29.2.9231 constructor operands.
const SET_POINT: u32 = 0x873a10;
const WIDTH_SETTER: u32 = 0x873b40;
const HEIGHT_SETTER: u32 = 0x873830;
const WIDE_MAPPING: u32 = 0xab28d0;
const GLOBAL_STRING: u32 = 0x895a00;
/// Option table: 16-byte records whose first word is the name and last word the default.
const OPTION_TABLE: u32 = 0xe68bd0;
const OPTION_STRIDE: u32 = 16;
const OPTION_DEFAULT: u32 = 12;

/// Instruction sites of one button's size, offset, anchor and creation.
struct Site {
    control: Control,
    /// `mov [esp], height` before the height setter call.
    height: u32,
    /// `mov [esp+4], y` of the offset pair.
    offset: u32,
    /// `mov [ebx/esi+field], eax` storing the created button.
    store: u32,
    field: u8,
    /// Tooltip title and description `push` sites in the hover handler.
    tip: u32,
    uber: u32,
    /// The `cmp ebx, [esi+field]` selecting this button in the hover handler.
    hover: u32,
}

const SITES: [Site; 5] = [
    Site {
        control: Control::Signal,
        height: 0x5ed239,
        offset: 0x5ed263,
        store: 0x5ed21a,
        field: 0x48,
        tip: 0x5efd6e,
        uber: 0x5efd8f,
        hover: 0x5efd5a,
    },
    Site {
        control: Control::Terrain,
        height: 0x5ed34b,
        offset: 0x5ed375,
        store: 0x5ed32c,
        field: 0x4c,
        tip: 0x5efdad,
        uber: 0x5efdce,
        hover: 0x5efd99,
    },
    Site {
        control: Control::Ally,
        height: 0x5ed406,
        offset: 0x5ed430,
        store: 0x5ed3e7,
        field: 0x50,
        tip: 0x5efdec,
        uber: 0x5efe0d,
        hover: 0x5efdd8,
    },
    Site {
        control: Control::Creep,
        height: 0x5ed4c1,
        offset: 0x5ed4eb,
        store: 0x5ed4a2,
        field: 0x54,
        tip: 0x5efe28,
        uber: 0x5efe45,
        hover: 0x5efe14,
    },
    Site {
        control: Control::Formation,
        height: 0x5ed57c,
        offset: 0x5ed5a6,
        store: 0x5ed55d,
        field: 0x58,
        tip: 0x5efe60,
        uber: 0x5efe7d,
        hover: 0x5efe4c,
    },
];

/// `push` sites of the three state keys (enabled, pushed, disabled) per art set.
type Keys = [u32; 3];

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: String| format!("minimap buttons {}: {reason}", origin.source);
        if image.len() > 32 << 20
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
            || sha256(image) != REVIEWED_IMAGE
        {
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
        Self::decode(origin.clone(), REVIEWED_IMAGE, |va, len, code| {
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
        read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut r = Reader(read);
        let viewport = [
            r.referenced(0x640ea8, &[0xf3, 0x0f, 0x10, 5])?,
            r.referenced(0x640e96, &[0xf3, 0x0f, 0x10, 0x1d])?,
        ];
        if viewport.iter().any(|v| *v <= 0.) {
            return Err("invalid viewport".into());
        }
        let mut buttons = Vec::new();
        for site in &SITES {
            buttons.push(button(&mut r, site)?);
        }
        let idle = idle(&mut r)?;
        for rect in buttons.iter().map(|b| b.rect).chain([idle.rect]) {
            if rect.width <= 0.
                || rect.height <= 0.
                || rect.x < 0.
                || rect.y < 0.
                || rect.x + rect.width > viewport[0]
                || rect.y + rect.height > viewport[1]
            {
                return Err("button rectangle leaves the viewport".into());
            }
        }
        Ok(Self {
            origin,
            image_sha256,
            viewport,
            buttons,
            idle,
        })
    }
}

fn button<F: FnMut(u32, usize, bool) -> Result<Vec<u8>, String>>(
    r: &mut Reader<F>,
    site: &Site,
) -> Result<Button, String> {
    let at = |r: &mut Reader<F>, va: u32, target: u32| {
        if r.call_target(va)? != target {
            return Err(format!("unexpected call target at {va:#x}"));
        }
        Ok(())
    };
    // Height, width, offset pair, TOPLEFT to the console-parent field's BOTTOMLEFT, creation.
    let height_va = site.height;
    let width_va = height_va + 0x13;
    let anchor = site.offset + 0xf;
    at(r, height_va + 7, HEIGHT_SETTER)?;
    at(r, width_va + 7, WIDTH_SETTER)?;
    at(r, anchor + 10, SET_POINT)?;
    if r.bytes(anchor, 10, true)? != [0x6a, 6, 0xff, 0xb6, 0x68, 4, 0, 0, 0x6a, 0] {
        return Err(format!("{:?} anchor pair", site.control));
    }
    if r.bytes(site.store, 6, true)? != [0x89, 0x86, site.field, 4, 0, 0] {
        return Err(format!("{:?} field store", site.control));
    }
    if r.word(site.hover, &[0x3b, 0x9e])? != 0x400 + u32::from(site.field) {
        return Err(format!("{:?} hover field", site.control));
    }
    let height = r.number(height_va, &[0xc7, 4, 0x24])?;
    let width = r.number(width_va, &[0xc7, 4, 0x24])?;
    let y = r.number(site.offset, &[0xc7, 0x44, 0x24, 4])?;
    let x = r.number(site.offset + 8, &[0xc7, 4, 0x24])?;
    let sets: Vec<[String; 3]> = match site.control {
        Control::Signal => vec![keys(r, [0x5ed283, 0x5ed2a0, 0x5ed2bd])?],
        Control::Terrain => {
            if r.bytes(0x5f462b, 7, true)? != [0x83, 0xb8, 0x18, 8, 0, 0, 0] {
                return Err("terrain flag comparison".into());
            }
            vec![
                keys(r, [0x5f4675, 0x5f4692, 0x5f46af])?,
                keys(r, [0x5f4634, 0x5f4651, 0x5f466e])?,
            ]
        }
        Control::Creep => {
            if r.bytes(0x5f35fb, 7, true)? != [0x83, 0xb8, 0x1c, 8, 0, 0, 0] {
                return Err("creep flag comparison".into());
            }
            vec![
                keys(r, [0x5f3645, 0x5f3662, 0x5f367f])?,
                keys(r, [0x5f3604, 0x5f3621, 0x5f363e])?,
            ]
        }
        Control::Formation => {
            if r.bytes(0x5f36b0, 2, true)? != [0x6a, 0x2d]
                || r.bytes(0x5f36b9, 4, true)? != [0x83, 0x7d, 0xfc, 0]
            {
                return Err("formation option comparison".into());
            }
            vec![
                keys(r, [0x5f3702, 0x5f371f, 0x5f373c])?,
                keys(r, [0x5f36c1, 0x5f36de, 0x5f36fb])?,
            ]
        }
        Control::Ally => {
            // Option value 1 selects the inactive set, 2 the active set, anything else off.
            if r.bytes(0x5f2c55, 2, true)? != [0x6a, 0x38]
                || r.bytes(0x5f2c63, 10, true)?
                    != [0x83, 0xe8, 1, 0x74, 0x47, 0x83, 0xe8, 1, 0x74, 0x21]
                // The press handler adds one and wraps to zero past the largest value.
                || r.bytes(0x5efc83, 9, true)? != [0x46, 0x8b, 0xcf, 0x83, 0xfe, 2, 0x0f, 0x47, 0xf0]
            {
                return Err("ally option value mapping".into());
            }
            vec![
                keys(r, [0x5f2c6d, 0x5f2c79, 0x5f2c87])?,
                keys(r, [0x5f2caf, 0x5f2cbb, 0x5f2cc9])?,
                keys(r, [0x5f2c8e, 0x5f2c9a, 0x5f2ca8])?,
            ]
        }
    };
    let initial = match site.control {
        Control::Signal => 0,
        Control::Ally => option_default(r, 0x6208be, 0x38)?,
        Control::Creep => flag(option_default(r, 0x6208f1, 0x39)?),
        Control::Terrain => flag(option_default(r, 0x620929, 0x3a)?),
        Control::Formation => flag(option_default(r, 0x5f36b0, 0x2d)?),
    };
    if usize::from(initial) >= sets.len() {
        return Err(format!("{:?} initial option outside its art", site.control));
    }
    let art = sets
        .into_iter()
        .map(|[enabled, pushed, disabled]| StateArt {
            enabled,
            pushed,
            disabled,
        })
        .collect();
    let rect = Rect {
        x,
        y: y - height,
        width,
        height,
    };
    if [x, y, width, height].iter().any(|v| !v.is_finite()) {
        return Err("button arithmetic overflow".into());
    }
    Ok(Button {
        control: site.control,
        rect,
        tip_key: r.string(site.tip)?,
        uber_key: r.string(site.uber)?,
        art,
        initial,
    })
}

fn keys<F: FnMut(u32, usize, bool) -> Result<Vec<u8>, String>>(
    r: &mut Reader<F>,
    sites: Keys,
) -> Result<[String; 3], String> {
    Ok([
        r.string(sites[0])?,
        r.string(sites[1])?,
        r.string(sites[2])?,
    ])
}

/// The client reads these options as "nonzero means on" (`sbb`/`neg` after a compare with zero).
fn flag(value: u8) -> u8 {
    u8::from(value != 0)
}

/// Default of the option whose index is pushed at `site`, from the client option table.
fn option_default<F: FnMut(u32, usize, bool) -> Result<Vec<u8>, String>>(
    r: &mut Reader<F>,
    site: u32,
    index: u8,
) -> Result<u8, String> {
    if r.bytes(site, 2, true)? != [0x6a, index] {
        return Err(format!("option index {index:#x} at {site:#x}"));
    }
    // The option getter shifts the index by four and reads the table at a fixed base.
    if r.bytes(0x42b5c0, 3, true)? != [0xc1, 0xe0, 4]
        || r.word(0x42b5c3, &[0x8b, 0xb8])? != OPTION_TABLE + OPTION_DEFAULT
    {
        return Err("option table access".into());
    }
    let record = OPTION_TABLE + OPTION_STRIDE * u32::from(index);
    let pointer = u32::from_le_bytes(
        r.bytes(record + OPTION_DEFAULT, 4, false)?
            .try_into()
            .map_err(|_| "truncated option record")?,
    );
    r.string_at(pointer)?
        .trim()
        .parse::<u8>()
        .map_err(|_| format!("option {index:#x} default is not a small integer"))
}

fn idle<F: FnMut(u32, usize, bool) -> Result<Vec<u8>, String>>(
    r: &mut Reader<F>,
) -> Result<Idle, String> {
    for (va, target) in [
        (0x620d60_u32, WIDE_MAPPING),
        (0x620d89, SET_POINT),
        (0x620d98, WIDTH_SETTER),
        (0x620da7, HEIGHT_SETTER),
        (0x620e3a, GLOBAL_STRING),
        (0x620e50, GLOBAL_STRING),
    ] {
        if r.call_target(va)? != target {
            return Err(format!("idle button call target at {va:#x}"));
        }
    }
    // BOTTOMLEFT to the console parent's BOTTOMLEFT; x passes through the wide-window
    // mapping, the identity at the 4:3 logical viewport used here.
    if r.bytes(0x620d84, 5, true)? != [0x6a, 6, 0x57, 0x6a, 6] {
        return Err("idle button anchor pair".into());
    }
    let x = r.number(0x620d59, &[0xc7, 4, 0x24])?;
    if r.number(0x620d47, &[0xc7, 0x85, 0xec, 0xfe, 0xff, 0xff])? != x {
        return Err("idle button anchor x operands differ".into());
    }
    let rect = Rect {
        x,
        y: r.number(0x620d77, &[0xc7, 0x44, 0x24, 4])?,
        width: r.number(0x620d91, &[0xc7, 4, 0x24])?,
        height: r.number(0x620da0, &[0xc7, 4, 0x24])?,
    };
    Ok(Idle {
        rect,
        art_key: r.string(0x620e8a)?,
        tip_key: r.string(0x620e35)?,
        desc_key: r.string(0x620e4b)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn put(map: &mut BTreeMap<u32, u8>, va: u32, bytes: &[u8]) {
        for (i, byte) in bytes.iter().enumerate() {
            map.insert(va + i as u32, *byte);
        }
    }
    fn op(map: &mut BTreeMap<u32, u8>, va: u32, prefix: &[u8], value: u32) {
        put(map, va, prefix);
        put(map, va + prefix.len() as u32, &value.to_le_bytes());
    }
    fn call(map: &mut BTreeMap<u32, u8>, va: u32, target: u32) {
        op(map, va, &[0xe8], target.wrapping_sub(va + 5));
    }
    fn number(map: &mut BTreeMap<u32, u8>, va: u32, prefix: &[u8], value: f32) {
        op(map, va, prefix, value.to_bits());
    }
    /// Pushes a string address at `va` and stores the string at a fresh address.
    fn string(map: &mut BTreeMap<u32, u8>, va: u32, text: &str, next: &mut u32) {
        op(map, va, &[0x68], *next);
        put(map, *next, format!("{text}\0").as_bytes());
        *next += 0x40;
    }

    /// Synthetic client memory with geometry, keys and defaults unlike the reviewed image.
    fn fixture() -> BTreeMap<u32, u8> {
        let mut map = BTreeMap::new();
        let mut next = 0x2000;
        op(&mut map, 0x640ea8, &[0xf3, 0x0f, 0x10, 5], 0x7000);
        op(&mut map, 0x640e96, &[0xf3, 0x0f, 0x10, 0x1d], 0x7004);
        put(&mut map, 0x7000, &0.8f32.to_le_bytes());
        put(&mut map, 0x7004, &0.6f32.to_le_bytes());
        for (i, site) in SITES.iter().enumerate() {
            let height = site.height;
            number(&mut map, height, &[0xc7, 4, 0x24], 0.02);
            call(&mut map, height + 7, HEIGHT_SETTER);
            number(&mut map, height + 0x13, &[0xc7, 4, 0x24], 0.03);
            call(&mut map, height + 0x13 + 7, WIDTH_SETTER);
            number(
                &mut map,
                site.offset,
                &[0xc7, 0x44, 0x24, 4],
                0.5 - 0.1 * i as f32,
            );
            number(&mut map, site.offset + 8, &[0xc7, 4, 0x24], 0.07);
            let anchor = site.offset + 0xf;
            put(
                &mut map,
                anchor,
                &[0x6a, 6, 0xff, 0xb6, 0x68, 4, 0, 0, 0x6a, 0],
            );
            call(&mut map, anchor + 10, SET_POINT);
            put(&mut map, site.store, &[0x89, 0x86, site.field, 4, 0, 0]);
            op(
                &mut map,
                site.hover,
                &[0x3b, 0x9e],
                0x400 + u32::from(site.field),
            );
            string(&mut map, site.tip, &format!("TIP{i}"), &mut next);
            string(&mut map, site.uber, &format!("UBER{i}"), &mut next);
        }
        let mut sets = |map: &mut BTreeMap<u32, u8>, sites: Keys, label: &str| {
            for (site, state) in sites.into_iter().zip(["Enabled", "Pushed", "Disabled"]) {
                string(map, site, &format!("{label}{state}"), &mut next);
            }
        };
        sets(&mut map, [0x5ed283, 0x5ed2a0, 0x5ed2bd], "Signal");
        sets(&mut map, [0x5f4675, 0x5f4692, 0x5f46af], "TerrainInactive");
        sets(&mut map, [0x5f4634, 0x5f4651, 0x5f466e], "TerrainActive");
        sets(&mut map, [0x5f3645, 0x5f3662, 0x5f367f], "CreepInactive");
        sets(&mut map, [0x5f3604, 0x5f3621, 0x5f363e], "CreepActive");
        sets(&mut map, [0x5f3702, 0x5f371f, 0x5f373c], "FormationOff");
        sets(&mut map, [0x5f36c1, 0x5f36de, 0x5f36fb], "FormationOn");
        sets(&mut map, [0x5f2c6d, 0x5f2c79, 0x5f2c87], "AllyOff");
        sets(&mut map, [0x5f2caf, 0x5f2cbb, 0x5f2cc9], "AllyInactive");
        sets(&mut map, [0x5f2c8e, 0x5f2c9a, 0x5f2ca8], "AllyActive");
        put(&mut map, 0x5f462b, &[0x83, 0xb8, 0x18, 8, 0, 0, 0]);
        put(&mut map, 0x5f35fb, &[0x83, 0xb8, 0x1c, 8, 0, 0, 0]);
        put(&mut map, 0x5f36b0, &[0x6a, 0x2d]);
        put(&mut map, 0x5f36b9, &[0x83, 0x7d, 0xfc, 0]);
        put(&mut map, 0x5f2c55, &[0x6a, 0x38]);
        put(
            &mut map,
            0x5f2c63,
            &[0x83, 0xe8, 1, 0x74, 0x47, 0x83, 0xe8, 1, 0x74, 0x21],
        );
        put(
            &mut map,
            0x5efc83,
            &[0x46, 0x8b, 0xcf, 0x83, 0xfe, 2, 0x0f, 0x47, 0xf0],
        );
        put(&mut map, 0x6208be, &[0x6a, 0x38]);
        put(&mut map, 0x6208f1, &[0x6a, 0x39]);
        put(&mut map, 0x620929, &[0x6a, 0x3a]);
        put(&mut map, 0x42b5c0, &[0xc1, 0xe0, 4]);
        op(
            &mut map,
            0x42b5c3,
            &[0x8b, 0xb8],
            OPTION_TABLE + OPTION_DEFAULT,
        );
        for (index, default) in [(0x38_u32, "2"), (0x39, "0"), (0x3a, "1"), (0x2d, "1")] {
            let record = OPTION_TABLE + OPTION_STRIDE * index;
            op(
                &mut map,
                record + OPTION_DEFAULT,
                &[],
                0x6000 + index * 0x10,
            );
            put(
                &mut map,
                0x6000 + index * 0x10,
                format!("{default}\0").as_bytes(),
            );
        }
        // Idle button.
        for (va, target) in [
            (0x620d60_u32, WIDE_MAPPING),
            (0x620d89, SET_POINT),
            (0x620d98, WIDTH_SETTER),
            (0x620da7, HEIGHT_SETTER),
            (0x620e3a, GLOBAL_STRING),
            (0x620e50, GLOBAL_STRING),
        ] {
            call(&mut map, va, target);
        }
        put(&mut map, 0x620d84, &[0x6a, 6, 0x57, 0x6a, 6]);
        number(&mut map, 0x620d59, &[0xc7, 4, 0x24], 0.01);
        number(
            &mut map,
            0x620d47,
            &[0xc7, 0x85, 0xec, 0xfe, 0xff, 0xff],
            0.01,
        );
        number(&mut map, 0x620d77, &[0xc7, 0x44, 0x24, 4], 0.2);
        number(&mut map, 0x620d91, &[0xc7, 4, 0x24], 0.05);
        number(&mut map, 0x620da0, &[0xc7, 4, 0x24], 0.04);
        string(&mut map, 0x620e8a, "IdleArt", &mut next);
        string(&mut map, 0x620e35, "IDLE_TIP", &mut next);
        string(&mut map, 0x620e4b, "IDLE_DESC", &mut next);
        map
    }

    fn rules(map: &BTreeMap<u32, u8>) -> Result<Rules, String> {
        Rules::decode(
            AssetIdentity {
                source: "fixture".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            REVIEWED_IMAGE,
            |va, len, _| {
                (0..len as u32)
                    .map(|i| {
                        map.get(&(va + i))
                            .copied()
                            .ok_or_else(|| format!("unmapped {va:#x}"))
                    })
                    .collect()
            },
        )
    }

    #[test]
    fn buttons_hang_below_the_anchor_and_keep_site_order() {
        let rules = rules(&fixture()).unwrap();
        let controls: Vec<_> = rules.buttons.iter().map(|b| b.control).collect();
        assert_eq!(
            controls,
            [
                Control::Signal,
                Control::Terrain,
                Control::Ally,
                Control::Creep,
                Control::Formation
            ]
        );
        let first = rules.buttons[0].rect;
        assert_eq!((first.x, first.width, first.height), (0.07, 0.03, 0.02));
        assert!((first.y - (0.5 - 0.02)).abs() < 1e-6);
        assert_eq!(rules.buttons[0].tip_key, "TIP0");
        assert_eq!(rules.buttons[4].uber_key, "UBER4");
        let idle = rules.idle.rect;
        assert_eq!(
            (idle.x, idle.y, idle.width, idle.height),
            (0.01, 0.2, 0.05, 0.04)
        );
        assert_eq!(rules.idle.art_key, "IdleArt");
        assert_eq!(rules.idle.desc_key, "IDLE_DESC");
    }

    #[test]
    fn art_sets_and_initial_values_follow_the_option_table() {
        let rules = rules(&fixture()).unwrap();
        let by = |c: Control| rules.buttons.iter().find(|b| b.control == c).unwrap();
        let ally = by(Control::Ally);
        assert_eq!(ally.art.len(), 3);
        assert_eq!(ally.art[0].enabled, "AllyOffEnabled");
        assert_eq!(ally.art[1].pushed, "AllyInactivePushed");
        assert_eq!(ally.art[2].disabled, "AllyActiveDisabled");
        assert_eq!(ally.initial, 2);
        // Option default "0" maps to the first (inactive) set; "1" to the second.
        assert_eq!(
            (by(Control::Creep).initial, by(Control::Terrain).initial),
            (0, 1)
        );
        assert_eq!(by(Control::Formation).initial, 1);
        assert_eq!(by(Control::Terrain).art[1].enabled, "TerrainActiveEnabled");
        assert_eq!(
            by(Control::Formation).art[0].disabled,
            "FormationOffDisabled"
        );
        assert_eq!(by(Control::Signal).art.len(), 1);
    }

    #[test]
    fn changed_instructions_and_defaults_refuse() {
        for (va, byte) in [
            (SITES[0].offset + 0xf, 0u8),
            (0x5f2c63, 0x84),
            (0x5efc86, 0x84),
            (0x5f462b, 0x82),
            (0x42b5c1, 0xe1),
        ] {
            let mut map = fixture();
            map.insert(va, byte);
            assert!(rules(&map).is_err(), "{va:#x}");
        }
        let mut map = fixture();
        call(&mut map, SITES[1].height + 7, WIDTH_SETTER);
        assert!(rules(&map).is_err());
        // A default outside the art set.
        let mut map = fixture();
        put(&mut map, 0x6000 + 0x38 * 0x10, b"3\0");
        assert!(rules(&map).is_err());
        let mut map = fixture();
        put(&mut map, 0x6000 + 0x3a * 0x10, b"x\0");
        assert!(rules(&map).is_err());
        let mut map = fixture();
        number(
            &mut map,
            0x620d47,
            &[0xc7, 0x85, 0xec, 0xfe, 0xff, 0xff],
            0.02,
        );
        assert!(rules(&map).is_err());
        let mut map = fixture();
        number(&mut map, SITES[0].offset, &[0xc7, 0x44, 0x24, 4], 2.0);
        assert!(rules(&map).is_err());
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE from the selected local install"]
    fn selected_client_matches_the_decoded_buttons() {
        let path =
            std::path::PathBuf::from(std::env::var_os("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE"));
        let image = std::fs::read(&path).expect("selected executable");
        let identity = crate::campaign_inputs::client_identity(&image, &path.display().to_string());
        let rules = Rules::reviewed_legacy(&image, identity).unwrap();
        assert_eq!(rules.buttons.len(), 5);
        assert_eq!(rules.idle.art_key, "IdlePeon");
        assert_eq!(rules.idle.tip_key, "IDLE_PEON");
        let initial: Vec<_> = rules.buttons.iter().map(|b| b.initial).collect();
        assert_eq!(initial, [0, 1, 1, 1, 1]);
        assert_eq!(
            rules.buttons[1].art[1].enabled,
            "MiniMapTerrainButtonActiveEnabled"
        );
    }
}
