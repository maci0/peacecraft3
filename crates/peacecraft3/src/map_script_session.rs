//! The application's script-session resource. The session itself lives in `pcx_play`, which
//! the headless startup also uses; this wrapper only supplies the Bevy `Resource` derive.
use bevy::prelude::Resource;
use pcx_script::Host;
use std::ops::{Deref, DerefMut};

pub use pcx_play::map_script_session::*;

#[derive(Resource)]
pub struct MapScriptSession<H: Host + Send + Sync + 'static>(
    pcx_play::map_script_session::MapScriptSession<H>,
);

impl<H: Host + Send + Sync + 'static> Default for MapScriptSession<H> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<H: Host + Send + Sync + 'static> Deref for MapScriptSession<H> {
    type Target = pcx_play::map_script_session::MapScriptSession<H>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<H: Host + Send + Sync + 'static> DerefMut for MapScriptSession<H> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
