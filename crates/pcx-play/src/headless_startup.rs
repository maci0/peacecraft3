//! Source-derived map startup without the Bevy application.
//!
//! One selected install and one map file go in; the production `MapStartupHost` comes out.
//! The steps are the application's campaign path (`native_campaign::configuration`,
//! `map_requests` binding, `campaign_prepare`) without its Bevy systems. Every game value comes
//! from the install's archives and the map bytes. The client identity is the SHA-256 of the
//! selected executable, which the application's adapter also uses.
use crate::campaign_host::{self, advance_tick};
use crate::campaign_inputs::{self, MapSources};
use crate::install_source::{MAX_AGGREGATE_BYTES, MAX_ASSET_BYTES, MAX_LISTED_PATHS};
use crate::map_admission::{
    IdentifiedScript, MapAdmissionBudget, MapAdmissionInputs, ParsedMapContext, prepare_map_context,
};
use crate::map_script_session::MapScriptSession;
use crate::map_scripts_import::MapScriptBudget;
use crate::map_startup_host::MapStartupHost;
use crate::scripts::{AssetIdentity, ScriptAsset};
use crate::source_receipt::{SourceReceiptRecorder, observe_catalog_profiles};
use pcx_assets::{Addon, GameMode};
use pcx_map::Map;
use pcx_objects::ObjectFile;
use pcx_objects::editor::{Editor, EditorReadLimits, Overlay};
use pcx_sim::world::{Definition, MetadataCatalog, ObjectCatalog, ResolutionProblem};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const EXECUTABLE: &str = "Warcraft III.exe";
/// A headless host has no renderer. Occluder fading is presentation only, so this host declares
/// the same no-fading capability as the application renderer (`map_ground::NoOcclusionPass`).
const NO_OCCLUSION_FADING: bool = true;
const COMMON_SCRIPT: &str = "Scripts\\common.j";
const BLIZZARD_SCRIPT: &str = "Scripts\\Blizzard.j";
/// Stock Misc tables the campaign startup admits through the selected source reader.
/// Their bytes are game rules and UI configuration, so the admission receipt binds them.
const MISC_SOURCES: &[(&str, u64)] = &[
    ("Units/MiscData.txt", 4 << 20),
    ("Units/MiscGame.txt", 4 << 20),
    ("UI/MiscData.txt", 2 << 20),
];
const MAP_READ_BUDGET: pcx_mpq::checked::ReadBudget = pcx_mpq::checked::ReadBudget {
    max_archive_bytes: 64 << 20,
    max_archive_table_bytes: 8 << 20,
    max_raw_packed_bytes: 16 << 20,
    max_decoded_bytes: 16 << 20,
};
const EDITOR_LIMITS: EditorReadLimits = EditorReadLimits {
    max_listed_paths: MAX_LISTED_PATHS,
    max_asset_bytes: MAX_ASSET_BYTES,
    max_aggregate_input_bytes: MAX_AGGREGATE_BYTES,
};
/// Operating ceilings for one startup, the same values the application's binding path uses.
const SCRIPT_BUDGET: MapScriptBudget = MapScriptBudget {
    max_script_bytes: MAX_ASSET_BYTES,
    max_wts_bytes: MAX_ASSET_BYTES,
    max_archive_bytes: MAX_ASSET_BYTES,
    max_archive_table_bytes: MAX_ASSET_BYTES,
    max_raw_packed_bytes: MAX_ASSET_BYTES,
    max_decoded_bytes: MAX_ASSET_BYTES,
    max_prelude_bytes: MAX_ASSET_BYTES,
    max_aggregate_input_bytes: MAX_AGGREGATE_BYTES,
};
const ADMISSION_BUDGET: MapAdmissionBudget = MapAdmissionBudget {
    scripts: SCRIPT_BUDGET,
    terrain_bytes: MAX_ASSET_BYTES,
    info_bytes: MAX_ASSET_BYTES,
    pathing_bytes: MAX_ASSET_BYTES,
    units_bytes: MAX_ASSET_BYTES,
    aggregate_input_bytes: MAX_AGGREGATE_BYTES,
    required_fields: 0,
};

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("install: {0}")]
    Install(String),
    #[error("{member}: {message}")]
    Member { member: String, message: String },
    #[error("map: {0}")]
    Map(String),
    #[error("catalog: {0}")]
    Catalog(String),
    #[error("source receipt: {0}")]
    Receipt(String),
    #[error("admission: {0}")]
    Admission(String),
    #[error("startup inputs: {0}")]
    Inputs(String),
    #[error("startup host: {0}")]
    Entrypoint(String),
    #[error("script session: {0}")]
    Session(String),
    #[error("campaign clock: {0}")]
    Clock(String),
    #[error("campaign profile: {0}")]
    Profile(String),
}

/// The admitted context and the production host built from it.
pub struct Startup {
    pub context: Arc<ParsedMapContext>,
    pub session: MapScriptSession<MapStartupHost>,
    /// Game time when the session was published, and the ticks stepped since.
    origin: f64,
    tick: u64,
    admission: crate::neutral_player::Sha256Digest,
    catalog_receipt: crate::neutral_player::Sha256Digest,
    cache: Arc<crate::campaign_cache::SessionStorage>,
}

/// Object definitions from the install's unit and item metadata plus the map's overrides.
/// The map's own unit and item overlays stay separate, as the application keeps them.
struct OwnedCatalog {
    editor: Editor,
    units: Overlay,
    items: Overlay,
}

impl ObjectCatalog for OwnedCatalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        MetadataCatalog {
            editor: &self.editor,
            units: &self.units,
            items: &self.items,
        }
        .resolve(rawcode)
    }
}

fn map_overlay(map: &Map<'_>, label: &str, member: &'static str) -> Result<Overlay, StartupError> {
    let raw = map
        .extract_checked(label, member, MAP_READ_BUDGET)
        .map_err(|error| StartupError::Map(error.to_string()))?;
    let Some(raw) = raw else {
        return Ok(Overlay::default());
    };
    let file =
        ObjectFile::parse(member, &raw).map_err(|error| StartupError::Map(error.to_string()))?;
    Ok(Overlay::of(&file))
}

/// Whether the selected mode is The Frozen Throne. The expansion flag is a function of the mode,
/// so no caller can pass a contradicting value.
pub fn expansion_of(mode: GameMode) -> bool {
    matches!(mode.addon, Addon::Tft)
}

/// Starts the map at `map_bytes` (labelled `map_label`) on the install at `install`.
///
/// `mode` selects the asset policy and the expansion flag. `seed` is the new-game seed,
/// supplied by the caller and never defaulted here.
/// `profile` is a saved PC3 campaign profile, or `None` for a fresh profile. Saves stay
/// in this session and can be exported through `Startup::campaign_profile`.
#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "startup.total")
)]
pub fn start(
    install: &Path,
    mode: GameMode,
    map_label: &str,
    map_bytes: &[u8],
    seed: i32,
    profile: Option<&[u8]>,
) -> Result<Startup, StartupError> {
    let cache = Arc::new(
        crate::campaign_cache::SessionStorage::new(profile).map_err(StartupError::Profile)?,
    );
    let exe_path = install.join(EXECUTABLE);
    let file = std::fs::File::open(&exe_path)
        .map_err(|error| StartupError::Install(format!("{}: {error}", exe_path.display())))?;
    let image_limit = MAX_ASSET_BYTES as u64;
    if file
        .metadata()
        .map_err(|error| StartupError::Install(error.to_string()))?
        .len()
        > image_limit
    {
        return Err(StartupError::Install(format!(
            "{}: image exceeds {image_limit} byte budget",
            exe_path.display()
        )));
    }
    let mut image = Vec::new();
    file.take(image_limit + 1)
        .read_to_end(&mut image)
        .map_err(|error| StartupError::Install(format!("{}: {error}", exe_path.display())))?;
    if image.len() > MAX_ASSET_BYTES {
        return Err(StartupError::Install(format!(
            "{}: image exceeds {image_limit} byte budget",
            exe_path.display()
        )));
    }
    let rules = crate::campaign_data_set::ImageRules::read(
        &image,
        campaign_inputs::client_identity(&image, &exe_path.display().to_string()),
    )
    .map_err(|error| StartupError::Install(error.to_string()))?;
    let global = match mode.addon {
        Addon::NoAddon => "VERSION_REIGN_OF_CHAOS",
        Addon::Tft => "VERSION_FROZEN_THRONE",
    };
    let recipe =
        crate::campaign_data_set::CampaignRecipe::pin(install, mode, rules, &image, None, global)
            .map_err(|error| StartupError::Install(error.to_string()))?;
    let resolver = recipe
        .open_map(map_bytes, map_label, MAP_READ_BUDGET)
        .map_err(|error| StartupError::Map(error.to_string()))?
        .resolver;

    let edition = format!("{:?} selected install", mode.addon);
    let read = |member: &str| -> Result<Vec<u8>, StartupError> {
        resolver
            .resolve_bounded(member, MAX_ASSET_BYTES as u64)
            .map(|asset| asset.bytes)
            .map_err(|error| StartupError::Member {
                member: member.into(),
                message: error.to_string(),
            })
    };
    let common_bytes = read(COMMON_SCRIPT)?;
    let blizzard_bytes = read(BLIZZARD_SCRIPT)?;
    let script_identity = |member: &str| AssetIdentity {
        edition: edition.clone(),
        version: "selected install".into(),
        source: member.into(),
    };
    let common_identity = script_identity(COMMON_SCRIPT);
    let blizzard_identity = script_identity(BLIZZARD_SCRIPT);
    let map_identity = AssetIdentity {
        edition: edition.clone(),
        version: "map bytes".into(),
        source: map_label.into(),
    };
    let mut recorder = SourceReceiptRecorder::new();

    let map = Map::parse(map_bytes).map_err(|error| StartupError::Map(error.to_string()))?;
    let editor = Editor::from_reader(install, &[], EDITOR_LIMITS, |member| {
        let result = match resolver.resolve_bounded(member, MAX_ASSET_BYTES as u64) {
            Ok(asset) => Ok(Some(asset.bytes)),
            Err(pcx_assets::Error::MissingAsset { .. }) => Ok(None),
            Err(error) => Err(error),
        };
        if let Ok(content) = &result {
            recorder.observe(member, content.as_deref());
        }
        result
    })
    .map_err(|error| StartupError::Catalog(error.to_string()))?;
    observe_catalog_profiles(
        &mut recorder,
        resolver.paths().cloned(),
        |member| -> Result<Option<Vec<u8>>, String> {
            match resolver.resolve_bounded(member, 4 << 20) {
                Ok(asset) => Ok(Some(asset.bytes)),
                Err(pcx_assets::Error::MissingAsset { .. }) => Ok(None),
                Err(error) => Err(error.to_string()),
            }
        },
    )
    .map_err(|error| StartupError::Receipt(error.to_string()))?;
    for &(member, limit) in MISC_SOURCES {
        let content = match resolver.resolve_bounded(member, limit) {
            Ok(asset) => Some(asset.bytes),
            Err(pcx_assets::Error::MissingAsset { .. }) => None,
            Err(error) => {
                return Err(StartupError::Member {
                    member: member.into(),
                    message: error.to_string(),
                });
            }
        };
        recorder.observe(member, content.as_deref());
    }
    let catalog_receipt = recorder
        .finish()
        .map_err(|error| StartupError::Receipt(error.to_string()))?;
    let catalog_identity = AssetIdentity {
        edition: edition.clone(),
        version: "selected install catalog receipt".into(),
        source: format!("sha256:{}", hex(&catalog_receipt.digest())),
    };
    let catalog = Arc::new(OwnedCatalog {
        editor,
        units: map_overlay(&map, map_label, "war3map.w3u")?,
        items: map_overlay(&map, map_label, "war3map.w3t")?,
    });

    let context = Arc::new(
        prepare_map_context(
            MapAdmissionInputs {
                map: &map,
                map_label,
                map_identity: &map_identity,
                common: IdentifiedScript {
                    identity: &common_identity,
                    asset: ScriptAsset {
                        label: COMMON_SCRIPT,
                        bytes: &common_bytes,
                    },
                },
                blizzard: IdentifiedScript {
                    identity: &blizzard_identity,
                    asset: ScriptAsset {
                        label: BLIZZARD_SCRIPT,
                        bytes: &blizzard_bytes,
                    },
                },
                catalog_identity: &catalog_identity,
                catalog: catalog.as_ref(),
                required_fields: &[],
            },
            ADMISSION_BUDGET,
        )
        .map_err(|error| StartupError::Admission(error.to_string()))?,
    );

    let sources = MapSources {
        map_label: map_label.into(),
        map_bytes: Arc::from(map_bytes),
        catalog_identity,
        catalog: catalog as Arc<dyn ObjectCatalog + Send + Sync>,
    };
    let client = campaign_inputs::client_identity(&image, &exe_path.display().to_string());
    let reader = Arc::new(resolver);
    let inputs = campaign_inputs::prepare_new_game(
        &context,
        &sources,
        &reader,
        &image,
        client,
        seed,
        expansion_of(mode),
    )
    .map_err(StartupError::Inputs)?;
    let config = campaign_host::entry_id(&context, "config")
        .map_err(|error| StartupError::Entrypoint(error.to_string()))?;
    let main = campaign_host::entry_id(&context, "main")
        .map_err(|error| StartupError::Entrypoint(error.to_string()))?;
    let post = inputs.post_config.clone();
    let admitted = Arc::clone(&context);
    let placement_map = Arc::clone(&sources.map_bytes);
    let mut session = MapScriptSession::<MapStartupHost>::default();
    let session_cache = Arc::clone(&cache);
    session
        .replace_sequenced(
            Arc::clone(&context),
            move |ctx| {
                campaign_host::create_host(
                    ctx,
                    inputs,
                    Some(reader),
                    placement_map,
                    NO_OCCLUSION_FADING,
                    session_cache,
                )
            },
            &[config, main],
            move |vm, completed| campaign_host::apply_after_config(vm, &admitted, completed, &post),
        )
        .map_err(|error| StartupError::Session(error.to_string()))?;
    let origin = session
        .host()
        .and_then(|host| host.objects().timer_game_time())
        .ok_or(StartupError::Clock(
            "the admitted map has no game clock".into(),
        ))?;
    let admission = digest_admission(AdmissionInputs {
        image: &image,
        map: map_bytes,
        common: &common_bytes,
        blizzard: &blizzard_bytes,
        catalog_receipt: catalog_receipt.digest(),
        seed,
        mode,
        context: &context,
        profile,
    });
    Ok(Startup {
        context,
        session,
        origin,
        tick: 0,
        admission,
        catalog_receipt: catalog_receipt.digest(),
        cache,
    })
}

/// Everything the admission read. Two starts agree on the digest exactly when they admitted the
/// same executable, map, scripts, catalog source content, seed, mode, profile and human roster.
struct AdmissionInputs<'a> {
    image: &'a [u8],
    map: &'a [u8],
    common: &'a [u8],
    blizzard: &'a [u8],
    catalog_receipt: crate::neutral_player::Sha256Digest,
    seed: i32,
    mode: GameMode,
    context: &'a ParsedMapContext,
    profile: Option<&'a [u8]>,
}

fn digest_admission(inputs: AdmissionInputs<'_>) -> crate::neutral_player::Sha256Digest {
    let AdmissionInputs {
        image,
        map,
        common,
        blizzard,
        catalog_receipt,
        seed,
        mode,
        context,
        profile,
    } = inputs;
    let humans = context
        .world
        .info
        .human_players()
        .map(|human| human.number())
        .collect::<Vec<_>>();
    let text = admission_text(
        image,
        map,
        common,
        blizzard,
        catalog_receipt,
        seed,
        mode,
        profile,
        &humans,
    );
    crate::neutral_player::sha256(text.as_bytes())
}

#[allow(clippy::too_many_arguments)]
fn admission_text(
    image: &[u8],
    map: &[u8],
    common: &[u8],
    blizzard: &[u8],
    catalog_receipt: crate::neutral_player::Sha256Digest,
    seed: i32,
    mode: GameMode,
    profile: Option<&[u8]>,
    humans: &[u32],
) -> String {
    let mut text = String::from("pcx-admission-v3\n");
    text += &format!("exe {}\n", hex(&crate::neutral_player::sha256(image)));
    text += &format!("map {}\n", hex(&crate::neutral_player::sha256(map)));
    text += &format!("common {}\n", hex(&crate::neutral_player::sha256(common)));
    text += &format!(
        "blizzard {}\n",
        hex(&crate::neutral_player::sha256(blizzard))
    );
    text += &format!(
        "catalog-receipt {}\nseed {seed}\nmode {mode:?}\n",
        hex(&catalog_receipt)
    );
    text += &format!("profile {}\n", profile_identity(profile));
    for human in humans {
        text += &format!("human {human}\n");
    }
    text
}

fn profile_identity(profile: Option<&[u8]>) -> String {
    profile.map_or_else(
        || "absent".into(),
        |bytes| hex(&crate::neutral_player::sha256(bytes)),
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The campaign clock's step and expiry budget: the application's launcher values
/// (`peacecraft3::launch_gui`), one 64 Hz tick per frame step.
const CLOCK_STEP_MICROS: u64 = 15_625;
const MAX_EXPIRIES_PER_TICK: usize = 4096;

/// Most clock ticks one `step_clock` call runs. A caller that needs more calls again, so no call
/// runs unbounded work and a network caller can interleave other requests between calls.
pub const MAX_CLOCK_TICKS_PER_CALL: u64 = 4096;

impl Startup {
    /// Current saved profile bytes. Exporting does not read or write the machine profile.
    pub fn campaign_profile(&self) -> Result<Option<Vec<u8>>, StartupError> {
        self.cache.snapshot().map_err(StartupError::Profile)
    }
    /// Ticks reached on the campaign clock since publication.
    /// Digest of this startup's admission inputs; see `digest_admission`.
    pub fn admission_digest(&self) -> crate::neutral_player::Sha256Digest {
        self.admission
    }
    /// Canonical digest of the catalog and Misc source content this startup admitted.
    pub fn catalog_receipt(&self) -> crate::neutral_player::Sha256Digest {
        self.catalog_receipt
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Steps the admitted session `ticks` times on the campaign clock and returns the tick
    /// reached. A call above `MAX_CLOCK_TICKS_PER_CALL` runs nothing. A failing tick ends the
    /// call with its tick number; earlier ticks in the call stay applied.
    pub fn step_clock(&mut self, ticks: u64) -> Result<u64, StartupError> {
        if ticks > MAX_CLOCK_TICKS_PER_CALL {
            return Err(StartupError::Clock(format!(
                "{ticks} ticks exceed the per-call bound {MAX_CLOCK_TICKS_PER_CALL}"
            )));
        }
        let step = Duration::from_micros(CLOCK_STEP_MICROS).as_secs_f64();
        for _ in 0..ticks {
            self.tick = self
                .tick
                .checked_add(1)
                .ok_or_else(|| StartupError::Clock("tick counter overflow".into()))?;
            let until = self.origin + step * self.tick as f64;
            advance_tick(&mut self.session, until, MAX_EXPIRIES_PER_TICK)
                .map_err(|error| StartupError::Clock(format!("tick {}: {error}", self.tick)))?;
        }
        Ok(self.tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_assets::{AssetMode, Behaviour, Build};

    fn mode(addon: Addon) -> GameMode {
        GameMode {
            build: Build::V1_29b,
            addon,
            assets: AssetMode::Vanilla,
            behaviour: Behaviour::Vanilla,
        }
    }

    #[test]
    fn expansion_follows_the_selected_addon() {
        assert!(expansion_of(mode(Addon::Tft)));
        assert!(!expansion_of(mode(Addon::NoAddon)));
    }

    fn catalog_receipt(content: &[u8]) -> crate::source_receipt::SourceReceipt {
        let mut recorder = crate::source_receipt::SourceReceiptRecorder::new();
        recorder.observe("Units\\MiscGame.txt", Some(content));
        recorder.finish().expect("receipt")
    }

    #[test]
    fn admission_binds_catalog_content_at_the_same_member_path() {
        let executable = b"executable";
        let map = b"map";
        let script = b"script";
        let seed = -7;
        let mode = mode(Addon::NoAddon);
        let humans = [0u32];
        let digest = |content: &[u8]| {
            crate::neutral_player::sha256(
                admission_text(
                    executable,
                    map,
                    script,
                    script,
                    catalog_receipt(content).digest(),
                    seed,
                    mode,
                    None,
                    &humans,
                )
                .as_bytes(),
            )
        };
        let original = digest(b"[Misc]\nFoo=1\n");
        let repeat = digest(b"[Misc]\nFoo=1\n");
        let changed = digest(b"[Misc]\nFoo=2\n");
        assert_eq!(original, repeat, "repeated admission must agree");
        assert_ne!(
            original, changed,
            "changed catalog bytes at one path must change admission"
        );
    }
}
