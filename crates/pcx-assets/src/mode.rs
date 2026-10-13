use std::{fmt, str::FromStr};

/// Requested classic behavior build; this is not install detection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Build {
    #[default]
    V1_29b,
    V1_28c,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Addon {
    #[default]
    Tft,
    NoAddon,
}

/// Presentation choice, independent of simulation behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AssetMode {
    #[default]
    Vanilla,
    Hd,
    Reforged,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Behaviour {
    #[default]
    Vanilla,
    Reforged,
    Fixes,
}

/// PLAN §3.6's four orthogonal axes, defaulting to the target fidelity baseline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameMode {
    pub build: Build,
    pub addon: Addon,
    pub assets: AssetMode,
    pub behaviour: Behaviour,
}

impl GameMode {
    /// Vanilla art and behavior policy; not a certificate of native parity.
    pub fn is_fidelity_baseline(self) -> bool {
        self.assets == AssetMode::Vanilla && self.behaviour == Behaviour::Vanilla
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeAxis {
    Build,
    Addon,
    Assets,
    Behaviour,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModeError {
    #[error("mode needs four hyphen-separated components; received {components}")]
    Shape { components: usize },
    #[error("unknown {axis:?} mode component {value:?}")]
    Component { axis: ModeAxis, value: String },
}

impl FromStr for GameMode {
    type Err = ModeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = value.split('-').take(5).collect();
        let [build, addon, assets, behaviour] = parts.as_slice() else {
            return Err(ModeError::Shape {
                components: value.split('-').count(),
            });
        };
        let unknown = |axis, value: &str| ModeError::Component {
            axis,
            value: value.to_owned(),
        };
        Ok(Self {
            build: match *build {
                "1.29b" => Build::V1_29b,
                "1.28c" => Build::V1_28c,
                value => return Err(unknown(ModeAxis::Build, value)),
            },
            addon: match *addon {
                "tft" => Addon::Tft,
                "noaddon" => Addon::NoAddon,
                value => return Err(unknown(ModeAxis::Addon, value)),
            },
            assets: match *assets {
                "vanilla" => AssetMode::Vanilla,
                "hd" => AssetMode::Hd,
                "reforged" => AssetMode::Reforged,
                value => return Err(unknown(ModeAxis::Assets, value)),
            },
            behaviour: match *behaviour {
                "vanilla" => Behaviour::Vanilla,
                "reforged" => Behaviour::Reforged,
                "fixes" => Behaviour::Fixes,
                value => return Err(unknown(ModeAxis::Behaviour, value)),
            },
        })
    }
}

impl fmt::Display for GameMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let build = match self.build {
            Build::V1_29b => "1.29b",
            Build::V1_28c => "1.28c",
        };
        let addon = match self.addon {
            Addon::Tft => "tft",
            Addon::NoAddon => "noaddon",
        };
        let assets = match self.assets {
            AssetMode::Vanilla => "vanilla",
            AssetMode::Hd => "hd",
            AssetMode::Reforged => "reforged",
        };
        let behaviour = match self.behaviour {
            Behaviour::Vanilla => "vanilla",
            Behaviour::Reforged => "reforged",
            Behaviour::Fixes => "fixes",
        };
        write!(f, "{build}-{addon}-{assets}-{behaviour}")
    }
}
