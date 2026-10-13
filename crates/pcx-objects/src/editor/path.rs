//! What ships where: the archive paths of the object-editor data.
//!
//! Every path here is spelled exactly as the archive listfile spells it,
//! because the census is what proves them. `Units\unitUI.slk` keeps its
//! lower-case `unit`; nothing normalises a path before it is asked for.

/// One object-editor `.slk`, and where the archives carry it.
///
/// The order is the load order; [`SlkId::id_column`] is the row-1 column that
/// holds a raw code, or `None` for a table that is not keyed by one (the
/// metadata tables are keyed by fourcc instead, through [`SlkId::code_column`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SlkId {
    /// `Units\UnitData.slk` — race, movement, formation, cargo, points.
    UnitData,
    /// `Units\unitUI.slk` — model, icon-name, selection, walk/run speed, armour
    /// type, team colour.
    UnitUi,
    /// `Units\UnitBalance.slk` — everything number-shaped: cost, HP, mana,
    /// regen, armour, speed, build time, sight, primary attribute, bounty.
    UnitBalance,
    /// `Units\UnitWeapons.slk` — both attack slots of every unit.
    UnitWeapons,
    /// `Units\UnitAbilities.slk` — the ability lists of every unit.
    UnitAbilities,
    /// `Units\ItemData.slk` — item class, cost, HP, armour, abilities, stock.
    ItemData,
    /// `Units\UpgradeData.slk` — upgrade cost, level cap, effects.
    UpgradeData,
    /// `Units\AbilityData.slk` — every ability's per-level data.
    AbilityData,
    /// `Units\UnitMetaData.slk` — the field schema for units and items.
    UnitMetaData,
    /// `Units\AbilityMetaData.slk` — the field schema for abilities.
    AbilityMetaData,
    /// `Units\UpgradeMetaData.slk` — the field schema for upgrades.
    UpgradeMetaData,
    /// `Units\UpgradeEffectMetaData.slk` — the schema for upgrade effects.
    UpgradeEffectMetaData,
    /// `Units\DestructableData.slk` — destructables.
    DestructableData,
    /// `Units\DestructableMetaData.slk` — the schema for destructables.
    DestructableMetaData,
    /// `Units\MiscMetaData.slk` — the shared misc field schema.
    MiscMetaData,
    /// `Units\AbilityBuffData.slk` — buffs.
    AbilityBuffData,
    /// `Units\AbilityBuffMetaData.slk` — the schema for buffs.
    AbilityBuffMetaData,
    /// `Doodads\Doodads.slk` — doodads.
    Doodads,
    /// `Doodads\DoodadMetaData.slk` — the schema for doodads.
    DoodadMetaData,
}

impl SlkId {
    /// Every table this crate decodes, in load order.
    pub const ALL: &'static [SlkId] = &[
        SlkId::UnitData,
        SlkId::UnitUi,
        SlkId::UnitBalance,
        SlkId::UnitWeapons,
        SlkId::UnitAbilities,
        SlkId::ItemData,
        SlkId::UpgradeData,
        SlkId::AbilityData,
        SlkId::UnitMetaData,
        SlkId::AbilityMetaData,
        SlkId::UpgradeMetaData,
        SlkId::UpgradeEffectMetaData,
        SlkId::DestructableData,
        SlkId::DestructableMetaData,
        SlkId::MiscMetaData,
        SlkId::AbilityBuffData,
        SlkId::AbilityBuffMetaData,
        SlkId::Doodads,
        SlkId::DoodadMetaData,
    ];

    /// The archive path, spelled as the listfile spells it.
    pub fn path(self) -> &'static str {
        match self {
            SlkId::UnitData => r"Units\UnitData.slk",
            SlkId::UnitUi => r"Units\unitUI.slk",
            SlkId::UnitBalance => r"Units\UnitBalance.slk",
            SlkId::UnitWeapons => r"Units\UnitWeapons.slk",
            SlkId::UnitAbilities => r"Units\UnitAbilities.slk",
            SlkId::ItemData => r"Units\ItemData.slk",
            SlkId::UpgradeData => r"Units\UpgradeData.slk",
            SlkId::AbilityData => r"Units\AbilityData.slk",
            SlkId::UnitMetaData => r"Units\UnitMetaData.slk",
            SlkId::AbilityMetaData => r"Units\AbilityMetaData.slk",
            SlkId::UpgradeMetaData => r"Units\UpgradeMetaData.slk",
            SlkId::UpgradeEffectMetaData => r"Units\UpgradeEffectMetaData.slk",
            SlkId::DestructableData => r"Units\DestructableData.slk",
            SlkId::DestructableMetaData => r"Units\DestructableMetaData.slk",
            SlkId::MiscMetaData => r"Units\MiscMetaData.slk",
            SlkId::AbilityBuffData => r"Units\AbilityBuffData.slk",
            SlkId::AbilityBuffMetaData => r"Units\AbilityBuffMetaData.slk",
            SlkId::Doodads => r"Doodads\Doodads.slk",
            SlkId::DoodadMetaData => r"Doodads\DoodadMetaData.slk",
        }
    }

    /// The row-1 column that names a raw code, for the tables keyed by one.
    ///
    /// Measured, not assumed: each of these is row 1's first named column, and
    /// each table's rows are unique under it.
    pub fn id_column(self) -> Option<&'static str> {
        Some(match self {
            SlkId::UnitData => "unitID",
            SlkId::UnitUi => "unitUIID",
            SlkId::UnitBalance => "unitBalanceID",
            SlkId::UnitWeapons => "unitWeapID",
            SlkId::UnitAbilities => "unitAbilID",
            SlkId::ItemData => "itemID",
            SlkId::UpgradeData => "upgradeid",
            SlkId::AbilityData => "code",
            _ => return None,
        })
    }

    /// The row-1 column that names a field fourcc, for the metadata tables.
    ///
    /// All six ship the fourcc in a column literally spelled `ID`.
    pub fn code_column(self) -> Option<&'static str> {
        match self {
            SlkId::UnitMetaData
            | SlkId::AbilityMetaData
            | SlkId::UpgradeMetaData
            | SlkId::UpgradeEffectMetaData
            | SlkId::DestructableMetaData
            | SlkId::MiscMetaData
            | SlkId::AbilityBuffMetaData
            | SlkId::DoodadMetaData => Some("ID"),
            _ => None,
        }
    }

    /// The column that holds the tooltip-ish comment of a row.
    pub fn comment_column(self) -> Option<&'static str> {
        Some(match self {
            SlkId::UnitData
            | SlkId::UnitUi
            | SlkId::UnitBalance
            | SlkId::UnitWeapons
            | SlkId::UnitAbilities => "comment(s)",
            SlkId::ItemData | SlkId::AbilityData => "comment",
            // `UpgradeData.slk` spells it `comments`, plural, unlike its
            // siblings — measured, not a typo to be smoothed over.
            SlkId::UpgradeData => "comments",
            _ => return None,
        })
    }

    /// A short human name for the inspector.
    pub fn name(self) -> &'static str {
        match self {
            SlkId::UnitData => "UnitData",
            SlkId::UnitUi => "unitUI",
            SlkId::UnitBalance => "UnitBalance",
            SlkId::UnitWeapons => "UnitWeapons",
            SlkId::UnitAbilities => "UnitAbilities",
            SlkId::ItemData => "ItemData",
            SlkId::UpgradeData => "UpgradeData",
            SlkId::AbilityData => "AbilityData",
            SlkId::UnitMetaData => "UnitMetaData",
            SlkId::AbilityMetaData => "AbilityMetaData",
            SlkId::UpgradeMetaData => "UpgradeMetaData",
            SlkId::UpgradeEffectMetaData => "UpgradeEffectMetaData",
            SlkId::DestructableData => "DestructableData",
            SlkId::DestructableMetaData => "DestructableMetaData",
            SlkId::MiscMetaData => "MiscMetaData",
            SlkId::AbilityBuffData => "AbilityBuffData",
            SlkId::AbilityBuffMetaData => "AbilityBuffMetaData",
            SlkId::Doodads => "Doodads",
            SlkId::DoodadMetaData => "DoodadMetaData",
        }
    }
}

/// A role the `.txt` layer plays: which archive files answer for it.
///
/// This is the "`CommandFunc.txt` and friends" family — the editor's INI-style
/// tables that bind a raw code to its command-button icon, tooltip and
/// hotkey. They are *not* `.slk`, and the `.slk` parser refuses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TxtRole {
    /// `Units\CommandFunc.txt` — command buttons (`[CmdMove]` → `Art=…`).
    ///
    /// This is the table the engine names by literal string: vanilla
    /// `game.dll` carries `Units\CommandFunc.txt` NUL-terminated in `.rdata`,
    /// together with the six `*UnitStrings.txt` it loads. 84 command sections
    /// ship, binding a command-button icon to a command, not to an ability.
    CommandFunc,
    /// `Units\CommandStrings.txt` — the tooltips and hotkeys of those buttons.
    CommandStrings,
    /// The six `Units\<Race>UnitFunc.txt` — art, button position, sound labels.
    UnitFunc,
    /// The six `Units\<Race>UnitStrings.txt` — names, tooltips, ubertips.
    UnitStrings,
    /// `Units\ItemFunc.txt` and `Units\ItemStrings.txt`.
    Item,
}

impl TxtRole {
    /// Every role this crate reads, in load order.
    pub const ALL: &'static [TxtRole] = &[
        TxtRole::CommandFunc,
        TxtRole::CommandStrings,
        TxtRole::UnitFunc,
        TxtRole::UnitStrings,
        TxtRole::Item,
    ];

    /// The archive files this role covers, in search order.
    ///
    /// The search order is the metadata-measured one: the race files are
    /// searched Human, Orc, Undead, NightElf, Neutral, Campaign, so the first
    /// section a raw code lands in wins.
    pub fn files(self) -> &'static [&'static str] {
        match self {
            TxtRole::CommandFunc => &[r"Units\CommandFunc.txt"],
            TxtRole::CommandStrings => &[r"Units\CommandStrings.txt"],
            TxtRole::UnitFunc => &[
                r"Units\HumanUnitFunc.txt",
                r"Units\OrcUnitFunc.txt",
                r"Units\UndeadUnitFunc.txt",
                r"Units\NightElfUnitFunc.txt",
                r"Units\NeutralUnitFunc.txt",
                r"Units\CampaignUnitFunc.txt",
            ],
            TxtRole::UnitStrings => &[
                r"Units\HumanUnitStrings.txt",
                r"Units\OrcUnitStrings.txt",
                r"Units\UndeadUnitStrings.txt",
                r"Units\NightElfUnitStrings.txt",
                r"Units\NeutralUnitStrings.txt",
                r"Units\CampaignUnitStrings.txt",
            ],
            TxtRole::Item => &[r"Units\ItemFunc.txt", r"Units\ItemStrings.txt"],
        }
    }

    /// A short human name for the inspector.
    pub fn name(self) -> &'static str {
        match self {
            TxtRole::CommandFunc => "CommandFunc",
            TxtRole::CommandStrings => "CommandStrings",
            TxtRole::UnitFunc => "UnitFunc",
            TxtRole::UnitStrings => "UnitStrings",
            TxtRole::Item => "Item",
        }
    }
}
