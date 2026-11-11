//! Held aura items: `AIau` (Legion Doom-Horn, class `AUau`, Unholy Aura), `AIad` (class `AHad`,
//! Devotion Aura), `AIba` (class `AHab`, Brilliance Aura) and `AIcd` (class `AOac`, Command
//! Aura). The registrations and timing come from the reviewed image and the row
//! values from the ability table; the decode is in `docs/re-notes/passive-held-items.md`, "Held
//! auras".
//!
//! All three classes register over the aura base `aura` (`0xb90db0`). The aura periodically applies a
//! buff to units around the holder; the host is `aura_host.rs`.
use super::*;
use spell_orb::{cell_text, fourcc, row_real};

/// Registration call of the Unholy Aura class (source `AUau`, computed base `aura`).
pub(super) const UNHOLY_WINDOW: u32 = 0xc657d0;
/// Registration call of the Devotion Aura class (source `AHad`, computed base `aura`).
pub(super) const DEVOTION_WINDOW: u32 = 0xbfb7a0;
/// Registration call of the Brilliance Aura class (source `AHab`, computed base `aura`).
pub(super) const BRILLIANCE_WINDOW: u32 = 0xbfaa90;
/// Registration call of the Command Aura class (source `AOac`, computed base `aura`).
pub(super) const COMMAND_WINDOW: u32 = 0xc25740;
/// `push 2` that converts the integer period of the ability's own timer (`0xb8fd4b`, start slot
/// `0xb8fd10`).
const TIMER_PUSH_VA: u32 = 0xb8fd4b;
/// `push 4` that converts the buff duration (`0x6f2ab6`, buff start `0x6f2aa0`).
const DURATION_PUSH_VA: u32 = 0x6f2ab6;
/// `push 4` of the static initializer that sets the global `0x112d820` (`0x401610`); the range
/// registration's period argument is the timer period divided by it (`0xb8fd88`).
const DIVISOR_PUSH_VA: u32 = 0x401610;
/// The initialized float the buff refresh compares value differences with (`0x10db054`).
const TOLERANCE_VA: u32 = 0x10db054;
const PUSH_IMM8: u8 = 0x6a;
const PUSH_IMM8_LEN: usize = 2;
/// The class both registrations name as their base.
const AURA_BASE: &str = "aura";
/// Nine `Misc` keys the aura reads when it picks targets and when its buff applies. All are zero
/// in every shipped rule set; a nonzero value changes the target test and is refused.
const MISC_KEYS: [&str; 10] = [
    "AnimatedUnitsBestowAuras",
    "BurrowedUnitsBestowAuras",
    "IllusionsBestowAuras",
    "InvisibleUnitsBestowAuras",
    "PolymorphedUnitsBestowAuras",
    "IllusionsGetMoveSpeedBonus",
    "IllusionsCanRestoreLife",
    "IllusionsCanRestoreMana",
    "IllusionsGetDefenseBonus",
    "IllusionsGetAttackBonus",
];

/// Bytes of the reviewed image the aura rules are read from.
#[derive(Clone, Debug)]
pub(crate) struct AuraImage {
    pub(crate) unholy_window: Vec<u8>,
    pub(crate) devotion_window: Vec<u8>,
    pub(crate) brilliance_window: Vec<u8>,
    pub(crate) command_window: Vec<u8>,
    pub(crate) base: Vec<u8>,
    pub(crate) timer_push: Vec<u8>,
    pub(crate) duration_push: Vec<u8>,
    pub(crate) divisor_push: Vec<u8>,
    pub(crate) tolerance: [u8; 4],
}

impl AuraImage {
    pub(crate) fn read(pe: &Pe<'_>) -> Result<Self, String> {
        let bytes = |va: u32, len: usize, code: bool| -> Result<Vec<u8>, String> {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated image bytes at {va:#x}"))
        };
        let unholy_window = bytes(UNHOLY_WINDOW, HEAL_WINDOW_LEN, true)?;
        let base_va = call_target(&unholy_window, UNHOLY_WINDOW, 5)?;
        Ok(Self {
            devotion_window: bytes(DEVOTION_WINDOW, HEAL_WINDOW_LEN, true)?,
            brilliance_window: bytes(BRILLIANCE_WINDOW, HEAL_WINDOW_LEN, true)?,
            command_window: bytes(COMMAND_WINDOW, HEAL_WINDOW_LEN, true)?,
            base: bytes(base_va, BASE_FN_LEN, true)?,
            timer_push: bytes(TIMER_PUSH_VA, PUSH_IMM8_LEN, true)?,
            duration_push: bytes(DURATION_PUSH_VA, PUSH_IMM8_LEN, true)?,
            divisor_push: bytes(DIVISOR_PUSH_VA, PUSH_IMM8_LEN, true)?,
            tolerance: bytes(TOLERANCE_VA, 4, false)?
                .try_into()
                .expect("four bytes"),
            unholy_window,
        })
    }
}

/// The timings and the comparison tolerance of the aura classes, as the image holds them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuraTiming {
    /// Period of the ability's own timer, which refreshes the holder's buff and arms the stay
    /// report of the next scan.
    pub tick: f64,
    /// Period of the range registration's scan.
    pub scan: f64,
    /// Seconds a buff lasts after its last application or refresh.
    pub duration: f64,
    /// Two values closer than this count as equal when a buff is refreshed.
    pub tolerance: f32,
}

#[derive(Clone, Debug)]
pub(super) struct AuraClasses {
    pub(super) unholy: String,
    pub(super) devotion: String,
    pub(super) brilliance: String,
    pub(super) command: String,
    timing: AuraTiming,
    /// The first nonzero `Misc` key, `Some(None)` when all are zero; `None` until
    /// `with_aura_misc` reads them.
    misc: Option<Option<&'static str>>,
}

/// What the buff does to the units it is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AuraEffect {
    /// `AUau`: `speed` (`DataA1`) is added to the movement multiplier and `regen` (`DataB1`) is a
    /// life regeneration rate in points per second.
    Unholy { speed: f32, regen: f32 },
    /// `AHad`: `armor` (`DataA1`) points of armor.
    Devotion { armor: f32 },
    /// `AHab`: `mana` (`DataA1`) points of mana regeneration per second.
    Brilliance { mana: f32 },
    /// `AOac`: attack damage. `amount` is `DataA1`, a fraction of the attack's base damage, or
    /// points when `flat` (`DataD1` nonzero). `melee` (`DataB1`) and `ranged` (`DataC1`) say which
    /// weapon types the buff and the target test cover. The flags are the first source's: a
    /// refresh replaces only the amount.
    Command {
        amount: f32,
        melee: bool,
        ranged: bool,
        flat: bool,
    },
}

impl AuraEffect {
    /// The one amount of a devotion, brilliance or command effect.
    pub(crate) fn amount(self) -> Option<f32> {
        match self {
            AuraEffect::Unholy { .. } => None,
            AuraEffect::Devotion { armor: amount }
            | AuraEffect::Brilliance { mana: amount }
            | AuraEffect::Command { amount, .. } => Some(amount),
        }
    }

    /// The same effect with its one amount replaced (the refresh `+0x370` of the buff classes
    /// keeps everything else the first source set).
    pub(crate) fn with_amount(self, amount: f32) -> Self {
        match self {
            AuraEffect::Unholy { .. } => self,
            AuraEffect::Devotion { .. } => AuraEffect::Devotion { armor: amount },
            AuraEffect::Brilliance { .. } => AuraEffect::Brilliance { mana: amount },
            AuraEffect::Command {
                melee,
                ranged,
                flat,
                ..
            } => AuraEffect::Command {
                amount,
                melee,
                ranged,
                flat,
            },
        }
    }

    /// Whether the effect adds a life or mana regeneration rate.
    pub(crate) fn changes_regen(self) -> bool {
        match self {
            AuraEffect::Unholy { regen, .. } => regen != 0.0,
            AuraEffect::Devotion { .. } | AuraEffect::Command { .. } => false,
            AuraEffect::Brilliance { mana } => mana != 0.0,
        }
    }
}

/// One held aura row at the effect level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeldAura {
    pub effect: AuraEffect,
    /// `Area1`: the radius around the holder.
    pub area: f32,
    /// The unit classes of the row's `targs1`.
    pub targets: OrbTargets,
    /// `BuffID1`: the buff every affected unit carries.
    pub buff: [u8; 4],
    pub timing: AuraTiming,
}

impl Rules {
    /// Attaches the aura classes and their timing. Both registrations must name the aura base and
    /// distinct classes; the three integers read from the image must be positive.
    pub(crate) fn with_auras(mut self, image: &AuraImage) -> Result<Self, String> {
        let (unholy, unholy_base) =
            class_from_call_window_at(&image.unholy_window, UNHOLY_WINDOW, &image.base)?;
        let (devotion, devotion_base) =
            class_from_call_window_at(&image.devotion_window, DEVOTION_WINDOW, &image.base)?;
        let (brilliance, brilliance_base) =
            class_from_call_window_at(&image.brilliance_window, BRILLIANCE_WINDOW, &image.base)?;
        let (command, command_base) =
            class_from_call_window_at(&image.command_window, COMMAND_WINDOW, &image.base)?;
        let classes = [&unholy, &devotion, &brilliance, &command];
        if [
            &unholy_base,
            &devotion_base,
            &brilliance_base,
            &command_base,
        ]
        .iter()
        .any(|base| **base != AURA_BASE)
        {
            return Err("aura registrations must name the aura base class".into());
        }
        if self.auras.is_some()
            || classes
                .iter()
                .enumerate()
                .any(|(i, class)| classes[..i].contains(class))
            || classes.iter().any(|class| self.is_registered(class))
        {
            return Err("aura registrations must name distinct classes".into());
        }
        let push = |code: &[u8]| -> Result<f64, String> {
            match code {
                [PUSH_IMM8, value @ 1..=0x7f] => Ok(f64::from(*value)),
                _ => Err("unsupported aura timing instruction".into()),
            }
        };
        let tick = push(&image.timer_push)?;
        let duration = push(&image.duration_push)?;
        let scan = tick / push(&image.divisor_push)?;
        let tolerance = f32::from_le_bytes(image.tolerance);
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err("the aura value tolerance is not a positive real".into());
        }
        self.auras = Some(AuraClasses {
            unholy,
            devotion,
            brilliance,
            command,
            timing: AuraTiming {
                tick,
                scan,
                duration,
                tolerance,
            },
            misc: None,
        });
        Ok(self)
    }

    /// Reads the `Misc` keys of the aura target test (`MiscGame.txt` first, then the map's
    /// profile); a missing key reads as 0, the default the client passes. A nonzero key makes the
    /// pickup refuse, because the target test it changes is not modelled.
    pub(crate) fn with_aura_misc(
        mut self,
        profiles: &[(&str, &[u8])],
        max_bytes: usize,
    ) -> Result<Self, String> {
        let (values, _) = pcx_objects::editor::misc::misc_scalars(profiles, MISC_KEYS, max_bytes)?;
        let classes = self
            .auras
            .as_mut()
            .ok_or("the aura keys need the aura registrations")?;
        classes.misc = Some(
            MISC_KEYS
                .iter()
                .zip(values)
                .find(|(_, value)| value.unwrap_or(0.0) != 0.0)
                .map(|(key, _)| *key),
        );
        Ok(self)
    }

    /// Whether `class` is one of the aura classes.
    pub(super) fn is_aura_class(&self, class: &str) -> bool {
        self.auras.as_ref().is_some_and(|auras| {
            [
                &auras.unholy,
                &auras.devotion,
                &auras.brilliance,
                &auras.command,
            ]
            .iter()
            .any(|aura| **aura == class)
        })
    }

    /// The aura of a held item, `None` when it has none. Two aura abilities on one item refuse.
    pub fn held_aura(&self, definition: &Definition) -> Result<Option<HeldAura>, String> {
        let item = self.item_row(definition)?;
        if item_flag(definition, item, "powerup")? || item_flag(definition, item, "usable")? {
            return Ok(None);
        }
        let abilities = item_text(definition, item, "abilList")?;
        let mut found = None;
        for alias in abilities
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty() && *a != "_")
        {
            if !self.is_aura_class(&self.ability_code(alias)?) {
                continue;
            }
            if found.is_some() {
                return Err(format!(
                    "two aura abilities on {} are unsupported",
                    definition.base_rawcode
                ));
            }
            found = Some(self.aura_row(alias)?);
        }
        Ok(found)
    }

    /// One aura row. The percent flags (`DataC1` of `AUau`, `DataB1` of `AHad` and `AHab`) must be
    /// zero: the percent mode multiplies by a unit virtual that is not identified. The command
    /// aura has no such mode: its flags select weapon types and flat or percent damage. A
    /// map-modified row
    /// refuses, because the overrides of its fields are not decoded.
    pub(super) fn aura_row(&self, alias: &str) -> Result<HeldAura, String> {
        let classes = self.auras.as_ref().ok_or("auras need their registration")?;
        if let Some(key) = classes.misc.ok_or_else(|| {
            format!("aura {alias} reads Misc keys, which this rule set was not given")
        })? {
            return Err(format!(
                "aura {alias}: Misc/{key} is set, which changes the target test; that is not modelled"
            ));
        }
        self.single_level(alias)?;
        let row = self.effect_row(alias, "aura")?;
        let real = |column: &str| -> Result<f32, String> {
            let value = row_real(row, alias, column)?;
            if value.is_finite() {
                Ok(value)
            } else {
                Err(format!("{column} of {alias} is not finite"))
            }
        };
        let area = real("Area1")?;
        if area <= 0.0 {
            return Err(format!("Area1 of aura {alias} is not positive"));
        }
        let code = self.ability_code(alias)?;
        let effect = if code == classes.command {
            AuraEffect::Command {
                amount: real("DataA1")?,
                melee: real("DataB1")? != 0.0,
                ranged: real("DataC1")? != 0.0,
                flat: real("DataD1")? != 0.0,
            }
        } else if code == classes.unholy {
            let (speed, regen) = (real("DataA1")?, real("DataB1")?);
            if real("DataC1")? != 0.0 {
                return Err(format!(
                    "aura {alias} DataC1 is set: the percent mode of the unholy aura scales the \
                     regeneration by a unit virtual (+0x130) that is not identified"
                ));
            }
            if regen < 0.0 {
                return Err(format!("aura {alias} DataB1 is negative"));
            }
            AuraEffect::Unholy { speed, regen }
        } else {
            if real("DataB1")? != 0.0 {
                return Err(format!(
                    "aura {alias} DataB1 is set: the percent mode scales the amount by a unit \
                     value that is not decoded"
                ));
            }
            let amount = real("DataA1")?;
            if code == classes.devotion {
                AuraEffect::Devotion { armor: amount }
            } else {
                AuraEffect::Brilliance { mana: amount }
            }
        };
        let buffs = cell_text(row, alias, "BuffID1")?;
        Ok(HeldAura {
            effect,
            area,
            targets: aura_targets(alias, &cell_text(row, alias, "targs1")?)?,
            buff: fourcc(alias, "BuffID1", &buffs)?,
            timing: classes.timing,
        })
    }
}

/// Class tokens of an aura row's `targs1`. The relationship and state tokens the shipped rows
/// carry (`friend`, `self`, `vuln`, `invu`) must all be present: the allowed-target test `0x6adb90`
/// was not read, so only the row that lists every one of them, which the host reads as "any
/// allied unit and the holder", is admitted.
fn aura_targets(alias: &str, targs: &str) -> Result<OrbTargets, String> {
    let mut targets = OrbTargets::default();
    let mut required = ["friend", "self", "vuln", "invu"].map(|token| (token, false));
    for token in targs.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let token = token.to_ascii_lowercase();
        match token.as_str() {
            "ground" => targets.ground = true,
            "air" => targets.air = true,
            "structure" => targets.structure = true,
            other => match required.iter_mut().find(|(name, _)| *name == other) {
                Some((_, seen)) => *seen = true,
                None => {
                    return Err(format!(
                        "aura {alias} targs1 token {other:?} is not decoded"
                    ));
                }
            },
        }
    }
    if let Some((name, _)) = required.iter().find(|(_, seen)| !seen) {
        return Err(format!(
            "aura {alias} targs1 lacks the {name:?} token; only the row with friend, self, vuln \
             and invu is decoded"
        ));
    }
    if !(targets.air || targets.ground || targets.structure) {
        return Err(format!("aura {alias} targs1 names no unit class"));
    }
    Ok(targets)
}

/// Synthetic image bytes with the shapes read above, for tests. Nothing here is a client value
/// beyond the class codes the registrations name.
#[cfg(test)]
pub(crate) mod fixture {
    use super::spell_orb::fixture::computed_at;
    use super::*;

    /// The tolerance of the synthetic image.
    pub(crate) const TOLERANCE: f32 = 0.001;
    pub(crate) const TICK: u8 = 2;
    pub(crate) const DURATION: u8 = 4;
    pub(crate) const DIVISOR: u8 = 4;

    pub(crate) fn base() -> Vec<u8> {
        let mut code = vec![MOV_EAX_IMM32];
        code.extend(AURA_BASE.bytes().rev());
        code.push(RET);
        code
    }

    /// An image with `AUau`, `AHad` and `AHab` over `aura`.
    pub(crate) fn image() -> AuraImage {
        AuraImage {
            unholy_window: computed_at("AUau", UNHOLY_WINDOW),
            devotion_window: computed_at("AHad", DEVOTION_WINDOW),
            brilliance_window: computed_at("AHab", BRILLIANCE_WINDOW),
            command_window: computed_at("AOac", COMMAND_WINDOW),
            base: base(),
            timer_push: vec![PUSH_IMM8, TICK],
            duration_push: vec![PUSH_IMM8, DURATION],
            divisor_push: vec![PUSH_IMM8, DIVISOR],
            tolerance: TOLERANCE.to_le_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_target_list_is_admitted_with_its_classes() {
        let targets = aura_targets("AIau", "air,ground,friend,self,vuln,invu").unwrap();
        assert_eq!(
            targets,
            OrbTargets {
                ground: true,
                air: true,
                structure: false,
                organic: false
            }
        );
    }

    #[test]
    fn a_target_list_that_differs_from_the_decoded_row_refuses() {
        for (targs, wanted) in [
            ("air,ground,friend,self,vuln", "invu"),
            ("air,ground,self,vuln,invu", "friend"),
            ("air,ground,friend,vuln,invu", "self"),
            ("air,ground,friend,self,vuln,invu,enemy", "enemy"),
            ("friend,self,vuln,invu", "no unit class"),
        ] {
            let error = aura_targets("AIau", targs).unwrap_err();
            assert!(error.contains(wanted), "{targs}: {error}");
        }
    }
}
