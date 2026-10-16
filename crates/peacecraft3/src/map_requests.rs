//! Explicit map requests publish parsed data, without starting script execution.
use crate::map_admission::MapAdmission;
use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::FileDragAndDrop;
use pcx_map::Map;
use pcx_play::map_admission::{IdentifiedScript, MapAdmissionBudget, MapAdmissionInputs};
use pcx_play::scripts::{AssetIdentity, ScriptAsset};
use pcx_sim::world::{Definition, ObjectCatalog, ResolutionProblem};
use std::sync::Arc;

pub struct BoundCatalog(pub Arc<dyn ObjectCatalog + Send + Sync>);
impl ObjectCatalog for BoundCatalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        self.0.resolve(rawcode)
    }
}
pub struct BoundScript {
    pub identity: AssetIdentity,
    pub label: String,
    pub bytes: Vec<u8>,
}
/// No edition, catalog, required-field selection or budget is inferred.
pub struct MapBindings {
    pub map_identity: AssetIdentity,
    pub map_label: String,
    pub receipts: Arc<crate::map_binding::BindingReceipts>,
    /// Exact selected map backing the supplied overlays/catalog; shared with caller asset storage.
    pub catalog_map_bytes: Arc<[u8]>,
    pub revision: u64,
    pub common: BoundScript,
    pub blizzard: BoundScript,
    pub catalog_identity: AssetIdentity,
    pub catalog: BoundCatalog,
    pub required_fields: Vec<[u8; 4]>,
    pub budget: MapAdmissionBudget,
    pub max_map_file_bytes: usize,
}
#[derive(Message)]
pub struct MapRequest {
    pub label: String,
    /// Resolver member identity, independent of the display label and local filename.
    pub map_member: pcx_assets::AssetPath,
    pub identity: AssetIdentity,
    pub binding_revision: u64,
    pub payload: MapPayload,
}
pub enum MapPayload {
    Shared(Arc<[u8]>),
    Bytes(Vec<u8>),
    #[cfg(not(target_arch = "wasm32"))]
    File(std::path::PathBuf),
}
#[derive(Debug, thiserror::Error)]
pub enum RequestFailure {
    #[error(transparent)]
    Binding(Box<crate::map_binding::BindingError>),
    #[error("configure map sources, identities, field bindings and input limits first")]
    MissingBindings,
    #[error("map binding revision {requested} is stale; current revision is {current}")]
    StaleBindings { requested: u64, current: u64 },
    #[error("selected map edition/version differs from the supplied binding")]
    IdentityMismatch,
    #[error("selected map member {requested} differs from the bound member {bound}")]
    MemberMismatch {
        requested: pcx_assets::AssetPath,
        bound: pcx_assets::AssetPath,
    },
    #[error("selected map bytes differ from the map used for object bindings")]
    CatalogMapMismatch,
    #[error("selected file has {actual} bytes; caller limit is {limit}")]
    FileBudget { actual: u64, limit: usize },
    #[cfg(not(target_arch = "wasm32"))]
    #[error("reading selected map: {0}")]
    File(std::io::Error),
    #[error(transparent)]
    Container(pcx_map::Error),
    #[error(transparent)]
    Admission(pcx_play::map_admission::MapAdmissionError),
}
pub struct MapOutcome {
    pub label: String,
    pub identity: Option<AssetIdentity>,
    pub binding_revision: Option<u64>,
    pub result: Result<(), RequestFailure>,
}
#[derive(Resource, Default)]
pub struct MapRequests {
    pub bindings: Option<MapBindings>,
    pub admission: MapAdmission,
    pub published_receipts: Option<Arc<crate::map_binding::BindingReceipts>>,
    pub last_outcome: Option<MapOutcome>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingBindingRequirement {
    SourceMount,
    MapIdentity,
    PreludeIdentities,
    CatalogIdentity,
    ExplicitArchivePolicy,
    MemberInventory,
    RequiredDataFields,
    InputReadLimits,
}
impl MissingBindingRequirement {
    pub fn label(self) -> &'static str {
        match self {
            Self::SourceMount => "select and mount local game sources",
            Self::MapIdentity => "identify the selected map edition/version/source",
            Self::PreludeIdentities => "identify the map's common and Blizzard scripts",
            Self::CatalogIdentity => "identify object data sources",
            Self::ExplicitArchivePolicy => "supply an explicit archive/namespace policy",
            Self::MemberInventory => "declare source members",
            Self::RequiredDataFields => "select required object tables and fields",
            Self::InputReadLimits => "supply source and map input limits",
        }
    }
}
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapRequestSet {
    Deliver,
}
impl MapRequests {
    pub fn missing_binding_requirements(&self) -> &'static [MissingBindingRequirement] {
        if self.bindings.is_some() {
            &[]
        } else {
            &[
                MissingBindingRequirement::SourceMount,
                MissingBindingRequirement::MapIdentity,
                MissingBindingRequirement::PreludeIdentities,
                MissingBindingRequirement::CatalogIdentity,
                MissingBindingRequirement::ExplicitArchivePolicy,
                MissingBindingRequirement::MemberInventory,
                MissingBindingRequirement::RequiredDataFields,
                MissingBindingRequirement::InputReadLimits,
            ]
        }
    }
    pub fn selected_map_request(&self) -> Result<MapRequest, RequestFailure> {
        let binding = self
            .bindings
            .as_ref()
            .ok_or(RequestFailure::MissingBindings)?;
        Ok(MapRequest {
            label: binding.map_label.clone(),
            map_member: binding.receipts.map.path.clone(),
            identity: binding.map_identity.clone(),
            binding_revision: binding.revision,
            payload: MapPayload::Shared(binding.catalog_map_bytes.clone()),
        })
    }

    /// Candidate binding, receipts and parsed context commit together.
    /// Selected sources must enforce caller budgets before payload reads/decode.
    pub fn replace_selected(
        &mut self,
        resolver: &pcx_assets::Resolver,
        input: crate::map_binding::BindingInputs<'_>,
        limits: crate::map_binding::BindingLimits,
    ) -> &MapOutcome {
        let label = input.map.label.to_owned();
        let identity = input.map.identity.clone();
        let revision = input.revision;
        let bindings = match crate::map_binding::bind_map_bounded(resolver, input, limits) {
            Ok(bindings) => bindings,
            Err(source) => {
                self.last_outcome = Some(MapOutcome {
                    label,
                    identity: Some(identity),
                    binding_revision: Some(revision),
                    result: Err(RequestFailure::Binding(Box::new(source))),
                });
                return self.last_outcome.as_ref().expect("binding outcome");
            }
        };
        let mut candidate = Self {
            bindings: Some(bindings),
            ..Default::default()
        };
        let request = candidate.selected_map_request().expect("candidate binding");
        candidate.deliver(request);
        let accepted = candidate
            .last_outcome
            .as_ref()
            .expect("delivered outcome")
            .result
            .is_ok();
        self.last_outcome = candidate.last_outcome;
        if accepted {
            self.bindings = candidate.bindings;
            self.admission = candidate.admission;
            self.published_receipts = candidate.published_receipts;
        }
        self.last_outcome.as_ref().expect("delivered outcome")
    }

    pub fn deliver(&mut self, request: MapRequest) {
        let result = self.prepare(&request);
        self.last_outcome = Some(MapOutcome {
            label: request.label,
            identity: Some(request.identity),
            binding_revision: Some(request.binding_revision),
            result,
        });
    }
    fn prepare(&mut self, request: &MapRequest) -> Result<(), RequestFailure> {
        let bindings = self
            .bindings
            .as_ref()
            .ok_or(RequestFailure::MissingBindings)?;
        if request.binding_revision != bindings.revision {
            return Err(RequestFailure::StaleBindings {
                requested: request.binding_revision,
                current: bindings.revision,
            });
        }
        if request.identity.edition != bindings.map_identity.edition
            || request.identity.version != bindings.map_identity.version
        {
            return Err(RequestFailure::IdentityMismatch);
        }
        if request.map_member != bindings.receipts.map.path {
            return Err(RequestFailure::MemberMismatch {
                requested: request.map_member.clone(),
                bound: bindings.receipts.map.path.clone(),
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let file;
        let bytes = match &request.payload {
            MapPayload::Bytes(bytes) => bytes.as_slice(),
            MapPayload::Shared(bytes) => bytes.as_ref(),
            #[cfg(not(target_arch = "wasm32"))]
            MapPayload::File(path) => {
                file = read_map_file(path, bindings.max_map_file_bytes)?;
                file.as_slice()
            }
        };
        if bytes.len() > bindings.max_map_file_bytes {
            return Err(RequestFailure::FileBudget {
                actual: bytes.len() as u64,
                limit: bindings.max_map_file_bytes,
            });
        }
        if bytes != bindings.catalog_map_bytes.as_ref() {
            return Err(RequestFailure::CatalogMapMismatch);
        }
        let map = Map::parse(bytes).map_err(RequestFailure::Container)?;
        let result = self
            .admission
            .replace(
                MapAdmissionInputs {
                    map: &map,
                    map_label: &request.label,
                    map_identity: &request.identity,
                    common: IdentifiedScript {
                        identity: &bindings.common.identity,
                        asset: ScriptAsset {
                            label: &bindings.common.label,
                            bytes: &bindings.common.bytes,
                        },
                    },
                    blizzard: IdentifiedScript {
                        identity: &bindings.blizzard.identity,
                        asset: ScriptAsset {
                            label: &bindings.blizzard.label,
                            bytes: &bindings.blizzard.bytes,
                        },
                    },
                    catalog_identity: &bindings.catalog_identity,
                    catalog: &bindings.catalog,
                    required_fields: &bindings.required_fields,
                },
                bindings.budget,
            )
            .map_err(RequestFailure::Admission);
        if result.is_ok() {
            self.published_receipts = Some(bindings.receipts.clone());
        }
        result
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn read_map_file(path: &std::path::Path, limit: usize) -> Result<Vec<u8>, RequestFailure> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(RequestFailure::File)?;
    let metadata = file.metadata().map_err(RequestFailure::File)?;
    if !metadata.is_file() {
        return Err(RequestFailure::File(std::io::Error::other(
            "selection is not a regular file",
        )));
    }
    let length = metadata.len();
    let size = usize::try_from(length)
        .ok()
        .filter(|size| *size <= limit)
        .ok_or(RequestFailure::FileBudget {
            actual: length,
            limit,
        })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|error| RequestFailure::File(std::io::Error::other(error)))?;
    bytes.resize(size, 0);
    file.read_exact(&mut bytes).map_err(RequestFailure::File)?;
    if file.read(&mut [0u8; 1]).map_err(RequestFailure::File)? != 0 {
        return Err(RequestFailure::File(std::io::Error::other(
            "selected file changed length during read",
        )));
    }
    Ok(bytes)
}
fn deliver_requests(mut requests: MessageReader<MapRequest>, mut state: ResMut<MapRequests>) {
    // Process every authored request in arrival order; each publication is atomic.
    for request in requests.read() {
        let result = state.prepare(request);
        state.last_outcome = Some(MapOutcome {
            label: request.label.clone(),
            identity: Some(request.identity.clone()),
            binding_revision: Some(request.binding_revision),
            result,
        });
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn dropped_files(mut drops: MessageReader<FileDragAndDrop>, mut state: ResMut<MapRequests>) {
    for event in drops.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
            let Some(bindings) = state.bindings.as_ref() else {
                // A path alone cannot establish edition/version or catalog bindings.
                state.last_outcome = Some(MapOutcome {
                    label: path_buf.display().to_string(),
                    identity: None,
                    binding_revision: None,
                    result: Err(RequestFailure::MissingBindings),
                });
                continue;
            };
            let request = MapRequest {
                label: path_buf.display().to_string(),
                map_member: bindings.receipts.map.path.clone(),
                identity: AssetIdentity {
                    source: path_buf.display().to_string(),
                    edition: bindings.map_identity.edition.clone(),
                    version: bindings.map_identity.version.clone(),
                },
                binding_revision: bindings.revision,
                payload: MapPayload::File(path_buf.clone()),
            };
            state.deliver(request);
        }
    }
}
pub fn install(app: &mut App) {
    app.init_resource::<MapRequests>()
        .add_message::<MapRequest>();
    #[cfg(not(target_arch = "wasm32"))]
    app.add_message::<FileDragAndDrop>().add_systems(
        Update,
        (dropped_files, deliver_requests)
            .chain()
            .in_set(MapRequestSet::Deliver),
    );
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, deliver_requests.in_set(MapRequestSet::Deliver));
}

struct OwnedMetadataCatalog {
    editor: pcx_objects::editor::Editor,
    units: pcx_objects::editor::report::Overlay,
    items: pcx_objects::editor::report::Overlay,
}
impl ObjectCatalog for OwnedMetadataCatalog {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        pcx_sim::world::MetadataCatalog {
            editor: &self.editor,
            units: &self.units,
            items: &self.items,
        }
        .resolve(rawcode)
    }
}
/// Explicit map unit/item overlays stay separate; the caller retains reader provenance.
pub fn bind_metadata<E: std::error::Error + Send + Sync + 'static>(
    source_label: &std::path::Path,
    listed: &[String],
    limits: pcx_objects::editor::EditorReadLimits,
    reader: impl FnMut(&str) -> Result<Option<Vec<u8>>, E>,
    units: pcx_objects::editor::report::Overlay,
    items: pcx_objects::editor::report::Overlay,
) -> Result<BoundCatalog, pcx_objects::Error> {
    let editor = pcx_objects::editor::Editor::from_reader(source_label, listed, limits, reader)?;
    Ok(BoundCatalog(Arc::new(OwnedMetadataCatalog {
        editor,
        units,
        items,
    })))
}

impl MapBindings {
    /// Clone the catalog arc this binding already selected.
    ///
    /// Install, edition, and map are `map_identity` and `map_label`, the same
    /// pair map admission stores on `ImportedWorld.source`. Receipts stay on
    /// this binding; they are not copied into a second catalog schema. No
    /// application system constructs `ObjectHost`. A session factory must call
    /// this while it still holds these bindings.
    pub fn admit_executed_units(
        &self,
        host: pcx_play::playercolor_host::ObjectHost,
        budget: pcx_play::playercolor_host::UnitBudget,
    ) -> Result<pcx_play::playercolor_host::ObjectHost, pcx_play::playercolor_host::Error> {
        host.with_unit_catalog(
            Arc::clone(&self.catalog.0),
            pcx_sim::world::Source {
                install: self.map_identity.source.clone(),
                edition: self.map_identity.edition.clone(),
                map: self.map_label.clone(),
            },
            budget,
        )
    }
}
