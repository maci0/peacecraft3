use crate::fixtures;
use bevy::prelude::*;
use pcx_assets::{
    ArchiveLayer, AssetPath, ContentSource, LayerKind, Resolver, SourceContent, SourceError,
    SourceInfo,
};
use pcx_objects::editor::path::SlkId;
use pcx_play::{
    map_admission::MapAdmissionBudget, map_scripts_import::MapScriptBudget, scripts::AssetIdentity,
};
use pcx_sim::world::{FieldValue, ObjectCatalog};
use peacecraft3::{map_binding::*, map_requests::*};
use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
const FIELDS: [[u8; 4]; 2] = [*b"STAT", *b"STAT"];
const TABLES: [SlkId; 2] = [SlkId::UnitData, SlkId::UnitMetaData];
pub(crate) fn info(id: &str) -> SourceInfo {
    SourceInfo {
        id: id.into(),
        kind: LayerKind::Mpq,
        origin: format!("{id} supplied origin"),
        version: Some(format!("{id} supplied version")),
    }
}
fn identity(source: &SourceInfo) -> AssetIdentity {
    AssetIdentity {
        source: source.origin.clone(),
        edition: format!("{} independently supplied edition", source.id),
        version: source.version.clone().unwrap(),
    }
}
#[derive(Clone)]
pub(crate) enum Reply {
    Bytes(Vec<u8>),
    Missing,
    Ambiguous,
    Invalid,
}
pub(crate) struct SyntheticSource {
    pub(crate) info: SourceInfo,
    pub(crate) assets: BTreeMap<String, Reply>,
    pub(crate) reads: Arc<AtomicUsize>,
}
impl ContentSource for SyntheticSource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }
    fn paths(&self) -> Result<Vec<String>, SourceError> {
        Ok(self.assets.keys().cloned().collect())
    }
    fn read(&self, member: &str) -> Result<Option<SourceContent>, SourceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        match self.assets.get(member) {
            Some(Reply::Bytes(bytes)) => Ok(Some(SourceContent {
                bytes: bytes.clone(),
                declared_length: bytes.len() as u64,
                locator: format!("{} exact locator::{member}", self.info.id),
            })),
            Some(Reply::Missing) | None => Ok(None),
            Some(Reply::Ambiguous) => Err(SourceError::Ambiguous {
                detail: "two synthetic exact source candidates".into(),
            }),
            Some(Reply::Invalid) => Err(SourceError::InvalidInput {
                detail: "synthetic extraction failed".into(),
            }),
        }
    }
    fn read_bounded(&self, member: &str, limit: u64) -> Result<Option<SourceContent>, SourceError> {
        if let Some(Reply::Bytes(bytes)) = self.assets.get(member)
            && bytes.len() as u64 > limit
        {
            return Err(SourceError::Limit {
                requested: bytes.len() as u64,
                limit,
            });
        }
        self.read(member)
    }
}
fn object(value: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    for word in [1u32, 1] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(b"UNIT");
    bytes.extend([0; 4]);
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(b"STAT");
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(value.to_le_bytes());
    bytes.extend(b"TAIL");
    bytes.extend(0u32.to_le_bytes());
    bytes
}
pub(crate) struct Fixture {
    pub(crate) files: Vec<(&'static str, Vec<u8>)>,
    pub(crate) raw: Vec<u8>,
    pub(crate) common: Vec<u8>,
    pub(crate) bj: Vec<u8>,
    pub(crate) unit_data: Vec<u8>,
    pub(crate) metadata: Vec<u8>,
    pub(crate) map_info: SourceInfo,
    pub(crate) common_info: SourceInfo,
    pub(crate) bj_info: SourceInfo,
    pub(crate) catalog_infos: Vec<SourceInfo>,
    pub(crate) map_id: AssetIdentity,
    pub(crate) common_id: AssetIdentity,
    pub(crate) bj_id: AssetIdentity,
    pub(crate) catalog_id: AssetIdentity,
    pub(crate) override_reply: Option<(String, Reply)>,
    pub(crate) map_reads: Arc<AtomicUsize>,
    pub(crate) common_reads: Arc<AtomicUsize>,
    pub(crate) bj_reads: Arc<AtomicUsize>,
    pub(crate) catalog_reads: Arc<AtomicUsize>,
}
impl Fixture {
    pub(crate) fn new(value: u32) -> Self {
        let map_info = info("map");
        let common_info = info("common");
        let bj_info = info("BJ");
        let stock = info("catalog");
        let files=vec![("war3map.w3e",fixtures::terrain_bytes(2,1)),("war3map.wpm",fixtures::pathing_bytes(4,2)),("war3map.w3i",fixtures::info().canonical_bytes()),("war3mapUnits.doo",fixtures::units().canonical_bytes()),("war3map.j",b"globals\ninteger RequiresHost = FixtureHeld()\nendglobals\nfunction config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n".to_vec()),("war3map.w3u",object(value))];
        let raw = fixtures::build_synthetic_map(
            "one selected synthetic map",
            &files
                .iter()
                .map(|(n, b)| (*n, b.as_slice()))
                .collect::<Vec<_>>(),
        );
        Self {files,raw,common:b"native FixtureHeld takes nothing returns integer\n".to_vec(),bj:b"// supplied BJ\n".to_vec(),unit_data:b"ID;PWXL;N;E\r\nB;Y2;X2\r\nC;X1;Y1;K\"unitID\"\r\nC;X2;Y1;K\"Score\"\r\nC;X1;Y2;K\"UNIT\"\r\nC;X2;Y2;K\"authored stock cell\"\r\nE\r\n".to_vec(),metadata:b"ID;PWXL;N;E\r\nB;Y2;X6\r\nC;X1;Y1;K\"ID\"\r\nC;X2;Y1;K\"field\"\r\nC;X3;Y1;K\"slk\"\r\nC;X4;Y1;K\"type\"\r\nC;X5;Y1;K\"index\"\r\nC;X6;Y1;K\"category\"\r\nC;X1;Y2;K\"STAT\"\r\nC;X2;Y2;K\"Score\"\r\nC;X3;Y2;K\"UnitData\"\r\nC;X4;Y2;K\"string\"\r\nC;X5;Y2;K\"-1\"\r\nC;X6;Y2;K\"synthetic category\"\r\nE\r\n".to_vec(),map_id:identity(&map_info),common_id:identity(&common_info),bj_id:identity(&bj_info),catalog_id:identity(&stock),map_info,common_info,bj_info,catalog_infos:vec![stock],override_reply:None,map_reads:Arc::new(AtomicUsize::new(0)),common_reads:Arc::new(AtomicUsize::new(0)),bj_reads:Arc::new(AtomicUsize::new(0)),catalog_reads:Arc::new(AtomicUsize::new(0))}
    }
    pub(crate) fn rebuild(&mut self) {
        self.raw = fixtures::build_synthetic_map(
            "one selected synthetic map",
            &self
                .files
                .iter()
                .map(|(n, b)| (*n, b.as_slice()))
                .collect::<Vec<_>>(),
        );
    }
    pub(crate) fn member(&self, name: &str) -> &[u8] {
        self.files
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(&[], |(_, b)| b)
    }
    pub(crate) fn resolver(&self) -> Resolver {
        let mode = pcx_assets::GameMode {
            build: pcx_assets::Build::V1_29b,
            addon: pcx_assets::Addon::Tft,
            assets: pcx_assets::AssetMode::Vanilla,
            behaviour: pcx_assets::Behaviour::Vanilla,
        };
        let mut stock = BTreeMap::from([
            (
                SlkId::UnitData.path().to_owned(),
                Reply::Bytes(self.unit_data.clone()),
            ),
            (
                SlkId::UnitMetaData.path().to_owned(),
                Reply::Bytes(self.metadata.clone()),
            ),
        ]);
        let mut common = BTreeMap::from([(
            "chosen common.j".to_owned(),
            Reply::Bytes(self.common.clone()),
        )]);
        let mut bj = BTreeMap::from([(
            "chosen Blizzard.j".to_owned(),
            Reply::Bytes(self.bj.clone()),
        )]);
        if let Some((name, reply)) = &self.override_reply {
            if name == "chosen common.j" {
                common.insert(name.clone(), reply.clone());
            } else if name == "chosen Blizzard.j" {
                bj.insert(name.clone(), reply.clone());
            } else {
                stock.insert(name.clone(), reply.clone());
            }
        }
        Resolver::new(
            mode,
            None,
            vec![
                ArchiveLayer {
                    priority: 8,
                    source: Box::new(SyntheticSource {
                        info: self.map_info.clone(),
                        assets: BTreeMap::from([(
                            "selected map.w3m".to_owned(),
                            Reply::Bytes(self.raw.clone()),
                        )]),
                        reads: self.map_reads.clone(),
                    }),
                },
                ArchiveLayer {
                    priority: 5,
                    source: Box::new(SyntheticSource {
                        info: self.common_info.clone(),
                        assets: common,
                        reads: self.common_reads.clone(),
                    }),
                },
                ArchiveLayer {
                    priority: 3,
                    source: Box::new(SyntheticSource {
                        info: self.bj_info.clone(),
                        assets: bj,
                        reads: self.bj_reads.clone(),
                    }),
                },
                ArchiveLayer {
                    priority: 1,
                    source: Box::new(SyntheticSource {
                        info: self.catalog_infos[0].clone(),
                        assets: stock,
                        reads: self.catalog_reads.clone(),
                    }),
                },
            ],
        )
        .unwrap()
    }
    pub(crate) fn input(&self, revision: u64) -> BindingInputs<'_> {
        BindingInputs {
            map: IdentifiedMember {
                path: "selected map.w3m",
                label: "chosen map label",
                identity: &self.map_id,
                source: &self.map_info,
            },
            common: IdentifiedMember {
                path: "chosen common.j",
                label: "chosen common label",
                identity: &self.common_id,
                source: &self.common_info,
            },
            blizzard: IdentifiedMember {
                path: "chosen Blizzard.j",
                label: "chosen BJ label",
                identity: &self.bj_id,
                source: &self.bj_info,
            },
            catalog_identity: &self.catalog_id,
            catalog_label: "chosen stock catalog",
            catalog_sources: &self.catalog_infos,
            required_catalog_tables: &TABLES,
            required_fields: &FIELDS,
            revision,
        }
    }
    pub(crate) fn limits(&self) -> BindingLimits {
        let map = pcx_map::Map::parse(&self.raw).unwrap();
        let header = pcx_mpq::Header::parse(&mut Cursor::new(map.archive())).unwrap();
        let largest = self.files.iter().map(|(_, b)| b.len()).max().unwrap();
        let prelude = self.common.len() + self.bj.len();
        let scripts = MapScriptBudget {
            max_script_bytes: self.member("war3map.j").len(),
            max_wts_bytes: 0,
            max_archive_bytes: map.archive().len(),
            max_archive_table_bytes: (header.hash_table_bytes() + header.block_table_bytes())
                as usize,
            max_raw_packed_bytes: largest,
            max_decoded_bytes: largest,
            max_prelude_bytes: prelude,
            max_aggregate_input_bytes: map.archive().len()
                + prelude
                + self.member("war3map.j").len(),
        };
        BindingLimits {
            reader: pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 5,
                max_asset_bytes: self.unit_data.len().max(self.metadata.len()),
                max_aggregate_input_bytes: self.unit_data.len() + self.metadata.len(),
            },
            overlay: OverlayLimits {
                unit_bytes: self.member("war3map.w3u").len(),
                item_bytes: self.member("war3map.w3t").len(),
                aggregate_bytes: self.member("war3map.w3u").len()
                    + self.member("war3map.w3t").len(),
            },
            admission: MapAdmissionBudget {
                scripts,
                terrain_bytes: self.member("war3map.w3e").len(),
                info_bytes: self.member("war3map.w3i").len(),
                pathing_bytes: self.member("war3map.wpm").len(),
                units_bytes: self.member("war3mapUnits.doo").len(),
                aggregate_input_bytes: map.archive().len()
                    + prelude
                    + self.files.iter().map(|(_, b)| b.len()).sum::<usize>(),
                required_fields: FIELDS.len(),
            },
            max_map_file_bytes: self.raw.len(),
            max_resolver_input_bytes: self.raw.len()
                + prelude
                + self.unit_data.len()
                + self.metadata.len()
                + self.member("war3map.w3u").len()
                + self.member("war3map.w3t").len(),
        }
    }
}
fn assert_old(
    state: &MapRequests,
    context: *const pcx_play::map_admission::ParsedMapContext,
    receipts: &Arc<BindingReceipts>,
) {
    assert_eq!(state.admission.context().unwrap() as *const _, context);
    assert!(Arc::ptr_eq(
        state.published_receipts.as_ref().unwrap(),
        receipts
    ));
    assert_eq!(state.bindings.as_ref().unwrap().revision, 7);
}
#[cfg_attr(test, test)]
pub fn resolver_to_installed_app_and_receipts() {
    let f = Fixture::new(231);
    let resolver = f.resolver();
    let bindings = bind_map(&resolver, f.input(7), f.limits()).unwrap();
    assert_eq!(bindings.catalog_map_bytes.as_ref(), f.raw);
    assert_eq!(bindings.receipts.map.provenance.source, f.map_info);
    assert_eq!(bindings.receipts.common.provenance.source, f.common_info);
    assert_eq!(bindings.receipts.blizzard.provenance.source, f.bj_info);
    assert_eq!(bindings.receipts.catalog.len(), 2);
    assert!(
        bindings
            .receipts
            .catalog
            .iter()
            .all(|r| r.provenance.source == f.catalog_infos[0]
                && r.provenance.archive_priority == Some(1)
                && r.provenance.locator.contains("exact locator"))
    );
    assert!(
        bindings
            .receipts
            .missing_catalog
            .iter()
            .any(|p| p == SlkId::UnitBalance.path())
    );
    let overlay = bindings.receipts.units.as_ref().unwrap();
    assert_eq!(overlay.hash, pcx_mpq::fnv1a64(f.member("war3map.w3u")));
    assert_eq!(overlay.map_source, f.map_info);
    assert!(bindings.receipts.items.is_none());
    let def = bindings.catalog.resolve(*b"UNIT").unwrap();
    assert!(matches!(
        def.fields[b"STAT"].value,
        Ok(FieldValue::Override(pcx_objects::Value::Int(231)))
    ));
    assert_eq!(def.modifications[0].end_word.to_le_bytes(), *b"TAIL");
    let raw = bindings.catalog_map_bytes.clone();
    let mut app = App::new();
    app.insert_resource(MapRequests {
        bindings: Some(bindings),
        ..Default::default()
    });
    peacecraft3::map_requests::install(&mut app);
    app.world_mut().write_message(MapRequest {
        label: "chosen map label".into(),
        map_member: AssetPath::new("selected map.w3m").unwrap(),
        identity: f.map_id.clone(),
        binding_revision: 7,
        payload: MapPayload::Shared(raw),
    });
    app.update();
    let state = app.world().resource::<MapRequests>();
    assert!(state.last_outcome.as_ref().unwrap().result.is_ok());
    let context = state.admission.context().unwrap();
    assert_eq!(context.sources.map, f.map_id);
    assert_eq!(context.sources.common, f.common_id);
    assert_eq!(context.sources.blizzard, f.bj_id);
    assert_eq!(context.sources.catalog, f.catalog_id);
    assert_eq!(context.required_fields, FIELDS);
    assert_eq!(context.map_digest.hash, pcx_mpq::fnv1a64(&f.raw));
    assert_eq!(
        state
            .published_receipts
            .as_ref()
            .unwrap()
            .map
            .provenance
            .locator,
        "map exact locator::selected map.w3m"
    );
    let def = &context.world.definitions[*context.world.records[0].definition.as_ref().unwrap()];
    assert!(matches!(
        def.fields[b"STAT"].value,
        Ok(FieldValue::Override(pcx_objects::Value::Int(231)))
    ));
}
#[cfg_attr(test, test)]
pub fn atomic_failed_binding_and_admission() {
    let f = Fixture::new(232);
    let resolver = f.resolver();
    let mut state = MapRequests::default();
    assert!(
        state
            .replace_selected(&resolver, f.input(7), f.limits())
            .result
            .is_ok()
    );
    let old = state.admission.context().unwrap() as *const _;
    let receipts = state.published_receipts.as_ref().unwrap().clone();
    let mut failed = Fixture::new(900);
    failed.override_reply = Some(("chosen common.j".into(), Reply::Missing));
    assert!(
        state
            .replace_selected(&failed.resolver(), failed.input(8), failed.limits())
            .result
            .is_err()
    );
    assert_old(&state, old, &receipts);
    assert!(
        matches!(&state.last_outcome.as_ref().unwrap().result,Err(RequestFailure::Binding(error)) if matches!(error.as_ref(),BindingError::Resolve{source:pcx_assets::Error::MissingContent{..},..}))
    );
    let mut malformed = Fixture::new(901);
    malformed.bj = b"function broken takes nothing returns nothing\n@\nendfunction\n".to_vec();
    assert!(
        state
            .replace_selected(
                &malformed.resolver(),
                malformed.input(8),
                malformed.limits()
            )
            .result
            .is_err()
    );
    assert_old(&state, old, &receipts);
    assert!(matches!(
        state.last_outcome.as_ref().unwrap().result,
        Err(RequestFailure::Admission(_))
    ));
    assert_eq!(
        state
            .last_outcome
            .as_ref()
            .unwrap()
            .identity
            .as_ref()
            .unwrap(),
        &malformed.map_id
    );
    let replacement = Fixture::new(902);
    assert!(
        state
            .replace_selected(
                &replacement.resolver(),
                replacement.input(8),
                replacement.limits()
            )
            .result
            .is_ok()
    );
    assert_eq!(state.bindings.as_ref().unwrap().revision, 8);
    assert!(!Arc::ptr_eq(
        state.published_receipts.as_ref().unwrap(),
        &receipts
    ));
    assert_eq!(
        state
            .published_receipts
            .as_ref()
            .unwrap()
            .units
            .as_ref()
            .unwrap()
            .hash,
        pcx_mpq::fnv1a64(replacement.member("war3map.w3u"))
    );
}
#[cfg_attr(test, test)]
pub fn stale_and_different_map_refuse() {
    let f = Fixture::new(233);
    let mut state = MapRequests::default();
    assert!(
        state
            .replace_selected(&f.resolver(), f.input(7), f.limits())
            .result
            .is_ok()
    );
    let old = state.admission.context().unwrap() as *const _;
    let receipts = state.published_receipts.as_ref().unwrap().clone();
    state.deliver(MapRequest {
        label: "stale caller".into(),
        map_member: AssetPath::new("selected map.w3m").unwrap(),
        identity: f.map_id.clone(),
        binding_revision: 6,
        payload: MapPayload::Bytes(f.raw.clone()),
    });
    assert!(matches!(
        state.last_outcome.as_ref().unwrap().result,
        Err(RequestFailure::StaleBindings { .. })
    ));
    assert_old(&state, old, &receipts);
    let other = Fixture::new(234);
    state.deliver(MapRequest {
        label: "different map".into(),
        map_member: AssetPath::new("selected map.w3m").unwrap(),
        identity: other.map_id.clone(),
        binding_revision: 7,
        payload: MapPayload::Bytes(other.raw.clone()),
    });
    assert!(matches!(
        state.last_outcome.as_ref().unwrap().result,
        Err(RequestFailure::CatalogMapMismatch)
    ));
    assert_old(&state, old, &receipts);
}
#[cfg_attr(test, test)]
pub fn map_member_is_bound_before_payload_reads() {
    let f = Fixture::new(235);
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    assert!(
        app.world_mut()
            .resource_mut::<MapRequests>()
            .replace_selected(&f.resolver(), f.input(7), f.limits())
            .result
            .is_ok()
    );
    let state = app.world().resource::<MapRequests>();
    let receipts = state.published_receipts.as_ref().unwrap().clone();
    let mut valid = state.selected_map_request().unwrap();
    assert_eq!(valid.map_member, receipts.map.path);
    valid.label = "different diagnostic label".into();
    valid.map_member = AssetPath::new("SELECTED MAP.W3M").unwrap();
    let MapPayload::Shared(bytes) = &valid.payload else {
        panic!("shared request");
    };
    assert!(Arc::ptr_eq(
        bytes,
        &state.bindings.as_ref().unwrap().catalog_map_bytes
    ));
    app.world_mut().write_message(valid);
    app.update();
    let state = app.world().resource::<MapRequests>();
    assert!(state.last_outcome.as_ref().unwrap().result.is_ok());
    assert_eq!(
        state.last_outcome.as_ref().unwrap().label,
        "different diagnostic label"
    );
    let old = state.admission.context().unwrap() as *const _;
    for payload in [
        MapPayload::Shared(Arc::from(f.raw.clone())),
        MapPayload::Bytes(Vec::new()),
    ] {
        let mut forged = app
            .world()
            .resource::<MapRequests>()
            .selected_map_request()
            .unwrap();
        forged.map_member = AssetPath::new("another map.w3m").unwrap();
        forged.payload = payload;
        app.world_mut().write_message(forged);
        app.update();
        let state = app.world().resource::<MapRequests>();
        assert!(matches!(
            &state.last_outcome.as_ref().unwrap().result,
            Err(RequestFailure::MemberMismatch { requested, bound })
                if requested.as_str() == "another map.w3m" && *bound == receipts.map.path
        ));
        assert_old(state, old, &receipts);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut forged = app
            .world()
            .resource::<MapRequests>()
            .selected_map_request()
            .unwrap();
        forged.map_member = AssetPath::new("another map.w3m").unwrap();
        forged.payload = MapPayload::File(std::path::PathBuf::new());
        app.world_mut().write_message(forged);
        app.update();
        let state = app.world().resource::<MapRequests>();
        assert!(matches!(
            state.last_outcome.as_ref().unwrap().result,
            Err(RequestFailure::MemberMismatch { .. })
        ));
        assert_old(state, old, &receipts);
    }
}

#[cfg_attr(test, test)]
pub fn missing_ambiguous_malformed_sources_distinct() {
    let f = Fixture::new(235);
    let resolver = f.resolver();
    let mut missing = f.input(7);
    missing.common.path = "unmounted common.j";
    assert!(matches!(
        bind_map(&resolver, missing, f.limits()),
        Err(BindingError::Resolve {
            source: pcx_assets::Error::MissingAsset { .. },
            ..
        })
    ));
    for reply in [Reply::Ambiguous, Reply::Invalid, Reply::Missing] {
        let mut fixture = Fixture::new(236);
        fixture.override_reply = Some((SlkId::UnitData.path().into(), reply.clone()));
        let error = bind_map(&fixture.resolver(), fixture.input(7), fixture.limits())
            .err()
            .unwrap();
        let BindingError::Editor {
            source: pcx_objects::Error::EditorAsset { source, .. },
            ..
        } = error
        else {
            panic!("reader cause missing")
        };
        let cause = source.downcast_ref::<BindingError>().unwrap();
        match reply {
            Reply::Ambiguous => assert!(matches!(
                cause,
                BindingError::Resolve {
                    source: pcx_assets::Error::Source {
                        source: SourceError::Ambiguous { .. },
                        ..
                    },
                    ..
                }
            )),
            Reply::Invalid => assert!(matches!(
                cause,
                BindingError::Resolve {
                    source: pcx_assets::Error::Source {
                        source: SourceError::InvalidInput { .. },
                        ..
                    },
                    ..
                }
            )),
            Reply::Missing => assert!(matches!(
                cause,
                BindingError::Resolve {
                    source: pcx_assets::Error::MissingContent { .. },
                    ..
                }
            )),
            Reply::Bytes(_) => unreachable!(),
        }
    }
    let mut malformed = Fixture::new(237);
    malformed.unit_data = b"invalid present SYLK".to_vec();
    let error = bind_map(
        &malformed.resolver(),
        malformed.input(7),
        malformed.limits(),
    )
    .err()
    .unwrap();
    assert!(
        matches!(error,BindingError::Editor {source:pcx_objects::Error::EditorAsset{member,source,..},..} if member==SlkId::UnitData.path() && source.downcast_ref::<pcx_slq::Error>().is_some())
    );
    let f = Fixture::new(238);
    let mut input = f.input(7);
    input.common.source = &f.bj_info;
    assert!(matches!(
        bind_map(&f.resolver(), input, f.limits()),
        Err(BindingError::SourceMismatch { .. })
    ));
}
#[cfg_attr(test, test)]
pub fn overlay_absence_parse_and_preflight() {
    let mut absent = Fixture::new(239);
    absent.files.retain(|(name, _)| *name != "war3map.w3u");
    absent.rebuild();
    let bound = bind_map(&absent.resolver(), absent.input(7), absent.limits()).unwrap();
    assert!(bound.receipts.units.is_none());
    assert!(bound.receipts.items.is_none());
    let def = bound.catalog.resolve(*b"UNIT").unwrap();
    assert!(
        matches!(&def.fields[b"STAT"].value,Ok(FieldValue::Cell(value)) if value=="authored stock cell")
    );
    let mut malformed = Fixture::new(240);
    malformed
        .files
        .iter_mut()
        .find(|(name, _)| *name == "war3map.w3u")
        .unwrap()
        .1 = vec![0];
    malformed.rebuild();
    assert!(matches!(
        bind_map(
            &malformed.resolver(),
            malformed.input(7),
            malformed.limits()
        ),
        Err(BindingError::OverlayParse {
            member: "war3map.w3u",
            ..
        })
    ));
    let mut duplicate = Fixture::new(246);
    let mut custom = Vec::new();
    custom.extend(1u32.to_le_bytes());
    custom.extend(0u32.to_le_bytes());
    custom.extend(2u32.to_le_bytes());
    for value in [311u32, 312] {
        custom.extend(b"UNIT");
        custom.extend(b"NEWW");
        custom.extend(1u32.to_le_bytes());
        custom.extend(b"STAT");
        custom.extend(0u32.to_le_bytes());
        custom.extend(value.to_le_bytes());
        custom.extend(b"TAIL");
    }
    duplicate
        .files
        .iter_mut()
        .find(|(name, _)| *name == "war3map.w3u")
        .unwrap()
        .1 = custom;
    duplicate.rebuild();
    assert!(
        matches!(bind_map(&duplicate.resolver(),duplicate.input(7),duplicate.limits()),Err(BindingError::AmbiguousOverlay{rawcode,first_record:0,second_record:1,..}) if rawcode==*b"NEWW")
    );
    let mut forged = Fixture::new(241);
    let mut limits = forged.limits();
    fixtures::alter_block(&mut forged.raw, "war3map.w3u", |block| {
        block[8..12].copy_from_slice(&u32::MAX.to_le_bytes())
    });
    assert!(matches!(
        bind_map(&forged.resolver(), forged.input(7), limits),
        Err(BindingError::OverlayRead { source, .. })
            if matches!(*source, pcx_mpq::checked::ReadError::BudgetExceeded { .. })
    ));
    assert_eq!(forged.common_reads.load(Ordering::SeqCst), 0);
    assert_eq!(forged.catalog_reads.load(Ordering::SeqCst), 0);
    let mut packed = Fixture::new(242);
    packed
        .files
        .iter_mut()
        .find(|(name, _)| *name == "war3map.w3u")
        .unwrap()
        .1 = vec![4, 0xAA];
    packed.files.push((
        "war3map.w3t",
        [1u32.to_le_bytes(), 0u32.to_le_bytes(), 0u32.to_le_bytes()].concat(),
    ));
    packed.rebuild();
    limits = packed.limits();
    fixtures::alter_block(&mut packed.raw, "war3map.w3u", |block| {
        block[8..12].copy_from_slice(&3u32.to_le_bytes());
        block[12..16].copy_from_slice(&0x81000200u32.to_le_bytes());
    });
    limits.overlay.unit_bytes = 3;
    limits.overlay.aggregate_bytes = 14;
    assert!(matches!(
        bind_map(&packed.resolver(), packed.input(7), limits),
        Err(BindingError::Budget {
            actual: 15,
            limit: 14,
            ..
        })
    ));
    assert_eq!(packed.common_reads.load(Ordering::SeqCst), 0);
    limits.overlay.aggregate_bytes = 15;
    assert!(matches!(
        bind_map(&packed.resolver(), packed.input(7), limits),
        Err(BindingError::OverlayRead { source, .. })
            if matches!(*source, pcx_mpq::checked::ReadError::ExtractionFailed { .. })
    ));
}
#[cfg_attr(test, test)]
pub fn exact_limits_and_deterministic_stop() {
    let f = Fixture::new(243);
    let limits = f.limits();
    assert!(bind_map(&f.resolver(), f.input(7), limits).is_ok());
    let mut limited = limits;
    limited.max_resolver_input_bytes -= 1;
    assert!(bind_map(&f.resolver(), f.input(7), limited).is_err());
    let mut limited = limits;
    limited.reader.max_asset_bytes -= 1;
    assert!(bind_map(&f.resolver(), f.input(7), limited).is_err());
    let mut limited = limits;
    limited.reader.max_aggregate_input_bytes -= 1;
    assert!(bind_map(&f.resolver(), f.input(7), limited).is_err());
    let mut limited = limits;
    limited.reader.max_listed_paths -= 1;
    assert!(matches!(
        bind_map(&f.resolver(), f.input(7), limited),
        Err(BindingError::Budget { .. })
    ));
    let f = Fixture::new(244);
    let mut limits = f.limits();
    limits.max_map_file_bytes = f.raw.len() - 1;
    assert!(matches!(
        bind_map(&f.resolver(), f.input(7), limits),
        Err(BindingError::Budget { .. })
    ));
    assert_eq!(f.map_reads.load(Ordering::SeqCst), 1);
    assert_eq!(f.common_reads.load(Ordering::SeqCst), 0);
    assert_eq!(f.catalog_reads.load(Ordering::SeqCst), 0);
    let f = Fixture::new(245);
    let mut limits = f.limits();
    limits.admission.scripts.max_prelude_bytes = f.common.len() - 1;
    assert!(matches!(
        bind_map(&f.resolver(), f.input(7), limits),
        Err(BindingError::Budget { .. })
    ));
    assert_eq!(f.common_reads.load(Ordering::SeqCst), 1);
    assert_eq!(f.bj_reads.load(Ordering::SeqCst), 0);
    assert_eq!(f.catalog_reads.load(Ordering::SeqCst), 0);
}

#[cfg_attr(test, test)]
pub fn message_order_keeps_last_success_and_reports_last_refusal() {
    let f = Fixture::new(240);
    let bindings = bind_map(&f.resolver(), f.input(7), f.limits()).unwrap();
    let bytes = bindings.catalog_map_bytes.clone();
    let mut app = App::new();
    app.insert_resource(MapRequests {
        bindings: Some(bindings),
        ..Default::default()
    });
    peacecraft3::map_requests::install(&mut app);
    for (label, revision) in [
        ("first accepted", 7),
        ("second accepted", 7),
        ("last refused", 8),
    ] {
        app.world_mut().write_message(MapRequest {
            label: label.into(),
            map_member: AssetPath::new("selected map.w3m").unwrap(),
            identity: f.map_id.clone(),
            binding_revision: revision,
            payload: MapPayload::Shared(bytes.clone()),
        });
    }
    app.update();
    let state = app.world().resource::<MapRequests>();
    assert_eq!(
        state.admission.context().unwrap().map_digest.label,
        "second accepted"
    );
    assert_eq!(state.last_outcome.as_ref().unwrap().label, "last refused");
    assert!(matches!(
        state.last_outcome.as_ref().unwrap().result,
        Err(RequestFailure::StaleBindings {
            requested: 8,
            current: 7
        })
    ));
    assert!(Arc::ptr_eq(
        state.published_receipts.as_ref().unwrap(),
        &state.bindings.as_ref().unwrap().receipts
    ));
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(test, test)]
pub fn native_file_drop_uses_exact_bytes_and_preserves_failed_publication() {
    let f = Fixture::new(241);
    let root = std::env::temp_dir().join(format!(
        "pcx-map-loading-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _directory = Directory(root.clone());
    let path = root.join("caller-selected.w3m");
    std::fs::write(&path, &f.raw).unwrap();
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    let emit = |app: &mut App, path: &std::path::Path| {
        app.world_mut()
            .write_message(bevy::window::FileDragAndDrop::DroppedFile {
                window: Entity::PLACEHOLDER,
                path_buf: path.to_owned(),
            });
        app.update();
    };
    emit(&mut app, &root.join("not-present.w3m"));
    assert!(matches!(
        app.world()
            .resource::<MapRequests>()
            .last_outcome
            .as_ref()
            .unwrap()
            .result,
        Err(RequestFailure::MissingBindings)
    ));
    app.world_mut().resource_mut::<MapRequests>().bindings =
        Some(bind_map(&f.resolver(), f.input(7), f.limits()).unwrap());
    emit(&mut app, &path);
    let state = app.world().resource::<MapRequests>();
    assert!(state.last_outcome.as_ref().unwrap().result.is_ok());
    let previous = state.admission.shared_context().unwrap();
    let receipts = state.published_receipts.as_ref().unwrap().clone();
    app.world_mut()
        .resource_mut::<MapRequests>()
        .bindings
        .as_mut()
        .unwrap()
        .max_map_file_bytes = f.raw.len() - 1;
    emit(&mut app, &path);
    let state = app.world().resource::<MapRequests>();
    assert!(matches!(
        state.last_outcome.as_ref().unwrap().result,
        Err(RequestFailure::FileBudget { .. })
    ));
    assert!(Arc::ptr_eq(
        &state.admission.shared_context().unwrap(),
        &previous
    ));
    assert!(Arc::ptr_eq(
        state.published_receipts.as_ref().unwrap(),
        &receipts
    ));
    app.world_mut()
        .resource_mut::<MapRequests>()
        .bindings
        .as_mut()
        .unwrap()
        .max_map_file_bytes = f.raw.len();
    let mut altered = f.raw.clone();
    altered[0] ^= 0x01;
    std::fs::write(&path, altered).unwrap();
    emit(&mut app, &path);
    let state = app.world().resource::<MapRequests>();
    assert!(matches!(
        state.last_outcome.as_ref().unwrap().result,
        Err(RequestFailure::CatalogMapMismatch)
    ));
    assert!(Arc::ptr_eq(
        &state.admission.shared_context().unwrap(),
        &previous
    ));
    assert!(Arc::ptr_eq(
        state.published_receipts.as_ref().unwrap(),
        &receipts
    ));
    emit(&mut app, &root);
    assert!(matches!(
        app.world()
            .resource::<MapRequests>()
            .last_outcome
            .as_ref()
            .unwrap()
            .result,
        Err(RequestFailure::File(_))
    ));
}

fn install_source_controls(app: &mut App) {
    crate::source_controls::install(app);
    app.add_systems(
        Update,
        crate::button_actions::clear.after(MapRequestSet::Deliver),
    );
}
fn spawn_source_control_nodes(world: &mut World) -> (Entity, Entity) {
    let button = world
        .spawn((
            bevy::ui_widgets::Button,
            crate::source_controls::LoadSelectedMap,
        ))
        .id();
    let text = world
        .spawn((Text::new(""), crate::source_controls::SelectedMapText))
        .id();
    (button, text)
}
#[cfg_attr(test, test)]
pub fn actual_source_card_click_preserves_configured_identity() {
    let f = Fixture::new(801);
    let bindings = bind_map(&f.resolver(), f.input(13), f.limits()).unwrap();
    let shared = bindings.catalog_map_bytes.clone();
    assert_eq!(bindings.map_identity, f.map_id);
    assert_eq!(bindings.map_label, "chosen map label");
    let mut app = App::new();
    app.insert_resource(MapRequests {
        bindings: Some(bindings),
        ..Default::default()
    });
    peacecraft3::map_requests::install(&mut app);
    install_source_controls(&mut app);
    let (button, text) = spawn_source_control_nodes(app.world_mut());
    app.update();
    let before = app.world().get::<Text>(text).unwrap().0.clone();
    assert!(before.contains("Selected map: chosen map label"));
    assert!(before.contains("map exact locator::selected map.w3m"));
    assert!(before.contains(&f.map_id.edition));
    assert!(
        app.world()
            .resource::<MapRequests>()
            .admission
            .context()
            .is_none()
    );
    let request = app
        .world()
        .resource::<MapRequests>()
        .selected_map_request()
        .unwrap();
    assert_eq!(request.identity, f.map_id);
    assert_eq!(request.label, "chosen map label");
    assert_eq!(
        request.map_member,
        AssetPath::new("selected map.w3m").unwrap()
    );
    assert_ne!(request.map_member.as_str(), request.label);
    assert_eq!(request.binding_revision, 13);
    let MapPayload::Shared(bytes) = request.payload else {
        panic!("unexpected full-copy payload")
    };
    assert!(Arc::ptr_eq(&bytes, &shared));
    let reads = f.map_reads.load(Ordering::SeqCst)
        + f.common_reads.load(Ordering::SeqCst)
        + f.bj_reads.load(Ordering::SeqCst)
        + f.catalog_reads.load(Ordering::SeqCst);
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: button });
    app.update();
    let state = app.world().resource::<MapRequests>();
    assert!(state.last_outcome.as_ref().unwrap().result.is_ok());
    assert_eq!(
        state
            .last_outcome
            .as_ref()
            .unwrap()
            .identity
            .as_ref()
            .unwrap(),
        &f.map_id
    );
    assert_eq!(state.admission.context().unwrap().sources.map, f.map_id);
    assert_eq!(
        reads,
        f.map_reads.load(Ordering::SeqCst)
            + f.common_reads.load(Ordering::SeqCst)
            + f.bj_reads.load(Ordering::SeqCst)
            + f.catalog_reads.load(Ordering::SeqCst)
    );
    assert!(
        app.world()
            .get::<Text>(text)
            .unwrap()
            .0
            .contains("Map data parsed. Gameplay is not started.")
    );
    let old = state.admission.context().unwrap() as *const _;
    app.update();
    assert_eq!(
        old,
        app.world()
            .resource::<MapRequests>()
            .admission
            .context()
            .unwrap() as *const _
    );
}
#[cfg_attr(test, test)]
pub fn unconfigured_source_card_has_typed_requirements() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    install_source_controls(&mut app);
    let (button, text) = spawn_source_control_nodes(app.world_mut());
    app.update();
    let state = app.world().resource::<MapRequests>();
    let required = state.missing_binding_requirements();
    assert!(required.contains(&MissingBindingRequirement::SourceMount));
    assert!(required.contains(&MissingBindingRequirement::PreludeIdentities));
    assert!(required.contains(&MissingBindingRequirement::RequiredDataFields));
    assert!(required.contains(&MissingBindingRequirement::InputReadLimits));
    assert!(matches!(
        state.selected_map_request(),
        Err(RequestFailure::MissingBindings)
    ));
    assert!(
        app.world()
            .get::<Text>(text)
            .unwrap()
            .0
            .contains("No map sources configured")
    );
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: button });
    app.update();
    let state = app.world().resource::<MapRequests>();
    assert!(state.admission.context().is_none());
    assert!(state.last_outcome.as_ref().unwrap().identity.is_none());
    assert!(matches!(
        state.last_outcome.as_ref().unwrap().result,
        Err(RequestFailure::MissingBindings)
    ));
}
#[cfg_attr(test, test)]
pub fn configured_source_card_failure_retains_context_and_receipts() {
    let f = Fixture::new(802);
    let mut app = App::new();
    app.insert_resource(MapRequests::default());
    assert!(
        app.world_mut()
            .resource_mut::<MapRequests>()
            .replace_selected(&f.resolver(), f.input(7), f.limits())
            .result
            .is_ok()
    );
    peacecraft3::map_requests::install(&mut app);
    install_source_controls(&mut app);
    let (button, text) = spawn_source_control_nodes(app.world_mut());
    let old = app
        .world()
        .resource::<MapRequests>()
        .admission
        .context()
        .unwrap() as *const _;
    let receipts = app
        .world()
        .resource::<MapRequests>()
        .published_receipts
        .as_ref()
        .unwrap()
        .clone();
    let old_raw = app
        .world()
        .resource::<MapRequests>()
        .bindings
        .as_ref()
        .unwrap()
        .catalog_map_bytes
        .clone();
    let mut invalid = old_raw.to_vec();
    invalid[0] = 0;
    app.world_mut()
        .resource_mut::<MapRequests>()
        .bindings
        .as_mut()
        .unwrap()
        .catalog_map_bytes = Arc::from(invalid);
    app.world_mut()
        .trigger(bevy::ui_widgets::Activate { entity: button });
    app.update();
    let state = app.world().resource::<MapRequests>();
    assert_eq!(old, state.admission.context().unwrap() as *const _);
    assert!(Arc::ptr_eq(
        state.published_receipts.as_ref().unwrap(),
        &receipts
    ));
    assert!(matches!(
        state.last_outcome.as_ref().unwrap().result,
        Err(RequestFailure::Container(_))
    ));
    assert!(
        app.world()
            .get::<Text>(text)
            .unwrap()
            .0
            .contains("Map request failed:")
    );
}

#[cfg_attr(test, test)]
pub fn bound_catalog_is_shared_with_executed_unit_admission() {
    use pcx_play::playercolor_host::{EnumHost, Limits, ObjectHost, UnitBudget};
    let mut f = Fixture::new(236);
    f.common.extend_from_slice(b"type agent extends handle\ntype widget extends agent\ntype unit extends widget\ntype player extends agent\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateUnit takes player p, integer id, real x, real y, real face returns unit\n");
    let bindings = bind_map_bounded(&f.resolver(), f.input(1), f.limits()).unwrap();
    let mut state = MapRequests {
        bindings: Some(bindings),
        ..Default::default()
    };
    let request = state.selected_map_request().unwrap();
    state.deliver(request);
    let context = state.admission.context().unwrap();
    let inner = EnumHost::checked_selected(
        context.scripts.shared_program(),
        "selected fixture",
        &[],
        Limits {
            max_handles: 8,
            admission_steps: 0,
            expression_depth: 1,
            expression_work: 0,
        },
    )
    .unwrap();
    let host = state
        .bindings
        .as_ref()
        .unwrap()
        .admit_executed_units(
            ObjectHost::checked(inner).unwrap(),
            UnitBudget {
                max_records: 1,
                max_retained_fields: 1,
                max_modifications: 1,
            },
        )
        .unwrap();
    let selected = &state.bindings.as_ref().unwrap().catalog.0;
    assert!(Arc::ptr_eq(&host.unit_catalog().unwrap(), selected));
    assert_eq!(
        host.unit_catalog().unwrap().resolve(*b"UNIT"),
        selected.resolve(*b"UNIT")
    );
    let declaration = context
        .scripts
        .program()
        .natives()
        .get("CreateUnit")
        .unwrap();
    assert_eq!(
        host.unit_declaration().unwrap(),
        (declaration.source.as_str(), declaration.span)
    );
}
