#![doc = include_str!("../README.md")]

mod archives;
mod mode;
mod path;
mod resolver;
mod source;

pub use archives::{CascSource, MpqSource, NamedMember};
pub use mode::{Addon, AssetMode, Behaviour, Build, GameMode, ModeAxis, ModeError};
pub use path::{AssetPath, PathError};
pub use resolver::{AssetIndex, Error, Provenance, ResolvedAsset, Resolver};
pub use source::{
    ArchiveLayer, ContentSource, LayerKind, OverrideDirectory, SourceContent, SourceError,
    SourceInfo,
};
