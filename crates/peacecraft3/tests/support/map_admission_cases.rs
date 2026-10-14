use crate::fixtures;
use pcx_map::Map;
use pcx_mpq::checked::ReadError;
use pcx_play::map_admission::{
    IdentifiedScript, MapAdmissionBudget, MapAdmissionError, MapAdmissionInputs,
};
use pcx_play::map_scripts_import::{MapScriptBudget, MapScriptImportError};
use pcx_play::scripts::{AssetIdentity, ScriptAsset};
use pcx_sim::world::{FieldValue, RecordProblem};
use peacecraft3::map_admission::MapAdmission;
use std::io::Cursor;
const FIELDS: [[u8; 4]; 2] = [*b"STAT", *b"STAT"];
struct Fixture {
    files: Vec<(&'static str, Vec<u8>)>,
    map_identity: AssetIdentity,
    common_identity: AssetIdentity,
    bj_identity: AssetIdentity,
    catalog_identity: AssetIdentity,
    common: Vec<u8>,
    bj: Vec<u8>,
    catalog: fixtures::Catalog,
}
fn identity(source: &str) -> AssetIdentity {
    AssetIdentity {
        edition: "caller-selected synthetic edition".into(),
        version: "caller-selected revision".into(),
        source: source.into(),
    }
}
impl Fixture {
    fn new(marker: &str, x: f32) -> Self {
        let mut units = fixtures::units();
        units.instances[0].position[0] = x;
        Self {files:vec![("war3map.w3e",fixtures::terrain_bytes(2,1)),("war3map.wpm",fixtures::pathing_bytes(4,2)),("war3map.w3i",fixtures::info().canonical_bytes()),("war3mapUnits.doo",units.canonical_bytes()),("war3map.j",format!("globals\ninteger AuthoredMarker = {marker}\ninteger RequiresHost = FixtureHeld()\nendglobals\nfunction config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n").into_bytes())],map_identity:identity(&format!("{}:{marker}",fixtures::source().install)),common_identity:identity("supplied-common source"),bj_identity:identity("supplied-BJ source"),catalog_identity:identity("supplied-catalog source"),common:b"native FixtureHeld takes nothing returns integer\n".to_vec(),bj:b"// supplied BJ\n".to_vec(),catalog:fixtures::catalog()}
    }
    fn bytes(&self) -> Vec<u8> {
        fixtures::build_synthetic_map(
            "caller synthetic map",
            &self
                .files
                .iter()
                .map(|(n, b)| (*n, b.as_slice()))
                .collect::<Vec<_>>(),
        )
    }
    fn member(&self, name: &str) -> &[u8] {
        self.files
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(&[], |(_, b)| b.as_slice())
    }
    fn inputs<'a>(
        &'a self,
        map: &'a Map<'a>,
        label: &'a str,
    ) -> MapAdmissionInputs<'a, fixtures::Catalog> {
        MapAdmissionInputs {
            map,
            map_label: label,
            map_identity: &self.map_identity,
            common: IdentifiedScript {
                identity: &self.common_identity,
                asset: ScriptAsset {
                    label: "chosen-common.fixture",
                    bytes: &self.common,
                },
            },
            blizzard: IdentifiedScript {
                identity: &self.bj_identity,
                asset: ScriptAsset {
                    label: "chosen-bj.fixture",
                    bytes: &self.bj,
                },
            },
            catalog_identity: &self.catalog_identity,
            catalog: &self.catalog,
            required_fields: &FIELDS,
        }
    }
    fn budget(&self, map: &Map<'_>) -> MapAdmissionBudget {
        let header = pcx_mpq::Header::parse(&mut Cursor::new(map.archive())).unwrap();
        let largest = self.files.iter().map(|(_, b)| b.len()).max().unwrap();
        let preludes = self.common.len() + self.bj.len();
        let scripts = MapScriptBudget {
            max_script_bytes: self.member("war3map.j").len(),
            max_wts_bytes: self.member("war3map.wts").len(),
            max_archive_bytes: map.archive().len(),
            max_archive_table_bytes: (header.hash_table_bytes() + header.block_table_bytes())
                as usize,
            max_raw_packed_bytes: largest,
            max_decoded_bytes: largest,
            max_prelude_bytes: preludes,
            max_aggregate_input_bytes: map.archive().len()
                + preludes
                + self.member("war3map.j").len()
                + self.member("war3map.wts").len(),
        };
        MapAdmissionBudget {
            scripts,
            terrain_bytes: self.member("war3map.w3e").len(),
            pathing_bytes: self.member("war3map.wpm").len(),
            info_bytes: self.member("war3map.w3i").len(),
            units_bytes: self.member("war3mapUnits.doo").len(),
            aggregate_input_bytes: map.archive().len()
                + preludes
                + self.files.iter().map(|(_, b)| b.len()).sum::<usize>(),
            required_fields: FIELDS.len(),
        }
    }
}
#[cfg_attr(test, test)]
pub fn app_consumer_publishes_all_sources_and_same_map_data_without_execution() {
    let a = Fixture::new("101", 1.25);
    let ab = a.bytes();
    let am = Map::parse(&ab).unwrap();
    let mut app = MapAdmission::default();
    app.replace(a.inputs(&am, "selected-A"), a.budget(&am))
        .unwrap();
    let context = app.context().unwrap();
    assert_eq!(context.map_digest.hash, pcx_mpq::fnv1a64(&ab));
    assert_eq!(context.sources.map, a.map_identity);
    assert_eq!(context.sources.common, a.common_identity);
    assert_eq!(context.sources.blizzard, a.bj_identity);
    assert_eq!(context.sources.catalog, a.catalog_identity);
    assert_eq!(context.scripts.identity, a.map_identity);
    assert_eq!(context.required_fields, FIELDS);
    assert_eq!(context.world.source.map, "selected-A");
    assert_eq!(context.world.source.install, a.map_identity.source);
    assert_eq!(context.terrain.raw(), a.member("war3map.w3e"));
    assert_eq!(context.world.info.raw(), a.member("war3map.w3i"));
    assert_eq!(
        context.world.records[0].placement.position[0].to_bits(),
        1.25f32.to_bits()
    );
    assert_eq!(context.world.records[0].placement.life, -1);
    assert_eq!(context.world.records[0].placement.mana, -1);
    assert_eq!(
        context.world.records[0].placement.control_word,
        fixtures::placement(*b"UNIT").control_word
    );
    assert!(
        matches!(context.world.definitions[0].fields[b"STAT"].value,Ok(FieldValue::Cell(ref value)) if value=="137")
    );
    assert!(matches!(
        context.world.records[1].problems.first(),
        Some(RecordProblem::Definition(_))
    ));
    assert!(
        context
            .scripts
            .digests
            .iter()
            .any(|d| d.label == "selected-A:war3map.j"
                && d.hash == pcx_mpq::fnv1a64(a.member("war3map.j")))
    );
    assert_eq!(context.world_member_digests.len(), 4);
    for digest in &context.world_member_digests {
        let (_, name) = digest.label.split_once(':').unwrap();
        assert_eq!(digest.hash, pcx_mpq::fnv1a64(a.member(name)));
    }
    let b = Fixture::new("202", -19.5);
    let bb = b.bytes();
    let bm = Map::parse(&bb).unwrap();
    app.replace(b.inputs(&bm, "selected-B"), b.budget(&bm))
        .unwrap();
    let context = app.context().unwrap();
    assert_eq!(context.map_digest.hash, pcx_mpq::fnv1a64(&bb));
    assert_ne!(context.map_digest.hash, pcx_mpq::fnv1a64(&ab));
    assert_eq!(
        context.world.records[0].placement.position[0].to_bits(),
        (-19.5f32).to_bits()
    );
    assert_eq!(context.world.source.map, "selected-B");
    assert!(
        context
            .scripts
            .digests
            .iter()
            .any(|d| d.label == "selected-B:war3map.j"
                && d.hash == pcx_mpq::fnv1a64(b.member("war3map.j")))
    );
}
#[cfg_attr(test, test)]
pub fn failed_preflight_parse_compile_or_identity_preserves_the_old_context() {
    let a = Fixture::new("101", 1.25);
    let bytes = a.bytes();
    let map = Map::parse(&bytes).unwrap();
    let mut app = MapAdmission::default();
    app.replace(a.inputs(&map, "old"), a.budget(&map)).unwrap();
    let old = app.context().unwrap() as *const _;
    let digest = app.context().unwrap().map_digest.hash;
    for which in 0..4 {
        let mut bad = Fixture::new("202", 8.0);
        if which == 0 {
            bad.files
                .iter_mut()
                .find(|(n, _)| *n == "war3map.w3i")
                .unwrap()
                .1 = vec![0];
        }
        if which == 1 {
            bad.files
                .iter_mut()
                .find(|(n, _)| *n == "war3map.j")
                .unwrap()
                .1 = b"invalid source!".to_vec();
        }
        if which == 2 {
            bad.map_identity.version.clear();
        }
        let bytes = bad.bytes();
        let map = Map::parse(&bytes).unwrap();
        let mut budget = bad.budget(&map);
        if which == 3 {
            budget.units_bytes -= 1;
        }
        assert!(
            app.replace(bad.inputs(&map, "bad candidate"), budget)
                .is_err()
        );
        assert!(std::ptr::eq(old, app.context().unwrap()));
        assert_eq!(app.context().unwrap().map_digest.hash, digest);
        assert_eq!(bad.catalog.1.get(), 0);
    }
}
#[cfg_attr(test, test)]
pub fn absent_malformed_and_failed_extraction_remain_distinct() {
    let mut missing = Fixture::new("1", 1.0);
    missing.files.retain(|(n, _)| *n != "war3map.w3i");
    let bytes = missing.bytes();
    let map = Map::parse(&bytes).unwrap();
    let mut app = MapAdmission::default();
    assert!(matches!(
        app.replace(missing.inputs(&map, "missing"), missing.budget(&map)),
        Err(MapAdmissionError::MissingMember {
            member: "war3map.w3i",
            ..
        })
    ));
    let mut malformed = Fixture::new("1", 1.0);
    malformed
        .files
        .iter_mut()
        .find(|(n, _)| *n == "war3map.w3i")
        .unwrap()
        .1 = vec![0];
    let bytes = malformed.bytes();
    let map = Map::parse(&bytes).unwrap();
    assert!(matches!(
        app.replace(malformed.inputs(&map, "malformed"), malformed.budget(&map)),
        Err(MapAdmissionError::Parse {
            member: "war3map.w3i",
            ..
        })
    ));
    let fixture = Fixture::new("1", 1.0);
    let mut bytes = fixture.bytes();
    fixtures::alter_block(&mut bytes, "war3map.w3e", |block| {
        let flags =
            u32::from_le_bytes(block[12..16].try_into().unwrap()) | pcx_mpq::flags::COMPRESS;
        block[12..16].copy_from_slice(&flags.to_le_bytes());
        let packed = u32::from_le_bytes(block[4..8].try_into().unwrap()) - 1;
        block[4..8].copy_from_slice(&packed.to_le_bytes());
    });
    let map = Map::parse(&bytes).unwrap();
    assert!(matches!(
        app.replace(fixture.inputs(&map, "decode failure"), fixture.budget(&map)),
        Err(MapAdmissionError::Read {
            member: "war3map.w3e",
            source: ReadError::ExtractionFailed { .. }
        })
    ));
    assert!(app.context().is_none());
}
#[cfg_attr(test, test)]
pub fn every_exact_input_ceiling_and_aggregate_accepts_and_one_under_refuses() {
    let fixture = Fixture::new("31", 3.0);
    let bytes = fixture.bytes();
    let map = Map::parse(&bytes).unwrap();
    let exact = fixture.budget(&map);
    let mut app = MapAdmission::default();
    app.replace(fixture.inputs(&map, "exact"), exact).unwrap();
    let digest = app.context().unwrap().map_digest.hash;
    for resource in 0..13 {
        let mut budget = exact;
        match resource {
            0 => budget.terrain_bytes -= 1,
            1 => budget.pathing_bytes -= 1,
            2 => budget.info_bytes -= 1,
            3 => budget.units_bytes -= 1,
            4 => budget.aggregate_input_bytes -= 1,
            5 => budget.required_fields -= 1,
            6 => budget.scripts.max_archive_bytes -= 1,
            7 => budget.scripts.max_archive_table_bytes -= 1,
            8 => budget.scripts.max_script_bytes -= 1,
            9 => budget.scripts.max_prelude_bytes -= 1,
            10 => budget.scripts.max_aggregate_input_bytes -= 1,
            11 => budget.scripts.max_raw_packed_bytes -= 1,
            12 => budget.scripts.max_decoded_bytes -= 1,
            _ => unreachable!(),
        };
        assert!(
            app.replace(fixture.inputs(&map, "one under"), budget)
                .is_err(),
            "resource {resource}"
        );
        assert_eq!(app.context().unwrap().map_digest.hash, digest);
    }
}
#[cfg_attr(test, test)]
pub fn forged_decoded_sizes_are_refused_before_decoding_or_catalog_work() {
    let fixture = Fixture::new("1", 1.0);
    let mut bytes = fixture.bytes();
    fixtures::alter_block(&mut bytes, "war3map.w3e", |block| {
        block[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        let flags =
            u32::from_le_bytes(block[12..16].try_into().unwrap()) | pcx_mpq::flags::COMPRESS;
        block[12..16].copy_from_slice(&flags.to_le_bytes());
    });
    let map = Map::parse(&bytes).unwrap();
    let budget = fixture.budget(&map);
    let mut app = MapAdmission::default();
    assert!(
        matches!(app.replace(fixture.inputs(&map,"forged"),budget),Err(MapAdmissionError::Read { member: "war3map.w3e", source: ReadError::BudgetExceeded {member:"war3map.w3e",declared_bytes,max_bytes,..}}) if declared_bytes==u32::MAX as usize && max_bytes==budget.terrain_bytes)
    );
    assert_eq!(fixture.catalog.1.get(), 0);
    assert!(app.context().is_none());
}
#[cfg_attr(test, test)]
pub fn malformed_archive_or_grid_and_missing_script_cannot_publish() {
    let fixture = Fixture::new("1", 1.0);
    let mut bytes = fixture.bytes();
    let offset = Map::parse(&bytes).unwrap().header.archive_offset;
    bytes[offset + 28..offset + 32].copy_from_slice(&u32::MAX.to_le_bytes());
    let map = Map::parse(&bytes).unwrap();
    let mut budget = fixture.budget(&map);
    budget.scripts.max_archive_table_bytes = usize::MAX;
    let mut app = MapAdmission::default();
    assert!(
        app.replace(fixture.inputs(&map, "bad table layout"), budget)
            .is_err()
    );
    assert_eq!(fixture.catalog.1.get(), 0);
    assert!(app.context().is_none());
    let mut nonaligned = Fixture::new("1", 1.0);
    nonaligned
        .files
        .iter_mut()
        .find(|(n, _)| *n == "war3map.wpm")
        .unwrap()
        .1 = fixtures::pathing_bytes(3, 2);
    let bytes = nonaligned.bytes();
    let map = Map::parse(&bytes).unwrap();
    assert!(matches!(
        app.replace(nonaligned.inputs(&map, "grid"), nonaligned.budget(&map)),
        Err(MapAdmissionError::Alignment { .. })
    ));
    assert_eq!(nonaligned.catalog.1.get(), 0);
    let bytes = fixtures::map_bytes(
        Some(&fixtures::terrain_bytes(2, 1)),
        &fixtures::pathing_bytes(4, 2),
    );
    let map = Map::parse(&bytes).unwrap();
    let mut budget = fixture.budget(&map);
    budget.scripts.max_archive_bytes = map.archive().len();
    assert!(matches!(
        app.replace(fixture.inputs(&map, "no compiled script"), budget),
        Err(MapAdmissionError::Input(
            MapScriptImportError::MissingScriptMember {
                member: "war3map.j",
                ..
            }
        ))
    ));
}
#[cfg_attr(test, test)]
pub fn optional_wts_absence_and_present_failure_keep_their_existing_contract() {
    let mut fixture = Fixture::new("1", 1.0);
    let bytes = fixture.bytes();
    let map = Map::parse(&bytes).unwrap();
    let mut app = MapAdmission::default();
    app.replace(fixture.inputs(&map, "no WTS"), fixture.budget(&map))
        .unwrap();
    assert_eq!(app.context().unwrap().scripts.digests.len(), 3);
    let digest = app.context().unwrap().map_digest.hash;
    fixture.files.push(("war3map.wts", b"STRING bad".to_vec()));
    let bytes = fixture.bytes();
    let map = Map::parse(&bytes).unwrap();
    assert!(matches!(
        app.replace(
            fixture.inputs(&map, "present bad WTS"),
            fixture.budget(&map)
        ),
        Err(MapAdmissionError::Input(
            MapScriptImportError::MalformedWts {
                member: "war3map.wts",
                ..
            }
        ))
    ));
    assert_eq!(app.context().unwrap().map_digest.hash, digest);
}
