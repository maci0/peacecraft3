//! Fresh-game reconstruction policy; saved preferences are not loaded here.
use super::{BUDGET, Pe, sha256};
use crate::{options_defaults, scripts::AssetIdentity};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshSettings {
    pub speed: i32,
    pub fog_enabled: bool,
    pub fog_mask_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshTriggers {
    pub initial_enabled: bool,
    pub timer_event: i32,
}

/// Fresh triggers use the enabled state, as observed in the retained original
/// lifecycle probe. Decode that operation and event identity, not constructor internals.
pub fn fresh_triggers(image: &[u8], origin: AssetIdentity) -> Result<FreshTriggers, String> {
    let read = || -> Result<FreshTriggers, String> {
        if image.len() > BUDGET.image_bytes || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
        {
            return Err("unsupported fresh-trigger client".into());
        }
        let pe = Pe::parse(image, BUDGET).map_err(|e| e.to_string())?;
        let code = |va, len| -> Result<&[u8], String> {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..len)
                .ok_or_else(|| format!("truncated trigger role {va:#x}"))
        };
        trigger_values(
            code(0x48fd72, 5)?,
            code(0x48f9a2, 5)?,
            code(0x499924, 3)?,
            code(0x5093e1, 5)?,
            code(0x4f8298, 5)?,
        )
        .ok_or_else(|| "unsupported trigger operand relationship".into())
    };
    read().map_err(|e| format!("{}: fresh triggers: {e}", origin.source))
}

fn trigger_values(
    enable: &[u8],
    disable: &[u8],
    getter: &[u8],
    event: &[u8],
    normalize: &[u8],
) -> Option<FreshTriggers> {
    let [0x6a, enabled @ 0..=1, 0x8d, 0x48, field] = enable else {
        return None;
    };
    let [0x6a, disabled @ 0..=1, 0x8d, 0x48, other] = disable else {
        return None;
    };
    if enabled == disabled || field != other || getter != [0x8d, 0x48, *field] {
        return None;
    }
    let [0x68, event @ ..] = event else {
        return None;
    };
    let [0x05, adjustment @ ..] = normalize else {
        return None;
    };
    Some(FreshTriggers {
        initial_enabled: *enabled != 0,
        timer_event: u32::from_le_bytes(event.try_into().ok()?)
            .wrapping_add(u32::from_le_bytes(adjustment.try_into().ok()?))
            as i32,
    })
}

pub fn fresh_settings(image: &[u8], origin: AssetIdentity) -> Result<FreshSettings, String> {
    let read = || -> Result<FreshSettings, String> {
        if image.len() > BUDGET.image_bytes || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
        {
            return Err("unsupported fresh-game client".into());
        }
        let pe = Pe::parse(image, BUDGET).map_err(|e| e.to_string())?;
        let defaults = options_defaults::Defaults::discover(
            image,
            origin.clone(),
            options_defaults::DiscoveryBudget {
                image: super::RuleInputBudget {
                    profile_bytes: 128 << 10,
                    ..BUDGET
                },
                string_bytes: 4096,
                candidate_limit: 32,
                work_bytes: 64 << 20,
            },
        )
        .map_err(|e| e.to_string())?;
        let entry = defaults
            .entries()
            .iter()
            .find(|e| e.name == "gamespeed")
            .ok_or("missing gamespeed preference fallback")?;
        let requested = std::str::from_utf8(entry.fallback)
            .map_err(|e| e.to_string())?
            .parse::<u8>()
            .map_err(|e| e.to_string())?;
        let code = |va, len| -> Result<&[u8], String> {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..len)
                .ok_or_else(|| format!("truncated fresh-game role {va:#x}"))
        };
        let speed =
            clamp_speed(code(0x4c2450, 48)?, requested).ok_or("unsupported game-speed setter")?;
        let constructor = code(0x4c33a0, 54)?;
        let flag = |getter| -> Result<bool, String> {
            let bytes = code(getter, 33)?;
            let mut fields = bytes
                .windows(4)
                .filter(|b| b[..2] == [0x8b, 0x40] && b[3] == 0xc3);
            let field = fields.next().ok_or("missing fog getter field")?[2];
            if fields.next().is_some() {
                return Err("ambiguous fog getter field".into());
            }
            fog_flag(constructor, field).ok_or_else(|| "unsupported fog constructor".into())
        };
        Ok(FreshSettings {
            speed,
            fog_enabled: flag(0x498eb0)?,
            fog_mask_enabled: flag(0x498ee0)?,
        })
    };
    read().map_err(|e| format!("{}: fresh-game settings: {e}", origin.source))
}

fn clamp_speed(b: &[u8], requested: u8) -> Option<i32> {
    if b.len() != 48
        || b[..7] != [0x55, 0x8b, 0xec, 0x8a, 0x45, 0x08, 0x3c]
        || b[8..13] != [0x0f, 0xb6, 0xd0, 0x56, 0xbe]
        || u32::from_le_bytes(b[13..17].try_into().ok()?) != u32::from(b[7])
        || b[17..30]
            != [
                0x0f, 0x43, 0xd6, 0x32, 0xc0, 0x3a, 0xc2, 0x1a, 0xc0, 0x22, 0xc2, 0x88, 0x41,
            ]
        || b[31..36] != [0x0f, 0xb6, 0xc0, 0x50, 0xe8]
        || b[40..] != [0x83, 0xc4, 0x04, 0x5e, 0x5d, 0xc2, 0x04, 0x00]
    {
        return None;
    }
    Some(i32::from(requested.min(b[7])))
}

fn fog_flag(constructor: &[u8], field: u8) -> Option<bool> {
    let mut stores = constructor
        .windows(7)
        .filter(|b| b[..3] == [0xc7, 0x46, field]);
    let value = u32::from_le_bytes(stores.next()?[3..7].try_into().ok()?);
    if stores.next().is_some() {
        return None;
    }
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_trigger_values_follow_selected_state_field_and_event_operands() {
        let mut enabled = [0x6a, 1, 0x8d, 0x48, 0x34];
        let mut disabled = [0x6a, 0, 0x8d, 0x48, 0x34];
        let getter = [0x8d, 0x48, 0x34];
        let event = [0x68, 0x59, 0x19, 0, 0];
        let adjustment = [0x05, 0xb0, 0xe6, 0xff, 0xff];
        let read = |e: &[u8], d: &[u8]| trigger_values(e, d, &getter, &event, &adjustment);
        assert_eq!(
            read(&enabled, &disabled),
            Some(FreshTriggers {
                initial_enabled: true,
                timer_event: 9
            })
        );
        enabled[1] = 0;
        disabled[1] = 1;
        assert_eq!(
            read(&enabled, &disabled),
            Some(FreshTriggers {
                initial_enabled: false,
                timer_event: 9
            })
        );
        enabled[4] = 0x35;
        assert_eq!(read(&enabled, &disabled), None);
        enabled[4] = 0x34;
        enabled[1] = 2;
        assert_eq!(read(&enabled, &disabled), None);
        assert_eq!(
            trigger_values(&[], &disabled, &getter, &event, &adjustment),
            None
        );
        assert_eq!(
            trigger_values(&disabled, &disabled, &getter, &event, &adjustment),
            None
        );
        assert_eq!(
            trigger_values(
                &disabled,
                &[0x6a, 0, 0x8d, 0x48, 0x34],
                &getter,
                &event[..4],
                &adjustment
            ),
            None
        );
    }
    #[test]
    fn fresh_values_follow_operands_and_reject_ambiguous_or_non_boolean_stores() {
        let mut b = vec![
            0x55, 0x8b, 0xec, 0x8a, 0x45, 0x08, 0x3c, 7, 0x0f, 0xb6, 0xd0, 0x56, 0xbe, 7, 0, 0, 0,
            0x0f, 0x43, 0xd6, 0x32, 0xc0, 0x3a, 0xc2, 0x1a, 0xc0, 0x22, 0xc2, 0x88, 0x41, 0x53,
            0x0f, 0xb6, 0xc0, 0x50, 0xe8, 0, 0, 0, 0, 0x83, 0xc4, 0x04, 0x5e, 0x5d, 0xc2, 0x04, 0,
        ];
        assert_eq!(clamp_speed(&b, 200), Some(7));
        assert_eq!(clamp_speed(&b, 0), Some(0));
        assert_eq!(clamp_speed(&b, 4), Some(4));
        b[13] = 6;
        assert_eq!(clamp_speed(&b, 200), None);
        let mut stores = vec![0xc7, 0x46, 0x30, 0, 0, 0, 0, 0xc7, 0x46, 0x34, 1, 0, 0, 0];
        assert_eq!(fog_flag(&stores, 0x30), Some(false));
        assert_eq!(fog_flag(&stores, 0x34), Some(true));
        stores.extend_from_within(..7);
        assert_eq!(fog_flag(&stores, 0x30), None);
        stores[10] = 2;
        assert_eq!(fog_flag(&stores, 0x34), None);
    }
}
