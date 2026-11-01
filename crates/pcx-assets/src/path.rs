use std::{fmt, str::FromStr};

/// Project-layer path identity: slash separators and ASCII lowercase.
/// Non-ASCII characters are retained; no Unicode or native CASC case rule is inferred.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct AssetPath(String);

impl<'de> serde::Deserialize<'de> for AssetPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("asset path is empty")]
    Empty,
    #[error("asset path {path:?} is absolute, contains traversal, or an unsafe character")]
    Unsafe { path: String },
}

impl AssetPath {
    pub fn new(value: &str) -> Result<Self, PathError> {
        if value.is_empty() {
            return Err(PathError::Empty);
        }
        let mut normalized = value.replace('\\', "/");
        if normalized.starts_with('/')
            || normalized.ends_with('/')
            || normalized.chars().any(|c| c.is_control() || c == ':')
            || normalized
                .split('/')
                .any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(PathError::Unsafe { path: value.into() });
        }
        normalized.make_ascii_lowercase();
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for AssetPath {
    type Err = PathError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl fmt::Display for AssetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
