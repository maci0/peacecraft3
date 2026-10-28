//! Ordinary Raven Form class identity and immediate orders from the selected client.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    pe_image::occurrences,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Class {
    pub code: [u8; 4],
    pub on: i32,
    pub off: i32,
    pub uninterruptible: u8,
    pub immediate_takeoff: u8,
}
#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub classes: Vec<Class>,
    pub flight: Result<FlightRules, String>,
}
fn word(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "morph operand truncated".into())
}
fn shape(b: &[u8], at: usize, s: &[u8]) -> Result<(), String> {
    if b.get(at..at + s.len()) == Some(s) {
        Ok(())
    } else {
        Err(format!("morph instruction differs at {at}"))
    }
}
fn unique(mut found: impl Iterator<Item = u32>) -> Result<u32, String> {
    let first = found.next().ok_or("morph source anchor missing")?;
    if found.next().is_some() {
        return Err("morph source anchor ambiguous".into());
    }
    Ok(first)
}
impl Rules {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("morph selected client differs".into());
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| e.to_string())?;
        Self::decode(
            origin,
            sha256(image),
            pe.code_ranges().collect(),
            pe.initialized_ranges().collect(),
            128 << 20,
            |va, n, code| {
                pe.read_from(va, code)
                    .map_err(|e| e.to_string())?
                    .get(..n)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| "morph mapped window truncated".into())
            },
        )
    }
    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        code: Vec<(u32, &[u8])>,
        data: Vec<(u32, &[u8])>,
        mut work: usize,
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut find = |ranges: &[(u32, &[u8])], needle: &[u8]| -> Result<u32, String> {
            let mut matches = Vec::new();
            for &(va, b) in ranges {
                work = work
                    .checked_sub(b.len())
                    .ok_or("morph scan work exhausted")?;
                for i in occurrences(b, needle) {
                    if matches.len() == 2 {
                        return Err("morph source anchor ambiguous".into());
                    }
                    matches.push(
                        va.checked_add(i.try_into().map_err(|_| "morph offset overflow")?)
                            .ok_or("morph address overflow")?,
                    )
                }
            }
            unique(matches.into_iter())
        };
        let mut classes = Vec::new();
        {
            let name = "CAbilityRavenForm";
            let name_va = find(&data, format!("{name}\0").as_bytes())?;
            let mut getter = vec![0xb8];
            getter.extend(name_va.to_le_bytes());
            getter.push(0xc3);
            let getter_va = find(&code, &getter)?;
            let name_slot = find(&data, &getter_va.to_le_bytes())?;
            // Reviewed x86 class ABI: name slot, code slot and immediate on/off virtuals.
            let table = name_slot
                .checked_sub(0x58)
                .ok_or("morph vtable underflow")?;
            let vtable = read(table, 0x350, false)?;
            let initializer = read(word(&vtable, 0x80)?, 0x70, true)?;
            // The reviewed initializer maps source morphFlags to class state.
            // The named source flag roles are joined to these independent consumers.
            shape(&initializer, 0x44, &[0xa8])?;
            shape(&initializer, 0x46, &[0x74, 0x07, 0x81, 0x4f, 0x20])?;
            shape(&initializer, 0x4f, &[0xa8])?;
            shape(&initializer, 0x51, &[0x74, 0x07, 0x81, 0x4f, 0x20])?;
            let immediate_takeoff = initializer[0x45];
            let uninterruptible = initializer[0x50];
            if !immediate_takeoff.is_power_of_two()
                || !uninterruptible.is_power_of_two()
                || immediate_takeoff == uninterruptible
            {
                return Err("morph source flag relationship invalid".into());
            }
            let mut constant = |slot: usize| -> Result<u32, String> {
                let f = word(&vtable, slot)?;
                let b = read(f, 6, true)?;
                shape(&b, 0, &[0xb8])?;
                shape(&b, 5, &[0xc3])?;
                word(&b, 1)
            };
            let class = Class {
                code: constant(0x1c)?.to_be_bytes(),
                on: constant(0x310)? as i32,
                off: constant(0x320)? as i32,
                uninterruptible,
                immediate_takeoff,
            };
            if !class.code.iter().all(u8::is_ascii_alphanumeric)
                || class.on == 0
                || class.off == 0
                || class.on == class.off
            {
                return Err("morph class/order relationship invalid".into());
            }
            // Class binding only: phase arithmetic and supported source flags are
            // admitted separately before this identity can authorize a cast.
            if classes
                .iter()
                .any(|c: &Class| c.code == class.code || c.on == class.on || c.off == class.off)
            {
                return Err("morph class/order collision".into());
            }
            classes.push(class);
        }
        let flight = FlightRules::decode(&code, &data, &mut read);
        Ok(Self {
            flight,
            origin,
            image_sha256,
            classes,
        })
    }
}

#[cfg(test)]
#[path = "unit_morph_rules_tests.rs"]
mod tests;

/// Ranked source cells for the initially admitted uninterrupted ordinary form.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub normal: [u8; 4],
    pub alternate: [u8; 4],
    pub mana_cost: f32,
    pub cast: f32,
    pub duration: f32,
    pub altitude_duration: f32,
    pub landing_delay: f32,
}
impl Spec {
    pub fn read(
        class: &Class,
        mut cell: impl FnMut(&str, pcx_objects::editor::FieldKind) -> Result<String, String>,
    ) -> Result<Self, String> {
        use pcx_objects::editor::FieldKind;
        fn kind(raw: String) -> Result<[u8; 4], String> {
            let value: [u8; 4] = raw
                .as_bytes()
                .try_into()
                .map_err(|_| "morph unit code length")?;
            if !value.iter().all(u8::is_ascii_alphanumeric) {
                return Err("morph invalid unit code".into());
            }
            Ok(value)
        }
        let normal = kind(cell("DataA", FieldKind::Token)?)?;
        let alternate = kind(cell("UnitID", FieldKind::Token)?)?;
        if normal == alternate {
            return Err("morph normal and alternate forms coincide".into());
        }
        let flags = cell("DataB", FieldKind::Int)?
            .parse::<u32>()
            .map_err(|_| "morph malformed source flags")?;
        if flags != u32::from(class.uninterruptible | class.immediate_takeoff) {
            return Err("morph flag combination is not qualified".into());
        }
        let mut number = |key, kind| -> Result<f32, String> {
            let value = cell(key, kind)?
                .parse::<f32>()
                .map_err(|_| format!("morph malformed {key}"))?;
            if !value.is_finite() || value < 0. {
                return Err(format!("morph invalid {key}"));
            }
            Ok(value)
        };
        Ok(Self {
            normal,
            alternate,
            mana_cost: number("Cost", FieldKind::Int)?,
            cast: number("Cast", FieldKind::Unreal)?,
            duration: number("Dur", FieldKind::Unreal)?,
            altitude_duration: number("DataC", FieldKind::Unreal)?,
            landing_delay: number("DataD", FieldKind::Unreal)?,
        })
    }
}

/// Independently admitted public height setter and its shared visual trajectory.
#[derive(Clone, Debug)]
pub struct FlightRules {
    pub minimum_target: f32,
}
impl FlightRules {
    fn decode(
        code: &[(u32, &[u8])],
        data: &[(u32, &[u8])],
        read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        fn address(ranges: &[(u32, &[u8])], needle: &[u8]) -> Result<u32, String> {
            unique(
                ranges
                    .iter()
                    .flat_map(|(va, b)| occurrences(b, needle).map(move |i| *va + i as u32)),
            )
        }
        fn call(b: &[u8], va: u32, at: usize) -> Result<u32, String> {
            shape(b, at, &[0xe8])?;
            Ok(va
                .wrapping_add(at as u32 + 5)
                .wrapping_add(word(b, at + 1)?))
        }
        fn handler(
            name: &[u8],
            signature: &[u8],
            code: &[(u32, &[u8])],
            data: &[(u32, &[u8])],
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
        ) -> Result<u32, String> {
            let mut needle = vec![0x68];
            needle.extend(address(data, signature)?.to_le_bytes());
            needle.push(0x68);
            needle.extend(address(data, name)?.to_le_bytes());
            let mut found = Vec::new();
            for (_, b) in code {
                for i in occurrences(b, &needle) {
                    if b.get(i + 10) != Some(&0x68) || b.get(i + 15) != Some(&0xe8) {
                        continue;
                    }
                    let va = word(b, i + 11)?;
                    // The alternate registration table contains a deliberately inert handler.
                    if read(va, 3, true)? == [0x55, 0x8b, 0xec] {
                        found.push(va);
                    }
                }
            }
            unique(found.into_iter())
        }
        let set = handler(b"SetUnitFlyHeight\0", b"(Hunit;RR)V\0", code, data, read)?;
        let get = handler(b"GetUnitFlyHeight\0", b"(Hunit;)R\0", code, data, read)?;
        let s = read(set, 76, true)?;
        let g = read(get, 33, true)?;
        shape(&s, 0xf, &[0x85, 0xc0, 0x74])?;
        shape(&s, 0x1b, &[0xf3, 0x0f, 0x10, 0x01, 0x0f, 0x2f, 0x05])?;
        shape(&s, 0x26, &[0x76, 0x10])?;
        shape(&s, 0x38, &[0x68])?;
        shape(&g, 0xe, &[0x85, 0xc0, 0x75])?;
        shape(&g, 0x19, &[0x8b, 0x80, 0x14, 0x04, 0, 0, 0x5d, 0xc3])?;
        if call(&s, set, 7)? != call(&g, get, 6)? || word(&s, 0x22)? != word(&g, 0x13)? {
            return Err("flight resolver/zero operand differs".into());
        }
        let helper = call(&s, set, 0x2f)?;
        if helper != call(&s, set, 0x43)? {
            return Err("flight clamp helper differs".into());
        }
        let h = read(helper, 0x80, true)?;
        shape(&h, 0x41, &[0x8b, 0x87, 0x10, 0x04, 0, 0])?;
        shape(&h, 0x4d, &[0x6a, 0x01, 0x56, 0xff, 0x10])?;
        let trajectory = call(&h, helper, 0x75)?;
        let t = read(trajectory, 0x278, true)?;
        shape(&t, 0x1a1, &[0xf3, 0x0f, 0x10, 0x45, 0xec, 0x0f, 0x2e, 0x05])?;
        if word(&t, 0x1a9)? != word(&s, 0x22)? {
            return Err("flight rate-zero operand differs".into());
        }
        shape(&t, 0x1ad, &[0x9f, 0xf6, 0xc4, 0x44, 0x0f, 0x8b])?;
        for at in [0x1c7, 0x1d4, 0x1eb] {
            call(&t, trajectory, at)?;
        }
        shape(&t, 0x24e, &[0x8d, 0x45, 0x0c, 0x8b, 0xcf, 0x50, 0xe8])?;
        let minimum_target = f32::from_bits(word(&read(word(&s, 0x39)?, 4, false)?, 0)?);
        if !minimum_target.is_finite() || minimum_target <= 0. {
            return Err("flight minimum target invalid".into());
        }
        Ok(Self { minimum_target })
    }
}
