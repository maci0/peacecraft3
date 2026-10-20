//! Score screen UI assets, read through the resolver that produced the exiting session's
//! bindings. Every file is read at its selected path. A missing or malformed file is
//! refused with its asset path; nothing is substituted.
use crate::campaign_score::{ScoreScreen, ScoreScreenState};
use crate::campaign_score_layout::{ScoreScreenLayout, includes};
use crate::map_requests::MapRequests;
use bevy::prelude::*;
use pcx_assets::Resolver;
use pcx_play::map_admission::ParsedMapContext;
use std::sync::Arc;

/// Selected score screen layout and the session context it was loaded for.
#[derive(Resource, Default)]
pub struct ScoreScreenAssets {
    layout: Option<ScoreScreenLayout>,
    loaded_for: Option<Arc<ParsedMapContext>>,
}

impl ScoreScreenAssets {
    /// The layout loaded for exactly `context`. A layout from another session, or a refused
    /// attempt for this one, is `None`, so a view can never draw stale content.
    pub fn layout_for(&self, context: &Arc<ParsedMapContext>) -> Option<&ScoreScreenLayout> {
        self.loaded_for
            .as_ref()
            .is_some_and(|loaded| Arc::ptr_eq(loaded, context))
            .then_some(self.layout.as_ref())
            .flatten()
    }

    /// Whether a load was attempted for exactly `context`, successful or not.
    pub fn attempted_for(&self, context: &Arc<ParsedMapContext>) -> bool {
        self.loaded_for
            .as_ref()
            .is_some_and(|loaded| Arc::ptr_eq(loaded, context))
    }
}

/// Selected path of the score screen and its string table. Its includes come from the
/// screen itself.
const SCORE_SCREEN_FDF: &str = r"UI\FrameDef\Glue\ScoreScreen.fdf";
const GLOBAL_STRINGS_FDF: &str = r"UI\FrameDef\GlobalStrings.fdf";
const UI_FILE_BYTES: u64 = 4 << 20;

fn read_text(resolver: &Resolver, path: &str) -> Result<String, String> {
    let asset = resolver
        .resolve_bounded(path, UI_FILE_BYTES)
        .map_err(|e| format!("{path}: {e}"))?;
    String::from_utf8(asset.bytes).map_err(|_| format!("{path}: not UTF-8 text"))
}

/// Reads the selected score screen, its includes and its string table.
pub fn load(resolver: &Resolver) -> Result<ScoreScreenLayout, String> {
    let screen = read_text(resolver, SCORE_SCREEN_FDF)?;
    let include_texts = includes(&screen)
        .into_iter()
        .map(|path| Ok((path.to_owned(), read_text(resolver, path)?)))
        .collect::<Result<Vec<(String, String)>, String>>()?;
    let borrowed: Vec<(&str, &str)> = include_texts
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    let strings = read_text(resolver, GLOBAL_STRINGS_FDF)?;
    ScoreScreenLayout::parse(&screen, &borrowed, &strings)
        .map_err(|e| format!("{SCORE_SCREEN_FDF}: {e:?}"))
}

/// Caches one load attempt per context. A distinct context drops the previous layout before
/// its own attempt, so a refused context never sees earlier content. Returns `true` only when
/// an attempt ran, so refusals are reported once per context.
pub fn refresh<K: ?Sized, F>(
    layout: &mut Option<ScoreScreenLayout>,
    loaded_for: &mut Option<Arc<K>>,
    key: &Arc<K>,
    load_attempt: F,
) -> Result<bool, String>
where
    F: FnOnce() -> Result<ScoreScreenLayout, String>,
{
    if loaded_for
        .as_ref()
        .is_some_and(|loaded| Arc::ptr_eq(loaded, key))
    {
        return Ok(false);
    }
    *layout = None;
    *loaded_for = Some(Arc::clone(key));
    *layout = Some(load_attempt()?);
    Ok(true)
}

/// Loads the layout once per exiting session, through the bindings of the admitted map that
/// the request's context came from.
pub fn load_assets(
    state: Res<ScoreScreenState>,
    requests: Res<MapRequests>,
    mut assets: ResMut<ScoreScreenAssets>,
) {
    let Some(ScoreScreen::Refused { context, .. }) = &state.0 else {
        return;
    };
    let ScoreScreenAssets { layout, loaded_for } = &mut *assets;
    let attempt = refresh(layout, loaded_for, context, || {
        let same_session = requests
            .admission
            .shared_context()
            .is_some_and(|admitted| Arc::ptr_eq(&admitted, context));
        if !same_session {
            return Err("exiting session is not the admitted map".to_owned());
        }
        let resolver = requests
            .bindings
            .as_ref()
            .and_then(|b| b.resolver.clone())
            .ok_or("admitted map has no retained selected resolver")?;
        load(&resolver)
    });
    if let Err(e) = attempt {
        warn!("score screen refused: {e}");
    }
}

pub fn install(app: &mut App) {
    app.init_resource::<ScoreScreenAssets>()
        .add_systems(Update, load_assets.after(crate::campaign_score::consume));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const SCREEN: &str = "Frame \"FRAME\" \"ScoreScreenFrame\" {\n\tWidth 0.8,\n\tHeight 0.6,\n}\n";

    fn layout() -> ScoreScreenLayout {
        ScoreScreenLayout::parse(SCREEN, &[], "").unwrap()
    }

    #[test]
    fn a_refused_context_never_sees_the_previous_layout() {
        let (mut slot, mut loaded) = (None, None);
        let a = Arc::new(1u8);
        let b = Arc::new(2u8);
        assert_eq!(
            refresh(&mut slot, &mut loaded, &a, || Ok(layout())),
            Ok(true)
        );
        assert!(slot.is_some(), "context A loads its layout");
        let refused = refresh(&mut slot, &mut loaded, &b, || {
            Err("ScoreScreen.fdf: missing".to_owned())
        });
        assert_eq!(refused, Err("ScoreScreen.fdf: missing".to_owned()));
        assert!(
            slot.is_none(),
            "B's failed attempt leaves no A layout usable"
        );
        assert!(Arc::ptr_eq(loaded.as_ref().unwrap(), &b));
    }

    #[test]
    fn a_distinct_successful_context_replaces_the_previous_layout_once() {
        let (mut slot, mut loaded) = (None, None);
        let a = Arc::new(1u8);
        let b = Arc::new(2u8);
        refresh(&mut slot, &mut loaded, &a, || Ok(layout())).unwrap();
        assert_eq!(
            refresh(&mut slot, &mut loaded, &b, || Ok(layout())),
            Ok(true)
        );
        assert!(slot.is_some());
        let attempts = Cell::new(0);
        assert_eq!(
            refresh(&mut slot, &mut loaded, &b, || {
                attempts.set(attempts.get() + 1);
                Ok(layout())
            }),
            Ok(false)
        );
        assert_eq!(attempts.get(), 0, "the same context is not reattempted");
    }

    #[test]
    fn a_refused_context_is_attempted_once_not_on_every_frame() {
        let (mut slot, mut loaded) = (None, None);
        let b = Arc::new(2u8);
        let attempts = Cell::new(0);
        for _ in 0..3 {
            let _ = refresh(&mut slot, &mut loaded, &b, || {
                attempts.set(attempts.get() + 1);
                Err("missing".to_owned())
            });
        }
        assert_eq!(attempts.get(), 1);
        assert!(slot.is_none());
    }
}
