//! Source-derived hero attribute abilities: automatic powerups (permanent strength on use) and
//! passive held-item bonuses (strength, agility, armor and flat attack damage while held). Both
//! read ability classes from the reviewed client's registration operands and amounts from
//! selected ability metadata with W3A overlays.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};
use pcx_objects::editor::defaults::{DefaultTables, Family, Object, Value};
use pcx_objects::editor::{FieldKind, FieldSource, ObjectKind, Overlay, SlkId};
use pcx_sim::world::{Definition, FieldProblem, FieldValue};
use std::sync::Arc;

/// Registration call for the powerup class (source `AIsm`, base `AIxm`).
const POWERUP_WINDOW: u32 = 0xbca0e0;
/// Registration call for the passive attribute class (source `AIab`, base `APbo`).
const PASSIVE_WINDOW: u32 = 0xbc9ce0;
/// Registration call for the passive armor class (source `AIde`, base `APbo`).
const ARMOR_WINDOW: u32 = 0xbc9d40;
/// Registration call for the held maximum-life class (source `AIml`, base `APbo`).
const MAX_LIFE_WINDOW: u32 = 0xbc9f40;
/// Registration call for the held maximum-mana class (source `AImm`, base `APbo`).
const MAX_MANA_WINDOW: u32 = 0xbc9f80;
/// Registration call for the passive flat attack-damage class (source `AIat`, base `APbo`).
const DAMAGE_WINDOW: u32 = 0xbc9c80;
/// Registration call for the life-regeneration class (source `Arel`, base `AREB`).
const LIFE_REGEN_WINDOW: u32 = 0xbca040;
/// Registration call for the mana-regeneration class (source `AIrm`, base `Arem`).
const MANA_REGEN_WINDOW: u32 = 0xbc9ec0;
/// Registration call for the healing class (source `AIhe`). Its base comes from a call, so the
/// window is longer than the immediate-base shape. Reviewed at `0xbf0520`.
pub(crate) const HEAL_WINDOW: u32 = 0xbf0520;
const HEAL_WINDOW_LEN: usize = 25;
/// Registration call for the orb base class (source `AIDB`, base from the call `0xbc92b0`, which
/// returns `AIat`). Same 25-byte layout as the healing window.
pub(crate) const ORB_BASE_WINDOW: u32 = 0xca6d00;
/// Registration call for the Orb of Fire class (source `AIfb`, base `AIDB`).
const FIRE_ORB_WINDOW: u32 = 0xca6d40;
/// Registration call for the Orb of Corruption class (source `AIcb`, base `AIDB`).
const CORRUPTION_ORB_WINDOW: u32 = 0xca6ce0;
/// Registration call for the Orb of Frost class (source `AIob`, base `AIDB`).
const FROST_ORB_WINDOW: u32 = 0xca6d80;
/// Registration call for the Orb of Venom class (source `AIpb`, base `AIDB`).
const VENOM_ORB_WINDOW: u32 = 0xca6e00;
/// Registration call for the poison class the venom orb triggers (source `Apo2`, base from the
/// call `0xbe2590`, which returns `Apoi`). Same 25-byte layout as the orb base window.
pub(crate) const POISON_WINDOW: u32 = 0xca6de0;
/// The class `0xbe2590` returns: the base the poison class registers over.
const POISON_BASE_CLASS: &str = "Apoi";
/// Initialized global the poison buff reads as the delay of its first tick (`0x6ef800`, the
/// `+0x360` slot of the periodic buff base).
const POISON_FIRST_TICK_VA: u32 = 0x10db058;
/// `DataD` bits of the poison row. Slot `+0x354` of the poison class (`0xca2d70`) splits `DataD`
/// into `& 7` (the stacking mode; 0 is the single-buff mode, which stays refused) and `& 8`
/// (which `BuffID1` entry names the buff). Any other bit refuses.
const POISON_MODE_MASK: i64 = 7;
const POISON_SECOND_BUFF_BIT: i64 = 8;
/// Registration call for the spell-resistance class (source `AIsr`, base `AIat`).
const SPELL_RESISTANCE_WINDOW: u32 = 0xbc9ee0;
/// Registration call for the held attack-speed class (source `AIas`, base `APbo`).
const ATTACK_SPEED_WINDOW: u32 = 0xbc9cc0;
/// Registration call for the movement-speed bonus class (source `AIms`, base `APbo`).
const MOVE_SPEED_BONUS_WINDOW: u32 = 0xbc9fc0;
/// Registration call for the held life-steal class (source `AIva`, base `APbo`).
const LIFE_STEAL_WINDOW: u32 = 0xbca120;
/// `Misc` key of `MiscGame.txt` that the speed-bonus class reads (`0xbbeee0`, `0xbbefc0`).
const MOVE_SPEED_STACK_KEY: &str = "MoveSpeedBonusesStack";
/// `Misc` keys the life-steal hit hook reads: `IllusionsCanRestoreLife` (`0xbc6a6a`) and
/// `MagicImmunesResistLeech` (the `+0x1f8` getter `0xbc6bf0`, read by the veto `0x682210`).
const LIFE_STEAL_ILLUSION_KEY: &str = "IllusionsCanRestoreLife";
const LIFE_STEAL_MAGIC_IMMUNE_KEY: &str = "MagicImmunesResistLeech";
/// `Misc` keys of `MiscGame.txt` that the Frost buff reads when it is applied (`0xc5b5e0`).
const FROST_MOVE_KEY: &str = "FrostMoveSpeedDecrease";
const FROST_ATTACK_KEY: &str = "FrostAttackSpeedDecrease";
/// `AbilityData` `DataE` values of an orb that are decoded: 0 attaches without the air switch
/// (`0xc9e3a0` skips `0xb1c6e0`), 2 is the value every other shipped orb row carries.
const ORB_NO_ATTACK_INDEX: i64 = 0;
const ORB_ENABLED_ATTACK_INDEX: i64 = 2;
/// Held-item classes whose class was decoded from the reviewed client but whose effect is not
/// modeled, with the reason the pickup stays refused. See `passive-held-items.md`, "Open held
/// classes".
const UNMODELED_PASSIVE_CLASSES: [(&str, &str); 0] = [];
const BASE_FN_LEN: usize = 6;
pub(crate) const REGISTER_FN: u32 = 0x7f6360;
const CLASS_WINDOW_LEN: usize = 24;
pub(crate) const PUSH_IMM32: u8 = 0x68;
pub(crate) const PUSH_EAX: u8 = 0x50;
pub(crate) const CALL_REL32: u8 = 0xe8;
pub(crate) const MOV_EAX_IMM32: u8 = 0xb8;
pub(crate) const RET: u8 = 0xc3;
pub(crate) const CLEANUP_RET: [u8; 4] = [0x83, 0xc4, 0x0c, RET];
const MAX_IMAGE_BYTES: usize = 32 << 20;
const IMAGE_BUDGET: RuleInputBudget = RuleInputBudget {
    image_bytes: MAX_IMAGE_BYTES,
    profile_bytes: 64 << 10,
    sections: 96,
};
/// Data columns are `DataA`..`DataZ`, so a metadata index names one of 26 letters.
const DATA_COLUMNS: u32 = 26;
/// The item ability's first level. Effect lookup reads `Data{column}{level}` at this level.
const EFFECT_LEVEL: u32 = 1;

/// Permanent hero strength granted by one tome. Only nonnegative authored values are admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrengthEffect {
    pub(crate) strength: i32,
}

impl StrengthEffect {
    pub fn strength(&self) -> i32 {
        self.strength
    }
}

/// Which units an orb's on-hit effect applies to: the `targs1` class tokens of its ability row.
/// Relationship and state tokens are refused when the row is read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OrbTargets {
    pub ground: bool,
    pub air: bool,
    pub structure: bool,
    /// The `organic` token: the target is not a mechanical unit (unit flag `0x454` bit 8 clear,
    /// mask bit `0x4000000` of the class routine `0x6b1c90`).
    pub organic: bool,
}

/// The on-hit effect of an orb, besides the flat attack damage that every orb adds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OrbKind {
    /// Splash of `DataA1` damage to enemies within `Area1` of the target, sparing the target.
    Fire { area: f32 },
    /// Armor penalty of `DataB1` on the target for `Dur1` (`HeroDur1` for a hero), tracked
    /// under buff `buff`.
    Corruption {
        armor_penalty: i32,
        duration: f32,
        hero_duration: f32,
        buff: [u8; 4],
    },
    /// Frost: the target's move speed falls by `move_decrease` and its attack speed by
    /// `attack_decrease` (fractions, from `MiscGame.txt`) for `Dur1` (`HeroDur1` for a hero),
    /// tracked under buff `buff`.
    Frost {
        move_decrease: f32,
        attack_decrease: f32,
        duration: f32,
        hero_duration: f32,
        buff: [u8; 4],
    },
    /// Venom: poison that deals `damage_per_second` to the target once per second, first
    /// `first_tick_delay` seconds after the hit, for `duration` (`hero_duration` for a hero).
    /// The poison of one holder refreshes and the poison of different holders stacks. The buff
    /// `buff` is the `BuffID1` entry the poison row's `DataD` selects.
    Poison {
        damage_per_second: f32,
        duration: f32,
        hero_duration: f32,
        first_tick_delay: f32,
        buff: [u8; 4],
    },
    /// The spell orb: a proc that casts the ability named by `UnitID1` on an armed hit.
    Spell(SpellOrb),
    /// The lightning orb: every armed hit casts the Purge of the sibling `AIlp` row, free of
    /// mana and cooldown.
    Lightning(Purge),
}

/// The orb classes a row can belong to.
#[derive(Clone, Copy)]
enum OrbFamily {
    Fire,
    Corruption,
    Frost,
    Venom,
    Spell,
    Lightning,
}

/// A held orb: `damage` is `DataA1`, already counted in [`AttributeBonus::damage`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orb {
    pub kind: OrbKind,
    pub damage: i32,
    pub targets: OrbTargets,
    /// Whether attaching the orb enables the holder's second weapon (`DataE1` nonzero).
    pub air_switch: bool,
}

/// Flat attributes, armor, attack damage, maximum life and regeneration from passive held
/// items. Unit strength also includes retained powerups. Armor points are added to unit armor
/// while held; damage is a flat addition to the unit's attack damage; maximum life applies to
/// any unit with vitals.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttributeBonus {
    pub strength: i32,
    pub agility: i32,
    pub intelligence: i32,
    pub armor: i32,
    pub damage: i32,
    pub max_life: i32,
    /// Flat maximum mana, applied to any unit with vitals.
    pub max_mana: i32,
    /// Life points per game second while held, separate from base regeneration eligibility.
    pub life_regen: i32,
    /// Fraction of the unit's normal mana regeneration added while held.
    pub mana_regen_fraction: f32,
    /// The held orb, when one is held. A unit holds at most one.
    pub orb: Option<Orb>,
    /// Product of `1 - DataB1` over the held spell-resistance abilities (`AIsr`), `None` when
    /// none is held. The reviewed client applies it to spell damage the unit takes; see the
    /// re-note for which recreation damage paths consume it.
    pub spell_damage_taken: Option<f32>,
    /// Sum of `DataA1` over the held attack-speed abilities (`AIas`). The client adds it to the
    /// unit's attack speed multiplier (`0xb1e490`), the same multiplier the frost and slow
    /// decreases subtract from.
    pub attack_speed: f32,
    /// The flat movement speed an item's `AIms` grants (`DataA1`), `None` without one. A unit's
    /// speed uses the largest value among its held items, kept per item by the speed state, so
    /// aggregating bonuses does not sum this.
    pub move_speed_bonus: Option<f32>,
    /// Whether `Misc/MoveSpeedBonusesStack` is set. The client then skips the movement object's
    /// recompute (`0xb69430`) when the ability attaches or detaches, so a unit already walking
    /// keeps its old speed until its next order or recompute (`unit-move-speed-model.md`).
    pub move_speed_refresh_skipped: bool,
    /// The reincarnation an `AIrc` ability on this item grants, `None` without one. It is read per
    /// item and never aggregated: each held item is its own death handler.
    pub reincarnation: Option<Reincarnation>,
    /// The held vampiric attack (`AIva`), `None` without one.
    pub life_steal: Option<LifeSteal>,
    /// The detection an `Adet` ability on this item grants (Gem of True Seeing), `None` without
    /// one. Read per item and not aggregated; the recreation has no invisible units to apply it to.
    pub detector: Option<Detector>,
    /// The Spell Shield an `ANss` ability on this item grants, `None` without one. Retained per
    /// item; a unit holds at most one. Hosted spell casts refuse where the client would consult it.
    pub spell_shield: Option<SpellShield>,
}

#[derive(Clone, Debug)]
struct Column {
    field: [u8; 4],
    data: u32,
}

/// A real-valued metadata column, such as the mana-regeneration fraction.
#[derive(Clone, Debug)]
struct FractionColumn {
    field: [u8; 4],
    data: u32,
    /// What the amount is, for refusal messages.
    label: &'static str,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    tables: Arc<DefaultTables>,
    overlay: Overlay,
    powerup_class: String,
    passive_class: String,
    armor_class: String,
    damage_class: String,
    strength: Column,
    agility: Column,
    intelligence: Column,
    armor: Column,
    damage: Column,
    /// Held maximum-life registration, attached by `with_max_life`.
    max_life: Option<AmountClass>,
    /// Held maximum-mana registration, attached by `with_max_mana`.
    max_mana: Option<AmountClass>,
    /// Healing registration, attached by `with_heal`. Absent until the healing window is read.
    heal: Option<AmountClass>,
    life_regen: Option<AmountClass>,
    mana_regen: Option<FractionClass>,
    /// Orb registrations, attached by `with_orbs`. Absent until the orb windows are read.
    orbs: Option<OrbClasses>,
    /// Spell-resistance registration, attached by `with_spell_resistance`.
    spell_resistance: Option<String>,
    /// Attack-speed registration, attached by `with_attack_speed`.
    attack_speed: Option<FractionClass>,
    /// Movement-speed bonus registration, attached by `with_move_speed_bonus`.
    move_speed_bonus: Option<MoveSpeedBonusClass>,
    /// `AIrc` and the hero reincarnation classes, attached by `with_reincarnation`.
    reincarnation: Option<ReincarnationClasses>,
    /// Life-steal registration, attached by `with_life_steal`.
    life_steal: Option<LifeStealClass>,
    /// The `Adet` class, attached by `with_detector`.
    detector: Option<DetectorClass>,
    /// The `ANss` class, attached by `with_spell_shield`.
    spell_shield: Option<SpellShieldClass>,
    /// `AUau` and `AHad`, attached by `with_auras`.
    auras: Option<AuraClasses>,
}

/// The registered `AIms` class and whether `Misc/MoveSpeedBonusesStack` is set, attached by
/// `with_move_speed_stacking`. The flag does not change the speed formula; it skips the
/// movement object's recompute when the ability attaches or detaches.
#[derive(Clone, Debug)]
struct MoveSpeedBonusClass {
    class: String,
    stacks: Option<bool>,
}

/// The vampiric attack of a held `AIva` item. On each armed hit the holder gains `fraction` of the
/// damage dealt, up to the life the target had; see `passive-held-items.md`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LifeSteal {
    /// `DataA1` of the row.
    pub fraction: f32,
    /// The unit classes of the row's `targs1` (the `enemy` token is required).
    pub targets: OrbTargets,
    /// `Misc/IllusionsCanRestoreLife`: whether an illusion on either side still heals.
    pub illusions_restore: bool,
}

/// The registered `AIva` class and the `Misc` keys its hit hook reads, attached by
/// `with_life_steal_misc`.
#[derive(Clone, Debug)]
struct LifeStealClass {
    class: String,
    misc: Option<LifeStealMisc>,
}

/// The `Misc` flags the life-steal hook reads.
#[derive(Clone, Copy, Debug)]
struct LifeStealMisc {
    illusions_restore: bool,
    magic_immune_veto: bool,
}

/// The registered orb classes: the shared `AIDB` base, `AIfb` (Orb of Fire, and Orb of
/// Kil'jaeden through its row's code) and `AIcb` (Orb of Corruption).
#[derive(Clone, Debug)]
struct OrbClasses {
    base: String,
    fire: String,
    corruption: String,
    /// `AIob` (Orb of Frost), attached by `with_frost_orb`.
    frost: Option<FrostClass>,
    /// `AIpb` (Orb of Venom) and its poison class, attached by `with_venom_orb`.
    venom: Option<VenomClasses>,
    /// `AIsb` (spell orbs), attached by `with_spell_orb`.
    spell: Option<SpellClass>,
    /// `AIlb` (the lightning orb), `Aprg` and `AIlp`, attached by `with_purge`.
    purge: Option<PurgeClasses>,
}

/// The registered Orb of Venom class, the poison class its item lists beside it, and the first
/// tick delay read from the image.
#[derive(Clone, Debug)]
struct VenomClasses {
    orb: String,
    poison: String,
    first_tick_delay: f32,
}

/// The registered Frost orb class and the `Misc` decreases its buff applies, attached by
/// `with_frost_slow`. A missing `Misc` key reads as 0, the default the client passes.
#[derive(Clone, Debug)]
struct FrostClass {
    class: String,
    decreases: Option<(f32, f32)>,
}

/// One admitted scalar ability class and its metadata-selected amount column.
#[derive(Clone, Debug)]
struct AmountClass {
    class: String,
    amount: Column,
}

/// One admitted real-valued ability class and its metadata-selected amount column.
#[derive(Clone, Debug)]
struct FractionClass {
    class: String,
    amount: FractionColumn,
}

impl Rules {
    /// Reads registration windows from the reviewed client image and selected metadata.
    pub fn read(
        tables: Arc<DefaultTables>,
        overlay: Overlay,
        image: &[u8],
        origin: AssetIdentity,
    ) -> Result<Self, String> {
        let context = format!("inventory attribute rules {}", origin.source);
        let fail = |why: &str| format!("{context}: {why}");
        let digest = sha256(image);
        if image.len() > MAX_IMAGE_BYTES
            || digest != REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail("unsupported client identity"));
        }
        let pe = Pe::parse(image, IMAGE_BUDGET).map_err(|e| fail(&e.to_string()))?;
        let window = |va: u32| {
            pe.read_from(va, true)
                .map_err(|e| fail(&e.to_string()))?
                .get(..CLASS_WINDOW_LEN)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| fail("truncated class registration window"))
        };
        let powerup = window(POWERUP_WINDOW)?;
        let passive = window(PASSIVE_WINDOW)?;
        let armor = window(ARMOR_WINDOW)?;
        let damage = window(DAMAGE_WINDOW)?;
        let max_life = window(MAX_LIFE_WINDOW)?;
        let max_mana = window(MAX_MANA_WINDOW)?;
        let life_regen = window(LIFE_REGEN_WINDOW)?;
        let mana_regen = window(MANA_REGEN_WINDOW)?;
        let heal = pe
            .read_from(HEAL_WINDOW, true)
            .map_err(|e| fail(&e.to_string()))?
            .get(..HEAL_WINDOW_LEN)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| fail("truncated healing registration window"))?;
        let base_va = call_target(&heal, HEAL_WINDOW, 5).map_err(|e| fail(&e))?;
        let base_code = pe
            .read_from(base_va, true)
            .map_err(|e| fail(&e.to_string()))?
            .get(..BASE_FN_LEN)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| fail("truncated healing base function"))?;
        let orb_base = pe
            .read_from(ORB_BASE_WINDOW, true)
            .map_err(|e| fail(&e.to_string()))?
            .get(..HEAL_WINDOW_LEN)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| fail("truncated orb base registration window"))?;
        let orb_base_va = call_target(&orb_base, ORB_BASE_WINDOW, 5).map_err(|e| fail(&e))?;
        let orb_base_code = pe
            .read_from(orb_base_va, true)
            .map_err(|e| fail(&e.to_string()))?
            .get(..BASE_FN_LEN)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| fail("truncated orb base function"))?;
        let fire_orb = window(FIRE_ORB_WINDOW)?;
        let corruption_orb = window(CORRUPTION_ORB_WINDOW)?;
        let frost_orb = window(FROST_ORB_WINDOW)?;
        let venom_orb = window(VENOM_ORB_WINDOW)?;
        let poison = pe
            .read_from(POISON_WINDOW, true)
            .map_err(|e| fail(&e.to_string()))?
            .get(..HEAL_WINDOW_LEN)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| fail("truncated poison registration window"))?;
        let poison_base_va = call_target(&poison, POISON_WINDOW, 5).map_err(|e| fail(&e))?;
        let poison_base_code = pe
            .read_from(poison_base_va, true)
            .map_err(|e| fail(&e.to_string()))?
            .get(..BASE_FN_LEN)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| fail("truncated poison base function"))?;
        let first_tick: [u8; 4] = pe
            .read_from(POISON_FIRST_TICK_VA, false)
            .map_err(|e| fail(&e.to_string()))?
            .get(..4)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| fail("truncated poison first tick delay"))?;
        let spell_resistance = window(SPELL_RESISTANCE_WINDOW)?;
        let attack_speed = window(ATTACK_SPEED_WINDOW)?;
        let move_speed_bonus = window(MOVE_SPEED_BONUS_WINDOW)?;
        let life_steal = window(LIFE_STEAL_WINDOW)?;
        let aura = AuraImage::read(&pe).map_err(|e| fail(&e))?;
        let spell_orb = SpellOrbImage::read(&pe).map_err(|e| fail(&e))?;
        let reincarnation = ReincarnationImage::read(&pe).map_err(|e| fail(&e))?;
        let detector = DetectorImage::read(&pe).map_err(|e| fail(&e))?;
        let spell_shield = SpellShieldImage::read(&pe).map_err(|e| fail(&e))?;
        let purge = PurgeImage::read(&pe).map_err(|e| fail(&e))?;
        Self::decode(
            tables,
            overlay,
            origin,
            digest,
            [&powerup, &passive, &armor, &damage],
        )
        .and_then(|rules| rules.with_max_life(&max_life))
        .and_then(|rules| rules.with_max_mana(&max_mana))
        .and_then(|rules| rules.with_life_regen(&life_regen))
        .and_then(|rules| rules.with_mana_regen(&mana_regen))
        .and_then(|rules| rules.with_heal(&heal, &base_code))
        .and_then(|rules| rules.with_orbs(&orb_base, &orb_base_code, &fire_orb, &corruption_orb))
        .and_then(|rules| rules.with_frost_orb(&frost_orb))
        .and_then(|rules| rules.with_venom_orb(&venom_orb, &poison, &poison_base_code, first_tick))
        .and_then(|rules| rules.with_spell_orb(&spell_orb))
        .and_then(|rules| rules.with_purge(&purge))
        .and_then(|rules| rules.with_spell_resistance(&spell_resistance))
        .and_then(|rules| rules.with_attack_speed(&attack_speed))
        .and_then(|rules| rules.with_move_speed_bonus(&move_speed_bonus))
        .and_then(|rules| rules.with_reincarnation(&reincarnation))
        .and_then(|rules| rules.with_life_steal(&life_steal))
        .and_then(|rules| rules.with_detector(&detector))
        .and_then(|rules| rules.with_spell_shield(&spell_shield))
        .and_then(|rules| rules.with_auras(&aura))
        .map_err(|e| fail(&e))
    }

    /// Attaches the orb classes. The shared base `AIDB` registers over `AIat` (the flat
    /// attack-damage class), and `AIfb` and `AIcb` register over `AIDB`. All four classes must be
    /// distinct from every other registered class.
    pub(crate) fn with_orbs(
        mut self,
        base_window: &[u8],
        base_fn: &[u8],
        fire_window: &[u8],
        corruption_window: &[u8],
    ) -> Result<Self, String> {
        let (orb_base, damage_base) =
            class_from_call_window_at(base_window, ORB_BASE_WINDOW, base_fn)?;
        if damage_base != self.damage_class {
            return Err("orb base registration must derive from the attack-damage class".into());
        }
        let (fire, fire_base) = class_from_window(fire_window)?;
        let (corruption, corruption_base) = class_from_window(corruption_window)?;
        if fire_base != orb_base || corruption_base != orb_base {
            return Err("orb registrations must name the orb base class".into());
        }
        let taken = [
            Some(&self.powerup_class),
            Some(&self.passive_class),
            Some(&self.armor_class),
            Some(&self.damage_class),
            self.max_life.as_ref().map(|c| &c.class),
            self.max_mana.as_ref().map(|c| &c.class),
            self.life_regen.as_ref().map(|c| &c.class),
            self.mana_regen.as_ref().map(|c| &c.class),
            self.heal.as_ref().map(|c| &c.class),
            Some(&orb_base),
        ];
        if self.orbs.is_some()
            || fire == corruption
            || taken
                .iter()
                .flatten()
                .any(|c| **c == fire || **c == corruption)
        {
            return Err("orb registrations must name distinct classes".into());
        }
        self.orbs = Some(OrbClasses {
            base: orb_base,
            fire,
            corruption,
            frost: None,
            venom: None,
            spell: None,
            purge: None,
        });
        Ok(self)
    }

    /// Whether `class` is one of the registrations already attached.
    fn is_registered(&self, class: &str) -> bool {
        let amount = |c: &Option<AmountClass>| c.as_ref().is_some_and(|c| c.class == class);
        if self.detector.as_ref().is_some_and(|d| d.class == class) {
            return true;
        }
        if self.spell_shield.as_ref().is_some_and(|s| s.class == class) {
            return true;
        }
        if self.reincarnation.as_ref().is_some_and(|classes| {
            [&classes.ankh, &classes.hero, &classes.creep]
                .iter()
                .any(|c| **c == class)
        }) {
            return true;
        }
        [
            &self.powerup_class,
            &self.passive_class,
            &self.armor_class,
            &self.damage_class,
        ]
        .iter()
        .any(|c| **c == class)
            || amount(&self.max_life)
            || amount(&self.max_mana)
            || amount(&self.heal)
            || amount(&self.life_regen)
            || self
                .mana_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
            || self.spell_resistance.as_deref() == Some(class)
            || self
                .life_steal
                .as_ref()
                .is_some_and(|steal| steal.class == class)
            || self.is_aura_class(class)
            || self
                .attack_speed
                .as_ref()
                .is_some_and(|speed| speed.class == class)
            || self
                .move_speed_bonus
                .as_ref()
                .is_some_and(|bonus| bonus.class == class)
            || self.orbs.as_ref().is_some_and(|orbs| {
                [&orbs.base, &orbs.fire, &orbs.corruption]
                    .iter()
                    .any(|c| **c == class)
                    || orbs
                        .frost
                        .as_ref()
                        .is_some_and(|frost| frost.class == class)
                    || orbs
                        .venom
                        .as_ref()
                        .is_some_and(|venom| venom.orb == class || venom.poison == class)
                    || orbs
                        .spell
                        .as_ref()
                        .is_some_and(|spell| spell.class == class || spell.black_arrow == class)
                    || orbs.purge.as_ref().is_some_and(|purge| {
                        [&purge.purge, &purge.orb, &purge.sibling]
                            .iter()
                            .any(|c| **c == class)
                    })
            })
    }

    /// Attaches the Orb of Venom and the poison class it triggers. The orb registers over the
    /// orb base; the poison class registers over `Apoi`, whose base function the caller reads
    /// from the image. `first_tick_delay` holds the initialized float the poison buff reads for
    /// its first tick; it must be finite and positive.
    pub(crate) fn with_venom_orb(
        mut self,
        orb_window: &[u8],
        poison_window: &[u8],
        poison_base_fn: &[u8],
        first_tick_delay: [u8; 4],
    ) -> Result<Self, String> {
        let (orb, orb_base) = class_from_window(orb_window)?;
        let (poison, poison_base) =
            class_from_call_window_at(poison_window, POISON_WINDOW, poison_base_fn)?;
        let orbs = self
            .orbs
            .as_ref()
            .ok_or("the venom orb registration needs the orb base")?;
        if orb_base != orbs.base {
            return Err("venom orb registration must name the orb base class".into());
        }
        if poison_base != POISON_BASE_CLASS {
            return Err(format!(
                "poison registration must name base {POISON_BASE_CLASS}"
            ));
        }
        if orbs.venom.is_some()
            || orb == poison
            || self.is_registered(&orb)
            || self.is_registered(&poison)
        {
            return Err("venom orb registrations must name distinct classes".into());
        }
        let first_tick_delay = f32::from_le_bytes(first_tick_delay);
        if !first_tick_delay.is_finite() || first_tick_delay <= 0.0 {
            return Err("poison first tick delay is not a positive real".into());
        }
        let orbs = self.orbs.as_mut().expect("checked above");
        orbs.venom = Some(VenomClasses {
            orb,
            poison,
            first_tick_delay,
        });
        Ok(self)
    }

    /// Attaches the Orb of Frost class. Its registration must name the orb base, which
    /// `with_orbs` attached.
    pub(crate) fn with_frost_orb(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, base) = class_from_window(window)?;
        let orbs = self
            .orbs
            .as_ref()
            .ok_or("the frost orb registration needs the orb base")?;
        if base != orbs.base {
            return Err("frost orb registration must name the orb base class".into());
        }
        if orbs.frost.is_some() || self.is_registered(&class) {
            return Err("frost orb registration must name a distinct class".into());
        }
        let orbs = self.orbs.as_mut().expect("checked above");
        orbs.frost = Some(FrostClass {
            class,
            decreases: None,
        });
        Ok(self)
    }

    /// Reads the move and attack speed decreases the Frost buff applies from the `Misc` section
    /// (`MiscGame.txt` first, then the map's profile). A key the profiles omit reads as 0, the
    /// default the client passes; a decrease outside `[0, 1)` refuses, because a speed
    /// multiplier of zero or below is a stall state that is not modelled.
    pub(crate) fn with_frost_slow(
        mut self,
        profiles: &[(&str, &[u8])],
        max_bytes: usize,
    ) -> Result<Self, String> {
        let ([movement, attack], _) = pcx_objects::editor::misc::misc_scalars(
            profiles,
            [FROST_MOVE_KEY, FROST_ATTACK_KEY],
            max_bytes,
        )?;
        let read = |key: &str, value: Option<f32>| {
            let value = value.unwrap_or(0.0);
            if (0.0..1.0).contains(&value) {
                Ok(value)
            } else {
                Err(format!("Misc/{key} must be in [0, 1) to be modelled"))
            }
        };
        let decreases = (
            read(FROST_MOVE_KEY, movement)?,
            read(FROST_ATTACK_KEY, attack)?,
        );
        let frost = self
            .orbs
            .as_mut()
            .and_then(|orbs| orbs.frost.as_mut())
            .ok_or("the frost decreases need the frost orb registration")?;
        frost.decreases = Some(decreases);
        Ok(self)
    }

    /// Attaches the spell-resistance class. Its registration must name the flat attack-damage
    /// class as its base, as the reviewed image registers it (`0xbc9ee0`).
    pub(crate) fn with_spell_resistance(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, base) = class_from_window(window)?;
        if base != self.damage_class {
            return Err("spell-resistance registration must name the attack-damage base".into());
        }
        if self.spell_resistance.is_some() || self.is_registered(&class) {
            return Err("spell-resistance registration must name a distinct class".into());
        }
        self.spell_resistance = Some(class);
        Ok(self)
    }

    /// Attaches the movement-speed bonus class. Its registration must name the passive base
    /// `APbo`, as the reviewed image registers it (`0xbc9fc0`).
    pub(crate) fn with_move_speed_bonus(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, base) = class_from_window(window)?;
        if base != "APbo" {
            return Err("speed-bonus registration must name base APbo".into());
        }
        if self.move_speed_bonus.is_some() || self.is_registered(&class) {
            return Err("speed-bonus registration must name a distinct class".into());
        }
        self.move_speed_bonus = Some(MoveSpeedBonusClass {
            class,
            stacks: None,
        });
        Ok(self)
    }

    /// Reads `Misc/MoveSpeedBonusesStack` (`MiscGame.txt` first, then the map's profile). A
    /// missing key reads as 0, the default the client passes.
    pub(crate) fn with_move_speed_stacking(
        mut self,
        profiles: &[(&str, &[u8])],
        max_bytes: usize,
    ) -> Result<Self, String> {
        let ([stacks], _) =
            pcx_objects::editor::misc::misc_scalars(profiles, [MOVE_SPEED_STACK_KEY], max_bytes)?;
        let class = self
            .move_speed_bonus
            .as_mut()
            .ok_or("the stacking flag needs the speed-bonus registration")?;
        class.stacks = Some(stacks.unwrap_or(0.0) != 0.0);
        Ok(self)
    }

    /// Attaches the held life-steal class. Its registration must name the passive base `APbo`, as
    /// the reviewed image registers it (`0xbca120`).
    pub(crate) fn with_life_steal(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, base) = class_from_window(window)?;
        if base != "APbo" {
            return Err("life-steal registration must name base APbo".into());
        }
        if self.life_steal.is_some() || self.is_registered(&class) {
            return Err("life-steal registration must name a distinct class".into());
        }
        self.life_steal = Some(LifeStealClass { class, misc: None });
        Ok(self)
    }

    /// Reads the `Misc` keys of the life-steal hit hook (`MiscGame.txt` first, then the map's
    /// profile); a missing key reads as 0, the default the client passes. A set
    /// `MagicImmunesResistLeech` makes the pickup refuse: the veto it enables tests the victim's
    /// immunity counter (`[unit+0x264]`), which no recreation state maps to.
    pub(crate) fn with_life_steal_misc(
        mut self,
        profiles: &[(&str, &[u8])],
        max_bytes: usize,
    ) -> Result<Self, String> {
        let ([illusions, magic_immune], _) = pcx_objects::editor::misc::misc_scalars(
            profiles,
            [LIFE_STEAL_ILLUSION_KEY, LIFE_STEAL_MAGIC_IMMUNE_KEY],
            max_bytes,
        )?;
        let class = self
            .life_steal
            .as_mut()
            .ok_or("the life-steal keys need the life-steal registration")?;
        class.misc = Some(LifeStealMisc {
            illusions_restore: illusions.unwrap_or(0.0) != 0.0,
            magic_immune_veto: magic_immune.unwrap_or(0.0) != 0.0,
        });
        Ok(self)
    }

    /// Attaches the healing class. The class key is an immediate in the window; the base comes
    /// from the reviewed base function, which the caller reads from the image.
    pub(crate) fn with_heal(mut self, window: &[u8], base_code: &[u8]) -> Result<Self, String> {
        let (class, _) = class_from_call_window(window, base_code)?;
        if class == self.powerup_class
            || class == self.passive_class
            || class == self.armor_class
            || class == self.damage_class
            || self
                .max_life
                .as_ref()
                .is_some_and(|max_life| max_life.class == class)
            || self
                .life_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
            || self
                .mana_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
            || self
                .max_mana
                .as_ref()
                .is_some_and(|max_mana| max_mana.class == class)
        {
            return Err("healing registration must name a distinct class".into());
        }
        let amount = column(&self.tables, b"Ihpg")?;
        self.heal = Some(AmountClass { class, amount });
        Ok(self)
    }

    /// Attaches the held maximum-life class. Its registration must name the passive base
    /// `APbo`, and its amount comes from the integer `Ilif` metadata column.
    pub(crate) fn with_max_life(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, base) = class_from_window(window)?;
        if base != "APbo" {
            return Err("maximum-life registration must name base APbo".into());
        }
        if self.max_life.is_some()
            || [
                &self.powerup_class,
                &self.passive_class,
                &self.armor_class,
                &self.damage_class,
            ]
            .contains(&&class)
            || self
                .life_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
            || self
                .mana_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
            || self.heal.as_ref().is_some_and(|heal| heal.class == class)
        {
            return Err("maximum-life registration must name a distinct class".into());
        }
        let amount = column(&self.tables, b"Ilif")?;
        self.max_life = Some(AmountClass { class, amount });
        Ok(self)
    }

    /// Attaches the held maximum-mana class. Its registration must name the passive base `APbo`,
    /// and its amount comes from the integer `Iman` metadata column.
    pub(crate) fn with_max_mana(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, base) = class_from_window(window)?;
        if base != "APbo" {
            return Err("maximum-mana registration must name base APbo".into());
        }
        if self.max_mana.is_some()
            || [
                &self.powerup_class,
                &self.passive_class,
                &self.armor_class,
                &self.damage_class,
            ]
            .contains(&&class)
            || self
                .max_life
                .as_ref()
                .is_some_and(|max_life| max_life.class == class)
        {
            return Err("maximum-mana registration must name a distinct class".into());
        }
        let amount = column(&self.tables, b"Iman")?;
        self.max_mana = Some(AmountClass { class, amount });
        Ok(self)
    }

    pub(crate) fn with_life_regen(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, _) = class_from_window(window)?;
        if self.life_regen.is_some()
            || [
                &self.powerup_class,
                &self.passive_class,
                &self.armor_class,
                &self.damage_class,
            ]
            .contains(&&class)
            || self.heal.as_ref().is_some_and(|heal| heal.class == class)
            || self
                .mana_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
            || self
                .max_life
                .as_ref()
                .is_some_and(|max_life| max_life.class == class)
            || self
                .max_mana
                .as_ref()
                .is_some_and(|max_mana| max_mana.class == class)
        {
            return Err("life regeneration registration must name a distinct class".into());
        }
        let amount = column(&self.tables, b"Ihpr")?;
        self.life_regen = Some(AmountClass { class, amount });
        Ok(self)
    }

    /// Attaches the held attack-speed class, whose amount is the real `Isx1` column. The client
    /// registers it over the same passive base as the attribute class (`APbo`).
    pub(crate) fn with_attack_speed(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, _) = class_from_window(window)?;
        if self.attack_speed.is_some() || self.is_registered(&class) {
            return Err("attack speed registration must name a distinct class".into());
        }
        let amount = fraction_column(&self.tables, b"Isx1", "attack speed")?;
        self.attack_speed = Some(FractionClass { class, amount });
        Ok(self)
    }

    /// Attaches the real-valued mana-regeneration class.
    pub(crate) fn with_mana_regen(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, _) = class_from_window(window)?;
        if self.mana_regen.is_some()
            || [
                &self.powerup_class,
                &self.passive_class,
                &self.armor_class,
                &self.damage_class,
            ]
            .contains(&&class)
            || self
                .life_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
            || self
                .max_life
                .as_ref()
                .is_some_and(|max_life| max_life.class == class)
            || self
                .max_mana
                .as_ref()
                .is_some_and(|max_mana| max_mana.class == class)
            || self.heal.as_ref().is_some_and(|heal| heal.class == class)
        {
            return Err("mana regeneration registration must name a distinct class".into());
        }
        let amount = fraction_column(&self.tables, b"Imrp", "mana regeneration")?;
        self.mana_regen = Some(FractionClass { class, amount });
        Ok(self)
    }

    /// Healing amount (`DataA1` at level 1 through the `Ihpg` admission) of an item whose ability
    /// list names the healing class. `None` for items with no abilities or no healing ability.
    /// Mixed lists and healing that is not admitted refuse.
    pub fn heal_effect(&self, definition: &Definition) -> Result<Option<i32>, String> {
        let item = self.item_row(definition)?;
        let abilities = item_text(definition, item, "abilList")?;
        let aliases: Vec<&str> = abilities
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty() && *a != "_")
            .collect();
        // A passive or unusable item has no activation, so it is not a use: no effect, no refusal.
        if aliases.is_empty() || !item_flag(definition, item, "usable")? {
            return Ok(None);
        }
        let Some(heal) = &self.heal else {
            return Err("healing registration is not admitted".into());
        };
        let mut healing = Vec::new();
        for alias in &aliases {
            if self.ability_code(alias)? == heal.class {
                healing.push(*alias);
            }
        }
        match healing[..] {
            [] => Ok(None),
            [alias] if aliases.len() == 1 => {
                self.single_level(alias)?;
                let amount = self.amount(alias, &heal.amount)?;
                if amount < 0 {
                    return Err(format!("negative healing amount on {}", item.rawcode()));
                }
                Ok(Some(amount))
            }
            _ => Err(format!(
                "mixed healing ability list on {} is unsupported",
                item.rawcode()
            )),
        }
    }

    /// Whether a healing item's single ability needs a selected target: its authored `targs1` is
    /// not `_`. A missing `targs1` cell refuses, since the selection convention is not guessed.
    pub fn heal_requires_target(&self, definition: &Definition) -> Result<bool, String> {
        let item = self.item_row(definition)?;
        let abilities = item_text(definition, item, "abilList")?;
        let [alias] = abilities.split(',').map(str::trim).collect::<Vec<_>>()[..] else {
            return Err("healing target needs exactly one ability".into());
        };
        let targets = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?
            .cell(SlkId::AbilityData, "targs1")
            .map_err(|e| e.to_string())?
            .text
            .trim()
            .to_owned();
        Ok(targets != "_")
    }

    /// Cast range (`Rng1`, level 1) of an item's single ability, from the source row. Overrides
    /// of the range field are not applied here, and that is not qualified.
    pub fn heal_range(&self, definition: &Definition) -> Result<f32, String> {
        let item = self.item_row(definition)?;
        let abilities = item_text(definition, item, "abilList")?;
        let [alias] = abilities.split(',').map(str::trim).collect::<Vec<_>>()[..] else {
            return Err("healing range needs exactly one ability".into());
        };
        let text = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?
            .cell(SlkId::AbilityData, "Rng1")
            .map_err(|e| e.to_string())?
            .text
            .trim()
            .to_owned();
        text.parse::<f32>()
            .ok()
            .filter(|r| r.is_finite() && *r >= 0.0)
            .ok_or_else(|| format!("range {text} of {alias} is not a finite distance"))
    }

    pub(crate) fn decode(
        tables: Arc<DefaultTables>,
        overlay: Overlay,
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        windows: [&[u8]; 4],
    ) -> Result<Self, String> {
        let [powerup_window, passive_window, armor_window, damage_window] = windows;
        let (powerup_class, _) = class_from_window(powerup_window)?;
        // The passive, armor and damage bases (`APbo`) are native classes with no ability row, so
        // they are not checked against metadata.
        let (passive_class, _) = class_from_window(passive_window)?;
        let (armor_class, _) = class_from_window(armor_window)?;
        let (damage_class, _) = class_from_window(damage_window)?;
        if powerup_class == passive_class
            || powerup_class == armor_class
            || powerup_class == damage_class
            || passive_class == armor_class
            || passive_class == damage_class
            || armor_class == damage_class
        {
            return Err("attribute registrations must name distinct classes".into());
        }
        let strength = column(&tables, b"Istr")?;
        let agility = column(&tables, b"Iagi")?;
        let intelligence = column(&tables, b"Iint")?;
        let armor = column(&tables, b"Idef")?;
        let damage = column(&tables, b"Iatt")?;
        Ok(Self {
            strength,
            agility,
            intelligence,
            armor,
            damage,
            origin,
            image_sha256,
            tables,
            overlay,
            powerup_class,
            passive_class,
            armor_class,
            damage_class,
            max_life: None,
            max_mana: None,
            heal: None,
            life_regen: None,
            mana_regen: None,
            attack_speed: None,
            orbs: None,
            spell_resistance: None,
            move_speed_bonus: None,
            reincarnation: None,
            life_steal: None,
            detector: None,
            spell_shield: None,
            auras: None,
        })
    }

    /// Strength granted by an item, `None` when the item is not an automatic powerup.
    /// Agility or intelligence effects refuse until their native paths are qualified.
    pub fn strength_effect(
        &self,
        definition: &Definition,
    ) -> Result<Option<StrengthEffect>, String> {
        let item = self.item_row(definition)?;
        if !item_flag(definition, item, "powerup")? {
            return Ok(None);
        }
        if !item_flag(definition, item, "usable")?
            || !item_flag(definition, item, "perishable")?
            || item_text(definition, item, "uses")? != "1"
        {
            return Err("powerup item flags or uses are unsupported".into());
        }
        let abilities = item_text(definition, item, "abilList")?;
        let [alias] = abilities.split(',').map(str::trim).collect::<Vec<_>>()[..] else {
            return Err("mixed or empty powerup ability list is unsupported".into());
        };
        if self.ability_code(alias)? != self.powerup_class {
            return Err("powerup ability class does not match the client registration".into());
        }
        self.single_level(alias)?;
        let agility = self.amount(alias, &self.agility)?;
        let intelligence = self.amount(alias, &self.intelligence)?;
        if agility != 0 || intelligence != 0 {
            return Err("agility or intelligence powerup effects are unsupported".into());
        }
        let strength = self.amount(alias, &self.strength)?;
        if strength < 0 {
            return Err("negative strength powerup is unsupported".into());
        }
        Ok(Some(StrengthEffect { strength }))
    }

    /// Passive attributes, armor, attack damage, maximum life and regeneration. `None` for
    /// powerups and usable items: a usable item is held without activation, so its abilities add
    /// no passive attribute here. Any passive ability outside the admitted classes refuses.
    pub fn held_attributes(
        &self,
        definition: &Definition,
    ) -> Result<Option<AttributeBonus>, String> {
        let item = self.item_row(definition)?;
        if item_flag(definition, item, "powerup")? || item_flag(definition, item, "usable")? {
            return Ok(None);
        }
        let abilities = item_text(definition, item, "abilList")?;
        let mut held = AttributeBonus::default();
        for alias in abilities
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty() && *a != "_")
        {
            let code = self.ability_code(alias)?;
            self.single_level(alias)?;
            if code == self.armor_class {
                let points = self.amount(alias, &self.armor)?;
                if points < 0 {
                    return Err(format!("negative armor on {}", item.rawcode()));
                }
                held.armor = held
                    .armor
                    .checked_add(points)
                    .ok_or("passive armor overflows")?;
                continue;
            }
            if code == self.damage_class {
                // The `AIat` attach (0xbc2a20) reads `DataA1` of any row with this class code and
                // never consults the metadata `useSpecific` list, which omits the shipped `AItx`.
                let points = self.amount(alias, &self.damage)?;
                if points < 0 {
                    return Err(format!("negative damage on {}", item.rawcode()));
                }
                held.damage = held
                    .damage
                    .checked_add(points)
                    .ok_or("passive damage overflows")?;
                continue;
            }
            if let Some(max_life) = &self.max_life
                && code == max_life.class
            {
                let points = self.amount(alias, &max_life.amount)?;
                if points < 0 {
                    return Err(format!("negative max-life on {}", item.rawcode()));
                }
                held.max_life = held
                    .max_life
                    .checked_add(points)
                    .ok_or("passive max-life overflows")?;
                continue;
            }
            if let Some(max_mana) = &self.max_mana
                && code == max_mana.class
            {
                let points = self.amount(alias, &max_mana.amount)?;
                if points < 0 {
                    return Err(format!("negative max-mana on {}", item.rawcode()));
                }
                held.max_mana = held
                    .max_mana
                    .checked_add(points)
                    .ok_or("passive max-mana overflows")?;
                continue;
            }
            if let Some(regen) = &self.life_regen
                && code == regen.class
            {
                let points = self.amount(alias, &regen.amount)?;
                if points < 0 {
                    return Err(format!("negative life regeneration on {}", item.rawcode()));
                }
                held.life_regen = held
                    .life_regen
                    .checked_add(points)
                    .ok_or("passive life regeneration overflows")?;
                continue;
            }
            if let Some(regen) = &self.mana_regen
                && code == regen.class
            {
                let fraction = self.fraction(alias, &regen.amount)?;
                if fraction < 0.0 {
                    return Err(format!("negative mana regeneration on {}", item.rawcode()));
                }
                let sum = held.mana_regen_fraction + fraction;
                if !sum.is_finite() {
                    return Err("passive mana regeneration overflows".into());
                }
                held.mana_regen_fraction = sum;
                continue;
            }
            if let Some(orbs) = &self.orbs {
                let family = if code == orbs.fire {
                    Some(OrbFamily::Fire)
                } else if code == orbs.corruption {
                    Some(OrbFamily::Corruption)
                } else if orbs.frost.as_ref().is_some_and(|frost| frost.class == code) {
                    Some(OrbFamily::Frost)
                } else if orbs.venom.as_ref().is_some_and(|venom| venom.orb == code) {
                    Some(OrbFamily::Venom)
                } else if orbs.spell.as_ref().is_some_and(|spell| spell.class == code) {
                    Some(OrbFamily::Spell)
                } else if orbs.purge.as_ref().is_some_and(|purge| purge.orb == code) {
                    Some(OrbFamily::Lightning)
                } else {
                    None
                };
                if let Some(venom) = &orbs.venom
                    && venom.poison == code
                {
                    // The poison ability is read through the venom orb that triggers it: the
                    // orb handler finds it on the owner by the item that carries both.
                    let orbs_listed = self.aliases_of_class(&abilities, &venom.orb)?;
                    if orbs_listed.len() != 1 {
                        return Err(format!(
                            "poison ability {alias} on {} needs exactly one {} ability beside it",
                            item.rawcode(),
                            venom.orb
                        ));
                    }
                    continue;
                }
                if let Some(purge) = &orbs.purge
                    && purge.sibling == code
                {
                    // The lightning purge is read through the orb that casts it: the orb
                    // handler finds it on the owner by its class.
                    let orbs_listed = self.aliases_of_class(&abilities, &purge.orb)?;
                    if orbs_listed.len() != 1 {
                        return Err(format!(
                            "lightning purge {alias} on {} needs exactly one {} ability beside it",
                            item.rawcode(),
                            purge.orb
                        ));
                    }
                    continue;
                }
                if let Some(family) = family {
                    if held.orb.is_some() {
                        return Err(format!(
                            "two orb abilities on {} are unsupported",
                            item.rawcode()
                        ));
                    }
                    let orb = self.orb(alias, family, &abilities)?;
                    held.damage = held
                        .damage
                        .checked_add(orb.damage)
                        .ok_or("passive damage overflows")?;
                    held.orb = Some(orb);
                    continue;
                }
            }
            if let Some(speed) = &self.attack_speed
                && code == speed.class
            {
                let bonus = self.fraction(alias, &speed.amount)?;
                let sum = held.attack_speed + bonus;
                if !sum.is_finite() {
                    return Err("passive attack speed overflows".into());
                }
                held.attack_speed = sum;
                continue;
            }
            if self.spell_resistance.as_deref() == Some(code.as_str()) {
                let (damage, taken) = self.spell_resistance(alias)?;
                held.damage = held
                    .damage
                    .checked_add(damage)
                    .ok_or("passive damage overflows")?;
                held.spell_damage_taken = Some(held.spell_damage_taken.unwrap_or(1.0) * taken);
                continue;
            }
            if let Some(class) = &self.detector
                && code == class.class
            {
                if held.detector.is_some() {
                    return Err(format!(
                        "two detector abilities on {} are unsupported",
                        item.rawcode()
                    ));
                }
                held.detector = Some(self.detector_row(alias, class)?);
                continue;
            }
            if let Some(class) = &self.spell_shield
                && code == class.class
            {
                if held.spell_shield.is_some() {
                    return Err(format!(
                        "two spell shield abilities on {} are unsupported",
                        item.rawcode()
                    ));
                }
                held.spell_shield = Some(self.spell_shield_row(alias)?);
                continue;
            }
            if let Some(classes) = &self.reincarnation
                && code == classes.ankh
            {
                if held.reincarnation.is_some() {
                    return Err(format!(
                        "two reincarnation abilities on {} are unsupported",
                        item.rawcode()
                    ));
                }
                held.reincarnation = Some(self.reincarnation_row(alias)?);
                continue;
            }
            if let Some(class) = &self.life_steal
                && code == class.class
            {
                let Some(misc) = class.misc else {
                    return Err(format!(
                        "life steal {alias} reads Misc/{LIFE_STEAL_ILLUSION_KEY}, which this \
                         rule set was not given"
                    ));
                };
                if misc.magic_immune_veto {
                    return Err(format!(
                        "life steal {alias}: Misc/{LIFE_STEAL_MAGIC_IMMUNE_KEY} is set, so the \
                         client vetoes it on magic immune targets through the immunity counter \
                         at unit +0x264, which is not modelled"
                    ));
                }
                if held.life_steal.is_some() {
                    return Err(format!(
                        "two life-steal abilities on {} are unsupported: the client arms one \
                         per attack",
                        item.rawcode()
                    ));
                }
                let (fraction, targets) = self.life_steal_row(alias)?;
                held.life_steal = Some(LifeSteal {
                    fraction,
                    targets,
                    illusions_restore: misc.illusions_restore,
                });
                continue;
            }
            if let Some(class) = &self.move_speed_bonus
                && code == class.class
            {
                match class.stacks {
                    Some(skipped) => held.move_speed_refresh_skipped |= skipped,
                    None => {
                        return Err(format!(
                            "speed bonus {alias} reads Misc/{MOVE_SPEED_STACK_KEY}, which this \
                             rule set was not given"
                        ));
                    }
                }
                let value = self.move_speed_bonus_value(alias)?;
                held.move_speed_bonus = Some(held.move_speed_bonus.map_or(value, |v| v.max(value)));
                continue;
            }
            if self.is_aura_class(&code) {
                // The aura is hosted per item by `aura_host.rs`; the row is read there through
                // `held_aura`, and an unsupported row refuses the pickup here.
                self.aura_row(alias)?;
                continue;
            }
            if code != self.passive_class {
                let reason = UNMODELED_PASSIVE_CLASSES
                    .iter()
                    .find(|(class, _)| *class == code)
                    .map_or(String::new(), |(_, reason)| format!(": {reason}"));
                return Err(format!(
                    "unsupported passive ability {alias} with code {code} on {}{reason}",
                    item.rawcode()
                ));
            }
            held.strength = held
                .strength
                .checked_add(self.amount(alias, &self.strength)?)
                .ok_or("passive strength overflows")?;
            held.agility = held
                .agility
                .checked_add(self.amount(alias, &self.agility)?)
                .ok_or("passive agility overflows")?;
            held.intelligence = held
                .intelligence
                .checked_add(self.amount(alias, &self.intelligence)?)
                .ok_or("passive intelligence overflows")?;
        }
        Ok(Some(held))
    }

    /// The ability row of an orb or spell-resistance item at the effect level. The reviewed
    /// client reads these fields straight from the row, so the amounts do not go through the
    /// attribute metadata admission (which omits `AIgd`). A map that modifies the row is
    /// refused, because the overrides of these fields are not decoded.
    fn effect_row(&self, alias: &str, what: &str) -> Result<Object<'_>, String> {
        let modified =
            self.overlay.new_object(alias).is_some() || !self.overlay.on(alias).is_empty();
        if modified {
            return Err(format!(
                "map-modified {what} ability {alias} is unsupported"
            ));
        }
        self.tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())
    }

    /// The aliases in the comma list `abilities` whose row code is `class`.
    fn aliases_of_class(&self, abilities: &str, class: &str) -> Result<Vec<String>, String> {
        let mut found = Vec::new();
        for alias in abilities
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty() && *a != "_")
        {
            if self.ability_code(alias)? == class {
                found.push(alias.to_owned());
            }
        }
        Ok(found)
    }

    /// The poison row an Orb of Venom triggers at the effect level: damage per second, the
    /// durations and the buff, as the poison class reads them (`0xca2d70`). `DataB` and `DataC`
    /// (move and attack slow) must be zero, because the recreation does not host those speed
    /// modifiers for poison; `DataD` must select the stacking mode.
    fn poison(&self, alias: &str, first_tick_delay: f32) -> Result<(OrbKind, OrbTargets), String> {
        let ability = self.effect_row(alias, "poison")?;
        let cell = |name: &str| {
            ability
                .cell(SlkId::AbilityData, name)
                .map_err(|e| e.to_string())
        };
        let real = |name: &str| match cell(name)?.typed(FieldKind::Real) {
            Ok(Value::Real(value)) if value.is_finite() => Ok(value as f32),
            _ => Err(format!("{name} of poison {alias} is not a finite real")),
        };
        // An unset cell reads as zero in the level record.
        let real_or_zero = |name: &str| {
            if matches!(cell(name)?.text.trim(), "" | "-" | "_") {
                Ok(0.0)
            } else {
                real(name)
            }
        };
        let damage_per_second = real("DataA1")?;
        if damage_per_second <= 0.0 {
            return Err(format!(
                "poison {alias} DataA1 is not positive; a heal over time is not decoded"
            ));
        }
        if real_or_zero("DataB1")? != 0.0 || real_or_zero("DataC1")? != 0.0 {
            return Err(format!(
                "poison {alias} slows move or attack speed (DataB1, DataC1), which is not hosted"
            ));
        }
        let mode = match cell("DataD1")?.typed(FieldKind::Int) {
            Ok(Value::Integer(n)) => n,
            _ => return Err(format!("DataD1 of poison {alias} is not an integer")),
        };
        if mode & POISON_MODE_MASK == 0 || mode & !(POISON_MODE_MASK | POISON_SECOND_BUFF_BIT) != 0
        {
            return Err(format!(
                "poison {alias} DataD1 {mode} is not a decoded stacking mode"
            ));
        }
        let (duration, hero_duration) = (real("Dur1")?, real("HeroDur1")?);
        if duration <= 0.0 || hero_duration <= 0.0 {
            return Err(format!(
                "poison {alias} duration is not positive; a buff without a timer is not decoded"
            ));
        }
        let index = usize::from(mode & POISON_SECOND_BUFF_BIT != 0);
        let buff: [u8; 4] = cell("BuffID1")?
            .text
            .split(',')
            .nth(index)
            .map(|code| code.trim().as_bytes().to_vec())
            .and_then(|code| code.try_into().ok())
            .filter(|code: &[u8; 4]| code.iter().all(u8::is_ascii_alphanumeric))
            .ok_or_else(|| format!("poison {alias} BuffID1 entry {index} is not a rawcode"))?;
        let targets = orb_targets(alias, cell("targs1")?.text)?;
        let kind = OrbKind::Poison {
            damage_per_second,
            duration,
            hero_duration,
            first_tick_delay,
            buff,
        };
        Ok((kind, targets))
    }

    /// One orb ability row at the effect level. `abilities` is the item's ability list, which
    /// the venom orb searches for its poison ability.
    fn orb(&self, alias: &str, family: OrbFamily, abilities: &str) -> Result<Orb, String> {
        let ability = self.effect_row(alias, "orb")?;
        let cell = |name: &str| {
            ability
                .cell(SlkId::AbilityData, name)
                .map_err(|e| e.to_string())
        };
        let int = |name: &str| match cell(name)?.typed(FieldKind::Int) {
            Ok(Value::Integer(n)) => Ok(n),
            _ => Err(format!("{name} of orb {alias} is not an integer")),
        };
        let real = |name: &str| match cell(name)?.typed(FieldKind::Real) {
            Ok(Value::Real(value)) if value.is_finite() => Ok(value as f32),
            _ => Err(format!("{name} of orb {alias} is not a finite real")),
        };
        let timer = || -> Result<(f32, f32, [u8; 4]), String> {
            let (duration, hero_duration) = (real("Dur1")?, real("HeroDur1")?);
            if duration <= 0.0 || hero_duration <= 0.0 {
                return Err(format!(
                    "orb {alias} duration is not positive; a buff without a timer is not decoded"
                ));
            }
            let buff = cell("BuffID1")?.text.trim().as_bytes().to_vec();
            let buff: [u8; 4] = buff
                .try_into()
                .ok()
                .filter(|code: &[u8; 4]| code.iter().all(u8::is_ascii_alphanumeric))
                .ok_or_else(|| format!("orb {alias} BuffID1 is not a rawcode"))?;
            Ok((duration, hero_duration, buff))
        };
        let air_switch = match int("DataE1")? {
            ORB_NO_ATTACK_INDEX => false,
            ORB_ENABLED_ATTACK_INDEX => true,
            _ => {
                return Err(format!(
                    "orb {alias} enabled attack index is not {ORB_NO_ATTACK_INDEX} or {ORB_ENABLED_ATTACK_INDEX}"
                ));
            }
        };
        let damage = i32::try_from(int("DataA1")?)
            .ok()
            .filter(|damage| *damage >= 0)
            .ok_or_else(|| format!("orb {alias} damage is negative or too large"))?;
        let targets = orb_targets(alias, cell("targs1")?.text)?;
        let kind = match family {
            OrbFamily::Corruption => {
                let armor_penalty = i32::try_from(int("DataB1")?)
                    .ok()
                    .filter(|penalty| *penalty >= 0)
                    .ok_or_else(|| format!("orb {alias} armor penalty is negative or too large"))?;
                let (duration, hero_duration, buff) = timer()?;
                OrbKind::Corruption {
                    armor_penalty,
                    duration,
                    hero_duration,
                    buff,
                }
            }
            OrbFamily::Frost => {
                let (move_decrease, attack_decrease) = self
                    .orbs
                    .as_ref()
                    .and_then(|orbs| orbs.frost.as_ref())
                    .and_then(|frost| frost.decreases)
                    .ok_or_else(|| {
                        format!(
                            "orb {alias} reads Misc/{FROST_MOVE_KEY} and Misc/{FROST_ATTACK_KEY}, \
                             which this rule set was not given"
                        )
                    })?;
                let (duration, hero_duration, buff) = timer()?;
                OrbKind::Frost {
                    move_decrease,
                    attack_decrease,
                    duration,
                    hero_duration,
                    buff,
                }
            }
            OrbFamily::Spell => OrbKind::Spell(self.spell_orb(alias)?),
            OrbFamily::Lightning => {
                let (purge, purge_targets) = self.lightning_purge(alias, abilities)?;
                // The orb arms on its own class list and the purge vetoes on its own; a pair
                // that differs would arm a hit the purge then refuses, which is not decoded.
                if purge_targets != targets {
                    return Err(format!(
                        "orb {alias} and its purge list different target classes"
                    ));
                }
                OrbKind::Lightning(purge)
            }
            OrbFamily::Fire => {
                let area = real("Area1")?;
                if area <= 0.0 {
                    return Err(format!("orb {alias} Area1 is not positive"));
                }
                OrbKind::Fire { area }
            }
            OrbFamily::Venom => {
                let venom = self
                    .orbs
                    .as_ref()
                    .and_then(|orbs| orbs.venom.as_ref())
                    .ok_or_else(|| format!("orb {alias} needs the venom registration"))?;
                let poisons = self.aliases_of_class(abilities, &venom.poison)?;
                let [poison_alias] = poisons.as_slice() else {
                    return Err(format!(
                        "orb {alias} needs exactly one {} ability on its item",
                        venom.poison
                    ));
                };
                let (kind, poison_targets) = self.poison(poison_alias, venom.first_tick_delay)?;
                // The orb arms on its own class list and the poison class vetoes on its own; a
                // pair that differs would arm a hit the poison then refuses, which is not decoded.
                if poison_targets != targets {
                    return Err(format!(
                        "orb {alias} and poison {poison_alias} list different target classes"
                    ));
                }
                kind
            }
        };
        Ok(Orb {
            kind,
            damage,
            targets,
            air_switch,
        })
    }

    /// One spell-resistance row: the flat attack damage its `AIat` base adds (`DataA1`) and the
    /// factor on spell damage taken, `1 - DataB1`. A fraction outside `[0, 1]` refuses.
    fn spell_resistance(&self, alias: &str) -> Result<(i32, f32), String> {
        let ability = self.effect_row(alias, "spell-resistance")?;
        let cell = |name: &str| {
            ability
                .cell(SlkId::AbilityData, name)
                .map_err(|e| e.to_string())
        };
        let damage = match cell("DataA1")?.typed(FieldKind::Int) {
            Ok(Value::Integer(n)) => i32::try_from(n).ok().filter(|n| *n >= 0),
            _ => None,
        }
        .ok_or_else(|| format!("DataA1 of {alias} is not a nonnegative integer"))?;
        let reduction = match cell("DataB1")?.typed(FieldKind::Real) {
            Ok(Value::Real(value)) if (0.0..=1.0).contains(&value) => value as f32,
            _ => return Err(format!("DataB1 of {alias} is not a fraction in [0, 1]")),
        };
        Ok((damage, 1.0 - reduction))
    }

    /// An `AIva` row at the effect level: the life-steal fraction `DataA1` (the attach `0xbc2170`
    /// reads it by class, and the hit hook `0xbc69b0` multiplies the damage by it) and the unit
    /// classes of `targs1`. A negative or nonfinite fraction refuses, and so does a map-modified
    /// row, because the overrides of these fields are not decoded.
    fn life_steal_row(&self, alias: &str) -> Result<(f32, OrbTargets), String> {
        let ability = self.effect_row(alias, "life-steal")?;
        let cell = |name: &str| {
            ability
                .cell(SlkId::AbilityData, name)
                .map_err(|e| e.to_string())
        };
        let fraction = match cell("DataA1")?.typed(FieldKind::Real) {
            Ok(Value::Real(value)) if value.is_finite() && value >= 0.0 => value as f32,
            _ => {
                return Err(format!(
                    "DataA1 of {alias} is not a finite nonnegative fraction"
                ));
            }
        };
        Ok((fraction, life_steal_targets(alias, cell("targs1")?.text)?))
    }

    /// The flat bonus of an `AIms` row: `DataA1`. Any finite real is admitted; a negative one never
    /// wins the maximum the client takes from zero.
    fn move_speed_bonus_value(&self, alias: &str) -> Result<f32, String> {
        let ability = self.effect_row(alias, "speed-bonus")?;
        let cell = ability
            .cell(SlkId::AbilityData, "DataA1")
            .map_err(|e| e.to_string())?;
        match cell.typed(FieldKind::Real) {
            Ok(Value::Real(value)) if value.is_finite() => Ok(value as f32),
            _ => Err(format!("DataA1 of {alias} is not a finite real")),
        }
    }

    /// Authored real fraction at the effect level, after the map's W3A override for that field.
    fn fraction(&self, alias: &str, column: &FractionColumn) -> Result<f32, String> {
        let mods = self
            .overlay
            .new_object(alias)
            .map(|(_, m)| m)
            .unwrap_or_else(|| self.overlay.on(alias));
        if let Some(entry) = mods
            .iter()
            .rev()
            .find(|m| m.field == column.field && m.selectors.is_none_or(|s| s[0] == EFFECT_LEVEL))
        {
            if let Some([_, pointer]) = entry.selectors
                && pointer != column.data
            {
                return Err(format!(
                    "{} override data pointer {pointer} does not match metadata data column {}",
                    column.label, column.data
                ));
            }
            return match entry.value {
                pcx_objects::Value::Real(value) | pcx_objects::Value::Unreal(value)
                    if value.is_finite() =>
                {
                    Ok(value)
                }
                _ => Err(format!("nonreal {} override", column.label)),
            };
        }
        let ability = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?;
        let letter =
            char::from_u32(u32::from(b'A') + column.data - 1).ok_or("fraction data column")?;
        let cell = ability
            .cell(SlkId::AbilityData, &format!("Data{letter}{EFFECT_LEVEL}"))
            .map_err(|e| e.to_string())?;
        match cell.typed(FieldKind::Real).map_err(|e| e.to_string())? {
            Value::Real(value) if value.is_finite() => Ok(value as f32),
            _ => Err(format!("{} field is not real", column.label)),
        }
    }

    fn item_row(&self, definition: &Definition) -> Result<Object<'_>, String> {
        if definition.kind != ObjectKind::Item {
            return Err(format!(
                "{}: not an item definition",
                definition.base_rawcode
            ));
        }
        // The definition is already resolved through the map's W3T: a custom item reads its
        // stock base row, and `item_text` applies the authored overrides on top.
        self.tables
            .object(Family::Item, &definition.base_rawcode)
            .map_err(|e| e.to_string())
    }

    fn ability_code(&self, alias: &str) -> Result<String, String> {
        let ability = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?;
        Ok(ability
            .cell(SlkId::AbilityData, "code")
            .map_err(|e| e.to_string())?
            .text
            .trim()
            .to_owned())
    }

    /// The effect reads one authored level, so the source row must declare exactly that level.
    fn single_level(&self, alias: &str) -> Result<(), String> {
        let levels = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?
            .cell(SlkId::AbilityData, "levels")
            .map_err(|e| e.to_string())?
            .text
            .trim()
            .parse::<u32>();
        if levels != Ok(EFFECT_LEVEL) {
            return Err(format!(
                "multi-level attribute ability {alias} is unsupported"
            ));
        }
        Ok(())
    }

    /// Authored amount at the effect level, after the map's W3A override for that field.
    fn amount(&self, alias: &str, column: &Column) -> Result<i32, String> {
        let mods = self
            .overlay
            .new_object(alias)
            .map(|(_, m)| m)
            .unwrap_or_else(|| self.overlay.on(alias));
        if let Some(entry) = mods
            .iter()
            .rev()
            .find(|m| m.field == column.field && m.selectors.is_none_or(|s| s[0] == EFFECT_LEVEL))
        {
            if let Some([_, pointer]) = entry.selectors
                && pointer != column.data
            {
                return Err(format!(
                    "attribute override data pointer {pointer} does not match metadata data column {}",
                    column.data
                ));
            }
            let pcx_objects::Value::Int(n) = entry.value else {
                return Err("noninteger attribute override".into());
            };
            return i32::try_from(n).map_err(|_| "attribute override exceeds integer".into());
        }
        let ability = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?;
        let letter = char::from_u32(u32::from(b'A') + column.data - 1).ok_or("attribute column")?;
        let cell = ability
            .cell(SlkId::AbilityData, &format!("Data{letter}{EFFECT_LEVEL}"))
            .map_err(|e| e.to_string())?;
        match cell.typed(FieldKind::Int).map_err(|e| e.to_string())? {
            Value::Integer(n) => {
                i32::try_from(n).map_err(|_| "attribute value exceeds integer".into())
            }
            _ => Err("attribute field is not integral".into()),
        }
    }
}

/// An `ItemData` column of the selected definition: the map's override when it authored one,
/// otherwise the stock base-row cell. An indexed override refuses.
fn item_text(definition: &Definition, item: Object<'_>, key: &str) -> Result<String, String> {
    let mut fields = definition.fields.values().filter(|field| {
        field.metadata.as_ref().is_some_and(|metadata| {
            matches!(
                &metadata.source,
                FieldSource::Column { slk: SlkId::ItemData, column } if column == key
            )
        })
    });
    match (fields.next(), fields.next()) {
        (Some(_), Some(_)) => Err(format!("ambiguous ItemData/{key} field")),
        (Some(field), None) => match &field.value {
            Ok(FieldValue::Override(pcx_objects::Value::Int(n))) => Ok(n.to_string()),
            Ok(FieldValue::Override(pcx_objects::Value::Str(text))) => std::str::from_utf8(text)
                .map(|text| text.trim().to_owned())
                .map_err(|_| format!("non-UTF8 ItemData/{key} override")),
            Ok(FieldValue::Override(_)) => Err(format!("unsupported ItemData/{key} override type")),
            Err(FieldProblem::Indexed(_)) => Err(format!("indexed ItemData/{key} override")),
            Ok(FieldValue::Cell(_)) | Err(_) => item_cell(item, key),
        },
        (None, _) => item_cell(item, key),
    }
}

fn item_cell(item: Object<'_>, key: &str) -> Result<String, String> {
    item.cell(SlkId::ItemData, key)
        .map(|c| c.text.trim().to_owned())
        .map_err(|e| e.to_string())
}

fn item_flag(definition: &Definition, item: Object<'_>, key: &str) -> Result<bool, String> {
    match item_text(definition, item, key)?.as_str() {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(format!("item {key} is not a boolean flag")),
    }
}

/// Source and base ability classes from a registration call. The callee (`0x7f6360`) reads cdecl
/// stack arguments: first (`[ebp+8]`, last push) hashes into the source-class registry entry,
/// second (`[ebp+0xc]`) into the base entry; the third is an unnamed pointer.
fn class_from_window(window: &[u8]) -> Result<(String, String), String> {
    let fail = || "unsupported class registration window".to_string();
    // Three pushed immediates, one call, stack cleanup and return.
    if window.len() != CLASS_WINDOW_LEN
        || [window[0], window[5], window[10]] != [PUSH_IMM32; 3]
        || window[15] != CALL_REL32
        || window[20..] != CLEANUP_RET
    {
        return Err(fail());
    }
    let key = |at: usize| -> Result<String, String> {
        let mut text: [u8; 4] = window[at..at + 4].try_into().map_err(|_| fail())?;
        if !text.iter().all(u8::is_ascii_alphanumeric) {
            return Err(fail());
        }
        // Immediates are little-endian, so the characters read in reverse.
        text.reverse();
        String::from_utf8(text.to_vec()).map_err(|_| fail())
    };
    Ok((key(11)?, key(6)?))
}

/// Target of the `call rel32` at `at` inside a registration window, given the window's own address.
fn call_target(window: &[u8], window_va: u32, at: usize) -> Result<u32, String> {
    let fail = || "unsupported registration call".to_string();
    if window.get(at) != Some(&CALL_REL32) {
        return Err(fail());
    }
    let rel: [u8; 4] = window
        .get(at + 1..at + 5)
        .ok_or_else(fail)?
        .try_into()
        .map_err(|_| fail())?;
    let next = i64::from(window_va) + at as i64 + 5;
    u32::try_from(next + i64::from(i32::from_le_bytes(rel))).map_err(|_| fail())
}

/// [`class_from_call_window_at`] for the healing window.
fn class_from_call_window(window: &[u8], base_code: &[u8]) -> Result<(String, String), String> {
    class_from_call_window_at(window, HEAL_WINDOW, base_code)
}

/// Source class from a registration window with a computed base, located at `window_va`. The
/// window pushes the third argument, calls the base function, pushes its result, pushes the
/// source class, calls the registration function (`0x7f6360`), cleans up and returns. The base
/// function must be `mov eax, imm32; ret`, and its immediate is the base class.
fn class_from_call_window_at(
    window: &[u8],
    window_va: u32,
    base_code: &[u8],
) -> Result<(String, String), String> {
    let fail = || "unsupported computed-base registration window".to_string();
    if window.len() != HEAL_WINDOW_LEN
        || window[0] != PUSH_IMM32
        || window[5] != CALL_REL32
        || window[10] != PUSH_EAX
        || window[11] != PUSH_IMM32
        || window[21..] != CLEANUP_RET
        || call_target(window, window_va, 16)? != REGISTER_FN
        || base_code.len() != BASE_FN_LEN
        || base_code[0] != MOV_EAX_IMM32
        || base_code[5] != RET
    {
        return Err(fail());
    }
    Ok((fourcc_le(&window[12..16])?, fourcc_le(&base_code[1..5])?))
}

/// Unit-class tokens of an orb's `targs1`. The decoded flag word adds the enemies bit and takes
/// the class bits from this list (`ground` 0x2, `air` 0x4, `structure` 0x8, `organic` 0x4000000). Tokens that select
/// destructables, items or wards are accepted and have no effect on unit targets; relationship and
/// state tokens, which the reviewed client resolves in the target query, are refused.
fn orb_targets(alias: &str, targs: &str) -> Result<OrbTargets, String> {
    let mut targets = OrbTargets::default();
    let mut any = false;
    for token in targs.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        any = true;
        match token.to_ascii_lowercase().as_str() {
            "ground" => targets.ground = true,
            "air" => targets.air = true,
            "structure" => targets.structure = true,
            "organic" => targets.organic = true,
            "ward" | "debris" | "item" | "tree" | "wall" | "decoration" | "bridge" => {}
            other => {
                return Err(format!("orb {alias} targs1 token {other:?} is not decoded"));
            }
        }
    }
    if any {
        Ok(targets)
    } else {
        Err(format!("orb {alias} targs1 is empty"))
    }
}

/// Unit classes of a life-steal row's `targs1`: the class tokens an orb reads, plus the `enemy`
/// token, which the hook's allowed-target test (`0x682450`, over the row's own `[this+0x4c]`
/// word) needs because the row authors it. A row without `enemy` refuses, as the ally case is
/// not decoded.
fn life_steal_targets(alias: &str, targs: &str) -> Result<OrbTargets, String> {
    let mut enemy = false;
    let classes: Vec<&str> = targs
        .split(',')
        .map(str::trim)
        .filter(|token| {
            let is_enemy = token.eq_ignore_ascii_case("enemy");
            enemy |= is_enemy;
            !is_enemy
        })
        .collect();
    if !enemy {
        return Err(format!(
            "life steal {alias} targs1 does not list enemy, which is the only decoded case"
        ));
    }
    orb_targets(alias, &classes.join(",")).map_err(|e| e.replacen("orb ", "life steal ", 1))
}

/// Four alphanumeric characters from a little-endian immediate.
fn fourcc_le(bytes: &[u8]) -> Result<String, String> {
    let fail = || "unsupported registration immediate".to_string();
    let mut text: [u8; 4] = bytes.try_into().map_err(|_| fail())?;
    if !text.iter().all(u8::is_ascii_alphanumeric) {
        return Err(fail());
    }
    text.reverse();
    String::from_utf8(text.to_vec()).map_err(|_| fail())
}

fn column(tables: &DefaultTables, id: &[u8; 4]) -> Result<Column, String> {
    let name = std::str::from_utf8(id).map_err(|_| "invalid attribute metadata id")?;
    let metadata = tables
        .metadata(*id)
        .ok_or_else(|| format!("{name} metadata missing"))?;
    let text = |key: &str| {
        metadata
            .attribute(key)
            .map(|c| c.text.trim().to_owned())
            .ok_or_else(|| format!("{name} metadata {key} missing"))
    };
    if text("field")? != "Data" || text("slk")? != "AbilityData" || text("type")? != "int" {
        return Err(format!("unsupported {name} metadata mapping"));
    }
    let data: u32 = text("data")?
        .parse()
        .map_err(|_| format!("invalid {name} data index"))?;
    if !(1..=DATA_COLUMNS).contains(&data) {
        return Err(format!("{name} data index outside column alphabet"));
    }
    Ok(Column { field: *id, data })
}

/// A real-valued metadata column: the same `Data` mapping as [`column`], with a real or unreal
/// authored type instead of an integer type.
fn fraction_column(
    tables: &DefaultTables,
    id: &[u8; 4],
    label: &'static str,
) -> Result<FractionColumn, String> {
    let name = std::str::from_utf8(id).map_err(|_| "invalid fraction metadata id")?;
    let metadata = tables
        .metadata(*id)
        .ok_or_else(|| format!("{name} metadata missing"))?;
    let text = |key: &str| {
        metadata
            .attribute(key)
            .map(|c| c.text.trim().to_owned())
            .ok_or_else(|| format!("{name} metadata {key} missing"))
    };
    if text("field")? != "Data"
        || text("slk")? != "AbilityData"
        || !matches!(text("type")?.as_str(), "real" | "unreal")
    {
        return Err(format!("unsupported {name} metadata mapping"));
    }
    let data: u32 = text("data")?
        .parse()
        .map_err(|_| format!("invalid {name} data index"))?;
    if !(1..=DATA_COLUMNS).contains(&data) {
        return Err(format!("{name} data index outside column alphabet"));
    }
    Ok(FractionColumn {
        field: *id,
        data,
        label,
    })
}

#[path = "inventory_detector.rs"]
pub(crate) mod detector;
pub use detector::Detector;
use detector::DetectorClass;
pub(crate) use detector::DetectorImage;
#[path = "inventory_spell_shield.rs"]
pub(crate) mod spell_shield;
pub use spell_shield::SpellShield;
use spell_shield::SpellShieldClass;
pub(crate) use spell_shield::SpellShieldImage;
#[path = "inventory_reincarnation.rs"]
pub(crate) mod reincarnation;
pub use reincarnation::Reincarnation;
use reincarnation::ReincarnationClasses;
pub(crate) use reincarnation::ReincarnationImage;
#[path = "inventory_aura.rs"]
pub(crate) mod aura;
use aura::AuraClasses;
pub(crate) use aura::AuraImage;
pub use aura::{AuraEffect, AuraTiming, HeldAura};
#[path = "inventory_purge.rs"]
pub(crate) mod purge;
use purge::PurgeClasses;
pub(crate) use purge::PurgeImage;
pub use purge::{Purge, PurgeTargets, SlowDecay};
#[path = "inventory_spell_orb.rs"]
pub(crate) mod spell_orb;
use spell_orb::SpellClass;
pub(crate) use spell_orb::SpellOrbImage;
pub use spell_orb::{BlackArrow, SpellOrb, WrappedAbility, WrappedTargets};
pub(crate) use spell_orb::{ProcCounters, ProcKind};

#[cfg(test)]
#[path = "inventory_powerup_tests.rs"]
mod tests;
