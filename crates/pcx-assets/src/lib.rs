//! Read-only source layering with a local override tree and per-asset provenance.
//!
//! Checked MPQ/CASC adapters consume caller-declared logical/member mappings.
//! Archive priorities are supplied explicitly; modes never infer their order.
//! The override tree wins first, followed by descending archive priority.
//! No asset is converted, repaired, or replaced with invented contents.

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
